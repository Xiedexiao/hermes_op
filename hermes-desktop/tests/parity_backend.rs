use hermes_desktop::backend::Database;
use hermes_desktop::backend::HermesConfig;
use hermes_desktop::backend::ParityMcpRuntimeManager;
use hermes_desktop::commands::parity::{
    ParityCronCreateRequest, ParityCronRunNowRequest, ParityCronSetEnabledRequest,
    ParityMcpProbeRequest, ParityMcpRuntimeCommandRequest, ParityMcpUpsertRequest,
    ParityQuickCommandSaveRequest, ParitySaveProviderSelectionRequest, ParityToolMetadata,
    ParityToolsetSaveRequest, parity_cron_create_for_db, parity_cron_list_for_db,
    parity_cron_run_now_for_db, parity_cron_set_enabled_for_db, parity_get_catalog_for_db,
    parity_get_runtime_readiness_for_db, parity_mcp_list_for_db,
    parity_mcp_probe_for_db_with_checker, parity_mcp_runtime_list_status_for_db,
    parity_mcp_runtime_reload_for_db, parity_mcp_runtime_start_for_db,
    parity_mcp_runtime_stop_for_db, parity_mcp_upsert_for_db, parity_quick_command_list_for_db,
    parity_quick_command_save_for_db, parity_toolset_list_for_db, parity_toolset_save_for_db,
};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct HomeOverride {
    key: &'static str,
    original: Option<OsString>,
}

impl HomeOverride {
    fn set(temp_home: &PathBuf) -> Self {
        let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let original = env::var_os(key);
        unsafe {
            env::set_var(key, temp_home);
        }
        Self { key, original }
    }
}

impl Drop for HomeOverride {
    fn drop(&mut self) {
        match self.original.as_ref() {
            Some(value) => unsafe { env::set_var(self.key, value) },
            None => unsafe { env::remove_var(self.key) },
        }
    }
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

    fn unset(key: &'static str) -> Self {
        let original = env::var_os(key);
        unsafe {
            env::remove_var(key);
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

fn unique_temp_home() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    env::temp_dir().join(format!("hermes-parity-test-{timestamp}"))
}

fn cleanup_temp_home(path: PathBuf) {
    match fs::remove_dir_all(path) {
        Ok(()) => {}
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => panic!("cleanup temp home: {err}"),
    }
}

fn insert_runtime_settings(db: &Database, value_json: &str) {
    let params: Vec<&dyn rusqlite::ToSql> = vec![&value_json, &"2026-04-25T00:00:00Z"];
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at)
         VALUES ('runtime', ?, ?)",
        &params,
    )
    .expect("runtime row should save");
}
use serde_json::json;

#[cfg(target_os = "windows")]
fn stdio_test_endpoint() -> String {
    "ping -n 30 127.0.0.1 >NUL".to_string()
}

#[cfg(not(target_os = "windows"))]
fn stdio_test_endpoint() -> String {
    "exec sleep 30".to_string()
}

#[test]
fn parity_catalog_returns_provider_catalog_and_persists_active_selection() {
    let db = Database::in_memory().expect("database should initialize");

    let initial = parity_get_catalog_for_db(&db).expect("catalog should load");
    assert_eq!(initial.active_provider, "openai");
    assert_eq!(initial.active_model, "gpt-4o");
    assert!(
        initial
            .providers
            .iter()
            .any(|provider| provider.id == "openai")
    );
    assert!(
        initial
            .providers
            .iter()
            .any(|provider| provider.id == "anthropic")
    );

    let saved = hermes_desktop::commands::parity::parity_save_provider_selection_for_db(
        &db,
        ParitySaveProviderSelectionRequest {
            provider: "openrouter".to_string(),
            model: "anthropic/claude-sonnet-4".to_string(),
            base_url: Some("https://openrouter.ai/api/v1".to_string()),
        },
    )
    .expect("provider selection should save");

    assert_eq!(saved.provider, "openrouter");
    assert_eq!(saved.model, "anthropic/claude-sonnet-4");
    assert_eq!(
        saved.base_url.as_deref(),
        Some("https://openrouter.ai/api/v1")
    );

    let updated = parity_get_catalog_for_db(&db).expect("catalog should reload");
    assert_eq!(updated.active_provider, "openrouter");
    assert_eq!(updated.active_model, "anthropic/claude-sonnet-4");
}

