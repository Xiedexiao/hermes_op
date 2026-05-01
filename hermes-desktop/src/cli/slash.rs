use super::foreground_store::load_snapshot_for_db;
use super::{CliError, missions, sessions};
use chrono::Utc;
use hermes_desktop::backend::{
    CreateMissionInput, Database, MissionPriority, MissionService, MissionServiceImpl,
    MissionStatus, config, create_app_state,
};
use hermes_desktop::commands::mission::mission_generate_plan_for_db;
use hermes_desktop::commands::skills::{
    SkillDetailItem, SkillInstallRequest, SkillListItem, SkillSetEnabledRequest,
    skills_install_for_db, skills_search_for_db, skills_set_enabled_for_db, skills_view_for_db,
};
use hermes_desktop::commands::timeline::record_run_event;
use hermes_desktop::commands::voice::{
    VoiceSetEnabledRequest, VoiceSpeakRequest, VoiceSummary, VoiceTranscribeRequest,
    voice_set_enabled_for_db, voice_speak_for_db, voice_summary_for_db, voice_transcribe_for_db,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[path = "queue.rs"]
mod queue;

#[derive(Debug, Clone, PartialEq, Eq)]
struct UsageSnapshot {
    active_mission_count: i64,
    pending_approval_count: i64,
    recent_session_count: i64,
    has_recent_session: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeSettingsSnapshot {
    provider: String,
    model: String,
    engine_profile: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ToolDescriptor {
    name: String,
    scope: String,
    description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillSummary {
    name: String,
    source: String,
    path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionRow {
    id: String,
    title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BackgroundRunItem {
    mission_id: String,
    mission_title: String,
    mission_status: String,
    run_id: String,
    run_status: String,
    prompt: String,
    step_count: i64,
    pending_step_count: i64,
    awaiting_approval_step_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedRuntimeSettings {
    #[serde(default)]
    provider: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key_ref: Option<String>,
    #[serde(default)]
    engine_profile: Option<String>,
    #[serde(default)]
    agent_engine_enabled: Option<bool>,
}

#[cfg(test)]
pub fn render_help() -> String {
    render_help_config(true, true)
}

fn render_help_config(include_interrupt_aliases: bool, include_foreground_status: bool) -> String {
    [
        help_row(
            "/help",
            "show slash command index and current CLI/TUI parity notes",
        ),
        help_row(
            "/model [provider:model|provider model|model]",
            "show or persist the current provider/model selection",
        ),
        help_row(
            "/busy [queue|interrupt|status]",
            "show or persist how busy plain-text input is routed",
        ),
        if include_interrupt_aliases {
            help_row(
                "/interrupt [follow-up prompt]",
                "request cancellation/interrupt when a foreground turn is busy; explain idle behavior otherwise",
            )
        } else {
            String::new()
        },
        if include_interrupt_aliases {
            help_row(
                "/cancel [follow-up prompt]",
                "alias of /interrupt for busy foreground turns; explain idle behavior otherwise",
            )
        } else {
            String::new()
        },
        if include_interrupt_aliases {
            help_row(
                "/stop [follow-up prompt]",
                "alias of /interrupt for busy foreground turns; explain idle behavior otherwise",
            )
        } else {
            String::new()
        },
        help_row(
            "/tools",
            "list discovered tool surfaces and availability hints",
        ),
        help_row(
            "/skills [list|search|view|install|enable|disable]",
            "list, inspect, install, toggle, or invoke discovered skills as /<skill>",
        ),
        help_row(
            "/title [new title]",
            "show or rename the latest session title",
        ),
        if include_foreground_status {
            help_row(
                "/foreground status",
                "show persisted foreground executor snapshot from the CLI store",
            )
        } else {
            String::new()
        },
        help_row(
            "/background <prompt> | status | latest",
            "queue a prompt as a background mission or inspect queued background runs",
        ),
        help_row(
            "/queue <prompt> | status",
            "queue a prompt for the next plain-text submission without interrupting the current turn",
        ),
        help_row(
            "/usage",
            "show active mission / approval / recent session summary",
        ),
        help_row(
            "/voice [on|off|status|transcribe|speak]",
            "show, toggle, transcribe, or queue local voice workflow events",
        ),
        help_row(
            "/continue",
            "load the latest session row for continuation flows",
        ),
        help_row(
            "/resume <session-id|title>",
            "resume by exact id or recent title match",
        ),
        help_row(
            "/sessions [latest|continue|resume|title|search|active|history|replay]",
            "show session lifecycle commands and examples",
        ),
        help_row(
            "/missions [list|get|plan|status]",
            "list missions, inspect one mission, generate a plan, or update mission status",
        ),
        help_row(
            "note",
            if include_interrupt_aliases {
                "plain prompts queue background missions; /queue stores prompts for the next plain submission; use trailing \\ for multi-line input; busy REPL interrupt aliases are consumed before slash handling"
            } else {
                "plain prompts queue background missions; /queue stores prompts for the next plain submission; use trailing \\ for multi-line input; interrupt is not wired yet"
            },
        ),
    ]
    .join("")
}

pub(crate) fn handle_from_cli(command: &str) -> Result<String, CliError> {
    let trimmed = command.trim();
    if trimmed == "/voice" || trimmed.starts_with("/voice ") {
        return handle_voice_command(trimmed);
    }
    if trimmed == "/skills" || trimmed.starts_with("/skills ") {
        return handle_skills_command(trimmed);
    }
    if trimmed == "/missions" || trimmed.starts_with("/missions ") {
        return handle_missions_command(trimmed);
    }
    if trimmed == "/foreground" || trimmed.starts_with("/foreground ") {
        return handle_foreground_command(trimmed);
    }
    if trimmed == "/background" || trimmed.starts_with("/background ") {
        return handle_background_command(trimmed);
    }
    if trimmed == "/queue" || trimmed.starts_with("/queue ") {
        return queue::handle_command(trimmed);
    }
    let first_token = trimmed.split_whitespace().next().unwrap_or_default();
    if !supported_commands().contains(&first_token)
        && let Some(rendered) = handle_dynamic_skill_command(trimmed)?
    {
        return Ok(rendered);
    }

    handle_with_help(
        command,
        true,
        true,
        sessions::get_latest_session,
        sessions::get_session,
        sessions::load_sessions,
        load_usage_snapshot,
        load_runtime_settings,
        persist_runtime_settings,
        load_tool_descriptors,
        load_skill_summaries,
        sessions::rename_session,
    )
}

pub fn handle(command: &str) -> Result<String, CliError> {
    let trimmed = command.trim();
    if trimmed == "/voice" || trimmed.starts_with("/voice ") {
        return handle_voice_command(trimmed);
    }
    if trimmed == "/skills" || trimmed.starts_with("/skills ") {
        return handle_skills_command(trimmed);
    }
    if trimmed == "/missions" || trimmed.starts_with("/missions ") {
        return handle_missions_command(trimmed);
    }
    if trimmed == "/foreground" || trimmed.starts_with("/foreground ") {
        return handle_foreground_command(trimmed);
    }
    if trimmed == "/background" || trimmed.starts_with("/background ") {
        return handle_background_command(trimmed);
    }
    if trimmed == "/queue" || trimmed.starts_with("/queue ") {
        return queue::handle_command(trimmed);
    }
    let first_token = trimmed.split_whitespace().next().unwrap_or_default();
    if !supported_commands().contains(&first_token)
        && let Some(rendered) = handle_dynamic_skill_command(trimmed)?
    {
        return Ok(rendered);
    }

    handle_with(
        command,
        sessions::get_latest_session,
        sessions::get_session,
        sessions::load_sessions,
        load_usage_snapshot,
        load_runtime_settings,
        persist_runtime_settings,
        load_tool_descriptors,
        load_skill_summaries,
        sessions::rename_session,
    )
}

fn help_row(command: &str, description: &str) -> String {
    format!("{command}\t{description}\n")
}

fn message(value: &str) -> String {
    format!("{value}\n")
}

fn render_usage(snapshot: &UsageSnapshot) -> String {
    format!(
        "usage\tactive_missions={}\tpending_approvals={}\trecent_sessions={}\thas_recent_session={}\n",
        snapshot.active_mission_count,
        snapshot.pending_approval_count,
        snapshot.recent_session_count,
        snapshot.has_recent_session,
    ) + "usage\thint\t/sessions latest | /sessions search <query> | /missions\n"
}

fn render_voice(enabled: bool) -> String {
    format!(
        "voice\tenabled={enabled}\tstt=local-text-capture\ttts=local-speak-queue\ttranscripts=0\tqueued=0\n"
    ) + "voice\tnote\tlocal transcription and queue state are persisted; audio capture/playback is still local-only.\n"
}

fn render_model(snapshot: &RuntimeSettingsSnapshot) -> String {
    format!(
        "model\tprovider={}\tmodel={}\tprofile={}\n",
        snapshot.provider,
        snapshot.model,
        snapshot.engine_profile.as_deref().unwrap_or("-"),
    ) + "model\thint\t/model openai gpt-4o | /model openrouter claude-sonnet-4\n"
}

fn render_busy_input_mode(mode: &str) -> String {
    let note = match mode {
        "queue" => "busy plain-text input will wait for the next foreground turn",
        _ => {
            "busy plain-text input will request interrupt continuation for the next foreground turn"
        }
    };
    format!("busy_input_mode\tmode={}\n", mode) + &format!("busy_input_mode\tnote\t{}\n", note)
}

fn render_tools(tools: &[ToolDescriptor]) -> String {
    if tools.is_empty() {
        return "no tools found\n".to_string();
    }

    format!("tools\tcount={}\n", tools.len())
        + &tools
            .iter()
            .map(|tool| format!("{}\t{}\t{}\n", tool.name, tool.scope, tool.description))
            .collect::<String>()
        + "tools\tnote\ttool execution help is descriptive only in this CLI lane\n"
}

fn render_skills(skills: &[SkillSummary]) -> String {
    if skills.is_empty() {
        return "no skills found\n".to_string();
    }

    format!("skills\tcount={}\n", skills.len())
        + &skills
            .iter()
            .map(|skill| format!("{}\t{}\t{}\n", skill.name, skill.source, skill.path))
            .collect::<String>()
        + "skills\tnote\tdiscovery is live; use /skills search <query> or /skills view <name> for deeper inspection\n"
}

fn render_skill_invocation(
    detail: &SkillDetailItem,
    command_key: &str,
    instruction: Option<&str>,
) -> String {
    let mut output = format!(
        "skill\tcommand={}\tname={}\tsource={}\tpath={}\n",
        command_key, detail.name, detail.source, detail.path
    );
    if let Some(value) = instruction {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            output.push_str(&format!("skill\tinstruction\t{}\n", trimmed));
        }
    }
    output.push_str(&format!(
        "[SYSTEM: The user has invoked the \"{}\" skill via {}. Follow its instructions below.]\n\n{}\n",
        detail.display_name,
        command_key,
        detail.content.trim_end(),
    ));
    output
}

fn render_title(session: &SessionRow) -> String {
    format!("title\t{}\t{}\n", session.id, session.title)
}

fn render_sessions_preview() -> String {
    concat!(
        "sessions\tlatest\thermes-operator-cli sessions latest | /continue\n",
        "sessions\tcontinue\thermes-operator-cli sessions continue | /continue\n",
        "sessions\tresume\thermes-operator-cli sessions resume <session-id|title> | /resume <session-id|title>\n",
        "sessions\ttitle\thermes-operator-cli sessions title [new title] | /title [new title]\n",
        "sessions\tsearch\thermes-operator-cli sessions search <query> | /sessions search <query>\n",
        "sessions\tactive\thermes-operator-cli sessions active | /sessions active\n",
        "sessions\tactive-clear\thermes-operator-cli sessions active clear | /sessions active clear\n",
        "sessions\thistory\thermes-operator-cli sessions history <session-id|active|latest> | /sessions history <session-id|active|latest>\n",
        "sessions\treplay\thermes-operator-cli sessions replay <session-id|active|latest> | /sessions replay <session-id|active|latest>\n",
    )
    .to_string()
}

fn render_missions_preview() -> String {
    "missions\tlist\thermes-operator-cli missions list | /missions\n".to_string()
}

fn handle_sessions_active_command(command: &str) -> Result<String, CliError> {
    match command.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["/sessions", "active"] => sessions::get_active_session(),
        ["/sessions", "active", "clear"] => {
            sessions::clear_active_session()?;
            Ok("sessions\tactive\tcleared=true\n".to_string())
        }
        _ => Err(CliError::InvalidUsage(
            "usage: /sessions active [clear]\n".to_string(),
        )),
    }
}

fn open_app_database() -> Result<Database, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))
}

fn render_voice_summary(summary: &VoiceSummary) -> String {
    format!(
        "voice\tenabled={}\tstt={}\ttts={}\ttranscripts={}\tqueued={}\n",
        summary.enabled,
        summary.stt_provider,
        summary.tts_provider,
        summary.transcription_count,
        summary.queued_speak_count,
    ) + "voice\tnote\tlocal transcription and queue state are persisted; audio capture/playback is still local-only.\n"
}

fn render_background_usage() -> String {
    "usage: /background <prompt> | /background status | /background latest\n".to_string()
}

fn render_foreground_usage() -> String {
    "usage: /foreground status\n".to_string()
}

fn render_queue_usage() -> String {
    "usage: /queue <prompt> | /queue status\n".to_string()
}

fn render_foreground_status() -> String {
    match open_app_database()
        .and_then(|db| load_snapshot_for_db(&db))
        .map(|snapshot| {
            format!(
                concat!(
                    "foreground\tstatus\tstate={}\tactive={}\tsession_id={}\trun_id={}\t",
                    "cancel_state={}\tpending={}\tinterrupts={}\tupdated_at={}\n",
                    "foreground\tnote\tsnapshot_source=cli_foreground_store\tfreshness=persisted\t",
                    "values reflect the latest saved foreground status snapshot.\n",
                ),
                snapshot.state,
                snapshot.active,
                snapshot.session_id.as_deref().unwrap_or("-"),
                snapshot.run_id.as_deref().unwrap_or("-"),
                snapshot.cancel_state.as_deref().unwrap_or("-"),
                snapshot.pending_count,
                snapshot.interrupt_count,
                snapshot.updated_at,
            )
        }) {
        Ok(output) => output,
        Err(error) => format!("foreground\terror\tfailed_to_load_status={}\n", error),
    }
}

fn render_background_status(runs: &[BackgroundRunItem]) -> String {
    if runs.is_empty() {
        return "background\tcount=0\n".to_string();
    }

    let mut output = format!("background\tcount={}\n", runs.len());
    for item in runs {
        output.push_str(&format!(
            "{}\t{}\t{}\tmission_status={}\trun_status={}\tsteps={}\tpending={}\tawaiting_approval={}\tprompt={}\n",
            item.mission_id,
            item.mission_title,
            item.run_id,
            item.mission_status,
            item.run_status,
            item.step_count,
            item.pending_step_count,
            item.awaiting_approval_step_count,
            item.prompt,
        ));
    }
    output
}

fn render_background_latest(item: &BackgroundRunItem) -> String {
    format!(
        "background\tlatest\tmission_id={}\ttitle={}\trun_id={}\tmission_status={}\trun_status={}\tsteps={}\tpending={}\tawaiting_approval={}\tprompt={}\n",
        item.mission_id,
        item.mission_title,
        item.run_id,
        item.mission_status,
        item.run_status,
        item.step_count,
        item.pending_step_count,
        item.awaiting_approval_step_count,
        item.prompt,
    )
}

fn render_skill_result(skills: &[SkillListItem], note: &str) -> String {
    if skills.is_empty() {
        return "no skills found\n".to_string();
    }

    format!("skills\tcount={}\n", skills.len())
        + &skills
            .iter()
            .map(|skill| {
                format!(
                    "{}\t{}\t{}\tenabled={}\n",
                    skill.name, skill.source, skill.path, skill.enabled
                )
            })
            .collect::<String>()
        + &format!("skills\tnote\t{note}\n")
}

fn render_skill_detail(detail: &SkillDetailItem) -> String {
    let description = detail.description.as_deref().unwrap_or("-");
    format!(
        "skills\tdetail\tname={}\tsource={}\tenabled={}\tpath={}\nskills\tdescription\t{}\n{}\n",
        detail.name,
        detail.source,
        detail.enabled,
        detail.path,
        description,
        detail.content.trim_end(),
    )
}

fn handle_voice_command(command: &str) -> Result<String, CliError> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let db = open_app_database()?;

    match parts.as_slice() {
        ["/voice"] | ["/voice", "status"] => Ok(render_voice_summary(
            &voice_summary_for_db(&db).map_err(app_to_cli)?,
        )),
        ["/voice", "on"] => {
            voice_set_enabled_for_db(&db, VoiceSetEnabledRequest { enabled: true })
                .map_err(app_to_cli)?;
            Ok(render_voice_summary(
                &voice_summary_for_db(&db).map_err(app_to_cli)?,
            ))
        }
        ["/voice", "off"] => {
            voice_set_enabled_for_db(&db, VoiceSetEnabledRequest { enabled: false })
                .map_err(app_to_cli)?;
            Ok(render_voice_summary(
                &voice_summary_for_db(&db).map_err(app_to_cli)?,
            ))
        }
        ["/voice", "transcribe", text @ ..] => {
            let transcript = text.join(" ");
            if transcript.trim().is_empty() {
                return Err(CliError::InvalidUsage(
                    "usage: /voice transcribe <text>\n".to_string(),
                ));
            }
            let result = voice_transcribe_for_db(
                &db,
                VoiceTranscribeRequest {
                    text: transcript,
                    source: Some("cli".to_string()),
                    language: None,
                    auto_queue_for_speech: None,
                },
            )
            .map_err(app_to_cli)?;
            Ok(format!(
                "voice\ttranscript\tprovider={}\ttext={}\n",
                result.provider, result.normalized_transcript
            ))
        }
        ["/voice", "speak", text @ ..] => {
            let text = text.join(" ");
            if text.trim().is_empty() {
                return Err(CliError::InvalidUsage(
                    "usage: /voice speak <text>\n".to_string(),
                ));
            }
            let result = voice_speak_for_db(
                &db,
                VoiceSpeakRequest {
                    text,
                    voice: None,
                    origin: Some("cli".to_string()),
                },
            )
            .map_err(app_to_cli)?;
            Ok(format!(
                "voice\tspeak\tqueued={}\tprovider={}\ttext={}\n",
                result.queued, result.provider, result.text
            ))
        }
        _ => Err(CliError::InvalidUsage(
            "usage: /voice [on|off|status|transcribe|speak]\n".to_string(),
        )),
    }
}

fn handle_foreground_command(command: &str) -> Result<String, CliError> {
    match command.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["/foreground", "status"] => Ok(render_foreground_status()),
        _ => Err(CliError::InvalidUsage(render_foreground_usage())),
    }
}

