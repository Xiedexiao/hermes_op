//! Skills discovery commands
//!
//! Exposes a real Tauri command for listing skills discovered from the
//! same configured/local directories used by the CLI slash surface.

use crate::backend::{
    AppError, CreateSessionMessageInput, Database, SessionMessage, SessionMessageRole,
    SessionService, SessionServiceImpl, config,
};
use crate::commands::runtime_adapters::{
    SkillToolRequest, SkillToolResponse, runtime_adapter_execute_skill_tool_for_db,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::State;

const DEFAULT_SKILL_SEARCH_LIMIT: usize = 20;
const MAX_SKILL_SEARCH_LIMIT: usize = 100;
const DEFAULT_SESSION_SKILL_INVOCATION_LIMIT: usize = 20;
const MAX_SESSION_SKILL_INVOCATION_LIMIT: usize = 100;
const SKILL_INVOCATION_SOURCE: &str = "skill_invocation";
const SKILL_RUNTIME_DEFAULT_TOOL_COMMAND: &str = "printf";
const SKILL_RUNTIME_DEFAULT_TIMEOUT_MS: u64 = 1_000;
const SKILL_RUNTIME_PACKAGE_MAX_PROMPT_CHARS: usize = 8_000;
const SKILL_MARKETPLACE_DEFAULT_LIMIT: usize = 50;
const SKILL_MARKETPLACE_MAX_LIMIT: usize = 200;
const SKILL_MARKETPLACE_MAX_MANIFEST_BYTES: usize = 512 * 1024;
const SKILL_MARKETPLACE_MAX_SKILL_BYTES: usize = 256 * 1024;
const SKILL_MARKETPLACE_HTTP_TIMEOUT_MS: u64 = 5_000;
const SKILL_MARKETPLACE_INSTALL_HISTORY_KEY: &str = "skills_marketplace_install_history";
const SKILL_MARKETPLACE_INSTALL_HISTORY_MAX_ITEMS: usize = 50;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillListItem {
    pub name: String,
    pub display_name: String,
    pub source: String,
    pub path: String,
    pub enabled: bool,
}

#[tauri::command]
pub fn skills_list(db: State<'_, Database>) -> Result<Vec<SkillListItem>, AppError> {
    list_skills_from_db(&db)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillDetailItem {
    pub name: String,
    pub display_name: String,
    pub description: Option<String>,
    pub source: String,
    pub path: String,
    pub enabled: bool,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillSearchRequest {
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillViewRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillInstallRequest {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub content: Option<String>,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMarketplaceListRequest {
    pub manifest_url: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMarketplaceInstallRequest {
    pub manifest_url: String,
    pub name: String,
    pub force: Option<bool>,
    pub target_remote_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMarketplaceInstallHistoryListRequest {
    pub limit: Option<usize>,
    pub marketplace_id: Option<String>,
    pub skill_name: Option<String>,
    pub target_remote_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMarketplaceEntry {
    pub name: String,
    pub title: String,
    pub description: String,
    pub source_url: Option<String>,
    pub content: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMarketplaceCatalog {
    pub schema_version: u32,
    pub marketplace_id: String,
    pub manifest_url: String,
    #[serde(default)]
    pub skills: Vec<SkillMarketplaceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMarketplaceInstallResult {
    pub marketplace_id: String,
    pub manifest_url: String,
    pub entry: SkillMarketplaceEntry,
    pub installed_skill: SkillDetailItem,
    pub target_remote_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillMarketplaceInstallHistoryItem {
    pub id: String,
    pub marketplace_id: String,
    pub skill_name: String,
    pub display_name: String,
    pub manifest_url: String,
    pub source_url: Option<String>,
    pub content_source_summary: String,
    pub installed_skill_name: String,
    #[serde(default)]
    pub target_remote_user_id: Option<String>,
    pub installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawSkillMarketplaceManifest {
    pub schema_version: Option<u32>,
    pub marketplace_id: Option<String>,
    #[serde(default)]
    pub skills: Vec<RawSkillMarketplaceEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawSkillMarketplaceEntry {
    pub name: String,
    pub title: Option<String>,
    pub description: Option<String>,
    pub source_url: Option<String>,
    pub content: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillInvokeRequest {
    pub name: String,
    pub instruction: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillInvocationPayload {
    pub name: String,
    pub display_name: String,
    pub command: String,
    pub source: String,
    pub path: String,
    pub instruction: Option<String>,
    pub rendered_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillInvokeSessionRequest {
    pub name: String,
    pub instruction: Option<String>,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillSessionInvocationListRequest {
    pub session_id: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillSessionInvocationResult {
    pub session_id: String,
    pub invocation: SkillInvocationPayload,
    pub message: SessionMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillRuntimeExecuteRequest {
    pub name: String,
    pub instruction: Option<String>,
    pub session_id: Option<String>,
    pub save_to_session: Option<bool>,
    pub dry_run: Option<bool>,
    pub tool_command: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillRuntimeExecutionPackage {
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub timeout_ms: u64,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SkillRuntimeExecutionResult {
    pub invocation: SkillInvocationPayload,
    pub execution_package: SkillRuntimeExecutionPackage,
    pub executed: bool,
    pub dry_run: bool,
    pub runtime_result: Option<SkillToolResponse>,
    pub session_message: Option<SessionMessage>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SkillPreferences {
    #[serde(default)]
    pub disabled_names: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSetEnabledRequest {
    pub name: String,
    pub enabled: bool,
}

pub fn skills_set_enabled_for_db(
    db: &Database,
    request: SkillSetEnabledRequest,
) -> Result<SkillPreferences, AppError> {
    let mut preferences = load_skill_preferences(db)?;
    let name = request.name.trim().to_string();

    preferences.disabled_names.retain(|item| item != &name);
    if !request.enabled && !name.is_empty() {
        preferences.disabled_names.push(name);
        preferences.disabled_names.sort();
        preferences.disabled_names.dedup();
    }

    save_skill_preferences(db, &preferences)?;
    Ok(preferences)
}

#[tauri::command]
pub fn skills_set_enabled(
    db: State<'_, Database>,
    request: SkillSetEnabledRequest,
) -> Result<SkillPreferences, AppError> {
    skills_set_enabled_for_db(&db, request)
}

#[tauri::command]
pub fn skills_search(
    db: State<'_, Database>,
    request: SkillSearchRequest,
) -> Result<Vec<SkillListItem>, AppError> {
    skills_search_for_db(&db, request.query, request.limit)
}

#[tauri::command]
pub fn skills_view(
    db: State<'_, Database>,
    request: SkillViewRequest,
) -> Result<SkillDetailItem, AppError> {
    skills_view_for_db(&db, request.name)
}

#[tauri::command]
pub fn skills_install(
    db: State<'_, Database>,
    request: SkillInstallRequest,
) -> Result<SkillDetailItem, AppError> {
    skills_install_for_db(&db, request)
}

#[tauri::command]
pub async fn skills_marketplace_list(
    request: SkillMarketplaceListRequest,
) -> Result<SkillMarketplaceCatalog, AppError> {
    skills_marketplace_list_for_request(request).await
}

#[tauri::command]
pub async fn skills_marketplace_install(
    db: State<'_, Database>,
    request: SkillMarketplaceInstallRequest,
) -> Result<SkillMarketplaceInstallResult, AppError> {
    let database = db.inner().clone();
    let install_root = default_install_root();
    skills_marketplace_install_with_root(&database, &install_root, request).await
}

#[tauri::command]
pub fn skills_marketplace_list_install_history(
    db: State<'_, Database>,
    request: SkillMarketplaceInstallHistoryListRequest,
) -> Result<Vec<SkillMarketplaceInstallHistoryItem>, AppError> {
    skills_marketplace_install_history_for_db(&db, request)
}

#[tauri::command]
pub fn skills_invoke(
    db: State<'_, Database>,
    request: SkillInvokeRequest,
) -> Result<SkillInvocationPayload, AppError> {
    invoke_skill_with_roots(&db, &configured_skill_roots(), request)
}

#[tauri::command]
pub fn skills_invoke_into_session(
    db: State<'_, Database>,
    request: SkillInvokeSessionRequest,
) -> Result<SkillSessionInvocationResult, AppError> {
    invoke_skill_into_session_with_roots(&db, &configured_skill_roots(), request)
}

#[tauri::command]
pub fn skills_list_session_invocations(
    db: State<'_, Database>,
    request: SkillSessionInvocationListRequest,
) -> Result<Vec<SessionMessage>, AppError> {
    skills_list_session_invocations_for_db(&db, request)
}

#[tauri::command]
pub fn skills_execute_runtime(
    db: State<'_, Database>,
    request: SkillRuntimeExecuteRequest,
) -> Result<SkillRuntimeExecutionResult, AppError> {
    execute_skill_runtime_with_roots(&db, &configured_skill_roots(), request)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SkillFileMetadata {
    display_name: String,
    description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NormalizedSkillInstallRequest {
    name: String,
    title: String,
    description: String,
    content: String,
    force: bool,
}

pub(crate) fn discover_skills_from_roots(
    roots: &[(String, PathBuf)],
) -> Result<Vec<SkillListItem>, AppError> {
    let mut discovered = BTreeMap::<String, SkillListItem>::new();

    for (source, root) in roots {
        if !root.exists() {
            continue;
        }

        if root.join("SKILL.md").is_file() {
            let name = skill_name_from_path(root);
            let path = root.join("SKILL.md");
            let metadata = parse_skill_metadata(&path, &name);
            discovered.entry(name.clone()).or_insert(SkillListItem {
                display_name: metadata.display_name,
                name,
                source: source.clone(),
                path: path.display().to_string(),
                enabled: true,
            });
        }

        let entries = fs::read_dir(root)
            .map_err(|err| AppError::io(format!("Failed to read skills dir: {}", err)))?;
        for entry in entries {
            let entry = entry
                .map_err(|err| AppError::io(format!("Failed to read skill entry: {}", err)))?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let skill_file = path.join("SKILL.md");
            if !skill_file.is_file() {
                continue;
            }

            let name = skill_name_from_path(&path);
            let metadata = parse_skill_metadata(&skill_file, &name);
            discovered.entry(name.clone()).or_insert(SkillListItem {
                display_name: metadata.display_name,
                name,
                source: source.clone(),
                path: skill_file.display().to_string(),
                enabled: true,
            });
        }
    }

    Ok(discovered.into_values().collect())
}

fn parse_skill_metadata(skill_file: &Path, fallback: &str) -> SkillFileMetadata {
    let Ok(contents) = fs::read_to_string(skill_file) else {
        return SkillFileMetadata {
            display_name: fallback.to_string(),
            description: None,
        };
    };

    let mut in_frontmatter = false;
    let mut display_name = None;
    let mut description = None;

    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if let Some(name) = trimmed.strip_prefix("name:") {
                let value = name.trim();
                if !value.is_empty() {
                    display_name = Some(value.to_string());
                }
                continue;
            }

            if let Some(value) = trimmed.strip_prefix("description:") {
                let value = value.trim();
                if !value.is_empty() {
                    description = Some(value.to_string());
                }
            }
        }
    }

    SkillFileMetadata {
        display_name: display_name.unwrap_or_else(|| fallback.to_string()),
        description,
    }
}

fn skill_name_from_path(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("unknown-skill")
        .to_string()
}

fn configured_skill_roots() -> Vec<(String, PathBuf)> {
    let mut roots = Vec::new();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    roots.push(("hermes".to_string(), home.join(".hermes").join("skills")));
    roots.push(("codex".to_string(), home.join(".codex").join("skills")));
    roots.push(("agents".to_string(), home.join(".agents").join("skills")));

    if let Ok(cfg) = config::load_config()
        && let Some(path) = cfg.skills_dir
    {
        roots.push(("config".to_string(), PathBuf::from(path)));
    }

    roots
}

fn default_install_root() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".hermes").join("skills")
}

fn list_skills_from_db_with_roots(
    db: &Database,
    roots: &[(String, PathBuf)],
) -> Result<Vec<SkillListItem>, AppError> {
    let disabled = load_skill_preferences(db)?.disabled_names;
    let mut skills = discover_skills_from_roots(roots)?;
    for skill in &mut skills {
        skill.enabled = !disabled.contains(&skill.name);
    }
    Ok(skills)
}

pub(crate) fn list_skills_from_db(db: &Database) -> Result<Vec<SkillListItem>, AppError> {
    list_skills_from_db_with_roots(db, &configured_skill_roots())
}

fn resolve_skill_search_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_SKILL_SEARCH_LIMIT)
        .clamp(1, MAX_SKILL_SEARCH_LIMIT)
}

fn resolve_session_skill_invocation_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_SESSION_SKILL_INVOCATION_LIMIT)
        .clamp(1, MAX_SESSION_SKILL_INVOCATION_LIMIT)
}

fn normalize_skill_invocation_session_id(session_id: String) -> Result<String, AppError> {
    let normalized = session_id.trim().to_string();
    if normalized.is_empty() {
        return Err(AppError::validation("session id is required"));
    }

    Ok(normalized)
}

fn skill_invocation_metadata_json() -> Result<String, AppError> {
    serde_json::to_string(&serde_json::json!({
        "source": SKILL_INVOCATION_SOURCE,
    }))
    .map_err(AppError::from_json_error)
}

fn normalize_skill_lookup_name(name: String) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("skill name is required"));
    }

    Ok(trimmed.to_string())
}

fn find_skill_detail_in_roots(
    db: &Database,
    roots: &[(String, PathBuf)],
    name: String,
) -> Result<SkillDetailItem, AppError> {
    let normalized_name = normalize_skill_lookup_name(name)?;
    let skill = list_skills_from_db_with_roots(db, roots)?
        .into_iter()
        .find(|item| item.name == normalized_name)
        .ok_or_else(|| AppError::validation("skill not found"))?;

    read_skill_detail(&skill)
}

fn read_skill_detail(skill: &SkillListItem) -> Result<SkillDetailItem, AppError> {
    let path = PathBuf::from(&skill.path);
    let content = fs::read_to_string(&path)
        .map_err(|err| AppError::io(format!("Failed to read skill file: {}", err)))?;
    let metadata = parse_skill_metadata(&path, &skill.name);

    Ok(SkillDetailItem {
        name: skill.name.clone(),
        display_name: metadata.display_name,
        description: metadata.description,
        source: skill.source.clone(),
        path: skill.path.clone(),
        enabled: skill.enabled,
        content,
    })
}

fn normalize_skill_install_request(
    request: SkillInstallRequest,
) -> Result<NormalizedSkillInstallRequest, AppError> {
    let name = normalize_install_name(request.name)?;
    let title = request
        .title
        .unwrap_or_else(|| name.clone())
        .trim()
        .to_string();
    let title = if title.is_empty() {
        name.clone()
    } else {
        title
    };
    let description = request.description.unwrap_or_default().trim().to_string();
    let content = request.content.unwrap_or_default().trim().to_string();

    for (field, value) in [
        ("skill name", &name),
        ("skill title", &title),
        ("skill description", &description),
        ("skill content", &content),
    ] {
        if !value.is_ascii() {
            return Err(AppError::validation(format!(
                "{} must contain only ASCII characters",
                field
            )));
        }
    }

    Ok(NormalizedSkillInstallRequest {
        name,
        title,
        description,
        content,
        force: request.force,
    })
}

fn normalize_install_name(name: String) -> Result<String, AppError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation("skill name is required"));
    }

    if trimmed == "." || trimmed == ".." {
        return Err(AppError::validation("skill name is invalid"));
    }

    if !trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err(AppError::validation(
            "skill name may only include ASCII letters, numbers, '-' or '_'",
        ));
    }

    Ok(trimmed.to_string())
}

fn render_skill_markdown(request: &NormalizedSkillInstallRequest) -> String {
    let mut markdown = format!(
        "---\nname: {}\ndescription: {}\n---\n\n",
        request.title, request.description
    );

    if request.content.is_empty() {
        markdown.push_str(&format!("# {}\n", request.title));
    } else {
        markdown.push_str(&request.content);
        markdown.push('\n');
    }

    markdown
}

fn normalize_skill_instruction(instruction: Option<String>) -> Option<String> {
    normalize_optional_trimmed_string(instruction)
}

fn normalize_optional_trimmed_string(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn normalize_optional_target_remote_user_id(value: Option<String>) -> Option<String> {
    normalize_optional_trimmed_string(value)
}

fn render_skill_invocation_payload(
    detail: &SkillDetailItem,
    command_key: &str,
    instruction: Option<&str>,
) -> String {
    let mut output = format!(
        "skill\tcommand={}\tname={}\tsource={}\tpath={}\n",
        command_key, detail.name, detail.source, detail.path
    );
    if let Some(value) = instruction {
        output.push_str(&format!("skill\tinstruction\t{}\n", value));
    }
    output.push_str(&format!(
        "[SYSTEM: The user has invoked the \"{}\" skill via {}. Follow its instructions below.]\n\n{}\n",
        detail.display_name,
        command_key,
        detail.content.trim_end(),
    ));
    output
}

fn skill_command_keys(skill: &SkillListItem) -> Vec<String> {
    let mut keys = Vec::new();

    if let Some(key) = build_skill_command_key(&skill.name) {
        keys.push(key);
    }
    if let Some(key) = build_skill_command_key(&skill.display_name) {
        keys.push(key);
    }

    keys.sort();
    keys.dedup();
    keys
}

fn build_skill_command_key(value: &str) -> Option<String> {
    let slug = skill_slug(value);
    if slug.is_empty() {
        None
    } else {
        Some(format!("/{}", slug))
    }
}

fn normalize_skill_command_key(command: &str) -> String {
    let raw = command.trim().trim_start_matches('/');
    let mut normalized = String::new();
    let mut last_was_hyphen = false;

    for character in raw.chars() {
        let lower = character.to_ascii_lowercase();
        if lower.is_ascii_alphanumeric() {
            normalized.push(lower);
            last_was_hyphen = false;
        } else if !last_was_hyphen && !normalized.is_empty() {
            normalized.push('-');
            last_was_hyphen = true;
        }
    }

    while normalized.ends_with('-') {
        normalized.pop();
    }

    format!("/{}", normalized)
}

fn skill_slug(value: &str) -> String {
    normalize_skill_command_key(value)
        .trim_start_matches('/')
        .to_string()
}

fn resolve_skill_invocation_command(skill: &SkillListItem, selector: &str) -> Option<String> {
    let requested = normalize_skill_command_key(selector);
    skill_command_keys(skill)
        .into_iter()
        .find(|key| *key == requested)
}

fn invoke_skill_with_roots(
    db: &Database,
    roots: &[(String, PathBuf)],
    request: SkillInvokeRequest,
) -> Result<SkillInvocationPayload, AppError> {
    let selector = normalize_skill_lookup_name(request.name)?;
    let instruction = normalize_skill_instruction(request.instruction);
    let (skill, command) = list_skills_from_db_with_roots(db, roots)?
        .into_iter()
        .find_map(|skill| {
            let command = resolve_skill_invocation_command(&skill, &selector)?;
            Some((skill, command))
        })
        .ok_or_else(|| AppError::validation("skill not found"))?;

    if !skill.enabled {
        return Err(AppError::validation("skill is disabled"));
    }

    let detail = read_skill_detail(&skill)?;
    let rendered_prompt =
        render_skill_invocation_payload(&detail, &command, instruction.as_deref());

    Ok(SkillInvocationPayload {
        name: detail.name,
        display_name: detail.display_name,
        command,
        source: detail.source,
        path: detail.path,
        instruction,
        rendered_prompt,
    })
}

fn invoke_skill_into_session_with_roots(
    db: &Database,
    roots: &[(String, PathBuf)],
    request: SkillInvokeSessionRequest,
) -> Result<SkillSessionInvocationResult, AppError> {
    let session_id = request.session_id.trim().to_string();
    if session_id.is_empty() {
        return Err(AppError::validation("session id is required"));
    }
    let invocation = invoke_skill_with_roots(
        db,
        roots,
        SkillInvokeRequest {
            name: request.name,
            instruction: request.instruction,
        },
    )?;
    let service = SessionServiceImpl::new(db.clone());
    let message = service.create_message(CreateSessionMessageInput {
        session_id: session_id.clone(),
        role: SessionMessageRole::System,
        content: invocation.rendered_prompt.clone(),
        source: SKILL_INVOCATION_SOURCE.to_string(),
    })?;
    tag_skill_invocation_message(db, &message.id)?;

    Ok(SkillSessionInvocationResult {
        session_id,
        invocation,
        message,
    })
}

pub(crate) fn execute_skill_runtime_with_roots(
    db: &Database,
    roots: &[(String, PathBuf)],
    request: SkillRuntimeExecuteRequest,
) -> Result<SkillRuntimeExecutionResult, AppError> {
    let session_id = normalize_optional_skill_runtime_session_id(request.session_id)?;
    let save_to_session = request.save_to_session.unwrap_or(session_id.is_some());
    if save_to_session && session_id.is_none() {
        return Err(AppError::validation(
            "session id is required when saving skill runtime context",
        ));
    }

    let dry_run = request.dry_run.unwrap_or(true);
    let tool_command = request.tool_command;
    let timeout_ms = request.timeout_ms;

    let invocation = invoke_skill_with_roots(
        db,
        roots,
        SkillInvokeRequest {
            name: request.name,
            instruction: request.instruction,
        },
    )?;
    let execution_package =
        build_skill_runtime_execution_package(&invocation, tool_command, timeout_ms)?;
    let session_message = if save_to_session {
        let session_id = session_id.expect("validated session id should exist");
        Some(save_skill_invocation_payload_to_session(
            db,
            session_id,
            &invocation,
        )?)
    } else {
        None
    };
    if dry_run {
        return Ok(SkillRuntimeExecutionResult {
            invocation,
            execution_package,
            executed: false,
            dry_run: true,
            runtime_result: None,
            session_message,
            summary: "generated safe local skill runtime execution package; dry_run prevented tool execution".to_string(),
        });
    }

    let runtime_result = runtime_adapter_execute_skill_tool_for_db(
        db,
        SkillToolRequest {
            command: execution_package.command.clone(),
            args: execution_package.args.clone(),
            cwd: execution_package.cwd.clone(),
            timeout_ms: Some(execution_package.timeout_ms),
        },
    )
    .map_err(|err| AppError::runtime(err.to_string()))?;

    Ok(SkillRuntimeExecutionResult {
        invocation,
        execution_package,
        executed: true,
        dry_run: false,
        runtime_result: Some(runtime_result),
        session_message,
        summary: "executed safe local skill runtime validation through runtime adapter".to_string(),
    })
}

fn normalize_optional_skill_runtime_session_id(
    session_id: Option<String>,
) -> Result<Option<String>, AppError> {
    match session_id {
        Some(value) => Ok(Some(normalize_skill_invocation_session_id(value)?)),
        None => Ok(None),
    }
}

fn build_skill_runtime_execution_package(
    invocation: &SkillInvocationPayload,
    tool_command: Option<String>,
    timeout_ms: Option<u64>,
) -> Result<SkillRuntimeExecutionPackage, AppError> {
    let command = normalize_skill_runtime_tool_command(tool_command)?;
    let timeout_ms = timeout_ms.unwrap_or(SKILL_RUNTIME_DEFAULT_TIMEOUT_MS);
    let preview = render_skill_runtime_execution_preview(invocation);
    let args = if command == "printf" {
        vec!["%s".to_string(), preview.clone()]
    } else {
        vec![preview.clone()]
    };

    Ok(SkillRuntimeExecutionPackage {
        command,
        args,
        cwd: None,
        timeout_ms,
        preview,
    })
}

fn normalize_skill_runtime_tool_command(command: Option<String>) -> Result<String, AppError> {
    let command = command
        .unwrap_or_else(|| SKILL_RUNTIME_DEFAULT_TOOL_COMMAND.to_string())
        .trim()
        .to_string();
    if command.is_empty() {
        return Err(AppError::validation(
            "skill runtime tool command is required",
        ));
    }
    if command != "printf" && command != "echo" {
        return Err(AppError::validation(
            "skill runtime execution only supports printf or echo validation commands",
        ));
    }

    Ok(command)
}

fn render_skill_runtime_execution_preview(invocation: &SkillInvocationPayload) -> String {
    let rendered_prompt = truncate_chars(
        &invocation.rendered_prompt,
        SKILL_RUNTIME_PACKAGE_MAX_PROMPT_CHARS,
    );
    serde_json::json!({
        "kind": "skill_runtime_execution_package",
        "marker": "skill-runtime",
        "mode": "local_allowlisted_validation",
        "skill": {
            "name": invocation.name,
            "display_name": invocation.display_name,
            "command": invocation.command,
            "source": invocation.source,
            "path": invocation.path,
            "instruction": invocation.instruction,
        },
        "runtime": {
            "adapter": "runtime_adapter_execute_skill_tool",
            "allowed_commands": ["printf", "echo"],
            "model_invocation": false,
            "paid_provider_invocation": false,
        },
        "rendered_prompt": rendered_prompt,
    })
    .to_string()
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }

    let mut output = value.chars().take(max_chars).collect::<String>();
    output.push_str("\n...[truncated]");
    output
}

fn save_skill_invocation_payload_to_session(
    db: &Database,
    session_id: String,
    invocation: &SkillInvocationPayload,
) -> Result<SessionMessage, AppError> {
    let service = SessionServiceImpl::new(db.clone());
    let message = service.create_message(CreateSessionMessageInput {
        session_id,
        role: SessionMessageRole::System,
        content: invocation.rendered_prompt.clone(),
        source: SKILL_INVOCATION_SOURCE.to_string(),
    })?;
    tag_skill_invocation_message(db, &message.id)?;
    Ok(message)
}

fn tag_skill_invocation_message(db: &Database, message_id: &str) -> Result<(), AppError> {
    let metadata_json = skill_invocation_metadata_json()?;
    db.execute(
        "UPDATE session_messages SET metadata_json = ?1 WHERE id = ?2",
        &[&metadata_json as &dyn rusqlite::ToSql, &message_id],
    )
    .map_err(|err| {
        AppError::storage(format!(
            "Failed to tag session message as skill invocation: {}",
            err
        ))
    })?;

    Ok(())
}

fn skills_list_session_invocations_for_db(
    db: &Database,
    request: SkillSessionInvocationListRequest,
) -> Result<Vec<SessionMessage>, AppError> {
    let session_id = normalize_skill_invocation_session_id(request.session_id)?;
    let limit = resolve_session_skill_invocation_limit(request.limit);
    let metadata_json = skill_invocation_metadata_json()?;

    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, created_at
             FROM session_messages
             WHERE session_id = ?1
               AND metadata_json = ?2
             ORDER BY datetime(created_at) DESC, rowid DESC
             LIMIT ?3",
        )?;
        let rows = stmt.query_map(
            rusqlite::params![session_id, metadata_json, limit as i64],
            |row| {
                Ok(SessionMessage {
                    id: row.get(0)?,
                    session_id: row.get(1)?,
                    role: SessionMessageRole::from_key(&row.get::<_, String>(2)?),
                    content: row.get(3)?,
                    source: SKILL_INVOCATION_SOURCE.to_string(),
                    created_at: row.get(4)?,
                })
            },
        )?;
        rows.collect::<Result<Vec<_>, _>>()
    })
    .map_err(|err| {
        AppError::storage(format!(
            "Failed to list session skill invocation messages: {}",
            err
        ))
    })
}

fn install_skill_with_root(
    db: &Database,
    install_root: &Path,
    request: SkillInstallRequest,
) -> Result<SkillDetailItem, AppError> {
    let request = normalize_skill_install_request(request)?;
    let skill_dir = install_root.join(&request.name);
    let skill_file = skill_dir.join("SKILL.md");
    if skill_file.exists() && !request.force {
        return Err(AppError::validation("skill already exists"));
    }

    fs::create_dir_all(&skill_dir)
        .map_err(|err| AppError::io(format!("Failed to create skill directory: {}", err)))?;
    fs::write(&skill_file, render_skill_markdown(&request))
        .map_err(|err| AppError::io(format!("Failed to write skill file: {}", err)))?;

    find_skill_detail_in_roots(
        db,
        &[("hermes".to_string(), install_root.to_path_buf())],
        request.name,
    )
}

pub fn skills_search_for_db(
    db: &Database,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<SkillListItem>, AppError> {
    let normalized_query = query.trim().to_lowercase();
    let resolved_limit = resolve_skill_search_limit(limit);

    Ok(list_skills_from_db(db)?
        .into_iter()
        .filter(|skill| {
            normalized_query.is_empty()
                || skill.name.to_lowercase().contains(&normalized_query)
                || skill
                    .display_name
                    .to_lowercase()
                    .contains(&normalized_query)
                || skill.source.to_lowercase().contains(&normalized_query)
                || skill.path.to_lowercase().contains(&normalized_query)
        })
        .take(resolved_limit)
        .collect())
}

pub fn skills_view_for_db(db: &Database, name: String) -> Result<SkillDetailItem, AppError> {
    find_skill_detail_in_roots(db, &configured_skill_roots(), name)
}

pub fn skills_install_for_db(
    db: &Database,
    request: SkillInstallRequest,
) -> Result<SkillDetailItem, AppError> {
    install_skill_with_root(db, &default_install_root(), request)
}

pub(crate) async fn skills_marketplace_list_for_request(
    request: SkillMarketplaceListRequest,
) -> Result<SkillMarketplaceCatalog, AppError> {
    let manifest_url = normalize_marketplace_url(request.manifest_url, "marketplace manifest")?;
    let manifest_json = read_marketplace_text(
        &manifest_url,
        "marketplace manifest",
        SKILL_MARKETPLACE_MAX_MANIFEST_BYTES,
    )
    .await?;
    let mut catalog = parse_skill_marketplace_manifest(&manifest_url, &manifest_json)?;
    let limit = normalize_marketplace_limit(request.limit);
    catalog.skills.truncate(limit.min(catalog.skills.len()));
    Ok(catalog)
}

pub(crate) async fn skills_marketplace_install_with_root(
    db: &Database,
    install_root: &Path,
    request: SkillMarketplaceInstallRequest,
) -> Result<SkillMarketplaceInstallResult, AppError> {
    let SkillMarketplaceInstallRequest {
        manifest_url,
        name,
        force,
        target_remote_user_id,
    } = request;
    let target_remote_user_id = normalize_optional_target_remote_user_id(target_remote_user_id);
    let catalog = skills_marketplace_list_for_request(SkillMarketplaceListRequest {
        manifest_url,
        limit: Some(SKILL_MARKETPLACE_MAX_LIMIT),
    })
    .await?;
    let requested_name = normalize_install_name(name)?;
    let entry = catalog
        .skills
        .iter()
        .find(|entry| entry.name == requested_name)
        .cloned()
        .ok_or_else(|| AppError::validation("marketplace skill not found"))?;
    let skill_content = marketplace_skill_content(&catalog.manifest_url, &entry).await?;
    let installed_skill = install_skill_with_root(
        db,
        install_root,
        SkillInstallRequest {
            name: entry.name.clone(),
            title: Some(entry.title.clone()),
            description: Some(entry.description.clone()),
            content: Some(skill_content),
            force: force.unwrap_or(false),
        },
    )?;
    let result = SkillMarketplaceInstallResult {
        marketplace_id: catalog.marketplace_id,
        manifest_url: catalog.manifest_url,
        entry,
        installed_skill,
        target_remote_user_id,
    };
    record_skill_marketplace_install_history(db, &result)?;
    Ok(result)
}

fn normalize_marketplace_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(SKILL_MARKETPLACE_DEFAULT_LIMIT)
        .clamp(1, SKILL_MARKETPLACE_MAX_LIMIT)
}

fn normalize_marketplace_install_history_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(SKILL_MARKETPLACE_INSTALL_HISTORY_MAX_ITEMS)
        .clamp(1, SKILL_MARKETPLACE_INSTALL_HISTORY_MAX_ITEMS)
}

fn parse_skill_marketplace_manifest(
    manifest_url: &str,
    manifest_json: &str,
) -> Result<SkillMarketplaceCatalog, AppError> {
    let raw: RawSkillMarketplaceManifest =
        serde_json::from_str(manifest_json).map_err(AppError::from_json_error)?;
    let marketplace_id = raw
        .marketplace_id
        .unwrap_or_else(|| "remote-skill-marketplace".to_string())
        .trim()
        .to_string();
    if marketplace_id.is_empty() {
        return Err(AppError::validation("marketplace id cannot be empty"));
    }

    let mut skills = Vec::new();
    for raw_entry in raw.skills {
        let name = normalize_install_name(raw_entry.name)?;
        let title = raw_entry
            .title
            .unwrap_or_else(|| name.clone())
            .trim()
            .to_string();
        let description = raw_entry.description.unwrap_or_default().trim().to_string();
        let source_url = raw_entry
            .source_url
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(|value| resolve_marketplace_source_url(manifest_url, &value))
            .transpose()?;
        let content = raw_entry
            .content
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if source_url.is_none() && content.is_none() {
            return Err(AppError::validation(format!(
                "marketplace skill `{}` requires source_url or inline content",
                name
            )));
        }
        let tags = raw_entry
            .tags
            .into_iter()
            .map(|tag| tag.trim().to_string())
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>();

        skills.push(SkillMarketplaceEntry {
            name,
            title: if title.is_empty() {
                "Untitled Skill".to_string()
            } else {
                title
            },
            description,
            source_url,
            content,
            tags,
        });
    }
    skills.sort_by(|left, right| left.name.cmp(&right.name));

    Ok(SkillMarketplaceCatalog {
        schema_version: raw.schema_version.unwrap_or(1),
        marketplace_id,
        manifest_url: manifest_url.to_string(),
        skills,
    })
}

async fn marketplace_skill_content(
    manifest_url: &str,
    entry: &SkillMarketplaceEntry,
) -> Result<String, AppError> {
    if let Some(content) = entry.content.as_deref() {
        return Ok(content.to_string());
    }

    let source_url = entry
        .source_url
        .as_deref()
        .ok_or_else(|| AppError::validation("marketplace skill source_url is required"))?;
    let resolved = resolve_marketplace_source_url(manifest_url, source_url)?;
    read_marketplace_text(
        &resolved,
        "marketplace skill content",
        SKILL_MARKETPLACE_MAX_SKILL_BYTES,
    )
    .await
}

fn normalize_marketplace_url(value: String, label: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation(format!("{} URL is required", label)));
    }
    validate_marketplace_url_or_path(trimmed, label)?;
    Ok(trimmed.to_string())
}

fn resolve_marketplace_source_url(
    manifest_url: &str,
    source_url: &str,
) -> Result<String, AppError> {
    validate_marketplace_url_or_path(source_url, "marketplace skill source")?;
    if is_supported_marketplace_url(source_url) || is_local_path_like(source_url) {
        return Ok(source_url.to_string());
    }

    if let Ok(base) = reqwest::Url::parse(manifest_url) {
        let joined = base.join(source_url).map_err(|err| {
            AppError::validation(format!("failed to resolve marketplace source URL: {}", err))
        })?;
        validate_marketplace_url_or_path(joined.as_str(), "marketplace skill source")?;
        return Ok(joined.to_string());
    }

    let manifest_path = PathBuf::from(manifest_url);
    let base_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    Ok(base_dir.join(source_url).display().to_string())
}

fn validate_marketplace_url_or_path(value: &str, label: &str) -> Result<(), AppError> {
    if value.contains('\0') {
        return Err(AppError::validation(format!(
            "{} contains an invalid NUL byte",
            label
        )));
    }
    if let Ok(url) = reqwest::Url::parse(value) {
        match url.scheme() {
            "file" | "http" | "https" => Ok(()),
            scheme => Err(AppError::validation(format!(
                "{} scheme `{}` is not supported; use file, http, or https",
                label, scheme
            ))),
        }
    } else {
        Ok(())
    }
}

fn is_supported_marketplace_url(value: &str) -> bool {
    reqwest::Url::parse(value)
        .ok()
        .is_some_and(|url| matches!(url.scheme(), "file" | "http" | "https"))
}

fn is_local_path_like(value: &str) -> bool {
    let path = Path::new(value);
    path.is_absolute() || value.starts_with("./") || value.starts_with("../") || path.exists()
}

async fn read_marketplace_text(
    location: &str,
    label: &str,
    max_bytes: usize,
) -> Result<String, AppError> {
    if let Ok(url) = reqwest::Url::parse(location) {
        return match url.scheme() {
            "file" => {
                let path = url
                    .to_file_path()
                    .map_err(|_| AppError::validation(format!("{} file URL is invalid", label)))?;
                read_marketplace_file_text(&path, label, max_bytes)
            }
            "http" | "https" => read_marketplace_http_text(location, label, max_bytes).await,
            scheme => Err(AppError::validation(format!(
                "{} scheme `{}` is not supported; use file, http, or https",
                label, scheme
            ))),
        };
    }

    read_marketplace_file_text(&PathBuf::from(location), label, max_bytes)
}

fn read_marketplace_file_text(
    path: &Path,
    label: &str,
    max_bytes: usize,
) -> Result<String, AppError> {
    let metadata = fs::metadata(path)
        .map_err(|err| AppError::io(format!("Failed to read {} metadata: {}", label, err)))?;
    if !metadata.is_file() {
        return Err(AppError::validation(format!(
            "{} must point to a regular file",
            label
        )));
    }
    if metadata.len() > max_bytes as u64 {
        return Err(AppError::validation(format!(
            "{} exceeds the {} byte safety limit",
            label, max_bytes
        )));
    }
    fs::read_to_string(path)
        .map_err(|err| AppError::io(format!("Failed to read {}: {}", label, err)))
}

async fn read_marketplace_http_text(
    url: &str,
    label: &str,
    max_bytes: usize,
) -> Result<String, AppError> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(
            SKILL_MARKETPLACE_HTTP_TIMEOUT_MS,
        ))
        .build()
        .map_err(|err| {
            AppError::runtime(format!("Failed to build marketplace HTTP client: {}", err))
        })?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| AppError::runtime(format!("Failed to fetch {}: {}", label, err)))?;
    let status = response.status();
    if !status.is_success() {
        return Err(AppError::runtime(format!(
            "Failed to fetch {}: HTTP {}",
            label, status
        )));
    }
    let bytes = response
        .bytes()
        .await
        .map_err(|err| AppError::runtime(format!("Failed to read {} response: {}", label, err)))?;
    if bytes.len() > max_bytes {
        return Err(AppError::validation(format!(
            "{} exceeds the {} byte safety limit",
            label, max_bytes
        )));
    }
    String::from_utf8(bytes.to_vec())
        .map_err(|err| AppError::validation(format!("{} is not valid UTF-8: {}", label, err)))
}

fn load_skill_preferences(db: &Database) -> Result<SkillPreferences, AppError> {
    match db.query_row(
        "SELECT value_json FROM app_settings WHERE key = 'skills_preferences'",
        &[],
        |row| row.get::<_, String>(0),
    ) {
        Ok(json) => serde_json::from_str(&json).map_err(AppError::from_json_error),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(SkillPreferences::default()),
        Err(err) => Err(AppError::storage(format!(
            "Failed to load skill preferences: {}",
            err
        ))),
    }
}

fn save_skill_preferences(db: &Database, preferences: &SkillPreferences) -> Result<(), AppError> {
    let json = serde_json::to_string(preferences).map_err(AppError::from_json_error)?;
    let now = chrono::Utc::now().to_rfc3339();
    let params: Vec<&dyn rusqlite::ToSql> = vec![&json, &now];
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES ('skills_preferences', ?, ?)",
        &params,
    )
    .map_err(|err| AppError::storage(format!("Failed to save skill preferences: {}", err)))?;
    Ok(())
}

