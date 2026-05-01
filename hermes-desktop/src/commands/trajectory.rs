//! Trajectory dataset export commands

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use tauri::State;
use uuid::Uuid;

use crate::backend::{AppError, Database};

const TRAJECTORY_RL_DEFAULT_EPOCHS: usize = 5;
const TRAJECTORY_RL_MAX_EPOCHS: usize = 50;
const TRAJECTORY_RL_DEFAULT_ALPHA: f64 = 0.25;
const TRAJECTORY_RL_DEFAULT_GAMMA: f64 = 0.9;
const TRAJECTORY_RL_TRAINING_JOBS_KEY: &str = "trajectory.rl_training_jobs";
const TRAJECTORY_RL_TRAINING_JOB_LIMIT: usize = 20;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TrajectoryExportRequest {
    pub mission_id: Option<String>,
    pub include_session_messages: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TrajectoryDatasetExport {
    pub schema_version: u32,
    pub exported_at: String,
    pub mission_id: Option<String>,
    pub item_count: usize,
    pub jsonl: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrajectoryRlTrainingRequest {
    pub jsonl: String,
    pub epochs: Option<usize>,
    pub alpha: Option<f64>,
    pub gamma: Option<f64>,
    pub job_name: Option<String>,
    pub target_remote_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TrajectoryRlTrainingJobListRequest {
    pub limit: Option<usize>,
    pub target_remote_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrajectoryRlPolicyEntry {
    pub state: String,
    pub action: String,
    pub q_value: f64,
    pub visits: usize,
    pub average_reward: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TrajectoryRlTrainingResult {
    pub job_id: String,
    pub job_name: Option<String>,
    #[serde(default)]
    pub target_remote_user_id: Option<String>,
    pub trained_at: String,
    pub input_line_count: usize,
    pub valid_transition_count: usize,
    pub invalid_line_count: usize,
    pub episode_count: usize,
    pub epochs: usize,
    pub alpha: f64,
    pub gamma: f64,
    pub average_reward: f64,
    #[serde(default)]
    pub policy: Vec<TrajectoryRlPolicyEntry>,
    pub artifact_json: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq)]
struct TrajectoryRlObservation {
    trajectory_id: String,
    timestamp: String,
    state: String,
    action: String,
    reward: f64,
}

#[derive(Debug, Clone)]
struct TrajectoryLine {
    sort_at: String,
    value: Value,
}

#[tauri::command]
pub fn trajectory_export_dataset(
    db: State<'_, Database>,
    request: TrajectoryExportRequest,
) -> Result<TrajectoryDatasetExport, AppError> {
    export_trajectory_dataset_for_db(db.inner(), request)
}

#[tauri::command]
pub fn trajectory_run_local_rl_training(
    db: State<'_, Database>,
    request: TrajectoryRlTrainingRequest,
) -> Result<TrajectoryRlTrainingResult, AppError> {
    run_local_rl_training_for_db(db.inner(), request)
}

#[tauri::command]
pub fn trajectory_list_local_rl_training_jobs(
    db: State<'_, Database>,
    request: TrajectoryRlTrainingJobListRequest,
) -> Result<Vec<TrajectoryRlTrainingResult>, AppError> {
    list_local_rl_training_jobs_for_db(db.inner(), request)
}

pub fn export_trajectory_dataset_for_db(
    db: &Database,
    request: TrajectoryExportRequest,
) -> Result<TrajectoryDatasetExport, AppError> {
    let mission_id = normalize_optional_mission_id(request.mission_id)?;
    if let Some(id) = mission_id.as_deref() {
        ensure_mission_exists(db, id)?;
    }

    let mut lines = Vec::new();
    append_run_lines(db, mission_id.as_deref(), &mut lines)?;
    append_execution_step_lines(db, mission_id.as_deref(), &mut lines)?;
    append_run_event_lines(db, mission_id.as_deref(), &mut lines)?;
    if request.include_session_messages.unwrap_or(true) {
        append_session_message_lines(db, mission_id.as_deref(), &mut lines)?;
    }

    lines.sort_by(|left, right| {
        left.sort_at
            .cmp(&right.sort_at)
            .then_with(|| kind_rank(&left.value).cmp(&kind_rank(&right.value)))
            .then_with(|| stable_id(&left.value).cmp(&stable_id(&right.value)))
    });

    let mut jsonl = String::new();
    for line in &lines {
        jsonl.push_str(&serde_json::to_string(&line.value).map_err(AppError::from_json_error)?);
        jsonl.push('\n');
    }

    Ok(TrajectoryDatasetExport {
        schema_version: 1,
        exported_at: Utc::now().to_rfc3339(),
        mission_id,
        item_count: lines.len(),
        jsonl,
    })
}

fn run_local_rl_training_for_db(
    db: &Database,
    request: TrajectoryRlTrainingRequest,
) -> Result<TrajectoryRlTrainingResult, AppError> {
    let epochs = normalize_rl_epochs(request.epochs);
    let alpha = normalize_rl_rate(request.alpha, TRAJECTORY_RL_DEFAULT_ALPHA, "alpha")?;
    let gamma = normalize_rl_rate(request.gamma, TRAJECTORY_RL_DEFAULT_GAMMA, "gamma")?;
    let job_name = request
        .job_name
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let target_remote_user_id = normalize_optional_trimmed_string(request.target_remote_user_id);
    let (input_line_count, invalid_line_count, observations) =
        parse_rl_observations(&request.jsonl);
    if observations.is_empty() {
        return Err(AppError::validation(
            "at least one valid trajectory transition is required for local RL training",
        ));
    }

    let mut episodes = BTreeMap::<String, Vec<TrajectoryRlObservation>>::new();
    for observation in observations {
        episodes
            .entry(observation.trajectory_id.clone())
            .or_default()
            .push(observation);
    }
    for episode in episodes.values_mut() {
        episode.sort_by(|left, right| {
            left.timestamp
                .cmp(&right.timestamp)
                .then_with(|| left.state.cmp(&right.state))
                .then_with(|| left.action.cmp(&right.action))
        });
    }

    let mut q_values = BTreeMap::<(String, String), f64>::new();
    let mut visits = BTreeMap::<(String, String), usize>::new();
    let mut reward_totals = BTreeMap::<(String, String), f64>::new();
    let mut total_reward = 0.0;
    let mut valid_transition_count = 0_usize;

    for episode in episodes.values() {
        for observation in episode {
            let key = (observation.state.clone(), observation.action.clone());
            *visits.entry(key.clone()).or_insert(0) += 1;
            *reward_totals.entry(key).or_insert(0.0) += observation.reward;
            total_reward += observation.reward;
            valid_transition_count += 1;
        }
    }

    for _ in 0..epochs {
        for episode in episodes.values() {
            for (index, observation) in episode.iter().enumerate() {
                let key = (observation.state.clone(), observation.action.clone());
                let next_max = episode
                    .get(index + 1)
                    .map(|next| max_q_for_state(&q_values, &next.state))
                    .unwrap_or(0.0);
                let current = *q_values.get(&key).unwrap_or(&0.0);
                let target = observation.reward + gamma * next_max;
                let updated = current + alpha * (target - current);
                q_values.insert(key, round_rl_value(updated));
            }
        }
    }

    let mut policy = q_values
        .iter()
        .map(|((state, action), q_value)| {
            let key = (state.clone(), action.clone());
            let visit_count = *visits.get(&key).unwrap_or(&0);
            let reward_total = *reward_totals.get(&key).unwrap_or(&0.0);
            TrajectoryRlPolicyEntry {
                state: state.clone(),
                action: action.clone(),
                q_value: *q_value,
                visits: visit_count,
                average_reward: if visit_count == 0 {
                    0.0
                } else {
                    round_rl_value(reward_total / visit_count as f64)
                },
            }
        })
        .collect::<Vec<_>>();
    policy.sort_by(|left, right| {
        left.state
            .cmp(&right.state)
            .then_with(|| right.q_value.total_cmp(&left.q_value))
            .then_with(|| left.action.cmp(&right.action))
    });

    let job_id = Uuid::new_v4().to_string();
    let trained_at = Utc::now().to_rfc3339();
    let average_reward = round_rl_value(total_reward / valid_transition_count as f64);
    let artifact_value = json!({
        "schema_version": 1,
        "algorithm": "tabular_td_q_learning",
        "job_id": job_id.clone(),
        "target_remote_user_id": target_remote_user_id.clone(),
        "epochs": epochs,
        "alpha": alpha,
        "gamma": gamma,
        "policy": policy.clone(),
    });
    let artifact_json =
        serde_json::to_string_pretty(&artifact_value).map_err(AppError::from_json_error)?;
    let summary = format!(
        "Local tabular RL training updated {} state-action value(s) from {} transition(s) across {} episode(s).",
        policy.len(),
        valid_transition_count,
        episodes.len()
    );
    let result = TrajectoryRlTrainingResult {
        job_id,
        job_name,
        target_remote_user_id,
        trained_at,
        input_line_count,
        valid_transition_count,
        invalid_line_count,
        episode_count: episodes.len(),
        epochs,
        alpha,
        gamma,
        average_reward,
        policy,
        artifact_json,
        summary,
    };
    persist_rl_training_job(db, &result)?;
    Ok(result)
}

fn normalize_rl_epochs(epochs: Option<usize>) -> usize {
    epochs
        .unwrap_or(TRAJECTORY_RL_DEFAULT_EPOCHS)
        .clamp(1, TRAJECTORY_RL_MAX_EPOCHS)
}

fn normalize_optional_trimmed_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_rl_rate(value: Option<f64>, default: f64, label: &str) -> Result<f64, AppError> {
    let value = value.unwrap_or(default);
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        return Err(AppError::validation(format!(
            "{} must be a finite value between 0 and 1",
            label
        )));
    }
    Ok(value)
}

fn parse_rl_observations(jsonl: &str) -> (usize, usize, Vec<TrajectoryRlObservation>) {
    let mut input_line_count = 0_usize;
    let mut invalid_line_count = 0_usize;
    let mut observations = Vec::new();

    for (line_index, line) in jsonl.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        input_line_count += 1;
        let value = match serde_json::from_str::<Value>(line) {
            Ok(value) => value,
            Err(_) => {
                invalid_line_count += 1;
                continue;
            }
        };
        match rl_observation_from_value(&value, line_index) {
            Some(observation) => observations.push(observation),
            None => invalid_line_count += 1,
        }
    }

    (input_line_count, invalid_line_count, observations)
}

fn rl_observation_from_value(value: &Value, line_index: usize) -> Option<TrajectoryRlObservation> {
    let kind = value.get("kind")?.as_str()?.trim();
    if kind.is_empty() {
        return None;
    }
    let trajectory_id = value
        .get("trajectory_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| format!("line:{}", line_index + 1));
    let timestamp = value
        .get("timestamp")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let state = rl_state_from_value(kind, value);
    let action = rl_action_from_value(kind, value);
    if state.is_empty() || action.is_empty() {
        return None;
    }
    Some(TrajectoryRlObservation {
        trajectory_id,
        timestamp,
        state,
        action,
        reward: rl_reward_from_value(value),
    })
}

fn rl_state_from_value(kind: &str, value: &Value) -> String {
    match kind {
        "run" => format!(
            "run:{}:{}",
            string_field(value, "run_type", "unknown"),
            string_field(value, "status", "unknown")
        ),
        "execution_step" => format!(
            "step:{}:{}",
            string_field(value, "mode", "unknown"),
            string_field(value, "status", "unknown")
        ),
        "run_event" => format!("event:{}", string_field(value, "event_type", "unknown")),
        "session_message" => format!("message:{}", string_field(value, "role", "unknown")),
        other => format!("{}:unknown", other),
    }
}

fn rl_action_from_value(kind: &str, value: &Value) -> String {
    match kind {
        "run" => string_field(value, "run_type", "run"),
        "execution_step" => string_field(value, "title", &string_field(value, "mode", "step")),
        "run_event" => string_field(value, "event_type", "event"),
        "session_message" => string_field(value, "role", "message"),
        other => other.to_string(),
    }
}

fn string_field(value: &Value, field: &str, fallback: &str) -> String {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn rl_reward_from_value(value: &Value) -> f64 {
    if let Some(reward_hint) = value.get("reward_hint")
        && let Some(reward) = reward_value(reward_hint)
    {
        return reward;
    }
    match string_field(value, "status", "").as_str() {
        "completed" | "succeeded" => 1.0,
        "failed" => -1.0,
        "cancelled" => -0.5,
        "running" => 0.1,
        _ => 0.0,
    }
}

fn reward_value(value: &Value) -> Option<f64> {
    if let Some(number) = value.as_f64() {
        return Some(number);
    }
    if let Some(flag) = value.as_bool() {
        return Some(if flag { 1.0 } else { 0.0 });
    }
    for field in ["reward", "value", "score"] {
        if let Some(number) = value.get(field).and_then(Value::as_f64) {
            return Some(number);
        }
    }
    None
}

fn max_q_for_state(q_values: &BTreeMap<(String, String), f64>, state: &str) -> f64 {
    q_values
        .iter()
        .filter_map(|((candidate_state, _), value)| {
            if candidate_state == state {
                Some(*value)
            } else {
                None
            }
        })
        .fold(0.0, f64::max)
}

fn round_rl_value(value: f64) -> f64 {
    (value * 1_000.0).round() / 1_000.0
}

fn persist_rl_training_job(
    db: &Database,
    result: &TrajectoryRlTrainingResult,
) -> Result<(), AppError> {
    let stored = db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&TRAJECTORY_RL_TRAINING_JOBS_KEY as &dyn rusqlite::ToSql],
        |row| row.get::<_, String>(0),
    );
    let mut jobs = match stored {
        Ok(value_json) => {
            serde_json::from_str::<Vec<Value>>(&value_json).map_err(AppError::from_json_error)?
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Vec::new(),
        Err(error) => {
            return Err(AppError::storage(format!(
                "Failed to load local RL training jobs: {}",
                error
            )));
        }
    };
    jobs.insert(
        0,
        serde_json::to_value(result).map_err(AppError::from_json_error)?,
    );
    jobs.truncate(TRAJECTORY_RL_TRAINING_JOB_LIMIT);
    let value_json = serde_json::to_string(&jobs).map_err(AppError::from_json_error)?;
    let updated_at = Utc::now().to_rfc3339();
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
        &[&TRAJECTORY_RL_TRAINING_JOBS_KEY, &value_json, &updated_at],
    )?;
    Ok(())
}

fn list_local_rl_training_jobs_for_db(
    db: &Database,
    request: TrajectoryRlTrainingJobListRequest,
) -> Result<Vec<TrajectoryRlTrainingResult>, AppError> {
    let target_remote_user_id = normalize_optional_trimmed_string(request.target_remote_user_id);
    let stored = db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&TRAJECTORY_RL_TRAINING_JOBS_KEY as &dyn rusqlite::ToSql],
        |row| row.get::<_, String>(0),
    );
    let mut jobs = match stored {
        Ok(value_json) => serde_json::from_str::<Vec<TrajectoryRlTrainingResult>>(&value_json)
            .map_err(AppError::from_json_error)?,
        Err(rusqlite::Error::QueryReturnedNoRows) => Vec::new(),
        Err(error) => {
            return Err(AppError::storage(format!(
                "Failed to load local RL training jobs: {}",
                error
            )));
        }
    };
    if let Some(target_remote_user_id) = target_remote_user_id.as_deref() {
        jobs.retain(|job| job.target_remote_user_id.as_deref() == Some(target_remote_user_id));
    }
    let limit = request
        .limit
        .unwrap_or(TRAJECTORY_RL_TRAINING_JOB_LIMIT)
        .clamp(1, TRAJECTORY_RL_TRAINING_JOB_LIMIT);
    jobs.truncate(limit);
    Ok(jobs)
}

