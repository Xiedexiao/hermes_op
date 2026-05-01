//! Skill evolution inbox commands.
//!
//! Stores SkillClaw-style candidate improvements as auditable local records.
//! This slice deliberately does not mutate SKILL.md files automatically.

use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::backend::{AppError, AppResult, Database};
use crate::commands::sessions::{SessionListRequest, session_list_recent_evidence_for_db};
use crate::commands::skills::{SkillListItem, list_skills_from_db};

const DEFAULT_CANDIDATE_LIMIT: usize = 25;
const MAX_CANDIDATE_LIMIT: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillEvolutionSourceRef {
    pub kind: String,
    pub id: String,
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillEvolutionCandidate {
    pub id: String,
    pub target_skill_name: Option<String>,
    pub action: String,
    pub status: String,
    pub evidence_summary: String,
    pub recommended_change: String,
    pub confidence: String,
    #[serde(default)]
    pub source_refs: Vec<SkillEvolutionSourceRef>,
    pub validation_notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SkillEvolutionCandidateListRequest {
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SkillEvolutionCandidateGenerateRequest {
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillEvolutionCandidateCreateRequest {
    pub target_skill_name: Option<String>,
    pub action: String,
    pub evidence_summary: String,
    pub recommended_change: String,
    pub confidence: String,
    #[serde(default)]
    pub source_refs: Vec<SkillEvolutionSourceRef>,
    pub validation_notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillEvolutionCandidateSetStatusRequest {
    pub id: String,
    pub status: String,
    pub validation_notes: Option<String>,
}

#[derive(Debug)]
struct SkillEvolutionCandidateRow {
    id: String,
    target_skill_name: Option<String>,
    action: String,
    status: String,
    evidence_summary: String,
    recommended_change: String,
    confidence: String,
    source_refs_json: String,
    validation_notes: Option<String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug)]
struct SkillEvolutionCandidateDraft {
    target_skill_name: Option<String>,
    action: String,
    evidence_summary: String,
    recommended_change: String,
    confidence: String,
    source_refs: Vec<SkillEvolutionSourceRef>,
    validation_notes: Option<String>,
}

#[tauri::command]
pub fn skill_evolution_candidate_list(
    db: State<'_, Database>,
    request: SkillEvolutionCandidateListRequest,
) -> Result<Vec<SkillEvolutionCandidate>, AppError> {
    skill_evolution_candidate_list_for_db(db.inner(), request)
}

#[tauri::command]
pub fn skill_evolution_candidate_create(
    db: State<'_, Database>,
    request: SkillEvolutionCandidateCreateRequest,
) -> Result<SkillEvolutionCandidate, AppError> {
    skill_evolution_candidate_create_for_db(db.inner(), request)
}

#[tauri::command]
pub fn skill_evolution_candidate_generate(
    db: State<'_, Database>,
    request: SkillEvolutionCandidateGenerateRequest,
) -> Result<Vec<SkillEvolutionCandidate>, AppError> {
    skill_evolution_candidate_generate_for_db(db.inner(), request)
}

#[tauri::command]
pub fn skill_evolution_candidate_set_status(
    db: State<'_, Database>,
    request: SkillEvolutionCandidateSetStatusRequest,
) -> Result<SkillEvolutionCandidate, AppError> {
    skill_evolution_candidate_set_status_for_db(db.inner(), request)
}

pub fn skill_evolution_candidate_list_for_db(
    db: &Database,
    request: SkillEvolutionCandidateListRequest,
) -> AppResult<Vec<SkillEvolutionCandidate>> {
    let limit = resolve_candidate_limit(request.limit) as i64;
    let status = normalize_optional_status(request.status)?;

    let rows = if let Some(status) = status {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, target_skill_name, action, status, evidence_summary,
                    recommended_change, confidence, source_refs_json, validation_notes,
                    created_at, updated_at
                 FROM skill_evolution_candidates
                 WHERE status = ?
                 ORDER BY updated_at DESC, created_at DESC
                 LIMIT ?",
            )?;
            let rows = stmt.query_map(params![status, limit], row_to_candidate_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })?
    } else {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, target_skill_name, action, status, evidence_summary,
                    recommended_change, confidence, source_refs_json, validation_notes,
                    created_at, updated_at
                 FROM skill_evolution_candidates
                 ORDER BY updated_at DESC, created_at DESC
                 LIMIT ?",
            )?;
            let rows = stmt.query_map(params![limit], row_to_candidate_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })?
    };

    rows.into_iter().map(row_into_candidate).collect()
}

