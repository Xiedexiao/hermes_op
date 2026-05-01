//! Agent Core 引擎服务
//!
//! 提供 Agent 引擎的状态管理和操作接口

use crate::backend::errors::AppResult;
use crate::backend::{AgentEngineStatus, SharedAppState};

use super::process_manager::{start_agent_process, stop_agent_process};

/// Agent Engine 服务 trait
pub trait AgentEngineService: Send + Sync {
    /// 获取引擎状态
    fn status(&self) -> AppResult<AgentEngineStatus>;

    /// 启动引擎
    fn start(&self, profile: Option<String>) -> AppResult<AgentEngineStatus>;

    /// 停止引擎
    fn stop(&self) -> AppResult<AgentEngineStatus>;

    /// 重启引擎
    fn restart(&self) -> AppResult<AgentEngineStatus>;
}

/// Agent Engine 服务实现
pub struct AgentEngineServiceImpl {
    state: SharedAppState,
}

impl AgentEngineServiceImpl {
    pub fn new(state: SharedAppState) -> Self {
        Self { state }
    }
}

impl AgentEngineService for AgentEngineServiceImpl {
    fn status(&self) -> AppResult<AgentEngineStatus> {
        let mut state = self.state.write();
        if let Some((pid, profile)) = super::process_manager::current_engine_runtime_state(&state) {
            state.engine_status.set_running(pid, profile);
        } else {
            state.engine_status.set_stopped();
        }

        Ok(state.engine_status.clone())
    }

    fn start(&self, profile: Option<String>) -> AppResult<AgentEngineStatus> {
        let mut state = self.state.write();
        let profile = profile.unwrap_or_else(|| "default".to_string());

        start_agent_process(&mut state, &profile)?;

        Ok(state.engine_status.clone())
    }

    fn stop(&self) -> AppResult<AgentEngineStatus> {
        let mut state = self.state.write();

        stop_agent_process(&mut state)?;

        Ok(state.engine_status.clone())
    }

    fn restart(&self) -> AppResult<AgentEngineStatus> {
        let mut state = self.state.write();

        // 先停止
        let _ = stop_agent_process(&mut state);

        // 再启动
        let profile = state
            .engine_status
            .profile
            .clone()
            .unwrap_or_else(|| "default".to_string());
        start_agent_process(&mut state, &profile)?;

        Ok(state.engine_status.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{AgentEngineStatus, AppState, HermesStatus};
    use parking_lot::RwLock;
    use std::sync::Arc;
    use uuid::Uuid;

    fn make_test_state() -> (SharedAppState, std::path::PathBuf) {
        let base_dir =
            std::env::temp_dir().join(format!("hermes-desktop-engine-service-{}", Uuid::new_v4()));
        let config_dir = base_dir.join("config");
        let data_dir = base_dir.join("data");
        let log_dir = base_dir.join("logs");

        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&log_dir).unwrap();

        let state = AppState {
            config_dir,
            db_path: data_dir.join("hermes.db"),
            log_dir,
            data_dir,
            engine_status: AgentEngineStatus::new(),
            hermes_status: HermesStatus::new(),
        };

        (Arc::new(RwLock::new(state)), base_dir)
    }

    #[test]
    fn test_agent_engine_service_status() {
        let (state, base_dir) = make_test_state();
        let service = AgentEngineServiceImpl::new(state.clone());

        let status = service.status().unwrap();
        assert!(!status.running);
        assert!(status.pid.is_none());

        std::fs::remove_dir_all(base_dir).unwrap();
    }

    #[test]
    fn test_agent_engine_start_stop() {
        let (state, base_dir) = make_test_state();
        let service = AgentEngineServiceImpl::new(state.clone());

        // 启动引擎
        let status = service.start(Some("test".to_string())).unwrap();
        assert!(status.running);
        assert_eq!(status.profile, Some("test".to_string()));
        assert_ne!(status.pid, Some(std::process::id()));

        // 停止引擎
        let status = service.stop().unwrap();
        assert!(!status.running);

        std::fs::remove_dir_all(base_dir).unwrap();
    }

    #[test]
    fn test_agent_engine_restart() {
        let (state, base_dir) = make_test_state();
        let service = AgentEngineServiceImpl::new(state.clone());

        // 启动引擎
        let first = service.start(None).unwrap();

        // 重启引擎
        let status = service.restart().unwrap();
        assert!(status.running);
        assert_ne!(status.pid, first.pid);

        // 清理
        let _ = service.stop();
        std::fs::remove_dir_all(base_dir).unwrap();
    }

    #[test]
    fn test_agent_engine_service_status_recovers_from_runtime_metadata_after_restart() {
        let (state, base_dir) = make_test_state();
        let service = AgentEngineServiceImpl::new(state.clone());
        let started = service.start(Some("persisted".to_string())).unwrap();
        let data_dir = state.read().data_dir.clone();

        let recovered_state = AppState {
            config_dir: base_dir.join("config"),
            db_path: data_dir.join("hermes.db"),
            log_dir: base_dir.join("logs"),
            data_dir,
            engine_status: AgentEngineStatus::new(),
            hermes_status: HermesStatus::new(),
        };
        let recovered = AgentEngineServiceImpl::new(Arc::new(RwLock::new(recovered_state)));

        let status = recovered.status().unwrap();
        assert!(status.running);
        assert_eq!(status.pid, started.pid);
        assert_eq!(status.profile.as_deref(), Some("persisted"));

        let _ = recovered.stop();
        std::fs::remove_dir_all(base_dir).unwrap();
    }

    #[test]
    fn test_agent_engine_service_status_cleans_stale_runtime_metadata() {
        let (state, base_dir) = make_test_state();
        let data_dir = state.read().data_dir.clone();
        let engine_state_path = data_dir.join("engine.state");
        let engine_lock_path = data_dir.join("engine.lock");

        std::fs::write(
            &engine_state_path,
            serde_json::json!({
                "pid": 999_991_u32,
                "profile": "stale",
                "started_at": "2026-04-22T00:00:00Z",
                "command": "sleep"
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(&engine_lock_path, "{\"pid\":999991}").unwrap();

        let service = AgentEngineServiceImpl::new(state.clone());
        let status = service.status().unwrap();

        assert!(!status.running);
        assert!(status.pid.is_none());
        assert!(!engine_state_path.exists());
        assert!(!engine_lock_path.exists());

        std::fs::remove_dir_all(base_dir).unwrap();
    }
}
