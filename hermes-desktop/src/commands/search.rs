//! Global search command

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::backend::AppError;
use crate::backend::{
    Database, MissionListFilter, MissionService, MissionServiceImpl, SessionService,
    SessionServiceImpl,
};
use crate::commands::memory::{
    MemoryRecordItem, MemoryRecordSearchRequest, memory_record_search_for_db,
};
use crate::commands::skills::SkillListItem;
use crate::commands::skills::list_skills_from_db;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalSearchRequest {
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalSearchResult {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub detail: String,
    pub route: String,
}

#[tauri::command]
pub fn global_search(
    db: State<'_, Database>,
    request: GlobalSearchRequest,
) -> Result<Vec<GlobalSearchResult>, AppError> {
    let trimmed = request.query.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    let mission_service = MissionServiceImpl::new(db.inner().clone());
    let session_service = SessionServiceImpl::new(db.inner().clone());
    let missions = mission_service.list(MissionListFilter {
        query: Some(trimmed.to_string()),
        status: None,
        limit: Some(8),
    })?;
    let knowledge = mission_service.list_knowledge_feed(Some(trimmed.to_string()))?;
    let sessions = session_service.search(trimmed, 8)?;
    let skills = list_skills_from_db(db.inner())?;
    let memory = memory_record_search_for_db(
        db.inner(),
        MemoryRecordSearchRequest {
            query: trimmed.to_string(),
            limit: Some(8),
        },
    )?;

    Ok(build_search_results(
        trimmed, missions, sessions, knowledge, skills, memory,
    ))
}

fn build_search_results(
    query: &str,
    missions: Vec<crate::backend::Mission>,
    sessions: Vec<crate::backend::Session>,
    knowledge: Vec<crate::backend::KnowledgeFeedItem>,
    skills: Vec<SkillListItem>,
    memory: Vec<MemoryRecordItem>,
) -> Vec<GlobalSearchResult> {
    let normalized = query.trim().to_lowercase();
    let mission_results = missions.into_iter().map(|mission| GlobalSearchResult {
        id: mission.id,
        kind: "mission".to_string(),
        title: mission.title,
        detail: format!(
            "{} · {}",
            mission.status.as_str(),
            mission.priority.as_str()
        ),
        route: "/missions".to_string(),
    });

    let session_results = sessions.into_iter().map(|session| GlobalSearchResult {
        id: session.id,
        kind: "session".to_string(),
        title: session.title,
        detail: format!(
            "{} · {}",
            session.source.as_str(),
            session.model_name.unwrap_or_else(|| "-".to_string())
        ),
        route: "/sessions".to_string(),
    });

    let knowledge_results = knowledge
        .into_iter()
        .take(8)
        .map(|item| GlobalSearchResult {
            id: item.id,
            kind: "knowledge".to_string(),
            title: item.title,
            detail: format!("{} · {}", item.mission_title, item.item_type),
            route: "/knowledge".to_string(),
        });

    let skill_results = skills
        .into_iter()
        .filter(|skill| {
            skill.name.to_lowercase().contains(&normalized)
                || skill.display_name.to_lowercase().contains(&normalized)
                || skill.source.to_lowercase().contains(&normalized)
        })
        .map(|skill| GlobalSearchResult {
            id: skill.name,
            kind: "skill".to_string(),
            title: skill.display_name,
            detail: format!(
                "{} · {}",
                skill.source,
                if skill.enabled { "enabled" } else { "disabled" }
            ),
            route: "/skills".to_string(),
        });

    let memory_results = memory.into_iter().map(|record| GlobalSearchResult {
        id: record.id,
        kind: "memory".to_string(),
        title: record.title,
        detail: format!(
            "{} · {} · {}",
            record.scope, record.scope_ref, record.source_type
        ),
        route: "/knowledge".to_string(),
    });

    mission_results
        .chain(session_results)
        .chain(knowledge_results)
        .chain(skill_results)
        .chain(memory_results)
        .take(12)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::build_search_results;
    use crate::backend::{
        KnowledgeFeedItem, Mission, MissionPriority, MissionStatus, Session, SessionSource,
    };
    use crate::commands::memory::MemoryRecordItem;
    use crate::commands::skills::SkillListItem;

    #[test]
    fn build_search_results_merges_missions_sessions_knowledge_skills_and_memory() {
        let results = build_search_results(
            "acme",
            vec![Mission {
                id: "mission-1".to_string(),
                title: "Acme rollout".to_string(),
                goal: "Ship".to_string(),
                constraints: vec![],
                success_criteria: vec![],
                status: MissionStatus::Planning,
                priority: MissionPriority::High,
                pinned: false,
                created_at: "2026-01-01".to_string(),
                updated_at: "2026-01-01".to_string(),
                last_activity_at: "2026-01-01".to_string(),
            }],
            vec![Session {
                id: "session-acme".to_string(),
                source: SessionSource::Cli,
                title: "Acme session".to_string(),
                model_name: Some("gpt-5.4".to_string()),
                parent_session_id: None,
                started_at: "2026-01-01".to_string(),
                updated_at: "2026-01-01".to_string(),
                ended_at: None,
            }],
            vec![KnowledgeFeedItem {
                id: "context-1".to_string(),
                mission_id: "mission-1".to_string(),
                mission_title: "Acme rollout".to_string(),
                source_kind: "context_item".to_string(),
                item_type: "note".to_string(),
                title: "Acme notes".to_string(),
                preview: Some("Budget".to_string()),
                source: None,
                path: None,
                created_at: "2026-01-01".to_string(),
            }],
            vec![SkillListItem {
                name: "acme-helper".to_string(),
                display_name: "Acme Helper".to_string(),
                source: "codex".to_string(),
                path: "/tmp/skill".to_string(),
                enabled: true,
            }],
            vec![MemoryRecordItem {
                id: "memory-1".to_string(),
                scope: "mission".to_string(),
                scope_ref: "mission-1".to_string(),
                title: "Acme preference".to_string(),
                content: "Prefers Wednesday updates".to_string(),
                source_type: "gateway:slack".to_string(),
                importance: "medium".to_string(),
                created_at: "2026-01-01".to_string(),
            }],
        );

        assert_eq!(
            results
                .iter()
                .map(|item| item.kind.as_str())
                .collect::<Vec<_>>(),
            vec!["mission", "session", "knowledge", "skill", "memory"]
        );
        assert_eq!(results[4].route, "/knowledge");
    }

    #[test]
    fn build_search_results_keeps_session_hits_even_when_query_only_matches_transcript() {
        let results = build_search_results(
            "blocked sync",
            vec![],
            vec![Session {
                id: "session-ops".to_string(),
                source: SessionSource::Cli,
                title: "Operations follow-up".to_string(),
                model_name: Some("gpt-5.4".to_string()),
                parent_session_id: None,
                started_at: "2026-01-01".to_string(),
                updated_at: "2026-01-01".to_string(),
                ended_at: None,
            }],
            vec![],
            vec![],
            vec![],
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].kind, "session");
        assert_eq!(results[0].id, "session-ops");
    }
}