pub fn skill_evolution_candidate_create_for_db(
    db: &Database,
    request: SkillEvolutionCandidateCreateRequest,
) -> AppResult<SkillEvolutionCandidate> {
    let action = normalize_action(request.action)?;
    let target_skill_name = normalize_optional_text(request.target_skill_name);
    if action == "refine" && target_skill_name.is_none() {
        return Err(AppError::validation(
            "target skill name is required for refine candidates",
        ));
    }

    let evidence_summary = normalize_required_text("evidence summary", request.evidence_summary)?;
    let recommended_change =
        normalize_required_text("recommended change", request.recommended_change)?;
    let confidence = normalize_confidence(request.confidence)?;
    let source_refs = normalize_source_refs(request.source_refs)?;
    let source_refs_json =
        serde_json::to_string(&source_refs).map_err(AppError::from_json_error)?;
    let validation_notes = normalize_optional_text(request.validation_notes);
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().to_rfc3339();

    db.with_connection(|conn| {
        conn.execute(
            "INSERT INTO skill_evolution_candidates (
                id, target_skill_name, action, status, evidence_summary, recommended_change,
                confidence, source_refs_json, validation_notes, created_at, updated_at
            ) VALUES (?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?, ?)",
            params![
                id,
                target_skill_name,
                action,
                evidence_summary,
                recommended_change,
                confidence,
                source_refs_json,
                validation_notes,
                now,
                now
            ],
        )
    })?;

    get_candidate_by_id(db, &id)?
        .ok_or_else(|| AppError::storage("created skill evolution candidate was not found"))
}

pub fn skill_evolution_candidate_generate_for_db(
    db: &Database,
    request: SkillEvolutionCandidateGenerateRequest,
) -> AppResult<Vec<SkillEvolutionCandidate>> {
    let available_skills = list_skills_from_db(db).unwrap_or_default();
    skill_evolution_candidate_generate_for_db_with_skills(db, request, &available_skills)
}

fn skill_evolution_candidate_generate_for_db_with_skills(
    db: &Database,
    request: SkillEvolutionCandidateGenerateRequest,
    available_skills: &[SkillListItem],
) -> AppResult<Vec<SkillEvolutionCandidate>> {
    let limit = resolve_candidate_limit(request.limit);
    let mut seen_primary_refs = load_existing_primary_source_refs(db)?;
    let mut drafts = collect_failed_run_drafts(db, limit)?;
    drafts.extend(collect_failed_execution_step_drafts(db, limit)?);
    drafts.extend(collect_failed_event_drafts(db, limit)?);
    drafts.extend(collect_session_signal_drafts(db, limit)?);
    let drafts = drafts
        .into_iter()
        .map(|draft| apply_skill_attribution_to_draft(draft, available_skills))
        .collect::<Vec<_>>();

    let mut created = Vec::new();
    for draft in drafts {
        let Some(primary_ref_key) = primary_source_ref_key(&draft.source_refs) else {
            continue;
        };
        if seen_primary_refs.contains(&primary_ref_key) {
            continue;
        }

        let candidate = skill_evolution_candidate_create_for_db(
            db,
            SkillEvolutionCandidateCreateRequest {
                target_skill_name: draft.target_skill_name,
                action: draft.action,
                evidence_summary: draft.evidence_summary,
                recommended_change: draft.recommended_change,
                confidence: draft.confidence,
                source_refs: draft.source_refs,
                validation_notes: draft.validation_notes,
            },
        )?;
        seen_primary_refs.insert(primary_ref_key);
        created.push(candidate);

        if created.len() >= limit {
            break;
        }
    }

    Ok(created)
}

pub fn skill_evolution_candidate_set_status_for_db(
    db: &Database,
    request: SkillEvolutionCandidateSetStatusRequest,
) -> AppResult<SkillEvolutionCandidate> {
    let id = normalize_required_text("candidate id", request.id)?;
    let status = normalize_status(request.status)?;
    let validation_notes = normalize_optional_text(request.validation_notes);
    let now = Utc::now().to_rfc3339();

    let changed = db.with_connection(|conn| {
        conn.execute(
            "UPDATE skill_evolution_candidates
             SET status = ?, validation_notes = ?, updated_at = ?
             WHERE id = ?",
            params![status, validation_notes, now, id],
        )
    })?;

    if changed == 0 {
        return Err(AppError::validation("skill evolution candidate not found"));
    }

    get_candidate_by_id(db, &id)?
        .ok_or_else(|| AppError::storage("updated skill evolution candidate was not found"))
}

