//! Local team governance service backed by `app_settings`.
//!
//! This file intentionally stays self-contained so the governance logic can be
//! developed and tested without widening the current module registration scope.

use crate::backend::{AppError, Database};
use chrono::Utc;
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fs;
use tauri::State;

const TEAM_SYNC_SETTINGS_KEY: &str = "team_governance";
const TEAM_SYNC_SCHEMA_VERSION: u32 = 1;
const TEAM_SYNC_AUDIT_LIMIT: usize = 200;
const TEAM_SYNC_EXPORT_AUDIT_TAIL: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamRole {
    Owner,
    Admin,
    Editor,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamMember {
    pub id: String,
    pub role: TeamRole,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamRolePolicy {
    pub role: TeamRole,
    pub allowed: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamAuditEvent {
    pub id: String,
    pub at: String,
    pub action: String,
    pub actor_member_id: String,
    pub subject_member_id: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamSyncGetStateResponse {
    pub schema_version: u32,
    pub members: Vec<TeamMember>,
    pub roles: Vec<TeamRolePolicy>,
    pub audit_events: Vec<TeamAuditEvent>,
    pub last_synced_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamSyncUpsertMemberRequest {
    pub actor_member_id: String,
    pub member_id: String,
    pub role: TeamRole,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamSyncCheckAccessRequest {
    pub actor_member_id: String,
    pub resource: String,
    pub action: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamSyncAccessDecision {
    pub allowed: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamSyncBundle {
    pub schema_version: u32,
    pub exported_at: String,
    pub members: Vec<TeamMember>,
    pub roles: Vec<TeamRolePolicy>,
    pub audit_events: Vec<TeamAuditEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamSyncExportBundleRequest {
    pub actor_member_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TeamSyncAuditExportFormat {
    Json,
    Jsonl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamSyncExportAuditRequest {
    pub actor_member_id: String,
    pub actor: Option<String>,
    pub action: Option<String>,
    pub limit: Option<usize>,
    pub format: TeamSyncAuditExportFormat,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamSyncExportAuditResponse {
    pub total: usize,
    pub exported_count: usize,
    pub payload: String,
    pub events: Vec<TeamAuditEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamSyncImportBundleRequest {
    pub actor_member_id: String,
    pub bundle: TeamSyncBundle,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamSyncRunFolderSyncRequest {
    pub actor_member_id: String,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TeamSyncRunFolderSyncResponse {
    pub state: TeamSyncGetStateResponse,
    pub bundle: Option<TeamSyncBundle>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeamSyncError {
    pub message: String,
}

impl TeamSyncError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for TeamSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for TeamSyncError {}

fn map_team_sync_error(err: TeamSyncError) -> rusqlite::Error {
    rusqlite::Error::ToSqlConversionFailure(Box::new(err))
}

fn with_team_sync_connection<T, F>(db: &Database, action: F) -> Result<T, AppError>
where
    F: FnOnce(&rusqlite::Connection) -> Result<T, TeamSyncError>,
{
    db.with_connection(|conn| action(conn).map_err(map_team_sync_error))
}

#[tauri::command]
pub fn team_sync_get_state(db: State<'_, Database>) -> Result<TeamSyncGetStateResponse, AppError> {
    with_team_sync_connection(db.inner(), team_sync_get_state_for_conn)
}

#[tauri::command]
pub fn team_sync_upsert_member(
    db: State<'_, Database>,
    request: TeamSyncUpsertMemberRequest,
) -> Result<TeamMember, AppError> {
    with_team_sync_connection(db.inner(), |conn| {
        team_sync_upsert_member_for_conn(conn, request)
    })
}

#[tauri::command]
pub fn team_sync_check_access(
    db: State<'_, Database>,
    request: TeamSyncCheckAccessRequest,
) -> Result<TeamSyncAccessDecision, AppError> {
    with_team_sync_connection(db.inner(), |conn| {
        team_sync_check_access_for_conn(conn, request)
    })
}

#[tauri::command]
pub fn team_sync_export_bundle(
    db: State<'_, Database>,
    request: TeamSyncExportBundleRequest,
) -> Result<TeamSyncBundle, AppError> {
    with_team_sync_connection(db.inner(), |conn| {
        team_sync_export_bundle_for_conn(conn, request)
    })
}

#[tauri::command]
pub fn team_sync_export_audit(
    db: State<'_, Database>,
    request: TeamSyncExportAuditRequest,
) -> Result<TeamSyncExportAuditResponse, AppError> {
    with_team_sync_connection(db.inner(), |conn| {
        team_sync_export_audit_for_conn(conn, request)
    })
}

#[tauri::command]
pub fn team_sync_import_bundle(
    db: State<'_, Database>,
    request: TeamSyncImportBundleRequest,
) -> Result<TeamSyncGetStateResponse, AppError> {
    with_team_sync_connection(db.inner(), |conn| {
        team_sync_import_bundle_for_conn(conn, request)
    })
}

#[tauri::command]
pub fn team_sync_run_folder_sync(
    db: State<'_, Database>,
    request: TeamSyncRunFolderSyncRequest,
) -> Result<TeamSyncRunFolderSyncResponse, AppError> {
    with_team_sync_connection(db.inner(), |conn| {
        team_sync_run_folder_sync_for_conn(conn, request)
    })
}

impl From<rusqlite::Error> for TeamSyncError {
    fn from(value: rusqlite::Error) -> Self {
        Self::new(format!("sqlite error: {value}"))
    }
}

impl From<serde_json::Error> for TeamSyncError {
    fn from(value: serde_json::Error) -> Self {
        Self::new(format!("json error: {value}"))
    }
}

impl From<std::io::Error> for TeamSyncError {
    fn from(value: std::io::Error) -> Self {
        Self::new(format!("io error: {value}"))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PersistedTeamGovernanceRecord {
    schema_version: u32,
    members: Vec<TeamMember>,
    roles: Vec<TeamRolePolicy>,
    audit_events: Vec<TeamAuditEvent>,
    last_synced_at: Option<String>,
    last_bundle: Option<TeamSyncBundle>,
}

impl Default for PersistedTeamGovernanceRecord {
    fn default() -> Self {
        Self {
            schema_version: TEAM_SYNC_SCHEMA_VERSION,
            members: Vec::new(),
            roles: default_role_policies(),
            audit_events: Vec::new(),
            last_synced_at: None,
            last_bundle: None,
        }
    }
}

pub fn team_sync_get_state_for_conn(
    conn: &rusqlite::Connection,
) -> Result<TeamSyncGetStateResponse, TeamSyncError> {
    Ok(load_record(conn)?.to_state())
}

pub fn team_sync_upsert_member_for_conn(
    conn: &rusqlite::Connection,
    request: TeamSyncUpsertMemberRequest,
) -> Result<TeamMember, TeamSyncError> {
    let actor_member_id = normalize_required("actor_member_id", &request.actor_member_id)?;
    let member_id = normalize_required("member_id", &request.member_id)?;
    let requested_role = request.role;
    let now = now_rfc3339();
    let mut record = load_record(conn)?;

    if record.members.is_empty() {
        let member = TeamMember {
            id: member_id.clone(),
            role: TeamRole::Owner,
            updated_at: now.clone(),
        };
        record.members.push(member.clone());
        sort_members(&mut record.members);
        record.push_audit(
            "bootstrap_owner",
            &actor_member_id,
            Some(&member_id),
            format!(
                "initialized local governance and promoted {member_id} to owner during bootstrap"
            ),
            now.clone(),
        );
        save_record(conn, &record, &now)?;
        return Ok(member);
    }

    let actor_role = actor_role(&record, &actor_member_id);
    if !is_allowed(actor_role.as_ref(), "member", "upsert") {
        record.push_audit(
            "deny_upsert_member",
            &actor_member_id,
            Some(&member_id),
            "actor does not have permission to update team members".to_string(),
            now.clone(),
        );
        save_record(conn, &record, &now)?;
        return Err(TeamSyncError::new(format!(
            "member {} is not allowed to upsert team members",
            actor_member_id
        )));
    }

    let existing_target_role = member_role(&record, &member_id);
    if actor_role != Some(TeamRole::Owner)
        && (requested_role == TeamRole::Owner || existing_target_role == Some(TeamRole::Owner))
    {
        record.push_audit(
            "deny_upsert_member",
            &actor_member_id,
            Some(&member_id),
            "only an owner can create, transfer, or modify owner membership".to_string(),
            now.clone(),
        );
        save_record(conn, &record, &now)?;
        return Err(TeamSyncError::new(
            "only an owner can create, transfer, or modify owner membership",
        ));
    }

    if existing_target_role == Some(TeamRole::Owner)
        && requested_role != TeamRole::Owner
        && owner_count(&record) <= 1
    {
        record.push_audit(
            "deny_upsert_member",
            &actor_member_id,
            Some(&member_id),
            "cannot remove the final owner from local governance".to_string(),
            now.clone(),
        );
        save_record(conn, &record, &now)?;
        return Err(TeamSyncError::new(
            "cannot remove the final owner from local governance",
        ));
    }

    upsert_member_in_record(&mut record, &member_id, requested_role.clone(), &now);
    record.push_audit(
        "upsert_member",
        &actor_member_id,
        Some(&member_id),
        format!("set role for {member_id} to {}", requested_role.as_str()),
        now.clone(),
    );
    save_record(conn, &record, &now)?;

    record
        .members
        .iter()
        .find(|member| member.id == member_id)
        .cloned()
        .ok_or_else(|| TeamSyncError::new("upserted member should exist"))
}

pub fn team_sync_check_access_for_conn(
    conn: &rusqlite::Connection,
    request: TeamSyncCheckAccessRequest,
) -> Result<TeamSyncAccessDecision, TeamSyncError> {
    let actor_member_id = normalize_required("actor_member_id", &request.actor_member_id)?;
    let resource = normalize_required("resource", &request.resource)?;
    let action = normalize_required("action", &request.action)?;
    let record = load_record(conn)?;

    if record.members.is_empty() {
        return Ok(TeamSyncAccessDecision {
            allowed: false,
            reason: "local governance has not been bootstrapped yet".to_string(),
        });
    }

    let Some(role) = actor_role(&record, &actor_member_id) else {
        return Ok(TeamSyncAccessDecision {
            allowed: false,
            reason: format!("member {actor_member_id} is not part of the local team"),
        });
    };

    let allowed = is_allowed(Some(&role), &resource, &action);
    let reason = if allowed {
        format!(
            "role {} allows {}:{} in the local governance service",
            role.as_str(),
            resource,
            action
        )
    } else {
        format!(
            "role {} does not allow {}:{} in the local governance service",
            role.as_str(),
            resource,
            action
        )
    };

    Ok(TeamSyncAccessDecision { allowed, reason })
}

pub fn team_sync_export_bundle_for_conn(
    conn: &rusqlite::Connection,
    request: TeamSyncExportBundleRequest,
) -> Result<TeamSyncBundle, TeamSyncError> {
    let actor_member_id = normalize_required("actor_member_id", &request.actor_member_id)?;
    let now = now_rfc3339();
    let mut record = load_record(conn)?;

    if !record.members.is_empty()
        && !is_allowed(
            actor_role(&record, &actor_member_id).as_ref(),
            "bundle",
            "export",
        )
    {
        record.push_audit(
            "deny_export_bundle",
            &actor_member_id,
            None,
            "actor does not have permission to export a team sync bundle".to_string(),
            now.clone(),
        );
        save_record(conn, &record, &now)?;
        return Err(TeamSyncError::new(format!(
            "member {} is not allowed to export a team sync bundle",
            actor_member_id
        )));
    }

    record.push_audit(
        "export_bundle",
        &actor_member_id,
        None,
        format!(
            "exported a local governance bundle with {} members",
            record.members.len()
        ),
        now.clone(),
    );
    let bundle = record.to_bundle(now.clone());
    record.last_bundle = Some(bundle.clone());
    save_record(conn, &record, &now)?;

    Ok(bundle)
}

pub fn team_sync_export_audit_for_conn(
    conn: &rusqlite::Connection,
    request: TeamSyncExportAuditRequest,
) -> Result<TeamSyncExportAuditResponse, TeamSyncError> {
    let actor_member_id = normalize_required("actor_member_id", &request.actor_member_id)?;
    let actor_filter = normalize_optional(request.actor)?;
    let action_filter = normalize_optional(request.action)?.map(|value| value.to_ascii_lowercase());
    let export_limit = normalize_limit(request.limit);
    let now = now_rfc3339();
    let mut record = load_record(conn)?;

    if !record.members.is_empty()
        && !is_allowed(
            actor_role(&record, &actor_member_id).as_ref(),
            "bundle",
            "export",
        )
    {
        record.push_audit(
            "deny_export_audit",
            &actor_member_id,
            None,
            "actor does not have permission to export local audit events".to_string(),
            now.clone(),
        );
        save_record(conn, &record, &now)?;
        return Err(TeamSyncError::new(format!(
            "member {} is not allowed to export local audit events",
            actor_member_id
        )));
    }

    let total = record
        .audit_events
        .iter()
        .filter(|event| {
            audit_event_matches(event, actor_filter.as_deref(), action_filter.as_deref())
        })
        .count();
    let events: Vec<TeamAuditEvent> = record
        .audit_events
        .iter()
        .filter(|event| {
            audit_event_matches(event, actor_filter.as_deref(), action_filter.as_deref())
        })
        .take(export_limit)
        .cloned()
        .collect();
    let payload = serialize_audit_export_payload(&events, &request.format)?;

    record.push_audit(
        "export_audit_events",
        &actor_member_id,
        None,
        format!(
            "exported {}/{} local audit events as {}",
            events.len(),
            total,
            request.format.as_str()
        ),
        now.clone(),
    );
    save_record(conn, &record, &now)?;

    Ok(TeamSyncExportAuditResponse {
        total,
        exported_count: events.len(),
        payload,
        events,
    })
}

pub fn team_sync_import_bundle_for_conn(
    conn: &rusqlite::Connection,
    request: TeamSyncImportBundleRequest,
) -> Result<TeamSyncGetStateResponse, TeamSyncError> {
    let actor_member_id = normalize_required("actor_member_id", &request.actor_member_id)?;
    let mut bundle = request.bundle;
    let now = now_rfc3339();
    let mut record = load_record(conn)?;

    if bundle.schema_version != TEAM_SYNC_SCHEMA_VERSION {
        return Err(TeamSyncError::new(format!(
            "unsupported team sync bundle schema_version {}; expected {}",
            bundle.schema_version, TEAM_SYNC_SCHEMA_VERSION
        )));
    }

    if !record.members.is_empty()
        && !is_allowed(
            actor_role(&record, &actor_member_id).as_ref(),
            "bundle",
            "import",
        )
    {
        record.push_audit(
            "deny_import_bundle",
            &actor_member_id,
            None,
            "actor does not have permission to import a team sync bundle".to_string(),
            now.clone(),
        );
        save_record(conn, &record, &now)?;
        return Err(TeamSyncError::new(format!(
            "member {} is not allowed to import a team sync bundle",
            actor_member_id
        )));
    }

    if bundle.roles.is_empty() {
        bundle.roles = default_role_policies();
    }

    record.members = merge_members(&record.members, &bundle.members);
    record.roles = bundle.roles.clone();
    record.audit_events = merge_audit_events(&record.audit_events, &bundle.audit_events);

    if !record.members.is_empty()
        && owner_count(&record) == 0
        && let Some(first_member) = record.members.first_mut()
    {
        first_member.role = TeamRole::Owner;
        first_member.updated_at = now.clone();
        let promoted_id = first_member.id.clone();
        record.push_audit(
            "promote_owner",
            &actor_member_id,
            Some(&promoted_id),
            "import produced a team without an owner; promoted the first member locally"
                .to_string(),
            now.clone(),
        );
    }

    record.last_synced_at = Some(now.clone());
    record.last_bundle = Some(bundle.clone());
    record.push_audit(
        "import_bundle",
        &actor_member_id,
        None,
        format!(
            "merged {} members and {} imported audit events from a local sync bundle",
            bundle.members.len(),
            bundle.audit_events.len()
        ),
        now.clone(),
    );
    save_record(conn, &record, &now)?;

    Ok(record.to_state())
}

pub fn team_sync_run_folder_sync_for_conn(
    conn: &rusqlite::Connection,
    request: TeamSyncRunFolderSyncRequest,
) -> Result<TeamSyncRunFolderSyncResponse, TeamSyncError> {
    let actor_member_id = normalize_required("actor_member_id", &request.actor_member_id)?;
    let Some(file_path) = request.file_path else {
        return Ok(TeamSyncRunFolderSyncResponse {
            state: team_sync_get_state_for_conn(conn)?,
            bundle: None,
        });
    };

    let path = std::path::PathBuf::from(file_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    if path.exists() {
        let raw = fs::read_to_string(&path)?;
        if !raw.trim().is_empty() {
            let bundle = serde_json::from_str::<TeamSyncBundle>(&raw)?;
            team_sync_import_bundle_for_conn(
                conn,
                TeamSyncImportBundleRequest {
                    actor_member_id: actor_member_id.clone(),
                    bundle,
                },
            )?;
        }
    }

    let bundle = team_sync_export_bundle_for_conn(
        conn,
        TeamSyncExportBundleRequest {
            actor_member_id: actor_member_id.clone(),
        },
    )?;
    fs::write(&path, serde_json::to_string_pretty(&bundle)?)?;

    let now = now_rfc3339();
    let mut record = load_record(conn)?;
    record.last_synced_at = Some(now.clone());
    record.last_bundle = Some(bundle.clone());
    record.push_audit(
        "folder_sync_write",
        &actor_member_id,
        None,
        format!(
            "synchronized local governance through bundle file {}",
            path.display()
        ),
        now.clone(),
    );
    save_record(conn, &record, &now)?;

    Ok(TeamSyncRunFolderSyncResponse {
        state: record.to_state(),
        bundle: Some(bundle),
    })
}

impl TeamRole {
    fn as_str(&self) -> &'static str {
        match self {
            TeamRole::Owner => "owner",
            TeamRole::Admin => "admin",
            TeamRole::Editor => "editor",
            TeamRole::Viewer => "viewer",
        }
    }
}

impl TeamSyncAuditExportFormat {
    fn as_str(&self) -> &'static str {
        match self {
            TeamSyncAuditExportFormat::Json => "json",
            TeamSyncAuditExportFormat::Jsonl => "jsonl",
        }
    }
}

impl PersistedTeamGovernanceRecord {
    fn to_state(&self) -> TeamSyncGetStateResponse {
        TeamSyncGetStateResponse {
            schema_version: self.schema_version,
            members: self.members.clone(),
            roles: self.roles.clone(),
            audit_events: self.audit_events.clone(),
            last_synced_at: self.last_synced_at.clone(),
        }
    }

    fn to_bundle(&self, exported_at: String) -> TeamSyncBundle {
        TeamSyncBundle {
            schema_version: self.schema_version,
            exported_at,
            members: self.members.clone(),
            roles: self.roles.clone(),
            audit_events: self
                .audit_events
                .iter()
                .take(TEAM_SYNC_EXPORT_AUDIT_TAIL)
                .cloned()
                .collect(),
        }
    }

    fn push_audit(
        &mut self,
        action: &str,
        actor_member_id: &str,
        subject_member_id: Option<&str>,
        detail: String,
        at: String,
    ) {
        self.audit_events.insert(
            0,
            TeamAuditEvent {
                id: uuid::Uuid::new_v4().to_string(),
                at,
                action: action.to_string(),
                actor_member_id: actor_member_id.to_string(),
                subject_member_id: subject_member_id.map(str::to_string),
                detail,
            },
        );
        self.audit_events.truncate(TEAM_SYNC_AUDIT_LIMIT);
    }
}

fn normalize_required(field: &str, value: &str) -> Result<String, TeamSyncError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(TeamSyncError::new(format!("{field} cannot be empty")));
    }
    Ok(normalized.to_string())
}

fn normalize_optional(value: Option<String>) -> Result<Option<String>, TeamSyncError> {
    value
        .map(|candidate| normalize_required("filter", &candidate))
        .transpose()
}

fn normalize_limit(limit: Option<usize>) -> usize {
    match limit {
        Some(0) | None => TEAM_SYNC_AUDIT_LIMIT,
        Some(value) => value.min(TEAM_SYNC_AUDIT_LIMIT),
    }
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn ensure_app_settings_table(conn: &rusqlite::Connection) -> Result<(), TeamSyncError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS app_settings (
            key TEXT PRIMARY KEY,
            value_json TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );",
    )?;
    Ok(())
}

fn load_record(
    conn: &rusqlite::Connection,
) -> Result<PersistedTeamGovernanceRecord, TeamSyncError> {
    ensure_app_settings_table(conn)?;
    let json = conn
        .query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [TEAM_SYNC_SETTINGS_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    let mut record = match json {
        Some(value) => serde_json::from_str::<PersistedTeamGovernanceRecord>(&value)?,
        None => PersistedTeamGovernanceRecord::default(),
    };

    if record.roles.is_empty() {
        record.roles = default_role_policies();
    }
    sort_members(&mut record.members);
    Ok(record)
}

fn save_record(
    conn: &rusqlite::Connection,
    record: &PersistedTeamGovernanceRecord,
    updated_at: &str,
) -> Result<(), TeamSyncError> {
    ensure_app_settings_table(conn)?;
    let value_json = serde_json::to_string(record)?;
    conn.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
        (TEAM_SYNC_SETTINGS_KEY, value_json, updated_at),
    )?;
    Ok(())
}

fn default_role_policies() -> Vec<TeamRolePolicy> {
    vec![
        TeamRolePolicy {
            role: TeamRole::Owner,
            allowed: vec!["*:*".to_string()],
        },
        TeamRolePolicy {
            role: TeamRole::Admin,
            allowed: vec![
                "audit:read".to_string(),
                "bundle:export".to_string(),
                "bundle:import".to_string(),
                "folder_sync:run".to_string(),
                "member:read".to_string(),
                "member:upsert".to_string(),
                "team:read".to_string(),
            ],
        },
        TeamRolePolicy {
            role: TeamRole::Editor,
            allowed: vec![
                "audit:read".to_string(),
                "bundle:export".to_string(),
                "folder_sync:run".to_string(),
                "member:read".to_string(),
                "team:read".to_string(),
            ],
        },
        TeamRolePolicy {
            role: TeamRole::Viewer,
            allowed: vec!["member:read".to_string(), "team:read".to_string()],
        },
    ]
}

fn actor_role(record: &PersistedTeamGovernanceRecord, member_id: &str) -> Option<TeamRole> {
    record
        .members
        .iter()
        .find(|member| member.id == member_id)
        .map(|member| member.role.clone())
}

fn member_role(record: &PersistedTeamGovernanceRecord, member_id: &str) -> Option<TeamRole> {
    actor_role(record, member_id)
}

fn owner_count(record: &PersistedTeamGovernanceRecord) -> usize {
    record
        .members
        .iter()
        .filter(|member| member.role == TeamRole::Owner)
        .count()
}

fn sort_members(members: &mut [TeamMember]) {
    members.sort_by(|left, right| left.id.cmp(&right.id));
}

fn is_allowed(role: Option<&TeamRole>, resource: &str, action: &str) -> bool {
    let resource = resource.trim().to_ascii_lowercase();
    let action = action.trim().to_ascii_lowercase();

    match role {
        Some(TeamRole::Owner) => true,
        Some(TeamRole::Admin) => matches!(
            (resource.as_str(), action.as_str()),
            ("team", "read")
                | ("member", "read")
                | ("member", "upsert")
                | ("audit", "read")
                | ("bundle", "export")
                | ("bundle", "import")
                | ("folder_sync", "run")
        ),
        Some(TeamRole::Editor) => matches!(
            (resource.as_str(), action.as_str()),
            ("team", "read")
                | ("member", "read")
                | ("audit", "read")
                | ("bundle", "export")
                | ("folder_sync", "run")
        ),
        Some(TeamRole::Viewer) => {
            matches!(
                (resource.as_str(), action.as_str()),
                ("team", "read") | ("member", "read")
            )
        }
        None => false,
    }
}

fn upsert_member_in_record(
    record: &mut PersistedTeamGovernanceRecord,
    member_id: &str,
    role: TeamRole,
    updated_at: &str,
) {
    if let Some(existing) = record
        .members
        .iter_mut()
        .find(|member| member.id == member_id)
    {
        existing.role = role;
        existing.updated_at = updated_at.to_string();
    } else {
        record.members.push(TeamMember {
            id: member_id.to_string(),
            role,
            updated_at: updated_at.to_string(),
        });
        sort_members(&mut record.members);
    }
}

fn merge_members(local: &[TeamMember], imported: &[TeamMember]) -> Vec<TeamMember> {
    let mut members: BTreeMap<String, TeamMember> = BTreeMap::new();

    for candidate in local.iter().chain(imported.iter()) {
        match members.get(&candidate.id) {
            Some(existing) if existing.updated_at > candidate.updated_at => {}
            Some(existing) if existing.updated_at == candidate.updated_at => {
                if existing.role == TeamRole::Owner && candidate.role != TeamRole::Owner {
                    continue;
                }
                members.insert(candidate.id.clone(), candidate.clone());
            }
            _ => {
                members.insert(candidate.id.clone(), candidate.clone());
            }
        }
    }

    members.into_values().collect()
}

fn merge_audit_events(
    local: &[TeamAuditEvent],
    imported: &[TeamAuditEvent],
) -> Vec<TeamAuditEvent> {
    let mut merged: HashMap<String, TeamAuditEvent> = HashMap::new();

    for event in local.iter().chain(imported.iter()) {
        match merged.get(&event.id) {
            Some(existing) if existing.at >= event.at => {}
            _ => {
                merged.insert(event.id.clone(), event.clone());
            }
        }
    }

    let mut values: Vec<TeamAuditEvent> = merged.into_values().collect();
    values.sort_by(|left, right| right.at.cmp(&left.at).then_with(|| right.id.cmp(&left.id)));
    values.truncate(TEAM_SYNC_AUDIT_LIMIT);
    values
}

fn audit_event_matches(
    event: &TeamAuditEvent,
    actor_filter: Option<&str>,
    action_filter: Option<&str>,
) -> bool {
    actor_filter
        .map(|actor| event.actor_member_id == actor)
        .unwrap_or(true)
        && action_filter
            .map(|action| event.action.eq_ignore_ascii_case(action))
            .unwrap_or(true)
}

fn serialize_audit_export_payload(
    events: &[TeamAuditEvent],
    format: &TeamSyncAuditExportFormat,
) -> Result<String, TeamSyncError> {
    match format {
        TeamSyncAuditExportFormat::Json => Ok(serde_json::to_string_pretty(events)?),
        TeamSyncAuditExportFormat::Jsonl => {
            let mut lines = Vec::with_capacity(events.len());
            for event in events {
                lines.push(serde_json::to_string(event)?);
            }
            Ok(lines.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        TeamRole, TeamSyncAuditExportFormat, TeamSyncCheckAccessRequest,
        TeamSyncExportAuditRequest, TeamSyncExportBundleRequest, TeamSyncImportBundleRequest,
        TeamSyncRunFolderSyncRequest, TeamSyncUpsertMemberRequest, team_sync_check_access_for_conn,
        team_sync_export_audit_for_conn, team_sync_export_bundle_for_conn,
        team_sync_get_state_for_conn, team_sync_import_bundle_for_conn,
        team_sync_run_folder_sync_for_conn, team_sync_upsert_member_for_conn,
    };
    use rusqlite::Connection;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn in_memory_connection() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory sqlite should open");
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );",
        )
        .expect("app_settings table should exist");
        conn
    }

    fn unique_temp_path() -> std::path::PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be valid")
            .as_nanos();
        std::env::temp_dir().join(format!("team-sync-{timestamp}.json"))
    }

    #[test]
    fn team_sync_enforces_rbac_and_records_audit_events() {
        let conn = in_memory_connection();

        let bootstrap = team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "alice".to_string(),
                role: TeamRole::Viewer,
            },
        )
        .expect("bootstrap upsert should succeed");
        assert_eq!(bootstrap.role, TeamRole::Owner);

        team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "bob".to_string(),
                role: TeamRole::Admin,
            },
        )
        .expect("owner should add admin");
        team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "carol".to_string(),
                role: TeamRole::Editor,
            },
        )
        .expect("owner should add editor");
        team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "dave".to_string(),
                role: TeamRole::Viewer,
            },
        )
        .expect("owner should add viewer");

        let denied = team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "bob".to_string(),
                member_id: "erin".to_string(),
                role: TeamRole::Owner,
            },
        )
        .expect_err("admin should not assign owner");
        assert!(
            denied.message.contains("owner"),
            "expected owner-specific denial, got {}",
            denied.message
        );

        let owner_access = team_sync_check_access_for_conn(
            &conn,
            TeamSyncCheckAccessRequest {
                actor_member_id: "alice".to_string(),
                resource: "member".to_string(),
                action: "upsert".to_string(),
            },
        )
        .expect("owner access should resolve");
        assert!(owner_access.allowed);

        let admin_access = team_sync_check_access_for_conn(
            &conn,
            TeamSyncCheckAccessRequest {
                actor_member_id: "bob".to_string(),
                resource: "bundle".to_string(),
                action: "import".to_string(),
            },
        )
        .expect("admin access should resolve");
        assert!(admin_access.allowed);

        let editor_access = team_sync_check_access_for_conn(
            &conn,
            TeamSyncCheckAccessRequest {
                actor_member_id: "carol".to_string(),
                resource: "member".to_string(),
                action: "upsert".to_string(),
            },
        )
        .expect("editor access should resolve");
        assert!(!editor_access.allowed);

        let viewer_access = team_sync_check_access_for_conn(
            &conn,
            TeamSyncCheckAccessRequest {
                actor_member_id: "dave".to_string(),
                resource: "team".to_string(),
                action: "read".to_string(),
            },
        )
        .expect("viewer access should resolve");
        assert!(viewer_access.allowed);

        let state = team_sync_get_state_for_conn(&conn).expect("state should load");
        assert_eq!(state.members.len(), 4);
        assert_eq!(state.audit_events.len(), 5);
        assert_eq!(state.audit_events[0].action, "deny_upsert_member");
        assert_eq!(state.audit_events[1].action, "upsert_member");
        assert_eq!(
            state.audit_events.last().map(|event| event.action.as_str()),
            Some("bootstrap_owner")
        );
    }

    #[test]
    fn team_sync_export_import_round_trip_preserves_members_and_audit_tail() {
        let source = in_memory_connection();

        team_sync_upsert_member_for_conn(
            &source,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "alice".to_string(),
                role: TeamRole::Owner,
            },
        )
        .expect("bootstrap should succeed");
        team_sync_upsert_member_for_conn(
            &source,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "bob".to_string(),
                role: TeamRole::Editor,
            },
        )
        .expect("owner should add editor");

        let exported = team_sync_export_bundle_for_conn(
            &source,
            TeamSyncExportBundleRequest {
                actor_member_id: "alice".to_string(),
            },
        )
        .expect("bundle export should succeed");
        assert_eq!(exported.members.len(), 2);
        assert!(exported.audit_events.len() >= 3);

        let target = in_memory_connection();
        let imported = team_sync_import_bundle_for_conn(
            &target,
            TeamSyncImportBundleRequest {
                actor_member_id: "bootstrap-import".to_string(),
                bundle: exported.clone(),
            },
        )
        .expect("bundle import should succeed for empty state");

        assert_eq!(imported.members, exported.members);
        assert!(imported.audit_events.len() > exported.audit_events.len());
        for event in &exported.audit_events {
            assert!(
                imported
                    .audit_events
                    .iter()
                    .any(|candidate| candidate.id == event.id),
                "import should preserve audit event {}",
                event.id
            );
        }
        assert_eq!(
            imported.audit_events[0].action, "import_bundle",
            "import audit should be newest"
        );
        assert_eq!(
            imported.last_synced_at,
            Some(imported.audit_events[0].at.clone())
        );

        let round_trip_bundle = team_sync_export_bundle_for_conn(
            &target,
            TeamSyncExportBundleRequest {
                actor_member_id: "alice".to_string(),
            },
        )
        .expect("round-trip export should succeed");
        assert_eq!(round_trip_bundle.members, exported.members);
        assert_eq!(round_trip_bundle.roles, exported.roles);
    }

    #[test]
    fn team_sync_run_folder_sync_reads_and_writes_local_bundle_file() {
        let conn = in_memory_connection();
        let path = unique_temp_path();

        team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "alice".to_string(),
                role: TeamRole::Owner,
            },
        )
        .expect("bootstrap should succeed");
        team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "bob".to_string(),
                role: TeamRole::Viewer,
            },
        )
        .expect("owner should add viewer");

        let first_sync = team_sync_run_folder_sync_for_conn(
            &conn,
            TeamSyncRunFolderSyncRequest {
                actor_member_id: "alice".to_string(),
                file_path: Some(path.display().to_string()),
            },
        )
        .expect("initial folder sync should write bundle");
        assert!(first_sync.bundle.is_some());
        assert!(path.exists());

        let serialized = fs::read_to_string(&path).expect("bundle file should exist");
        assert!(
            serialized.contains("\"members\""),
            "bundle file should contain members json"
        );

        let imported_bundle = team_sync_export_bundle_for_conn(
            &conn,
            TeamSyncExportBundleRequest {
                actor_member_id: "alice".to_string(),
            },
        )
        .expect("bundle export should succeed");

        let peer = in_memory_connection();
        team_sync_import_bundle_for_conn(
            &peer,
            TeamSyncImportBundleRequest {
                actor_member_id: "peer-bootstrap".to_string(),
                bundle: imported_bundle,
            },
        )
        .expect("peer should import bundle");
        team_sync_upsert_member_for_conn(
            &peer,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "carol".to_string(),
                role: TeamRole::Admin,
            },
        )
        .expect("peer owner should add admin");

        let peer_sync = team_sync_run_folder_sync_for_conn(
            &peer,
            TeamSyncRunFolderSyncRequest {
                actor_member_id: "alice".to_string(),
                file_path: Some(path.display().to_string()),
            },
        )
        .expect("peer sync should merge into folder bundle");
        assert!(
            peer_sync
                .state
                .members
                .iter()
                .any(|member| member.id == "carol" && member.role == TeamRole::Admin)
        );

        let local_sync = team_sync_run_folder_sync_for_conn(
            &conn,
            TeamSyncRunFolderSyncRequest {
                actor_member_id: "alice".to_string(),
                file_path: Some(path.display().to_string()),
            },
        )
        .expect("local sync should pull peer member");
        assert!(
            local_sync
                .state
                .members
                .iter()
                .any(|member| member.id == "carol" && member.role == TeamRole::Admin)
        );

        fs::remove_file(path).expect("temp bundle should clean up");
    }

    #[test]
    fn team_sync_export_audit_filters_json_and_records_local_export_event() {
        let conn = in_memory_connection();

        team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "alice".to_string(),
                role: TeamRole::Owner,
            },
        )
        .expect("bootstrap should succeed");
        team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "bob".to_string(),
                role: TeamRole::Editor,
            },
        )
        .expect("owner should add editor");
        team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "carol".to_string(),
                role: TeamRole::Viewer,
            },
        )
        .expect("owner should add viewer");

        let denied = team_sync_export_audit_for_conn(
            &conn,
            TeamSyncExportAuditRequest {
                actor_member_id: "carol".to_string(),
                actor: None,
                action: None,
                limit: None,
                format: TeamSyncAuditExportFormat::Json,
            },
        )
        .expect_err("viewer should not export audit events");
        assert!(
            denied.message.contains("not allowed"),
            "expected permission denial, got {}",
            denied.message
        );

        let before_export = team_sync_get_state_for_conn(&conn).expect("state should load");
        let response = team_sync_export_audit_for_conn(
            &conn,
            TeamSyncExportAuditRequest {
                actor_member_id: "bob".to_string(),
                actor: Some("alice".to_string()),
                action: Some("upsert_member".to_string()),
                limit: Some(2),
                format: TeamSyncAuditExportFormat::Json,
            },
        )
        .expect("editor should export filtered audit events");

        assert_eq!(response.total, 2);
        assert_eq!(response.exported_count, 2);
        assert_eq!(response.events.len(), 2);
        assert!(
            response
                .events
                .iter()
                .all(|event| event.actor_member_id == "alice" && event.action == "upsert_member")
        );

        let payload_events: Vec<serde_json::Value> =
            serde_json::from_str(&response.payload).expect("json payload should deserialize");
        assert_eq!(payload_events.len(), 2);
        assert_eq!(payload_events[0]["action"], "upsert_member");

        let after_export = team_sync_get_state_for_conn(&conn).expect("state should reload");
        assert_eq!(
            after_export.audit_events.len(),
            before_export.audit_events.len() + 1
        );
        assert_eq!(after_export.audit_events[0].action, "export_audit_events");
        assert_eq!(after_export.audit_events[0].actor_member_id, "bob");
        assert_eq!(after_export.audit_events[1].action, "deny_export_audit");
        assert!(
            response
                .events
                .iter()
                .all(|event| event.action != "export_audit_events"),
            "export metadata should not be injected into exported payload"
        );
    }

    #[test]
    fn team_sync_export_audit_supports_jsonl_and_limit_filters() {
        let conn = in_memory_connection();

        team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "alice".to_string(),
                role: TeamRole::Owner,
            },
        )
        .expect("bootstrap should succeed");
        team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "bob".to_string(),
                role: TeamRole::Admin,
            },
        )
        .expect("owner should add admin");
        team_sync_upsert_member_for_conn(
            &conn,
            TeamSyncUpsertMemberRequest {
                actor_member_id: "alice".to_string(),
                member_id: "carol".to_string(),
                role: TeamRole::Viewer,
            },
        )
        .expect("owner should add viewer");

        let response = team_sync_export_audit_for_conn(
            &conn,
            TeamSyncExportAuditRequest {
                actor_member_id: "bob".to_string(),
                actor: Some("alice".to_string()),
                action: None,
                limit: Some(1),
                format: TeamSyncAuditExportFormat::Jsonl,
            },
        )
        .expect("admin should export audit events as jsonl");

        assert_eq!(response.total, 3);
        assert_eq!(response.exported_count, 1);
        assert_eq!(response.events.len(), 1);
        let jsonl_lines: Vec<&str> = response.payload.lines().collect();
        assert_eq!(jsonl_lines.len(), 1);

        let event: serde_json::Value =
            serde_json::from_str(jsonl_lines[0]).expect("jsonl line should be valid json");
        assert_eq!(event["actor_member_id"], "alice");
    }
}
