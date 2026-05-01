//! Session 数据仓储

use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::backend::domain::{
    CreateSessionInput, CreateSessionMessageInput, Session, SessionMessage,
    SessionMessageHistoryQuery, SessionMessageRole, SessionSource,
};
use crate::backend::{AppError, AppResult, Database};

#[derive(Clone)]
pub struct SessionRepository {
    db: Database,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecentSessionEvidence {
    pub id: String,
    pub source: SessionSource,
    pub title: String,
    pub model_name: Option<String>,
    pub parent_session_id: Option<String>,
    pub started_at: String,
    pub updated_at: String,
    pub ended_at: Option<String>,
}

impl From<Session> for RecentSessionEvidence {
    fn from(session: Session) -> Self {
        Self {
            id: session.id,
            source: session.source,
            title: session.title,
            model_name: session.model_name,
            parent_session_id: session.parent_session_id,
            started_at: session.started_at,
            updated_at: session.updated_at,
            ended_at: session.ended_at,
        }
    }
}

impl SessionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create(&self, input: CreateSessionInput) -> AppResult<Session> {
        let now = Utc::now().to_rfc3339();
        let session = Session {
            id: Uuid::new_v4().to_string(),
            source: input.source,
            title: input.title,
            model_name: input.model_name,
            parent_session_id: input.parent_session_id,
            started_at: now.clone(),
            updated_at: now,
            ended_at: None,
        };

        self.db.execute(
            "INSERT INTO sessions (
                id, source, title, model_name, parent_session_id,
                started_at, updated_at, ended_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &session.id as &dyn rusqlite::ToSql,
                &session.source.as_str(),
                &session.title,
                &session.model_name,
                &session.parent_session_id,
                &session.started_at,
                &session.updated_at,
                &session.ended_at,
            ],
        )?;