fn get_candidate_by_id(db: &Database, id: &str) -> AppResult<Option<SkillEvolutionCandidate>> {
    let row = db.with_connection(|conn| {
        conn.query_row(
            "SELECT id, target_skill_name, action, status, evidence_summary,
                recommended_change, confidence, source_refs_json, validation_notes,
                created_at, updated_at
             FROM skill_evolution_candidates
             WHERE id = ?",
            params![id],
            row_to_candidate_row,
        )
        .optional()
    })?;

    row.map(row_into_candidate).transpose()
}

fn load_existing_primary_source_refs(
    db: &Database,
) -> AppResult<std::collections::HashSet<String>> {
    let source_refs_json = db.with_connection(|conn| {
        let mut stmt = conn.prepare("SELECT source_refs_json FROM skill_evolution_candidates")?;
        let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()
    })?;

    let mut keys = std::collections::HashSet::new();
    for source_refs_json in source_refs_json {
        let source_refs: Vec<SkillEvolutionSourceRef> =
            serde_json::from_str(&source_refs_json).map_err(AppError::from_json_error)?;
        if let Some(key) = primary_source_ref_key(&source_refs) {
            keys.insert(key);
        }
    }
    Ok(keys)
}

fn primary_source_ref_key(source_refs: &[SkillEvolutionSourceRef]) -> Option<String> {
    source_refs
        .first()
        .map(|source_ref| format!("{}:{}", source_ref.kind, source_ref.id))
}

fn collect_failed_run_drafts(
    db: &Database,
    limit: usize,
) -> AppResult<Vec<SkillEvolutionCandidateDraft>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT runs.id, missions.title, runs.type, runs.summary, runs.error_message
             FROM runs
             INNER JOIN missions ON missions.id = runs.mission_id
             WHERE runs.status = 'failed'
             ORDER BY COALESCE(runs.finished_at, runs.started_at, missions.last_activity_at) DESC, runs.rowid DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    })?
    .into_iter()
    .map(|(run_id, mission_title, run_type, summary, error_message)| {
        let summary_fragment = summary.unwrap_or_else(|| "No summary recorded.".to_string());
        let has_error_message = error_message.is_some();
        let error_fragment =
            error_message.unwrap_or_else(|| "No explicit error recorded.".to_string());
        Ok(SkillEvolutionCandidateDraft {
            target_skill_name: None,
            action: "create".to_string(),
            evidence_summary: format!(
                "Mission \"{}\" has a failed {} run. Summary: {} Error: {}",
                mission_title, run_type, summary_fragment, error_fragment
            ),
            recommended_change: format!(
                "Create a reusable recovery or preflight skill for {} runs so similar failures are caught before retry.",
                run_type
            ),
            confidence: if has_error_message {
                "high".to_string()
            } else {
                "medium".to_string()
            },
            source_refs: vec![SkillEvolutionSourceRef {
                kind: "run".to_string(),
                id: run_id,
                title: Some(mission_title),
            }],
            validation_notes: Some(
                "Auto-generated from a failed run signal; review before applying to SKILL.md."
                    .to_string(),
            ),
        })
    })
    .collect()
}

fn collect_failed_execution_step_drafts(
    db: &Database,
    limit: usize,
) -> AppResult<Vec<SkillEvolutionCandidateDraft>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT execution_steps.id, execution_steps.run_id, missions.title,
                    execution_steps.title, execution_steps.mode, execution_steps.risk_level,
                    execution_steps.status, execution_steps.output_summary
             FROM execution_steps
             INNER JOIN missions ON missions.id = execution_steps.mission_id
             WHERE execution_steps.status = 'failed'
                OR (execution_steps.status = 'awaiting_approval' AND execution_steps.risk_level = 'high')
             ORDER BY execution_steps.updated_at DESC, execution_steps.rowid DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, Option<String>>(7)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    })?
    .into_iter()
    .map(
        |(step_id, run_id, mission_title, step_title, mode, risk_level, status, output_summary)| {
            let output_fragment =
                output_summary.unwrap_or_else(|| "No output summary recorded.".to_string());
            Ok(SkillEvolutionCandidateDraft {
                target_skill_name: None,
                action: "create".to_string(),
                evidence_summary: format!(
                    "Mission \"{}\" has a {} {}-risk {} step \"{}\". Output: {}",
                    mission_title, status, risk_level, mode, step_title, output_fragment
                ),
                recommended_change: format!(
                    "Create a reusable {} execution checklist that adds preflight checks, dependency validation, and safer recovery steps for {}-risk work.",
                    mode, risk_level
                ),
                confidence: if risk_level == "high" || status == "failed" {
                    "high".to_string()
                } else {
                    "medium".to_string()
                },
                source_refs: vec![
                    SkillEvolutionSourceRef {
                        kind: "execution_step".to_string(),
                        id: step_id,
                        title: Some(step_title),
                    },
                    SkillEvolutionSourceRef {
                        kind: "run".to_string(),
                        id: run_id,
                        title: Some(mission_title),
                    },
                ],
                validation_notes: Some(
                    "Auto-generated from execution-step failure/approval signals; review before applying."
                        .to_string(),
                ),
            })
        },
    )
    .collect()
}

