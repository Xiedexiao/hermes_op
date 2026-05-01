//! Playbook / growth suggestion synthesis for missions.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use tauri::State;

use crate::backend::{AppError, Database};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaybookRequest {
    pub mission_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissionPlaybook {
    pub mission_id: String,
    pub mission_title: String,
    pub generated_at: String,
    pub summary: String,
    #[serde(default)]
    pub suggestions: Vec<PlaybookSuggestion>,
    #[serde(default)]
    pub evidence_cards: Vec<PlaybookEvidenceCard>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaybookSuggestion {
    pub id: String,
    pub kind: String,
    pub priority: String,
    pub title: String,
    pub rationale: String,
    #[serde(default)]
    pub actions: Vec<String>,
    #[serde(default)]
    pub evidence_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlaybookEvidenceCard {
    pub id: String,
    pub category: String,
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub bullets: Vec<String>,
    #[serde(default)]
    pub source_refs: Vec<String>,
}

#[tauri::command]
pub fn playbook_get(
    db: State<'_, Database>,
    request: PlaybookRequest,
) -> Result<MissionPlaybook, AppError> {
    playbook_get_for_db(db.inner(), &request.mission_id)
}

pub fn playbook_get_for_db(db: &Database, mission_id: &str) -> Result<MissionPlaybook, AppError> {
    let mission_id = mission_id.trim();
    if mission_id.is_empty() {
        return Err(AppError::validation("mission id cannot be empty"));
    }

    let snapshot = load_mission_snapshot(db, mission_id)?;
    let synthesized = synthesize_playbook(&snapshot);

    Ok(MissionPlaybook {
        mission_id: snapshot.mission.id,
        mission_title: snapshot.mission.title,
        generated_at: Utc::now().to_rfc3339(),
        summary: synthesized.summary,
        suggestions: synthesized.suggestions,
        evidence_cards: synthesized.evidence_cards,
    })
}

#[derive(Debug, Clone)]
struct MissionRow {
    id: String,
    title: String,
    goal: String,
}

#[derive(Debug, Clone)]
struct RunRow {
    id: String,
    run_type: String,
    status: String,
    started_at: Option<String>,
    finished_at: Option<String>,
    summary: Option<String>,
    error_message: Option<String>,
}

#[derive(Debug, Clone)]
struct ExecutionStepRow {
    id: String,
    run_id: String,
    title: String,
    status: String,
    risk_level: String,
    output_summary: Option<String>,
    updated_at: String,
}

#[derive(Debug, Clone)]
struct MemoryRecordRow {
    id: String,
    title: String,
    content: String,
    source_type: String,
    importance: String,
    created_at: String,
}

#[derive(Debug, Clone)]
struct ScenarioRunRow {
    id: String,
    created_at: String,
    recommendation: Option<String>,
    selected_label: Option<String>,
    comparison_summary: Option<String>,
    option_labels: Vec<String>,
    variable_changes: Vec<String>,
}

#[derive(Debug, Clone)]
struct RunEventRow {
    id: String,
    run_id: String,
    event_type: String,
    message: String,
    created_at: String,
}

#[derive(Debug, Clone)]
struct MissionSnapshot {
    mission: MissionRow,
    runs: Vec<RunRow>,
    execution_steps: Vec<ExecutionStepRow>,
    memory_records: Vec<MemoryRecordRow>,
    scenario_runs: Vec<ScenarioRunRow>,
    run_events: Vec<RunEventRow>,
}

#[derive(Debug, Clone)]
struct SynthesizedPlaybook {
    summary: String,
    suggestions: Vec<PlaybookSuggestion>,
    evidence_cards: Vec<PlaybookEvidenceCard>,
}

fn load_mission_snapshot(db: &Database, mission_id: &str) -> Result<MissionSnapshot, AppError> {
    let mission = db
        .query_row(
            "SELECT id, title, goal FROM missions WHERE id = ?1",
            &[&mission_id as &dyn rusqlite::ToSql],
            |row| {
                Ok(MissionRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    goal: row.get(2)?,
                })
            },
        )
        .map_err(|error| match error {
            rusqlite::Error::QueryReturnedNoRows => AppError::validation("mission not found"),
            other => AppError::storage(format!("Failed to load mission: {}", other)),
        })?;

    db.with_connection(|conn| {
        let runs = {
            let mut stmt = conn.prepare(
                "SELECT id, type, status, started_at, finished_at, summary, error_message
                 FROM runs
                 WHERE mission_id = ?1
                 ORDER BY COALESCE(finished_at, started_at, '') DESC, rowid DESC",
            )?;
            let rows = stmt.query_map([mission_id], |row| {
                Ok(RunRow {
                    id: row.get(0)?,
                    run_type: row.get(1)?,
                    status: row.get(2)?,
                    started_at: row.get(3)?,
                    finished_at: row.get(4)?,
                    summary: row.get(5)?,
                    error_message: row.get(6)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let execution_steps = {
            let mut stmt = conn.prepare(
                "SELECT id, run_id, title, status, risk_level, output_summary, updated_at
                 FROM execution_steps
                 WHERE mission_id = ?1
                 ORDER BY datetime(updated_at) DESC, rowid DESC",
            )?;
            let rows = stmt.query_map([mission_id], |row| {
                Ok(ExecutionStepRow {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    title: row.get(2)?,
                    status: row.get(3)?,
                    risk_level: row.get(4)?,
                    output_summary: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let memory_records = {
            let mut stmt = conn.prepare(
                "SELECT id, title, content, source_type, importance, created_at
                 FROM memory_records
                 WHERE scope = 'mission' AND scope_ref = ?1
                 ORDER BY datetime(created_at) DESC, rowid DESC",
            )?;
            let rows = stmt.query_map([mission_id], |row| {
                Ok(MemoryRecordRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    content: row.get(2)?,
                    source_type: row.get(3)?,
                    importance: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let scenario_runs = {
            let mut stmt = conn.prepare(
                "SELECT id, options_json, recommendation, created_at
                 FROM scenario_runs
                 WHERE mission_id = ?1
                 ORDER BY datetime(created_at) DESC, rowid DESC",
            )?;
            let rows = stmt.query_map([mission_id], |row| {
                let id = row.get::<_, String>(0)?;
                let payload_json = row.get::<_, String>(1)?;
                let recommendation = row.get::<_, Option<String>>(2)?;
                let created_at = row.get::<_, String>(3)?;
                Ok(parse_scenario_run_row(
                    id,
                    payload_json,
                    recommendation,
                    created_at,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let run_events = {
            let mut stmt = conn.prepare(
                "SELECT id, run_id, event_type, message, created_at
                 FROM run_events
                 WHERE mission_id = ?1
                 ORDER BY datetime(created_at) DESC, rowid DESC",
            )?;
            let rows = stmt.query_map([mission_id], |row| {
                Ok(RunEventRow {
                    id: row.get(0)?,
                    run_id: row.get(1)?,
                    event_type: row.get(2)?,
                    message: row.get(3)?,
                    created_at: row.get(4)?,
                })
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        Ok(MissionSnapshot {
            mission,
            runs,
            execution_steps,
            memory_records,
            scenario_runs,
            run_events,
        })
    })
}

fn synthesize_playbook(snapshot: &MissionSnapshot) -> SynthesizedPlaybook {
    let mut evidence_cards = Vec::new();
    let mut suggestions = Vec::new();

    let execution_signal = build_execution_signal(snapshot);
    if let Some(card) = execution_signal.card.clone() {
        evidence_cards.push(card);
    }
    if let Some(suggestion) = execution_signal.suggestion.clone() {
        suggestions.push(suggestion);
    }

    let approval_signal = build_approval_signal(snapshot);
    if let Some(card) = approval_signal.card.clone() {
        evidence_cards.push(card);
    }
    if let Some(suggestion) = approval_signal.suggestion.clone() {
        suggestions.push(suggestion);
    }

    let scenario_signal = build_scenario_signal(snapshot);
    if let Some(card) = scenario_signal.card.clone() {
        evidence_cards.push(card);
    }
    if let Some(suggestion) = scenario_signal.suggestion.clone() {
        suggestions.push(suggestion);
    }

    let memory_signal = build_memory_signal(snapshot);
    if let Some(card) = memory_signal.card.clone() {
        evidence_cards.push(card);
    }
    if let Some(suggestion) = memory_signal.suggestion.clone() {
        suggestions.push(suggestion);
    }

    let event_signal = build_event_signal(snapshot);
    if !suggestions.is_empty()
        && let Some(card) = event_signal.card.clone()
    {
        evidence_cards.push(card);
    }

    if suggestions.is_empty() {
        let momentum_signal = build_momentum_signal(snapshot);
        if let Some(card) = momentum_signal.card.clone() {
            evidence_cards.push(card);
        }
        if let Some(suggestion) = momentum_signal.suggestion.clone() {
            suggestions.push(suggestion);
        }
    }

    let summary = build_summary(
        snapshot,
        &suggestions,
        &execution_signal,
        &approval_signal,
        &scenario_signal,
    );

    SynthesizedPlaybook {
        summary,
        suggestions,
        evidence_cards,
    }
}

#[derive(Debug, Clone)]
struct SignalBundle {
    card: Option<PlaybookEvidenceCard>,
    suggestion: Option<PlaybookSuggestion>,
    headline: Option<String>,
}

fn build_execution_signal(snapshot: &MissionSnapshot) -> SignalBundle {
    let failed_steps = snapshot
        .execution_steps
        .iter()
        .filter(|step| step.status == "failed")
        .collect::<Vec<_>>();
    if failed_steps.is_empty() {
        return SignalBundle {
            card: None,
            suggestion: None,
            headline: None,
        };
    }

    let mut failures_by_title = BTreeMap::<String, usize>::new();
    let mut sources = Vec::new();
    for step in &failed_steps {
        *failures_by_title.entry(step.title.clone()).or_insert(0) += 1;
        push_unique(&mut sources, format!("step:{}", step.id));
        push_unique(&mut sources, format!("run:{}", step.run_id));
    }
    let failed_run_count = snapshot
        .runs
        .iter()
        .filter(|run| run.status == "failed")
        .count();
    let recurring_failure = failures_by_title
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(title, count)| (title.clone(), *count))
        .unwrap_or_else(|| ("execution step".to_string(), failed_steps.len()));
    let latest_summary = failed_steps
        .iter()
        .find_map(|step| step.output_summary.as_deref())
        .map(truncate_text)
        .unwrap_or_else(|| "Recent failures did not include a step summary.".to_string());
    let bullets = vec![
        format!(
            "{} failed {} times.",
            recurring_failure.0, recurring_failure.1
        ),
        format!(
            "{} failed step(s) and {} failed run(s) recorded.",
            failed_steps.len(),
            failed_run_count
        ),
        format!("Latest failure note: {}", latest_summary),
    ];

    SignalBundle {
        card: Some(PlaybookEvidenceCard {
            id: "execution_signal".to_string(),
            category: "execution_signal".to_string(),
            title: "Execution friction is concentrated in one motion".to_string(),
            summary: format!(
                "{} failed step(s) across {} run(s), led by repeated breakdowns in {}.",
                failed_steps.len(),
                snapshot.runs.len(),
                recurring_failure.0
            ),
            bullets,
            source_refs: sources,
        }),
        suggestion: Some(PlaybookSuggestion {
            id: "stabilize_execution".to_string(),
            kind: "stabilize_execution".to_string(),
            priority: "high".to_string(),
            title: "Stabilize outreach before scaling".to_string(),
            rationale: format!(
                "{} is repeatedly failing, which means the mission needs a tighter operating pattern before broader rollout.",
                recurring_failure.0
            ),
            actions: vec![
                format!(
                    "Turn {} into a reusable checklist or template.",
                    recurring_failure.0
                ),
                "Attach quantified proof from mission memory before the next rerun.".to_string(),
                "Compare the next attempt directly against the strongest scenario recommendation."
                    .to_string(),
            ],
            evidence_ids: vec!["execution_signal".to_string(), "event_signal".to_string()],
        }),
        headline: Some(format!(
            "Execution needs stabilization around {}.",
            recurring_failure.0
        )),
    }
}

fn build_approval_signal(snapshot: &MissionSnapshot) -> SignalBundle {
    let awaiting_steps = snapshot
        .execution_steps
        .iter()
        .filter(|step| step.status == "awaiting_approval")
        .collect::<Vec<_>>();
    if awaiting_steps.is_empty() {
        return SignalBundle {
            card: None,
            suggestion: None,
            headline: None,
        };
    }

    let mut sources = Vec::new();
    let mut bullets = Vec::new();
    for step in awaiting_steps.iter().take(3) {
        push_unique(&mut sources, format!("step:{}", step.id));
        push_unique(&mut sources, format!("run:{}", step.run_id));
        bullets.push(format!(
            "{} is blocked in {} risk mode as of {}.",
            step.title, step.risk_level, step.updated_at
        ));
    }

    SignalBundle {
        card: Some(PlaybookEvidenceCard {
            id: "approval_signal".to_string(),
            category: "approval_signal".to_string(),
            title: "Approvals are slowing the mission".to_string(),
            summary: format!(
                "{} step(s) are waiting on approval before execution can move.",
                awaiting_steps.len()
            ),
            bullets,
            source_refs: sources,
        }),
        suggestion: Some(PlaybookSuggestion {
            id: "unblock_approval".to_string(),
            kind: "unblock_approval".to_string(),
            priority: "high".to_string(),
            title: "Unblock approval-dependent steps".to_string(),
            rationale: format!(
                "{} active step(s) are approval-gated, so reducing reviewer friction should unlock faster mission progress.",
                awaiting_steps.len()
            ),
            actions: vec![
                "Pre-package the next approval request with outcome proof and rollback notes."
                    .to_string(),
                "Bundle recurring approval asks into one reusable approved play.".to_string(),
                "Move low-risk preparation work ahead of the approval wall.".to_string(),
            ],
            evidence_ids: vec!["approval_signal".to_string()],
        }),
        headline: Some("approval remains the primary operational bottleneck.".to_string()),
    }
}

fn build_scenario_signal(snapshot: &MissionSnapshot) -> SignalBundle {
    if snapshot.scenario_runs.is_empty() {
        return SignalBundle {
            card: None,
            suggestion: None,
            headline: None,
        };
    }

    let mut label_counts = BTreeMap::<String, usize>::new();
    let mut source_refs = Vec::new();
    let mut bullets = Vec::new();
    for run in &snapshot.scenario_runs {
        push_unique(&mut source_refs, format!("scenario:{}", run.id));
        if let Some(summary) = run.comparison_summary.as_deref() {
            bullets.push(format!("{}: {}", run.created_at, truncate_text(summary)));
        }
        if let Some(label) = scenario_focus_label(run) {
            *label_counts.entry(label).or_insert(0) += 1;
        }
    }
    let Some((winner, count)) = label_counts
        .iter()
        .max_by_key(|(_, count)| **count)
        .map(|(label, count)| (label.clone(), *count))
    else {
        return SignalBundle {
            card: None,
            suggestion: None,
            headline: None,
        };
    };

    let variable_hint = snapshot
        .scenario_runs
        .iter()
        .flat_map(|run| run.variable_changes.iter())
        .next()
        .cloned();

    let mut card_bullets = Vec::new();
    card_bullets.push(format!(
        "{} is the preferred direction in {} of {} scenario run(s).",
        winner,
        count,
        snapshot.scenario_runs.len()
    ));
    if let Some(variable_hint) = variable_hint.clone() {
        card_bullets.push(format!("Recurring decision variable: {}", variable_hint));
    }
    card_bullets.extend(bullets.into_iter().take(2));

    SignalBundle {
        card: Some(PlaybookEvidenceCard {
            id: "scenario_signal".to_string(),
            category: "scenario_signal".to_string(),
            title: "Scenario analysis is converging".to_string(),
            summary: format!(
                "{} is the winning scenario path across {} saved simulations.",
                winner, count
            ),
            bullets: card_bullets,
            source_refs,
        }),
        suggestion: Some(PlaybookSuggestion {
            id: "lean_into_scenario".to_string(),
            kind: "lean_into_scenario".to_string(),
            priority: if count >= 2 { "high" } else { "medium" }.to_string(),
            title: "Lean into the winning scenario path".to_string(),
            rationale: format!(
                "Scenario runs repeatedly point to {}, so the next growth motion should reflect that operating assumption.",
                winner
            ),
            actions: vec![
                format!("Convert {} into the default next-run playbook.", winner),
                "Track the variables that most changed the scenario score.".to_string(),
                "Use the strongest scenario summary as copy for the next mission brief."
                    .to_string(),
            ],
            evidence_ids: vec!["scenario_signal".to_string()],
        }),
        headline: Some(format!("{} is the clearest scenario path.", winner)),
    }
}

fn build_memory_signal(snapshot: &MissionSnapshot) -> SignalBundle {
    let high_signal_records = snapshot
        .memory_records
        .iter()
        .filter(|record| matches!(record.importance.as_str(), "high" | "critical"))
        .collect::<Vec<_>>();
    if high_signal_records.is_empty() {
        return SignalBundle {
            card: None,
            suggestion: None,
            headline: None,
        };
    }

    let bullets = high_signal_records
        .iter()
        .take(3)
        .map(|record| {
            format!(
                "{} ({}, {}): {}",
                record.title,
                record.source_type,
                record.created_at,
                truncate_text(&record.content)
            )
        })
        .collect::<Vec<_>>();
    let source_refs = high_signal_records
        .iter()
        .map(|record| format!("memory:{}", record.id))
        .collect::<Vec<_>>();

    SignalBundle {
        card: Some(PlaybookEvidenceCard {
            id: "memory_signal".to_string(),
            category: "memory_signal".to_string(),
            title: "Mission memory contains reusable proof".to_string(),
            summary: format!(
                "{} high-signal memory record(s) can be converted into sharper proof assets.",
                high_signal_records.len()
            ),
            bullets,
            source_refs,
        }),
        suggestion: Some(PlaybookSuggestion {
            id: "activate_memory".to_string(),
            kind: "activate_memory".to_string(),
            priority: "medium".to_string(),
            title: "Turn mission memory into proof assets".to_string(),
            rationale: "The mission already knows what resonates; that knowledge should be made explicit in the next playbook iteration.".to_string(),
            actions: vec![
                "Promote the top mission memories into the next brief or template.".to_string(),
                "Use memory-backed phrasing when requesting approval or drafting outreach.".to_string(),
                "Retain only memories that change the next decision or message.".to_string(),
            ],
            evidence_ids: vec!["memory_signal".to_string()],
        }),
        headline: Some("Mission memory already contains usable proof.".to_string()),
    }
}

fn build_event_signal(snapshot: &MissionSnapshot) -> SignalBundle {
    if snapshot.run_events.is_empty() {
        return SignalBundle {
            card: None,
            suggestion: None,
            headline: None,
        };
    }

    let mut bullets = Vec::new();
    let mut source_refs = Vec::new();
    for event in snapshot.run_events.iter().take(3) {
        bullets.push(format!(
            "{} [{} / {}]: {}",
            event.created_at, event.event_type, event.run_id, event.message
        ));
        source_refs.push(format!("event:{}", event.id));
    }

    SignalBundle {
        card: Some(PlaybookEvidenceCard {
            id: "event_signal".to_string(),
            category: "event_signal".to_string(),
            title: "Recent run events show the current story".to_string(),
            summary: format!(
                "{} recent event(s) capture the latest mission motion.",
                snapshot.run_events.len().min(3)
            ),
            bullets,
            source_refs,
        }),
        suggestion: None,
        headline: None,
    }
}

fn build_momentum_signal(snapshot: &MissionSnapshot) -> SignalBundle {
    let completed_runs = snapshot
        .runs
        .iter()
        .filter(|run| run.status == "completed")
        .count();
    let latest_event = snapshot.run_events.first();
    let mut source_refs = Vec::new();
    let mut bullets = Vec::new();
    for run in snapshot.runs.iter().take(2) {
        source_refs.push(format!("run:{}", run.id));
        let run_summary = match (
            run.summary.as_deref(),
            run.started_at.as_deref(),
            run.finished_at.as_deref(),
            run.error_message.as_deref(),
        ) {
            (Some(summary), _, _, _) => truncate_text(summary),
            (_, Some(started_at), Some(finished_at), Some(error_message)) => format!(
                "{} run {} -> {} and ended {} ({})",
                run.run_type,
                started_at,
                finished_at,
                run.status,
                truncate_text(error_message)
            ),
            (_, Some(started_at), Some(finished_at), None) => format!(
                "{} run {} -> {} and ended {}.",
                run.run_type, started_at, finished_at, run.status
            ),
            (_, Some(started_at), None, Some(error_message)) => format!(
                "{} run started {} and is {} ({})",
                run.run_type,
                started_at,
                run.status,
                truncate_text(error_message)
            ),
            (_, Some(started_at), None, None) => format!(
                "{} run started {} and is {}.",
                run.run_type, started_at, run.status
            ),
            (_, None, _, Some(error_message)) => format!(
                "{} run ended {} ({})",
                run.run_type,
                run.status,
                truncate_text(error_message)
            ),
            _ => format!("{} run ended with status {}.", run.run_type, run.status),
        };
        bullets.push(format!("Run {}: {}", run.id, run_summary));
    }
    if let Some(event) = latest_event {
        source_refs.push(format!("event:{}", event.id));
        bullets.push(format!("Latest event: {}", event.message));
    }

    SignalBundle {
        card: Some(PlaybookEvidenceCard {
            id: "momentum_signal".to_string(),
            category: "momentum_signal".to_string(),
            title: "The mission has a healthy recent signal".to_string(),
            summary: format!(
                "{} completed run(s) and {} recent event(s) indicate momentum worth preserving.",
                completed_runs,
                snapshot.run_events.len().min(3)
            ),
            bullets,
            source_refs,
        }),
        suggestion: Some(PlaybookSuggestion {
            id: "keep_momentum".to_string(),
            kind: "keep_momentum".to_string(),
            priority: "medium".to_string(),
            title: "Preserve the current winning motion".to_string(),
            rationale: "There is no acute friction signal, so the best play is to repeat what is already producing clean progress.".to_string(),
            actions: vec![
                "Repeat the most recent successful motion with a slightly narrower scope.".to_string(),
                "Document the successful pattern before adding new complexity.".to_string(),
                "Add one lightweight growth experiment instead of changing the whole flow.".to_string(),
            ],
            evidence_ids: vec!["momentum_signal".to_string()],
        }),
        headline: Some("The mission has a healthy recent signal.".to_string()),
    }
}

fn build_summary(
    snapshot: &MissionSnapshot,
    suggestions: &[PlaybookSuggestion],
    execution_signal: &SignalBundle,
    approval_signal: &SignalBundle,
    scenario_signal: &SignalBundle,
) -> String {
    if suggestions.len() == 1 && suggestions[0].kind == "keep_momentum" {
        return format!(
            "{} is showing a healthy recent signal; keep repeating the last successful motion with lightweight instrumentation.",
            snapshot.mission.title
        );
    }

    let mut clauses = Vec::new();
    if let Some(headline) = scenario_signal.headline.as_deref() {
        clauses.push(headline.to_string());
    }
    if let Some(headline) = approval_signal.headline.as_deref() {
        clauses.push(headline.to_string());
    }
    if let Some(headline) = execution_signal.headline.as_deref() {
        clauses.push(headline.to_string());
    }
    if clauses.is_empty() {
        clauses.push(format!(
            "Build the next playbook from {} recorded mission signals.",
            snapshot.runs.len()
                + snapshot.execution_steps.len()
                + snapshot.memory_records.len()
                + snapshot.scenario_runs.len()
                + snapshot.run_events.len()
        ));
    }

    format!("{} {}", snapshot.mission.goal, clauses.join(" "))
}

fn parse_scenario_run_row(
    id: String,
    payload_json: String,
    recommendation: Option<String>,
    created_at: String,
) -> ScenarioRunRow {
    let payload = serde_json::from_str::<Value>(&payload_json).unwrap_or(Value::Null);
    let normalized_recommendation = normalize_optional_text(recommendation);
    let mut option_lookup = BTreeMap::<String, String>::new();
    let mut option_labels = Vec::new();
    let mut selected_option_id = None;
    let comparison_summary;
    let mut variable_changes = Vec::new();

    match payload {
        Value::Array(items) => {
            for item in items {
                if let Some(label) = item
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                {
                    option_labels.push(label.to_string());
                }
            }
            comparison_summary = None;
        }
        Value::Object(map) => {
            if let Some(cards) = map.get("option_cards").and_then(Value::as_array) {
                for card in cards {
                    if let Some(label) = card
                        .get("label")
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                    {
                        option_labels.push(label.to_string());
                        if let Some(id_value) = card
                            .get("id")
                            .and_then(Value::as_str)
                            .map(str::trim)
                            .filter(|value| !value.is_empty())
                        {
                            option_lookup.insert(id_value.to_string(), label.to_string());
                        }
                    }
                }
            }
            selected_option_id = map
                .get("selected_option_id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToString::to_string);
            comparison_summary = normalize_optional_text(
                map.get("comparison_summary")
                    .and_then(Value::as_str)
                    .map(ToString::to_string),
            );
            if let Some(variables) = map.get("variables").and_then(Value::as_array) {
                for variable in variables {
                    if let Some(label) = variable.get("label").and_then(Value::as_str) {
                        let current = variable
                            .get("current_value")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .trim();
                        let proposed = variable
                            .get("proposed_value")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .trim();
                        let label = label.trim();
                        if !label.is_empty() && (!current.is_empty() || !proposed.is_empty()) {
                            variable_changes
                                .push(format!("{}: {} -> {}", label, current, proposed));
                        }
                    }
                }
            }
        }
        _ => {
            comparison_summary = None;
        }
    }

    let selected_label = selected_option_id
        .as_deref()
        .and_then(|selected_id| option_lookup.get(selected_id).cloned())
        .or_else(|| normalized_recommendation.clone());

    ScenarioRunRow {
        id,
        created_at,
        recommendation: normalized_recommendation,
        selected_label,
        comparison_summary,
        option_labels,
        variable_changes,
    }
}

fn scenario_focus_label(run: &ScenarioRunRow) -> Option<String> {
    run.selected_label
        .clone()
        .or_else(|| run.recommendation.clone())
        .or_else(|| run.option_labels.first().cloned())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn truncate_text(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= 96 {
        return trimmed.to_string();
    }
    let truncated = trimmed.chars().take(93).collect::<String>();
    format!("{truncated}...")
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !values.iter().any(|existing| existing == &value) {
        values.push(value);
    }
}

#[cfg(test)]
mod tests {
    use super::playbook_get_for_db;
    use crate::backend::Database;

    #[test]
    fn playbook_rejects_empty_mission_id() {
        let db = Database::in_memory().expect("db should initialize");
        let error = playbook_get_for_db(&db, "   ").expect_err("empty mission id should fail");
        assert_eq!(error.code, "validation_error");
        assert_eq!(error.message, "mission id cannot be empty");
    }

    #[test]
    fn playbook_synthesizes_growth_suggestions_and_evidence_cards() {
        let db = Database::in_memory().expect("db should initialize");
        insert_mission(
            &db,
            "mission-growth",
            "Expand Hermes desktop adoption",
            "Increase team usage of guided execution",
        );
        insert_run(
            &db,
            "run-exec-1",
            "mission-growth",
            "execution",
            "failed",
            Some("2026-04-20T10:00:00Z"),
            Some("2026-04-20T10:30:00Z"),
            Some("Initial rollout hit approval friction"),
            Some("Approval path stalled"),
        );
        insert_run(
            &db,
            "run-exec-2",
            "mission-growth",
            "execution",
            "running",
            Some("2026-04-22T09:00:00Z"),
            None,
            Some("Retrying with refined playbook"),
            None,
        );
        insert_execution_step(
            &db,
            "step-failed-1",
            "mission-growth",
            "run-exec-1",
            "Draft stakeholder outreach",
            "cli",
            "medium",
            "failed",
            Some("Response rate was low"),
            "2026-04-20T10:10:00Z",
        );
        insert_execution_step(
            &db,
            "step-failed-2",
            "mission-growth",
            "run-exec-2",
            "Draft stakeholder outreach",
            "cli",
            "medium",
            "failed",
            Some("Second attempt still weak"),
            "2026-04-22T09:10:00Z",
        );
        insert_execution_step(
            &db,
            "step-awaiting",
            "mission-growth",
            "run-exec-2",
            "Request outbound approval",
            "cli",
            "high",
            "awaiting_approval",
            None,
            "2026-04-22T09:15:00Z",
        );
        insert_memory(
            &db,
            "memory-1",
            "mission",
            "mission-growth",
            "Champion prefers proof",
            "Decision-makers respond to quantified evidence",
            "manual",
            "high",
            "2026-04-21T08:00:00Z",
        );
        insert_memory(
            &db,
            "memory-2",
            "mission",
            "mission-growth",
            "Pilot team pattern",
            "Teams adopt faster when rollout starts with one pilot squad",
            "slack",
            "high",
            "2026-04-21T09:00:00Z",
        );
        insert_scenario_run(
            &db,
            "scenario-1",
            "mission-growth",
            "Current broad rollout",
            r#"{"option_cards":[{"id":"pilot","label":"Pilot-first rollout","score":0.91,"time_horizon":"30 days","confidence":"high","expected_benefits":["Tighter feedback loop"],"risks":["Slower top-line reach"]},{"id":"broad","label":"Broad launch","score":0.4,"time_horizon":"30 days","confidence":"medium"}],"variables":[{"id":"proof","label":"Proof depth","current_value":"generic","proposed_value":"quantified","impact":"high","uncertainty":"medium"}],"comparison_summary":"Pilot-first rollout beats broad launch when proof quality is high","selected_option_id":"pilot"}"#,
            Some("Pilot-first rollout"),
            "2026-04-21T12:00:00Z",
        );
        insert_scenario_run(
            &db,
            "scenario-2",
            "mission-growth",
            "Current broad rollout",
            r#"{"option_cards":[{"id":"pilot","label":"Pilot-first rollout","score":0.88,"time_horizon":"45 days","confidence":"high"},{"id":"broad","label":"Broad launch","score":0.45,"time_horizon":"30 days","confidence":"medium"}],"variables":[{"id":"proof","label":"Proof depth","current_value":"generic","proposed_value":"quantified","impact":"high","uncertainty":"low"}],"comparison_summary":"Pilot-first rollout remains strongest","selected_option_id":"pilot"}"#,
            Some("Pilot-first rollout"),
            "2026-04-22T12:00:00Z",
        );
        insert_event(
            &db,
            "event-1",
            "run-exec-2",
            "mission-growth",
            "step_failed",
            "Outreach draft underperformed",
            Some("{\"step_id\":\"step-failed-2\"}"),
            "2026-04-22T09:11:00Z",
        );
        insert_event(
            &db,
            "event-2",
            "run-exec-2",
            "mission-growth",
            "step_started",
            "Approval request queued",
            Some("{\"step_id\":\"step-awaiting\"}"),
            "2026-04-22T09:15:30Z",
        );

        let playbook = playbook_get_for_db(&db, "mission-growth").expect("playbook should build");

        assert_eq!(playbook.mission_title, "Expand Hermes desktop adoption");
        assert!(playbook.summary.contains("Pilot-first rollout"));
        assert!(playbook.summary.contains("approval"));
        assert!(playbook.evidence_cards.len() >= 4);

        let suggestion_titles = playbook
            .suggestions
            .iter()
            .map(|suggestion| suggestion.title.as_str())
            .collect::<Vec<_>>();
        assert!(
            suggestion_titles
                .iter()
                .any(|title| title.contains("Stabilize outreach before scaling"))
        );
        assert!(
            suggestion_titles
                .iter()
                .any(|title| title.contains("Unblock approval-dependent steps"))
        );
        assert!(
            suggestion_titles
                .iter()
                .any(|title| title.contains("Lean into the winning scenario path"))
        );
        assert!(
            suggestion_titles
                .iter()
                .any(|title| title.contains("Turn mission memory into proof assets"))
        );

        let scenario_card = playbook
            .evidence_cards
            .iter()
            .find(|card| card.category == "scenario_signal")
            .expect("scenario signal card should exist");
        assert!(scenario_card.summary.contains("Pilot-first rollout"));
        assert!(
            scenario_card
                .source_refs
                .iter()
                .any(|source| source == "scenario:scenario-1")
        );
        assert!(
            scenario_card
                .source_refs
                .iter()
                .any(|source| source == "scenario:scenario-2")
        );

        let execution_card = playbook
            .evidence_cards
            .iter()
            .find(|card| card.category == "execution_signal")
            .expect("execution signal card should exist");
        assert!(execution_card.summary.contains("2 failed"));
        assert!(
            execution_card
                .bullets
                .iter()
                .any(|bullet| bullet.contains("Draft stakeholder outreach"))
        );
    }

    #[test]
    fn playbook_falls_back_to_momentum_when_data_is_thin() {
        let db = Database::in_memory().expect("db should initialize");
        insert_mission(
            &db,
            "mission-thin",
            "Launch internal habit loop",
            "Keep momentum on a healthy mission",
        );
        insert_run(
            &db,
            "run-growth-1",
            "mission-thin",
            "growth",
            "completed",
            Some("2026-04-23T08:00:00Z"),
            Some("2026-04-23T09:00:00Z"),
            Some("Weekly experiment recap shipped"),
            None,
        );
        insert_event(
            &db,
            "event-thin-1",
            "run-growth-1",
            "mission-thin",
            "step_completed",
            "Recap delivered to pilot users",
            None,
            "2026-04-23T09:00:00Z",
        );

        let playbook = playbook_get_for_db(&db, "mission-thin").expect("playbook should build");

        assert_eq!(playbook.suggestions.len(), 1);
        assert_eq!(playbook.suggestions[0].kind, "keep_momentum");
        assert!(
            playbook.suggestions[0]
                .title
                .contains("Preserve the current winning motion")
        );
        assert_eq!(playbook.evidence_cards.len(), 1);
        assert_eq!(playbook.evidence_cards[0].category, "momentum_signal");
        assert!(playbook.summary.contains("healthy recent signal"));
    }

    fn insert_mission(db: &Database, id: &str, title: &str, goal: &str) {
        db.execute(
            "INSERT INTO missions (
                id, title, goal, constraints_json, success_criteria_json, status, priority, pinned,
                created_at, updated_at, last_activity_at
            ) VALUES (?1, ?2, ?3, '[]', '[]', 'planning', 'high', 0, ?4, ?4, ?4)",
            &[
                &id as &dyn rusqlite::ToSql,
                &title,
                &goal,
                &"2026-04-20T08:00:00Z",
            ],
        )
        .expect("mission should insert");
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_run(
        db: &Database,
        id: &str,
        mission_id: &str,
        run_type: &str,
        status: &str,
        started_at: Option<&str>,
        finished_at: Option<&str>,
        summary: Option<&str>,
        error_message: Option<&str>,
    ) {
        db.execute(
            "INSERT INTO runs (
                id, mission_id, type, status, started_at, finished_at, summary, error_message
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            &[
                &id as &dyn rusqlite::ToSql,
                &mission_id,
                &run_type,
                &status,
                &started_at,
                &finished_at,
                &summary,
                &error_message,
            ],
        )
        .expect("run should insert");
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_execution_step(
        db: &Database,
        id: &str,
        mission_id: &str,
        run_id: &str,
        title: &str,
        mode: &str,
        risk_level: &str,
        status: &str,
        output_summary: Option<&str>,
        updated_at: &str,
    ) {
        db.execute(
            "INSERT INTO execution_steps (
                id, mission_id, run_id, title, mode, risk_level, status, input_payload,
                output_summary, created_at, updated_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL, ?8, ?9, ?9)",
            &[
                &id as &dyn rusqlite::ToSql,
                &mission_id,
                &run_id,
                &title,
                &mode,
                &risk_level,
                &status,
                &output_summary,
                &updated_at,
            ],
        )
        .expect("execution step should insert");
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_memory(
        db: &Database,
        id: &str,
        scope: &str,
        scope_ref: &str,
        title: &str,
        content: &str,
        source_type: &str,
        importance: &str,
        created_at: &str,
    ) {
        db.execute(
            "INSERT INTO memory_records (
                id, scope, scope_ref, title, content, source_type, importance, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            &[
                &id as &dyn rusqlite::ToSql,
                &scope,
                &scope_ref,
                &title,
                &content,
                &source_type,
                &importance,
                &created_at,
            ],
        )
        .expect("memory should insert");
    }

    fn insert_scenario_run(
        db: &Database,
        id: &str,
        mission_id: &str,
        baseline: &str,
        options_json: &str,
        recommendation: Option<&str>,
        created_at: &str,
    ) {
        db.execute(
            "INSERT INTO scenario_runs (
                id, mission_id, baseline, options_json, recommendation, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            &[
                &id as &dyn rusqlite::ToSql,
                &mission_id,
                &baseline,
                &options_json,
                &recommendation,
                &created_at,
            ],
        )
        .expect("scenario should insert");
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_event(
        db: &Database,
        id: &str,
        run_id: &str,
        mission_id: &str,
        event_type: &str,
        message: &str,
        payload_json: Option<&str>,
        created_at: &str,
    ) {
        db.execute(
            "INSERT INTO run_events (
                id, run_id, mission_id, event_type, message, payload_json, created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            &[
                &id as &dyn rusqlite::ToSql,
                &run_id,
                &mission_id,
                &event_type,
                &message,
                &payload_json,
                &created_at,
            ],
        )
        .expect("event should insert");
    }
}