fn handle_skills_command(command: &str) -> Result<String, CliError> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    let db = open_app_database()?;

    match parts.as_slice() {
        ["/skills"] | ["/skills", "list"] => Ok(render_skill_result(
            &skills_search_for_db(&db, "".to_string(), Some(100)).map_err(app_to_cli)?,
            "discovered skills",
        )),
        ["/skills", "search", query @ ..] => {
            let query = query.join(" ");
            if query.trim().is_empty() {
                return Err(CliError::InvalidUsage(
                    "usage: /skills search <query>\n".to_string(),
                ));
            }
            Ok(render_skill_result(
                &skills_search_for_db(&db, query, Some(100)).map_err(app_to_cli)?,
                "filtered skill results",
            ))
        }
        ["/skills", "view", name] => Ok(render_skill_detail(
            &skills_view_for_db(&db, (*name).to_string()).map_err(app_to_cli)?,
        )),
        ["/skills", "install", name] => {
            let installed = skills_install_for_db(
                &db,
                SkillInstallRequest {
                    name: (*name).to_string(),
                    title: None,
                    description: None,
                    content: None,
                    force: false,
                },
            )
            .map_err(app_to_cli)?;
            Ok(format!(
                "skills\tinstalled\t{}\t{}\n",
                installed.name, installed.path
            ))
        }
        ["/skills", "enable", name] => {
            skills_set_enabled_for_db(
                &db,
                SkillSetEnabledRequest {
                    name: (*name).to_string(),
                    enabled: true,
                },
            )
            .map_err(app_to_cli)?;
            Ok(format!("skills\tupdated\tname={}\tenabled=true\n", name))
        }
        ["/skills", "disable", name] => {
            skills_set_enabled_for_db(
                &db,
                SkillSetEnabledRequest {
                    name: (*name).to_string(),
                    enabled: false,
                },
            )
            .map_err(app_to_cli)?;
            Ok(format!("skills\tupdated\tname={}\tenabled=false\n", name))
        }
        _ => Err(CliError::InvalidUsage(
            "usage: /skills [list|search|view|install|enable|disable] ...\n".to_string(),
        )),
    }
}

