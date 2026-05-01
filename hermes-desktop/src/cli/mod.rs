pub mod foreground;
pub mod foreground_runtime;
pub mod foreground_store;
pub mod missions;
pub mod repl;
pub mod runtime;
pub mod sessions;
pub mod slash;

use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};

#[cfg(test)]
pub(crate) static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Repl,
    Slash(String),
    RuntimeStatus,
    MissionsList,
    SessionsList,
    SessionsLatest,
    SessionsContinue,
    SessionsResume(String),
    SessionsTitle(Option<String>),
    SessionsSearch(String),
    SessionsGet(String),
    SessionsRename(String, String),
}

#[derive(Debug)]
pub enum CliError {
    Io(io::Error),
    InvalidUsage(String),
    Runtime(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::InvalidUsage(message) => write!(f, "{message}"),
            Self::Runtime(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<io::Error> for CliError {
    fn from(value: io::Error) -> Self {
        Self::Io(value)
    }
}

pub fn run<I, T, W>(args: I, writer: &mut W) -> Result<(), CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
    W: Write,
{
    run_with(
        args,
        writer,
        runtime::load_status,
        missions::load_missions,
        sessions::load_sessions,
        sessions::get_session,
        sessions::get_latest_session,
        sessions::rename_session,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_with<I, T, W, RF, MF, SF, SG, SL, SR>(
    args: I,
    writer: &mut W,
    runtime_loader: RF,
    missions_loader: MF,
    sessions_loader: SF,
    session_getter: SG,
    latest_session_getter: SL,
    session_renamer: SR,
) -> Result<(), CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
    W: Write,
    RF: FnOnce() -> Result<runtime::RuntimeStatusSnapshot, CliError>,
    MF: FnOnce() -> Result<Vec<missions::MissionListItem>, CliError>,
    SF: FnOnce() -> Result<Vec<sessions::SessionListItem>, CliError>,
    SG: FnOnce(&str) -> Result<Option<String>, CliError>,
    SL: FnOnce() -> Result<Option<String>, CliError>,
    SR: FnOnce(&str, &str) -> Result<String, CliError>,
{
    match parse_command(args)? {
        Command::Repl => {
            let reader = io::BufReader::new(io::stdin());
            repl::run(reader, writer)?;
        }
        Command::Slash(command) => {
            let output = slash::handle_from_cli(&command)?;
            writer.write_all(output.as_bytes())?;
        }
        Command::RuntimeStatus => {
            let output = runtime::render_status(&runtime_loader()?);
            writer.write_all(output.as_bytes())?;
        }
        Command::MissionsList => {
            let output = missions::render_list(&missions_loader()?);
            writer.write_all(output.as_bytes())?;
        }
        Command::SessionsList => {
            let output = sessions::render_list(&sessions_loader()?);
            writer.write_all(output.as_bytes())?;
        }
        Command::SessionsResume(selector) => {
            let exact = session_getter(&selector)?;
            let output = if let Some(row) = exact {
                row
            } else {
                let sessions = sessions_loader()?;
                sessions::find_resume_candidate(&sessions, &selector)
                    .map(sessions::render_match)
                    .unwrap_or_else(|| "session not found\n".to_string())
            };
            writer.write_all(output.as_bytes())?;
        }
        Command::SessionsTitle(next_title) => {
            let latest = latest_session_getter()?
                .map(parse_session_row)
                .transpose()?
                .ok_or_else(|| CliError::InvalidUsage("no sessions found\n".to_string()))?;
            let output = match next_title {
                Some(title) => {
                    let renamed = session_renamer(&latest.id, &title)?;
                    sessions::render_title(&parse_session_row(renamed)?)
                }
                None => sessions::render_title(&latest),
            };
            writer.write_all(output.as_bytes())?;
        }
        Command::SessionsSearch(query) => {
            let sessions = sessions_loader()?;
            let matches = sessions::search_recent(&sessions, &query);
            let output = if matches.is_empty() {
                "no sessions found\n".to_string()
            } else {
                matches
                    .into_iter()
                    .map(sessions::render_match)
                    .collect::<String>()
            };
            writer.write_all(output.as_bytes())?;
        }
        Command::SessionsGet(id) => {
            let output = session_getter(&id)?.unwrap_or_else(|| "session not found\n".to_string());
            writer.write_all(output.as_bytes())?;
        }
        Command::SessionsLatest | Command::SessionsContinue => {
            let output =
                latest_session_getter()?.unwrap_or_else(|| "no sessions found\n".to_string());
            writer.write_all(output.as_bytes())?;
        }
        Command::SessionsRename(id, title) => {
            let output = session_renamer(&id, &title)?;
            writer.write_all(output.as_bytes())?;
        }
    }

    Ok(())
}

fn parse_session_row(row: String) -> Result<sessions::SessionListItem, CliError> {
    let mut parts = row.trim_end().split('\t');
    let id = parts
        .next()
        .ok_or_else(|| CliError::Runtime("invalid session row".to_string()))?;
    let source = parts
        .next()
        .ok_or_else(|| CliError::Runtime("invalid session row".to_string()))?;
    let title = parts
        .next()
        .ok_or_else(|| CliError::Runtime("invalid session row".to_string()))?;
    let model_name = parts.next().map(|value| value.to_string());

    Ok(sessions::SessionListItem {
        id: id.to_string(),
        source: source.to_string(),
        title: title.to_string(),
        model_name,
    })
}

fn parse_command<I, T>(args: I) -> Result<Command, CliError>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args: Vec<String> = args
        .into_iter()
        .map(|value| value.into().to_string_lossy().into_owned())
        .collect();

    match args.as_slice() {
        [] | [_] => Ok(Command::Repl),
        [_, command, rest @ ..] if command.starts_with('/') => {
            let mut slash_command = command.clone();
            for token in rest {
                slash_command.push(' ');
                slash_command.push_str(token);
            }

            Ok(Command::Slash(slash_command))
        }
        [_, group, action] if group == "runtime" && action == "status" => {
            Ok(Command::RuntimeStatus)
        }
        [_, group] if group == "busy" => Ok(Command::Slash("/busy".to_string())),
        [_, group, mode] if group == "busy" => Ok(Command::Slash(format!("/busy {}", mode))),
        [_, action] if matches!(action.as_str(), "interrupt" | "cancel" | "stop") => {
            Ok(Command::Slash(format!("/{action}")))
        }
        [_, action, request @ ..] if matches!(action.as_str(), "interrupt" | "cancel" | "stop") => {
            Ok(Command::Slash(format!("/{action} {}", request.join(" "))))
        }
        [_, group, action] if group == "missions" && action == "list" => Ok(Command::MissionsList),
        [_, group, action] if group == "missions" && matches!(action.as_str(), "get" | "plan") => {
            Err(CliError::InvalidUsage(usage().to_string()))
        }
        [_, group, action, mission_id]
            if group == "missions" && matches!(action.as_str(), "get" | "plan") =>
        {
            Ok(Command::Slash(format!("/missions {action} {mission_id}")))
        }
        [_, group, action] if group == "missions" && action == "status" => {
            Err(CliError::InvalidUsage(usage().to_string()))
        }
        [_, group, action, mission_id, status] if group == "missions" && action == "status" => Ok(
            Command::Slash(format!("/missions status {mission_id} {status}")),
        ),
        [_, group, action] if group == "foreground" && action == "status" => {
            Ok(Command::Slash("/foreground status".to_string()))
        }
        [_, group] if group == "background" => Ok(Command::Slash("/background".to_string())),
        [_, group, action]
            if group == "background" && matches!(action.as_str(), "status" | "latest") =>
        {
            Ok(Command::Slash(format!("/background {action}")))
        }
        [_, group, prompt @ ..] if group == "background" => {
            Ok(Command::Slash(format!("/background {}", prompt.join(" "))))
        }
        [_, group] if group == "queue" => Ok(Command::Slash("/queue".to_string())),
        [_, group, action] if group == "queue" && action == "status" => {
            Ok(Command::Slash("/queue status".to_string()))
        }
        [_, group, prompt @ ..] if group == "queue" => {
            Ok(Command::Slash(format!("/queue {}", prompt.join(" "))))
        }
        [_, group, action] if group == "sessions" && action == "list" => Ok(Command::SessionsList),
        [_, group, action] if group == "sessions" && action == "latest" => {
            Ok(Command::SessionsLatest)
        }
        [_, group, action] if group == "sessions" && action == "continue" => {
            Ok(Command::SessionsContinue)
        }
        [_, group, action, selector @ ..] if group == "sessions" && action == "resume" => {
            let selector = selector.join(" ").trim().to_string();
            if selector.is_empty() {
                return Err(CliError::InvalidUsage(usage().to_string()));
            }
            Ok(Command::SessionsResume(selector))
        }
        [_, group, action] if group == "sessions" && action == "title" => {
            Ok(Command::SessionsTitle(None))
        }
        [_, group, action, title @ ..] if group == "sessions" && action == "title" => {
            let title = title.join(" ").trim().to_string();
            Ok(Command::SessionsTitle(Some(title)))
        }
        [_, group, action] if group == "sessions" && action == "active" => {
            Ok(Command::Slash("/sessions active".to_string()))
        }
        [_, group, action, subaction]
            if group == "sessions" && action == "active" && subaction == "clear" =>
        {
            Ok(Command::Slash("/sessions active clear".to_string()))
        }
        [_, group, action, query @ ..] if group == "sessions" && action == "search" => {
            let query = query.join(" ").trim().to_string();
            if query.is_empty() {
                return Err(CliError::InvalidUsage(usage().to_string()));
            }
            Ok(Command::SessionsSearch(query))
        }
        [_, group, action, selector @ ..]
            if group == "sessions" && matches!(action.as_str(), "history" | "replay") =>
        {
            let selector = selector.join(" ").trim().to_string();
            if selector.is_empty() {
                return Err(CliError::InvalidUsage(usage().to_string()));
            }
            Ok(Command::Slash(format!("/sessions {action} {selector}")))
        }
        [_, group, action, id] if group == "sessions" && action == "get" => {
            Ok(Command::SessionsGet(id.clone()))
        }
        [_, group, action, id, title] if group == "sessions" && action == "rename" => {
            Ok(Command::SessionsRename(id.clone(), title.clone()))
        }
        _ => Err(CliError::InvalidUsage(usage().to_string())),
    }
}

fn usage() -> &'static str {
    concat!(
        "usage:\n",
        "  hermes-operator-cli\tstart interactive repl\n",
        "  hermes-operator-cli runtime status\tshow runtime status\n",
        "  hermes-operator-cli busy [queue|interrupt|status]\tshow or persist busy-input routing\n",
        "  hermes-operator-cli interrupt [request]\trequest an interrupt/cancel for the active REPL turn\n",
        "  hermes-operator-cli cancel [request]\talias of interrupt for the active REPL turn\n",
        "  hermes-operator-cli stop [request]\talias of interrupt for the active REPL turn\n",
        "  hermes-operator-cli missions list\tlist known missions\n",
        "  hermes-operator-cli missions get <id>\tshow one mission summary\n",
        "  hermes-operator-cli missions plan <id>\tgenerate a plan for one mission\n",
        "  hermes-operator-cli missions status <id> <status>\tupdate mission status\n",
        "  hermes-operator-cli sessions list\tlist recent sessions as <id>\\t<source>\\t<title>\n",
        "  hermes-operator-cli sessions latest\tshow the most recent session as <id>\\t<source>\\t<title>\\t<model>\n",
        "  hermes-operator-cli sessions continue\talias of sessions latest; returns the same row for continuation flows\n",
        "  hermes-operator-cli sessions resume <session-id|title>\tresume by exact id or recent title match\n",
        "  hermes-operator-cli sessions title\tshow the latest session title\n",
        "  hermes-operator-cli sessions title <title>\trename the latest session\n",
        "  hermes-operator-cli sessions search <query>\tfilter recent sessions by id/title/source\n",
        "  hermes-operator-cli sessions active\tshow the current active session handoff\n",
        "  hermes-operator-cli sessions active clear\tclear the current active session handoff\n",
        "  hermes-operator-cli sessions history <session-id|active|latest>\tshow session transcript lines for one session\n",
        "  hermes-operator-cli sessions replay <session-id|active|latest>\talias of sessions history\n",
        "  hermes-operator-cli sessions get <id>\tshow one session as <id>\\t<source>\\t<title>\\t<model>\n",
        "  hermes-operator-cli sessions rename <id> <title>\trename one session and print the updated row as <id>\\t<source>\\t<title>\\t<model>\n",
        "  hermes-operator-cli foreground status\tshow persisted foreground executor snapshot from the CLI store\n",
        "  hermes-operator-cli background <prompt>\tqueue a prompt as a background mission\n",
        "  hermes-operator-cli background status\tlist queued background missions\n",
        "  hermes-operator-cli background latest\tshow the most recent queued background mission\n",
        "  hermes-operator-cli queue <prompt>\tqueue a prompt for the next plain submission\n",
        "  hermes-operator-cli queue status\tlist queued prompts for the next plain submission\n",
    )
}

#[cfg(test)]
mod tests {
    use super::foreground_store::{ForegroundSnapshot, save_snapshot_for_db};
    use super::*;
    use hermes_desktop::backend::{
        CreateSessionInput, CreateSessionMessageInput, Database, SessionMessageRole,
        SessionService, SessionServiceImpl, SessionSource, create_app_state,
    };
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TempHome {
        root: PathBuf,
        previous_home: Option<OsString>,
        previous_xdg_data_home: Option<OsString>,
    }

    impl TempHome {
        fn new() -> Self {
            let suffix = format!(
                "{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time should be after unix epoch")
                    .as_nanos()
            );
            let root = std::env::temp_dir().join(format!("hermes-cli-mod-test-{suffix}"));
            fs::create_dir_all(&root).expect("create temp home");

            let xdg_data_home = root.join(".local").join("share");
            fs::create_dir_all(&xdg_data_home).expect("create temp xdg data");

            let previous_home = env::var_os("HOME");
            let previous_xdg_data_home = env::var_os("XDG_DATA_HOME");
            unsafe {
                env::set_var("HOME", &root);
                env::set_var("XDG_DATA_HOME", &xdg_data_home);
            }

            Self {
                root,
                previous_home,
                previous_xdg_data_home,
            }
        }
    }

    impl Drop for TempHome {
        fn drop(&mut self) {
            unsafe {
                if let Some(value) = self.previous_home.as_ref() {
                    env::set_var("HOME", value);
                } else {
                    env::remove_var("HOME");
                }

                if let Some(value) = self.previous_xdg_data_home.as_ref() {
                    env::set_var("XDG_DATA_HOME", value);
                } else {
                    env::remove_var("XDG_DATA_HOME");
                }
            }

            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn seed_foreground_snapshot(snapshot: &ForegroundSnapshot) {
        let state = create_app_state().expect("create app state");
        let db_path = {
            let guard = state.read();
            guard.db_path.clone()
        };
        let db = Database::new(&db_path).expect("open db");
        save_snapshot_for_db(&db, snapshot).expect("seed foreground snapshot");
    }

    fn seed_session_with_messages(title: &str, messages: &[(&str, &str)]) -> String {
        let state = create_app_state().expect("create app state");
        let db_path = {
            let guard = state.read();
            guard.db_path.clone()
        };
        let db = Database::new(&db_path).expect("open db");
        let service = SessionServiceImpl::new(db);
        let session = service
            .create(CreateSessionInput {
                source: SessionSource::Cli,
                title: title.to_string(),
                model_name: Some("gpt-5.4".to_string()),
                parent_session_id: None,
            })
            .expect("create session");

        for (role, content) in messages {
            service
                .create_message(CreateSessionMessageInput {
                    session_id: session.id.clone(),
                    role: SessionMessageRole::from_key(role),
                    content: (*content).to_string(),
                    source: "local".to_string(),
                })
                .expect("create session message");
        }

        session.id
    }

    #[test]
    fn parses_no_args_as_repl_command() {
        let command = parse_command(["hermes-operator-cli"]).expect("command parses");

        assert_eq!(command, Command::Repl);
    }

    #[test]
    fn parses_runtime_status_command() {
        let command =
            parse_command(["hermes-operator-cli", "runtime", "status"]).expect("command parses");

        assert_eq!(command, Command::RuntimeStatus);
    }

    #[test]
    fn parses_busy_direct_commands_as_busy_slash_commands() {
        assert_eq!(
            parse_command(["hermes-operator-cli", "busy"]).expect("busy parses"),
            Command::Slash("/busy".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "busy", "status"]).expect("busy status parses"),
            Command::Slash("/busy status".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "busy", "queue"]).expect("busy queue parses"),
            Command::Slash("/busy queue".to_string())
        );
    }

    #[test]
    fn parses_missions_list_command() {
        let command =
            parse_command(["hermes-operator-cli", "missions", "list"]).expect("command parses");

        assert_eq!(command, Command::MissionsList);
    }

    #[test]
    fn parses_missions_subcommands_as_slash_commands() {
        assert_eq!(
            parse_command(["hermes-operator-cli", "missions", "get", "mission-001"])
                .expect("missions get parses"),
            Command::Slash("/missions get mission-001".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "missions", "plan", "mission-001"])
                .expect("missions plan parses"),
            Command::Slash("/missions plan mission-001".to_string())
        );
        assert_eq!(
            parse_command([
                "hermes-operator-cli",
                "missions",
                "status",
                "mission-001",
                "paused",
            ])
            .expect("missions status parses"),
            Command::Slash("/missions status mission-001 paused".to_string())
        );
    }

    #[test]
    fn parses_sessions_list_command() {
        let command =
            parse_command(["hermes-operator-cli", "sessions", "list"]).expect("command parses");

        assert_eq!(command, Command::SessionsList);
    }

    #[test]
    fn parses_sessions_latest_command() {
        let command =
            parse_command(["hermes-operator-cli", "sessions", "latest"]).expect("command parses");

        assert_eq!(command, Command::SessionsLatest);
    }

    #[test]
    fn parses_sessions_continue_command() {
        let command =
            parse_command(["hermes-operator-cli", "sessions", "continue"]).expect("command parses");

        assert_eq!(command, Command::SessionsContinue);
    }

    #[test]
    fn parses_sessions_resume_title_and_search_commands() {
        assert_eq!(
            parse_command([
                "hermes-operator-cli",
                "sessions",
                "resume",
                "Quarterly",
                "planning",
            ])
            .expect("resume parses"),
            Command::SessionsResume("Quarterly planning".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "sessions", "title"]).expect("title show parses"),
            Command::SessionsTitle(None)
        );
        assert_eq!(
            parse_command([
                "hermes-operator-cli",
                "sessions",
                "title",
                "Renamed",
                "session",
            ])
            .expect("title rename parses"),
            Command::SessionsTitle(Some("Renamed session".to_string()))
        );
        assert_eq!(
            parse_command([
                "hermes-operator-cli",
                "sessions",
                "search",
                "quarterly",
                "planning",
            ])
            .expect("search parses"),
            Command::SessionsSearch("quarterly planning".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "sessions", "active"])
                .expect("active handoff parses"),
            Command::Slash("/sessions active".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "sessions", "active", "clear"])
                .expect("active handoff clear parses"),
            Command::Slash("/sessions active clear".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "sessions", "history", "latest"])
                .expect("history latest parses"),
            Command::Slash("/sessions history latest".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "sessions", "replay", "active"])
                .expect("replay active parses"),
            Command::Slash("/sessions replay active".to_string())
        );
    }

    #[test]
    fn parses_sessions_get_command() {
        let command = parse_command(["hermes-operator-cli", "sessions", "get", "session-001"])
            .expect("command parses");

        assert_eq!(command, Command::SessionsGet("session-001".to_string()));
    }

    #[test]
    fn parses_sessions_rename_command() {
        let command = parse_command([
            "hermes-operator-cli",
            "sessions",
            "rename",
            "session-001",
            "Renamed session",
        ])
        .expect("command parses");

        assert_eq!(
            command,
            Command::SessionsRename("session-001".to_string(), "Renamed session".to_string())
        );
    }

    #[test]
    fn parses_slash_help_command() {
        let command = parse_command(["hermes-operator-cli", "/help"]);

        assert!(command.is_ok(), "slash help command should parse");
    }

    #[test]
    fn parses_slash_command_with_inline_argument_tokens() {
        let command = parse_command(["hermes-operator-cli", "/resume", "session-001"]);

        assert_eq!(
            command.expect("slash command should parse"),
            Command::Slash("/resume session-001".to_string())
        );
    }

    #[test]
    fn parses_background_direct_commands_as_background_slash_commands() {
        assert_eq!(
            parse_command(["hermes-operator-cli", "background"]).expect("background parses"),
            Command::Slash("/background".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "background", "status"])
                .expect("background status parses"),
            Command::Slash("/background status".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "background", "latest"])
                .expect("background latest parses"),
            Command::Slash("/background latest".to_string())
        );
        assert_eq!(
            parse_command([
                "hermes-operator-cli",
                "background",
                "summarize",
                "quarterly",
                "roadmap",
            ])
            .expect("background prompt parses"),
            Command::Slash("/background summarize quarterly roadmap".to_string())
        );
    }

    #[test]
    fn parses_foreground_direct_status_command_as_foreground_slash_command() {
        assert_eq!(
            parse_command(["hermes-operator-cli", "foreground", "status"])
                .expect("foreground status parses"),
            Command::Slash("/foreground status".to_string())
        );
    }

    #[test]
    fn parses_queue_direct_commands_as_queue_slash_commands() {
        assert_eq!(
            parse_command(["hermes-operator-cli", "queue"]).expect("queue parses"),
            Command::Slash("/queue".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "queue", "status"]).expect("queue status parses"),
            Command::Slash("/queue status".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "queue", "follow", "up", "prompt",])
                .expect("queue prompt parses"),
            Command::Slash("/queue follow up prompt".to_string())
        );
    }

    #[test]
    fn parses_interrupt_direct_commands_as_interrupt_slash_commands() {
        assert_eq!(
            parse_command(["hermes-operator-cli", "interrupt"]).expect("interrupt parses"),
            Command::Slash("/interrupt".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "cancel", "current", "turn"])
                .expect("cancel parses"),
            Command::Slash("/cancel current turn".to_string())
        );
        assert_eq!(
            parse_command(["hermes-operator-cli", "stop", "after", "this", "reply"])
                .expect("stop parses"),
            Command::Slash("/stop after this reply".to_string())
        );
    }

    #[test]
    fn rejects_unknown_command_with_usage() {
        let error =
            parse_command(["hermes-operator-cli", "runtime"]).expect_err("command should fail");

        assert_eq!(
            error.to_string(),
            "usage:\n  hermes-operator-cli\tstart interactive repl\n  hermes-operator-cli runtime status\tshow runtime status\n  hermes-operator-cli busy [queue|interrupt|status]\tshow or persist busy-input routing\n  hermes-operator-cli interrupt [request]\trequest an interrupt/cancel for the active REPL turn\n  hermes-operator-cli cancel [request]\talias of interrupt for the active REPL turn\n  hermes-operator-cli stop [request]\talias of interrupt for the active REPL turn\n  hermes-operator-cli missions list\tlist known missions\n  hermes-operator-cli missions get <id>\tshow one mission summary\n  hermes-operator-cli missions plan <id>\tgenerate a plan for one mission\n  hermes-operator-cli missions status <id> <status>\tupdate mission status\n  hermes-operator-cli sessions list\tlist recent sessions as <id>\\t<source>\\t<title>\n  hermes-operator-cli sessions latest\tshow the most recent session as <id>\\t<source>\\t<title>\\t<model>\n  hermes-operator-cli sessions continue\talias of sessions latest; returns the same row for continuation flows\n  hermes-operator-cli sessions resume <session-id|title>\tresume by exact id or recent title match\n  hermes-operator-cli sessions title\tshow the latest session title\n  hermes-operator-cli sessions title <title>\trename the latest session\n  hermes-operator-cli sessions search <query>\tfilter recent sessions by id/title/source\n  hermes-operator-cli sessions active\tshow the current active session handoff\n  hermes-operator-cli sessions active clear\tclear the current active session handoff\n  hermes-operator-cli sessions history <session-id|active|latest>\tshow session transcript lines for one session\n  hermes-operator-cli sessions replay <session-id|active|latest>\talias of sessions history\n  hermes-operator-cli sessions get <id>\tshow one session as <id>\\t<source>\\t<title>\\t<model>\n  hermes-operator-cli sessions rename <id> <title>\trename one session and print the updated row as <id>\\t<source>\\t<title>\\t<model>\n  hermes-operator-cli foreground status\tshow persisted foreground executor snapshot from the CLI store\n  hermes-operator-cli background <prompt>\tqueue a prompt as a background mission\n  hermes-operator-cli background status\tlist queued background missions\n  hermes-operator-cli background latest\tshow the most recent queued background mission\n  hermes-operator-cli queue <prompt>\tqueue a prompt for the next plain submission\n  hermes-operator-cli queue status\tlist queued prompts for the next plain submission\n"
        );
    }

    #[test]
    fn usage_clarifies_repl_and_sessions_commands() {
        let usage = usage();

        assert!(usage.contains("hermes-operator-cli\tstart interactive repl"));
        assert!(usage.contains(
            "hermes-operator-cli busy [queue|interrupt|status]\tshow or persist busy-input routing"
        ));
        assert!(usage.contains(
            "hermes-operator-cli interrupt [request]\trequest an interrupt/cancel for the active REPL turn"
        ));
        assert!(usage.contains(
            "hermes-operator-cli cancel [request]\talias of interrupt for the active REPL turn"
        ));
        assert!(usage.contains(
            "hermes-operator-cli stop [request]\talias of interrupt for the active REPL turn"
        ));
        assert!(usage.contains(
            "hermes-operator-cli background <prompt>\tqueue a prompt as a background mission"
        ));
        assert!(usage.contains(
            "hermes-operator-cli foreground status\tshow persisted foreground executor snapshot from the CLI store"
        ));
        assert!(
            usage
                .contains("hermes-operator-cli background status\tlist queued background missions")
        );
        assert!(usage.contains(
            "hermes-operator-cli background latest\tshow the most recent queued background mission"
        ));
        assert!(usage.contains("hermes-operator-cli missions get <id>\tshow one mission summary"));
        assert!(
            usage.contains(
                "hermes-operator-cli missions plan <id>\tgenerate a plan for one mission"
            )
        );
        assert!(
            usage.contains(
                "hermes-operator-cli missions status <id> <status>\tupdate mission status"
            )
        );
        assert!(usage.contains(
            "hermes-operator-cli queue <prompt>\tqueue a prompt for the next plain submission"
        ));
        assert!(usage.contains(
            "hermes-operator-cli queue status\tlist queued prompts for the next plain submission"
        ));
        assert!(usage.contains(
            "hermes-operator-cli sessions latest\tshow the most recent session as <id>\\t<source>\\t<title>\\t<model>"
        ));
        assert!(usage.contains(
            "hermes-operator-cli sessions continue\talias of sessions latest; returns the same row for continuation flows"
        ));
        assert!(usage.contains(
            "hermes-operator-cli sessions resume <session-id|title>\tresume by exact id or recent title match"
        ));
        assert!(
            usage.contains("hermes-operator-cli sessions title\tshow the latest session title")
        );
        assert!(usage.contains(
            "hermes-operator-cli sessions search <query>\tfilter recent sessions by id/title/source"
        ));
        assert!(usage.contains(
            "hermes-operator-cli sessions active\tshow the current active session handoff"
        ));
        assert!(usage.contains(
            "hermes-operator-cli sessions active clear\tclear the current active session handoff"
        ));
        assert!(usage.contains(
            "hermes-operator-cli sessions history <session-id|active|latest>\tshow session transcript lines for one session"
        ));
        assert!(usage.contains(
            "hermes-operator-cli sessions replay <session-id|active|latest>\talias of sessions history"
        ));
        assert!(usage.contains(
            "hermes-operator-cli sessions rename <id> <title>\trename one session and print the updated row as <id>\\t<source>\\t<title>\\t<model>"
        ));
    }

    #[test]
    fn runs_slash_help_command() {
        let mut output = Vec::new();

        run_with(
            ["hermes-operator-cli", "/help"],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("slash help should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            concat!(
                "/help\tshow slash command index and current CLI/TUI parity notes\n",
                "/model [provider:model|provider model|model]\tshow or persist the current provider/model selection\n",
                "/busy [queue|interrupt|status]\tshow or persist how busy plain-text input is routed\n",
                "/interrupt [follow-up prompt]\trequest cancellation/interrupt when a foreground turn is busy; explain idle behavior otherwise\n",
                "/cancel [follow-up prompt]\talias of /interrupt for busy foreground turns; explain idle behavior otherwise\n",
                "/stop [follow-up prompt]\talias of /interrupt for busy foreground turns; explain idle behavior otherwise\n",
                "/tools\tlist discovered tool surfaces and availability hints\n",
                "/skills [list|search|view|install|enable|disable]\tlist, inspect, install, toggle, or invoke discovered skills as /<skill>\n",
                "/title [new title]\tshow or rename the latest session title\n",
                "/foreground status\tshow persisted foreground executor snapshot from the CLI store\n",
                "/background <prompt> | status | latest\tqueue a prompt as a background mission or inspect queued background runs\n",
                "/queue <prompt> | status\tqueue a prompt for the next plain-text submission without interrupting the current turn\n",
                "/usage\tshow active mission / approval / recent session summary\n",
                "/voice [on|off|status|transcribe|speak]\tshow, toggle, transcribe, or queue local voice workflow events\n",
                "/continue\tload the latest session row for continuation flows\n",
                "/resume <session-id|title>\tresume by exact id or recent title match\n",
                "/sessions [latest|continue|resume|title|search|active|history|replay]\tshow session lifecycle commands and examples\n",
                "/missions [list|get|plan|status]\tlist missions, inspect one mission, generate a plan, or update mission status\n",
                "note\tplain prompts queue background missions; /queue stores prompts for the next plain submission; use trailing \\ for multi-line input; busy REPL interrupt aliases are consumed before slash handling\n",
            )
        );
    }

    #[test]
    fn direct_interrupt_commands_run_through_slash_handler() {
        let mut output = Vec::new();

        run_with(
            ["hermes-operator-cli", "stop", "after", "this", "reply"],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("direct stop command should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            concat!(
                "interrupt\talias=/stop\tstatus=idle\tfollow_up=after this reply\n",
                "interrupt\tnote\tinterrupt commands are consumed by the foreground controller only while a REPL turn is busy; there is no active foreground turn to interrupt right now.\n",
                "interrupt\thint\tsubmit a prompt first, then use /interrupt, /cancel, or /stop while that turn is still running.\n",
            )
        );
    }

    #[test]
    fn runs_foreground_direct_status_command_via_persisted_snapshot_output() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        seed_foreground_snapshot(&ForegroundSnapshot {
            active: true,
            state: "running".to_string(),
            session_id: Some("session-123".to_string()),
            run_id: Some("run-456".to_string()),
            cancel_state: Some("requested".to_string()),
            pending_count: 3,
            interrupt_count: 1,
            updated_at: "2026-04-24T00:00:00Z".to_string(),
        });

        let mut output = Vec::new();

        run_with(
            ["hermes-operator-cli", "foreground", "status"],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("direct foreground status should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            concat!(
                "foreground\tstatus\tstate=running\tactive=true\tsession_id=session-123\trun_id=run-456\tcancel_state=requested\tpending=3\tinterrupts=1\tupdated_at=2026-04-24T00:00:00Z\n",
                "foreground\tnote\tsnapshot_source=cli_foreground_store\tfreshness=persisted\tvalues reflect the latest saved foreground status snapshot.\n",
            )
        );
    }

    #[test]
    fn runs_background_direct_commands_via_existing_slash_behavior() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();

        let mut queued_output = Vec::new();
        run_with(
            [
                "hermes-operator-cli",
                "background",
                "summarize",
                "quarterly",
                "roadmap",
            ],
            &mut queued_output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("background enqueue should succeed");

        let queued_output = String::from_utf8(queued_output).expect("output is utf8");
        assert!(queued_output.contains("background\tqueued\tmission_id="));
        assert!(queued_output.contains("\trun_id="));
        assert!(queued_output.contains("\tstatus=queued"));
        assert!(queued_output.contains("\tprompt=summarize quarterly roadmap"));

        let mut status_output = Vec::new();
        run_with(
            ["hermes-operator-cli", "background", "status"],
            &mut status_output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("background status should succeed");

        let status_output = String::from_utf8(status_output).expect("output is utf8");
        assert!(status_output.contains("background\tcount=1"));
        assert!(status_output.contains("\tmission_status=awaiting_approval"));
        assert!(status_output.contains("\trun_status=queued"));

        let mut latest_output = Vec::new();
        run_with(
            ["hermes-operator-cli", "background", "latest"],
            &mut latest_output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("background latest should succeed");

        let latest_output = String::from_utf8(latest_output).expect("output is utf8");
        assert!(latest_output.contains("background\tlatest\tmission_id="));
        assert!(latest_output.contains("\tprompt=summarize quarterly roadmap"));
    }

    #[test]
    fn runs_busy_direct_commands_via_existing_slash_behavior() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();

        let mut queue_output = Vec::new();
        run_with(
            ["hermes-operator-cli", "busy", "queue"],
            &mut queue_output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("busy queue should succeed");
        assert_eq!(
            String::from_utf8(queue_output).expect("output is utf8"),
            concat!(
                "busy_input_mode\tmode=queue\n",
                "busy_input_mode\tnote\tbusy plain-text input will wait for the next foreground turn\n",
            )
        );

        let mut status_output = Vec::new();
        run_with(
            ["hermes-operator-cli", "busy", "status"],
            &mut status_output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("busy status should succeed");
        assert_eq!(
            String::from_utf8(status_output).expect("output is utf8"),
            concat!(
                "busy_input_mode\tmode=queue\n",
                "busy_input_mode\tnote\tbusy plain-text input will wait for the next foreground turn\n",
            )
        );
    }

    #[test]
    fn runs_queue_direct_commands_via_existing_slash_behavior() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();

        let mut queued_output = Vec::new();
        run_with(
            ["hermes-operator-cli", "queue", "follow", "up", "prompt"],
            &mut queued_output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("queue enqueue should succeed");

        assert_eq!(
            String::from_utf8(queued_output).expect("output is utf8"),
            "queue\tqueued\tcount=1\tprompt=follow up prompt\n"
        );

        let mut status_output = Vec::new();
        run_with(
            ["hermes-operator-cli", "queue", "status"],
            &mut status_output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("queue status should succeed");

        let status_output = String::from_utf8(status_output).expect("output is utf8");
        assert!(status_output.contains("queue\tcount=1"));
        assert!(status_output.contains("queue\titem\tindex=1\tprompt=follow up prompt"));
    }

    #[test]
    fn runs_sessions_resume_title_and_search_commands() {
        let mut output = Vec::new();

        run_with(
            [
                "hermes-operator-cli",
                "sessions",
                "resume",
                "Quarterly planning",
            ],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || {
                Ok(vec![sessions::SessionListItem {
                    id: "session-002".to_string(),
                    source: "cli".to_string(),
                    title: "Quarterly planning review".to_string(),
                    model_name: None,
                }])
            },
            |id| {
                assert_eq!(id, "Quarterly planning");
                Ok(None)
            },
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("sessions resume should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            "session-002\tcli\tQuarterly planning review\t-\n"
        );

        let mut output = Vec::new();

        run_with(
            ["hermes-operator-cli", "sessions", "title"],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || {
                Ok(Some(
                    "session-003\tcli\tCurrent title\tgpt-5.4\n".to_string(),
                ))
            },
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("sessions title should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            "title\tsession-003\tCurrent title\n"
        );

        let mut output = Vec::new();

        run_with(
            ["hermes-operator-cli", "sessions", "search", "cli"],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || {
                Ok(vec![
                    sessions::SessionListItem {
                        id: "session-001".to_string(),
                        source: "cli".to_string(),
                        title: "CLI planning".to_string(),
                        model_name: None,
                    },
                    sessions::SessionListItem {
                        id: "session-002".to_string(),
                        source: "desktop".to_string(),
                        title: "Desktop review".to_string(),
                        model_name: None,
                    },
                ])
            },
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("sessions search should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            "session-001\tcli\tCLI planning\t-\n"
        );
    }

    #[test]
    fn runs_sessions_active_direct_commands_via_existing_slash_behavior() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();

        let mut output = Vec::new();
        run(["hermes-operator-cli", "sessions", "active"], &mut output)
            .expect("sessions active should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            "sessions\tactive\tnone\n"
        );
    }

    #[test]
    fn runs_runtime_status_command() {
        let mut output = Vec::new();

        run_with(
            ["hermes-operator-cli", "runtime", "status"],
            &mut output,
            || {
                Ok(runtime::RuntimeStatusSnapshot {
                    engine_running: true,
                    engine_profile: Some("default".to_string()),
                    engine_pid: Some(4242),
                    hermes_installed: true,
                    hermes_running: true,
                    hermes_version: Some("hermes 0.1.0".to_string()),
                    hermes_pid: Some(5151),
                })
            },
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("runtime status should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            "runtime\tengine=running\tprofile=default\tpid=4242\nhermes\tinstalled=true\trunning=true\tversion=hermes 0.1.0\tpid=5151\n"
        );
    }

    #[test]
    fn runs_missions_list_command() {
        let mut output = Vec::new();

        run_with(
            ["hermes-operator-cli", "missions", "list"],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || {
                Ok(vec![missions::MissionListItem {
                    id: "mission-001".to_string(),
                    title: "Bootstrap Hermes parity".to_string(),
                    status: "planning".to_string(),
                    priority: "medium".to_string(),
                }])
            },
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("missions list should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            "mission-001\tplanning\tmedium\tBootstrap Hermes parity\n"
        );
    }

    #[test]
    fn runs_sessions_list_command() {
        let mut output = Vec::new();

        run_with(
            ["hermes-operator-cli", "sessions", "list"],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || {
                Ok(vec![sessions::SessionListItem {
                    id: "session-001".to_string(),
                    source: "cli".to_string(),
                    title: "Recent session".to_string(),
                    model_name: Some("gpt-4o".to_string()),
                }])
            },
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("sessions list should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            "session-001\tcli\tRecent session\n"
        );
    }

    #[test]
    fn runs_sessions_latest_command() {
        let mut output = Vec::new();

        run_with(
            ["hermes-operator-cli", "sessions", "latest"],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || {
                Ok(Some(
                    "session-001\tcli\tLatest session\tgpt-4o\n".to_string(),
                ))
            },
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("sessions latest should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            "session-001\tcli\tLatest session\tgpt-4o\n"
        );
    }

    #[test]
    fn runs_sessions_get_command() {
        let mut output = Vec::new();

        run_with(
            ["hermes-operator-cli", "sessions", "get", "session-001"],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |id| {
                assert_eq!(id, "session-001");
                Ok(Some(
                    "session-001\tcli\tLatest session\tgpt-4o\n".to_string(),
                ))
            },
            || unreachable!("latest session getter should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("sessions get should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            "session-001\tcli\tLatest session\tgpt-4o\n"
        );
    }

    #[test]
    fn runs_sessions_continue_command() {
        let mut output = Vec::new();

        run_with(
            ["hermes-operator-cli", "sessions", "continue"],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || {
                Ok(Some(
                    "session-001\tcli\tLatest session\tgpt-4o\n".to_string(),
                ))
            },
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("sessions continue should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            "session-001\tcli\tLatest session\tgpt-4o\n"
        );
    }

    #[test]
    fn runs_sessions_rename_command() {
        let mut output = Vec::new();

        run_with(
            [
                "hermes-operator-cli",
                "sessions",
                "rename",
                "session-001",
                "Renamed session",
            ],
            &mut output,
            || unreachable!("runtime loader should not be used"),
            || unreachable!("mission loader should not be used"),
            || unreachable!("sessions loader should not be used"),
            |_| unreachable!("session getter should not be used"),
            || unreachable!("latest session getter should not be used"),
            |id, title| {
                assert_eq!(id, "session-001");
                assert_eq!(title, "Renamed session");
                Ok("session-001\tcli\tRenamed session\tgpt-4o\n".to_string())
            },
        )
        .expect("sessions rename should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            "session-001\tcli\tRenamed session\tgpt-4o\n"
        );
    }

    #[test]
    fn runs_sessions_history_command_via_direct_subcommand() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let session_id = seed_session_with_messages(
            "CLI transcript",
            &[("user", "first prompt"), ("assistant", "second reply")],
        );
        let mut output = Vec::new();

        run(
            ["hermes-operator-cli", "sessions", "history", "latest"],
            &mut output,
        )
        .expect("sessions history should succeed");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            concat!(
                "session_history\tresolved_via=latest\tsession_id=",
                "{session_id}",
                "\tsource=cli\ttitle=CLI transcript\tcount=2\n",
                "session_message\tsession_id=",
                "{session_id}",
                "\trole=user\tsource=local\tcontent_json=\"first prompt\"\n",
                "session_message\tsession_id=",
                "{session_id}",
                "\trole=assistant\tsource=local\tcontent_json=\"second reply\"\n",
            )
            .replace("{session_id}", &session_id)
        );
    }
}
