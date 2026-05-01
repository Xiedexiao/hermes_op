use super::CliError;
use hermes_desktop::backend::{
    Database, Session, SessionMessage, SessionMessageHistoryQuery, SessionService,
    SessionServiceImpl, create_app_state,
};
use hermes_desktop::commands::sessions::session_get_active_for_db;

const ACTIVE_SESSION_KEY: &str = "active_session_selection";
const SESSION_HISTORY_LIMIT: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionListItem {
    pub id: String,
    pub source: String,
    pub title: String,
    pub model_name: Option<String>,
}

pub fn load_sessions() -> Result<Vec<SessionListItem>, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    let db = Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))?;
    let service = SessionServiceImpl::new(db);
    let sessions = service
        .list_recent(20)
        .map_err(|err| CliError::Runtime(err.to_string()))?;

    Ok(sessions
        .into_iter()
        .map(|session| SessionListItem {
            id: session.id,
            source: session.source.as_str().to_string(),
            title: session.title,
            model_name: session.model_name,
        })
        .collect())
}

pub fn get_session(id: &str) -> Result<Option<String>, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    let db = Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))?;
    let service = SessionServiceImpl::new(db);
    let session = service
        .get_by_id(id)
        .map_err(|err| CliError::Runtime(err.to_string()))?;

    Ok(session.map(|item| {
        render_session_detail(
            &item.id,
            item.source.as_str(),
            &item.title,
            item.model_name.as_deref(),
        )
    }))
}

pub fn get_latest_session() -> Result<Option<String>, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    let db = Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))?;
    let service = SessionServiceImpl::new(db);
    let latest = service
        .list_recent(1)
        .map_err(|err| CliError::Runtime(err.to_string()))?;

    Ok(latest.into_iter().next().map(|item| {
        render_session_detail(
            &item.id,
            item.source.as_str(),
            &item.title,
            item.model_name.as_deref(),
        )
    }))
}

pub fn get_session_history(selector: &str) -> Result<String, CliError> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err(CliError::InvalidUsage(
            "usage: /sessions history <session-id|active|latest>\n".to_string(),
        ));
    }

    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    let db = Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))?;
    let service = SessionServiceImpl::new(db.clone());
    let resolved = resolve_history_session(&db, &service, selector)?;
    let Some((resolved_via, session)) = resolved else {
        return Ok(match selector.to_ascii_lowercase().as_str() {
            "active" => "session_history\tresolved_via=active\tnone\n".to_string(),
            "latest" => "no sessions found\n".to_string(),
            _ => "session not found\n".to_string(),
        });
    };

    let mut messages = service
        .list_message_history(SessionMessageHistoryQuery {
            session_id: session.id.clone(),
            limit: SESSION_HISTORY_LIMIT,
            role: None,
            text_query: None,
        })
        .map_err(|err| CliError::Runtime(err.to_string()))?;
    messages.reverse();

    Ok(render_session_history(
        resolved_via,
        &map_session_to_list_item(session),
        &messages,
    ))
}

pub fn rename_session(id: &str, title: &str) -> Result<String, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    let db = Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))?;
    let service = SessionServiceImpl::new(db);
    let (id, title) = normalize_rename_input(id, title);
    let session = service
        .rename(&id, title)
        .map_err(|err| CliError::Runtime(err.to_string()))?;

    Ok(render_session_detail(
        &session.id,
        session.source.as_str(),
        &session.title,
        session.model_name.as_deref(),
    ))
}

pub fn get_active_session() -> Result<String, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    let db = Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))?;
    let active =
        session_get_active_for_db(&db).map_err(|err| CliError::Runtime(err.to_string()))?;
    Ok(match active {
        Some(active) => format!(
            "sessions\tactive\tid={}\tsource={}\ttitle={}\tmodel={}\treason={}\tactivated_at={}\n",
            active.session.id,
            active.session.source.as_str(),
            active.session.title,
            active.session.model_name.as_deref().unwrap_or("-"),
            active.reason,
            active.activated_at,
        ),
        None => "sessions\tactive\tnone\n".to_string(),
    })
}

pub fn clear_active_session() -> Result<bool, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    let db = Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))?;
    db.execute(
        "DELETE FROM app_settings WHERE key = ?1",
        &[&ACTIVE_SESSION_KEY as &dyn rusqlite::ToSql],
    )
    .map_err(|err| CliError::Runtime(err.to_string()))?;
    Ok(true)
}

