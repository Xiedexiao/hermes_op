//! 设置相关命令
//!
//! 处理应用设置和运行时设置的读写

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::backend::{AppError, Database, config};

fn default_busy_input_mode() -> String {
    "interrupt".to_string()
}

fn default_busy_input_mode_option() -> Option<String> {
    Some(default_busy_input_mode())
}

fn configured_busy_input_mode() -> String {
    config::load_config()
        .unwrap_or_default()
        .busy_input_mode
        .trim()
        .to_ascii_lowercase()
}

/// 设置响应载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsPayload {
    /// 应用设置
    pub app: AppSettingsPayload,
    /// 运行时设置
    pub runtime: RuntimeSettingsPayload,
}

/// 应用设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettingsPayload {
    #[serde(default)]
    pub theme_mode: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub launch_at_login: Option<bool>,
    #[serde(default)]
    pub default_workspace_path: Option<String>,
    #[serde(default)]
    pub log_level: Option<String>,
    #[serde(default)]
    pub require_approval_for_risk: Option<String>,
}

impl Default for AppSettingsPayload {
    fn default() -> Self {
        Self {
            theme_mode: Some("system".to_string()),
            language: Some("zh-CN".to_string()),
            launch_at_login: Some(false),
            default_workspace_path: Some(String::new()),
            log_level: Some("info".to_string()),
            require_approval_for_risk: Some("high".to_string()),
        }
    }
}

/// 运行时模型档位设置
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeModelProfilePayload {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_ref: Option<String>,
}

/// Native CUA 自动模型路由设置
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimeNativeCuaAutoModelsPayload {
    #[serde(default)]
    pub easy: Option<RuntimeModelProfilePayload>,
    #[serde(default)]
    pub standard: Option<RuntimeModelProfilePayload>,
    #[serde(default)]
    pub hard: Option<RuntimeModelProfilePayload>,
}

/// 运行时设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSettingsPayload {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_ref: Option<String>,
    #[serde(default)]
    pub engine_profile: Option<String>,
    #[serde(default)]
    pub agent_engine_enabled: Option<bool>,
    #[serde(default = "default_busy_input_mode_option")]
    pub busy_input_mode: Option<String>,
    #[serde(default)]
    pub native_cua_auto_models: Option<RuntimeNativeCuaAutoModelsPayload>,
}

impl Default for RuntimeSettingsPayload {
    fn default() -> Self {
        Self {
            provider: Some("openai".to_string()),
            model: Some("gpt-4o".to_string()),
            base_url: None,
            api_key_ref: None,
            engine_profile: Some("default".to_string()),
            agent_engine_enabled: Some(true),
            busy_input_mode: default_busy_input_mode_option(),
            native_cua_auto_models: None,
        }
    }
}

/// 保存设置请求
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSettingsRequest {
    pub app: Option<AppSettingsPayload>,
    pub runtime: Option<RuntimeSettingsPayload>,
}

/// 保存设置响应
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveSettingsResponse {
    pub ok: bool,
}

/// 获取设置
#[tauri::command]
pub fn settings_get(db: State<'_, Database>) -> Result<SettingsPayload, AppError> {
    let app = get_app_settings(&db).unwrap_or_default();
    let runtime = get_runtime_settings(&db).unwrap_or_default();

    Ok(SettingsPayload { app, runtime })
}

/// 保存设置
#[tauri::command]
pub fn settings_save(
    db: State<'_, Database>,
    request: SaveSettingsRequest,
) -> Result<SaveSettingsResponse, AppError> {
    let now = Utc::now().to_rfc3339();

    // 保存应用设置
    if let Some(app) = request.app {
        let json = serde_json::to_string(&app).map_err(AppError::from_json_error)?;
        let params: Vec<&dyn rusqlite::ToSql> = vec![&json, &now];
        db.execute(
            "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES ('app', ?, ?)",
            &params,
        )
        .map_err(|e| AppError::storage(format!("Failed to save app settings: {}", e)))?;
    }

    // 保存运行时设置
    if let Some(runtime) = request.runtime {
        sync_busy_input_mode_to_config(runtime.busy_input_mode.as_deref())?;
        save_runtime_settings_record(&db, &runtime, &now)?;
    }

    tracing::info!("Settings saved");
    Ok(SaveSettingsResponse { ok: true })
}

/// 从数据库获取应用设置
fn get_app_settings(db: &Database) -> Result<AppSettingsPayload, AppError> {
    match db.query_row(
        "SELECT value_json FROM app_settings WHERE key = 'app'",
        &[],
        |row| {
            let value: String = row.get(0)?;
            Ok(value)
        },
    ) {
        Ok(json) => serde_json::from_str(&json).map_err(AppError::from_json_error),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(AppSettingsPayload::default()),
        Err(e) => Err(AppError::storage(format!(
            "Failed to get app settings: {}",
            e
        ))),
    }
}

fn save_runtime_settings_record(
    db: &Database,
    runtime: &RuntimeSettingsPayload,
    updated_at: &str,
) -> Result<(), AppError> {
    let json = serde_json::to_string(runtime).map_err(AppError::from_json_error)?;
    let params: Vec<&dyn rusqlite::ToSql> = vec![&json, &updated_at];
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES ('runtime', ?, ?)",
        &params,
    )
    .map_err(|e| AppError::storage(format!("Failed to save runtime settings: {}", e)))?;

    Ok(())
}

fn sync_busy_input_mode_to_config(value: Option<&str>) -> Result<(), AppError> {
    let mut cfg = config::load_config().unwrap_or_default();
    cfg.busy_input_mode = match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "queue" => "queue".to_string(),
        _ => "interrupt".to_string(),
    };
    config::save_config(&cfg).map_err(AppError::runtime)
}

