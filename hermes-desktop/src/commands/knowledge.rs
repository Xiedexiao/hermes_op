//! Knowledge 命令

use chrono::Utc;
use reqwest::header::CONTENT_TYPE;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tauri::State;

use crate::backend::{
    AppError, ContextItemType, CreateMissionContextItemInput, Database, KnowledgeFeedItem,
    KnowledgeSource, MissionContextItem, MissionService, MissionServiceImpl,
};

const MAX_LOCAL_IMPORT_BYTES: u64 = 1024 * 1024;
const DEFAULT_FOLDER_IMPORT_MAX_FILES: usize = 20;
const MAX_FOLDER_IMPORT_FILES: usize = 100;
const URL_PREVIEW_TIMEOUT_SECS: u64 = 8;
const MAX_URL_PREVIEW_BYTES: usize = 128 * 1024;
const MAX_URL_PREVIEW_CHARS: usize = 600;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeListRequest {
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeImportRequest {
    pub mission_id: String,
    pub r#type: ContextItemType,
    pub title: String,
    pub content_preview: Option<String>,
    pub source_uri: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeFolderImportRequest {
    pub mission_id: String,
    pub folder_path: String,
    pub title_prefix: Option<String>,
    pub recursive: Option<bool>,
    pub max_files: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeFolderImportResponse {
    pub imported_count: usize,
    pub skipped_count: usize,
    pub items: Vec<MissionContextItem>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeUrlPreviewRequest {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeUrlPreviewResponse {
    pub url: String,
    pub status: u16,
    pub content_type: Option<String>,
    pub title: Option<String>,
    pub preview: String,
    pub fetched_at: String,
    pub truncated: bool,
}

#[tauri::command]
pub fn knowledge_list(
    db: State<'_, Database>,
    request: Option<KnowledgeListRequest>,
) -> Result<Vec<KnowledgeFeedItem>, AppError> {
    let service = MissionServiceImpl::new(db.inner().clone());
    service.list_knowledge_feed(request.and_then(|value| value.query))
}

#[tauri::command]
pub fn knowledge_source_list(
    db: State<'_, Database>,
    request: Option<KnowledgeListRequest>,
) -> Result<Vec<KnowledgeSource>, AppError> {
    let service = MissionServiceImpl::new(db.inner().clone());
    service.list_knowledge_sources(request.and_then(|value| value.query))
}

#[tauri::command]
pub fn knowledge_import(
    db: State<'_, Database>,
    request: KnowledgeImportRequest,
) -> Result<MissionContextItem, AppError> {
    let service = MissionServiceImpl::new(db.inner().clone());
    service.add_context_item(build_context_item_input(request)?)
}

#[tauri::command]
pub fn knowledge_import_folder(
    db: State<'_, Database>,
    request: KnowledgeFolderImportRequest,
) -> Result<KnowledgeFolderImportResponse, AppError> {
    knowledge_import_folder_for_db(db.inner(), request)
}

#[tauri::command]
pub async fn knowledge_fetch_url_preview(
    request: KnowledgeUrlPreviewRequest,
) -> Result<KnowledgeUrlPreviewResponse, AppError> {
    fetch_url_preview(&request.url).await
}

fn build_context_item_input(
    request: KnowledgeImportRequest,
) -> Result<CreateMissionContextItemInput, AppError> {
    let source_uri = normalize_optional_text(request.source_uri);
    let mut content_preview = normalize_optional_text(request.content_preview);

    let source_uri = match request.r#type {
        ContextItemType::Url => {
            let source_uri = source_uri
                .as_deref()
                .ok_or_else(|| AppError::validation("url import source_uri is required"))?;
            Some(validate_http_url(source_uri, "url import source_uri")?)
        }
        _ => source_uri,
    };

    if request.r#type == ContextItemType::File
        && content_preview.is_none()
        && let Some(source_uri) = source_uri.as_deref()
    {
        content_preview = Some(read_utf8_file(source_uri)?);
    }

    Ok(CreateMissionContextItemInput {
        mission_id: request.mission_id.trim().to_string(),
        r#type: request.r#type,
        title: request.title.trim().to_string(),
        content_preview,
        source_uri,
    })
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

async fn fetch_url_preview(url: &str) -> Result<KnowledgeUrlPreviewResponse, AppError> {
    let url = validate_http_url(url, "knowledge url preview url")?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(URL_PREVIEW_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(5))
        .build()
        .map_err(|err| {
            AppError::runtime(format!("Failed to build preview HTTP client: {}", err))
        })?;

    let mut response = client.get(&url).send().await.map_err(|err| {
        AppError::runtime(format!("Failed to fetch knowledge URL preview: {}", err))
    })?;

    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());
    let (body, body_truncated) =
        read_response_body_limited(&mut response, MAX_URL_PREVIEW_BYTES).await?;
    let document = String::from_utf8_lossy(&body);
    let title = extract_html_title(&document);
    let preview_text = extract_preview_text(&document, title.as_deref());
    let preview_truncated = preview_text.chars().count() > MAX_URL_PREVIEW_CHARS;
    let preview = truncate_chars(&preview_text, MAX_URL_PREVIEW_CHARS);

    Ok(KnowledgeUrlPreviewResponse {
        url,
        status,
        content_type,
        title,
        preview,
        fetched_at: Utc::now().to_rfc3339(),
        truncated: body_truncated || preview_truncated,
    })
}

async fn read_response_body_limited(
    response: &mut reqwest::Response,
    max_bytes: usize,
) -> Result<(Vec<u8>, bool), AppError> {
    let mut body = Vec::new();
    let mut truncated = false;

    while let Some(chunk) = response.chunk().await.map_err(|err| {
        AppError::runtime(format!(
            "Failed to read knowledge URL preview response: {}",
            err
        ))
    })? {
        if body.len() >= max_bytes {
            truncated = true;
            break;
        }

        let remaining = max_bytes - body.len();
        if chunk.len() > remaining {
            body.extend_from_slice(&chunk[..remaining]);
            truncated = true;
            break;
        }

        body.extend_from_slice(&chunk);
    }

    Ok((body, truncated))
}

fn validate_http_url(value: &str, field_name: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation(format!(
            "{} cannot be empty",
            field_name
        )));
    }

    let parsed = reqwest::Url::parse(trimmed).map_err(|err| {
        AppError::validation(format!(
            "{} must be a valid http(s) URL: {}",
            field_name, err
        ))
    })?;

    match parsed.scheme() {
        "http" | "https" => Ok(parsed.to_string()),
        _ => Err(AppError::validation(format!(
            "{} must use http or https",
            field_name
        ))),
    }
}

fn read_utf8_file(source_uri: &str) -> Result<String, AppError> {
    if source_uri.starts_with("http://") || source_uri.starts_with("https://") {
        return Err(AppError::validation(
            "file import source_uri must be a local path",
        ));
    }

    let path = local_path_from_source_uri(source_uri);
    read_utf8_local_path(&path)
}

fn read_utf8_local_path(path: &Path) -> Result<String, AppError> {
    let metadata = fs::metadata(path).map_err(|err| {
        AppError::io(format!(
            "Failed to inspect knowledge import file {}: {}",
            path.display(),
            err
        ))
    })?;

    if !metadata.is_file() {
        return Err(AppError::validation(format!(
            "knowledge import path is not a file: {}",
            path.display()
        )));
    }

    if metadata.len() > MAX_LOCAL_IMPORT_BYTES {
        return Err(AppError::validation(format!(
            "knowledge import file is larger than {} bytes: {}",
            MAX_LOCAL_IMPORT_BYTES,
            path.display()
        )));
    }

    fs::read_to_string(path).map_err(|err| {
        AppError::io(format!(
            "Failed to read UTF-8 knowledge import file {}: {}",
            path.display(),
            err
        ))
    })
}

fn local_path_from_source_uri(source_uri: &str) -> PathBuf {
    Path::new(source_uri.strip_prefix("file://").unwrap_or(source_uri)).to_path_buf()
}

fn knowledge_import_folder_for_db(
    db: &Database,
    request: KnowledgeFolderImportRequest,
) -> Result<KnowledgeFolderImportResponse, AppError> {
    let folder_path = validate_local_folder_path(&request.folder_path)?;
    let recursive = request.recursive.unwrap_or(false);
    let max_files = normalize_folder_import_max_files(request.max_files)?;
    let title_prefix = normalize_optional_text(request.title_prefix);
    let service = MissionServiceImpl::new(db.clone());
    let canonical_root = fs::canonicalize(&folder_path).map_err(|err| {
        AppError::io(format!(
            "Failed to resolve knowledge import folder {}: {}",
            folder_path.display(),
            err
        ))
    })?;
    let collected = collect_supported_folder_files(&canonical_root, recursive)?;
    let mut skipped_count = collected.skipped_count;
    let mut items = Vec::new();
    let supported_file_count = collected.files.len();

    for path in collected.files.into_iter().take(max_files) {
        let canonical_file = match fs::canonicalize(&path) {
            Ok(value) => value,
            Err(_) => {
                skipped_count += 1;
                continue;
            }
        };
        if !canonical_file.starts_with(&canonical_root) {
            skipped_count += 1;
            continue;
        }

        let content_preview = match read_utf8_local_path(&canonical_file) {
            Ok(value) => value,
            Err(_) => {
                skipped_count += 1;
                continue;
            }
        };
        let relative_path = canonical_file
            .strip_prefix(&canonical_root)
            .unwrap_or(canonical_file.as_path());
        let title = build_folder_import_title(title_prefix.as_deref(), relative_path);
        let source_uri = canonical_file.to_string_lossy().to_string();
        let item = service.add_context_item(CreateMissionContextItemInput {
            mission_id: request.mission_id.trim().to_string(),
            r#type: ContextItemType::File,
            title,
            content_preview: Some(content_preview),
            source_uri: Some(source_uri),
        })?;
        items.push(item);
    }

    skipped_count += supported_file_count.saturating_sub(max_files);

    Ok(KnowledgeFolderImportResponse {
        imported_count: items.len(),
        skipped_count,
        summary: format!(
            "Imported {} file(s) from {}. Skipped {} item(s).",
            items.len(),
            canonical_root.display(),
            skipped_count
        ),
        items,
    })
}

fn validate_local_folder_path(folder_path: &str) -> Result<PathBuf, AppError> {
    let trimmed = folder_path.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation(
            "folder import folder_path cannot be empty",
        ));
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Err(AppError::validation(
            "folder import folder_path must be a local path",
        ));
    }

    let path = local_path_from_source_uri(trimmed);
    let metadata = fs::metadata(&path).map_err(|err| {
        AppError::io(format!(
            "Failed to inspect knowledge import folder {}: {}",
            path.display(),
            err
        ))
    })?;
    if !metadata.is_dir() {
        return Err(AppError::validation(format!(
            "knowledge import folder path is not a directory: {}",
            path.display()
        )));
    }
    Ok(path)
}