fn load_skill_marketplace_install_history(
    db: &Database,
) -> Result<Vec<SkillMarketplaceInstallHistoryItem>, AppError> {
    match db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&SKILL_MARKETPLACE_INSTALL_HISTORY_KEY],
        |row| row.get::<_, String>(0),
    ) {
        Ok(json) => serde_json::from_str(&json).map_err(AppError::from_json_error),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Vec::new()),
        Err(err) => Err(AppError::storage(format!(
            "Failed to load marketplace install history: {}",
            err
        ))),
    }
}

fn save_skill_marketplace_install_history(
    db: &Database,
    history: &[SkillMarketplaceInstallHistoryItem],
) -> Result<(), AppError> {
    let json = serde_json::to_string(history).map_err(AppError::from_json_error)?;
    let updated_at = history
        .first()
        .map(|item| item.installed_at.clone())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
        &[&SKILL_MARKETPLACE_INSTALL_HISTORY_KEY, &json, &updated_at],
    )
    .map_err(|err| {
        AppError::storage(format!(
            "Failed to save marketplace install history: {}",
            err
        ))
    })?;
    Ok(())
}

fn record_skill_marketplace_install_history(
    db: &Database,
    result: &SkillMarketplaceInstallResult,
) -> Result<(), AppError> {
    let installed_at = chrono::Utc::now().to_rfc3339();
    let mut history = load_skill_marketplace_install_history(db)?;
    history.insert(
        0,
        SkillMarketplaceInstallHistoryItem {
            id: uuid::Uuid::new_v4().to_string(),
            marketplace_id: result.marketplace_id.clone(),
            skill_name: result.entry.name.clone(),
            display_name: result.entry.title.clone(),
            manifest_url: result.manifest_url.clone(),
            source_url: result.entry.source_url.clone(),
            content_source_summary: skill_marketplace_content_source_summary(&result.entry),
            installed_skill_name: result.installed_skill.name.clone(),
            target_remote_user_id: result.target_remote_user_id.clone(),
            installed_at,
        },
    );
    history.truncate(SKILL_MARKETPLACE_INSTALL_HISTORY_MAX_ITEMS);
    save_skill_marketplace_install_history(db, &history)
}

