//! 应用状态管理
//!
//! 管理全局应用状态，包括路径配置、运行时状态等

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::errors::{AppError, AppResult};

/// Agent Core 运行状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AgentEngineStatus {
    /// 是否正在运行
    pub running: bool,
    /// 进程 ID
    pub pid: Option<u32>,
    /// 当前 profile
    pub profile: Option<String>,
    /// 上次错误信息
    pub last_error: Option<String>,
}

impl AgentEngineStatus {
    /// 创建新的引擎状态
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置为运行状态
    pub fn set_running(&mut self, pid: u32, profile: impl Into<String>) {
        self.running = true;
        self.pid = Some(pid);
        self.profile = Some(profile.into());
        self.last_error = None;
    }

    /// 设置为停止状态
    pub fn set_stopped(&mut self) {
        self.running = false;
        self.pid = None;
    }

    /// 设置错误状态
    pub fn set_error(&mut self, error: impl Into<String>) {
        self.last_error = Some(error.into());
    }
}

/// Hermes 安装状态
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HermesStatus {
    /// 是否已安装
    pub installed: bool,
    /// 是否正在运行
    pub running: bool,
    /// 版本号
    pub version: Option<String>,
}

impl HermesStatus {
    /// 创建新的 Hermes 状态
    pub fn new() -> Self {
        Self::default()
    }
}

/// 应用全局状态
#[derive(Debug, Clone)]
pub struct AppState {
    /// 配置目录路径
    pub config_dir: PathBuf,
    /// SQLite 数据库路径
    pub db_path: PathBuf,
    /// 日志目录路径
    pub log_dir: PathBuf,
    /// 数据目录路径
    pub data_dir: PathBuf,
    /// Agent Core 运行时状态
    pub engine_status: AgentEngineStatus,
    /// Hermes 状态
    pub hermes_status: HermesStatus,
}

impl AppState {
    /// 初始化应用状态
    pub fn init() -> AppResult<Self> {
        let base = Self::get_app_base_dir()?;

        let config_dir = base.join("config");
        let data_dir = base.join("data");
        let log_dir = base.join("logs");
        let db_path = data_dir.join("hermes.db");

        // 确保目录存在
        std::fs::create_dir_all(&config_dir).map_err(AppError::from_io_error)?;
        std::fs::create_dir_all(&data_dir).map_err(AppError::from_io_error)?;
        std::fs::create_dir_all(&log_dir).map_err(AppError::from_io_error)?;

        Ok(Self {
            config_dir,
            db_path,
            log_dir,
            data_dir,
            engine_status: AgentEngineStatus::new(),
            hermes_status: HermesStatus::new(),
        })
    }

    /// 获取应用基础目录
    fn get_app_base_dir() -> AppResult<PathBuf> {
        directories::ProjectDirs::from("ai", "hermes", "operator")
            .map(|dirs| dirs.data_dir().to_path_buf())
            .ok_or_else(|| AppError::unknown("无法获取应用目录"))
    }

    /// 获取配置路径
    pub fn get_config_path(&self, name: &str) -> PathBuf {
        self.config_dir.join(name)
    }

    /// 获取日志路径
    pub fn get_log_path(&self, name: &str) -> PathBuf {
        self.log_dir.join(name)
    }
}

use parking_lot::RwLock;
use std::sync::Arc;

/// 全局应用状态管理器
pub type SharedAppState = Arc<RwLock<AppState>>;

/// 创建共享的应用状态
pub fn create_app_state() -> AppResult<SharedAppState> {
    let state = AppState::init()?;
    Ok(Arc::new(RwLock::new(state)))
}
