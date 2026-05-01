//! Local agent exchange mailbox backed by `app_settings`.
//!
//! This is intentionally local-first: it prepares and imports JSON bundles for
//! future remote users and their agents, but does not call a remote service or
//! claim realtime delivery.

use crate::backend::{AppError, AppResult, Database};
use chrono::{DateTime, Utc};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tauri::State;
use uuid::Uuid;

const AGENT_EXCHANGE_SETTINGS_KEY: &str = "agent_exchange.mailbox";
const AGENT_EXCHANGE_SCHEMA_VERSION: u32 = 1;
const AGENT_EXCHANGE_MESSAGE_LIMIT: usize = 200;
const AGENT_EXCHANGE_REMOTE_USER_LIMIT: usize = 200;
const AGENT_EXCHANGE_DEFAULT_LIST_LIMIT: usize = 50;
const AGENT_EXCHANGE_MAX_LIST_LIMIT: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentExchangeDirection {
    Inbound,
    Outbound,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentExchangeMessageStatus {
    Draft,
    Sent,
    Received,
    Archived,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AgentExchangeRemoteUserStatus {
    Active,
    Paused,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentExchangeRemoteUser {
    pub user_id: String,
    pub display_name: String,
    pub default_agent_id: String,
    pub transport_label: Option<String>,
    pub route_hint: Option<String>,
    pub status: AgentExchangeRemoteUserStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeMessage {
    pub id: String,
    pub thread_id: String,
    pub direction: AgentExchangeDirection,
    pub local_agent_id: String,
    pub remote_agent_id: String,
    pub remote_user_id: Option<String>,
    pub subject: Option<String>,
    pub body: String,
    pub payload_json: Option<serde_json::Value>,
    pub status: AgentExchangeMessageStatus,
    pub source_message_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeState {
    pub schema_version: u32,
    pub messages: Vec<AgentExchangeMessage>,
    #[serde(default)]
    pub remote_users: Vec<AgentExchangeRemoteUser>,
    pub last_imported_at: Option<String>,
    pub last_exported_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeBundle {
    pub schema_version: u32,
    pub exported_at: String,
    pub messages: Vec<AgentExchangeMessage>,
    #[serde(default)]
    pub remote_users: Vec<AgentExchangeRemoteUser>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeListRequest {
    pub direction: Option<String>,
    pub status: Option<String>,
    pub thread_id: Option<String>,
    pub remote_agent_id: Option<String>,
    pub remote_user_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeListRemoteUsersRequest {
    pub query: Option<String>,
    pub status: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeUpsertRemoteUserRequest {
    pub user_id: String,
    pub display_name: String,
    pub default_agent_id: String,
    pub transport_label: Option<String>,
    pub route_hint: Option<String>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeDeleteRemoteUserRequest {
    pub user_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeDraftOutboundRequest {
    pub local_agent_id: String,
    pub remote_agent_id: String,
    pub remote_user_id: Option<String>,
    pub thread_id: Option<String>,
    pub subject: Option<String>,
    pub body: String,
    pub payload_json: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeIngestInboundRequest {
    pub local_agent_id: String,
    pub remote_agent_id: String,
    pub remote_user_id: Option<String>,
    pub thread_id: Option<String>,
    pub subject: Option<String>,
    pub body: String,
    pub payload_json: Option<serde_json::Value>,
    pub source_message_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeExportBundleRequest {
    pub direction: Option<String>,
    pub status: Option<String>,
    pub thread_id: Option<String>,
    pub remote_agent_id: Option<String>,
    pub remote_user_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeImportBundleRequest {
    pub bundle: AgentExchangeBundle,
    pub local_agent_id: Option<String>,
    pub as_inbound: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeUpdateMessageStatusRequest {
    pub message_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeDeleteMessageRequest {
    pub message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeImportBundleResponse {
    pub state: AgentExchangeState,
    pub imported_count: usize,
    pub skipped_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeFolderSyncRequest {
    pub path: String,
    pub local_agent_id: Option<String>,
    pub as_inbound: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentExchangeFolderSyncResponse {
    pub state: AgentExchangeState,
    pub imported_count: usize,
    pub skipped_count: usize,
    pub exported_count: usize,
    pub path: String,
    pub synced_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct PersistedAgentExchangeMailbox {
    schema_version: u32,
    messages: Vec<AgentExchangeMessage>,
    #[serde(default)]
    remote_users: Vec<AgentExchangeRemoteUser>,
    last_imported_at: Option<String>,
    last_exported_at: Option<String>,
}

impl Default for PersistedAgentExchangeMailbox {
    fn default() -> Self {
        Self {
            schema_version: AGENT_EXCHANGE_SCHEMA_VERSION,
            messages: Vec::new(),
            remote_users: Vec::new(),
            last_imported_at: None,
            last_exported_at: None,
        }
    }
}

#[tauri::command]
pub fn agent_exchange_get_state(db: State<'_, Database>) -> AppResult<AgentExchangeState> {
    agent_exchange_get_state_for_db(db.inner())
}

#[tauri::command]
pub fn agent_exchange_list_remote_users(
    db: State<'_, Database>,
    request: AgentExchangeListRemoteUsersRequest,
) -> AppResult<Vec<AgentExchangeRemoteUser>> {
    agent_exchange_list_remote_users_for_db(db.inner(), request)
}

#[tauri::command]
pub fn agent_exchange_upsert_remote_user(
    db: State<'_, Database>,
    request: AgentExchangeUpsertRemoteUserRequest,
) -> AppResult<AgentExchangeRemoteUser> {
    agent_exchange_upsert_remote_user_for_db(db.inner(), request)
}

#[tauri::command]
pub fn agent_exchange_delete_remote_user(
    db: State<'_, Database>,
    request: AgentExchangeDeleteRemoteUserRequest,
) -> AppResult<AgentExchangeState> {
    agent_exchange_delete_remote_user_for_db(db.inner(), request.user_id)
}

#[tauri::command]
pub fn agent_exchange_list_messages(
    db: State<'_, Database>,
    request: AgentExchangeListRequest,
) -> AppResult<Vec<AgentExchangeMessage>> {
    agent_exchange_list_messages_for_db(db.inner(), request)
}

#[tauri::command]
pub fn agent_exchange_draft_outbound(
    db: State<'_, Database>,
    request: AgentExchangeDraftOutboundRequest,
) -> AppResult<AgentExchangeMessage> {
    agent_exchange_draft_outbound_for_db(db.inner(), request)
}

#[tauri::command]
pub fn agent_exchange_ingest_inbound(
    db: State<'_, Database>,
    request: AgentExchangeIngestInboundRequest,
) -> AppResult<AgentExchangeMessage> {
    agent_exchange_ingest_inbound_for_db(db.inner(), request)
}

#[tauri::command]
pub fn agent_exchange_export_bundle(
    db: State<'_, Database>,
    request: AgentExchangeExportBundleRequest,
) -> AppResult<AgentExchangeBundle> {
    agent_exchange_export_bundle_for_db(db.inner(), request)
}

#[tauri::command]
pub fn agent_exchange_import_bundle(
    db: State<'_, Database>,
    request: AgentExchangeImportBundleRequest,
) -> AppResult<AgentExchangeImportBundleResponse> {
    agent_exchange_import_bundle_for_db(db.inner(), request)
}

#[tauri::command]
pub fn agent_exchange_update_message_status(
    db: State<'_, Database>,
    request: AgentExchangeUpdateMessageStatusRequest,
) -> AppResult<AgentExchangeMessage> {
    agent_exchange_update_message_status_for_db(db.inner(), request)
}

#[tauri::command]
pub fn agent_exchange_delete_message(
    db: State<'_, Database>,
    request: AgentExchangeDeleteMessageRequest,
) -> AppResult<AgentExchangeState> {
    agent_exchange_delete_message_for_db(db.inner(), request.message_id)
}

#[tauri::command]
pub fn agent_exchange_run_folder_sync(
    db: State<'_, Database>,
    request: AgentExchangeFolderSyncRequest,
) -> AppResult<AgentExchangeFolderSyncResponse> {
    agent_exchange_run_folder_sync_for_db(db.inner(), request)
}

pub fn agent_exchange_get_state_for_db(db: &Database) -> AppResult<AgentExchangeState> {
    Ok(load_mailbox(db)?.to_state())
}

pub fn agent_exchange_list_messages_for_db(
    db: &Database,
    request: AgentExchangeListRequest,
) -> AppResult<Vec<AgentExchangeMessage>> {
    let mailbox = load_mailbox(db)?;
    let filter = MessageFilter::from_list_request(request)?;
    Ok(filter_messages(&mailbox.messages, &filter))
}

pub fn agent_exchange_list_remote_users_for_db(
    db: &Database,
    request: AgentExchangeListRemoteUsersRequest,
) -> AppResult<Vec<AgentExchangeRemoteUser>> {
    let mailbox = load_mailbox(db)?;
    let filter = RemoteUserFilter::from_list_request(request)?;
    Ok(filter_remote_users(&mailbox.remote_users, &filter))
}

pub fn agent_exchange_upsert_remote_user_for_db(
    db: &Database,
    request: AgentExchangeUpsertRemoteUserRequest,
) -> AppResult<AgentExchangeRemoteUser> {
    let user_id = normalize_required("user_id", &request.user_id)?;
    let display_name = normalize_required("display_name", &request.display_name)?;
    let default_agent_id = normalize_required("default_agent_id", &request.default_agent_id)?;
    let transport_label = normalize_optional(request.transport_label);
    let route_hint = normalize_optional(request.route_hint);
    let status =
        parse_remote_user_status(request.status)?.unwrap_or(AgentExchangeRemoteUserStatus::Active);
    let now = now_rfc3339();
    let mut mailbox = load_mailbox(db)?;

    let remote_user = if let Some(existing) = mailbox
        .remote_users
        .iter_mut()
        .find(|remote_user| remote_user.user_id == user_id)
    {
        existing.display_name = display_name;
        existing.default_agent_id = default_agent_id;
        existing.transport_label = transport_label;
        existing.route_hint = route_hint;
        existing.status = status;
        existing.updated_at = now.clone();
        existing.clone()
    } else {
        let remote_user = AgentExchangeRemoteUser {
            user_id,
            display_name,
            default_agent_id,
            transport_label,
            route_hint,
            status,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        mailbox.remote_users.push(remote_user.clone());
        remote_user
    };

    trim_remote_users(&mut mailbox.remote_users);
    save_mailbox(db, &mailbox, &now)?;
    Ok(remote_user)
}

pub fn agent_exchange_delete_remote_user_for_db(
    db: &Database,
    user_id: String,
) -> AppResult<AgentExchangeState> {
    let user_id = normalize_required("user_id", &user_id)?;
    let mut mailbox = load_mailbox(db)?;
    let original_len = mailbox.remote_users.len();
    mailbox
        .remote_users
        .retain(|remote_user| remote_user.user_id != user_id);
    if mailbox.remote_users.len() == original_len {
        return Err(AppError::validation(format!(
            "agent exchange remote user `{}` was not found",
            user_id
        )));
    }

    let now = now_rfc3339();
    save_mailbox(db, &mailbox, &now)?;
    Ok(mailbox.to_state())
}

pub fn agent_exchange_draft_outbound_for_db(
    db: &Database,
    request: AgentExchangeDraftOutboundRequest,
) -> AppResult<AgentExchangeMessage> {
    let local_agent_id = normalize_required("local_agent_id", &request.local_agent_id)?;
    let remote_agent_id = normalize_required("remote_agent_id", &request.remote_agent_id)?;
    let body = normalize_required("body", &request.body)?;
    let now = now_rfc3339();
    let message = AgentExchangeMessage {
        id: Uuid::new_v4().to_string(),
        thread_id: normalize_optional(request.thread_id)
            .unwrap_or_else(|| format!("thread-{}", Uuid::new_v4())),
        direction: AgentExchangeDirection::Outbound,
        local_agent_id,
        remote_agent_id,
        remote_user_id: normalize_optional(request.remote_user_id),
        subject: normalize_optional(request.subject),
        body,
        payload_json: request.payload_json,
        status: AgentExchangeMessageStatus::Draft,
        source_message_id: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    let mut mailbox = load_mailbox(db)?;
    mailbox.messages.insert(0, message.clone());
    trim_messages(&mut mailbox.messages);
    save_mailbox(db, &mailbox, &now)?;
    Ok(message)
}

pub fn agent_exchange_ingest_inbound_for_db(
    db: &Database,
    request: AgentExchangeIngestInboundRequest,
) -> AppResult<AgentExchangeMessage> {
    let local_agent_id = normalize_required("local_agent_id", &request.local_agent_id)?;
    let remote_agent_id = normalize_required("remote_agent_id", &request.remote_agent_id)?;
    let body = normalize_required("body", &request.body)?;
    let source_message_id = normalize_optional(request.source_message_id);
    let now = now_rfc3339();
    let message = AgentExchangeMessage {
        id: Uuid::new_v4().to_string(),
        thread_id: normalize_optional(request.thread_id)
            .unwrap_or_else(|| format!("thread-{}", Uuid::new_v4())),
        direction: AgentExchangeDirection::Inbound,
        local_agent_id,
        remote_agent_id,
        remote_user_id: normalize_optional(request.remote_user_id),
        subject: normalize_optional(request.subject),
        body,
        payload_json: request.payload_json,
        status: AgentExchangeMessageStatus::Received,
        source_message_id,
        created_at: now.clone(),
        updated_at: now.clone(),
    };

    let mut mailbox = load_mailbox(db)?;
    if is_duplicate_import(&mailbox.messages, &message) {
        return Err(AppError::validation(
            "agent exchange message already exists",
        ));
    }
    mailbox.messages.insert(0, message.clone());
    trim_messages(&mut mailbox.messages);
    save_mailbox(db, &mailbox, &now)?;
    Ok(message)
}

pub fn agent_exchange_export_bundle_for_db(
    db: &Database,
    request: AgentExchangeExportBundleRequest,
) -> AppResult<AgentExchangeBundle> {
    let mut mailbox = load_mailbox(db)?;
    let now = now_rfc3339();
    mailbox.last_exported_at = Some(now.clone());
    save_mailbox(db, &mailbox, &now)?;
    let filter = MessageFilter::from_export_request(request)?;
    let messages = filter_messages(&mailbox.messages, &filter);
    let remote_users = collect_export_remote_users(
        &mailbox.remote_users,
        &messages,
        filter.remote_user_id.as_deref(),
    );
    Ok(AgentExchangeBundle {
        schema_version: AGENT_EXCHANGE_SCHEMA_VERSION,
        exported_at: now,
        messages,
        remote_users,
    })
}

pub fn agent_exchange_import_bundle_for_db(
    db: &Database,
    request: AgentExchangeImportBundleRequest,
) -> AppResult<AgentExchangeImportBundleResponse> {
    let AgentExchangeImportBundleRequest {
        bundle,
        local_agent_id,
        as_inbound,
    } = request;

    if bundle.schema_version != AGENT_EXCHANGE_SCHEMA_VERSION {
        return Err(AppError::validation(format!(
            "unsupported agent exchange bundle schema_version {}",
            bundle.schema_version
        )));
    }

    let import_as_inbound = as_inbound.unwrap_or(true);
    let local_agent_id_override = normalize_optional(local_agent_id);
    let now = now_rfc3339();
    let mut mailbox = load_mailbox(db)?;
    let mut imported_count = 0usize;
    let mut skipped_count = 0usize;
    mailbox.remote_users = merge_remote_users(&mailbox.remote_users, &bundle.remote_users)?;

    for imported in bundle.messages {
        let candidate = if import_as_inbound {
            message_as_inbound(imported, local_agent_id_override.as_deref(), &now)?
        } else {
            imported
        };
        if is_duplicate_import(&mailbox.messages, &candidate) {
            skipped_count += 1;
            continue;
        }
        mailbox.messages.insert(0, candidate);
        imported_count += 1;
    }

    trim_messages(&mut mailbox.messages);
    trim_remote_users(&mut mailbox.remote_users);
    mailbox.last_imported_at = Some(now.clone());
    save_mailbox(db, &mailbox, &now)?;

    Ok(AgentExchangeImportBundleResponse {
        state: mailbox.to_state(),
        imported_count,
        skipped_count,
    })
}

pub fn agent_exchange_update_message_status_for_db(
    db: &Database,
    request: AgentExchangeUpdateMessageStatusRequest,
) -> AppResult<AgentExchangeMessage> {
    let message_id = normalize_required("message_id", &request.message_id)?;
    let status = parse_message_status(request.status)?;
    let mut mailbox = load_mailbox(db)?;
    let message = mailbox
        .messages
        .iter_mut()
        .find(|message| message.id == message_id)
        .ok_or_else(|| {
            AppError::validation(format!(
                "agent exchange message `{}` was not found",
                message_id
            ))
        })?;

    ensure_status_allowed_for_direction(&message.direction, &status)?;
    if message.status != status {
        let now = now_rfc3339();
        message.status = status;
        message.updated_at = now.clone();
        let updated_message = message.clone();
        trim_messages(&mut mailbox.messages);
        save_mailbox(db, &mailbox, &now)?;
        return Ok(updated_message);
    }

    Ok(message.clone())
}

pub fn agent_exchange_delete_message_for_db(
    db: &Database,
    message_id: String,
) -> AppResult<AgentExchangeState> {
    let message_id = normalize_required("message_id", &message_id)?;
    let mut mailbox = load_mailbox(db)?;
    let original_len = mailbox.messages.len();
    mailbox.messages.retain(|message| message.id != message_id);
    if mailbox.messages.len() == original_len {
        return Err(AppError::validation(format!(
            "agent exchange message `{}` was not found",
            message_id
        )));
    }

    let now = now_rfc3339();
    save_mailbox(db, &mailbox, &now)?;
    Ok(mailbox.to_state())
}

pub fn agent_exchange_run_folder_sync_for_db(
    db: &Database,
    request: AgentExchangeFolderSyncRequest,
) -> AppResult<AgentExchangeFolderSyncResponse> {
    let path = normalize_required("path", &request.path)?;
    let path_buf = PathBuf::from(&path);

    let (imported_count, skipped_count) = if path_buf.exists() {
        let bundle_json = fs::read_to_string(&path_buf).map_err(AppError::from_io_error)?;
        let bundle = serde_json::from_str::<AgentExchangeBundle>(&bundle_json)
            .map_err(AppError::from_json_error)?;
        let import_response = agent_exchange_import_bundle_for_db(
            db,
            AgentExchangeImportBundleRequest {
                bundle,
                local_agent_id: request.local_agent_id,
                as_inbound: request.as_inbound,
            },
        )?;
        (
            import_response.imported_count,
            import_response.skipped_count,
        )
    } else {
        (0usize, 0usize)
    };

    let export_bundle = agent_exchange_export_bundle_for_db(
        db,
        AgentExchangeExportBundleRequest {
            direction: None,
            status: None,
            thread_id: None,
            remote_agent_id: None,
            remote_user_id: None,
            limit: Some(AGENT_EXCHANGE_MESSAGE_LIMIT),
        },
    )?;

    if let Some(parent) = path_buf.parent() {
        fs::create_dir_all(parent).map_err(AppError::from_io_error)?;
    }

    let bundle_json =
        serde_json::to_string_pretty(&export_bundle).map_err(AppError::from_json_error)?;
    fs::write(&path_buf, bundle_json).map_err(AppError::from_io_error)?;

    Ok(AgentExchangeFolderSyncResponse {
        state: agent_exchange_get_state_for_db(db)?,
        imported_count,
        skipped_count,
        exported_count: export_bundle.messages.len(),
        path,
        synced_at: export_bundle.exported_at,
    })
}

impl PersistedAgentExchangeMailbox {
    fn to_state(&self) -> AgentExchangeState {
        AgentExchangeState {
            schema_version: self.schema_version,
            messages: self.messages.clone(),
            remote_users: self.remote_users.clone(),
            last_imported_at: self.last_imported_at.clone(),
            last_exported_at: self.last_exported_at.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct MessageFilter {
    direction: Option<AgentExchangeDirection>,
    status: Option<AgentExchangeMessageStatus>,
    thread_id: Option<String>,
    remote_agent_id: Option<String>,
    remote_user_id: Option<String>,
    limit: usize,
}

impl MessageFilter {
    fn from_list_request(request: AgentExchangeListRequest) -> AppResult<Self> {
        Ok(Self {
            direction: parse_direction_filter(request.direction)?,
            status: parse_status_filter(request.status)?,
            thread_id: normalize_optional(request.thread_id),
            remote_agent_id: normalize_optional(request.remote_agent_id),
            remote_user_id: normalize_optional(request.remote_user_id),
            limit: normalize_limit(request.limit),
        })
    }

    fn from_export_request(request: AgentExchangeExportBundleRequest) -> AppResult<Self> {
        Ok(Self {
            direction: parse_direction_filter(request.direction)?,
            status: parse_status_filter(request.status)?,
            thread_id: normalize_optional(request.thread_id),
            remote_agent_id: normalize_optional(request.remote_agent_id),
            remote_user_id: normalize_optional(request.remote_user_id),
            limit: normalize_limit(request.limit),
        })
    }
}

#[derive(Debug, Clone)]
struct RemoteUserFilter {
    query: Option<String>,
    status: Option<AgentExchangeRemoteUserStatus>,
    limit: usize,
}

impl RemoteUserFilter {
    fn from_list_request(request: AgentExchangeListRemoteUsersRequest) -> AppResult<Self> {
        Ok(Self {
            query: normalize_optional(request.query).map(|value| value.to_ascii_lowercase()),
            status: parse_remote_user_status(request.status)?,
            limit: normalize_limit(request.limit),
        })
    }
}

fn message_as_inbound(
    imported: AgentExchangeMessage,
    local_agent_id_override: Option<&str>,
    now: &str,
) -> AppResult<AgentExchangeMessage> {
    let local_agent_id = local_agent_id_override
        .map(str::to_string)
        .unwrap_or_else(|| imported.remote_agent_id.clone());
    if local_agent_id.trim().is_empty() {
        return Err(AppError::validation(
            "local_agent_id is required when importing as inbound",
        ));
    }

    Ok(AgentExchangeMessage {
        id: Uuid::new_v4().to_string(),
        thread_id: imported.thread_id,
        direction: AgentExchangeDirection::Inbound,
        local_agent_id,
        remote_agent_id: imported.local_agent_id,
        remote_user_id: imported.remote_user_id,
        subject: imported.subject,
        body: imported.body,
        payload_json: imported.payload_json,
        status: AgentExchangeMessageStatus::Received,
        source_message_id: Some(imported.id),
        created_at: now.to_string(),
        updated_at: now.to_string(),
    })
}

fn filter_messages(
    messages: &[AgentExchangeMessage],
    filter: &MessageFilter,
) -> Vec<AgentExchangeMessage> {
    messages
        .iter()
        .filter(|message| {
            filter
                .direction
                .as_ref()
                .is_none_or(|direction| &message.direction == direction)
                && filter
                    .status
                    .as_ref()
                    .is_none_or(|status| &message.status == status)
                && filter
                    .thread_id
                    .as_deref()
                    .is_none_or(|thread_id| message.thread_id == thread_id)
                && filter
                    .remote_agent_id
                    .as_deref()
                    .is_none_or(|remote_agent_id| message.remote_agent_id == remote_agent_id)
                && filter
                    .remote_user_id
                    .as_deref()
                    .is_none_or(|remote_user_id| {
                        message.remote_user_id.as_deref() == Some(remote_user_id)
                    })
        })
        .take(filter.limit)
        .cloned()
        .collect()
}

fn filter_remote_users(
    remote_users: &[AgentExchangeRemoteUser],
    filter: &RemoteUserFilter,
) -> Vec<AgentExchangeRemoteUser> {
    remote_users
        .iter()
        .filter(|remote_user| {
            filter
                .status
                .as_ref()
                .is_none_or(|status| &remote_user.status == status)
                && filter
                    .query
                    .as_deref()
                    .is_none_or(|query| remote_user_matches_query(remote_user, query))
        })
        .take(filter.limit)
        .cloned()
        .collect()
}

fn remote_user_matches_query(remote_user: &AgentExchangeRemoteUser, query: &str) -> bool {
    let mut haystacks = vec![
        remote_user.user_id.to_ascii_lowercase(),
        remote_user.display_name.to_ascii_lowercase(),
        remote_user.default_agent_id.to_ascii_lowercase(),
    ];
    if let Some(transport_label) = remote_user.transport_label.as_ref() {
        haystacks.push(transport_label.to_ascii_lowercase());
    }
    if let Some(route_hint) = remote_user.route_hint.as_ref() {
        haystacks.push(route_hint.to_ascii_lowercase());
    }

    haystacks.iter().any(|haystack| haystack.contains(query))
}

fn collect_export_remote_users(
    remote_users: &[AgentExchangeRemoteUser],
    messages: &[AgentExchangeMessage],
    explicit_remote_user_id: Option<&str>,
) -> Vec<AgentExchangeRemoteUser> {
    let mut selected_ids: Vec<String> = messages
        .iter()
        .filter_map(|message| message.remote_user_id.clone())
        .collect();
    if let Some(remote_user_id) = explicit_remote_user_id
        && !selected_ids
            .iter()
            .any(|candidate| candidate == remote_user_id)
    {
        selected_ids.push(remote_user_id.to_string());
    }

    let mut selected: Vec<AgentExchangeRemoteUser> = remote_users
        .iter()
        .filter(|remote_user| {
            selected_ids
                .iter()
                .any(|user_id| user_id == &remote_user.user_id)
        })
        .cloned()
        .collect();
    sort_remote_users(&mut selected);
    selected
}

fn is_duplicate_import(
    messages: &[AgentExchangeMessage],
    candidate: &AgentExchangeMessage,
) -> bool {
    messages.iter().any(|message| {
        message.id == candidate.id
            || candidate
                .source_message_id
                .as_deref()
                .is_some_and(|source_message_id| {
                    message.source_message_id.as_deref() == Some(source_message_id)
                        || message.id == source_message_id
                })
    })
}

fn parse_direction_filter(value: Option<String>) -> AppResult<Option<AgentExchangeDirection>> {
    let Some(value) = normalize_optional(value) else {
        return Ok(None);
    };
    match value.as_str() {
        "inbound" => Ok(Some(AgentExchangeDirection::Inbound)),
        "outbound" => Ok(Some(AgentExchangeDirection::Outbound)),
        _ => Err(AppError::validation(
            "direction must be inbound or outbound for agent exchange messages",
        )),
    }
}

fn parse_status_filter(value: Option<String>) -> AppResult<Option<AgentExchangeMessageStatus>> {
    let Some(value) = normalize_optional(value) else {
        return Ok(None);
    };
    match value.as_str() {
        "draft" => Ok(Some(AgentExchangeMessageStatus::Draft)),
        "sent" => Ok(Some(AgentExchangeMessageStatus::Sent)),
        "received" => Ok(Some(AgentExchangeMessageStatus::Received)),
        "archived" => Ok(Some(AgentExchangeMessageStatus::Archived)),
        _ => Err(AppError::validation(
            "status must be draft, sent, received, or archived for agent exchange messages",
        )),
    }
}

fn parse_remote_user_status(
    value: Option<String>,
) -> AppResult<Option<AgentExchangeRemoteUserStatus>> {
    let Some(value) = normalize_optional(value) else {
        return Ok(None);
    };
    match value.as_str() {
        "active" => Ok(Some(AgentExchangeRemoteUserStatus::Active)),
        "paused" => Ok(Some(AgentExchangeRemoteUserStatus::Paused)),
        "blocked" => Ok(Some(AgentExchangeRemoteUserStatus::Blocked)),
        _ => Err(AppError::validation(
            "status must be active, paused, or blocked for agent exchange remote users",
        )),
    }
}

fn parse_message_status(value: String) -> AppResult<AgentExchangeMessageStatus> {
    parse_status_filter(Some(value))?.ok_or_else(|| {
        AppError::validation(
            "status must be draft, sent, received, or archived for agent exchange messages",
        )
    })
}

fn ensure_status_allowed_for_direction(
    direction: &AgentExchangeDirection,
    status: &AgentExchangeMessageStatus,
) -> AppResult<()> {
    let allowed = match direction {
        AgentExchangeDirection::Outbound => matches!(
            status,
            AgentExchangeMessageStatus::Draft
                | AgentExchangeMessageStatus::Sent
                | AgentExchangeMessageStatus::Archived
        ),
        AgentExchangeDirection::Inbound => {
            matches!(
                status,
                AgentExchangeMessageStatus::Received | AgentExchangeMessageStatus::Archived
            )
        }
    };

    if allowed {
        return Ok(());
    }

    Err(AppError::validation(format!(
        "{} messages cannot be marked {}",
        match direction {
            AgentExchangeDirection::Inbound => "inbound",
            AgentExchangeDirection::Outbound => "outbound",
        },
        match status {
            AgentExchangeMessageStatus::Draft => "draft",
            AgentExchangeMessageStatus::Sent => "sent",
            AgentExchangeMessageStatus::Received => "received",
            AgentExchangeMessageStatus::Archived => "archived",
        }
    )))
}

fn normalize_required(field: &str, value: &str) -> AppResult<String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(AppError::validation(format!("{field} cannot be empty")));
    }
    Ok(normalized.to_string())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|candidate| candidate.trim().to_string())
        .filter(|candidate| !candidate.is_empty())
}

fn normalize_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(AGENT_EXCHANGE_DEFAULT_LIST_LIMIT)
        .clamp(1, AGENT_EXCHANGE_MAX_LIST_LIMIT)
}

fn trim_messages(messages: &mut Vec<AgentExchangeMessage>) {
    messages.sort_by(|left, right| {
        right
            .updated_at
            .cmp(&left.updated_at)
            .then_with(|| right.created_at.cmp(&left.created_at))
            .then_with(|| left.id.cmp(&right.id))
    });
    messages.truncate(AGENT_EXCHANGE_MESSAGE_LIMIT);
}

fn sort_remote_users(remote_users: &mut [AgentExchangeRemoteUser]) {
    remote_users.sort_by(|left, right| {
        compare_timestamp_desc(&left.updated_at, &right.updated_at)
            .then_with(|| compare_timestamp_desc(&left.created_at, &right.created_at))
            .then_with(|| left.user_id.cmp(&right.user_id))
    });
}

fn trim_remote_users(remote_users: &mut Vec<AgentExchangeRemoteUser>) {
    sort_remote_users(remote_users);
    remote_users.truncate(AGENT_EXCHANGE_REMOTE_USER_LIMIT);
}

fn merge_remote_users(
    existing: &[AgentExchangeRemoteUser],
    incoming: &[AgentExchangeRemoteUser],
) -> AppResult<Vec<AgentExchangeRemoteUser>> {
    let mut merged = existing.to_vec();
    for incoming_remote_user in incoming.iter().cloned() {
        let incoming_remote_user = normalize_imported_remote_user(incoming_remote_user)?;
        if let Some(existing_remote_user) = merged
            .iter_mut()
            .find(|remote_user| remote_user.user_id == incoming_remote_user.user_id)
        {
            if incoming_is_same_or_newer(existing_remote_user, &incoming_remote_user) {
                *existing_remote_user = incoming_remote_user;
            }
        } else {
            merged.push(incoming_remote_user);
        }
    }

    trim_remote_users(&mut merged);
    Ok(merged)
}

fn normalize_imported_remote_user(
    remote_user: AgentExchangeRemoteUser,
) -> AppResult<AgentExchangeRemoteUser> {
    Ok(AgentExchangeRemoteUser {
        user_id: normalize_required("remote_users.user_id", &remote_user.user_id)?,
        display_name: normalize_required("remote_users.display_name", &remote_user.display_name)?,
        default_agent_id: normalize_required(
            "remote_users.default_agent_id",
            &remote_user.default_agent_id,
        )?,
        transport_label: normalize_optional(remote_user.transport_label),
        route_hint: normalize_optional(remote_user.route_hint),
        status: remote_user.status,
        created_at: remote_user.created_at,
        updated_at: remote_user.updated_at,
    })
}

fn incoming_is_same_or_newer(
    existing: &AgentExchangeRemoteUser,
    incoming: &AgentExchangeRemoteUser,
) -> bool {
    compare_rfc3339(&incoming.updated_at, &existing.updated_at).is_ge()
}

fn compare_timestamp_desc(left: &str, right: &str) -> std::cmp::Ordering {
    compare_rfc3339(right, left)
}

fn compare_rfc3339(left: &str, right: &str) -> std::cmp::Ordering {
    match (
        DateTime::parse_from_rfc3339(left),
        DateTime::parse_from_rfc3339(right),
    ) {
        (Ok(left), Ok(right)) => left.cmp(&right),
        _ => left.cmp(right),
    }
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

fn load_mailbox(db: &Database) -> AppResult<PersistedAgentExchangeMailbox> {
    let value_json = db.with_connection(|conn| {
        conn.query_row(
            "SELECT value_json FROM app_settings WHERE key = ?1",
            [AGENT_EXCHANGE_SETTINGS_KEY],
            |row| row.get::<_, String>(0),
        )
        .optional()
    })?;

    match value_json {
        Some(value) => serde_json::from_str::<PersistedAgentExchangeMailbox>(&value)
            .map_err(AppError::from_json_error),
        None => Ok(PersistedAgentExchangeMailbox::default()),
    }
}

fn save_mailbox(
    db: &Database,
    mailbox: &PersistedAgentExchangeMailbox,
    updated_at: &str,
) -> AppResult<()> {
    let value_json = serde_json::to_string(mailbox).map_err(AppError::from_json_error)?;
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
        &[&AGENT_EXCHANGE_SETTINGS_KEY, &value_json, &updated_at],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        AGENT_EXCHANGE_REMOTE_USER_LIMIT, AGENT_EXCHANGE_SETTINGS_KEY, AgentExchangeBundle,
        AgentExchangeDraftOutboundRequest, AgentExchangeExportBundleRequest,
        AgentExchangeFolderSyncRequest, AgentExchangeImportBundleRequest,
        AgentExchangeIngestInboundRequest, AgentExchangeListRemoteUsersRequest,
        AgentExchangeListRequest, AgentExchangeMessage, AgentExchangeMessageStatus,
        AgentExchangeRemoteUser, AgentExchangeRemoteUserStatus,
        AgentExchangeUpdateMessageStatusRequest, AgentExchangeUpsertRemoteUserRequest,
        agent_exchange_delete_message_for_db, agent_exchange_delete_remote_user_for_db,
        agent_exchange_draft_outbound_for_db, agent_exchange_export_bundle_for_db,
        agent_exchange_get_state_for_db, agent_exchange_import_bundle_for_db,
        agent_exchange_ingest_inbound_for_db, agent_exchange_list_messages_for_db,
        agent_exchange_list_remote_users_for_db, agent_exchange_run_folder_sync_for_db,
        agent_exchange_update_message_status_for_db, agent_exchange_upsert_remote_user_for_db,
    };
    use crate::backend::Database;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::thread::sleep;
    use std::time::Duration;
    use uuid::Uuid;

    struct TempFileWorkspace {
        root: PathBuf,
    }

    impl TempFileWorkspace {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("hermes-agent-exchange-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).expect("workspace should create");
            Self { root }
        }

        fn path(&self, name: &str) -> PathBuf {
            self.root.join(name)
        }
    }

    impl Drop for TempFileWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn path_string(path: &Path) -> String {
        path.to_string_lossy().to_string()
    }

    #[test]
    fn persisted_mailbox_without_remote_users_defaults_to_empty_directory() {
        let db = Database::in_memory().expect("db should initialize");
        let legacy_mailbox_json = serde_json::json!({
            "schema_version": 1,
            "messages": [],
            "last_imported_at": null,
            "last_exported_at": null
        })
        .to_string();
        db.execute(
            "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
            &[
                &AGENT_EXCHANGE_SETTINGS_KEY,
                &legacy_mailbox_json,
                &"2026-01-01T00:00:00+00:00",
            ],
        )
        .expect("legacy mailbox should seed");

        let state = agent_exchange_get_state_for_db(&db).expect("legacy state should load");
        assert!(state.remote_users.is_empty());

        let remote_user = agent_exchange_upsert_remote_user_for_db(
            &db,
            AgentExchangeUpsertRemoteUserRequest {
                user_id: "future-remote-user".to_string(),
                display_name: "Future Remote User".to_string(),
                default_agent_id: "future-agent".to_string(),
                transport_label: None,
                route_hint: None,
                status: None,
            },
        )
        .expect("remote user should write into upgraded mailbox");
        assert_eq!(remote_user.status, AgentExchangeRemoteUserStatus::Active);
    }

    #[test]
    fn legacy_bundle_without_remote_users_imports_with_empty_directory() {
        let db = Database::in_memory().expect("db should initialize");
        let legacy_bundle_json = serde_json::json!({
            "schema_version": 1,
            "exported_at": "2026-01-01T00:00:00+00:00",
            "messages": []
        })
        .to_string();
        let bundle: AgentExchangeBundle =
            serde_json::from_str(&legacy_bundle_json).expect("legacy bundle should parse");

        assert!(bundle.remote_users.is_empty());

        let imported = agent_exchange_import_bundle_for_db(
            &db,
            AgentExchangeImportBundleRequest {
                bundle,
                local_agent_id: None,
                as_inbound: None,
            },
        )
        .expect("legacy bundle should import");
        assert_eq!(imported.imported_count, 0);
        assert_eq!(imported.skipped_count, 0);
        assert!(imported.state.remote_users.is_empty());
    }

    #[test]
    fn draft_outbound_message_persists_and_exports_as_bundle() {
        let db = Database::in_memory().expect("db should initialize");

        let message = agent_exchange_draft_outbound_for_db(
            &db,
            AgentExchangeDraftOutboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "remote-agent".to_string(),
                remote_user_id: Some("remote-user".to_string()),
                thread_id: Some("thread-alpha".to_string()),
                subject: Some("Plan handoff".to_string()),
                body: "Please review this local plan.".to_string(),
                payload_json: Some(serde_json::json!({ "kind": "handoff" })),
            },
        )
        .expect("outbound draft should persist");

        assert_eq!(message.thread_id, "thread-alpha");
        assert_eq!(message.status, AgentExchangeMessageStatus::Draft);

        let bundle = agent_exchange_export_bundle_for_db(
            &db,
            AgentExchangeExportBundleRequest {
                direction: Some("outbound".to_string()),
                status: Some("draft".to_string()),
                thread_id: None,
                remote_agent_id: None,
                remote_user_id: None,
                limit: None,
            },
        )
        .expect("bundle should export");

        assert_eq!(bundle.messages.len(), 1);
        assert_eq!(bundle.messages[0].id, message.id);
        assert!(
            agent_exchange_get_state_for_db(&db)
                .expect("state should load")
                .last_exported_at
                .is_some()
        );
    }

    #[test]
    fn importing_remote_bundle_as_inbound_deduplicates_source_message() {
        let sender_db = Database::in_memory().expect("sender db should initialize");
        let receiver_db = Database::in_memory().expect("receiver db should initialize");
        let outbound = agent_exchange_draft_outbound_for_db(
            &sender_db,
            AgentExchangeDraftOutboundRequest {
                local_agent_id: "sender-agent".to_string(),
                remote_agent_id: "receiver-agent".to_string(),
                remote_user_id: Some("receiver-user".to_string()),
                thread_id: Some("thread-shared".to_string()),
                subject: Some("Shared context".to_string()),
                body: "Here is the context bundle.".to_string(),
                payload_json: None,
            },
        )
        .expect("sender draft should persist");
        let bundle = AgentExchangeBundle {
            schema_version: 1,
            exported_at: outbound.created_at.clone(),
            messages: vec![outbound.clone()],
            remote_users: vec![],
        };

        let first_import = agent_exchange_import_bundle_for_db(
            &receiver_db,
            AgentExchangeImportBundleRequest {
                bundle: bundle.clone(),
                local_agent_id: Some("receiver-agent".to_string()),
                as_inbound: Some(true),
            },
        )
        .expect("first import should succeed");
        assert_eq!(first_import.imported_count, 1);
        assert_eq!(first_import.skipped_count, 0);
        assert_eq!(
            first_import.state.messages[0].remote_agent_id,
            "sender-agent"
        );
        assert_eq!(
            first_import.state.messages[0].source_message_id.as_deref(),
            Some(outbound.id.as_str())
        );

        let second_import = agent_exchange_import_bundle_for_db(
            &receiver_db,
            AgentExchangeImportBundleRequest {
                bundle,
                local_agent_id: Some("receiver-agent".to_string()),
                as_inbound: Some(true),
            },
        )
        .expect("duplicate import should skip");
        assert_eq!(second_import.imported_count, 0);
        assert_eq!(second_import.skipped_count, 1);
        assert_eq!(second_import.state.messages.len(), 1);
    }

    #[test]
    fn inbound_ingest_and_list_filters_by_thread_direction_and_remote_agent() {
        let db = Database::in_memory().expect("db should initialize");
        agent_exchange_ingest_inbound_for_db(
            &db,
            AgentExchangeIngestInboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "remote-a".to_string(),
                remote_user_id: None,
                thread_id: Some("thread-a".to_string()),
                subject: None,
                body: "Inbound message".to_string(),
                payload_json: None,
                source_message_id: Some("source-1".to_string()),
            },
        )
        .expect("inbound ingest should persist");
        agent_exchange_draft_outbound_for_db(
            &db,
            AgentExchangeDraftOutboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "remote-b".to_string(),
                remote_user_id: None,
                thread_id: Some("thread-b".to_string()),
                subject: None,
                body: "Outbound message".to_string(),
                payload_json: None,
            },
        )
        .expect("outbound draft should persist");

        let filtered = agent_exchange_list_messages_for_db(
            &db,
            AgentExchangeListRequest {
                direction: Some("inbound".to_string()),
                status: Some("received".to_string()),
                thread_id: Some("thread-a".to_string()),
                remote_agent_id: Some("remote-a".to_string()),
                remote_user_id: None,
                limit: Some(10),
            },
        )
        .expect("messages should filter");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].body, "Inbound message");
    }

    #[test]
    fn upsert_remote_user_creates_and_update_preserves_created_at() {
        let db = Database::in_memory().expect("db should initialize");

        let created = agent_exchange_upsert_remote_user_for_db(
            &db,
            AgentExchangeUpsertRemoteUserRequest {
                user_id: " remote-user-1 ".to_string(),
                display_name: " Remote User ".to_string(),
                default_agent_id: " remote-agent ".to_string(),
                transport_label: Some(" Matrix ".to_string()),
                route_hint: Some(" inbox/a ".to_string()),
                status: Some("active".to_string()),
            },
        )
        .expect("remote user should create");

        sleep(Duration::from_millis(2));

        let updated = agent_exchange_upsert_remote_user_for_db(
            &db,
            AgentExchangeUpsertRemoteUserRequest {
                user_id: "remote-user-1".to_string(),
                display_name: "Remote User".to_string(),
                default_agent_id: "remote-agent".to_string(),
                transport_label: Some("Matrix".to_string()),
                route_hint: Some(" inbox/b ".to_string()),
                status: Some("paused".to_string()),
            },
        )
        .expect("remote user should update");

        assert_eq!(created.user_id, "remote-user-1");
        assert_eq!(created.display_name, "Remote User");
        assert_eq!(created.default_agent_id, "remote-agent");
        assert_eq!(created.transport_label.as_deref(), Some("Matrix"));
        assert_eq!(created.route_hint.as_deref(), Some("inbox/a"));
        assert_eq!(created.status, AgentExchangeRemoteUserStatus::Active);
        assert_eq!(updated.created_at, created.created_at);
        assert_ne!(updated.updated_at, created.updated_at);
        assert_eq!(updated.route_hint.as_deref(), Some("inbox/b"));
        assert_eq!(updated.status, AgentExchangeRemoteUserStatus::Paused);
    }

    #[test]
    fn upserting_remote_users_over_limit_trims_oldest_profiles() {
        let db = Database::in_memory().expect("db should initialize");

        for index in 0..(AGENT_EXCHANGE_REMOTE_USER_LIMIT + 5) {
            agent_exchange_upsert_remote_user_for_db(
                &db,
                AgentExchangeUpsertRemoteUserRequest {
                    user_id: format!("remote-user-{index:03}"),
                    display_name: format!("Remote User {index:03}"),
                    default_agent_id: format!("agent-{index:03}"),
                    transport_label: None,
                    route_hint: None,
                    status: Some("active".to_string()),
                },
            )
            .expect("remote user should upsert");
            sleep(Duration::from_millis(1));
        }

        let state = agent_exchange_get_state_for_db(&db).expect("state should load");
        assert_eq!(state.remote_users.len(), AGENT_EXCHANGE_REMOTE_USER_LIMIT);
        assert_eq!(state.remote_users[0].user_id, "remote-user-204");
        assert_eq!(
            state
                .remote_users
                .last()
                .expect("capped remote users should have a tail")
                .user_id,
            "remote-user-005"
        );
        assert!(
            !state
                .remote_users
                .iter()
                .any(|remote_user| remote_user.user_id == "remote-user-000")
        );
    }

    #[test]
    fn list_remote_users_filters_by_query_status_and_limit() {
        let db = Database::in_memory().expect("db should initialize");

        agent_exchange_upsert_remote_user_for_db(
            &db,
            AgentExchangeUpsertRemoteUserRequest {
                user_id: "alpha".to_string(),
                display_name: "Alpha User".to_string(),
                default_agent_id: "agent-alpha".to_string(),
                transport_label: Some("Matrix".to_string()),
                route_hint: Some("hq/alpha".to_string()),
                status: Some("active".to_string()),
            },
        )
        .expect("alpha should create");
        agent_exchange_upsert_remote_user_for_db(
            &db,
            AgentExchangeUpsertRemoteUserRequest {
                user_id: "bravo".to_string(),
                display_name: "Bravo User".to_string(),
                default_agent_id: "agent-bravo".to_string(),
                transport_label: Some("Telegram".to_string()),
                route_hint: Some("ops/bravo".to_string()),
                status: Some("paused".to_string()),
            },
        )
        .expect("bravo should create");
        agent_exchange_upsert_remote_user_for_db(
            &db,
            AgentExchangeUpsertRemoteUserRequest {
                user_id: "charlie".to_string(),
                display_name: "Charlie User".to_string(),
                default_agent_id: "agent-charlie".to_string(),
                transport_label: Some("telegram".to_string()),
                route_hint: Some("ops/charlie".to_string()),
                status: Some("paused".to_string()),
            },
        )
        .expect("charlie should create");

        let filtered = agent_exchange_list_remote_users_for_db(
            &db,
            AgentExchangeListRemoteUsersRequest {
                query: Some(" TELEGRAM ".to_string()),
                status: Some("paused".to_string()),
                limit: Some(1),
            },
        )
        .expect("remote users should filter");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].user_id, "charlie");
        assert_eq!(filtered[0].status, AgentExchangeRemoteUserStatus::Paused);
    }

    #[test]
    fn list_and_export_filter_messages_by_remote_user_id() {
        let db = Database::in_memory().expect("db should initialize");

        let remote_user_a = agent_exchange_upsert_remote_user_for_db(
            &db,
            AgentExchangeUpsertRemoteUserRequest {
                user_id: "remote-a".to_string(),
                display_name: "Remote A".to_string(),
                default_agent_id: "agent-a".to_string(),
                transport_label: Some("Matrix".to_string()),
                route_hint: Some("route/a".to_string()),
                status: Some("active".to_string()),
            },
        )
        .expect("remote user a should create");
        let remote_user_b = agent_exchange_upsert_remote_user_for_db(
            &db,
            AgentExchangeUpsertRemoteUserRequest {
                user_id: "remote-b".to_string(),
                display_name: "Remote B".to_string(),
                default_agent_id: "agent-b".to_string(),
                transport_label: Some("Matrix".to_string()),
                route_hint: Some("route/b".to_string()),
                status: Some("blocked".to_string()),
            },
        )
        .expect("remote user b should create");

        let kept = agent_exchange_draft_outbound_for_db(
            &db,
            AgentExchangeDraftOutboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "agent-a".to_string(),
                remote_user_id: Some("remote-a".to_string()),
                thread_id: Some("thread-remote-a".to_string()),
                subject: Some("For A".to_string()),
                body: "Message for remote A".to_string(),
                payload_json: None,
            },
        )
        .expect("message for remote a should persist");
        agent_exchange_draft_outbound_for_db(
            &db,
            AgentExchangeDraftOutboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "agent-b".to_string(),
                remote_user_id: Some("remote-b".to_string()),
                thread_id: Some("thread-remote-b".to_string()),
                subject: Some("For B".to_string()),
                body: "Message for remote B".to_string(),
                payload_json: None,
            },
        )
        .expect("message for remote b should persist");

        let filtered = agent_exchange_list_messages_for_db(
            &db,
            AgentExchangeListRequest {
                direction: None,
                status: None,
                thread_id: None,
                remote_agent_id: None,
                remote_user_id: Some("remote-a".to_string()),
                limit: Some(10),
            },
        )
        .expect("messages should filter by remote user");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].id, kept.id);

        let exported = agent_exchange_export_bundle_for_db(
            &db,
            AgentExchangeExportBundleRequest {
                direction: None,
                status: None,
                thread_id: None,
                remote_agent_id: None,
                remote_user_id: Some("remote-a".to_string()),
                limit: Some(10),
            },
        )
        .expect("bundle should filter by remote user");

        assert_eq!(exported.messages.len(), 1);
        assert_eq!(exported.messages[0].id, kept.id);
        assert_eq!(exported.remote_users, vec![remote_user_a]);

        let exported_without_messages = agent_exchange_export_bundle_for_db(
            &db,
            AgentExchangeExportBundleRequest {
                direction: Some("inbound".to_string()),
                status: None,
                thread_id: None,
                remote_agent_id: None,
                remote_user_id: Some("remote-b".to_string()),
                limit: Some(10),
            },
        )
        .expect("bundle should still include explicitly requested remote user");

        assert!(exported_without_messages.messages.is_empty());
        assert_eq!(exported_without_messages.remote_users, vec![remote_user_b]);
    }

    #[test]
    fn importing_bundle_merges_remote_user_profiles_without_duplicating_messages() {
        let receiver_db = Database::in_memory().expect("receiver db should initialize");
        let existing = agent_exchange_upsert_remote_user_for_db(
            &receiver_db,
            AgentExchangeUpsertRemoteUserRequest {
                user_id: "shared-user".to_string(),
                display_name: "Shared User".to_string(),
                default_agent_id: "agent-old".to_string(),
                transport_label: Some("Matrix".to_string()),
                route_hint: Some("old-route".to_string()),
                status: Some("active".to_string()),
            },
        )
        .expect("existing profile should create");

        let imported_message = AgentExchangeMessage {
            id: "bundle-message-1".to_string(),
            thread_id: "thread-shared-user".to_string(),
            direction: super::AgentExchangeDirection::Outbound,
            local_agent_id: "sender-agent".to_string(),
            remote_agent_id: "receiver-agent".to_string(),
            remote_user_id: Some("shared-user".to_string()),
            subject: Some("Shared".to_string()),
            body: "Imported body".to_string(),
            payload_json: None,
            status: AgentExchangeMessageStatus::Sent,
            source_message_id: None,
            created_at: "2025-01-01T00:00:00+00:00".to_string(),
            updated_at: "2025-01-01T00:00:00+00:00".to_string(),
        };
        let incoming_remote_user = AgentExchangeRemoteUser {
            user_id: "shared-user".to_string(),
            display_name: "Shared User Updated".to_string(),
            default_agent_id: "agent-new".to_string(),
            transport_label: Some("Relay".to_string()),
            route_hint: Some("new-route".to_string()),
            status: AgentExchangeRemoteUserStatus::Blocked,
            created_at: existing.created_at.clone(),
            updated_at: "2030-01-01T00:00:00+00:00".to_string(),
        };
        let bundle = AgentExchangeBundle {
            schema_version: 1,
            exported_at: "2030-01-02T00:00:00+00:00".to_string(),
            messages: vec![imported_message.clone()],
            remote_users: vec![incoming_remote_user.clone()],
        };

        let first_import = agent_exchange_import_bundle_for_db(
            &receiver_db,
            AgentExchangeImportBundleRequest {
                bundle: bundle.clone(),
                local_agent_id: Some("receiver-agent".to_string()),
                as_inbound: Some(true),
            },
        )
        .expect("first import should succeed");
        assert_eq!(first_import.imported_count, 1);
        assert_eq!(first_import.skipped_count, 0);
        assert_eq!(first_import.state.messages.len(), 1);
        assert_eq!(first_import.state.remote_users.len(), 1);
        assert_eq!(first_import.state.remote_users[0], incoming_remote_user);

        let second_import = agent_exchange_import_bundle_for_db(
            &receiver_db,
            AgentExchangeImportBundleRequest {
                bundle,
                local_agent_id: Some("receiver-agent".to_string()),
                as_inbound: Some(true),
            },
        )
        .expect("second import should deduplicate message");
        assert_eq!(second_import.imported_count, 0);
        assert_eq!(second_import.skipped_count, 1);
        assert_eq!(second_import.state.messages.len(), 1);
        assert_eq!(second_import.state.remote_users.len(), 1);
        assert_eq!(second_import.state.remote_users[0], incoming_remote_user);
    }

    #[test]
    fn importing_remote_users_over_limit_trims_oldest_profiles_and_keeps_messages() {
        let db = Database::in_memory().expect("db should initialize");
        let kept_message = agent_exchange_draft_outbound_for_db(
            &db,
            AgentExchangeDraftOutboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "remote-agent-trimmed".to_string(),
                remote_user_id: Some("trimmed-user".to_string()),
                thread_id: Some("thread-trimmed-user".to_string()),
                subject: Some("Keep referenced message".to_string()),
                body: "Messages should outlive trimmed profiles".to_string(),
                payload_json: None,
            },
        )
        .expect("message should persist before import");

        let mut remote_users = Vec::with_capacity(AGENT_EXCHANGE_REMOTE_USER_LIMIT + 1);
        remote_users.push(AgentExchangeRemoteUser {
            user_id: "trimmed-user".to_string(),
            display_name: "Trimmed User".to_string(),
            default_agent_id: "remote-agent-trimmed".to_string(),
            transport_label: Some("Matrix".to_string()),
            route_hint: Some("trimmed/route".to_string()),
            status: AgentExchangeRemoteUserStatus::Paused,
            created_at: "2029-01-01T00:00:00+00:00".to_string(),
            updated_at: "2029-01-01T00:00:00+00:00".to_string(),
        });
        for index in 0..AGENT_EXCHANGE_REMOTE_USER_LIMIT {
            let year = 2030 + index;
            let timestamp = format!("{year:04}-01-01T00:00:00+00:00");
            remote_users.push(AgentExchangeRemoteUser {
                user_id: format!("future-user-{index:03}"),
                display_name: format!("Future User {index:03}"),
                default_agent_id: format!("future-agent-{index:03}"),
                transport_label: Some("Relay".to_string()),
                route_hint: Some(format!("future/{index:03}")),
                status: AgentExchangeRemoteUserStatus::Active,
                created_at: timestamp.clone(),
                updated_at: timestamp,
            });
        }

        let response = agent_exchange_import_bundle_for_db(
            &db,
            AgentExchangeImportBundleRequest {
                bundle: AgentExchangeBundle {
                    schema_version: 1,
                    exported_at: "2250-01-01T00:00:00+00:00".to_string(),
                    messages: vec![],
                    remote_users,
                },
                local_agent_id: None,
                as_inbound: Some(true),
            },
        )
        .expect("bundle should import");

        assert_eq!(
            response.state.remote_users.len(),
            AGENT_EXCHANGE_REMOTE_USER_LIMIT
        );
        assert_eq!(response.state.remote_users[0].user_id, "future-user-199");
        assert!(
            !response
                .state
                .remote_users
                .iter()
                .any(|remote_user| remote_user.user_id == "trimmed-user")
        );
        assert_eq!(response.state.messages.len(), 1);
        assert_eq!(response.state.messages[0].id, kept_message.id);
        assert_eq!(
            response.state.messages[0].remote_user_id.as_deref(),
            Some("trimmed-user")
        );
    }

    #[test]
    fn folder_sync_imports_existing_bundle_and_exports_local_mailbox_to_same_path() {
        let workspace = TempFileWorkspace::new();
        let bundle_path = workspace.path("exchange/shared.json");
        let sender_db = Database::in_memory().expect("sender db should initialize");
        let receiver_db = Database::in_memory().expect("receiver db should initialize");

        let outbound = agent_exchange_draft_outbound_for_db(
            &sender_db,
            AgentExchangeDraftOutboundRequest {
                local_agent_id: "sender-agent".to_string(),
                remote_agent_id: "receiver-agent".to_string(),
                remote_user_id: Some("receiver-user".to_string()),
                thread_id: Some("thread-sync".to_string()),
                subject: Some("Sync payload".to_string()),
                body: "Bundle from sender".to_string(),
                payload_json: Some(serde_json::json!({ "kind": "sync" })),
            },
        )
        .expect("sender draft should persist");

        fs::create_dir_all(
            bundle_path
                .parent()
                .expect("bundle path should have parent"),
        )
        .expect("parent dir should create");
        fs::write(
            &bundle_path,
            serde_json::to_string_pretty(&AgentExchangeBundle {
                schema_version: 1,
                exported_at: outbound.created_at.clone(),
                messages: vec![outbound.clone()],
                remote_users: vec![],
            })
            .expect("bundle json should serialize"),
        )
        .expect("bundle file should write");

        let response = agent_exchange_run_folder_sync_for_db(
            &receiver_db,
            AgentExchangeFolderSyncRequest {
                path: path_string(&bundle_path),
                local_agent_id: Some("receiver-agent".to_string()),
                as_inbound: Some(true),
            },
        )
        .expect("folder sync should succeed");

        assert_eq!(response.imported_count, 1);
        assert_eq!(response.skipped_count, 0);
        assert_eq!(response.exported_count, 1);
        assert_eq!(response.path, path_string(&bundle_path));
        assert_eq!(response.state.messages.len(), 1);
        assert_eq!(
            response.state.messages[0].source_message_id.as_deref(),
            Some(outbound.id.as_str())
        );
        assert!(response.state.last_exported_at.is_some());
        assert_eq!(
            response.synced_at,
            response
                .state
                .last_exported_at
                .clone()
                .expect("sync time should match export")
        );

        let written_json = fs::read_to_string(&bundle_path).expect("synced bundle should exist");
        assert!(written_json.contains("\n  \"schema_version\""));
        let written_bundle: AgentExchangeBundle =
            serde_json::from_str(&written_json).expect("written bundle should parse");
        assert_eq!(written_bundle.messages.len(), 1);
        assert_eq!(
            written_bundle.messages[0].direction,
            super::AgentExchangeDirection::Inbound
        );
        assert_eq!(
            written_bundle.messages[0].source_message_id.as_deref(),
            Some(outbound.id.as_str())
        );
    }

    #[test]
    fn folder_sync_exports_when_bundle_file_is_missing_and_creates_parent_dirs() {
        let workspace = TempFileWorkspace::new();
        let bundle_path = workspace.path("nested/mailbox/exchange.json");
        let db = Database::in_memory().expect("db should initialize");

        let drafted = agent_exchange_draft_outbound_for_db(
            &db,
            AgentExchangeDraftOutboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "remote-agent".to_string(),
                remote_user_id: None,
                thread_id: Some("thread-export".to_string()),
                subject: Some("Export only".to_string()),
                body: "Write mailbox to disk".to_string(),
                payload_json: None,
            },
        )
        .expect("outbound draft should persist");

        let response = agent_exchange_run_folder_sync_for_db(
            &db,
            AgentExchangeFolderSyncRequest {
                path: path_string(&bundle_path),
                local_agent_id: None,
                as_inbound: None,
            },
        )
        .expect("folder sync should export even when file is absent");

        assert_eq!(response.imported_count, 0);
        assert_eq!(response.skipped_count, 0);
        assert_eq!(response.exported_count, 1);
        assert!(bundle_path.exists());
        let written_bundle: AgentExchangeBundle = serde_json::from_str(
            &fs::read_to_string(&bundle_path).expect("exported bundle should read"),
        )
        .expect("exported bundle should parse");
        assert_eq!(written_bundle.messages.len(), 1);
        assert_eq!(written_bundle.messages[0].id, drafted.id);
        assert_eq!(
            written_bundle.messages[0].direction,
            super::AgentExchangeDirection::Outbound
        );
    }

    #[test]
    fn outbound_draft_can_be_marked_sent_then_archived() {
        let db = Database::in_memory().expect("db should initialize");
        let drafted = agent_exchange_draft_outbound_for_db(
            &db,
            AgentExchangeDraftOutboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "remote-agent".to_string(),
                remote_user_id: None,
                thread_id: Some("thread-lifecycle".to_string()),
                subject: Some("Lifecycle".to_string()),
                body: "Track outbound status changes".to_string(),
                payload_json: None,
            },
        )
        .expect("outbound draft should persist");

        let sent = agent_exchange_update_message_status_for_db(
            &db,
            AgentExchangeUpdateMessageStatusRequest {
                message_id: drafted.id.clone(),
                status: "sent".to_string(),
            },
        )
        .expect("draft should become sent");
        assert_eq!(sent.status, AgentExchangeMessageStatus::Sent);
        assert_ne!(sent.updated_at, drafted.updated_at);

        let archived = agent_exchange_update_message_status_for_db(
            &db,
            AgentExchangeUpdateMessageStatusRequest {
                message_id: drafted.id.clone(),
                status: "archived".to_string(),
            },
        )
        .expect("sent should become archived");
        assert_eq!(archived.status, AgentExchangeMessageStatus::Archived);
        assert_ne!(archived.updated_at, sent.updated_at);
    }

    #[test]
    fn archived_inbound_can_be_restored_to_received() {
        let db = Database::in_memory().expect("db should initialize");
        let inbound = agent_exchange_ingest_inbound_for_db(
            &db,
            AgentExchangeIngestInboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "remote-agent".to_string(),
                remote_user_id: None,
                thread_id: Some("thread-restore".to_string()),
                subject: Some("Restore".to_string()),
                body: "Archive then restore inbound".to_string(),
                payload_json: None,
                source_message_id: Some("remote-1".to_string()),
            },
        )
        .expect("inbound ingest should persist");

        let archived = agent_exchange_update_message_status_for_db(
            &db,
            AgentExchangeUpdateMessageStatusRequest {
                message_id: inbound.id.clone(),
                status: "archived".to_string(),
            },
        )
        .expect("received should archive");
        assert_eq!(archived.status, AgentExchangeMessageStatus::Archived);

        let restored = agent_exchange_update_message_status_for_db(
            &db,
            AgentExchangeUpdateMessageStatusRequest {
                message_id: inbound.id.clone(),
                status: "received".to_string(),
            },
        )
        .expect("archived inbound should restore to received");
        assert_eq!(restored.status, AgentExchangeMessageStatus::Received);
        assert_ne!(restored.updated_at, archived.updated_at);
    }

    #[test]
    fn inbound_message_cannot_transition_to_sent() {
        let db = Database::in_memory().expect("db should initialize");
        let inbound = agent_exchange_ingest_inbound_for_db(
            &db,
            AgentExchangeIngestInboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "remote-agent".to_string(),
                remote_user_id: None,
                thread_id: Some("thread-invalid".to_string()),
                subject: None,
                body: "Inbound cannot be marked sent".to_string(),
                payload_json: None,
                source_message_id: Some("remote-2".to_string()),
            },
        )
        .expect("inbound ingest should persist");

        let error = agent_exchange_update_message_status_for_db(
            &db,
            AgentExchangeUpdateMessageStatusRequest {
                message_id: inbound.id,
                status: "sent".to_string(),
            },
        )
        .expect_err("inbound sent transition should be rejected");
        assert_eq!(error.code, "validation_error");
        assert!(error.message.contains("inbound"));
        assert!(error.message.contains("sent"));
    }

    #[test]
    fn delete_message_removes_only_target_message() {
        let db = Database::in_memory().expect("db should initialize");
        let first = agent_exchange_draft_outbound_for_db(
            &db,
            AgentExchangeDraftOutboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "remote-a".to_string(),
                remote_user_id: None,
                thread_id: Some("thread-delete".to_string()),
                subject: Some("Delete target".to_string()),
                body: "First message".to_string(),
                payload_json: None,
            },
        )
        .expect("first message should persist");
        let second = agent_exchange_ingest_inbound_for_db(
            &db,
            AgentExchangeIngestInboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "remote-b".to_string(),
                remote_user_id: None,
                thread_id: Some("thread-delete".to_string()),
                subject: Some("Keep".to_string()),
                body: "Second message".to_string(),
                payload_json: None,
                source_message_id: Some("remote-3".to_string()),
            },
        )
        .expect("second message should persist");

        let state = agent_exchange_delete_message_for_db(&db, first.id.clone())
            .expect("delete should remove only target message");

        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].id, second.id);
        assert!(!state.messages.iter().any(|message| message.id == first.id));
    }

    #[test]
    fn deleting_remote_user_leaves_messages_intact() {
        let db = Database::in_memory().expect("db should initialize");
        agent_exchange_upsert_remote_user_for_db(
            &db,
            AgentExchangeUpsertRemoteUserRequest {
                user_id: "remote-user-1".to_string(),
                display_name: "Remote User".to_string(),
                default_agent_id: "remote-agent".to_string(),
                transport_label: Some("Matrix".to_string()),
                route_hint: Some("route/a".to_string()),
                status: Some("active".to_string()),
            },
        )
        .expect("remote user should create");
        let message = agent_exchange_draft_outbound_for_db(
            &db,
            AgentExchangeDraftOutboundRequest {
                local_agent_id: "local-agent".to_string(),
                remote_agent_id: "remote-agent".to_string(),
                remote_user_id: Some("remote-user-1".to_string()),
                thread_id: Some("thread-delete-remote-user".to_string()),
                subject: Some("Keep message".to_string()),
                body: "Message should remain".to_string(),
                payload_json: None,
            },
        )
        .expect("message should persist");

        let state = agent_exchange_delete_remote_user_for_db(&db, "remote-user-1".to_string())
            .expect("remote user should delete");

        assert!(state.remote_users.is_empty());
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.messages[0].id, message.id);
        assert_eq!(
            state.messages[0].remote_user_id.as_deref(),
            Some("remote-user-1")
        );
    }
}