fn handle_missions_command(command: &str) -> Result<String, CliError> {
    let db = open_app_database()?;

    match command.split_whitespace().collect::<Vec<_>>().as_slice() {
        ["/missions"] | ["/missions", "list"] => {
            Ok(missions::render_list(&missions::load_missions()?))
        }
        ["/missions", "get", mission_id] => Ok(missions::render_detail(
            &missions::load_mission_detail(mission_id)?,
        )),
        ["/missions", "plan", mission_id] => {
            let generated = mission_generate_plan_for_db(&db, mission_id).map_err(app_to_cli)?;
            Ok(missions::render_plan_summary(mission_id, &generated))
        }
        ["/missions", "status", mission_id, status] => {
            let status = parse_mission_status(status)?;
            let updated = missions::update_status(mission_id, status)?;
            Ok(missions::render_status(&updated))
        }
        _ => Err(CliError::InvalidUsage(
            "usage: /missions [list|get|plan|status] ...\n".to_string(),
        )),
    }
}

fn handle_background_command(command: &str) -> Result<String, CliError> {
    let trimmed = command.trim();
    if trimmed == "/background" {
        return Err(CliError::InvalidUsage(render_background_usage()));
    }
    let db = open_app_database()?;
    if trimmed == "/background status" {
        let items = list_background_runs(&db)?;
        return Ok(render_background_status(&items));
    }
    if trimmed == "/background latest" {
        let items = list_background_runs(&db)?;
        return Ok(items
            .first()
            .map(render_background_latest)
            .unwrap_or_else(|| "background\tcount=0\n".to_string()));
    }

    let prompt = trimmed
        .strip_prefix("/background")
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CliError::InvalidUsage(render_background_usage()))?;

    let mission_service = MissionServiceImpl::new(db.clone());
    let mission = mission_service
        .create(CreateMissionInput {
            title: background_title_from_prompt(prompt),
            goal: prompt.to_string(),
            constraints: vec!["Queued from /background CLI flow".to_string()],
            success_criteria: vec!["Generate a baseline execution plan".to_string()],
            priority: MissionPriority::Medium,
        })
        .map_err(app_to_cli)?;
    let generated = mission_generate_plan_for_db(&db, &mission.id).map_err(app_to_cli)?;
    let payload_json = serde_json::json!({ "prompt": prompt }).to_string();
    record_run_event(
        &db,
        &mission.id,
        &generated.run.id,
        "background_enqueued",
        &format!("Queued background prompt: {}", truncate_cli_text(prompt)),
        Some(payload_json),
    )
    .map_err(app_to_cli)?;

    Ok(format!(
        "background\tqueued\tmission_id={}\trun_id={}\tstatus={}\tprompt={}\n",
        mission.id,
        generated.run.id,
        generated.run.status.as_str(),
        prompt,
    ))
}

pub(crate) fn load_queued_prompts() -> Result<Vec<String>, CliError> {
    queue::load_queued_prompts()
}

pub(crate) fn clear_queued_prompts() -> Result<(), CliError> {
    queue::clear_queued_prompts()
}

fn app_to_cli(error: hermes_desktop::backend::AppError) -> CliError {
    CliError::Runtime(error.message)
}

fn handle_dynamic_skill_command(command: &str) -> Result<Option<String>, CliError> {
    let first = command
        .split_whitespace()
        .next()
        .filter(|token| token.starts_with('/'))
        .unwrap_or_default();
    if first.is_empty() {
        return Ok(None);
    }

    let Ok(db) = open_app_database() else {
        return Ok(None);
    };
    let skills = skills_search_for_db(&db, "".to_string(), Some(1000)).map_err(app_to_cli)?;
    let requested = normalize_skill_command_key(first);

    for skill in skills.into_iter().filter(|item| item.enabled) {
        let mut command_keys = skill_command_keys(&skill);
        command_keys.sort();
        command_keys.dedup();

        if let Some(command_key) = command_keys.into_iter().find(|key| *key == requested) {
            let detail = skills_view_for_db(&db, skill.name).map_err(app_to_cli)?;
            let instruction = command
                .strip_prefix(first)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            return Ok(Some(render_skill_invocation(
                &detail,
                &command_key,
                instruction,
            )));
        }
    }

    Ok(None)
}

fn skill_command_keys(skill: &SkillListItem) -> Vec<String> {
    let mut keys = Vec::new();

    if let Some(key) = build_skill_command_key(&skill.name) {
        keys.push(key);
    }
    if let Some(key) = build_skill_command_key(&skill.display_name) {
        keys.push(key);
    }

    keys
}

fn build_skill_command_key(value: &str) -> Option<String> {
    let slug = skill_slug(value);
    if slug.is_empty() {
        None
    } else {
        Some(format!("/{}", slug))
    }
}

fn normalize_skill_command_key(command: &str) -> String {
    let raw = command.trim().trim_start_matches('/');
    let mut normalized = String::new();
    let mut last_was_hyphen = false;

    for character in raw.chars() {
        let lower = character.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            normalized.push(lower);
            last_was_hyphen = false;
        } else if !last_was_hyphen && !normalized.is_empty() {
            normalized.push('-');
            last_was_hyphen = true;
        }
    }

    while normalized.ends_with('-') {
        normalized.pop();
    }

    format!("/{}", normalized)
}

fn skill_slug(value: &str) -> String {
    normalize_skill_command_key(value)
        .trim_start_matches('/')
        .to_string()
}

fn background_title_from_prompt(prompt: &str) -> String {
    let words = prompt
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    let candidate = if words.is_empty() {
        prompt.trim()
    } else {
        words.trim()
    };
    truncate_cli_text(candidate)
}

fn truncate_cli_text(value: &str) -> String {
    const MAX_CHARS: usize = 72;
    let trimmed = value.trim();
    let truncated = trimmed.chars().take(MAX_CHARS).collect::<String>();
    if trimmed.chars().count() > MAX_CHARS {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

fn parse_mission_status(value: &str) -> Result<MissionStatus, CliError> {
    match value.trim() {
        "draft" => Ok(MissionStatus::Draft),
        "researching" => Ok(MissionStatus::Researching),
        "simulating" => Ok(MissionStatus::Simulating),
        "planning" => Ok(MissionStatus::Planning),
        "awaiting_approval" => Ok(MissionStatus::AwaitingApproval),
        "executing" => Ok(MissionStatus::Executing),
        "paused" => Ok(MissionStatus::Paused),
        "completed" => Ok(MissionStatus::Completed),
        "failed" => Ok(MissionStatus::Failed),
        "archived" => Ok(MissionStatus::Archived),
        _ => Err(CliError::InvalidUsage(
            "usage: /missions status <mission-id> <draft|researching|simulating|planning|awaiting_approval|executing|paused|completed|failed|archived>\n".to_string(),
        )),
    }
}

fn list_background_runs(db: &Database) -> Result<Vec<BackgroundRunItem>, CliError> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT
                missions.id,
                missions.title,
                missions.status,
                runs.id,
                runs.status,
                COALESCE(
                    json_extract((
                        SELECT run_events.payload_json
                        FROM run_events
                        WHERE run_events.run_id = runs.id
                          AND run_events.event_type = 'background_enqueued'
                        ORDER BY datetime(run_events.created_at) DESC, run_events.rowid DESC
                        LIMIT 1
                    ), '$.prompt'),
                    ''
                ) AS prompt,
                (SELECT COUNT(*) FROM execution_steps WHERE execution_steps.run_id = runs.id),
                (SELECT COUNT(*) FROM execution_steps WHERE execution_steps.run_id = runs.id AND execution_steps.status = 'pending'),
                (SELECT COUNT(*) FROM execution_steps WHERE execution_steps.run_id = runs.id AND execution_steps.status = 'awaiting_approval')
             FROM runs
             INNER JOIN missions ON missions.id = runs.mission_id
             WHERE EXISTS (
                SELECT 1
                FROM run_events
                WHERE run_events.run_id = runs.id
                  AND run_events.event_type = 'background_enqueued'
             )
             ORDER BY datetime(COALESCE(runs.started_at, missions.last_activity_at)) DESC, runs.rowid DESC",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(BackgroundRunItem {
                mission_id: row.get(0)?,
                mission_title: row.get(1)?,
                mission_status: row.get(2)?,
                run_id: row.get(3)?,
                run_status: row.get(4)?,
                prompt: row.get(5)?,
                step_count: row.get(6)?,
                pending_step_count: row.get(7)?,
                awaiting_approval_step_count: row.get(8)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
    })
    .map_err(|err| CliError::Runtime(err.to_string()))
}