fn collect_failed_event_drafts(
    db: &Database,
    limit: usize,
) -> AppResult<Vec<SkillEvolutionCandidateDraft>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT run_events.id, run_events.run_id, missions.title, run_events.event_type, run_events.message
             FROM run_events
             INNER JOIN missions ON missions.id = run_events.mission_id
             WHERE run_events.event_type LIKE '%failed%'
             ORDER BY run_events.created_at DESC, run_events.rowid DESC
             LIMIT ?1",
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>()
    })?
    .into_iter()
    .map(|(event_id, run_id, mission_title, event_type, message)| {
        Ok(SkillEvolutionCandidateDraft {
            target_skill_name: None,
            action: "create".to_string(),
            evidence_summary: format!(
                "Mission \"{}\" emitted runtime event \"{}\": {}",
                mission_title, event_type, message
            ),
            recommended_change:
                "Capture this failure-handling sequence in a reusable troubleshooting or recovery skill candidate."
                    .to_string(),
            confidence: "medium".to_string(),
            source_refs: vec![
                SkillEvolutionSourceRef {
                    kind: "run_event".to_string(),
                    id: event_id,
                    title: Some(event_type),
                },
                SkillEvolutionSourceRef {
                    kind: "run".to_string(),
                    id: run_id,
                    title: Some(mission_title),
                },
            ],
            validation_notes: Some(
                "Auto-generated from a failed runtime event; review before applying."
                    .to_string(),
            ),
        })
    })
    .collect()
}

fn collect_session_signal_drafts(
    db: &Database,
    limit: usize,
) -> AppResult<Vec<SkillEvolutionCandidateDraft>> {
    let drafts = session_list_recent_evidence_for_db(
        db,
        Some(SessionListRequest { limit: Some(limit) }),
    )?
    .into_iter()
    .filter_map(|session| {
            let session_id = session.id;
            let source = session.source.as_str().to_string();
            let title = session.title;
            let model_name = session.model_name;
            let parent_session_id = session.parent_session_id;
            let updated_at = session.updated_at;
            let normalized_title = title.to_lowercase();
            if !has_session_signal_keywords(&normalized_title) {
                return None;
            }

            let model_fragment = model_name.unwrap_or_else(|| "unknown model".to_string());
            let parent_fragment = if let Some(parent_session_id) = parent_session_id {
                format!(" Linked to prior session {}.", parent_session_id)
            } else {
                String::new()
            };

            Some(SkillEvolutionCandidateDraft {
                target_skill_name: None,
                action: "create".to_string(),
                evidence_summary: format!(
                    "Recent {} session \"{}\" suggests a recovery or failure pattern. Model: {}. Updated at {}.{}",
                    source, title, model_fragment, updated_at, parent_fragment
                ),
                recommended_change: format!(
                    "Capture a reusable recovery, retry, or troubleshooting procedure for {} sessions that look like \"{}\".",
                    source, title
                ),
                confidence: if parent_fragment.is_empty() {
                    "low".to_string()
                } else {
                    "medium".to_string()
                },
                source_refs: vec![SkillEvolutionSourceRef {
                    kind: "session".to_string(),
                    id: session_id,
                    title: Some(title),
                }],
                validation_notes: Some(
                    "Auto-generated from session title heuristics; review carefully before applying."
                        .to_string(),
                ),
            })
        })
    .collect();

    Ok(drafts)
}

fn has_session_signal_keywords(normalized_title: &str) -> bool {
    const SIGNAL_KEYWORDS: &[&str] = &[
        "error", "errors", "fail", "failed", "failure", "retry", "recover", "resume", "fix",
        "debug", "blocked", "stuck", "报错", "失败", "错误", "重试", "恢复", "修复", "调试",
        "卡住", "阻塞",
    ];

    SIGNAL_KEYWORDS
        .iter()
        .any(|keyword| normalized_title.contains(keyword))
}

