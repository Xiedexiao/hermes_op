//! Messaging gateway 业务服务

use chrono::Utc;

use crate::backend::domain::{
    CreateGatewayConversationInput, CreateGatewayMessageInput, CreateSessionInput,
    GatewayDirection, GatewayIngestMessageInput, GatewayIngestResult, GatewayMessage,
    GatewayMessageListFilter, GatewaySource,
};
use crate::backend::storage::GatewayRepository;
use crate::backend::{
    AppError, AppResult, Database, GatewayConversation, SessionService, SessionServiceImpl,
};

pub trait GatewayService: Send + Sync {
    fn ingest_message(&self, input: GatewayIngestMessageInput) -> AppResult<GatewayIngestResult>;
    fn list_recent_conversations(&self, limit: usize) -> AppResult<Vec<GatewayConversation>>;
    fn list_recent_messages(
        &self,
        filter: GatewayMessageListFilter,
    ) -> AppResult<Vec<GatewayMessage>>;
}

pub struct GatewayServiceImpl {
    db: Database,
    repo: GatewayRepository,
}

impl GatewayServiceImpl {
    pub fn new(db: Database) -> Self {
        Self {
            repo: GatewayRepository::new(db.clone()),
            db,
        }
    }

    fn session_service(&self) -> SessionServiceImpl {
        SessionServiceImpl::new(self.db.clone())
    }
}

impl GatewayService for GatewayServiceImpl {
    fn ingest_message(&self, input: GatewayIngestMessageInput) -> AppResult<GatewayIngestResult> {
        let GatewayIngestMessageInput {
            source,
            external_conversation_id,
            external_thread_id,
            external_message_id,
            channel_name,
            participant_display,
            direction,
            sender_id,
            sender_display,
            subject,
            body,
            payload_json,
            received_at,
        } = input;

        let conversation = match self.repo.find_conversation(
            &source,
            &external_conversation_id,
            &external_thread_id,
        )? {
            Some(existing) => existing,
            None => {
                let session = self.session_service().create(CreateSessionInput {
                    source: source.to_session_source(),
                    title: build_session_title(
                        &source,
                        channel_name.as_deref(),
                        participant_display.as_deref(),
                        &external_conversation_id,
                    ),
                    model_name: None,
                    parent_session_id: None,
                })?;

                self.repo
                    .create_conversation(CreateGatewayConversationInput {
                        source: source.clone(),
                        external_conversation_id: external_conversation_id.clone(),
                        external_thread_id: external_thread_id.clone(),
                        channel_name: channel_name.clone(),
                        participant_display: participant_display.clone(),
                        session_id: session.id,
                        last_message_at: received_at.clone(),
                    })?
            }
        };

        let message = self.repo.create_message(CreateGatewayMessageInput {
            conversation_id: conversation.id.clone(),
            session_id: conversation.session_id.clone(),
            source,
            external_message_id,
            direction,
            sender_id,
            sender_display,
            subject,
            body,
            payload_json,
            received_at: received_at.clone(),
        })?;

        let conversation = self.repo.update_conversation_activity(
            &conversation.id,
            channel_name.as_deref(),
            participant_display.as_deref(),
            &received_at,
        )?;
        self.repo
            .touch_session(&conversation.session_id, &received_at)?;
        self.repo.create_session_memory_record(
            &conversation.session_id,
            &message.source,
            message.subject.as_deref().unwrap_or("Gateway message"),
            &message.body,
            &message.received_at,
        )?;

        Ok(GatewayIngestResult {
            conversation,
            message,
        })
    }

    fn list_recent_conversations(&self, limit: usize) -> AppResult<Vec<GatewayConversation>> {
        self.repo.list_recent_conversations(normalize_limit(limit))
    }

    fn list_recent_messages(
        &self,
        filter: GatewayMessageListFilter,
    ) -> AppResult<Vec<GatewayMessage>> {
        self.repo.list_recent_messages(&GatewayMessageListFilter {
            conversation_id: filter.conversation_id,
            session_id: filter.session_id,
            limit: normalize_limit(filter.limit),
        })
    }
}

pub fn parse_gateway_source(value: &str) -> AppResult<GatewaySource> {
    GatewaySource::from_key(value).ok_or_else(|| {
        AppError::validation(
            "gateway source must be telegram, discord, slack, whatsapp, signal, or email",
        )
    })
}

pub fn parse_gateway_direction(value: Option<&str>) -> AppResult<GatewayDirection> {
    match value {
        Some(raw) => GatewayDirection::from_key(raw)
            .ok_or_else(|| AppError::validation("gateway direction must be inbound or outbound")),
        None => Ok(GatewayDirection::Inbound),
    }
}

pub fn normalize_required(value: &str, field_name: &str) -> AppResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(AppError::validation(format!(
            "{} cannot be empty",
            field_name
        )));
    }

    Ok(normalized.to_string())
}

pub fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let normalized = value.trim();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized.to_string())
        }
    })
}

pub fn resolve_received_at(value: Option<String>) -> AppResult<String> {
    match value {
        Some(value) => normalize_required(&value, "received_at"),
        None => Ok(Utc::now().to_rfc3339()),
    }
}

fn normalize_limit(limit: usize) -> usize {
    limit.max(1)
}

fn build_session_title(
    source: &GatewaySource,
    channel_name: Option<&str>,
    participant_display: Option<&str>,
    external_conversation_id: &str,
) -> String {
    let target = channel_name
        .filter(|value| !value.is_empty())
        .or(participant_display.filter(|value| !value.is_empty()))
        .unwrap_or(external_conversation_id);
    format!("{} {}", source.as_str(), target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::SessionSource;

    #[test]
    fn parse_gateway_source_maps_supported_values() {
        assert_eq!(
            parse_gateway_source("Slack")
                .expect("slack should parse")
                .to_session_source(),
            SessionSource::Slack
        );
    }

    #[test]
    fn parse_gateway_direction_defaults_to_inbound() {
        assert_eq!(
            parse_gateway_direction(None).expect("direction should default"),
            GatewayDirection::Inbound
        );
    }

    #[test]
    fn normalize_required_rejects_blank_values() {
        let error = normalize_required("   ", "body").expect_err("blank should fail");
        assert_eq!(error.code, "validation_error");
    }
}