fn skill_marketplace_content_source_summary(entry: &SkillMarketplaceEntry) -> String {
    if let Some(source_url) = entry.source_url.as_ref() {
        return source_url.clone();
    }
    if entry.content.is_some() {
        return "inline manifest content".to_string();
    }
    "unknown source".to_string()
}

fn normalize_optional_marketplace_history_filter(value: Option<String>) -> Option<String> {
    normalize_optional_trimmed_string(value)
}

pub(crate) fn skills_marketplace_install_history_for_db(
    db: &Database,
    request: SkillMarketplaceInstallHistoryListRequest,
) -> Result<Vec<SkillMarketplaceInstallHistoryItem>, AppError> {
    let limit = normalize_marketplace_install_history_limit(request.limit);
    let marketplace_id = normalize_optional_marketplace_history_filter(request.marketplace_id);
    let skill_name = normalize_optional_marketplace_history_filter(request.skill_name);
    let target_remote_user_id =
        normalize_optional_target_remote_user_id(request.target_remote_user_id);

    let mut history = load_skill_marketplace_install_history(db)?;
    history.retain(|item| {
        let marketplace_matches = marketplace_id
            .as_ref()
            .is_none_or(|value| item.marketplace_id == *value);
        let skill_matches = skill_name
            .as_ref()
            .is_none_or(|value| item.skill_name == *value || item.installed_skill_name == *value);
        let target_matches = target_remote_user_id
            .as_deref()
            .is_none_or(|value| item.target_remote_user_id.as_deref() == Some(value));
        marketplace_matches && skill_matches && target_matches
    });
    history.truncate(limit);
    Ok(history)
}

