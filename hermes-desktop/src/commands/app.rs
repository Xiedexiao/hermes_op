//! 应用相关命令
//!
//! 处理应用级命令，如 bootstrap

use chrono::Utc;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::Arc;
use tauri::State;

use crate::backend::{
    AppError, ControlApiServerHandle, Database, Mission, MissionPriority, MissionStatus, config,
    current_engine_heartbeat,
};
use crate::commands::sessions::{ActiveSessionSelection, session_get_active_for_db};

// Keep this in sync with cli/foreground_store.rs; the desktop library cannot import the bin-only CLI module.
const FOREGROUND_SNAPSHOT_KEY: &str = "cli_foreground_snapshot";

fn default_busy_input_mode() -> String {
    "interrupt".to_string()
}

fn configured_busy_input_mode() -> String {
    config::load_config()
        .unwrap_or_default()
        .busy_input_mode
        .trim()
        .to_ascii_lowercase()
}

/// Bootstrap 响应载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapPayload {
    /// 应用设置
    pub app_settings: AppSettingsPayload,
    /// 运行时设置
    pub runtime_settings: RuntimeSettingsPayload,
    /// Agent Engine 状态
    pub engine_status: EngineStatusPayload,
    /// Hermes 状态
    pub hermes_status: HermesStatusPayload,
    /// 前台快照
    pub foreground_snapshot: ForegroundSnapshotPayload,
    /// 当前恢复中的 session
    pub active_session: Option<ActiveSessionSelection>,
    /// 应用重启后应恢复的未完成 Mission
    pub active_mission: Option<Mission>,
    /// 摘要信息
    pub summary: SummaryPayload,
}

/// 应用设置载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettingsPayload {
    #[serde(default)]
    pub theme_mode: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub launch_at_login: bool,
    #[serde(default)]
    pub default_workspace_path: String,
    #[serde(default)]
    pub log_level: String,
    #[serde(default)]
    pub require_approval_for_risk: String,
}

impl Default for AppSettingsPayload {
    fn default() -> Self {
        Self {
            theme_mode: "system".to_string(),
            language: "zh-CN".to_string(),
            launch_at_login: false,
            default_workspace_path: String::new(),
            log_level: "info".to_string(),
            require_approval_for_risk: "high".to_string(),
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

/// 运行时设置载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeSettingsPayload {
    #[serde(default)]
    pub provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub base_url: Option<String>,
    #[serde(default)]
    pub api_key_ref: Option<String>,
    #[serde(default)]
    pub engine_profile: Option<String>,
    #[serde(default)]
    pub agent_engine_enabled: bool,
    #[serde(default = "default_busy_input_mode")]
    pub busy_input_mode: String,
    #[serde(default)]
    pub native_cua_auto_models: Option<RuntimeNativeCuaAutoModelsPayload>,
}

impl Default for RuntimeSettingsPayload {
    fn default() -> Self {
        Self {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            base_url: None,
            api_key_ref: None,
            engine_profile: Some("default".to_string()),
            agent_engine_enabled: true,
            busy_input_mode: default_busy_input_mode(),
            native_cua_auto_models: None,
        }
    }
}

/// 引擎状态载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EngineStatusPayload {
    pub running: bool,
    pub profile: Option<String>,
    pub pid: Option<u32>,
    pub last_error: Option<String>,
}

/// Hermes 状态载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesStatusPayload {
    pub installed: bool,
    pub running: bool,
    pub version: Option<String>,
}

/// Foreground 快照载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForegroundSnapshotPayload {
    pub active: bool,
    pub state: String,
    pub session_id: Option<String>,
    pub run_id: Option<String>,
    pub cancel_state: Option<String>,
    pub pending_count: usize,
    pub interrupt_count: usize,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredForegroundSnapshot {
    active: bool,
    state: String,
    session_id: Option<String>,
    run_id: Option<String>,
    cancel_state: Option<String>,
    pending_count: usize,
    interrupt_count: usize,
    updated_at: String,
}

