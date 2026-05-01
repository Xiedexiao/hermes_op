//! Messaging gateway commands.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::backend::{
    AppError, Database, GatewayConversation, GatewayIngestMessageInput, GatewayIngestResult,
    GatewayMessage, GatewayMessageListFilter, GatewayService, GatewayServiceImpl,
    normalize_optional, normalize_required, parse_gateway_direction, parse_gateway_source,
    resolve_received_at,
};

const DEFAULT_RECENT_LIMIT: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayIngestMessageRequest {
    pub source: String,
    pub external_conversation_id: String,
    #[serde(default)]
    pub external_thread_id: Option<String>,
    pub external_message_id: String,
    #[serde(default)]
    pub channel_name: Option<String>,
    #[serde(default)]
    pub participant_display: Option<String>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub sender_id: Option<String>,
    #[serde(default)]
    pub sender_display: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    pub body: String,
    #[serde(default)]
    pub payload_json: Option<serde_json::Value>,
    #[serde(default)]
    pub received_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayListRecentConversationsRequest {
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayListRecentMessagesRequest {
    #[serde(default)]
    pub conversation_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    pub limit: Option<usize>,
}

fn service(db: &Database) -> GatewayServiceImpl {
    GatewayServiceImpl::new(db.clone())
}

impl GatewayIngestMessageRequest {
    fn into_service_input(self) -> Result<GatewayIngestMessageInput, AppError> {
        Ok(GatewayIngestMessageInput {
            source: parse_gateway_source(&self.source)?,
            external_conversation_id: normalize_required(
                &self.external_conversation_id,
                "external_conversation_id",
            )?,
            external_thread_id: normalize_optional(self.external_thread_id).unwrap_or_default(),
            external_message_id: normalize_required(
                &self.external_message_id,
                "external_message_id",
            )?,
            channel_name: normalize_optional(self.channel_name),
            participant_display: normalize_optional(self.participant_display),
            direction: parse_gateway_direction(self.direction.as_deref())?,
            sender_id: normalize_optional(self.sender_id),
            sender_display: normalize_optional(self.sender_display),
            subject: normalize_optional(self.subject),
            body: normalize_required(&self.body, "body")?,
            payload_json: self.payload_json,
            received_at: resolve_received_at(self.received_at)?,
        })
    }
}

fn resolve_recent_limit(limit: Option<usize>) -> usize {
    limit.unwrap_or(DEFAULT_RECENT_LIMIT).max(1)
}

pub fn gateway_ingest_message_for_db(
    db: &Database,
    request: GatewayIngestMessageRequest,
) -> Result<GatewayIngestResult, AppError> {
    service(db).ingest_message(request.into_service_input()?)
}

#[tauri::command]
pub fn gateway_ingest_message(
    db: State<'_, Database>,
    request: GatewayIngestMessageRequest,
) -> Result<GatewayIngestResult, AppError> {
    gateway_ingest_message_for_db(db.inner(), request)
}

pub fn gateway_list_recent_conversations_for_db(
    db: &Database,
    request: Option<GatewayListRecentConversationsRequest>,
) -> Result<Vec<GatewayConversation>, AppError> {
    service(db)
        .list_recent_conversations(resolve_recent_limit(request.and_then(|value| value.limit)))
}

#[tauri::command]
pub fn gateway_list_recent_conversations(
    db: State<'_, Database>,
    request: Option<GatewayListRecentConversationsRequest>,
) -> Result<Vec<GatewayConversation>, AppError> {
    gateway_list_recent_conversations_for_db(db.inner(), request)
}

pub fn gateway_list_recent_messages_for_db(
    db: &Database,
    request: GatewayListRecentMessagesRequest,
) -> Result<Vec<GatewayMessage>, AppError> {
    service(db).list_recent_messages(GatewayMessageListFilter {
        conversation_id: normalize_optional(request.conversation_id),
        session_id: normalize_optional(request.session_id),
        limit: resolve_recent_limit(request.limit),
    })
}

#[tauri::command]
pub fn gateway_list_recent_messages(
    db: State<'_, Database>,
    request: GatewayListRecentMessagesRequest,
) -> Result<Vec<GatewayMessage>, AppError> {
    gateway_list_recent_messages_for_db(db.inner(), request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gateway_ingest_request_normalizes_fields() {
        let input = GatewayIngestMessageRequest {
            source: " telegram ".to_string(),
            external_conversation_id: " convo ".to_string(),
            external_thread_id: Some("   ".to_string()),
            external_message_id: " msg ".to_string(),
            channel_name: Some(" channel ".to_string()),
            participant_display: Some(" display ".to_string()),
            direction: None,
            sender_id: Some(" sender ".to_string()),
            sender_display: Some(" sender display ".to_string()),
            subject: Some(" subject ".to_string()),
            body: " body ".to_string(),
            payload_json: None,
            received_at: Some("2026-04-23T10:00:00Z".to_string()),
        };

        let normalized = input
            .into_service_input()
            .expect("request should normalize");
        assert_eq!(normalized.source.as_str(), "telegram");
        assert_eq!(normalized.external_thread_id, "");
        assert_eq!(normalized.body, "body");
    }
}