fn normalize_folder_import_max_files(value: Option<usize>) -> Result<usize, AppError> {
    match value.unwrap_or(DEFAULT_FOLDER_IMPORT_MAX_FILES) {
        0 => Err(AppError::validation(
            "folder import max_files must be at least 1",
        )),
        value if value > MAX_FOLDER_IMPORT_FILES => Err(AppError::validation(format!(
            "folder import max_files must be less than or equal to {}",
            MAX_FOLDER_IMPORT_FILES
        ))),
        value => Ok(value),
    }
}

struct CollectedFolderFiles {
    files: Vec<PathBuf>,
    skipped_count: usize,
}

fn collect_supported_folder_files(
    root: &Path,
    recursive: bool,
) -> Result<CollectedFolderFiles, AppError> {
    let mut files = Vec::new();
    let mut skipped_count = 0_usize;
    let mut saw_entry = false;

    collect_supported_folder_files_into(
        root,
        root,
        recursive,
        &mut saw_entry,
        &mut skipped_count,
        &mut files,
    )?;

    if !saw_entry {
        return Err(AppError::validation(format!(
            "knowledge import folder is empty: {}",
            root.display()
        )));
    }
    if files.is_empty() {
        return Err(AppError::validation(format!(
            "knowledge import folder has no supported files: {}",
            root.display()
        )));
    }

    Ok(CollectedFolderFiles {
        files,
        skipped_count,
    })
}

