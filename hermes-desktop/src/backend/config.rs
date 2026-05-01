//! Hermes 配置管理模块
//!
//! 负责 Hermes Agent 的配置读取和保存

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

fn default_busy_input_mode() -> String {
    "interrupt".to_string()
}

/// Hermes 配置结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HermesConfig {
    /// 模型提供商: openai, anthropic, deepseek, ollama, openrouter
    pub provider: String,
    /// API Key
    pub api_key: Option<String>,
    /// 默认模型
    pub model: String,
    /// Base URL (可选)
    pub base_url: Option<String>,
    /// 工作目录
    pub work_dir: String,
    /// 技能目录
    pub skills_dir: Option<String>,
    /// CLI 忙碌时回车行为: interrupt | queue
    #[serde(default = "default_busy_input_mode")]
    pub busy_input_mode: String,
}

impl Default for HermesConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        Self {
            provider: "openai".to_string(),
            api_key: None,
            model: "gpt-4o".to_string(),
            base_url: None,
            work_dir: home.to_string_lossy().to_string(),
            skills_dir: None,
            busy_input_mode: default_busy_input_mode(),
        }
    }
}

/// 获取配置目录路径
fn get_config_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".hermes")
}

/// 获取配置文件路径
fn get_config_path() -> PathBuf {
    get_config_dir().join("config.json")
}

/// 加载配置
pub fn load_config() -> Result<HermesConfig, String> {
    let config_path = get_config_path();

    if !config_path.exists() {
        return Ok(HermesConfig::default());
    }

    let content = fs::read_to_string(&config_path).map_err(|e| format!("读取配置失败: {}", e))?;

    serde_json::from_str(&content).map_err(|e| format!("解析配置失败: {}", e))
}

/// 保存配置
pub fn save_config(config: &HermesConfig) -> Result<(), String> {
    let config_dir = get_config_dir();
    let config_path = get_config_path();

    // 创建配置目录
    fs::create_dir_all(&config_dir).map_err(|e| format!("创建配置目录失败: {}", e))?;

    // 序列化为 JSON
    let json =
        serde_json::to_string_pretty(config).map_err(|e| format!("序列化配置失败: {}", e))?;

    // 写入文件
    fs::write(&config_path, json).map_err(|e| format!("保存配置失败: {}", e))?;

    Ok(())
}

/// 检查配置是否存在
pub fn config_exists() -> bool {
    get_config_path().exists()
}

/// 删除配置
pub fn delete_config() -> Result<(), String> {
    let config_path = get_config_path();

    if config_path.exists() {
        fs::remove_file(&config_path).map_err(|e| format!("删除配置失败: {}", e))?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct HomeOverride {
        key: &'static str,
        original: Option<String>,
    }

    impl HomeOverride {
        fn set(temp_home: &PathBuf) -> Self {
            let key = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
            let original = std::env::var(key).ok();
            unsafe {
                std::env::set_var(key, temp_home);
            }
            Self { key, original }
        }
    }

    impl Drop for HomeOverride {
        fn drop(&mut self) {
            match self.original.as_deref() {
                Some(value) => unsafe {
                    std::env::set_var(self.key, value);
                },
                None => unsafe {
                    std::env::remove_var(self.key);
                },
            }
        }
    }

    fn unique_temp_home() -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("hermes-config-test-{timestamp}"))
    }

    #[test]
    fn default_uses_interrupt_busy_input_mode() {
        assert_eq!(HermesConfig::default().busy_input_mode, "interrupt");
    }

    #[test]
    fn load_config_backfills_missing_busy_input_mode() {
        let _guard = env_lock().lock().expect("lock poisoned");
        let temp_home = unique_temp_home();
        let _home = HomeOverride::set(&temp_home);
        let config_path = get_config_path();

        fs::create_dir_all(config_path.parent().expect("config dir")).expect("create config dir");
        fs::write(
            &config_path,
            r#"{
  "provider": "openai",
  "api_key": null,
  "model": "gpt-4o",
  "base_url": null,
  "work_dir": "/tmp/work",
  "skills_dir": null
}"#,
        )
        .expect("write legacy config");

        let loaded = load_config().expect("load config");
        assert_eq!(loaded.busy_input_mode, "interrupt");

        fs::remove_dir_all(temp_home).expect("cleanup temp home");
    }

    #[test]
    fn save_and_load_round_trip_preserves_busy_input_mode() {
        let _guard = env_lock().lock().expect("lock poisoned");
        let temp_home = unique_temp_home();
        let _home = HomeOverride::set(&temp_home);

        let config = HermesConfig {
            provider: "openrouter".to_string(),
            api_key: Some("secret".to_string()),
            model: "gpt-5".to_string(),
            base_url: Some("https://example.com".to_string()),
            work_dir: "/tmp/hermes".to_string(),
            skills_dir: Some("/tmp/skills".to_string()),
            busy_input_mode: "queue".to_string(),
        };

        save_config(&config).expect("save config");
        let loaded = load_config().expect("load config");

        assert_eq!(loaded.provider, config.provider);
        assert_eq!(loaded.api_key, config.api_key);
        assert_eq!(loaded.model, config.model);
        assert_eq!(loaded.base_url, config.base_url);
        assert_eq!(loaded.work_dir, config.work_dir);
        assert_eq!(loaded.skills_dir, config.skills_dir);
        assert_eq!(loaded.busy_input_mode, config.busy_input_mode);

        fs::remove_dir_all(temp_home).expect("cleanup temp home");
    }
}
