//! Terminal backend profile registry and local status probes.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::backend::{AppError, AppResult, Database};

const DEFAULT_BACKENDS: [(&str, &str); 6] = [
    ("local", "Local"),
    ("docker", "Docker"),
    ("ssh", "SSH"),
    ("modal", "Modal"),
    ("daytona", "Daytona"),
    ("singularity", "Singularity"),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TerminalBackendProfile {
    pub id: String,
    pub kind: String,
    pub display_name: String,
    pub enabled: bool,
    #[serde(default)]
    pub config: Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerminalBackendProfileInput {
    pub id: Option<String>,
    pub kind: String,
    pub display_name: String,
    pub enabled: bool,
    pub config: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalBackendStatus {
    pub id: String,
    pub kind: String,
    pub display_name: String,
    pub enabled: bool,
    pub availability: String,
    pub configured: bool,
    pub testable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_command: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TerminalBackendTestResult {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub availability: String,
    pub message: String,
}

pub struct TerminalBackendRegistry {
    db: Database,
}

impl TerminalBackendRegistry {
    pub fn new(db: Database) -> Self {
        Self { db }
    }

    pub fn list_profiles(&self) -> AppResult<Vec<TerminalBackendProfile>> {
        self.ensure_defaults()?;
        self.db.with_connection(|conn| {
            let mut statement = conn.prepare(
                "SELECT id, kind, display_name, enabled, config_json, created_at, updated_at
                 FROM terminal_backend_profiles
                 ORDER BY
                    CASE kind
                        WHEN 'local' THEN 0
                        WHEN 'docker' THEN 1
                        WHEN 'ssh' THEN 2
                        WHEN 'modal' THEN 3
                        WHEN 'daytona' THEN 4
                        WHEN 'singularity' THEN 5
                        ELSE 99
                    END,
                    id",
            )?;
            let rows = statement.query_map([], row_to_profile)?;
            rows.collect()
        })
    }

    pub fn save_profile(
        &self,
        input: TerminalBackendProfileInput,
    ) -> AppResult<TerminalBackendProfile> {
        self.ensure_defaults()?;
        let kind = normalize_kind(&input.kind)?;
        let id = input
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or(kind.as_str())
            .to_string();
        let display_name = input.display_name.trim();
        if display_name.is_empty() {
            return Err(AppError::validation(
                "Terminal backend display_name cannot be empty",
            ));
        }

        let existing_created_at = self.created_at_for(&id)?;
        let now = Utc::now().to_rfc3339();
        let created_at = existing_created_at.unwrap_or_else(|| now.clone());
        let config_json =
            serde_json::to_string(&input.config).map_err(AppError::from_json_error)?;
        let enabled = input.enabled as i64;
        let params: Vec<&dyn rusqlite::ToSql> = vec![
            &id,
            &kind,
            &display_name,
            &enabled,
            &config_json,
            &created_at,
            &now,
        ];
        self.db.execute(
            "INSERT OR REPLACE INTO terminal_backend_profiles
             (id, kind, display_name, enabled, config_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            &params,
        )?;

        self.profile_by_id(&id)
    }

    pub fn list_statuses_with_checker<F>(
        &self,
        command_available: F,
    ) -> AppResult<Vec<TerminalBackendStatus>>
    where
        F: Fn(&str) -> bool,
    {
        let profiles = self.list_profiles()?;
        Ok(profiles
            .iter()
            .map(|profile| status_for_profile(profile, &command_available))
            .collect())
    }

    pub fn test_profile_with_checker<F>(
        &self,
        id: &str,
        command_available: F,
    ) -> AppResult<TerminalBackendTestResult>
    where
        F: Fn(&str) -> bool,
    {
        let profile = self.profile_by_id_or_default(id)?;
        let status = status_for_profile(&profile, &command_available);
        let result_status = match profile.kind.as_str() {
            "local" => "passed",
            "docker" | "ssh" if status.availability == "available" => "passed",
            "modal" | "daytona" | "singularity" => "skipped",
            _ => "failed",
        }
        .to_string();

        Ok(TerminalBackendTestResult {
            id: status.id,
            kind: status.kind,
            status: result_status,
            availability: status.availability,
            message: status.message,
        })
    }

    fn ensure_schema(&self) -> AppResult<()> {
        self.db.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS terminal_backend_profiles (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                display_name TEXT NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                config_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        )
    }

    fn ensure_defaults(&self) -> AppResult<()> {
        self.ensure_schema()?;
        let now = Utc::now().to_rfc3339();
        let empty_config = "{}".to_string();

        for (kind, display_name) in DEFAULT_BACKENDS {
            let params: Vec<&dyn rusqlite::ToSql> =
                vec![&kind, &kind, &display_name, &empty_config, &now, &now];
            self.db.execute(
                "INSERT OR IGNORE INTO terminal_backend_profiles
                 (id, kind, display_name, enabled, config_json, created_at, updated_at)
                 VALUES (?, ?, ?, 1, ?, ?, ?)",
                &params,
            )?;
        }

        Ok(())
    }

    fn created_at_for(&self, id: &str) -> AppResult<Option<String>> {
        match self.db.query_row(
            "SELECT created_at FROM terminal_backend_profiles WHERE id = ?1",
            &[&id],
            |row| row.get(0),
        ) {
            Ok(created_at) => Ok(Some(created_at)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(err) => Err(AppError::storage(format!(
                "Failed to load terminal backend profile timestamp: {}",
                err
            ))),
        }
    }

    fn profile_by_id(&self, id: &str) -> AppResult<TerminalBackendProfile> {
        self.db
            .query_row(
                "SELECT id, kind, display_name, enabled, config_json, created_at, updated_at
                 FROM terminal_backend_profiles WHERE id = ?1",
                &[&id],
                row_to_profile,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => {
                    AppError::validation(format!("Terminal backend profile not found: {}", id))
                }
                other => AppError::storage(format!(
                    "Failed to load terminal backend profile: {}",
                    other
                )),
            })
    }

    fn profile_by_id_or_default(&self, id: &str) -> AppResult<TerminalBackendProfile> {
        self.ensure_defaults()?;
        self.profile_by_id(id)
    }
}

pub fn default_command_available(command: &str) -> bool {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path).any(|directory| {
        let candidate = directory.join(command);
        if candidate.is_file() {
            return true;
        }

        #[cfg(target_os = "windows")]
        {
            let exe_candidate = directory.join(format!("{}.exe", command));
            if exe_candidate.is_file() {
                return true;
            }
        }

        false
    })
}

fn row_to_profile(row: &rusqlite::Row<'_>) -> rusqlite::Result<TerminalBackendProfile> {
    let config_json: String = row.get(4)?;
    let config =
        serde_json::from_str(&config_json).unwrap_or_else(|_| Value::Object(Default::default()));
    Ok(TerminalBackendProfile {
        id: row.get(0)?,
        kind: row.get(1)?,
        display_name: row.get(2)?,
        enabled: row.get::<_, i64>(3)? != 0,
        config,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

fn normalize_kind(kind: &str) -> AppResult<String> {
    let normalized = kind.trim().to_ascii_lowercase();
    if DEFAULT_BACKENDS
        .iter()
        .any(|(supported_kind, _)| *supported_kind == normalized)
    {
        Ok(normalized)
    } else {
        Err(AppError::validation(format!(
            "Unsupported terminal backend kind: {}",
            kind
        )))
    }
}

fn status_for_profile<F>(
    profile: &TerminalBackendProfile,
    command_available: &F,
) -> TerminalBackendStatus
where
    F: Fn(&str) -> bool,
{
    if !profile.enabled {
        return TerminalBackendStatus {
            id: profile.id.clone(),
            kind: profile.kind.clone(),
            display_name: profile.display_name.clone(),
            enabled: profile.enabled,
            availability: "disabled".to_string(),
            configured: false,
            testable: false,
            required_command: None,
            message: "Terminal backend profile is disabled.".to_string(),
        };
    }

    match profile.kind.as_str() {
        "local" => status(
            profile,
            "available",
            true,
            true,
            None,
            "Local terminal backend is available.",
        ),
        "docker" => command_status(
            profile,
            "docker",
            command_available("docker"),
            "Docker CLI is available.",
            "docker CLI was not found on PATH.",
        ),
        "ssh" => ssh_status(profile, command_available("ssh")),
        "modal" | "daytona" | "singularity" => configured_only_status(profile),
        _ => status(
            profile,
            "unavailable",
            false,
            false,
            None,
            "Unknown terminal backend kind.",
        ),
    }
}

fn command_status(
    profile: &TerminalBackendProfile,
    command: &str,
    available: bool,
    available_message: &str,
    unavailable_message: &str,
) -> TerminalBackendStatus {
    if available {
        status(
            profile,
            "available",
            true,
            true,
            Some(command),
            available_message,
        )
    } else {
        status(
            profile,
            "unavailable",
            false,
            true,
            Some(command),
            unavailable_message,
        )
    }
}

fn ssh_status(profile: &TerminalBackendProfile, ssh_available: bool) -> TerminalBackendStatus {
    let configured =
        has_non_empty_config_value(profile, "host") && has_non_empty_config_value(profile, "user");
    if !configured {
        return status(
            profile,
            "misconfigured",
            false,
            true,
            Some("ssh"),
            "SSH backend requires non-empty host and user config values.",
        );
    }

    command_status(
        profile,
        "ssh",
        ssh_available,
        "SSH config is valid and ssh is available.",
        "SSH config is valid, but ssh was not found on PATH.",
    )
}

fn configured_only_status(profile: &TerminalBackendProfile) -> TerminalBackendStatus {
    if has_any_config(profile) {
        status(
            profile,
            "configured",
            true,
            false,
            None,
            "Backend profile is configured; live testing is unavailable without local dependencies.",
        )
    } else {
        status(
            profile,
            "unavailable",
            false,
            false,
            None,
            "Backend profile has no configuration and no local tester.",
        )
    }
}

fn status(
    profile: &TerminalBackendProfile,
    availability: &str,
    configured: bool,
    testable: bool,
    required_command: Option<&str>,
    message: &str,
) -> TerminalBackendStatus {
    TerminalBackendStatus {
        id: profile.id.clone(),
        kind: profile.kind.clone(),
        display_name: profile.display_name.clone(),
        enabled: profile.enabled,
        availability: availability.to_string(),
        configured,
        testable,
        required_command: required_command.map(str::to_string),
        message: message.to_string(),
    }
}

fn has_any_config(profile: &TerminalBackendProfile) -> bool {
    match &profile.config {
        Value::Object(values) => !values.is_empty(),
        Value::Null => false,
        _ => true,
    }
}

fn has_non_empty_config_value(profile: &TerminalBackendProfile, key: &str) -> bool {
    profile
        .config
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
}
