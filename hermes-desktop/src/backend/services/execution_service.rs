//! Execution 业务服务

use crate::backend::domain::{CreateExecutionStepInput, ExecutionStep, ExecutionStepStatus};
use crate::backend::storage::ExecutionRepository;
use crate::backend::{AppError, AppResult, Database};

pub trait ExecutionService: Send + Sync {
    fn plan_step(&self, input: CreateExecutionStepInput) -> AppResult<ExecutionStep>;
    fn list_steps(&self, mission_id: &str, run_id: &str) -> AppResult<Vec<ExecutionStep>>;
    fn list_steps_for_mission(&self, mission_id: &str) -> AppResult<Vec<ExecutionStep>>;
    fn approve_step(&self, id: &str) -> AppResult<ExecutionStep>;
    fn start_step(&self, id: &str) -> AppResult<ExecutionStep>;
    fn pause_step(&self, id: &str) -> AppResult<ExecutionStep>;
    fn complete_step(&self, id: &str, output_summary: Option<String>) -> AppResult<ExecutionStep>;
    fn fail_step(&self, id: &str, output_summary: Option<String>) -> AppResult<ExecutionStep>;
    fn retry_step(&self, id: &str) -> AppResult<ExecutionStep>;
    fn resume_step(&self, id: &str) -> AppResult<ExecutionStep>;
    fn rerun_step(&self, id: &str) -> AppResult<ExecutionStep>;
    fn confirm_skip_step(&self, id: &str) -> AppResult<ExecutionStep>;
}

pub struct ExecutionServiceImpl {
    repo: ExecutionRepository,
}

impl ExecutionServiceImpl {
    pub fn new(db: Database) -> Self {
        Self {
            repo: ExecutionRepository::new(db),
        }
    }
}

impl ExecutionService for ExecutionServiceImpl {
    fn plan_step(&self, input: CreateExecutionStepInput) -> AppResult<ExecutionStep> {
        if input.title.trim().is_empty() {
            return Err(AppError::validation("execution step title cannot be empty"));
        }

        self.repo.create(input, ExecutionStepStatus::Pending)
    }

    fn list_steps(&self, mission_id: &str, run_id: &str) -> AppResult<Vec<ExecutionStep>> {
        self.repo.list_by_run(mission_id, run_id)
    }

    fn list_steps_for_mission(&self, mission_id: &str) -> AppResult<Vec<ExecutionStep>> {
        self.repo.list_by_mission(mission_id)
    }

    fn approve_step(&self, id: &str) -> AppResult<ExecutionStep> {
        let step = self
            .repo
            .get(id)?
            .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
        let next = step.status.next_status_for_approve().ok_or_else(|| {
            AppError::validation("execution step cannot be approved from current status")
        })?;
        self.repo.update_status(id, next, None)
    }

    fn start_step(&self, id: &str) -> AppResult<ExecutionStep> {
        let step = self
            .repo
            .get(id)?
            .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
        let next = step.status.next_status_for_start().ok_or_else(|| {
            AppError::validation("execution step cannot be started from current status")
        })?;
        self.repo.update_status(id, next, None)
    }

    fn pause_step(&self, id: &str) -> AppResult<ExecutionStep> {
        let step = self
            .repo
            .get(id)?
            .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
        let next = step.status.next_status_for_pause().ok_or_else(|| {
            AppError::validation("execution step cannot be paused from current status")
        })?;
        self.repo
            .update_status(id, next, step.output_summary.clone())
    }

    fn complete_step(&self, id: &str, output_summary: Option<String>) -> AppResult<ExecutionStep> {
        let step = self
            .repo
            .get(id)?
            .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
        let next = step.status.next_status_for_complete().ok_or_else(|| {
            AppError::validation("execution step cannot be completed from current status")
        })?;
        self.repo.update_status(id, next, output_summary)
    }

    fn fail_step(&self, id: &str, output_summary: Option<String>) -> AppResult<ExecutionStep> {
        let step = self
            .repo
            .get(id)?
            .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
        let next = step.status.next_status_for_fail().ok_or_else(|| {
            AppError::validation("execution step cannot be failed from current status")
        })?;
        self.repo.update_status(id, next, output_summary)
    }

