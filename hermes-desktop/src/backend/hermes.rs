//! Hermes Agent 运行时管理模块
//!
//! 负责 Hermes Agent 的启动、停止、状态监控

use serde::{Deserialize, Serialize};
use std::process::Command;

/// Hermes 状态
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesStatus {
    /// 是否已安装
    pub installed: bool,
    /// 版本号
    pub version: Option<String>,
    /// 是否运行中
    pub running: bool,
    /// 进程 ID
    pub pid: Option<u32>,
}

/// 获取 Hermes 安装状态和版本
pub fn get_install_status() -> (bool, Option<String>) {
    let output = Command::new("hermes").arg("--version").output();

    match output {
        Ok(result) if result.status.success() => {
            let version = String::from_utf8_lossy(&result.stdout).trim().to_string();
            (true, Some(version))
        }
        _ => (false, None),
    }
}

/// 检查 Hermes 是否运行中
#[cfg(target_os = "windows")]
pub fn is_running() -> bool {
    Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq *hermes*"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("hermes"))
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
pub fn is_running() -> bool {
    Command::new("pgrep")
        .args(["-f", "hermes"])
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false)
}

/// 获取 Hermes 进程 PID
#[cfg(target_os = "windows")]
pub fn get_pid() -> Option<u32> {
    None // Windows 实现略复杂，这里简化
}

#[cfg(not(target_os = "windows"))]
pub fn get_pid() -> Option<u32> {
    let output = Command::new("pgrep").args(["-f", "hermes"]).output().ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split_whitespace()
        .next()
        .and_then(|s| s.parse().ok())
}

/// 获取完整的 Hermes 状态
pub fn get_status() -> HermesStatus {
    let (installed, version) = get_install_status();
    let running = is_running();
    let pid = if running { get_pid() } else { None };

    HermesStatus {
        installed,
        version,
        running,
        pid,
    }
}

/// 启动 Hermes Agent
pub fn start() -> Result<String, String> {
    if !is_running() {
        Command::new("hermes")
            .spawn()
            .map_err(|e| format!("启动失败: {}", e))?;
        Ok("Hermes 启动中...".to_string())
    } else {
        Ok("Hermes 已在运行中".to_string())
    }
}

/// 停止 Hermes Agent
pub fn stop() -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "hermes.exe"])
            .output();
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("pkill").args(["-f", "hermes"]).output();
    }

    Ok("Hermes 已停止".to_string())
}

/// 重启 Hermes Agent
pub fn restart() -> Result<String, String> {
    let _ = stop();
    std::thread::sleep(std::time::Duration::from_millis(500));
    start()
}
