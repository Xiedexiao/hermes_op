//! Execution 数据仓储

use chrono::Utc;
use rusqlite::params;
use uuid::Uuid;

use crate::backend::domain::{
    CreateExecutionStepInput, ExecutionMode, ExecutionStep, ExecutionStepStatus, RiskLevel,
};
use crate::backend::{AppError, AppResult, Database};

#[derive(Clone)]
pub struct ExecutionRepository {
    db: Database,
}

impl ExecutionRepository {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn create(
        &self,
        input: CreateExecutionStepInput,
        status: ExecutionStepStatus,
    ) -> AppResult<ExecutionStep> {
        let now = Utc::now().to_rfc3339();
        let step = ExecutionStep {
            id: Uuid::new_v4().to_string(),
            mission_id: input.mission_id,
            run_id: input.run_id,
            title: input.title,
            mode: input.mode,
            risk_level: input.risk_level,
            status,
            input_payload: input.input_payload,
            output_summary: None,
            created_at: now.clone(),
            updated_at: now,
        };

        self.db.execute(
            "INSERT INTO execution_steps (
                id, mission_id, run_id, title, mode, risk_level, status,
                input_payload, output_summary, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &step.id as &dyn rusqlite::ToSql,
                &step.mission_id,
                &step.run_id,
                &step.title,
                &step.mode.as_str(),
                &step.risk_level.as_str(),
                &step.status.as_str(),
                &step.input_payload,
                &step.output_summary,
                &step.created_at,
                &step.updated_at,
            ],
        )?;

