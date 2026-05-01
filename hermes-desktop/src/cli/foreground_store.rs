use chrono::Utc;
use hermes_desktop::backend::Database;
use serde::{Deserialize, Serialize};

use super::CliError;

const FOREGROUND_SNAPSHOT_KEY: &str = "cli_foreground_snapshot";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ForegroundSnapshot {
    pub(crate) active: bool,
    pub(crate) state: String,
    pub(crate) session_id: Option<String>,
    pub(crate) run_id: Option<String>,
    pub(crate) cancel_state: Option<String>,
    pub(crate) pending_count: usize,
    pub(crate) interrupt_count: usize,
    pub(crate) updated_at: String,
}

pub(crate) fn default_snapshot() -> ForegroundSnapshot {
    ForegroundSnapshot {
        active: false,
        state: "idle".to_string(),
        session_id: None,
        run_id: None,
        cancel_state: None,
        pending_count: 0,
        interrupt_count: 0,
        updated_at: Utc::now().to_rfc3339(),
    }
}

pub(crate) fn load_snapshot_for_db(db: &Database) -> Result<ForegroundSnapshot, CliError> {
    let stored = db
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?",
            &[&FOREGROUND_SNAPSHOT_KEY as &dyn rusqlite::ToSql],
            |row| row.get::<_, String>(0),
        )
        .ok();

    Ok(stored
        .and_then(|json| serde_json::from_str::<ForegroundSnapshot>(&json).ok())
        .unwrap_or_else(default_snapshot))
}

pub(crate) fn save_snapshot_for_db(
    db: &Database,
    snapshot: &ForegroundSnapshot,
) -> Result<(), CliError> {
    let value_json =
        serde_json::to_string(snapshot).map_err(|err| CliError::Runtime(err.to_string()))?;
    let params: Vec<&dyn rusqlite::ToSql> =
        vec![&FOREGROUND_SNAPSHOT_KEY, &value_json, &snapshot.updated_at];
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?, ?, ?)",
        &params,
    )
    .map_err(|err| CliError::Runtime(err.to_string()))?;
    Ok(())
}

pub(crate) fn clear_snapshot_for_db(db: &Database) -> Result<(), CliError> {
    db.execute(
        "DELETE FROM app_settings WHERE key = ?",
        &[&FOREGROUND_SNAPSHOT_KEY as &dyn rusqlite::ToSql],
    )
    .map_err(|err| CliError::Runtime(err.to_string()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        ForegroundSnapshot, clear_snapshot_for_db, load_snapshot_for_db, save_snapshot_for_db,
    };
    use hermes_desktop::backend::Database;

    #[test]
    fn snapshot_round_trips_through_app_settings() {
        let db = Database::in_memory().expect("db should initialize");
        let snapshot = ForegroundSnapshot {
            active: true,
            state: "running".to_string(),
            session_id: Some("session-123".to_string()),
            run_id: Some("run-123".to_string()),
            cancel_state: Some("active".to_string()),
            pending_count: 2,
            interrupt_count: 1,
            updated_at: "2026-04-24T00:00:00Z".to_string(),
        };

        save_snapshot_for_db(&db, &snapshot).expect("save snapshot");
        let loaded = load_snapshot_for_db(&db).expect("load snapshot");
        assert_eq!(loaded, snapshot);

        clear_snapshot_for_db(&db).expect("clear snapshot");
        let cleared = load_snapshot_for_db(&db).expect("load cleared snapshot");
        assert!(!cleared.active);
        assert_eq!(cleared.state, "idle");
    }
}
