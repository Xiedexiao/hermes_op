//! Session 命令

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::backend::{
    AppError, CreateSessionMessageInput, Database, RecentSessionEvidence, Session, SessionMessage,
    SessionMessageRole, SessionService, SessionServiceImpl,
};

const DEFAULT_RECENT_LIMIT: usize = 20;
const DEFAULT_SEARCH_LIMIT: usize = 20;
const DEFAULT_MESSAGE_LIMIT: usize = 50;
const ACTIVE_SESSION_KEY: &str = "active_session_selection";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionListRequest {
    pub limit: Option<usize>,
}

fn resolve_recent_limit(request: Option<SessionListRequest>) -> usize {
    request
        .and_then(|request| request.limit)
        .unwrap_or(DEFAULT_RECENT_LIMIT)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRenameRequest {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSearchRequest {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessageListRequest {
    pub session_id: String,
    pub limit: Option<usize>,
    pub role: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessageCreateRequest {
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResumeByTitleRequest {
    pub title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionActivateRequest {
    pub id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionReplaySnapshotRequest {
    pub session_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveSessionSelection {
    pub session: Session,
    pub reason: String,
    pub activated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionReplaySnapshot {
    pub resolved_via: String,
    pub session: Option<Session>,
    pub active_selection: Option<ActiveSessionSelection>,
    pub messages: Vec<SessionMessage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PersistedActiveSessionSelection {
    session_id: String,
    reason: String,
    activated_at: String,
}

fn select_latest_session(sessions: Vec<Session>) -> Option<Session> {
    sessions.into_iter().next()
}

fn normalize_session_id(id: String) -> String {
    id.trim().to_string()
}

fn normalize_search_query(query: String) -> String {
    query.trim().to_string()
}

fn normalize_message_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_MESSAGE_LIMIT).clamp(1, 200)
}

fn normalize_reason(reason: Option<String>, fallback: &str) -> String {
    reason
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| fallback.to_string())
}

fn parse_message_role(value: &str) -> SessionMessageRole {
    SessionMessageRole::from_key(value.trim())
}

fn normalize_optional_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

impl SessionRenameRequest {
    fn into_service_args(self) -> (String, String) {
        (normalize_session_id(self.id), self.title.trim().to_string())
    }
}

impl SessionMessageListRequest {
    fn into_service_query(self) -> crate::backend::SessionMessageHistoryQuery {
        crate::backend::SessionMessageHistoryQuery {
            session_id: normalize_session_id(self.session_id),
            limit: normalize_message_limit(self.limit),
            role: normalize_optional_filter(self.role).map(|value| parse_message_role(&value)),
            text_query: normalize_optional_filter(self.query),
        }
    }
}

impl SessionReplaySnapshotRequest {
    fn normalized_session_id(&self) -> Option<String> {
        normalize_optional_filter(self.session_id.clone())
    }

    fn limit(&self) -> usize {
        normalize_message_limit(self.limit)
    }
}

#[tauri::command]
pub fn session_list_recent(
    db: State<'_, Database>,
    request: Option<SessionListRequest>,
) -> Result<Vec<Session>, AppError> {
    let service = SessionServiceImpl::new(db.inner().clone());
    let limit = resolve_recent_limit(request);
    service.list_recent(limit)
}

pub fn session_list_recent_evidence_for_db(
    db: &Database,
    request: Option<SessionListRequest>,
) -> Result<Vec<RecentSessionEvidence>, AppError> {
    let service = SessionServiceImpl::new(db.clone());
    let limit = resolve_recent_limit(request);
    service.list_recent_evidence(limit)
}

#[tauri::command]
pub fn session_get(db: State<'_, Database>, id: String) -> Result<Option<Session>, AppError> {
    let service = SessionServiceImpl::new(db.inner().clone());
    let id = normalize_session_id(id);
    service.get_by_id(&id)
}

#[tauri::command]
pub fn session_get_latest(db: State<'_, Database>) -> Result<Option<Session>, AppError> {
    let service = SessionServiceImpl::new(db.inner().clone());
    service.list_recent(1).map(select_latest_session)
}

pub fn session_continue_latest_for_db(db: &Database) -> Result<Option<Session>, AppError> {
    let service = SessionServiceImpl::new(db.clone());
    let latest = service.list_recent(1).map(select_latest_session)?;
    if let Some(session) = latest.as_ref() {
        let _ = persist_active_session_selection(db, session, "continue_latest")?;
    }
    Ok(latest)
}

#[tauri::command]
pub fn session_continue_latest(db: State<'_, Database>) -> Result<Option<Session>, AppError> {
    session_continue_latest_for_db(db.inner())
}

#[tauri::command]
pub fn session_search(
    db: State<'_, Database>,
    request: SessionSearchRequest,
) -> Result<Vec<Session>, AppError> {
    let service = SessionServiceImpl::new(db.inner().clone());
    let query = normalize_search_query(request.query);
    if query.is_empty() {
        return Ok(Vec::new());
    }

    service.search(&query, request.limit.unwrap_or(DEFAULT_SEARCH_LIMIT))
}

#[tauri::command]
pub fn session_message_list(
    db: State<'_, Database>,
    request: SessionMessageListRequest,
) -> Result<Vec<SessionMessage>, AppError> {
    let service = SessionServiceImpl::new(db.inner().clone());
    service.list_message_history(request.into_service_query())
}

#[tauri::command]
pub fn session_message_create(
    db: State<'_, Database>,
    request: SessionMessageCreateRequest,
) -> Result<SessionMessage, AppError> {
    let service = SessionServiceImpl::new(db.inner().clone());
    service.create_message(CreateSessionMessageInput {
        session_id: request.session_id.trim().to_string(),
        role: parse_message_role(&request.role),
        content: request.content.trim().to_string(),
        source: request
            .source
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "local".to_string()),
    })
}

fn replay_messages_for_session(
    service: &SessionServiceImpl,
    session_id: String,
    limit: usize,
) -> Result<Vec<SessionMessage>, AppError> {
    let mut messages =
        service.list_message_history(crate::backend::SessionMessageHistoryQuery {
            session_id,
            limit,
            role: None,
            text_query: None,
        })?;
    messages.reverse();
    Ok(messages)
}

pub fn session_replay_snapshot_for_db(
    db: &Database,
    request: Option<SessionReplaySnapshotRequest>,
) -> Result<SessionReplaySnapshot, AppError> {
    let service = SessionServiceImpl::new(db.clone());
    let request = request.unwrap_or(SessionReplaySnapshotRequest {
        session_id: None,
        limit: None,
    });
    let limit = request.limit();

    if let Some(session_id) = request.normalized_session_id() {
        let session = service
            .get_by_id(&session_id)?
            .ok_or_else(|| AppError::validation("session not found"))?;
        return Ok(SessionReplaySnapshot {
            resolved_via: "session_id".to_string(),
            active_selection: None,
            messages: replay_messages_for_session(&service, session.id.clone(), limit)?,
            session: Some(session),
        });
    }

    if let Some(active_selection) = session_get_active_for_db(db)? {
        return Ok(SessionReplaySnapshot {
            resolved_via: "active_session".to_string(),
            messages: replay_messages_for_session(
                &service,
                active_selection.session.id.clone(),
                limit,
            )?,
            session: Some(active_selection.session.clone()),
            active_selection: Some(active_selection),
        });
    }

    if let Some(session) = service.get_latest()? {
        return Ok(SessionReplaySnapshot {
            resolved_via: "latest_session".to_string(),
            active_selection: None,
            messages: replay_messages_for_session(&service, session.id.clone(), limit)?,
            session: Some(session),
        });
    }

    Ok(SessionReplaySnapshot {
        resolved_via: "none".to_string(),
        session: None,
        active_selection: None,
        messages: Vec::new(),
    })
}

#[tauri::command]
pub fn session_replay_snapshot(
    db: State<'_, Database>,
    request: Option<SessionReplaySnapshotRequest>,
) -> Result<SessionReplaySnapshot, AppError> {
    session_replay_snapshot_for_db(db.inner(), request)
}

#[tauri::command]
pub fn session_resume_by_title(
    db: State<'_, Database>,
    request: SessionResumeByTitleRequest,
) -> Result<Option<Session>, AppError> {
    let service = SessionServiceImpl::new(db.inner().clone());
    let query = normalize_search_query(request.title);
    if query.is_empty() {
        return Ok(None);
    }

    let matched = service
        .search(&query, DEFAULT_SEARCH_LIMIT)
        .map(select_latest_session)?;
    if let Some(session) = matched.as_ref() {
        let _ = persist_active_session_selection(db.inner(), session, "resume_by_title")?;
    }
    Ok(matched)
}

pub fn session_activate_for_db(
    db: &Database,
    request: SessionActivateRequest,
) -> Result<ActiveSessionSelection, AppError> {
    let service = SessionServiceImpl::new(db.clone());
    let id = normalize_session_id(request.id);
    if id.is_empty() {
        return Err(AppError::validation("session id cannot be empty"));
    }
    let session = service
        .get_by_id(&id)?
        .ok_or_else(|| AppError::validation("session not found"))?;
    persist_active_session_selection(db, &session, &normalize_reason(request.reason, "manual"))
}

#[tauri::command]
pub fn session_activate(
    db: State<'_, Database>,
    request: SessionActivateRequest,
) -> Result<ActiveSessionSelection, AppError> {
    session_activate_for_db(db.inner(), request)
}

pub fn session_get_active_for_db(
    db: &Database,
) -> Result<Option<ActiveSessionSelection>, AppError> {
    let raw = match db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&ACTIVE_SESSION_KEY],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) => value,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(err) => {
            return Err(AppError::storage(format!(
                "Failed to load active session selection: {}",
                err
            )));
        }
    };
    let persisted: PersistedActiveSessionSelection =
        serde_json::from_str(&raw).map_err(AppError::from_json_error)?;
    let service = SessionServiceImpl::new(db.clone());
    let Some(session) = service.get_by_id(&persisted.session_id)? else {
        return Ok(None);
    };
    Ok(Some(ActiveSessionSelection {
        session,
        reason: persisted.reason,
        activated_at: persisted.activated_at,
    }))
}

#[tauri::command]
pub fn session_get_active(
    db: State<'_, Database>,
) -> Result<Option<ActiveSessionSelection>, AppError> {
    session_get_active_for_db(db.inner())
}

#[tauri::command]
pub fn session_clear_active(db: State<'_, Database>) -> Result<bool, AppError> {
    db.execute(
        "DELETE FROM app_settings WHERE key = ?1",
        &[&ACTIVE_SESSION_KEY as &dyn rusqlite::ToSql],
    )?;
    Ok(true)
}

fn persist_active_session_selection(
    db: &Database,
    session: &Session,
    reason: &str,
) -> Result<ActiveSessionSelection, AppError> {
    let activated_at = chrono::Utc::now().to_rfc3339();
    let persisted = PersistedActiveSessionSelection {
        session_id: session.id.clone(),
        reason: reason.to_string(),
        activated_at: activated_at.clone(),
    };
    let value_json = serde_json::to_string(&persisted).map_err(AppError::from_json_error)?;
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
        &[
            &ACTIVE_SESSION_KEY as &dyn rusqlite::ToSql,
            &value_json,
            &activated_at,
        ],
    )?;
    Ok(ActiveSessionSelection {
        session: session.clone(),
        reason: persisted.reason,
        activated_at,
    })
}

#[tauri::command]
pub fn session_rename(
    db: State<'_, Database>,
    request: SessionRenameRequest,
) -> Result<Session, AppError> {
    let service = SessionServiceImpl::new(db.inner().clone());
    let (id, title) = request.into_service_args();
    service.rename(&id, title)
}

#[cfg(test)]
mod tests {
    use super::{
        SessionActivateRequest, SessionListRequest, SessionMessageListRequest,
        SessionRenameRequest, SessionReplaySnapshotRequest, normalize_message_limit,
        normalize_search_query, normalize_session_id, resolve_recent_limit, select_latest_session,
        session_activate_for_db, session_continue_latest_for_db, session_get_active_for_db,
        session_list_recent_evidence_for_db, session_replay_snapshot_for_db,
    };
    use crate::backend::{
        CreateSessionInput, CreateSessionMessageInput, Database, Session,
        SessionMessageHistoryQuery, SessionMessageRole, SessionService, SessionServiceImpl,
        SessionSource,
    };

    #[test]
    fn session_list_recent_defaults_to_twenty() {
        assert_eq!(resolve_recent_limit(None), 20);
        assert_eq!(
            resolve_recent_limit(Some(SessionListRequest { limit: None })),
            20
        );
    }

    #[test]
    fn session_get_normalizes_whitespace_around_id() {
        assert_eq!(
            normalize_session_id("  session-001  \n".to_string()),
            "session-001"
        );
    }

    #[test]
    fn session_search_normalizes_whitespace_around_query() {
        assert_eq!(
            normalize_search_query("  quarterly planning  \n".to_string()),
            "quarterly planning"
        );
    }

    #[test]
    fn session_message_limit_defaults_to_fifty() {
        assert_eq!(normalize_message_limit(None), 50);
        assert_eq!(normalize_message_limit(Some(500)), 200);
    }

    #[test]
    fn session_message_list_request_into_service_query_trims_optional_filters() {
        let query = SessionMessageListRequest {
            session_id: "  session-001  ".to_string(),
            limit: Some(500),
            role: Some("  assistant  ".to_string()),
            query: Some("  blocked retry  ".to_string()),
        }
        .into_service_query();

        assert_eq!(query.session_id, "session-001");
        assert_eq!(query.limit, 200);
        assert_eq!(query.role, Some(SessionMessageRole::Assistant));
        assert_eq!(query.text_query.as_deref(), Some("blocked retry"));
    }

    #[test]
    fn session_message_list_request_into_service_query_drops_blank_optional_filters() {
        let query = SessionMessageListRequest {
            session_id: "session-001".to_string(),
            limit: None,
            role: Some("   ".to_string()),
            query: Some("\n".to_string()),
        }
        .into_service_query();

        assert_eq!(query.limit, 50);
        assert_eq!(query.role, None);
        assert_eq!(query.text_query, None);
    }

    #[test]
    fn session_rename_request_into_service_args_trims_fields() {
        let (id, title) = SessionRenameRequest {
            id: "  session-001  ".to_string(),
            title: "  新标题  ".to_string(),
        }
        .into_service_args();

        assert_eq!(id, "session-001");
        assert_eq!(title, "新标题");
    }

    #[test]
    fn session_get_latest_returns_none_for_empty_sessions() {
        assert_eq!(select_latest_session(Vec::new()), None);
    }

    #[test]
    fn session_get_latest_returns_first_recent_session() {
        let latest = Session {
            id: "session-latest".to_string(),
            source: SessionSource::Desktop,
            title: "Latest".to_string(),
            model_name: Some("gpt-5.4".to_string()),
            parent_session_id: None,
            started_at: "2026-04-22T10:00:00Z".to_string(),
            updated_at: "2026-04-22T10:00:00Z".to_string(),
            ended_at: None,
        };
        let older = Session {
            id: "session-older".to_string(),
            source: SessionSource::Cli,
            title: "Older".to_string(),
            model_name: None,
            parent_session_id: None,
            started_at: "2026-04-21T10:00:00Z".to_string(),
            updated_at: "2026-04-21T10:00:00Z".to_string(),
            ended_at: None,
        };

        assert_eq!(
            select_latest_session(vec![latest.clone(), older]),
            Some(latest)
        );
    }

    #[test]
    fn session_continue_latest_for_db_returns_latest_session() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db.clone());
        let _ = service
            .create(CreateSessionInput {
                source: SessionSource::Cli,
                title: "Older".to_string(),
                model_name: Some("gpt-4o".to_string()),
                parent_session_id: None,
            })
            .expect("older session should create");
        let latest = service
            .create(CreateSessionInput {
                source: SessionSource::Desktop,
                title: "Latest".to_string(),
                model_name: Some("gpt-5.4".to_string()),
                parent_session_id: None,
            })
            .expect("latest session should create");

        let continued = session_continue_latest_for_db(&db)
            .expect("continue latest should work")
            .expect("latest session should exist");

        assert_eq!(continued.id, latest.id);
        assert_eq!(continued.title, "Latest");
        let active = session_get_active_for_db(&db)
            .expect("active selection should load")
            .expect("active selection should exist");
        assert_eq!(active.session.id, latest.id);
        assert_eq!(active.reason, "continue_latest");
    }

    #[test]
    fn session_activate_for_db_persists_manual_handoff() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db.clone());
        let session = service
            .create(CreateSessionInput {
                source: SessionSource::Desktop,
                title: "Manual resume".to_string(),
                model_name: Some("gpt-5.4".to_string()),
                parent_session_id: None,
            })
            .expect("session should create");

        let active = session_activate_for_db(
            &db,
            SessionActivateRequest {
                id: session.id.clone(),
                reason: Some("manual_resume".to_string()),
            },
        )
        .expect("manual activation should persist");

        assert_eq!(active.session.id, session.id);
        assert_eq!(active.reason, "manual_resume");
    }

    #[test]
    fn session_resume_by_title_persists_active_selection() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db.clone());
        let created = service
            .create(CreateSessionInput {
                source: SessionSource::Cli,
                title: "Quarterly planning".to_string(),
                model_name: Some("gpt-4o".to_string()),
                parent_session_id: None,
            })
            .expect("session should create");

        let query = normalize_search_query("Quarterly planning".to_string());
        let matched = service
            .search(&query, 20)
            .expect("search should work")
            .into_iter()
            .next()
            .expect("match should exist");
        let persisted = super::persist_active_session_selection(&db, &matched, "resume_by_title")
            .expect("active selection should persist");

        assert_eq!(persisted.session.id, created.id);
        let active = session_get_active_for_db(&db)
            .expect("active selection should load")
            .expect("active selection should exist");
        assert_eq!(active.session.id, created.id);
        assert_eq!(active.reason, "resume_by_title");
    }

    #[test]
    fn session_list_recent_evidence_for_db_returns_ordered_evidence_payloads() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db.clone());
        let _older = service
            .create(CreateSessionInput {
                source: SessionSource::Cli,
                title: "Older".to_string(),
                model_name: Some("gpt-4o".to_string()),
                parent_session_id: None,
            })
            .expect("older session should create");
        let latest = service
            .create(CreateSessionInput {
                source: SessionSource::Desktop,
                title: "Recover blocked flow".to_string(),
                model_name: Some("gpt-5.4".to_string()),
                parent_session_id: Some("session-parent".to_string()),
            })
            .expect("latest session should create");

        let evidence =
            session_list_recent_evidence_for_db(&db, Some(SessionListRequest { limit: Some(2) }))
                .expect("recent evidence should list");

        assert_eq!(evidence.len(), 2);
        assert_eq!(evidence[0].id, latest.id);
        assert_eq!(evidence[0].source, SessionSource::Desktop);
        assert_eq!(evidence[0].title, "Recover blocked flow");
        assert_eq!(evidence[0].model_name.as_deref(), Some("gpt-5.4"));
        assert_eq!(
            evidence[0].parent_session_id.as_deref(),
            Some("session-parent")
        );
    }

    #[test]
    fn session_message_create_and_list_round_trip() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db.clone());
        let session = service
            .create(CreateSessionInput {
                source: SessionSource::Cli,
                title: "History".to_string(),
                model_name: Some("gpt-4o".to_string()),
                parent_session_id: None,
            })
            .expect("session should create");

        let created = service
            .create_message(CreateSessionMessageInput {
                session_id: session.id.clone(),
                role: SessionMessageRole::Note,
                content: "Saved local recovery note".to_string(),
                source: "local".to_string(),
            })
            .expect("message should create");

        let listed = service
            .list_message_history(SessionMessageHistoryQuery {
                session_id: session.id.clone(),
                limit: 20,
                role: None,
                text_query: None,
            })
            .expect("history should list");

        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
    }

    #[test]
    fn session_replay_snapshot_for_db_prefers_explicit_session_id_and_returns_chronological_messages()
     {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db.clone());
        let active_session = service
            .create(CreateSessionInput {
                source: SessionSource::Desktop,
                title: "Active".to_string(),
                model_name: Some("gpt-5.4".to_string()),
                parent_session_id: None,
            })
            .expect("active session should create");
        let replay_session = service
            .create(CreateSessionInput {
                source: SessionSource::Cli,
                title: "Replay target".to_string(),
                model_name: Some("gpt-4o".to_string()),
                parent_session_id: Some(active_session.id.clone()),
            })
            .expect("replay session should create");

        let older = service
            .create_message(CreateSessionMessageInput {
                session_id: replay_session.id.clone(),
                role: SessionMessageRole::User,
                content: "First prompt".to_string(),
                source: "local".to_string(),
            })
            .expect("older message should create");
        let newer = service
            .create_message(CreateSessionMessageInput {
                session_id: replay_session.id.clone(),
                role: SessionMessageRole::Assistant,
                content: "Second reply".to_string(),
                source: "local".to_string(),
            })
            .expect("newer message should create");

        db.execute(
            "UPDATE session_messages SET created_at = ?2 WHERE id = ?1",
            &[
                &older.id as &dyn rusqlite::ToSql,
                &"2026-04-25T10:00:00Z".to_string(),
            ],
        )
        .expect("older timestamp should update");
        db.execute(
            "UPDATE session_messages SET created_at = ?2 WHERE id = ?1",
            &[
                &newer.id as &dyn rusqlite::ToSql,
                &"2026-04-25T10:05:00Z".to_string(),
            ],
        )
        .expect("newer timestamp should update");

        let _ = session_activate_for_db(
            &db,
            SessionActivateRequest {
                id: active_session.id.clone(),
                reason: Some("manual_resume".to_string()),
            },
        )
        .expect("active session should persist");

        let snapshot = session_replay_snapshot_for_db(
            &db,
            Some(SessionReplaySnapshotRequest {
                session_id: Some(format!("  {}  ", replay_session.id)),
                limit: Some(10),
            }),
        )
        .expect("replay snapshot should resolve");

        assert_eq!(snapshot.resolved_via, "session_id");
        assert_eq!(
            snapshot.session.expect("selected session should exist").id,
            replay_session.id
        );
        assert!(snapshot.active_selection.is_none());
        assert_eq!(snapshot.messages.len(), 2);
        assert_eq!(snapshot.messages[0].content, "First prompt");
        assert_eq!(snapshot.messages[1].content, "Second reply");
    }

    #[test]
    fn session_replay_snapshot_for_db_falls_back_to_active_selection() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db.clone());
        let active_session = service
            .create(CreateSessionInput {
                source: SessionSource::Desktop,
                title: "Manual resume".to_string(),
                model_name: Some("gpt-5.4".to_string()),
                parent_session_id: None,
            })
            .expect("active session should create");
        let latest_session = service
            .create(CreateSessionInput {
                source: SessionSource::Cli,
                title: "Latest only".to_string(),
                model_name: Some("gpt-4o".to_string()),
                parent_session_id: None,
            })
            .expect("latest session should create");

        let _ = service
            .create_message(CreateSessionMessageInput {
                session_id: active_session.id.clone(),
                role: SessionMessageRole::Assistant,
                content: "Resume from here".to_string(),
                source: "local".to_string(),
            })
            .expect("active session message should create");
        let _ = service
            .create_message(CreateSessionMessageInput {
                session_id: latest_session.id.clone(),
                role: SessionMessageRole::Assistant,
                content: "Latest session message".to_string(),
                source: "local".to_string(),
            })
            .expect("latest session message should create");

        let _ = session_activate_for_db(
            &db,
            SessionActivateRequest {
                id: active_session.id.clone(),
                reason: Some("resume_by_title".to_string()),
            },
        )
        .expect("active session should persist");

        let snapshot =
            session_replay_snapshot_for_db(&db, None).expect("replay snapshot should resolve");

        assert_eq!(snapshot.resolved_via, "active_session");
        assert_eq!(
            snapshot.session.expect("selected session should exist").id,
            active_session.id
        );
        let active = snapshot
            .active_selection
            .expect("active selection metadata should be included");
        assert_eq!(active.session.id, active_session.id);
        assert_eq!(active.reason, "resume_by_title");
        assert_eq!(snapshot.messages.len(), 1);
        assert_eq!(snapshot.messages[0].content, "Resume from here");
    }

    #[test]
    fn session_replay_snapshot_for_db_falls_back_to_latest_session() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db.clone());
        let _older = service
            .create(CreateSessionInput {
                source: SessionSource::Cli,
                title: "Older".to_string(),
                model_name: Some("gpt-4o".to_string()),
                parent_session_id: None,
            })
            .expect("older session should create");
        let latest = service
            .create(CreateSessionInput {
                source: SessionSource::Desktop,
                title: "Latest".to_string(),
                model_name: Some("gpt-5.4".to_string()),
                parent_session_id: None,
            })
            .expect("latest session should create");

        let _ = service
            .create_message(CreateSessionMessageInput {
                session_id: latest.id.clone(),
                role: SessionMessageRole::User,
                content: "Latest transcript".to_string(),
                source: "local".to_string(),
            })
            .expect("latest session message should create");

        let snapshot =
            session_replay_snapshot_for_db(&db, None).expect("replay snapshot should resolve");

        assert_eq!(snapshot.resolved_via, "latest_session");
        assert_eq!(
            snapshot.session.expect("selected session should exist").id,
            latest.id
        );
        assert!(snapshot.active_selection.is_none());
        assert_eq!(snapshot.messages.len(), 1);
        assert_eq!(snapshot.messages[0].content, "Latest transcript");
    }

    #[test]
    fn session_replay_snapshot_for_db_returns_none_payload_when_no_session_resolves() {
        let db = Database::in_memory().expect("database should initialize");

        let snapshot =
            session_replay_snapshot_for_db(&db, None).expect("replay snapshot should resolve");

        assert_eq!(snapshot.resolved_via, "none");
        assert!(snapshot.session.is_none());
        assert!(snapshot.active_selection.is_none());
        assert!(snapshot.messages.is_empty());
    }
}