#[test]
fn parity_runtime_readiness_reports_missing_provider_credentials() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let temp_home = unique_temp_home();
    let _home = HomeOverride::set(&temp_home);
    let _openai_key = EnvVarOverride::unset("OPENAI_API_KEY");
    let db = Database::in_memory().expect("database should initialize");

    let initial = parity_get_runtime_readiness_for_db(&db).expect("readiness should load");
    assert_eq!(initial.provider, "openai");
    assert_eq!(initial.status, "missing_api_key");
    assert!(!initial.can_authenticate);
    assert_eq!(initial.auth.kind, "none");
    assert_eq!(initial.auth.env_var.as_deref(), Some("OPENAI_API_KEY"));
    assert!(!initial.auth.available);
    assert!(initial.message.contains("OPENAI_API_KEY"));

    cleanup_temp_home(temp_home);
}

#[test]
fn parity_runtime_readiness_uses_runtime_api_key_ref_when_env_var_exists() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let temp_home = unique_temp_home();
    let _home = HomeOverride::set(&temp_home);
    let _openrouter_key = EnvVarOverride::set("OPENROUTER_API_KEY", "runtime-secret");
    let db = Database::in_memory().expect("database should initialize");

    insert_runtime_settings(
        &db,
        r#"{
  "provider": "openrouter",
  "model": "anthropic/claude-sonnet-4",
  "base_url": "https://openrouter.ai/api/v1",
  "api_key_ref": "OPENROUTER_API_KEY",
  "engine_profile": "default",
  "agent_engine_enabled": true,
  "busy_input_mode": "interrupt"
}"#,
    );

    let configured = parity_get_runtime_readiness_for_db(&db).expect("readiness should reload");
    assert_eq!(configured.provider, "openrouter");
    assert_eq!(configured.status, "ready");
    assert!(configured.can_authenticate);
    assert_eq!(configured.auth.kind, "runtime_api_key_ref");
    assert_eq!(
        configured.auth.env_var.as_deref(),
        Some("OPENROUTER_API_KEY")
    );
    assert!(configured.auth.available);
    assert_eq!(
        configured.base_url.as_deref(),
        Some("https://openrouter.ai/api/v1")
    );

    cleanup_temp_home(temp_home);
}

#[test]
fn parity_runtime_readiness_falls_back_to_provider_env_var() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let temp_home = unique_temp_home();
    let _home = HomeOverride::set(&temp_home);
    let _anthropic_key = EnvVarOverride::set("ANTHROPIC_API_KEY", "provider-secret");
    let db = Database::in_memory().expect("database should initialize");

    insert_runtime_settings(
        &db,
        r#"{
  "provider": "anthropic",
  "model": "claude-sonnet-4",
  "base_url": null,
  "api_key_ref": null,
  "engine_profile": "default",
  "agent_engine_enabled": true,
  "busy_input_mode": "interrupt"
}"#,
    );

    let configured = parity_get_runtime_readiness_for_db(&db).expect("readiness should reload");
    assert_eq!(configured.provider, "anthropic");
    assert_eq!(configured.status, "ready");
    assert!(configured.can_authenticate);
    assert_eq!(configured.auth.kind, "provider_env");
    assert_eq!(
        configured.auth.env_var.as_deref(),
        Some("ANTHROPIC_API_KEY")
    );
    assert!(configured.auth.available);
    assert!(configured.message.contains("ANTHROPIC_API_KEY"));

    cleanup_temp_home(temp_home);
}

#[test]
fn parity_runtime_readiness_falls_back_to_config_api_key() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let temp_home = unique_temp_home();
    let _home = HomeOverride::set(&temp_home);
    let _deepseek_key = EnvVarOverride::unset("DEEPSEEK_API_KEY");
    let db = Database::in_memory().expect("database should initialize");

    hermes_desktop::backend::save_config(&HermesConfig {
        provider: "deepseek".to_string(),
        api_key: Some("config-secret".to_string()),
        model: "deepseek-chat".to_string(),
        base_url: None,
        work_dir: "/tmp/hermes".to_string(),
        skills_dir: None,
        busy_input_mode: "interrupt".to_string(),
    })
    .expect("config should save");

    insert_runtime_settings(
        &db,
        r#"{
  "provider": "deepseek",
  "model": "deepseek-chat",
  "base_url": null,
  "api_key_ref": null,
  "engine_profile": "default",
  "agent_engine_enabled": true,
  "busy_input_mode": "interrupt"
}"#,
    );

    let configured = parity_get_runtime_readiness_for_db(&db).expect("readiness should reload");
    assert_eq!(configured.provider, "deepseek");
    assert_eq!(configured.status, "ready");
    assert!(configured.can_authenticate);
    assert_eq!(configured.auth.kind, "config_api_key");
    assert_eq!(configured.auth.label, "Config API key");
    assert_eq!(configured.auth.env_var.as_deref(), Some("DEEPSEEK_API_KEY"));
    assert!(configured.auth.available);

    cleanup_temp_home(temp_home);
}

