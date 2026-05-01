//! Messaging gateway 领域模型

use serde::{Deserialize, Serialize};

use crate::backend::domain::SessionSource;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatewaySource {
    Telegram,
    Discord,
    Slack,
    Whatsapp,
    Signal,
    Email,
}

impl GatewaySource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Telegram => "telegram",
            Self::Discord => "discord",
            Self::Slack => "slack",
            Self::Whatsapp => "whatsapp",
            Self::Signal => "signal",
            Self::Email => "email",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "telegram" => Some(Self::Telegram),
            "discord" => Some(Self::Discord),
            "slack" => Some(Self::Slack),
            "whatsapp" => Some(Self::Whatsapp),
            "signal" => Some(Self::Signal),
            "email" => Some(Self::Email),
            _ => None,
        }
    }

    pub fn to_session_source(&self) -> SessionSource {
        match self {
            Self::Telegram => SessionSource::Telegram,
            Self::Discord => SessionSource::Discord,
            Self::Slack => SessionSource::Slack,
            Self::Whatsapp => SessionSource::Whatsapp,
            Self::Signal => SessionSource::Signal,
            Self::Email => SessionSource::Email,
        }
    }
}

impl PartialEq<&str> for GatewaySource {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GatewayDirection {
    Inbound,
    Outbound,
}

impl PartialEq<&str> for GatewayDirection {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl GatewayDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Inbound => "inbound",
            Self::Outbound => "outbound",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "inbound" => Some(Self::Inbound),
            "outbound" => Some(Self::Outbound),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayConversation {
    pub id: String,
    pub source: GatewaySource,
    pub external_conversation_id: String,
    pub external_thread_id: String,
    pub channel_name: Option<String>,
    pub participant_display: Option<String>,
    pub session_id: String,
    pub last_message_at: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayMessage {
    pub id: String,
    pub conversation_id: String,
    pub session_id: String,
    pub source: GatewaySource,
    pub external_message_id: String,
    pub direction: GatewayDirection,
    pub sender_id: Option<String>,
    pub sender_display: Option<String>,
    pub subject: Option<String>,
    pub body: String,
    pub payload_json: Option<serde_json::Value>,
    pub received_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayIngestResult {
    pub conversation: GatewayConversation,
    pub message: GatewayMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayIngestMessageInput {
    pub source: GatewaySource,
    pub external_conversation_id: String,
    pub external_thread_id: String,
    pub external_message_id: String,
    pub channel_name: Option<String>,
    pub participant_display: Option<String>,
    pub direction: GatewayDirection,
    pub sender_id: Option<String>,
    pub sender_display: Option<String>,
    pub subject: Option<String>,
    pub body: String,
    pub payload_json: Option<serde_json::Value>,
    pub received_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateGatewayConversationInput {
    pub source: GatewaySource,
    pub external_conversation_id: String,
    pub external_thread_id: String,
    pub channel_name: Option<String>,
    pub participant_display: Option<String>,
    pub session_id: String,
    pub last_message_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CreateGatewayMessageInput {
    pub conversation_id: String,
    pub session_id: String,
    pub source: GatewaySource,
    pub external_message_id: String,
    pub direction: GatewayDirection,
    pub sender_id: Option<String>,
    pub sender_display: Option<String>,
    pub subject: Option<String>,
    pub body: String,
    pub payload_json: Option<serde_json::Value>,
    pub received_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GatewayMessageListFilter {
    pub conversation_id: Option<String>,
    pub session_id: Option<String>,
    pub limit: usize,
}
