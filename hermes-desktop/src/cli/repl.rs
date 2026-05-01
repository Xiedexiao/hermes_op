use std::io::{self, BufRead, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use hermes_desktop::backend::{Database, config, create_app_state};
use uuid::Uuid;

use super::CliError;
use super::foreground::{
    BusyInputMode, BusySubmitOutcome, ForegroundInputKind, ForegroundTurnController, RunId,
};
use super::foreground_store::{ForegroundSnapshot, clear_snapshot_for_db, save_snapshot_for_db};

const PRIMARY_PROMPT: &str = "> ";
const CONTINUATION_PROMPT: &str = "... ";
const REPL_POLL_INTERVAL: Duration = Duration::from_millis(25);
const DEFAULT_STOP_FOLLOW_UP: &str =
    "Please stop the current response and wait for the next user instruction.";
const WELCOME_BANNER: &str = concat!(
    "Hermes operator REPL\n",
    "Type `exit` to quit, `/help` for commands.\n",
    "Plain prompts queue background missions. Use `/queue` to append prompts to the next plain submission. Multi-line input uses trailing \\\\; busy /interrupt, /cancel, and /stop request cancellation and can seed the next continuation.\n",
    "> "
);
const STALE_HELP_NOTE: &str = concat!(
    "note\tplain prompts queue background missions; /queue stores prompts for the next plain submission; ",
    "use trailing \\ for multi-line input; interrupt is not wired yet\n"
);
const BUSY_ALIAS_HELP_NOTE: &str = concat!(
    "note\tplain prompts queue background missions; /queue stores prompts for the next plain submission; ",
    "use trailing \\ for multi-line input; busy REPL interrupt aliases are consumed before slash handling\n"
);
const REPL_HELP_NOTE: &str = concat!(
    "note\tplain prompts queue background missions; /queue stores prompts for the next plain submission; ",
    "use trailing \\ for multi-line input; busy /interrupt, /cancel, and /stop request cancellation and can seed the next continuation\n"
);
const INTERRUPT_ORDER_NOTE: &str = "interrupt\tnote\tactive turn marked for cancellation; continuation runs before queued follow-up input.\n";
const QUEUE_ORDER_NOTE: &str =
    "queue\tnote\tqueued follow-up waits for the active turn and any interrupt continuation.\n";

pub fn run<R, W>(reader: R, writer: &mut W) -> Result<(), CliError>
where
    R: BufRead + Send + 'static,
    W: Write,
{
    run_with_processor(
        reader,
        writer,
        configured_busy_input_mode(),
        process_plain_prompt,
    )
}

fn run_with_processor<R, W, F>(
    reader: R,
    writer: &mut W,
    busy_input_mode: BusyInputMode,
    processor: F,
) -> Result<(), CliError>
where
    R: BufRead + Send + 'static,
    W: Write,
    F: Fn(String, Arc<AtomicBool>) -> Result<String, CliError> + Send + Sync + 'static,
{
    writer.write_all(WELCOME_BANNER.as_bytes())?;

    let db = open_cli_database()?;
    let session_id = format!("foreground-{}", Uuid::new_v4());
    save_repl_snapshot(
        &db,
        build_snapshot(
            &session_id,
            None,
            &ForegroundTurnController::new(busy_input_mode),
        ),
    )?;

    let (input_tx, input_rx) = mpsc::channel::<Result<Option<String>, io::Error>>();
    thread::spawn(move || {
        let mut reader = reader;
        loop {
            let mut line = String::new();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = input_tx.send(Ok(None));
                    break;
                }
                Ok(_) => {
                    if input_tx.send(Ok(Some(line))).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    let _ = input_tx.send(Err(error));
                    break;
                }
            }
        }
    });

    let processor = Arc::new(processor);
    let mut controller = ForegroundTurnController::new(busy_input_mode);
    let mut active_run: Option<ActiveForegroundRun> = None;
    let mut pending_prompt: Option<String> = None;
    let mut should_exit = false;

    loop {
        let run_completed = flush_completed_run(
            &db,
            &session_id,
            &mut controller,
            &mut active_run,
            processor.clone(),
            writer,
        )?;
        if run_completed {
            if should_exit && active_run.is_none() {
                break;
            }
            writer.write_all(PRIMARY_PROMPT.as_bytes())?;
            continue;
        }

        if should_exit {
            if active_run.is_none() {
                break;
            }
            thread::sleep(REPL_POLL_INTERVAL);
            continue;
        }

        match input_rx.recv_timeout(REPL_POLL_INTERVAL) {
            Ok(Ok(Some(raw_line))) => {
                let line = trim_line_endings(&raw_line);
                if line.trim() == "exit" {
                    should_exit = true;
                    continue;
                }
                handle_line(
                    line,
                    &mut pending_prompt,
                    &mut controller,
                    &mut active_run,
                    processor.clone(),
                    &db,
                    &session_id,
                    writer,
                )?;
            }
            Ok(Ok(None)) => {
                should_exit = true;
            }
            Ok(Err(error)) => return Err(CliError::Io(error)),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                should_exit = true;
            }
        }
    }

    clear_snapshot_for_db(&db)?;
    Ok(())
}

struct ActiveForegroundRun {
    run_id: RunId,
    cancel_flag: Arc<AtomicBool>,
    result_rx: mpsc::Receiver<Result<String, CliError>>,
}

fn process_plain_prompt(prompt: String, _cancel_flag: Arc<AtomicBool>) -> Result<String, CliError> {
    let queue = super::slash::load_queued_prompts()?;
    let prompt = combine_submission_prompt(&queue, prompt.trim_end());
    let output = super::slash::handle(&format!("/background {}", prompt))?;
    if !queue.is_empty() {
        super::slash::clear_queued_prompts()?;
    }
    Ok(output)
}

