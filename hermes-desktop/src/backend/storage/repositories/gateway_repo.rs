//! Messaging gateway 数据仓储

use chrono::Utc;
use rusqlite::{OptionalExtension, params};
use uuid::Uuid;

use crate::backend::domain::{
    CreateGatewayConversationInput, CreateGatewayMessageInput, GatewayConversation, GatewayMessage,
    GatewayMessageListFilter, GatewaySource,
};
use crate::backend::{AppError, AppResult, Database, GatewayDirection};

#[derive(Clone)]
pub struct GatewayRepository {
    db: Database,
}

impl GatewayRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn find_conversation(
        &self,
        source: &GatewaySource,
        external_conversation_id: &str,
        external_thread_id: &str,
    ) -> AppResult<Option<GatewayConversation>> {
        self.ensure_schema()?;
        self.db
            .query_row(
                "SELECT
                    id, source, external_conversation_id, external_thread_id,
                    channel_name, participant_display, session_id, last_message_at, created_at
                 FROM gateway_conversations
                 WHERE source = ?1
                   AND external_conversation_id = ?2
                   AND external_thread_id = ?3",
                &[
                    &source.as_str() as &dyn rusqlite::ToSql,
                    &external_conversation_id,
                    &external_thread_id,
                ],
                map_conversation_row,
            )
            .optional()
            .map_err(|err| {
                AppError::storage(format!("Failed to fetch gateway conversation: {}", err))
            })
    }

    pub fn create_conversation(
        &self,
        input: CreateGatewayConversationInput,
    ) -> AppResult<GatewayConversation> {
        self.ensure_schema()?;
        let conversation = GatewayConversation {
            id: Uuid::new_v4().to_string(),
            source: input.source,
            external_conversation_id: input.external_conversation_id,
            external_thread_id: input.external_thread_id,
            channel_name: input.channel_name,
            participant_display: input.participant_display,
            session_id: input.session_id,
            last_message_at: input.last_message_at,
            created_at: Utc::now().to_rfc3339(),
        };

        self.db.execute(
            "INSERT INTO gateway_conversations (
                id, source, external_conversation_id, external_thread_id,
                channel_name, participant_display, session_id, last_message_at, created_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &conversation.id as &dyn rusqlite::ToSql,
                &conversation.source.as_str(),
                &conversation.external_conversation_id,
                &conversation.external_thread_id,
                &conversation.channel_name,
                &conversation.participant_display,
                &conversation.session_id,
                &conversation.last_message_at,
                &conversation.created_at,
            ],
        )?;

        Ok(conversation)
    }

    pub fn update_conversation_activity(
        &self,
        id: &str,
        channel_name: Option<&str>,
        participant_display: Option<&str>,
        last_message_at: &str,
    ) -> AppResult<GatewayConversation> {
        self.ensure_schema()?;
        self.db.execute(
            "UPDATE gateway_conversations
             SET channel_name = COALESCE(?2, channel_name),
                 participant_display = COALESCE(?3, participant_display),
                 last_message_at = ?4
             WHERE id = ?1",
            &[
                &id as &dyn rusqlite::ToSql,
                &channel_name,
                &participant_display,
                &last_message_at,
            ],
        )?;

        self.get_conversation(id)?
            .ok_or_else(|| AppError::storage(format!("gateway conversation not found: {}", id)))
    }

    pub fn get_conversation(&self, id: &str) -> AppResult<Option<GatewayConversation>> {
        self.ensure_schema()?;
        self.db
            .query_row(
                "SELECT
                    id, source, external_conversation_id, external_thread_id,
                    channel_name, participant_display, session_id, last_message_at, created_at
                 FROM gateway_conversations
                 WHERE id = ?1",
                &[&id],
                map_conversation_row,
            )
            .optional()
            .map_err(|err| {
                AppError::storage(format!("Failed to fetch gateway conversation: {}", err))
            })
    }

    pub fn list_recent_conversations(&self, limit: usize) -> AppResult<Vec<GatewayConversation>> {
        self.ensure_schema()?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, source, external_conversation_id, external_thread_id,
                    channel_name, participant_display, session_id, last_message_at, created_at
                 FROM gateway_conversations
                 ORDER BY datetime(last_message_at) DESC, rowid DESC
                 LIMIT ?1",
            )?;

            let rows = stmt.query_map(params![limit as i64], map_conversation_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    pub fn create_message(&self, input: CreateGatewayMessageInput) -> AppResult<GatewayMessage> {
        self.ensure_schema()?;
        let payload_json = input
            .payload_json
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(AppError::from_json_error)?;
        let message = GatewayMessage {
            id: Uuid::new_v4().to_string(),
            conversation_id: input.conversation_id,
            session_id: input.session_id,
            source: input.source,
            external_message_id: input.external_message_id,
            direction: input.direction,
            sender_id: input.sender_id,
            sender_display: input.sender_display,
            subject: input.subject,
            body: input.body,
            payload_json: input.payload_json,
            received_at: input.received_at,
        };

        self.db.execute(
            "INSERT INTO gateway_messages (
                id, conversation_id, session_id, source, external_message_id,
                direction, sender_id, sender_display, subject, body, payload_json, received_at
             ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &message.id as &dyn rusqlite::ToSql,
                &message.conversation_id,
                &message.session_id,
                &message.source.as_str(),
                &message.external_message_id,
                &message.direction.as_str(),
                &message.sender_id,
                &message.sender_display,
                &message.subject,
                &message.body,
                &payload_json,
                &message.received_at,
            ],
        )?;

        Ok(message)
    }

    pub fn touch_session(&self, session_id: &str, updated_at: &str) -> AppResult<()> {
        self.db.execute(
            "UPDATE sessions SET updated_at = ?2 WHERE id = ?1",
            &[&session_id as &dyn rusqlite::ToSql, &updated_at],
        )?;
        Ok(())
    }

    pub fn create_session_memory_record(
        &self,
        session_id: &str,
        source: &GatewaySource,
        title: &str,
        content: &str,
        created_at: &str,
    ) -> AppResult<()> {
        let id = Uuid::new_v4().to_string();
        let scope = "session".to_string();
        let source_type = format!("gateway:{}", source.as_str());
        let importance = "medium".to_string();
        self.db.execute(
            "INSERT INTO memory_records
             (id, scope, scope_ref, title, content, source_type, importance, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &id as &dyn rusqlite::ToSql,
                &scope,
                &session_id,
                &title,
                &content,
                &source_type,
                &importance,
                &created_at,
            ],
        )?;
        Ok(())
    }

    pub fn list_recent_messages(
        &self,
        filter: &GatewayMessageListFilter,
    ) -> AppResult<Vec<GatewayMessage>> {
        self.ensure_schema()?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, conversation_id, session_id, source, external_message_id,
                    direction, sender_id, sender_display, subject, body, payload_json, received_at
                 FROM gateway_messages
                 WHERE (?1 IS NULL OR conversation_id = ?1)
                   AND (?2 IS NULL OR session_id = ?2)
                 ORDER BY datetime(received_at) DESC, rowid DESC
                 LIMIT ?3",
            )?;

            let rows = stmt.query_map(
                params![
                    filter.conversation_id.as_deref(),
                    filter.session_id.as_deref(),
                    filter.limit as i64
                ],
                map_message_row,
            )?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn ensure_schema(&self) -> AppResult<()> {
        self.db.execute_batch(
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
                created_at TEXT NOT NULL,
                UNIQUE(source, external_conversation_id, external_thread_id),
                FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
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
                received_at TEXT NOT NULL,
                UNIQUE(source, external_message_id),
                FOREIGN KEY(conversation_id) REFERENCES gateway_conversations(id) ON DELETE CASCADE,
                FOREIGN KEY(session_id) REFERENCES sessions(id) ON DELETE CASCADE
            );
            "#,
        )
    }
}

