//! Hermes Agent 安装器模块
//!
//! 负责 Hermes Agent 的安装、升级、修复

use std::process::Command;

/// 安装结果
#[derive(Debug)]
pub struct InstallResult {
    /// 是否成功
    pub success: bool,
    /// 消息
    pub message: String,
}

/// 使用 uv 安装 Hermes Agent
pub fn install_hermes() -> InstallResult {
    let output = Command::new("uv")
        .args(["pip", "install", "hermes-agent", "--system"])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let output = String::from_utf8_lossy(&result.stdout);
            InstallResult {
                success: true,
                message: format!("Hermes Agent 安装成功!\n{}", output),
            }
        }
        Ok(result) => {
            let error = String::from_utf8_lossy(&result.stderr);
            InstallResult {
                success: false,
                message: format!("安装失败: {}", error),
            }
        }
        Err(e) => InstallResult {
            success: false,
            message: format!("安装失败: {}", e),
        },
    }
}

/// 使用 uv 卸载 Hermes Agent
pub fn uninstall_hermes() -> InstallResult {
    let output = Command::new("uv")
        .args(["pip", "uninstall", "hermes-agent", "-y"])
        .output();

    match output {
        Ok(result) if result.status.success() => InstallResult {
            success: true,
            message: "Hermes Agent 卸载成功".to_string(),
        },
        Ok(result) => {
            let error = String::from_utf8_lossy(&result.stderr);
            InstallResult {
                success: false,
                message: format!("卸载失败: {}", error),
            }
        }
        Err(e) => InstallResult {
            success: false,
            message: format!("卸载失败: {}", e),
        },
    }
}

/// 升级 Hermes Agent
pub fn upgrade_hermes() -> InstallResult {
    let output = Command::new("uv")
        .args(["pip", "install", "--upgrade", "hermes-agent", "--system"])
        .output();

    match output {
        Ok(result) if result.status.success() => {
            let output = String::from_utf8_lossy(&result.stdout);
            InstallResult {
                success: true,
                message: format!("Hermes Agent 升级成功!\n{}", output),
            }
        }
        Ok(result) => {
            let error = String::from_utf8_lossy(&result.stderr);
            InstallResult {
                success: false,
                message: format!("升级失败: {}", error),
            }
        }
        Err(e) => InstallResult {
            success: false,
            message: format!("升级失败: {}", e),
        },
    }
}
