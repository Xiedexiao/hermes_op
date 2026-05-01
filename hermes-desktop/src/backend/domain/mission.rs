//! Mission 领域模型

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MissionStatus {
    Draft,
    Researching,
    Simulating,
    Planning,
    AwaitingApproval,
    Executing,
    Paused,
    Completed,
    Failed,
    Archived,
}

impl MissionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Researching => "researching",
            Self::Simulating => "simulating",
            Self::Planning => "planning",
            Self::AwaitingApproval => "awaiting_approval",
            Self::Executing => "executing",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Archived => "archived",
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            "researching" => Self::Researching,
            "simulating" => Self::Simulating,
            "planning" => Self::Planning,
            "awaiting_approval" => Self::AwaitingApproval,
            "executing" => Self::Executing,
            "paused" => Self::Paused,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "archived" => Self::Archived,
            _ => Self::Draft,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MissionPriority {
    Low,
    Medium,
    High,
}

impl MissionPriority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            "low" => Self::Low,
            "high" => Self::High,
            _ => Self::Medium,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Mission {
    pub id: String,
    pub title: String,
    pub goal: String,
    pub constraints: Vec<String>,
    pub success_criteria: Vec<String>,
    pub status: MissionStatus,
    pub priority: MissionPriority,
    pub pinned: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_activity_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContextItemType {
    File,
    Url,
    Note,
    Memory,
    KnowledgeResult,
    Artifact,
}

impl ContextItemType {
    pub fn from_key(value: &str) -> Self {
        match value {
            "file" => Self::File,
            "url" => Self::Url,
            "memory" => Self::Memory,
            "knowledge_result" => Self::KnowledgeResult,
            "artifact" => Self::Artifact,
            _ => Self::Note,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissionContextItem {
    pub id: String,
    pub mission_id: String,
    pub r#type: ContextItemType,
    pub title: String,
    pub content_preview: Option<String>,
    pub source_uri: Option<String>,
    pub pinned: bool,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateMissionContextItemInput {
    pub mission_id: String,
    pub r#type: ContextItemType,
    pub title: String,
    pub content_preview: Option<String>,
    pub source_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct KnowledgeSource {
    pub id: String,
    pub r#type: String,
    pub title: String,
    pub source_uri: String,
    pub index_status: String,
    pub chunk_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateKnowledgeChunkInput {
    pub content: String,
    pub metadata_json: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateKnowledgeSourceInput {
    pub r#type: String,
    pub title: String,
    pub source_uri: String,
    pub index_status: String,
    pub chunk_count: i64,
    pub updated_at: String,
    pub chunks: Vec<CreateKnowledgeChunkInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunType {
    Research,
    Simulation,
    Council,
    Execution,
    Growth,
}

impl RunType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Research => "research",
            Self::Simulation => "simulation",
            Self::Council => "council",
            Self::Execution => "execution",
            Self::Growth => "growth",
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            "simulation" => Self::Simulation,
            "council" => Self::Council,
            "execution" => Self::Execution,
            "growth" => Self::Growth,
            _ => Self::Research,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_key(value: &str) -> Self {
        match value {
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            _ => Self::Queued,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Run {
    pub id: String,
    pub mission_id: String,
    pub r#type: RunType,
    pub status: RunStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub summary: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactType {
    Markdown,
    Report,
    Plan,
    Json,
    Text,
    Image,
    File,
}

impl ArtifactType {
    pub fn from_key(value: &str) -> Self {
        match value {
            "report" => Self::Report,
            "plan" => Self::Plan,
            "json" => Self::Json,
            "text" => Self::Text,
            "image" => Self::Image,
            "file" => Self::File,
            _ => Self::Markdown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Artifact {
    pub id: String,
    pub mission_id: String,
    pub run_id: Option<String>,
    pub r#type: ArtifactType,
    pub title: String,
    pub path: String,
    pub mime_type: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissionDetail {
    pub mission: Mission,
    #[serde(default)]
    pub context_items: Vec<MissionContextItem>,
    #[serde(default)]
    pub runs: Vec<Run>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
}

impl MissionDetail {
    pub fn from_mission(mission: Mission) -> Self {
        Self {
            mission,
            context_items: vec![],
            runs: vec![],
            artifacts: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CreateMissionInput {
    pub title: String,
    pub goal: String,
    pub constraints: Vec<String>,
    pub success_criteria: Vec<String>,
    pub priority: MissionPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateMissionInput {
    pub id: String,
    pub title: String,
    pub goal: String,
    pub constraints: Vec<String>,
    pub success_criteria: Vec<String>,
    pub priority: MissionPriority,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MissionListFilter {
    pub query: Option<String>,
    pub status: Option<MissionStatus>,
    pub limit: Option<usize>,
}

impl MissionListFilter {
    pub fn normalized(self) -> Self {
        Self {
            query: self
                .query
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            status: self.status,
            limit: self.limit,
        }
    }
}
