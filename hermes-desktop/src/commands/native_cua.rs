use crate::backend::{AppError, Database, provider_api_key_env, provider_requires_api_key};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tauri::State;
use uuid::Uuid;

const NATIVE_CUA_SESSIONS_KEY: &str = "native_cua.sessions";
const NATIVE_CUA_AUDIT_LOG_KEY: &str = "native_cua.audit_events";
const NATIVE_CUA_PLANS_KEY: &str = "native_cua.plans";
const NATIVE_CUA_HISTORY_KEY: &str = "native_cua.history";
const NATIVE_CUA_MEMORY_KEY: &str = "native_cua.memory_records";
const NATIVE_CUA_MODEL_TURNS_KEY: &str = "native_cua.model_turns";
const NATIVE_CUA_CONFIRM_PHRASE: &str = "RUN NATIVE CUA ACTION";
const NATIVE_CUA_MODEL_CONFIRM_PHRASE: &str = "INVOKE NATIVE CUA MODEL";
const NATIVE_CUA_RUNTIME_SETTINGS_KEY: &str = "runtime";
const NATIVE_CUA_AUDIT_LOG_LIMIT: usize = 500;
const NATIVE_CUA_AUDIT_EXPORT_DEFAULT_LIMIT: usize = 100;
const NATIVE_CUA_SESSION_LIMIT: usize = 100;
const NATIVE_CUA_PLAN_LIMIT: usize = 100;
const NATIVE_CUA_HISTORY_LIMIT: usize = 1_000;
const NATIVE_CUA_MEMORY_LIMIT: usize = 500;
const NATIVE_CUA_MAX_STEP_ACTIONS: usize = 8;
const NATIVE_CUA_TASK_MAX_CHARS: usize = 2_000;
const NATIVE_CUA_TEXT_MAX_CHARS: usize = 4_000;
const NATIVE_CUA_SUMMARY_MAX_CHARS: usize = 420;
const NATIVE_CUA_EXECUTION_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaProbeResponse {
    pub readiness: String,
    pub available: bool,
    pub platform: Option<String>,
    pub safety_mode: Option<String>,
    pub active_session_id: Option<String>,
    pub notes: Vec<String>,
    pub warnings: Vec<String>,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaStartSessionRequest {
    pub task: String,
    pub session_id: Option<String>,
    #[serde(default)]
    pub model_mode: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaPreviewModelRouteRequest {
    pub task: String,
    #[serde(default)]
    pub model_mode: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaModelRoutePreview {
    pub model_mode: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key_ref: Option<String>,
    pub model_difficulty: Option<String>,
    pub model_selection_reason: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaSession {
    pub session_id: String,
    pub status: String,
    pub task: Option<String>,
    pub resumed: bool,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub summary: Option<String>,
    pub model_mode: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key_ref: Option<String>,
    pub model_difficulty: Option<String>,
    pub model_selection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct StoredNativeCuaSession {
    session_id: String,
    status: String,
    task: String,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    model_mode: Option<String>,
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key_ref: Option<String>,
    #[serde(default)]
    model_difficulty: Option<String>,
    #[serde(default)]
    model_selection_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeCuaSessionModelConfig {
    mode: String,
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    api_key_ref: Option<String>,
    difficulty: Option<String>,
    selection_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaObserveRequest {
    pub session_id: Option<String>,
    pub dry_run: Option<bool>,
    pub capture_screenshot: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaObservation {
    pub session_id: String,
    pub dry_run: bool,
    pub capture_screenshot: bool,
    pub screenshot_captured: Option<bool>,
    pub screenshot_path: Option<String>,
    pub status: String,
    pub summary: Option<String>,
    pub observation: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaActionRequest {
    pub session_id: Option<String>,
    pub action_type: String,
    pub text: Option<String>,
    pub key: Option<String>,
    pub modifiers: Option<Vec<String>>,
    pub app: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub dx: Option<f64>,
    pub dy: Option<f64>,
    pub dry_run: Option<bool>,
    pub confirmation_phrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaActionResult {
    pub session_id: String,
    pub action_type: String,
    pub dry_run: bool,
    pub executed: bool,
    pub status: String,
    pub summary: Option<String>,
    pub audit_message: Option<String>,
    pub planned_command: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaAuditEvent {
    pub id: String,
    pub occurred_at: String,
    pub event_type: String,
    pub status: String,
    pub session_id: Option<String>,
    pub dry_run: Option<bool>,
    pub summary: String,
    pub planned_command: Vec<String>,
    pub screenshot_path: Option<String>,
    pub action_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaAuditListRequest {
    pub limit: Option<usize>,
    pub session_id: Option<String>,
    pub event_type: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaAuditExportRequest {
    pub limit: Option<usize>,
    pub session_id: Option<String>,
    pub event_type: Option<String>,
    pub status: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaAuditExportResponse {
    pub total: usize,
    pub exported_count: usize,
    pub format: String,
    pub payload: String,
    pub events: Vec<NativeCuaAuditEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaSkillMetadata {
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaPlanTaskRequest {
    pub session_id: Option<String>,
    pub task: Option<String>,
    pub skill_catalog: Option<Vec<NativeCuaSkillMetadata>>,
    pub max_steps: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaPlanStep {
    pub index: usize,
    pub goal: String,
    pub suggested_action: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaPlanResponse {
    pub session_id: String,
    pub task: String,
    pub created_at: String,
    pub updated_at: String,
    pub source: String,
    pub status: String,
    pub selected_skills: Vec<String>,
    pub iteration_info: Value,
    pub steps: Vec<NativeCuaPlanStep>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaRunStepRequest {
    pub session_id: Option<String>,
    pub dry_run: Option<bool>,
    pub capture_screenshot: Option<bool>,
    pub brain_state: Option<Value>,
    pub actions: Option<Vec<Value>>,
    pub max_actions: Option<usize>,
    pub confirmation_phrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaStepActionResult {
    pub action_name: String,
    pub raw_action: Value,
    pub status: String,
    pub summary: String,
    pub native_result: Option<NativeCuaActionResult>,
    pub memory_record: Option<NativeCuaMemoryRecord>,
    pub is_done: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaStepRecord {
    pub id: String,
    pub session_id: String,
    pub step_index: usize,
    pub occurred_at: String,
    pub status: String,
    pub brain_state: Value,
    pub observation: Option<NativeCuaObservation>,
    pub actions: Vec<NativeCuaStepActionResult>,
    pub final_result: Option<String>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaRunStepResponse {
    pub session_id: String,
    pub step: NativeCuaStepRecord,
    pub history_len: usize,
    pub done: bool,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaHistoryListRequest {
    pub session_id: Option<String>,
    pub limit: Option<usize>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaRecordInfoRequest {
    pub session_id: Option<String>,
    pub text: String,
    pub file_name: String,
    pub screenshot_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaMemoryRecord {
    pub id: String,
    pub session_id: String,
    pub file_name: String,
    pub text: String,
    pub path: String,
    pub screenshot_path: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaTrajectoryExportRequest {
    pub session_id: Option<String>,
    pub format: Option<String>,
    pub include_audit: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NativeCuaTrajectoryExportResponse {
    pub session_id: Option<String>,
    pub format: String,
    pub exported_count: usize,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaModelTurnRequest {
    pub session_id: Option<String>,
    pub role: String,
    pub include_screenshot_data_url: Option<bool>,
    pub max_history: Option<usize>,
    pub extra_context: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaPromptMessage {
    pub role: String,
    pub content: String,
    pub attachments: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaModelTurnResponse {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub messages: Vec<NativeCuaPromptMessage>,
    pub response_schema: Value,
    pub action_catalog: Vec<String>,
    pub created_at: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaApplyModelOutputRequest {
    pub session_id: Option<String>,
    pub role: String,
    pub output: Value,
    pub dry_run: Option<bool>,
    pub capture_screenshot: Option<bool>,
    pub confirmation_phrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaApplyModelOutputResponse {
    pub session_id: String,
    pub role: String,
    pub status: String,
    pub output: Value,
    pub step_result: Option<NativeCuaRunStepResponse>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaInvokeModelRequest {
    pub session_id: Option<String>,
    pub role: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub base_url: Option<String>,
    pub api_key_ref: Option<String>,
    pub dry_run: Option<bool>,
    pub apply_output: Option<bool>,
    pub capture_screenshot: Option<bool>,
    pub extra_context: Option<String>,
    pub model_confirmation_phrase: Option<String>,
    pub action_confirmation_phrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NativeCuaInvokeModelResponse {
    pub session_id: String,
    pub role: String,
    pub provider: String,
    pub model: String,
    pub dry_run: bool,
    pub requested: bool,
    pub status: String,
    pub prompt_turn: NativeCuaModelTurnResponse,
    pub http_request_preview: Value,
    pub raw_output: Option<Value>,
    pub parsed_output: Option<Value>,
    pub apply_result: Option<NativeCuaApplyModelOutputResponse>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct NativeCuaModelTurnRecord {
    id: String,
    session_id: String,
    role: String,
    created_at: String,
    prompt_summary: String,
    output: Option<Value>,
    status: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeCuaCommandPlan {
    command: Vec<String>,
    requires_command: bool,
    summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeCuaPlatform {
    Macos,
    Linux,
    Windows,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScreenSize {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeCuaExecutionOutcome {
    stdout: String,
    stderr: String,
    exit_code: Option<i32>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PersistedNativeCuaModelProfileSettings {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key_ref: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PersistedNativeCuaAutoModelSettings {
    #[serde(default)]
    easy: Option<PersistedNativeCuaModelProfileSettings>,
    #[serde(default)]
    standard: Option<PersistedNativeCuaModelProfileSettings>,
    #[serde(default)]
    hard: Option<PersistedNativeCuaModelProfileSettings>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct PersistedNativeCuaRuntimeSettings {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key_ref: Option<String>,
    #[serde(default)]
    native_cua_auto_models: Option<PersistedNativeCuaAutoModelSettings>,
}

#[derive(Debug, Clone, PartialEq)]
struct NativeCuaModelHttpRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Value,
}

#[derive(Debug, Clone)]
struct PreparedNativeCuaModelInvocation {
    prompt_turn: NativeCuaModelTurnResponse,
    provider: String,
    model: String,
    dry_run: bool,
    apply_output: bool,
    capture_screenshot: bool,
    action_confirmation_phrase: Option<String>,
    http_request: NativeCuaModelHttpRequest,
    http_request_preview: Value,
}

#[tauri::command]
pub fn native_cua_probe(db: State<'_, Database>) -> Result<NativeCuaProbeResponse, AppError> {
    native_cua_probe_for_db(db.inner())
}

#[tauri::command]
pub fn native_cua_start_session(
    db: State<'_, Database>,
    request: NativeCuaStartSessionRequest,
) -> Result<NativeCuaSession, AppError> {
    native_cua_start_session_for_db(db.inner(), request)
}

#[tauri::command]
pub fn native_cua_preview_model_route(
    db: State<'_, Database>,
    request: NativeCuaPreviewModelRouteRequest,
) -> Result<NativeCuaModelRoutePreview, AppError> {
    native_cua_preview_model_route_for_db(db.inner(), request)
}

#[tauri::command]
pub fn native_cua_observe(
    db: State<'_, Database>,
    request: NativeCuaObserveRequest,
) -> Result<NativeCuaObservation, AppError> {
    native_cua_observe_for_db_with_executor(db.inner(), request, execute_command)
}

#[tauri::command]
pub fn native_cua_execute_action(
    db: State<'_, Database>,
    request: NativeCuaActionRequest,
) -> Result<NativeCuaActionResult, AppError> {
    native_cua_execute_action_for_db_with_executor(db.inner(), request, execute_command)
}

#[tauri::command]
pub fn native_cua_list_audit_events(
    db: State<'_, Database>,
    request: NativeCuaAuditListRequest,
) -> Result<Vec<NativeCuaAuditEvent>, AppError> {
    native_cua_list_audit_for_db(db.inner(), request)
}

#[tauri::command]
pub fn native_cua_export_audit_events(
    db: State<'_, Database>,
    request: NativeCuaAuditExportRequest,
) -> Result<NativeCuaAuditExportResponse, AppError> {
    native_cua_export_audit_for_db(db.inner(), request)
}

#[tauri::command]
pub fn native_cua_plan_task(
    db: State<'_, Database>,
    request: NativeCuaPlanTaskRequest,
) -> Result<NativeCuaPlanResponse, AppError> {
    native_cua_plan_task_for_db(db.inner(), request)
}

#[tauri::command]
pub fn native_cua_run_step(
    db: State<'_, Database>,
    request: NativeCuaRunStepRequest,
) -> Result<NativeCuaRunStepResponse, AppError> {
    native_cua_run_step_for_db_with_executor(db.inner(), request, execute_command)
}

#[tauri::command]
pub fn native_cua_list_history(
    db: State<'_, Database>,
    request: NativeCuaHistoryListRequest,
) -> Result<Vec<NativeCuaStepRecord>, AppError> {
    native_cua_list_history_for_db(db.inner(), request)
}

#[tauri::command]
pub fn native_cua_record_info(
    db: State<'_, Database>,
    request: NativeCuaRecordInfoRequest,
) -> Result<NativeCuaMemoryRecord, AppError> {
    native_cua_record_info_for_db(db.inner(), request)
}

#[tauri::command]
pub fn native_cua_export_trajectory(
    db: State<'_, Database>,
    request: NativeCuaTrajectoryExportRequest,
) -> Result<NativeCuaTrajectoryExportResponse, AppError> {
    native_cua_export_trajectory_for_db(db.inner(), request)
}

#[tauri::command]
pub fn native_cua_prepare_model_turn(
    db: State<'_, Database>,
    request: NativeCuaModelTurnRequest,
) -> Result<NativeCuaModelTurnResponse, AppError> {
    native_cua_prepare_model_turn_for_db(db.inner(), request)
}

#[tauri::command]
pub fn native_cua_apply_model_output(
    db: State<'_, Database>,
    request: NativeCuaApplyModelOutputRequest,
) -> Result<NativeCuaApplyModelOutputResponse, AppError> {
    native_cua_apply_model_output_for_db_with_executor(db.inner(), request, execute_command)
}

#[tauri::command]
pub async fn native_cua_invoke_model(
    db: State<'_, Database>,
    request: NativeCuaInvokeModelRequest,
) -> Result<NativeCuaInvokeModelResponse, AppError> {
    native_cua_invoke_model_for_db(db.inner().clone(), request).await
}

fn native_cua_probe_for_db(db: &Database) -> Result<NativeCuaProbeResponse, AppError> {
    let platform = detect_platform();
    let mut notes = vec![
        "Hermes native CUA is the in-product safe execution substrate, not the external TuriX bridge.".to_string(),
        "Default mode is dry-run; live actions require an explicit confirmation phrase.".to_string(),
    ];
    let mut warnings = vec![
        "This probe is not OSWorld/SOTA evidence and does not certify VLM task performance."
            .to_string(),
    ];
    let mut capabilities = vec![
        "session persistence".to_string(),
        "readiness probe".to_string(),
        "dry-run observe/action planning".to_string(),
        "audit list/export".to_string(),
        "confirmation-gated live actions".to_string(),
        "local planner loop".to_string(),
        "TuriX-compatible action schema translation".to_string(),
        "step history and trajectory export".to_string(),
        "record_info memory files".to_string(),
    ];

    let available = match platform {
        NativeCuaPlatform::Macos => {
            let osascript = command_exists("osascript");
            let screencapture = command_exists("screencapture");
            if osascript {
                capabilities.push("macOS System Events action planning".to_string());
            } else {
                warnings.push("macOS osascript is not available in PATH.".to_string());
            }
            if screencapture {
                capabilities.push("macOS screenshot capture".to_string());
            }
            osascript
        }
        NativeCuaPlatform::Linux => {
            let xdotool = command_exists("xdotool");
            if xdotool {
                capabilities.push("Linux xdotool action execution".to_string());
            } else {
                warnings.push(
                    "Linux xdotool is not available; live actions will be rejected.".to_string(),
                );
            }
            if linux_screenshot_tool().is_some() {
                capabilities.push("Linux screenshot capture".to_string());
            } else {
                warnings.push("No supported Linux screenshot tool found (gnome-screenshot, scrot, import, spectacle).".to_string());
            }
            xdotool
        }
        NativeCuaPlatform::Windows => {
            let powershell = command_exists("powershell") || command_exists("powershell.exe");
            if powershell {
                capabilities.push("Windows PowerShell desktop action execution".to_string());
                capabilities.push("Windows screenshot capture".to_string());
            } else {
                warnings.push("Windows PowerShell is not available in PATH.".to_string());
            }
            powershell
        }
        NativeCuaPlatform::Unsupported => {
            warnings.push(format!(
                "Unsupported platform `{}` for live native CUA execution.",
                env::consts::OS
            ));
            false
        }
    };

    let active_session_id = load_sessions(db)?
        .into_iter()
        .find(|session| session.status == "active")
        .map(|session| session.session_id);
    if active_session_id.is_none() {
        notes.push("Start a native CUA session before observe/action calls.".to_string());
    }

    let readiness = if available {
        "ready_with_safety_gate".to_string()
    } else {
        "dry_run_only_or_missing_platform_tools".to_string()
    };

    Ok(NativeCuaProbeResponse {
        readiness,
        available,
        platform: Some(env::consts::OS.to_string()),
        safety_mode: Some("dry_run_default_confirmation_required".to_string()),
        active_session_id,
        notes,
        warnings,
        capabilities,
    })
}

fn native_cua_start_session_for_db(
    db: &Database,
    request: NativeCuaStartSessionRequest,
) -> Result<NativeCuaSession, AppError> {
    let task = validate_bounded_text("task", request.task, NATIVE_CUA_TASK_MAX_CHARS)?;
    let requested_session_id = normalize_optional_text(request.session_id);
    let model_config = resolve_session_model_config(
        db,
        &task,
        request.model_mode,
        request.provider,
        request.model,
        request.base_url,
        request.api_key_ref,
    )?;
    let now = Utc::now().to_rfc3339();
    let mut sessions = load_sessions(db)?;
    let mut resumed = false;
    let session_id = requested_session_id.unwrap_or_else(|| Uuid::new_v4().to_string());

    if let Some(existing) = sessions
        .iter_mut()
        .find(|session| session.session_id == session_id)
    {
        existing.status = "active".to_string();
        existing.task = task.clone();
        existing.model_mode = Some(model_config.mode.clone());
        existing.provider = model_config.provider.clone();
        existing.model = model_config.model.clone();
        existing.base_url = model_config.base_url.clone();
        existing.api_key_ref = model_config.api_key_ref.clone();
        existing.model_difficulty = model_config.difficulty.clone();
        existing.model_selection_reason = model_config.selection_reason.clone();
        existing.updated_at = now.clone();
        resumed = true;
    } else {
        sessions.insert(
            0,
            StoredNativeCuaSession {
                session_id: session_id.clone(),
                status: "active".to_string(),
                task: task.clone(),
                created_at: now.clone(),
                updated_at: now.clone(),
                model_mode: Some(model_config.mode.clone()),
                provider: model_config.provider.clone(),
                model: model_config.model.clone(),
                base_url: model_config.base_url.clone(),
                api_key_ref: model_config.api_key_ref.clone(),
                model_difficulty: model_config.difficulty.clone(),
                model_selection_reason: model_config.selection_reason.clone(),
            },
        );
    }

    for session in &mut sessions {
        if session.session_id != session_id && session.status == "active" {
            session.status = "paused".to_string();
            session.updated_at = now.clone();
        }
    }
    sessions.truncate(NATIVE_CUA_SESSION_LIMIT);
    save_sessions(db, &sessions)?;

    let response = session_response(
        sessions
            .iter()
            .find(|session| session.session_id == session_id)
            .expect("session should be present after insert or resume"),
        resumed,
        Some(if resumed {
            format!(
                "Resumed Hermes native CUA session `{}` with a refreshed task using `{}` model mode.",
                session_id, model_config.mode
            )
        } else {
            format!(
                "Started Hermes native CUA session `{}` in dry-run-first safety mode using `{}` model mode.",
                session_id, model_config.mode
            )
        }),
    );

    record_audit_event(
        db,
        NativeCuaAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: now,
            event_type: "session".to_string(),
            status: if resumed { "resumed" } else { "started" }.to_string(),
            session_id: Some(session_id),
            dry_run: None,
            summary: response.summary.clone().unwrap_or_default(),
            planned_command: Vec::new(),
            screenshot_path: None,
            action_type: None,
        },
    )?;

    Ok(response)
}

fn native_cua_preview_model_route_for_db(
    db: &Database,
    request: NativeCuaPreviewModelRouteRequest,
) -> Result<NativeCuaModelRoutePreview, AppError> {
    let task = validate_bounded_text("task", request.task, NATIVE_CUA_TASK_MAX_CHARS)?;
    let model_config = resolve_session_model_config(
        db,
        &task,
        request.model_mode,
        request.provider,
        request.model,
        request.base_url,
        request.api_key_ref,
    )?;

    let summary = match model_config.mode.as_str() {
        "auto" => model_config
            .selection_reason
            .clone()
            .unwrap_or_else(|| "Auto selected a Native CUA model route.".to_string()),
        "custom" => format!(
            "Custom selected `{}` / `{}` for this Native CUA task.",
            model_config.provider.as_deref().unwrap_or("provider"),
            model_config.model.as_deref().unwrap_or("model")
        ),
        _ => model_config
            .selection_reason
            .clone()
            .unwrap_or_else(|| "Resolved a Native CUA model route.".to_string()),
    };

    Ok(NativeCuaModelRoutePreview {
        model_mode: model_config.mode,
        provider: model_config.provider,
        model: model_config.model,
        base_url: model_config.base_url,
        api_key_ref: model_config.api_key_ref,
        model_difficulty: model_config.difficulty,
        model_selection_reason: model_config.selection_reason,
        summary,
    })
}

fn native_cua_observe_for_db_with_executor<F>(
    db: &Database,
    request: NativeCuaObserveRequest,
    executor: F,
) -> Result<NativeCuaObservation, AppError>
where
    F: Fn(&[String]) -> Result<NativeCuaExecutionOutcome, AppError>,
{
    let session = resolve_session(db, request.session_id)?;
    let dry_run = request.dry_run.unwrap_or(true);
    let capture_screenshot = request.capture_screenshot.unwrap_or(false);
    touch_session(db, &session.session_id)?;

    let screenshot_path = if capture_screenshot {
        Some(next_screenshot_path(&session.session_id)?)
    } else {
        None
    };
    let command_plan = if let Some(path) = screenshot_path.as_ref() {
        plan_screenshot_command(path)?
    } else {
        NativeCuaCommandPlan {
            command: Vec::new(),
            requires_command: false,
            summary: "Native CUA observe requested metadata only; no screenshot command is needed."
                .to_string(),
        }
    };

    let mut screenshot_captured = false;
    let mut status = if dry_run { "dry_run" } else { "observed" }.to_string();
    let mut execution = json!({
        "executed": false,
        "stdout": null,
        "stderr": null,
        "exit_code": null,
    });

    if !dry_run && command_plan.requires_command {
        let outcome = executor(&command_plan.command)?;
        if outcome.exit_code == Some(0) {
            screenshot_captured = true;
        } else {
            status = "failed".to_string();
        }
        execution = json!({
            "executed": true,
            "stdout": truncate_summary(outcome.stdout),
            "stderr": truncate_summary(outcome.stderr),
            "exit_code": outcome.exit_code,
        });
    }

    let summary = if dry_run {
        if command_plan.command.is_empty() {
            "Native CUA observe dry-run recorded session metadata without executing OS commands."
                .to_string()
        } else {
            format!(
                "Native CUA observe dry-run planned screenshot command `{}` without executing it.",
                command_plan.command.join(" ")
            )
        }
    } else if screenshot_captured {
        format!(
            "Native CUA observe captured screenshot for session `{}`.",
            session.session_id
        )
    } else if command_plan.requires_command {
        format!(
            "Native CUA observe attempted screenshot command `{}` but it did not complete successfully.",
            command_plan.command.join(" ")
        )
    } else {
        format!(
            "Native CUA observe refreshed metadata for session `{}` without screenshot capture.",
            session.session_id
        )
    };

    let observation = json!({
        "session": {
            "session_id": session.session_id,
            "status": session.status,
            "task": session.task,
            "created_at": session.created_at,
            "updated_at": session.updated_at,
        },
        "platform": env::consts::OS,
        "planned_command": command_plan.command,
        "execution": execution,
        "safety": {
            "dry_run_default": true,
            "live_action_confirmation_phrase": NATIVE_CUA_CONFIRM_PHRASE,
            "not_osworld_or_sota_evidence": true,
        }
    });

    let event = NativeCuaAuditEvent {
        id: Uuid::new_v4().to_string(),
        occurred_at: Utc::now().to_rfc3339(),
        event_type: "observe".to_string(),
        status: status.clone(),
        session_id: Some(session.session_id.clone()),
        dry_run: Some(dry_run),
        summary: summary.clone(),
        planned_command: command_plan.command.clone(),
        screenshot_path: screenshot_path
            .as_ref()
            .map(|path| path.display().to_string()),
        action_type: None,
    };
    record_audit_event(db, event)?;

    Ok(NativeCuaObservation {
        session_id: session.session_id,
        dry_run,
        capture_screenshot,
        screenshot_captured: Some(screenshot_captured),
        screenshot_path: screenshot_path.map(|path| path.display().to_string()),
        status,
        summary: Some(summary),
        observation,
    })
}

fn native_cua_execute_action_for_db_with_executor<F>(
    db: &Database,
    request: NativeCuaActionRequest,
    executor: F,
) -> Result<NativeCuaActionResult, AppError>
where
    F: Fn(&[String]) -> Result<NativeCuaExecutionOutcome, AppError>,
{
    let session = resolve_session(db, request.session_id.clone())?;
    let dry_run = request.dry_run.unwrap_or(true);
    if !dry_run
        && request.confirmation_phrase.as_deref().map(str::trim) != Some(NATIVE_CUA_CONFIRM_PHRASE)
    {
        return Err(AppError::validation(format!(
            "non-dry-run native CUA actions require exact confirmation phrase `{}`",
            NATIVE_CUA_CONFIRM_PHRASE
        )));
    }

    let normalized_request = normalize_action_request(request)?;
    let command_plan = plan_action_command(&normalized_request, detect_platform(), None)?;
    touch_session(db, &session.session_id)?;

    let mut executed = false;
    let mut status = "dry_run".to_string();
    let mut command_failure = None;

    if !dry_run {
        if command_plan.requires_command {
            ensure_command_available(&command_plan.command)?;
            let outcome = executor(&command_plan.command)?;
            executed = true;
            if outcome.exit_code == Some(0) {
                status = "executed".to_string();
            } else {
                status = "failed".to_string();
                command_failure = Some(format!(
                    "exit={:?} stderr={}",
                    outcome.exit_code,
                    truncate_summary(outcome.stderr)
                ));
            }
        } else {
            thread::sleep(Duration::from_millis(250));
            executed = true;
            status = "executed".to_string();
        }
    }

    let action_type = normalized_request.action_type.clone();
    let summary = if dry_run {
        if command_plan.command.is_empty() {
            format!(
                "Native CUA dry-run planned `{}` without an OS command.",
                action_type
            )
        } else {
            format!(
                "Native CUA dry-run planned `{}` command `{}` without executing it.",
                action_type,
                command_plan.command.join(" ")
            )
        }
    } else if let Some(failure) = command_failure {
        format!(
            "Native CUA live `{}` attempted command `{}` but failed: {}.",
            action_type,
            command_plan.command.join(" "),
            failure
        )
    } else if command_plan.command.is_empty() {
        format!(
            "Native CUA live `{}` completed without an OS command.",
            action_type
        )
    } else {
        format!(
            "Native CUA live `{}` executed command `{}`.",
            action_type,
            command_plan.command.join(" ")
        )
    };

    let audit_message = format!(
        "{} for session `{}`; dry_run={}; executed={}",
        summary, session.session_id, dry_run, executed
    );
    record_audit_event(
        db,
        NativeCuaAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: Utc::now().to_rfc3339(),
            event_type: "action".to_string(),
            status: status.clone(),
            session_id: Some(session.session_id.clone()),
            dry_run: Some(dry_run),
            summary: summary.clone(),
            planned_command: command_plan.command.clone(),
            screenshot_path: None,
            action_type: Some(action_type.clone()),
        },
    )?;

    Ok(NativeCuaActionResult {
        session_id: session.session_id,
        action_type,
        dry_run,
        executed,
        status,
        summary: Some(summary),
        audit_message: Some(audit_message),
        planned_command: command_plan.command,
    })
}

fn native_cua_list_audit_for_db(
    db: &Database,
    request: NativeCuaAuditListRequest,
) -> Result<Vec<NativeCuaAuditEvent>, AppError> {
    let session_filter = normalize_filter(request.session_id);
    let event_type_filter = normalize_filter(request.event_type);
    let status_filter = normalize_filter(request.status);
    let limit = normalize_list_limit(request.limit);
    let mut events = load_audit_events(db)?;
    events.retain(|event| {
        optional_filter_matches(event.session_id.as_deref(), session_filter.as_deref())
            && filter_matches(&event.event_type, event_type_filter.as_deref())
            && filter_matches(&event.status, status_filter.as_deref())
    });
    if let Some(limit) = limit {
        events.truncate(limit);
    }
    Ok(events)
}

fn native_cua_export_audit_for_db(
    db: &Database,
    request: NativeCuaAuditExportRequest,
) -> Result<NativeCuaAuditExportResponse, AppError> {
    let format = normalize_export_format(request.format)?;
    let events = native_cua_list_audit_for_db(
        db,
        NativeCuaAuditListRequest {
            limit: Some(normalize_export_limit(request.limit)),
            session_id: request.session_id,
            event_type: request.event_type,
            status: request.status,
        },
    )?;
    let total = load_audit_events(db)?.len();
    let payload = serialize_audit_events(&events, &format)?;
    Ok(NativeCuaAuditExportResponse {
        total,
        exported_count: events.len(),
        format,
        payload,
        events,
    })
}

fn native_cua_prepare_model_turn_for_db(
    db: &Database,
    request: NativeCuaModelTurnRequest,
) -> Result<NativeCuaModelTurnResponse, AppError> {
    let session = resolve_session(db, request.session_id)?;
    let role = normalize_model_role(&request.role)?;
    let max_history = request.max_history.unwrap_or(6).clamp(1, 20);
    let history = native_cua_list_history_for_db(
        db,
        NativeCuaHistoryListRequest {
            session_id: Some(session.session_id.clone()),
            limit: Some(max_history),
            status: None,
        },
    )?;
    let plans = load_plans(db)?
        .into_iter()
        .filter(|plan| plan.session_id == session.session_id)
        .collect::<Vec<_>>();
    let memories = load_memory_records(db)?
        .into_iter()
        .filter(|memory| memory.session_id == session.session_id)
        .take(8)
        .collect::<Vec<_>>();
    let extra_context = normalize_optional_text(request.extra_context);
    let action_catalog = turix_action_catalog();
    let response_schema = response_schema_for_model_role(&role);
    let attachments = if request.include_screenshot_data_url.unwrap_or(false) {
        latest_screenshot_attachment(&history)
            .map(|attachment| vec![attachment])
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let system = NativeCuaPromptMessage {
        role: "system".to_string(),
        content: model_system_prompt(&role),
        attachments: Vec::new(),
    };
    let user = NativeCuaPromptMessage {
        role: "user".to_string(),
        content: build_model_user_prompt(
            &role,
            &session,
            &plans,
            &history,
            &memories,
            extra_context.as_deref(),
        )?,
        attachments,
    };
    let now = Utc::now().to_rfc3339();
    let response = NativeCuaModelTurnResponse {
        id: Uuid::new_v4().to_string(),
        session_id: session.session_id.clone(),
        role: role.clone(),
        provider: None,
        model: None,
        messages: vec![system, user],
        response_schema,
        action_catalog,
        created_at: now.clone(),
        summary: format!(
            "Prepared Hermes native CUA {} model turn for session `{}`.",
            role, session.session_id
        ),
    };
    record_model_turn(
        db,
        NativeCuaModelTurnRecord {
            id: response.id.clone(),
            session_id: response.session_id.clone(),
            role: role.clone(),
            created_at: now.clone(),
            prompt_summary: response.summary.clone(),
            output: None,
            status: "prepared".to_string(),
        },
    )?;
    record_audit_event(
        db,
        NativeCuaAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: now,
            event_type: "model_turn".to_string(),
            status: "prepared".to_string(),
            session_id: Some(session.session_id),
            dry_run: None,
            summary: response.summary.clone(),
            planned_command: Vec::new(),
            screenshot_path: None,
            action_type: Some(role),
        },
    )?;
    Ok(response)
}

fn native_cua_apply_model_output_for_db_with_executor<F>(
    db: &Database,
    request: NativeCuaApplyModelOutputRequest,
    executor: F,
) -> Result<NativeCuaApplyModelOutputResponse, AppError>
where
    F: Copy + Fn(&[String]) -> Result<NativeCuaExecutionOutcome, AppError>,
{
    let session = resolve_session(db, request.session_id.clone())?;
    let role = normalize_model_role(&request.role)?;
    let output = request.output;
    let step_result = if role == "actor" {
        let actions = output
            .get("action")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                AppError::validation("actor model output must contain an `action` array")
            })?
            .clone();
        Some(native_cua_run_step_for_db_with_executor(
            db,
            NativeCuaRunStepRequest {
                session_id: Some(session.session_id.clone()),
                dry_run: request.dry_run,
                capture_screenshot: request.capture_screenshot,
                brain_state: None,
                actions: Some(actions),
                max_actions: Some(NATIVE_CUA_MAX_STEP_ACTIONS),
                confirmation_phrase: request.confirmation_phrase,
            },
            executor,
        )?)
    } else {
        None
    };
    let now = Utc::now().to_rfc3339();
    let status = if role == "actor" { "applied" } else { "stored" }.to_string();
    let summary = if role == "actor" {
        format!(
            "Applied actor model output to Hermes native CUA session `{}`.",
            session.session_id
        )
    } else {
        format!(
            "Stored {} model output for Hermes native CUA session `{}`.",
            role, session.session_id
        )
    };
    record_model_turn(
        db,
        NativeCuaModelTurnRecord {
            id: Uuid::new_v4().to_string(),
            session_id: session.session_id.clone(),
            role: role.clone(),
            created_at: now.clone(),
            prompt_summary: summary.clone(),
            output: Some(output.clone()),
            status: status.clone(),
        },
    )?;
    record_audit_event(
        db,
        NativeCuaAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: now,
            event_type: "model_output".to_string(),
            status: status.clone(),
            session_id: Some(session.session_id.clone()),
            dry_run: request.dry_run,
            summary: summary.clone(),
            planned_command: Vec::new(),
            screenshot_path: step_result
                .as_ref()
                .and_then(|step| step.step.observation.as_ref())
                .and_then(|observation| observation.screenshot_path.clone()),
            action_type: Some(role.clone()),
        },
    )?;
    Ok(NativeCuaApplyModelOutputResponse {
        session_id: session.session_id,
        role,
        status,
        output,
        step_result,
        summary,
    })
}

async fn native_cua_invoke_model_for_db(
    db: Database,
    request: NativeCuaInvokeModelRequest,
) -> Result<NativeCuaInvokeModelResponse, AppError> {
    let prepared = prepare_native_cua_model_invocation(&db, request)?;
    if prepared.dry_run {
        return Ok(build_native_cua_invoke_model_response(
            prepared, false, None, None, None,
        ));
    }

    let raw_output = send_native_cua_model_http_request(&prepared.http_request).await?;
    finalize_native_cua_model_invocation_for_db_with_executor(
        &db,
        prepared,
        raw_output,
        execute_command,
    )
}

#[cfg(test)]
fn native_cua_invoke_model_for_db_with_sender_and_executor<S, F>(
    db: &Database,
    request: NativeCuaInvokeModelRequest,
    mut sender: S,
    executor: F,
) -> Result<NativeCuaInvokeModelResponse, AppError>
where
    S: FnMut(&NativeCuaModelHttpRequest) -> Result<Value, AppError>,
    F: Copy + Fn(&[String]) -> Result<NativeCuaExecutionOutcome, AppError>,
{
    let prepared = prepare_native_cua_model_invocation(db, request)?;
    if prepared.dry_run {
        return Ok(build_native_cua_invoke_model_response(
            prepared, false, None, None, None,
        ));
    }

    let raw_output = sender(&prepared.http_request)?;
    finalize_native_cua_model_invocation_for_db_with_executor(db, prepared, raw_output, executor)
}

fn prepare_native_cua_model_invocation(
    db: &Database,
    request: NativeCuaInvokeModelRequest,
) -> Result<PreparedNativeCuaModelInvocation, AppError> {
    let role = normalize_model_role(&request.role)?;
    let dry_run = request.dry_run.unwrap_or(true);
    let apply_output = request.apply_output.unwrap_or(false);
    let capture_screenshot = request.capture_screenshot.unwrap_or(false);
    let resolved = resolve_native_cua_model_runtime_config(db, &request)?;
    let mut prompt_turn = native_cua_prepare_model_turn_for_db(
        db,
        NativeCuaModelTurnRequest {
            session_id: request.session_id,
            role: role.clone(),
            include_screenshot_data_url: Some(capture_screenshot),
            max_history: None,
            extra_context: request.extra_context,
        },
    )?;
    prompt_turn.provider = Some(resolved.provider.clone());
    prompt_turn.model = Some(resolved.model.clone());

    let http_request = build_native_cua_model_http_request(
        &resolved.provider,
        &resolved.model,
        &resolved.base_url,
        resolved.api_key.as_deref(),
        &prompt_turn,
    )?;
    let http_request_preview = native_cua_model_http_request_preview(&http_request);

    if !dry_run {
        let phrase = normalize_optional_text(request.model_confirmation_phrase).unwrap_or_default();
        if phrase != NATIVE_CUA_MODEL_CONFIRM_PHRASE {
            return Err(AppError::validation(format!(
                "live native CUA model invocation requires exact confirmation phrase `{}`",
                NATIVE_CUA_MODEL_CONFIRM_PHRASE
            )));
        }
        if provider_requires_api_key(&resolved.provider) && resolved.api_key.is_none() {
            let env_hint = provider_api_key_env(&resolved.provider)
                .map(|name| format!(" or provider env `{name}`"))
                .unwrap_or_default();
            let ref_hint = resolved
                .api_key_ref
                .as_deref()
                .map(|name| format!("runtime api_key_ref env `{name}`"))
                .unwrap_or_else(|| "runtime api_key_ref env".to_string());
            return Err(AppError::validation(format!(
                "native CUA provider `{}` requires an API key via {}{}",
                resolved.provider, ref_hint, env_hint
            )));
        }
    }

    Ok(PreparedNativeCuaModelInvocation {
        prompt_turn,
        provider: resolved.provider,
        model: resolved.model,
        dry_run,
        apply_output,
        capture_screenshot,
        action_confirmation_phrase: normalize_optional_text(request.action_confirmation_phrase),
        http_request,
        http_request_preview,
    })
}

fn finalize_native_cua_model_invocation_for_db_with_executor<F>(
    db: &Database,
    prepared: PreparedNativeCuaModelInvocation,
    raw_output: Value,
    executor: F,
) -> Result<NativeCuaInvokeModelResponse, AppError>
where
    F: Copy + Fn(&[String]) -> Result<NativeCuaExecutionOutcome, AppError>,
{
    let parsed_output = parse_native_cua_model_output(&prepared.provider, &raw_output)?;
    let apply_result = if prepared.apply_output {
        Some(native_cua_apply_model_output_for_db_with_executor(
            db,
            NativeCuaApplyModelOutputRequest {
                session_id: Some(prepared.prompt_turn.session_id.clone()),
                role: prepared.prompt_turn.role.clone(),
                output: parsed_output.clone(),
                dry_run: Some(prepared.dry_run),
                capture_screenshot: Some(prepared.capture_screenshot),
                confirmation_phrase: prepared.action_confirmation_phrase.clone(),
            },
            executor,
        )?)
    } else {
        None
    };

    Ok(build_native_cua_invoke_model_response(
        prepared,
        true,
        Some(raw_output),
        Some(parsed_output),
        apply_result,
    ))
}

fn build_native_cua_invoke_model_response(
    prepared: PreparedNativeCuaModelInvocation,
    requested: bool,
    raw_output: Option<Value>,
    parsed_output: Option<Value>,
    apply_result: Option<NativeCuaApplyModelOutputResponse>,
) -> NativeCuaInvokeModelResponse {
    let status = if prepared.dry_run {
        "dry_run".to_string()
    } else if let Some(result) = apply_result.as_ref() {
        result.status.clone()
    } else {
        "completed".to_string()
    };
    let summary = if prepared.dry_run {
        format!(
            "Prepared dry-run Hermes native CUA {} model request for provider `{}` and model `{}`.",
            prepared.prompt_turn.role, prepared.provider, prepared.model
        )
    } else if apply_result.is_some() {
        format!(
            "Invoked Hermes native CUA {} model via `{}` / `{}` and applied the parsed output.",
            prepared.prompt_turn.role, prepared.provider, prepared.model
        )
    } else {
        format!(
            "Invoked Hermes native CUA {} model via `{}` / `{}`.",
            prepared.prompt_turn.role, prepared.provider, prepared.model
        )
    };

    NativeCuaInvokeModelResponse {
        session_id: prepared.prompt_turn.session_id.clone(),
        role: prepared.prompt_turn.role.clone(),
        provider: prepared.provider,
        model: prepared.model,
        dry_run: prepared.dry_run,
        requested,
        status,
        prompt_turn: prepared.prompt_turn,
        http_request_preview: prepared.http_request_preview,
        raw_output,
        parsed_output,
        apply_result,
        summary,
    }
}

#[derive(Debug, Clone)]
struct ResolvedNativeCuaModelConfig {
    provider: String,
    model: String,
    base_url: String,
    api_key_ref: Option<String>,
    api_key: Option<String>,
}

fn stored_session_model_config(
    session: &StoredNativeCuaSession,
) -> Option<NativeCuaSessionModelConfig> {
    let mode = session.model_mode.as_deref().unwrap_or("auto");
    if !matches!(mode, "auto" | "custom") || session.provider.is_none() || session.model.is_none() {
        return None;
    }

    Some(NativeCuaSessionModelConfig {
        mode: mode.to_string(),
        provider: session.provider.clone(),
        model: session.model.clone(),
        base_url: session.base_url.clone(),
        api_key_ref: session.api_key_ref.clone(),
        difficulty: session.model_difficulty.clone(),
        selection_reason: session.model_selection_reason.clone(),
    })
}

fn resolve_native_cua_model_runtime_config(
    db: &Database,
    request: &NativeCuaInvokeModelRequest,
) -> Result<ResolvedNativeCuaModelConfig, AppError> {
    let runtime = load_native_cua_runtime_settings(db)?;
    let session_model_config = resolve_session(db, request.session_id.clone())
        .ok()
        .and_then(|session| stored_session_model_config(&session));
    let request_provider = normalize_optional_text(request.provider.clone());
    let session_provider = session_model_config
        .as_ref()
        .and_then(|config| normalize_optional_text(config.provider.clone()));
    let runtime_provider = normalize_optional_text(runtime.provider.clone());
    let provider = normalize_native_cua_provider(
        request_provider
            .clone()
            .or(session_provider.clone())
            .or(runtime_provider.clone())
            .unwrap_or_else(|| "openai".to_string()),
    )?;

    let use_session_model = request.model.is_none()
        && session_provider
            .as_deref()
            .is_some_and(|session_provider| session_provider == provider);
    let use_runtime_model = request.model.is_none()
        && !use_session_model
        && runtime_provider
            .as_deref()
            .is_some_and(|runtime_provider| runtime_provider == provider);
    let use_session_base_url = request.base_url.is_none()
        && session_provider
            .as_deref()
            .is_some_and(|session_provider| session_provider == provider);
    let use_runtime_base_url = request.base_url.is_none()
        && !use_session_base_url
        && runtime_provider
            .as_deref()
            .is_some_and(|runtime_provider| runtime_provider == provider);
    let model = normalize_optional_text(request.model.clone())
        .or_else(|| {
            if use_session_model {
                session_model_config
                    .as_ref()
                    .and_then(|config| normalize_optional_text(config.model.clone()))
            } else {
                None
            }
        })
        .or_else(|| {
            if use_runtime_model {
                normalize_optional_text(runtime.model.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| default_native_cua_model_for_provider(&provider).to_string());
    let base_url = normalize_optional_text(request.base_url.clone())
        .or_else(|| {
            if use_session_base_url {
                session_model_config
                    .as_ref()
                    .and_then(|config| normalize_optional_text(config.base_url.clone()))
            } else {
                None
            }
        })
        .or_else(|| {
            if use_runtime_base_url {
                normalize_optional_text(runtime.base_url.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| default_native_cua_base_url(&provider).to_string());
    let api_key_ref = normalize_optional_text(request.api_key_ref.clone())
        .or_else(|| {
            session_model_config
                .as_ref()
                .and_then(|config| normalize_optional_text(config.api_key_ref.clone()))
        })
        .or_else(|| normalize_optional_text(runtime.api_key_ref.clone()));
    let api_key = resolve_native_cua_api_key(&provider, api_key_ref.as_deref());

    Ok(ResolvedNativeCuaModelConfig {
        provider,
        model,
        base_url,
        api_key_ref,
        api_key,
    })
}

fn load_native_cua_runtime_settings(
    db: &Database,
) -> Result<PersistedNativeCuaRuntimeSettings, AppError> {
    match db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&NATIVE_CUA_RUNTIME_SETTINGS_KEY],
        |row| row.get::<_, String>(0),
    ) {
        Ok(json) => serde_json::from_str(&json).map_err(AppError::from_json_error),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(PersistedNativeCuaRuntimeSettings {
            provider: Some("openai".to_string()),
            model: Some("gpt-4o".to_string()),
            base_url: None,
            api_key_ref: None,
            native_cua_auto_models: None,
        }),
        Err(error) => Err(AppError::storage(format!(
            "Failed to load runtime settings for native CUA model invocation: {}",
            error
        ))),
    }
}

fn normalize_native_cua_provider(provider: String) -> Result<String, AppError> {
    let normalized = provider.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "openai" | "openrouter" | "deepseek" | "anthropic" | "ollama" => Ok(normalized),
        _ => Err(AppError::validation(format!(
            "unsupported native CUA provider `{}`",
            provider
        ))),
    }
}

fn default_native_cua_model_for_provider(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "claude-sonnet-4",
        "deepseek" => "deepseek-chat",
        "ollama" => "qwen2.5-coder",
        "openrouter" => "anthropic/claude-sonnet-4",
        _ => "gpt-4o",
    }
}

fn default_native_cua_auto_model_for_provider(provider: &str, difficulty: &str) -> &'static str {
    match (provider, difficulty) {
        ("anthropic", "hard") => "claude-opus-4",
        ("anthropic", _) => "claude-sonnet-4",
        ("deepseek", "hard") => "deepseek-reasoner",
        ("deepseek", _) => "deepseek-chat",
        ("ollama", "easy") => "llama3.1",
        ("ollama", _) => "qwen2.5-coder",
        ("openrouter", "easy") => "openai/gpt-4o-mini",
        ("openrouter", "hard") => "anthropic/claude-opus-4",
        ("openrouter", _) => "anthropic/claude-sonnet-4",
        (_, "easy") => "gpt-4o-mini",
        (_, "hard") => "gpt-4.1",
        _ => "gpt-4o",
    }
}

fn default_native_cua_base_url(provider: &str) -> &'static str {
    match provider {
        "anthropic" => "https://api.anthropic.com",
        "deepseek" => "https://api.deepseek.com",
        "ollama" => "http://localhost:11434",
        "openrouter" => "https://openrouter.ai/api",
        _ => "https://api.openai.com",
    }
}

fn resolve_native_cua_api_key(provider: &str, api_key_ref: Option<&str>) -> Option<String> {
    if let Some(reference) = api_key_ref
        && let Ok(value) = env::var(reference)
    {
        let trimmed = value.trim().to_string();
        if !trimmed.is_empty() {
            return Some(trimmed);
        }
    }

    provider_api_key_env(provider).and_then(|name| {
        env::var(name).ok().and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

fn build_native_cua_model_http_request(
    provider: &str,
    model: &str,
    base_url: &str,
    api_key: Option<&str>,
    prompt_turn: &NativeCuaModelTurnResponse,
) -> Result<NativeCuaModelHttpRequest, AppError> {
    let (url, body) = match provider {
        "anthropic" => (
            build_native_cua_endpoint(base_url, "/v1/messages"),
            json!({
                "model": model,
                "system": prompt_turn
                    .messages
                    .iter()
                    .filter(|message| message.role == "system")
                    .map(render_native_cua_prompt_message)
                    .collect::<Vec<_>>()
                    .join("\n\n"),
                "messages": prompt_turn
                    .messages
                    .iter()
                    .filter(|message| message.role != "system")
                    .map(|message| json!({
                        "role": message.role,
                        "content": render_native_cua_prompt_message(message),
                    }))
                    .collect::<Vec<_>>(),
                "max_tokens": 1024,
            }),
        ),
        "ollama" => (
            build_native_cua_endpoint(base_url, "/api/chat"),
            json!({
                "model": model,
                "messages": prompt_turn
                    .messages
                    .iter()
                    .map(|message| json!({
                        "role": message.role,
                        "content": render_native_cua_prompt_message(message),
                    }))
                    .collect::<Vec<_>>(),
                "stream": false,
                "format": prompt_turn.response_schema.clone(),
            }),
        ),
        _ => (
            build_native_cua_endpoint(base_url, "/v1/chat/completions"),
            json!({
                "model": model,
                "messages": prompt_turn
                    .messages
                    .iter()
                    .map(|message| json!({
                        "role": message.role,
                        "content": render_native_cua_prompt_message(message),
                    }))
                    .collect::<Vec<_>>(),
                "response_format": {
                    "type": "json_schema",
                    "json_schema": {
                        "name": format!("hermes_native_cua_{}", prompt_turn.role),
                        "schema": prompt_turn.response_schema.clone(),
                    }
                }
            }),
        ),
    };

    let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
    if let Some(api_key) = api_key
        && provider_requires_api_key(provider)
    {
        let header_value = if provider == "anthropic" {
            api_key.to_string()
        } else {
            format!("Bearer {}", api_key)
        };
        let header_name = if provider == "anthropic" {
            "x-api-key".to_string()
        } else {
            "authorization".to_string()
        };
        headers.push((header_name, header_value));
    }
    if provider == "anthropic" {
        headers.push(("anthropic-version".to_string(), "2023-06-01".to_string()));
    }

    Ok(NativeCuaModelHttpRequest {
        method: "POST".to_string(),
        url,
        headers,
        body,
    })
}

fn render_native_cua_prompt_message(message: &NativeCuaPromptMessage) -> String {
    if message.attachments.is_empty() {
        return message.content.clone();
    }

    format!(
        "{}\n\nAttachments:\n{}",
        message.content,
        serde_json::to_string_pretty(&message.attachments).unwrap_or_else(|_| "[]".to_string())
    )
}

fn build_native_cua_endpoint(base_url: &str, path: &str) -> String {
    let base = base_url.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    if let Some((first_segment, remaining_path)) = path.split_once('/')
        && base.ends_with(&format!("/{}", first_segment))
    {
        return format!("{}/{}", base, remaining_path);
    }
    format!("{}/{}", base, path)
}

fn native_cua_model_http_request_preview(request: &NativeCuaModelHttpRequest) -> Value {
    let headers = request
        .headers
        .iter()
        .map(|(name, value)| {
            let preview_value = if name.eq_ignore_ascii_case("authorization")
                || name.eq_ignore_ascii_case("x-api-key")
            {
                "<redacted>".to_string()
            } else {
                value.clone()
            };
            (name.clone(), Value::String(preview_value))
        })
        .collect::<serde_json::Map<String, Value>>();
    json!({
        "method": request.method,
        "url": request.url,
        "headers": headers,
        "body": request.body,
    })
}

async fn send_native_cua_model_http_request(
    request: &NativeCuaModelHttpRequest,
) -> Result<Value, AppError> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .build()
        .map_err(|error| {
            AppError::runtime(format!(
                "Failed to build native CUA model HTTP client: {}",
                error
            ))
        })?;

    let mut builder = client.post(&request.url);
    for (name, value) in &request.headers {
        builder = builder.header(name, value);
    }
    let response = builder.json(&request.body).send().await.map_err(|error| {
        AppError::runtime(format!("Failed to invoke native CUA model: {}", error))
    })?;
    let status = response.status();
    let body_text = response.text().await.map_err(|error| {
        AppError::runtime(format!(
            "Failed to read native CUA model response body: {}",
            error
        ))
    })?;

    if !status.is_success() {
        return Err(AppError::runtime(format!(
            "native CUA model request failed with HTTP {}: {}",
            status.as_u16(),
            truncate_chars(&body_text, 500)
        )));
    }

    serde_json::from_str(&body_text).map_err(|error| {
        AppError::runtime(format!(
            "Failed to parse native CUA model response JSON: {}",
            error
        ))
    })
}

fn parse_native_cua_model_output(provider: &str, raw_output: &Value) -> Result<Value, AppError> {
    match provider {
        "anthropic" => parse_model_json_from_value(
            raw_output
                .get("content")
                .ok_or_else(|| AppError::runtime("anthropic response missing `content`"))?,
        ),
        "ollama" => parse_model_json_from_value(
            raw_output
                .get("message")
                .and_then(|message| message.get("content"))
                .ok_or_else(|| AppError::runtime("ollama response missing `message.content`"))?,
        ),
        _ => parse_model_json_from_value(
            raw_output
                .get("choices")
                .and_then(Value::as_array)
                .and_then(|choices| choices.first())
                .and_then(|choice| choice.get("message"))
                .and_then(|message| message.get("content"))
                .ok_or_else(|| {
                    AppError::runtime(
                        "OpenAI-compatible response missing `choices[0].message.content`",
                    )
                })?,
        ),
    }
}

fn parse_model_json_from_value(value: &Value) -> Result<Value, AppError> {
    match value {
        Value::String(text) => parse_model_json_from_text(text),
        Value::Array(items) => {
            if items.iter().all(|item| item.get("text").is_some()) {
                let combined = items
                    .iter()
                    .filter_map(|item| item.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n");
                parse_model_json_from_text(&combined)
            } else {
                Ok(Value::Array(items.clone()))
            }
        }
        Value::Object(_) => Ok(value.clone()),
        _ => Err(AppError::runtime(
            "native CUA model output did not contain JSON text or JSON object",
        )),
    }
}

fn parse_model_json_from_text(text: &str) -> Result<Value, AppError> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err(AppError::runtime("native CUA model returned empty content"));
    }

    let direct = strip_json_code_fence(trimmed);
    if let Ok(parsed) = serde_json::from_str::<Value>(direct) {
        return Ok(parsed);
    }

    if let Some(slice) = best_effort_json_slice(direct)
        && let Ok(parsed) = serde_json::from_str::<Value>(slice)
    {
        return Ok(parsed);
    }

    Err(AppError::runtime(format!(
        "native CUA model content was not valid JSON: {}",
        truncate_chars(trimmed, 240)
    )))
}

fn strip_json_code_fence(text: &str) -> &str {
    let trimmed = text.trim();
    if let Some(rest) = trimmed.strip_prefix("```json") {
        return rest.strip_suffix("```").unwrap_or(rest).trim();
    }
    if let Some(rest) = trimmed.strip_prefix("```") {
        return rest.strip_suffix("```").unwrap_or(rest).trim();
    }
    trimmed
}

fn best_effort_json_slice(text: &str) -> Option<&str> {
    let candidates = [
        (text.find('{'), text.rfind('}')),
        (text.find('['), text.rfind(']')),
    ];
    for (start, end) in candidates {
        if let (Some(start), Some(end)) = (start, end)
            && start < end
        {
            return Some(&text[start..=end]);
        }
    }
    None
}

fn native_cua_plan_task_for_db(
    db: &Database,
    request: NativeCuaPlanTaskRequest,
) -> Result<NativeCuaPlanResponse, AppError> {
    let session = match normalize_optional_text(request.session_id.clone()) {
        Some(session_id) => resolve_session(db, Some(session_id))?,
        None => resolve_session(db, None)?,
    };
    let task = match request.task {
        Some(task) => validate_bounded_text("task", task, NATIVE_CUA_TASK_MAX_CHARS)?,
        None => session.task.clone(),
    };
    let max_steps = request.max_steps.unwrap_or(8).clamp(3, 24);
    let skills = request.skill_catalog.unwrap_or_default();
    let selected_skills = select_skills_for_task(&task, &skills);
    let steps = build_local_plan_steps(&task, max_steps);
    let now = Utc::now().to_rfc3339();
    let response = NativeCuaPlanResponse {
        session_id: session.session_id.clone(),
        task: task.clone(),
        created_at: now.clone(),
        updated_at: now.clone(),
        source: "hermes_native_local_planner".to_string(),
        status: "planned".to_string(),
        selected_skills,
        iteration_info: json!({
            "current_iteration": 1,
            "total_iterations": 1,
            "planner": "local_deterministic",
            "not_osworld_or_sota_evidence": true,
        }),
        steps,
        summary: format!(
            "Planned Hermes native CUA loop for session `{}` using local deterministic planner.",
            session.session_id
        ),
    };
    save_plan(db, response.clone())?;
    record_audit_event(
        db,
        NativeCuaAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: now,
            event_type: "plan".to_string(),
            status: "planned".to_string(),
            session_id: Some(session.session_id),
            dry_run: None,
            summary: response.summary.clone(),
            planned_command: Vec::new(),
            screenshot_path: None,
            action_type: None,
        },
    )?;
    Ok(response)
}

fn native_cua_run_step_for_db_with_executor<F>(
    db: &Database,
    request: NativeCuaRunStepRequest,
    executor: F,
) -> Result<NativeCuaRunStepResponse, AppError>
where
    F: Copy + Fn(&[String]) -> Result<NativeCuaExecutionOutcome, AppError>,
{
    let session = resolve_session(db, request.session_id.clone())?;
    let dry_run = request.dry_run.unwrap_or(true);
    let history_len = load_history(db)?
        .into_iter()
        .filter(|step| step.session_id == session.session_id)
        .count();
    let step_index = history_len + 1;
    let observation = native_cua_observe_for_db_with_executor(
        db,
        NativeCuaObserveRequest {
            session_id: Some(session.session_id.clone()),
            dry_run: Some(true),
            capture_screenshot: request.capture_screenshot,
        },
        executor,
    )?;
    let brain_state = request
        .brain_state
        .unwrap_or_else(|| build_local_brain_state(db, &session, step_index, &observation));
    let actions = match request.actions {
        Some(actions) if !actions.is_empty() => actions,
        _ => default_actions_for_step(db, &session.session_id, step_index),
    };
    let max_actions = request
        .max_actions
        .unwrap_or(NATIVE_CUA_MAX_STEP_ACTIONS)
        .clamp(1, NATIVE_CUA_MAX_STEP_ACTIONS);
    let mut action_results = Vec::new();
    let mut final_result = None;
    let mut status = "success".to_string();

    for raw_action in actions.into_iter().take(max_actions) {
        let translated = translate_turix_action(
            &session.session_id,
            &raw_action,
            dry_run,
            request.confirmation_phrase.clone(),
        )?;
        let action_result = execute_translated_action(db, translated, executor)?;
        if action_result.status == "failed" || action_result.status == "error" {
            status = "error".to_string();
        }
        if action_result.is_done {
            final_result = Some(action_result.summary.clone());
            status = "done".to_string();
            action_results.push(action_result);
            break;
        }
        action_results.push(action_result);
    }

    if action_results.is_empty() {
        status = "wait".to_string();
    }
    let done = status == "done";
    let summary = format!(
        "Native CUA step {} for session `{}` finished with status `{}` and {} action result(s).",
        step_index,
        session.session_id,
        status,
        action_results.len()
    );
    let record = NativeCuaStepRecord {
        id: Uuid::new_v4().to_string(),
        session_id: session.session_id.clone(),
        step_index,
        occurred_at: Utc::now().to_rfc3339(),
        status: status.clone(),
        brain_state,
        observation: Some(observation),
        actions: action_results,
        final_result,
        summary: summary.clone(),
    };
    record_history_step(db, record.clone())?;
    record_audit_event(
        db,
        NativeCuaAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: Utc::now().to_rfc3339(),
            event_type: "step".to_string(),
            status,
            session_id: Some(session.session_id.clone()),
            dry_run: Some(dry_run),
            summary: summary.clone(),
            planned_command: Vec::new(),
            screenshot_path: record
                .observation
                .as_ref()
                .and_then(|observation| observation.screenshot_path.clone()),
            action_type: None,
        },
    )?;

    Ok(NativeCuaRunStepResponse {
        session_id: session.session_id,
        step: record,
        history_len: history_len + 1,
        done,
        summary,
    })
}

fn native_cua_list_history_for_db(
    db: &Database,
    request: NativeCuaHistoryListRequest,
) -> Result<Vec<NativeCuaStepRecord>, AppError> {
    let session_filter = normalize_filter(request.session_id);
    let status_filter = normalize_filter(request.status);
    let mut history = load_history(db)?;
    history.retain(|step| {
        filter_matches(&step.session_id, session_filter.as_deref())
            && filter_matches(&step.status, status_filter.as_deref())
    });
    if let Some(limit) = normalize_list_limit(request.limit) {
        history.truncate(limit.min(NATIVE_CUA_HISTORY_LIMIT));
    }
    Ok(history)
}

fn native_cua_record_info_for_db(
    db: &Database,
    request: NativeCuaRecordInfoRequest,
) -> Result<NativeCuaMemoryRecord, AppError> {
    let session = resolve_session(db, request.session_id)?;
    record_memory_for_session(
        db,
        &session.session_id,
        request.text,
        request.file_name,
        request.screenshot_path,
    )
}

fn native_cua_export_trajectory_for_db(
    db: &Database,
    request: NativeCuaTrajectoryExportRequest,
) -> Result<NativeCuaTrajectoryExportResponse, AppError> {
    let format = normalize_export_format(request.format)?;
    let session_filter = normalize_filter(request.session_id.clone());
    let include_audit = request.include_audit.unwrap_or(true);
    let mut lines = Vec::new();

    for plan in load_plans(db)? {
        if filter_matches(&plan.session_id, session_filter.as_deref()) {
            lines
                .push(json!({"kind":"native_cua_plan","source":"hermes_native_cua","value": plan}));
        }
    }
    for step in load_history(db)? {
        if filter_matches(&step.session_id, session_filter.as_deref()) {
            lines
                .push(json!({"kind":"native_cua_step","source":"hermes_native_cua","value": step}));
        }
    }
    for memory in load_memory_records(db)? {
        if filter_matches(&memory.session_id, session_filter.as_deref()) {
            lines.push(
                json!({"kind":"native_cua_memory","source":"hermes_native_cua","value": memory}),
            );
        }
    }
    if include_audit {
        for event in load_audit_events(db)? {
            if optional_filter_matches(event.session_id.as_deref(), session_filter.as_deref()) {
                lines.push(
                    json!({"kind":"native_cua_audit","source":"hermes_native_cua","value": event}),
                );
            }
        }
    }

    let payload = if format == "json" {
        serde_json::to_string_pretty(&lines).map_err(AppError::from_json_error)?
    } else {
        lines
            .iter()
            .map(|line| serde_json::to_string(line).map_err(AppError::from_json_error))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n")
    };

    Ok(NativeCuaTrajectoryExportResponse {
        session_id: request.session_id,
        format,
        exported_count: lines.len(),
        payload,
    })
}

fn build_local_plan_steps(task: &str, max_steps: usize) -> Vec<NativeCuaPlanStep> {
    let mut goals = split_task_into_goals(task);
    if goals.is_empty() {
        goals.push("Observe the desktop and identify the next safe action.".to_string());
    }
    let mut steps = Vec::new();
    steps.push(NativeCuaPlanStep {
        index: 1,
        goal: "Observe the current desktop and compare it with prior memory.".to_string(),
        suggested_action: "observe".to_string(),
        status: "pending".to_string(),
    });
    for goal in goals.into_iter().take(max_steps.saturating_sub(2)) {
        steps.push(NativeCuaPlanStep {
            index: steps.len() + 1,
            goal,
            suggested_action: "actor_actions".to_string(),
            status: "pending".to_string(),
        });
    }
    steps.push(NativeCuaPlanStep {
        index: steps.len() + 1,
        goal: "Verify the result, record useful information, and finish when the task is complete."
            .to_string(),
        suggested_action: "done_or_record_info".to_string(),
        status: "pending".to_string(),
    });
    steps.truncate(max_steps);
    steps
}

fn split_task_into_goals(task: &str) -> Vec<String> {
    task.split(['\n', ';', '；', '。'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.trim_matches(|ch| ch == '-' || ch == '*')
                .trim()
                .to_string()
        })
        .filter(|part| !part.is_empty())
        .collect()
}

fn select_skills_for_task(task: &str, skills: &[NativeCuaSkillMetadata]) -> Vec<String> {
    let task_lower = task.to_ascii_lowercase();
    skills
        .iter()
        .filter(|skill| {
            let name = skill.name.to_ascii_lowercase();
            let description = skill.description.to_ascii_lowercase();
            task_lower.contains(&name)
                || description
                    .split(|ch: char| !ch.is_ascii_alphanumeric())
                    .filter(|token| token.len() >= 4)
                    .any(|token| task_lower.contains(token))
        })
        .take(5)
        .map(|skill| skill.name.clone())
        .collect()
}

fn build_local_brain_state(
    db: &Database,
    session: &StoredNativeCuaSession,
    step_index: usize,
    observation: &NativeCuaObservation,
) -> Value {
    let previous = load_history(db)
        .unwrap_or_default()
        .into_iter()
        .find(|step| step.session_id == session.session_id);
    json!({
        "analysis": {
            "analysis": observation.summary.clone().unwrap_or_else(|| "Observed native CUA session state.".to_string()),
            "sop_check": "Use dry-run by default, execute only confirmation-gated native actions."
        },
        "current_state": {
            "step_evaluate": previous.map(|step| step.status).unwrap_or_else(|| "first_step".to_string()),
            "next_goal": next_goal_from_plan(db, &session.session_id, step_index)
                .unwrap_or_else(|| session.task.clone()),
            "memory": latest_memory_summary(db, &session.session_id),
            "not_osworld_or_sota_evidence": true
        }
    })
}

fn next_goal_from_plan(db: &Database, session_id: &str, step_index: usize) -> Option<String> {
    load_plans(db)
        .ok()?
        .into_iter()
        .find(|plan| plan.session_id == session_id)
        .and_then(|plan| {
            plan.steps
                .into_iter()
                .find(|step| step.index == step_index)
                .map(|step| step.goal)
        })
}

fn default_actions_for_step(db: &Database, session_id: &str, step_index: usize) -> Vec<Value> {
    let is_last_planned_step = load_plans(db)
        .ok()
        .and_then(|plans| plans.into_iter().find(|plan| plan.session_id == session_id))
        .map(|plan| step_index >= plan.steps.len())
        .unwrap_or(false);
    if is_last_planned_step {
        vec![
            json!({"done": {"text": "Native CUA local loop reached the final planned verification step."}}),
        ]
    } else {
        vec![
            json!({"wait": {"text": "Planner step requires actor/model-provided concrete action."}}),
        ]
    }
}

#[derive(Debug, Clone)]
struct TranslatedNativeCuaAction {
    action_name: String,
    raw_action: Value,
    native_request: Option<NativeCuaActionRequest>,
    record_text: Option<String>,
    record_file_name: Option<String>,
    done_text: Option<String>,
}

fn translate_turix_action(
    session_id: &str,
    raw_action: &Value,
    dry_run: bool,
    confirmation_phrase: Option<String>,
) -> Result<TranslatedNativeCuaAction, AppError> {
    let object = raw_action
        .as_object()
        .ok_or_else(|| AppError::validation("native CUA step action must be a JSON object"))?;
    if object.len() != 1 {
        return Err(AppError::validation(
            "native CUA step action must contain exactly one action key",
        ));
    }
    let (name, payload) = object.iter().next().expect("one action should exist");
    let payload = if payload.is_object() {
        payload
    } else {
        &json!({})
    };
    let action_name = name.to_string();
    let mut native_request = NativeCuaActionRequest {
        session_id: Some(session_id.to_string()),
        action_type: action_name.clone(),
        text: None,
        key: None,
        modifiers: None,
        app: None,
        x: None,
        y: None,
        dx: None,
        dy: None,
        dry_run: Some(dry_run),
        confirmation_phrase,
    };
    let mut record_text = None;
    let mut record_file_name = None;
    let mut done_text = None;

    match name.as_str() {
        "done" => {
            done_text = Some(
                string_field(payload, "text").unwrap_or_else(|| "Task marked done.".to_string()),
            )
        }
        "wait" => native_request.action_type = "wait".to_string(),
        "input_text" => {
            native_request.action_type = "type_text".to_string();
            native_request.text = Some(required_json_string(payload, "text", name)?);
        }
        "open_app" => {
            native_request.action_type = "open_app".to_string();
            native_request.app = Some(required_json_string(payload, "app_name", name)?);
        }
        "run_apple_script" => {
            native_request.action_type = "run_apple_script".to_string();
            native_request.text = Some(required_json_string(payload, "script", name)?);
        }
        "Hotkey" => {
            native_request.action_type = "key_press".to_string();
            native_request.key = Some(required_json_string(payload, "key", name)?);
        }
        "multi_Hotkey" => {
            let mut keys = ["key1", "key2", "key3"]
                .into_iter()
                .filter_map(|field| string_field(payload, field))
                .collect::<Vec<_>>();
            if keys.len() < 2 {
                return Err(AppError::validation(
                    "multi_Hotkey requires at least key1 and key2",
                ));
            }
            let key = keys.pop().unwrap_or_default();
            native_request.action_type = "hotkey".to_string();
            native_request.key = Some(key);
            native_request.modifiers = Some(keys);
        }
        "Click" => {
            native_request.action_type = "click".to_string();
            let (x, y) = required_position(payload, "position", name)?;
            native_request.x = Some(x);
            native_request.y = Some(y);
        }
        "RightSingle" => {
            native_request.action_type = "right_click".to_string();
            let (x, y) = required_position(payload, "position", name)?;
            native_request.x = Some(x);
            native_request.y = Some(y);
        }
        "move_mouse" => {
            native_request.action_type = "move_mouse".to_string();
            let (x, y) = required_position(payload, "position", name)?;
            native_request.x = Some(x);
            native_request.y = Some(y);
        }
        "Drag" => {
            native_request.action_type = "drag_mouse".to_string();
            let (x1, y1) = required_position(payload, "position1", name)?;
            let (x2, y2) = required_position(payload, "position2", name)?;
            native_request.x = Some(x1);
            native_request.y = Some(y1);
            native_request.dx = Some(((x2 - x1) * 10_000.0).round());
            native_request.dy = Some(((y2 - y1) * 10_000.0).round());
        }
        "scroll_up" | "scroll_down" => {
            native_request.action_type = "scroll".to_string();
            let (x, y) = required_position(payload, "position", name)?;
            native_request.x = Some(x);
            native_request.y = Some(y);
            let amount = number_field(payload, "dy")
                .unwrap_or(5.0)
                .abs()
                .clamp(1.0, 25.0);
            native_request.dy = Some(if name == "scroll_up" { amount } else { -amount });
            native_request.dx = number_field(payload, "dx");
        }
        "record_info" => {
            record_text = Some(required_json_string(payload, "text", name)?);
            record_file_name = Some(required_json_string(payload, "file_name", name)?);
        }
        _ => {
            native_request.action_type = normalize_action_type(name)?;
        }
    }

    Ok(TranslatedNativeCuaAction {
        action_name,
        raw_action: raw_action.clone(),
        native_request: if record_text.is_some() || done_text.is_some() {
            None
        } else {
            Some(native_request)
        },
        record_text,
        record_file_name,
        done_text,
    })
}

fn execute_translated_action<F>(
    db: &Database,
    translated: TranslatedNativeCuaAction,
    executor: F,
) -> Result<NativeCuaStepActionResult, AppError>
where
    F: Fn(&[String]) -> Result<NativeCuaExecutionOutcome, AppError>,
{
    if let Some(text) = translated.done_text {
        return Ok(NativeCuaStepActionResult {
            action_name: translated.action_name,
            raw_action: translated.raw_action,
            status: "done".to_string(),
            summary: text,
            native_result: None,
            memory_record: None,
            is_done: true,
        });
    }

    if let Some(text) = translated.record_text {
        let request = NativeCuaRecordInfoRequest {
            session_id: translated
                .native_request
                .as_ref()
                .and_then(|request| request.session_id.clone()),
            text,
            file_name: translated
                .record_file_name
                .unwrap_or_else(|| "record_info".to_string()),
            screenshot_path: None,
        };
        let memory_record = native_cua_record_info_for_db(db, request)?;
        let summary = format!("Recorded info to `{}`.", memory_record.file_name);
        return Ok(NativeCuaStepActionResult {
            action_name: translated.action_name,
            raw_action: translated.raw_action,
            status: "recorded".to_string(),
            summary,
            native_result: None,
            memory_record: Some(memory_record),
            is_done: false,
        });
    }

    let native_request = translated.native_request.ok_or_else(|| {
        AppError::validation("translated native CUA action had no executable payload")
    })?;
    let native_result =
        native_cua_execute_action_for_db_with_executor(db, native_request, executor)?;
    let status = native_result.status.clone();
    let summary = native_result
        .summary
        .clone()
        .unwrap_or_else(|| status.clone());
    Ok(NativeCuaStepActionResult {
        action_name: translated.action_name,
        raw_action: translated.raw_action,
        status,
        summary,
        native_result: Some(native_result),
        memory_record: None,
        is_done: false,
    })
}

fn required_json_string(payload: &Value, field: &str, action: &str) -> Result<String, AppError> {
    string_field(payload, field).ok_or_else(|| {
        AppError::validation(format!(
            "native CUA TuriX action `{}` requires `{}`",
            action, field
        ))
    })
}

fn string_field(payload: &Value, field: &str) -> Option<String> {
    payload
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn number_field(payload: &Value, field: &str) -> Option<f64> {
    payload.get(field).and_then(Value::as_f64)
}

fn required_position(payload: &Value, field: &str, action: &str) -> Result<(f64, f64), AppError> {
    let values = payload
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| {
            AppError::validation(format!(
                "native CUA action `{}` requires `{}`",
                action, field
            ))
        })?;
    if values.len() < 2 {
        return Err(AppError::validation(format!(
            "native CUA action `{}` field `{}` must contain x and y",
            action, field
        )));
    }
    let x = values[0]
        .as_f64()
        .ok_or_else(|| AppError::validation("native CUA x coordinate must be numeric"))?;
    let y = values[1]
        .as_f64()
        .ok_or_else(|| AppError::validation("native CUA y coordinate must be numeric"))?;
    Ok((
        normalize_coordinate_value("x", x)?,
        normalize_coordinate_value("y", y)?,
    ))
}

fn save_plan(db: &Database, plan: NativeCuaPlanResponse) -> Result<(), AppError> {
    let mut plans = load_plans(db)?;
    plans.retain(|existing| existing.session_id != plan.session_id);
    plans.insert(0, plan);
    plans.truncate(NATIVE_CUA_PLAN_LIMIT);
    save_app_setting(db, NATIVE_CUA_PLANS_KEY, &plans)
}

fn load_plans(db: &Database) -> Result<Vec<NativeCuaPlanResponse>, AppError> {
    load_app_setting(db, NATIVE_CUA_PLANS_KEY, "native CUA plans")
}

fn record_history_step(db: &Database, step: NativeCuaStepRecord) -> Result<(), AppError> {
    let mut history = load_history(db)?;
    history.insert(0, step);
    history.truncate(NATIVE_CUA_HISTORY_LIMIT);
    save_app_setting(db, NATIVE_CUA_HISTORY_KEY, &history)
}

fn load_history(db: &Database) -> Result<Vec<NativeCuaStepRecord>, AppError> {
    load_app_setting(db, NATIVE_CUA_HISTORY_KEY, "native CUA history")
}

fn record_memory_for_session(
    db: &Database,
    session_id: &str,
    text: String,
    file_name: String,
    screenshot_path: Option<String>,
) -> Result<NativeCuaMemoryRecord, AppError> {
    let text = validate_bounded_text("record_info.text", text, NATIVE_CUA_TEXT_MAX_CHARS)?;
    let safe_file_name = sanitize_record_file_name(&file_name);
    let dir = env::temp_dir()
        .join("hermes-native-cua")
        .join("records")
        .join(sanitize_path_fragment(session_id));
    fs::create_dir_all(&dir).map_err(AppError::from_io_error)?;
    let path = unique_record_path(dir.join(&safe_file_name));
    fs::write(&path, &text).map_err(AppError::from_io_error)?;
    let record = NativeCuaMemoryRecord {
        id: Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&safe_file_name)
            .to_string(),
        text,
        path: path.display().to_string(),
        screenshot_path,
        created_at: Utc::now().to_rfc3339(),
    };
    let mut records = load_memory_records(db)?;
    records.insert(0, record.clone());
    records.truncate(NATIVE_CUA_MEMORY_LIMIT);
    save_app_setting(db, NATIVE_CUA_MEMORY_KEY, &records)?;
    record_audit_event(
        db,
        NativeCuaAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: Utc::now().to_rfc3339(),
            event_type: "memory".to_string(),
            status: "recorded".to_string(),
            session_id: Some(session_id.to_string()),
            dry_run: None,
            summary: format!("Recorded native CUA memory file `{}`.", record.file_name),
            planned_command: Vec::new(),
            screenshot_path: record.screenshot_path.clone(),
            action_type: Some("record_info".to_string()),
        },
    )?;
    Ok(record)
}

fn load_memory_records(db: &Database) -> Result<Vec<NativeCuaMemoryRecord>, AppError> {
    load_app_setting(db, NATIVE_CUA_MEMORY_KEY, "native CUA memory records")
}

fn latest_memory_summary(db: &Database, session_id: &str) -> String {
    let records = load_memory_records(db).unwrap_or_default();
    let names = records
        .into_iter()
        .filter(|record| record.session_id == session_id)
        .take(5)
        .map(|record| record.file_name)
        .collect::<Vec<_>>();
    if names.is_empty() {
        "No recorded native CUA memory yet.".to_string()
    } else {
        format!("Recent recorded files: {}", names.join(", "))
    }
}

fn load_app_setting<T>(db: &Database, key: &str, label: &str) -> Result<Vec<T>, AppError>
where
    T: for<'de> Deserialize<'de>,
{
    let stored = db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&key],
        |row| row.get::<_, String>(0),
    );
    match stored {
        Ok(value_json) => {
            serde_json::from_str::<Vec<T>>(&value_json).map_err(AppError::from_json_error)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Vec::new()),
        Err(error) => Err(AppError::storage(format!(
            "Failed to load {}: {}",
            label, error
        ))),
    }
}

fn save_app_setting<T>(db: &Database, key: &str, value: &[T]) -> Result<(), AppError>
where
    T: Serialize,
{
    let value_json = serde_json::to_string(value).map_err(AppError::from_json_error)?;
    let now = Utc::now().to_rfc3339();
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
        &[&key, &value_json, &now],
    )?;
    Ok(())
}

fn sanitize_record_file_name(file_name: &str) -> String {
    let mut cleaned = file_name
        .trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches(|ch| matches!(ch, '.' | '_' | '-'))
        .to_string();
    if cleaned.is_empty() {
        cleaned = "record_info".to_string();
    }
    if !cleaned.to_ascii_lowercase().ends_with(".txt") {
        cleaned.push_str(".txt");
    }
    cleaned.chars().take(80).collect()
}

fn unique_record_path(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("record_info")
        .to_string();
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("txt");
    for index in 1..1_000 {
        let candidate = path.with_file_name(format!("{}_{}.{}", stem, index, extension));
        if !candidate.exists() {
            return candidate;
        }
    }
    path
}

fn normalize_model_role(role: &str) -> Result<String, AppError> {
    let normalized = role.trim().to_ascii_lowercase().replace('-', "_");
    match normalized.as_str() {
        "brain" | "actor" | "planner" | "memory" => Ok(normalized),
        _ => Err(AppError::validation(format!(
            "unsupported native CUA model role `{}`",
            role
        ))),
    }
}

fn turix_action_catalog() -> Vec<String> {
    [
        "done",
        "input_text",
        "open_app",
        "run_apple_script",
        "Hotkey",
        "multi_Hotkey",
        "Click",
        "RightSingle",
        "Drag",
        "move_mouse",
        "scroll_up",
        "scroll_down",
        "record_info",
        "wait",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn response_schema_for_model_role(role: &str) -> Value {
    match role {
        "actor" => json!({
            "action": {
                "type": "array",
                "items": "TuriX-compatible single-key action object",
                "allowed_keys": turix_action_catalog(),
            }
        }),
        "brain" => json!({
            "analysis": {"analysis": "string", "sop_check": "string"},
            "current_state": {"step_evaluate": "string", "next_goal": "string", "memory": "string"}
        }),
        "planner" => json!({
            "natural_language_plan": "string",
            "step_by_step_plan": [{"description": "string", "important_search_info": "string"}],
            "selected_skills": ["string"],
            "iteration_info": {"current_iteration": "number", "total_iterations": "number"}
        }),
        "memory" => json!({
            "summary_memory": "string",
            "recent_memory": "string",
            "records_to_keep": ["string"]
        }),
        _ => json!({}),
    }
}

fn model_system_prompt(role: &str) -> String {
    match role {
        "brain" => "You are the Hermes Native CUA Brain. Read observations/history, evaluate the previous step, and choose the next goal. Return only JSON matching the schema.".to_string(),
        "actor" => "You are the Hermes Native CUA Actor. Return only JSON with an action array using the TuriX-compatible action schema. Prefer wait over unsafe actions; live execution is confirmation-gated.".to_string(),
        "planner" => "You are the Hermes Native CUA Planner. Build a concise step-by-step plan and select relevant skills by name. Return only JSON matching the schema.".to_string(),
        "memory" => "You are the Hermes Native CUA Memory module. Compress history and records into durable memory. Return only JSON matching the schema.".to_string(),
        _ => "You are a Hermes Native CUA model role. Return strict JSON.".to_string(),
    }
}

fn build_model_user_prompt(
    role: &str,
    session: &StoredNativeCuaSession,
    plans: &[NativeCuaPlanResponse],
    history: &[NativeCuaStepRecord],
    memories: &[NativeCuaMemoryRecord],
    extra_context: Option<&str>,
) -> Result<String, AppError> {
    let payload = json!({
        "role": role,
        "session": session,
        "plans": plans,
        "recent_history": history,
        "memory_records": memories,
        "extra_context": extra_context,
        "safety": {
            "dry_run_default": true,
            "live_confirmation_phrase": NATIVE_CUA_CONFIRM_PHRASE,
            "not_osworld_or_sota_evidence": true
        },
        "action_catalog": turix_action_catalog(),
        "response_schema": response_schema_for_model_role(role),
    });
    serde_json::to_string_pretty(&payload).map_err(AppError::from_json_error)
}

fn latest_screenshot_attachment(history: &[NativeCuaStepRecord]) -> Option<Value> {
    history.iter().find_map(|step| {
        step.observation.as_ref().and_then(|observation| {
            observation.screenshot_path.as_ref().map(|path| {
                json!({
                    "type": "screenshot_path",
                    "path": path,
                    "note": "No base64 dependency is added; model runners can load this local file if authorized."
                })
            })
        })
    })
}

fn record_model_turn(db: &Database, record: NativeCuaModelTurnRecord) -> Result<(), AppError> {
    let mut records = load_model_turns(db)?;
    records.insert(0, record);
    records.truncate(NATIVE_CUA_HISTORY_LIMIT);
    save_app_setting(db, NATIVE_CUA_MODEL_TURNS_KEY, &records)
}

fn load_model_turns(db: &Database) -> Result<Vec<NativeCuaModelTurnRecord>, AppError> {
    load_app_setting(db, NATIVE_CUA_MODEL_TURNS_KEY, "native CUA model turns")
}

fn load_sessions(db: &Database) -> Result<Vec<StoredNativeCuaSession>, AppError> {
    let stored = db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&NATIVE_CUA_SESSIONS_KEY],
        |row| row.get::<_, String>(0),
    );

    match stored {
        Ok(value_json) => serde_json::from_str::<Vec<StoredNativeCuaSession>>(&value_json)
            .map_err(AppError::from_json_error),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Vec::new()),
        Err(error) => Err(AppError::storage(format!(
            "Failed to load native CUA sessions: {}",
            error
        ))),
    }
}

fn save_sessions(db: &Database, sessions: &[StoredNativeCuaSession]) -> Result<(), AppError> {
    let value_json = serde_json::to_string(sessions).map_err(AppError::from_json_error)?;
    let now = Utc::now().to_rfc3339();
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
        &[&NATIVE_CUA_SESSIONS_KEY, &value_json, &now],
    )?;
    Ok(())
}

fn touch_session(db: &Database, session_id: &str) -> Result<(), AppError> {
    let now = Utc::now().to_rfc3339();
    let mut sessions = load_sessions(db)?;
    if let Some(session) = sessions
        .iter_mut()
        .find(|session| session.session_id == session_id)
    {
        session.updated_at = now;
        save_sessions(db, &sessions)?;
        Ok(())
    } else {
        Err(AppError::validation(format!(
            "native CUA session `{}` was not found",
            session_id
        )))
    }
}

fn resolve_session(
    db: &Database,
    session_id: Option<String>,
) -> Result<StoredNativeCuaSession, AppError> {
    let sessions = load_sessions(db)?;
    if let Some(session_id) = normalize_optional_text(session_id) {
        return sessions
            .into_iter()
            .find(|session| session.session_id == session_id)
            .ok_or_else(|| {
                AppError::validation(format!("native CUA session `{}` was not found", session_id))
            });
    }

    sessions
        .into_iter()
        .find(|session| session.status == "active")
        .ok_or_else(|| {
            AppError::validation("start or resume a native CUA session before observe/action calls")
        })
}

fn session_response(
    session: &StoredNativeCuaSession,
    resumed: bool,
    summary: Option<String>,
) -> NativeCuaSession {
    NativeCuaSession {
        session_id: session.session_id.clone(),
        status: session.status.clone(),
        task: Some(session.task.clone()),
        resumed,
        created_at: Some(session.created_at.clone()),
        updated_at: Some(session.updated_at.clone()),
        summary,
        model_mode: session
            .model_mode
            .clone()
            .or_else(|| Some("auto".to_string())),
        provider: session.provider.clone(),
        model: session.model.clone(),
        base_url: session.base_url.clone(),
        api_key_ref: session.api_key_ref.clone(),
        model_difficulty: session.model_difficulty.clone(),
        model_selection_reason: session.model_selection_reason.clone(),
    }
}

fn resolve_session_model_config(
    db: &Database,
    task: &str,
    model_mode: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    api_key_ref: Option<String>,
) -> Result<NativeCuaSessionModelConfig, AppError> {
    let provider = normalize_optional_text(provider);
    let model = normalize_optional_text(model);
    let base_url = normalize_optional_text(base_url);
    let api_key_ref = normalize_optional_text(api_key_ref);
    let mode = normalize_optional_text(model_mode)
        .map(|mode| mode.to_ascii_lowercase().replace('-', "_"))
        .unwrap_or_else(|| {
            if provider.is_some() || model.is_some() || base_url.is_some() || api_key_ref.is_some()
            {
                "custom".to_string()
            } else {
                "auto".to_string()
            }
        });

    match mode.as_str() {
        "auto" => resolve_auto_session_model_config(db, task),
        "custom" => {
            let provider =
                normalize_native_cua_provider(provider.unwrap_or_else(|| "openai".to_string()))?;
            let model = model
                .unwrap_or_else(|| default_native_cua_model_for_provider(&provider).to_string());
            let base_url =
                base_url.or_else(|| Some(default_native_cua_base_url(&provider).to_string()));
            Ok(NativeCuaSessionModelConfig {
                mode,
                provider: Some(provider),
                model: Some(model),
                base_url,
                api_key_ref,
                difficulty: None,
                selection_reason: Some(
                    "User selected a custom model for this Native CUA task.".to_string(),
                ),
            })
        }
        _ => Err(AppError::validation(format!(
            "unsupported native CUA session model mode `{}`; use `auto` or `custom`",
            mode
        ))),
    }
}

fn resolve_auto_session_model_config(
    db: &Database,
    task: &str,
) -> Result<NativeCuaSessionModelConfig, AppError> {
    let runtime = load_native_cua_runtime_settings(db)?;
    let (difficulty, reason) = classify_native_cua_task_difficulty(task);
    let profile = runtime
        .native_cua_auto_models
        .as_ref()
        .and_then(|settings| match difficulty.as_str() {
            "easy" => settings.easy.as_ref(),
            "hard" => settings.hard.as_ref(),
            _ => settings.standard.as_ref(),
        });
    let has_tier_profile = profile.is_some();
    let profile_provider =
        profile.and_then(|profile| normalize_optional_text(profile.provider.clone()));
    let runtime_provider = normalize_optional_text(runtime.provider.clone());
    let provider = normalize_native_cua_provider(
        profile_provider
            .clone()
            .or(runtime_provider.clone())
            .unwrap_or_else(|| "openai".to_string()),
    )?;
    let use_runtime_model = (!has_tier_profile || difficulty == "standard")
        && runtime_provider
            .as_deref()
            .is_some_and(|runtime_provider| runtime_provider == provider);
    let model = profile
        .and_then(|profile| normalize_optional_text(profile.model.clone()))
        .or_else(|| {
            if use_runtime_model {
                normalize_optional_text(runtime.model.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| {
            default_native_cua_auto_model_for_provider(&provider, &difficulty).to_string()
        });
    let base_url = profile
        .and_then(|profile| normalize_optional_text(profile.base_url.clone()))
        .or_else(|| {
            if runtime_provider
                .as_deref()
                .is_some_and(|runtime_provider| runtime_provider == provider)
            {
                normalize_optional_text(runtime.base_url.clone())
            } else {
                None
            }
        })
        .or_else(|| Some(default_native_cua_base_url(&provider).to_string()));
    let api_key_ref = profile
        .and_then(|profile| normalize_optional_text(profile.api_key_ref.clone()))
        .or_else(|| normalize_optional_text(runtime.api_key_ref.clone()));

    Ok(NativeCuaSessionModelConfig {
        mode: "auto".to_string(),
        provider: Some(provider.clone()),
        model: Some(model.clone()),
        base_url,
        api_key_ref,
        difficulty: Some(difficulty.clone()),
        selection_reason: Some(format!(
            "Auto selected `{}` / `{}` for `{}` difficulty. {}",
            provider, model, difficulty, reason
        )),
    })
}

fn classify_native_cua_task_difficulty(task: &str) -> (String, String) {
    let normalized = task.to_ascii_lowercase();
    let char_count = task.chars().count();
    let hard_keywords = [
        "multi-step",
        "workflow",
        "analyze",
        "research",
        "compare",
        "debug",
        "implement",
        "integrate",
        "spreadsheet",
        "browser",
        "email",
        "calendar",
        "复杂",
        "分析",
        "研究",
        "对比",
        "调试",
        "实现",
        "集成",
        "多个",
        "全部",
        "跨应用",
        "表格",
        "浏览器",
    ];
    let easy_keywords = [
        "click",
        "open",
        "type",
        "press",
        "wait",
        "screenshot",
        "点击",
        "打开",
        "输入",
        "等待",
    ];
    let hard_hits = hard_keywords
        .iter()
        .filter(|keyword| normalized.contains(**keyword))
        .count();
    let easy_hits = easy_keywords
        .iter()
        .filter(|keyword| normalized.contains(**keyword))
        .count();

    if char_count >= 700 || hard_hits >= 3 {
        return (
            "hard".to_string(),
            format!("Detected {hard_hits} complex-task signals across {char_count} characters."),
        );
    }
    if char_count >= 220 || hard_hits >= 1 {
        return (
            "standard".to_string(),
            format!(
                "Detected {hard_hits} planning/task-complexity signals across {char_count} characters."
            ),
        );
    }
    if easy_hits > 0 || char_count < 120 {
        return (
            "easy".to_string(),
            format!("Detected {easy_hits} direct-action signals across {char_count} characters."),
        );
    }
    (
        "standard".to_string(),
        format!("No direct-action shortcut matched across {char_count} characters."),
    )
}

fn load_audit_events(db: &Database) -> Result<Vec<NativeCuaAuditEvent>, AppError> {
    let stored = db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&NATIVE_CUA_AUDIT_LOG_KEY],
        |row| row.get::<_, String>(0),
    );

    match stored {
        Ok(value_json) => serde_json::from_str::<Vec<NativeCuaAuditEvent>>(&value_json)
            .map_err(AppError::from_json_error),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Vec::new()),
        Err(error) => Err(AppError::storage(format!(
            "Failed to load native CUA audit log: {}",
            error
        ))),
    }
}

fn record_audit_event(db: &Database, event: NativeCuaAuditEvent) -> Result<(), AppError> {
    let mut log = load_audit_events(db)?;
    log.insert(0, event);
    log.truncate(NATIVE_CUA_AUDIT_LOG_LIMIT);
    let value_json = serde_json::to_string(&log).map_err(AppError::from_json_error)?;
    let now = Utc::now().to_rfc3339();
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
        &[&NATIVE_CUA_AUDIT_LOG_KEY, &value_json, &now],
    )?;
    Ok(())
}

fn normalize_action_request(
    mut request: NativeCuaActionRequest,
) -> Result<NativeCuaActionRequest, AppError> {
    request.action_type = normalize_action_type(&request.action_type)?;
    request.text = match request.text.take() {
        Some(text) => Some(validate_bounded_text(
            "text",
            text,
            NATIVE_CUA_TEXT_MAX_CHARS,
        )?),
        None => None,
    };
    request.key = normalize_optional_text(request.key);
    request.app = normalize_optional_text(request.app);
    request.modifiers = request.modifiers.map(|modifiers| {
        modifiers
            .into_iter()
            .filter_map(|modifier| normalize_optional_text(Some(modifier)))
            .collect::<Vec<_>>()
    });
    if matches!(request.modifiers.as_ref(), Some(modifiers) if modifiers.is_empty()) {
        request.modifiers = None;
    }
    request.x = normalize_coordinate("x", request.x)?;
    request.y = normalize_coordinate("y", request.y)?;
    Ok(request)
}

fn normalize_action_type(action_type: &str) -> Result<String, AppError> {
    let normalized = action_type.trim().to_ascii_lowercase().replace('-', "_");
    let canonical = match normalized.as_str() {
        "wait" => "wait",
        "done" => "done",
        "open_app" | "launch_app" => "open_app",
        "type_text" | "input_text" => "type_text",
        "key_press" | "press_key" => "key_press",
        "hotkey" | "multi_hotkey" => "hotkey",
        "run_apple_script" | "apple_script" => "run_apple_script",
        "click" => "click",
        "double_click" => "double_click",
        "right_click" => "right_click",
        "move_mouse" | "move_pointer" => "move_mouse",
        "drag_mouse" | "drag_pointer" => "drag_mouse",
        "scroll" => "scroll",
        _ => {
            return Err(AppError::validation(format!(
                "unsupported native CUA action type `{}`",
                action_type
            )));
        }
    };
    Ok(canonical.to_string())
}

fn plan_action_command(
    request: &NativeCuaActionRequest,
    platform: NativeCuaPlatform,
    screen_size: Option<ScreenSize>,
) -> Result<NativeCuaCommandPlan, AppError> {
    if request.action_type == "wait" || request.action_type == "done" {
        return Ok(NativeCuaCommandPlan {
            command: Vec::new(),
            requires_command: false,
            summary: format!(
                "{} action uses an internal state transition.",
                request.action_type
            ),
        });
    }
    if request.action_type == "run_apple_script" {
        let script = required_field("text", request.text.as_deref(), "run_apple_script")?;
        return Ok(NativeCuaCommandPlan {
            command: vec![
                "osascript".to_string(),
                "-e".to_string(),
                script.to_string(),
            ],
            requires_command: true,
            summary: "Planned macOS AppleScript command.".to_string(),
        });
    }

    let command = match platform {
        NativeCuaPlatform::Linux => plan_linux_action(request, screen_size)?,
        NativeCuaPlatform::Macos => plan_macos_action(request)?,
        NativeCuaPlatform::Windows => plan_windows_action(request)?,
        NativeCuaPlatform::Unsupported => {
            return Err(AppError::runtime(format!(
                "native CUA live execution is unsupported on platform `{}`",
                env::consts::OS
            )));
        }
    };
    Ok(NativeCuaCommandPlan {
        summary: format!("Planned native CUA `{}` command.", request.action_type),
        requires_command: true,
        command,
    })
}

fn plan_linux_action(
    request: &NativeCuaActionRequest,
    screen_size: Option<ScreenSize>,
) -> Result<Vec<String>, AppError> {
    let mut command = vec!["xdotool".to_string()];
    match request.action_type.as_str() {
        "open_app" => {
            let app = required_field("app", request.app.as_deref(), "open_app")?;
            command.extend([
                "search".to_string(),
                "--name".to_string(),
                app.to_string(),
                "windowactivate".to_string(),
                "--sync".to_string(),
            ]);
        }
        "type_text" => {
            let text = required_field("text", request.text.as_deref(), "type_text")?;
            command.extend([
                "type".to_string(),
                "--delay".to_string(),
                "0".to_string(),
                text.to_string(),
            ]);
        }
        "key_press" => {
            let key = required_field("key", request.key.as_deref(), "key_press")?;
            command.extend(["key".to_string(), key.to_string()]);
        }
        "hotkey" => {
            let key = required_field("key", request.key.as_deref(), "hotkey")?;
            let combo = linux_hotkey_combo(request.modifiers.as_deref().unwrap_or(&[]), key);
            command.extend(["key".to_string(), combo]);
        }
        "click" | "double_click" | "right_click" | "move_mouse" => {
            let (x, y) = scaled_coordinates(request, screen_size)?;
            command.extend(["mousemove".to_string(), x.to_string(), y.to_string()]);
            match request.action_type.as_str() {
                "click" => command.extend(["click".to_string(), "1".to_string()]),
                "double_click" => command.extend([
                    "click".to_string(),
                    "--repeat".to_string(),
                    "2".to_string(),
                    "1".to_string(),
                ]),
                "right_click" => command.extend(["click".to_string(), "3".to_string()]),
                _ => {}
            }
        }
        "drag_mouse" => {
            let (x, y) = scaled_coordinates(request, screen_size)?;
            let dx = request.dx.unwrap_or(0.0).round() as i32;
            let dy = request.dy.unwrap_or(0.0).round() as i32;
            command.extend([
                "mousemove".to_string(),
                x.to_string(),
                y.to_string(),
                "mousedown".to_string(),
                "1".to_string(),
                "mousemove_relative".to_string(),
                "--".to_string(),
                dx.to_string(),
                dy.to_string(),
                "mouseup".to_string(),
                "1".to_string(),
            ]);
        }
        "scroll" => {
            let dy = request.dy.unwrap_or(0.0);
            let button = if dy < 0.0 { "5" } else { "4" };
            let repeats = dy.abs().round().clamp(1.0, 20.0) as u32;
            command.extend([
                "click".to_string(),
                "--repeat".to_string(),
                repeats.to_string(),
                button.to_string(),
            ]);
        }
        _ => return Err(AppError::validation("unsupported Linux native CUA action")),
    }
    Ok(command)
}

fn plan_macos_action(request: &NativeCuaActionRequest) -> Result<Vec<String>, AppError> {
    let script = match request.action_type.as_str() {
        "open_app" => format!(
            "tell application {} to activate",
            applescript_string(required_field("app", request.app.as_deref(), "open_app")?)
        ),
        "run_apple_script" => {
            required_field("text", request.text.as_deref(), "run_apple_script")?.to_string()
        }
        "type_text" => format!(
            "tell application \"System Events\" to keystroke {}",
            applescript_string(required_field(
                "text",
                request.text.as_deref(),
                "type_text"
            )?)
        ),
        "key_press" => format!(
            "tell application \"System Events\" to keystroke {}",
            applescript_string(required_field("key", request.key.as_deref(), "key_press")?)
        ),
        "hotkey" => format!(
            "tell application \"System Events\" to keystroke {} using {{{}}}",
            applescript_string(required_field("key", request.key.as_deref(), "hotkey")?),
            macos_modifier_list(request.modifiers.as_deref().unwrap_or(&[]))
        ),
        "click" | "double_click" | "right_click" | "move_mouse" => {
            let x = normalized_to_screen_units("x", request.x)?;
            let y = normalized_to_screen_units("y", request.y)?;
            match request.action_type.as_str() {
                "move_mouse" => format!(
                    "tell application \"System Events\" to set mouse location to {{{}, {}}}",
                    x, y
                ),
                "right_click" => format!(
                    "tell application \"System Events\" to click at {{{}, {}}} using secondary button",
                    x, y
                ),
                "double_click" => format!(
                    "tell application \"System Events\" to double click at {{{}, {}}}",
                    x, y
                ),
                _ => format!(
                    "tell application \"System Events\" to click at {{{}, {}}}",
                    x, y
                ),
            }
        }
        "drag_mouse" => {
            let x = normalized_to_screen_units("x", request.x)?;
            let y = normalized_to_screen_units("y", request.y)?;
            let dx = request.dx.unwrap_or(0.0).round() as i32;
            let dy = request.dy.unwrap_or(0.0).round() as i32;
            format!(
                "tell application \"System Events\" to drag at {{{}, {}}} by {{{}, {}}}",
                x, y, dx, dy
            )
        }
        "scroll" => {
            let dy = request.dy.unwrap_or(0.0);
            if dy < 0.0 {
                "tell application \"System Events\" to scroll down".to_string()
            } else {
                "tell application \"System Events\" to scroll up".to_string()
            }
        }
        _ => return Err(AppError::validation("unsupported macOS native CUA action")),
    };
    Ok(vec!["osascript".to_string(), "-e".to_string(), script])
}

fn plan_windows_action(request: &NativeCuaActionRequest) -> Result<Vec<String>, AppError> {
    let script = match request.action_type.as_str() {
        "open_app" => format!(
            "Start-Process -FilePath {}",
            powershell_string(required_field("app", request.app.as_deref(), "open_app")?)
        ),
        "type_text" => format!(
            "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait({})",
            powershell_string(required_field(
                "text",
                request.text.as_deref(),
                "type_text"
            )?)
        ),
        "key_press" => format!(
            "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait({})",
            powershell_string(required_field("key", request.key.as_deref(), "key_press")?)
        ),
        "hotkey" => format!(
            "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.SendKeys]::SendWait({})",
            powershell_string(&windows_hotkey_combo(
                request.modifiers.as_deref().unwrap_or(&[]),
                required_field("key", request.key.as_deref(), "hotkey")?,
            ))
        ),
        "click" | "double_click" | "right_click" | "move_mouse" | "drag_mouse" => {
            let x = normalized_to_screen_units("x", request.x)?;
            let y = normalized_to_screen_units("y", request.y)?;
            windows_pointer_script(&request.action_type, x, y, request.dx, request.dy)
        }
        "scroll" => {
            let amount = request.dy.unwrap_or(0.0).round() as i32;
            format!(
                "$signature='[DllImport(\"user32.dll\")] public static extern void mouse_event(int dwFlags, int dx, int dy, int cButtons, int dwExtraInfo);'; Add-Type -MemberDefinition $signature -Name NativeMouse -Namespace Hermes; [Hermes.NativeMouse]::mouse_event(2048,0,0,{},0)",
                amount.saturating_mul(120)
            )
        }
        _ => {
            return Err(AppError::validation(
                "unsupported Windows native CUA action",
            ));
        }
    };
    Ok(vec![
        windows_powershell_command(),
        "-NoProfile".to_string(),
        "-Command".to_string(),
        script,
    ])
}

fn plan_screenshot_command(path: &Path) -> Result<NativeCuaCommandPlan, AppError> {
    match detect_platform() {
        NativeCuaPlatform::Macos => {
            if !command_exists("screencapture") {
                return Err(AppError::runtime(
                    "screencapture is not available for native CUA observe",
                ));
            }
            Ok(NativeCuaCommandPlan {
                command: vec![
                    "screencapture".to_string(),
                    "-x".to_string(),
                    path.display().to_string(),
                ],
                requires_command: true,
                summary: "Planned macOS screencapture command.".to_string(),
            })
        }
        NativeCuaPlatform::Linux => {
            let tool = linux_screenshot_tool().ok_or_else(|| {
                AppError::runtime("no supported Linux screenshot tool found for native CUA observe")
            })?;
            let command = match tool.as_str() {
                "gnome-screenshot" => vec![tool, "-f".to_string(), path.display().to_string()],
                "scrot" => vec![tool, path.display().to_string()],
                "import" => vec![
                    tool,
                    "-window".to_string(),
                    "root".to_string(),
                    path.display().to_string(),
                ],
                "spectacle" => vec![
                    tool,
                    "-b".to_string(),
                    "-n".to_string(),
                    "-o".to_string(),
                    path.display().to_string(),
                ],
                _ => {
                    return Err(AppError::runtime(
                        "unsupported Linux screenshot command planner",
                    ));
                }
            };
            Ok(NativeCuaCommandPlan {
                command,
                requires_command: true,
                summary: "Planned Linux screenshot command.".to_string(),
            })
        }
        NativeCuaPlatform::Windows => Ok(NativeCuaCommandPlan {
            command: vec![
                windows_powershell_command(),
                "-NoProfile".to_string(),
                "-Command".to_string(),
                format!(
                    "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $bounds=[System.Windows.Forms.Screen]::PrimaryScreen.Bounds; $bmp=New-Object System.Drawing.Bitmap $bounds.Width,$bounds.Height; $graphics=[System.Drawing.Graphics]::FromImage($bmp); $graphics.CopyFromScreen($bounds.Location,[System.Drawing.Point]::Empty,$bounds.Size); $bmp.Save({}); $graphics.Dispose(); $bmp.Dispose()",
                    powershell_string(&path.display().to_string())
                ),
            ],
            requires_command: true,
            summary: "Planned Windows PowerShell screenshot command.".to_string(),
        }),
        NativeCuaPlatform::Unsupported => Err(AppError::runtime(format!(
            "native CUA observe is unsupported on platform `{}`",
            env::consts::OS
        ))),
    }
}

fn execute_command(command: &[String]) -> Result<NativeCuaExecutionOutcome, AppError> {
    if command.is_empty() {
        return Ok(NativeCuaExecutionOutcome {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: Some(0),
        });
    }

    let mut child = Command::new(&command[0])
        .args(&command[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            AppError::runtime(format!("failed to start `{}`: {}", command[0], error))
        })?;

    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let output = child.wait_with_output().map_err(AppError::from_io_error)?;
                return Ok(NativeCuaExecutionOutcome {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: output.status.code(),
                });
            }
            Ok(None) if started.elapsed() >= NATIVE_CUA_EXECUTION_TIMEOUT => {
                let _ = child.kill();
                let output = child.wait_with_output().map_err(AppError::from_io_error)?;
                return Ok(NativeCuaExecutionOutcome {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: format!(
                        "{}\ncommand timed out after {:?}",
                        String::from_utf8_lossy(&output.stderr),
                        NATIVE_CUA_EXECUTION_TIMEOUT
                    ),
                    exit_code: None,
                });
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(error) => {
                return Err(AppError::runtime(format!(
                    "failed while waiting for `{}`: {}",
                    command[0], error
                )));
            }
        }
    }
}

fn ensure_command_available(command: &[String]) -> Result<(), AppError> {
    if command.first().is_none_or(|binary| !command_exists(binary)) {
        return Err(AppError::runtime(format!(
            "native CUA command `{}` is not available in PATH",
            command.first().map(String::as_str).unwrap_or("<empty>")
        )));
    }
    Ok(())
}

fn detect_platform() -> NativeCuaPlatform {
    match env::consts::OS {
        "macos" => NativeCuaPlatform::Macos,
        "linux" => NativeCuaPlatform::Linux,
        "windows" => NativeCuaPlatform::Windows,
        _ => NativeCuaPlatform::Unsupported,
    }
}

fn command_exists(binary: &str) -> bool {
    if binary.contains(std::path::MAIN_SEPARATOR) {
        return PathBuf::from(binary).is_file();
    }
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };

    #[cfg(windows)]
    let extensions = env::var_os("PATHEXT")
        .map(|value| {
            value
                .to_string_lossy()
                .split(';')
                .map(|entry| entry.trim().to_ascii_lowercase())
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|| vec![".exe".to_string(), ".cmd".to_string(), ".bat".to_string()]);

    for path in env::split_paths(&paths) {
        let candidate = path.join(binary);
        if candidate.is_file() {
            return true;
        }
        #[cfg(windows)]
        {
            for extension in &extensions {
                if path.join(format!("{}{}", binary, extension)).is_file() {
                    return true;
                }
            }
        }
    }
    false
}

fn linux_screenshot_tool() -> Option<String> {
    ["gnome-screenshot", "scrot", "import", "spectacle"]
        .into_iter()
        .find(|tool| command_exists(tool))
        .map(str::to_string)
}

fn next_screenshot_path(session_id: &str) -> Result<PathBuf, AppError> {
    let safe_session_id = sanitize_path_fragment(session_id);
    let dir = env::temp_dir().join("hermes-native-cua");
    fs::create_dir_all(&dir).map_err(AppError::from_io_error)?;
    Ok(dir.join(format!(
        "{}-{}.png",
        safe_session_id,
        Utc::now().timestamp_millis()
    )))
}

fn sanitize_path_fragment(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn validate_bounded_text(field: &str, value: String, max_chars: usize) -> Result<String, AppError> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        return Err(AppError::validation(format!(
            "native CUA `{}` is required",
            field
        )));
    }
    if trimmed.chars().count() > max_chars {
        return Err(AppError::validation(format!(
            "native CUA `{}` exceeds {} characters",
            field, max_chars
        )));
    }
    Ok(trimmed)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn normalize_coordinate(field: &str, value: Option<f64>) -> Result<Option<f64>, AppError> {
    value
        .map(|value| normalize_coordinate_value(field, value))
        .transpose()
}

fn normalize_coordinate_value(field: &str, value: f64) -> Result<f64, AppError> {
    if !value.is_finite() || !(0.0..=1000.0).contains(&value) {
        return Err(AppError::validation(format!(
            "native CUA `{}` coordinate must be normalized between 0 and 1 or use TuriX 0-1000 scale",
            field
        )));
    }
    if value > 1.0 {
        Ok(value / 1000.0)
    } else {
        Ok(value)
    }
}

fn required_field<'a>(
    field: &str,
    value: Option<&'a str>,
    action: &str,
) -> Result<&'a str, AppError> {
    value.ok_or_else(|| {
        AppError::validation(format!(
            "native CUA action `{}` requires `{}`",
            action, field
        ))
    })
}

fn scaled_coordinates(
    request: &NativeCuaActionRequest,
    screen_size: Option<ScreenSize>,
) -> Result<(i32, i32), AppError> {
    let x = request
        .x
        .ok_or_else(|| AppError::validation("native CUA action requires normalized `x`"))?;
    let y = request
        .y
        .ok_or_else(|| AppError::validation("native CUA action requires normalized `y`"))?;
    let screen_size = screen_size.unwrap_or(ScreenSize {
        width: 10_000,
        height: 10_000,
    });
    Ok((
        (x * f64::from(screen_size.width)).round() as i32,
        (y * f64::from(screen_size.height)).round() as i32,
    ))
}

fn normalized_to_screen_units(field: &str, value: Option<f64>) -> Result<i32, AppError> {
    let value = value.ok_or_else(|| {
        AppError::validation(format!("native CUA action requires normalized `{}`", field))
    })?;
    Ok((value * 10_000.0).round() as i32)
}

fn linux_hotkey_combo(modifiers: &[String], key: &str) -> String {
    let mut parts = modifiers
        .iter()
        .map(|modifier| match modifier.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "super" | "win" | "windows" => "super".to_string(),
            "control" | "ctrl" => "ctrl".to_string(),
            "option" | "alt" => "alt".to_string(),
            "shift" => "shift".to_string(),
            other => other.to_string(),
        })
        .collect::<Vec<_>>();
    parts.push(key.to_string());
    parts.join("+")
}

fn macos_modifier_list(modifiers: &[String]) -> String {
    let mut parts = modifiers
        .iter()
        .map(|modifier| match modifier.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "super" | "win" | "windows" => "command down".to_string(),
            "control" | "ctrl" => "control down".to_string(),
            "option" | "alt" => "option down".to_string(),
            "shift" => "shift down".to_string(),
            other => format!("{} down", other),
        })
        .collect::<Vec<_>>();
    if parts.is_empty() {
        parts.push("command down".to_string());
    }
    parts.join(", ")
}

fn windows_hotkey_combo(modifiers: &[String], key: &str) -> String {
    let mut combo = String::new();
    for modifier in modifiers {
        match modifier.to_ascii_lowercase().as_str() {
            "control" | "ctrl" => combo.push('^'),
            "shift" => combo.push('+'),
            "option" | "alt" => combo.push('%'),
            "cmd" | "command" | "super" | "win" | "windows" => combo.push('^'),
            _ => {}
        }
    }
    combo.push_str(key);
    combo
}

fn applescript_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn powershell_string(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

fn windows_powershell_command() -> String {
    if command_exists("powershell") {
        "powershell".to_string()
    } else {
        "powershell.exe".to_string()
    }
}

fn windows_pointer_script(
    action_type: &str,
    x: i32,
    y: i32,
    dx: Option<f64>,
    dy: Option<f64>,
) -> String {
    let click_flags = match action_type {
        "right_click" => "8,16",
        _ => "2,4",
    };
    let click_count = if action_type == "double_click" { 2 } else { 1 };
    let drag_dx = dx.unwrap_or(0.0).round() as i32;
    let drag_dy = dy.unwrap_or(0.0).round() as i32;
    let click_script = if action_type == "move_mouse" {
        String::new()
    } else if action_type == "drag_mouse" {
        format!(
            "[Hermes.NativeMouse]::mouse_event(2,0,0,0,0); [System.Windows.Forms.Cursor]::Position=New-Object System.Drawing.Point({},{}); [Hermes.NativeMouse]::mouse_event(4,0,0,0,0);",
            x.saturating_add(drag_dx),
            y.saturating_add(drag_dy)
        )
    } else {
        (0..click_count)
            .map(|_| {
                let mut parts = click_flags.split(',');
                format!(
                    "[Hermes.NativeMouse]::mouse_event({},0,0,0,0); [Hermes.NativeMouse]::mouse_event({},0,0,0,0);",
                    parts.next().unwrap_or("2"),
                    parts.next().unwrap_or("4")
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    format!(
        "Add-Type -AssemblyName System.Windows.Forms; Add-Type -AssemblyName System.Drawing; $signature='[DllImport(\"user32.dll\")] public static extern void mouse_event(int dwFlags, int dx, int dy, int cButtons, int dwExtraInfo);'; Add-Type -MemberDefinition $signature -Name NativeMouse -Namespace Hermes; [System.Windows.Forms.Cursor]::Position=New-Object System.Drawing.Point({},{}); {}",
        x, y, click_script
    )
}

fn normalize_filter(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}

fn filter_matches(value: &str, filter: Option<&str>) -> bool {
    filter.is_none_or(|filter| value.eq_ignore_ascii_case(filter))
}

fn optional_filter_matches(value: Option<&str>, filter: Option<&str>) -> bool {
    filter.is_none_or(|filter| value.is_some_and(|value| value.eq_ignore_ascii_case(filter)))
}

fn normalize_list_limit(limit: Option<usize>) -> Option<usize> {
    match limit {
        Some(0) | None => None,
        Some(limit) => Some(limit.min(NATIVE_CUA_AUDIT_LOG_LIMIT)),
    }
}

fn normalize_export_limit(limit: Option<usize>) -> usize {
    match limit {
        Some(0) | None => NATIVE_CUA_AUDIT_EXPORT_DEFAULT_LIMIT,
        Some(limit) => limit.min(NATIVE_CUA_AUDIT_LOG_LIMIT),
    }
}

fn normalize_export_format(format: Option<String>) -> Result<String, AppError> {
    let normalized = format
        .unwrap_or_else(|| "json".to_string())
        .trim()
        .to_ascii_lowercase();
    if normalized == "json" || normalized == "jsonl" {
        return Ok(normalized);
    }
    Err(AppError::validation(format!(
        "unsupported native CUA audit export format `{}`",
        normalized
    )))
}

fn serialize_audit_events(
    events: &[NativeCuaAuditEvent],
    format: &str,
) -> Result<String, AppError> {
    match format {
        "json" => serde_json::to_string_pretty(events).map_err(AppError::from_json_error),
        "jsonl" => events
            .iter()
            .map(|event| serde_json::to_string(event).map_err(AppError::from_json_error))
            .collect::<Result<Vec<_>, _>>()
            .map(|lines| lines.join("\n")),
        _ => Err(AppError::validation(format!(
            "unsupported native CUA audit export format `{}`",
            format
        ))),
    }
}

fn truncate_summary(value: String) -> String {
    let char_count = value.chars().count();
    if char_count <= NATIVE_CUA_SUMMARY_MAX_CHARS {
        return value;
    }

    let mut truncated = value
        .chars()
        .take(NATIVE_CUA_SUMMARY_MAX_CHARS.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::{
        NativeCuaActionRequest, NativeCuaApplyModelOutputRequest, NativeCuaAuditExportRequest,
        NativeCuaAuditListRequest, NativeCuaExecutionOutcome, NativeCuaHistoryListRequest,
        NativeCuaInvokeModelRequest, NativeCuaModelTurnRequest, NativeCuaObserveRequest,
        NativeCuaPlanTaskRequest, NativeCuaPlatform, NativeCuaPreviewModelRouteRequest,
        NativeCuaRunStepRequest, NativeCuaSkillMetadata, NativeCuaStartSessionRequest,
        NativeCuaTrajectoryExportRequest, ScreenSize, load_sessions,
        native_cua_apply_model_output_for_db_with_executor,
        native_cua_execute_action_for_db_with_executor, native_cua_export_audit_for_db,
        native_cua_export_trajectory_for_db,
        native_cua_invoke_model_for_db_with_sender_and_executor, native_cua_list_audit_for_db,
        native_cua_list_history_for_db, native_cua_observe_for_db_with_executor,
        native_cua_plan_task_for_db, native_cua_prepare_model_turn_for_db,
        native_cua_preview_model_route_for_db, native_cua_run_step_for_db_with_executor,
        native_cua_start_session_for_db, normalize_action_request, plan_action_command,
    };
    use crate::backend::{AppError, Database};
    use serde_json::{Value, json};
    use std::env;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvVarOverride {
        key: &'static str,
        original: Option<OsString>,
    }

    impl EnvVarOverride {
        fn set(key: &'static str, value: &str) -> Self {
            let original = env::var_os(key);
            unsafe {
                env::set_var(key, value);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarOverride {
        fn drop(&mut self) {
            match self.original.as_ref() {
                Some(value) => unsafe { env::set_var(self.key, value) },
                None => unsafe { env::remove_var(self.key) },
            }
        }
    }

    fn insert_runtime_settings(db: &Database, value: Value) {
        let value_json = serde_json::to_string(&value).expect("runtime settings should serialize");
        db.execute(
            "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
            &[&"runtime", &value_json, &"2026-04-28T00:00:00Z"],
        )
        .expect("runtime settings should insert");
    }

    #[test]
    fn native_cua_preview_model_route_reports_auto_selection_without_starting_session() {
        let db = Database::in_memory().expect("database should initialize");
        insert_runtime_settings(
            &db,
            json!({
                "provider": "openai",
                "model": "gpt-4o",
                "base_url": "https://api.openai.com/v1",
                "api_key_ref": null,
                "engine_profile": "default",
                "agent_engine_enabled": true,
                "busy_input_mode": "interrupt",
                "native_cua_auto_models": {
                    "hard": {
                        "provider": "openrouter",
                        "model": "anthropic/claude-opus-4",
                        "base_url": "https://openrouter.ai/api/v1",
                        "api_key_ref": "OPENROUTER_API_KEY"
                    }
                }
            }),
        );

        let preview = native_cua_preview_model_route_for_db(
            &db,
            NativeCuaPreviewModelRouteRequest {
                task: "Analyze a complex multi-step browser workflow, compare options, debug failures, and implement every required desktop change.".to_string(),
                model_mode: Some("auto".to_string()),
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("route preview should resolve auto settings");

        assert_eq!(preview.model_mode, "auto");
        assert_eq!(preview.model_difficulty.as_deref(), Some("hard"));
        assert_eq!(preview.provider.as_deref(), Some("openrouter"));
        assert_eq!(preview.model.as_deref(), Some("anthropic/claude-opus-4"));
        assert_eq!(preview.api_key_ref.as_deref(), Some("OPENROUTER_API_KEY"));
        assert!(preview.summary.contains("Auto selected"));
        assert!(load_sessions(&db).expect("sessions should load").is_empty());
    }

    #[test]
    fn native_cua_preview_model_route_reports_custom_selection_without_starting_session() {
        let db = Database::in_memory().expect("database should initialize");

        let preview = native_cua_preview_model_route_for_db(
            &db,
            NativeCuaPreviewModelRouteRequest {
                task: "Click the settings button.".to_string(),
                model_mode: Some("custom".to_string()),
                provider: Some("ollama".to_string()),
                model: Some("qwen2.5-coder".to_string()),
                base_url: Some("http://localhost:11434".to_string()),
                api_key_ref: None,
            },
        )
        .expect("route preview should resolve custom settings");

        assert_eq!(preview.model_mode, "custom");
        assert_eq!(preview.model_difficulty, None);
        assert_eq!(preview.provider.as_deref(), Some("ollama"));
        assert_eq!(preview.model.as_deref(), Some("qwen2.5-coder"));
        assert!(preview.summary.contains("Custom selected"));
        assert!(load_sessions(&db).expect("sessions should load").is_empty());
    }

    #[test]
    fn native_cua_dry_run_action_does_not_execute_and_records_audit() {
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Open a local app safely".to_string(),
                session_id: None,
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");
        let response = native_cua_execute_action_for_db_with_executor(
            &db,
            NativeCuaActionRequest {
                session_id: Some(session.session_id.clone()),
                action_type: "type_text".to_string(),
                text: Some("hello".to_string()),
                key: None,
                modifiers: None,
                app: None,
                x: None,
                y: None,
                dx: None,
                dy: None,
                dry_run: Some(true),
                confirmation_phrase: None,
            },
            |_| panic!("dry-run native CUA step must not execute OS command"),
        )
        .expect("dry-run action should succeed");

        assert!(!response.executed);
        assert!(response.dry_run);

        let audit = native_cua_list_audit_for_db(
            &db,
            NativeCuaAuditListRequest {
                limit: None,
                session_id: Some(session.session_id),
                event_type: Some("action".to_string()),
                status: Some("dry_run".to_string()),
            },
        )
        .expect("dry-run audit should persist");
        assert_eq!(audit.len(), 1);
        assert_eq!(audit[0].action_type.as_deref(), Some("type_text"));
    }

    #[test]
    fn native_cua_non_dry_run_requires_exact_confirmation_phrase() {
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Click guarded target".to_string(),
                session_id: None,
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");
        let error = native_cua_execute_action_for_db_with_executor(
            &db,
            NativeCuaActionRequest {
                session_id: Some(session.session_id),
                action_type: "click".to_string(),
                text: None,
                key: None,
                modifiers: None,
                app: None,
                x: Some(0.5),
                y: Some(0.5),
                dx: None,
                dy: None,
                dry_run: Some(false),
                confirmation_phrase: Some("RUN DESKTOP ACTION".to_string()),
            },
            |_| panic!("dry-run native CUA step must not execute OS command"),
        )
        .expect_err("wrong phrase should fail");

        assert_eq!(error.code, "validation_error");
        assert!(error.message.contains("RUN NATIVE CUA ACTION"));
    }

    #[test]
    fn native_cua_session_resume_round_trips_through_app_settings() {
        let db = Database::in_memory().expect("database should initialize");
        let first = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Initial task".to_string(),
                session_id: Some("manual-session".to_string()),
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");
        let second = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Updated task".to_string(),
                session_id: Some(first.session_id.clone()),
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should resume");

        assert_eq!(first.session_id, second.session_id);
        assert!(!first.resumed);
        assert!(second.resumed);
        assert_eq!(second.task.as_deref(), Some("Updated task"));

        let stored = db
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = ?1",
                &[&"native_cua.sessions"],
                |row| row.get::<_, String>(0),
            )
            .expect("sessions should be stored");
        let stored_json: Value = serde_json::from_str(&stored).expect("session JSON should parse");
        assert_eq!(stored_json.as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn native_cua_linux_action_command_planning_uses_normalized_coordinates() {
        let request = normalize_action_request(NativeCuaActionRequest {
            session_id: None,
            action_type: "click".to_string(),
            text: None,
            key: None,
            modifiers: None,
            app: None,
            x: Some(0.25),
            y: Some(0.5),
            dx: None,
            dy: None,
            dry_run: Some(true),
            confirmation_phrase: None,
        })
        .expect("request should normalize");
        let plan = plan_action_command(
            &request,
            NativeCuaPlatform::Linux,
            Some(ScreenSize {
                width: 1920,
                height: 1080,
            }),
        )
        .expect("plan should build");

        assert_eq!(plan.command.first().map(String::as_str), Some("xdotool"));
        assert!(plan.command.iter().any(|part| part == "480"));
        assert!(plan.command.iter().any(|part| part == "540"));
        assert!(plan.command.iter().any(|part| part == "click"));
    }

    #[test]
    fn native_cua_audit_export_jsonl_contains_action_event() {
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Export audit".to_string(),
                session_id: None,
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");

        native_cua_observe_for_db_with_executor(
            &db,
            NativeCuaObserveRequest {
                session_id: Some(session.session_id),
                dry_run: Some(true),
                capture_screenshot: Some(false),
            },
            |_| {
                Ok(NativeCuaExecutionOutcome {
                    stdout: String::new(),
                    stderr: String::new(),
                    exit_code: Some(0),
                })
            },
        )
        .expect("observe dry-run should succeed");

        let export = native_cua_export_audit_for_db(
            &db,
            NativeCuaAuditExportRequest {
                limit: Some(10),
                session_id: None,
                event_type: None,
                status: None,
                format: Some("jsonl".to_string()),
            },
        )
        .expect("audit export should succeed");

        assert!(export.exported_count >= 2);
        assert!(export.payload.contains("\"event_type\":\"observe\""));
        assert!(export.payload.contains("\"event_type\":\"session\""));
    }

    #[test]
    fn native_cua_plan_task_persists_skills_and_exports_trajectory() {
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Use browser skill; inspect page; record result".to_string(),
                session_id: Some("loop-session".to_string()),
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");

        let plan = native_cua_plan_task_for_db(
            &db,
            NativeCuaPlanTaskRequest {
                session_id: Some(session.session_id.clone()),
                task: None,
                skill_catalog: Some(vec![NativeCuaSkillMetadata {
                    name: "browser".to_string(),
                    description: "browser automation and page inspection".to_string(),
                }]),
                max_steps: Some(6),
            },
        )
        .expect("plan should persist");

        assert_eq!(plan.session_id, session.session_id);
        assert!(plan.steps.len() >= 3);
        assert_eq!(plan.selected_skills, vec!["browser".to_string()]);

        let export = native_cua_export_trajectory_for_db(
            &db,
            NativeCuaTrajectoryExportRequest {
                session_id: Some(session.session_id),
                format: Some("jsonl".to_string()),
                include_audit: Some(false),
            },
        )
        .expect("trajectory export should succeed");
        assert!(export.payload.contains("native_cua_plan"));
        assert_eq!(export.exported_count, 1);
    }

    #[test]
    fn native_cua_run_step_translates_turix_actions_records_memory_and_history() {
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Type and record result".to_string(),
                session_id: None,
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");
        let response = native_cua_run_step_for_db_with_executor(
            &db,
            NativeCuaRunStepRequest {
                session_id: Some(session.session_id.clone()),
                dry_run: Some(true),
                capture_screenshot: Some(false),
                brain_state: None,
                actions: Some(vec![
                    json!({"input_text": {"text": "hello"}}),
                    json!({"record_info": {"text": "observed hello", "file_name": "note"}}),
                    json!({"done": {"text": "finished"}}),
                ]),
                max_actions: Some(5),
                confirmation_phrase: None,
            },
            |_| panic!("dry-run native CUA step must not execute OS command"),
        )
        .expect("run step should succeed");

        assert!(response.done);
        assert_eq!(response.step.actions.len(), 3);
        assert_eq!(response.step.actions[0].action_name, "input_text");
        assert_eq!(response.step.actions[1].status, "recorded");
        assert_eq!(response.step.actions[2].status, "done");
        assert_eq!(
            response.step.actions[0]
                .native_result
                .as_ref()
                .map(|result| result.executed),
            Some(false)
        );

        let history = native_cua_list_history_for_db(
            &db,
            NativeCuaHistoryListRequest {
                session_id: Some(session.session_id.clone()),
                limit: Some(10),
                status: None,
            },
        )
        .expect("history should list");
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].session_id, session.session_id);

        let trajectory = native_cua_export_trajectory_for_db(
            &db,
            NativeCuaTrajectoryExportRequest {
                session_id: Some(session.session_id),
                format: Some("jsonl".to_string()),
                include_audit: Some(true),
            },
        )
        .expect("trajectory export should include step and memory");
        assert!(trajectory.payload.contains("native_cua_step"));
        assert!(trajectory.payload.contains("native_cua_memory"));
    }

    #[test]
    fn native_cua_run_step_accepts_turix_thousandth_scale_coordinates() {
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Click a TuriX actor target".to_string(),
                session_id: Some("turix-thousandths".to_string()),
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");

        let response = native_cua_run_step_for_db_with_executor(
            &db,
            NativeCuaRunStepRequest {
                session_id: Some(session.session_id),
                dry_run: Some(true),
                capture_screenshot: Some(false),
                brain_state: None,
                actions: Some(vec![
                    json!({"Click": {"position": [250, 500]}}),
                    json!({"Drag": {"position1": [100, 200], "position2": [300, 400]}}),
                    json!({"scroll_up": {"position": [500, 750], "dy": 25}}),
                ]),
                max_actions: Some(5),
                confirmation_phrase: None,
            },
            |_| panic!("dry-run native CUA step must not execute OS command"),
        )
        .expect("TuriX 0-1000 coordinates should normalize into native coordinates");

        assert_eq!(response.step.actions.len(), 3);
        let click = response.step.actions[0]
            .native_result
            .as_ref()
            .expect("click should translate to native result");
        assert_eq!(
            click.planned_command,
            vec![
                "xdotool".to_string(),
                "mousemove".to_string(),
                "2500".to_string(),
                "5000".to_string(),
                "click".to_string(),
                "1".to_string(),
            ]
        );
        let drag = response.step.actions[1]
            .native_result
            .as_ref()
            .expect("drag should translate to native result");
        assert!(drag.planned_command.contains(&"2000".to_string()));
        let scroll = response.step.actions[2]
            .native_result
            .as_ref()
            .expect("scroll should translate to native result");
        assert_eq!(scroll.status, "dry_run");
    }

    #[test]
    fn native_cua_prepare_actor_model_turn_includes_action_schema_and_context() {
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Open browser and search safely".to_string(),
                session_id: Some("model-turn-session".to_string()),
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");
        native_cua_plan_task_for_db(
            &db,
            NativeCuaPlanTaskRequest {
                session_id: Some(session.session_id.clone()),
                task: None,
                skill_catalog: Some(vec![NativeCuaSkillMetadata {
                    name: "browser".to_string(),
                    description: "Search and inspect web pages".to_string(),
                }]),
                max_steps: Some(4),
            },
        )
        .expect("plan should persist");

        let turn = native_cua_prepare_model_turn_for_db(
            &db,
            NativeCuaModelTurnRequest {
                session_id: Some(session.session_id.clone()),
                role: "actor".to_string(),
                include_screenshot_data_url: Some(false),
                max_history: Some(5),
                extra_context: Some("Prefer dry-run-safe actions.".to_string()),
            },
        )
        .expect("actor turn should prepare");

        assert_eq!(turn.session_id, session.session_id);
        assert_eq!(turn.role, "actor");
        assert!(turn.response_schema.get("action").is_some());
        assert!(turn.action_catalog.iter().any(|action| action == "Click"));
        assert!(
            turn.messages
                .iter()
                .any(|message| message.content.contains("Prefer dry-run-safe actions."))
        );
        assert!(turn.summary.contains("actor"));
    }

    #[test]
    fn native_cua_apply_actor_model_output_runs_step_and_records_model_turn() {
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Type hello".to_string(),
                session_id: None,
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");

        let response = native_cua_apply_model_output_for_db_with_executor(
            &db,
            NativeCuaApplyModelOutputRequest {
                session_id: Some(session.session_id.clone()),
                role: "actor".to_string(),
                output: json!({"action": [{"input_text": {"text": "hello"}}, {"done": {"text": "done"}}]}),
                dry_run: Some(true),
                capture_screenshot: Some(false),
                confirmation_phrase: None,
            },
            |_| panic!("dry-run model output must not execute OS command"),
        )
        .expect("actor output should apply");

        assert_eq!(response.session_id, session.session_id);
        assert_eq!(response.role, "actor");
        assert_eq!(response.status, "applied");
        assert!(response.step_result.as_ref().is_some_and(|step| step.done));
        assert!(response.summary.contains("actor"));
    }

    #[test]
    fn native_cua_invoke_model_dry_run_builds_preview_without_calling_sender() {
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Inspect a page and stay safe".to_string(),
                session_id: Some("invoke-dry-run".to_string()),
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");
        let mut sender_calls = 0;

        let response = native_cua_invoke_model_for_db_with_sender_and_executor(
            &db,
            NativeCuaInvokeModelRequest {
                session_id: Some(session.session_id.clone()),
                role: "actor".to_string(),
                provider: Some("openai".to_string()),
                model: Some("gpt-4o".to_string()),
                base_url: Some("https://api.openai.com".to_string()),
                api_key_ref: None,
                dry_run: Some(true),
                apply_output: Some(false),
                capture_screenshot: Some(false),
                extra_context: Some("Prefer wait over risky actions.".to_string()),
                model_confirmation_phrase: None,
                action_confirmation_phrase: None,
            },
            |_| {
                sender_calls += 1;
                Err(AppError::runtime(
                    "dry-run native CUA invoke must not call sender",
                ))
            },
            |_| panic!("dry-run invoke should not execute desktop actions"),
        )
        .expect("dry-run invoke should succeed");

        assert_eq!(sender_calls, 0);
        assert!(response.dry_run);
        assert!(!response.requested);
        assert_eq!(response.status, "dry_run");
        assert_eq!(response.provider, "openai");
        assert_eq!(response.model, "gpt-4o");
        assert_eq!(response.prompt_turn.role, "actor");
        assert_eq!(
            response.http_request_preview["method"].as_str(),
            Some("POST")
        );
        assert_eq!(
            response.http_request_preview["url"].as_str(),
            Some("https://api.openai.com/v1/chat/completions")
        );
        assert_eq!(
            response.http_request_preview["body"]["messages"][0]["role"].as_str(),
            Some("system")
        );
        assert!(
            response.http_request_preview["body"]["response_format"]["json_schema"]["schema"]
                .get("action")
                .is_some()
        );
    }

    #[test]
    fn native_cua_auto_mode_routes_hard_tasks_to_configured_deep_model() {
        let db = Database::in_memory().expect("database should initialize");
        insert_runtime_settings(
            &db,
            json!({
                "provider": "openai",
                "model": "gpt-4o",
                "base_url": "https://api.openai.com/v1",
                "api_key_ref": null,
                "engine_profile": "default",
                "agent_engine_enabled": true,
                "busy_input_mode": "interrupt",
                "native_cua_auto_models": {
                    "easy": {
                        "provider": "openai",
                        "model": "gpt-4o-mini",
                        "base_url": "https://api.openai.com/v1"
                    },
                    "standard": {
                        "provider": "openai",
                        "model": "gpt-4o",
                        "base_url": "https://api.openai.com/v1"
                    },
                    "hard": {
                        "provider": "openrouter",
                        "model": "anthropic/claude-opus-4",
                        "base_url": "https://openrouter.ai/api/v1",
                        "api_key_ref": "OPENROUTER_API_KEY"
                    }
                }
            }),
        );
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Analyze a complex multi-step browser and spreadsheet workflow, compare options, integrate findings, and implement every required desktop change.".to_string(),
                session_id: Some("auto-hard-session".to_string()),
                model_mode: Some("auto".to_string()),
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start with auto model routing");

        assert_eq!(session.model_mode.as_deref(), Some("auto"));
        assert_eq!(session.model_difficulty.as_deref(), Some("hard"));
        assert_eq!(session.provider.as_deref(), Some("openrouter"));
        assert_eq!(session.model.as_deref(), Some("anthropic/claude-opus-4"));
        assert!(
            session
                .model_selection_reason
                .as_deref()
                .is_some_and(|reason| reason.contains("hard"))
        );

        let response = native_cua_invoke_model_for_db_with_sender_and_executor(
            &db,
            NativeCuaInvokeModelRequest {
                session_id: Some(session.session_id),
                role: "actor".to_string(),
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
                dry_run: Some(true),
                apply_output: Some(false),
                capture_screenshot: Some(false),
                extra_context: None,
                model_confirmation_phrase: None,
                action_confirmation_phrase: None,
            },
            |_| {
                Err(AppError::runtime(
                    "dry-run native CUA invoke must not call sender",
                ))
            },
            |_| panic!("dry-run invoke should not execute desktop actions"),
        )
        .expect("dry-run invoke should use auto-selected deep model");

        assert_eq!(response.provider, "openrouter");
        assert_eq!(response.model, "anthropic/claude-opus-4");
        assert_eq!(
            response.http_request_preview["url"].as_str(),
            Some("https://openrouter.ai/api/v1/chat/completions")
        );
    }

    #[test]
    fn native_cua_invoke_model_uses_custom_session_model_over_desktop_auto_default() {
        let db = Database::in_memory().expect("database should initialize");
        insert_runtime_settings(
            &db,
            json!({
                "provider": "openai",
                "model": "gpt-4o",
                "base_url": "https://api.openai.com/v1",
                "api_key_ref": null,
                "engine_profile": "default",
                "agent_engine_enabled": true,
                "busy_input_mode": "interrupt"
            }),
        );
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Use custom model for this task".to_string(),
                session_id: Some("custom-session-model".to_string()),
                model_mode: Some("custom".to_string()),
                provider: Some("ollama".to_string()),
                model: Some("qwen2.5-coder".to_string()),
                base_url: Some("http://localhost:11434".to_string()),
                api_key_ref: None,
            },
        )
        .expect("session should start with custom model config");

        assert_eq!(session.model_mode.as_deref(), Some("custom"));
        assert_eq!(session.provider.as_deref(), Some("ollama"));
        assert_eq!(session.model.as_deref(), Some("qwen2.5-coder"));

        let response = native_cua_invoke_model_for_db_with_sender_and_executor(
            &db,
            NativeCuaInvokeModelRequest {
                session_id: Some(session.session_id),
                role: "actor".to_string(),
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
                dry_run: Some(true),
                apply_output: Some(false),
                capture_screenshot: Some(false),
                extra_context: None,
                model_confirmation_phrase: None,
                action_confirmation_phrase: None,
            },
            |_| {
                Err(AppError::runtime(
                    "dry-run native CUA invoke must not call sender",
                ))
            },
            |_| panic!("dry-run invoke should not execute desktop actions"),
        )
        .expect("dry-run invoke should use custom session model settings");

        assert_eq!(response.provider, "ollama");
        assert_eq!(response.model, "qwen2.5-coder");
        assert_eq!(
            response.http_request_preview["url"].as_str(),
            Some("http://localhost:11434/api/chat")
        );
    }

    #[test]
    fn native_cua_invoke_model_uses_saved_desktop_runtime_settings_without_duplicate_api_version() {
        let db = Database::in_memory().expect("database should initialize");
        insert_runtime_settings(
            &db,
            json!({
                "provider": "openrouter",
                "model": "anthropic/claude-sonnet-4",
                "base_url": "https://openrouter.ai/api/v1",
                "api_key_ref": null,
                "engine_profile": "default",
                "agent_engine_enabled": true,
                "busy_input_mode": "interrupt"
            }),
        );
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Use saved desktop model settings".to_string(),
                session_id: Some("saved-runtime-model".to_string()),
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");
        let mut sender_calls = 0;

        let response = native_cua_invoke_model_for_db_with_sender_and_executor(
            &db,
            NativeCuaInvokeModelRequest {
                session_id: Some(session.session_id),
                role: "actor".to_string(),
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
                dry_run: Some(true),
                apply_output: Some(false),
                capture_screenshot: Some(false),
                extra_context: None,
                model_confirmation_phrase: None,
                action_confirmation_phrase: None,
            },
            |_| {
                sender_calls += 1;
                Err(AppError::runtime(
                    "dry-run native CUA invoke must not call sender",
                ))
            },
            |_| panic!("dry-run invoke should not execute desktop actions"),
        )
        .expect("dry-run invoke should use saved runtime settings");

        assert_eq!(sender_calls, 0);
        assert_eq!(response.provider, "openrouter");
        assert_eq!(response.model, "anthropic/claude-sonnet-4");
        assert_eq!(
            response.http_request_preview["url"].as_str(),
            Some("https://openrouter.ai/api/v1/chat/completions")
        );
    }

    #[test]
    fn native_cua_invoke_model_live_call_requires_exact_confirmation_phrase() {
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Call actor model".to_string(),
                session_id: None,
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");
        let mut sender_calls = 0;

        let error = native_cua_invoke_model_for_db_with_sender_and_executor(
            &db,
            NativeCuaInvokeModelRequest {
                session_id: Some(session.session_id),
                role: "actor".to_string(),
                provider: Some("openai".to_string()),
                model: Some("gpt-4o".to_string()),
                base_url: Some("https://api.openai.com".to_string()),
                api_key_ref: None,
                dry_run: Some(false),
                apply_output: Some(false),
                capture_screenshot: Some(false),
                extra_context: None,
                model_confirmation_phrase: Some("RUN MODEL".to_string()),
                action_confirmation_phrase: None,
            },
            |_| {
                sender_calls += 1;
                Err(AppError::runtime(
                    "sender should not run when phrase is wrong",
                ))
            },
            |_| panic!("rejected invoke should not execute desktop actions"),
        )
        .expect_err("wrong model confirmation phrase should fail");

        assert_eq!(sender_calls, 0);
        assert_eq!(error.code, "validation_error");
        assert!(error.message.contains("INVOKE NATIVE CUA MODEL"));
    }

    #[test]
    fn native_cua_invoke_model_live_openai_response_parses_and_applies_actor_output() {
        let _guard = env_lock().lock().expect("lock poisoned");
        let _api_key = EnvVarOverride::set("HERMES_NATIVE_CUA_TEST_OPENAI_KEY", "test-key");
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Finish with a done action".to_string(),
                session_id: Some("invoke-live-openai".to_string()),
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");
        let mut sender_calls = 0;

        let response = native_cua_invoke_model_for_db_with_sender_and_executor(
            &db,
            NativeCuaInvokeModelRequest {
                session_id: Some(session.session_id.clone()),
                role: "actor".to_string(),
                provider: Some("openai".to_string()),
                model: Some("gpt-4o".to_string()),
                base_url: Some("https://api.openai.com".to_string()),
                api_key_ref: Some("HERMES_NATIVE_CUA_TEST_OPENAI_KEY".to_string()),
                dry_run: Some(false),
                apply_output: Some(true),
                capture_screenshot: Some(false),
                extra_context: None,
                model_confirmation_phrase: Some("INVOKE NATIVE CUA MODEL".to_string()),
                action_confirmation_phrase: None,
            },
            |_| {
                sender_calls += 1;
                Ok(json!({
                    "choices": [{
                        "message": {
                            "content": "{\"action\":[{\"done\":{\"text\":\"Completed by model.\"}}]}"
                        }
                    }]
                }))
            },
            |_| panic!("done-only actor output should not execute desktop actions"),
        )
        .expect("live invoke should succeed");

        assert_eq!(sender_calls, 1);
        assert!(!response.dry_run);
        assert!(response.requested);
        assert_eq!(response.status, "applied");
        assert_eq!(response.provider, "openai");
        assert_eq!(response.model, "gpt-4o");
        assert_eq!(
            response
                .parsed_output
                .as_ref()
                .and_then(|value| value.get("action")),
            Some(&json!([{"done": {"text": "Completed by model."}}]))
        );
        assert!(
            response
                .apply_result
                .as_ref()
                .and_then(|result| result.step_result.as_ref())
                .is_some_and(|step| step.done)
        );
    }

    #[test]
    fn native_cua_invoke_model_ollama_does_not_require_api_key() {
        let db = Database::in_memory().expect("database should initialize");
        let session = native_cua_start_session_for_db(
            &db,
            NativeCuaStartSessionRequest {
                task: "Use local ollama".to_string(),
                session_id: Some("invoke-ollama".to_string()),
                model_mode: None,
                provider: None,
                model: None,
                base_url: None,
                api_key_ref: None,
            },
        )
        .expect("session should start");
        let mut sender_calls = 0;
        let mut saw_auth_header = false;

        let response = native_cua_invoke_model_for_db_with_sender_and_executor(
            &db,
            NativeCuaInvokeModelRequest {
                session_id: Some(session.session_id),
                role: "actor".to_string(),
                provider: Some("ollama".to_string()),
                model: Some("qwen2.5-coder".to_string()),
                base_url: Some("http://localhost:11434".to_string()),
                api_key_ref: None,
                dry_run: Some(false),
                apply_output: Some(false),
                capture_screenshot: Some(false),
                extra_context: None,
                model_confirmation_phrase: Some("INVOKE NATIVE CUA MODEL".to_string()),
                action_confirmation_phrase: None,
            },
            |outbound| {
                sender_calls += 1;
                saw_auth_header = outbound
                    .headers
                    .iter()
                    .any(|(name, _)| name.eq_ignore_ascii_case("authorization"));
                assert_eq!(outbound.url, "http://localhost:11434/api/chat");
                Ok(json!({
                    "message": {
                        "content": "{\"action\":[{\"done\":{\"text\":\"Ollama done.\"}}]}"
                    }
                }))
            },
            |_| panic!("apply_output=false should not execute desktop actions"),
        )
        .expect("ollama invoke should succeed");

        assert_eq!(sender_calls, 1);
        assert!(!saw_auth_header);
        assert_eq!(response.provider, "ollama");
        assert_eq!(response.model, "qwen2.5-coder");
        assert!(response.raw_output.is_some());
        assert!(response.apply_result.is_none());
    }
}