pub fn render_list(sessions: &[SessionListItem]) -> String {
    if sessions.is_empty() {
        return "no sessions found\n".to_string();
    }

    let mut output = String::new();
    for session in sessions {
        output.push_str(&format!(
            "{}\t{}\t{}\n",
            session.id, session.source, session.title
        ));
    }
    output
}

pub fn render_match(session: &SessionListItem) -> String {
    render_session_detail(
        &session.id,
        &session.source,
        &session.title,
        session.model_name.as_deref(),
    )
}

pub fn render_title(session: &SessionListItem) -> String {
    format!("title\t{}\t{}\n", session.id, session.title)
}

fn render_session_history(
    resolved_via: &str,
    session: &SessionListItem,
    messages: &[SessionMessage],
) -> String {
    let mut output = format!(
        "session_history\tresolved_via={resolved_via}\tsession_id={}\tsource={}\ttitle={}\tcount={}\n",
        session.id,
        session.source,
        session.title,
        messages.len(),
    );
    for message in messages {
        let content_json =
            serde_json::to_string(&message.content).unwrap_or_else(|_| "\"\"".to_string());
        output.push_str(&format!(
            "session_message\tsession_id={}\trole={}\tsource={}\tcontent_json={content_json}\n",
            session.id,
            message.role.as_str(),
            message.source,
        ));
    }
    output
}

pub fn search_recent<'a>(sessions: &'a [SessionListItem], query: &str) -> Vec<&'a SessionListItem> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let normalized = trimmed.to_lowercase();
    sessions
        .iter()
        .filter(|session| {
            session.id.to_lowercase().contains(&normalized)
                || session.source.to_lowercase().contains(&normalized)
                || session.title.to_lowercase().contains(&normalized)
        })
        .collect()
}

pub fn find_resume_candidate<'a>(
    sessions: &'a [SessionListItem],
    selector: &str,
) -> Option<&'a SessionListItem> {
    let trimmed = selector.trim();
    if trimmed.is_empty() {
        return None;
    }

    let normalized = trimmed.to_lowercase();
    sessions
        .iter()
        .find(|session| session.id.eq_ignore_ascii_case(trimmed))
        .or_else(|| {
            sessions
                .iter()
                .find(|session| session.title.eq_ignore_ascii_case(trimmed))
        })
        .or_else(|| {
            sessions
                .iter()
                .find(|session| session.title.to_lowercase().contains(&normalized))
        })
        .or_else(|| {
            sessions
                .iter()
                .find(|session| session.id.to_lowercase().contains(&normalized))
        })
}

fn render_session_detail(id: &str, source: &str, title: &str, model_name: Option<&str>) -> String {
    format!(
        "{}\t{}\t{}\t{}\n",
        id,
        source,
        title,
        model_name.unwrap_or("-")
    )
}

fn normalize_rename_input(id: &str, title: &str) -> (String, String) {
    (id.trim().to_string(), title.trim().to_string())
}

fn resolve_history_session(
    db: &Database,
    service: &SessionServiceImpl,
    selector: &str,
) -> Result<Option<(&'static str, Session)>, CliError> {
    if selector.eq_ignore_ascii_case("latest") {
        return service
            .get_latest()
            .map(|session| session.map(|item| ("latest", item)))
            .map_err(|err| CliError::Runtime(err.to_string()));
    }

    if selector.eq_ignore_ascii_case("active") {
        return session_get_active_for_db(db)
            .map(|active| active.map(|item| ("active", item.session)))
            .map_err(|err| CliError::Runtime(err.to_string()));
    }

    service
        .get_by_id(selector)
        .map(|session| session.map(|item| ("session_id", item)))
        .map_err(|err| CliError::Runtime(err.to_string()))
}

