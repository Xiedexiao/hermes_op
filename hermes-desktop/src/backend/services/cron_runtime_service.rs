//! Local parity cron runtime.
//!
//! This service turns persisted parity cron metadata into concrete local app
//! state. It deliberately stays inside the desktop database surface and does
//! not call MCP runtime or HTTP control APIs.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::backend::{
    AppError, AppResult, CreateExecutionStepInput, Database, ExecutionMode, ExecutionRepository,
    ExecutionStepStatus, RiskLevel,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityCronRuntimeStatus {
    pub status: String,
    #[serde(default)]
    pub worker_started_at: Option<String>,
    #[serde(default)]
    pub last_heartbeat_at: Option<String>,
    #[serde(default)]
    pub last_poll_started_at: Option<String>,
    #[serde(default)]
    pub last_poll_completed_at: Option<String>,
    #[serde(default)]
    pub last_error: Option<String>,
    pub last_dispatch_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityCronRuntimeTickResult {
    pub checked_jobs: u32,
    pub dispatched_jobs: u32,
    pub heartbeat_at: String,
    #[serde(default)]
    pub dispatches: Vec<ParityCronDispatchOutcome>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ParityCronDispatchOutcome {
    pub job_id: String,
    pub job_name: String,
    pub reason: String,
    pub status: String,
    pub session_id: String,
    pub mission_id: String,
    pub run_id: String,
    pub run_event_id: String,
    pub dispatched_at: String,
}

#[derive(Debug, Clone)]
pub struct ParityCronRuntimeService {
    db: Database,
}

#[derive(Debug, Clone)]
struct CronCandidate {
    id: String,
    name: String,
    schedule: String,
    prompt: String,
    deliver_to: Option<String>,
    enabled: bool,
    last_run_status: Option<String>,
    created_at: String,
    last_completed_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DispatchReason {
    Scheduled,
    RunNow,
}

impl DispatchReason {
    fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::RunNow => "run_now",
        }
    }

    fn run_count_increment(self) -> i64 {
        match self {
            Self::Scheduled => 1,
            Self::RunNow => 0,
        }
    }
}

impl ParityCronRuntimeService {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn ensure_schema(&self) -> AppResult<()> {
        self.db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS parity_cron_jobs (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                schedule TEXT NOT NULL,
                prompt TEXT NOT NULL,
                deliver_to TEXT,
                enabled INTEGER NOT NULL DEFAULT 1,
                status TEXT NOT NULL,
                last_run_requested_at TEXT,
                last_run_status TEXT,
                run_count INTEGER NOT NULL DEFAULT 0,
                paused_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS parity_cron_runtime_state (
                job_id TEXT PRIMARY KEY,
                last_started_at TEXT,
                last_completed_at TEXT,
                last_status TEXT,
                last_error TEXT,
                last_session_id TEXT,
                last_mission_id TEXT,
                last_run_id TEXT,
                last_run_event_id TEXT,
                updated_at TEXT NOT NULL,
                FOREIGN KEY (job_id) REFERENCES parity_cron_jobs(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS parity_cron_runtime_heartbeat (
                id TEXT PRIMARY KEY,
                status TEXT NOT NULL,
                worker_started_at TEXT,
                last_heartbeat_at TEXT,
                last_poll_started_at TEXT,
                last_poll_completed_at TEXT,
                last_error TEXT,
                last_dispatch_count INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )?;

        let now = Utc::now().to_rfc3339();
        self.db.execute(
            "INSERT OR IGNORE INTO parity_cron_runtime_heartbeat
             (id, status, worker_started_at, last_heartbeat_at, last_dispatch_count)
             VALUES (?, ?, ?, ?, ?)",
            &[&"local", &"idle", &now, &now, &0_i64],
        )?;

        Ok(())
    }

    pub fn status(&self) -> AppResult<ParityCronRuntimeStatus> {
        self.ensure_schema()?;
        self.db
            .query_row(
                "SELECT status, worker_started_at, last_heartbeat_at, last_poll_started_at,
                        last_poll_completed_at, last_error, last_dispatch_count
                 FROM parity_cron_runtime_heartbeat
                 WHERE id = 'local'",
                &[],
                |row| {
                    Ok(ParityCronRuntimeStatus {
                        status: row.get(0)?,
                        worker_started_at: row.get(1)?,
                        last_heartbeat_at: row.get(2)?,
                        last_poll_started_at: row.get(3)?,
                        last_poll_completed_at: row.get(4)?,
                        last_error: row.get(5)?,
                        last_dispatch_count: row.get::<_, i64>(6)? as u32,
                    })
                },
            )
            .map_err(|err| AppError::storage(format!("Failed to load cron runtime status: {err}")))
    }

    pub fn poll_once(&self) -> AppResult<ParityCronRuntimeTickResult> {
        self.ensure_schema()?;
        let now = Utc::now();
        let started_at = now.to_rfc3339();
        self.record_poll_started(&started_at)?;

        match self.poll_due_jobs(now) {
            Ok(result) => {
                self.record_poll_completed(&result.heartbeat_at, result.dispatched_jobs)?;
                Ok(result)
            }
            Err(err) => {
                self.record_poll_error(&started_at, &err.to_string())?;
                Err(err)
            }
        }
    }

    pub fn dispatch_requested_job(
        &self,
        job_id: &str,
    ) -> AppResult<Option<ParityCronDispatchOutcome>> {
        self.ensure_schema()?;
        let job_id = job_id.trim();
        if job_id.is_empty() {
            return Err(AppError::validation("cron id is required"));
        }

        let Some(job) = self.load_candidate(job_id)? else {
            return Err(AppError::validation("cron job not found"));
        };

        if job.last_run_status.as_deref() != Some("requested") {
            return Ok(None);
        }

        self.dispatch_job(&job, DispatchReason::RunNow, Utc::now())
            .map(Some)
    }

    fn poll_due_jobs(&self, now: DateTime<Utc>) -> AppResult<ParityCronRuntimeTickResult> {
        let jobs = self.load_candidates()?;
        let checked_jobs = jobs.len() as u32;
        let mut dispatches = Vec::new();

        for job in jobs {
            if let Some(reason) = due_reason(&job, now) {
                dispatches.push(self.dispatch_job(&job, reason, now)?);
            }
        }

        let heartbeat_at = Utc::now().to_rfc3339();
        Ok(ParityCronRuntimeTickResult {
            checked_jobs,
            dispatched_jobs: dispatches.len() as u32,
            heartbeat_at,
            dispatches,
        })
    }

    fn load_candidates(&self) -> AppResult<Vec<CronCandidate>> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT j.id, j.name, j.schedule, j.prompt, j.deliver_to, j.enabled,
                        j.last_run_status, j.created_at, s.last_completed_at
                 FROM parity_cron_jobs j
                 LEFT JOIN parity_cron_runtime_state s ON s.job_id = j.id
                 WHERE j.enabled = 1 OR j.last_run_status = 'requested'
                 ORDER BY datetime(j.updated_at) ASC, j.rowid ASC",
            )?;
            let rows = stmt.query_map([], map_candidate_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    fn load_candidate(&self, job_id: &str) -> AppResult<Option<CronCandidate>> {
        match self.db.query_row(
            "SELECT j.id, j.name, j.schedule, j.prompt, j.deliver_to, j.enabled,
                    j.last_run_status, j.created_at, s.last_completed_at
             FROM parity_cron_jobs j
             LEFT JOIN parity_cron_runtime_state s ON s.job_id = j.id
             WHERE j.id = ?1",
            &[&job_id],
            map_candidate_row,
        ) {
            Ok(job) => Ok(Some(job)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(AppError::storage(format!(
                "Failed to load cron runtime job: {err}"
            ))),
        }
    }

    fn dispatch_job(
        &self,
        job: &CronCandidate,
        reason: DispatchReason,
        now: DateTime<Utc>,
    ) -> AppResult<ParityCronDispatchOutcome> {
        let dispatched_at = now.to_rfc3339();
        let session_id = Uuid::new_v4().to_string();
        let mission_id = Uuid::new_v4().to_string();
        let run_id = Uuid::new_v4().to_string();
        let run_event_id = Uuid::new_v4().to_string();
        let title = format!("Cron: {}", job.name);
        let constraints_json = serde_json::to_string(&vec![format!(
            "Scheduled by parity cron job {} using {}",
            job.id, job.schedule
        )])
        .map_err(AppError::from_json_error)?;
        let success_json = serde_json::to_string(&vec!["Local cron dispatch recorded".to_string()])
            .map_err(AppError::from_json_error)?;
        let summary = format!(
            "Cron job \"{}\" dispatched by {}",
            job.name,
            reason.as_str()
        );
        let payload_json = serde_json::json!({
            "job_id": job.id,
            "job_name": job.name,
            "schedule": job.schedule,
            "deliver_to": job.deliver_to,
            "session_id": session_id,
            "reason": reason.as_str(),
            "prompt": job.prompt,
        })
        .to_string();
        let background_event_id = Uuid::new_v4().to_string();
        let final_job_status = if job.enabled { "scheduled" } else { "paused" };
        let run_increment = reason.run_count_increment();
        let no_error: Option<String> = None;
        let execution_repo = ExecutionRepository::new(self.db.clone());

        self.db.transaction(|tx| {
            tx.execute(
                "INSERT INTO sessions (
                    id, source, title, model_name, parent_session_id,
                    started_at, updated_at, ended_at
                ) VALUES (?1, 'cron', ?2, NULL, NULL, ?3, ?3, ?3)",
                (&session_id, &title, &dispatched_at),
            )?;

            tx.execute(
                "INSERT INTO missions (
                    id, title, goal, constraints_json, success_criteria_json,
                    status, priority, pinned, created_at, updated_at, last_activity_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, 'awaiting_approval', 'medium', 0, ?6, ?6, ?6)",
                (
                    &mission_id,
                    &title,
                    &job.prompt,
                    &constraints_json,
                    &success_json,
                    &dispatched_at,
                ),
            )?;

            tx.execute(
                "INSERT INTO runs (
                    id, mission_id, type, status, started_at, finished_at, summary, error_message
                ) VALUES (?1, ?2, 'execution', 'queued', ?3, NULL, ?4, NULL)",
                (&run_id, &mission_id, &dispatched_at, &summary),
            )?;

            tx.execute(
                "INSERT INTO run_events (
                    id, run_id, mission_id, event_type, message, payload_json, created_at
                ) VALUES (?1, ?2, ?3, 'background_enqueued', ?4, ?5, ?6)",
                (
                    &background_event_id,
                    &run_id,
                    &mission_id,
                    &format!("Queued cron prompt: {}", job.prompt),
                    &payload_json,
                    &dispatched_at,
                ),
            )?;

            tx.execute(
                "INSERT INTO run_events (
                    id, run_id, mission_id, event_type, message, payload_json, created_at
                ) VALUES (?1, ?2, ?3, 'cron_dispatch_completed', ?4, ?5, ?6)",
                (
                    &run_event_id,
                    &run_id,
                    &mission_id,
                    &summary,
                    &payload_json,
                    &dispatched_at,
                ),
            )?;

            tx.execute(
                "INSERT INTO parity_cron_runtime_state (
                    job_id, last_started_at, last_completed_at, last_status, last_error,
                    last_session_id, last_mission_id, last_run_id, last_run_event_id, updated_at
                ) VALUES (?1, ?2, ?2, 'completed', NULL, ?3, ?4, ?5, ?6, ?2)
                 ON CONFLICT(job_id) DO UPDATE SET
                    last_started_at = excluded.last_started_at,
                    last_completed_at = excluded.last_completed_at,
                    last_status = excluded.last_status,
                    last_error = excluded.last_error,
                    last_session_id = excluded.last_session_id,
                    last_mission_id = excluded.last_mission_id,
                    last_run_id = excluded.last_run_id,
                    last_run_event_id = excluded.last_run_event_id,
                    updated_at = excluded.updated_at",
                (
                    &job.id,
                    &dispatched_at,
                    &session_id,
                    &mission_id,
                    &run_id,
                    &run_event_id,
                ),
            )?;

            tx.execute(
                "UPDATE parity_cron_jobs
                 SET status = ?2,
                     last_run_requested_at = ?3,
                     last_run_status = 'completed',
                     run_count = run_count + ?4,
                     paused_at = CASE WHEN enabled = 0 THEN COALESCE(paused_at, ?3) ELSE NULL END,
                     updated_at = ?3
                 WHERE id = ?1",
                (&job.id, &final_job_status, &dispatched_at, &run_increment),
            )?;

            let _ = &no_error;
            Ok(())
        })?;

        let step_specs = [
            (
                "Review scheduled brief",
                ExecutionMode::Api,
                RiskLevel::Low,
                ExecutionStepStatus::Pending,
                Some(
                    serde_json::json!({
                        "job_id": job.id,
                        "schedule": job.schedule,
                        "prompt": job.prompt,
                    })
                    .to_string(),
                ),
            ),
            (
                "Prepare cron execution draft",
                ExecutionMode::Cli,
                RiskLevel::Medium,
                ExecutionStepStatus::Pending,
                Some(
                    serde_json::json!({
                        "deliver_to": job.deliver_to,
                        "reason": reason.as_str(),
                    })
                    .to_string(),
                ),
            ),
            (
                "Request approval for scheduled delivery",
                ExecutionMode::Cli,
                RiskLevel::High,
                ExecutionStepStatus::AwaitingApproval,
                Some(
                    serde_json::json!({
                        "deliver_to": job.deliver_to,
                        "requires_approval": true,
                    })
                    .to_string(),
                ),
            ),
        ];

        for (title, mode, risk_level, status, input_payload) in step_specs {
            execution_repo.create(
                CreateExecutionStepInput {
                    mission_id: mission_id.clone(),
                    run_id: run_id.clone(),
                    title: title.to_string(),
                    mode,
                    risk_level,
                    input_payload,
                },
                status,
            )?;
        }

        Ok(ParityCronDispatchOutcome {
            job_id: job.id.clone(),
            job_name: job.name.clone(),
            reason: reason.as_str().to_string(),
            status: "completed".to_string(),
            session_id,
            mission_id,
            run_id,
            run_event_id,
            dispatched_at,
        })
    }

    fn record_poll_started(&self, at: &str) -> AppResult<()> {
        self.db.execute(
            "UPDATE parity_cron_runtime_heartbeat
             SET status = 'polling',
                 last_heartbeat_at = ?1,
                 last_poll_started_at = ?1,
                 last_error = NULL
             WHERE id = 'local'",
            &[&at],
        )?;
        Ok(())
    }

    fn record_poll_completed(&self, at: &str, dispatch_count: u32) -> AppResult<()> {
        let count = dispatch_count as i64;
        self.db.execute(
            "UPDATE parity_cron_runtime_heartbeat
             SET status = 'idle',
                 last_heartbeat_at = ?1,
                 last_poll_completed_at = ?1,
                 last_dispatch_count = ?2,
                 last_error = NULL
             WHERE id = 'local'",
            &[&at as &dyn rusqlite::ToSql, &count],
        )?;
        Ok(())
    }

    fn record_poll_error(&self, at: &str, message: &str) -> AppResult<()> {
        self.db.execute(
            "UPDATE parity_cron_runtime_heartbeat
             SET status = 'error',
                 last_heartbeat_at = ?1,
                 last_poll_completed_at = ?1,
                 last_error = ?2
             WHERE id = 'local'",
            &[&at, &message],
        )?;
        Ok(())
    }
}