        Ok(session)
    }

    pub fn list_recent(&self, limit: usize) -> AppResult<Vec<Session>> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, source, title, model_name, parent_session_id,
                    started_at, updated_at, ended_at
                 FROM sessions
                 ORDER BY datetime(updated_at) DESC, rowid DESC
                 LIMIT ?1",
            )?;

            let rows = stmt.query_map(params![limit as i64], map_session_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    pub fn get_latest(&self) -> AppResult<Option<Session>> {
        Ok(self.list_recent(1)?.into_iter().next())
    }

    pub fn list_recent_evidence(&self, limit: usize) -> AppResult<Vec<RecentSessionEvidence>> {
        self.list_recent(limit).map(|sessions| {
            sessions
                .into_iter()
                .map(RecentSessionEvidence::from)
                .collect()
        })
    }

    pub fn get_by_id(&self, id: &str) -> AppResult<Option<Session>> {
        match self.db.query_row(
            "SELECT
                id, source, title, model_name, parent_session_id,
                started_at, updated_at, ended_at
             FROM sessions
             WHERE id = ?1",
            &[&id],
            map_session_row,
        ) {
            Ok(session) => Ok(Some(session)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::storage(format!("Failed to fetch session: {}", e))),
        }
    }

    pub fn rename(&self, id: &str, title: String) -> AppResult<Session> {
        let updated_at = Utc::now().to_rfc3339();
        self.db.execute(
            "UPDATE sessions
             SET title = ?2, updated_at = ?3
             WHERE id = ?1",
            &[&id as &dyn rusqlite::ToSql, &title, &updated_at],
        )?;

        self.get_by_id(id)?
            .ok_or_else(|| AppError::storage(format!("session not found: {}", id)))
    }

    pub fn create_message(&self, input: CreateSessionMessageInput) -> AppResult<SessionMessage> {
        let created_at = Utc::now().to_rfc3339();
        let item = SessionMessage {
            id: Uuid::new_v4().to_string(),
            session_id: input.session_id,
            role: input.role,
            content: input.content,
            source: input.source,
            created_at: created_at.clone(),
        };

        self.db.execute(
            "INSERT INTO session_messages (id, session_id, role, content, metadata_json, created_at)
             VALUES (?, ?, ?, ?, NULL, ?)",
            &[
                &item.id as &dyn rusqlite::ToSql,
                &item.session_id,
                &item.role.as_str(),
                &item.content,
                &item.created_at,
            ],
        )?;

        self.db.execute(
            "UPDATE sessions SET updated_at = ?2 WHERE id = ?1",
            &[&item.session_id as &dyn rusqlite::ToSql, &created_at],
        )?;

        Ok(item)
    }

    pub fn list_message_history(
        &self,
        query: &SessionMessageHistoryQuery,
    ) -> AppResult<Vec<SessionMessage>> {
        self.db.with_connection(|conn| {
            let mut items = Vec::new();
            let role = query.role.as_ref().map(SessionMessageRole::as_str);
            let text_query = query.text_query.as_deref();

            let mut stmt = conn.prepare(
                "SELECT id, session_id, role, content, created_at
                 FROM session_messages
                 WHERE session_id = ?1
                   AND (?2 IS NULL OR role = ?2)
                   AND (?3 IS NULL OR lower(content) LIKE '%' || lower(?3) || '%')
                 ORDER BY datetime(created_at) DESC, rowid DESC
                 LIMIT ?4",
            )?;
            let rows = stmt.query_map(
                params![query.session_id, role, text_query, query.limit as i64],
                |row| {
                    Ok(SessionMessage {
                        id: row.get(0)?,
                        session_id: row.get(1)?,
                        role: SessionMessageRole::from_key(&row.get::<_, String>(2)?),
                        content: row.get(3)?,
                        source: "local".to_string(),
                        created_at: row.get(4)?,
                    })
                },
            )?;
            items.extend(rows.collect::<Result<Vec<_>, _>>()?);

            let gateway_exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='gateway_messages'",
                [],
                |row| row.get(0),
            )?;
            if gateway_exists > 0 {
                let mut gateway_stmt = conn.prepare(
                    "SELECT id, session_id, direction, body, received_at
                     FROM gateway_messages
                     WHERE session_id = ?1
                       AND (
                           ?2 IS NULL
                           OR CASE
                               WHEN direction = 'inbound' THEN 'user'
                               ELSE 'assistant'
                           END = ?2
                       )
                       AND (?3 IS NULL OR lower(body) LIKE '%' || lower(?3) || '%')
                     ORDER BY datetime(received_at) DESC, rowid DESC
                     LIMIT ?4",
                )?;
                let gateway_rows = gateway_stmt.query_map(
                    params![query.session_id, role, text_query, query.limit as i64],
                    |row| {
                        let direction: String = row.get(2)?;
                        Ok(SessionMessage {
                            id: row.get(0)?,
                            session_id: row.get(1)?,
                            role: if direction == "inbound" {
                                SessionMessageRole::User
                            } else {
                                SessionMessageRole::Assistant
                            },
                            content: row.get(3)?,
                            source: "gateway".to_string(),
                            created_at: row.get(4)?,
                        })
                    },
                )?;
                items.extend(gateway_rows.collect::<Result<Vec<_>, _>>()?);
            }

            items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
            items.truncate(query.limit);
            Ok(items)
        })
    }

    pub fn search(&self, query: &str, limit: usize) -> AppResult<Vec<Session>> {
        let normalized = query.trim();
        if normalized.is_empty() {
            return Ok(Vec::new());
        }

        self.db.with_connection(|conn| {
            let gateway_exists: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='gateway_messages'",
                [],
                |row| row.get(0),
            )?;
            let sql = if gateway_exists > 0 {
                r#"
                WITH candidate_sessions AS (
                    SELECT
                        id, source, title, model_name, parent_session_id,
                        started_at, updated_at, ended_at,
                        sessions.rowid AS session_rowid,
                        EXISTS(
                            SELECT 1
                            FROM session_messages
                            WHERE session_id = sessions.id
                              AND lower(content) LIKE '%' || lower(?1) || '%'
                        ) AS local_match,
                        EXISTS(
                            SELECT 1
                            FROM gateway_messages
                            WHERE session_id = sessions.id
                              AND lower(body) LIKE '%' || lower(?1) || '%'
                        ) AS gateway_match
                    FROM sessions
                )
                SELECT
                    id, source, title, model_name, parent_session_id,
                    started_at, updated_at, ended_at
                FROM candidate_sessions
                WHERE
                    lower(id) = lower(?1)
                    OR lower(title) = lower(?1)
                    OR lower(id) LIKE '%' || lower(?1) || '%'
                    OR lower(title) LIKE '%' || lower(?1) || '%'
                    OR lower(source) LIKE '%' || lower(?1) || '%'
                    OR lower(COALESCE(model_name, '')) LIKE '%' || lower(?1) || '%'
                    OR lower(COALESCE(parent_session_id, '')) LIKE '%' || lower(?1) || '%'
                    OR local_match
                    OR gateway_match
                ORDER BY
                    CASE
                        WHEN lower(title) = lower(?1) THEN 0
                        WHEN lower(id) = lower(?1) THEN 1
                        WHEN lower(title) LIKE '%' || lower(?1) || '%' THEN 2
                        WHEN lower(id) LIKE '%' || lower(?1) || '%' THEN 3
                        WHEN lower(source) LIKE '%' || lower(?1) || '%' THEN 4
                        WHEN lower(COALESCE(model_name, '')) LIKE '%' || lower(?1) || '%' THEN 5
                        WHEN lower(COALESCE(parent_session_id, '')) LIKE '%' || lower(?1) || '%' THEN 6
                        WHEN local_match THEN 7
                        WHEN gateway_match THEN 8
                        ELSE 9
                    END,
                    datetime(updated_at) DESC,
                    session_rowid DESC
                LIMIT ?2
                "#
            } else {
                r#"
                WITH candidate_sessions AS (
                    SELECT
                        id, source, title, model_name, parent_session_id,
                        started_at, updated_at, ended_at,
                        sessions.rowid AS session_rowid,
                        EXISTS(
                            SELECT 1
                            FROM session_messages
                            WHERE session_id = sessions.id
                              AND lower(content) LIKE '%' || lower(?1) || '%'
                        ) AS local_match
                    FROM sessions
                )
                SELECT
                    id, source, title, model_name, parent_session_id,
                    started_at, updated_at, ended_at
                FROM candidate_sessions
                WHERE
                    lower(id) = lower(?1)
                    OR lower(title) = lower(?1)
                    OR lower(id) LIKE '%' || lower(?1) || '%'
                    OR lower(title) LIKE '%' || lower(?1) || '%'
                    OR lower(source) LIKE '%' || lower(?1) || '%'
                    OR lower(COALESCE(model_name, '')) LIKE '%' || lower(?1) || '%'
                    OR lower(COALESCE(parent_session_id, '')) LIKE '%' || lower(?1) || '%'
                    OR local_match
                ORDER BY
                    CASE
                        WHEN lower(title) = lower(?1) THEN 0
                        WHEN lower(id) = lower(?1) THEN 1
                        WHEN lower(title) LIKE '%' || lower(?1) || '%' THEN 2
                        WHEN lower(id) LIKE '%' || lower(?1) || '%' THEN 3
                        WHEN lower(source) LIKE '%' || lower(?1) || '%' THEN 4
                        WHEN lower(COALESCE(model_name, '')) LIKE '%' || lower(?1) || '%' THEN 5
                        WHEN lower(COALESCE(parent_session_id, '')) LIKE '%' || lower(?1) || '%' THEN 6
                        WHEN local_match THEN 7
                        ELSE 8
                    END,
                    datetime(updated_at) DESC,
                    session_rowid DESC
                LIMIT ?2
                "#
            };

            let mut stmt = conn.prepare(sql)?;

            let rows = stmt.query_map(params![normalized, limit as i64], map_session_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }
}

fn map_session_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Session> {
    Ok(Session {
        id: row.get(0)?,
        source: SessionSource::from_key(&row.get::<_, String>(1)?),
        title: row.get(2)?,
        model_name: row.get(3)?,
        parent_session_id: row.get(4)?,
        started_at: row.get(5)?,
        updated_at: row.get(6)?,
        ended_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_input(title: &str) -> CreateSessionInput {
        CreateSessionInput {
            source: SessionSource::Cli,
            title: title.to_string(),
            model_name: Some("gpt-4o".to_string()),
            parent_session_id: None,
        }
    }

    #[test]
    fn create_persists_and_returns_session() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db);

        let session = repo
            .create(sample_input("首次会话"))
            .expect("session should be created");

        assert_eq!(session.title, "首次会话");
        assert_eq!(session.source, SessionSource::Cli);
        assert_eq!(session.model_name.as_deref(), Some("gpt-4o"));
        assert!(!session.id.is_empty());
    }

    #[test]
    fn list_recent_returns_latest_first() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db);

        let _ = repo.create(sample_input("第一个")).expect("first session");
        let _ = repo.create(sample_input("第二个")).expect("second session");

        let sessions = repo.list_recent(10).expect("sessions should list");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].title, "第二个");
        assert_eq!(sessions[1].title, "第一个");
    }

    #[test]
    fn get_by_id_returns_session_and_none_for_missing() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db);

        let created = repo
            .create(sample_input("会话"))
            .expect("session should create");
        let found = repo
            .get_by_id(&created.id)
            .expect("lookup should work")
            .expect("session should exist");
        assert_eq!(found.id, created.id);

        assert!(
            repo.get_by_id("missing-id")
                .expect("missing lookup should work")
                .is_none()
        );
    }

    #[test]
    fn get_latest_returns_none_when_repository_is_empty() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db);

        assert!(
            repo.get_latest()
                .expect("latest lookup should work")
                .is_none()
        );
    }

    #[test]
    fn get_latest_returns_most_recent_session() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db);

        let first = repo.create(sample_input("第一个")).expect("first session");
        let second = repo.create(sample_input("第二个")).expect("second session");

        let latest = repo
            .get_latest()
            .expect("latest lookup should work")
            .expect("latest session should exist");

        assert_eq!(latest.id, second.id);
        assert_ne!(latest.id, first.id);
    }

    #[test]
    fn create_and_list_message_history_merges_local_and_gateway_messages() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db.clone());
        let session = repo
            .create(sample_input("History session"))
            .expect("session should create");

        repo.create_message(CreateSessionMessageInput {
            session_id: session.id.clone(),
            role: SessionMessageRole::Note,
            content: "Local recovery note".to_string(),
            source: "local".to_string(),
        })
        .expect("local message should create");

        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS gateway_conversations (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                external_conversation_id TEXT NOT NULL,
                external_thread_id TEXT NOT NULL DEFAULT '',
                channel_name TEXT,
                participant_display TEXT,
                session_id TEXT NOT NULL,
                last_message_at TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS gateway_messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                source TEXT NOT NULL,
                external_message_id TEXT NOT NULL,
                direction TEXT NOT NULL,
                sender_id TEXT,
                sender_display TEXT,
                subject TEXT,
                body TEXT NOT NULL,
                payload_json TEXT,
                received_at TEXT NOT NULL
            );
            "#,
        )
        .expect("gateway schema should create");
        db.execute(
            "INSERT INTO gateway_messages (
                id, conversation_id, session_id, source, external_message_id,
                direction, sender_id, sender_display, subject, body, payload_json, received_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"gateway-msg-1" as &dyn rusqlite::ToSql,
                &"conversation-1",
                &session.id,
                &"slack",
                &"external-1",
                &"inbound",
                &Option::<String>::None,
                &Option::<String>::None,
                &Option::<String>::None,
                &"Inbound gateway message".to_string(),
                &Option::<String>::None,
                &"2026-04-26T00:00:00Z".to_string(),
            ],
        )
        .expect("gateway message should insert");

        let history = repo
            .list_message_history(&SessionMessageHistoryQuery {
                session_id: session.id.clone(),
                limit: 20,
                role: None,
                text_query: None,
            })
            .expect("history should list");

        assert_eq!(history.len(), 2);
        assert!(history.iter().any(|item| item.source == "local"));
        assert!(history.iter().any(|item| item.source == "gateway"));
    }

    #[test]
    fn list_message_history_filters_by_role_when_requested() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db.clone());
        let session = repo
            .create(sample_input("Filtered history session"))
            .expect("session should create");

        repo.create_message(CreateSessionMessageInput {
            session_id: session.id.clone(),
            role: SessionMessageRole::Note,
            content: "Local note".to_string(),
            source: "local".to_string(),
        })
        .expect("local message should create");

        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS gateway_conversations (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                external_conversation_id TEXT NOT NULL,
                external_thread_id TEXT NOT NULL DEFAULT '',
                channel_name TEXT,
                participant_display TEXT,
                session_id TEXT NOT NULL,
                last_message_at TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS gateway_messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                source TEXT NOT NULL,
                external_message_id TEXT NOT NULL,
                direction TEXT NOT NULL,
                sender_id TEXT,
                sender_display TEXT,
                subject TEXT,
                body TEXT NOT NULL,
                payload_json TEXT,
                received_at TEXT NOT NULL
            );
            "#,
        )
        .expect("gateway schema should create");
        db.execute(
            "INSERT INTO gateway_messages (
                id, conversation_id, session_id, source, external_message_id,
                direction, sender_id, sender_display, subject, body, payload_json, received_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"gateway-msg-2" as &dyn rusqlite::ToSql,
                &"conversation-2",
                &session.id,
                &"slack",
                &"external-2",
                &"outbound",
                &Option::<String>::None,
                &Option::<String>::None,
                &Option::<String>::None,
                &"Gateway assistant reply".to_string(),
                &Option::<String>::None,
                &"2026-04-26T00:00:01Z".to_string(),
            ],
        )
        .expect("gateway message should insert");

        let history = repo
            .list_message_history(&crate::backend::domain::SessionMessageHistoryQuery {
                session_id: session.id.clone(),
                limit: 20,
                role: Some(SessionMessageRole::Assistant),
                text_query: None,
            })
            .expect("history should list");

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].source, "gateway");
        assert_eq!(history[0].role, SessionMessageRole::Assistant);
    }

    #[test]
    fn list_message_history_filters_by_text_query_case_insensitively() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db.clone());
        let session = repo
            .create(sample_input("Search history session"))
            .expect("session should create");

        repo.create_message(CreateSessionMessageInput {
            session_id: session.id.clone(),
            role: SessionMessageRole::Note,
            content: "Need Retry on blocked sync".to_string(),
            source: "local".to_string(),
        })
        .expect("local message should create");
        repo.create_message(CreateSessionMessageInput {
            session_id: session.id.clone(),
            role: SessionMessageRole::User,
            content: "Something else".to_string(),
            source: "local".to_string(),
        })
        .expect("second local message should create");

        let history = repo
            .list_message_history(&crate::backend::domain::SessionMessageHistoryQuery {
                session_id: session.id.clone(),
                limit: 20,
                role: None,
                text_query: Some("retry".to_string()),
            })
            .expect("history should list");

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].content, "Need Retry on blocked sync");
    }

    #[test]
    fn list_recent_evidence_returns_recent_sessions_with_heuristic_fields() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db);

        let _older = repo
            .create(CreateSessionInput {
                source: SessionSource::Cli,
                title: "Older session".to_string(),
                model_name: Some("gpt-4o".to_string()),
                parent_session_id: None,
            })
            .expect("older session should create");
        let latest = repo
            .create(CreateSessionInput {
                source: SessionSource::Desktop,
                title: "Recover failed sync".to_string(),
                model_name: Some("gpt-5.4".to_string()),
                parent_session_id: Some("session-parent".to_string()),
            })
            .expect("latest session should create");

        let evidence = repo
            .list_recent_evidence(2)
            .expect("recent evidence should list");

        assert_eq!(evidence.len(), 2);
        assert_eq!(evidence[0].id, latest.id);
        assert_eq!(evidence[0].source, SessionSource::Desktop);
        assert_eq!(evidence[0].title, "Recover failed sync");
        assert_eq!(evidence[0].model_name.as_deref(), Some("gpt-5.4"));
        assert_eq!(
            evidence[0].parent_session_id.as_deref(),
            Some("session-parent")
        );
        assert!(!evidence[0].started_at.is_empty());
        assert!(!evidence[0].updated_at.is_empty());
        assert_eq!(evidence[0].ended_at, None);
        assert_eq!(evidence[1].title, "Older session");
    }

    #[test]
    fn rename_updates_title_and_updated_at() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db);

        let created = repo
            .create(sample_input("旧标题"))
            .expect("session should create");

        let renamed = repo
            .rename(&created.id, "新标题".to_string())
            .expect("rename should work");

        assert_eq!(renamed.id, created.id);
        assert_eq!(renamed.title, "新标题");
        assert_ne!(renamed.updated_at, created.updated_at);

        let found = repo
            .get_by_id(&created.id)
            .expect("lookup should work")
            .expect("session should exist");
        assert_eq!(found.title, "新标题");
        assert_eq!(found.updated_at, renamed.updated_at);
    }

    #[test]
    fn search_returns_matches_beyond_recent_window_and_ranks_exact_titles_first() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db);

        for index in 0..25 {
            let _ = repo
                .create(sample_input(&format!("Noise session {:02}", index)))
                .expect("noise session should create");
        }

        let exact = repo
            .create(sample_input("Quarterly planning"))
            .expect("exact session should create");
        let partial = repo
            .create(sample_input("Quarterly planning follow-up"))
            .expect("partial session should create");

        let matches = repo
            .search("Quarterly planning", 10)
            .expect("search should succeed");

        assert!(!matches.is_empty());
        assert_eq!(matches[0].id, exact.id);
        assert!(matches.iter().any(|session| session.id == partial.id));
    }

    #[test]
    fn search_matches_transcript_content_from_local_session_messages() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db);
        let session = repo
            .create(sample_input("Operations follow-up"))
            .expect("session should create");

        repo.create_message(CreateSessionMessageInput {
            session_id: session.id.clone(),
            role: SessionMessageRole::Assistant,
            content: "Need retry on blocked sync after deploy".to_string(),
            source: "local".to_string(),
        })
        .expect("message should create");

        let matches = repo
            .search("blocked sync", 10)
            .expect("search should succeed");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, session.id);
    }

    #[test]
    fn search_matches_transcript_content_from_gateway_messages() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db.clone());
        let session = repo
            .create(sample_input("Customer escalation"))
            .expect("session should create");

        db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS gateway_conversations (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                external_conversation_id TEXT NOT NULL,
                external_thread_id TEXT NOT NULL DEFAULT '',
                channel_name TEXT,
                participant_display TEXT,
                session_id TEXT NOT NULL,
                last_message_at TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS gateway_messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                source TEXT NOT NULL,
                external_message_id TEXT NOT NULL,
                direction TEXT NOT NULL,
                sender_id TEXT,
                sender_display TEXT,
                subject TEXT,
                body TEXT NOT NULL,
                payload_json TEXT,
                received_at TEXT NOT NULL
            );
            "#,
        )
        .expect("gateway schema should create");
        db.execute(
            "INSERT INTO gateway_messages (
                id, conversation_id, session_id, source, external_message_id,
                direction, sender_id, sender_display, subject, body, payload_json, received_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"gateway-search-msg" as &dyn rusqlite::ToSql,
                &"conversation-search",
                &session.id,
                &"slack",
                &"external-search",
                &"inbound",
                &Option::<String>::None,
                &Option::<String>::None,
                &Option::<String>::None,
                &"Escalate the blocked sync before tonight's cutover".to_string(),
                &Option::<String>::None,
                &"2026-04-26T00:00:01Z".to_string(),
            ],
        )
        .expect("gateway message should insert");

        let matches = repo.search("cutover", 10).expect("search should succeed");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, session.id);
    }
}