fn normalize_optional_mission_id(mission_id: Option<String>) -> Result<Option<String>, AppError> {
    match mission_id.map(|value| value.trim().to_string()) {
        Some(value) if value.is_empty() => Err(AppError::validation("mission id cannot be empty")),
        Some(value) => Ok(Some(value)),
        None => Ok(None),
    }
}

fn ensure_mission_exists(db: &Database, mission_id: &str) -> Result<(), AppError> {
    let count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM missions WHERE id = ?1",
            &[&mission_id as &dyn rusqlite::ToSql],
            |row| row.get(0),
        )
        .map_err(|err| AppError::storage(format!("Failed to validate mission: {}", err)))?;
    if count == 0 {
        return Err(AppError::validation("mission not found"));
    }
    Ok(())
}

fn append_run_lines(
    db: &Database,
    mission_id: Option<&str>,
    lines: &mut Vec<TrajectoryLine>,
) -> Result<(), AppError> {
    db.with_connection(|conn| {
        let mut stmt = if mission_id.is_some() {
            conn.prepare(
                "SELECT id, mission_id, type, status, started_at, finished_at, summary, error_message
                 FROM runs WHERE mission_id = ?1",
            )?
        } else {
            conn.prepare(
                "SELECT id, mission_id, type, status, started_at, finished_at, summary, error_message
                 FROM runs",
            )?
        };
        let mut rows = match mission_id {
            Some(id) => stmt.query([id])?,
            None => stmt.query([])?,
        };

        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let mission_id: String = row.get(1)?;
            let run_type: String = row.get(2)?;
            let status: String = row.get(3)?;
            let started_at: Option<String> = row.get(4)?;
            let finished_at: Option<String> = row.get(5)?;
            let summary: Option<String> = row.get(6)?;
            let error_message: Option<String> = row.get(7)?;
            let timestamp = finished_at
                .clone()
                .or_else(|| started_at.clone())
                .unwrap_or_default();
            lines.push(TrajectoryLine {
                sort_at: timestamp.clone(),
                value: json!({
                    "schema_version": 1,
                    "kind": "run",
                    "trajectory_id": trajectory_id(&mission_id, &id),
                    "mission_id": mission_id,
                    "run_id": id,
                    "timestamp": timestamp,
                    "run_type": run_type,
                    "status": status,
                    "started_at": started_at,
                    "finished_at": finished_at,
                    "summary": summary,
                    "error_message": error_message,
                }),
            });
        }
        Ok(())
    })
}

