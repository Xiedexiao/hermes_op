//! Session 业务服务

use crate::backend::domain::{
    CreateSessionInput, CreateSessionMessageInput, Session, SessionMessage,
    SessionMessageHistoryQuery,
};
use crate::backend::storage::{RecentSessionEvidence, SessionRepository};
use crate::backend::{AppError, AppResult, Database};

pub trait SessionService: Send + Sync {
    fn create(&self, input: CreateSessionInput) -> AppResult<Session>;
    fn create_message(&self, input: CreateSessionMessageInput) -> AppResult<SessionMessage>;
    fn list_recent(&self, limit: usize) -> AppResult<Vec<Session>>;
    fn list_recent_evidence(&self, limit: usize) -> AppResult<Vec<RecentSessionEvidence>>;
    fn list_message_history(
        &self,
        query: SessionMessageHistoryQuery,
    ) -> AppResult<Vec<SessionMessage>>;
    fn search(&self, query: &str, limit: usize) -> AppResult<Vec<Session>>;
    fn get_latest(&self) -> AppResult<Option<Session>>;
    fn get_by_id(&self, id: &str) -> AppResult<Option<Session>>;
    fn rename(&self, id: &str, title: String) -> AppResult<Session>;
}

pub struct SessionServiceImpl {
    repo: SessionRepository,
}

impl SessionServiceImpl {
    pub fn new(db: Database) -> Self {
        Self {
            repo: SessionRepository::new(db),
        }
    }
}

impl SessionService for SessionServiceImpl {
    fn create(&self, input: CreateSessionInput) -> AppResult<Session> {
        if input.title.trim().is_empty() {
            return Err(AppError::validation("session title cannot be empty"));
        }

        self.repo.create(input)
    }

    fn create_message(&self, input: CreateSessionMessageInput) -> AppResult<SessionMessage> {
        if input.session_id.trim().is_empty() {
            return Err(AppError::validation("session id cannot be empty"));
        }
        if input.content.trim().is_empty() {
            return Err(AppError::validation(
                "session message content cannot be empty",
            ));
        }
        self.repo.create_message(input)
    }

    fn list_recent(&self, limit: usize) -> AppResult<Vec<Session>> {
        self.repo.list_recent(limit)
    }

    fn list_recent_evidence(&self, limit: usize) -> AppResult<Vec<RecentSessionEvidence>> {
        self.repo.list_recent_evidence(limit)
    }

    fn list_message_history(
        &self,
        mut query: SessionMessageHistoryQuery,
    ) -> AppResult<Vec<SessionMessage>> {
        query.session_id = query.session_id.trim().to_string();
        if query.session_id.is_empty() {
            return Err(AppError::validation("session id cannot be empty"));
        }
        query.text_query = query
            .text_query
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        self.repo.list_message_history(&query)
    }

    fn search(&self, query: &str, limit: usize) -> AppResult<Vec<Session>> {
        let normalized = query.trim();
        if normalized.is_empty() {
            return Ok(Vec::new());
        }

        self.repo.search(normalized, limit)
    }

    fn get_latest(&self) -> AppResult<Option<Session>> {
        self.repo.get_latest()
    }

    fn get_by_id(&self, id: &str) -> AppResult<Option<Session>> {
        self.repo.get_by_id(id)
    }

    fn rename(&self, id: &str, title: String) -> AppResult<Session> {
        if title.trim().is_empty() {
            return Err(AppError::validation("session title cannot be empty"));
        }

        self.repo.rename(id, title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::domain::{SessionMessageHistoryQuery, SessionMessageRole, SessionSource};

    fn sample_input(title: &str) -> CreateSessionInput {
        CreateSessionInput {
            source: SessionSource::Cli,
            title: title.to_string(),
            model_name: Some("gpt-4o".to_string()),
            parent_session_id: None,
        }
    }

    #[test]
    fn create_rejects_blank_title() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db);

        let result = service.create(sample_input("   "));
        assert!(result.is_err());
    }

