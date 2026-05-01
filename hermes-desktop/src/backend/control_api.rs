//! Local HTTP control API for programmatic desktop access.
//!
//! The server intentionally stays small and localhost-only. It exposes a
//! narrow JSON surface for health, runtime, recent sessions, mission list and
//! creation, plus parity catalog inspection.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};

use crate::backend::{
    AppError, AppState, CreateMissionInput, Database, Mission, MissionListFilter, MissionPriority,
    MissionService, MissionServiceImpl, MissionStatus, ParityCatalog, ParityService,
    ParityServiceImpl, Session, SessionMessage, SessionMessageHistoryQuery, SessionMessageRole,
    SessionService, SessionServiceImpl, hermes,
};
use crate::commands::{
    app::get_foreground_snapshot,
    sessions::{
        ActiveSessionSelection, SessionReplaySnapshot, SessionReplaySnapshotRequest,
        session_get_active_for_db, session_replay_snapshot_for_db,
    },
};

const DEFAULT_CONTROL_API_HOST: &str = "127.0.0.1";
const DEFAULT_CONTROL_API_PORT: u16 = 47_831;
const DEFAULT_SESSION_MESSAGE_LIMIT: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlRequest {
    pub method: String,
    pub path: String,
    pub query: BTreeMap<String, String>,
    pub body: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JsonResponse {
    pub status_code: u16,
    pub body: serde_json::Value,
}

pub trait ControlApiBackend: Send + Sync {
    fn health_payload(&self) -> Result<serde_json::Value, AppError>;
    fn runtime_payload(&self) -> Result<serde_json::Value, AppError>;
    fn recent_sessions(&self, limit: usize) -> Result<Vec<Session>, AppError>;
    fn active_session_handoff(&self) -> Result<Option<ActiveSessionSelection>, AppError>;
    fn latest_session(&self) -> Result<Option<Session>, AppError>;
    fn session_message_history(
        &self,
        session_id: String,
        limit: usize,
        role: Option<String>,
        query: Option<String>,
    ) -> Result<Vec<SessionMessage>, AppError>;
    fn session_replay_snapshot(
        &self,
        session_id: Option<String>,
        limit: usize,
    ) -> Result<SessionReplaySnapshot, AppError>;
    fn list_missions(
        &self,
        query: Option<String>,
        status: Option<MissionStatus>,
        limit: Option<usize>,
    ) -> Result<Vec<Mission>, AppError>;
    fn create_mission(&self, body: serde_json::Value) -> Result<Mission, AppError>;
    fn parity_catalog(&self) -> Result<ParityCatalog, AppError>;
}

#[derive(Debug, Clone)]
pub struct DesktopControlApiBackend {
    db: Database,
    app_state: Arc<RwLock<AppState>>,
    bind_host: String,
    bind_port: u16,
}

#[derive(Debug, Clone)]
pub struct ControlApiConfig {
    pub host: String,
    pub port: u16,
}

impl Default for ControlApiConfig {
    fn default() -> Self {
        let port = std::env::var("HERMES_CONTROL_API_PORT")
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(DEFAULT_CONTROL_API_PORT);
        Self {
            host: DEFAULT_CONTROL_API_HOST.to_string(),
            port,
        }
    }
}

#[derive(Debug)]
pub struct ControlApiServerHandle {
    host: String,
    port: u16,
    shutdown: Arc<AtomicBool>,
    join: Mutex<Option<thread::JoinHandle<()>>>,
}

impl ControlApiServerHandle {
    pub fn start(
        db: Database,
        app_state: Arc<RwLock<AppState>>,
        config: ControlApiConfig,
    ) -> Result<Self, AppError> {
        let host = config.host;
        let listener = TcpListener::bind((host.as_str(), config.port)).map_err(|err| {
            AppError::runtime(format!("Failed to bind control API listener: {}", err))
        })?;
        listener.set_nonblocking(true).map_err(|err| {
            AppError::runtime(format!("Failed to configure control API: {}", err))
        })?;

        let port = listener
            .local_addr()
            .map_err(|err| {
                AppError::runtime(format!("Failed to read control API address: {}", err))
            })?
            .port();

        let shutdown = Arc::new(AtomicBool::new(false));
        let join_shutdown = Arc::clone(&shutdown);
        let backend = DesktopControlApiBackend::new(db, app_state, host.clone(), port);

        let join = thread::Builder::new()
            .name("hermes-control-api".to_string())
            .spawn(move || control_api_loop(listener, join_shutdown, backend))
            .map_err(|err| {
                AppError::runtime(format!("Failed to spawn control API thread: {}", err))
            })?;

        Ok(Self {
            host,
            port,
            shutdown,
            join: Mutex::new(Some(join)),
        })
    }

    pub fn host(&self) -> &str {
        &self.host
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.lock().take() {
            let _ = join.join();
        }
    }
}

impl Drop for ControlApiServerHandle {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.get_mut().take() {
            let _ = join.join();
        }
    }
}

