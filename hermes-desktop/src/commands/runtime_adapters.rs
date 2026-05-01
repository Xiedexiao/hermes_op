use crate::backend::{AppError, AppResult, Database};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use tauri::State;
use uuid::Uuid;

const DEFAULT_TIMEOUT_MS: u64 = 5_000;
const MIN_TIMEOUT_MS: u64 = 50;
const MAX_TIMEOUT_MS: u64 = 30_000;
const MAX_CAPTURE_BYTES: usize = 16 * 1024;
const UNKNOWN_EXIT_CODE: i32 = -1;
const SKILL_TOOL_ALLOWLIST: &[&str] = &["echo", "printf", "cat", "pwd", "true", "false"];
const SKILL_TOOL_CAT_MAX_FILE_BYTES: u64 = 64 * 1024;
const DESKTOP_ACTION_CONFIRM_PHRASE: &str = "RUN DESKTOP ACTION";
const GUI_AUTOMATION_MAX_STEPS: usize = 25;
const RUNTIME_ADAPTER_AUDIT_LOG_KEY: &str = "runtime_adapters.audit_log";
const RUNTIME_ADAPTER_AUDIT_LOG_LIMIT: usize = 200;
const RUNTIME_ADAPTER_AUDIT_EXPORT_DEFAULT_LIMIT: usize = 50;
const RUNTIME_ADAPTER_AUDIT_SUMMARY_MAX_CHARS: usize = 240;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeAdapterAuditEvent {
    pub id: String,
    pub occurred_at: String,
    pub kind: String,
    pub status: String,
    pub summary: String,
    pub duration_ms: Option<u64>,
    pub timed_out: bool,
    pub target: Option<String>,
    #[serde(default)]
    pub target_remote_user_id: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeAdapterAuditListRequest {
    pub limit: Option<usize>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub target_remote_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeAdapterAuditExportRequest {
    pub limit: Option<usize>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub format: Option<String>,
    pub target_remote_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeAdapterAuditExportResponse {
    pub total: usize,
    pub exported_count: usize,
    pub format: String,
    pub payload: String,
    pub events: Vec<RuntimeAdapterAuditEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeAdapterError {
    pub code: String,
    pub message: String,
}

impl RuntimeAdapterError {
    fn validation(message: impl Into<String>) -> Self {
        Self {
            code: "validation_error".to_string(),
            message: message.into(),
        }
    }

    fn runtime(message: impl Into<String>) -> Self {
        Self {
            code: "runtime_error".to_string(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for RuntimeAdapterError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for RuntimeAdapterError {}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillToolRequest {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillToolResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub audit_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopExecutorProbe {
    pub platform: String,
    pub session_type: Option<String>,
    pub display: Option<String>,
    pub wayland_display: Option<String>,
    pub has_graphical_session: bool,
    pub tool_availability: BTreeMap<String, bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopActionRequest {
    pub executor: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub dry_run: Option<bool>,
    pub confirmation_phrase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DesktopActionResponse {
    pub executed: bool,
    pub planned_command: Vec<String>,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
    pub timed_out: bool,
    pub audit_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuiAutomationStepRequest {
    pub label: Option<String>,
    pub executor: String,
    #[serde(default)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuiAutomationRequest {
    #[serde(default)]
    pub steps: Vec<GuiAutomationStepRequest>,
    pub dry_run: Option<bool>,
    pub confirmation_phrase: Option<String>,
    pub stop_on_error: Option<bool>,
    pub target_remote_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuiAutomationStepResult {
    pub index: usize,
    pub label: Option<String>,
    pub status: String,
    pub response: DesktopActionResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GuiAutomationResponse {
    pub executed: bool,
    pub dry_run: bool,
    pub step_count: usize,
    pub completed_count: usize,
    pub planned_commands: Vec<Vec<String>>,
    pub results: Vec<GuiAutomationStepResult>,
    pub target_remote_user_id: Option<String>,
    pub audit_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrajectorySummaryRequest {
    pub jsonl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrajectorySummaryResponse {
    pub line_count: usize,
    pub kind_counts: BTreeMap<String, usize>,
    pub source_counts: BTreeMap<String, usize>,
    pub reward_hint_count: usize,
    pub invalid_line_count: usize,
}

#[tauri::command]
pub fn runtime_adapter_execute_skill_tool(
    db: State<'_, Database>,
    request: SkillToolRequest,
) -> Result<SkillToolResponse, RuntimeAdapterError> {
    runtime_adapter_execute_skill_tool_for_db(db.inner(), request)
}

pub(crate) fn runtime_adapter_execute_skill_tool_for_db(
    db: &Database,
    request: SkillToolRequest,
) -> Result<SkillToolResponse, RuntimeAdapterError> {
    let started_at = Instant::now();
    let command = match normalize_command_name(&request.command) {
        Ok(command) => command,
        Err(error) => {
            return Err(record_skill_tool_error(
                db,
                request.command.clone(),
                "rejected",
                error,
                None,
                false,
                None,
            ));
        }
    };
    if let Err(error) = ensure_allowlisted(&command, SKILL_TOOL_ALLOWLIST, "skill tool") {
        return Err(record_skill_tool_error(
            db, command, "rejected", error, None, false, None,
        ));
    }

    let cwd = match normalize_optional_cwd(request.cwd) {
        Ok(cwd) => cwd,
        Err(error) => {
            return Err(record_skill_tool_error(
                db, command, "rejected", error, None, false, None,
            ));
        }
    };
    let timeout_ms = normalize_timeout_ms(request.timeout_ms);
    if let Err(error) = validate_skill_tool_args(&command, &request.args, cwd.as_deref()) {
        return Err(record_skill_tool_error(
            db, command, "rejected", error, None, false, None,
        ));
    }
    let invocation = CommandInvocation {
        program: command.clone(),
        args: request.args,
        cwd,
    };
    let execution = match execute_command(&invocation, timeout_ms) {
        Ok(execution) => execution,
        Err(error) => {
            return Err(record_skill_tool_error(
                db,
                command,
                "failed",
                error,
                Some(elapsed_ms(started_at)),
                false,
                None,
            ));
        }
    };

    let status = command_execution_status(&execution).to_string();
    let summary = truncate_summary(skill_tool_summary(
        &command,
        execution.exit_code,
        execution.timed_out,
        timeout_ms,
    ));
    let duration_ms = execution.duration_ms;
    let timed_out = execution.timed_out;
    let exit_code = execution.exit_code;

    let response = SkillToolResponse {
        exit_code: execution.exit_code,
        stdout: execution.stdout,
        stderr: execution.stderr,
        duration_ms,
        timed_out,
        audit_message: if execution.timed_out {
            format!(
                "skill tool `{}` timed out after {} ms and was terminated",
                command, timeout_ms
            )
        } else {
            format!(
                "skill tool executed allowlisted command `{}` within normalized timeout {} ms",
                command, timeout_ms
            )
        },
    };

    record_runtime_adapter_audit_event(
        db,
        RuntimeAdapterAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: Utc::now().to_rfc3339(),
            kind: "skill_tool".to_string(),
            status,
            summary,
            duration_ms: Some(duration_ms),
            timed_out,
            target: Some(command),
            target_remote_user_id: None,
            exit_code: Some(exit_code),
        },
    )
    .map_err(runtime_error_from_audit_failure)?;

    Ok(response)
}

#[tauri::command]
pub fn runtime_adapter_probe_desktop_executor() -> DesktopExecutorProbe {
    let mut tool_availability = BTreeMap::new();
    for tool in ["xdotool", "osascript", "powershell", "powershell.exe"] {
        tool_availability.insert(tool.to_string(), is_tool_available(tool));
    }

    let display = env::var("DISPLAY").ok().filter(|value| !value.is_empty());
    let wayland_display = env::var("WAYLAND_DISPLAY")
        .ok()
        .filter(|value| !value.is_empty());

    DesktopExecutorProbe {
        platform: env::consts::OS.to_string(),
        session_type: env::var("XDG_SESSION_TYPE")
            .ok()
            .filter(|value| !value.is_empty()),
        display: display.clone(),
        wayland_display: wayland_display.clone(),
        has_graphical_session: display.is_some() || wayland_display.is_some(),
        tool_availability,
    }
}

#[tauri::command]
pub fn runtime_adapter_execute_desktop_action(
    db: State<'_, Database>,
    request: DesktopActionRequest,
) -> Result<DesktopActionResponse, RuntimeAdapterError> {
    runtime_adapter_execute_desktop_action_for_db(db.inner(), request)
}

#[tauri::command]
pub fn runtime_adapter_run_gui_automation(
    db: State<'_, Database>,
    request: GuiAutomationRequest,
) -> Result<GuiAutomationResponse, RuntimeAdapterError> {
    runtime_adapter_run_gui_automation_for_db(db.inner(), request)
}

fn runtime_adapter_execute_desktop_action_for_db(
    db: &Database,
    request: DesktopActionRequest,
) -> Result<DesktopActionResponse, RuntimeAdapterError> {
    let started_at = Instant::now();
    let executor = match normalize_command_name(&request.executor) {
        Ok(executor) => executor,
        Err(error) => {
            return Err(record_desktop_action_error(
                db,
                request.executor,
                "rejected",
                error,
                None,
                false,
                None,
            ));
        }
    };
    if let Err(error) =
        ensure_allowlisted(&executor, desktop_executor_allowlist(), "desktop executor")
    {
        return Err(record_desktop_action_error(
            db, executor, "rejected", error, None, false, None,
        ));
    }

    let planned_command = planned_command(&executor, &request.args);
    if request.dry_run.unwrap_or(true) {
        let response = DesktopActionResponse {
            executed: false,
            planned_command: planned_command.clone(),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            duration_ms: 0,
            timed_out: false,
            audit_message: format!(
                "desktop action dry-run preview; planned command `{}` was not executed",
                executor
            ),
        };
        record_runtime_adapter_audit_event(
            db,
            RuntimeAdapterAuditEvent {
                id: Uuid::new_v4().to_string(),
                occurred_at: Utc::now().to_rfc3339(),
                kind: "desktop_action".to_string(),
                status: "dry_run".to_string(),
                summary: truncate_summary(desktop_action_dry_run_summary(&planned_command)),
                duration_ms: Some(0),
                timed_out: false,
                target: Some(executor),
                target_remote_user_id: None,
                exit_code: None,
            },
        )
        .map_err(runtime_error_from_audit_failure)?;
        return Ok(response);
    }

    if request.confirmation_phrase.as_deref().map(str::trim) != Some(DESKTOP_ACTION_CONFIRM_PHRASE)
    {
        return Err(record_desktop_action_error(
            db,
            executor,
            "rejected",
            RuntimeAdapterError::validation(format!(
                "non-dry-run desktop action requires confirmation phrase `{}`",
                DESKTOP_ACTION_CONFIRM_PHRASE
            )),
            Some(elapsed_ms(started_at)),
            false,
            None,
        ));
    }

    if !is_tool_available(&executor) {
        return Err(record_desktop_action_error(
            db,
            executor.clone(),
            "failed",
            RuntimeAdapterError::runtime(format!(
                "desktop executor `{}` is not available on PATH",
                executor
            )),
            Some(elapsed_ms(started_at)),
            false,
            None,
        ));
    }

    let execution = match execute_command(
        &CommandInvocation {
            program: executor.clone(),
            args: request.args,
            cwd: None,
        },
        DEFAULT_TIMEOUT_MS,
    ) {
        Ok(execution) => execution,
        Err(error) => {
            return Err(record_desktop_action_error(
                db,
                executor,
                "failed",
                error,
                Some(elapsed_ms(started_at)),
                false,
                None,
            ));
        }
    };

    let status = command_execution_status(&execution).to_string();
    let summary = truncate_summary(desktop_action_summary(
        &executor,
        execution.exit_code,
        execution.timed_out,
        DEFAULT_TIMEOUT_MS,
    ));
    let duration_ms = execution.duration_ms;
    let timed_out = execution.timed_out;
    let exit_code = execution.exit_code;

    let response = DesktopActionResponse {
        executed: true,
        planned_command: planned_command.clone(),
        exit_code: Some(exit_code),
        stdout: execution.stdout,
        stderr: execution.stderr,
        duration_ms,
        timed_out,
        audit_message: if execution.timed_out {
            format!(
                "desktop action via `{}` timed out after {} ms and was terminated",
                executor, DEFAULT_TIMEOUT_MS
            )
        } else {
            format!(
                "desktop action executed via allowlisted executor `{}`",
                executor
            )
        },
    };

    record_runtime_adapter_audit_event(
        db,
        RuntimeAdapterAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: Utc::now().to_rfc3339(),
            kind: "desktop_action".to_string(),
            status,
            summary,
            duration_ms: Some(duration_ms),
            timed_out,
            target: Some(executor),
            target_remote_user_id: None,
            exit_code: Some(exit_code),
        },
    )
    .map_err(runtime_error_from_audit_failure)?;

    Ok(response)
}

pub(crate) fn runtime_adapter_run_gui_automation_for_db(
    db: &Database,
    request: GuiAutomationRequest,
) -> Result<GuiAutomationResponse, RuntimeAdapterError> {
    let started_at = Instant::now();
    let GuiAutomationRequest {
        steps,
        dry_run,
        confirmation_phrase,
        stop_on_error,
        target_remote_user_id,
    } = request;
    let dry_run = dry_run.unwrap_or(true);
    let stop_on_error = stop_on_error.unwrap_or(false);
    let target_remote_user_id = normalize_target_remote_user_id(target_remote_user_id);
    let step_count = steps.len();
    if step_count == 0 {
        return Err(record_gui_automation_error(
            db,
            "rejected",
            RuntimeAdapterError::validation("at least one GUI automation step is required"),
            Some(elapsed_ms(started_at)),
            target_remote_user_id.clone(),
        ));
    }
    if step_count > GUI_AUTOMATION_MAX_STEPS {
        return Err(record_gui_automation_error(
            db,
            "rejected",
            RuntimeAdapterError::validation(format!(
                "GUI automation supports at most {} steps per macro",
                GUI_AUTOMATION_MAX_STEPS
            )),
            Some(elapsed_ms(started_at)),
            target_remote_user_id.clone(),
        ));
    }
    if !dry_run
        && confirmation_phrase.as_deref().map(str::trim) != Some(DESKTOP_ACTION_CONFIRM_PHRASE)
    {
        return Err(record_gui_automation_error(
            db,
            "rejected",
            RuntimeAdapterError::validation(format!(
                "non-dry-run GUI automation requires confirmation phrase `{}`",
                DESKTOP_ACTION_CONFIRM_PHRASE
            )),
            Some(elapsed_ms(started_at)),
            target_remote_user_id.clone(),
        ));
    }

    let mut results = Vec::with_capacity(step_count);
    for (index, step) in steps.into_iter().enumerate() {
        let response = runtime_adapter_execute_desktop_action_for_db(
            db,
            DesktopActionRequest {
                executor: step.executor,
                args: step.args,
                dry_run: Some(dry_run),
                confirmation_phrase: confirmation_phrase.clone(),
            },
        )?;
        let status = if response.timed_out {
            "timed_out"
        } else if response.exit_code.is_some_and(|code| code != 0) {
            "failed"
        } else if response.executed {
            "succeeded"
        } else {
            "dry_run"
        };
        results.push(GuiAutomationStepResult {
            index,
            label: normalize_gui_automation_label(step.label),
            status: status.to_string(),
            response,
        });
        if stop_on_error && matches!(status, "failed" | "timed_out") {
            break;
        }
    }

    let planned_commands = results
        .iter()
        .map(|result| result.response.planned_command.clone())
        .collect::<Vec<_>>();
    let completed_count = results.len();
    let status = if dry_run {
        "dry_run"
    } else if results
        .iter()
        .any(|result| result.status == "failed" || result.status == "timed_out")
    {
        "failed"
    } else {
        "succeeded"
    };
    let duration_ms = elapsed_ms(started_at);
    let audit_message = if dry_run {
        format!(
            "GUI automation macro dry-run planned {} allowlisted desktop action step(s)",
            step_count
        )
    } else {
        format!(
            "GUI automation macro executed {} allowlisted desktop action step(s)",
            completed_count
        )
    };

    record_runtime_adapter_audit_event(
        db,
        RuntimeAdapterAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: Utc::now().to_rfc3339(),
            kind: "gui_automation".to_string(),
            status: status.to_string(),
            summary: truncate_summary(gui_automation_summary(
                status,
                step_count,
                completed_count,
                &planned_commands,
            )),
            duration_ms: Some(duration_ms),
            timed_out: results.iter().any(|result| result.response.timed_out),
            target: Some("desktop_macro".to_string()),
            target_remote_user_id: target_remote_user_id.clone(),
            exit_code: None,
        },
    )
    .map_err(runtime_error_from_audit_failure)?;

    Ok(GuiAutomationResponse {
        executed: !dry_run,
        dry_run,
        step_count,
        completed_count,
        planned_commands,
        results,
        target_remote_user_id,
        audit_message,
    })
}

fn normalize_gui_automation_label(label: Option<String>) -> Option<String> {
    label
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_target_remote_user_id(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[tauri::command]
pub fn runtime_adapter_summarize_trajectory_jsonl(
    db: State<'_, Database>,
    request: TrajectorySummaryRequest,
) -> Result<TrajectorySummaryResponse, RuntimeAdapterError> {
    runtime_adapter_summarize_trajectory_jsonl_for_db(db.inner(), request)
}

fn runtime_adapter_summarize_trajectory_jsonl_for_db(
    db: &Database,
    request: TrajectorySummaryRequest,
) -> Result<TrajectorySummaryResponse, RuntimeAdapterError> {
    let started_at = Instant::now();
    let mut line_count = 0;
    let mut invalid_line_count = 0;
    let mut reward_hint_count = 0;
    let mut kind_counts = BTreeMap::new();
    let mut source_counts = BTreeMap::new();

    for line in request.jsonl.lines() {
        if line.trim().is_empty() {
            continue;
        }

        line_count += 1;
        let parsed: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => {
                invalid_line_count += 1;
                continue;
            }
        };

        if let Some(kind) = parsed.get("kind").and_then(Value::as_str) {
            *kind_counts.entry(kind.to_string()).or_insert(0) += 1;
        }

        if let Some(source) = parsed.get("source").and_then(Value::as_str) {
            *source_counts.entry(source.to_string()).or_insert(0) += 1;
        }

        if parsed
            .get("reward_hint")
            .is_some_and(|value| !value.is_null())
        {
            reward_hint_count += 1;
        }
    }

    let response = TrajectorySummaryResponse {
        line_count,
        kind_counts,
        source_counts,
        reward_hint_count,
        invalid_line_count,
    };

    record_runtime_adapter_audit_event(
        db,
        RuntimeAdapterAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: Utc::now().to_rfc3339(),
            kind: "trajectory_summary".to_string(),
            status: "succeeded".to_string(),
            summary: truncate_summary(trajectory_summary_audit_summary(&response)),
            duration_ms: Some(elapsed_ms(started_at)),
            timed_out: false,
            target: None,
            target_remote_user_id: None,
            exit_code: None,
        },
    )
    .map_err(runtime_error_from_audit_failure)?;

    Ok(response)
}

#[tauri::command]
pub fn runtime_adapter_list_audit_events(
    db: State<'_, Database>,
    request: RuntimeAdapterAuditListRequest,
) -> Result<Vec<RuntimeAdapterAuditEvent>, AppError> {
    list_runtime_adapter_audit_events_for_db(db.inner(), request)
}

#[tauri::command]
pub fn runtime_adapter_export_audit_events(
    db: State<'_, Database>,
    request: RuntimeAdapterAuditExportRequest,
) -> Result<RuntimeAdapterAuditExportResponse, AppError> {
    export_runtime_adapter_audit_events_for_db(db.inner(), request)
}

#[derive(Debug, Clone)]
struct CommandInvocation {
    program: String,
    args: Vec<String>,
    cwd: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandExecution {
    exit_code: i32,
    stdout: String,
    stderr: String,
    duration_ms: u64,
    timed_out: bool,
}

fn desktop_executor_allowlist() -> &'static [&'static str] {
    #[cfg(target_os = "macos")]
    {
        &["osascript"]
    }

    #[cfg(target_os = "windows")]
    {
        &["powershell", "powershell.exe"]
    }

    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        &["xdotool"]
    }
}

fn normalize_command_name(value: &str) -> Result<String, RuntimeAdapterError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(RuntimeAdapterError::validation("command cannot be empty"));
    }
    if trimmed.contains(std::path::MAIN_SEPARATOR)
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        return Err(RuntimeAdapterError::validation(
            "command must be a bare executable name",
        ));
    }
    Ok(trimmed.to_string())
}

fn normalize_optional_cwd(value: Option<String>) -> Result<Option<PathBuf>, RuntimeAdapterError> {
    match value {
        Some(path) if path.trim().is_empty() => {
            Err(RuntimeAdapterError::validation("cwd cannot be empty"))
        }
        Some(path) => Ok(Some(PathBuf::from(path))),
        None => Ok(None),
    }
}

fn normalize_timeout_ms(value: Option<u64>) -> u64 {
    value
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(MIN_TIMEOUT_MS, MAX_TIMEOUT_MS)
}

fn validate_skill_tool_args(
    command: &str,
    args: &[String],
    cwd: Option<&Path>,
) -> Result<(), RuntimeAdapterError> {
    if command != "cat" {
        return Ok(());
    }

    validate_cat_file_args(args, cwd)
}

fn validate_cat_file_args(args: &[String], cwd: Option<&Path>) -> Result<(), RuntimeAdapterError> {
    if args.is_empty() {
        return Err(RuntimeAdapterError::validation(
            "cat requires at least one relative file path",
        ));
    }

    let root = resolve_skill_tool_cwd(cwd)?;
    let canonical_root = fs::canonicalize(&root).map_err(|error| {
        RuntimeAdapterError::validation(format!(
            "cat cwd `{}` must resolve to an existing directory: {}",
            root.display(),
            error
        ))
    })?;

    if !canonical_root.is_dir() {
        return Err(RuntimeAdapterError::validation(format!(
            "cat cwd `{}` must be a directory",
            canonical_root.display()
        )));
    }

    for raw_arg in args {
        validate_cat_file_arg(raw_arg, &canonical_root)?;
    }

    Ok(())
}

fn validate_cat_file_arg(raw_arg: &str, canonical_root: &Path) -> Result<(), RuntimeAdapterError> {
    if raw_arg.trim().is_empty() {
        return Err(RuntimeAdapterError::validation(
            "cat file path cannot be empty",
        ));
    }

    let candidate = Path::new(raw_arg);
    if candidate.is_absolute() {
        return Err(RuntimeAdapterError::validation(format!(
            "cat only supports relative file paths within `{}`",
            canonical_root.display()
        )));
    }

    if candidate
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(RuntimeAdapterError::validation(format!(
            "cat path traversal is not allowed: `{}`",
            raw_arg
        )));
    }

    let canonical_target = fs::canonicalize(canonical_root.join(candidate)).map_err(|error| {
        RuntimeAdapterError::validation(format!(
            "cat target `{}` must resolve to an existing file: {}",
            raw_arg, error
        ))
    })?;

    if !canonical_target.starts_with(canonical_root) {
        return Err(RuntimeAdapterError::validation(format!(
            "cat target `{}` must stay within `{}`",
            raw_arg,
            canonical_root.display()
        )));
    }

    let metadata = fs::metadata(&canonical_target).map_err(|error| {
        RuntimeAdapterError::validation(format!(
            "failed to inspect cat target `{}`: {}",
            raw_arg, error
        ))
    })?;

    if !metadata.is_file() {
        return Err(RuntimeAdapterError::validation(format!(
            "cat target `{}` must be a regular file",
            raw_arg
        )));
    }

    if metadata.len() > SKILL_TOOL_CAT_MAX_FILE_BYTES {
        return Err(RuntimeAdapterError::validation(format!(
            "cat target `{}` exceeds the 64 KiB size limit",
            raw_arg
        )));
    }

    Ok(())
}

fn resolve_skill_tool_cwd(cwd: Option<&Path>) -> Result<PathBuf, RuntimeAdapterError> {
    let current_dir = env::current_dir().map_err(|error| {
        RuntimeAdapterError::runtime(format!("failed to resolve current directory: {}", error))
    })?;

    Ok(match cwd {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => current_dir.join(path),
        None => current_dir,
    })
}

fn ensure_allowlisted(
    candidate: &str,
    allowlist: &[&str],
    label: &str,
) -> Result<(), RuntimeAdapterError> {
    if allowlist.contains(&candidate) {
        return Ok(());
    }

    Err(RuntimeAdapterError::validation(format!(
        "{} `{}` is not allowlisted",
        label, candidate
    )))
}

fn execute_command(
    invocation: &CommandInvocation,
    timeout_ms: u64,
) -> Result<CommandExecution, RuntimeAdapterError> {
    let mut command = Command::new(&invocation.program);
    command.args(&invocation.args);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(cwd) = invocation.cwd.as_ref() {
        command.current_dir(cwd);
    }

    let started_at = Instant::now();
    let mut child = command.spawn().map_err(|err| {
        RuntimeAdapterError::runtime(format!(
            "failed to execute `{}`: {}",
            invocation.program, err
        ))
    })?;
    let timeout = Duration::from_millis(timeout_ms);

    loop {
        if child
            .try_wait()
            .map_err(|err| {
                RuntimeAdapterError::runtime(format!(
                    "failed to poll `{}`: {}",
                    invocation.program, err
                ))
            })?
            .is_some()
        {
            let output = child.wait_with_output().map_err(|err| {
                RuntimeAdapterError::runtime(format!(
                    "failed to collect `{}` output: {}",
                    invocation.program, err
                ))
            })?;
            return Ok(CommandExecution {
                exit_code: output.status.code().unwrap_or(UNKNOWN_EXIT_CODE),
                stdout: truncate_output(&output.stdout),
                stderr: truncate_output(&output.stderr),
                duration_ms: elapsed_ms(started_at),
                timed_out: false,
            });
        }

        if started_at.elapsed() >= timeout {
            let _ = child.kill();
            let output = child.wait_with_output().map_err(|err| {
                RuntimeAdapterError::runtime(format!(
                    "failed to collect timed-out `{}` output: {}",
                    invocation.program, err
                ))
            })?;
            let stderr = append_timeout_message(
                truncate_output(&output.stderr),
                &invocation.program,
                timeout_ms,
            );
            return Ok(CommandExecution {
                exit_code: UNKNOWN_EXIT_CODE,
                stdout: truncate_output(&output.stdout),
                stderr,
                duration_ms: elapsed_ms(started_at),
                timed_out: true,
            });
        }

        thread::sleep(Duration::from_millis(5));
    }
}

fn elapsed_ms(started_at: Instant) -> u64 {
    started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}

fn append_timeout_message(mut stderr: String, program: &str, timeout_ms: u64) -> String {
    if !stderr.is_empty() && !stderr.ends_with('\n') {
        stderr.push('\n');
    }
    stderr.push_str(&format!(
        "process `{}` timed out after {} ms and was terminated",
        program, timeout_ms
    ));
    stderr
}

fn truncate_output(bytes: &[u8]) -> String {
    if bytes.len() <= MAX_CAPTURE_BYTES {
        return String::from_utf8_lossy(bytes).into_owned();
    }

    let mut truncated = bytes[..MAX_CAPTURE_BYTES].to_vec();
    let suffix = b"\n...[truncated]";
    if truncated.len() + suffix.len() > MAX_CAPTURE_BYTES {
        truncated.truncate(MAX_CAPTURE_BYTES - suffix.len());
    }
    truncated.extend_from_slice(suffix);
    String::from_utf8_lossy(&truncated).into_owned()
}

fn planned_command(program: &str, args: &[String]) -> Vec<String> {
    let mut planned = Vec::with_capacity(args.len() + 1);
    planned.push(program.to_string());
    planned.extend(args.iter().cloned());
    planned
}

fn is_tool_available(tool: &str) -> bool {
    let paths = env::var_os("PATH")
        .map(|raw| env::split_paths(&raw).collect::<Vec<_>>())
        .unwrap_or_default();

    paths.iter().any(|dir| is_executable_path(dir, tool))
}

fn is_executable_path(dir: &Path, tool: &str) -> bool {
    let direct = dir.join(tool);
    if direct.is_file() {
        return true;
    }

    #[cfg(target_os = "windows")]
    {
        if Path::new(tool).extension().is_some() {
            return false;
        }

        for extension in ["exe", "cmd", "bat"] {
            if dir.join(format!("{}.{}", tool, extension)).is_file() {
                return true;
            }
        }
    }

    false
}

#[allow(dead_code)]
fn _assert_os_str_compatibility(_: &OsStr) {}

fn record_skill_tool_error(
    db: &Database,
    command: String,
    status: &str,
    error: RuntimeAdapterError,
    duration_ms: Option<u64>,
    timed_out: bool,
    exit_code: Option<i32>,
) -> RuntimeAdapterError {
    let summary = truncate_summary(format!("skill tool `{}` {}", command, error.message));
    audit_error_result(
        db,
        RuntimeAdapterAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: Utc::now().to_rfc3339(),
            kind: "skill_tool".to_string(),
            status: status.to_string(),
            summary,
            duration_ms,
            timed_out,
            target: Some(command),
            target_remote_user_id: None,
            exit_code,
        },
        error,
    )
}

fn record_desktop_action_error(
    db: &Database,
    executor: String,
    status: &str,
    error: RuntimeAdapterError,
    duration_ms: Option<u64>,
    timed_out: bool,
    exit_code: Option<i32>,
) -> RuntimeAdapterError {
    let summary = truncate_summary(format!("desktop action `{}` {}", executor, error.message));
    audit_error_result(
        db,
        RuntimeAdapterAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: Utc::now().to_rfc3339(),
            kind: "desktop_action".to_string(),
            status: status.to_string(),
            summary,
            duration_ms,
            timed_out,
            target: Some(executor),
            target_remote_user_id: None,
            exit_code,
        },
        error,
    )
}

fn record_gui_automation_error(
    db: &Database,
    status: &str,
    error: RuntimeAdapterError,
    duration_ms: Option<u64>,
    target_remote_user_id: Option<String>,
) -> RuntimeAdapterError {
    let summary = truncate_summary(format!("GUI automation macro {}", error.message));
    audit_error_result(
        db,
        RuntimeAdapterAuditEvent {
            id: Uuid::new_v4().to_string(),
            occurred_at: Utc::now().to_rfc3339(),
            kind: "gui_automation".to_string(),
            status: status.to_string(),
            summary,
            duration_ms,
            timed_out: false,
            target: Some("desktop_macro".to_string()),
            target_remote_user_id,
            exit_code: None,
        },
        error,
    )
}

fn audit_error_result(
    db: &Database,
    event: RuntimeAdapterAuditEvent,
    error: RuntimeAdapterError,
) -> RuntimeAdapterError {
    match record_runtime_adapter_audit_event(db, event) {
        Ok(()) => error,
        Err(audit_error) => RuntimeAdapterError {
            code: error.code,
            message: format!(
                "{} (audit persistence failed: {})",
                error.message, audit_error
            ),
        },
    }
}

fn runtime_error_from_audit_failure(error: AppError) -> RuntimeAdapterError {
    RuntimeAdapterError::runtime(format!(
        "failed to persist runtime adapter audit event: {error}"
    ))
}

fn list_runtime_adapter_audit_events_for_db(
    db: &Database,
    request: RuntimeAdapterAuditListRequest,
) -> AppResult<Vec<RuntimeAdapterAuditEvent>> {
    let kind_filter = normalize_audit_filter(request.kind);
    let status_filter = normalize_audit_filter(request.status);
    let target_remote_user_id = normalize_target_remote_user_id(request.target_remote_user_id);
    let limit = normalize_audit_list_limit(request.limit);
    let mut events = load_runtime_adapter_audit_log(db)?;
    events.sort_by(|left, right| {
        right
            .occurred_at
            .cmp(&left.occurred_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    events.retain(|event| audit_filter_matches(&event.kind, kind_filter.as_deref()));
    events.retain(|event| audit_filter_matches(&event.status, status_filter.as_deref()));
    events.retain(|event| {
        target_remote_user_id
            .as_deref()
            .is_none_or(|target| event.target_remote_user_id.as_deref() == Some(target))
    });
    if let Some(limit) = limit {
        events.truncate(limit.min(events.len()));
    }
    Ok(events)
}

fn export_runtime_adapter_audit_events_for_db(
    db: &Database,
    request: RuntimeAdapterAuditExportRequest,
) -> AppResult<RuntimeAdapterAuditExportResponse> {
    let format = normalize_audit_export_format(request.format)?;
    let kind = request.kind;
    let status = request.status;
    let target_remote_user_id = request.target_remote_user_id;
    let total = list_runtime_adapter_audit_events_for_db(
        db,
        RuntimeAdapterAuditListRequest {
            limit: None,
            kind: kind.clone(),
            status: status.clone(),
            target_remote_user_id: target_remote_user_id.clone(),
        },
    )?
    .len();
    let events = list_runtime_adapter_audit_events_for_db(
        db,
        RuntimeAdapterAuditListRequest {
            limit: Some(normalize_audit_export_limit(request.limit)),
            kind,
            status,
            target_remote_user_id,
        },
    )?;
    let payload = serialize_audit_events(&events, &format)?;

    Ok(RuntimeAdapterAuditExportResponse {
        total,
        exported_count: events.len(),
        format,
        payload,
        events,
    })
}

fn load_runtime_adapter_audit_log(db: &Database) -> AppResult<Vec<RuntimeAdapterAuditEvent>> {
    let stored = db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&RUNTIME_ADAPTER_AUDIT_LOG_KEY],
        |row| row.get::<_, String>(0),
    );

    match stored {
        Ok(value_json) => serde_json::from_str::<Vec<RuntimeAdapterAuditEvent>>(&value_json)
            .map_err(AppError::from_json_error),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Vec::new()),
        Err(error) => Err(AppError::storage(format!(
            "Failed to load runtime adapter audit log: {}",
            error
        ))),
    }
}

fn record_runtime_adapter_audit_event(
    db: &Database,
    event: RuntimeAdapterAuditEvent,
) -> AppResult<()> {
    let mut log = load_runtime_adapter_audit_log(db)?;
    log.insert(0, event);
    log.truncate(RUNTIME_ADAPTER_AUDIT_LOG_LIMIT);
    let value_json = serde_json::to_string(&log).map_err(AppError::from_json_error)?;
    let now = Utc::now().to_rfc3339();
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
        &[&RUNTIME_ADAPTER_AUDIT_LOG_KEY, &value_json, &now],
    )?;
    Ok(())
}

fn normalize_audit_filter(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let normalized = value.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}

fn audit_filter_matches(value: &str, filter: Option<&str>) -> bool {
    filter.is_none_or(|filter| value.eq_ignore_ascii_case(filter))
}

fn normalize_audit_list_limit(limit: Option<usize>) -> Option<usize> {
    match limit {
        Some(0) | None => None,
        Some(limit) => Some(limit.min(RUNTIME_ADAPTER_AUDIT_LOG_LIMIT)),
    }
}

fn normalize_audit_export_limit(limit: Option<usize>) -> usize {
    match limit {
        Some(0) | None => RUNTIME_ADAPTER_AUDIT_EXPORT_DEFAULT_LIMIT,
        Some(limit) => limit.min(RUNTIME_ADAPTER_AUDIT_LOG_LIMIT),
    }
}

fn normalize_audit_export_format(format: Option<String>) -> AppResult<String> {
    let normalized = format
        .unwrap_or_else(|| "json".to_string())
        .trim()
        .to_ascii_lowercase();
    if normalized == "json" || normalized == "jsonl" {
        return Ok(normalized);
    }
    Err(AppError::validation(format!(
        "unsupported runtime adapter audit export format `{}`",
        normalized
    )))
}

fn serialize_audit_events(events: &[RuntimeAdapterAuditEvent], format: &str) -> AppResult<String> {
    match format {
        "json" => serde_json::to_string_pretty(events).map_err(AppError::from_json_error),
        "jsonl" => events
            .iter()
            .map(|event| serde_json::to_string(event).map_err(AppError::from_json_error))
            .collect::<AppResult<Vec<_>>>()
            .map(|lines| lines.join("\n")),
        _ => Err(AppError::validation(format!(
            "unsupported runtime adapter audit export format `{}`",
            format
        ))),
    }
}

fn command_execution_status(execution: &CommandExecution) -> &'static str {
    if execution.timed_out {
        "timed_out"
    } else if execution.exit_code == 0 {
        "succeeded"
    } else {
        "failed"
    }
}

fn skill_tool_summary(command: &str, exit_code: i32, timed_out: bool, timeout_ms: u64) -> String {
    if timed_out {
        format!(
            "skill tool `{}` timed out after {} ms and was terminated",
            command, timeout_ms
        )
    } else if exit_code == 0 {
        format!("skill tool `{}` succeeded with exit code 0", command)
    } else {
        format!(
            "skill tool `{}` finished with exit code {}",
            command, exit_code
        )
    }
}

fn desktop_action_dry_run_summary(planned_command: &[String]) -> String {
    format!(
        "desktop action dry-run planned `{}` without executing GUI automation",
        planned_command.join(" ")
    )
}

fn desktop_action_summary(
    executor: &str,
    exit_code: i32,
    timed_out: bool,
    timeout_ms: u64,
) -> String {
    if timed_out {
        format!(
            "desktop action `{}` timed out after {} ms and was terminated",
            executor, timeout_ms
        )
    } else if exit_code == 0 {
        format!("desktop action `{}` succeeded with exit code 0", executor)
    } else {
        format!(
            "desktop action `{}` finished with exit code {}",
            executor, exit_code
        )
    }
}

fn gui_automation_summary(
    status: &str,
    step_count: usize,
    completed_count: usize,
    planned_commands: &[Vec<String>],
) -> String {
    let preview = planned_commands
        .iter()
        .take(3)
        .map(|command| command.join(" "))
        .collect::<Vec<_>>()
        .join(" | ");
    if preview.is_empty() {
        format!(
            "GUI automation macro {} with {} step(s), {} completed",
            status, step_count, completed_count
        )
    } else {
        format!(
            "GUI automation macro {} with {} step(s), {} completed; planned: {}",
            status, step_count, completed_count, preview
        )
    }
}

fn trajectory_summary_audit_summary(summary: &TrajectorySummaryResponse) -> String {
    format!(
        "trajectory summary processed {} JSONL lines with {} invalid and {} reward hints",
        summary.line_count, summary.invalid_line_count, summary.reward_hint_count
    )
}

fn truncate_summary(value: String) -> String {
    let char_count = value.chars().count();
    if char_count <= RUNTIME_ADAPTER_AUDIT_SUMMARY_MAX_CHARS {
        return value;
    }

    let mut truncated = value
        .chars()
        .take(RUNTIME_ADAPTER_AUDIT_SUMMARY_MAX_CHARS.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::{
        DESKTOP_ACTION_CONFIRM_PHRASE, DesktopActionRequest, GUI_AUTOMATION_MAX_STEPS,
        GuiAutomationRequest, GuiAutomationStepRequest, RuntimeAdapterAuditEvent,
        RuntimeAdapterAuditExportRequest, RuntimeAdapterAuditListRequest, SkillToolRequest,
        TrajectorySummaryRequest, export_runtime_adapter_audit_events_for_db,
        list_runtime_adapter_audit_events_for_db, runtime_adapter_execute_desktop_action_for_db,
        runtime_adapter_execute_skill_tool_for_db, runtime_adapter_probe_desktop_executor,
        runtime_adapter_run_gui_automation_for_db,
        runtime_adapter_summarize_trajectory_jsonl_for_db,
    };
    use crate::backend::Database;
    use std::collections::HashSet;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvVarOverride {
        key: &'static str,
        original: Option<OsString>,
    }

    impl EnvVarOverride {
        fn set_os(key: &'static str, value: &OsString) -> Self {
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

    fn prepend_path(dir: &Path) -> EnvVarOverride {
        let mut paths = vec![dir.to_path_buf()];
        if let Some(existing) = env::var_os("PATH") {
            paths.extend(env::split_paths(&existing));
        }
        let joined = env::join_paths(paths).expect("PATH entries should join");
        EnvVarOverride::set_os("PATH", &joined)
    }

    #[cfg(unix)]
    fn install_test_desktop_executor(bin_dir: &Path) {
        let script_path = bin_dir.join(desktop_executor_name_for_platform());
        fs::write(
            &script_path,
            "#!/bin/sh\nif [ \"$1\" = \"fail\" ]; then\n  exit 42\nfi\nif [ \"$1\" = \"ok\" ]; then\n  exit 0\nfi\nexit 0\n",
        )
        .expect("stub desktop executor should be written");
        let mut permissions = fs::metadata(&script_path)
            .expect("stub desktop executor metadata should load")
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_path, permissions)
            .expect("stub desktop executor should be executable");
    }

    #[cfg(windows)]
    fn install_test_desktop_executor(bin_dir: &Path) {
        fs::write(
            bin_dir.join("powershell.cmd"),
            "@echo off\r\nif \"%1\"==\"fail\" exit /b 42\r\nif \"%1\"==\"ok\" exit /b 0\r\nexit /b 0\r\n",
        )
        .expect("stub desktop executor should be written");
    }

    #[test]
    fn runtime_adapter_executes_allowlisted_command_and_blocks_unknown() {
        let db = Database::in_memory().expect("database should initialize");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let cwd = std::env::temp_dir().join(format!("runtime-adapter-{unique}"));
        fs::create_dir_all(&cwd).expect("temporary working directory should exist");

        let ok = runtime_adapter_execute_skill_tool_for_db(
            &db,
            SkillToolRequest {
                command: "pwd".to_string(),
                args: Vec::new(),
                cwd: Some(cwd.display().to_string()),
                timeout_ms: Some(250),
            },
        )
        .expect("allowlisted command should execute");
        assert_eq!(ok.exit_code, 0);
        assert_eq!(ok.stdout.trim(), cwd.display().to_string());
        assert!(ok.audit_message.contains("pwd"));

        let blocked = runtime_adapter_execute_skill_tool_for_db(
            &db,
            SkillToolRequest {
                command: "python3".to_string(),
                args: vec!["-c".to_string(), "print('nope')".to_string()],
                cwd: None,
                timeout_ms: None,
            },
        )
        .expect_err("unknown command must be rejected");
        assert_eq!(blocked.code, "validation_error");
        assert!(blocked.message.contains("not allowlisted"));
    }

    #[cfg(unix)]
    #[test]
    fn runtime_adapter_executes_allowlisted_printf_without_timeout() {
        let db = Database::in_memory().expect("database should initialize");
        let cwd = unique_runtime_adapter_test_dir();
        let large_output = "x".repeat(32 * 1024);
        let timed_out = runtime_adapter_execute_skill_tool_for_db(
            &db,
            SkillToolRequest {
                command: "printf".to_string(),
                args: vec![large_output],
                cwd: Some(cwd.display().to_string()),
                timeout_ms: Some(50),
            },
        )
        .expect("printf should return an audited response");

        assert!(!timed_out.timed_out);
        assert_eq!(timed_out.exit_code, 0);
        assert!(timed_out.stderr.is_empty());
        assert!(!timed_out.stdout.is_empty());
    }

    #[test]
    fn runtime_adapter_rejects_cat_absolute_paths_and_audits_rejection() {
        let db = Database::in_memory().expect("database should initialize");
        let file = unique_runtime_adapter_test_dir().join("allowed.txt");
        fs::write(&file, "secret").expect("test file should be written");

        let error = runtime_adapter_execute_skill_tool_for_db(
            &db,
            SkillToolRequest {
                command: "cat".to_string(),
                args: vec![file.display().to_string()],
                cwd: None,
                timeout_ms: Some(250),
            },
        )
        .expect_err("absolute path reads must be rejected");

        assert_eq!(error.code, "validation_error");
        assert!(error.message.contains("relative"));

        let events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: Some("skill_tool".to_string()),
                status: Some("rejected".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("audit log should load");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].target.as_deref(), Some("cat"));
        assert!(events[0].summary.contains("relative"));
    }

    #[test]
    fn runtime_adapter_rejects_cat_traversal_directories_and_oversized_files() {
        let db = Database::in_memory().expect("database should initialize");
        let cwd = unique_runtime_adapter_test_dir();
        let nested = cwd.join("nested");
        let large_file = cwd.join("large.txt");

        fs::create_dir_all(&nested).expect("nested directory should exist");
        fs::write(&large_file, "x".repeat((64 * 1024) + 1))
            .expect("oversized file should be written");

        let traversal = runtime_adapter_execute_skill_tool_for_db(
            &db,
            SkillToolRequest {
                command: "cat".to_string(),
                args: vec!["../escape.txt".to_string()],
                cwd: Some(cwd.display().to_string()),
                timeout_ms: Some(250),
            },
        )
        .expect_err("path traversal must be rejected");
        assert_eq!(traversal.code, "validation_error");
        assert!(traversal.message.contains("traversal"));

        let directory = runtime_adapter_execute_skill_tool_for_db(
            &db,
            SkillToolRequest {
                command: "cat".to_string(),
                args: vec!["nested".to_string()],
                cwd: Some(cwd.display().to_string()),
                timeout_ms: Some(250),
            },
        )
        .expect_err("directories must be rejected");
        assert_eq!(directory.code, "validation_error");
        assert!(directory.message.contains("regular file"));

        let oversized = runtime_adapter_execute_skill_tool_for_db(
            &db,
            SkillToolRequest {
                command: "cat".to_string(),
                args: vec!["large.txt".to_string()],
                cwd: Some(cwd.display().to_string()),
                timeout_ms: Some(250),
            },
        )
        .expect_err("oversized files must be rejected");
        assert_eq!(oversized.code, "validation_error");
        assert!(oversized.message.contains("64 KiB"));

        let events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: Some("skill_tool".to_string()),
                status: Some("rejected".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("audit log should load");
        assert_eq!(events.len(), 3);

        let summaries = events
            .iter()
            .map(|event| event.summary.clone())
            .collect::<HashSet<_>>();
        assert!(
            summaries
                .iter()
                .any(|summary| summary.contains("traversal"))
        );
        assert!(
            summaries
                .iter()
                .any(|summary| summary.contains("regular file"))
        );
        assert!(summaries.iter().any(|summary| summary.contains("64 KiB")));
    }

    #[test]
    fn desktop_adapter_defaults_to_dry_run() {
        let db = Database::in_memory().expect("database should initialize");
        let response = runtime_adapter_execute_desktop_action_for_db(
            &db,
            DesktopActionRequest {
                executor: desktop_executor_name_for_platform().to_string(),
                args: vec!["getactivewindow".to_string()],
                dry_run: None,
                confirmation_phrase: None,
            },
        )
        .expect("default dry run should succeed");

        assert!(!response.executed);
        assert_eq!(
            response.planned_command[0],
            desktop_executor_name_for_platform()
        );
        assert!(response.audit_message.contains("dry-run"));
        assert!(response.exit_code.is_none());
    }

    #[test]
    fn desktop_adapter_rejects_non_dry_run_without_backend_confirmation() {
        let db = Database::in_memory().expect("database should initialize");
        let error = runtime_adapter_execute_desktop_action_for_db(
            &db,
            DesktopActionRequest {
                executor: desktop_executor_name_for_platform().to_string(),
                args: vec!["getactivewindow".to_string()],
                dry_run: Some(false),
                confirmation_phrase: None,
            },
        )
        .expect_err("non-dry-run desktop action should require backend confirmation");

        assert!(error.message.contains("RUN DESKTOP ACTION"));

        let events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: Some("desktop_action".to_string()),
                status: Some("rejected".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("audit log should load");
        assert_eq!(events.len(), 1);
        assert!(events[0].summary.contains("confirmation"));
    }

    #[test]
    fn trajectory_summary_counts_jsonl_kinds() {
        let db = Database::in_memory().expect("database should initialize");
        let summary = runtime_adapter_summarize_trajectory_jsonl_for_db(
            &db,
            TrajectorySummaryRequest {
                jsonl: concat!(
                    "{\"kind\":\"run\",\"source\":\"sim\",\"reward_hint\":1}\n",
                    "{\"kind\":\"run_event\",\"source\":\"sim\"}\n",
                    "{\"kind\":\"run\",\"source\":\"desktop\",\"reward_hint\":true}\n",
                    "not-json\n"
                )
                .to_string(),
            },
        )
        .expect("summary should succeed");

        assert_eq!(summary.line_count, 4);
        assert_eq!(summary.invalid_line_count, 1);
        assert_eq!(summary.reward_hint_count, 2);
        assert_eq!(summary.kind_counts.get("run"), Some(&2));
        assert_eq!(summary.kind_counts.get("run_event"), Some(&1));
        assert_eq!(summary.source_counts.get("sim"), Some(&2));
        assert_eq!(summary.source_counts.get("desktop"), Some(&1));
    }

    #[test]
    fn runtime_adapter_probe_reports_platform_and_common_tools() {
        let probe = runtime_adapter_probe_desktop_executor();

        assert!(!probe.platform.is_empty());
        assert!(probe.tool_availability.contains_key("xdotool"));
        assert!(probe.tool_availability.contains_key("osascript"));
        assert!(probe.tool_availability.contains_key("powershell"));
    }

    #[test]
    fn runtime_adapter_persists_audit_events_for_skill_tool_desktop_action_and_trajectory() {
        let db = Database::in_memory().expect("database should initialize");

        let skill = runtime_adapter_execute_skill_tool_for_db(
            &db,
            SkillToolRequest {
                command: "true".to_string(),
                args: Vec::new(),
                cwd: None,
                timeout_ms: Some(250),
            },
        )
        .expect("skill tool should execute");
        assert_eq!(skill.exit_code, 0);

        let desktop = runtime_adapter_execute_desktop_action_for_db(
            &db,
            DesktopActionRequest {
                executor: desktop_executor_name_for_platform().to_string(),
                args: vec!["getactivewindow".to_string()],
                dry_run: Some(true),
                confirmation_phrase: None,
            },
        )
        .expect("desktop dry run should succeed");
        assert!(!desktop.executed);

        let summary = runtime_adapter_summarize_trajectory_jsonl_for_db(
            &db,
            TrajectorySummaryRequest {
                jsonl: concat!(
                    "{\"kind\":\"run\",\"source\":\"sim\"}\n",
                    "{\"kind\":\"run_event\",\"source\":\"desktop\"}\n"
                )
                .to_string(),
            },
        )
        .expect("trajectory summary should succeed");
        assert_eq!(summary.line_count, 2);

        let events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: None,
                status: None,
                target_remote_user_id: None,
            },
        )
        .expect("audit log should load");

        assert_eq!(events.len(), 3);
        assert_eq!(events[0].kind, "trajectory_summary");
        assert_eq!(events[1].kind, "desktop_action");
        assert_eq!(events[1].status, "dry_run");
        assert_eq!(events[2].kind, "skill_tool");
        assert_eq!(events[2].status, "succeeded");
        assert!(events.iter().all(|event| !event.occurred_at.is_empty()));
        assert!(
            events
                .iter()
                .all(|event| event.summary.len() <= 240 && !event.summary.is_empty())
        );
    }

    #[test]
    fn runtime_adapter_audit_export_can_filter_and_emit_jsonl_without_mutating_log() {
        let db = Database::in_memory().expect("database should initialize");

        runtime_adapter_execute_skill_tool_for_db(
            &db,
            SkillToolRequest {
                command: "true".to_string(),
                args: Vec::new(),
                cwd: None,
                timeout_ms: Some(250),
            },
        )
        .expect("skill tool should execute");
        runtime_adapter_execute_desktop_action_for_db(
            &db,
            DesktopActionRequest {
                executor: desktop_executor_name_for_platform().to_string(),
                args: vec!["getactivewindow".to_string()],
                dry_run: Some(true),
                confirmation_phrase: None,
            },
        )
        .expect("desktop dry run should succeed");

        let exported = export_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditExportRequest {
                limit: Some(1),
                kind: Some("desktop_action".to_string()),
                status: Some("dry_run".to_string()),
                format: Some("jsonl".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("audit export should succeed");

        assert_eq!(exported.total, 1);
        assert_eq!(exported.exported_count, 1);
        assert_eq!(exported.format, "jsonl");
        assert!(exported.payload.contains("\"kind\":\"desktop_action\""));
        assert!(!exported.payload.contains("\"kind\":\"skill_tool\""));

        let events_after_export = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: None,
                status: None,
                target_remote_user_id: None,
            },
        )
        .expect("audit log should still load");
        assert_eq!(events_after_export.len(), 2);
    }

    fn desktop_executor_name_for_platform() -> &'static str {
        #[cfg(target_os = "macos")]
        {
            "osascript"
        }

        #[cfg(target_os = "windows")]
        {
            "powershell"
        }

        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        {
            "xdotool"
        }
    }

    fn unique_runtime_adapter_test_dir() -> std::path::PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos();
        let cwd = std::env::temp_dir().join(format!("runtime-adapter-{unique}"));
        fs::create_dir_all(&cwd).expect("temporary working directory should exist");
        cwd
    }

    #[test]
    fn gui_automation_macro_dry_runs_multiple_desktop_actions_and_audits() {
        let db = Database::in_memory().expect("database should initialize");
        let response = runtime_adapter_run_gui_automation_for_db(
            &db,
            GuiAutomationRequest {
                steps: vec![
                    GuiAutomationStepRequest {
                        label: Some("observe-window".to_string()),
                        executor: desktop_executor_name_for_platform().to_string(),
                        args: vec!["getactivewindow".to_string()],
                    },
                    GuiAutomationStepRequest {
                        label: Some("read-title".to_string()),
                        executor: desktop_executor_name_for_platform().to_string(),
                        args: vec!["getwindowname".to_string(), "%1".to_string()],
                    },
                ],
                dry_run: Some(true),
                confirmation_phrase: None,
                stop_on_error: None,
                target_remote_user_id: None,
            },
        )
        .expect("gui automation dry-run should succeed");

        assert!(!response.executed);
        assert!(response.dry_run);
        assert_eq!(response.step_count, 2);
        assert_eq!(response.completed_count, 2);
        assert_eq!(response.results[0].label.as_deref(), Some("observe-window"));
        assert_eq!(response.planned_commands.len(), 2);
        assert!(response.results.iter().all(|step| !step.response.executed));

        let events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: Some("gui_automation".to_string()),
                status: Some("dry_run".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("audit log should load");
        assert_eq!(events.len(), 1);
        assert!(events[0].summary.contains("2 step"));
    }

    #[test]
    fn gui_automation_macro_trims_remote_user_metadata_in_response_and_audit() {
        let db = Database::in_memory().expect("database should initialize");
        let response = runtime_adapter_run_gui_automation_for_db(
            &db,
            GuiAutomationRequest {
                steps: vec![GuiAutomationStepRequest {
                    label: Some("  observe-window  ".to_string()),
                    executor: desktop_executor_name_for_platform().to_string(),
                    args: vec!["getactivewindow".to_string()],
                }],
                dry_run: Some(true),
                confirmation_phrase: None,
                stop_on_error: None,
                target_remote_user_id: Some("  remote-user-42  ".to_string()),
            },
        )
        .expect("gui automation dry-run should succeed");

        assert_eq!(
            response.target_remote_user_id.as_deref(),
            Some("remote-user-42")
        );

        let events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: Some("gui_automation".to_string()),
                status: Some("dry_run".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("audit log should load");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].target_remote_user_id.as_deref(),
            Some("remote-user-42")
        );

        runtime_adapter_run_gui_automation_for_db(
            &db,
            GuiAutomationRequest {
                steps: vec![GuiAutomationStepRequest {
                    label: Some("other-window".to_string()),
                    executor: desktop_executor_name_for_platform().to_string(),
                    args: vec!["getactivewindow".to_string()],
                }],
                dry_run: Some(true),
                confirmation_phrase: None,
                stop_on_error: None,
                target_remote_user_id: Some("remote-user-99".to_string()),
            },
        )
        .expect("second gui automation dry-run should succeed");

        let filtered_events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: Some(1),
                kind: Some("gui_automation".to_string()),
                status: Some("dry_run".to_string()),
                target_remote_user_id: Some("  remote-user-42  ".to_string()),
            },
        )
        .expect("audit log should filter by target remote user before limit");
        assert_eq!(filtered_events.len(), 1);
        assert_eq!(
            filtered_events[0].target_remote_user_id.as_deref(),
            Some("remote-user-42")
        );

        let unmatched_events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: Some("gui_automation".to_string()),
                status: Some("dry_run".to_string()),
                target_remote_user_id: Some("missing-remote".to_string()),
            },
        )
        .expect("audit log should return empty for unmatched target remote user");
        assert!(unmatched_events.is_empty());
    }

    #[test]
    fn gui_automation_macro_rejects_over_limit_and_audits_rejection() {
        let db = Database::in_memory().expect("database should initialize");
        let steps = (0..=GUI_AUTOMATION_MAX_STEPS)
            .map(|index| GuiAutomationStepRequest {
                label: Some(format!("step-{index}")),
                executor: desktop_executor_name_for_platform().to_string(),
                args: vec!["getactivewindow".to_string()],
            })
            .collect::<Vec<_>>();

        let error = runtime_adapter_run_gui_automation_for_db(
            &db,
            GuiAutomationRequest {
                steps,
                dry_run: Some(true),
                confirmation_phrase: None,
                stop_on_error: None,
                target_remote_user_id: None,
            },
        )
        .expect_err("over-limit macro should be rejected before planning actions");

        assert_eq!(error.code, "validation_error");
        assert!(error.message.contains("at most"));

        let events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: Some("gui_automation".to_string()),
                status: Some("rejected".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("rejection audit log should load");
        assert_eq!(events.len(), 1);
        assert!(events[0].summary.contains("at most"));
    }

    #[test]
    fn gui_automation_macro_rejects_non_dry_run_without_confirmation() {
        let db = Database::in_memory().expect("database should initialize");
        let error = runtime_adapter_run_gui_automation_for_db(
            &db,
            GuiAutomationRequest {
                steps: vec![GuiAutomationStepRequest {
                    label: None,
                    executor: desktop_executor_name_for_platform().to_string(),
                    args: vec!["getactivewindow".to_string()],
                }],
                dry_run: Some(false),
                confirmation_phrase: None,
                stop_on_error: None,
                target_remote_user_id: Some("  remote-user-99  ".to_string()),
            },
        )
        .expect_err("non-dry-run macro should require confirmation");

        assert_eq!(error.code, "validation_error");
        assert!(error.message.contains("RUN DESKTOP ACTION"));

        let events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: Some("gui_automation".to_string()),
                status: Some("rejected".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("rejection audit log should load");
        assert_eq!(events.len(), 1);
        assert_eq!(
            events[0].target_remote_user_id.as_deref(),
            Some("remote-user-99")
        );
    }

    #[test]
    fn gui_automation_macro_stops_after_failed_step_when_requested() {
        let _env_lock = env_lock().lock().expect("env lock should acquire");
        let db = Database::in_memory().expect("database should initialize");
        let bin_dir = unique_runtime_adapter_test_dir();
        install_test_desktop_executor(&bin_dir);
        let _path_override = prepend_path(&bin_dir);

        let response = runtime_adapter_run_gui_automation_for_db(
            &db,
            GuiAutomationRequest {
                steps: vec![
                    GuiAutomationStepRequest {
                        label: Some("fail-first".to_string()),
                        executor: desktop_executor_name_for_platform().to_string(),
                        args: vec!["fail".to_string()],
                    },
                    GuiAutomationStepRequest {
                        label: Some("would-succeed".to_string()),
                        executor: desktop_executor_name_for_platform().to_string(),
                        args: vec!["ok".to_string()],
                    },
                ],
                dry_run: Some(false),
                confirmation_phrase: Some(DESKTOP_ACTION_CONFIRM_PHRASE.to_string()),
                stop_on_error: Some(true),
                target_remote_user_id: None,
            },
        )
        .expect("gui automation should return a failed step result instead of rejecting");

        assert!(response.executed);
        assert!(!response.dry_run);
        assert_eq!(response.step_count, 2);
        assert_eq!(response.completed_count, 1);
        assert_eq!(response.planned_commands.len(), 1);
        assert_eq!(response.results.len(), 1);
        assert_eq!(response.results[0].label.as_deref(), Some("fail-first"));
        assert_eq!(response.results[0].status, "failed");
        assert_eq!(response.results[0].response.exit_code, Some(42));
        assert!(!response.results[0].response.timed_out);

        let gui_events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: Some("gui_automation".to_string()),
                status: Some("failed".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("gui automation failure audit should load");
        assert_eq!(gui_events.len(), 1);
        assert!(gui_events[0].summary.contains("failed"));
        assert!(gui_events[0].summary.contains("1 completed"));

        let failed_step_events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: Some("desktop_action".to_string()),
                status: Some("failed".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("desktop action failure audit should load");
        assert_eq!(failed_step_events.len(), 1);
        assert_eq!(failed_step_events[0].exit_code, Some(42));

        let succeeded_step_events = list_runtime_adapter_audit_events_for_db(
            &db,
            RuntimeAdapterAuditListRequest {
                limit: None,
                kind: Some("desktop_action".to_string()),
                status: Some("succeeded".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("desktop action success audit should load");
        assert!(succeeded_step_events.is_empty());
    }

    #[test]
    fn runtime_adapter_audit_event_deserializes_without_remote_user_metadata() {
        let events = serde_json::from_str::<Vec<RuntimeAdapterAuditEvent>>(
            r#"[{
                "id":"audit-1",
                "occurred_at":"2026-04-29T00:00:00Z",
                "kind":"gui_automation",
                "status":"succeeded",
                "summary":"existing audit event",
                "duration_ms":12,
                "timed_out":false,
                "target":"desktop_macro",
                "exit_code":null
            }]"#,
        )
        .expect("legacy audit JSON should deserialize");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].target_remote_user_id, None);
    }
}
