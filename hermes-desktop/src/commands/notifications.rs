//! Notifications 命令

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::backend::{AppError, Database};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NotificationItem {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub message: String,
    pub mission_id: Option<String>,
    pub route: String,
    pub created_at: String,
}

#[tauri::command]
pub fn notifications_list(db: State<'_, Database>) -> Result<Vec<NotificationItem>, AppError> {
    build_notifications(db.inner())
}

fn build_notifications(db: &Database) -> Result<Vec<NotificationItem>, AppError> {
    let pending = db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT e.id, e.title, e.mission_id, e.updated_at, m.title
             FROM execution_steps e
             JOIN missions m ON m.id = e.mission_id
             WHERE e.status = 'awaiting_approval'
             ORDER BY datetime(e.updated_at) DESC, e.rowid DESC
             LIMIT 8",
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(NotificationItem {
                id: format!("approval:{}", row.get::<_, String>(0)?),
                kind: "pending_approval".to_string(),
                title: row.get::<_, String>(1)?,
                message: format!("Mission: {}", row.get::<_, String>(4)?),
                mission_id: Some(row.get::<_, String>(2)?),
                route: "/operate".to_string(),
                created_at: row.get::<_, String>(3)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
    })?;

    let failed_runs = db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT r.id, r.mission_id, r.summary, r.error_message, COALESCE(r.finished_at, r.started_at, m.last_activity_at), m.title
             FROM runs r
             JOIN missions m ON m.id = r.mission_id
             WHERE r.status = 'failed'
             ORDER BY datetime(COALESCE(r.finished_at, r.started_at, m.last_activity_at)) DESC, r.rowid DESC
             LIMIT 8",
        )?;

        let rows = stmt.query_map([], |row| {
            let run_id: String = row.get(0)?;
            let mission_id: String = row.get(1)?;
            let summary: Option<String> = row.get(2)?;
            let error_message: Option<String> = row.get(3)?;
            let created_at: String = row.get(4)?;
            let mission_title: String = row.get(5)?;
            Ok(NotificationItem {
                id: format!("run-failed:{}", run_id),
                kind: "run_failed".to_string(),
                title: mission_title,
                message: error_message.or(summary).unwrap_or_else(|| "Run failed".to_string()),
                mission_id: Some(mission_id),
                route: "/simulation".to_string(),
                created_at,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
    })?;

    let completed_runs = db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT r.id, r.mission_id, r.summary, COALESCE(r.finished_at, r.started_at, m.last_activity_at), m.title
             FROM runs r
             JOIN missions m ON m.id = r.mission_id
             WHERE r.status = 'completed'
             ORDER BY datetime(COALESCE(r.finished_at, r.started_at, m.last_activity_at)) DESC, r.rowid DESC
             LIMIT 8",
        )?;

        let rows = stmt.query_map([], |row| {
            let run_id: String = row.get(0)?;
            let mission_id: String = row.get(1)?;
            let summary: Option<String> = row.get(2)?;
            let created_at: String = row.get(3)?;
            let mission_title: String = row.get(4)?;
            Ok(NotificationItem {
                id: format!("run-completed:{}", run_id),
                kind: "run_completed".to_string(),
                title: mission_title,
                message: summary.unwrap_or_else(|| "Run completed".to_string()),
                mission_id: Some(mission_id),
                route: "/simulation".to_string(),
                created_at,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
    })?;

    let mut items = Vec::new();
    items.extend(pending);
    items.extend(failed_runs);
    items.extend(completed_runs);
    items.sort_by(|left, right| right.created_at.cmp(&left.created_at));
    items.truncate(12);
    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::build_notifications;
    use crate::backend::{CreateMissionInput, Database, MissionPriority, MissionRepository};

    fn sample_input(title: &str) -> CreateMissionInput {
        CreateMissionInput {
            title: title.to_string(),
            goal: format!("{title} goal"),
            constraints: vec![],
            success_criteria: vec!["done".to_string()],
            priority: MissionPriority::Medium,
        }
    }

    #[test]
    fn build_notifications_merges_pending_failed_and_completed_items() {
        let db = Database::in_memory().expect("db should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Acme rollout"))
            .expect("mission should create");

        db.execute(
            "INSERT INTO runs (id, mission_id, type, status, started_at, finished_at, summary, error_message)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"run-complete" as &dyn rusqlite::ToSql,
                &mission.id,
                &"simulation",
                &"completed",
                &Some("2026-04-23T08:00:00Z".to_string()),
                &Some("2026-04-23T08:10:00Z".to_string()),
                &Some("Simulation completed".to_string()),
                &Option::<String>::None,
            ],
        )
        .expect("completed run should insert");

        db.execute(
            "INSERT INTO runs (id, mission_id, type, status, started_at, finished_at, summary, error_message)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"run-failed" as &dyn rusqlite::ToSql,
                &mission.id,
                &"execution",
                &"failed",
                &Some("2026-04-23T09:00:00Z".to_string()),
                &Some("2026-04-23T09:05:00Z".to_string()),
                &Some("Execution failed".to_string()),
                &Some("tool exited with 1".to_string()),
            ],
        )
        .expect("failed run should insert");

        db.execute(
            "INSERT INTO execution_steps (
                id, mission_id, run_id, title, mode, risk_level, status, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &"step-1" as &dyn rusqlite::ToSql,
                &mission.id,
                &"run-failed",
                &"Approve deployment".to_string(),
                &"cli",
                &"high",
                &"awaiting_approval",
                &"2026-04-23T10:00:00Z".to_string(),
                &"2026-04-23T10:00:00Z".to_string(),
            ],
        )
        .expect("execution step should insert");

        let items = build_notifications(&db).expect("notifications should build");
        assert_eq!(
            items
                .iter()
                .map(|item| item.kind.as_str())
                .collect::<Vec<_>>(),
            vec!["pending_approval", "run_failed", "run_completed"]
        );
        assert_eq!(items[0].route, "/operate");
        assert_eq!(items[1].route, "/simulation");
    }
}