#[cfg(test)]
mod tests {
    use super::{
        SkillInstallRequest, SkillInvokeRequest, SkillInvokeSessionRequest, SkillListItem,
        SkillMarketplaceInstallHistoryListRequest, SkillMarketplaceInstallRequest,
        SkillMarketplaceListRequest, SkillPreferences, SkillRuntimeExecuteRequest,
        discover_skills_from_roots, execute_skill_runtime_with_roots, find_skill_detail_in_roots,
        install_skill_with_root, invoke_skill_into_session_with_roots, invoke_skill_with_roots,
        list_skills_from_db_with_roots, load_skill_preferences,
        resolve_session_skill_invocation_limit, resolve_skill_search_limit, save_skill_preferences,
        skills_list_session_invocations_for_db, skills_marketplace_install_history_for_db,
        skills_marketplace_install_with_root, skills_marketplace_list_for_request,
    };
    use crate::backend::{CreateSessionMessageInput, Database, SessionMessageRole, SessionService};
    use std::fs;
    use std::path::{Path, PathBuf};
    use uuid::Uuid;

    struct TempSkillWorkspace {
        root: PathBuf,
    }

    impl TempSkillWorkspace {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("hermes-skills-test-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).expect("create temp workspace");
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }
    }

    impl Drop for TempSkillWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn write_skill(dir: &Path, frontmatter_name: &str) -> String {
        fs::create_dir_all(dir).expect("create skill dir");
        let skill_file = dir.join("SKILL.md");
        fs::write(
            &skill_file,
            format!(
                "---\nname: {frontmatter_name}\ndescription: Example skill\n---\n\n# {frontmatter_name}\n"
            ),
        )
        .expect("write skill file");
        skill_file.display().to_string()
    }