fn open_cli_database() -> Result<Database, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))
}

fn configured_busy_input_mode() -> BusyInputMode {
    match config::load_config()
        .unwrap_or_default()
        .busy_input_mode
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "queue" => BusyInputMode::Queue,
        _ => BusyInputMode::Interrupt,
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_line<W, F>(
    line: &str,
    pending_prompt: &mut Option<String>,
    controller: &mut ForegroundTurnController,
    active_run: &mut Option<ActiveForegroundRun>,
    processor: Arc<F>,
    db: &Database,
    session_id: &str,
    writer: &mut W,
) -> Result<(), CliError>
where
    W: Write,
    F: Fn(String, Arc<AtomicBool>) -> Result<String, CliError> + Send + Sync + 'static,
{
    if active_run.is_some() {
        return handle_busy_line(
            line,
            pending_prompt,
            controller,
            active_run,
            db,
            session_id,
            writer,
        );
    }

    if pending_prompt.is_none() && line.trim().starts_with('/') {
        let output = handle_slash_command(line.trim())?;
        writer.write_all(output.as_bytes())?;
        writer.write_all(PRIMARY_PROMPT.as_bytes())?;
        return Ok(());
    }

    if let Some(fragment) = strip_multiline_continuation(line) {
        append_multiline_fragment(pending_prompt, fragment);
        writer.write_all(CONTINUATION_PROMPT.as_bytes())?;
        return Ok(());
    }

    if line.trim().is_empty() && pending_prompt.is_none() {
        writer.write_all(PRIMARY_PROMPT.as_bytes())?;
        return Ok(());
    }

    let prompt = finalize_prompt(pending_prompt, line);
    spawn_foreground_run(controller, active_run, prompt, processor, db, session_id)?;
    writer.write_all(PRIMARY_PROMPT.as_bytes())?;
    Ok(())
}

fn handle_busy_line<W>(
    line: &str,
    pending_prompt: &mut Option<String>,
    controller: &mut ForegroundTurnController,
    active_run: &mut Option<ActiveForegroundRun>,
    db: &Database,
    session_id: &str,
    writer: &mut W,
) -> Result<(), CliError>
where
    W: Write,
{
    if let Some(fragment) = strip_multiline_continuation(line) {
        append_multiline_fragment(pending_prompt, fragment);
        writer.write_all(CONTINUATION_PROMPT.as_bytes())?;
        return Ok(());
    }

    let submission = finalize_prompt(pending_prompt, line);
    if submission.trim().is_empty() {
        writer.write_all(PRIMARY_PROMPT.as_bytes())?;
        return Ok(());
    }

    let outcome = if submission.trim().starts_with('/') {
        controller.submit_slash_while_busy(submission.trim())
    } else {
        controller.submit_text_while_busy(submission.trim())
    }
    .map_err(|error| CliError::Runtime(format!("foreground controller error: {error:?}")))?;

    if matches!(outcome, BusySubmitOutcome::RecordedInterrupt(_))
        && let Some(run) = active_run.as_ref()
    {
        run.cancel_flag.store(true, Ordering::SeqCst);
    }

    save_repl_snapshot(
        db,
        build_snapshot(session_id, active_run.as_ref(), controller),
    )?;

    writer.write_all(render_busy_submit_outcome(&outcome).as_bytes())?;
    writer.write_all(PRIMARY_PROMPT.as_bytes())?;
    Ok(())
}

fn append_multiline_fragment(pending_prompt: &mut Option<String>, fragment: &str) {
    match pending_prompt.as_mut() {
        Some(buffer) => {
            if !buffer.is_empty() {
                buffer.push('\n');
            }
            buffer.push_str(fragment);
        }
        None => {
            *pending_prompt = Some(fragment.to_string());
        }
    }
}

fn finalize_prompt(pending_prompt: &mut Option<String>, line: &str) -> String {
    match pending_prompt.take() {
        Some(mut buffer) => {
            if !buffer.is_empty() {
                buffer.push('\n');
            }
            buffer.push_str(line);
            buffer
        }
        None => line.to_string(),
    }
}

fn flush_completed_run<W, F>(
    db: &Database,
    session_id: &str,
    controller: &mut ForegroundTurnController,
    active_run: &mut Option<ActiveForegroundRun>,
    processor: Arc<F>,
    writer: &mut W,
) -> Result<bool, CliError>
where
    W: Write,
    F: Fn(String, Arc<AtomicBool>) -> Result<String, CliError> + Send + Sync + 'static,
{
    let Some(run) = active_run.as_ref() else {
        return Ok(false);
    };

    match run.result_rx.try_recv() {
        Ok(result) => {
            let run_id = run.run_id;
            *active_run = None;
            controller.finish_run(run_id).map_err(|error| {
                CliError::Runtime(format!("foreground controller error: {error:?}"))
            })?;
            writer.write_all(result?.as_bytes())?;
            drain_follow_up_work(db, session_id, controller, active_run, processor, writer)?;
            save_repl_snapshot(
                db,
                build_snapshot(session_id, active_run.as_ref(), controller),
            )?;
            Ok(true)
        }
        Err(mpsc::TryRecvError::Empty) => Ok(false),
        Err(mpsc::TryRecvError::Disconnected) => {
            let run_id = run.run_id;
            *active_run = None;
            controller.finish_run(run_id).map_err(|error| {
                CliError::Runtime(format!("foreground controller error: {error:?}"))
            })?;
            writer.write_all(b"foreground\terror\trun channel disconnected\n")?;
            save_repl_snapshot(
                db,
                build_snapshot(session_id, active_run.as_ref(), controller),
            )?;
            Ok(true)
        }
    }
}

fn drain_follow_up_work<W, F>(
    db: &Database,
    session_id: &str,
    controller: &mut ForegroundTurnController,
    active_run: &mut Option<ActiveForegroundRun>,
    processor: Arc<F>,
    writer: &mut W,
) -> Result<(), CliError>
where
    W: Write,
    F: Fn(String, Arc<AtomicBool>) -> Result<String, CliError> + Send + Sync + 'static,
{
    while let Some(work_item) = take_next_follow_up_work_item(controller) {
        match work_item {
            FollowUpWorkItem::ForegroundPrompt(prompt) => {
                spawn_foreground_run(controller, active_run, prompt, processor, db, session_id)?;
                return Ok(());
            }
            FollowUpWorkItem::SlashCommand(command) => {
                let output = handle_slash_command(&command)?;
                writer.write_all(output.as_bytes())?;
            }
        }
    }

    Ok(())
}

enum FollowUpWorkItem {
    ForegroundPrompt(String),
    SlashCommand(String),
}

fn take_next_follow_up_work_item(
    controller: &mut ForegroundTurnController,
) -> Option<FollowUpWorkItem> {
    if let Some(prompt) = take_interrupt_follow_up_prompt(controller) {
        return Some(FollowUpWorkItem::ForegroundPrompt(prompt));
    }

    controller
        .take_next_pending_prompt()
        .map(|prompt| match prompt.kind {
            ForegroundInputKind::PlainText => FollowUpWorkItem::ForegroundPrompt(prompt.text),
            ForegroundInputKind::SlashCommand => FollowUpWorkItem::SlashCommand(prompt.text),
        })
}

fn take_interrupt_follow_up_prompt(controller: &mut ForegroundTurnController) -> Option<String> {
    let mut parts = Vec::new();
    while let Some(request) = controller.take_next_interrupt_request() {
        if let Some(prompt) = interrupt_request_follow_up_text(&request) {
            parts.push(prompt);
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n"))
    }
}

fn interrupt_request_follow_up_text(
    request: &super::foreground::InterruptRequest,
) -> Option<String> {
    match request.kind {
        ForegroundInputKind::PlainText => {
            let text = request.text.trim();
            (!text.is_empty()).then(|| text.to_string())
        }
        ForegroundInputKind::SlashCommand => interrupt_slash_follow_up_text(&request.text),
    }
}

fn interrupt_slash_follow_up_text(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut parts = trimmed.splitn(2, char::is_whitespace);
    let command = parts
        .next()
        .unwrap_or_default()
        .trim_start_matches('/')
        .to_ascii_lowercase();
    let remainder = parts.next().unwrap_or_default().trim();

    match command.as_str() {
        "interrupt" | "stop" | "cancel" if remainder.is_empty() => {
            Some(DEFAULT_STOP_FOLLOW_UP.to_string())
        }
        "interrupt" | "stop" | "cancel" => Some(remainder.to_string()),
        _ => Some(trimmed.to_string()),
    }
}

fn render_busy_submit_outcome(outcome: &BusySubmitOutcome) -> String {
    match outcome {
        BusySubmitOutcome::QueuedPrompt(prompt) => format!(
            "queue\tbusy\tkind={}\ttext={}\n{}",
            input_kind_label(prompt.kind),
            prompt.text,
            QUEUE_ORDER_NOTE
        ),
        BusySubmitOutcome::RecordedInterrupt(request) => {
            let mut rendered = format!(
                "interrupt\trequested\tkind={}\ttext={}\n",
                input_kind_label(request.kind),
                request.text
            );
            if let Some(follow_up) = interrupt_request_follow_up_text(request) {
                rendered.push_str(format!("interrupt\tcontinuation\ttext={follow_up}\n").as_str());
            }
            rendered.push_str(INTERRUPT_ORDER_NOTE);
            rendered
        }
    }
}

fn handle_slash_command(command: &str) -> Result<String, CliError> {
    let output = super::slash::handle(command)?;
    Ok(adapt_slash_output_for_repl(&output))
}

fn adapt_slash_output_for_repl(output: &str) -> String {
    output
        .replace(BUSY_ALIAS_HELP_NOTE, REPL_HELP_NOTE)
        .replace(STALE_HELP_NOTE, REPL_HELP_NOTE)
}

fn spawn_foreground_run<F>(
    controller: &mut ForegroundTurnController,
    active_run: &mut Option<ActiveForegroundRun>,
    prompt: String,
    processor: Arc<F>,
    db: &Database,
    session_id: &str,
) -> Result<(), CliError>
where
    F: Fn(String, Arc<AtomicBool>) -> Result<String, CliError> + Send + Sync + 'static,
{
    let run_id = controller
        .start_run()
        .map_err(|error| CliError::Runtime(format!("foreground controller error: {error:?}")))?;
    let (result_tx, result_rx) = mpsc::channel();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let cancel_flag_for_worker = cancel_flag.clone();
    thread::spawn(move || {
        let result = processor(prompt, cancel_flag_for_worker);
        let _ = result_tx.send(result);
    });
    *active_run = Some(ActiveForegroundRun {
        run_id,
        cancel_flag,
        result_rx,
    });
    save_repl_snapshot(
        db,
        build_snapshot(session_id, active_run.as_ref(), controller),
    )?;
    Ok(())
}

fn build_snapshot(
    session_id: &str,
    active_run: Option<&ActiveForegroundRun>,
    controller: &ForegroundTurnController,
) -> ForegroundSnapshot {
    let status = controller.active_run_status();
    ForegroundSnapshot {
        active: active_run.is_some(),
        state: match controller.state() {
            super::foreground::ForegroundTurnState::Idle => "idle".to_string(),
            super::foreground::ForegroundTurnState::Running => "running".to_string(),
            super::foreground::ForegroundTurnState::Interrupting => "interrupting".to_string(),
        },
        session_id: Some(session_id.to_string()),
        run_id: active_run.map(|run| run.run_id.0.to_string()),
        cancel_state: status.map(|value| match value.cancel_state {
            super::foreground::ActiveRunCancelState::Active => "active".to_string(),
            super::foreground::ActiveRunCancelState::CancelRequested => {
                "cancel_requested".to_string()
            }
        }),
        pending_count: controller.pending_len(),
        interrupt_count: controller.interrupt_len(),
        updated_at: chrono::Utc::now().to_rfc3339(),
    }
}

fn save_repl_snapshot(db: &Database, snapshot: ForegroundSnapshot) -> Result<(), CliError> {
    save_snapshot_for_db(db, &snapshot)
}

fn trim_line_endings(line: &str) -> &str {
    line.trim_end_matches(['\r', '\n'])
}

fn strip_multiline_continuation(line: &str) -> Option<&str> {
    let trimmed = trim_line_endings(line);
    trimmed
        .strip_suffix('\\')
        .map(|value| value.trim_end_matches([' ', '\t']))
        .filter(|value| !value.is_empty() || trimmed == "\\")
}

fn combine_submission_prompt(queued_prompts: &[String], prompt: &str) -> String {
    let current_prompt = prompt.trim();
    if queued_prompts.is_empty() {
        return current_prompt.to_string();
    }

    let mut sections = queued_prompts
        .iter()
        .enumerate()
        .map(|(index, queued_prompt)| {
            format!("[Queued prompt {}]\n{}", index + 1, queued_prompt.trim())
        })
        .collect::<Vec<_>>();
    sections.push(format!("[Primary prompt]\n{current_prompt}"));
    sections.join("\n\n")
}

fn input_kind_label(kind: ForegroundInputKind) -> &'static str {
    match kind {
        ForegroundInputKind::PlainText => "plain_text",
        ForegroundInputKind::SlashCommand => "slash_command",
    }
}

#[cfg(test)]
mod tests {
    use super::super::TEST_ENV_LOCK;
    use super::super::foreground_store::{clear_snapshot_for_db, load_snapshot_for_db};
    use super::*;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use uuid::Uuid;

    struct TempHome {
        root: PathBuf,
        previous_home: Option<OsString>,
        previous_xdg_data_home: Option<OsString>,
    }

    impl TempHome {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("hermes-repl-test-{}", Uuid::new_v4()));
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

    #[test]
    fn trim_line_endings_removes_unix_and_windows_newlines() {
        assert_eq!(trim_line_endings("hello\n"), "hello");
        assert_eq!(trim_line_endings("hello\r\n"), "hello");
    }

    #[test]
    fn strip_multiline_continuation_detects_trailing_backslash() {
        assert_eq!(
            strip_multiline_continuation("first line\\\n"),
            Some("first line")
        );
        assert_eq!(strip_multiline_continuation("plain line\n"), None);
    }

    #[test]
    fn configured_busy_input_mode_defaults_to_interrupt() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        assert_eq!(configured_busy_input_mode(), BusyInputMode::Interrupt);
    }

    #[test]
    fn configured_busy_input_mode_reads_queue_from_config() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let cfg = config::HermesConfig {
            busy_input_mode: "queue".to_string(),
            ..Default::default()
        };
        config::save_config(&cfg).expect("save config");

        assert_eq!(configured_busy_input_mode(), BusyInputMode::Queue);
    }

    #[test]
    fn prints_welcome_and_exits_on_exit_command() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let mut output = Vec::new();

        run(Cursor::new(b"exit\n".to_vec()), &mut output).expect("repl should exit cleanly");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            WELCOME_BANNER
        );
    }

    #[test]
    fn foreground_snapshot_persists_run_lifecycle_and_clears_on_exit() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let db = open_cli_database().expect("open cli database");
        let session_id = "foreground-test-session";
        let mut controller = ForegroundTurnController::new(BusyInputMode::Queue);
        let mut active_run = None;
        let mut pending_prompt = None;
        let mut output = Vec::new();
        let processor = Arc::new(move |prompt: String, cancel_flag: Arc<AtomicBool>| {
            if prompt == "first prompt" {
                while !cancel_flag.load(Ordering::SeqCst) {
                    thread::sleep(Duration::from_millis(10));
                }
            }
            Ok(format!("processed\t{prompt}\n"))
        });

        save_repl_snapshot(&db, build_snapshot(session_id, None, &controller))
            .expect("save initial snapshot");

        spawn_foreground_run(
            &mut controller,
            &mut active_run,
            "first prompt".to_string(),
            processor.clone(),
            &db,
            session_id,
        )
        .expect("spawn first run");

        let running_snapshot = load_snapshot_for_db(&db).expect("load running snapshot");
        assert!(running_snapshot.active);
        assert_eq!(running_snapshot.state, "running");
        assert_eq!(running_snapshot.session_id.as_deref(), Some(session_id));
        assert_eq!(running_snapshot.run_id.as_deref(), Some("1"));
        assert_eq!(running_snapshot.cancel_state.as_deref(), Some("active"));
        assert_eq!(running_snapshot.pending_count, 0);
        assert_eq!(running_snapshot.interrupt_count, 0);

        handle_busy_line(
            "queued follow-up",
            &mut pending_prompt,
            &mut controller,
            &mut active_run,
            &db,
            session_id,
            &mut output,
        )
        .expect("record queued follow-up");

        let queued_snapshot = load_snapshot_for_db(&db).expect("load queued snapshot");
        assert!(queued_snapshot.active);
        assert_eq!(queued_snapshot.state, "running");
        assert_eq!(queued_snapshot.session_id, running_snapshot.session_id);
        assert_eq!(queued_snapshot.run_id, running_snapshot.run_id);
        assert_eq!(queued_snapshot.cancel_state.as_deref(), Some("active"));
        assert_eq!(queued_snapshot.pending_count, 1);
        assert_eq!(queued_snapshot.interrupt_count, 0);

        handle_busy_line(
            "/stop",
            &mut pending_prompt,
            &mut controller,
            &mut active_run,
            &db,
            session_id,
            &mut output,
        )
        .expect("record stop request");

        let interrupting_snapshot = load_snapshot_for_db(&db).expect("load interrupting snapshot");
        assert!(interrupting_snapshot.active);
        assert_eq!(interrupting_snapshot.state, "interrupting");
        assert_eq!(
            interrupting_snapshot.session_id,
            running_snapshot.session_id
        );
        assert_eq!(interrupting_snapshot.run_id, running_snapshot.run_id);
        assert_eq!(
            interrupting_snapshot.cancel_state.as_deref(),
            Some("cancel_requested")
        );
        assert_eq!(interrupting_snapshot.pending_count, 1);
        assert_eq!(interrupting_snapshot.interrupt_count, 1);

        assert_eq!(queued_snapshot.session_id, running_snapshot.session_id);
        assert_eq!(
            interrupting_snapshot.session_id,
            running_snapshot.session_id
        );
        assert_eq!(queued_snapshot.run_id, running_snapshot.run_id);
        assert_eq!(interrupting_snapshot.run_id, running_snapshot.run_id);

        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        while std::time::Instant::now() < deadline {
            let _ = flush_completed_run(
                &db,
                session_id,
                &mut controller,
                &mut active_run,
                processor.clone(),
                &mut output,
            )
            .expect("flush completed run");

            if active_run.is_none()
                && controller.pending_len() == 0
                && controller.interrupt_len() == 0
            {
                break;
            }

            thread::sleep(Duration::from_millis(10));
        }

        assert!(
            active_run.is_none(),
            "expected no active run after draining follow-up work"
        );
        assert_eq!(controller.pending_len(), 0);
        assert_eq!(controller.interrupt_len(), 0);

        let idle_snapshot = load_snapshot_for_db(&db).expect("load idle snapshot");
        assert!(!idle_snapshot.active);
        assert_eq!(idle_snapshot.state, "idle");
        assert_eq!(idle_snapshot.session_id.as_deref(), Some(session_id));
        assert_eq!(idle_snapshot.run_id, None);
        assert_eq!(idle_snapshot.cancel_state, None);
        assert_eq!(idle_snapshot.pending_count, 0);
        assert_eq!(idle_snapshot.interrupt_count, 0);

        clear_snapshot_for_db(&db).expect("clear snapshot");

        let cleared_snapshot = load_snapshot_for_db(&db).expect("load cleared snapshot");
        assert!(!cleared_snapshot.active);
        assert_eq!(cleared_snapshot.state, "idle");
        assert_eq!(cleared_snapshot.session_id, None);
        assert_eq!(cleared_snapshot.run_id, None);
        assert_eq!(cleared_snapshot.cancel_state, None);
        assert_eq!(cleared_snapshot.pending_count, 0);
        assert_eq!(cleared_snapshot.interrupt_count, 0);

        let rendered = String::from_utf8(output).expect("output is utf8");
        assert!(rendered.contains("queue\tbusy\tkind=plain_text\ttext=queued follow-up\n"));
        assert!(rendered.contains("interrupt\trequested\tkind=slash_command\ttext=/stop\n"));
        assert!(rendered.contains("processed\tfirst prompt\n"));
    }

    #[test]
    fn keeps_prompting_until_exit_is_entered() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let mut output = Vec::new();

        run(Cursor::new(b"hello\nexit\n".to_vec()), &mut output).expect("repl should exit cleanly");

        let rendered = String::from_utf8(output).expect("output is utf8");
        assert!(rendered.contains("background\tqueued\tmission_id="));
        assert!(rendered.contains("\tprompt=hello\n"));
        assert!(rendered.starts_with("Hermes operator REPL\n"));
    }

    #[test]
    fn multiline_prompt_uses_continuation_prompt_and_submits_joined_text() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let mut output = Vec::new();

        run(
            Cursor::new(b"draft roadmap\\\nwith milestones\nexit\n".to_vec()),
            &mut output,
        )
        .expect("repl should exit cleanly");

        let rendered = String::from_utf8(output).expect("output is utf8");
        assert!(rendered.contains(CONTINUATION_PROMPT));
        assert!(rendered.contains("draft roadmap"));
        assert!(rendered.contains("with milestones"));
        assert!(rendered.contains("background\tqueued\tmission_id="));
    }

    #[test]
    fn queued_prompts_are_consumed_with_next_plain_input() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let mut output = Vec::new();

        run(
            Cursor::new(
                b"/queue first queued prompt\nlaunch current prompt\n/queue status\nexit\n"
                    .to_vec(),
            ),
            &mut output,
        )
        .expect("repl should exit cleanly");

        let rendered = String::from_utf8(output).expect("output is utf8");
        assert!(rendered.contains("queue\tqueued\tcount=1\tprompt=first queued prompt\n"));
        assert!(rendered.contains("background\tqueued\tmission_id="));
        assert!(rendered.contains(
            "\tprompt=[Queued prompt 1]\nfirst queued prompt\n\n[Primary prompt]\nlaunch current prompt\n"
        ));
        assert!(rendered.contains("queue\tcount=0\n"));
    }

    #[test]
    fn queue_mode_routes_busy_plain_text_to_pending_follow_up_submission() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_processor = calls.clone();
        let mut output = Vec::new();
        run_with_processor(
            Cursor::new(b"first prompt\nsecond prompt\nexit\n".to_vec()),
            &mut output,
            BusyInputMode::Queue,
            move |prompt, _cancel_flag| {
                let current = calls_for_processor.fetch_add(1, Ordering::SeqCst);
                if current == 0 {
                    thread::sleep(Duration::from_millis(80));
                }
                Ok(format!("processed\t{}\n", prompt))
            },
        )
        .expect("repl should exit cleanly");

        let rendered = String::from_utf8(output).expect("output is utf8");
        assert!(rendered.contains("queue\tbusy\tkind=plain_text\ttext=second prompt\n"));
        assert!(rendered.contains(QUEUE_ORDER_NOTE));
        assert!(rendered.contains("processed\tfirst prompt\n"));
        assert!(rendered.contains("processed\tsecond prompt\n"));
    }

    #[test]
    fn interrupt_mode_records_busy_plain_text_for_follow_up_submission() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_processor = calls.clone();
        let mut output = Vec::new();
        run_with_processor(
            Cursor::new(b"first prompt\ninterrupt me please\nexit\n".to_vec()),
            &mut output,
            BusyInputMode::Interrupt,
            move |prompt, _cancel_flag| {
                let current = calls_for_processor.fetch_add(1, Ordering::SeqCst);
                if current == 0 {
                    thread::sleep(Duration::from_millis(80));
                }
                Ok(format!("processed\t{}\n", prompt))
            },
        )
        .expect("repl should exit cleanly");

        let rendered = String::from_utf8(output).expect("output is utf8");
        assert!(rendered.contains("processed\tfirst prompt\n"));
        assert!(rendered.contains("processed\tinterrupt me please\n"));
        if rendered.contains("interrupt\trequested\tkind=plain_text\ttext=interrupt me please\n") {
            assert!(rendered.contains("interrupt\tcontinuation\ttext=interrupt me please\n"));
            assert!(rendered.contains(INTERRUPT_ORDER_NOTE));
        }
    }

    #[test]
    fn interrupt_mode_preserves_busy_plain_text_arrival_order() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_processor = calls.clone();
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let prompts_for_processor = prompts.clone();
        let mut output = Vec::new();

        run_with_processor(
            Cursor::new(b"first prompt\nsecond prompt\nthird prompt\nexit\n".to_vec()),
            &mut output,
            BusyInputMode::Interrupt,
            move |prompt, _cancel_flag| {
                prompts_for_processor
                    .lock()
                    .expect("lock prompts")
                    .push(prompt.clone());
                let current = calls_for_processor.fetch_add(1, Ordering::SeqCst);
                if current == 0 {
                    thread::sleep(Duration::from_millis(80));
                }
                Ok(format!("processed\t{}\n", prompt))
            },
        )
        .expect("repl should exit cleanly");

        let rendered = String::from_utf8(output).expect("output is utf8");
        if rendered.contains("interrupt\trequested\tkind=plain_text\ttext=second prompt\n") {
            assert!(rendered.contains(INTERRUPT_ORDER_NOTE));
        }
        let recorded = prompts.lock().expect("lock prompts").clone();
        assert_eq!(recorded.first().map(String::as_str), Some("first prompt"));
        if recorded.len() == 2 {
            assert_eq!(recorded[1].as_str(), "second prompt\nthird prompt");
        } else if recorded.len() == 3 {
            assert_eq!(
                recorded.as_slice(),
                ["first prompt", "second prompt", "third prompt"]
            );
        } else {
            panic!("unexpected prompt sequence: {:?}", recorded);
        }
    }

    #[test]
    fn busy_interrupt_slash_turns_into_follow_up_prompt_text() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_processor = calls.clone();
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let prompts_for_processor = prompts.clone();
        let mut output = Vec::new();

        run_with_processor(
            Cursor::new(
                b"first prompt\n/interrupt revise with bullets\nfinal detail\nexit\n".to_vec(),
            ),
            &mut output,
            BusyInputMode::Queue,
            move |prompt, _cancel_flag| {
                prompts_for_processor
                    .lock()
                    .expect("lock prompts")
                    .push(prompt.clone());
                let current = calls_for_processor.fetch_add(1, Ordering::SeqCst);
                if current == 0 {
                    thread::sleep(Duration::from_millis(80));
                }
                Ok(format!("processed\t{}\n", prompt))
            },
        )
        .expect("repl should exit cleanly");

        let rendered = String::from_utf8(output).expect("output is utf8");
        assert!(
            rendered.contains(
                "interrupt\trequested\tkind=slash_command\ttext=/interrupt revise with bullets\n"
            ),
            "rendered output:\n{rendered}"
        );
        assert!(rendered.contains("interrupt\tcontinuation\ttext=revise with bullets\n"));
        assert!(rendered.contains(INTERRUPT_ORDER_NOTE));
        if rendered.contains("queue\tbusy\tkind=plain_text\ttext=final detail\n") {
            assert_eq!(
                prompts.lock().expect("lock prompts").as_slice(),
                ["first prompt", "revise with bullets", "final detail"]
            );
        } else {
            let recorded = prompts.lock().expect("lock prompts").clone();
            assert_eq!(
                recorded.as_slice(),
                ["first prompt", "revise with bullets", "final detail"]
            );
        }
    }

    #[test]
    fn busy_stop_slash_without_payload_primes_next_foreground_turn() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_processor = calls.clone();
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let prompts_for_processor = prompts.clone();
        let mut output = Vec::new();

        run_with_processor(
            Cursor::new(b"first prompt\n/stop\nexit\n".to_vec()),
            &mut output,
            BusyInputMode::Queue,
            move |prompt, _cancel_flag| {
                prompts_for_processor
                    .lock()
                    .expect("lock prompts")
                    .push(prompt.clone());
                let current = calls_for_processor.fetch_add(1, Ordering::SeqCst);
                if current == 0 {
                    thread::sleep(Duration::from_millis(80));
                }
                Ok(format!("processed\t{}\n", prompt))
            },
        )
        .expect("repl should exit cleanly");

        let rendered = String::from_utf8(output).expect("output is utf8");
        assert!(rendered.contains("interrupt\trequested\tkind=slash_command\ttext=/stop\n"));
        assert!(rendered.contains(
            format!("interrupt\tcontinuation\ttext={DEFAULT_STOP_FOLLOW_UP}\n").as_str()
        ));
        assert_eq!(
            prompts.lock().expect("lock prompts").as_slice(),
            [
                "first prompt",
                "Please stop the current response and wait for the next user instruction."
            ]
        );
    }

    #[test]
    fn idle_slash_input_still_uses_normal_slash_handler() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let processor_calls = Arc::new(AtomicUsize::new(0));
        let processor_calls_for_handler = processor_calls.clone();
        let mut output = Vec::new();

        run_with_processor(
            Cursor::new(b"/help\nexit\n".to_vec()),
            &mut output,
            BusyInputMode::Interrupt,
            move |prompt, _cancel_flag| {
                processor_calls_for_handler.fetch_add(1, Ordering::SeqCst);
                Ok(format!("processed\t{}\n", prompt))
            },
        )
        .expect("repl should exit cleanly");

        let rendered = String::from_utf8(output).expect("output is utf8");
        assert_eq!(processor_calls.load(Ordering::SeqCst), 0);
        assert!(
            rendered.contains("/help\tshow slash command index and current CLI/TUI parity notes\n")
        );
    }

    #[test]
    fn interrupt_mode_sets_cancel_flag_for_inflight_processor() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let mut output = Vec::new();

        run_with_processor(
            Cursor::new(b"first prompt\nsecond prompt\nexit\n".to_vec()),
            &mut output,
            BusyInputMode::Interrupt,
            move |prompt, cancel_flag| {
                for _ in 0..20 {
                    if cancel_flag.load(Ordering::SeqCst) {
                        return Ok(format!("interrupted\t{}\n", prompt));
                    }
                    thread::sleep(Duration::from_millis(10));
                }
                Ok(format!("processed\t{}\n", prompt))
            },
        )
        .expect("repl should exit cleanly");

        let rendered = String::from_utf8(output).expect("output is utf8");
        assert!(
            rendered.contains("interrupt\trequested\tkind=plain_text\ttext=second prompt\n"),
            "rendered output:\n{rendered}"
        );
        assert!(rendered.contains("interrupt\tcontinuation\ttext=second prompt\n"));
        assert!(rendered.contains("interrupted\tfirst prompt\n"));
        assert!(rendered.contains("processed\tsecond prompt\n"));
    }

    #[test]
    fn interrupt_continuation_runs_before_earlier_queued_follow_up_input() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_processor = calls.clone();
        let prompts = Arc::new(Mutex::new(Vec::new()));
        let prompts_for_processor = prompts.clone();
        let mut output = Vec::new();

        run_with_processor(
            Cursor::new(
                b"first prompt\nqueued follow-up\n/interrupt revise with bullets\nexit\n".to_vec(),
            ),
            &mut output,
            BusyInputMode::Queue,
            move |prompt, _cancel_flag| {
                prompts_for_processor
                    .lock()
                    .expect("lock prompts")
                    .push(prompt.clone());
                let current = calls_for_processor.fetch_add(1, Ordering::SeqCst);
                if current == 0 {
                    thread::sleep(Duration::from_millis(80));
                }
                Ok(format!("processed\t{}\n", prompt))
            },
        )
        .expect("repl should exit cleanly");

        let rendered = String::from_utf8(output).expect("output is utf8");
        if rendered.contains("interrupt\tcontinuation\ttext=revise with bullets\n") {
            assert!(
                rendered.contains("queue\tbusy\tkind=plain_text\ttext=queued follow-up\n"),
                "rendered output:\n{rendered}"
            );
            assert_eq!(
                prompts.lock().expect("lock prompts").as_slice(),
                ["first prompt", "revise with bullets", "queued follow-up"]
            );
        } else {
            assert!(
                rendered.contains(
                    "interrupt\talias=/interrupt\tstatus=idle\tfollow_up=revise with bullets\n"
                ),
                "rendered output:\n{rendered}"
            );
            assert_eq!(
                prompts.lock().expect("lock prompts").as_slice(),
                ["first prompt", "queued follow-up"]
            );
        }
    }

    #[test]
    fn take_next_follow_up_work_item_prioritizes_interrupt_continuation_before_pending_queue() {
        let mut controller = ForegroundTurnController::new(BusyInputMode::Queue);
        controller.start_run().expect("run starts");
        controller
            .submit_text_while_busy("queued follow-up")
            .expect("queue plain follow-up");
        controller
            .submit_slash_while_busy("/stop")
            .expect("record interrupt follow-up");
        controller.finish_run(RunId(1)).expect("finish initial run");

        assert!(matches!(
            take_next_follow_up_work_item(&mut controller),
            Some(FollowUpWorkItem::ForegroundPrompt(ref prompt))
                if prompt == DEFAULT_STOP_FOLLOW_UP
        ));
        assert!(matches!(
            take_next_follow_up_work_item(&mut controller),
            Some(FollowUpWorkItem::ForegroundPrompt(ref prompt))
                if prompt == "queued follow-up"
        ));
        assert!(take_next_follow_up_work_item(&mut controller).is_none());
    }

    #[test]
    fn adapt_slash_output_for_repl_rewrites_stale_help_note() {
        let original = format!("/help\t...\n{STALE_HELP_NOTE}");
        assert_eq!(
            adapt_slash_output_for_repl(&original),
            format!("/help\t...\n{REPL_HELP_NOTE}")
        );
    }

    #[test]
    fn adapt_slash_output_for_repl_rewrites_busy_alias_help_note() {
        let original = format!("/help\t...\n{BUSY_ALIAS_HELP_NOTE}");
        assert_eq!(
            adapt_slash_output_for_repl(&original),
            format!("/help\t...\n{REPL_HELP_NOTE}")
        );
    }

    #[test]
    fn renders_help_for_slash_help_input() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let mut output = Vec::new();
        run(Cursor::new(b"/help\nexit\n".to_vec()), &mut output).expect("repl should exit cleanly");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            concat!(
                "Hermes operator REPL\n",
                "Type `exit` to quit, `/help` for commands.\n",
                "Plain prompts queue background missions. Use `/queue` to append prompts to the next plain submission. Multi-line input uses trailing \\\\; busy /interrupt, /cancel, and /stop request cancellation and can seed the next continuation.\n",
                "> ",
                "/help\tshow slash command index and current CLI/TUI parity notes\n",
                "/model [provider:model|provider model|model]\tshow or persist the current provider/model selection\n",
                "/busy [queue|interrupt|status]\tshow or persist how busy plain-text input is routed\n",
                "/interrupt [follow-up prompt]\trequest cancellation/interrupt when a foreground turn is busy; explain idle behavior otherwise\n",
                "/cancel [follow-up prompt]\talias of /interrupt for busy foreground turns; explain idle behavior otherwise\n",
                "/stop [follow-up prompt]\talias of /interrupt for busy foreground turns; explain idle behavior otherwise\n",
                "/tools\tlist discovered tool surfaces and availability hints\n",
                "/skills [list|search|view|install|enable|disable]\tlist, inspect, install, toggle, or invoke discovered skills as /<skill>\n",
                "/title [new title]\tshow or rename the latest session title\n",
                "/background <prompt> | status | latest\tqueue a prompt as a background mission or inspect queued background runs\n",
                "/queue <prompt> | status\tqueue a prompt for the next plain-text submission without interrupting the current turn\n",
                "/usage\tshow active mission / approval / recent session summary\n",
                "/voice [on|off|status|transcribe|speak]\tshow, toggle, transcribe, or queue local voice workflow events\n",
                "/continue\tload the latest session row for continuation flows\n",
                "/resume <session-id|title>\tresume by exact id or recent title match\n",
                "/sessions [latest|continue|resume|title|search|active|history|replay]\tshow session lifecycle commands and examples\n",
                "/missions [list|get|plan|status]\tlist missions, inspect one mission, generate a plan, or update mission status\n",
                "note\tplain prompts queue background missions; /queue stores prompts for the next plain submission; use trailing \\ for multi-line input; busy /interrupt, /cancel, and /stop request cancellation and can seed the next continuation\n",
                "> "
            )
        );
    }

    #[test]
    fn renders_model_message_for_slash_model_input() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let mut output = Vec::new();

        run(Cursor::new(b"/model\nexit\n".to_vec()), &mut output)
            .expect("repl should exit cleanly");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            concat!(
                "Hermes operator REPL\n",
                "Type `exit` to quit, `/help` for commands.\n",
                "Plain prompts queue background missions. Use `/queue` to append prompts to the next plain submission. Multi-line input uses trailing \\\\; busy /interrupt, /cancel, and /stop request cancellation and can seed the next continuation.\n",
                "> ",
                "model\tprovider=openai\tmodel=gpt-4o\tprofile=default\n",
                "model\thint\t/model openai gpt-4o | /model openrouter claude-sonnet-4\n",
                "> "
            )
        );
    }

    #[test]
    fn renders_resumed_session_for_slash_resume_input() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let mut output = Vec::new();

        run(
            Cursor::new(b"/resume session-001\nexit\n".to_vec()),
            &mut output,
        )
        .expect("repl should exit cleanly");

        assert_eq!(
            String::from_utf8(output).expect("output is utf8"),
            concat!(
                "Hermes operator REPL\n",
                "Type `exit` to quit, `/help` for commands.\n",
                "Plain prompts queue background missions. Use `/queue` to append prompts to the next plain submission. Multi-line input uses trailing \\\\; busy /interrupt, /cancel, and /stop request cancellation and can seed the next continuation.\n",
                "> ",
                "session not found\n",
                "> "
            )
        );
    }
}