    #[test]
    fn create_and_list_recent_round_trip() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db);

        let created = service
            .create(sample_input("CLI 会话"))
            .expect("session should create");
        let sessions = service.list_recent(10).expect("sessions should list");

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].id, created.id);
    }

    #[test]
    fn get_by_id_returns_existing_session() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db);

        let created = service
            .create(sample_input("恢复会话"))
            .expect("create should work");
        let found = service
            .get_by_id(&created.id)
            .expect("lookup should work")
            .expect("session should exist");

        assert_eq!(found.title, "恢复会话");
    }

    #[test]
    fn get_latest_returns_none_when_no_session_exists() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db);

        assert!(
            service
                .get_latest()
                .expect("latest lookup should work")
                .is_none()
        );
    }

    #[test]
    fn get_latest_returns_most_recent_session() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db);

        let _ = service
            .create(sample_input("第一个"))
            .expect("first create");
        let second = service
            .create(sample_input("第二个"))
            .expect("second create");

        let latest = service
            .get_latest()
            .expect("latest lookup should work")
            .expect("latest session should exist");

        assert_eq!(latest.id, second.id);
        assert_eq!(latest.title, "第二个");
    }

    #[test]
    fn rename_rejects_blank_title() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db);
        let created = service
            .create(sample_input("原标题"))
            .expect("create should work");

        let result = service.rename(&created.id, "   ".to_string());

        assert!(result.is_err());
    }

    #[test]
    fn rename_updates_session_title() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db);
        let created = service
            .create(sample_input("原标题"))
            .expect("create should work");

        let renamed = service
            .rename(&created.id, "新标题".to_string())
            .expect("rename should work");

        assert_eq!(renamed.id, created.id);
        assert_eq!(renamed.title, "新标题");

        let found = service
            .get_by_id(&created.id)
            .expect("lookup should work")
            .expect("session should exist");
        assert_eq!(found.title, "新标题");
    }

    #[test]
    fn search_lists_persisted_matches_beyond_recent_only_views() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db);

        for index in 0..24 {
            service
                .create(sample_input(&format!("Noise session {:02}", index)))
                .expect("noise session should create");
        }

        let exact = service
            .create(sample_input("Quarterly planning"))
            .expect("exact session should create");

        let matches = service
            .search("Quarterly planning", 5)
            .expect("search should succeed");

        assert!(!matches.is_empty());
        assert_eq!(matches[0].id, exact.id);
    }

    #[test]
    fn list_recent_evidence_exposes_recent_session_summaries_for_heuristics() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db);

        let _older = service
            .create(sample_input("Older"))
            .expect("older session should create");
        let latest = service
            .create(CreateSessionInput {
                source: SessionSource::Desktop,
                title: "Retry blocked sync".to_string(),
                model_name: Some("gpt-5.4".to_string()),
                parent_session_id: Some("session-parent".to_string()),
            })
            .expect("latest session should create");

        let evidence = service
            .list_recent_evidence(2)
            .expect("recent evidence should list");

        assert_eq!(evidence.len(), 2);
        assert_eq!(evidence[0].id, latest.id);
        assert_eq!(evidence[0].title, "Retry blocked sync");
        assert_eq!(evidence[0].source, SessionSource::Desktop);
        assert_eq!(evidence[0].model_name.as_deref(), Some("gpt-5.4"));
        assert_eq!(
            evidence[0].parent_session_id.as_deref(),
            Some("session-parent")
        );
    }

    #[test]
    fn create_message_and_list_message_history_round_trip() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db.clone());
        let session = service
            .create(sample_input("History session"))
            .expect("session should create");

        let created = service
            .create_message(CreateSessionMessageInput {
                session_id: session.id.clone(),
                role: crate::backend::domain::SessionMessageRole::Note,
                content: "Remember the blocked follow-up".to_string(),
                source: "local".to_string(),
            })
            .expect("message should create");

        let history = service
            .list_message_history(SessionMessageHistoryQuery {
                session_id: session.id.clone(),
                limit: 20,
                role: None,
                text_query: None,
            })
            .expect("history should list");

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, created.id);
        assert_eq!(history[0].content, "Remember the blocked follow-up");
    }

    #[test]
    fn list_message_history_treats_blank_text_query_as_no_filter() {
        let db = Database::in_memory().expect("database should initialize");
        let service = SessionServiceImpl::new(db.clone());
        let session = service
            .create(sample_input("History session"))
            .expect("session should create");

        let created = service
            .create_message(CreateSessionMessageInput {
                session_id: session.id.clone(),
                role: SessionMessageRole::Note,
                content: "Remember the blocked follow-up".to_string(),
                source: "local".to_string(),
            })
            .expect("message should create");

        let history = service
            .list_message_history(SessionMessageHistoryQuery {
                session_id: format!("  {}  ", session.id),
                limit: 20,
                role: None,
                text_query: Some("   ".to_string()),
            })
            .expect("history should list");

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].id, created.id);
    }
}
