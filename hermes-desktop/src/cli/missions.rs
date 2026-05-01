use super::CliError;
use hermes_desktop::backend::{
    Database, Mission, MissionDetail, MissionListFilter, MissionService, MissionServiceImpl,
    MissionStatus, create_app_state,
};
use hermes_desktop::commands::mission::GeneratedMissionPlan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionListItem {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: String,
}

fn open_app_database() -> Result<Database, CliError> {
    let state = create_app_state().map_err(|err| CliError::Runtime(err.to_string()))?;
    let db_path = {
        let guard = state.read();
        guard.db_path.clone()
    };
    Database::new(&db_path).map_err(|err| CliError::Runtime(err.to_string()))
}

pub fn load_missions() -> Result<Vec<MissionListItem>, CliError> {
    let db = open_app_database()?;
    let service = MissionServiceImpl::new(db);
    let missions = service
        .list(MissionListFilter {
            query: None,
            status: None,
            limit: Some(50),
        })
        .map_err(|err| CliError::Runtime(err.to_string()))?;

    Ok(missions
        .into_iter()
        .map(|mission| MissionListItem {
            id: mission.id,
            title: mission.title,
            status: mission.status.as_str().to_string(),
            priority: mission.priority.as_str().to_string(),
        })
        .collect())
}

pub fn render_list(missions: &[MissionListItem]) -> String {
    if missions.is_empty() {
        return "no missions found\n".to_string();
    }

    let mut output = String::new();
    for mission in missions {
        output.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            mission.id, mission.status, mission.priority, mission.title
        ));
    }
    output
}

pub fn render_detail(detail: &MissionDetail) -> String {
    format!(
        "missions\tdetail\tid={}\tstatus={}\tpriority={}\tpinned={}\nmissions\tgoal\t{}\nmissions\tcounts\tcontext_items={}\truns={}\tartifacts={}\n",
        detail.mission.id,
        detail.mission.status.as_str(),
        detail.mission.priority.as_str(),
        detail.mission.pinned,
        detail.mission.goal,
        detail.context_items.len(),
        detail.runs.len(),
        detail.artifacts.len(),
    )
}

pub fn render_plan_summary(mission_id: &str, generated: &GeneratedMissionPlan) -> String {
    format!(
        "missions\tplan\tmission_id={}\trun_id={}\trun_status={}\tsteps={}\n",
        mission_id,
        generated.run.id,
        generated.run.status.as_str(),
        generated.steps.len(),
    )
}

pub fn render_status(mission: &Mission) -> String {
    format!(
        "missions\tstatus\tid={}\tstatus={}\n",
        mission.id,
        mission.status.as_str()
    )
}

pub fn load_mission_detail(id: &str) -> Result<MissionDetail, CliError> {
    let db = open_app_database()?;
    let service = MissionServiceImpl::new(db);
    load_mission_detail_with(id, |mission_id| {
        service
            .get(mission_id)
            .map_err(|err| CliError::Runtime(err.to_string()))
    })
}

fn load_mission_detail_with<F>(id: &str, getter: F) -> Result<MissionDetail, CliError>
where
    F: FnOnce(&str) -> Result<Option<MissionDetail>, CliError>,
{
    let mission_id = id.trim();
    if mission_id.is_empty() {
        return Err(CliError::InvalidUsage(
            "mission id cannot be empty\n".to_string(),
        ));
    }

    getter(mission_id)?
        .ok_or_else(|| CliError::Runtime(format!("mission not found: {}", mission_id)))
}

pub fn update_status(id: &str, status: MissionStatus) -> Result<Mission, CliError> {
    let db = open_app_database()?;
    let service = MissionServiceImpl::new(db);
    update_status_with(id, status, |mission_id, next_status| {
        service
            .set_status(mission_id, next_status)
            .map_err(|err| CliError::Runtime(err.to_string()))
    })
}