fn apply_skill_attribution_to_draft(
    mut draft: SkillEvolutionCandidateDraft,
    available_skills: &[SkillListItem],
) -> SkillEvolutionCandidateDraft {
    if draft.target_skill_name.is_some() {
        return draft;
    }

    let mut attribution_text = draft.evidence_summary.clone();
    attribution_text.push('\n');
    attribution_text.push_str(&draft.recommended_change);
    for source_ref in &draft.source_refs {
        attribution_text.push('\n');
        attribution_text.push_str(&source_ref.kind);
        attribution_text.push(' ');
        attribution_text.push_str(&source_ref.id);
        if let Some(title) = &source_ref.title {
            attribution_text.push(' ');
            attribution_text.push_str(title);
        }
    }

    if let Some(target_skill_name) = infer_target_skill_name(available_skills, &attribution_text) {
        draft.target_skill_name = Some(target_skill_name.clone());
        draft.action = "refine".to_string();
        draft.recommended_change = format!(
            "Refine existing skill {}. {}",
            target_skill_name, draft.recommended_change
        );
    }

    draft
}

fn infer_target_skill_name(skills: &[SkillListItem], text: &str) -> Option<String> {
    let haystack = text.trim().to_lowercase();
    if haystack.is_empty() {
        return None;
    }

    for skill in skills.iter().filter(|skill| skill.enabled) {
        let normalized_name = skill.name.trim().to_lowercase();
        let normalized_display_name = skill.display_name.trim().to_lowercase();
        if !normalized_name.is_empty() && haystack.contains(&normalized_name) {
            return Some(skill.name.clone());
        }
        if !normalized_display_name.is_empty() && haystack.contains(&normalized_display_name) {
            return Some(skill.name.clone());
        }
    }

    let mut best_match: Option<(usize, String)> = None;
    for skill in skills.iter().filter(|skill| skill.enabled) {
        let tokens = skill_match_tokens(skill);
        let score = tokens
            .iter()
            .filter(|token| haystack.contains(token.as_str()))
            .count();
        if score >= 2 {
            match &best_match {
                Some((best_score, _)) if *best_score >= score => {}
                _ => best_match = Some((score, skill.name.clone())),
            }
        }
    }

    best_match.map(|(_, name)| name)
}

fn skill_match_tokens(skill: &SkillListItem) -> Vec<String> {
    let mut tokens = Vec::new();
    for source in [&skill.name, &skill.display_name] {
        let normalized = source.to_lowercase();
        for token in normalized
            .split(|ch: char| !ch.is_ascii_alphanumeric())
            .filter(|token| token.len() >= 4)
        {
            if !tokens.iter().any(|existing| existing == token) {
                tokens.push(token.to_string());
            }
        }
    }
    tokens
}

