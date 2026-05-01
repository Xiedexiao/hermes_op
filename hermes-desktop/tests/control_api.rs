use hermes_desktop::backend::control_api::{ControlApiBackend, ControlRequest, handle_request};
use hermes_desktop::backend::{
    AppError, Mission, MissionPriority, MissionStatus, ParityCatalog, ParityProviderCatalog,
    Session, SessionMessage, SessionMessageRole, SessionSource,
};
use hermes_desktop::commands::sessions::{ActiveSessionSelection, SessionReplaySnapshot};
use std::collections::BTreeMap;

#[derive(Default)]
struct StubBackend {
    sessions: Vec<Session>,
    latest_session: Option<Session>,
    active_selection: Option<ActiveSessionSelection>,
    message_history: Vec<SessionMessage>,
    replay_snapshot: Option<SessionReplaySnapshot>,
    missions: Vec<Mission>,
    created_mission: Option<Mission>,
    catalog: Option<ParityCatalog>,
}

impl ControlApiBackend for StubBackend {
    fn health_payload(&self) -> Result<serde_json::Value, AppError> {
        Ok(serde_json::json!({
            "status": "ok",
            "bind": { "host": "127.0.0.1", "port": 47831 }
        }))
    }

    fn runtime_payload(&self) -> Result<serde_json::Value, AppError> {
        Ok(serde_json::json!({
            "engine": {
                "running": true,
                "profile": "default",
                "pid": 42
            },
            "hermes": {
                "installed": true,
                "running": false,
                "version": "0.1.0"
            }
        }))
    }

    fn recent_sessions(&self, _limit: usize) -> Result<Vec<Session>, AppError> {
        Ok(self.sessions.clone())
    }

    fn active_session_handoff(&self) -> Result<Option<ActiveSessionSelection>, AppError> {
        Ok(self.active_selection.clone())
    }

    fn latest_session(&self) -> Result<Option<Session>, AppError> {
        Ok(self.latest_session.clone())
    }

    fn session_message_history(
        &self,
        _session_id: String,
        _limit: usize,
        _role: Option<String>,
        _query: Option<String>,
    ) -> Result<Vec<SessionMessage>, AppError> {
        Ok(self.message_history.clone())
    }

    fn session_replay_snapshot(
        &self,
        _session_id: Option<String>,
        _limit: usize,
    ) -> Result<SessionReplaySnapshot, AppError> {
        self.replay_snapshot
            .clone()
            .ok_or_else(|| AppError::validation("expected replay snapshot"))
    }

    fn list_missions(
        &self,
        _query: Option<String>,
        _status: Option<MissionStatus>,
        _limit: Option<usize>,
    ) -> Result<Vec<Mission>, AppError> {
        Ok(self.missions.clone())
    }

    fn create_mission(&self, _body: serde_json::Value) -> Result<Mission, AppError> {
        self.created_mission
            .clone()
            .ok_or_else(|| AppError::validation("expected created mission"))
    }

    fn parity_catalog(&self) -> Result<ParityCatalog, AppError> {
        self.catalog
            .clone()
            .ok_or_else(|| AppError::validation("expected parity catalog"))
    }
}

fn request(method: &str, path: &str) -> ControlRequest {
    ControlRequest {
        method: method.to_string(),
        path: path.to_string(),
        query: Default::default(),
        body: Vec::new(),
    }
}

fn request_with_query(method: &str, path: &str, query: &[(&str, &str)]) -> ControlRequest {
    ControlRequest {
        method: method.to_string(),
        path: path.to_string(),
        query: query
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect::<BTreeMap<_, _>>(),
        body: Vec::new(),
    }
}

fn sample_session(id: &str, title: &str) -> Session {
    Session {
        id: id.to_string(),
        source: SessionSource::Desktop,
        title: title.to_string(),
        model_name: Some("gpt-5.4".to_string()),
        parent_session_id: None,
        started_at: "2026-04-22T10:00:00Z".to_string(),
        updated_at: "2026-04-22T10:00:00Z".to_string(),
        ended_at: None,
    }
}

fn sample_active_selection(
    session_id: &str,
    title: &str,
    reason: &str,
    activated_at: &str,
) -> ActiveSessionSelection {
    ActiveSessionSelection {
        session: sample_session(session_id, title),
        reason: reason.to_string(),
        activated_at: activated_at.to_string(),
    }
}

fn sample_message(
    id: &str,
    session_id: &str,
    role: SessionMessageRole,
    content: &str,
) -> SessionMessage {
    SessionMessage {
        id: id.to_string(),
        session_id: session_id.to_string(),
        role,
        content: content.to_string(),
        source: "local".to_string(),
        created_at: "2026-04-22T10:05:00Z".to_string(),
    }
}

fn sample_mission(id: &str, title: &str) -> Mission {
    Mission {
        id: id.to_string(),
        title: title.to_string(),
        goal: format!("Goal for {title}"),
        constraints: vec!["stay local".to_string()],
        success_criteria: vec!["return json".to_string()],
        status: MissionStatus::Draft,
        priority: MissionPriority::Medium,
        pinned: false,
        created_at: "2026-04-22T10:00:00Z".to_string(),
        updated_at: "2026-04-22T10:00:00Z".to_string(),
        last_activity_at: "2026-04-22T10:00:00Z".to_string(),
    }
}