fn update_status_with<F>(id: &str, status: MissionStatus, setter: F) -> Result<Mission, CliError>
where
    F: FnOnce(&str, MissionStatus) -> Result<Mission, CliError>,
{
    let mission_id = id.trim();
    if mission_id.is_empty() {
        return Err(CliError::InvalidUsage(
            "mission id cannot be empty\n".to_string(),
        ));
    }

    setter(mission_id, status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hermes_desktop::backend::{
        Artifact, ArtifactType, ExecutionMode, ExecutionStep, ExecutionStepStatus, MissionPriority,
        Run, RunStatus, RunType,
    };

    fn sample_mission(status: MissionStatus) -> Mission {
        Mission {
            id: "mission-001".to_string(),
            title: "Bootstrap Hermes parity".to_string(),
            goal: "Ship reusable mission helpers".to_string(),
            constraints: vec!["stay local".to_string()],
            success_criteria: vec!["cover helpers with tests".to_string()],
            status,
            priority: MissionPriority::Medium,
            pinned: true,
            created_at: "2026-04-24T10:00:00Z".to_string(),
            updated_at: "2026-04-24T10:00:00Z".to_string(),
            last_activity_at: "2026-04-24T10:00:00Z".to_string(),
        }
    }

    fn sample_detail(status: MissionStatus) -> MissionDetail {
        MissionDetail {
            mission: sample_mission(status),
            context_items: vec![
                hermes_desktop::backend::MissionContextItem {
                    id: "context-001".to_string(),
                    mission_id: "mission-001".to_string(),
                    r#type: hermes_desktop::backend::ContextItemType::Note,
                    title: "Context".to_string(),
                    content_preview: Some("preview".to_string()),
                    source_uri: None,
                    pinned: false,
                    created_at: "2026-04-24T10:00:00Z".to_string(),
                },
                hermes_desktop::backend::MissionContextItem {
                    id: "context-002".to_string(),
                    mission_id: "mission-001".to_string(),
                    r#type: hermes_desktop::backend::ContextItemType::Artifact,
                    title: "Artifact link".to_string(),
                    content_preview: None,
                    source_uri: Some("/tmp/plan.md".to_string()),
                    pinned: false,
                    created_at: "2026-04-24T10:01:00Z".to_string(),
                },
            ],
            runs: vec![Run {
                id: "run-001".to_string(),
                mission_id: "mission-001".to_string(),
                r#type: RunType::Execution,
                status: RunStatus::Queued,
                started_at: Some("2026-04-24T10:02:00Z".to_string()),
                finished_at: None,
                summary: Some("Queued baseline plan".to_string()),
                error_message: None,
            }],
            artifacts: vec![Artifact {
                id: "artifact-001".to_string(),
                mission_id: "mission-001".to_string(),
                run_id: Some("run-001".to_string()),
                r#type: ArtifactType::Plan,
                title: "Mission plan".to_string(),
                path: "/tmp/plan.md".to_string(),
                mime_type: Some("text/markdown".to_string()),
                created_at: "2026-04-24T10:03:00Z".to_string(),
            }],
        }
    }

    fn sample_plan() -> hermes_desktop::commands::mission::GeneratedMissionPlan {
        hermes_desktop::commands::mission::GeneratedMissionPlan {
            run: Run {
                id: "run-001".to_string(),
                mission_id: "mission-001".to_string(),
                r#type: RunType::Execution,
                status: RunStatus::Queued,
                started_at: Some("2026-04-24T10:02:00Z".to_string()),
                finished_at: None,
                summary: Some("Queued baseline plan".to_string()),
                error_message: None,
            },
            steps: vec![ExecutionStep {
                id: "step-001".to_string(),
                mission_id: "mission-001".to_string(),
                run_id: "run-001".to_string(),
                title: "Review mission context".to_string(),
                mode: ExecutionMode::Cli,
                risk_level: hermes_desktop::backend::RiskLevel::Low,
                status: ExecutionStepStatus::Pending,
                input_payload: None,
                output_summary: None,
                created_at: "2026-04-24T10:02:00Z".to_string(),
                updated_at: "2026-04-24T10:02:00Z".to_string(),
            }],
        }
    }

    #[test]
    fn renders_empty_mission_list() {
        assert_eq!(render_list(&[]), "no missions found\n");
    }

    #[test]
    fn renders_mission_rows() {
        let rendered = render_list(&[MissionListItem {
            id: "mission-001".to_string(),
            title: "Bootstrap Hermes parity".to_string(),
            status: "planning".to_string(),
            priority: "medium".to_string(),
        }]);

        assert_eq!(
            rendered,
            "mission-001\tplanning\tmedium\tBootstrap Hermes parity\n"
        );
    }

    #[test]
    fn renders_mission_detail_summary() {
        assert_eq!(
            render_detail(&sample_detail(MissionStatus::Planning)),
            concat!(
                "missions\tdetail\tid=mission-001\tstatus=planning\tpriority=medium\tpinned=true\n",
                "missions\tgoal\tShip reusable mission helpers\n",
                "missions\tcounts\tcontext_items=2\truns=1\tartifacts=1\n",
            )
        );
    }

    #[test]
    fn renders_mission_plan_summary() {
        assert_eq!(
            render_plan_summary("mission-001", &sample_plan()),
            "missions\tplan\tmission_id=mission-001\trun_id=run-001\trun_status=queued\tsteps=1\n"
        );
    }

    #[test]
    fn loads_mission_detail_or_returns_not_found_error() {
        let detail = load_mission_detail_with("mission-001", |mission_id| {
            assert_eq!(mission_id, "mission-001");
            Ok(Some(sample_detail(MissionStatus::Planning)))
        })
        .expect("detail should load");
        assert_eq!(detail.mission.id, "mission-001");

        let error = load_mission_detail_with("missing", |_| Ok(None)).expect_err("missing mission");
        assert_eq!(error.to_string(), "mission not found: missing");
    }

    #[test]
    fn updates_status_and_renders_status_line() {
        let updated = update_status_with(
            "mission-001",
            MissionStatus::Paused,
            |mission_id, status| {
                assert_eq!(mission_id, "mission-001");
                assert_eq!(status, MissionStatus::Paused);
                Ok(sample_mission(MissionStatus::Paused))
            },
        )
        .expect("status should update");

        assert_eq!(updated.status, MissionStatus::Paused);
        assert_eq!(
            render_status(&updated),
            "missions\tstatus\tid=mission-001\tstatus=paused\n"
        );
    }
}