impl DesktopControlApiBackend {
    pub fn new(
        db: Database,
        app_state: Arc<RwLock<AppState>>,
        bind_host: String,
        bind_port: u16,
    ) -> Self {
        Self {
            db,
            app_state,
            bind_host,
            bind_port,
        }
    }
}

impl ControlApiBackend for DesktopControlApiBackend {
    fn health_payload(&self) -> Result<serde_json::Value, AppError> {
        Ok(serde_json::json!({
            "status": "ok",
            "bind": {
                "host": self.bind_host,
                "port": self.bind_port,
            }
        }))
    }

    fn runtime_payload(&self) -> Result<serde_json::Value, AppError> {
        let state = self.app_state.read();
        let hermes_status = hermes::get_status();
        let foreground_snapshot = get_foreground_snapshot(&self.db)?;
        Ok(serde_json::json!({
            "engine": {
                "running": state.engine_status.running,
                "profile": state.engine_status.profile,
                "pid": state.engine_status.pid,
                "last_error": state.engine_status.last_error,
            },
            "hermes": {
                "installed": hermes_status.installed,
                "running": hermes_status.running,
                "version": hermes_status.version,
                "pid": hermes_status.pid,
            },
            "foreground_snapshot": foreground_snapshot,
        }))
    }

    fn recent_sessions(&self, limit: usize) -> Result<Vec<Session>, AppError> {
        SessionServiceImpl::new(self.db.clone()).list_recent(limit)
    }

    fn active_session_handoff(&self) -> Result<Option<ActiveSessionSelection>, AppError> {
        session_get_active_for_db(&self.db)
    }

    fn latest_session(&self) -> Result<Option<Session>, AppError> {
        SessionServiceImpl::new(self.db.clone()).get_latest()
    }

    fn session_message_history(
        &self,
        session_id: String,
        limit: usize,
        role: Option<String>,
        query: Option<String>,
    ) -> Result<Vec<SessionMessage>, AppError> {
        SessionServiceImpl::new(self.db.clone()).list_message_history(SessionMessageHistoryQuery {
            session_id,
            limit,
            role: role.map(|value| SessionMessageRole::from_key(value.trim())),
            text_query: normalize_optional_filter(query),
        })
    }

    fn session_replay_snapshot(
        &self,
        session_id: Option<String>,
        limit: usize,
    ) -> Result<SessionReplaySnapshot, AppError> {
        session_replay_snapshot_for_db(
            &self.db,
            Some(SessionReplaySnapshotRequest {
                session_id,
                limit: Some(limit),
            }),
        )
    }

    fn list_missions(
        &self,
        query: Option<String>,
        status: Option<MissionStatus>,
        limit: Option<usize>,
    ) -> Result<Vec<Mission>, AppError> {
        MissionServiceImpl::new(self.db.clone()).list(
            MissionListFilter {
                query,
                status,
                limit,
            }
            .normalized(),
        )
    }

