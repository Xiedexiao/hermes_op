//! Mission 命令

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::State;
use uuid::Uuid;

use crate::backend::AppError;
use crate::backend::{
    CreateExecutionStepInput, CreateMissionInput, Database, ExecutionMode, ExecutionRepository,
    ExecutionStep, ExecutionStepStatus, Mission, MissionDetail, MissionListFilter, MissionPriority,
    MissionRepository, MissionService, MissionServiceImpl, MissionStatus, RiskLevel, Run,
    RunStatus, RunType, UpdateMissionInput,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionListRequest {
    pub query: Option<String>,
    pub status: Option<MissionStatus>,
    pub limit: Option<usize>,
}

impl MissionListRequest {
    fn into_filter(self) -> MissionListFilter {
        MissionListFilter {
            query: self.query,
            status: self.status,
            limit: self.limit,
        }
        .normalized()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionCreateRequest {
    pub title: String,
    pub goal: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub success_criteria: Vec<String>,
    pub priority: MissionPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionUpdateRequest {
    pub id: String,
    pub title: String,
    pub goal: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub success_criteria: Vec<String>,
    pub priority: MissionPriority,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionPinnedRequest {
    pub id: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionStatusRequest {
    pub id: String,
    pub status: MissionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionGeneratePlanRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedMissionPlan {
    pub run: Run,
    #[serde(default)]
    pub steps: Vec<ExecutionStep>,
}

#[tauri::command]
pub fn mission_list(
    db: State<'_, Database>,
    request: Option<MissionListRequest>,
) -> Result<Vec<Mission>, AppError> {
    let service = MissionServiceImpl::new(db.inner().clone());
    let filter = request
        .map(MissionListRequest::into_filter)
        .unwrap_or_default();

    service.list(filter)
}

#[tauri::command]
pub fn mission_create(
    db: State<'_, Database>,
    request: MissionCreateRequest,
) -> Result<Mission, AppError> {
    let service = MissionServiceImpl::new(db.inner().clone());
    service.create(CreateMissionInput {
        title: request.title,
        goal: request.goal,
        constraints: request.constraints,
        success_criteria: request.success_criteria,
        priority: request.priority,
    })
}

#[tauri::command]
pub fn mission_update(
    db: State<'_, Database>,
    request: MissionUpdateRequest,
) -> Result<Mission, AppError> {
    let service = MissionServiceImpl::new(db.inner().clone());
    service.update(UpdateMissionInput {
        id: request.id,
        title: request.title,
        goal: request.goal,
        constraints: request.constraints,
        success_criteria: request.success_criteria,
        priority: request.priority,
    })
}

#[tauri::command]
pub fn mission_set_pinned(
    db: State<'_, Database>,
    request: MissionPinnedRequest,
) -> Result<Mission, AppError> {
    let service = MissionServiceImpl::new(db.inner().clone());
    service.set_pinned(&request.id, request.pinned)
}

#[tauri::command]
pub fn mission_set_status(
    db: State<'_, Database>,
    request: MissionStatusRequest,
) -> Result<Mission, AppError> {
    let service = MissionServiceImpl::new(db.inner().clone());
    service.set_status(&request.id, request.status)
}

#[tauri::command]
pub fn mission_generate_plan(
    db: State<'_, Database>,
    request: MissionGeneratePlanRequest,
) -> Result<GeneratedMissionPlan, AppError> {
    mission_generate_plan_for_db(db.inner(), &request.id)
}

#[tauri::command]
pub fn mission_get(db: State<'_, Database>, id: String) -> Result<Option<MissionDetail>, AppError> {
    let service = MissionServiceImpl::new(db.inner().clone());
    service.get(&id)
}

pub fn mission_generate_plan_for_db(
    db: &Database,
    mission_id: &str,
) -> Result<GeneratedMissionPlan, AppError> {
    let mission_id = mission_id.trim().to_string();
    if mission_id.is_empty() {
        return Err(AppError::validation("mission id cannot be empty"));
    }

    let mission_repo = MissionRepository::new(db.clone());
    let mission = mission_repo
        .get(&mission_id)?
        .ok_or_else(|| AppError::storage(format!("mission not found: {}", mission_id)))?;
    if mission.status == MissionStatus::Archived {
        return Err(AppError::validation(
            "archived mission cannot generate a plan",
        ));
    }

    let now = Utc::now().to_rfc3339();
    let run = Run {
        id: Uuid::new_v4().to_string(),
        mission_id: mission_id.clone(),
        r#type: RunType::Execution,
        status: RunStatus::Queued,
        started_at: Some(now.clone()),
        finished_at: None,
        summary: Some("Generated baseline execution plan".to_string()),
        error_message: None,
    };

    db.execute(
        "INSERT INTO runs (id, mission_id, type, status, started_at, finished_at, summary, error_message)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            &run.id as &dyn rusqlite::ToSql,
            &run.mission_id,
            &run.r#type.as_str(),
            &run.status.as_str(),
            &run.started_at,
            &run.finished_at,
            &run.summary,
            &run.error_message,
        ],
    )?;

    let execution_repo = ExecutionRepository::new(db.clone());
    let step_specs = [
        (
            "Review mission context",
            ExecutionMode::Api,
            RiskLevel::Low,
            ExecutionStepStatus::Pending,
            Some(
                serde_json::json!({
                    "mission_id": mission.id,
                    "goal": mission.goal,
                })
                .to_string(),
            ),
        ),
        (
            "Prepare execution plan",
            ExecutionMode::Cli,
            RiskLevel::Medium,
            ExecutionStepStatus::Pending,
            Some("{\"adapter\":\"cli\",\"action\":\"draft-plan\"}".to_string()),
        ),
        (
            "Request approval before external action",
            ExecutionMode::Cli,
            RiskLevel::High,
            ExecutionStepStatus::AwaitingApproval,
            Some("{\"requires_approval\":true}".to_string()),
        ),
    ];

    let mut steps = Vec::with_capacity(step_specs.len());
    for (title, mode, risk_level, status, input_payload) in step_specs {
        steps.push(execution_repo.create(
            CreateExecutionStepInput {
                mission_id: mission_id.clone(),
                run_id: run.id.clone(),
                title: title.to_string(),
                mode,
                risk_level,
                input_payload,
            },
            status,
        )?);
    }

    mission_repo.set_status(&mission_id, MissionStatus::AwaitingApproval)?;

    Ok(GeneratedMissionPlan { run, steps })
}

#[cfg(test)]
mod tests {
    use super::{MissionListRequest, mission_generate_plan_for_db};
    use crate::backend::{
        CreateMissionInput, Database, MissionPriority, MissionRepository, MissionStatus,
    };

    #[test]
    fn mission_list_request_trims_blank_query_into_empty_filter() {
        let filter = MissionListRequest {
            query: Some("   ".to_string()),
            status: Some(MissionStatus::Planning),
            limit: Some(20),
        }
        .into_filter();

        assert_eq!(filter.query, None);
        assert_eq!(filter.status, Some(MissionStatus::Planning));
        assert_eq!(filter.limit, Some(20));
    }

    #[test]
    fn generate_mission_plan_persists_run_steps_and_marks_mission_awaiting_approval() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(CreateMissionInput {
                title: "Prepare account plan".to_string(),
                goal: "Create a customer visit plan".to_string(),
                constraints: vec!["Do not send external messages".to_string()],
                success_criteria: vec!["Draft plan ready".to_string()],
                priority: MissionPriority::High,
            })
            .expect("mission should create");

        let generated =
            mission_generate_plan_for_db(&db, &mission.id).expect("plan should generate");

        assert_eq!(generated.run.mission_id, mission.id);
        assert_eq!(generated.run.r#type.as_str(), "execution");
        assert_eq!(generated.steps.len(), 3);
        assert_eq!(generated.steps[0].status.as_str(), "pending");
        serde_json::from_str::<serde_json::Value>(
            generated.steps[0]
                .input_payload
                .as_deref()
                .expect("first step should have payload"),
        )
        .expect("first step payload should be valid JSON");
        assert_eq!(generated.steps[1].risk_level.as_str(), "medium");
        assert_eq!(generated.steps[2].risk_level.as_str(), "high");
        assert_eq!(generated.steps[2].status.as_str(), "awaiting_approval");

        let updated = repo
            .get(&mission.id)
            .expect("mission lookup should work")
            .expect("mission should exist");
        assert_eq!(updated.status, MissionStatus::AwaitingApproval);
    }
}