fn collect_supported_folder_files_into(
    root: &Path,
    current: &Path,
    recursive: bool,
    saw_entry: &mut bool,
    skipped_count: &mut usize,
    files: &mut Vec<PathBuf>,
) -> Result<(), AppError> {
    let mut entries = fs::read_dir(current)
        .map_err(|err| {
            AppError::io(format!(
                "Failed to read knowledge import folder {}: {}",
                current.display(),
                err
            ))
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|err| {
            AppError::io(format!(
                "Failed to enumerate knowledge import folder {}: {}",
                current.display(),
                err
            ))
        })?;
    entries.sort_by_key(|entry| entry.path());

    for entry in entries {
        *saw_entry = true;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|err| {
            AppError::io(format!(
                "Failed to inspect knowledge import entry {}: {}",
                path.display(),
                err
            ))
        })?;

        if file_type.is_dir() {
            if recursive {
                collect_supported_folder_files_into(
                    root,
                    &path,
                    recursive,
                    saw_entry,
                    skipped_count,
                    files,
                )?;
            } else if path != root {
                *skipped_count += 1;
            }
            continue;
        }

        if !file_type.is_file() {
            *skipped_count += 1;
            continue;
        }

        if is_supported_folder_import_file(&path) {
            files.push(path);
        } else {
            *skipped_count += 1;
        }
    }

    Ok(())
}

