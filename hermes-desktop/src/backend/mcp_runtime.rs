//! MCP runtime process control helpers.

use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;

use crate::backend::{AppError, AppResult};

const PROCESS_MANAGEMENT_MODE: &str = "process";
const EXTERNAL_MANAGEMENT_MODE: &str = "external";

pub fn management_mode_for_transport(transport: &str) -> &'static str {
    match transport.trim().to_ascii_lowercase().as_str() {
        "stdio" => PROCESS_MANAGEMENT_MODE,
        _ => EXTERNAL_MANAGEMENT_MODE,
    }
}

pub fn default_runtime_status_for_transport(transport: &str) -> &'static str {
    if management_mode_for_transport(transport) == PROCESS_MANAGEMENT_MODE {
        "stopped"
    } else {
        "external"
    }
}

pub fn external_status_message(transport: &str) -> Option<String> {
    if management_mode_for_transport(transport) == EXTERNAL_MANAGEMENT_MODE {
        Some(format!(
            "Managed externally for {} transport",
            transport.trim()
        ))
    } else {
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityMcpProcessObservation {
    pub running: bool,
    pub pid: Option<u32>,
    pub exit_code: Option<i32>,
}

struct ManagedProcess {
    child: Child,
}

#[derive(Default)]
pub struct ParityMcpRuntimeManager {
    processes: Mutex<HashMap<String, ManagedProcess>>,
}

impl ParityMcpRuntimeManager {
    pub fn start_process(&self, server_id: &str, endpoint: &str) -> AppResult<u32> {
        let normalized_id = server_id.trim();
        if normalized_id.is_empty() {
            return Err(AppError::validation("mcp id is required"));
        }

        let existing = self.inspect_process(normalized_id, None)?;
        if existing.running {
            return existing
                .pid
                .ok_or_else(|| AppError::runtime("running process missing pid"));
        }

        let mut command = shell_command(endpoint);
        command.stdin(Stdio::null());
        command.stdout(Stdio::null());
        command.stderr(Stdio::null());

        let child = command
            .spawn()
            .map_err(|err| AppError::runtime(format!("Failed to spawn MCP server: {}", err)))?;
        let pid = child.id();

        self.processes
            .lock()
            .insert(normalized_id.to_string(), ManagedProcess { child });

        Ok(pid)
    }

    pub fn stop_process(&self, server_id: &str, pid: Option<u32>) -> AppResult<Option<i32>> {
        let normalized_id = server_id.trim();
        if normalized_id.is_empty() {
            return Err(AppError::validation("mcp id is required"));
        }

        if let Some(mut managed) = self.processes.lock().remove(normalized_id) {
            kill_child(&mut managed.child)?;
            let status = managed.child.wait().map_err(|err| {
                AppError::runtime(format!("Failed to wait for MCP server: {}", err))
            })?;
            return Ok(status.code());
        }

        if let Some(existing_pid) = pid
            && is_process_alive(existing_pid)?
        {
            terminate_pid(existing_pid)?;
            wait_for_process_exit(existing_pid)?;
        }

        Ok(None)
    }

    pub fn inspect_process(
        &self,
        server_id: &str,
        pid: Option<u32>,
    ) -> AppResult<ParityMcpProcessObservation> {
        let normalized_id = server_id.trim();
        if normalized_id.is_empty() {
            return Err(AppError::validation("mcp id is required"));
        }

        let mut processes = self.processes.lock();
        let mut exited = None;

        if let Some(managed) = processes.get_mut(normalized_id) {
            match managed.child.try_wait() {
                Ok(Some(status)) => {
                    exited = Some(status.code());
                }
                Ok(None) => {
                    return Ok(ParityMcpProcessObservation {
                        running: true,
                        pid: Some(managed.child.id()),
                        exit_code: None,
                    });
                }
                Err(err) => {
                    return Err(AppError::runtime(format!(
                        "Failed to inspect MCP server: {}",
                        err
                    )));
                }
            }
        }

        if exited.is_some() {
            processes.remove(normalized_id);
            return Ok(ParityMcpProcessObservation {
                running: false,
                pid: None,
                exit_code: exited.flatten(),
            });
        }

        drop(processes);

        if let Some(existing_pid) = pid
            && is_process_alive(existing_pid)?
        {
            return Ok(ParityMcpProcessObservation {
                running: true,
                pid: Some(existing_pid),
                exit_code: None,
            });
        }

        Ok(ParityMcpProcessObservation {
            running: false,
            pid: None,
            exit_code: None,
        })
    }
}

fn shell_command(endpoint: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command.arg("/C").arg(endpoint);
        command
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut command = Command::new("sh");
        command.arg("-lc").arg(endpoint);
        command
    }
}

fn kill_child(child: &mut Child) -> AppResult<()> {
    match child.kill() {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::InvalidInput => Ok(()),
        Err(err) => Err(AppError::runtime(format!(
            "Failed to stop MCP server: {}",
            err
        ))),
    }
}

fn wait_for_process_exit(pid: u32) -> AppResult<()> {
    for _ in 0..20 {
        if !is_process_alive(pid)? {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }

    #[cfg(target_os = "windows")]
    {
        Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output()
            .map_err(|err| {
                AppError::runtime(format!("Failed to force stop MCP server: {}", err))
            })?;
    }

    #[cfg(not(target_os = "windows"))]
    {
        Command::new("kill")
            .args(["-KILL", &pid.to_string()])
            .output()
            .map_err(|err| {
                AppError::runtime(format!("Failed to force stop MCP server: {}", err))
            })?;
    }

    Ok(())
}

#[cfg(target_os = "windows")]
fn terminate_pid(pid: u32) -> AppResult<()> {
    Command::new("taskkill")
        .args(["/PID", &pid.to_string()])
        .output()
        .map_err(|err| AppError::runtime(format!("Failed to stop MCP server: {}", err)))?;
    Ok(())
}

#[cfg(not(target_os = "windows"))]
fn terminate_pid(pid: u32) -> AppResult<()> {
    Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output()
        .map_err(|err| AppError::runtime(format!("Failed to stop MCP server: {}", err)))?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn is_process_alive(pid: u32) -> AppResult<bool> {
    let output = Command::new("tasklist")
        .args(["/FI", &format!("PID eq {}", pid)])
        .output()
        .map_err(|err| AppError::runtime(format!("Failed to inspect MCP server: {}", err)))?;
    Ok(String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()))
}

#[cfg(not(target_os = "windows"))]
fn is_process_alive(pid: u32) -> AppResult<bool> {
    let status = Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map_err(|err| AppError::runtime(format!("Failed to inspect MCP server: {}", err)))?;
    Ok(status.success())
}
