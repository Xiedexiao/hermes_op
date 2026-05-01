//! Hermes parity 后端领域模型

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityCatalog {
    pub providers: Vec<ParityProviderCatalog>,
    pub active_provider: String,
    pub active_model: String,
    #[serde(default)]
    pub tool_visibility_options: Vec<String>,
    #[serde(default)]
    pub cron_status_options: Vec<String>,
    #[serde(default)]
    pub mcp_filter_modes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityProviderCatalog {
    pub id: String,
    pub display_name: String,
    pub supports_custom_endpoint: bool,
    #[serde(default)]
    pub models: Vec<ParityModelCatalogEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityModelCatalogEntry {
    pub id: String,
    pub display_name: String,
    pub recommended: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityProviderSelection {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityRuntimeAuthSource {
    pub kind: String,
    pub label: String,
    #[serde(default)]
    pub env_var: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityRuntimeReadiness {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_ref: Option<String>,
    pub api_key_ref_configured: bool,
    pub uses_custom_endpoint: bool,
    pub can_authenticate: bool,
    pub auth: ParityRuntimeAuthSource,
    #[serde(default)]
    pub sources: Vec<ParityRuntimeAuthSource>,
    pub status: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityProviderSelectionInput {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityToolMetadata {
    pub name: String,
    pub description: String,
    pub visible: bool,
    pub enabled: bool,
    pub availability: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityToolset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub enabled: bool,
    pub source: String,
    #[serde(default)]
    pub tools: Vec<ParityToolMetadata>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityToolsetInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    pub enabled: bool,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub tools: Vec<ParityToolMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityCronJob {
    pub id: String,
    pub name: String,
    pub schedule: String,
    pub prompt: String,
    #[serde(default)]
    pub deliver_to: Option<String>,
    pub enabled: bool,
    pub status: String,
    #[serde(default)]
    pub last_run_requested_at: Option<String>,
    #[serde(default)]
    pub last_run_status: Option<String>,
    pub run_count: u32,
    #[serde(default)]
    pub paused_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityCronJobInput {
    pub name: String,
    pub schedule: String,
    pub prompt: String,
    #[serde(default)]
    pub deliver_to: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityMcpServer {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub endpoint: String,
    pub enabled: bool,
    pub tool_filter_mode: String,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub blocked_tools: Vec<String>,
    pub resources_enabled: bool,
    pub prompts_enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityMcpServerInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub transport: String,
    pub endpoint: String,
    pub enabled: bool,
    pub tool_filter_mode: String,
    #[serde(default)]
    pub allowed_tools: Vec<String>,
    #[serde(default)]
    pub blocked_tools: Vec<String>,
    pub resources_enabled: bool,
    pub prompts_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityMcpRuntimeState {
    pub server_id: String,
    pub runtime_status: String,
    pub management_mode: String,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub last_started_at: Option<String>,
    #[serde(default)]
    pub last_stopped_at: Option<String>,
    #[serde(default)]
    pub last_reloaded_at: Option<String>,
    #[serde(default)]
    pub last_exit_code: Option<i32>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub status_message: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityMcpServerRuntimeStatus {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub endpoint: String,
    pub enabled: bool,
    pub runtime_status: String,
    pub management_mode: String,
    #[serde(default)]
    pub pid: Option<u32>,
    #[serde(default)]
    pub last_started_at: Option<String>,
    #[serde(default)]
    pub last_stopped_at: Option<String>,
    #[serde(default)]
    pub last_reloaded_at: Option<String>,
    #[serde(default)]
    pub last_exit_code: Option<i32>,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub status_message: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityMcpProbeResult {
    pub id: String,
    pub name: String,
    pub transport: String,
    pub endpoint: String,
    pub management_mode: String,
    pub tool_filter_mode: String,
    pub allowed_tool_count: usize,
    pub blocked_tool_count: usize,
    pub resources_enabled: bool,
    pub prompts_enabled: bool,
    pub handshake_status: String,
    pub handshake_reason: String,
    pub status: String,
    pub message: String,
    #[serde(default)]
    pub command_available: Option<bool>,
    #[serde(default)]
    pub url_valid: Option<bool>,
    #[serde(default)]
    pub parsed_command: Option<String>,
    #[serde(default)]
    pub parsed_args: Vec<String>,
    #[serde(default)]
    pub endpoint_scheme: Option<String>,
    #[serde(default)]
    pub endpoint_host: Option<String>,
    #[serde(default)]
    pub endpoint_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityQuickCommand {
    pub id: String,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub description: Option<String>,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityQuickCommandInput {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub description: Option<String>,
    pub enabled: bool,
}
