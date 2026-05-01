//! Hermes parity 后端服务

use chrono::Utc;
use uuid::Uuid;

use crate::backend::{
    AppError, AppResult, Database, ParityCatalog, ParityCronJob, ParityCronJobInput,
    ParityMcpRuntimeState, ParityMcpServer, ParityMcpServerInput, ParityMcpServerRuntimeStatus,
    ParityModelCatalogEntry, ParityProviderCatalog, ParityProviderSelection,
    ParityProviderSelectionInput, ParityQuickCommand, ParityQuickCommandInput, ParityToolMetadata,
    ParityToolset, ParityToolsetInput, default_runtime_status_for_transport,
    external_status_message, management_mode_for_transport,
};

const RUNTIME_SETTINGS_KEY: &str = "runtime";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct PersistedRuntimeSettings {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key_ref: Option<String>,
    #[serde(default)]
    engine_profile: Option<String>,
    #[serde(default)]
    agent_engine_enabled: Option<bool>,
}

impl Default for PersistedRuntimeSettings {
    fn default() -> Self {
        Self {
            provider: Some("openai".to_string()),
            model: Some("gpt-4o".to_string()),
            base_url: None,
            api_key_ref: None,
            engine_profile: Some("default".to_string()),
            agent_engine_enabled: Some(true),
        }
    }
}

pub trait ParityService: Send + Sync {
    fn get_catalog(&self) -> AppResult<ParityCatalog>;
    fn save_provider_selection(
        &self,
        input: ParityProviderSelectionInput,
    ) -> AppResult<ParityProviderSelection>;
    fn list_toolsets(&self) -> AppResult<Vec<ParityToolset>>;
    fn save_toolset(&self, input: ParityToolsetInput) -> AppResult<ParityToolset>;
    fn list_cron_jobs(&self) -> AppResult<Vec<ParityCronJob>>;
    fn create_cron_job(&self, input: ParityCronJobInput) -> AppResult<ParityCronJob>;
    fn set_cron_job_enabled(&self, id: &str, enabled: bool) -> AppResult<ParityCronJob>;
    fn run_cron_job_now(&self, id: &str) -> AppResult<ParityCronJob>;
    fn list_mcp_servers(&self) -> AppResult<Vec<ParityMcpServer>>;
    fn upsert_mcp_server(&self, input: ParityMcpServerInput) -> AppResult<ParityMcpServer>;
    fn list_quick_commands(&self) -> AppResult<Vec<ParityQuickCommand>>;
    fn save_quick_command(&self, input: ParityQuickCommandInput) -> AppResult<ParityQuickCommand>;
}

pub struct ParityServiceImpl {
    db: Database,
}

