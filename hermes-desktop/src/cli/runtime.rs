use super::CliError;
use hermes_desktop::backend::{
    AgentEngineService, AgentEngineServiceImpl, Database, create_app_state,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeStatusSnapshot {
    pub engine_running: bool,
    pub engine_profile: Option<String>,
    pub engine_pid: Option<u32>,
    pub hermes_installed: bool,
    pub hermes_running: bool,
    pub hermes_version: Option<String>,
    pub hermes_pid: Option<u32>,
}

pub fn load_status() -> Result<RuntimeStatusSnapshot, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    let _db = Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))?;
    let engine = AgentEngineServiceImpl::new(state.clone())
        .status()
        .map_err(|err| CliError::Runtime(err.to_string()))?;
    let hermes = {
        let guard = state.read();
        guard.hermes_status.clone()
    };

    Ok(RuntimeStatusSnapshot {
        engine_running: engine.running,
        engine_profile: engine.profile,
        engine_pid: engine.pid,
        hermes_installed: hermes.installed,
        hermes_running: hermes.running,
        hermes_version: hermes.version,
        hermes_pid: None,
    })
}

pub fn render_status(snapshot: &RuntimeStatusSnapshot) -> String {
    format!(
        "runtime\tengine={}\tprofile={}\tpid={}\nhermes\tinstalled={}\trunning={}\tversion={}\tpid={}\n",
        if snapshot.engine_running {
            "running"
        } else {
            "stopped"
        },
        snapshot.engine_profile.as_deref().unwrap_or("-"),
        snapshot
            .engine_pid
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
        snapshot.hermes_installed,
        snapshot.hermes_running,
        snapshot.hermes_version.as_deref().unwrap_or("-"),
        snapshot
            .hermes_pid
            .map(|value| value.to_string())
            .unwrap_or_else(|| "-".to_string()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_minimal_runtime_snapshot() {
        let rendered = render_status(&RuntimeStatusSnapshot {
            engine_running: false,
            engine_profile: None,
            engine_pid: None,
            hermes_installed: false,
            hermes_running: false,
            hermes_version: None,
            hermes_pid: None,
        });

        assert_eq!(
            rendered,
            "runtime\tengine=stopped\tprofile=-\tpid=-\nhermes\tinstalled=false\trunning=false\tversion=-\tpid=-\n"
        );
    }
}
