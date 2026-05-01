//! Council step commands

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::backend::{AppError, Database};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CouncilStepItem {
    pub id: String,
    pub mission_id: String,
    pub run_id: String,
    pub role: String,
    pub status: String,
    pub input_summary: Option<String>,
    pub output_summary: Option<String>,
    pub review_note: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouncilStepListRequest {
    pub mission_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CouncilStepCreateRequest {
    pub mission_id: String,
    pub run_id: String,
    pub role: String,
    pub status: String,
    pub input_summary: Option<String>,
    pub output_summary: Option<String>,
    pub review_note: Option<String>,
}

#[tauri::command]
pub fn council_step_list(
    db: State<'_, Database>,
    request: CouncilStepListRequest,
) -> Result<Vec<CouncilStepItem>, AppError> {
    list_council_steps(db.inner(), &request.mission_id)
}

#[tauri::command]
pub fn council_step_create(
    db: State<'_, Database>,
    request: CouncilStepCreateRequest,
) -> Result<CouncilStepItem, AppError> {
    create_council_step(db.inner(), request)
}

fn create_council_step(
    db: &Database,
    request: CouncilStepCreateRequest,
) -> Result<CouncilStepItem, AppError> {
    let mission_id = request.mission_id.trim().to_string();
    let run_id = request.run_id.trim().to_string();
    let role = request.role.trim().to_string();
    let status = request.status.trim().to_string();
    if mission_id.is_empty() || run_id.is_empty() || role.is_empty() || status.is_empty() {
        return Err(AppError::validation("council step fields cannot be empty"));
    }

    let item = CouncilStepItem {
        id: Uuid::new_v4().to_string(),
        mission_id,
        run_id,
        role,
        status,
        input_summary: request
            .input_summary
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        output_summary: request
            .output_summary
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        review_note: request
            .review_note
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty()),
        created_at: Utc::now().to_rfc3339(),
        updated_at: Utc::now().to_rfc3339(),
    };

    db.execute(
        "INSERT INTO council_steps (id, mission_id, run_id, role, status, input_summary, output_summary, review_note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            &item.id as &dyn rusqlite::ToSql,
            &item.mission_id,
            &item.run_id,
            &item.role,
            &item.status,
            &item.input_summary,
            &item.output_summary,
            &item.review_note,
            &item.created_at,
            &item.updated_at,
        ],
    )?;

    Ok(item)
}

fn list_council_steps(db: &Database, mission_id: &str) -> Result<Vec<CouncilStepItem>, AppError> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, mission_id, run_id, role, status, input_summary, output_summary, review_note, created_at, updated_at
             FROM council_steps
             WHERE mission_id = ?1
             ORDER BY datetime(updated_at) DESC, rowid DESC",
        )?;

        let rows = stmt.query_map([mission_id], |row| {
            Ok(CouncilStepItem {
                id: row.get(0)?,
                mission_id: row.get(1)?,
                run_id: row.get(2)?,
                role: row.get(3)?,
                status: row.get(4)?,
                input_summary: row.get(5)?,
                output_summary: row.get(6)?,
                review_note: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
    })
}

#[cfg(test)]
mod tests {
    use super::{CouncilStepCreateRequest, create_council_step, list_council_steps};
    use crate::backend::{CreateMissionInput, Database, MissionPriority, MissionRepository};

    fn seed_mission_and_run(db: &Database) -> (String, String) {
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(CreateMissionInput {
                title: "Council mission".to_string(),
                goal: "Review plan".to_string(),
                constraints: vec![],
                success_criteria: vec!["Done".to_string()],
                priority: MissionPriority::Medium,
            })
            .expect("mission should create");

        db.execute(
            "INSERT INTO runs (id, mission_id, type, status, started_at, summary)
             VALUES (?, ?, ?, ?, ?, ?)",
            &[
                &"run-council" as &dyn rusqlite::ToSql,
                &mission.id,
                &"council",
                &"running",
                &"2026-04-23T08:00:00Z",
                &"Council started",
            ],
        )
        .expect("run should insert");

        (mission.id, "run-council".to_string())
    }

    #[test]
    fn create_and_list_council_steps_for_a_mission() {
        let db = Database::in_memory().expect("db should initialize");
        let (mission_id, run_id) = seed_mission_and_run(&db);

        let created = create_council_step(
            &db,
            CouncilStepCreateRequest {
                mission_id: mission_id.clone(),
                run_id: run_id.clone(),
                role: "critic".to_string(),
                status: "running".to_string(),
                input_summary: Some("Review pricing plan".to_string()),
                output_summary: Some("Need clearer assumptions".to_string()),
                review_note: Some("Return to planner".to_string()),
            },
        )
        .expect("council step should create");

        let steps = list_council_steps(&db, &mission_id).expect("steps should list");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0], created);
    }
}