    #[test]
    fn discover_skills_collects_root_and_child_skills_with_display_metadata() {
        let workspace = TempSkillWorkspace::new();
        let hermes_root = workspace.path().join("hermes-skills");
        let root_skill_path = write_skill(&hermes_root, "Root Helper");
        let child_skill_dir = hermes_root.join("plan");
        let child_skill_path = write_skill(&child_skill_dir, "Plan Designer");

        let skills = discover_skills_from_roots(&[("hermes".to_string(), hermes_root)])
            .expect("discover skills");

        assert_eq!(
            skills,
            vec![
                SkillListItem {
                    name: "hermes-skills".to_string(),
                    display_name: "Root Helper".to_string(),
                    source: "hermes".to_string(),
                    path: root_skill_path,
                    enabled: true,
                },
                SkillListItem {
                    name: "plan".to_string(),
                    display_name: "Plan Designer".to_string(),
                    source: "hermes".to_string(),
                    path: child_skill_path,
                    enabled: true,
                },
            ]
        );
    }

    #[test]
    fn discover_skills_prefers_the_first_source_for_duplicate_names() {
        let workspace = TempSkillWorkspace::new();
        let codex_root = workspace.path().join("codex-skills");
        let agents_root = workspace.path().join("agents-skills");
        let first_path = write_skill(&codex_root.join("plan"), "Codex Plan");
        write_skill(&agents_root.join("plan"), "Agents Plan");
        let unique_path = write_skill(&agents_root.join("debug"), "Debug Specialist");

        let skills = discover_skills_from_roots(&[
            ("codex".to_string(), codex_root),
            ("agents".to_string(), agents_root),
        ])
        .expect("discover skills");

        assert_eq!(
            skills,
            vec![
                SkillListItem {
                    name: "debug".to_string(),
                    display_name: "Debug Specialist".to_string(),
                    source: "agents".to_string(),
                    path: unique_path,
                    enabled: true,
                },
                SkillListItem {
                    name: "plan".to_string(),
                    display_name: "Codex Plan".to_string(),
                    source: "codex".to_string(),
                    path: first_path,
                    enabled: true,
                },
            ]
        );
    }

    #[test]
    fn skill_preferences_round_trip_disabled_names() {
        let db = Database::in_memory().expect("db should initialize");
        let preferences = SkillPreferences {
            disabled_names: vec!["plan".to_string(), "debug".to_string()],
        };

        save_skill_preferences(&db, &preferences).expect("preferences should save");
        let loaded = load_skill_preferences(&db).expect("preferences should load");

        assert_eq!(loaded, preferences);
    }

