//! Session 领域模型

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionSource {
    Cli,
    Desktop,
    Telegram,
    Discord,
    Slack,
    Whatsapp,
    Signal,
    Email,
    Cron,
    Unknown,
}

impl SessionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Cli => "cli",
            Self::Desktop => "desktop",
            Self::Telegram => "telegram",
            Self::Discord => "discord",
            Self::Slack => "slack",
            Self::Whatsapp => "whatsapp",
            Self::Signal => "signal",
            Self::Email => "email",
            Self::Cron => "cron",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            "cli" => Self::Cli,
            "desktop" => Self::Desktop,
            "telegram" => Self::Telegram,
            "discord" => Self::Discord,
            "slack" => Self::Slack,
            "whatsapp" => Self::Whatsapp,
            "signal" => Self::Signal,
            "email" => Self::Email,
            "cron" => Self::Cron,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub source: SessionSource,
    pub title: String,
    pub model_name: Option<String>,
    pub parent_session_id: Option<String>,
    pub started_at: String,
    pub updated_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateSessionInput {
    pub source: SessionSource,
    pub title: String,
    pub model_name: Option<String>,
    pub parent_session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionMessageRole {
    User,
    Assistant,
    System,
    Tool,
    Note,
}

impl SessionMessageRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::System => "system",
            Self::Tool => "tool",
            Self::Note => "note",
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            "user" => Self::User,
            "assistant" => Self::Assistant,
            "system" => Self::System,
            "tool" => Self::Tool,
            _ => Self::Note,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionMessage {
    pub id: String,
    pub session_id: String,
    pub role: SessionMessageRole,
    pub content: String,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateSessionMessageInput {
    pub session_id: String,
    pub role: SessionMessageRole,
    pub content: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionMessageHistoryQuery {
    pub session_id: String,
    pub limit: usize,
    pub role: Option<SessionMessageRole>,
    pub text_query: Option<String>,
}
