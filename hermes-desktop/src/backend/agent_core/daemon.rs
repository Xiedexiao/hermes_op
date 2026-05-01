//! Agent engine daemon runtime and heartbeat helpers.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use crate::backend::{AppError, AppResult, Database};

const DEFAULT_HEARTBEAT_INTERVAL_MS: u64 = 1_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EngineHeartbeat {
    pub profile: String,
    pub started_at: String,
    pub last_heartbeat_at: String,
    pub queued_background_runs: u32,
    pub awaiting_approval_steps: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EngineDaemonConfig {
    pub profile: String,
    pub data_dir: PathBuf,
    pub once: bool,
    pub heartbeat_interval_ms: u64,
}

pub fn engine_heartbeat_path(data_dir: &Path) -> PathBuf {
    data_dir.join("engine.heartbeat.json")
}

pub fn read_engine_heartbeat(data_dir: &Path) -> Option<EngineHeartbeat> {
    let path = engine_heartbeat_path(data_dir);
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

pub fn clear_engine_heartbeat(data_dir: &Path) -> AppResult<()> {
    let path = engine_heartbeat_path(data_dir);
    if path.exists() {
        std::fs::remove_file(path)
            .map_err(|err| AppError::io(format!("Failed to remove engine heartbeat: {err}")))?;
    }
    Ok(())
}

pub fn parse_engine_daemon_args<I, T>(args: I) -> Result<Option<EngineDaemonConfig>, String>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args = args
        .into_iter()
        .map(|value| value.into().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    if !args.iter().any(|item| item == "--engine-daemon") {
        return Ok(None);
    }

    let mut profile = "default".to_string();
    let mut data_dir = None::<PathBuf>;
    let mut once = false;
    let mut heartbeat_interval_ms = DEFAULT_HEARTBEAT_INTERVAL_MS;

    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--profile" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--profile requires a value".to_string())?;
                profile = value.trim().to_string();
                index += 2;
            }
            "--data-dir" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--data-dir requires a value".to_string())?;
                data_dir = Some(PathBuf::from(value));
                index += 2;
            }
            "--once" => {
                once = true;
                index += 1;
            }
            "--heartbeat-interval-ms" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| "--heartbeat-interval-ms requires a value".to_string())?;
                heartbeat_interval_ms = value
                    .parse::<u64>()
                    .map_err(|_| "invalid heartbeat interval".to_string())?
                    .max(100);
                index += 2;
            }
            _ => {
                index += 1;
            }
        }
    }

    let data_dir = data_dir.ok_or_else(|| "--data-dir is required".to_string())?;
    if profile.trim().is_empty() {
        return Err("--profile cannot be empty".to_string());
    }

    Ok(Some(EngineDaemonConfig {
        profile,
        data_dir,
        once,
        heartbeat_interval_ms,
    }))
}

pub fn maybe_run_engine_daemon_from_args<I, T>(args: I) -> Result<bool, String>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let Some(config) = parse_engine_daemon_args(args)? else {
        return Ok(false);
    };

    run_engine_daemon(config).map_err(|err| err.message)?;
    Ok(true)
}

pub fn run_engine_daemon(config: EngineDaemonConfig) -> AppResult<()> {
    let started_at = Utc::now().to_rfc3339();
    std::fs::create_dir_all(&config.data_dir)
        .map_err(|err| AppError::io(format!("Failed to create engine data dir: {err}")))?;

    if config.once {
        return run_engine_daemon_tick(&config.data_dir, &config.profile, &started_at);
    }

    loop {
        run_engine_daemon_tick(&config.data_dir, &config.profile, &started_at)?;
        thread::sleep(Duration::from_millis(config.heartbeat_interval_ms));
    }
}

pub fn run_engine_daemon_tick(data_dir: &Path, profile: &str, started_at: &str) -> AppResult<()> {
    let db = Database::new(data_dir.join("hermes.db"))?;
    let heartbeat = build_engine_heartbeat(&db, profile, started_at)?;
    let raw = serde_json::to_string(&heartbeat).map_err(AppError::from_json_error)?;
    std::fs::write(engine_heartbeat_path(data_dir), raw)
        .map_err(|err| AppError::io(format!("Failed to write engine heartbeat: {err}")))?;
    Ok(())
}