#[test]
fn parity_runtime_readiness_marks_ollama_ready_without_api_key() {
    let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
    let temp_home = unique_temp_home();
    let _home = HomeOverride::set(&temp_home);
    let db = Database::in_memory().expect("database should initialize");

    insert_runtime_settings(
        &db,
        r#"{
  "provider": "ollama",
  "model": "qwen2.5-coder",
  "base_url": "http://127.0.0.1:11434",
  "api_key_ref": null,
  "engine_profile": "default",
  "agent_engine_enabled": true,
  "busy_input_mode": "interrupt"
}"#,
    );

    let configured = parity_get_runtime_readiness_for_db(&db).expect("readiness should reload");
    assert_eq!(configured.provider, "ollama");
    assert_eq!(configured.status, "ready");
    assert!(configured.can_authenticate);
    assert_eq!(configured.auth.kind, "not_required");
    assert_eq!(configured.auth.label, "No API key required");
    assert!(configured.auth.available);
    assert!(configured.message.contains("does not require an API key"));

    cleanup_temp_home(temp_home);
}

#[test]
fn parity_toolsets_seed_defaults_and_allow_enabled_visibility_updates() {
    let db = Database::in_memory().expect("database should initialize");

    let seeded = parity_toolset_list_for_db(&db).expect("toolsets should list");
    assert!(seeded.iter().any(|toolset| toolset.id == "workspace"));
    assert!(seeded.iter().any(|toolset| {
        toolset.id == "workspace"
            && toolset
                .tools
                .iter()
                .any(|tool| tool.name == "shell" && tool.visible)
    }));

    let saved = parity_toolset_save_for_db(
        &db,
        ParityToolsetSaveRequest {
            id: Some("workspace".to_string()),
            name: "Workspace".to_string(),
            description: Some("Workspace tool access".to_string()),
            enabled: false,
            source: Some("system".to_string()),
            tools: vec![
                ParityToolMetadata {
                    name: "shell".to_string(),
                    description: "Terminal execution".to_string(),
                    visible: true,
                    enabled: true,
                    availability: "stable".to_string(),
                },
                ParityToolMetadata {
                    name: "desktop".to_string(),
                    description: "Desktop automation".to_string(),
                    visible: false,
                    enabled: false,
                    availability: "hidden".to_string(),
                },
            ],
        },
    )
    .expect("toolset should save");

    assert!(!saved.enabled);
    assert!(
        saved
            .tools
            .iter()
            .any(|tool| tool.name == "desktop" && !tool.visible)
    );

    let updated = parity_toolset_list_for_db(&db).expect("toolsets should reload");
    let workspace = updated
        .iter()
        .find(|toolset| toolset.id == "workspace")
        .expect("workspace toolset should exist");
    assert!(!workspace.enabled);
    assert!(
        workspace
            .tools
            .iter()
            .any(|tool| tool.name == "desktop" && !tool.visible && !tool.enabled)
    );
}