    #[test]
    fn resolve_skill_search_limit_applies_default_and_cap() {
        assert_eq!(resolve_skill_search_limit(None), 20);
        assert_eq!(resolve_skill_search_limit(Some(0)), 1);
        assert_eq!(resolve_skill_search_limit(Some(250)), 100);
    }

    #[test]
    fn list_skills_from_db_with_roots_marks_disabled_skills() {
        let workspace = TempSkillWorkspace::new();
        let db = Database::in_memory().expect("db should initialize");
        let skill_root = workspace.path().join("hermes-skills");
        write_skill(&skill_root.join("plan"), "Plan Designer");
        save_skill_preferences(
            &db,
            &SkillPreferences {
                disabled_names: vec!["plan".to_string()],
            },
        )
        .expect("preferences should save");

        let skills = list_skills_from_db_with_roots(&db, &[("hermes".to_string(), skill_root)])
            .expect("skills should load");

        assert_eq!(skills.len(), 1);
        assert!(!skills[0].enabled);
    }

    #[test]
    fn find_skill_detail_in_roots_requires_exact_discovered_name() {
        let workspace = TempSkillWorkspace::new();
        let db = Database::in_memory().expect("db should initialize");
        let skill_root = workspace.path().join("hermes-skills");
        let skill_path = write_skill(&skill_root.join("plan"), "Plan Designer");

        let detail = find_skill_detail_in_roots(
            &db,
            &[("hermes".to_string(), skill_root.clone())],
            "plan".to_string(),
        )
        .expect("skill detail should load");

        assert_eq!(detail.name, "plan");
        assert_eq!(detail.display_name, "Plan Designer");
        assert_eq!(detail.path, skill_path);
        assert!(
            detail.content.contains("description: Example skill"),
            "expected full skill content"
        );

        let err = find_skill_detail_in_roots(
            &db,
            &[("hermes".to_string(), skill_root)],
            "../plan".to_string(),
        )
        .expect_err("path traversal names should be rejected");
        assert_eq!(err.code, "validation_error");
    }

    #[test]
    fn invoke_skill_into_session_records_payload_as_session_message() {
        let workspace = TempSkillWorkspace::new();
        let hermes_root = workspace.path().join("hermes-skills");
        write_skill(&hermes_root.join("plan"), "Plan Designer");
        let db = Database::in_memory().expect("db should initialize");
        let service = crate::backend::SessionServiceImpl::new(db.clone());
        let session = service
            .create(crate::backend::CreateSessionInput {
                source: crate::backend::SessionSource::Cli,
                title: "Skill invocation session".to_string(),
                model_name: None,
                parent_session_id: None,
            })
            .expect("session should create");

        let saved = invoke_skill_into_session_with_roots(
            &db,
            &[("hermes".to_string(), hermes_root)],
            SkillInvokeSessionRequest {
                name: "plan".to_string(),
                instruction: Some("Draft launch plan".to_string()),
                session_id: session.id.clone(),
            },
        )
        .expect("skill invocation should save into session");

        assert_eq!(saved.session_id, session.id);
        assert_eq!(saved.invocation.name, "plan");
        assert_eq!(
            saved.message.role,
            crate::backend::SessionMessageRole::System
        );
        assert_eq!(saved.message.source, "skill_invocation");
        assert!(saved.message.content.contains("skill	command=/plan"));
        assert!(saved.message.content.contains("Draft launch plan"));
    }

    #[test]
    fn list_session_skill_invocations_returns_only_skill_source_messages() {
        let workspace = TempSkillWorkspace::new();
        let hermes_root = workspace.path().join("hermes-skills");
        write_skill(&hermes_root.join("plan"), "Plan Designer");
        let db = Database::in_memory().expect("db should initialize");
        let service = crate::backend::SessionServiceImpl::new(db.clone());
        let session = service
            .create(crate::backend::CreateSessionInput {
                source: crate::backend::SessionSource::Cli,
                title: "Skill invocation history".to_string(),
                model_name: None,
                parent_session_id: None,
            })
            .expect("session should create");

        let first = invoke_skill_into_session_with_roots(
            &db,
            &[("hermes".to_string(), hermes_root.clone())],
            SkillInvokeSessionRequest {
                name: "plan".to_string(),
                instruction: Some("First invocation".to_string()),
                session_id: session.id.clone(),
            },
        )
        .expect("first invocation should save");

        let local_message = service
            .create_message(CreateSessionMessageInput {
                session_id: session.id.clone(),
                role: SessionMessageRole::Note,
                content: "Local note that should be excluded".to_string(),
                source: "local".to_string(),
            })
            .expect("local message should save");

        let second = invoke_skill_into_session_with_roots(
            &db,
            &[("hermes".to_string(), hermes_root)],
            SkillInvokeSessionRequest {
                name: "plan".to_string(),
                instruction: Some("Second invocation".to_string()),
                session_id: session.id.clone(),
            },
        )
        .expect("second invocation should save");

        db.execute(
            "UPDATE session_messages SET created_at = ?2 WHERE id = ?1",
            &[
                &first.message.id as &dyn rusqlite::ToSql,
                &"2026-01-01T00:00:00Z",
            ],
        )
        .expect("first invocation timestamp should update");
        db.execute(
            "UPDATE session_messages SET created_at = ?2 WHERE id = ?1",
            &[
                &local_message.id as &dyn rusqlite::ToSql,
                &"2026-01-01T00:01:00Z",
            ],
        )
        .expect("local message timestamp should update");
        db.execute(
            "UPDATE session_messages SET created_at = ?2 WHERE id = ?1",
            &[
                &second.message.id as &dyn rusqlite::ToSql,
                &"2026-01-01T00:02:00Z",
            ],
        )
        .expect("second invocation timestamp should update");

        let items = skills_list_session_invocations_for_db(
            &db,
            super::SkillSessionInvocationListRequest {
                session_id: session.id.clone(),
                limit: Some(10),
            },
        )
        .expect("skill invocations should list");

        assert_eq!(items.len(), 2);
        assert_eq!(
            items
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>(),
            vec![second.message.id.as_str(), first.message.id.as_str()]
        );
        assert!(
            items.iter().all(|item| item.source == "skill_invocation"),
            "expected only skill invocation messages"
        );
        assert!(
            items.iter().all(|item| item.session_id == session.id),
            "expected session id to round-trip"
        );
        assert!(
            items
                .iter()
                .all(|item| item.role == SessionMessageRole::System),
            "expected invocation messages to preserve role"
        );
        assert!(
            items
                .iter()
                .any(|item| item.content.contains("First invocation"))
        );
        assert!(
            items
                .iter()
                .any(|item| item.content.contains("Second invocation"))
        );
        assert!(
            items.iter().all(|item| item.id != local_message.id),
            "non-invocation local messages should be excluded"
        );
    }

    #[test]
    fn resolve_session_skill_invocation_limit_applies_default_and_cap() {
        assert_eq!(resolve_session_skill_invocation_limit(None), 20);
        assert_eq!(resolve_session_skill_invocation_limit(Some(0)), 1);
        assert_eq!(resolve_session_skill_invocation_limit(Some(250)), 100);
    }

    #[test]
    fn invoke_skill_with_roots_renders_runtime_payload_for_enabled_skill() {
        let workspace = TempSkillWorkspace::new();
        let hermes_root = workspace.path().join("hermes-skills");
        let skill_path = write_skill(&hermes_root.join("plan"), "Plan Designer");
        let db = Database::in_memory().expect("db should initialize");

        let payload = invoke_skill_with_roots(
            &db,
            &[("hermes".to_string(), hermes_root)],
            SkillInvokeRequest {
                name: "/plan-designer".to_string(),
                instruction: Some("Draft a launch plan".to_string()),
            },
        )
        .expect("enabled skill should render invocation payload");

        assert_eq!(payload.name, "plan");
        assert_eq!(payload.display_name, "Plan Designer");
        assert_eq!(payload.command, "/plan-designer");
        assert_eq!(payload.source, "hermes");
        assert_eq!(payload.path, skill_path);
        assert_eq!(payload.instruction, Some("Draft a launch plan".to_string()));
        assert!(
            payload
                .rendered_prompt
                .contains("skill	command=/plan-designer	name=plan")
        );
        assert!(
            payload
                .rendered_prompt
                .contains("skill	instruction	Draft a launch plan")
        );
        assert!(payload.rendered_prompt.contains(
            "[SYSTEM: The user has invoked the \"Plan Designer\" skill via /plan-designer."
        ));
        assert!(payload.rendered_prompt.contains("# Plan Designer"));
    }

    #[test]
    fn invoke_skill_with_roots_rejects_disabled_skill() {
        let workspace = TempSkillWorkspace::new();
        let hermes_root = workspace.path().join("hermes-skills");
        write_skill(&hermes_root.join("plan"), "Plan Designer");
        let db = Database::in_memory().expect("db should initialize");
        save_skill_preferences(
            &db,
            &SkillPreferences {
                disabled_names: vec!["plan".to_string()],
            },
        )
        .expect("preferences should save");

        let err = invoke_skill_with_roots(
            &db,
            &[("hermes".to_string(), hermes_root)],
            SkillInvokeRequest {
                name: "plan".to_string(),
                instruction: None,
            },
        )
        .expect_err("disabled skills should not be invokable");

        assert_eq!(err.code, "validation_error");
        assert_eq!(err.message, "skill is disabled");
    }