fn append_execution_step_lines(
    db: &Database,
    mission_id: Option<&str>,
    lines: &mut Vec<TrajectoryLine>,
) -> Result<(), AppError> {
    db.with_connection(|conn| {
        let mut stmt = if mission_id.is_some() {
            conn.prepare(
                "SELECT id, mission_id, run_id, title, mode, risk_level, status,
                        input_payload, output_summary, created_at, updated_at
                 FROM execution_steps WHERE mission_id = ?1",
            )?
        } else {
            conn.prepare(
                "SELECT id, mission_id, run_id, title, mode, risk_level, status,
                        input_payload, output_summary, created_at, updated_at
                 FROM execution_steps",
            )?
        };
        let mut rows = match mission_id {
            Some(id) => stmt.query([id])?,
            None => stmt.query([])?,
        };

        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let mission_id: String = row.get(1)?;
            let run_id: String = row.get(2)?;
            let title: String = row.get(3)?;
            let mode: String = row.get(4)?;
            let risk_level: String = row.get(5)?;
            let status: String = row.get(6)?;
            let input_payload: Option<String> = row.get(7)?;
            let output_summary: Option<String> = row.get(8)?;
            let created_at: String = row.get(9)?;
            let updated_at: String = row.get(10)?;
            lines.push(TrajectoryLine {
                sort_at: updated_at.clone(),
                value: json!({
                    "schema_version": 1,
                    "kind": "execution_step",
                    "trajectory_id": trajectory_id(&mission_id, &run_id),
                    "mission_id": mission_id,
                    "run_id": run_id,
                    "step_id": id,
                    "timestamp": updated_at,
                    "created_at": created_at,
                    "title": title,
                    "mode": mode,
                    "risk_level": risk_level,
                    "status": status,
                    "input_payload": parse_optional_json(input_payload),
                    "output_summary": output_summary,
                }),
            });
        }
        Ok(())
    })
}

