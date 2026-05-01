//! Hermes parity 命令

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::backend::{
    AppError, Database, McpHandshakeSynthesisInput, McpStaticEvidenceStatus, ParityCatalog,
    ParityCronJob, ParityCronJobInput, ParityCronRuntimeService, ParityCronRuntimeStatus,
    ParityCronRuntimeTickResult, ParityMcpProbeResult, ParityMcpRuntimeManager, ParityMcpServer,
    ParityMcpServerInput, ParityMcpServerRuntimeStatus, ParityProviderSelection,
    ParityProviderSelectionInput, ParityQuickCommand, ParityQuickCommandInput,
    ParityRuntimeAuthSource, ParityRuntimeReadiness, ParityService, ParityServiceImpl,
    ParityToolset, ParityToolsetInput, ProviderSecretSourceInputs, build_probe_evidence,
    default_command_available, load_config, provider_secret_env_var,
    resolve_provider_secret_sources, synthesize_handshake_evidence,
};

pub use crate::backend::ParityToolMetadata;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParitySaveProviderSelectionRequest {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityToolsetSaveRequest {
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
pub struct ParityCronCreateRequest {
    pub name: String,
    pub schedule: String,
    pub prompt: String,
    #[serde(default)]
    pub deliver_to: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityCronSetEnabledRequest {
    pub id: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityCronRunNowRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityMcpUpsertRequest {
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
pub struct ParityMcpRuntimeCommandRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityQuickCommandSaveRequest {
    #[serde(default)]
    pub id: Option<String>,
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub description: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityMcpProbeRequest {
    pub id: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
struct PersistedRuntimeSettings {
    provider: Option<String>,
    model: Option<String>,
    base_url: Option<String>,
    api_key_ref: Option<String>,
}

fn service(db: &Database) -> ParityServiceImpl {
    ParityServiceImpl::new(db.clone())
}

pub fn parity_get_catalog_for_db(db: &Database) -> Result<ParityCatalog, AppError> {
    service(db).get_catalog()
}

#[tauri::command]
pub fn parity_get_catalog(db: State<'_, Database>) -> Result<ParityCatalog, AppError> {
    parity_get_catalog_for_db(db.inner())
}

pub fn parity_save_provider_selection_for_db(
    db: &Database,
    request: ParitySaveProviderSelectionRequest,
) -> Result<ParityProviderSelection, AppError> {
    service(db).save_provider_selection(ParityProviderSelectionInput {
        provider: request.provider,
        model: request.model,
        base_url: request.base_url,
    })
}

#[tauri::command]
pub fn parity_save_provider_selection(
    db: State<'_, Database>,
    request: ParitySaveProviderSelectionRequest,
) -> Result<ParityProviderSelection, AppError> {
    parity_save_provider_selection_for_db(db.inner(), request)
}

pub fn parity_get_runtime_readiness_for_db(
    db: &Database,
) -> Result<ParityRuntimeReadiness, AppError> {
    let settings = load_runtime_settings(db)?;
    let provider = settings.provider.unwrap_or_else(|| "openai".to_string());
    let model = settings.model.unwrap_or_else(|| "gpt-4o".to_string());
    let base_url = normalize_optional_text(settings.base_url);
    let api_key_ref = normalize_optional_text(settings.api_key_ref);
    let config_api_key = load_config()
        .ok()
        .and_then(|config| normalize_optional_text(config.api_key));
    let api_key_ref_configured = api_key_ref.is_some();
    let uses_custom_endpoint = base_url.is_some();
    let secret_resolution = resolve_provider_secret_sources(
        ProviderSecretSourceInputs::new(
            &provider,
            api_key_ref.as_deref(),
            config_api_key.as_deref(),
        ),
        env_var_has_value,
    );
    let mut sources = secret_resolution
        .sources
        .iter()
        .map(|source| ParityRuntimeAuthSource {
            kind: source.kind.as_str().to_string(),
            label: source.label.to_string(),
            env_var: source.env_var.clone(),
            available: source.available,
        })
        .collect::<Vec<_>>();
    if sources.is_empty() {
        sources.push(ParityRuntimeAuthSource {
            kind: "none".to_string(),
            label: "No auth source configured".to_string(),
            env_var: None,
            available: false,
        });
    }
    let auth = if let Some(source) = secret_resolution.selected_source() {
        ParityRuntimeAuthSource {
            kind: source.kind.as_str().to_string(),
            label: source.label.to_string(),
            env_var: source
                .env_var
                .clone()
                .or_else(|| provider_secret_env_var_hint(&provider)),
            available: source.available,
        }
    } else if let Some(reference) = api_key_ref.as_deref() {
        ParityRuntimeAuthSource {
            kind: "runtime_api_key_ref".to_string(),
            label: "Runtime API key reference".to_string(),
            env_var: Some(reference.to_string()),
            available: false,
        }
    } else {
        ParityRuntimeAuthSource {
            kind: "none".to_string(),
            label: "No auth source configured".to_string(),
            env_var: provider_secret_env_var_hint(&provider),
            available: false,
        }
    };
    let can_authenticate = auth.available;
    let status = if can_authenticate {
        "ready".to_string()
    } else {
        "missing_api_key".to_string()
    };
    let message = build_runtime_readiness_message(
        &provider,
        uses_custom_endpoint,
        api_key_ref.as_deref(),
        &auth,
    );

    Ok(ParityRuntimeReadiness {
        provider,
        model,
        base_url,
        api_key_ref,
        api_key_ref_configured,
        uses_custom_endpoint,
        can_authenticate,
        auth,
        sources,
        status,
        message,
    })
}

#[tauri::command]
pub fn parity_get_runtime_readiness(
    db: State<'_, Database>,
) -> Result<ParityRuntimeReadiness, AppError> {
    parity_get_runtime_readiness_for_db(db.inner())
}

pub fn parity_toolset_list_for_db(db: &Database) -> Result<Vec<ParityToolset>, AppError> {
    service(db).list_toolsets()
}

#[tauri::command]
pub fn parity_toolset_list(db: State<'_, Database>) -> Result<Vec<ParityToolset>, AppError> {
    parity_toolset_list_for_db(db.inner())
}

pub fn parity_toolset_save_for_db(
    db: &Database,
    request: ParityToolsetSaveRequest,
) -> Result<ParityToolset, AppError> {
    service(db).save_toolset(ParityToolsetInput {
        id: request.id,
        name: request.name,
        description: request.description,
        enabled: request.enabled,
        source: request.source,
        tools: request.tools,
    })
}

#[tauri::command]
pub fn parity_toolset_save(
    db: State<'_, Database>,
    request: ParityToolsetSaveRequest,
) -> Result<ParityToolset, AppError> {
    parity_toolset_save_for_db(db.inner(), request)
}

pub fn parity_cron_list_for_db(db: &Database) -> Result<Vec<ParityCronJob>, AppError> {
    service(db).list_cron_jobs()
}

#[tauri::command]
pub fn parity_cron_list(db: State<'_, Database>) -> Result<Vec<ParityCronJob>, AppError> {
    parity_cron_list_for_db(db.inner())
}

pub fn parity_cron_create_for_db(
    db: &Database,
    request: ParityCronCreateRequest,
) -> Result<ParityCronJob, AppError> {
    service(db).create_cron_job(ParityCronJobInput {
        name: request.name,
        schedule: request.schedule,
        prompt: request.prompt,
        deliver_to: request.deliver_to,
        enabled: request.enabled,
    })
}

#[tauri::command]
pub fn parity_cron_create(
    db: State<'_, Database>,
    request: ParityCronCreateRequest,
) -> Result<ParityCronJob, AppError> {
    parity_cron_create_for_db(db.inner(), request)
}

pub fn parity_cron_set_enabled_for_db(
    db: &Database,
    request: ParityCronSetEnabledRequest,
) -> Result<ParityCronJob, AppError> {
    service(db).set_cron_job_enabled(&request.id, request.enabled)
}

#[tauri::command]
pub fn parity_cron_set_enabled(
    db: State<'_, Database>,
    request: ParityCronSetEnabledRequest,
) -> Result<ParityCronJob, AppError> {
    parity_cron_set_enabled_for_db(db.inner(), request)
}

pub fn parity_cron_run_now_for_db(
    db: &Database,
    request: ParityCronRunNowRequest,
) -> Result<ParityCronJob, AppError> {
    let job_id = request.id.trim().to_string();
    let requested = service(db).run_cron_job_now(&job_id)?;
    let runtime = ParityCronRuntimeService::new(db.clone());
    let _ = runtime.dispatch_requested_job(&job_id)?;

    service(db)
        .list_cron_jobs()?
        .into_iter()
        .find(|job| job.id == job_id)
        .or(Some(requested))
        .ok_or_else(|| AppError::validation("cron job not found"))
}

#[tauri::command]
pub fn parity_cron_run_now(
    db: State<'_, Database>,
    request: ParityCronRunNowRequest,
) -> Result<ParityCronJob, AppError> {
    parity_cron_run_now_for_db(db.inner(), request)
}

pub fn parity_cron_runtime_status_for_db(
    db: &Database,
) -> Result<ParityCronRuntimeStatus, AppError> {
    ParityCronRuntimeService::new(db.clone()).status()
}

#[tauri::command]
pub fn parity_cron_runtime_status(
    db: State<'_, Database>,
) -> Result<ParityCronRuntimeStatus, AppError> {
    parity_cron_runtime_status_for_db(db.inner())
}

pub fn parity_cron_runtime_tick_for_db(
    db: &Database,
) -> Result<ParityCronRuntimeTickResult, AppError> {
    ParityCronRuntimeService::new(db.clone()).poll_once()
}

#[tauri::command]
pub fn parity_cron_runtime_tick(
    db: State<'_, Database>,
) -> Result<ParityCronRuntimeTickResult, AppError> {
    parity_cron_runtime_tick_for_db(db.inner())
}

pub fn parity_mcp_list_for_db(db: &Database) -> Result<Vec<ParityMcpServer>, AppError> {
    service(db).list_mcp_servers()
}

#[tauri::command]
pub fn parity_mcp_list(db: State<'_, Database>) -> Result<Vec<ParityMcpServer>, AppError> {
    parity_mcp_list_for_db(db.inner())
}

pub fn parity_mcp_upsert_for_db(
    db: &Database,
    request: ParityMcpUpsertRequest,
) -> Result<ParityMcpServer, AppError> {
    service(db).upsert_mcp_server(ParityMcpServerInput {
        id: request.id,
        name: request.name,
        transport: request.transport,
        endpoint: request.endpoint,
        enabled: request.enabled,
        tool_filter_mode: request.tool_filter_mode,
        allowed_tools: request.allowed_tools,
        blocked_tools: request.blocked_tools,
        resources_enabled: request.resources_enabled,
        prompts_enabled: request.prompts_enabled,
    })
}

#[tauri::command]
pub fn parity_mcp_upsert(
    db: State<'_, Database>,
    request: ParityMcpUpsertRequest,
) -> Result<ParityMcpServer, AppError> {
    parity_mcp_upsert_for_db(db.inner(), request)
}

pub fn parity_mcp_runtime_list_status_for_db(
    db: &Database,
    runtime: &ParityMcpRuntimeManager,
) -> Result<Vec<ParityMcpServerRuntimeStatus>, AppError> {
    let service = service(db);
    let statuses = service.list_mcp_server_runtime_statuses()?;
    let mut reconciled = Vec::with_capacity(statuses.len());

    for status in statuses {
        reconciled.push(reconcile_mcp_runtime_status(&service, runtime, &status.id)?);
    }

    Ok(reconciled)
}

#[tauri::command]
pub fn parity_mcp_runtime_list_status(
    db: State<'_, Database>,
    runtime: State<'_, ParityMcpRuntimeManager>,
) -> Result<Vec<ParityMcpServerRuntimeStatus>, AppError> {
    parity_mcp_runtime_list_status_for_db(db.inner(), runtime.inner())
}

pub fn parity_mcp_probe_for_db(
    db: &Database,
    request: ParityMcpProbeRequest,
) -> Result<ParityMcpProbeResult, AppError> {
    parity_mcp_probe_for_db_with_checker(db, request, default_command_available)
}

pub fn parity_mcp_probe_for_db_with_checker<F>(
    db: &Database,
    request: ParityMcpProbeRequest,
    command_available: F,
) -> Result<ParityMcpProbeResult, AppError>
where
    F: Fn(&str) -> bool,
{
    let id = request.id.trim();
    if id.is_empty() {
        return Err(AppError::validation("mcp id is required"));
    }
    let parity_service = service(db);
    let status = parity_service.get_mcp_server_runtime_status(id)?;
    let server = parity_service
        .list_mcp_servers()?
        .into_iter()
        .find(|server| server.id == status.id)
        .ok_or_else(|| {
            AppError::storage(format!(
                "Failed to load parity MCP server {} for probe evidence",
                status.id
            ))
        })?;
    let evidence = build_probe_evidence(&status.transport, &status.endpoint, command_available);
    let handshake = synthesize_handshake_evidence(McpHandshakeSynthesisInput {
        transport: &status.transport,
        endpoint: &status.endpoint,
        static_status: match evidence.status {
            crate::backend::McpProbeStatus::Ready => McpStaticEvidenceStatus::Ready,
            crate::backend::McpProbeStatus::Error => McpStaticEvidenceStatus::Error,
            crate::backend::McpProbeStatus::Warning => McpStaticEvidenceStatus::Warning,
        },
        command_available: evidence.command_available,
        url_valid: evidence.url_valid,
        detail: evidence.endpoint_detail.as_deref(),
    });

    Ok(ParityMcpProbeResult {
        id: status.id,
        name: status.name,
        transport: status.transport,
        endpoint: status.endpoint,
        management_mode: status.management_mode,
        tool_filter_mode: server.tool_filter_mode,
        allowed_tool_count: server.allowed_tools.len(),
        blocked_tool_count: server.blocked_tools.len(),
        resources_enabled: server.resources_enabled,
        prompts_enabled: server.prompts_enabled,
        handshake_status: handshake.status.as_str().to_string(),
        handshake_reason: handshake.reason,
        status: evidence.status.as_str().to_string(),
        message: evidence.message,
        command_available: evidence.command_available,
        url_valid: evidence.url_valid,
        parsed_command: evidence.parsed_command,
        parsed_args: evidence.parsed_args,
        endpoint_scheme: evidence.endpoint_scheme,
        endpoint_host: evidence.endpoint_host,
        endpoint_detail: evidence.endpoint_detail,
    })
}

#[tauri::command]
pub fn parity_mcp_probe(
    db: State<'_, Database>,
    request: ParityMcpProbeRequest,
) -> Result<ParityMcpProbeResult, AppError> {
    parity_mcp_probe_for_db(db.inner(), request)
}

pub fn parity_mcp_runtime_start_for_db(
    db: &Database,
    runtime: &ParityMcpRuntimeManager,
    request: ParityMcpRuntimeCommandRequest,
) -> Result<ParityMcpServerRuntimeStatus, AppError> {
    let service = service(db);
    let status = service.get_mcp_server_runtime_status(request.id.trim())?;
    let mut state = service.get_mcp_runtime_state(&status.id)?;
    let now = Utc::now().to_rfc3339();

    if status.management_mode == "process" {
        let pid = runtime.start_process(&status.id, &status.endpoint)?;
        state.runtime_status = "running".to_string();
        state.pid = Some(pid);
        state.last_started_at = Some(now.clone());
        state.last_error = None;
        state.status_message = None;
    } else {
        state.runtime_status = "external".to_string();
        state.pid = None;
        state.last_started_at = Some(now.clone());
    }

    state.updated_at = now;
    service.save_mcp_runtime_state(state)?;
    reconcile_mcp_runtime_status(&service, runtime, &status.id)
}

#[tauri::command]
pub fn parity_mcp_runtime_start(
    db: State<'_, Database>,
    runtime: State<'_, ParityMcpRuntimeManager>,
    request: ParityMcpRuntimeCommandRequest,
) -> Result<ParityMcpServerRuntimeStatus, AppError> {
    parity_mcp_runtime_start_for_db(db.inner(), runtime.inner(), request)
}

pub fn parity_mcp_runtime_stop_for_db(
    db: &Database,
    runtime: &ParityMcpRuntimeManager,
    request: ParityMcpRuntimeCommandRequest,
) -> Result<ParityMcpServerRuntimeStatus, AppError> {
    let service = service(db);
    let status = service.get_mcp_server_runtime_status(request.id.trim())?;
    let mut state = service.get_mcp_runtime_state(&status.id)?;
    let now = Utc::now().to_rfc3339();

    if status.management_mode == "process" {
        let exit_code = runtime.stop_process(&status.id, status.pid)?;
        state.runtime_status = "stopped".to_string();
        state.pid = None;
        state.last_exit_code = exit_code;
        state.status_message = None;
    } else {
        state.runtime_status = "external".to_string();
        state.pid = None;
    }

    state.last_stopped_at = Some(now.clone());
    state.last_error = None;
    state.updated_at = now;
    service.save_mcp_runtime_state(state)?;
    reconcile_mcp_runtime_status(&service, runtime, &status.id)
}

#[tauri::command]
pub fn parity_mcp_runtime_stop(
    db: State<'_, Database>,
    runtime: State<'_, ParityMcpRuntimeManager>,
    request: ParityMcpRuntimeCommandRequest,
) -> Result<ParityMcpServerRuntimeStatus, AppError> {
    parity_mcp_runtime_stop_for_db(db.inner(), runtime.inner(), request)
}

pub fn parity_mcp_runtime_reload_for_db(
    db: &Database,
    runtime: &ParityMcpRuntimeManager,
    request: ParityMcpRuntimeCommandRequest,
) -> Result<ParityMcpServerRuntimeStatus, AppError> {
    let service = service(db);
    let status = service.get_mcp_server_runtime_status(request.id.trim())?;
    let mut state = service.get_mcp_runtime_state(&status.id)?;
    let now = Utc::now().to_rfc3339();

    if status.management_mode == "process" {
        let _ = runtime.stop_process(&status.id, status.pid)?;
        let pid = runtime.start_process(&status.id, &status.endpoint)?;
        state.runtime_status = "running".to_string();
        state.pid = Some(pid);
        state.last_started_at = Some(now.clone());
        state.status_message = None;
    } else {
        state.runtime_status = "external".to_string();
        state.pid = None;
    }

    state.last_reloaded_at = Some(now.clone());
    state.last_error = None;
    state.updated_at = now;
    service.save_mcp_runtime_state(state)?;
    reconcile_mcp_runtime_status(&service, runtime, &status.id)
}

#[tauri::command]
pub fn parity_mcp_runtime_reload(
    db: State<'_, Database>,
    runtime: State<'_, ParityMcpRuntimeManager>,
    request: ParityMcpRuntimeCommandRequest,
) -> Result<ParityMcpServerRuntimeStatus, AppError> {
    parity_mcp_runtime_reload_for_db(db.inner(), runtime.inner(), request)
}

pub fn parity_quick_command_list_for_db(
    db: &Database,
) -> Result<Vec<ParityQuickCommand>, AppError> {
    service(db).list_quick_commands()
}

#[tauri::command]
pub fn parity_quick_command_list(
    db: State<'_, Database>,
) -> Result<Vec<ParityQuickCommand>, AppError> {
    parity_quick_command_list_for_db(db.inner())
}

pub fn parity_quick_command_save_for_db(
    db: &Database,
    request: ParityQuickCommandSaveRequest,
) -> Result<ParityQuickCommand, AppError> {
    service(db).save_quick_command(ParityQuickCommandInput {
        id: request.id,
        name: request.name,
        command: request.command,
        description: request.description,
        enabled: request.enabled,
    })
}

#[tauri::command]
pub fn parity_quick_command_save(
    db: State<'_, Database>,
    request: ParityQuickCommandSaveRequest,
) -> Result<ParityQuickCommand, AppError> {
    parity_quick_command_save_for_db(db.inner(), request)
}

fn reconcile_mcp_runtime_status(
    service: &ParityServiceImpl,
    runtime: &ParityMcpRuntimeManager,
    server_id: &str,
) -> Result<ParityMcpServerRuntimeStatus, AppError> {
    let status = service.get_mcp_server_runtime_status(server_id)?;
    if status.management_mode != "process" {
        return Ok(status);
    }

    let observation = runtime.inspect_process(&status.id, status.pid)?;
    let mut state = service.get_mcp_runtime_state(&status.id)?;
    let mut changed = false;

    if observation.running {
        if state.runtime_status != "running" {
            state.runtime_status = "running".to_string();
            changed = true;
        }
        if state.pid != observation.pid {
            state.pid = observation.pid;
            changed = true;
        }
        if state.last_error.is_some() {
            state.last_error = None;
            changed = true;
        }
    } else {
        if state.runtime_status == "running" {
            state.runtime_status = "stopped".to_string();
            changed = true;
        }
        if state.pid.take().is_some() {
            changed = true;
        }
        if observation.exit_code.is_some() && state.last_exit_code != observation.exit_code {
            state.last_exit_code = observation.exit_code;
            changed = true;
        }
    }

    if changed {
        state.updated_at = Utc::now().to_rfc3339();
        service.save_mcp_runtime_state(state)?;
    }

    service.get_mcp_server_runtime_status(server_id)
}

fn load_runtime_settings(db: &Database) -> Result<PersistedRuntimeSettings, AppError> {
    match db.query_row(
        "SELECT value_json FROM app_settings WHERE key = 'runtime'",
        &[],
        |row| row.get::<_, String>(0),
    ) {
        Ok(json) => serde_json::from_str(&json).map_err(AppError::from_json_error),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(PersistedRuntimeSettings::default()),
        Err(err) => Err(AppError::storage(format!(
            "Failed to load runtime settings for parity readiness: {}",
            err
        ))),
    }
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|item| item.trim().to_string())
        .filter(|item| !item.is_empty())
}

fn build_runtime_readiness_message(
    provider: &str,
    uses_custom_endpoint: bool,
    api_key_ref: Option<&str>,
    auth: &ParityRuntimeAuthSource,
) -> String {
    let endpoint_message = if uses_custom_endpoint {
        "the configured custom endpoint"
    } else {
        "the default endpoint"
    };

    match auth.kind.as_str() {
        "not_required" => format!(
            "Provider selection can authenticate because {provider} does not require an API key and will use {endpoint_message}."
        ),
        "runtime_api_key_ref" if auth.available => format!(
            "Provider selection can authenticate using env var {} from runtime api_key_ref and will use {endpoint_message}.",
            auth.env_var.as_deref().unwrap_or_default()
        ),
        "provider_env" => format!(
            "Provider selection can authenticate using provider env var {} and will use {endpoint_message}.",
            auth.env_var.as_deref().unwrap_or_default()
        ),
        "config_api_key" => {
            if let Some(reference) = api_key_ref {
                format!(
                    "Runtime api_key_ref {} is not set, so provider selection will fall back to the config API key and use {endpoint_message}.",
                    reference
                )
            } else {
                format!(
                    "Provider selection can authenticate using the config API key and will use {endpoint_message}."
                )
            }
        }
        "runtime_api_key_ref" => format!(
            "Runtime api_key_ref {} is configured, but that env var is not set, so provider selection cannot authenticate yet.",
            auth.env_var.as_deref().unwrap_or_default()
        ),
        _ => match auth.env_var.as_deref() {
            Some(env_var) => format!(
                "Provider selection cannot authenticate yet. Set {} or add an API key to the config for provider {}.",
                env_var, provider
            ),
            None => format!(
                "Provider selection cannot authenticate yet. Configure an API key source for provider {}.",
                provider
            ),
        },
    }
}

fn env_var_has_value(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| !value.trim().is_empty())
        .unwrap_or(false)
}

fn provider_secret_env_var_hint(provider: &str) -> Option<String> {
    provider_secret_env_var(provider).map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::{
        ParityMcpProbeRequest, ParityMcpUpsertRequest, parity_mcp_probe_for_db_with_checker,
        parity_mcp_upsert_for_db,
    };
    use crate::backend::Database;
    use serde_json::json;

    #[test]
    fn mcp_probe_includes_stdio_inventory_evidence() {
        let db = Database::in_memory().expect("database should initialize");

        parity_mcp_upsert_for_db(
            &db,
            ParityMcpUpsertRequest {
                id: Some("stdio-probe".to_string()),
                name: "Filesystem".to_string(),
                transport: "stdio".to_string(),
                endpoint: "npx -y @modelcontextprotocol/server-filesystem --root \"/tmp/hermes workspace\"".to_string(),
                enabled: true,
                tool_filter_mode: "allow_list".to_string(),
                allowed_tools: vec!["read_file".to_string(), "list_dir".to_string()],
                blocked_tools: vec!["delete_file".to_string()],
                resources_enabled: true,
                prompts_enabled: false,
            },
        )
        .expect("mcp server should save");

        let probe = parity_mcp_probe_for_db_with_checker(
            &db,
            ParityMcpProbeRequest {
                id: "stdio-probe".to_string(),
            },
            |_| true,
        )
        .expect("probe should succeed");

        let serialized = serde_json::to_value(&probe).expect("probe should serialize");
        assert_eq!(serialized["management_mode"], "process");
        assert_eq!(serialized["tool_filter_mode"], "allow_list");
        assert_eq!(serialized["allowed_tool_count"], 2);
        assert_eq!(serialized["blocked_tool_count"], 1);
        assert_eq!(serialized["resources_enabled"], true);
        assert_eq!(serialized["prompts_enabled"], false);
        assert_eq!(serialized["parsed_command"], "npx");
        assert_eq!(
            serialized["parsed_args"],
            json!([
                "-y",
                "@modelcontextprotocol/server-filesystem",
                "--root",
                "/tmp/hermes workspace"
            ])
        );
        assert_eq!(
            serialized["endpoint_detail"],
            "Parsed stdio command with 4 argument(s)."
        );
    }

    #[test]
    fn mcp_probe_includes_endpoint_validation_detail_for_remote_transports() {
        let db = Database::in_memory().expect("database should initialize");

        parity_mcp_upsert_for_db(
            &db,
            ParityMcpUpsertRequest {
                id: Some("bad-sse".to_string()),
                name: "Bad SSE".to_string(),
                transport: "sse".to_string(),
                endpoint: "ftp://example.com/mcp".to_string(),
                enabled: true,
                tool_filter_mode: "block_list".to_string(),
                allowed_tools: vec!["read_resource".to_string()],
                blocked_tools: vec!["delete_resource".to_string(), "write_resource".to_string()],
                resources_enabled: false,
                prompts_enabled: true,
            },
        )
        .expect("mcp server should save");

        let probe = parity_mcp_probe_for_db_with_checker(
            &db,
            ParityMcpProbeRequest {
                id: "bad-sse".to_string(),
            },
            |_| true,
        )
        .expect("probe should succeed");

        let serialized = serde_json::to_value(&probe).expect("probe should serialize");
        assert_eq!(serialized["management_mode"], "external");
        assert_eq!(serialized["tool_filter_mode"], "block_list");
        assert_eq!(serialized["allowed_tool_count"], 1);
        assert_eq!(serialized["blocked_tool_count"], 2);
        assert_eq!(serialized["resources_enabled"], false);
        assert_eq!(serialized["prompts_enabled"], true);
        assert_eq!(serialized["endpoint_scheme"], "ftp");
        assert_eq!(serialized["endpoint_host"], "example.com");
        assert_eq!(
            serialized["endpoint_detail"],
            "Unsupported URL scheme `ftp`; expected http or https."
        );
    }
}