    #[test]
    fn execute_skill_runtime_with_roots_generates_dry_run_package_and_saves_session_context() {
        let workspace = TempSkillWorkspace::new();
        let hermes_root = workspace.path().join("hermes-skills");
        write_skill(&hermes_root.join("plan"), "Plan Designer");
        let db = Database::in_memory().expect("db should initialize");
        let service = crate::backend::SessionServiceImpl::new(db.clone());
        let session = service
            .create(crate::backend::CreateSessionInput {
                source: crate::backend::SessionSource::Cli,
                title: "Skill runtime dry run".to_string(),
                model_name: None,
                parent_session_id: None,
            })
            .expect("session should create");

        let result = execute_skill_runtime_with_roots(
            &db,
            &[("hermes".to_string(), hermes_root)],
            SkillRuntimeExecuteRequest {
                name: "plan".to_string(),
                instruction: Some("Draft a launch plan".to_string()),
                session_id: Some(session.id.clone()),
                save_to_session: Some(true),
                dry_run: Some(true),
                tool_command: None,
                timeout_ms: Some(250),
            },
        )
        .expect("dry-run runtime package should generate");

        assert!(!result.executed);
        assert!(result.dry_run);
        assert!(result.runtime_result.is_none());
        assert_eq!(result.invocation.name, "plan");
        assert_eq!(result.execution_package.command, "printf");
        assert!(
            result
                .execution_package
                .args
                .iter()
                .any(|arg| arg.contains("skill-runtime"))
        );
        assert!(result.execution_package.preview.contains("command=/plan"));
        assert!(
            result
                .execution_package
                .preview
                .contains("Draft a launch plan")
        );
        let message = result
            .session_message
            .expect("session context should be saved");
        assert_eq!(message.session_id, session.id);
        assert_eq!(message.source, "skill_invocation");
        assert!(message.content.contains("Draft a launch plan"));
    }

    #[test]
    fn execute_skill_runtime_with_roots_runs_allowlisted_printf_validation() {
        let workspace = TempSkillWorkspace::new();
        let hermes_root = workspace.path().join("hermes-skills");
        write_skill(&hermes_root.join("plan"), "Plan Designer");
        let db = Database::in_memory().expect("db should initialize");

        let result = execute_skill_runtime_with_roots(
            &db,
            &[("hermes".to_string(), hermes_root)],
            SkillRuntimeExecuteRequest {
                name: "/plan".to_string(),
                instruction: Some("Validate local runtime bridge".to_string()),
                session_id: None,
                save_to_session: Some(false),
                dry_run: Some(false),
                tool_command: Some("printf".to_string()),
                timeout_ms: Some(250),
            },
        )
        .expect("allowlisted printf validation should execute");

        assert!(result.executed);
        assert!(!result.dry_run);
        assert!(result.session_message.is_none());
        let runtime_result = result
            .runtime_result
            .expect("runtime adapter result should exist");
        assert_eq!(runtime_result.exit_code, 0);
        assert!(runtime_result.stdout.contains("skill-runtime"));
        assert!(
            runtime_result
                .stdout
                .contains("Validate local runtime bridge")
        );
        assert!(
            runtime_result
                .audit_message
                .contains("allowlisted command `printf`")
        );
    }

    #[test]
    fn execute_skill_runtime_with_roots_rejects_disabled_skill_before_tool_execution() {
        let workspace = TempSkillWorkspace::new();
        let hermes_root = workspace.path().join("hermes-skills");
        write_skill(&hermes_root.join("plan"), "Plan Designer");
        let db = Database::in_memory().expect("db should initialize");
        save_skill_preferences(
            &db,
            &SkillPreferences {
                disabled_names: vec!["plan".to_string()],
            },
        )
        .expect("preferences should save");

        let err = execute_skill_runtime_with_roots(
            &db,
            &[("hermes".to_string(), hermes_root)],
            SkillRuntimeExecuteRequest {
                name: "plan".to_string(),
                instruction: None,
                session_id: None,
                save_to_session: Some(false),
                dry_run: Some(false),
                tool_command: Some("printf".to_string()),
                timeout_ms: Some(250),
            },
        )
        .expect_err("disabled skill should not reach runtime adapter");

        assert_eq!(err.code, "validation_error");
        assert_eq!(err.message, "skill is disabled");
    }

    #[test]
    fn install_skill_with_root_writes_ascii_skill_and_supports_force_overwrite() {
        let workspace = TempSkillWorkspace::new();
        let db = Database::in_memory().expect("db should initialize");
        let install_root = workspace.path().join("hermes-skills");

        let installed = install_skill_with_root(
            &db,
            &install_root,
            SkillInstallRequest {
                name: "planner".to_string(),
                title: Some("Planner".to_string()),
                description: Some("Example planner skill".to_string()),
                content: Some("## Usage\n\nFollow the plan.\n".to_string()),
                force: false,
            },
        )
        .expect("skill should install");

        assert_eq!(installed.name, "planner");
        assert_eq!(installed.display_name, "Planner");
        assert_eq!(
            installed.description.as_deref(),
            Some("Example planner skill")
        );
        assert!(
            installed.path.ends_with("planner/SKILL.md"),
            "expected installed skill path"
        );

        let written = fs::read_to_string(PathBuf::from(&installed.path)).expect("read installed");
        assert!(written.is_ascii(), "generated skill should remain ASCII");
        assert!(written.contains("description: Example planner skill"));

        let err = install_skill_with_root(
            &db,
            &install_root,
            SkillInstallRequest {
                name: "planner".to_string(),
                title: Some("Planner".to_string()),
                description: Some("Duplicate".to_string()),
                content: Some("duplicate".to_string()),
                force: false,
            },
        )
        .expect_err("duplicate install should fail without force");
        assert_eq!(err.code, "validation_error");

        let overwritten = install_skill_with_root(
            &db,
            &install_root,
            SkillInstallRequest {
                name: "planner".to_string(),
                title: Some("Planner".to_string()),
                description: Some("Forced overwrite".to_string()),
                content: Some("Updated content".to_string()),
                force: true,
            },
        )
        .expect("forced install should overwrite");
        assert_eq!(overwritten.description.as_deref(), Some("Forced overwrite"));
        assert!(overwritten.content.contains("Updated content"));
    }

    #[test]
    fn install_skill_with_root_rejects_invalid_names() {
        let workspace = TempSkillWorkspace::new();
        let db = Database::in_memory().expect("db should initialize");
        let install_root = workspace.path().join("hermes-skills");

        let err = install_skill_with_root(
            &db,
            &install_root,
            SkillInstallRequest {
                name: "../escape".to_string(),
                title: Some("Escape".to_string()),
                description: Some("bad".to_string()),
                content: Some("bad".to_string()),
                force: false,
            },
        )
        .expect_err("invalid names should be rejected");

        assert_eq!(err.code, "validation_error");
    }

    #[tokio::test]
    async fn skill_marketplace_lists_file_manifest_and_installs_selected_skill() {
        let workspace = TempSkillWorkspace::new();
        let db = Database::in_memory().expect("db should initialize");
        let marketplace_root = workspace.path().join("marketplace");
        fs::create_dir_all(&marketplace_root).expect("create marketplace root");
        let remote_skill_file = marketplace_root.join("REMOTE_SKILL.md");
        fs::write(
            &remote_skill_file,
            "---\nname: Market Planner\ndescription: Remote marketplace skill\n---\n\n# Market Planner\n\nUse this skill from a catalog.\n",
        )
        .expect("write remote skill");
        let manifest_file = marketplace_root.join("marketplace.json");
        let manifest_json = serde_json::json!({
            "schema_version": 1,
            "marketplace_id": "local-fixture",
            "skills": [
                {
                    "name": "market-planner",
                    "title": "Market Planner",
                    "description": "Remote marketplace skill",
                    "source_url": remote_skill_file.display().to_string(),
                    "tags": ["planning", "remote"]
                }
            ]
        })
        .to_string();
        fs::write(&manifest_file, manifest_json).expect("write manifest");

        let catalog = skills_marketplace_list_for_request(SkillMarketplaceListRequest {
            manifest_url: manifest_file.display().to_string(),
            limit: None,
        })
        .await
        .expect("marketplace catalog should load");
        assert_eq!(catalog.marketplace_id, "local-fixture");
        assert_eq!(catalog.skills.len(), 1);
        assert_eq!(catalog.skills[0].name, "market-planner");
        assert_eq!(catalog.skills[0].tags, vec!["planning", "remote"]);

        let installed = skills_marketplace_install_with_root(
            &db,
            &workspace.path().join("installed-skills"),
            SkillMarketplaceInstallRequest {
                manifest_url: manifest_file.display().to_string(),
                name: "market-planner".to_string(),
                force: Some(false),
                target_remote_user_id: None,
            },
        )
        .await
        .expect("marketplace skill should install");

        assert_eq!(installed.marketplace_id, "local-fixture");
        assert_eq!(installed.entry.name, "market-planner");
        assert_eq!(installed.installed_skill.name, "market-planner");
        assert!(
            installed
                .installed_skill
                .content
                .contains("Use this skill from a catalog")
        );
    }