fn append_run_event_lines(
    db: &Database,
    mission_id: Option<&str>,
    lines: &mut Vec<TrajectoryLine>,
) -> Result<(), AppError> {
    db.with_connection(|conn| {
        let mut stmt = if mission_id.is_some() {
            conn.prepare(
                "SELECT id, run_id, mission_id, event_type, message, payload_json, created_at
                 FROM run_events WHERE mission_id = ?1",
            )?
        } else {
            conn.prepare(
                "SELECT id, run_id, mission_id, event_type, message, payload_json, created_at
                 FROM run_events",
            )?
        };
        let mut rows = match mission_id {
            Some(id) => stmt.query([id])?,
            None => stmt.query([])?,
        };

        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let run_id: String = row.get(1)?;
            let mission_id: String = row.get(2)?;
            let event_type: String = row.get(3)?;
            let message: String = row.get(4)?;
            let payload_json: Option<String> = row.get(5)?;
            let created_at: String = row.get(6)?;
            lines.push(TrajectoryLine {
                sort_at: created_at.clone(),
                value: json!({
                    "schema_version": 1,
                    "kind": "run_event",
                    "trajectory_id": trajectory_id(&mission_id, &run_id),
                    "mission_id": mission_id,
                    "run_id": run_id,
                    "event_id": id,
                    "timestamp": created_at,
                    "event_type": event_type,
                    "message": message,
                    "payload": parse_optional_json(payload_json),
                }),
            });
        }
        Ok(())
    })
}

