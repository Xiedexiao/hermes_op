//! Hermes Desktop - Rust 后端业务逻辑模块
//!
//! 这个模块包含纯 Rust 业务逻辑，不依赖 Tauri 框架

pub mod agent_core;
pub mod app_state;
pub mod config;
pub mod control_api;
pub mod domain;
pub mod env;
pub mod errors;
pub mod hermes;
pub mod installer;
pub mod mcp_handshake;
pub mod mcp_probe;
pub mod mcp_runtime;
pub mod provider_auth;
pub mod secret_source;
pub mod services;
pub mod storage;
pub mod terminal_backends;

// 显式导出，避免名称冲突
pub use agent_core::{
    AgentEngineService, AgentEngineServiceImpl, EngineHeartbeat, clear_engine_heartbeat,
    current_engine_heartbeat, engine_heartbeat_path, maybe_run_engine_daemon_from_args,
    start_agent_process, stop_agent_process,
};
pub use app_state::{AgentEngineStatus, AppState, HermesStatus, SharedAppState, create_app_state};
pub use config::*;
pub use control_api::*;
pub use domain::*;
pub use env::*;
pub use errors::{AppError, AppResult};
pub use hermes::*;
pub use installer::*;
pub use mcp_handshake::*;
pub use mcp_probe::*;
pub use mcp_runtime::*;
pub use provider_auth::*;
pub use secret_source::*;
pub use services::*;
pub use storage::*;
pub use terminal_backends::*;