fn render_sessions_search(sessions: &[sessions::SessionListItem], query: &str) -> String {
    let matches = sessions::search_recent(sessions, query);
    if matches.is_empty() {
        return "no sessions found\n".to_string();
    }

    matches
        .into_iter()
        .map(sessions::render_match)
        .collect::<String>()
}

fn suggest_command(command: &str) -> String {
    let mut best = "/help";
    let mut best_distance = usize::MAX;

    for candidate in supported_commands() {
        let distance = levenshtein(command, candidate);
        if distance < best_distance {
            best = candidate;
            best_distance = distance;
        }
    }

    if best == "/help" {
        "/help".to_string()
    } else {
        format!("{best}, /help")
    }
}

fn supported_commands() -> &'static [&'static str] {
    &[
        "/help",
        "/model",
        "/busy",
        "/interrupt",
        "/cancel",
        "/stop",
        "/tools",
        "/skills",
        "/title",
        "/foreground",
        "/background",
        "/queue",
        "/usage",
        "/voice",
        "/continue",
        "/resume",
        "/sessions",
        "/missions",
    ]
}

fn levenshtein(left: &str, right: &str) -> usize {
    let right_chars: Vec<char> = right.chars().collect();
    let mut costs: Vec<usize> = (0..=right_chars.len()).collect();

    for (i, left_char) in left.chars().enumerate() {
        let mut previous_cost = costs[0];
        costs[0] = i + 1;

        for (j, right_char) in right_chars.iter().enumerate() {
            let insertion = costs[j + 1] + 1;
            let deletion = costs[j] + 1;
            let substitution = previous_cost + usize::from(left_char != *right_char);
            previous_cost = costs[j + 1];
            costs[j + 1] = insertion.min(deletion).min(substitution);
        }
    }

    *costs.last().unwrap_or(&0)
}

#[allow(clippy::too_many_arguments)]
fn handle_with<LG, SG, SLG, UG, RLG, RSG, TLG, KLG, SR>(
    command: &str,
    latest_session_getter: LG,
    session_getter: SG,
    sessions_loader: SLG,
    usage_loader: UG,
    runtime_settings_loader: RLG,
    runtime_settings_saver: RSG,
    tools_loader: TLG,
    skills_loader: KLG,
    session_renamer: SR,
) -> Result<String, CliError>
where
    LG: FnOnce() -> Result<Option<String>, CliError>,
    SG: FnOnce(&str) -> Result<Option<String>, CliError>,
    SLG: FnOnce() -> Result<Vec<sessions::SessionListItem>, CliError>,
    UG: FnOnce() -> Result<UsageSnapshot, CliError>,
    RLG: FnOnce() -> Result<RuntimeSettingsSnapshot, CliError>,
    RSG: FnOnce(&str, &str) -> Result<RuntimeSettingsSnapshot, CliError>,
    TLG: FnOnce() -> Result<Vec<ToolDescriptor>, CliError>,
    KLG: FnOnce() -> Result<Vec<SkillSummary>, CliError>,
    SR: FnOnce(&str, &str) -> Result<String, CliError>,
{
    handle_with_help(
        command,
        true,
        false,
        latest_session_getter,
        session_getter,
        sessions_loader,
        usage_loader,
        runtime_settings_loader,
        runtime_settings_saver,
        tools_loader,
        skills_loader,
        session_renamer,
    )
}

#[allow(clippy::too_many_arguments)]
fn handle_with_help<LG, SG, SLG, UG, RLG, RSG, TLG, KLG, SR>(
    command: &str,
    include_interrupt_aliases_in_help: bool,
    include_foreground_status_in_help: bool,
    latest_session_getter: LG,
    session_getter: SG,
    sessions_loader: SLG,
    usage_loader: UG,
    runtime_settings_loader: RLG,
    runtime_settings_saver: RSG,
    tools_loader: TLG,
    skills_loader: KLG,
    session_renamer: SR,
) -> Result<String, CliError>
where
    LG: FnOnce() -> Result<Option<String>, CliError>,
    SG: FnOnce(&str) -> Result<Option<String>, CliError>,
    SLG: FnOnce() -> Result<Vec<sessions::SessionListItem>, CliError>,
    UG: FnOnce() -> Result<UsageSnapshot, CliError>,
    RLG: FnOnce() -> Result<RuntimeSettingsSnapshot, CliError>,
    RSG: FnOnce(&str, &str) -> Result<RuntimeSettingsSnapshot, CliError>,
    TLG: FnOnce() -> Result<Vec<ToolDescriptor>, CliError>,
    KLG: FnOnce() -> Result<Vec<SkillSummary>, CliError>,
    SR: FnOnce(&str, &str) -> Result<String, CliError>,
{
    let trimmed = command.trim();
    let parts: Vec<&str> = trimmed.split_whitespace().collect();

    match parts.as_slice() {
        ["/help"] => Ok(if include_interrupt_aliases_in_help {
            render_help_config(true, include_foreground_status_in_help)
        } else {
            render_help_config(false, include_foreground_status_in_help)
        }),
        ["/continue"] => {
            Ok(latest_session_getter()?.unwrap_or_else(|| message("no sessions found")))
        }
        ["/resume"] => Err(CliError::InvalidUsage(
            "usage: /resume <session-id|title>\n".to_string(),
        )),
        ["/resume", selector @ ..] => {
            let selector = selector.join(" ");
            let exact = session_getter(&selector)?;
            if let Some(row) = exact {
                Ok(row)
            } else {
                let sessions = sessions_loader()?;
                Ok(sessions::find_resume_candidate(&sessions, &selector)
                    .map(sessions::render_match)
                    .unwrap_or_else(|| message("session not found")))
            }
        }
        ["/usage"] => Ok(render_usage(&usage_loader()?)),
        ["/model"] => Ok(render_model(&runtime_settings_loader()?)),
        ["/model", value] => {
            let current = runtime_settings_loader()?;
            let (provider, model) = parse_model_update(value, None, &current)?;
            Ok(render_model(&runtime_settings_saver(&provider, &model)?))
        }
        ["/model", provider, model] => Ok(render_model(&runtime_settings_saver(provider, model)?)),
        ["/busy"] | ["/busy", "status"] => Ok(render_busy_input_mode(
            &config::load_config()
                .unwrap_or_default()
                .busy_input_mode
                .trim()
                .to_ascii_lowercase(),
        )),
        ["/busy", mode] if matches!(*mode, "queue" | "interrupt") => {
            let mut cfg = config::load_config().unwrap_or_default();
            cfg.busy_input_mode = (*mode).to_string();
            config::save_config(&cfg).map_err(CliError::Runtime)?;
            Ok(render_busy_input_mode(mode))
        }
        ["/busy", ..] => Err(CliError::InvalidUsage(
            "usage: /busy [queue|interrupt|status]\n".to_string(),
        )),
        [command, rest @ ..] if matches!(*command, "/interrupt" | "/cancel" | "/stop") => {
            Ok(render_interrupt_alias(command, &rest.join(" ")))
        }
        ["/tools"] => Ok(render_tools(&tools_loader()?)),
        ["/skills"] => Ok(render_skills(&skills_loader()?)),
        ["/voice"] | ["/voice", "status"] => Ok(render_voice(false)),
        ["/voice", "on"] => Ok(render_voice(true)),
        ["/voice", "off"] => Ok(render_voice(false)),
        ["/voice", ..] => Err(CliError::InvalidUsage(
            "usage: /voice [on|off|status]\n".to_string(),
        )),
        ["/title"] => {
            let session = latest_session_getter()?
                .map(|row| parse_session_row(&row))
                .transpose()?;
            Ok(session
                .map(|session| render_title(&session))
                .unwrap_or_else(|| message("no sessions found")))
        }
        ["/title", rest @ ..] => {
            let next_title = rest.join(" ");
            let session = latest_session_getter()?
                .map(|row| parse_session_row(&row))
                .transpose()?
                .ok_or_else(|| CliError::InvalidUsage("no sessions found\n".to_string()))?;
            let renamed = session_renamer(&session.id, &next_title)?;
            Ok(render_title(&parse_session_row(&renamed)?))
        }
        ["/background"] => Err(CliError::InvalidUsage(render_background_usage())),
        ["/sessions"] => Ok(render_sessions_preview()),
        ["/sessions", "latest"] | ["/sessions", "continue"] => {
            Ok(latest_session_getter()?.unwrap_or_else(|| message("no sessions found")))
        }
        ["/sessions", "resume"] => Err(CliError::InvalidUsage(
            "usage: /resume <session-id|title>\n".to_string(),
        )),
        ["/sessions", "resume", selector @ ..] => {
            let selector = selector.join(" ");
            let exact = session_getter(&selector)?;
            if let Some(row) = exact {
                Ok(row)
            } else {
                let sessions = sessions_loader()?;
                Ok(sessions::find_resume_candidate(&sessions, &selector)
                    .map(sessions::render_match)
                    .unwrap_or_else(|| message("session not found")))
            }
        }
        ["/sessions", "title"] => {
            let session = latest_session_getter()?
                .map(|row| parse_session_row(&row))
                .transpose()?;
            Ok(session
                .map(|session| render_title(&session))
                .unwrap_or_else(|| message("no sessions found")))
        }
        ["/sessions", "title", rest @ ..] => {
            let next_title = rest.join(" ");
            let session = latest_session_getter()?
                .map(|row| parse_session_row(&row))
                .transpose()?
                .ok_or_else(|| CliError::InvalidUsage("no sessions found\n".to_string()))?;
            let renamed = session_renamer(&session.id, &next_title)?;
            Ok(render_title(&parse_session_row(&renamed)?))
        }
        ["/sessions", "search"] => Ok(render_sessions_search(&sessions_loader()?, "")),
        ["/sessions", "search", query @ ..] => Ok(render_sessions_search(
            &sessions_loader()?,
            &query.join(" "),
        )),
        ["/sessions", "active"] | ["/sessions", "active", "clear"] => {
            handle_sessions_active_command(trimmed)
        }
        ["/sessions", "history"] | ["/sessions", "replay"] => Err(CliError::InvalidUsage(
            "usage: /sessions history <session-id|active|latest>\n".to_string(),
        )),
        ["/sessions", action, selector @ ..] if matches!(*action, "history" | "replay") => {
            sessions::get_session_history(&selector.join(" "))
        }
        ["/missions"] => Ok(render_missions_preview()),
        _ => Err(CliError::InvalidUsage(format!(
            "unknown slash command: {trimmed}\nDid you mean: {}?\n",
            suggest_command(trimmed)
        ))),
    }
}