#[test]
fn parity_cron_jobs_support_create_pause_resume_and_run_now_metadata() {
    let db = Database::in_memory().expect("database should initialize");

    let created = parity_cron_create_for_db(
        &db,
        ParityCronCreateRequest {
            name: "每日回顾".to_string(),
            schedule: "0 9 * * *".to_string(),
            prompt: "总结今天需要关注的任务".to_string(),
            deliver_to: Some("inbox".to_string()),
            enabled: true,
        },
    )
    .expect("cron job should create");

    assert_eq!(created.status, "scheduled");
    assert!(created.last_run_requested_at.is_none());

    let paused = parity_cron_set_enabled_for_db(
        &db,
        ParityCronSetEnabledRequest {
            id: created.id.clone(),
            enabled: false,
        },
    )
    .expect("cron job should pause");
    assert_eq!(paused.status, "paused");
    assert!(paused.paused_at.is_some());

    let resumed = parity_cron_set_enabled_for_db(
        &db,
        ParityCronSetEnabledRequest {
            id: created.id.clone(),
            enabled: true,
        },
    )
    .expect("cron job should resume");
    assert_eq!(resumed.status, "scheduled");
    assert!(resumed.paused_at.is_none());

    let dispatched = parity_cron_run_now_for_db(
        &db,
        ParityCronRunNowRequest {
            id: created.id.clone(),
        },
    )
    .expect("cron run-now should dispatch locally");
    assert_eq!(dispatched.last_run_status.as_deref(), Some("completed"));
    assert!(dispatched.last_run_requested_at.is_some());
    assert_eq!(dispatched.run_count, 1);

    let session_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM sessions WHERE source = 'cron'",
            &[],
            |row| row.get(0),
        )
        .expect("cron session evidence should exist");
    assert_eq!(session_count, 1);

    let run_event_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM run_events WHERE event_type = 'cron_dispatch_completed'",
            &[],
            |row| row.get(0),
        )
        .expect("cron run event evidence should exist");
    assert_eq!(run_event_count, 1);

    let background_event_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM run_events WHERE event_type = 'background_enqueued'",
            &[],
            |row| row.get(0),
        )
        .expect("background enqueue evidence should exist");
    assert_eq!(background_event_count, 1);

    let mission_status: String = db
        .query_row("SELECT status FROM missions LIMIT 1", &[], |row| row.get(0))
        .expect("cron mission should exist");
    assert_eq!(mission_status, "awaiting_approval");

    let run_status: String = db
        .query_row("SELECT status FROM runs LIMIT 1", &[], |row| row.get(0))
        .expect("cron run should exist");
    assert_eq!(run_status, "queued");

    let step_count: i64 = db
        .query_row("SELECT COUNT(*) FROM execution_steps", &[], |row| {
            row.get(0)
        })
        .expect("cron execution plan should exist");
    assert_eq!(step_count, 3);

    let jobs = parity_cron_list_for_db(&db).expect("cron jobs should list");
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].id, created.id);
    assert_eq!(jobs[0].run_count, 1);
    assert_eq!(jobs[0].last_run_status.as_deref(), Some("completed"));
}

#[test]
fn parity_mcp_upsert_persists_server_metadata_and_filters() {
    let db = Database::in_memory().expect("database should initialize");

    let saved = parity_mcp_upsert_for_db(
        &db,
        ParityMcpUpsertRequest {
            id: Some("filesystem".to_string()),
            name: "Filesystem".to_string(),
            transport: "stdio".to_string(),
            endpoint: "npx -y @modelcontextprotocol/server-filesystem".to_string(),
            enabled: true,
            tool_filter_mode: "allow_list".to_string(),
            allowed_tools: vec!["read_file".to_string(), "list_dir".to_string()],
            blocked_tools: vec!["delete_file".to_string()],
            resources_enabled: true,
            prompts_enabled: false,
        },
    )
    .expect("mcp server should save");

    assert_eq!(saved.tool_filter_mode, "allow_list");
    assert_eq!(saved.allowed_tools.len(), 2);
    assert!(!saved.prompts_enabled);

    let servers = parity_mcp_list_for_db(&db).expect("mcp servers should list");
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].id, "filesystem");
    assert_eq!(servers[0].blocked_tools, vec!["delete_file"]);
}