fn append_session_message_lines(
    db: &Database,
    mission_id: Option<&str>,
    lines: &mut Vec<TrajectoryLine>,
) -> Result<(), AppError> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT m.id, m.session_id, m.role, m.content, m.metadata_json, m.created_at,
                    s.source, s.title, s.model_name
             FROM session_messages m
             INNER JOIN sessions s ON s.id = m.session_id",
        )?;
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            let id: String = row.get(0)?;
            let session_id: String = row.get(1)?;
            let role: String = row.get(2)?;
            let content: String = row.get(3)?;
            let metadata_json: Option<String> = row.get(4)?;
            let created_at: String = row.get(5)?;
            let source: String = row.get(6)?;
            let title: String = row.get(7)?;
            let model_name: Option<String> = row.get(8)?;
            let metadata = parse_optional_json(metadata_json);
            let metadata_mission_id = metadata
                .as_ref()
                .and_then(|value| value.get("mission_id"))
                .and_then(Value::as_str)
                .map(str::to_string);
            if mission_id.is_some_and(|id| metadata_mission_id.as_deref() != Some(id)) {
                continue;
            }
            let metadata_run_id = metadata
                .as_ref()
                .and_then(|value| value.get("run_id"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let trajectory_id = match (metadata_mission_id.as_deref(), metadata_run_id.as_deref()) {
                (Some(mission_id), Some(run_id)) => trajectory_id(mission_id, run_id),
                (Some(mission_id), None) => format!("{}:session:{}", mission_id, session_id),
                (None, _) => format!("session:{}", session_id),
            };
            lines.push(TrajectoryLine {
                sort_at: created_at.clone(),
                value: json!({
                    "schema_version": 1,
                    "kind": "session_message",
                    "trajectory_id": trajectory_id,
                    "mission_id": metadata_mission_id,
                    "run_id": metadata_run_id,
                    "session_id": session_id,
                    "message_id": id,
                    "timestamp": created_at,
                    "role": role,
                    "content": content,
                    "metadata": metadata,
                    "session": {
                        "source": source,
                        "title": title,
                        "model_name": model_name,
                    },
                }),
            });
        }
        Ok(())
    })
}

