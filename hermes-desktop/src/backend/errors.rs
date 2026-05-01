//! 统一错误类型定义
//!
//! 定义所有应用层错误，统一格式便于前端处理

use serde::{Deserialize, Serialize};

/// 应用层统一错误
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppError {
    /// 错误码
    pub code: String,
    /// 错误消息
    pub message: String,
    /// 是否可重试
    pub retryable: bool,
    /// 详细数据
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl AppError {
    /// 创建验证错误
    pub fn validation(message: impl Into<String>) -> Self {
        Self {
            code: "validation_error".to_string(),
            message: message.into(),
            retryable: false,
            details: None,
        }
    }

    /// 创建存储错误
    pub fn storage(message: impl Into<String>) -> Self {
        Self {
            code: "storage_error".to_string(),
            message: message.into(),
            retryable: false,
            details: None,
        }
    }

    /// 创建运行时错误
    pub fn runtime(message: impl Into<String>) -> Self {
        Self {
            code: "runtime_error".to_string(),
            message: message.into(),
            retryable: false,
            details: None,
        }
    }

    /// 创建 Hermes 运行时错误
    pub fn hermes_runtime(message: impl Into<String>) -> Self {
        Self {
            code: "hermes_runtime_error".to_string(),
            message: message.into(),
            retryable: true,
            details: None,
        }
    }

    /// 创建 IO 错误
    pub fn io(message: impl Into<String>) -> Self {
        Self {
            code: "io_error".to_string(),
            message: message.into(),
            retryable: false,
            details: None,
        }
    }

    /// 创建未知错误
    pub fn unknown(message: impl Into<String>) -> Self {
        Self {
            code: "unknown_error".to_string(),
            message: message.into(),
            retryable: false,
            details: None,
        }
    }

    /// 从 std::io::Error 转换
    pub fn from_io_error(e: std::io::Error) -> Self {
        Self::io(format!("IO error: {}", e))
    }

    /// 从 serde_json::Error 转换
    pub fn from_json_error(e: serde_json::Error) -> Self {
        Self::io(format!("JSON error: {}", e))
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

/// 应用层 Result 类型别名
pub type AppResult<T> = Result<T, AppError>;
