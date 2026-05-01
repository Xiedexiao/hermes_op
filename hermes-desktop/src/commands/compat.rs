//! 向后兼容命令
//!
//! 保留原有的命令以兼容旧代码

use crate::backend::{
    config::{self, HermesConfig},
    env,
    hermes::{self, HermesStatus},
    installer,
};

/// 检查环境
#[tauri::command]
pub fn check_environment() -> env::EnvCheckResult {
    env::check_environment()
}

/// 获取 Hermes 状态
#[tauri::command]
pub fn get_hermes_status() -> HermesStatus {
    hermes::get_status()
}

/// 安装 Hermes
#[tauri::command]
pub fn install_hermes() -> Result<String, String> {
    let result = installer::install_hermes();
    if result.success {
        Ok(result.message)
    } else {
        Err(result.message)
    }
}

/// 卸载 Hermes
#[tauri::command]
pub fn uninstall_hermes() -> Result<String, String> {
    let result = installer::uninstall_hermes();
    if result.success {
        Ok(result.message)
    } else {
        Err(result.message)
    }
}

/// 升级 Hermes
#[tauri::command]
pub fn upgrade_hermes() -> Result<String, String> {
    let result = installer::upgrade_hermes();
    if result.success {
        Ok(result.message)
    } else {
        Err(result.message)
    }
}

/// 启动 Hermes
#[tauri::command]
pub fn start_hermes() -> Result<String, String> {
    hermes::start()
}

/// 停止 Hermes
#[tauri::command]
pub fn stop_hermes() -> Result<String, String> {
    hermes::stop()
}

/// 重启 Hermes
#[tauri::command]
pub fn restart_hermes() -> Result<String, String> {
    hermes::restart()
}

/// 加载配置
#[tauri::command]
pub fn load_config() -> Result<HermesConfig, String> {
    config::load_config()
}

/// 保存配置
#[tauri::command]
pub fn save_config(config: HermesConfig) -> Result<(), String> {
    config::save_config(&config)
}