fn map_session_to_list_item(session: Session) -> SessionListItem {
    SessionListItem {
        id: session.id,
        source: session.source.as_str().to_string(),
        title: session.title,
        model_name: session.model_name,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_desktop::backend::{SessionMessage, SessionMessageRole};

    #[test]
    fn renders_empty_session_list() {
        assert_eq!(render_list(&[]), "no sessions found\n");
    }

    #[test]
    fn renders_session_rows() {
        let rendered = render_list(&[SessionListItem {
            id: "session-001".to_string(),
            source: "cli".to_string(),
            title: "Recent session".to_string(),
            model_name: Some("gpt-4o".to_string()),
        }]);

        assert_eq!(rendered, "session-001\tcli\tRecent session\n");
    }

    #[test]
    fn renders_session_detail_with_model() {
        let rendered =
            render_session_detail("session-001", "cli", "Latest session", Some("gpt-4o"));

        assert_eq!(rendered, "session-001\tcli\tLatest session\tgpt-4o\n");
    }

    #[test]
    fn renders_session_detail_without_model() {
        let rendered = render_session_detail("session-001", "cli", "Latest session", None);

        assert_eq!(rendered, "session-001\tcli\tLatest session\t-\n");
    }

    #[test]
    fn normalize_rename_input_trims_id_and_title() {
        let (id, title) = normalize_rename_input("  session-001  ", "  Renamed session  ");

        assert_eq!(id, "session-001");
        assert_eq!(title, "Renamed session");
    }

    #[test]
    fn search_recent_matches_id_source_and_title_case_insensitively() {
        let sessions = vec![
            SessionListItem {
                id: "session-001".to_string(),
                source: "cli".to_string(),
                title: "Quarterly Planning".to_string(),
                model_name: Some("gpt-5.4".to_string()),
            },
            SessionListItem {
                id: "desktop-002".to_string(),
                source: "desktop".to_string(),
                title: "Roadmap Review".to_string(),
                model_name: None,
            },
        ];

        assert_eq!(search_recent(&sessions, "quarterly").len(), 1);
        assert_eq!(search_recent(&sessions, "CLI").len(), 1);
        assert_eq!(search_recent(&sessions, "desktop-002").len(), 1);
    }

    #[test]
    fn find_resume_candidate_prefers_exact_id_then_title_then_partial_title() {
        let sessions = vec![
            SessionListItem {
                id: "session-001".to_string(),
                source: "cli".to_string(),
                title: "Quarterly Planning".to_string(),
                model_name: Some("gpt-5.4".to_string()),
            },
            SessionListItem {
                id: "session-002".to_string(),
                source: "cli".to_string(),
                title: "Quarterly Planning Review".to_string(),
                model_name: None,
            },
        ];

        assert_eq!(
            find_resume_candidate(&sessions, "session-001").map(|session| session.id.as_str()),
            Some("session-001")
        );
        assert_eq!(
            find_resume_candidate(&sessions, "Quarterly Planning")
                .map(|session| session.id.as_str()),
            Some("session-001")
        );
        assert_eq!(
            find_resume_candidate(&sessions, "planning review").map(|session| session.id.as_str()),
            Some("session-002")
        );
    }

    #[test]
    fn render_match_and_title_use_detail_and_title_formats() {
        let session = SessionListItem {
            id: "session-001".to_string(),
            source: "cli".to_string(),
            title: "Planning".to_string(),
            model_name: Some("gpt-5.4".to_string()),
        };

        assert_eq!(
            render_match(&session),
            "session-001\tcli\tPlanning\tgpt-5.4\n"
        );
        assert_eq!(render_title(&session), "title\tsession-001\tPlanning\n");
    }

    #[test]
    fn render_session_history_uses_chronological_message_rows_with_json_content() {
        let session = SessionListItem {
            id: "session-001".to_string(),
            source: "cli".to_string(),
            title: "Planning".to_string(),
            model_name: Some("gpt-5.4".to_string()),
        };
        let messages = vec![
            SessionMessage {
                id: "message-001".to_string(),
                session_id: session.id.clone(),
                role: SessionMessageRole::User,
                content: "line with\ttab".to_string(),
                source: "local".to_string(),
                created_at: "2026-04-26T10:00:00Z".to_string(),
            },
            SessionMessage {
                id: "message-002".to_string(),
                session_id: session.id.clone(),
                role: SessionMessageRole::Assistant,
                content: "line one\nline two".to_string(),
                source: "gateway".to_string(),
                created_at: "2026-04-26T10:01:00Z".to_string(),
            },
        ];

        assert_eq!(
            render_session_history("latest", &session, &messages),
            concat!(
                "session_history\tresolved_via=latest\tsession_id=session-001\tsource=cli\ttitle=Planning\tcount=2\n",
                "session_message\tsession_id=session-001\trole=user\tsource=local\tcontent_json=\"line with\\ttab\"\n",
                "session_message\tsession_id=session-001\trole=assistant\tsource=gateway\tcontent_json=\"line one\\nline two\"\n",
            )
        );
    }
}