/// 摘要载荷
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryPayload {
    pub active_mission_count: i64,
    pub pending_approval_count: i64,
    pub recent_session_count: i64,
    pub has_recent_session: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceDiagnosticsPayload {
    pub paths: WorkspacePathsPayload,
    pub status: WorkspaceStatusPayload,
    pub counts: WorkspaceCountsPayload,
    #[serde(default)]
    pub recent_logs: Vec<WorkspaceLogFilePayload>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspacePathsPayload {
    pub config_dir: String,
    pub data_dir: String,
    pub log_dir: String,
    pub db_path: String,
    pub control_api_url: String,
    pub default_workspace_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatusPayload {
    pub config_exists: bool,
    pub database_exists: bool,
    pub default_workspace_exists: bool,
    pub engine_last_heartbeat_at: Option<String>,
    pub engine_queued_background_runs: u32,
    pub engine_awaiting_approval_steps: u32,
    pub foreground_updated_at: String,
    pub cron_last_heartbeat_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceCountsPayload {
    pub missions: u32,
    pub sessions: u32,
    pub knowledge_sources: u32,
    pub memory_records: u32,
    pub run_events: u32,
    pub cron_jobs: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceLogFilePayload {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
}

/// 获取引导数据
#[tauri::command]
pub fn app_get_bootstrap(
    state: State<'_, Arc<RwLock<crate::backend::AppState>>>,
    db: State<'_, Database>,
) -> Result<BootstrapPayload, AppError> {
    // 获取应用状态
    let app_state = state.read();

    // 从数据库获取应用设置
    let app_settings = get_app_settings(&db).unwrap_or_default();
    let runtime_settings = get_runtime_settings(&db).unwrap_or_default();
    let foreground_snapshot = get_foreground_snapshot(&db)?;
    let active_session = session_get_active_for_db(&db).unwrap_or(None);
    let active_mission = get_recoverable_mission(&db)?;

    // 获取统计信息
    let (active_mission_count, pending_approval_count) = get_summary_stats(&db);
    let (recent_session_count, has_recent_session) = get_session_summary_stats(&db);

    Ok(BootstrapPayload {
        app_settings,
        runtime_settings,
        engine_status: EngineStatusPayload {
            running: app_state.engine_status.running,
            profile: app_state.engine_status.profile.clone(),
            pid: app_state.engine_status.pid,
            last_error: app_state.engine_status.last_error.clone(),
        },
        hermes_status: HermesStatusPayload {
            installed: app_state.hermes_status.installed,
            running: app_state.hermes_status.running,
            version: app_state.hermes_status.version.clone(),
        },
        foreground_snapshot,
        active_session,
        active_mission,
        summary: SummaryPayload {
            active_mission_count,
            pending_approval_count,
            recent_session_count,
            has_recent_session,
        },
    })
}

#[tauri::command]
pub fn app_get_workspace_diagnostics(
    state: State<'_, Arc<RwLock<crate::backend::AppState>>>,
    db: State<'_, Database>,
    control_api: State<'_, ControlApiServerHandle>,
) -> Result<WorkspaceDiagnosticsPayload, AppError> {
    let app_state = state.read().clone();
    let foreground_snapshot = get_foreground_snapshot(&db)?;
    let engine_heartbeat = current_engine_heartbeat(&app_state);
    let app_settings = get_app_settings(&db).unwrap_or_default();
    build_workspace_diagnostics(
        &db,
        &app_state,
        &foreground_snapshot,
        engine_heartbeat,
        &control_api_url(control_api.inner()),
        app_settings.default_workspace_path,
    )
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
            if runtime.busy_input_mode.trim().is_empty() {
                runtime.busy_input_mode = configured_busy_input_mode();
            }
            Ok(runtime)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(RuntimeSettingsPayload {
            busy_input_mode: configured_busy_input_mode(),
            ..Default::default()
        }),
        Err(e) => Err(AppError::storage(format!(
            "Failed to get runtime settings: {}",
            e
        ))),
    }
}

pub(crate) fn get_foreground_snapshot(
    db: &Database,
) -> Result<ForegroundSnapshotPayload, AppError> {
    let snapshot = db
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?",
            &[&FOREGROUND_SNAPSHOT_KEY as &dyn rusqlite::ToSql],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|json| serde_json::from_str::<StoredForegroundSnapshot>(&json).ok())
        .unwrap_or_else(default_foreground_snapshot);

    Ok(ForegroundSnapshotPayload {
        active: snapshot.active,
        state: snapshot.state,
        session_id: snapshot.session_id,
        run_id: snapshot.run_id,
        cancel_state: snapshot.cancel_state,
        pending_count: snapshot.pending_count,
        interrupt_count: snapshot.interrupt_count,
        updated_at: snapshot.updated_at,
    })
}

fn default_foreground_snapshot() -> StoredForegroundSnapshot {
    StoredForegroundSnapshot {
        active: false,
        state: "idle".to_string(),
        session_id: None,
        run_id: None,
        cancel_state: None,
        pending_count: 0,
        interrupt_count: 0,
        updated_at: Utc::now().to_rfc3339(),
    }
}

/// 获取摘要统计
fn get_summary_stats(db: &Database) -> (i64, i64) {
    let active_count = db
        .query_row(
            "SELECT COUNT(*) FROM missions WHERE status NOT IN ('archived', 'completed', 'failed')",
            &[],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0);

    let pending_count = db
        .query_row(
            "SELECT COUNT(*) FROM execution_steps WHERE status = 'awaiting_approval'",
            &[],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0);

    (active_count, pending_count)
}

fn get_session_summary_stats(db: &Database) -> (i64, bool) {
    let recent_session_count = db
        .query_row(
            "SELECT COUNT(*) FROM (
                SELECT 1
                FROM sessions
                ORDER BY datetime(updated_at) DESC, rowid DESC
                LIMIT 20
            )",
            &[],
            |row| row.get::<_, i64>(0),
        )
        .unwrap_or(0);

    (recent_session_count, recent_session_count > 0)
}

fn get_recoverable_mission(db: &Database) -> Result<Option<Mission>, AppError> {
    match db.query_row(
        "SELECT
            id, title, goal, constraints_json, success_criteria_json,
            status, priority, pinned, created_at, updated_at, last_activity_at
         FROM missions
         WHERE status NOT IN ('archived', 'completed', 'failed')
         ORDER BY pinned DESC, datetime(last_activity_at) DESC, rowid DESC
         LIMIT 1",
        &[],
        map_recoverable_mission_row,
    ) {
        Ok(mission) => Ok(Some(mission)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(AppError::storage(format!(
            "Failed to recover active mission: {}",
            error
        ))),
    }
}

fn map_recoverable_mission_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Mission> {
    let constraints_json: String = row.get(3)?;
    let success_criteria_json: String = row.get(4)?;
    let status: String = row.get(5)?;
    let priority: String = row.get(6)?;

    Ok(Mission {
        id: row.get(0)?,
        title: row.get(1)?,
        goal: row.get(2)?,
        constraints: serde_json::from_str(&constraints_json).unwrap_or_default(),
        success_criteria: serde_json::from_str(&success_criteria_json).unwrap_or_default(),
        status: MissionStatus::from_key(&status),
        priority: MissionPriority::from_key(&priority),
        pinned: row.get::<_, i64>(7)? != 0,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        last_activity_at: row.get(10)?,
    })
}

fn build_workspace_diagnostics(
    db: &Database,
    app_state: &crate::backend::AppState,
    foreground_snapshot: &ForegroundSnapshotPayload,
    engine_heartbeat: Option<crate::backend::EngineHeartbeat>,
    control_api_url: &str,
    default_workspace_path: String,
) -> Result<WorkspaceDiagnosticsPayload, AppError> {
    let config_dir = app_state.config_dir.to_string_lossy().to_string();
    let data_dir = app_state.data_dir.to_string_lossy().to_string();
    let log_dir = app_state.log_dir.to_string_lossy().to_string();
    let db_path = app_state.db_path.to_string_lossy().to_string();
    let default_workspace_path = default_workspace_path.trim().to_string();
    let default_workspace_value = if default_workspace_path.is_empty() {
        None
    } else {
        Some(default_workspace_path.clone())
    };
    let default_workspace_exists = default_workspace_value
        .as_deref()
        .map(|path| std::path::Path::new(path).exists())
        .unwrap_or(false);
    let recent_logs = collect_recent_logs(&app_state.log_dir)?;
    let counts = WorkspaceCountsPayload {
        missions: query_table_count(db, "missions")?,
        sessions: query_table_count(db, "sessions")?,
        knowledge_sources: query_table_count(db, "knowledge_sources")?,
        memory_records: query_table_count(db, "memory_records")?,
        run_events: query_table_count(db, "run_events")?,
        cron_jobs: query_table_count(db, "parity_cron_jobs")?,
    };
    let cron_last_heartbeat_at = db
        .query_row(
            "SELECT last_heartbeat_at FROM parity_cron_runtime_heartbeat WHERE id = 'local'",
            &[],
            |row| row.get::<_, Option<String>>(0),
        )
        .ok()
        .flatten();

    Ok(WorkspaceDiagnosticsPayload {
        paths: WorkspacePathsPayload {
            config_dir: config_dir.clone(),
            data_dir,
            log_dir: log_dir.clone(),
            db_path: db_path.clone(),
            control_api_url: control_api_url.to_string(),
            default_workspace_path: default_workspace_value,
        },
        status: WorkspaceStatusPayload {
            config_exists: std::path::Path::new(&config_dir).exists(),
            database_exists: std::path::Path::new(&db_path).exists(),
            default_workspace_exists,
            engine_last_heartbeat_at: engine_heartbeat
                .as_ref()
                .map(|item| item.last_heartbeat_at.clone()),
            engine_queued_background_runs: engine_heartbeat
                .as_ref()
                .map(|item| item.queued_background_runs)
                .unwrap_or(0),
            engine_awaiting_approval_steps: engine_heartbeat
                .as_ref()
                .map(|item| item.awaiting_approval_steps)
                .unwrap_or(0),
            foreground_updated_at: foreground_snapshot.updated_at.clone(),
            cron_last_heartbeat_at,
        },
        counts,
        recent_logs,
    })
}

fn query_table_count(db: &Database, table: &str) -> Result<u32, AppError> {
    let sql = format!("SELECT COUNT(*) FROM {table}");
    let count: i64 = db
        .query_row(&sql, &[], |row| row.get(0))
        .map_err(|err| AppError::storage(format!("Failed to count table {table}: {err}")))?;
    Ok(count.max(0) as u32)
}

fn collect_recent_logs(dir: &std::path::Path) -> Result<Vec<WorkspaceLogFilePayload>, AppError> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut files = fs::read_dir(dir)
        .map_err(|err| AppError::io(format!("Failed to read log dir: {err}")))?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let path = entry.path();
            let metadata = entry.metadata().ok()?;
            if !metadata.is_file() {
                return None;
            }
            let modified = metadata.modified().ok()?;
            Some((
                modified,
                WorkspaceLogFilePayload {
                    name: entry.file_name().to_string_lossy().to_string(),
                    path: path.to_string_lossy().to_string(),
                    size_bytes: metadata.len(),
                },
            ))
        })
        .collect::<Vec<_>>();
    files.sort_by_key(|item| std::cmp::Reverse(item.0));
    Ok(files.into_iter().take(5).map(|(_, item)| item).collect())
}

