//! 进程管理器
//!
//! 管理 Agent Core 进程的启动、停止和状态检查

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::{self, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::backend::AppState;
use crate::backend::errors::{AppError, AppResult};

use super::daemon::{EngineHeartbeat, clear_engine_heartbeat, read_engine_heartbeat};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EngineRuntimeState {
    pid: u32,
    profile: String,
    started_at: String,
    command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EngineLockState {
    pid: u32,
    locked_at: String,
}

/// 检查进程是否正在运行
pub fn is_process_running(pid: u32) -> bool {
    // 在 Unix 系统上检查进程是否存在
    #[cfg(unix)]
    {
        process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .output()
            .map(|output| {
                if !output.status.success() {
                    return false;
                }

                process::Command::new("ps")
                    .args(["-o", "stat=", "-p", &pid.to_string()])
                    .output()
                    .map(|ps_output| {
                        let stat = String::from_utf8_lossy(&ps_output.stdout);
                        let trimmed = stat.trim();
                        !trimmed.is_empty() && !trimmed.starts_with('Z')
                    })
                    .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    // 在 Windows 上使用 tasklist
    #[cfg(windows)]
    {
        process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output()
            .map(|output| {
                let output_str = String::from_utf8_lossy(&output.stdout);
                output_str.contains(&pid.to_string())
            })
            .unwrap_or(false)
    }
}

/// 查找 Agent Core 进程
///
/// 检查 runtime 目录中的 engine.state 文件来获取进程信息
pub fn find_agent_process(state: &AppState) -> Option<u32> {
    if let Some(metadata) = load_engine_runtime_state(state) {
        if is_process_running(metadata.pid) {
            Some(metadata.pid)
        } else {
            let _ = clear_runtime_files(state);
            None
        }
    } else {
        let _ = clear_runtime_files(state);
        None
    }
}

pub(crate) fn current_engine_runtime_state(state: &AppState) -> Option<(u32, String)> {
    load_engine_runtime_state(state).and_then(|metadata| {
        if is_process_running(metadata.pid) {
            Some((metadata.pid, metadata.profile))
        } else {
            let _ = clear_runtime_files(state);
            None
        }
    })
}

pub fn current_engine_heartbeat(state: &AppState) -> Option<EngineHeartbeat> {
    read_engine_heartbeat(&state.data_dir)
}

pub fn start_agent_process(state: &mut AppState, profile: &str) -> AppResult<u32> {
    if let Some((pid, persisted_profile)) = current_engine_runtime_state(state) {
        state.engine_status.set_running(pid, persisted_profile);
        return Ok(pid);
    }

    let (mut command, command_label) = build_engine_command(state, profile)?;
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::runtime(format!("Failed to spawn agent engine: {}", e)))?;
    let pid = child.id();

    state.engine_status.set_running(pid, profile);
    persist_runtime_state(
        state,
        &EngineRuntimeState {
            pid,
            profile: profile.to_string(),
            started_at: Utc::now().to_rfc3339(),
            command: command_label,
        },
    )?;

    tracing::info!("Agent Core started with PID: {}", pid);
    Ok(pid)
}

pub fn stop_agent_process(state: &mut AppState) -> AppResult<()> {
    if let Some((pid, _)) = current_engine_runtime_state(state) {
        terminate_process(pid)?;
    }

    clear_runtime_files(state)?;
    state.engine_status.set_stopped();

    tracing::info!("Agent Core stopped");
    Ok(())
}

/// 获取 Agent Core 状态文件路径
pub fn get_engine_state_path(state: &AppState) -> PathBuf {
    state.data_dir.join("engine.state")
}

/// 获取 Agent Core 锁文件路径
pub fn get_engine_lock_path(state: &AppState) -> PathBuf {
    state.data_dir.join("engine.lock")
}

fn persist_runtime_state(state: &AppState, runtime: &EngineRuntimeState) -> AppResult<()> {
    let engine_state_path = get_engine_state_path(state);
    let engine_lock_path = get_engine_lock_path(state);
    let lock_state = EngineLockState {
        pid: runtime.pid,
        locked_at: Utc::now().to_rfc3339(),
    };

    std::fs::write(
        &engine_state_path,
        serde_json::to_string(runtime).map_err(AppError::from_json_error)?,
    )
    .map_err(|e| AppError::io(format!("Failed to write engine state: {}", e)))?;

    std::fs::write(
        &engine_lock_path,
        serde_json::to_string(&lock_state).map_err(AppError::from_json_error)?,
    )
    .map_err(|e| AppError::io(format!("Failed to create engine lock: {}", e)))?;

    Ok(())
}

fn load_engine_runtime_state(state: &AppState) -> Option<EngineRuntimeState> {
    let engine_state_path = get_engine_state_path(state);
    if !engine_state_path.exists() {
        return None;
    }

    let raw = std::fs::read_to_string(&engine_state_path).ok()?;
    if let Ok(metadata) = serde_json::from_str::<EngineRuntimeState>(&raw) {
        return Some(metadata);
    }

    raw.trim()
        .parse::<u32>()
        .ok()
        .map(|pid| EngineRuntimeState {
            pid,
            profile: "default".to_string(),
            started_at: String::new(),
            command: "legacy".to_string(),
        })
}

fn clear_runtime_files(state: &AppState) -> AppResult<()> {
    let engine_state_path = get_engine_state_path(state);
    let engine_lock_path = get_engine_lock_path(state);

    if engine_state_path.exists() {
        std::fs::remove_file(&engine_state_path)
            .map_err(|e| AppError::io(format!("Failed to remove engine state: {}", e)))?;
    }

    if engine_lock_path.exists() {
        std::fs::remove_file(&engine_lock_path)
            .map_err(|e| AppError::io(format!("Failed to remove engine lock: {}", e)))?;
    }

    clear_engine_heartbeat(&state.data_dir)?;

    Ok(())
}

fn build_engine_command(state: &AppState, profile: &str) -> AppResult<(Command, String)> {
    let executable = std::env::current_exe()
        .map_err(|err| AppError::runtime(format!("Failed to resolve current executable: {err}")))?;
    let data_dir = state.data_dir.to_string_lossy().to_string();
    let command_label = format!(
        "{} --engine-daemon --profile {} --data-dir {}",
        executable.display(),
        profile,
        data_dir
    );

    #[cfg(test)]
    {
        #[cfg(unix)]
        {
            let mut command = Command::new("sh");
            command.args(["-c", "while true; do sleep 1; done"]);
            Ok((command, command_label))
        }

        #[cfg(windows)]
        {
            let mut command = Command::new("cmd");
            command.args(["/C", "ping", "-t", "127.0.0.1", ">", "NUL"]);
            return Ok((command, command_label));
        }
    }

    #[cfg(not(test))]
    {
        let mut command = Command::new(executable);
        command.args([
            "--engine-daemon",
            "--profile",
            profile,
            "--data-dir",
            data_dir.as_str(),
        ]);
        Ok((command, command_label))
    }
}

fn terminate_process(pid: u32) -> AppResult<()> {
    #[cfg(unix)]
    {
        let _ = Command::new("kill")
            .args(["-TERM", &pid.to_string()])
            .status();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if !is_process_running(pid) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
        let _ = Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .status();
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            if !is_process_running(pid) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/T", "/F"])
            .status();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if !is_process_running(pid) {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(50));
        }
    }

    Err(AppError::runtime(format!(
        "Failed to terminate engine process: {}",
        pid
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{AgentEngineStatus, AppState, HermesStatus};
    use std::time::{Duration, Instant};
    use uuid::Uuid;

    fn make_test_state() -> (AppState, PathBuf) {
        let base_dir =
            std::env::temp_dir().join(format!("hermes-desktop-process-manager-{}", Uuid::new_v4()));
        let config_dir = base_dir.join("config");
        let data_dir = base_dir.join("data");
        let log_dir = base_dir.join("logs");

        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::create_dir_all(&data_dir).unwrap();
        std::fs::create_dir_all(&log_dir).unwrap();

        (
            AppState {
                config_dir,
                db_path: data_dir.join("hermes.db"),
                log_dir,
                data_dir,
                engine_status: AgentEngineStatus::new(),
                hermes_status: HermesStatus::new(),
            },
            base_dir,
        )
    }

    fn wait_until_stopped(pid: u32) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if !is_process_running(pid) {
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("process {} did not stop in time", pid);
    }

    #[test]
    fn test_get_engine_paths() {
        let (state, base_dir) = make_test_state();
        let state_path = get_engine_state_path(&state);
        let lock_path = get_engine_lock_path(&state);

        assert!(state_path.ends_with("engine.state"));
        assert!(lock_path.ends_with("engine.lock"));

        std::fs::remove_dir_all(base_dir).unwrap();
    }

    #[test]
    fn test_start_agent_process_spawns_real_child_and_persists_runtime_metadata() {
        let (mut state, base_dir) = make_test_state();

        let pid = start_agent_process(&mut state, "test-profile").unwrap();
        assert_ne!(
            pid,
            process::id(),
            "engine PID must belong to a child process"
        );
        assert!(
            is_process_running(pid),
            "spawned child should still be running"
        );

        let metadata: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(get_engine_state_path(&state)).unwrap())
                .unwrap();
        assert_eq!(metadata["pid"].as_u64(), Some(pid as u64));
        assert_eq!(metadata["profile"].as_str(), Some("test-profile"));
        assert!(metadata["started_at"].as_str().is_some());
        let command = metadata["command"]
            .as_str()
            .expect("command should persist as string");
        assert!(
            command.contains("--engine-daemon"),
            "engine process should launch the Hermes daemon entrypoint, got: {command}"
        );
        assert!(
            command.contains("--profile test-profile"),
            "engine process should preserve the requested profile, got: {command}"
        );

        let lock_metadata: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(get_engine_lock_path(&state)).unwrap())
                .unwrap();
        assert_eq!(lock_metadata["pid"].as_u64(), Some(pid as u64));

        stop_agent_process(&mut state).unwrap();
        wait_until_stopped(pid);
        std::fs::remove_dir_all(base_dir).unwrap();
    }

    #[test]
    fn test_find_agent_process_cleans_stale_runtime_files() {
        let (state, base_dir) = make_test_state();
        let engine_state_path = get_engine_state_path(&state);
        let engine_lock_path = get_engine_lock_path(&state);

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
        std::fs::write(
            &engine_lock_path,
            serde_json::json!({
                "pid": 999_991_u32
            })
            .to_string(),
        )
        .unwrap();

        assert_eq!(find_agent_process(&state), None);
        assert!(
            !engine_state_path.exists(),
            "stale engine.state should be removed"
        );
        assert!(
            !engine_lock_path.exists(),
            "stale engine.lock should be removed"
        );

        std::fs::remove_dir_all(base_dir).unwrap();
    }

    #[test]
    fn test_stop_agent_process_terminates_child_and_clears_runtime_files() {
        let (mut state, base_dir) = make_test_state();
        let pid = start_agent_process(&mut state, "stop-test").unwrap();

        stop_agent_process(&mut state).unwrap();

        wait_until_stopped(pid);
        assert!(!get_engine_state_path(&state).exists());
        assert!(!get_engine_lock_path(&state).exists());
        assert!(!state.engine_status.running);
        assert!(state.engine_status.pid.is_none());

        std::fs::remove_dir_all(base_dir).unwrap();
    }
}