    fn create_mission(&self, body: serde_json::Value) -> Result<Mission, AppError> {
        let payload: ControlMissionCreatePayload =
            serde_json::from_value(body).map_err(AppError::from_json_error)?;
        MissionServiceImpl::new(self.db.clone()).create(CreateMissionInput {
            title: payload.title,
            goal: payload.goal,
            constraints: payload.constraints,
            success_criteria: payload.success_criteria,
            priority: payload.priority,
        })
    }

    fn parity_catalog(&self) -> Result<ParityCatalog, AppError> {
        ParityServiceImpl::new(self.db.clone()).get_catalog()
    }
}

#[derive(Debug, Clone, Deserialize)]
struct ControlMissionCreatePayload {
    title: String,
    goal: String,
    #[serde(default)]
    constraints: Vec<String>,
    #[serde(default)]
    success_criteria: Vec<String>,
    priority: MissionPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ControlSessionMessagesPayload {
    resolved_via: String,
    session_id: Option<String>,
    messages: Vec<SessionMessage>,
}

pub fn handle_request<B: ControlApiBackend>(
    backend: &B,
    request: ControlRequest,
) -> Result<JsonResponse, AppError> {
    match (request.method.as_str(), request.path.as_str()) {
        ("GET", "/api/control/health") => Ok(ok_response(200, backend.health_payload()?)),
        ("GET", "/api/control/runtime") => Ok(ok_response(200, backend.runtime_payload()?)),
        ("GET", "/api/control/sessions/recent") => {
            let limit = request
                .query
                .get("limit")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(20);
            Ok(ok_response(
                200,
                serde_json::to_value(backend.recent_sessions(limit)?)
                    .map_err(AppError::from_json_error)?,
            ))
        }
        ("GET", "/api/control/sessions/active") => Ok(ok_response(
            200,
            serde_json::to_value(backend.active_session_handoff()?)
                .map_err(AppError::from_json_error)?,
        )),
        ("GET", "/api/control/sessions/messages") => {
            let limit = request
                .query
                .get("limit")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(DEFAULT_SESSION_MESSAGE_LIMIT)
                .clamp(1, 200);
            let role = normalize_optional_filter(request.query.get("role").cloned());
            let query = normalize_optional_filter(request.query.get("query").cloned());
            let payload = if let Some(session_id) =
                normalize_optional_filter(request.query.get("session_id").cloned())
            {
                ControlSessionMessagesPayload {
                    resolved_via: "session_id".to_string(),
                    messages: backend.session_message_history(
                        session_id.clone(),
                        limit,
                        role,
                        query,
                    )?,
                    session_id: Some(session_id),
                }
            } else if let Some(active) = backend.active_session_handoff()? {
                let session_id = active.session.id;
                ControlSessionMessagesPayload {
                    resolved_via: "active_session".to_string(),
                    messages: backend.session_message_history(
                        session_id.clone(),
                        limit,
                        role,
                        query,
                    )?,
                    session_id: Some(session_id),
                }
            } else if let Some(latest) = backend.latest_session()? {
                let session_id = latest.id;
                ControlSessionMessagesPayload {
                    resolved_via: "latest_session".to_string(),
                    messages: backend.session_message_history(
                        session_id.clone(),
                        limit,
                        role,
                        query,
                    )?,
                    session_id: Some(session_id),
                }
            } else {
                ControlSessionMessagesPayload {
                    resolved_via: "none".to_string(),
                    session_id: None,
                    messages: Vec::new(),
                }
            };
            Ok(ok_response(
                200,
                serde_json::to_value(payload).map_err(AppError::from_json_error)?,
            ))
        }
        ("GET", "/api/control/sessions/replay") => {
            let limit = request
                .query
                .get("limit")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(DEFAULT_SESSION_MESSAGE_LIMIT)
                .clamp(1, 200);
            let session_id = normalize_optional_filter(request.query.get("session_id").cloned());
            Ok(ok_response(
                200,
                serde_json::to_value(backend.session_replay_snapshot(session_id, limit)?)
                    .map_err(AppError::from_json_error)?,
            ))
        }
        ("GET", "/api/control/missions") => {
            let query = request.query.get("query").cloned();
            let status = request
                .query
                .get("status")
                .map(|value| MissionStatus::from_key(value));
            let limit = request
                .query
                .get("limit")
                .and_then(|value| value.parse::<usize>().ok());
            Ok(ok_response(
                200,
                serde_json::to_value(backend.list_missions(query, status, limit)?)
                    .map_err(AppError::from_json_error)?,
            ))
        }
        ("POST", "/api/control/missions") => {
            let body = parse_json_body(&request.body)?;
            Ok(ok_response(
                201,
                serde_json::to_value(backend.create_mission(body)?)
                    .map_err(AppError::from_json_error)?,
            ))
        }
        ("GET", "/api/control/parity/catalog") => Ok(ok_response(
            200,
            serde_json::to_value(backend.parity_catalog()?).map_err(AppError::from_json_error)?,
        )),
        _ => Ok(error_response(
            404,
            "not_found",
            "Control API route not found",
        )),
    }
}

fn control_api_loop(
    listener: TcpListener,
    shutdown: Arc<AtomicBool>,
    backend: DesktopControlApiBackend,
) {
    while !shutdown.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((mut stream, _)) => {
                let response = match parse_http_request(&mut stream)
                    .and_then(|request| handle_request(&backend, request))
                {
                    Ok(response) => response,
                    Err(err) => error_response(500, "internal_error", &err.to_string()),
                };
                let _ = write_http_response(&mut stream, &response);
            }
            Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(err) => {
                tracing::error!("Control API accept failed: {}", err);
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

fn parse_http_request(stream: &mut TcpStream) -> Result<ControlRequest, AppError> {
    stream
        .set_read_timeout(Some(Duration::from_millis(500)))
        .map_err(|err| AppError::runtime(format!("Failed to set control API timeout: {}", err)))?;

    let mut buffer = Vec::new();
    let mut header_end = None;
    let mut chunk = [0_u8; 1024];

    while header_end.is_none() {
        let read = stream.read(&mut chunk).map_err(|err| {
            AppError::runtime(format!("Failed to read control API request: {}", err))
        })?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
        header_end = find_header_end(&buffer);
        if buffer.len() > 64 * 1024 {
            return Err(AppError::validation("control api request too large"));
        }
    }

    let header_end =
        header_end.ok_or_else(|| AppError::validation("invalid HTTP request: missing headers"))?;
    let (method, target, content_length) = {
        let header_text = std::str::from_utf8(&buffer[..header_end]).map_err(|err| {
            AppError::validation(format!("invalid HTTP header encoding: {}", err))
        })?;
        let mut lines = header_text.split("\r\n");
        let request_line = lines
            .next()
            .ok_or_else(|| AppError::validation("invalid HTTP request line"))?;
        let mut request_parts = request_line.split_whitespace();
        let method = request_parts
            .next()
            .ok_or_else(|| AppError::validation("missing HTTP method"))?
            .to_string();
        let target = request_parts
            .next()
            .ok_or_else(|| AppError::validation("missing HTTP path"))?
            .to_string();

        let content_length = lines
            .filter_map(|line| line.split_once(':'))
            .find_map(|(name, value)| {
                if name.trim().eq_ignore_ascii_case("content-length") {
                    value.trim().parse::<usize>().ok()
                } else {
                    None
                }
            })
            .unwrap_or(0);

        (method, target, content_length)
    };

    let body_start = header_end + 4;
    while buffer.len() < body_start + content_length {
        let read = stream.read(&mut chunk).map_err(|err| {
            AppError::runtime(format!("Failed to read control API body: {}", err))
        })?;
        if read == 0 {
            break;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }

    let body = buffer
        .get(body_start..body_start.saturating_add(content_length))
        .unwrap_or(&[])
        .to_vec();
    let (path, query) = split_path_and_query(&target);

    Ok(ControlRequest {
        method,
        path,
        query,
        body,
    })
}

fn find_header_end(buffer: &[u8]) -> Option<usize> {
    buffer.windows(4).position(|window| window == b"\r\n\r\n")
}

fn split_path_and_query(target: &str) -> (String, BTreeMap<String, String>) {
    if let Some((path, raw_query)) = target.split_once('?') {
        (path.to_string(), parse_query(raw_query))
    } else {
        (target.to_string(), BTreeMap::new())
    }
}

fn parse_query(raw_query: &str) -> BTreeMap<String, String> {
    let mut query = BTreeMap::new();
    for pair in raw_query.split('&') {
        if pair.is_empty() {
            continue;
        }
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        query.insert(key.to_string(), value.to_string());
    }
    query
}

fn normalize_optional_filter(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn parse_json_body(body: &[u8]) -> Result<serde_json::Value, AppError> {
    if body.is_empty() {
        return Err(AppError::validation("request body is required"));
    }

    serde_json::from_slice(body).map_err(AppError::from_json_error)
}

fn ok_response(status_code: u16, data: serde_json::Value) -> JsonResponse {
    JsonResponse {
        status_code,
        body: serde_json::json!({
            "ok": true,
            "data": data,
        }),
    }
}

fn error_response(status_code: u16, code: &str, message: &str) -> JsonResponse {
    JsonResponse {
        status_code,
        body: serde_json::json!({
            "ok": false,
            "error": {
                "code": code,
                "message": message,
            }
        }),
    }
}

fn write_http_response(stream: &mut TcpStream, response: &JsonResponse) -> Result<(), AppError> {
    let body = serde_json::to_vec(&response.body).map_err(AppError::from_json_error)?;
    let status_text = match response.status_code {
        200 => "OK",
        201 => "Created",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "OK",
    };
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status_code,
        status_text,
        body.len()
    );
    stream
        .write_all(header.as_bytes())
        .and_then(|_| stream.write_all(&body))
        .and_then(|_| stream.flush())
        .map_err(|err| AppError::runtime(format!("Failed to write control API response: {}", err)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{AgentEngineStatus, HermesStatus};
    use std::path::PathBuf;

    #[test]
    fn runtime_payload_includes_foreground_snapshot() {
        let db = Database::in_memory().expect("database should initialize");
        let value_json = serde_json::json!({
            "active": true,
            "state": "running",
            "session_id": "session-123",
            "run_id": "run-456",
            "cancel_state": "cancel_requested",
            "pending_count": 2,
            "interrupt_count": 1,
            "updated_at": "2026-04-24T00:00:00Z"
        })
        .to_string();
        let updated_at = "2026-04-24T00:00:00Z";
        let params: Vec<&dyn rusqlite::ToSql> =
            vec![&"cli_foreground_snapshot", &value_json, &updated_at];
        db.execute(
            "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?, ?, ?)",
            &params,
        )
        .expect("snapshot row should insert");

        let app_state = Arc::new(RwLock::new(AppState {
            config_dir: PathBuf::new(),
            db_path: PathBuf::new(),
            log_dir: PathBuf::new(),
            data_dir: PathBuf::new(),
            engine_status: AgentEngineStatus {
                running: true,
                pid: Some(42),
                profile: Some("default".to_string()),
                last_error: None,
            },
            hermes_status: HermesStatus::default(),
        }));
        let backend = DesktopControlApiBackend::new(db, app_state, "127.0.0.1".to_string(), 47831);

        let payload = backend
            .runtime_payload()
            .expect("runtime payload should build");

        assert_eq!(
            payload["foreground_snapshot"]["state"].as_str(),
            Some("running")
        );
        assert_eq!(
            payload["foreground_snapshot"]["session_id"].as_str(),
            Some("session-123")
        );
        assert_eq!(
            payload["foreground_snapshot"]["pending_count"].as_i64(),
            Some(2)
        );
        assert_eq!(
            payload["foreground_snapshot"]["interrupt_count"].as_i64(),
            Some(1)
        );
    }
}