fn is_supported_folder_import_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "txt" | "json" | "csv"
            )
        })
        .unwrap_or(false)
}

fn build_folder_import_title(title_prefix: Option<&str>, relative_path: &Path) -> String {
    let relative = relative_path.to_string_lossy().replace('\\', "/");
    match title_prefix.filter(|value| !value.trim().is_empty()) {
        Some(prefix) => format!("{} / {}", prefix.trim(), relative),
        None => relative,
    }
}

fn extract_html_title(document: &str) -> Option<String> {
    let lower = document.to_ascii_lowercase();
    let title_start = lower.find("<title")?;
    let content_start = title_start + lower[title_start..].find('>')? + 1;
    let content_end = content_start + lower[content_start..].find("</title>")?;
    let title = decode_html_entities(&document[content_start..content_end]);
    let title = collapse_whitespace(&title);
    (!title.is_empty()).then_some(title)
}

fn extract_preview_text(document: &str, fallback: Option<&str>) -> String {
    let without_blocks = strip_html_block_sections(document);
    let stripped = strip_html_tags(&without_blocks);
    let decoded = decode_html_entities(&stripped);
    let collapsed = collapse_whitespace(&decoded);

    if collapsed.is_empty() {
        fallback.unwrap_or_default().to_string()
    } else {
        collapsed
    }
}

fn strip_html_block_sections(document: &str) -> String {
    let mut output = document.to_string();
    for tag in ["script", "style", "noscript"] {
        output = strip_html_block_tag(&output, tag);
    }
    output
}

fn strip_html_block_tag(document: &str, tag_name: &str) -> String {
    let mut remaining = document;
    let mut stripped = String::with_capacity(document.len());
    let open_tag = format!("<{}", tag_name);
    let close_tag = format!("</{}>", tag_name);

    loop {
        let lower = remaining.to_ascii_lowercase();
        let Some(open_index) = lower.find(&open_tag) else {
            stripped.push_str(remaining);
            break;
        };

        stripped.push_str(&remaining[..open_index]);
        let after_open = &remaining[open_index..];
        let after_open_lower = &lower[open_index..];

        let Some(close_index) = after_open_lower.find(&close_tag) else {
            break;
        };

        let close_end = close_index + close_tag.len();
        remaining = &after_open[close_end..];
    }

    stripped
}

fn strip_html_tags(document: &str) -> String {
    let mut result = String::with_capacity(document.len());
    let mut inside_tag = false;

    for ch in document.chars() {
        match ch {
            '<' => {
                inside_tag = true;
                if !result.ends_with(char::is_whitespace) && !result.is_empty() {
                    result.push(' ');
                }
            }
            '>' => inside_tag = false,
            _ if !inside_tag => result.push(ch),
            _ => {}
        }
    }

    result
}

fn decode_html_entities(value: &str) -> String {
    value
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
}