fn map_candidate_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<CronCandidate> {
    Ok(CronCandidate {
        id: row.get(0)?,
        name: row.get(1)?,
        schedule: row.get(2)?,
        prompt: row.get(3)?,
        deliver_to: row.get(4)?,
        enabled: row.get::<_, i64>(5)? != 0,
        last_run_status: row.get(6)?,
        created_at: row.get(7)?,
        last_completed_at: row.get(8)?,
    })
}

fn due_reason(job: &CronCandidate, now: DateTime<Utc>) -> Option<DispatchReason> {
    if job.last_run_status.as_deref() == Some("requested") {
        return Some(DispatchReason::RunNow);
    }
    if !job.enabled {
        return None;
    }

    let every_seconds = parse_every_seconds(&job.schedule)?;
    let anchor = job
        .last_completed_at
        .as_deref()
        .or(Some(job.created_at.as_str()))
        .and_then(parse_rfc3339_utc);

    match anchor {
        Some(anchor) if now.signed_duration_since(anchor).num_seconds() < every_seconds => None,
        _ => Some(DispatchReason::Scheduled),
    }
}

fn parse_every_seconds(schedule: &str) -> Option<i64> {
    let seconds = schedule
        .trim()
        .strip_prefix("@every:")?
        .trim()
        .parse()
        .ok()?;
    (seconds > 0).then_some(seconds)
}