fn render_interrupt_alias(command: &str, request: &str) -> String {
    let request = request.trim();
    let mut output = format!("interrupt\talias={command}\tstatus=idle");
    if !request.is_empty() {
        output.push_str(&format!("\tfollow_up={request}"));
    }
    output.push('\n');
    output.push_str(
        "interrupt\tnote\tinterrupt commands are consumed by the foreground controller only while a REPL turn is busy; there is no active foreground turn to interrupt right now.\n",
    );
    output.push_str(
        "interrupt\thint\tsubmit a prompt first, then use /interrupt, /cancel, or /stop while that turn is still running.\n",
    );
    output
}

fn parse_model_update(
    first: &str,
    second: Option<&str>,
    current: &RuntimeSettingsSnapshot,
) -> Result<(String, String), CliError> {
    if let Some(model) = second {
        return Ok((first.trim().to_string(), model.trim().to_string()));
    }

    if let Some((provider, model)) = first.split_once(':') {
        if provider.trim().is_empty() || model.trim().is_empty() {
            return Err(CliError::InvalidUsage(
                "usage: /model [provider:model|provider model|model]\n".to_string(),
            ));
        }
        return Ok((provider.trim().to_string(), model.trim().to_string()));
    }

    if first.trim().is_empty() {
        return Err(CliError::InvalidUsage(
            "usage: /model [provider:model|provider model|model]\n".to_string(),
        ));
    }

    Ok((current.provider.clone(), first.trim().to_string()))
}

fn load_usage_snapshot() -> Result<UsageSnapshot, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    let db = Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))?;
    let recent_session_count = query_count(
        &db,
        "SELECT COUNT(*) FROM (
            SELECT 1
            FROM sessions
            ORDER BY datetime(updated_at) DESC, rowid DESC
            LIMIT 20
        )",
    );

    Ok(UsageSnapshot {
        active_mission_count: query_count(
            &db,
            "SELECT COUNT(*) FROM missions WHERE status NOT IN ('archived', 'completed', 'failed')",
        ),
        pending_approval_count: query_count(
            &db,
            "SELECT COUNT(*) FROM execution_steps WHERE status = 'awaiting_approval'",
        ),
        recent_session_count,
        has_recent_session: recent_session_count > 0,
    })
}

fn query_count(db: &Database, sql: &str) -> i64 {
    db.query_row(sql, &[], |row| row.get::<_, i64>(0))
        .unwrap_or(0)
}

fn load_runtime_settings() -> Result<RuntimeSettingsSnapshot, CliError> {
    let config_settings = config::load_config().unwrap_or_default();
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    let db = Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))?;

    let persisted = db
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = 'runtime'",
            &[],
            |row| row.get::<_, String>(0),
        )
        .ok()
        .and_then(|json| serde_json::from_str::<PersistedRuntimeSettings>(&json).ok())
        .unwrap_or(PersistedRuntimeSettings {
            provider: Some(config_settings.provider.clone()),
            model: Some(config_settings.model.clone()),
            base_url: config_settings.base_url.clone(),
            api_key_ref: None,
            engine_profile: Some("default".to_string()),
            agent_engine_enabled: Some(true),
        });

    Ok(RuntimeSettingsSnapshot {
        provider: persisted
            .provider
            .unwrap_or_else(|| config_settings.provider.clone()),
        model: persisted
            .model
            .unwrap_or_else(|| config_settings.model.clone()),
        engine_profile: persisted.engine_profile.or(Some("default".to_string())),
    })
}

fn persist_runtime_settings(
    provider: &str,
    model: &str,
) -> Result<RuntimeSettingsSnapshot, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    let db = Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))?;
    let current = load_runtime_settings()?;
    let now = Utc::now().to_rfc3339();
    let persisted = PersistedRuntimeSettings {
        provider: Some(provider.trim().to_string()),
        model: Some(model.trim().to_string()),
        base_url: None,
        api_key_ref: None,
        engine_profile: current.engine_profile.clone(),
        agent_engine_enabled: Some(true),
    };
    let json =
        serde_json::to_string(&persisted).map_err(|err| CliError::Runtime(err.to_string()))?;
    let params: Vec<&dyn rusqlite::ToSql> = vec![&json, &now];
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES ('runtime', ?, ?)",
        &params,
    )
    .map_err(|err| CliError::Runtime(err.to_string()))?;

    let mut cfg = config::load_config().unwrap_or_default();
    cfg.provider = provider.trim().to_string();
    cfg.model = model.trim().to_string();
    config::save_config(&cfg).map_err(CliError::Runtime)?;

    Ok(RuntimeSettingsSnapshot {
        provider: provider.trim().to_string(),
        model: model.trim().to_string(),
        engine_profile: current.engine_profile,
    })
}

fn load_tool_descriptors() -> Result<Vec<ToolDescriptor>, CliError> {
    Ok(vec![
        ToolDescriptor {
            name: "runtime".to_string(),
            scope: "tauri".to_string(),
            description: "engine status, start, stop, restart".to_string(),
        },
        ToolDescriptor {
            name: "missions".to_string(),
            scope: "cli+tauri".to_string(),
            description: "mission listing and creation surfaces".to_string(),
        },
        ToolDescriptor {
            name: "sessions".to_string(),
            scope: "cli+tauri".to_string(),
            description: "session lifecycle, resume, continue, rename, title".to_string(),
        },
        ToolDescriptor {
            name: "execution".to_string(),
            scope: "tauri".to_string(),
            description: "approve, start, pause, complete, retry, rerun, resume".to_string(),
        },
        ToolDescriptor {
            name: "settings".to_string(),
            scope: "tauri".to_string(),
            description: "persisted runtime/provider/model configuration".to_string(),
        },
        ToolDescriptor {
            name: "environment".to_string(),
            scope: "compat".to_string(),
            description: "environment checks, install, upgrade, hermes runtime".to_string(),
        },
    ])
}

fn load_skill_summaries() -> Result<Vec<SkillSummary>, CliError> {
    let mut roots: Vec<(String, PathBuf)> = Vec::new();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    roots.push(("hermes".to_string(), home.join(".hermes").join("skills")));
    roots.push(("codex".to_string(), home.join(".codex").join("skills")));
    roots.push(("agents".to_string(), home.join(".agents").join("skills")));

    if let Ok(cfg) = config::load_config()
        && let Some(path) = cfg.skills_dir
    {
        roots.push(("config".to_string(), PathBuf::from(path)));
    }

    let mut discovered: BTreeMap<String, SkillSummary> = BTreeMap::new();
    for (source, root) in roots {
        for (name, path) in collect_skill_paths(&root)? {
            discovered.entry(name.clone()).or_insert(SkillSummary {
                name,
                source: source.clone(),
                path,
            });
        }
    }

    Ok(discovered.into_values().collect())
}

fn collect_skill_paths(root: &Path) -> Result<Vec<(String, String)>, CliError> {
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut skills = Vec::new();
    if root.join("SKILL.md").is_file() {
        skills.push((
            skill_name_from_path(root),
            root.join("SKILL.md").display().to_string(),
        ));
    }

    for entry in fs::read_dir(root).map_err(|err| CliError::Runtime(err.to_string()))? {
        let entry = entry.map_err(|err| CliError::Runtime(err.to_string()))?;
        let path = entry.path();
        if path.is_dir() {
            let skill_file = path.join("SKILL.md");
            if skill_file.is_file() {
                skills.push((
                    skill_name_from_path(&path),
                    skill_file.display().to_string(),
                ));
            }
        }
    }

    Ok(skills)
}

fn skill_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown-skill")
        .to_string()
}