fn collapse_whitespace(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::{
        KnowledgeFolderImportRequest, KnowledgeImportRequest, MAX_URL_PREVIEW_BYTES,
        MAX_URL_PREVIEW_CHARS, build_context_item_input, fetch_url_preview,
        knowledge_import_folder_for_db,
    };
    use crate::backend::{
        ContextItemType, CreateMissionInput, Database, MissionPriority, MissionRepository,
        MissionService, MissionServiceImpl,
    };
    use std::fs;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener};
    use std::path::{Path, PathBuf};
    use std::thread;
    use std::time::Duration;
    use uuid::Uuid;

    struct TempFileWorkspace {
        root: PathBuf,
    }

    impl TempFileWorkspace {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("hermes-knowledge-file-{}", Uuid::new_v4()));
            fs::create_dir_all(&root).expect("workspace should create");
            Self { root }
        }

        fn write_text(&self, name: &str, contents: &str) -> PathBuf {
            let path = self.root.join(name);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("parent dirs should create");
            }
            fs::write(&path, contents).expect("file should write");
            path
        }

        fn create_dir(&self, name: &str) -> PathBuf {
            let path = self.root.join(name);
            fs::create_dir_all(&path).expect("directory should create");
            path
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

    fn create_test_mission(db: &Database) -> String {
        MissionRepository::new(db.clone())
            .create(CreateMissionInput {
                title: "知识导入任务".to_string(),
                goal: "验证 folder import".to_string(),
                constraints: vec![],
                success_criteria: vec!["文件导入完成".to_string()],
                priority: MissionPriority::Medium,
            })
            .expect("mission should create")
            .id
    }

    struct TestHttpServer {
        address: SocketAddr,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl TestHttpServer {
        fn serve(status_line: &str, content_type: &str, body: String) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
            let address = listener
                .local_addr()
                .expect("listener should have local addr");
            let response = format!(
                "HTTP/1.1 {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                status_line,
                content_type,
                body.len(),
                body
            );

            let handle = thread::spawn(move || {
                let (mut stream, _) = listener.accept().expect("server should accept");
                let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
                let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
                let mut request = [0_u8; 1024];
                let _ = stream.read(&mut request);
                let _ = stream.write_all(response.as_bytes());
            });

            Self {
                address,
                handle: Some(handle),
            }
        }

        fn url(&self, path: &str) -> String {
            format!("http://{}{}", self.address, path)
        }
    }

    impl Drop for TestHttpServer {
        fn drop(&mut self) {
            if let Some(handle) = self.handle.take()
                && handle.is_finished()
            {
                handle.join().expect("server thread should join");
            }
        }
    }

    #[test]
    fn file_import_reads_utf8_file_when_preview_is_missing() {
        let workspace = TempFileWorkspace::new();
        let path =
            workspace.write_text("brief.md", "客户背景：预算已确认。\n下一步：整理拜访材料。");

        let input = build_context_item_input(KnowledgeImportRequest {
            mission_id: "mission-001".to_string(),
            r#type: ContextItemType::File,
            title: "  客户简报  ".to_string(),
            content_preview: None,
            source_uri: Some(path_string(&path)),
        })
        .expect("file import should build context input");

        assert_eq!(input.mission_id, "mission-001");
        assert_eq!(input.title, "客户简报");
        assert_eq!(
            input.source_uri.as_deref(),
            Some(path_string(&path).as_str())
        );
        assert_eq!(
            input.content_preview.as_deref(),
            Some("客户背景：预算已确认。\n下一步：整理拜访材料。")
        );
    }

    #[test]
    fn url_import_rejects_non_http_source_uri() {
        let error = build_context_item_input(KnowledgeImportRequest {
            mission_id: "mission-001".to_string(),
            r#type: ContextItemType::Url,
            title: "无效链接".to_string(),
            content_preview: Some("待抓取".to_string()),
            source_uri: Some("ftp://example.com/brief".to_string()),
        })
        .expect_err("non-http url import should fail validation");

        assert_eq!(error.code, "validation_error");
        assert!(
            error.message.contains("http"),
            "unexpected error message: {}",
            error.message
        );
    }

    #[test]
    fn folder_import_persists_supported_files_and_skips_unsupported_entries() {
        let workspace = TempFileWorkspace::new();
        workspace.write_text("brief.md", "# 客户简报\n预算已确认");
        workspace.write_text("notes/timeline.txt", "周三演示\n周五复盘");
        workspace.write_text("data.csv", "company,stage\nAcme,proposal");
        workspace.write_text("ignore.png", "not a real image");
        let db = Database::in_memory().expect("database should initialize");
        let mission_id = create_test_mission(&db);

        let imported = knowledge_import_folder_for_db(
            &db,
            KnowledgeFolderImportRequest {
                mission_id: mission_id.clone(),
                folder_path: path_string(&workspace.root),
                title_prefix: Some("客户资料".to_string()),
                recursive: Some(true),
                max_files: Some(10),
            },
        )
        .expect("folder import should succeed");

        assert_eq!(imported.imported_count, 3);
        assert_eq!(imported.skipped_count, 1);
        assert_eq!(imported.items.len(), 3);
        assert!(imported.summary.contains("Imported 3"));
        assert!(
            imported
                .items
                .iter()
                .any(|item| item.title == "客户资料 / brief.md")
        );
        assert!(
            imported
                .items
                .iter()
                .any(|item| item.title == "客户资料 / notes/timeline.txt")
        );

        let service = MissionServiceImpl::new(db.clone());
        let sources = service
            .list_knowledge_sources(None)
            .expect("knowledge sources should list");
        assert_eq!(sources.len(), 3);
        assert!(sources.iter().all(|source| source.r#type == "file"));
    }

    #[test]
    fn folder_import_rejects_empty_directory() {
        let workspace = TempFileWorkspace::new();
        let empty_dir = workspace.create_dir("empty");
        let db = Database::in_memory().expect("database should initialize");
        let mission_id = create_test_mission(&db);

        let error = knowledge_import_folder_for_db(
            &db,
            KnowledgeFolderImportRequest {
                mission_id,
                folder_path: path_string(&empty_dir),
                title_prefix: None,
                recursive: Some(false),
                max_files: None,
            },
        )
        .expect_err("empty folder import should fail");

        assert_eq!(error.code, "validation_error");
        assert!(
            error.message.contains("empty"),
            "unexpected error message: {}",
            error.message
        );
    }

    #[test]
    fn folder_import_applies_max_files_limit_and_reports_remaining_supported_files_as_skipped() {
        let workspace = TempFileWorkspace::new();
        workspace.write_text("a.md", "A");
        workspace.write_text("b.md", "B");
        workspace.write_text("c.md", "C");
        let db = Database::in_memory().expect("database should initialize");
        let mission_id = create_test_mission(&db);

        let imported = knowledge_import_folder_for_db(
            &db,
            KnowledgeFolderImportRequest {
                mission_id,
                folder_path: path_string(&workspace.root),
                title_prefix: None,
                recursive: Some(false),
                max_files: Some(2),
            },
        )
        .expect("folder import should succeed");

        assert_eq!(imported.imported_count, 2);
        assert_eq!(imported.skipped_count, 1);
    }

    #[test]
    fn folder_import_rejects_http_folder_paths() {
        let db = Database::in_memory().expect("database should initialize");
        let mission_id = create_test_mission(&db);

        let error = knowledge_import_folder_for_db(
            &db,
            KnowledgeFolderImportRequest {
                mission_id,
                folder_path: "https://example.com/knowledge".to_string(),
                title_prefix: None,
                recursive: Some(false),
                max_files: Some(5),
            },
        )
        .expect_err("remote folder import should fail");

        assert_eq!(error.code, "validation_error");
        assert!(
            error.message.contains("local"),
            "unexpected error message: {}",
            error.message
        );
    }

    #[tokio::test]
    async fn url_preview_fetch_extracts_title_and_preview_from_html() {
        let body = r#"
            <html>
              <head>
                <title>  Example Brief  </title>
                <style>.hidden { display: none; }</style>
              </head>
              <body>
                <script>console.log("ignore me")</script>
                <h1>Customer Expansion</h1>
                <p>Budget confirmed &amp; rollout scheduled next week.</p>
              </body>
            </html>
        "#;
        let server = TestHttpServer::serve("200 OK", "text/html; charset=utf-8", body.to_string());

        let preview = fetch_url_preview(&server.url("/brief"))
            .await
            .expect("preview fetch should succeed");

        assert_eq!(preview.status, 200);
        assert_eq!(
            preview.content_type.as_deref(),
            Some("text/html; charset=utf-8")
        );
        assert_eq!(preview.title.as_deref(), Some("Example Brief"));
        assert!(preview.preview.contains("Customer Expansion"));
        assert!(
            preview
                .preview
                .contains("Budget confirmed & rollout scheduled next week.")
        );
        assert!(!preview.preview.contains("ignore me"));
        assert!(!preview.truncated);
        assert!(preview.fetched_at.contains('T'));
    }

    #[tokio::test]
    async fn url_preview_fetch_marks_truncation_when_response_exceeds_limit() {
        let oversized_text = "alpha ".repeat((MAX_URL_PREVIEW_BYTES / 6) + 1024);
        let body = format!(
            "<html><head><title>Large Page</title></head><body><p>{}</p></body></html>",
            oversized_text
        );
        let server = TestHttpServer::serve("200 OK", "text/html", body);

        let preview = fetch_url_preview(&server.url("/large"))
            .await
            .expect("preview fetch should succeed");

        assert!(preview.truncated);
        assert_eq!(preview.title.as_deref(), Some("Large Page"));
        assert!(preview.preview.chars().count() <= MAX_URL_PREVIEW_CHARS);
        assert!(!preview.preview.is_empty());
    }
}