fn map_conversation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<GatewayConversation> {
    Ok(GatewayConversation {
        id: row.get(0)?,
        source: parse_source(&row.get::<_, String>(1)?, 1)?,
        external_conversation_id: row.get(2)?,
        external_thread_id: row.get(3)?,
        channel_name: row.get(4)?,
        participant_display: row.get(5)?,
        session_id: row.get(6)?,
        last_message_at: row.get(7)?,
        created_at: row.get(8)?,
    })
}

fn map_message_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<GatewayMessage> {
    let payload_json = row
        .get::<_, Option<String>>(10)?
        .map(|json| {
            serde_json::from_str(&json).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    10,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })
        })
        .transpose()?;

    Ok(GatewayMessage {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        session_id: row.get(2)?,
        source: parse_source(&row.get::<_, String>(3)?, 3)?,
        external_message_id: row.get(4)?,
        direction: parse_direction(&row.get::<_, String>(5)?, 5)?,
        sender_id: row.get(6)?,
        sender_display: row.get(7)?,
        subject: row.get(8)?,
        body: row.get(9)?,
        payload_json,
        received_at: row.get(11)?,
    })
}

fn parse_source(value: &str, column: usize) -> rusqlite::Result<GatewaySource> {
    GatewaySource::from_key(value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            format!("invalid gateway source: {}", value).into(),
        )
    })
}

fn parse_direction(value: &str, column: usize) -> rusqlite::Result<GatewayDirection> {
    GatewayDirection::from_key(value).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            format!("invalid gateway direction: {}", value).into(),
        )
    })
}