fn parse_rfc3339_utc(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::{CronCandidate, DispatchReason, due_reason, parse_every_seconds};
    use chrono::{Duration, Utc};

    fn candidate(schedule: &str) -> CronCandidate {
        CronCandidate {
            id: "job-1".to_string(),
            name: "Job".to_string(),
            schedule: schedule.to_string(),
            prompt: "Do work".to_string(),
            deliver_to: None,
            enabled: true,
            last_run_status: None,
            created_at: (Utc::now() - Duration::seconds(60)).to_rfc3339(),
            last_completed_at: None,
        }
    }

    #[test]
    fn parse_every_seconds_accepts_positive_seconds_only() {
        assert_eq!(parse_every_seconds("@every:30"), Some(30));
        assert_eq!(parse_every_seconds("@every: 5"), Some(5));
        assert_eq!(parse_every_seconds("@every:0"), None);
        assert_eq!(parse_every_seconds("0 * * * *"), None);
    }

    #[test]
    fn due_reason_prioritizes_run_now_request() {
        let mut job = candidate("0 * * * *");
        job.last_run_status = Some("requested".to_string());

        assert_eq!(due_reason(&job, Utc::now()), Some(DispatchReason::RunNow));
    }

    #[test]
    fn due_reason_respects_every_interval_after_completion() {
        let now = Utc::now();
        let mut job = candidate("@every:60");
        job.created_at = (now - Duration::seconds(61)).to_rfc3339();
        assert_eq!(due_reason(&job, now), Some(DispatchReason::Scheduled));

        job.last_completed_at = Some((now - Duration::seconds(10)).to_rfc3339());
        assert_eq!(due_reason(&job, now), None);

        job.last_completed_at = Some((now - Duration::seconds(60)).to_rfc3339());
        assert_eq!(due_reason(&job, now), Some(DispatchReason::Scheduled));
    }
}
