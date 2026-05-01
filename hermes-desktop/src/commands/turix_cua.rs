use crate::backend::{AppError, AppResult, Database};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tauri::State;
use uuid::Uuid;

const TURIX_CUA_AUDIT_LOG_KEY: &str = "turix_cua.audit_log";
const TURIX_CUA_AUDIT_LOG_LIMIT: usize = 200;
const TURIX_CUA_AUDIT_EXPORT_DEFAULT_LIMIT: usize = 50;
const TURIX_CUA_SUMMARY_MAX_CHARS: usize = 240;
const TURIX_CUA_TEMPLATE_PROJECT_DIR: &str = "your_dir/TuriX-CUA";
const TURIX_CUA_TEMPLATE_API_KEY: &str = "your_api_key_here";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TurixCuaProbeRequest {
    pub repo_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurixCuaConfigSummary {
    pub task_present: bool,
    pub resume: bool,
    pub agent_id_present: bool,
    pub has_template_api_key: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurixCuaProbeResponse {
    pub status: String,
    pub repo_path: String,
    pub repo_exists: bool,
    pub config_path: String,
    pub config_exists: bool,
    pub run_script_path: String,
    pub run_script_exists: bool,
    pub run_script_ready: bool,
    pub python_entry_path: String,
    pub python_entry_exists: bool,
    pub requirements_path: String,
    pub requirements_exists: bool,
    pub permission_hints: Vec<String>,
    pub notes: Vec<String>,
    pub config_summary: Option<TurixCuaConfigSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TurixCuaStartRequest {
    pub repo_path: Option<String>,
    pub task: Option<String>,
    pub resume_agent_id: Option<String>,
    pub config_path: Option<String>,
    pub launcher: Option<String>,
    pub dry_run: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurixCuaPlanResponse {
    pub dry_run: bool,
    pub launcher: String,
    pub repo_path: String,
    pub base_config_path: String,
    pub derived_config_path: Option<String>,
    pub planned_command: Vec<String>,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurixCuaStartResponse {
    pub executed: bool,
    pub dry_run: bool,
    pub launcher: String,
    pub repo_path: String,
    pub base_config_path: String,
    pub derived_config_path: Option<String>,
    pub planned_command: Vec<String>,
    pub pid: Option<u32>,
    pub audit_message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurixCuaAuditEvent {
    pub id: String,
    pub occurred_at: String,
    pub action: String,
    pub status: String,
    pub launcher: String,
    pub dry_run: bool,
    pub summary: String,
    pub repo_path: String,
    pub command: Vec<String>,
    pub resume_agent_id: Option<String>,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TurixCuaAuditListRequest {
    pub limit: Option<usize>,
    pub action: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TurixCuaAuditExportRequest {
    pub limit: Option<usize>,
    pub action: Option<String>,
    pub status: Option<String>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TurixCuaAuditExportResponse {
    pub total: usize,
    pub exported_count: usize,
    pub format: String,
    pub payload: String,
    pub events: Vec<TurixCuaAuditEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TurixCuaLauncher {
    Script,
    Python,
}

impl TurixCuaLauncher {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Script => "script",
            Self::Python => "python",
        }
    }
}

#[derive(Debug, Clone)]
struct TurixCuaRepoLayout {
    repo_path: PathBuf,
    config_path: PathBuf,
    run_script_path: PathBuf,
    python_entry_path: PathBuf,
    requirements_path: PathBuf,
}

#[derive(Debug, Clone)]
struct TurixCuaLaunchPlan {
    dry_run: bool,
    launcher: TurixCuaLauncher,
    repo_path: PathBuf,
    base_config_path: PathBuf,
    derived_config_path: Option<PathBuf>,
    planned_command: Vec<String>,
    notes: Vec<String>,
    task: Option<String>,
    resume_agent_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TurixCuaLaunchOutcome {
    pid: u32,
}

#[tauri::command]
pub fn turix_cua_probe(request: TurixCuaProbeRequest) -> Result<TurixCuaProbeResponse, AppError> {
    turix_cua_probe_for_request(request)
}

#[tauri::command]
pub fn turix_cua_plan_command(
    request: TurixCuaStartRequest,
) -> Result<TurixCuaPlanResponse, AppError> {
    turix_cua_plan_command_for_request(request)
}

#[tauri::command]
pub fn turix_cua_run(
    db: State<'_, Database>,
    request: TurixCuaStartRequest,
) -> Result<TurixCuaStartResponse, AppError> {
    turix_cua_start_run_for_db(db.inner(), request)
}

#[tauri::command]
pub fn turix_cua_list_audit_events(
    db: State<'_, Database>,
    request: TurixCuaAuditListRequest,
) -> Result<Vec<TurixCuaAuditEvent>, AppError> {
    turix_cua_list_audit_for_db(db.inner(), request)
}

#[tauri::command]
pub fn turix_cua_export_audit_events(
    db: State<'_, Database>,
    request: TurixCuaAuditExportRequest,
) -> Result<TurixCuaAuditExportResponse, AppError> {
    turix_cua_export_audit_for_db(db.inner(), request)
}

fn turix_cua_probe_for_request(
    request: TurixCuaProbeRequest,
) -> Result<TurixCuaProbeResponse, AppError> {
    let layout = resolve_repo_layout(request.repo_path.as_deref());
    let repo_exists = layout.repo_path.is_dir();
    let config_exists = layout.config_path.is_file();
    let run_script_exists = layout.run_script_path.is_file();
    let run_script_ready = if run_script_exists {
        is_run_script_ready(&layout.run_script_path)?
    } else {
        false
    };
    let python_entry_exists = layout.python_entry_path.is_file();
    let requirements_exists = layout.requirements_path.is_file();

    let mut notes = Vec::new();
    let config_summary = if !repo_exists {
        notes.push(format!(
            "TuriX-CUA repository is missing at `{}`.",
            layout.repo_path.display()
        ));
        None
    } else if !config_exists {
        notes.push(format!(
            "Base config is missing at `{}`.",
            layout.config_path.display()
        ));
        None
    } else {
        match load_json(&layout.config_path) {
            Ok(config) => {
                let summary = summarize_config(&config);
                if summary.has_template_api_key {
                    notes.push(
                        "examples/config.json still contains template API key values and will need real credentials before a non-dry-run launch can succeed."
                            .to_string(),
                    );
                }
                Some(summary)
            }
            Err(error) => {
                notes.push(format!(
                    "Config exists but could not be parsed: {}",
                    error.message
                ));
                None
            }
        }
    };

    if run_script_exists && !run_script_ready {
        notes.push(
            "OpenClaw run_turix.sh still contains the template project path and is not launch-ready; the bridge will prefer python examples/main.py."
                .to_string(),
        );
    }
    if !python_entry_exists {
        notes.push(format!(
            "Python entrypoint is missing at `{}`.",
            layout.python_entry_path.display()
        ));
    }
    if !requirements_exists {
        notes.push(format!(
            "requirements.txt is missing at `{}`.",
            layout.requirements_path.display()
        ));
    } else {
        notes.push(
            "Prepare a Python 3.12 environment and install `requirements.txt` before a non-dry-run launch."
                .to_string(),
        );
    }
    notes.push(
        "Bridge responses only report local probe/launch state; they do not infer or expose any OSWorld benchmark score."
            .to_string(),
    );

    let status = if !repo_exists {
        "missing"
    } else if config_exists && requirements_exists && python_entry_exists {
        "ready"
    } else {
        "warning"
    };

    Ok(TurixCuaProbeResponse {
        status: status.to_string(),
        repo_path: layout.repo_path.display().to_string(),
        repo_exists,
        config_path: layout.config_path.display().to_string(),
        config_exists,
        run_script_path: layout.run_script_path.display().to_string(),
        run_script_exists,
        run_script_ready,
        python_entry_path: layout.python_entry_path.display().to_string(),
        python_entry_exists,
        requirements_path: layout.requirements_path.display().to_string(),
        requirements_exists,
        permission_hints: permission_hints(),
        notes,
        config_summary,
    })
}

fn turix_cua_plan_command_for_request(
    request: TurixCuaStartRequest,
) -> Result<TurixCuaPlanResponse, AppError> {
    let plan = build_launch_plan(&request)?;
    Ok(plan_response(&plan))
}

fn turix_cua_start_run_for_db(
    db: &Database,
    request: TurixCuaStartRequest,
) -> Result<TurixCuaStartResponse, AppError> {
    turix_cua_start_run_for_db_with_executor(db, request, spawn_launch_plan)
}

fn turix_cua_start_run_for_db_with_executor<F>(
    db: &Database,
    request: TurixCuaStartRequest,
    mut executor: F,
) -> Result<TurixCuaStartResponse, AppError>
where
    F: FnMut(&TurixCuaLaunchPlan) -> AppResult<TurixCuaLaunchOutcome>,
{
    let plan = build_launch_plan(&request)?;
    if plan.dry_run {
        let response = TurixCuaStartResponse {
            executed: false,
            dry_run: true,
            launcher: plan.launcher.as_str().to_string(),
            repo_path: plan.repo_path.display().to_string(),
            base_config_path: plan.base_config_path.display().to_string(),
            derived_config_path: plan
                .derived_config_path
                .as_ref()
                .map(|path| path.display().to_string()),
            planned_command: plan.planned_command.clone(),
            pid: None,
            audit_message: format!(
                "TuriX CUA dry-run preview; planned {} launch was not executed.",
                plan.launcher.as_str()
            ),
        };
        record_audit_event(
            db,
            build_audit_event(
                &plan,
                "start",
                "dry_run",
                None,
                turix_cua_dry_run_summary(&plan),
            ),
        )?;
        return Ok(response);
    }

    materialize_runtime_config(&plan)?;
    let outcome = executor(&plan)?;
    let response = TurixCuaStartResponse {
        executed: true,
        dry_run: false,
        launcher: plan.launcher.as_str().to_string(),
        repo_path: plan.repo_path.display().to_string(),
        base_config_path: plan.base_config_path.display().to_string(),
        derived_config_path: plan
            .derived_config_path
            .as_ref()
            .map(|path| path.display().to_string()),
        planned_command: plan.planned_command.clone(),
        pid: Some(outcome.pid),
        audit_message: format!(
            "TuriX CUA started via {} launcher with pid {}.",
            plan.launcher.as_str(),
            outcome.pid
        ),
    };
    record_audit_event(
        db,
        build_audit_event(
            &plan,
            "start",
            "started",
            Some(outcome.pid),
            turix_cua_started_summary(&plan, outcome.pid),
        ),
    )?;
    Ok(response)
}

fn turix_cua_list_audit_for_db(
    db: &Database,
    request: TurixCuaAuditListRequest,
) -> Result<Vec<TurixCuaAuditEvent>, AppError> {
    let action_filter = normalize_filter(request.action);
    let status_filter = normalize_filter(request.status);
    let limit = normalize_list_limit(request.limit);
    let mut events = load_audit_events(db)?;
    events.sort_by(|left, right| {
        right
            .occurred_at
            .cmp(&left.occurred_at)
            .then_with(|| right.id.cmp(&left.id))
    });
    events.retain(|event| filter_matches(&event.action, action_filter.as_deref()));
    events.retain(|event| filter_matches(&event.status, status_filter.as_deref()));
    if let Some(limit) = limit {
        events.truncate(limit.min(events.len()));
    }
    Ok(events)
}

fn turix_cua_export_audit_for_db(
    db: &Database,
    request: TurixCuaAuditExportRequest,
) -> Result<TurixCuaAuditExportResponse, AppError> {
    let format = normalize_export_format(request.format)?;
    let events = turix_cua_list_audit_for_db(
        db,
        TurixCuaAuditListRequest {
            limit: Some(normalize_export_limit(request.limit)),
            action: request.action.clone(),
            status: request.status.clone(),
        },
    )?;
    let total = turix_cua_list_audit_for_db(
        db,
        TurixCuaAuditListRequest {
            limit: None,
            action: request.action,
            status: request.status,
        },
    )?
    .len();
    let payload = serialize_audit_events(&events, &format)?;
    Ok(TurixCuaAuditExportResponse {
        total,
        exported_count: events.len(),
        format,
        payload,
        events,
    })
}

fn build_launch_plan(request: &TurixCuaStartRequest) -> Result<TurixCuaLaunchPlan, AppError> {
    let task = request
        .task
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let resume_agent_id = request
        .resume_agent_id
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    if task.is_none() && resume_agent_id.is_none() {
        return Err(AppError::validation(
            "TuriX CUA start requires a task or resume_agent_id",
        ));
    }

    let layout = resolve_repo_layout(request.repo_path.as_deref());
    if !layout.repo_path.is_dir() {
        return Err(AppError::validation(format!(
            "TuriX-CUA repository not found at `{}`",
            layout.repo_path.display()
        )));
    }

    let base_config_path = request
        .config_path
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| layout.config_path.clone());
    if !base_config_path.is_file() {
        return Err(AppError::validation(format!(
            "TuriX config not found at `{}`",
            base_config_path.display()
        )));
    }

    let launcher = select_launcher(&layout, request.launcher.as_deref())?;
    let dry_run = request.dry_run.unwrap_or(true);
    let derived_config_path = Some(next_runtime_config_path(&layout.repo_path));
    let planned_command = build_planned_command(
        &layout,
        &launcher,
        derived_config_path
            .as_ref()
            .expect("derived config path should exist"),
        task.as_deref(),
        resume_agent_id.as_deref(),
    );
    let mut notes = Vec::new();
    if matches!(launcher, TurixCuaLauncher::Python) {
        notes.push(
            "Python launcher uses an ephemeral config copy so the checked-in examples/config.json is not mutated."
                .to_string(),
        );
    } else {
        notes.push(
            "Script launcher is only selected when run_turix.sh does not contain the template project path."
                .to_string(),
        );
    }
    notes.push(
        "Bridge responses only report local launch state; they do not infer or expose any OSWorld benchmark score."
            .to_string(),
    );

    Ok(TurixCuaLaunchPlan {
        dry_run,
        launcher,
        repo_path: layout.repo_path,
        base_config_path,
        derived_config_path,
        planned_command,
        notes,
        task,
        resume_agent_id,
    })
}

fn plan_response(plan: &TurixCuaLaunchPlan) -> TurixCuaPlanResponse {
    TurixCuaPlanResponse {
        dry_run: true,
        launcher: plan.launcher.as_str().to_string(),
        repo_path: plan.repo_path.display().to_string(),
        base_config_path: plan.base_config_path.display().to_string(),
        derived_config_path: plan
            .derived_config_path
            .as_ref()
            .map(|path| path.display().to_string()),
        planned_command: plan.planned_command.clone(),
        notes: plan.notes.clone(),
    }
}

fn select_launcher(
    layout: &TurixCuaRepoLayout,
    requested: Option<&str>,
) -> Result<TurixCuaLauncher, AppError> {
    let requested = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_ascii_lowercase());
    let script_exists = layout.run_script_path.is_file();
    let script_ready = script_exists && is_run_script_ready(&layout.run_script_path)?;
    let python_exists = layout.python_entry_path.is_file();

    match requested.as_deref() {
        Some("script") => {
            if !script_exists {
                return Err(AppError::validation(format!(
                    "TuriX run script not found at `{}`",
                    layout.run_script_path.display()
                )));
            }
            if !script_ready {
                return Err(AppError::validation(
                    "TuriX run script exists but still contains the template project path; use launcher `python` instead".to_string(),
                ));
            }
            Ok(TurixCuaLauncher::Script)
        }
        Some("python") => {
            if !python_exists {
                return Err(AppError::validation(format!(
                    "TuriX python entrypoint not found at `{}`",
                    layout.python_entry_path.display()
                )));
            }
            Ok(TurixCuaLauncher::Python)
        }
        Some(other) => Err(AppError::validation(format!(
            "unsupported TuriX launcher `{}`; expected `script` or `python`",
            other
        ))),
        None => {
            if script_ready {
                Ok(TurixCuaLauncher::Script)
            } else if python_exists {
                Ok(TurixCuaLauncher::Python)
            } else if script_exists {
                Err(AppError::validation(
                    "TuriX run script exists but is not launch-ready, and python entrypoint is missing"
                        .to_string(),
                ))
            } else {
                Err(AppError::validation(format!(
                    "No launchable TuriX entrypoint found under `{}`",
                    layout.repo_path.display()
                )))
            }
        }
    }
}

fn build_planned_command(
    layout: &TurixCuaRepoLayout,
    launcher: &TurixCuaLauncher,
    derived_config_path: &Path,
    task: Option<&str>,
    resume_agent_id: Option<&str>,
) -> Vec<String> {
    match launcher {
        TurixCuaLauncher::Script => {
            let mut command = vec![
                "bash".to_string(),
                layout.run_script_path.display().to_string(),
                "--config".to_string(),
                derived_config_path.display().to_string(),
            ];
            if let Some(resume_agent_id) = resume_agent_id {
                command.push("--resume".to_string());
                command.push(resume_agent_id.to_string());
            }
            if let Some(task) = task {
                command.push(task.to_string());
            }
            command
        }
        TurixCuaLauncher::Python => vec![
            "python3".to_string(),
            layout.python_entry_path.display().to_string(),
            "-c".to_string(),
            derived_config_path.display().to_string(),
        ],
    }
}

fn materialize_runtime_config(plan: &TurixCuaLaunchPlan) -> Result<(), AppError> {
    let Some(derived_config_path) = plan.derived_config_path.as_ref() else {
        return Ok(());
    };
    let mut config = load_json(&plan.base_config_path)?;
    apply_config_overrides(&mut config, plan)?;
    if let Some(parent) = derived_config_path.parent() {
        fs::create_dir_all(parent).map_err(AppError::from_io_error)?;
    }
    let json = serde_json::to_string_pretty(&config).map_err(AppError::from_json_error)?;
    fs::write(derived_config_path, json).map_err(AppError::from_io_error)?;
    Ok(())
}

fn apply_config_overrides(config: &mut Value, plan: &TurixCuaLaunchPlan) -> Result<(), AppError> {
    let root = config
        .as_object_mut()
        .ok_or_else(|| AppError::validation("TuriX config root must be a JSON object"))?;
    let agent = ensure_object(root, "agent")?;

    if let Some(task) = plan.task.as_ref() {
        agent.insert("task".to_string(), Value::String(task.clone()));
    }
    if let Some(resume_agent_id) = plan.resume_agent_id.as_ref() {
        agent.insert("resume".to_string(), Value::Bool(true));
        agent.insert(
            "agent_id".to_string(),
            Value::String(resume_agent_id.clone()),
        );
    } else {
        agent.insert("resume".to_string(), Value::Bool(false));
    }

    Ok(())
}

fn ensure_object<'a>(
    root: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>, AppError> {
    let value = root
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value
        .as_object_mut()
        .ok_or_else(|| AppError::validation(format!("`{}` must be a JSON object", key)))
}

fn spawn_launch_plan(plan: &TurixCuaLaunchPlan) -> Result<TurixCuaLaunchOutcome, AppError> {
    let (program, args) = plan
        .planned_command
        .split_first()
        .ok_or_else(|| AppError::runtime("planned command is empty"))?;
    let mut command = Command::new(program);
    command.args(args);
    command.current_dir(&plan.repo_path);
    command.stdin(Stdio::null());
    command.stdout(Stdio::null());
    command.stderr(Stdio::null());
    let child = command.spawn().map_err(|error| {
        AppError::runtime(format!(
            "failed to start TuriX launcher `{}`: {}",
            program, error
        ))
    })?;
    Ok(TurixCuaLaunchOutcome { pid: child.id() })
}

fn resolve_repo_layout(repo_override: Option<&str>) -> TurixCuaRepoLayout {
    let repo_path = repo_override
        .map(PathBuf::from)
        .unwrap_or_else(default_repo_path);
    TurixCuaRepoLayout {
        config_path: repo_path.join("examples/config.json"),
        run_script_path: repo_path.join("OpenCLaw_TuriX_skill/scripts/run_turix.sh"),
        python_entry_path: repo_path.join("examples/main.py"),
        requirements_path: repo_path.join("requirements.txt"),
        repo_path,
    }
}

fn default_repo_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../repos/TuriX-CUA")
}

fn next_runtime_config_path(repo_path: &Path) -> PathBuf {
    repo_path
        .join(".turix_tmp/hermes_desktop_bridge")
        .join(format!(
            "config-{}-{}.json",
            Utc::now().format("%Y%m%dT%H%M%S"),
            Uuid::new_v4()
        ))
}

fn load_json(path: &Path) -> Result<Value, AppError> {
    let raw = fs::read_to_string(path).map_err(AppError::from_io_error)?;
    serde_json::from_str(&raw).map_err(AppError::from_json_error)
}

fn summarize_config(config: &Value) -> TurixCuaConfigSummary {
    let agent = config.get("agent");
    let task_present = agent
        .and_then(|value| value.get("task"))
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    let resume = agent
        .and_then(|value| value.get("resume"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let agent_id_present = agent
        .and_then(|value| value.get("agent_id"))
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    TurixCuaConfigSummary {
        task_present,
        resume,
        agent_id_present,
        has_template_api_key: has_template_api_key(config),
    }
}

fn has_template_api_key(config: &Value) -> bool {
    ["brain_llm", "actor_llm", "memory_llm", "planner_llm"]
        .into_iter()
        .filter_map(|key| config.get(key))
        .filter_map(|value| value.get("api_key"))
        .filter_map(Value::as_str)
        .any(|api_key| api_key.eq_ignore_ascii_case(TURIX_CUA_TEMPLATE_API_KEY))
}

fn is_run_script_ready(path: &Path) -> Result<bool, AppError> {
    let script = fs::read_to_string(path).map_err(AppError::from_io_error)?;
    Ok(!script.contains(TURIX_CUA_TEMPLATE_PROJECT_DIR))
}

fn permission_hints() -> Vec<String> {
    vec![
        "Grant Screen Recording to Terminal/IDE and, if needed, `/usr/bin/python3` before a non-dry-run launch.".to_string(),
        "Grant Accessibility to Terminal/IDE and the Python process used to run TuriX.".to_string(),
        "If Safari automation is part of the task, enable Safari Remote Automation and JavaScript from Apple Events.".to_string(),
    ]
}

fn build_audit_event(
    plan: &TurixCuaLaunchPlan,
    action: &str,
    status: &str,
    pid: Option<u32>,
    summary: String,
) -> TurixCuaAuditEvent {
    TurixCuaAuditEvent {
        id: Uuid::new_v4().to_string(),
        occurred_at: Utc::now().to_rfc3339(),
        action: action.to_string(),
        status: status.to_string(),
        launcher: plan.launcher.as_str().to_string(),
        dry_run: plan.dry_run,
        summary: truncate_summary(summary),
        repo_path: plan.repo_path.display().to_string(),
        command: plan.planned_command.clone(),
        resume_agent_id: plan.resume_agent_id.clone(),
        pid,
    }
}

fn turix_cua_dry_run_summary(plan: &TurixCuaLaunchPlan) -> String {
    if let Some(resume_agent_id) = plan.resume_agent_id.as_deref() {
        format!(
            "TuriX CUA dry-run planned {} resume for agent `{}` with command `{}`",
            plan.launcher.as_str(),
            resume_agent_id,
            plan.planned_command.join(" ")
        )
    } else {
        format!(
            "TuriX CUA dry-run planned {} run with command `{}`",
            plan.launcher.as_str(),
            plan.planned_command.join(" ")
        )
    }
}

fn turix_cua_started_summary(plan: &TurixCuaLaunchPlan, pid: u32) -> String {
    format!(
        "TuriX CUA started via {} launcher with pid {} using `{}`",
        plan.launcher.as_str(),
        pid,
        plan.planned_command.join(" ")
    )
}

fn load_audit_events(db: &Database) -> Result<Vec<TurixCuaAuditEvent>, AppError> {
    let stored = db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&TURIX_CUA_AUDIT_LOG_KEY],
        |row| row.get::<_, String>(0),
    );

    match stored {
        Ok(value_json) => serde_json::from_str::<Vec<TurixCuaAuditEvent>>(&value_json)
            .map_err(AppError::from_json_error),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Vec::new()),
        Err(error) => Err(AppError::storage(format!(
            "Failed to load TuriX CUA audit log: {}",
            error
        ))),
    }
}

fn record_audit_event(db: &Database, event: TurixCuaAuditEvent) -> Result<(), AppError> {
    let mut log = load_audit_events(db)?;
    log.insert(0, event);
    log.truncate(TURIX_CUA_AUDIT_LOG_LIMIT);
    let value_json = serde_json::to_string(&log).map_err(AppError::from_json_error)?;
    let now = Utc::now().to_rfc3339();
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
        &[&TURIX_CUA_AUDIT_LOG_KEY, &value_json, &now],
    )?;
    Ok(())
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

fn normalize_list_limit(limit: Option<usize>) -> Option<usize> {
    match limit {
        Some(0) | None => None,
        Some(limit) => Some(limit.min(TURIX_CUA_AUDIT_LOG_LIMIT)),
    }
}

fn normalize_export_limit(limit: Option<usize>) -> usize {
    match limit {
        Some(0) | None => TURIX_CUA_AUDIT_EXPORT_DEFAULT_LIMIT,
        Some(limit) => limit.min(TURIX_CUA_AUDIT_LOG_LIMIT),
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
        "unsupported TuriX CUA audit export format `{}`",
        normalized
    )))
}

fn serialize_audit_events(events: &[TurixCuaAuditEvent], format: &str) -> Result<String, AppError> {
    match format {
        "json" => serde_json::to_string_pretty(events).map_err(AppError::from_json_error),
        "jsonl" => events
            .iter()
            .map(|event| serde_json::to_string(event).map_err(AppError::from_json_error))
            .collect::<Result<Vec<_>, _>>()
            .map(|lines| lines.join("\n")),
        _ => Err(AppError::validation(format!(
            "unsupported TuriX CUA audit export format `{}`",
            format
        ))),
    }
}

fn truncate_summary(value: String) -> String {
    let char_count = value.chars().count();
    if char_count <= TURIX_CUA_SUMMARY_MAX_CHARS {
        return value;
    }

    let mut truncated = value
        .chars()
        .take(TURIX_CUA_SUMMARY_MAX_CHARS.saturating_sub(3))
        .collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::{
        TURIX_CUA_AUDIT_LOG_KEY, TurixCuaAuditExportRequest, TurixCuaAuditListRequest,
        TurixCuaLaunchOutcome, TurixCuaProbeRequest, TurixCuaStartRequest,
        turix_cua_export_audit_for_db, turix_cua_list_audit_for_db,
        turix_cua_plan_command_for_request, turix_cua_probe_for_request,
        turix_cua_start_run_for_db_with_executor,
    };
    use crate::backend::Database;
    use serde_json::Value;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn turix_cua_dry_run_does_not_execute_and_records_audit() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = create_ready_repo_fixture();
        let executed = Arc::new(AtomicBool::new(false));
        let executed_flag = Arc::clone(&executed);

        let response = turix_cua_start_run_for_db_with_executor(
            &db,
            TurixCuaStartRequest {
                repo_path: Some(repo.display().to_string()),
                task: Some("Open Chrome".to_string()),
                resume_agent_id: None,
                config_path: None,
                launcher: Some("python".to_string()),
                dry_run: Some(true),
            },
            move |_| {
                executed_flag.store(true, Ordering::SeqCst);
                Ok(TurixCuaLaunchOutcome { pid: 77 })
            },
        )
        .expect("dry-run should succeed");

        assert!(!response.executed);
        assert!(response.dry_run);
        assert!(!executed.load(Ordering::SeqCst));

        let audit = turix_cua_list_audit_for_db(
            &db,
            TurixCuaAuditListRequest {
                limit: None,
                action: Some("start".to_string()),
                status: Some("dry_run".to_string()),
            },
        )
        .expect("dry-run audit should persist");
        assert_eq!(audit.len(), 1);
        assert_eq!(
            audit[0].command.first().map(String::as_str),
            Some("python3")
        );
    }

    #[test]
    fn turix_cua_probe_reports_missing_repo() {
        let unique = unique_suffix();
        let missing = std::env::temp_dir().join(format!("turix-missing-{unique}"));
        let response = turix_cua_probe_for_request(TurixCuaProbeRequest {
            repo_path: Some(missing.display().to_string()),
        })
        .expect("probe should return a structured missing response");

        assert_eq!(response.status, "missing");
        assert!(!response.repo_exists);
        assert!(response.config_summary.is_none());
    }

    #[test]
    fn turix_cua_audit_round_trips_through_app_settings() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = create_ready_repo_fixture();

        turix_cua_start_run_for_db_with_executor(
            &db,
            TurixCuaStartRequest {
                repo_path: Some(repo.display().to_string()),
                task: Some("Draft a note".to_string()),
                resume_agent_id: None,
                config_path: None,
                launcher: Some("python".to_string()),
                dry_run: Some(true),
            },
            |_| Ok(TurixCuaLaunchOutcome { pid: 88 }),
        )
        .expect("dry-run should persist audit");

        let stored = db
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = ?1",
                &[&TURIX_CUA_AUDIT_LOG_KEY],
                |row| row.get::<_, String>(0),
            )
            .expect("audit setting should be stored");
        let stored_json: Value = serde_json::from_str(&stored).expect("audit JSON should parse");
        assert_eq!(stored_json.as_array().map(Vec::len), Some(1));

        let export = turix_cua_export_audit_for_db(
            &db,
            TurixCuaAuditExportRequest {
                limit: Some(10),
                action: None,
                status: None,
                format: Some("jsonl".to_string()),
            },
        )
        .expect("audit export should succeed");
        assert_eq!(export.total, 1);
        assert_eq!(export.exported_count, 1);
        assert!(export.payload.contains("\"action\":\"start\""));
    }

    #[test]
    fn turix_cua_plan_includes_resume_parameters_for_ready_script_repo() {
        let repo = create_ready_repo_fixture();

        let response = turix_cua_plan_command_for_request(TurixCuaStartRequest {
            repo_path: Some(repo.display().to_string()),
            task: Some("Continue task".to_string()),
            resume_agent_id: Some("agent-42".to_string()),
            config_path: None,
            launcher: Some("script".to_string()),
            dry_run: None,
        })
        .expect("plan should succeed");

        assert_eq!(response.launcher, "script");
        assert!(
            response
                .planned_command
                .iter()
                .any(|part| part == "--resume")
        );
        assert!(
            response
                .planned_command
                .iter()
                .any(|part| part == "agent-42")
        );
    }

    fn create_ready_repo_fixture() -> PathBuf {
        let root = unique_test_dir("turix-ready");
        let examples = root.join("examples");
        let skill_scripts = root.join("OpenCLaw_TuriX_skill/scripts");
        fs::create_dir_all(&examples).expect("examples directory should exist");
        fs::create_dir_all(&skill_scripts).expect("skill scripts directory should exist");

        fs::write(
            root.join("requirements.txt"),
            "pyobjc>=11.0.0\npynput\nplaywright>=1.49.0\n",
        )
        .expect("requirements should write");
        fs::write(
            examples.join("main.py"),
            "import argparse\nparser=argparse.ArgumentParser()\nparser.add_argument('-c','--config')\n",
        )
        .expect("python entry should write");
        fs::write(
            examples.join("config.json"),
            r#"{
  "brain_llm": { "api_key": "your_api_key_here" },
  "actor_llm": { "api_key": "your_api_key_here" },
  "memory_llm": { "api_key": "your_api_key_here" },
  "agent": {
    "task": "fixture task",
    "resume": false,
    "agent_id": null
  }
}"#,
        )
        .expect("config should write");
        fs::write(
            skill_scripts.join("run_turix.sh"),
            "#!/bin/bash\nset -e\npython3 examples/main.py -c examples/config.json\n",
        )
        .expect("script should write");

        root
    }

    fn unique_test_dir(prefix: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("{prefix}-{}", unique_suffix()));
        fs::create_dir_all(&dir).expect("test directory should create");
        dir
    }

    fn unique_suffix() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after unix epoch")
            .as_nanos()
    }

    #[allow(dead_code)]
    fn _assert_path(_: &Path) {}
}