        Ok(step)
    }

    pub fn list_by_run(&self, mission_id: &str, run_id: &str) -> AppResult<Vec<ExecutionStep>> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, mission_id, run_id, title, mode, risk_level, status,
                    input_payload, output_summary, created_at, updated_at
                 FROM execution_steps
                 WHERE mission_id = ?1 AND run_id = ?2
                 ORDER BY datetime(created_at) ASC, rowid ASC",
            )?;

            let rows = stmt.query_map(params![mission_id, run_id], map_execution_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    pub fn list_by_mission(&self, mission_id: &str) -> AppResult<Vec<ExecutionStep>> {
        self.db.with_connection(|conn| {
            let mut stmt = conn.prepare(
                "SELECT
                    id, mission_id, run_id, title, mode, risk_level, status,
                    input_payload, output_summary, created_at, updated_at
                 FROM execution_steps
                 WHERE mission_id = ?1
                 ORDER BY datetime(created_at) ASC, rowid ASC",
            )?;

            let rows = stmt.query_map(params![mission_id], map_execution_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    pub fn get(&self, id: &str) -> AppResult<Option<ExecutionStep>> {
        match self.db.query_row(
            "SELECT
                id, mission_id, run_id, title, mode, risk_level, status,
                input_payload, output_summary, created_at, updated_at
             FROM execution_steps
             WHERE id = ?1",
            &[&id],
            map_execution_row,
        ) {
            Ok(step) => Ok(Some(step)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(AppError::storage(format!(
                "Failed to fetch execution step: {}",
                e
            ))),
        }
    }

    pub fn update_status(
        &self,
        id: &str,
        status: ExecutionStepStatus,
        output_summary: Option<String>,
    ) -> AppResult<ExecutionStep> {
        let updated_at = Utc::now().to_rfc3339();
        self.db.execute(
            "UPDATE execution_steps
             SET status = ?2, output_summary = ?3, updated_at = ?4
             WHERE id = ?1",
            &[
                &id as &dyn rusqlite::ToSql,
                &status.as_str(),
                &output_summary,
                &updated_at,
            ],
        )?;

        self.get(id)?
            .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))
    }
}

fn map_execution_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ExecutionStep> {
    Ok(ExecutionStep {
        id: row.get(0)?,
        mission_id: row.get(1)?,
        run_id: row.get(2)?,
        title: row.get(3)?,
        mode: ExecutionMode::from_key(&row.get::<_, String>(4)?),
        risk_level: RiskLevel::from_key(&row.get::<_, String>(5)?),
        status: ExecutionStepStatus::from_key(&row.get::<_, String>(6)?),
        input_payload: row.get(7)?,
        output_summary: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::domain::{ExecutionMode, RiskLevel};
    use chrono::Utc;

    fn sample_input(mission_id: &str, run_id: &str, title: &str) -> CreateExecutionStepInput {
        CreateExecutionStepInput {
            mission_id: mission_id.to_string(),
            run_id: run_id.to_string(),
            title: title.to_string(),
            mode: ExecutionMode::Cli,
            risk_level: RiskLevel::Low,
            input_payload: None,
        }
    }

    fn seed_mission_and_run(db: &Database, mission_id: &str, run_id: &str) {
        let now = Utc::now().to_rfc3339();
        let constraints_json =
            serde_json::to_string(&vec!["只读执行"]).expect("json should serialize");
        let success_json =
            serde_json::to_string(&vec!["生成执行步骤"]).expect("json should serialize");

        db.execute(
            "INSERT OR IGNORE INTO missions (
                id, title, goal, constraints_json, success_criteria_json,
                status, priority, pinned, created_at, updated_at, last_activity_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &mission_id as &dyn rusqlite::ToSql,
                &format!("mission-{mission_id}"),
                &"生成执行计划",
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

    #[test]
    fn create_persists_pending_step_with_contract_fields() {
        let db = Database::in_memory().expect("in-memory database should initialize");
        seed_mission_and_run(&db, "mission-001", "run-001");
        let repo = ExecutionRepository::new(db);

        let created = repo
            .create(
                sample_input("mission-001", "run-001", "执行命令"),
                ExecutionStepStatus::Pending,
            )
            .expect("step should be created");

        assert_eq!(created.mission_id, "mission-001");
        assert_eq!(created.run_id, "run-001");
        assert_eq!(created.title, "执行命令");
        assert_eq!(created.mode, ExecutionMode::Cli);
        assert_eq!(created.risk_level, RiskLevel::Low);
        assert_eq!(created.status, ExecutionStepStatus::Pending);
        assert!(created.output_summary.is_none());
        assert!(!created.id.is_empty());
        assert!(!created.created_at.is_empty());
        assert!(!created.updated_at.is_empty());
    }

    #[test]
    fn list_by_mission_returns_steps_for_same_mission_across_runs() {
        let db = Database::in_memory().expect("in-memory database should initialize");
        seed_mission_and_run(&db, "mission-001", "run-001");
        seed_mission_and_run(&db, "mission-001", "run-002");
        seed_mission_and_run(&db, "mission-002", "run-003");
        let repo = ExecutionRepository::new(db);

        let first = repo
            .create(
                sample_input("mission-001", "run-001", "收集上下文"),
                ExecutionStepStatus::Pending,
            )
            .expect("first step should be created");
        let second = repo
            .create(
                sample_input("mission-001", "run-002", "整理命令序列"),
                ExecutionStepStatus::Pending,
            )
            .expect("second step should be created");
        repo.create(
            sample_input("mission-002", "run-003", "其他任务"),
            ExecutionStepStatus::Pending,
        )
        .expect("other mission step should be created");

        let listed = repo
            .list_by_mission("mission-001")
            .expect("steps should list by mission");

        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, first.id);
        assert_eq!(listed[1].id, second.id);
    }

    #[test]
    fn create_persists_across_repository_instances() {
        let db = Database::in_memory().expect("in-memory database should initialize");
        seed_mission_and_run(&db, "mission-001", "run-001");

        let repo_a = ExecutionRepository::new(db.clone());
        let created = repo_a
            .create(
                sample_input("mission-001", "run-001", "跨实例持久化"),
                ExecutionStepStatus::Pending,
            )
            .expect("step should be created");

        let repo_b = ExecutionRepository::new(db);
        let fetched = repo_b
            .get(&created.id)
            .expect("fetch should succeed")
            .expect("step should exist");

        assert_eq!(fetched.id, created.id);
        assert_eq!(fetched.title, "跨实例持久化");
    }
}