    fn retry_step(&self, id: &str) -> AppResult<ExecutionStep> {
        let step = self
            .repo
            .get(id)?
            .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
        let next = step.status.next_status_for_retry().ok_or_else(|| {
            AppError::validation("execution step cannot be retried from current status")
        })?;
        self.repo
            .update_status(id, next, step.output_summary.clone())
    }

    fn resume_step(&self, id: &str) -> AppResult<ExecutionStep> {
        let step = self
            .repo
            .get(id)?
            .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
        let next = step.status.next_status_for_resume().ok_or_else(|| {
            AppError::validation("execution step cannot be resumed from current status")
        })?;
        self.repo
            .update_status(id, next, step.output_summary.clone())
    }

    fn rerun_step(&self, id: &str) -> AppResult<ExecutionStep> {
        let step = self
            .repo
            .get(id)?
            .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
        let next = step.status.next_status_for_rerun().ok_or_else(|| {
            AppError::validation("execution step cannot be rerun from current status")
        })?;
        self.repo
            .update_status(id, next, step.output_summary.clone())
    }

    fn confirm_skip_step(&self, id: &str) -> AppResult<ExecutionStep> {
        let step = self
            .repo
            .get(id)?
            .ok_or_else(|| AppError::storage(format!("execution step not found: {}", id)))?;
        let next = step.status.next_status_for_confirm_skip().ok_or_else(|| {
            AppError::validation(
                "execution step cannot be confirmed as skipped from current status",
            )
        })?;
        self.repo
            .update_status(id, next, step.output_summary.clone())
    }
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
            input_payload: Some("{\"path\":\"./notes.md\"}".to_string()),
        }
    }

    fn seed_mission_and_run(db: &Database, mission_id: &str, run_id: &str) {
        let now = Utc::now().to_rfc3339();
        let constraints_json =
            serde_json::to_string(&vec!["不接命令层"]).expect("json should serialize");
        let success_json =
            serde_json::to_string(&vec!["可创建执行步骤"]).expect("json should serialize");

        db.execute(
            "INSERT OR IGNORE INTO missions (
                id, title, goal, constraints_json, success_criteria_json,
                status, priority, pinned, created_at, updated_at, last_activity_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &mission_id as &dyn rusqlite::ToSql,
                &format!("mission-{mission_id}"),
                &"执行任务",
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
    fn plan_step_rejects_blank_title() {
        let db = Database::in_memory().expect("database should initialize");
        let service = ExecutionServiceImpl::new(db);

        let result = service.plan_step(CreateExecutionStepInput {
            title: "  ".to_string(),
            ..sample_input("mission-001", "run-001", "占位")
        });

        assert!(result.is_err());
    }

    #[test]
    fn plan_step_and_list_by_mission_round_trip_through_service() {
        let db = Database::in_memory().expect("database should initialize");
        seed_mission_and_run(&db, "mission-001", "run-001");
        seed_mission_and_run(&db, "mission-001", "run-002");
        seed_mission_and_run(&db, "mission-002", "run-003");
        let service = ExecutionServiceImpl::new(db);

        let first = service
            .plan_step(sample_input(
                "mission-001",
                "run-001",
                "生成本地 Markdown 文档",
            ))
            .expect("first step should be created");
        let second = service
            .plan_step(sample_input("mission-001", "run-002", "执行 CLI 检查"))
            .expect("second step should be created");
        service
            .plan_step(sample_input("mission-002", "run-003", "其他任务"))
            .expect("third step should be created");

        let listed = service
            .list_steps_for_mission("mission-001")
            .expect("steps should list by mission");

        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].id, first.id);
        assert_eq!(listed[1].id, second.id);
        assert!(
            listed
                .iter()
                .all(|step| step.status == ExecutionStepStatus::Pending)
        );
    }

    #[test]
    fn approve_and_complete_follow_legal_status_paths() {
        let db = Database::in_memory().expect("database should initialize");
        seed_mission_and_run(&db, "mission-001", "run-001");
        let service = ExecutionServiceImpl::new(db);

        let pending = service
            .plan_step(sample_input("mission-001", "run-001", "执行步骤"))
            .expect("step should be created");

        assert!(service.approve_step(&pending.id).is_err());

        let awaiting = service
            .repo
            .update_status(&pending.id, ExecutionStepStatus::AwaitingApproval, None)
            .expect("status update should work");
        assert_eq!(awaiting.status, ExecutionStepStatus::AwaitingApproval);

        let running = service
            .approve_step(&pending.id)
            .expect("approve should move to running");
        assert_eq!(running.status, ExecutionStepStatus::Running);

        let completed = service
            .complete_step(&pending.id, Some("done".to_string()))
            .expect("complete should work");
        assert_eq!(completed.status, ExecutionStepStatus::Completed);
        assert_eq!(completed.output_summary.as_deref(), Some("done"));
    }

    #[test]
    fn recovery_actions_move_failed_paused_and_skipped_steps_back_into_flow() {
        let db = Database::in_memory().expect("database should initialize");
        seed_mission_and_run(&db, "mission-001", "run-001");
        let service = ExecutionServiceImpl::new(db.clone());
        let repo = ExecutionRepository::new(db);

        let failed = service
            .plan_step(sample_input("mission-001", "run-001", "失败步骤"))
            .expect("failed step should be created");
        let failed = repo
            .update_status(
                &failed.id,
                ExecutionStepStatus::Failed,
                Some("network timeout".to_string()),
            )
            .expect("failed state should persist");

        let paused = service
            .plan_step(sample_input("mission-001", "run-001", "暂停步骤"))
            .expect("paused step should be created");
        let paused = repo
            .update_status(
                &paused.id,
                ExecutionStepStatus::Running,
                Some("started".to_string()),
            )
            .and_then(|step| {
                repo.update_status(
                    &step.id,
                    ExecutionStepStatus::Paused,
                    Some("waiting for human confirmation".to_string()),
                )
            })
            .expect("paused state should persist");

        let skipped = service
            .plan_step(sample_input("mission-001", "run-001", "跳过步骤"))
            .expect("skipped step should be created");
        let skipped = repo
            .update_status(
                &skipped.id,
                ExecutionStepStatus::Skipped,
                Some("dependency unavailable".to_string()),
            )
            .expect("skipped state should persist");

        let retried = service
            .retry_step(&failed.id)
            .expect("failed steps can be retried");
        assert_eq!(retried.status, ExecutionStepStatus::Pending);
        assert_eq!(retried.output_summary.as_deref(), Some("network timeout"));

        let resumed = service
            .resume_step(&paused.id)
            .expect("paused steps can resume");
        assert_eq!(resumed.status, ExecutionStepStatus::Running);
        assert_eq!(
            resumed.output_summary.as_deref(),
            Some("waiting for human confirmation")
        );

        let rerun = service
            .rerun_step(&skipped.id)
            .expect("skipped steps can rerun");
        assert_eq!(rerun.status, ExecutionStepStatus::Pending);
        assert_eq!(
            rerun.output_summary.as_deref(),
            Some("dependency unavailable")
        );

        let skipped_again = repo
            .update_status(
                &skipped.id,
                ExecutionStepStatus::Skipped,
                Some("dependency unavailable".to_string()),
            )
            .expect("skipped state should persist again");
        assert_eq!(skipped_again.status, ExecutionStepStatus::Skipped);

        let confirmed = service
            .confirm_skip_step(&skipped.id)
            .expect("skipped steps can be explicitly confirmed");
        assert_eq!(confirmed.status, ExecutionStepStatus::Skipped);
        assert_eq!(
            confirmed.output_summary.as_deref(),
            Some("dependency unavailable")
        );
    }

    #[test]
    fn recovery_actions_reject_illegal_statuses() {
        let db = Database::in_memory().expect("database should initialize");
        seed_mission_and_run(&db, "mission-001", "run-001");
        let service = ExecutionServiceImpl::new(db);

        let pending = service
            .plan_step(sample_input("mission-001", "run-001", "普通步骤"))
            .expect("step should be created");

        assert!(service.retry_step(&pending.id).is_err());
        assert!(service.resume_step(&pending.id).is_err());
        assert!(service.rerun_step(&pending.id).is_err());
        assert!(service.confirm_skip_step(&pending.id).is_err());
    }
}