fn parse_session_row(row: &str) -> Result<SessionRow, CliError> {
    let mut parts = row.trim_end().split('\t');
    let id = parts
        .next()
        .ok_or_else(|| CliError::Runtime("invalid session row".to_string()))?;
    let _source = parts
        .next()
        .ok_or_else(|| CliError::Runtime("invalid session row".to_string()))?;
    let title = parts
        .next()
        .ok_or_else(|| CliError::Runtime("invalid session row".to_string()))?;

    Ok(SessionRow {
        id: id.to_string(),
        title: title.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::super::TEST_ENV_LOCK;
    use super::super::foreground_store::{ForegroundSnapshot, save_snapshot_for_db};
    use super::sessions;
    use super::{
        RuntimeSettingsSnapshot, SkillSummary, ToolDescriptor, UsageSnapshot, handle, handle_with,
        open_app_database, render_help, render_usage,
    };
    use hermes_desktop::backend::{
        CreateSessionInput, CreateSessionMessageInput, Database, SessionMessageRole,
        SessionService, SessionServiceImpl, SessionSource, create_app_state,
    };
    use hermes_desktop::commands::sessions::{SessionActivateRequest, session_activate_for_db};
    use hermes_desktop::commands::voice::{VoiceHistoryListRequest, voice_list_history_for_db};
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    struct TempHome {
        root: PathBuf,
        previous_home: Option<OsString>,
        previous_xdg_data_home: Option<OsString>,
    }

    impl TempHome {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("hermes-cli-test-{}", Uuid::new_v4()));
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

        fn path(&self) -> &Path {
            &self.root
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

    fn write_skill(home: &TempHome, source: &str, name: &str, body: &str) {
        let skill_dir = home
            .path()
            .join(format!(".{source}"))
            .join("skills")
            .join(name);
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            format!("---\nname: {name}\ndescription: Example {name}\n---\n\n# {name}\n\n{body}\n"),
        )
        .expect("write skill file");
    }

    fn seed_mission(id: &str, title: &str) {
        let state = create_app_state().expect("create app state");
        let db_path = {
            let guard = state.read();
            guard.db_path.clone()
        };
        let db = Database::new(&db_path).expect("open db");
        let now = "2026-04-24T10:00:00Z".to_string();
        let constraints_json = serde_json::to_string(&vec!["stay local"]).expect("json");
        let success_json = serde_json::to_string(&vec!["render mission list"]).expect("json");
        db.execute(
            "INSERT OR REPLACE INTO missions (
                id, title, goal, constraints_json, success_criteria_json,
                status, priority, pinned, created_at, updated_at, last_activity_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &id as &dyn rusqlite::ToSql,
                &title,
                &format!("Goal for {title}"),
                &constraints_json,
                &success_json,
                &"planning",
                &"medium",
                &0_i64,
                &now,
                &now,
                &now,
            ],
        )
        .expect("seed mission");
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

    fn seed_active_session(
        title: &str,
        reason: Option<&str>,
        model_name: Option<&str>,
    ) -> (String, String) {
        let state = create_app_state().expect("create app state");
        let db_path = {
            let guard = state.read();
            guard.db_path.clone()
        };
        let db = Database::new(&db_path).expect("open db");
        let service = SessionServiceImpl::new(db.clone());
        let session = service
            .create(CreateSessionInput {
                source: SessionSource::Cli,
                title: title.to_string(),
                model_name: model_name.map(str::to_string),
                parent_session_id: None,
            })
            .expect("create session");
        let active = session_activate_for_db(
            &db,
            SessionActivateRequest {
                id: session.id.clone(),
                reason: reason.map(str::to_string),
            },
        )
        .expect("activate session");
        (active.session.id, active.activated_at)
    }

    fn seed_session_with_messages(
        title: &str,
        messages: &[(&str, &str)],
        activate_reason: Option<&str>,
    ) -> String {
        let state = create_app_state().expect("create app state");
        let db_path = {
            let guard = state.read();
            guard.db_path.clone()
        };
        let db = Database::new(&db_path).expect("open db");
        let service = SessionServiceImpl::new(db.clone());
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
        if let Some(reason) = activate_reason {
            session_activate_for_db(
                &db,
                SessionActivateRequest {
                    id: session.id.clone(),
                    reason: Some(reason.to_string()),
                },
            )
            .expect("activate session");
        }

        session.id
    }

    #[test]
    fn render_help_lists_supported_commands() {
        assert_eq!(
            render_help(),
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
    fn handle_voice_commands_persist_and_record_local_activity() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();

        assert_eq!(
            handle("/voice").expect("voice status succeeds"),
            concat!(
                "voice\tenabled=false\tstt=local-text-capture\ttts=local-speak-queue\ttranscripts=0\tqueued=0\n",
                "voice\tnote\tlocal transcription and queue state are persisted; audio capture/playback is still local-only.\n",
            )
        );
        assert_eq!(
            handle("/voice on").expect("voice enable succeeds"),
            concat!(
                "voice\tenabled=true\tstt=local-text-capture\ttts=local-speak-queue\ttranscripts=0\tqueued=0\n",
                "voice\tnote\tlocal transcription and queue state are persisted; audio capture/playback is still local-only.\n",
            )
        );
        assert_eq!(
            handle("/voice transcribe hello world").expect("voice transcribe succeeds"),
            "voice\ttranscript\tprovider=local-text-capture\ttext=hello world\n"
        );
        assert_eq!(
            handle("/voice speak hello queue").expect("voice speak succeeds"),
            "voice\tspeak\tqueued=true\tprovider=local-speak-queue\ttext=hello queue\n"
        );
        let db = open_app_database().expect("open app database");
        let transcription_history = voice_list_history_for_db(
            &db,
            VoiceHistoryListRequest {
                kind: Some("transcription".to_string()),
                limit: Some(10),
                include_payload: true,
            },
        )
        .expect("load transcription history");
        assert_eq!(
            transcription_history.items[0].source.as_deref(),
            Some("cli")
        );
        let speech_history = voice_list_history_for_db(
            &db,
            VoiceHistoryListRequest {
                kind: Some("speech".to_string()),
                limit: Some(10),
                include_payload: true,
            },
        )
        .expect("load speech history");
        assert_eq!(speech_history.items[0].origin.as_deref(), Some("cli"));
        assert_eq!(
            handle("/voice status").expect("voice status succeeds"),
            concat!(
                "voice\tenabled=true\tstt=local-text-capture\ttts=local-speak-queue\ttranscripts=1\tqueued=1\n",
                "voice\tnote\tlocal transcription and queue state are persisted; audio capture/playback is still local-only.\n",
            )
        );
        assert_eq!(
            handle("/voice off").expect("voice disable succeeds"),
            concat!(
                "voice\tenabled=false\tstt=local-text-capture\ttts=local-speak-queue\ttranscripts=1\tqueued=1\n",
                "voice\tnote\tlocal transcription and queue state are persisted; audio capture/playback is still local-only.\n",
            )
        );
    }

    #[test]
    fn handle_model_tools_skills_and_title_return_real_outputs() {
        assert_eq!(
            handle_with(
                "/model",
                || unreachable!("continue getter should not be used"),
                |_| unreachable!("resume getter should not be used"),
                || unreachable!("sessions loader should not be used"),
                || unreachable!("usage loader should not be used"),
                || {
                    Ok(RuntimeSettingsSnapshot {
                        provider: "openai".to_string(),
                        model: "gpt-4o".to_string(),
                        engine_profile: Some("default".to_string()),
                    })
                },
                |_, _| unreachable!("model saver should not be used"),
                || {
                    Ok(vec![
                        ToolDescriptor {
                            name: "sessions".to_string(),
                            scope: "cli".to_string(),
                            description: "session lifecycle and title management".to_string(),
                        },
                        ToolDescriptor {
                            name: "execution".to_string(),
                            scope: "tauri".to_string(),
                            description: "approve, start, pause, complete, retry, rerun"
                                .to_string(),
                        },
                    ])
                },
                || {
                    Ok(vec![SkillSummary {
                        name: "plan".to_string(),
                        source: "codex".to_string(),
                        path: "/tmp/plan/SKILL.md".to_string(),
                    }])
                },
                |_, _| Ok("session-001\tcli\tRenamed session\tgpt-5.4\n".to_string()),
            )
            .expect("model succeeds"),
            concat!(
                "model\tprovider=openai\tmodel=gpt-4o\tprofile=default\n",
                "model\thint\t/model openai gpt-4o | /model openrouter claude-sonnet-4\n",
            )
        );
        assert_eq!(
            handle_with(
                "/tools",
                || unreachable!("continue getter should not be used"),
                |_| unreachable!("resume getter should not be used"),
                || unreachable!("sessions loader should not be used"),
                || unreachable!("usage loader should not be used"),
                || unreachable!("model loader should not be used"),
                |_, _| unreachable!("model saver should not be used"),
                || {
                    Ok(vec![ToolDescriptor {
                        name: "sessions".to_string(),
                        scope: "cli".to_string(),
                        description: "session lifecycle and title management".to_string(),
                    }])
                },
                || unreachable!("skills loader should not be used"),
                |_, _| unreachable!("session renamer should not be used"),
            )
            .expect("tools succeeds"),
            concat!(
                "tools\tcount=1\n",
                "sessions\tcli\tsession lifecycle and title management\n",
                "tools\tnote\ttool execution help is descriptive only in this CLI lane\n",
            )
        );
        assert_eq!(
            handle_with(
                "/skills",
                || unreachable!("continue getter should not be used"),
                |_| unreachable!("resume getter should not be used"),
                || unreachable!("sessions loader should not be used"),
                || unreachable!("usage loader should not be used"),
                || unreachable!("model loader should not be used"),
                |_, _| unreachable!("model saver should not be used"),
                || unreachable!("tools loader should not be used"),
                || {
                    Ok(vec![SkillSummary {
                        name: "plan".to_string(),
                        source: "codex".to_string(),
                        path: "/tmp/plan/SKILL.md".to_string(),
                    }])
                },
                |_, _| unreachable!("session renamer should not be used"),
            )
            .expect("skills succeeds"),
            concat!(
                "skills\tcount=1\n",
                "plan\tcodex\t/tmp/plan/SKILL.md\n",
                "skills\tnote\tdiscovery is live; use /skills search <query> or /skills view <name> for deeper inspection\n",
            )
        );
        assert_eq!(
            handle_with(
                "/title",
                || Ok(Some(
                    "session-001\tcli\tCurrent session\tgpt-5.4\n".to_string()
                )),
                |_| unreachable!("resume getter should not be used"),
                || unreachable!("sessions loader should not be used"),
                || unreachable!("usage loader should not be used"),
                || unreachable!("model loader should not be used"),
                |_, _| unreachable!("model saver should not be used"),
                || unreachable!("tools loader should not be used"),
                || unreachable!("skills loader should not be used"),
                |_, _| unreachable!("session renamer should not be used"),
            )
            .expect("title succeeds"),
            "title\tsession-001\tCurrent session\n"
        );
    }

    #[test]
    fn handle_model_can_persist_new_selection() {
        let updated = handle_with(
            "/model openrouter claude-sonnet-4",
            || unreachable!("continue getter should not be used"),
            |_| unreachable!("resume getter should not be used"),
            || unreachable!("sessions loader should not be used"),
            || unreachable!("usage loader should not be used"),
            || {
                Ok(RuntimeSettingsSnapshot {
                    provider: "openai".to_string(),
                    model: "gpt-4o".to_string(),
                    engine_profile: Some("default".to_string()),
                })
            },
            |provider, model| {
                assert_eq!(provider, "openrouter");
                assert_eq!(model, "claude-sonnet-4");
                Ok(RuntimeSettingsSnapshot {
                    provider: provider.to_string(),
                    model: model.to_string(),
                    engine_profile: Some("default".to_string()),
                })
            },
            || unreachable!("tools loader should not be used"),
            || unreachable!("skills loader should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("model update succeeds");

        assert_eq!(
            updated,
            concat!(
                "model\tprovider=openrouter\tmodel=claude-sonnet-4\tprofile=default\n",
                "model\thint\t/model openai gpt-4o | /model openrouter claude-sonnet-4\n",
            )
        );
    }

    #[test]
    fn handle_title_can_rename_latest_session() {
        let rendered = handle_with(
            "/title Renamed session",
            || {
                Ok(Some(
                    "session-001\tcli\tCurrent session\tgpt-5.4\n".to_string(),
                ))
            },
            |_| unreachable!("resume getter should not be used"),
            || unreachable!("sessions loader should not be used"),
            || unreachable!("usage loader should not be used"),
            || unreachable!("model loader should not be used"),
            |_, _| unreachable!("model saver should not be used"),
            || unreachable!("tools loader should not be used"),
            || unreachable!("skills loader should not be used"),
            |id, title| {
                assert_eq!(id, "session-001");
                assert_eq!(title, "Renamed session");
                Ok("session-001\tcli\tRenamed session\tgpt-5.4\n".to_string())
            },
        )
        .expect("title rename succeeds");

        assert_eq!(rendered, "title\tsession-001\tRenamed session\n");
    }

    #[test]
    fn render_usage_formats_summary_counts() {
        let rendered = render_usage(&UsageSnapshot {
            active_mission_count: 3,
            pending_approval_count: 1,
            recent_session_count: 8,
            has_recent_session: true,
        });

        assert_eq!(
            rendered,
            concat!(
                "usage\tactive_missions=3\tpending_approvals=1\trecent_sessions=8\thas_recent_session=true\n",
                "usage\thint\t/sessions latest | /sessions search <query> | /missions\n",
            )
        );
    }

    #[test]
    fn handle_continue_and_resume_can_return_real_session_rows() {
        assert_eq!(
            handle_with(
                "/continue",
                || Ok(Some(
                    "session-latest\tcli\tLatest session\tgpt-5.4\n".to_string()
                )),
                |_| unreachable!("resume getter should not be used"),
                || unreachable!("sessions loader should not be used"),
                || unreachable!("usage loader should not be used"),
                || unreachable!("model loader should not be used"),
                |_, _| unreachable!("model saver should not be used"),
                || unreachable!("tools loader should not be used"),
                || unreachable!("skills loader should not be used"),
                |_, _| unreachable!("session renamer should not be used"),
            )
            .expect("continue succeeds"),
            "session-latest\tcli\tLatest session\tgpt-5.4\n"
        );
        assert_eq!(
            handle_with(
                "/resume session-001",
                || unreachable!("continue getter should not be used"),
                |id| {
                    assert_eq!(id, "session-001");
                    Ok(Some(
                        "session-001\tcli\tRecovered session\tgpt-5.4\n".to_string(),
                    ))
                },
                || unreachable!("sessions loader should not be used"),
                || unreachable!("usage loader should not be used"),
                || unreachable!("model loader should not be used"),
                |_, _| unreachable!("model saver should not be used"),
                || unreachable!("tools loader should not be used"),
                || unreachable!("skills loader should not be used"),
                |_, _| unreachable!("session renamer should not be used"),
            )
            .expect("resume succeeds"),
            "session-001\tcli\tRecovered session\tgpt-5.4\n"
        );
    }

    #[test]
    fn handle_resume_requires_session_id() {
        let error = handle_with(
            "/resume",
            || unreachable!("continue getter should not be used"),
            |_| unreachable!("resume getter should not be used"),
            || unreachable!("sessions loader should not be used"),
            || unreachable!("usage loader should not be used"),
            || unreachable!("model loader should not be used"),
            |_, _| unreachable!("model saver should not be used"),
            || unreachable!("tools loader should not be used"),
            || unreachable!("skills loader should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect_err("resume without id should fail");

        assert_eq!(error.to_string(), "usage: /resume <session-id|title>\n");
    }

    #[test]
    fn handle_usage_can_render_real_summary() {
        assert_eq!(
            handle_with(
                "/usage",
                || unreachable!("continue getter should not be used"),
                |_| unreachable!("resume getter should not be used"),
                || unreachable!("sessions loader should not be used"),
                || {
                    Ok(UsageSnapshot {
                        active_mission_count: 2,
                        pending_approval_count: 1,
                        recent_session_count: 4,
                        has_recent_session: true,
                    })
                },
                || unreachable!("model loader should not be used"),
                |_, _| unreachable!("model saver should not be used"),
                || unreachable!("tools loader should not be used"),
                || unreachable!("skills loader should not be used"),
                |_, _| unreachable!("session renamer should not be used"),
            )
            .expect("usage succeeds"),
            concat!(
                "usage\tactive_missions=2\tpending_approvals=1\trecent_sessions=4\thas_recent_session=true\n",
                "usage\thint\t/sessions latest | /sessions search <query> | /missions\n",
            )
        );
    }

    #[test]
    fn handle_supports_real_background_usage_and_mission_listing() {
        let sessions = handle_with(
            "/sessions",
            || unreachable!("continue getter should not be used"),
            |_| unreachable!("resume getter should not be used"),
            || unreachable!("sessions loader should not be used"),
            || unreachable!("usage loader should not be used"),
            || unreachable!("model loader should not be used"),
            |_, _| unreachable!("model saver should not be used"),
            || unreachable!("tools loader should not be used"),
            || unreachable!("skills loader should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("sessions preview succeeds");
        assert!(sessions.contains("sessions\tlatest"));
        assert!(sessions.contains("/resume <session-id|title>"));
        assert!(sessions.contains("sessions\thistory"));
        assert!(sessions.contains("sessions\treplay"));

        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        seed_mission("mission-001", "Hermes parity follow-up");

        let missions = handle("/missions").expect("missions list succeeds");
        assert_eq!(
            missions,
            "mission-001\tplanning\tmedium\tHermes parity follow-up\n"
        );
    }

    #[test]
    fn handle_sessions_active_renders_current_active_handoff() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let (session_id, activated_at) =
            seed_active_session("CLI handoff", Some("manual_resume"), Some("gpt-5.4"));

        assert_eq!(
            handle("/sessions active").expect("active session succeeds"),
            format!(
                "sessions\tactive\tid={session_id}\tsource=cli\ttitle=CLI handoff\tmodel=gpt-5.4\treason=manual_resume\tactivated_at={activated_at}\n"
            )
        );
    }

    #[test]
    fn handle_sessions_active_clear_clears_current_active_handoff() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        seed_active_session("CLI handoff", Some("manual_resume"), Some("gpt-5.4"));

        assert_eq!(
            handle("/sessions active clear").expect("clear active session succeeds"),
            "sessions\tactive\tcleared=true\n"
        );
        assert_eq!(
            handle("/sessions active").expect("active session after clear succeeds"),
            "sessions\tactive\tnone\n"
        );
    }

    #[test]
    fn handle_sessions_history_renders_latest_transcript_in_chronological_order() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let session_id = seed_session_with_messages(
            "Transcript session",
            &[("user", "first prompt"), ("assistant", "second reply")],
            None,
        );

        assert_eq!(
            handle("/sessions history latest").expect("latest history succeeds"),
            concat!(
                "session_history\tresolved_via=latest\tsession_id=",
                "{session_id}",
                "\tsource=cli\ttitle=Transcript session\tcount=2\n",
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

    #[test]
    fn handle_sessions_replay_resolves_active_session() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        let session_id = seed_session_with_messages(
            "Replay session",
            &[
                ("user", "line with\ttab"),
                ("assistant", "line one\nline two"),
            ],
            Some("manual_resume"),
        );

        assert_eq!(
            handle("/sessions replay active").expect("active replay succeeds"),
            concat!(
                "session_history\tresolved_via=active\tsession_id=",
                "{session_id}",
                "\tsource=cli\ttitle=Replay session\tcount=2\n",
                "session_message\tsession_id=",
                "{session_id}",
                "\trole=user\tsource=local\tcontent_json=\"line with\\ttab\"\n",
                "session_message\tsession_id=",
                "{session_id}",
                "\trole=assistant\tsource=local\tcontent_json=\"line one\\nline two\"\n",
            )
            .replace("{session_id}", &session_id)
        );
    }

    #[test]
    fn handle_background_requires_prompt_or_supported_subcommand() {
        let error = handle("/background").expect_err("background without prompt should fail");
        assert_eq!(
            error.to_string(),
            "usage: /background <prompt> | /background status | /background latest\n"
        );
    }

    #[test]
    fn handle_foreground_requires_status_subcommand() {
        let error = handle("/foreground").expect_err("foreground without subcommand should fail");
        assert_eq!(error.to_string(), "usage: /foreground status\n");
    }

    #[test]
    fn handle_foreground_status_returns_persisted_snapshot_output() {
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

        assert_eq!(
            handle("/foreground status").expect("foreground status succeeds"),
            concat!(
                "foreground\tstatus\tstate=running\tactive=true\tsession_id=session-123\trun_id=run-456\tcancel_state=requested\tpending=3\tinterrupts=1\tupdated_at=2026-04-24T00:00:00Z\n",
                "foreground\tnote\tsnapshot_source=cli_foreground_store\tfreshness=persisted\tvalues reflect the latest saved foreground status snapshot.\n",
            )
        );
    }

    #[test]
    fn handle_queue_requires_prompt_or_supported_subcommand() {
        let error = handle("/queue").expect_err("queue without prompt should fail");
        assert_eq!(
            error.to_string(),
            "usage: /queue <prompt> | /queue status\n"
        );
    }

    #[test]
    fn handle_queue_can_persist_prompt_and_report_status() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();

        let queued =
            handle("/queue follow up with a rollout checklist").expect("queue enqueue succeeds");
        assert_eq!(
            queued,
            "queue\tqueued\tcount=1\tprompt=follow up with a rollout checklist\n"
        );

        let status = handle("/queue status").expect("queue status succeeds");
        assert_eq!(
            status,
            concat!(
                "queue\tcount=1\n",
                "queue\titem\tindex=1\tprompt=follow up with a rollout checklist\n",
            )
        );
    }

    #[test]
    fn handle_background_can_enqueue_prompt_backed_mission_and_report_status() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();

        let queued =
            handle("/background summarize quarterly roadmap").expect("background enqueue succeeds");
        assert!(queued.contains("background\tqueued\tmission_id="));
        assert!(queued.contains("\trun_id="));
        assert!(queued.contains("\tstatus=queued"));
        assert!(queued.contains("\tprompt=summarize quarterly roadmap"));

        let status = handle("/background status").expect("background status succeeds");
        assert!(status.contains("background\tcount=1"));
        assert!(status.contains("\tmission_status=awaiting_approval"));
        assert!(status.contains("\trun_status=queued"));

        let latest = handle("/background latest").expect("background latest succeeds");
        assert!(latest.contains("background\tlatest\tmission_id="));
        assert!(latest.contains("\tprompt=summarize quarterly roadmap"));
    }

    #[test]
    fn handle_missions_get_plan_and_status_drive_real_mission_workflow() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let _home = TempHome::new();
        seed_mission("mission-002", "Mission detail flow");

        let detail = handle("/missions get mission-002").expect("mission detail succeeds");
        assert!(detail.contains("missions\tdetail\tid=mission-002\tstatus=planning"));
        assert!(detail.contains("missions\tgoal\tGoal for Mission detail flow"));
        assert!(detail.contains("missions\tcounts\tcontext_items=0\truns=0\tartifacts=0"));

        let plan = handle("/missions plan mission-002").expect("mission plan succeeds");
        assert!(plan.contains("missions\tplan\tmission_id=mission-002\trun_id="));
        assert!(plan.contains("\trun_status=queued\tsteps=3"));

        let status = handle("/missions status mission-002 paused").expect("status update succeeds");
        assert_eq!(status, "missions\tstatus\tid=mission-002\tstatus=paused\n");

        let updated_detail = handle("/missions get mission-002").expect("updated detail succeeds");
        assert!(updated_detail.contains("missions\tdetail\tid=mission-002\tstatus=paused"));
        assert!(updated_detail.contains("missions\tcounts\tcontext_items=0\truns=1\tartifacts=0"));
    }

    #[test]
    fn handle_skills_search_view_install_and_toggle_use_local_skill_state() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let home = TempHome::new();
        write_skill(&home, "codex", "plan", "Follow the plan.");

        let search = handle("/skills search pla").expect("skills search succeeds");
        let expected_path = home
            .path()
            .join(".codex")
            .join("skills")
            .join("plan")
            .join("SKILL.md");
        assert_eq!(
            search,
            format!(
                "skills\tcount=1\nplan\tcodex\t{}\tenabled=true\nskills\tnote\tfiltered skill results\n",
                expected_path.display()
            )
        );

        let detail = handle("/skills view plan").expect("skills view succeeds");
        assert!(detail.contains("skills\tdetail\tname=plan\tsource=codex\tenabled=true"));
        assert!(detail.contains("Follow the plan."));

        let installed = handle("/skills install planner").expect("skills install succeeds");
        assert!(installed.contains("skills\tinstalled\tplanner"));
        assert!(
            home.path()
                .join(".hermes")
                .join("skills")
                .join("planner")
                .join("SKILL.md")
                .exists()
        );

        let disabled = handle("/skills disable plan").expect("skills disable succeeds");
        assert_eq!(disabled, "skills\tupdated\tname=plan\tenabled=false\n");

        let disabled_search = handle("/skills search plan").expect("skills search succeeds");
        assert!(disabled_search.contains("\tenabled=false\n"));

        let enabled = handle("/skills enable plan").expect("skills enable succeeds");
        assert_eq!(enabled, "skills\tupdated\tname=plan\tenabled=true\n");
    }

    #[test]
    fn handle_dynamic_skill_slash_command_renders_invocation_payload() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let home = TempHome::new();
        write_skill(
            &home,
            "codex",
            "plan",
            "Follow the plan and write a markdown design.",
        );

        let rendered = handle("/plan draft the migration plan").expect("skill slash succeeds");

        assert!(rendered.contains("skill\tcommand=/plan\tname=plan\tsource=codex"));
        assert!(rendered.contains("skill\tinstruction\tdraft the migration plan"));
        assert!(rendered.contains("Follow the plan and write a markdown design."));
        assert!(rendered.contains("[SYSTEM: The user has invoked the \"plan\" skill"));
    }

    #[test]
    fn handle_dynamic_skill_slash_command_uses_display_name_slug_and_underscore_alias() {
        let _guard = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let home = TempHome::new();
        let skill_dir = home.path().join(".codex").join("skills").join("audiocraft");
        fs::create_dir_all(&skill_dir).expect("create skill dir");
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: AudioCraft Audio Generation\ndescription: Generate audio\n---\n\n# AudioCraft\n\nGenerate some audio.\n",
        )
        .expect("write skill file");

        let hyphen = handle("/audiocraft-audio-generation synth wave")
            .expect("hyphenated skill slash succeeds");
        assert!(hyphen.contains("skill\tcommand=/audiocraft-audio-generation\tname=audiocraft"));
        assert!(hyphen.contains("skill\tinstruction\tsynth wave"));
        assert!(hyphen.contains("Generate some audio."));

        let underscored = handle("/audiocraft_audio_generation synth wave")
            .expect("underscored skill slash succeeds");
        assert!(
            underscored.contains("skill\tcommand=/audiocraft-audio-generation\tname=audiocraft")
        );
    }

    #[test]
    fn handle_sessions_resume_can_fall_back_to_recent_title_matching() {
        let rendered = handle_with(
            "/resume quarterly planning",
            || unreachable!("latest session getter should not be used"),
            |id| {
                assert_eq!(id, "quarterly planning");
                Ok(None)
            },
            || {
                Ok(vec![sessions::SessionListItem {
                    id: "session-002".to_string(),
                    source: "cli".to_string(),
                    title: "Quarterly Planning Review".to_string(),
                    model_name: Some("gpt-5.4".to_string()),
                }])
            },
            || unreachable!("usage loader should not be used"),
            || unreachable!("model loader should not be used"),
            |_, _| unreachable!("model saver should not be used"),
            || unreachable!("tools loader should not be used"),
            || unreachable!("skills loader should not be used"),
            |_, _| unreachable!("session renamer should not be used"),
        )
        .expect("resume fallback succeeds");

        assert_eq!(
            rendered,
            "session-002\tcli\tQuarterly Planning Review\tgpt-5.4\n"
        );
    }

    #[test]
    fn handle_interrupt_aliases_render_clear_idle_hint() {
        assert_eq!(
            handle("/interrupt").expect("interrupt succeeds"),
            concat!(
                "interrupt\talias=/interrupt\tstatus=idle\n",
                "interrupt\tnote\tinterrupt commands are consumed by the foreground controller only while a REPL turn is busy; there is no active foreground turn to interrupt right now.\n",
                "interrupt\thint\tsubmit a prompt first, then use /interrupt, /cancel, or /stop while that turn is still running.\n",
            )
        );
        assert_eq!(
            handle("/cancel current turn").expect("cancel succeeds"),
            concat!(
                "interrupt\talias=/cancel\tstatus=idle\tfollow_up=current turn\n",
                "interrupt\tnote\tinterrupt commands are consumed by the foreground controller only while a REPL turn is busy; there is no active foreground turn to interrupt right now.\n",
                "interrupt\thint\tsubmit a prompt first, then use /interrupt, /cancel, or /stop while that turn is still running.\n",
            )
        );
        assert_eq!(
            handle("/stop after this reply").expect("stop succeeds"),
            concat!(
                "interrupt\talias=/stop\tstatus=idle\tfollow_up=after this reply\n",
                "interrupt\tnote\tinterrupt commands are consumed by the foreground controller only while a REPL turn is busy; there is no active foreground turn to interrupt right now.\n",
                "interrupt\thint\tsubmit a prompt first, then use /interrupt, /cancel, or /stop while that turn is still running.\n",
            )
        );
    }

    #[test]
    fn handle_rejects_unknown_interrupt_command_with_interrupt_suggestion() {
        let error = handle("/intrupt").expect_err("unknown interrupt alias should fail");

        assert_eq!(
            error.to_string(),
            "unknown slash command: /intrupt\nDid you mean: /interrupt, /help?\n"
        );
    }

    #[test]
    fn handle_rejects_unknown_command_with_suggestions() {
        let error = handle("/modle").expect_err("unknown slash command should fail");

        assert_eq!(
            error.to_string(),
            "unknown slash command: /modle\nDid you mean: /model, /help?\n"
        );
    }

    #[test]
    fn handle_rejects_unknown_foreground_command_with_foreground_suggestion() {
        let error = handle("/foregroun").expect_err("unknown foreground slash command should fail");

        assert_eq!(
            error.to_string(),
            "unknown slash command: /foregroun\nDid you mean: /foreground, /help?\n"
        );
    }
}
