use chrono::Utc;
use hermes_desktop::backend::{
    CreateExecutionStepInput, Database, ExecutionMode, ExecutionService, ExecutionServiceImpl,
    ExecutionStepStatus, RiskLevel,
};
use hermes_desktop::commands::execution::{
    ExecutionRunCliStepRequest, execution_run_cli_step_for_db,
};

fn seed_mission_and_run(db: &Database, mission_id: &str, run_id: &str) {
    let now = Utc::now().to_rfc3339();
    let constraints_json = serde_json::to_string(&Vec::<String>::new()).expect("json");
    let success_json = serde_json::to_string(&vec!["cli runner works"]).expect("json");

    db.execute(
        "INSERT OR IGNORE INTO missions (
            id, title, goal, constraints_json, success_criteria_json,
            status, priority, pinned, created_at, updated_at, last_activity_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            &mission_id as &dyn rusqlite::ToSql,
            &"CLI mission",
            &"Run a CLI step",
            &constraints_json,
            &success_json,
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

#[test]
fn execution_cli_runner_runs_command_and_records_events() {
    let db = Database::in_memory().expect("database should initialize");
    seed_mission_and_run(&db, "mission-cli", "run-cli");
    let service = ExecutionServiceImpl::new(db.clone());
    let step = service
        .plan_step(CreateExecutionStepInput {
            mission_id: "mission-cli".to_string(),
            run_id: "run-cli".to_string(),
            title: "Print output".to_string(),
            mode: ExecutionMode::Cli,
            risk_level: RiskLevel::Low,
            input_payload: Some(serde_json::json!({ "command": "printf hermes-cli" }).to_string()),
        })
        .expect("step should create");

    let completed = execution_run_cli_step_for_db(
        &db,
        ExecutionRunCliStepRequest {
            id: step.id.clone(),
            cwd: None,
        },
    )
    .expect("cli step should run");

    assert_eq!(completed.status, ExecutionStepStatus::Completed);
    assert!(
        completed
            .output_summary
            .as_deref()
            .expect("summary")
            .contains("hermes-cli")
    );

    let event_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM run_events WHERE run_id = ?1 AND event_type IN ('step_started', 'step_completed')",
            &[&"run-cli"],
            |row| row.get(0),
        )
        .expect("events should count");
    assert_eq!(event_count, 2);
}

#[test]
fn execution_cli_runner_blocks_high_risk_step_before_approval() {
    let db = Database::in_memory().expect("database should initialize");
    seed_mission_and_run(&db, "mission-risk", "run-risk");
    let service = ExecutionServiceImpl::new(db.clone());
    let step = service
        .plan_step(CreateExecutionStepInput {
            mission_id: "mission-risk".to_string(),
            run_id: "run-risk".to_string(),
            title: "High risk".to_string(),
            mode: ExecutionMode::Cli,
            risk_level: RiskLevel::High,
            input_payload: Some(serde_json::json!({ "command": "printf blocked" }).to_string()),
        })
        .expect("step should create");

    let error = execution_run_cli_step_for_db(
        &db,
        ExecutionRunCliStepRequest {
            id: step.id,
            cwd: None,
        },
    )
    .expect_err("high risk pending step should be blocked");

    assert_eq!(error.code, "validation_error");
}