fn control_api_url(handle: &ControlApiServerHandle) -> String {
    format!("http://{}:{}/api/control", handle.host(), handle.port())
}

#[cfg(test)]
mod tests {
    use super::{
        ForegroundSnapshotPayload, RuntimeSettingsPayload, build_workspace_diagnostics,
        configured_busy_input_mode, default_busy_input_mode, get_foreground_snapshot,
        get_recoverable_mission, get_runtime_settings, get_session_summary_stats,
    };
    use crate::backend::{
        AppState, CreateSessionInput, Database, EngineHeartbeat, ParityCronRuntimeService,
        SessionRepository, SessionSource, config,
    };
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
        env::temp_dir().join(format!("hermes-app-test-{timestamp}"))
    }

    fn sample_session(title: &str) -> CreateSessionInput {
        CreateSessionInput {
            source: SessionSource::Cli,
            title: title.to_string(),
            model_name: Some("gpt-4o".to_string()),
            parent_session_id: None,
        }
    }

    fn make_test_state() -> (AppState, PathBuf) {
        let base_dir = unique_temp_home();
        let config_dir = base_dir.join("config");
        let data_dir = base_dir.join("data");
        let log_dir = base_dir.join("logs");

        std::fs::create_dir_all(&config_dir).expect("config dir should create");
        std::fs::create_dir_all(&data_dir).expect("data dir should create");
        std::fs::create_dir_all(&log_dir).expect("log dir should create");

        (
            AppState {
                config_dir,
                db_path: data_dir.join("hermes.db"),
                log_dir,
                data_dir,
                engine_status: crate::backend::AgentEngineStatus::new(),
                hermes_status: crate::backend::HermesStatus::new(),
            },
            base_dir,
        )
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

    fn insert_foreground_snapshot_row(db: &Database, value_json: &str) {
        let updated_at = "2026-04-24T00:00:00Z";
        let params: Vec<&dyn rusqlite::ToSql> =
            vec![&super::FOREGROUND_SNAPSHOT_KEY, &value_json, &updated_at];
        db.execute(
            "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?, ?, ?)",
            &params,
        )
        .expect("foreground snapshot row should insert");
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_mission_row(
        db: &Database,
        id: &str,
        title: &str,
        goal: &str,
        status: &str,
        priority: &str,
        pinned: bool,
        last_activity_at: &str,
    ) {
        db.execute(
            "INSERT INTO missions (
                id, title, goal, constraints_json, success_criteria_json,
                status, priority, pinned, created_at, updated_at, last_activity_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &id as &dyn rusqlite::ToSql,
                &title,
                &goal,
                &"[]",
                &"[]",
                &status,
                &priority,
                &(pinned as i64),
                &last_activity_at,
                &last_activity_at,
                &last_activity_at,
            ],
        )
        .expect("mission row should insert");
    }

    #[test]
    fn runtime_settings_default_uses_interrupt_busy_input_mode() {
        assert_eq!(
            RuntimeSettingsPayload::default().busy_input_mode,
            default_busy_input_mode()
        );
    }

    #[test]
    fn get_runtime_settings_uses_interrupt_busy_input_mode_when_row_missing() {
        let _guard = env_lock().lock().unwrap_or_else(|error| error.into_inner());
        let temp_home = unique_temp_home();
        let _home = HomeOverride::set(&temp_home);
        let db = Database::in_memory().expect("database should initialize");

        let runtime = get_runtime_settings(&db).expect("runtime settings should load");

        assert_eq!(runtime.busy_input_mode, default_busy_input_mode());
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

        assert_eq!(runtime.busy_input_mode, default_busy_input_mode());
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
    fn session_summary_stats_report_empty_state_without_sessions() {
        let db = Database::in_memory().expect("database should initialize");

        let (recent_session_count, has_recent_session) = get_session_summary_stats(&db);

        assert_eq!(recent_session_count, 0);
        assert!(!has_recent_session);
    }

    #[test]
    fn session_summary_stats_count_existing_sessions_and_detect_presence() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = SessionRepository::new(db.clone());

        repo.create(sample_session("First session"))
            .expect("first session should create");
        repo.create(sample_session("Second session"))
            .expect("second session should create");

        let (recent_session_count, has_recent_session) = get_session_summary_stats(&db);

        assert_eq!(recent_session_count, 2);
        assert!(has_recent_session);
    }

    #[test]
    fn recoverable_mission_prefers_pinned_then_recent_unfinished_work() {
        let db = Database::in_memory().expect("database should initialize");
        insert_mission_row(
            &db,
            "completed-newer",
            "Completed newer",
            "Completed work should not resume",
            "completed",
            "high",
            false,
            "2026-04-26T09:00:00Z",
        );
        insert_mission_row(
            &db,
            "active-recent",
            "Recent active",
            "Most recent unfinished mission",
            "executing",
            "medium",
            false,
            "2026-04-26T08:00:00Z",
        );
        insert_mission_row(
            &db,
            "active-pinned",
            "Pinned active",
            "Pinned unfinished mission should resume first",
            "paused",
            "high",
            true,
            "2026-04-26T07:00:00Z",
        );

        let mission = get_recoverable_mission(&db)
            .expect("recoverable mission should load")
            .expect("unfinished mission should be selected");

        assert_eq!(mission.id, "active-pinned");
        assert_eq!(mission.title, "Pinned active");
        assert_eq!(mission.status.as_str(), "paused");
        assert!(mission.pinned);
    }

    #[test]
    fn recoverable_mission_ignores_terminal_missions() {
        let db = Database::in_memory().expect("database should initialize");
        insert_mission_row(
            &db,
            "completed",
            "Completed",
            "Terminal mission",
            "completed",
            "medium",
            true,
            "2026-04-26T09:00:00Z",
        );
        insert_mission_row(
            &db,
            "failed",
            "Failed",
            "Terminal mission",
            "failed",
            "high",
            false,
            "2026-04-26T08:00:00Z",
        );
        insert_mission_row(
            &db,
            "archived",
            "Archived",
            "Terminal mission",
            "archived",
            "low",
            false,
            "2026-04-26T07:00:00Z",
        );

        let mission = get_recoverable_mission(&db).expect("recoverable mission should load");

        assert!(mission.is_none());
    }

    #[test]
    fn get_foreground_snapshot_reads_persisted_snapshot() {
        let db = Database::in_memory().expect("database should initialize");
        insert_foreground_snapshot_row(
            &db,
            r#"{
  "active": true,
  "state": "running",
  "session_id": "session-123",
  "run_id": "run-456",
  "cancel_state": "requested",
  "pending_count": 3,
  "interrupt_count": 1,
  "updated_at": "2026-04-24T00:00:00Z"
}"#,
        );

        let payload = get_foreground_snapshot(&db).expect("snapshot should load");

        assert!(payload.active);
        assert_eq!(payload.state, "running");
        assert_eq!(payload.session_id.as_deref(), Some("session-123"));
        assert_eq!(payload.run_id.as_deref(), Some("run-456"));
        assert_eq!(payload.cancel_state.as_deref(), Some("requested"));
        assert_eq!(payload.pending_count, 3);
        assert_eq!(payload.interrupt_count, 1);
        assert_eq!(payload.updated_at, "2026-04-24T00:00:00Z");
    }

    #[test]
    fn build_workspace_diagnostics_reports_real_paths_counts_and_logs() {
        let (app_state, base_dir) = make_test_state();
        let db = Database::new(&app_state.db_path).expect("db should initialize");
        ParityCronRuntimeService::new(db.clone())
            .ensure_schema()
            .expect("cron runtime schema should initialize");
        let repo = SessionRepository::new(db.clone());
        repo.create(sample_session("Diagnostics session"))
            .expect("session should create");
        db.execute(
            "INSERT INTO missions (
                id, title, goal, constraints_json, success_criteria_json,
                status, priority, pinned, created_at, updated_at, last_activity_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"mission-001" as &dyn rusqlite::ToSql,
                &"Diagnostics mission".to_string(),
                &"Verify workspace diagnostics".to_string(),
                &"[]".to_string(),
                &"[]".to_string(),
                &"planning",
                &"medium",
                &0_i64,
                &"2026-04-25T00:00:00Z".to_string(),
                &"2026-04-25T00:00:00Z".to_string(),
                &"2026-04-25T00:00:00Z".to_string(),
            ],
        )
        .expect("mission should insert");
        db.execute(
            "INSERT INTO runs (id, mission_id, type, status, started_at, summary)
             VALUES (?, ?, ?, ?, ?, ?)",
            &[
                &"run-001" as &dyn rusqlite::ToSql,
                &"mission-001",
                &"execution",
                &"queued",
                &Some("2026-04-25T00:00:00Z".to_string()),
                &Some("Queued diagnostics".to_string()),
            ],
        )
        .expect("run should insert");
        db.execute(
            "INSERT INTO run_events (id, run_id, mission_id, event_type, message, payload_json, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            &[
                &"event-001" as &dyn rusqlite::ToSql,
                &"run-001",
                &"mission-001",
                &"background_enqueued",
                &"Queued diagnostics".to_string(),
                &Option::<String>::None,
                &"2026-04-25T00:00:00Z".to_string(),
            ],
        )
        .expect("run event should insert");
        db.execute(
            "INSERT INTO parity_cron_jobs (
                id, name, schedule, prompt, deliver_to, enabled, status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"cron-001" as &dyn rusqlite::ToSql,
                &"Daily brief".to_string(),
                &"0 9 * * *".to_string(),
                &"Summarize work".to_string(),
                &Some("desktop".to_string()),
                &1_i64,
                &"idle".to_string(),
                &"2026-04-25T00:00:00Z".to_string(),
                &"2026-04-25T00:00:00Z".to_string(),
            ],
        )
        .expect("cron job should insert");
        db.execute(
            "UPDATE parity_cron_runtime_heartbeat
             SET last_heartbeat_at = ?, last_dispatch_count = ?
             WHERE id = 'local'",
            &[&"2026-04-25T01:00:00Z" as &dyn rusqlite::ToSql, &2_i64],
        )
        .expect("cron heartbeat should update");
        std::fs::write(app_state.log_dir.join("engine.log"), "runtime ok")
            .expect("log file should write");

        let diagnostics = build_workspace_diagnostics(
            &db,
            &app_state,
            &ForegroundSnapshotPayload {
                active: false,
                state: "idle".to_string(),
                session_id: None,
                run_id: None,
                cancel_state: None,
                pending_count: 0,
                interrupt_count: 0,
                updated_at: "2026-04-25T02:00:00Z".to_string(),
            },
            Some(EngineHeartbeat {
                profile: "ops".to_string(),
                started_at: "2026-04-25T00:00:00Z".to_string(),
                last_heartbeat_at: "2026-04-25T02:30:00Z".to_string(),
                queued_background_runs: 1,
                awaiting_approval_steps: 3,
            }),
            "http://127.0.0.1:47831/api/control",
            base_dir.to_string_lossy().to_string(),
        )
        .expect("diagnostics should build");

        assert!(diagnostics.paths.db_path.ends_with("hermes.db"));
        assert_eq!(diagnostics.counts.missions, 1);
        assert_eq!(diagnostics.counts.sessions, 1);
        assert_eq!(diagnostics.counts.run_events, 1);
        assert_eq!(diagnostics.counts.cron_jobs, 1);
        assert_eq!(
            diagnostics.status.engine_last_heartbeat_at.as_deref(),
            Some("2026-04-25T02:30:00Z")
        );
        assert_eq!(diagnostics.status.engine_queued_background_runs, 1);
        assert_eq!(diagnostics.status.engine_awaiting_approval_steps, 3);
        assert_eq!(
            diagnostics.status.cron_last_heartbeat_at.as_deref(),
            Some("2026-04-25T01:00:00Z")
        );
        assert_eq!(diagnostics.recent_logs.len(), 1);
        assert!(diagnostics.status.default_workspace_exists);

        std::fs::remove_dir_all(base_dir).expect("temp dir should clean");
    }

    #[test]
    fn control_api_url_formats_host_and_port() {
        let handle = crate::backend::ControlApiConfig {
            host: "127.0.0.1".to_string(),
            port: 47_831,
        };
        let formatted = format!("http://{}:{}/api/control", handle.host, handle.port);
        assert_eq!(formatted, "http://127.0.0.1:47831/api/control");
    }
}