fn build_engine_heartbeat(
    db: &Database,
    profile: &str,
    started_at: &str,
) -> AppResult<EngineHeartbeat> {
    let queued_background_runs = query_count(
        db,
        "SELECT COUNT(*)
         FROM runs
         WHERE status IN ('queued', 'running')
           AND EXISTS (
             SELECT 1
             FROM run_events
             WHERE run_events.run_id = runs.id
               AND run_events.event_type = 'background_enqueued'
           )",
    )?;
    let awaiting_approval_steps = query_count(
        db,
        "SELECT COUNT(*) FROM execution_steps WHERE status = 'awaiting_approval'",
    )?;

    Ok(EngineHeartbeat {
        profile: profile.trim().to_string(),
        started_at: started_at.to_string(),
        last_heartbeat_at: Utc::now().to_rfc3339(),
        queued_background_runs,
        awaiting_approval_steps,
    })
}

fn query_count(db: &Database, sql: &str) -> AppResult<u32> {
    let count: i64 = db
        .query_row(sql, &[], |row| row.get(0))
        .map_err(|err| AppError::storage(format!("Failed to query engine count: {err}")))?;
    Ok(count.max(0) as u32)
}

#[cfg(test)]
mod tests {
    use super::{
        engine_heartbeat_path, maybe_run_engine_daemon_from_args, parse_engine_daemon_args,
        read_engine_heartbeat, run_engine_daemon_tick,
    };
    use crate::backend::Database;
    use chrono::Utc;
    use uuid::Uuid;

    fn make_temp_data_dir() -> std::path::PathBuf {
        let base = std::env::temp_dir().join(format!("hermes-engine-daemon-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&base).expect("temp dir should create");
        base
    }

    fn seed_engine_tables(db: &Database, mission_id: &str, run_id: &str) {
        let now = Utc::now().to_rfc3339();
        let constraints_json =
            serde_json::to_string(&vec!["Queued from test"]).expect("json should serialize");
        let success_json = serde_json::to_string(&vec!["Complete background work"])
            .expect("json should serialize");

        db.execute(
            "INSERT INTO missions (
                id, title, goal, constraints_json, success_criteria_json,
                status, priority, pinned, created_at, updated_at, last_activity_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &mission_id as &dyn rusqlite::ToSql,
                &"Background mission".to_string(),
                &"Process queued work".to_string(),
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
        .expect("mission should seed");

        db.execute(
            "INSERT INTO runs (
                id, mission_id, type, status, started_at, summary
            ) VALUES (?, ?, ?, ?, ?, ?)",
            &[
                &run_id as &dyn rusqlite::ToSql,
                &mission_id,
                &"execution",
                &"queued",
                &Some(now.clone()),
                &Some("Queued work".to_string()),
            ],
        )
        .expect("run should seed");

        db.execute(
            "INSERT INTO run_events (
                id, run_id, mission_id, event_type, message, payload_json, created_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?)",
            &[
                &"event-background" as &dyn rusqlite::ToSql,
                &run_id,
                &mission_id,
                &"background_enqueued",
                &"Queued background work".to_string(),
                &Some("{\"prompt\":\"summarize roadmap\"}".to_string()),
                &now,
            ],
        )
        .expect("run event should seed");

        db.execute(
            "INSERT INTO execution_steps (
                id, mission_id, run_id, title, mode, risk_level, status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"step-awaiting" as &dyn rusqlite::ToSql,
                &mission_id,
                &run_id,
                &"Request approval".to_string(),
                &"cli",
                &"high",
                &"awaiting_approval",
                &now,
                &now,
            ],
        )
        .expect("execution step should seed");
    }

    #[test]
    fn parse_engine_daemon_args_returns_none_without_flag() {
        assert_eq!(
            parse_engine_daemon_args(["hermes-desktop"]).expect("args should parse"),
            None
        );
    }

    #[test]
    fn maybe_run_engine_daemon_from_args_rejects_missing_data_dir() {
        let error =
            maybe_run_engine_daemon_from_args(["bin", "--engine-daemon", "--profile", "ops"])
                .expect_err("missing data dir should fail");
        assert!(error.contains("--data-dir is required"));
    }

    #[test]
    fn run_engine_daemon_tick_writes_heartbeat_from_database_state() {
        let data_dir = make_temp_data_dir();
        let db = Database::new(data_dir.join("hermes.db")).expect("db should initialize");
        seed_engine_tables(&db, "mission-001", "run-001");

        run_engine_daemon_tick(&data_dir, "ops", "2026-04-25T00:00:00Z")
            .expect("tick should write heartbeat");

        let heartbeat =
            read_engine_heartbeat(&data_dir).expect("heartbeat should be readable after tick");
        assert_eq!(heartbeat.profile, "ops");
        assert_eq!(heartbeat.started_at, "2026-04-25T00:00:00Z");
        assert_eq!(heartbeat.queued_background_runs, 1);
        assert_eq!(heartbeat.awaiting_approval_steps, 1);
        assert!(engine_heartbeat_path(&data_dir).exists());

        std::fs::remove_dir_all(data_dir).expect("temp dir should clean");
    }
}