fn parse_optional_json(value: Option<String>) -> Option<Value> {
    value.map(|raw| serde_json::from_str(&raw).unwrap_or_else(|_| json!({ "raw": raw })))
}

fn trajectory_id(mission_id: &str, run_id: &str) -> String {
    format!("{}:{}", mission_id, run_id)
}

fn kind_rank(value: &Value) -> u8 {
    match value
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default()
    {
        "run" => 0,
        "execution_step" => 1,
        "run_event" => 2,
        "session_message" => 3,
        _ => 9,
    }
}

fn stable_id(value: &Value) -> String {
    value
        .get("run_id")
        .or_else(|| value.get("step_id"))
        .or_else(|| value.get("event_id"))
        .or_else(|| value.get("message_id"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        TrajectoryExportRequest, TrajectoryRlTrainingJobListRequest, TrajectoryRlTrainingRequest,
        export_trajectory_dataset_for_db, list_local_rl_training_jobs_for_db,
        run_local_rl_training_for_db,
    };
    use crate::backend::Database;

    #[test]
    fn trajectory_export_serializes_runs_steps_events_and_messages_as_jsonl() {
        let db = Database::in_memory().expect("db should initialize");
        db.execute(
            "INSERT INTO missions (id, title, goal, status, priority, created_at, updated_at, last_activity_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?6)",
            &[
                &"mission-1" as &dyn rusqlite::ToSql,
                &"Trajectory Mission",
                &"Create trainable evidence",
                &"active",
                &"high",
                &"2026-04-27T00:00:00Z",
            ],
        )
        .expect("mission insert");
        db.execute(
            "INSERT INTO runs (id, mission_id, type, status, started_at, finished_at, summary, error_message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            &[
                &"run-1" as &dyn rusqlite::ToSql,
                &"mission-1",
                &"execution",
                &"completed",
                &"2026-04-27T00:01:00Z",
                &"2026-04-27T00:03:00Z",
                &"Run completed",
                &Option::<String>::None,
            ],
        )
        .expect("run insert");
        db.execute(
            "INSERT INTO execution_steps (id, mission_id, run_id, title, mode, risk_level, status, input_payload, output_summary, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            &[
                &"step-1" as &dyn rusqlite::ToSql,
                &"mission-1",
                &"run-1",
                &"Run CLI command",
                &"cli",
                &"low",
                &"completed",
                &"{\"command\":\"echo ok\"}",
                &"ok",
                &"2026-04-27T00:01:30Z",
                &"2026-04-27T00:02:00Z",
            ],
        )
        .expect("step insert");
        db.execute(
            "INSERT INTO run_events (id, run_id, mission_id, event_type, message, payload_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            &[
                &"event-1" as &dyn rusqlite::ToSql,
                &"run-1",
                &"mission-1",
                &"step_completed",
                &"CLI step completed",
                &"{\"exit_code\":0}",
                &"2026-04-27T00:02:10Z",
            ],
        )
        .expect("event insert");
        db.execute(
            "INSERT INTO sessions (id, source, title, model_name, started_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            &[
                &"session-1" as &dyn rusqlite::ToSql,
                &"cli",
                &"Trajectory session",
                &"gpt-test",
                &"2026-04-27T00:00:10Z",
                &"2026-04-27T00:04:00Z",
            ],
        )
        .expect("session insert");
        db.execute(
            "INSERT INTO session_messages (id, session_id, role, content, metadata_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            &[
                &"message-1" as &dyn rusqlite::ToSql,
                &"session-1",
                &"assistant",
                &"Completed the CLI step",
                &"{\"mission_id\":\"mission-1\",\"run_id\":\"run-1\"}",
                &"2026-04-27T00:03:30Z",
            ],
        )
        .expect("message insert");

        let export = export_trajectory_dataset_for_db(
            &db,
            TrajectoryExportRequest {
                mission_id: Some("mission-1".to_string()),
                include_session_messages: Some(true),
            },
        )
        .expect("trajectory should export");

        assert_eq!(export.schema_version, 1);
        assert_eq!(export.mission_id.as_deref(), Some("mission-1"));
        assert_eq!(export.item_count, 4);
        assert!(export.jsonl.contains("\"kind\":\"run\""));
        assert!(export.jsonl.contains("\"kind\":\"execution_step\""));
        assert!(export.jsonl.contains("\"kind\":\"run_event\""));
        assert!(export.jsonl.contains("\"kind\":\"session_message\""));
        assert!(
            export
                .jsonl
                .contains("\"trajectory_id\":\"mission-1:run-1\"")
        );
    }

    #[test]
    fn trajectory_export_can_exclude_session_messages() {
        let db = Database::in_memory().expect("db should initialize");
        db.execute(
            "INSERT INTO missions (id, title, goal, status, priority, created_at, updated_at, last_activity_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6, ?6)",
            &[
                &"mission-2" as &dyn rusqlite::ToSql,
                &"No Session Export",
                &"Export non-chat signals",
                &"active",
                &"medium",
                &"2026-04-27T00:00:00Z",
            ],
        )
        .expect("mission insert");
        db.execute(
            "INSERT INTO sessions (id, source, title, started_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            &[
                &"session-2" as &dyn rusqlite::ToSql,
                &"cli",
                &"No Session Export",
                &"2026-04-27T00:00:10Z",
                &"2026-04-27T00:04:00Z",
            ],
        )
        .expect("session insert");
        db.execute(
            "INSERT INTO session_messages (id, session_id, role, content, metadata_json, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            &[
                &"message-2" as &dyn rusqlite::ToSql,
                &"session-2",
                &"user",
                &"Do the task",
                &"{\"mission_id\":\"mission-2\"}",
                &"2026-04-27T00:03:30Z",
            ],
        )
        .expect("message insert");

        let export = export_trajectory_dataset_for_db(
            &db,
            TrajectoryExportRequest {
                mission_id: Some("mission-2".to_string()),
                include_session_messages: Some(false),
            },
        )
        .expect("trajectory should export");

        assert_eq!(export.item_count, 0);
        assert!(!export.jsonl.contains("session_message"));
    }

    #[test]
    fn local_rl_training_updates_policy_from_rewarded_trajectory_jsonl_and_persists_job() {
        let db = Database::in_memory().expect("db should initialize");
        let result = run_local_rl_training_for_db(
            &db,
            TrajectoryRlTrainingRequest {
                jsonl: concat!(
                    "{\"kind\":\"run\",\"trajectory_id\":\"episode-1\",\"timestamp\":\"2026-04-27T00:00:00Z\",\"run_type\":\"simulation\",\"status\":\"completed\",\"reward_hint\":1}\n",
                    "{\"kind\":\"execution_step\",\"trajectory_id\":\"episode-1\",\"timestamp\":\"2026-04-27T00:01:00Z\",\"mode\":\"cli\",\"status\":\"completed\",\"title\":\"verify result\",\"reward_hint\":2}\n",
                    "{\"kind\":\"run\",\"trajectory_id\":\"episode-2\",\"timestamp\":\"2026-04-27T00:00:00Z\",\"run_type\":\"execution\",\"status\":\"failed\",\"reward_hint\":-1}\n",
                    "not-json\n"
                )
                .to_string(),
                epochs: Some(4),
                alpha: Some(0.4),
                gamma: Some(0.7),
                job_name: Some("fixture policy".to_string()),
                target_remote_user_id: Some("  future-user-42  ".to_string()),
            },
        )
        .expect("local RL training should run");

        assert_eq!(result.input_line_count, 4);
        assert_eq!(result.invalid_line_count, 1);
        assert_eq!(result.valid_transition_count, 3);
        assert_eq!(result.episode_count, 2);
        assert_eq!(result.epochs, 4);
        assert!(result.average_reward > 0.0);
        assert!(
            result
                .policy
                .iter()
                .any(|entry| entry.action == "verify result")
        );
        assert_eq!(
            result.target_remote_user_id.as_deref(),
            Some("future-user-42")
        );
        assert!(result.artifact_json.contains("q_value"));
        assert!(
            result
                .artifact_json
                .contains("\"target_remote_user_id\": \"future-user-42\"")
        );
        assert!(result.summary.contains("Local tabular RL training"));

        let stored = db
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = 'trajectory.rl_training_jobs'",
                &[],
                |row| row.get::<_, String>(0),
            )
            .expect("training jobs should persist");
        assert!(stored.contains(&result.job_id));
        assert!(stored.contains("\"target_remote_user_id\":\"future-user-42\""));
    }

    #[test]
    fn local_rl_training_jobs_list_returns_persisted_history_newest_first() {
        let db = Database::in_memory().expect("db should initialize");
        let first = run_local_rl_training_for_db(
            &db,
            TrajectoryRlTrainingRequest {
                jsonl: "{\"kind\":\"run\",\"trajectory_id\":\"episode-a\",\"timestamp\":\"2026-04-27T00:00:00Z\",\"run_type\":\"simulation\",\"status\":\"completed\",\"reward_hint\":1}\n".to_string(),
                epochs: Some(2),
                alpha: None,
                gamma: None,
                job_name: Some("first policy".to_string()),
                target_remote_user_id: Some("   ".to_string()),
            },
        )
        .expect("first training job should run");
        let second = run_local_rl_training_for_db(
            &db,
            TrajectoryRlTrainingRequest {
                jsonl: "{\"kind\":\"execution_step\",\"trajectory_id\":\"episode-b\",\"timestamp\":\"2026-04-27T00:01:00Z\",\"mode\":\"desktop\",\"status\":\"completed\",\"title\":\"apply result\",\"reward_hint\":2}\n".to_string(),
                epochs: Some(3),
                alpha: None,
                gamma: None,
                job_name: Some("second policy".to_string()),
                target_remote_user_id: Some("remote-user-b".to_string()),
            },
        )
        .expect("second training job should run");

        let jobs = list_local_rl_training_jobs_for_db(
            &db,
            TrajectoryRlTrainingJobListRequest {
                limit: Some(1),
                target_remote_user_id: None,
            },
        )
        .expect("training job history should list");

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].job_id, second.job_id);
        assert_eq!(jobs[0].job_name.as_deref(), Some("second policy"));
        assert_eq!(
            jobs[0].target_remote_user_id.as_deref(),
            Some("remote-user-b")
        );
        assert!(
            jobs[0]
                .artifact_json
                .contains("\"target_remote_user_id\": \"remote-user-b\"")
        );
        assert_ne!(jobs[0].job_id, first.job_id);

        let all_jobs = list_local_rl_training_jobs_for_db(
            &db,
            TrajectoryRlTrainingJobListRequest {
                limit: Some(8),
                target_remote_user_id: None,
            },
        )
        .expect("full training job history should list");
        assert_eq!(all_jobs.len(), 2);
        assert_eq!(all_jobs[1].job_id, first.job_id);
        assert_eq!(all_jobs[1].target_remote_user_id, None);

        let remote_user_b_jobs = list_local_rl_training_jobs_for_db(
            &db,
            TrajectoryRlTrainingJobListRequest {
                limit: Some(8),
                target_remote_user_id: Some("  remote-user-b  ".to_string()),
            },
        )
        .expect("targeted training job history should list");
        assert_eq!(remote_user_b_jobs.len(), 1);
        assert_eq!(remote_user_b_jobs[0].job_id, second.job_id);

        let unknown_remote_user_jobs = list_local_rl_training_jobs_for_db(
            &db,
            TrajectoryRlTrainingJobListRequest {
                limit: Some(8),
                target_remote_user_id: Some("unknown-remote-user".to_string()),
            },
        )
        .expect("unknown targeted training job history should list empty");
        assert!(unknown_remote_user_jobs.is_empty());
    }

    #[test]
    fn local_rl_training_rejects_jsonl_without_valid_transitions() {
        let db = Database::in_memory().expect("db should initialize");
        let error = run_local_rl_training_for_db(
            &db,
            TrajectoryRlTrainingRequest {
                jsonl: "not-json\n{\"kind\":null}\n".to_string(),
                epochs: None,
                alpha: None,
                gamma: None,
                job_name: None,
                target_remote_user_id: None,
            },
        )
        .expect_err("empty training data should fail");

        assert_eq!(error.code, "validation_error");
        assert!(error.message.contains("valid trajectory transition"));
    }
}
