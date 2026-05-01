//! 运行时相关命令
//!
//! 处理 Agent Engine 和 Hermes 运行时的启停控制

use parking_lot::RwLock;
use std::sync::Arc;
use tauri::State;

use crate::backend::agent_core::AgentEngineService;
use crate::backend::{AppError, Database, agent_core, hermes};
use crate::commands::app::{ForegroundSnapshotPayload, get_foreground_snapshot};

/// 运行时状态响应
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct RuntimeStatusResponse {
    /// 引擎状态
    pub engine: EngineState,
    /// Hermes 状态
    pub hermes: HermesState,
    /// Foreground 快照
    pub foreground_snapshot: ForegroundSnapshotPayload,
}

/// 引擎状态
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct EngineState {
    pub running: bool,
    pub profile: Option<String>,
    pub pid: Option<u32>,
    pub started_at: Option<String>,
    pub last_heartbeat_at: Option<String>,
    pub queued_background_runs: u32,
    pub awaiting_approval_steps: u32,
    pub last_error: Option<String>,
}

/// Hermes 状态
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct HermesState {
    pub installed: bool,
    pub running: bool,
    pub version: Option<String>,
}

/// 获取运行时状态
#[tauri::command]
pub fn runtime_get_status(
    state: State<'_, Arc<RwLock<crate::backend::AppState>>>,
    db: State<'_, Database>,
) -> Result<RuntimeStatusResponse, AppError> {
    let mut app_state = state.write();

    // 获取 Agent Core 状态
    let engine_status = agent_core::AgentEngineServiceImpl::new(Arc::clone(&state))
        .status()
        .unwrap_or_else(|_| app_state.engine_status.clone());
    let engine_heartbeat = agent_core::current_engine_heartbeat(&app_state);
    let hermes_status = hermes::get_status();
    let foreground_snapshot = get_foreground_snapshot(&db)?;
    app_state.hermes_status.installed = hermes_status.installed;
    app_state.hermes_status.running = hermes_status.running;
    app_state.hermes_status.version = hermes_status.version.clone();

    Ok(RuntimeStatusResponse {
        engine: EngineState {
            running: engine_status.running,
            profile: engine_status.profile,
            pid: engine_status.pid,
            started_at: engine_heartbeat
                .as_ref()
                .map(|item| item.started_at.clone()),
            last_heartbeat_at: engine_heartbeat
                .as_ref()
                .map(|item| item.last_heartbeat_at.clone()),
            queued_background_runs: engine_heartbeat
                .as_ref()
                .map(|item| item.queued_background_runs)
                .unwrap_or(0),
            awaiting_approval_steps: engine_heartbeat
                .as_ref()
                .map(|item| item.awaiting_approval_steps)
                .unwrap_or(0),
            last_error: app_state.engine_status.last_error.clone(),
        },
        hermes: HermesState {
            installed: hermes_status.installed,
            running: hermes_status.running,
            version: hermes_status.version,
        },
        foreground_snapshot,
    })
}

/// 启动 Agent Engine
#[tauri::command]
pub fn runtime_start_engine(
    state: State<'_, Arc<RwLock<crate::backend::AppState>>>,
    db: State<'_, Database>,
) -> Result<RuntimeStatusResponse, AppError> {
    let mut app_state = state.write();

    // 使用 process_manager 启动
    let profile = "default".to_string();
    agent_core::start_agent_process(&mut app_state, &profile)
        .map_err(|e| AppError::runtime(format!("Failed to start engine: {}", e)))?;

    tracing::info!("Agent Engine started");
    drop(app_state);
    runtime_get_status(state, db)
}

/// 停止 Agent Engine
#[tauri::command]
pub fn runtime_stop_engine(
    state: State<'_, Arc<RwLock<crate::backend::AppState>>>,
    db: State<'_, Database>,
) -> Result<RuntimeStatusResponse, AppError> {
    let mut app_state = state.write();

    // 使用 process_manager 停止
    agent_core::stop_agent_process(&mut app_state)
        .map_err(|e| AppError::runtime(format!("Failed to stop engine: {}", e)))?;

    tracing::info!("Agent Engine stopped");
    drop(app_state);
    runtime_get_status(state, db)
}

/// 重启 Agent Engine
#[tauri::command]
pub fn runtime_restart_engine(
    state: State<'_, Arc<RwLock<crate::backend::AppState>>>,
    db: State<'_, Database>,
) -> Result<RuntimeStatusResponse, AppError> {
    let mut app_state = state.write();

    // 先停止
    let _ = agent_core::stop_agent_process(&mut app_state);

    // 再启动
    let profile = "default".to_string();
    agent_core::start_agent_process(&mut app_state, &profile)
        .map_err(|e| AppError::runtime(format!("Failed to restart engine: {}", e)))?;

    tracing::info!("Agent Engine restarted");
    drop(app_state);
    runtime_get_status(state, db)
}
