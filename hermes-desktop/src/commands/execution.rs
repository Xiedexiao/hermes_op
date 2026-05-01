//! Execution 命令

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::process::Command;
use tauri::State;

use crate::backend::storage::ExecutionRepository;
use crate::backend::{
    AppError, Database, ExecutionMode, ExecutionService, ExecutionServiceImpl, ExecutionStep,
    ExecutionStepStatus,
};
use crate::commands::timeline::record_run_event;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionListByMissionRequest {
    pub mission_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStepActionRequest {
    pub id: String,
    #[serde(default)]
    pub output_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionRunCliStepRequest {
    pub id: String,
    #[serde(default)]
    pub cwd: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStepNoteRequest {
    pub id: String,
    pub note: String,
    #[serde(default)]
    pub pause_before_continue: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPrepareDesktopHandoffRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionMarkDesktopHandoffReviewedRequest {
    pub run_id: String,
    pub step_id: String,
    #[serde(default)]
    pub review_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionDesktopHandoff {
    pub step_id: String,
    pub mission_id: String,
    pub run_id: String,
    pub title: String,
    pub status: String,
    pub risk_level: String,
    pub automatic_execution: bool,
    pub reason: String,
    #[serde(default)]
    pub checklist: Vec<String>,
    pub input_payload: Option<Value>,
    pub handoff_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionDesktopHandoffQueueRequest {
    pub mission_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionDesktopHandoffQueueItem {
    pub step: ExecutionStep,
    pub handoff_prepared: bool,
    pub prepared_event_count: usize,
    pub latest_prepared_at: Option<String>,
    pub handoff_reviewed: bool,
    pub reviewed_event_count: usize,
    pub latest_reviewed_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct ExecutionCliPayload {
    command: String,
    #[serde(default)]
    cwd: Option<String>,
}

#[tauri::command]
pub fn execution_list_by_mission(
    db: State<'_, Database>,
    request: ExecutionListByMissionRequest,
) -> Result<Vec<ExecutionStep>, AppError> {
    let service = ExecutionServiceImpl::new(db.inner().clone());
    service.list_steps_for_mission(&request.mission_id)
}

#[tauri::command]
pub fn execution_approve_step(
    db: State<'_, Database>,
    request: ExecutionStepActionRequest,
) -> Result<ExecutionStep, AppError> {
    let service = ExecutionServiceImpl::new(db.inner().clone());
    let step = service.approve_step(&request.id)?;
    record_run_event(
        db.inner(),
        &step.mission_id,
        &step.run_id,
        "step_started",
        &format!("Approved step: {}", step.title),
        None,
    )?;
    Ok(step)
}

#[tauri::command]
pub fn execution_start_step(
    db: State<'_, Database>,
    request: ExecutionStepActionRequest,
) -> Result<ExecutionStep, AppError> {
    let service = ExecutionServiceImpl::new(db.inner().clone());
    let step = service.start_step(&request.id)?;
    record_run_event(
        db.inner(),
        &step.mission_id,
        &step.run_id,
        "step_started",
        &format!("Started step: {}", step.title),
        step.input_payload.clone(),
    )?;
    Ok(step)
}

pub fn execution_run_cli_step_for_db(
    db: &Database,
    request: ExecutionRunCliStepRequest,
) -> Result<ExecutionStep, AppError> {
    let id = request.id.trim();
    if id.is_empty() {
        return Err(AppError::validation("execution step id cannot be empty"));
    }

    let repo = ExecutionRepository::new(db.clone());
    let existing = repo
        .get(id)?
        .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
    if existing.mode != ExecutionMode::Cli {
        return Err(AppError::validation(
            "execution runner only supports cli steps",
        ));
    }
    if existing.risk_level.requires_approval() && existing.status != ExecutionStepStatus::Running {
        return Err(AppError::validation(
            "high-risk cli steps must be approved before execution",
        ));
    }

    let payload = parse_cli_payload(existing.input_payload.as_deref())?;
    let cwd = request.cwd.or(payload.cwd).and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    });

    let service = ExecutionServiceImpl::new(db.clone());
    let running = match existing.status {
        ExecutionStepStatus::Pending => service.start_step(id)?,
        ExecutionStepStatus::Running => existing,
        _ => {
            return Err(AppError::validation(
                "cli step must be pending or running before execution",
            ));
        }
    };
    record_run_event(
        db,
        &running.mission_id,
        &running.run_id,
        "step_started",
        &format!("Running CLI step: {}", running.title),
        Some(payload.command.clone()),
    )?;

    let output = run_shell_command(&payload.command, cwd.as_deref())?;
    let summary = summarize_cli_output(output.status, &output.stdout, &output.stderr);
    let finished = if output.status == 0 {
        let step = service.complete_step(id, Some(summary.clone()))?;
        record_run_event(
            db,
            &step.mission_id,
            &step.run_id,
            "step_completed",
            &format!("Completed CLI step: {}", step.title),
            Some(summary),
        )?;
        step
    } else {
        let step = service.fail_step(id, Some(summary.clone()))?;
        record_run_event(
            db,
            &step.mission_id,
            &step.run_id,
            "step_failed",
            &format!("Failed CLI step: {}", step.title),
            Some(summary),
        )?;
        step
    };

    Ok(finished)
}

#[tauri::command]
pub fn execution_run_cli_step(
    db: State<'_, Database>,
    request: ExecutionRunCliStepRequest,
) -> Result<ExecutionStep, AppError> {
    execution_run_cli_step_for_db(db.inner(), request)
}

pub fn execution_add_step_note_for_db(
    db: &Database,
    request: ExecutionStepNoteRequest,
) -> Result<ExecutionStep, AppError> {
    let id = request.id.trim();
    if id.is_empty() {
        return Err(AppError::validation("execution step id cannot be empty"));
    }

    let note = request.note.trim();
    if note.is_empty() {
        return Err(AppError::validation("execution step note cannot be empty"));
    }

    let repo = ExecutionRepository::new(db.clone());
    let existing = repo
        .get(id)?
        .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
    let created_at = Utc::now().to_rfc3339();
    let annotated_payload =
        append_user_note_to_payload(existing.input_payload.as_deref(), note, created_at.as_str())?;
    let pause_before_continue = request.pause_before_continue.unwrap_or(true);
    let next_status = if pause_before_continue && existing.status == ExecutionStepStatus::Running {
        ExecutionStepStatus::Paused
    } else {
        existing.status.clone()
    };

    db.execute(
        "UPDATE execution_steps
         SET input_payload = ?2, status = ?3, updated_at = ?4
         WHERE id = ?1",
        &[
            &id as &dyn rusqlite::ToSql,
            &annotated_payload,
            &next_status.as_str(),
            &created_at,
        ],
    )?;

    let updated = repo
        .get(id)?
        .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
    record_run_event(
        db,
        &updated.mission_id,
        &updated.run_id,
        "step_note_added",
        &format!("Added note to step: {}", updated.title),
        Some(note.to_string()),
    )?;

    if existing.status == ExecutionStepStatus::Running
        && updated.status == ExecutionStepStatus::Paused
    {
        record_run_event(
            db,
            &updated.mission_id,
            &updated.run_id,
            "step_paused_for_note",
            &format!("Paused step for user note: {}", updated.title),
            Some(note.to_string()),
        )?;
    }

    Ok(updated)
}

#[tauri::command]
pub fn execution_add_step_note(
    db: State<'_, Database>,
    request: ExecutionStepNoteRequest,
) -> Result<ExecutionStep, AppError> {
    execution_add_step_note_for_db(db.inner(), request)
}

pub fn execution_prepare_desktop_handoff_for_db(
    db: &Database,
    request: ExecutionPrepareDesktopHandoffRequest,
) -> Result<ExecutionDesktopHandoff, AppError> {
    let id = request.id.trim();
    if id.is_empty() {
        return Err(AppError::validation("execution step id cannot be empty"));
    }

    let repo = ExecutionRepository::new(db.clone());
    let step = repo
        .get(id)?
        .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
    if step.mode != ExecutionMode::Desktop {
        return Err(AppError::validation(
            "desktop handoff only supports desktop steps",
        ));
    }

    let input_payload = step
        .input_payload
        .as_ref()
        .map(|raw| serde_json::from_str(raw).unwrap_or_else(|_| json!({ "raw": raw })));
    let checklist = build_desktop_handoff_checklist(&step);
    let reason = "Desktop GUI runtime is not connected; Hermes can prepare an auditable handoff but will not pretend to operate the desktop automatically.".to_string();
    let handoff_prompt = render_desktop_handoff_prompt(&step, &checklist);
    let handoff = ExecutionDesktopHandoff {
        step_id: step.id.clone(),
        mission_id: step.mission_id.clone(),
        run_id: step.run_id.clone(),
        title: step.title.clone(),
        status: step.status.as_str().to_string(),
        risk_level: step.risk_level.as_str().to_string(),
        automatic_execution: false,
        reason,
        checklist,
        input_payload,
        handoff_prompt,
    };

    record_run_event(
        db,
        &handoff.mission_id,
        &handoff.run_id,
        "desktop_handoff_prepared",
        &format!("Prepared desktop handoff: {}", handoff.title),
        Some(serde_json::to_string(&handoff).map_err(AppError::from_json_error)?),
    )?;

    Ok(handoff)
}

#[tauri::command]
pub fn execution_prepare_desktop_handoff(
    db: State<'_, Database>,
    request: ExecutionPrepareDesktopHandoffRequest,
) -> Result<ExecutionDesktopHandoff, AppError> {
    execution_prepare_desktop_handoff_for_db(db.inner(), request)
}

pub fn execution_mark_desktop_handoff_reviewed_for_db(
    db: &Database,
    request: ExecutionMarkDesktopHandoffReviewedRequest,
) -> Result<(), AppError> {
    let run_id = request.run_id.trim();
    if run_id.is_empty() {
        return Err(AppError::validation("execution run id cannot be empty"));
    }

    let step_id = request.step_id.trim();
    if step_id.is_empty() {
        return Err(AppError::validation("execution step id cannot be empty"));
    }

    let review_note = request
        .review_note
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let repo = ExecutionRepository::new(db.clone());
    let step = repo
        .get(step_id)?
        .ok_or_else(|| AppError::storage(format!("execution step not found: {}", step_id)))?;
    if step.mode != ExecutionMode::Desktop {
        return Err(AppError::validation(
            "desktop handoff review only supports desktop steps",
        ));
    }
    if step.run_id != run_id {
        return Err(AppError::validation(
            "desktop handoff review requires a matching run and step",
        ));
    }

    let (prepared_event_count, _) = db.with_connection(|conn| {
        load_desktop_handoff_event_projection(
            conn,
            &step.run_id,
            &step.mission_id,
            &step.id,
            "desktop_handoff_prepared",
        )
    })?;
    if prepared_event_count == 0 {
        return Err(AppError::validation(
            "desktop handoff must be prepared before it can be reviewed",
        ));
    }

    let payload = json!({
        "step_id": step.id,
        "note": review_note,
    });
    record_run_event(
        db,
        &step.mission_id,
        &step.run_id,
        "desktop_handoff_reviewed",
        &format!("Reviewed desktop handoff: {}", step.title),
        Some(serde_json::to_string(&payload).map_err(AppError::from_json_error)?),
    )?;

    Ok(())
}

#[tauri::command]
pub fn execution_mark_desktop_handoff_reviewed(
    db: State<'_, Database>,
    request: ExecutionMarkDesktopHandoffReviewedRequest,
) -> Result<(), AppError> {
    execution_mark_desktop_handoff_reviewed_for_db(db.inner(), request)
}

pub fn execution_list_desktop_handoff_queue_for_db(
    db: &Database,
    request: ExecutionDesktopHandoffQueueRequest,
) -> Result<Vec<ExecutionDesktopHandoffQueueItem>, AppError> {
    let mission_id = request
        .mission_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    db.with_connection(|conn| {
        let mut stmt = if mission_id.is_some() {
            conn.prepare(
                "SELECT id, mission_id, run_id, title, mode, risk_level, status,
                        input_payload, output_summary, created_at, updated_at
                 FROM execution_steps
                 WHERE mode = 'desktop' AND mission_id = ?1
                 ORDER BY datetime(updated_at) DESC, rowid DESC",
            )?
        } else {
            conn.prepare(
                "SELECT id, mission_id, run_id, title, mode, risk_level, status,
                        input_payload, output_summary, created_at, updated_at
                 FROM execution_steps
                 WHERE mode = 'desktop'
                 ORDER BY datetime(updated_at) DESC, rowid DESC",
            )?
        };
        let mut rows = match mission_id.as_deref() {
            Some(id) => stmt.query([id])?,
            None => stmt.query([])?,
        };
        let mut items = Vec::new();
        while let Some(row) = rows.next()? {
            let step = ExecutionStep {
                id: row.get(0)?,
                mission_id: row.get(1)?,
                run_id: row.get(2)?,
                title: row.get(3)?,
                mode: ExecutionMode::from_key(&row.get::<_, String>(4)?),
                risk_level: crate::backend::RiskLevel::from_key(&row.get::<_, String>(5)?),
                status: ExecutionStepStatus::from_key(&row.get::<_, String>(6)?),
                input_payload: row.get(7)?,
                output_summary: row.get(8)?,
                created_at: row.get(9)?,
                updated_at: row.get(10)?,
            };
            let (prepared_event_count, latest_prepared_at) = load_desktop_handoff_event_projection(
                conn,
                &step.run_id,
                &step.mission_id,
                &step.id,
                "desktop_handoff_prepared",
            )?;
            let (reviewed_event_count, latest_reviewed_at) = load_desktop_handoff_event_projection(
                conn,
                &step.run_id,
                &step.mission_id,
                &step.id,
                "desktop_handoff_reviewed",
            )?;
            items.push(ExecutionDesktopHandoffQueueItem {
                step,
                handoff_prepared: prepared_event_count > 0,
                prepared_event_count,
                latest_prepared_at,
                handoff_reviewed: reviewed_event_count > 0,
                reviewed_event_count,
                latest_reviewed_at,
            });
        }
        Ok(items)
    })
}

#[tauri::command]
pub fn execution_list_desktop_handoff_queue(
    db: State<'_, Database>,
    request: ExecutionDesktopHandoffQueueRequest,
) -> Result<Vec<ExecutionDesktopHandoffQueueItem>, AppError> {
    execution_list_desktop_handoff_queue_for_db(db.inner(), request)
}

fn load_desktop_handoff_event_projection(
    conn: &rusqlite::Connection,
    run_id: &str,
    mission_id: &str,
    step_id: &str,
    event_type: &str,
) -> rusqlite::Result<(usize, Option<String>)> {
    let (event_count, latest_event_at): (i64, Option<String>) = conn.query_row(
        "SELECT COUNT(*), MAX(created_at)
         FROM run_events
         WHERE run_id = ?1
           AND mission_id = ?2
           AND event_type = ?3
           AND payload_json IS NOT NULL
           AND json_valid(payload_json)
           AND json_extract(payload_json, '$.step_id') = ?4",
        (run_id, mission_id, event_type, step_id),
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok((event_count as usize, latest_event_at))
}

fn build_desktop_handoff_checklist(step: &ExecutionStep) -> Vec<String> {
    let mut checklist = Vec::new();
    if step.risk_level.requires_approval() {
        checklist.push("Confirm approval before touching the desktop target.".to_string());
    }
    checklist.push("Confirm the target app/window and user-visible state manually.".to_string());
    checklist
        .push("Review the input payload and translate it into explicit GUI actions.".to_string());
    checklist.push(
        "Execute only through an approved desktop runtime or manual operator path.".to_string(),
    );
    checklist.push(
        "Capture evidence, then mark the step completed or failed with an output summary."
            .to_string(),
    );
    checklist
}

fn render_desktop_handoff_prompt(step: &ExecutionStep, checklist: &[String]) -> String {
    let mut prompt = format!(
        "desktop_handoff\tstep={}\tmission={}\trun={}\trisk={}\tstatus={}\n",
        step.id,
        step.mission_id,
        step.run_id,
        step.risk_level.as_str(),
        step.status.as_str(),
    );
    prompt.push_str(&format!("title\t{}\n", step.title));
    if let Some(payload) = step.input_payload.as_deref() {
        prompt.push_str(&format!("input_payload\t{}\n", payload));
    }
    prompt.push_str("checklist\n");
    for item in checklist {
        prompt.push_str(&format!("- {}\n", item));
    }
    prompt.push_str("note\tNo automatic desktop GUI execution is performed by this command.\n");
    prompt
}

fn append_user_note_to_payload(
    input_payload: Option<&str>,
    note: &str,
    created_at: &str,
) -> Result<String, AppError> {
    let mut payload = match input_payload
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        Some(raw) if raw.starts_with('{') => {
            serde_json::from_str::<Value>(raw).unwrap_or_else(|_| json!({ "raw_input": raw }))
        }
        Some(raw) => json!({ "raw_input": raw }),
        None => json!({}),
    };

    if !payload.is_object() {
        payload = json!({ "raw_input": payload });
    }

    let object = payload
        .as_object_mut()
        .ok_or_else(|| AppError::runtime("failed to build execution step note payload"))?;
    let notes = object
        .entry("user_notes")
        .or_insert_with(|| Value::Array(Vec::new()));
    if !notes.is_array() {
        *notes = Value::Array(Vec::new());
    }
    notes
        .as_array_mut()
        .ok_or_else(|| AppError::runtime("failed to append execution step note"))?
        .push(json!({
            "note": note,
            "created_at": created_at,
        }));
    object.insert(
        "latest_user_note".to_string(),
        Value::String(note.to_string()),
    );

    serde_json::to_string(&payload).map_err(AppError::from_json_error)
}

#[tauri::command]
pub fn execution_pause_step(
    db: State<'_, Database>,
    request: ExecutionStepActionRequest,
) -> Result<ExecutionStep, AppError> {
    let service = ExecutionServiceImpl::new(db.inner().clone());
    service.pause_step(&request.id)
}

#[tauri::command]
pub fn execution_complete_step(
    db: State<'_, Database>,
    request: ExecutionStepActionRequest,
) -> Result<ExecutionStep, AppError> {
    let service = ExecutionServiceImpl::new(db.inner().clone());
    let step = service.complete_step(&request.id, request.output_summary)?;
    record_run_event(
        db.inner(),
        &step.mission_id,
        &step.run_id,
        "step_completed",
        &format!("Completed step: {}", step.title),
        step.output_summary.clone(),
    )?;
    Ok(step)
}

fn parse_cli_payload(input_payload: Option<&str>) -> Result<ExecutionCliPayload, AppError> {
    let raw = input_payload
        .map(str::trim)
        .filter(|payload| !payload.is_empty())
        .ok_or_else(|| AppError::validation("cli step input_payload is required"))?;
    let payload: ExecutionCliPayload = if raw.starts_with('{') {
        serde_json::from_str(raw).map_err(AppError::from_json_error)?
    } else {
        ExecutionCliPayload {
            command: raw.to_string(),
            cwd: None,
        }
    };
    if payload.command.trim().is_empty() {
        return Err(AppError::validation("cli command cannot be empty"));
    }
    Ok(ExecutionCliPayload {
        command: payload.command.trim().to_string(),
        cwd: payload.cwd,
    })
}

struct CliCommandOutput {
    status: i32,
    stdout: String,
    stderr: String,
}

fn run_shell_command(command: &str, cwd: Option<&str>) -> Result<CliCommandOutput, AppError> {
    let mut process = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", command]);
        cmd
    };
    if let Some(cwd) = cwd {
        process.current_dir(cwd);
    }
    let output = process
        .output()
        .map_err(|err| AppError::runtime(format!("Failed to run CLI step: {}", err)))?;
    Ok(CliCommandOutput {
        status: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn summarize_cli_output(status: i32, stdout: &str, stderr: &str) -> String {
    let mut parts = vec![format!("exit_code={}", status)];
    if !stdout.is_empty() {
        parts.push(format!("stdout={}", truncate_output(stdout)));
    }
    if !stderr.is_empty() {
        parts.push(format!("stderr={}", truncate_output(stderr)));
    }
    parts.join("\n")
}

fn truncate_output(value: &str) -> String {
    const MAX_OUTPUT_CHARS: usize = 4_000;
    if value.chars().count() <= MAX_OUTPUT_CHARS {
        return value.to_string();
    }
    let truncated = value.chars().take(MAX_OUTPUT_CHARS).collect::<String>();
    format!("{}...[truncated]", truncated)
}

#[tauri::command]
pub fn execution_retry_step(
    db: State<'_, Database>,
    request: ExecutionStepActionRequest,
) -> Result<ExecutionStep, AppError> {
    let service = ExecutionServiceImpl::new(db.inner().clone());
    service.retry_step(&request.id)
}

#[tauri::command]
pub fn execution_resume_step(
    db: State<'_, Database>,
    request: ExecutionStepActionRequest,
) -> Result<ExecutionStep, AppError> {
    let service = ExecutionServiceImpl::new(db.inner().clone());
    service.resume_step(&request.id)
}

#[tauri::command]
pub fn execution_rerun_step(
    db: State<'_, Database>,
    request: ExecutionStepActionRequest,
) -> Result<ExecutionStep, AppError> {
    let service = ExecutionServiceImpl::new(db.inner().clone());
    service.rerun_step(&request.id)
}

#[tauri::command]
pub fn execution_confirm_skip_step(
    db: State<'_, Database>,
    request: ExecutionStepActionRequest,
) -> Result<ExecutionStep, AppError> {
    let service = ExecutionServiceImpl::new(db.inner().clone());
    service.confirm_skip_step(&request.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{CreateExecutionStepInput, RiskLevel};
    use chrono::Utc;

    fn seed_mission_and_run(db: &Database, mission_id: &str, run_id: &str) {
        let now = Utc::now().to_rfc3339();
        db.execute(
            "INSERT OR IGNORE INTO missions (
                id, title, goal, constraints_json, success_criteria_json,
                status, priority, pinned, created_at, updated_at, last_activity_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &mission_id as &dyn rusqlite::ToSql,
                &format!("mission-{mission_id}"),
                &"执行批注测试",
                &"[]",
                &"[]",
                &"executing",
                &"medium",
                &0_i64,
                &now,
                &now,
                &now,
            ],
        )
        .expect("mission should seed");
        db.execute(
            "INSERT OR IGNORE INTO runs (id, mission_id, type, status) VALUES (?, ?, ?, ?)",
            &[
                &run_id as &dyn rusqlite::ToSql,
                &mission_id,
                &"execution",
                &"queued",
            ],
        )
        .expect("run should seed");
    }

    fn create_step(db: &Database, status: ExecutionStepStatus) -> ExecutionStep {
        seed_mission_and_run(db, "mission-note", "run-note");
        let repo = ExecutionRepository::new(db.clone());
        repo.create(
            CreateExecutionStepInput {
                mission_id: "mission-note".to_string(),
                run_id: "run-note".to_string(),
                title: "执行前检查".to_string(),
                mode: ExecutionMode::Cli,
                risk_level: RiskLevel::Low,
                input_payload: Some("{\"command\":\"echo before\"}".to_string()),
            },
            status,
        )
        .expect("step should create")
    }

    fn create_step_with_mode(
        db: &Database,
        status: ExecutionStepStatus,
        mode: ExecutionMode,
        risk_level: RiskLevel,
    ) -> ExecutionStep {
        seed_mission_and_run(db, "mission-desktop", "run-desktop");
        let repo = ExecutionRepository::new(db.clone());
        repo.create(
            CreateExecutionStepInput {
                mission_id: "mission-desktop".to_string(),
                run_id: "run-desktop".to_string(),
                title: "Open target app".to_string(),
                mode,
                risk_level,
                input_payload: Some(
                    "{\"app\":\"Calendar\",\"action\":\"create event\"}".to_string(),
                ),
            },
            status,
        )
        .expect("step should create")
    }

    #[test]
    fn list_desktop_handoff_queue_returns_desktop_steps_with_prepared_flag() {
        let db = Database::in_memory().expect("database should initialize");
        let pending = create_step_with_mode(
            &db,
            ExecutionStepStatus::Pending,
            ExecutionMode::Desktop,
            RiskLevel::Medium,
        );
        let prepared = create_step_with_mode(
            &db,
            ExecutionStepStatus::AwaitingApproval,
            ExecutionMode::Desktop,
            RiskLevel::High,
        );
        execution_prepare_desktop_handoff_for_db(
            &db,
            ExecutionPrepareDesktopHandoffRequest {
                id: prepared.id.clone(),
            },
        )
        .expect("handoff should prepare");

        let queue = execution_list_desktop_handoff_queue_for_db(
            &db,
            ExecutionDesktopHandoffQueueRequest {
                mission_id: Some("mission-desktop".to_string()),
            },
        )
        .expect("queue should load");

        assert_eq!(queue.len(), 2);
        assert!(
            queue
                .iter()
                .any(|item| item.step.id == pending.id && !item.handoff_prepared)
        );
        assert!(
            queue
                .iter()
                .any(|item| item.step.id == prepared.id && item.handoff_prepared)
        );
    }

    #[test]
    fn mark_desktop_handoff_reviewed_updates_queue_review_state_for_exact_step() {
        let db = Database::in_memory().expect("database should initialize");
        let reviewed = create_step_with_mode(
            &db,
            ExecutionStepStatus::AwaitingApproval,
            ExecutionMode::Desktop,
            RiskLevel::High,
        );
        let untouched = create_step_with_mode(
            &db,
            ExecutionStepStatus::Pending,
            ExecutionMode::Desktop,
            RiskLevel::Medium,
        );

        execution_prepare_desktop_handoff_for_db(
            &db,
            ExecutionPrepareDesktopHandoffRequest {
                id: reviewed.id.clone(),
            },
        )
        .expect("reviewed step should prepare");
        execution_prepare_desktop_handoff_for_db(
            &db,
            ExecutionPrepareDesktopHandoffRequest {
                id: untouched.id.clone(),
            },
        )
        .expect("untouched step should prepare");

        execution_mark_desktop_handoff_reviewed_for_db(
            &db,
            ExecutionMarkDesktopHandoffReviewedRequest {
                run_id: reviewed.run_id.clone(),
                step_id: reviewed.id.clone(),
                review_note: Some("用户已核对窗口与输入".to_string()),
            },
        )
        .expect("review event should record");

        let queue = execution_list_desktop_handoff_queue_for_db(
            &db,
            ExecutionDesktopHandoffQueueRequest {
                mission_id: Some("mission-desktop".to_string()),
            },
        )
        .expect("queue should load");

        let reviewed_item = queue
            .iter()
            .find(|item| item.step.id == reviewed.id)
            .expect("reviewed item should exist");
        assert!(reviewed_item.handoff_reviewed);
        assert_eq!(reviewed_item.reviewed_event_count, 1);
        assert!(reviewed_item.latest_reviewed_at.is_some());

        let untouched_item = queue
            .iter()
            .find(|item| item.step.id == untouched.id)
            .expect("untouched item should exist");
        assert!(!untouched_item.handoff_reviewed);
        assert_eq!(untouched_item.reviewed_event_count, 0);
        assert_eq!(untouched_item.latest_reviewed_at, None);

        let payload_json = db
            .query_row(
                "SELECT payload_json
                 FROM run_events
                 WHERE run_id = ?1 AND event_type = 'desktop_handoff_reviewed'
                 ORDER BY datetime(created_at) DESC, rowid DESC
                 LIMIT 1",
                &[&reviewed.run_id as &dyn rusqlite::ToSql],
                |row| row.get::<_, String>(0),
            )
            .expect("review payload should exist");
        let payload: Value = serde_json::from_str(&payload_json).expect("payload should be json");
        assert_eq!(
            payload.get("step_id").and_then(Value::as_str),
            Some(reviewed.id.as_str())
        );
        assert_eq!(
            payload.get("note").and_then(Value::as_str),
            Some("用户已核对窗口与输入")
        );
    }

    #[test]
    fn prepare_desktop_handoff_exports_manual_runtime_instructions_and_records_event() {
        let db = Database::in_memory().expect("database should initialize");
        let step = create_step_with_mode(
            &db,
            ExecutionStepStatus::AwaitingApproval,
            ExecutionMode::Desktop,
            RiskLevel::High,
        );

        let handoff = execution_prepare_desktop_handoff_for_db(
            &db,
            ExecutionPrepareDesktopHandoffRequest {
                id: step.id.clone(),
            },
        )
        .expect("desktop handoff should prepare");

        assert_eq!(handoff.step_id, step.id);
        assert!(!handoff.automatic_execution);
        assert!(
            handoff
                .reason
                .contains("Desktop GUI runtime is not connected")
        );
        assert!(handoff.handoff_prompt.contains("Open target app"));
        assert!(handoff.handoff_prompt.contains("Calendar"));
        assert!(
            handoff
                .checklist
                .iter()
                .any(|item| item.contains("approval"))
        );

        let event_count = db
            .query_row(
                "SELECT COUNT(*) FROM run_events WHERE run_id = ?1 AND event_type = 'desktop_handoff_prepared'",
                &[&step.run_id as &dyn rusqlite::ToSql],
                |row| row.get::<_, i64>(0),
            )
            .expect("event count should query");
        assert_eq!(event_count, 1);
    }

    #[test]
    fn prepare_desktop_handoff_rejects_non_desktop_steps() {
        let db = Database::in_memory().expect("database should initialize");
        let step = create_step(&db, ExecutionStepStatus::Pending);

        let err = execution_prepare_desktop_handoff_for_db(
            &db,
            ExecutionPrepareDesktopHandoffRequest { id: step.id },
        )
        .expect_err("cli steps should not prepare desktop handoff");

        assert_eq!(err.code, "validation_error");
    }

    #[test]
    fn add_step_note_appends_user_note_and_records_event() {
        let db = Database::in_memory().expect("database should initialize");
        let step = create_step(&db, ExecutionStepStatus::Pending);

        let updated = execution_add_step_note_for_db(
            &db,
            ExecutionStepNoteRequest {
                id: step.id.clone(),
                note: "确认客户窗口后再执行".to_string(),
                pause_before_continue: None,
            },
        )
        .expect("note should save");

        assert_eq!(updated.status, ExecutionStepStatus::Pending);
        let payload = updated.input_payload.expect("input payload should persist");
        assert!(payload.contains("确认客户窗口后再执行"));
        assert!(payload.contains("user_notes"));

        let event_count = db
            .query_row(
                "SELECT COUNT(*) FROM run_events WHERE run_id = ? AND event_type = 'step_note_added'",
                &[&step.run_id as &dyn rusqlite::ToSql],
                |row| row.get::<_, i64>(0),
            )
            .expect("event count should query");
        assert_eq!(event_count, 1);
    }

    #[test]
    fn add_step_note_pauses_running_step_before_it_continues() {
        let db = Database::in_memory().expect("database should initialize");
        let step = create_step(&db, ExecutionStepStatus::Running);

        let updated = execution_add_step_note_for_db(
            &db,
            ExecutionStepNoteRequest {
                id: step.id.clone(),
                note: "暂停，先等待人工复核".to_string(),
                pause_before_continue: Some(true),
            },
        )
        .expect("note should pause running step");

        assert_eq!(updated.status, ExecutionStepStatus::Paused);
        assert!(
            updated
                .input_payload
                .as_deref()
                .unwrap_or_default()
                .contains("暂停，先等待人工复核")
        );
    }
}
