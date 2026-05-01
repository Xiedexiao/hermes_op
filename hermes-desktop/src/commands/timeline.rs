//! Run timeline / event commands

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::backend::{AppError, AppResult, Database};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunEventItem {
    pub id: String,
    pub mission_id: String,
    pub run_id: String,
    pub event_type: String,
    pub message: String,
    pub payload_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunEventListRequest {
    pub mission_id: String,
}

#[tauri::command]
pub fn run_event_list(
    db: State<'_, Database>,
    request: RunEventListRequest,
) -> Result<Vec<RunEventItem>, AppError> {
    list_run_events(db.inner(), &request.mission_id)
}

pub fn record_run_event(
    db: &Database,
    mission_id: &str,
    run_id: &str,
    event_type: &str,
    message: &str,
    payload_json: Option<String>,
) -> AppResult<()> {
    let item = RunEventItem {
        id: Uuid::new_v4().to_string(),
        mission_id: mission_id.to_string(),
        run_id: run_id.to_string(),
        event_type: event_type.to_string(),
        message: message.to_string(),
        payload_json,
        created_at: Utc::now().to_rfc3339(),
    };

    db.execute(
        "INSERT INTO run_events (id, run_id, mission_id, event_type, message, payload_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        &[
            &item.id as &dyn rusqlite::ToSql,
            &item.run_id,
            &item.mission_id,
            &item.event_type,
            &item.message,
            &item.payload_json,
            &item.created_at,
        ],
    )?;

    Ok(())
}

fn list_run_events(db: &Database, mission_id: &str) -> AppResult<Vec<RunEventItem>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, mission_id, run_id, event_type, message, payload_json, created_at
             FROM run_events
             WHERE mission_id = ?1
             ORDER BY datetime(created_at) DESC, rowid DESC",
        )?;

        let rows = stmt.query_map([mission_id], |row| {
            Ok(RunEventItem {
                id: row.get(0)?,
                mission_id: row.get(1)?,
                run_id: row.get(2)?,
                event_type: row.get(3)?,
                message: row.get(4)?,
                payload_json: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
    })
}

#[cfg(test)]
mod tests {
    use super::{list_run_events, record_run_event};
    use crate::backend::{CreateMissionInput, Database, MissionPriority, MissionRepository};

    fn seed_mission_and_run(db: &Database) -> (String, String) {
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(CreateMissionInput {
                title: "Timeline mission".to_string(),
                goal: "Track events".to_string(),
                constraints: vec![],
                success_criteria: vec!["See timeline".to_string()],
                priority: MissionPriority::Medium,
            })
            .expect("mission should create");

        db.execute(
            "INSERT INTO runs (id, mission_id, type, status, started_at, summary)
             VALUES (?, ?, ?, ?, ?, ?)",
            &[
                &"run-001" as &dyn rusqlite::ToSql,
                &mission.id,
                &"execution",
                &"running",
                &"2026-04-23T08:00:00Z",
                &"Run started",
            ],
        )
        .expect("run should insert");

        (mission.id, "run-001".to_string())
    }

    #[test]
    fn record_and_list_run_events_round_trip() {
        let db = Database::in_memory().expect("db should initialize");
        let (mission_id, run_id) = seed_mission_and_run(&db);

        record_run_event(
            &db,
            &mission_id,
            &run_id,
            "step_started",
            "Started markdown generation",
            Some("{\"step\":\"generate-md\"}".to_string()),
        )
        .expect("event should record");

        let events = list_run_events(&db, &mission_id).expect("events should list");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].mission_id, mission_id);
        assert_eq!(events[0].run_id, run_id);
        assert_eq!(events[0].event_type, "step_started");
        assert_eq!(events[0].message, "Started markdown generation");
        assert_eq!(
            events[0].payload_json.as_deref(),
            Some("{\"step\":\"generate-md\"}")
        );
    }
}
