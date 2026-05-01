//! 环境检测模块
//!
//! 检测系统环境是否满足 Hermes Agent 运行要求

use serde::{Deserialize, Serialize};
use std::process::Command;

/// 环境检测项
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvCheckItem {
    /// 检测项名称
    pub name: String,
    /// 状态: pass, warning, fail
    pub status: String,
    /// 检测消息
    pub message: String,
    /// 是否可自动修复
    pub can_fix: bool,
}

/// 环境检测结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvCheckResult {
    /// 检测项列表
    pub items: Vec<EnvCheckItem>,
    /// 总体状态
    pub overall_status: String,
}

/// 检测操作系统
fn check_os() -> EnvCheckItem {
    #[cfg(target_os = "macos")]
    {
        EnvCheckItem {
            name: "操作系统".to_string(),
            status: "pass".to_string(),
            message: "macOS".to_string(),
            can_fix: false,
        }
    }

    #[cfg(target_os = "linux")]
    {
        EnvCheckItem {
            name: "操作系统".to_string(),
            status: "pass".to_string(),
            message: "Linux".to_string(),
            can_fix: false,
        }
    }

    #[cfg(target_os = "windows")]
    {
        EnvCheckItem {
            name: "操作系统".to_string(),
            status: "pass".to_string(),
            message: "Windows".to_string(),
            can_fix: false,
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        EnvCheckItem {
            name: "操作系统".to_string(),
            status: "warning".to_string(),
            message: "未知操作系统".to_string(),
            can_fix: false,
        }
    }
}

/// 检测 Python 是否安装
fn check_python() -> EnvCheckItem {
    let python_cmd = if cfg!(target_os = "windows") {
        "python"
    } else {
        "python3"
    };

    match Command::new(python_cmd).arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            EnvCheckItem {
                name: "Python".to_string(),
                status: "pass".to_string(),
                message: version,
                can_fix: false,
            }
        }
        Ok(_) => EnvCheckItem {
            name: "Python".to_string(),
            status: "fail".to_string(),
            message: "Python 命令执行失败".to_string(),
            can_fix: true,
        },
        Err(_) => EnvCheckItem {
            name: "Python".to_string(),
            status: "fail".to_string(),
            message: "Python 未安装".to_string(),
            can_fix: true,
        },
    }
}

/// 检测 uv 是否安装
fn check_uv() -> EnvCheckItem {
    match Command::new("uv").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            EnvCheckItem {
                name: "uv".to_string(),
                status: "pass".to_string(),
                message: version,
                can_fix: false,
            }
        }
        Ok(_) => EnvCheckItem {
            name: "uv".to_string(),
            status: "fail".to_string(),
            message: "uv 命令执行失败".to_string(),
            can_fix: true,
        },
        Err(_) => EnvCheckItem {
            name: "uv".to_string(),
            status: "fail".to_string(),
            message: "uv 未安装".to_string(),
            can_fix: true,
        },
    }
}

/// 检测 Hermes Agent 是否安装
fn check_hermes() -> EnvCheckItem {
    match Command::new("hermes").arg("--version").output() {
        Ok(output) if output.status.success() => {
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            EnvCheckItem {
                name: "Hermes Agent".to_string(),
                status: "pass".to_string(),
                message: format!("已安装: {}", version),
                can_fix: false,
            }
        }
        Ok(_) => EnvCheckItem {
            name: "Hermes Agent".to_string(),
            status: "fail".to_string(),
            message: "Hermes 命令执行失败".to_string(),
            can_fix: true,
        },
        Err(_) => EnvCheckItem {
            name: "Hermes Agent".to_string(),
            status: "fail".to_string(),
            message: "Hermes 未安装".to_string(),
            can_fix: true,
        },
    }
}

/// 执行完整的环境检测
pub fn check_environment() -> EnvCheckResult {
    let items = vec![check_os(), check_python(), check_uv(), check_hermes()];

    let overall_status = if items.iter().all(|i| i.status == "pass") {
        "pass"
    } else if items.iter().any(|i| i.status == "fail") {
        "fail"
    } else {
        "warning"
    }
    .to_string();

    EnvCheckResult {
        items,
        overall_status,
    }
}