fn row_to_candidate_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<SkillEvolutionCandidateRow> {
    Ok(SkillEvolutionCandidateRow {
        id: row.get(0)?,
        target_skill_name: row.get(1)?,
        action: row.get(2)?,
        status: row.get(3)?,
        evidence_summary: row.get(4)?,
        recommended_change: row.get(5)?,
        confidence: row.get(6)?,
        source_refs_json: row.get(7)?,
        validation_notes: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn row_into_candidate(row: SkillEvolutionCandidateRow) -> AppResult<SkillEvolutionCandidate> {
    let source_refs =
        serde_json::from_str(&row.source_refs_json).map_err(AppError::from_json_error)?;
    Ok(SkillEvolutionCandidate {
        id: row.id,
        target_skill_name: row.target_skill_name,
        action: row.action,
        status: row.status,
        evidence_summary: row.evidence_summary,
        recommended_change: row.recommended_change,
        confidence: row.confidence,
        source_refs,
        validation_notes: row.validation_notes,
        created_at: row.created_at,
        updated_at: row.updated_at,
    })
}

fn resolve_candidate_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_CANDIDATE_LIMIT)
        .clamp(1, MAX_CANDIDATE_LIMIT)
}

fn normalize_optional_status(status: Option<String>) -> AppResult<Option<String>> {
    status.map(normalize_status).transpose()
}

fn normalize_action(action: String) -> AppResult<String> {
    let action = normalize_required_text("action", action)?;
    match action.as_str() {
        "refine" | "create" | "skip" => Ok(action),
        _ => Err(AppError::validation(
            "action must be one of refine, create, or skip",
        )),
    }
}

fn normalize_status(status: String) -> AppResult<String> {
    let status = normalize_required_text("status", status)?;
    match status.as_str() {
        "pending" | "accepted" | "rejected" => Ok(status),
        _ => Err(AppError::validation(
            "status must be one of pending, accepted, or rejected",
        )),
    }
}

fn normalize_confidence(confidence: String) -> AppResult<String> {
    let confidence = normalize_required_text("confidence", confidence)?;
    match confidence.as_str() {
        "low" | "medium" | "high" => Ok(confidence),
        _ => Err(AppError::validation(
            "confidence must be one of low, medium, or high",
        )),
    }
}

fn normalize_required_text(field: &str, value: String) -> AppResult<String> {
    let value = value.trim().to_string();
    if value.is_empty() {
        return Err(AppError::validation(format!("{} is required", field)));
    }
    Ok(value)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_source_refs(
    source_refs: Vec<SkillEvolutionSourceRef>,
) -> AppResult<Vec<SkillEvolutionSourceRef>> {
    source_refs
        .into_iter()
        .map(|source_ref| {
            Ok(SkillEvolutionSourceRef {
                kind: normalize_required_text("source ref kind", source_ref.kind)?,
                id: normalize_required_text("source ref id", source_ref.id)?,
                title: normalize_optional_text(source_ref.title),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        SkillEvolutionCandidateCreateRequest, SkillEvolutionCandidateDraft,
        SkillEvolutionCandidateGenerateRequest, SkillEvolutionCandidateListRequest,
        SkillEvolutionCandidateSetStatusRequest, SkillEvolutionSourceRef,
        apply_skill_attribution_to_draft, infer_target_skill_name,
        skill_evolution_candidate_create_for_db,
        skill_evolution_candidate_generate_for_db_with_skills,
        skill_evolution_candidate_list_for_db, skill_evolution_candidate_set_status_for_db,
    };
    use crate::backend::Database;
    use crate::commands::skills::SkillListItem;
    use chrono::Utc;

    fn seed_runtime_failure_signals(db: &Database) {
        let now = Utc::now().to_rfc3339();
        let constraints_json =
            serde_json::to_string(&vec!["readonly"]).expect("serialize constraints");
        let success_json =
            serde_json::to_string(&vec!["recover from failure"]).expect("serialize success");

        db.execute(
            "INSERT INTO missions (
                id, title, goal, constraints_json, success_criteria_json,
                status, priority, pinned, created_at, updated_at, last_activity_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"mission-auto-1" as &dyn rusqlite::ToSql,
                &"Runtime recovery mission",
                &"Stabilize recurring runtime failures",
                &constraints_json,
                &success_json,
                &"executing",
                &"high",
                &0_i64,
                &now,
                &now,
                &now,
            ],
        )
        .expect("mission should seed");

        db.execute(
            "INSERT INTO runs (
                id, mission_id, type, status, started_at, finished_at, summary, error_message
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"run-auto-1" as &dyn rusqlite::ToSql,
                &"mission-auto-1",
                &"execution",
                &"failed",
                &now,
                &now,
                &"CLI recovery failed after multiple retries",
                &"missing output directory and broken command preflight",
            ],
        )
        .expect("failed run should seed");

        db.execute(
            "INSERT INTO execution_steps (
                id, mission_id, run_id, title, mode, risk_level, status,
                input_payload, output_summary, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"step-auto-1" as &dyn rusqlite::ToSql,
                &"mission-auto-1",
                &"run-auto-1",
                &"Repair missing output directory",
                &"cli",
                &"high",
                &"failed",
                &"{\"command\":\"mkdir -p /tmp_workspace/out && ./run.sh\"}",
                &"Command exited with missing dependency",
                &now,
                &now,
            ],
        )
        .expect("failed step should seed");

        db.execute(
            "INSERT INTO run_events (
                id, run_id, mission_id, event_type, message, payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            &[
                &"event-auto-1" as &dyn rusqlite::ToSql,
                &"run-auto-1",
                &"mission-auto-1",
                &"step_failed",
                &"Step failed because the output directory did not exist",
                &Some("{\"step_id\":\"step-auto-1\"}".to_string()),
                &now,
            ],
        )
        .expect("failed event should seed");
    }

    fn seed_session_failure_signals(db: &Database) {
        let now = Utc::now().to_rfc3339();
        db.execute(
            "INSERT INTO sessions (
                id, source, title, model_name, parent_session_id, started_at, updated_at, ended_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"session-auto-1" as &dyn rusqlite::ToSql,
                &"desktop",
                &"frontend-design retry after contrast error",
                &Some("gpt-5.4".to_string()),
                &Some("session-parent-1".to_string()),
                &now,
                &now,
                &None::<String>,
            ],
        )
        .expect("session signal should seed");
    }

    fn sample_request() -> SkillEvolutionCandidateCreateRequest {
        SkillEvolutionCandidateCreateRequest {
            target_skill_name: Some("frontend-design".to_string()),
            action: "refine".to_string(),
            evidence_summary: "Two recent UI sessions repeated the same contrast bug.".to_string(),
            recommended_change: "Add a short contrast preflight before final screenshots."
                .to_string(),
            confidence: "medium".to_string(),
            source_refs: vec![SkillEvolutionSourceRef {
                kind: "session".to_string(),
                id: "session-1".to_string(),
                title: Some("Landing page review".to_string()),
            }],
            validation_notes: None,
        }
    }

    #[test]
    fn create_and_list_candidate_round_trip() {
        let db = Database::in_memory().expect("database should initialize");

        let created = skill_evolution_candidate_create_for_db(&db, sample_request())
            .expect("candidate should create");
        let candidates = skill_evolution_candidate_list_for_db(
            &db,
            SkillEvolutionCandidateListRequest {
                status: Some("pending".to_string()),
                limit: Some(10),
            },
        )
        .expect("candidate list should load");

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, created.id);
        assert_eq!(candidates[0].action, "refine");
        assert_eq!(candidates[0].status, "pending");
        assert_eq!(candidates[0].source_refs[0].kind, "session");
    }

    #[test]
    fn set_status_updates_review_fields() {
        let db = Database::in_memory().expect("database should initialize");
        let created = skill_evolution_candidate_create_for_db(&db, sample_request())
            .expect("candidate should create");

        let accepted = skill_evolution_candidate_set_status_for_db(
            &db,
            SkillEvolutionCandidateSetStatusRequest {
                id: created.id.clone(),
                status: "accepted".to_string(),
                validation_notes: Some(
                    "Verified by reviewer before applying to skill history.".to_string(),
                ),
            },
        )
        .expect("candidate status should update");

        assert_eq!(accepted.status, "accepted");
        assert_eq!(
            accepted.validation_notes.as_deref(),
            Some("Verified by reviewer before applying to skill history.")
        );
        assert!(accepted.updated_at >= created.updated_at);
    }

    #[test]
    fn validates_action_status_confidence_and_target_skill() {
        let db = Database::in_memory().expect("database should initialize");

        let mut invalid_action = sample_request();
        invalid_action.action = "rewrite-everything".to_string();
        assert!(skill_evolution_candidate_create_for_db(&db, invalid_action).is_err());

        let mut invalid_confidence = sample_request();
        invalid_confidence.confidence = "certain".to_string();
        assert!(skill_evolution_candidate_create_for_db(&db, invalid_confidence).is_err());

        let mut missing_target = sample_request();
        missing_target.target_skill_name = None;
        assert!(skill_evolution_candidate_create_for_db(&db, missing_target).is_err());

        let created = skill_evolution_candidate_create_for_db(&db, sample_request())
            .expect("candidate should create");
        assert!(
            skill_evolution_candidate_set_status_for_db(
                &db,
                SkillEvolutionCandidateSetStatusRequest {
                    id: created.id,
                    status: "maybe".to_string(),
                    validation_notes: None,
                },
            )
            .is_err()
        );
    }

    #[test]
    fn generate_candidates_from_runtime_failure_signals() {
        let db = Database::in_memory().expect("database should initialize");
        seed_runtime_failure_signals(&db);

        let generated = skill_evolution_candidate_generate_for_db_with_skills(
            &db,
            SkillEvolutionCandidateGenerateRequest { limit: Some(10) },
            &[],
        )
        .expect("candidates should generate");

        assert_eq!(generated.len(), 3);
        assert!(
            generated
                .iter()
                .all(|candidate| candidate.action == "create")
        );
        assert!(
            generated
                .iter()
                .all(|candidate| candidate.status == "pending")
        );
        assert!(generated.iter().any(|candidate| {
            candidate
                .source_refs
                .iter()
                .any(|source_ref| source_ref.kind == "run")
        }));
        assert!(generated.iter().any(|candidate| {
            candidate
                .source_refs
                .iter()
                .any(|source_ref| source_ref.kind == "execution_step")
        }));
        assert!(generated.iter().any(|candidate| {
            candidate
                .source_refs
                .iter()
                .any(|source_ref| source_ref.kind == "run_event")
        }));
    }

    #[test]
    fn generate_candidates_skips_existing_source_refs() {
        let db = Database::in_memory().expect("database should initialize");
        seed_runtime_failure_signals(&db);

        let first = skill_evolution_candidate_generate_for_db_with_skills(
            &db,
            SkillEvolutionCandidateGenerateRequest { limit: Some(10) },
            &[],
        )
        .expect("first generation should succeed");
        let second = skill_evolution_candidate_generate_for_db_with_skills(
            &db,
            SkillEvolutionCandidateGenerateRequest { limit: Some(10) },
            &[],
        )
        .expect("second generation should succeed");
        let all_candidates = skill_evolution_candidate_list_for_db(
            &db,
            SkillEvolutionCandidateListRequest {
                status: None,
                limit: Some(20),
            },
        )
        .expect("candidate list should load");

        assert_eq!(first.len(), 3);
        assert!(second.is_empty());
        assert_eq!(all_candidates.len(), 3);
    }

    #[test]
    fn infer_target_skill_name_prefers_exact_and_multi_token_matches() {
        let skills = vec![
            SkillListItem {
                name: "frontend-design".to_string(),
                display_name: "Frontend Design".to_string(),
                source: "test".to_string(),
                path: "/tmp/frontend-design/SKILL.md".to_string(),
                enabled: true,
            },
            SkillListItem {
                name: "security-review".to_string(),
                display_name: "Security Review".to_string(),
                source: "test".to_string(),
                path: "/tmp/security-review/SKILL.md".to_string(),
                enabled: true,
            },
        ];

        let exact = infer_target_skill_name(
            &skills,
            "frontend-design repeatedly failed to validate color contrast on the landing page",
        );
        let token_match = infer_target_skill_name(
            &skills,
            "The frontend design workflow repeatedly produced low-contrast cards and unclear hierarchy.",
        );
        let weak = infer_target_skill_name(&skills, "Need a better plan");

        assert_eq!(exact.as_deref(), Some("frontend-design"));
        assert_eq!(token_match.as_deref(), Some("frontend-design"));
        assert!(weak.is_none());
    }

    #[test]
    fn apply_skill_attribution_converts_matching_draft_to_refine() {
        let skills = vec![SkillListItem {
            name: "frontend-design".to_string(),
            display_name: "Frontend Design".to_string(),
            source: "test".to_string(),
            path: "/tmp/frontend-design/SKILL.md".to_string(),
            enabled: true,
        }];
        let draft = SkillEvolutionCandidateDraft {
            target_skill_name: None,
            action: "create".to_string(),
            evidence_summary:
                "Frontend design review failed because the landing page contrast was too weak."
                    .to_string(),
            recommended_change:
                "Create a reusable recovery or preflight skill for the UI review flow.".to_string(),
            confidence: "medium".to_string(),
            source_refs: vec![SkillEvolutionSourceRef {
                kind: "run".to_string(),
                id: "run-ui-1".to_string(),
                title: Some("Landing page review".to_string()),
            }],
            validation_notes: None,
        };

        let attributed = apply_skill_attribution_to_draft(draft, &skills);

        assert_eq!(
            attributed.target_skill_name.as_deref(),
            Some("frontend-design")
        );
        assert_eq!(attributed.action, "refine");
        assert!(
            attributed
                .recommended_change
                .contains("Refine existing skill frontend-design")
        );
    }

    #[test]
    fn generate_with_explicit_skill_list_keeps_create_when_no_match_exists() {
        let db = Database::in_memory().expect("database should initialize");
        seed_runtime_failure_signals(&db);

        let generated = skill_evolution_candidate_generate_for_db_with_skills(
            &db,
            SkillEvolutionCandidateGenerateRequest { limit: Some(10) },
            &[],
        )
        .expect("candidates should generate");

        assert_eq!(generated.len(), 3);
        assert!(
            generated
                .iter()
                .all(|candidate| candidate.action == "create")
        );
    }

    #[test]
    fn session_signal_can_generate_refine_candidate_when_title_matches_skill() {
        let db = Database::in_memory().expect("database should initialize");
        seed_session_failure_signals(&db);
        let skills = vec![SkillListItem {
            name: "frontend-design".to_string(),
            display_name: "Frontend Design".to_string(),
            source: "test".to_string(),
            path: "/tmp/frontend-design/SKILL.md".to_string(),
            enabled: true,
        }];

        let generated = skill_evolution_candidate_generate_for_db_with_skills(
            &db,
            SkillEvolutionCandidateGenerateRequest { limit: Some(10) },
            &skills,
        )
        .expect("session-based candidates should generate");

        assert!(generated.iter().any(|candidate| {
            candidate
                .source_refs
                .iter()
                .any(|source_ref| source_ref.kind == "session")
                && candidate.action == "refine"
                && candidate.target_skill_name.as_deref() == Some("frontend-design")
        }));
    }
}