#[test]
fn health_route_returns_ok_payload() {
    let backend = StubBackend::default();

    let response = handle_request(&backend, request("GET", "/api/control/health"))
        .expect("health route should succeed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body["ok"], true);
    assert_eq!(response.body["data"]["status"], "ok");
}

#[test]
fn recent_sessions_route_returns_sessions() {
    let backend = StubBackend {
        sessions: vec![sample_session("session-1", "Desktop Session")],
        ..Default::default()
    };

    let response = handle_request(&backend, request("GET", "/api/control/sessions/recent"))
        .expect("session route should succeed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body["data"][0]["id"], "session-1");
}

#[test]
fn active_session_route_returns_current_handoff() {
    let backend = StubBackend {
        active_selection: Some(sample_active_selection(
            "session-active",
            "Continue Handoff",
            "manual_resume",
            "2026-04-24T09:00:00Z",
        )),
        ..Default::default()
    };

    let response = handle_request(&backend, request("GET", "/api/control/sessions/active"))
        .expect("active session route should succeed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body["data"]["session"]["id"], "session-active");
    assert_eq!(response.body["data"]["reason"], "manual_resume");
}

#[test]
fn session_messages_route_returns_history_for_requested_session() {
    let backend = StubBackend {
        message_history: vec![sample_message(
            "msg-1",
            "session-7",
            SessionMessageRole::Assistant,
            "Resuming the blocked follow-up",
        )],
        ..Default::default()
    };

    let response = handle_request(
        &backend,
        request_with_query(
            "GET",
            "/api/control/sessions/messages",
            &[("session_id", "session-7"), ("limit", "10")],
        ),
    )
    .expect("session messages route should succeed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body["data"]["resolved_via"], "session_id");
    assert_eq!(response.body["data"]["session_id"], "session-7");
    assert_eq!(response.body["data"]["messages"][0]["id"], "msg-1");
}

#[test]
fn session_messages_route_falls_back_to_active_session() {
    let backend = StubBackend {
        active_selection: Some(sample_active_selection(
            "session-active",
            "Pinned Handoff",
            "continue_latest",
            "2026-04-24T09:30:00Z",
        )),
        message_history: vec![sample_message(
            "msg-active",
            "session-active",
            SessionMessageRole::User,
            "Pick up where we left off",
        )],
        ..Default::default()
    };

    let response = handle_request(&backend, request("GET", "/api/control/sessions/messages"))
        .expect("session messages route should resolve active session");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body["data"]["resolved_via"], "active_session");
    assert_eq!(response.body["data"]["session_id"], "session-active");
    assert_eq!(response.body["data"]["messages"][0]["id"], "msg-active");
}

#[test]
fn session_replay_route_returns_snapshot_for_requested_session() {
    let session = sample_session("session-22", "Replay Target");
    let backend = StubBackend {
        replay_snapshot: Some(SessionReplaySnapshot {
            resolved_via: "session_id".to_string(),
            session: Some(session.clone()),
            active_selection: None,
            messages: vec![sample_message(
                "msg-replay",
                "session-22",
                SessionMessageRole::Assistant,
                "Replay this thread",
            )],
        }),
        ..Default::default()
    };

    let response = handle_request(
        &backend,
        request_with_query(
            "GET",
            "/api/control/sessions/replay",
            &[("session_id", "session-22"), ("limit", "5")],
        ),
    )
    .expect("session replay route should succeed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body["data"]["resolved_via"], "session_id");
    assert_eq!(response.body["data"]["session"]["id"], "session-22");
    assert_eq!(response.body["data"]["messages"][0]["id"], "msg-replay");
}

#[test]
fn mission_list_route_returns_missions() {
    let backend = StubBackend {
        missions: vec![sample_mission("mission-1", "Local API")],
        ..Default::default()
    };

    let response = handle_request(&backend, request("GET", "/api/control/missions"))
        .expect("mission list should succeed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body["data"][0]["id"], "mission-1");
}

#[test]
fn mission_create_route_accepts_json_body() {
    let created_mission = sample_mission("mission-created", "Created Mission");
    let backend = StubBackend {
        created_mission: Some(created_mission.clone()),
        ..Default::default()
    };
    let mut request = request("POST", "/api/control/missions");
    request.body = serde_json::json!({
        "title": "Created Mission",
        "goal": "Expose a local API",
        "constraints": ["localhost only"],
        "success_criteria": ["return JSON"],
        "priority": "medium"
    })
    .to_string()
    .into_bytes();

    let response = handle_request(&backend, request).expect("mission create should succeed");

    assert_eq!(response.status_code, 201);
    assert_eq!(response.body["data"]["id"], created_mission.id);
}

#[test]
fn parity_catalog_route_returns_catalog() {
    let backend = StubBackend {
        catalog: Some(ParityCatalog {
            providers: vec![ParityProviderCatalog {
                id: "openai".to_string(),
                display_name: "OpenAI".to_string(),
                supports_custom_endpoint: true,
                models: vec![],
            }],
            active_provider: "openai".to_string(),
            active_model: "gpt-4o".to_string(),
            tool_visibility_options: vec![],
            cron_status_options: vec![],
            mcp_filter_modes: vec![],
        }),
        ..Default::default()
    };

    let response = handle_request(&backend, request("GET", "/api/control/parity/catalog"))
        .expect("catalog route should succeed");

    assert_eq!(response.status_code, 200);
    assert_eq!(response.body["data"]["active_provider"], "openai");
}

#[test]
fn unsupported_route_returns_not_found() {
    let backend = StubBackend::default();

    let response = handle_request(&backend, request("DELETE", "/api/control/missions"))
        .expect("router should return not found response");

    assert_eq!(response.status_code, 404);
    assert_eq!(response.body["ok"], false);
}