#[test]
fn parity_mcp_probe_reports_missing_stdio_commands_and_valid_http_urls() {
    let db = Database::in_memory().expect("database should initialize");

    parity_mcp_upsert_for_db(
        &db,
        ParityMcpUpsertRequest {
            id: Some("missing-stdio".to_string()),
            name: "Missing command".to_string(),
            transport: "stdio".to_string(),
            endpoint: "missing-command --serve".to_string(),
            enabled: true,
            tool_filter_mode: "allow_all".to_string(),
            allowed_tools: vec![],
            blocked_tools: vec![],
            resources_enabled: true,
            prompts_enabled: true,
        },
    )
    .expect("stdio server should save");

    let missing = parity_mcp_probe_for_db_with_checker(
        &db,
        ParityMcpProbeRequest {
            id: "missing-stdio".to_string(),
        },
        |_| false,
    )
    .expect("probe should return result");
    assert_eq!(missing.status, "error");
    assert_eq!(missing.transport, "stdio");
    assert_eq!(missing.command_available, Some(false));
    let missing_json = serde_json::to_value(&missing).expect("probe should serialize");
    assert_eq!(missing_json["management_mode"], "process");
    assert_eq!(missing_json["tool_filter_mode"], "allow_all");
    assert_eq!(missing_json["allowed_tool_count"], 0);
    assert_eq!(missing_json["blocked_tool_count"], 0);
    assert_eq!(missing_json["resources_enabled"], true);
    assert_eq!(missing_json["prompts_enabled"], true);
    assert_eq!(missing_json["parsed_command"], "missing-command");
    assert_eq!(missing_json["parsed_args"], json!(["--serve"]));
    assert_eq!(
        missing_json["endpoint_detail"],
        "Parsed stdio command with 1 argument(s)."
    );

    parity_mcp_upsert_for_db(
        &db,
        ParityMcpUpsertRequest {
            id: Some("http-server".to_string()),
            name: "HTTP server".to_string(),
            transport: "http".to_string(),
            endpoint: "https://example.com/mcp".to_string(),
            enabled: true,
            tool_filter_mode: "allow_all".to_string(),
            allowed_tools: vec![],
            blocked_tools: vec![],
            resources_enabled: true,
            prompts_enabled: true,
        },
    )
    .expect("http server should save");

    let http = parity_mcp_probe_for_db_with_checker(
        &db,
        ParityMcpProbeRequest {
            id: "http-server".to_string(),
        },
        |_| true,
    )
    .expect("http probe should return result");
    assert_eq!(http.status, "ready");
    assert_eq!(http.transport, "http");
    assert_eq!(http.url_valid, Some(true));
    let http_json = serde_json::to_value(&http).expect("probe should serialize");
    assert_eq!(http_json["management_mode"], "external");
    assert_eq!(http_json["tool_filter_mode"], "allow_all");
    assert_eq!(http_json["allowed_tool_count"], 0);
    assert_eq!(http_json["blocked_tool_count"], 0);
    assert_eq!(http_json["resources_enabled"], true);
    assert_eq!(http_json["prompts_enabled"], true);
    assert_eq!(http_json["endpoint_scheme"], "https");
    assert_eq!(http_json["endpoint_host"], "example.com");
    assert_eq!(
        http_json["endpoint_detail"],
        "Valid https URL with host `example.com`."
    );
}

#[test]
fn parity_mcp_probe_surfaces_invalid_endpoint_detail_without_handshake() {
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
    .expect("sse server should save");

    let invalid = parity_mcp_probe_for_db_with_checker(
        &db,
        ParityMcpProbeRequest {
            id: "bad-sse".to_string(),
        },
        |_| true,
    )
    .expect("probe should return result");

    assert_eq!(invalid.status, "error");
    assert_eq!(invalid.transport, "sse");
    assert_eq!(invalid.url_valid, Some(false));
    let invalid_json = serde_json::to_value(&invalid).expect("probe should serialize");
    assert_eq!(invalid_json["management_mode"], "external");
    assert_eq!(invalid_json["tool_filter_mode"], "block_list");
    assert_eq!(invalid_json["allowed_tool_count"], 1);
    assert_eq!(invalid_json["blocked_tool_count"], 2);
    assert_eq!(invalid_json["resources_enabled"], false);
    assert_eq!(invalid_json["prompts_enabled"], true);
    assert_eq!(invalid_json["endpoint_scheme"], "ftp");
    assert_eq!(invalid_json["endpoint_host"], "example.com");
    assert_eq!(
        invalid_json["endpoint_detail"],
        "Unsupported URL scheme `ftp`; expected http or https."
    );
}

