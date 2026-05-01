//! Terminal backend commands.

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::backend::{
    AppError, Database, TerminalBackendProfile, TerminalBackendProfileInput,
    TerminalBackendRegistry, TerminalBackendStatus, TerminalBackendTestResult,
    default_command_available,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalBackendSaveProfileRequest {
    pub id: Option<String>,
    pub kind: String,
    pub display_name: String,
    pub enabled: bool,
    #[serde(default)]
    pub config: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalBackendCommandRequest {
    pub id: String,
}

fn registry(db: &Database) -> TerminalBackendRegistry {
    TerminalBackendRegistry::new(db.clone())
}

pub fn terminal_backend_list_profiles_for_db(
    db: &Database,
) -> Result<Vec<TerminalBackendProfile>, AppError> {
    registry(db).list_profiles()
}

#[tauri::command]
pub fn terminal_backend_list_profiles(
    db: State<'_, Database>,
) -> Result<Vec<TerminalBackendProfile>, AppError> {
    terminal_backend_list_profiles_for_db(db.inner())
}

pub fn terminal_backend_save_profile_for_db(
    db: &Database,
    request: TerminalBackendSaveProfileRequest,
) -> Result<TerminalBackendProfile, AppError> {
    registry(db).save_profile(TerminalBackendProfileInput {
        id: request.id,
        kind: request.kind,
        display_name: request.display_name,
        enabled: request.enabled,
        config: request.config,
    })
}

#[tauri::command]
pub fn terminal_backend_save_profile(
    db: State<'_, Database>,
    request: TerminalBackendSaveProfileRequest,
) -> Result<TerminalBackendProfile, AppError> {
    terminal_backend_save_profile_for_db(db.inner(), request)
}

pub fn terminal_backend_list_status_for_db(
    db: &Database,
) -> Result<Vec<TerminalBackendStatus>, AppError> {
    terminal_backend_list_status_for_db_with_checker(db, default_command_available)
}

pub fn terminal_backend_list_status_for_db_with_checker<F>(
    db: &Database,
    command_available: F,
) -> Result<Vec<TerminalBackendStatus>, AppError>
where
    F: Fn(&str) -> bool,
{
    registry(db).list_statuses_with_checker(command_available)
}

#[tauri::command]
pub fn terminal_backend_list_status(
    db: State<'_, Database>,
) -> Result<Vec<TerminalBackendStatus>, AppError> {
    terminal_backend_list_status_for_db(db.inner())
}

pub fn terminal_backend_test_profile_for_db(
    db: &Database,
    id: &str,
) -> Result<TerminalBackendTestResult, AppError> {
    terminal_backend_test_profile_for_db_with_checker(db, id, default_command_available)
}

pub fn terminal_backend_test_profile_for_db_with_checker<F>(
    db: &Database,
    id: &str,
    command_available: F,
) -> Result<TerminalBackendTestResult, AppError>
where
    F: Fn(&str) -> bool,
{
    registry(db).test_profile_with_checker(id, command_available)
}

#[tauri::command]
pub fn terminal_backend_test_profile(
    db: State<'_, Database>,
    request: TerminalBackendCommandRequest,
) -> Result<TerminalBackendTestResult, AppError> {
    terminal_backend_test_profile_for_db(db.inner(), &request.id)
}