impl ParityServiceImpl {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    fn ensure_schema(&self) -> AppResult<()> {
        self.db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS parity_toolsets (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                description TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                source TEXT NOT NULL,
                tools_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS parity_cron_jobs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                schedule TEXT NOT NULL,
                prompt TEXT NOT NULL,
                deliver_to TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                status TEXT NOT NULL,
                last_run_requested_at TEXT,
                last_run_status TEXT,
                run_count INTEGER NOT NULL DEFAULT 0,
                paused_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS parity_mcp_servers (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                transport TEXT NOT NULL,
                endpoint TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                tool_filter_mode TEXT NOT NULL,
                allowed_tools_json TEXT NOT NULL,
                blocked_tools_json TEXT NOT NULL,
                resources_enabled INTEGER NOT NULL DEFAULT 1,
                prompts_enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS parity_mcp_runtime_state (
                server_id TEXT PRIMARY KEY,
                runtime_status TEXT NOT NULL,
                management_mode TEXT NOT NULL,
                pid INTEGER,
                last_started_at TEXT,
                last_stopped_at TEXT,
                last_reloaded_at TEXT,
                last_exit_code INTEGER,
                last_error TEXT,
                status_message TEXT,
                updated_at TEXT NOT NULL,
                FOREIGN KEY(server_id) REFERENCES parity_mcp_servers(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS parity_quick_commands (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                command TEXT NOT NULL,
                description TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        )
    }

    fn ensure_default_toolsets(&self) -> AppResult<()> {
        self.ensure_schema()?;
        let count: i64 = self
            .db
            .query_row("SELECT COUNT(*) FROM parity_toolsets", &[], |row| {
                row.get(0)
            })
            .map_err(|err| {
                AppError::storage(format!("Failed to count parity toolsets: {}", err))
            })?;

        if count > 0 {
            return Ok(());
        }

        let defaults = vec![
            default_toolset(
                "workspace",
                "Workspace",
                Some("Workspace execution and file access".to_string()),
                vec![
                    tool("shell", "Terminal execution", true, true, "stable"),
                    tool(
                        "filesystem",
                        "Read and write project files",
                        true,
                        true,
                        "stable",
                    ),
                    tool("desktop", "Desktop automation", false, false, "preview"),
                ],
            ),
            default_toolset(
                "network",
                "Network",
                Some("HTTP, browser, and remote connectivity".to_string()),
                vec![
                    tool("http", "Fetch remote resources", true, true, "stable"),
                    tool("browser", "Interactive web browsing", true, true, "beta"),
                ],
            ),
            default_toolset(
                "knowledge",
                "Knowledge",
                Some("Knowledge retrieval and memory surfaces".to_string()),
                vec![
                    tool("memory", "Long-term memory access", true, true, "stable"),
                    tool(
                        "search",
                        "Search across local knowledge",
                        true,
                        true,
                        "stable",
                    ),
                ],
            ),
        ];

        for toolset in defaults {
            self.insert_or_replace_toolset(&toolset)?;
        }

        Ok(())
    }

    fn load_runtime_settings(&self) -> AppResult<PersistedRuntimeSettings> {
        match self.db.query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            &[&RUNTIME_SETTINGS_KEY],
            |row| row.get::<_, String>(0),
        ) {
            Ok(json) => serde_json::from_str(&json).map_err(AppError::from_json_error),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(PersistedRuntimeSettings::default()),
            Err(err) => Err(AppError::storage(format!(
                "Failed to load runtime settings: {}",
                err
            ))),
        }
    }

    fn save_runtime_settings(&self, settings: &PersistedRuntimeSettings) -> AppResult<()> {
        let json = serde_json::to_string(settings).map_err(AppError::from_json_error)?;
        let now = now_rfc3339();
        let params: Vec<&dyn rusqlite::ToSql> = vec![&RUNTIME_SETTINGS_KEY, &json, &now];
        self.db.execute(
            "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?, ?, ?)",
            &params,
        )?;
        Ok(())
    }

    fn insert_or_replace_toolset(&self, toolset: &ParityToolset) -> AppResult<()> {
        let tools_json =
            serde_json::to_string(&toolset.tools).map_err(AppError::from_json_error)?;
        let enabled = toolset.enabled as i64;
        let params: Vec<&dyn rusqlite::ToSql> = vec![
            &toolset.id,
            &toolset.name,
            &toolset.description,
            &enabled,
            &toolset.source,
            &tools_json,
            &toolset.created_at,
            &toolset.updated_at,
        ];
        self.db.execute(
            "INSERT OR REPLACE INTO parity_toolsets
             (id, name, description, enabled, source, tools_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &params,
        )?;
        Ok(())
    }

    fn get_toolset(&self, id: &str) -> AppResult<ParityToolset> {
        self.db
            .query_row(
                "SELECT id, name, description, enabled, source, tools_json, created_at, updated_at
                 FROM parity_toolsets
                 WHERE id = ?1",
                &[&id],
                map_toolset_row,
            )
            .map_err(|err| AppError::storage(format!("Failed to load parity toolset: {}", err)))
    }

    fn get_cron_job(&self, id: &str) -> AppResult<ParityCronJob> {
        self.db
            .query_row(
                "SELECT id, name, schedule, prompt, deliver_to, enabled, status,
                        last_run_requested_at, last_run_status, run_count, paused_at,
                        created_at, updated_at
                 FROM parity_cron_jobs
                 WHERE id = ?1",
                &[&id],
                map_cron_row,
            )
            .map_err(|err| AppError::storage(format!("Failed to load parity cron job: {}", err)))
    }

    fn get_mcp_server(&self, id: &str) -> AppResult<ParityMcpServer> {
        self.db
            .query_row(
                "SELECT id, name, transport, endpoint, enabled, tool_filter_mode,
                        allowed_tools_json, blocked_tools_json, resources_enabled,
                        prompts_enabled, created_at, updated_at
                 FROM parity_mcp_servers
                 WHERE id = ?1",
                &[&id],
                map_mcp_row,
            )
            .map_err(|err| AppError::storage(format!("Failed to load parity MCP server: {}", err)))
    }

    fn get_quick_command(&self, id: &str) -> AppResult<ParityQuickCommand> {
        self.db
            .query_row(
                "SELECT id, name, command, description, enabled, created_at, updated_at
                 FROM parity_quick_commands
                 WHERE id = ?1",
                &[&id],
                map_quick_command_row,
            )
            .map_err(|err| {
                AppError::storage(format!("Failed to load parity quick command: {}", err))
            })
    }

    fn get_raw_mcp_runtime_state(
        &self,
        server_id: &str,
    ) -> Result<ParityMcpRuntimeState, rusqlite::Error> {
        self.db.query_row(
            "SELECT server_id, runtime_status, management_mode, pid, last_started_at,
                        last_stopped_at, last_reloaded_at, last_exit_code, last_error,
                        status_message, updated_at
                 FROM parity_mcp_runtime_state
                 WHERE server_id = ?1",
            &[&server_id],
            map_mcp_runtime_state_row,
        )
    }

    fn insert_or_replace_mcp_runtime_state(&self, state: &ParityMcpRuntimeState) -> AppResult<()> {
        let pid = state.pid.map(i64::from);
        let last_exit_code = state.last_exit_code.map(i64::from);
        let params: Vec<&dyn rusqlite::ToSql> = vec![
            &state.server_id,
            &state.runtime_status,
            &state.management_mode,
            &pid,
            &state.last_started_at,
            &state.last_stopped_at,
            &state.last_reloaded_at,
            &last_exit_code,
            &state.last_error,
            &state.status_message,
            &state.updated_at,
        ];
        self.db.execute(
            "INSERT OR REPLACE INTO parity_mcp_runtime_state
             (server_id, runtime_status, management_mode, pid, last_started_at,
              last_stopped_at, last_reloaded_at, last_exit_code, last_error,
              status_message, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &params,
        )?;
        Ok(())
    }

    fn normalize_mcp_runtime_state(
        &self,
        server: &ParityMcpServer,
        mut state: ParityMcpRuntimeState,
    ) -> AppResult<ParityMcpRuntimeState> {
        let expected_management_mode = management_mode_for_transport(&server.transport).to_string();
        let expected_status = default_runtime_status_for_transport(&server.transport).to_string();
        let expected_message = external_status_message(&server.transport);

        let mut changed = false;

        if state.management_mode != expected_management_mode {
            state.management_mode = expected_management_mode.clone();
            changed = true;
        }

        if expected_management_mode == "external" {
            if state.runtime_status != expected_status {
                state.runtime_status = expected_status;
                changed = true;
            }
            if state.pid.take().is_some() {
                changed = true;
            }
            if state.status_message != expected_message {
                state.status_message = expected_message;
                changed = true;
            }
        } else {
            if state.runtime_status == "external" {
                state.runtime_status = expected_status;
                changed = true;
            }
            if state.status_message.as_deref()
                == external_status_message(&server.transport).as_deref()
            {
                state.status_message = None;
                changed = true;
            }
        }

        if changed {
            state.updated_at = now_rfc3339();
            self.insert_or_replace_mcp_runtime_state(&state)?;
        }

        Ok(state)
    }

    pub fn get_mcp_runtime_state(&self, server_id: &str) -> AppResult<ParityMcpRuntimeState> {
        self.ensure_schema()?;
        let server = self.get_mcp_server(server_id)?;
        match self.get_raw_mcp_runtime_state(server_id) {
            Ok(state) => self.normalize_mcp_runtime_state(&server, state),
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let state = default_mcp_runtime_state(&server);
                self.insert_or_replace_mcp_runtime_state(&state)?;
                Ok(state)
            }
            Err(err) => Err(AppError::storage(format!(
                "Failed to load parity MCP runtime state: {}",
                err
            ))),
        }
    }

    pub fn save_mcp_runtime_state(
        &self,
        state: ParityMcpRuntimeState,
    ) -> AppResult<ParityMcpRuntimeState> {
        self.ensure_schema()?;
        self.insert_or_replace_mcp_runtime_state(&state)?;
        self.get_mcp_runtime_state(&state.server_id)
    }

    pub fn get_mcp_server_runtime_status(
        &self,
        server_id: &str,
    ) -> AppResult<ParityMcpServerRuntimeStatus> {
        self.ensure_schema()?;
        let server = self.get_mcp_server(server_id)?;
        let runtime_state = self.get_mcp_runtime_state(server_id)?;
        Ok(runtime_status_from_parts(server, runtime_state))
    }

    pub fn list_mcp_server_runtime_statuses(&self) -> AppResult<Vec<ParityMcpServerRuntimeStatus>> {
        self.ensure_schema()?;
        let servers = self.list_mcp_servers()?;
        let mut statuses = Vec::with_capacity(servers.len());
        for server in servers {
            let runtime_state = self.get_mcp_runtime_state(&server.id)?;
            statuses.push(runtime_status_from_parts(server, runtime_state));
        }
        Ok(statuses)
    }
}

impl ParityService for ParityServiceImpl {
    fn get_catalog(&self) -> AppResult<ParityCatalog> {
        self.ensure_schema()?;
        let settings = self.load_runtime_settings()?;
        let providers = provider_catalog();

        Ok(ParityCatalog {
            providers,
            active_provider: settings.provider.unwrap_or_else(|| "openai".to_string()),
            active_model: settings.model.unwrap_or_else(|| "gpt-4o".to_string()),
            tool_visibility_options: vec![
                "visible".to_string(),
                "hidden".to_string(),
                "experimental".to_string(),
            ],
            cron_status_options: vec![
                "scheduled".to_string(),
                "paused".to_string(),
                "requested".to_string(),
            ],
            mcp_filter_modes: vec![
                "all".to_string(),
                "allow_list".to_string(),
                "block_list".to_string(),
            ],
        })
    }

    fn save_provider_selection(
        &self,
        input: ParityProviderSelectionInput,
    ) -> AppResult<ParityProviderSelection> {
        self.ensure_schema()?;
        let provider = input.provider.trim().to_string();
        let model = input.model.trim().to_string();
        let base_url = normalize_optional_text(input.base_url);

        if provider.is_empty() {
            return Err(AppError::validation("provider is required"));
        }
        if model.is_empty() {
            return Err(AppError::validation("model is required"));
        }

        let mut current = self.load_runtime_settings()?;
        current.provider = Some(provider.clone());
        current.model = Some(model.clone());
        current.base_url = base_url.clone();
        self.save_runtime_settings(&current)?;

        Ok(ParityProviderSelection {
            provider,
            model,
            base_url,
        })
    }

    fn list_toolsets(&self) -> AppResult<Vec<ParityToolset>> {
        self.ensure_default_toolsets()?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, description, enabled, source, tools_json, created_at, updated_at
                 FROM parity_toolsets
                 ORDER BY name ASC, id ASC",
            )?;
            let rows = stmt.query_map([], map_toolset_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn save_toolset(&self, input: ParityToolsetInput) -> AppResult<ParityToolset> {
        self.ensure_default_toolsets()?;

        let id = normalize_optional_text(input.id).unwrap_or_else(|| slugify(&input.name));
        let name = input.name.trim().to_string();
        let description = normalize_optional_text(input.description);
        let source = normalize_optional_text(input.source).unwrap_or_else(|| "custom".to_string());
        let tools = normalize_tools(input.tools)?;

        if id.is_empty() {
            return Err(AppError::validation("toolset id is required"));
        }
        if name.is_empty() {
            return Err(AppError::validation("toolset name is required"));
        }

        let now = now_rfc3339();
        let created_at = self
            .get_toolset(&id)
            .map(|existing| existing.created_at)
            .unwrap_or_else(|_| now.clone());

        let toolset = ParityToolset {
            id: id.clone(),
            name,
            description,
            enabled: input.enabled,
            source,
            tools,
            created_at,
            updated_at: now,
        };

        self.insert_or_replace_toolset(&toolset)?;
        self.get_toolset(&id)
    }

    fn list_cron_jobs(&self) -> AppResult<Vec<ParityCronJob>> {
        self.ensure_schema()?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, schedule, prompt, deliver_to, enabled, status,
                        last_run_requested_at, last_run_status, run_count, paused_at,
                        created_at, updated_at
                 FROM parity_cron_jobs
                 ORDER BY datetime(updated_at) DESC, name ASC",
            )?;
            let rows = stmt.query_map([], map_cron_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn create_cron_job(&self, input: ParityCronJobInput) -> AppResult<ParityCronJob> {
        self.ensure_schema()?;
        let name = input.name.trim().to_string();
        let schedule = input.schedule.trim().to_string();
        let prompt = input.prompt.trim().to_string();
        let deliver_to = normalize_optional_text(input.deliver_to);

        if name.is_empty() {
            return Err(AppError::validation("cron name is required"));
        }
        if schedule.is_empty() {
            return Err(AppError::validation("cron schedule is required"));
        }
        if prompt.is_empty() {
            return Err(AppError::validation("cron prompt is required"));
        }

        let id = Uuid::new_v4().to_string();
        let now = now_rfc3339();
        let status = if input.enabled {
            "scheduled".to_string()
        } else {
            "paused".to_string()
        };
        let paused_at = if input.enabled {
            None
        } else {
            Some(now.clone())
        };
        let enabled = input.enabled as i64;

        let params: Vec<&dyn rusqlite::ToSql> = vec![
            &id,
            &name,
            &schedule,
            &prompt,
            &deliver_to,
            &enabled,
            &status,
            &paused_at,
            &now,
            &now,
        ];
        self.db.execute(
            "INSERT INTO parity_cron_jobs
             (id, name, schedule, prompt, deliver_to, enabled, status, paused_at, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &params,
        )?;

        self.get_cron_job(&id)
    }

    fn set_cron_job_enabled(&self, id: &str, enabled: bool) -> AppResult<ParityCronJob> {
        self.ensure_schema()?;
        let normalized_id = id.trim().to_string();
        if normalized_id.is_empty() {
            return Err(AppError::validation("cron id is required"));
        }

        let status = if enabled { "scheduled" } else { "paused" };
        let now = now_rfc3339();
        let paused_at = if enabled { None } else { Some(now.clone()) };
        let enabled_value = enabled as i64;
        let params: Vec<&dyn rusqlite::ToSql> =
            vec![&enabled_value, &status, &paused_at, &now, &normalized_id];
        let updated = self.db.execute(
            "UPDATE parity_cron_jobs
             SET enabled = ?, status = ?, paused_at = ?, updated_at = ?
             WHERE id = ?",
            &params,
        )?;
        if updated == 0 {
            return Err(AppError::validation("cron job not found"));
        }

        self.get_cron_job(&normalized_id)
    }

    fn run_cron_job_now(&self, id: &str) -> AppResult<ParityCronJob> {
        self.ensure_schema()?;
        let normalized_id = id.trim().to_string();
        if normalized_id.is_empty() {
            return Err(AppError::validation("cron id is required"));
        }

        let now = now_rfc3339();
        let params: Vec<&dyn rusqlite::ToSql> = vec![&now, &"requested", &now, &normalized_id];
        let updated = self.db.execute(
            "UPDATE parity_cron_jobs
             SET last_run_requested_at = ?,
                 last_run_status = ?,
                 run_count = run_count + 1,
                 updated_at = ?
             WHERE id = ?",
            &params,
        )?;
        if updated == 0 {
            return Err(AppError::validation("cron job not found"));
        }

        self.get_cron_job(&normalized_id)
    }

    fn list_mcp_servers(&self) -> AppResult<Vec<ParityMcpServer>> {
        self.ensure_schema()?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, transport, endpoint, enabled, tool_filter_mode,
                        allowed_tools_json, blocked_tools_json, resources_enabled,
                        prompts_enabled, created_at, updated_at
                 FROM parity_mcp_servers
                 ORDER BY name ASC, id ASC",
            )?;
            let rows = stmt.query_map([], map_mcp_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn upsert_mcp_server(&self, input: ParityMcpServerInput) -> AppResult<ParityMcpServer> {
        self.ensure_schema()?;

        let id = normalize_optional_text(input.id).unwrap_or_else(|| slugify(&input.name));
        let name = input.name.trim().to_string();
        let transport = input.transport.trim().to_string();
        let endpoint = input.endpoint.trim().to_string();
        let tool_filter_mode = input.tool_filter_mode.trim().to_string();
        let allowed_tools = normalize_string_list(input.allowed_tools);
        let blocked_tools = normalize_string_list(input.blocked_tools);

        if id.is_empty() {
            return Err(AppError::validation("mcp id is required"));
        }
        if name.is_empty() {
            return Err(AppError::validation("mcp name is required"));
        }
        if transport.is_empty() {
            return Err(AppError::validation("mcp transport is required"));
        }
        if endpoint.is_empty() {
            return Err(AppError::validation("mcp endpoint is required"));
        }
        if tool_filter_mode.is_empty() {
            return Err(AppError::validation("mcp filter mode is required"));
        }

        let allowed_json =
            serde_json::to_string(&allowed_tools).map_err(AppError::from_json_error)?;
        let blocked_json =
            serde_json::to_string(&blocked_tools).map_err(AppError::from_json_error)?;
        let now = now_rfc3339();
        let created_at = self
            .get_mcp_server(&id)
            .map(|existing| existing.created_at)
            .unwrap_or_else(|_| now.clone());
        let enabled = input.enabled as i64;
        let resources_enabled = input.resources_enabled as i64;
        let prompts_enabled = input.prompts_enabled as i64;
        let params: Vec<&dyn rusqlite::ToSql> = vec![
            &id,
            &name,
            &transport,
            &endpoint,
            &enabled,
            &tool_filter_mode,
            &allowed_json,
            &blocked_json,
            &resources_enabled,
            &prompts_enabled,
            &created_at,
            &now,
        ];
        self.db.execute(
            "INSERT OR REPLACE INTO parity_mcp_servers
             (id, name, transport, endpoint, enabled, tool_filter_mode,
              allowed_tools_json, blocked_tools_json, resources_enabled,
              prompts_enabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &params,
        )?;

        let server = self.get_mcp_server(&id)?;
        let current_state = match self.get_raw_mcp_runtime_state(&id) {
            Ok(state) => self.normalize_mcp_runtime_state(&server, state)?,
            Err(rusqlite::Error::QueryReturnedNoRows) => {
                let state = default_mcp_runtime_state(&server);
                self.insert_or_replace_mcp_runtime_state(&state)?;
                state
            }
            Err(err) => {
                return Err(AppError::storage(format!(
                    "Failed to reconcile parity MCP runtime state: {}",
                    err
                )));
            }
        };

        if current_state.server_id != id {
            return Err(AppError::storage(
                "Failed to reconcile parity MCP runtime state",
            ));
        }

        self.get_mcp_server(&id)
    }

    fn list_quick_commands(&self) -> AppResult<Vec<ParityQuickCommand>> {
        self.ensure_schema()?;
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, name, command, description, enabled, created_at, updated_at
                 FROM parity_quick_commands
                 ORDER BY name ASC, id ASC",
            )?;
            let rows = stmt.query_map([], map_quick_command_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn save_quick_command(&self, input: ParityQuickCommandInput) -> AppResult<ParityQuickCommand> {
        self.ensure_schema()?;
        let id = normalize_optional_text(input.id).unwrap_or_else(|| slugify(&input.name));
        let name = input.name.trim().to_string();
        let command = input.command.trim().to_string();
        let description = normalize_optional_text(input.description);

        if id.is_empty() {
            return Err(AppError::validation("quick command id is required"));
        }
        if name.is_empty() {
            return Err(AppError::validation("quick command name is required"));
        }
        if command.is_empty() {
            return Err(AppError::validation("quick command command is required"));
        }

        let now = now_rfc3339();
        let created_at = self
            .get_quick_command(&id)
            .map(|existing| existing.created_at)
            .unwrap_or_else(|_| now.clone());
        let enabled = input.enabled as i64;
        let params: Vec<&dyn rusqlite::ToSql> = vec![
            &id,
            &name,
            &command,
            &description,
            &enabled,
            &created_at,
            &now,
        ];
        self.db.execute(
            "INSERT OR REPLACE INTO parity_quick_commands
             (id, name, command, description, enabled, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            &params,
        )?;

        self.get_quick_command(&id)
    }
}

fn provider_catalog() -> Vec<ParityProviderCatalog> {
    vec![
        provider(
            "openai",
            "OpenAI",
            true,
            vec![
                model("gpt-4o", "GPT-4o", true),
                model("gpt-4.1", "GPT-4.1", false),
                model("o4-mini", "o4-mini", false),
            ],
        ),
        provider(
            "anthropic",
            "Anthropic",
            true,
            vec![
                model("claude-sonnet-4", "Claude Sonnet 4", true),
                model("claude-opus-4", "Claude Opus 4", false),
            ],
        ),
        provider(
            "deepseek",
            "DeepSeek",
            true,
            vec![
                model("deepseek-chat", "DeepSeek Chat", true),
                model("deepseek-reasoner", "DeepSeek Reasoner", false),
            ],
        ),
        provider(
            "ollama",
            "Ollama",
            true,
            vec![
                model("qwen2.5-coder", "Qwen 2.5 Coder", true),
                model("llama3.1", "Llama 3.1", false),
            ],
        ),
        provider(
            "openrouter",
            "OpenRouter",
            true,
            vec![
                model("anthropic/claude-sonnet-4", "Claude Sonnet 4", true),
                model("openai/gpt-4o", "GPT-4o", false),
            ],
        ),
    ]
}

fn provider(
    id: &str,
    display_name: &str,
    supports_custom_endpoint: bool,
    models: Vec<ParityModelCatalogEntry>,
) -> ParityProviderCatalog {
    ParityProviderCatalog {
        id: id.to_string(),
        display_name: display_name.to_string(),
        supports_custom_endpoint,
        models,
    }
}

fn model(id: &str, display_name: &str, recommended: bool) -> ParityModelCatalogEntry {
    ParityModelCatalogEntry {
        id: id.to_string(),
        display_name: display_name.to_string(),
        recommended,
    }
}

fn tool(
    name: &str,
    description: &str,
    visible: bool,
    enabled: bool,
    availability: &str,
) -> ParityToolMetadata {
    ParityToolMetadata {
        name: name.to_string(),
        description: description.to_string(),
        visible,
        enabled,
        availability: availability.to_string(),
    }
}

fn default_toolset(
    id: &str,
    name: &str,
    description: Option<String>,
    tools: Vec<ParityToolMetadata>,
) -> ParityToolset {
    let now = now_rfc3339();
    ParityToolset {
        id: id.to_string(),
        name: name.to_string(),
        description,
        enabled: true,
        source: "system".to_string(),
        tools,
        created_at: now.clone(),
        updated_at: now,
    }
}

fn normalize_tools(tools: Vec<ParityToolMetadata>) -> AppResult<Vec<ParityToolMetadata>> {
    let mut normalized = Vec::new();
    for tool in tools {
        let name = tool.name.trim().to_string();
        let description = tool.description.trim().to_string();
        let availability = tool.availability.trim().to_string();
        if name.is_empty() {
            return Err(AppError::validation("tool name is required"));
        }
        normalized.push(ParityToolMetadata {
            name,
            description,
            visible: tool.visible,
            enabled: tool.enabled,
            availability: if availability.is_empty() {
                "stable".to_string()
            } else {
                availability
            },
        });
    }
    normalized.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(normalized)
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|item| {
        let trimmed = item.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn normalize_string_list(values: Vec<String>) -> Vec<String> {
    let mut normalized = values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    normalized.sort();
    normalized.dedup();
    normalized
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn map_toolset_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ParityToolset> {
    let tools_json: String = row.get(5)?;
    let tools = serde_json::from_str(&tools_json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(ParityToolset {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        enabled: row.get::<_, i64>(3)? != 0,
        source: row.get(4)?,
        tools,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn map_cron_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ParityCronJob> {
    Ok(ParityCronJob {
        id: row.get(0)?,
        name: row.get(1)?,
        schedule: row.get(2)?,
        prompt: row.get(3)?,
        deliver_to: row.get(4)?,
        enabled: row.get::<_, i64>(5)? != 0,
        status: row.get(6)?,
        last_run_requested_at: row.get(7)?,
        last_run_status: row.get(8)?,
        run_count: row.get::<_, i64>(9)? as u32,
        paused_at: row.get(10)?,
        created_at: row.get(11)?,
        updated_at: row.get(12)?,
    })
}

fn map_mcp_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ParityMcpServer> {
    let allowed_json: String = row.get(6)?;
    let blocked_json: String = row.get(7)?;
    let allowed_tools = serde_json::from_str(&allowed_json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let blocked_tools = serde_json::from_str(&blocked_json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(err))
    })?;

    Ok(ParityMcpServer {
        id: row.get(0)?,
        name: row.get(1)?,
        transport: row.get(2)?,
        endpoint: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        tool_filter_mode: row.get(5)?,
        allowed_tools,
        blocked_tools,
        resources_enabled: row.get::<_, i64>(8)? != 0,
        prompts_enabled: row.get::<_, i64>(9)? != 0,
        created_at: row.get(10)?,
        updated_at: row.get(11)?,
    })
}

fn default_mcp_runtime_state(server: &ParityMcpServer) -> ParityMcpRuntimeState {
    ParityMcpRuntimeState {
        server_id: server.id.clone(),
        runtime_status: default_runtime_status_for_transport(&server.transport).to_string(),
        management_mode: management_mode_for_transport(&server.transport).to_string(),
        pid: None,
        last_started_at: None,
        last_stopped_at: None,
        last_reloaded_at: None,
        last_exit_code: None,
        last_error: None,
        status_message: external_status_message(&server.transport),
        updated_at: now_rfc3339(),
    }
}

fn runtime_status_from_parts(
    server: ParityMcpServer,
    runtime: ParityMcpRuntimeState,
) -> ParityMcpServerRuntimeStatus {
    ParityMcpServerRuntimeStatus {
        id: server.id,
        name: server.name,
        transport: server.transport,
        endpoint: server.endpoint,
        enabled: server.enabled,
        runtime_status: runtime.runtime_status,
        management_mode: runtime.management_mode,
        pid: runtime.pid,
        last_started_at: runtime.last_started_at,
        last_stopped_at: runtime.last_stopped_at,
        last_reloaded_at: runtime.last_reloaded_at,
        last_exit_code: runtime.last_exit_code,
        last_error: runtime.last_error,
        status_message: runtime.status_message,
        updated_at: runtime.updated_at,
    }
}

fn map_mcp_runtime_state_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ParityMcpRuntimeState> {
    Ok(ParityMcpRuntimeState {
        server_id: row.get(0)?,
        runtime_status: row.get(1)?,
        management_mode: row.get(2)?,
        pid: row.get::<_, Option<i64>>(3)?.map(|pid| pid as u32),
        last_started_at: row.get(4)?,
        last_stopped_at: row.get(5)?,
        last_reloaded_at: row.get(6)?,
        last_exit_code: row.get::<_, Option<i64>>(7)?.map(|code| code as i32),
        last_error: row.get(8)?,
        status_message: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

fn map_quick_command_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ParityQuickCommand> {
    Ok(ParityQuickCommand {
        id: row.get(0)?,
        name: row.get(1)?,
        command: row.get(2)?,
        description: row.get(3)?,
        enabled: row.get::<_, i64>(4)? != 0,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::{ParityService, ParityServiceImpl};
    use crate::backend::{Database, ParityCronJobInput, ParityProviderSelectionInput};

    #[test]
    fn provider_selection_round_trips_via_runtime_settings() {
        let db = Database::in_memory().expect("database should initialize");
        let service = ParityServiceImpl::new(db);

        let saved = service
            .save_provider_selection(ParityProviderSelectionInput {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4".to_string(),
                base_url: Some("https://api.anthropic.com".to_string()),
            })
            .expect("provider selection should save");

        assert_eq!(saved.provider, "anthropic");
        assert_eq!(saved.model, "claude-sonnet-4");

        let catalog = service.get_catalog().expect("catalog should load");
        assert_eq!(catalog.active_provider, "anthropic");
        assert_eq!(catalog.active_model, "claude-sonnet-4");
    }

    #[test]
    fn cron_run_now_updates_metadata() {
        let db = Database::in_memory().expect("database should initialize");
        let service = ParityServiceImpl::new(db);

        let job = service
            .create_cron_job(ParityCronJobInput {
                name: "sync".to_string(),
                schedule: "0 * * * *".to_string(),
                prompt: "sync state".to_string(),
                deliver_to: None,
                enabled: true,
            })
            .expect("cron job should create");

        let updated = service
            .run_cron_job_now(&job.id)
            .expect("run now should update metadata");

        assert_eq!(updated.run_count, 1);
        assert_eq!(updated.last_run_status.as_deref(), Some("requested"));
        assert!(updated.last_run_requested_at.is_some());
    }
}