#[test]
fn parity_mcp_runtime_stdio_servers_persist_status_and_support_start_reload_stop() {
    let db = Database::in_memory().expect("database should initialize");
    let start_runtime = ParityMcpRuntimeManager::default();

    parity_mcp_upsert_for_db(
        &db,
        ParityMcpUpsertRequest {
            id: Some("filesystem".to_string()),
            name: "Filesystem".to_string(),
            transport: "stdio".to_string(),
            endpoint: stdio_test_endpoint(),
            enabled: true,
            tool_filter_mode: "allow_all".to_string(),
            allowed_tools: vec![],
            blocked_tools: vec![],
            resources_enabled: true,
            prompts_enabled: true,
        },
    )
    .expect("mcp server should save");

    let started = parity_mcp_runtime_start_for_db(
        &db,
        &start_runtime,
        ParityMcpRuntimeCommandRequest {
            id: "filesystem".to_string(),
        },
    )
    .expect("stdio server should start");

    assert_eq!(started.runtime_status, "running");
    assert_eq!(started.management_mode, "process");
    let first_pid = started.pid.expect("stdio server should have a pid");
    assert!(started.last_started_at.is_some());

    let fresh_runtime = ParityMcpRuntimeManager::default();
    let listed = parity_mcp_runtime_list_status_for_db(&db, &fresh_runtime)
        .expect("runtime status should list from persisted state");
    let listed_server = listed
        .iter()
        .find(|server| server.id == "filesystem")
        .expect("filesystem runtime status should exist");
    assert_eq!(listed_server.runtime_status, "running");
    assert_eq!(listed_server.pid, Some(first_pid));

    let reloaded = parity_mcp_runtime_reload_for_db(
        &db,
        &fresh_runtime,
        ParityMcpRuntimeCommandRequest {
            id: "filesystem".to_string(),
        },
    )
    .expect("stdio server should reload");

    assert_eq!(reloaded.runtime_status, "running");
    assert_eq!(reloaded.management_mode, "process");
    assert!(reloaded.last_reloaded_at.is_some());
    assert_ne!(reloaded.pid, Some(first_pid));

    let stopped = parity_mcp_runtime_stop_for_db(
        &db,
        &fresh_runtime,
        ParityMcpRuntimeCommandRequest {
            id: "filesystem".to_string(),
        },
    )
    .expect("stdio server should stop");

    assert_eq!(stopped.runtime_status, "stopped");
    assert!(stopped.pid.is_none());
    assert!(stopped.last_stopped_at.is_some());

    let final_statuses = parity_mcp_runtime_list_status_for_db(&db, &fresh_runtime)
        .expect("runtime status should still list after stop");
    let final_status = final_statuses
        .iter()
        .find(|server| server.id == "filesystem")
        .expect("filesystem runtime status should still exist");
    assert_eq!(final_status.runtime_status, "stopped");
    assert!(final_status.pid.is_none());
}

#[test]
fn parity_mcp_runtime_http_servers_are_reported_as_managed_externally() {
    let db = Database::in_memory().expect("database should initialize");
    let runtime = ParityMcpRuntimeManager::default();

    parity_mcp_upsert_for_db(
        &db,
        ParityMcpUpsertRequest {
            id: Some("remote".to_string()),
            name: "Remote SSE".to_string(),
            transport: "sse".to_string(),
            endpoint: "https://example.com/mcp".to_string(),
            enabled: true,
            tool_filter_mode: "allow_all".to_string(),
            allowed_tools: vec![],
            blocked_tools: vec![],
            resources_enabled: true,
            prompts_enabled: true,
        },
    )
    .expect("external mcp server should save");

    let started = parity_mcp_runtime_start_for_db(
        &db,
        &runtime,
        ParityMcpRuntimeCommandRequest {
            id: "remote".to_string(),
        },
    )
    .expect("external server should record managed status");

    assert_eq!(started.runtime_status, "external");
    assert_eq!(started.management_mode, "external");
    assert!(started.pid.is_none());
    assert!(started.status_message.is_some());

    let listed = parity_mcp_runtime_list_status_for_db(&db, &runtime)
        .expect("external runtime status should list");
    let listed_server = listed
        .iter()
        .find(|server| server.id == "remote")
        .expect("external runtime status should exist");
    assert_eq!(listed_server.runtime_status, "external");
    assert_eq!(listed_server.management_mode, "external");

    let stopped = parity_mcp_runtime_stop_for_db(
        &db,
        &runtime,
        ParityMcpRuntimeCommandRequest {
            id: "remote".to_string(),
        },
    )
    .expect("external stop should keep metadata external");

    assert_eq!(stopped.runtime_status, "external");
    assert!(stopped.last_stopped_at.is_some());
}

#[test]
fn parity_quick_commands_can_be_created_and_reenabled() {
    let db = Database::in_memory().expect("database should initialize");

    let created = parity_quick_command_save_for_db(
        &db,
        ParityQuickCommandSaveRequest {
            id: Some("daily-brief".to_string()),
            name: "Daily Brief".to_string(),
            command: "/plan summarize current priorities".to_string(),
            description: Some("Daily summary shortcut".to_string()),
            enabled: false,
        },
    )
    .expect("quick command should save");
    assert!(!created.enabled);

    let updated = parity_quick_command_save_for_db(
        &db,
        ParityQuickCommandSaveRequest {
            id: Some("daily-brief".to_string()),
            name: "Daily Brief".to_string(),
            command: "/plan summarize current priorities".to_string(),
            description: Some("Daily summary shortcut".to_string()),
            enabled: true,
        },
    )
    .expect("quick command should update");
    assert!(updated.enabled);

    let commands = parity_quick_command_list_for_db(&db).expect("quick commands should list");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].id, "daily-brief");
    assert!(commands[0].enabled);
}