/// 从数据库获取运行时设置
fn get_runtime_settings(db: &Database) -> Result<RuntimeSettingsPayload, AppError> {
    match db.query_row(
        "SELECT value_json FROM app_settings WHERE key = 'runtime'",
        &[],
        |row| {
            let value: String = row.get(0)?;
            Ok(value)
        },
    ) {
        Ok(json) => {
            let mut runtime: RuntimeSettingsPayload =
                serde_json::from_str(&json).map_err(AppError::from_json_error)?;
            if runtime
                .busy_input_mode
                .as_deref()
                .unwrap_or_default()
                .trim()
                .is_empty()
            {
                runtime.busy_input_mode = Some(configured_busy_input_mode());
            }
            Ok(runtime)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(RuntimeSettingsPayload {
            busy_input_mode: Some(configured_busy_input_mode()),
            ..Default::default()
        }),
        Err(e) => Err(AppError::storage(format!(
            "Failed to get runtime settings: {}",
            e
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeSettingsPayload, configured_busy_input_mode, default_busy_input_mode,
        get_runtime_settings, save_runtime_settings_record, sync_busy_input_mode_to_config,
    };
    use crate::backend::{Database, config};
    use std::env;
    use std::ffi::OsString;
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

    fn unique_temp_home() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        env::temp_dir().join(format!("hermes-settings-test-{timestamp}"))
    }

    fn insert_runtime_settings_row(db: &Database, value_json: &str) {
        let updated_at = "2026-04-24T00:00:00Z";
        let params: Vec<&dyn rusqlite::ToSql> = vec![&value_json, &updated_at];
        db.execute(
            "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES ('runtime', ?, ?)",
            &params,
        )
        .expect("runtime settings row should insert");
    }

    #[test]
    fn runtime_settings_default_uses_interrupt_busy_input_mode() {
        assert_eq!(
            RuntimeSettingsPayload::default().busy_input_mode.as_deref(),
            Some(default_busy_input_mode().as_str())
        );
    }

    #[test]
    fn get_runtime_settings_uses_interrupt_busy_input_mode_when_row_missing() {
        let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let temp_home = unique_temp_home();
        let _home = HomeOverride::set(&temp_home);
        let db = Database::in_memory().expect("database should initialize");

        let runtime = get_runtime_settings(&db).expect("runtime settings should load");

        assert_eq!(
            runtime.busy_input_mode.as_deref(),
            Some(default_busy_input_mode().as_str())
        );
    }

    #[test]
    fn get_runtime_settings_backfills_missing_busy_input_mode_from_legacy_row() {
        let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let temp_home = unique_temp_home();
        let _home = HomeOverride::set(&temp_home);
        let db = Database::in_memory().expect("database should initialize");
        insert_runtime_settings_row(
            &db,
            r#"{
  "provider": "openai",
  "model": "gpt-4o",
  "base_url": null,
  "api_key_ref": null,
  "engine_profile": "default",
  "agent_engine_enabled": true
}"#,
        );

        let runtime = get_runtime_settings(&db).expect("runtime settings should load");

        assert_eq!(
            runtime.busy_input_mode.as_deref(),
            Some(default_busy_input_mode().as_str())
        );
    }

    #[test]
    fn save_runtime_settings_round_trip_preserves_busy_input_mode() {
        let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let temp_home = unique_temp_home();
        let _home = HomeOverride::set(&temp_home);
        let db = Database::in_memory().expect("database should initialize");
        let runtime = RuntimeSettingsPayload {
            provider: Some("openrouter".to_string()),
            model: Some("gpt-5".to_string()),
            base_url: Some("https://example.com".to_string()),
            api_key_ref: Some("key-ref".to_string()),
            engine_profile: Some("default".to_string()),
            agent_engine_enabled: Some(true),
            busy_input_mode: Some("queue".to_string()),
            native_cua_auto_models: None,
        };

        save_runtime_settings_record(&db, &runtime, "2026-04-24T00:00:00Z")
            .expect("runtime settings should save");
        let loaded = get_runtime_settings(&db).expect("runtime settings should load");

        assert_eq!(loaded.provider, runtime.provider);
        assert_eq!(loaded.model, runtime.model);
        assert_eq!(loaded.base_url, runtime.base_url);
        assert_eq!(loaded.api_key_ref, runtime.api_key_ref);
        assert_eq!(loaded.engine_profile, runtime.engine_profile);
        assert_eq!(loaded.agent_engine_enabled, runtime.agent_engine_enabled);
        assert_eq!(loaded.busy_input_mode, runtime.busy_input_mode);
        assert_eq!(
            loaded.native_cua_auto_models,
            runtime.native_cua_auto_models
        );
    }

    #[test]
    fn configured_busy_input_mode_reads_config_file() {
        let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let temp_home = unique_temp_home();
        let _home = HomeOverride::set(&temp_home);

        let cfg = config::HermesConfig {
            busy_input_mode: "queue".to_string(),
            ..Default::default()
        };
        config::save_config(&cfg).expect("save config");

        assert_eq!(configured_busy_input_mode(), "queue");
    }

    #[test]
    fn sync_busy_input_mode_to_config_updates_config_file() {
        let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let temp_home = unique_temp_home();
        let _home = HomeOverride::set(&temp_home);

        sync_busy_input_mode_to_config(Some("queue")).expect("sync queue");
        assert_eq!(
            config::load_config().expect("load config").busy_input_mode,
            "queue"
        );

        sync_busy_input_mode_to_config(Some("invalid")).expect("sync fallback");
        assert_eq!(
            config::load_config().expect("load config").busy_input_mode,
            "interrupt"
        );
    }
}