    #[tokio::test]
    async fn skill_marketplace_installs_inline_content_without_source_url() {
        let workspace = TempSkillWorkspace::new();
        let db = Database::in_memory().expect("db should initialize");
        let manifest_file = workspace.path().join("inline-marketplace.json");
        let manifest_json = serde_json::json!({
            "schema_version": 1,
            "marketplace_id": "inline-fixture",
            "skills": [
                {
                    "name": "inline-planner",
                    "title": "Inline Planner",
                    "description": "Inline marketplace skill",
                    "content": "# Inline Planner\n\nUse this inline skill from a manifest."
                }
            ]
        })
        .to_string();
        fs::write(&manifest_file, manifest_json).expect("write inline manifest");

        let catalog = skills_marketplace_list_for_request(SkillMarketplaceListRequest {
            manifest_url: manifest_file.display().to_string(),
            limit: None,
        })
        .await
        .expect("inline marketplace catalog should load");
        assert_eq!(catalog.marketplace_id, "inline-fixture");
        assert_eq!(catalog.skills.len(), 1);
        assert_eq!(catalog.skills[0].source_url, None);
        assert!(catalog.skills[0].content.is_some());

        let installed = skills_marketplace_install_with_root(
            &db,
            &workspace.path().join("installed-inline-skills"),
            SkillMarketplaceInstallRequest {
                manifest_url: manifest_file.display().to_string(),
                name: "inline-planner".to_string(),
                force: Some(false),
                target_remote_user_id: None,
            },
        )
        .await
        .expect("inline marketplace skill should install");

        assert_eq!(installed.marketplace_id, "inline-fixture");
        assert_eq!(installed.entry.name, "inline-planner");
        assert!(
            installed
                .installed_skill
                .content
                .contains("Use this inline skill")
        );
    }

    #[tokio::test]
    async fn skill_marketplace_install_records_auditable_history() {
        let workspace = TempSkillWorkspace::new();
        let db = Database::in_memory().expect("db should initialize");
        let marketplace_root = workspace.path().join("history-marketplace");
        fs::create_dir_all(&marketplace_root).expect("create history marketplace root");
        let remote_skill_file = marketplace_root.join("AUDIT_SKILL.md");
        fs::write(
            &remote_skill_file,
            "---\nname: Audit Skill\ndescription: Marketplace audit skill\n---\n\n# Audit Skill\n\nTrack installs.\n",
        )
        .expect("write remote skill");
        let manifest_file = marketplace_root.join("history-marketplace.json");
        fs::write(
            &manifest_file,
            serde_json::json!({
                "schema_version": 1,
                "marketplace_id": "history-fixture",
                "skills": [
                    {
                        "name": "audit-skill",
                        "title": "Audit Skill",
                        "description": "Marketplace audit skill",
                        "source_url": remote_skill_file.display().to_string()
                    }
                ]
            })
            .to_string(),
        )
        .expect("write manifest");

        let installed = skills_marketplace_install_with_root(
            &db,
            &workspace.path().join("installed-audit-skills"),
            SkillMarketplaceInstallRequest {
                manifest_url: manifest_file.display().to_string(),
                name: "audit-skill".to_string(),
                force: Some(false),
                target_remote_user_id: Some("  remote-user-123  ".to_string()),
            },
        )
        .await
        .expect("marketplace skill should install");

        let history = skills_marketplace_install_history_for_db(
            &db,
            SkillMarketplaceInstallHistoryListRequest {
                limit: None,
                marketplace_id: None,
                skill_name: None,
                target_remote_user_id: None,
            },
        )
        .expect("history should load");

        assert_eq!(
            installed.target_remote_user_id.as_deref(),
            Some("remote-user-123")
        );
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].marketplace_id, "history-fixture");
        assert_eq!(history[0].skill_name, "audit-skill");
        assert_eq!(history[0].display_name, "Audit Skill");
        assert_eq!(history[0].manifest_url, manifest_file.display().to_string());
        assert_eq!(
            history[0].target_remote_user_id.as_deref(),
            Some("remote-user-123")
        );
        assert_eq!(
            history[0].source_url.as_deref(),
            Some(remote_skill_file.display().to_string().as_str())
        );
        assert_eq!(
            history[0].installed_skill_name,
            installed.installed_skill.name
        );
        assert!(!history[0].id.is_empty());
        assert!(!history[0].installed_at.is_empty());
        assert!(history[0].content_source_summary.contains("AUDIT_SKILL.md"));
    }

    #[test]
    fn skill_marketplace_history_without_remote_user_field_still_deserializes() {
        let db = Database::in_memory().expect("db should initialize");
        let legacy_history_json = serde_json::json!([
            {
                "id": "history-1",
                "marketplace_id": "legacy-fixture",
                "skill_name": "legacy-skill",
                "display_name": "Legacy Skill",
                "manifest_url": "file:///tmp/legacy-marketplace.json",
                "source_url": null,
                "content_source_summary": "inline manifest content",
                "installed_skill_name": "legacy-skill",
                "installed_at": "2026-04-29T00:00:00Z"
            }
        ])
        .to_string();

        db.execute(
            "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
            &[
                &super::SKILL_MARKETPLACE_INSTALL_HISTORY_KEY,
                &legacy_history_json,
                &"2026-04-29T00:00:00Z",
            ],
        )
        .expect("write legacy history");

        let history = skills_marketplace_install_history_for_db(
            &db,
            SkillMarketplaceInstallHistoryListRequest {
                limit: None,
                marketplace_id: None,
                skill_name: None,
                target_remote_user_id: None,
            },
        )
        .expect("legacy history should load");

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].marketplace_id, "legacy-fixture");
        assert_eq!(history[0].skill_name, "legacy-skill");
        assert_eq!(history[0].target_remote_user_id, None);
    }

    #[tokio::test]
    async fn skill_marketplace_history_supports_filters_and_limit() {
        let workspace = TempSkillWorkspace::new();
        let db = Database::in_memory().expect("db should initialize");
        let marketplace_root = workspace.path().join("filter-marketplace");
        fs::create_dir_all(&marketplace_root).expect("create filter marketplace root");

        let alpha_skill_file = marketplace_root.join("ALPHA.md");
        fs::write(
            &alpha_skill_file,
            "---\nname: Alpha Skill\ndescription: Alpha remote skill\n---\n\n# Alpha Skill\n\nAlpha remote content.\n",
        )
        .expect("write alpha skill");

        let beta_manifest_file = marketplace_root.join("beta-marketplace.json");
        fs::write(
            &beta_manifest_file,
            serde_json::json!({
                "schema_version": 1,
                "marketplace_id": "beta-fixture",
                "skills": [
                    {
                        "name": "beta-inline",
                        "title": "Beta Inline",
                        "description": "Inline beta skill",
                        "content": "# Beta Inline\n\nInline beta content."
                    }
                ]
            })
            .to_string(),
        )
        .expect("write beta manifest");

        let alpha_manifest_file = marketplace_root.join("alpha-marketplace.json");
        fs::write(
            &alpha_manifest_file,
            serde_json::json!({
                "schema_version": 1,
                "marketplace_id": "alpha-fixture",
                "skills": [
                    {
                        "name": "alpha-remote",
                        "title": "Alpha Skill",
                        "description": "Alpha remote skill",
                        "source_url": alpha_skill_file.display().to_string()
                    }
                ]
            })
            .to_string(),
        )
        .expect("write alpha manifest");

        skills_marketplace_install_with_root(
            &db,
            &workspace.path().join("installed-filter-skills"),
            SkillMarketplaceInstallRequest {
                manifest_url: beta_manifest_file.display().to_string(),
                name: "beta-inline".to_string(),
                force: Some(false),
                target_remote_user_id: Some(" beta-remote ".to_string()),
            },
        )
        .await
        .expect("beta skill should install");

        skills_marketplace_install_with_root(
            &db,
            &workspace.path().join("installed-filter-skills"),
            SkillMarketplaceInstallRequest {
                manifest_url: alpha_manifest_file.display().to_string(),
                name: "alpha-remote".to_string(),
                force: Some(false),
                target_remote_user_id: Some("alpha-remote-user".to_string()),
            },
        )
        .await
        .expect("alpha skill should install");

        let filtered = skills_marketplace_install_history_for_db(
            &db,
            SkillMarketplaceInstallHistoryListRequest {
                limit: Some(10),
                marketplace_id: Some("alpha-fixture".to_string()),
                skill_name: Some("alpha-remote".to_string()),
                target_remote_user_id: None,
            },
        )
        .expect("filtered history should load");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].marketplace_id, "alpha-fixture");
        assert_eq!(filtered[0].skill_name, "alpha-remote");
        assert_eq!(
            filtered[0].source_url.as_deref(),
            Some(alpha_skill_file.display().to_string().as_str())
        );

        let limited = skills_marketplace_install_history_for_db(
            &db,
            SkillMarketplaceInstallHistoryListRequest {
                limit: Some(1),
                marketplace_id: None,
                skill_name: None,
                target_remote_user_id: None,
            },
        )
        .expect("limited history should load");

        assert_eq!(limited.len(), 1);
        assert_eq!(limited[0].marketplace_id, "alpha-fixture");
        assert_eq!(limited[0].skill_name, "alpha-remote");
        assert_eq!(
            limited[0].content_source_summary,
            alpha_skill_file.display().to_string()
        );

        let target_filtered = skills_marketplace_install_history_for_db(
            &db,
            SkillMarketplaceInstallHistoryListRequest {
                limit: Some(1),
                marketplace_id: None,
                skill_name: None,
                target_remote_user_id: Some(" beta-remote ".to_string()),
            },
        )
        .expect("target-filtered history should load");

        assert_eq!(target_filtered.len(), 1);
        assert_eq!(target_filtered[0].marketplace_id, "beta-fixture");
        assert_eq!(target_filtered[0].skill_name, "beta-inline");
        assert_eq!(
            target_filtered[0].target_remote_user_id.as_deref(),
            Some("beta-remote")
        );

        let unmatched_target = skills_marketplace_install_history_for_db(
            &db,
            SkillMarketplaceInstallHistoryListRequest {
                limit: Some(10),
                marketplace_id: None,
                skill_name: None,
                target_remote_user_id: Some("missing-remote".to_string()),
            },
        )
        .expect("unmatched target-filtered history should load");
        assert!(unmatched_target.is_empty());
    }
}
