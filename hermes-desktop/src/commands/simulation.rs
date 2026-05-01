//! Simulation 命令

use chrono::Utc;
use rusqlite::params;
use serde::{Deserialize, Serialize};
use std::cmp::{Ordering, Reverse};
use std::collections::BTreeMap;
use tauri::State;
use uuid::Uuid;

use crate::backend::{
    AppError, AppResult, Database, MissionListFilter, MissionPriority, MissionRepository,
    MissionStatus, RunStatus, RunType,
};

const RECENT_RUN_LIMIT: usize = 12;
const OVERVIEW_MISSION_LIMIT: usize = 10_000;
const HANDOFF_POLICY_TEMPLATES_KEY: &str = "simulation.handoff_policy_templates";
const SCORING_FORMULA_TEMPLATES_KEY: &str = "simulation.scoring_formula_templates";
const TEMPLATE_BUNDLE_AUDIT_LOG_KEY: &str = "simulation.template_bundle_audit_log";
const TEMPLATE_BUNDLE_AUDIT_LOG_LIMIT: usize = 50;
const TEMPLATE_BUNDLE_AUDIT_LOG_EXPORT_DEFAULT_LIMIT: usize = 20;
const TEMPLATE_BUNDLE_AUDIT_LOG_EXPORT_MAX_LIMIT: usize = TEMPLATE_BUNDLE_AUDIT_LOG_LIMIT;
const LOCAL_SANDBOX_ENGINE_NAME: &str = "local_deterministic_multi_agent_sandbox";
const LOCAL_SANDBOX_DEFAULT_ROUNDS: usize = 3;
const LOCAL_SANDBOX_MAX_ROUNDS: usize = 12;
const LOCAL_SANDBOX_HISTORY_DEFAULT_LIMIT: usize = 10;
const LOCAL_SANDBOX_HISTORY_MAX_LIMIT: usize = 50;
const HIGH_FIDELITY_SANDBOX_ENGINE_NAME: &str = "local_high_fidelity_world_sandbox";
const EXTERNAL_SAAS_SIMULATION_ENGINE_NAME: &str = "external_saas_provider_adapter";
const EXTERNAL_SAAS_CONFIRM_PHRASE: &str = "RUN EXTERNAL SAAS SIMULATION";
const EXTERNAL_SAAS_DEFAULT_TIMEOUT_MS: u64 = 5_000;
const EXTERNAL_SAAS_MAX_TIMEOUT_MS: u64 = 30_000;
const EXTERNAL_SAAS_MAX_RESPONSE_CHARS: usize = 12_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationCreateScenarioRunRequest {
    pub mission_id: String,
    pub baseline: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub option_cards: Vec<ScenarioOptionCard>,
    #[serde(default)]
    pub variables: Vec<ScenarioVariable>,
    pub recommendation: Option<String>,
    pub recommendation_reason: Option<String>,
    pub comparison_summary: Option<String>,
    pub selected_option_id: Option<String>,
    pub handoff_target: Option<String>,
    pub execution_risk_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioOptionCard {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub assumptions: Vec<String>,
    #[serde(default)]
    pub expected_benefits: Vec<String>,
    #[serde(default)]
    pub risks: Vec<String>,
    #[serde(default)]
    pub projected_outcomes: Vec<String>,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub time_horizon: String,
    pub confidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScenarioVariable {
    pub id: String,
    pub label: String,
    pub current_value: String,
    pub proposed_value: String,
    pub impact: String,
    pub uncertainty: String,
    #[serde(default)]
    pub impact_weight: f64,
    #[serde(default)]
    pub uncertainty_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationScenarioRun {
    pub id: String,
    pub mission_id: String,
    pub mission_title: String,
    pub baseline: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub option_cards: Vec<ScenarioOptionCard>,
    #[serde(default)]
    pub variables: Vec<ScenarioVariable>,
    pub recommendation: Option<String>,
    pub recommendation_reason: Option<String>,
    pub comparison_summary: Option<String>,
    pub selected_option_id: Option<String>,
    pub handoff_target: String,
    pub execution_risk_level: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationScenarioRunListRequest {
    pub mission_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationHandoffPolicyTemplate {
    pub id: String,
    pub name: String,
    pub handoff_target: String,
    pub execution_risk_level: String,
    pub description: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationSaveHandoffPolicyTemplateRequest {
    pub id: Option<String>,
    pub name: String,
    pub handoff_target: String,
    pub execution_risk_level: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationScoringFormulaTemplate {
    pub id: String,
    pub name: String,
    pub base_score: f64,
    pub impact_multiplier: f64,
    pub uncertainty_penalty: f64,
    pub description: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationSaveScoringFormulaTemplateRequest {
    pub id: Option<String>,
    pub name: String,
    pub base_score: f64,
    pub impact_multiplier: f64,
    pub uncertainty_penalty: f64,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationTemplateBundle {
    pub schema_version: u32,
    pub exported_at: String,
    #[serde(default)]
    pub handoff_policy_templates: Vec<SimulationHandoffPolicyTemplate>,
    #[serde(default)]
    pub scoring_formula_templates: Vec<SimulationScoringFormulaTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationImportTemplateBundleRequest {
    pub bundle_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationImportTemplateBundleResponse {
    pub imported_handoff_policy_templates: usize,
    pub imported_scoring_formula_templates: usize,
    #[serde(default)]
    pub handoff_policy_templates: Vec<SimulationHandoffPolicyTemplate>,
    #[serde(default)]
    pub scoring_formula_templates: Vec<SimulationScoringFormulaTemplate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationTemplateBundleAuditEntry {
    pub id: String,
    pub action: String,
    pub actor: String,
    pub handoff_policy_template_count: usize,
    pub scoring_formula_template_count: usize,
    pub note: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationExportTemplateBundleAuditLogRequest {
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationExportTemplateBundleAuditLogResponse {
    pub total: usize,
    pub exported_count: usize,
    #[serde(default)]
    pub events: Vec<SimulationTemplateBundleAuditEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationTemplateBundlePreflightSection {
    pub create_count: usize,
    pub update_count: usize,
    pub unchanged_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationTemplateBundleConflict {
    pub id: String,
    pub template_type: String,
    pub existing_name: String,
    pub incoming_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationTemplateBundlePreflightResponse {
    pub schema_version: u32,
    pub total_count: usize,
    pub handoff_policy_templates: SimulationTemplateBundlePreflightSection,
    pub scoring_formula_templates: SimulationTemplateBundlePreflightSection,
    #[serde(default)]
    pub conflicts: Vec<SimulationTemplateBundleConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationComparisonRequest {
    pub mission_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationRunLocalSandboxRequest {
    pub mission_id: String,
    pub baseline: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub agents: Vec<SimulationSandboxAgentRequest>,
    pub rounds: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationLocalSandboxRunListRequest {
    pub mission_id: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationCapabilityRunListRequest {
    pub mission_id: Option<String>,
    pub limit: Option<usize>,
    pub target_remote_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationSandboxAgentRequest {
    pub name: String,
    pub role: String,
    pub stance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationLocalSandboxAgent {
    pub name: String,
    pub role: String,
    pub stance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationLocalSandboxTurn {
    pub round: usize,
    pub option: String,
    pub agent_name: String,
    pub agent_role: String,
    pub agent_stance: String,
    pub score: f64,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationLocalSandboxOptionScore {
    pub option: String,
    pub average_score: f64,
    pub total_score: f64,
    pub turn_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationLocalSandboxRecommendation {
    pub option: String,
    pub average_score: f64,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationLocalSandboxRun {
    pub run_id: String,
    pub mission_id: String,
    pub engine: String,
    pub rounds: usize,
    #[serde(default)]
    pub agents: Vec<SimulationLocalSandboxAgent>,
    #[serde(default)]
    pub turns: Vec<SimulationLocalSandboxTurn>,
    #[serde(default)]
    pub option_scores: Vec<SimulationLocalSandboxOptionScore>,
    pub recommendation: SimulationLocalSandboxRecommendation,
    pub audit_event_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationRunExternalSaasRequest {
    pub mission_id: String,
    pub provider: String,
    pub endpoint_url: Option<String>,
    pub input_json: Option<String>,
    pub target_remote_user_id: Option<String>,
    pub dry_run: Option<bool>,
    pub confirmation_phrase: Option<String>,
    pub timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationExternalSaasRun {
    pub run_id: String,
    pub mission_id: String,
    pub engine: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    pub provider: String,
    pub endpoint_url: Option<String>,
    #[serde(default)]
    pub target_remote_user_id: Option<String>,
    pub dry_run: bool,
    pub executed: bool,
    pub network_invocation: bool,
    pub request_preview: String,
    pub response_status: Option<u16>,
    pub response_body: String,
    pub summary: String,
    pub audit_event_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationRunHighFidelitySandboxRequest {
    pub mission_id: String,
    pub baseline: String,
    #[serde(default)]
    pub options: Vec<String>,
    #[serde(default)]
    pub agents: Vec<SimulationSandboxAgentRequest>,
    pub rounds: Option<usize>,
    #[serde(default)]
    pub variables: Vec<ScenarioVariable>,
    pub target_remote_user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationHighFidelitySandboxRun {
    pub run_id: String,
    pub mission_id: String,
    pub engine: String,
    #[serde(default)]
    pub created_at: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub target_remote_user_id: Option<String>,
    pub base_run: SimulationLocalSandboxRun,
    pub world: SimulationHighFidelityWorld,
    pub summary: String,
    pub audit_event_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationHighFidelityWorld {
    #[serde(default)]
    pub entities: Vec<SimulationHighFidelityEntity>,
    #[serde(default)]
    pub variables: Vec<SimulationHighFidelityVariable>,
    #[serde(default)]
    pub timeline: Vec<SimulationHighFidelityTimelineEvent>,
    pub event_graph: SimulationHighFidelityEventGraph,
    #[serde(default)]
    pub option_metric_heatmap: Vec<SimulationHighFidelityMetricCell>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationHighFidelityEntity {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub state: String,
    pub risk_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationHighFidelityVariable {
    pub id: String,
    pub label: String,
    pub current_value: String,
    pub proposed_value: String,
    pub impact: String,
    pub uncertainty: String,
    pub pressure_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationHighFidelityTimelineEvent {
    pub tick: usize,
    pub round: usize,
    pub actor: String,
    pub option: String,
    pub score: f64,
    pub score_delta: f64,
    #[serde(default)]
    pub state_changes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationHighFidelityEventGraph {
    #[serde(default)]
    pub nodes: Vec<SimulationHighFidelityGraphNode>,
    #[serde(default)]
    pub edges: Vec<SimulationHighFidelityGraphEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationHighFidelityGraphNode {
    pub id: String,
    pub label: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationHighFidelityGraphEdge {
    pub from: String,
    pub to: String,
    pub label: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationHighFidelityMetricCell {
    pub option: String,
    pub metric: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationComparisonMatrix {
    pub mission_id: String,
    pub mission_title: String,
    pub scenario_count: usize,
    pub scenarios: Vec<SimulationComparisonScenario>,
    pub variable_axes: Vec<SimulationVariableAxis>,
    pub option_patterns: Vec<SimulationOptionPattern>,
    pub path_evolution: Vec<SimulationPathEvolutionStep>,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationComparisonScenario {
    pub scenario_run_id: String,
    pub created_at: String,
    pub selected_option_id: Option<String>,
    pub selected_option_label: String,
    pub recommendation: Option<String>,
    pub comparison_summary: String,
    pub average_option_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationVariableAxis {
    pub label: String,
    pub values: Vec<String>,
    pub impacts: Vec<String>,
    pub uncertainties: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationOptionPattern {
    pub label: String,
    pub appearance_count: usize,
    pub selected_count: usize,
    pub average_score: f64,
    pub latest_time_horizon: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SimulationPathEvolutionStep {
    pub scenario_run_id: String,
    pub created_at: String,
    pub selected_option_label: String,
    pub score: f64,
    pub variable_changes: Vec<String>,
    pub narrative: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct ScenarioRunPayload {
    #[serde(default)]
    option_cards: Vec<ScenarioOptionCard>,
    #[serde(default)]
    variables: Vec<ScenarioVariable>,
    pub recommendation_reason: Option<String>,
    pub comparison_summary: Option<String>,
    pub selected_option_id: Option<String>,
    pub handoff_target: Option<String>,
    pub execution_risk_level: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationOverview {
    pub summary: SimulationOverviewSummary,
    #[serde(default)]
    pub counts_by_type: Vec<SimulationCount>,
    #[serde(default)]
    pub counts_by_status: Vec<SimulationCount>,
    #[serde(default)]
    pub recent_runs: Vec<SimulationRecentRun>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationOverviewSummary {
    pub total_missions: usize,
    pub active_missions: usize,
    pub simulating_missions: usize,
    pub missions_with_runs: usize,
    pub total_runs: usize,
    pub simulation_runs: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationCount {
    pub key: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SimulationRecentRun {
    pub run_id: String,
    pub mission_id: String,
    pub mission_title: String,
    pub mission_status: MissionStatus,
    pub mission_priority: MissionPriority,
    pub mission_last_activity_at: String,
    pub run_type: RunType,
    pub run_status: RunStatus,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub activity_at: String,
    pub run_activity_at: Option<String>,
    pub summary: Option<String>,
    pub error_message: Option<String>,
}

#[tauri::command]
pub fn simulation_get_overview(db: State<'_, Database>) -> Result<SimulationOverview, AppError> {
    let repo = MissionRepository::new(db.inner().clone());
    build_simulation_overview(&repo)
}

#[tauri::command]
pub fn simulation_create_scenario_run(
    db: State<'_, Database>,
    request: SimulationCreateScenarioRunRequest,
) -> Result<SimulationScenarioRun, AppError> {
    create_scenario_run(db.inner(), request)
}

#[tauri::command]
pub fn simulation_list_scenario_runs(
    db: State<'_, Database>,
    request: SimulationScenarioRunListRequest,
) -> Result<Vec<SimulationScenarioRun>, AppError> {
    list_scenario_runs(db.inner(), &request.mission_id)
}

#[tauri::command]
pub fn simulation_compare_scenarios(
    db: State<'_, Database>,
    request: SimulationComparisonRequest,
) -> Result<SimulationComparisonMatrix, AppError> {
    compare_scenario_runs(db.inner(), &request.mission_id)
}

#[tauri::command]
pub fn simulation_get_comparison_matrix(
    db: State<'_, Database>,
    request: SimulationComparisonRequest,
) -> Result<SimulationComparisonMatrix, AppError> {
    simulation_compare_scenarios(db, request)
}

#[tauri::command]
pub fn simulation_run_local_sandbox(
    db: State<'_, Database>,
    request: SimulationRunLocalSandboxRequest,
) -> Result<SimulationLocalSandboxRun, AppError> {
    run_local_sandbox_simulation(db.inner(), request)
}

#[tauri::command]
pub fn simulation_list_local_sandbox_runs(
    db: State<'_, Database>,
    request: SimulationLocalSandboxRunListRequest,
) -> Result<Vec<SimulationLocalSandboxRun>, AppError> {
    list_local_sandbox_runs_for_db(db.inner(), request)
}

#[tauri::command]
pub fn simulation_list_external_saas_runs(
    db: State<'_, Database>,
    request: SimulationCapabilityRunListRequest,
) -> Result<Vec<SimulationExternalSaasRun>, AppError> {
    list_external_saas_runs_for_db(db.inner(), request)
}

#[tauri::command]
pub async fn simulation_run_external_saas(
    db: State<'_, Database>,
    request: SimulationRunExternalSaasRequest,
) -> Result<SimulationExternalSaasRun, AppError> {
    let database = db.inner().clone();
    run_external_saas_simulation_for_db(&database, request).await
}

#[tauri::command]
pub fn simulation_run_high_fidelity_sandbox(
    db: State<'_, Database>,
    request: SimulationRunHighFidelitySandboxRequest,
) -> Result<SimulationHighFidelitySandboxRun, AppError> {
    run_high_fidelity_sandbox_simulation(db.inner(), request)
}

#[tauri::command]
pub fn simulation_list_high_fidelity_sandbox_runs(
    db: State<'_, Database>,
    request: SimulationCapabilityRunListRequest,
) -> Result<Vec<SimulationHighFidelitySandboxRun>, AppError> {
    list_high_fidelity_sandbox_runs_for_db(db.inner(), request)
}

#[tauri::command]
pub fn simulation_list_handoff_policy_templates(
    db: State<'_, Database>,
) -> Result<Vec<SimulationHandoffPolicyTemplate>, AppError> {
    list_handoff_policy_templates_for_db(db.inner())
}

#[tauri::command]
pub fn simulation_save_handoff_policy_template(
    db: State<'_, Database>,
    request: SimulationSaveHandoffPolicyTemplateRequest,
) -> Result<SimulationHandoffPolicyTemplate, AppError> {
    save_handoff_policy_template_for_db(db.inner(), request)
}

#[tauri::command]
pub fn simulation_list_scoring_formula_templates(
    db: State<'_, Database>,
) -> Result<Vec<SimulationScoringFormulaTemplate>, AppError> {
    list_scoring_formula_templates_for_db(db.inner())
}

#[tauri::command]
pub fn simulation_save_scoring_formula_template(
    db: State<'_, Database>,
    request: SimulationSaveScoringFormulaTemplateRequest,
) -> Result<SimulationScoringFormulaTemplate, AppError> {
    save_scoring_formula_template_for_db(db.inner(), request)
}

#[tauri::command]
pub fn simulation_export_template_bundle(
    db: State<'_, Database>,
) -> Result<SimulationTemplateBundle, AppError> {
    export_template_bundle_for_db(db.inner())
}

#[tauri::command]
pub fn simulation_import_template_bundle(
    db: State<'_, Database>,
    request: SimulationImportTemplateBundleRequest,
) -> Result<SimulationImportTemplateBundleResponse, AppError> {
    import_template_bundle_for_db(db.inner(), request)
}

#[tauri::command]
pub fn simulation_list_template_bundle_audit_log(
    db: State<'_, Database>,
) -> Result<Vec<SimulationTemplateBundleAuditEntry>, AppError> {
    list_template_bundle_audit_log_for_db(db.inner())
}

#[tauri::command]
pub fn simulation_export_template_bundle_audit_log(
    db: State<'_, Database>,
    request: SimulationExportTemplateBundleAuditLogRequest,
) -> Result<SimulationExportTemplateBundleAuditLogResponse, AppError> {
    export_template_bundle_audit_log_for_db(db.inner(), request)
}

#[tauri::command]
pub fn simulation_preflight_template_bundle_import(
    db: State<'_, Database>,
    request: SimulationImportTemplateBundleRequest,
) -> Result<SimulationTemplateBundlePreflightResponse, AppError> {
    preflight_template_bundle_import_for_db(db.inner(), request)
}

fn build_simulation_overview(repo: &MissionRepository) -> AppResult<SimulationOverview> {
    let missions = repo.list(MissionListFilter {
        query: None,
        status: None,
        limit: Some(OVERVIEW_MISSION_LIMIT),
    })?;
    let total_missions = missions.len();
    let active_missions = missions
        .iter()
        .filter(|mission| {
            !matches!(
                mission.status,
                MissionStatus::Archived | MissionStatus::Completed | MissionStatus::Failed
            )
        })
        .count();
    let simulating_missions = missions
        .iter()
        .filter(|mission| mission.status == MissionStatus::Simulating)
        .count();

    let mut total_runs = 0_usize;
    let mut simulation_runs = 0_usize;
    let mut missions_with_runs = 0_usize;
    let mut counts_by_type = [
        ("research".to_string(), 0_usize),
        ("simulation".to_string(), 0),
        ("council".to_string(), 0),
        ("execution".to_string(), 0),
        ("growth".to_string(), 0),
    ];
    let mut counts_by_status = [
        ("queued".to_string(), 0_usize),
        ("running".to_string(), 0),
        ("completed".to_string(), 0),
        ("failed".to_string(), 0),
        ("cancelled".to_string(), 0),
    ];
    let mut recent_runs = Vec::new();

    for mission in missions {
        let runs = repo.list_runs(&mission.id)?;
        if !runs.is_empty() {
            missions_with_runs += 1;
        }

        for run in runs {
            total_runs += 1;
            increment_count(&mut counts_by_type, run_type_key(&run.r#type));
            increment_count(&mut counts_by_status, run_status_key(&run.status));
            if run.r#type == RunType::Simulation {
                simulation_runs += 1;
            }

            if include_in_recent_runs(&run.r#type) {
                let started_at = run.started_at.clone();
                let finished_at = run.finished_at.clone();
                let activity_at = [
                    Some(mission.last_activity_at.as_str()),
                    finished_at.as_deref(),
                    started_at.as_deref(),
                ]
                .into_iter()
                .flatten()
                .max()
                .unwrap_or(mission.last_activity_at.as_str())
                .to_string();

                recent_runs.push(SimulationRecentRun {
                    run_id: run.id,
                    mission_id: mission.id.clone(),
                    mission_title: mission.title.clone(),
                    mission_status: mission.status.clone(),
                    mission_priority: mission.priority.clone(),
                    mission_last_activity_at: mission.last_activity_at.clone(),
                    run_type: run.r#type,
                    run_status: run.status,
                    started_at: started_at.clone(),
                    finished_at: finished_at.clone(),
                    activity_at,
                    run_activity_at: finished_at.or(started_at),
                    summary: run.summary,
                    error_message: run.error_message,
                });
            }
        }
    }

    recent_runs.sort_by(|left, right| {
        right
            .activity_at
            .cmp(&left.activity_at)
            .then_with(|| right.run_activity_at.cmp(&left.run_activity_at))
    });
    recent_runs.truncate(RECENT_RUN_LIMIT);

    Ok(SimulationOverview {
        summary: SimulationOverviewSummary {
            total_missions,
            active_missions,
            simulating_missions,
            missions_with_runs,
            total_runs,
            simulation_runs,
        },
        counts_by_type: counts_by_type
            .into_iter()
            .map(|(key, count)| SimulationCount { key, count })
            .collect(),
        counts_by_status: counts_by_status
            .into_iter()
            .map(|(key, count)| SimulationCount { key, count })
            .collect(),
        recent_runs,
    })
}

fn export_template_bundle_for_db(db: &Database) -> AppResult<SimulationTemplateBundle> {
    let bundle = SimulationTemplateBundle {
        schema_version: 1,
        exported_at: Utc::now().to_rfc3339(),
        handoff_policy_templates: list_handoff_policy_templates_for_db(db)?,
        scoring_formula_templates: list_scoring_formula_templates_for_db(db)?,
    };
    record_template_bundle_audit_event(
        db,
        "export",
        bundle.handoff_policy_templates.len(),
        bundle.scoring_formula_templates.len(),
        "Manual local template bundle export",
    )?;
    Ok(bundle)
}

fn preflight_template_bundle_import_for_db(
    db: &Database,
    request: SimulationImportTemplateBundleRequest,
) -> AppResult<SimulationTemplateBundlePreflightResponse> {
    let bundle = parse_template_bundle_json(&request.bundle_json)?;
    let existing_handoff = list_handoff_policy_templates_for_db(db)?;
    let existing_formula = list_scoring_formula_templates_for_db(db)?;
    let (handoff_policy_templates, mut handoff_conflicts) =
        preflight_handoff_templates(&existing_handoff, &bundle.handoff_policy_templates);
    let (scoring_formula_templates, mut formula_conflicts) =
        preflight_formula_templates(&existing_formula, &bundle.scoring_formula_templates);
    handoff_conflicts.append(&mut formula_conflicts);
    let total_count =
        bundle.handoff_policy_templates.len() + bundle.scoring_formula_templates.len();

    Ok(SimulationTemplateBundlePreflightResponse {
        schema_version: bundle.schema_version,
        total_count,
        handoff_policy_templates,
        scoring_formula_templates,
        conflicts: handoff_conflicts,
    })
}

fn parse_template_bundle_json(bundle_json: &str) -> AppResult<SimulationTemplateBundle> {
    let bundle_json = bundle_json.trim();
    if bundle_json.is_empty() {
        return Err(AppError::validation(
            "simulation template bundle cannot be empty",
        ));
    }

    let bundle: SimulationTemplateBundle =
        serde_json::from_str(bundle_json).map_err(AppError::from_json_error)?;
    if bundle.schema_version == 0 {
        return Err(AppError::validation(
            "simulation template bundle schema_version must be greater than zero",
        ));
    }
    Ok(bundle)
}

fn preflight_handoff_templates(
    existing: &[SimulationHandoffPolicyTemplate],
    incoming: &[SimulationHandoffPolicyTemplate],
) -> (
    SimulationTemplateBundlePreflightSection,
    Vec<SimulationTemplateBundleConflict>,
) {
    let mut section = SimulationTemplateBundlePreflightSection {
        create_count: 0,
        update_count: 0,
        unchanged_count: 0,
    };
    let mut conflicts = Vec::new();
    for template in incoming {
        match existing.iter().find(|item| item.id == template.id) {
            Some(current) if current == template => section.unchanged_count += 1,
            Some(current) => {
                section.update_count += 1;
                conflicts.push(SimulationTemplateBundleConflict {
                    id: template.id.clone(),
                    template_type: "handoff_policy".to_string(),
                    existing_name: current.name.clone(),
                    incoming_name: template.name.clone(),
                });
            }
            None => section.create_count += 1,
        }
    }
    (section, conflicts)
}

fn preflight_formula_templates(
    existing: &[SimulationScoringFormulaTemplate],
    incoming: &[SimulationScoringFormulaTemplate],
) -> (
    SimulationTemplateBundlePreflightSection,
    Vec<SimulationTemplateBundleConflict>,
) {
    let mut section = SimulationTemplateBundlePreflightSection {
        create_count: 0,
        update_count: 0,
        unchanged_count: 0,
    };
    let mut conflicts = Vec::new();
    for template in incoming {
        match existing.iter().find(|item| item.id == template.id) {
            Some(current) if current == template => section.unchanged_count += 1,
            Some(current) => {
                section.update_count += 1;
                conflicts.push(SimulationTemplateBundleConflict {
                    id: template.id.clone(),
                    template_type: "scoring_formula".to_string(),
                    existing_name: current.name.clone(),
                    incoming_name: template.name.clone(),
                });
            }
            None => section.create_count += 1,
        }
    }
    (section, conflicts)
}

fn import_template_bundle_for_db(
    db: &Database,
    request: SimulationImportTemplateBundleRequest,
) -> AppResult<SimulationImportTemplateBundleResponse> {
    let bundle = parse_template_bundle_json(&request.bundle_json)?;

    let imported_handoff_policy_templates = bundle.handoff_policy_templates.len();
    let imported_scoring_formula_templates = bundle.scoring_formula_templates.len();

    for template in bundle.handoff_policy_templates {
        save_handoff_policy_template_for_db(
            db,
            SimulationSaveHandoffPolicyTemplateRequest {
                id: Some(template.id),
                name: template.name,
                handoff_target: template.handoff_target,
                execution_risk_level: template.execution_risk_level,
                description: Some(template.description),
            },
        )?;
    }

    for template in bundle.scoring_formula_templates {
        save_scoring_formula_template_for_db(
            db,
            SimulationSaveScoringFormulaTemplateRequest {
                id: Some(template.id),
                name: template.name,
                base_score: template.base_score,
                impact_multiplier: template.impact_multiplier,
                uncertainty_penalty: template.uncertainty_penalty,
                description: Some(template.description),
            },
        )?;
    }

    record_template_bundle_audit_event(
        db,
        "import",
        imported_handoff_policy_templates,
        imported_scoring_formula_templates,
        "Manual local template bundle import",
    )?;

    Ok(SimulationImportTemplateBundleResponse {
        imported_handoff_policy_templates,
        imported_scoring_formula_templates,
        handoff_policy_templates: list_handoff_policy_templates_for_db(db)?,
        scoring_formula_templates: list_scoring_formula_templates_for_db(db)?,
    })
}

fn list_template_bundle_audit_log_for_db(
    db: &Database,
) -> AppResult<Vec<SimulationTemplateBundleAuditEntry>> {
    let stored = db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&TEMPLATE_BUNDLE_AUDIT_LOG_KEY],
        |row| row.get::<_, String>(0),
    );

    match stored {
        Ok(value_json) => {
            serde_json::from_str::<Vec<SimulationTemplateBundleAuditEntry>>(&value_json)
                .map_err(AppError::from_json_error)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(Vec::new()),
        Err(error) => Err(AppError::storage(format!(
            "Failed to load template bundle audit log: {}",
            error
        ))),
    }
}

fn export_template_bundle_audit_log_for_db(
    db: &Database,
    request: SimulationExportTemplateBundleAuditLogRequest,
) -> AppResult<SimulationExportTemplateBundleAuditLogResponse> {
    let mut events = list_template_bundle_audit_log_for_db(db)?;
    events.sort_by(|left, right| {
        right
            .occurred_at
            .cmp(&left.occurred_at)
            .then_with(|| right.id.cmp(&left.id))
    });

    let total = events.len();
    let limit = normalize_template_bundle_audit_export_limit(request.limit);
    events.truncate(limit.min(total));

    Ok(SimulationExportTemplateBundleAuditLogResponse {
        total,
        exported_count: events.len(),
        events,
    })
}

fn record_template_bundle_audit_event(
    db: &Database,
    action: &str,
    handoff_policy_template_count: usize,
    scoring_formula_template_count: usize,
    note: &str,
) -> AppResult<()> {
    let mut log = list_template_bundle_audit_log_for_db(db)?;
    log.insert(
        0,
        SimulationTemplateBundleAuditEntry {
            id: Uuid::new_v4().to_string(),
            action: action.to_string(),
            actor: "local-operator".to_string(),
            handoff_policy_template_count,
            scoring_formula_template_count,
            note: note.to_string(),
            occurred_at: Utc::now().to_rfc3339(),
        },
    );
    log.truncate(TEMPLATE_BUNDLE_AUDIT_LOG_LIMIT);
    let value_json = serde_json::to_string(&log).map_err(AppError::from_json_error)?;
    let now = Utc::now().to_rfc3339();
    db.execute(
        "INSERT OR REPLACE INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)",
        &[&TEMPLATE_BUNDLE_AUDIT_LOG_KEY, &value_json, &now],
    )?;
    Ok(())
}

fn normalize_template_bundle_audit_export_limit(limit: Option<usize>) -> usize {
    match limit {
        Some(0) | None => TEMPLATE_BUNDLE_AUDIT_LOG_EXPORT_DEFAULT_LIMIT,
        Some(value) => value.min(TEMPLATE_BUNDLE_AUDIT_LOG_EXPORT_MAX_LIMIT),
    }
}

fn list_handoff_policy_templates_for_db(
    db: &Database,
) -> AppResult<Vec<SimulationHandoffPolicyTemplate>> {
    let stored = db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&HANDOFF_POLICY_TEMPLATES_KEY],
        |row| row.get::<_, String>(0),
    );

    match stored {
        Ok(value_json) => {
            let mut templates =
                serde_json::from_str::<Vec<SimulationHandoffPolicyTemplate>>(&value_json)
                    .map_err(AppError::from_json_error)?;
            if templates.is_empty() {
                templates = default_handoff_policy_templates();
            }
            templates.sort_by(|left, right| left.name.cmp(&right.name));
            Ok(templates)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(default_handoff_policy_templates()),
        Err(error) => Err(AppError::storage(format!(
            "Failed to load handoff policy templates: {}",
            error
        ))),
    }
}

fn save_handoff_policy_template_for_db(
    db: &Database,
    request: SimulationSaveHandoffPolicyTemplateRequest,
) -> AppResult<SimulationHandoffPolicyTemplate> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::validation(
            "handoff policy template name cannot be empty",
        ));
    }

    let updated_at = Utc::now().to_rfc3339();
    let template = SimulationHandoffPolicyTemplate {
        id: normalize_optional_text(request.id)
            .map(|id| slugify(&id))
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| slugify(&name)),
        name,
        handoff_target: normalize_handoff_target(Some(request.handoff_target)),
        execution_risk_level: normalize_execution_risk_level(Some(request.execution_risk_level)),
        description: normalize_optional_text(request.description).unwrap_or_else(|| {
            "Reusable scenario handoff policy for subsequent Mission simulations.".to_string()
        }),
        updated_at,
    };

    let mut templates = list_handoff_policy_templates_for_db(db)?;
    if let Some(existing) = templates
        .iter_mut()
        .find(|existing| existing.id == template.id)
    {
        *existing = template.clone();
    } else {
        templates.push(template.clone());
    }
    templates.sort_by(|left, right| left.name.cmp(&right.name));

    let value_json = serde_json::to_string(&templates).map_err(AppError::from_json_error)?;
    db.execute(
        "INSERT INTO app_settings (key, value_json, updated_at)
         VALUES (?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET
            value_json = excluded.value_json,
            updated_at = excluded.updated_at",
        &[
            &HANDOFF_POLICY_TEMPLATES_KEY as &dyn rusqlite::ToSql,
            &value_json,
            &template.updated_at,
        ],
    )?;

    Ok(template)
}

fn default_handoff_policy_templates() -> Vec<SimulationHandoffPolicyTemplate> {
    let updated_at = "built-in".to_string();
    vec![
        SimulationHandoffPolicyTemplate {
            id: "council-and-execution".to_string(),
            name: "Council + Execution".to_string(),
            handoff_target: "council_and_execution".to_string(),
            execution_risk_level: "medium".to_string(),
            description:
                "Create both a Scenario Reviewer Council step and an Execution review step."
                    .to_string(),
            updated_at: updated_at.clone(),
        },
        SimulationHandoffPolicyTemplate {
            id: "council-only".to_string(),
            name: "Council only".to_string(),
            handoff_target: "council_only".to_string(),
            execution_risk_level: "medium".to_string(),
            description: "Create only the Scenario Reviewer Council step after saving.".to_string(),
            updated_at: updated_at.clone(),
        },
        SimulationHandoffPolicyTemplate {
            id: "execution-approval".to_string(),
            name: "Execution approval".to_string(),
            handoff_target: "execution_only".to_string(),
            execution_risk_level: "high".to_string(),
            description: "Create a high-risk Execution review step that starts awaiting approval."
                .to_string(),
            updated_at: updated_at.clone(),
        },
        SimulationHandoffPolicyTemplate {
            id: "timeline-only".to_string(),
            name: "Timeline only".to_string(),
            handoff_target: "timeline_only".to_string(),
            execution_risk_level: "low".to_string(),
            description: "Record only the completed Simulation run and timeline event.".to_string(),
            updated_at,
        },
    ]
}

fn list_scoring_formula_templates_for_db(
    db: &Database,
) -> AppResult<Vec<SimulationScoringFormulaTemplate>> {
    let stored = db.query_row(
        "SELECT value_json FROM app_settings WHERE key = ?1",
        &[&SCORING_FORMULA_TEMPLATES_KEY],
        |row| row.get::<_, String>(0),
    );

    match stored {
        Ok(value_json) => {
            let mut templates =
                serde_json::from_str::<Vec<SimulationScoringFormulaTemplate>>(&value_json)
                    .map_err(AppError::from_json_error)?;
            if templates.is_empty() {
                templates = default_scoring_formula_templates();
            }
            templates.sort_by(|left, right| left.name.cmp(&right.name));
            Ok(templates)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(default_scoring_formula_templates()),
        Err(error) => Err(AppError::storage(format!(
            "Failed to load scoring formula templates: {}",
            error
        ))),
    }
}

fn save_scoring_formula_template_for_db(
    db: &Database,
    request: SimulationSaveScoringFormulaTemplateRequest,
) -> AppResult<SimulationScoringFormulaTemplate> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::validation(
            "scoring formula template name cannot be empty",
        ));
    }

    let updated_at = Utc::now().to_rfc3339();
    let template = SimulationScoringFormulaTemplate {
        id: normalize_optional_text(request.id)
            .map(|id| slugify(&id))
            .filter(|id| !id.is_empty())
            .unwrap_or_else(|| slugify(&name)),
        name,
        base_score: normalize_formula_value(request.base_score, 20.0, 80.0, 56.0),
        impact_multiplier: normalize_formula_value(request.impact_multiplier, 0.0, 30.0, 18.0),
        uncertainty_penalty: normalize_formula_value(request.uncertainty_penalty, 0.0, 30.0, 14.0),
        description: normalize_optional_text(request.description).unwrap_or_else(|| {
            "Reusable scenario scoring formula for subsequent Mission simulations.".to_string()
        }),
        updated_at,
    };

    let mut templates = list_scoring_formula_templates_for_db(db)?;
    if let Some(existing) = templates
        .iter_mut()
        .find(|existing| existing.id == template.id)
    {
        *existing = template.clone();
    } else {
        templates.push(template.clone());
    }
    templates.sort_by(|left, right| left.name.cmp(&right.name));

    let value_json = serde_json::to_string(&templates).map_err(AppError::from_json_error)?;
    db.execute(
        "INSERT INTO app_settings (key, value_json, updated_at)
         VALUES (?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET
            value_json = excluded.value_json,
            updated_at = excluded.updated_at",
        &[
            &SCORING_FORMULA_TEMPLATES_KEY as &dyn rusqlite::ToSql,
            &value_json,
            &template.updated_at,
        ],
    )?;

    Ok(template)
}

fn default_scoring_formula_templates() -> Vec<SimulationScoringFormulaTemplate> {
    let updated_at = "built-in".to_string();
    vec![
        SimulationScoringFormulaTemplate {
            id: "balanced".to_string(),
            name: "Balanced".to_string(),
            base_score: 56.0,
            impact_multiplier: 18.0,
            uncertainty_penalty: 14.0,
            description: "Balanced default scoring for most strategy simulations.".to_string(),
            updated_at: updated_at.clone(),
        },
        SimulationScoringFormulaTemplate {
            id: "impact-forward".to_string(),
            name: "Impact forward".to_string(),
            base_score: 58.0,
            impact_multiplier: 24.0,
            uncertainty_penalty: 10.0,
            description: "Favor high-impact scenarios when uncertainty is acceptable.".to_string(),
            updated_at: updated_at.clone(),
        },
        SimulationScoringFormulaTemplate {
            id: "risk-sensitive".to_string(),
            name: "Risk sensitive".to_string(),
            base_score: 52.0,
            impact_multiplier: 14.0,
            uncertainty_penalty: 24.0,
            description: "Penalize uncertain scenario paths more aggressively.".to_string(),
            updated_at,
        },
    ]
}

fn normalize_formula_value(value: f64, min: f64, max: f64, fallback: f64) -> f64 {
    if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    }
}

fn create_scenario_run(
    db: &Database,
    request: SimulationCreateScenarioRunRequest,
) -> AppResult<SimulationScenarioRun> {
    let mission_id = request.mission_id.trim().to_string();
    let baseline = request.baseline.trim().to_string();
    let variables = normalize_variables(request.variables);
    let mut option_cards = normalize_option_cards(request.option_cards, request.options);
    option_cards = derive_option_scores(option_cards, &variables);
    let requested_recommendation = normalize_optional_text(request.recommendation);
    let requested_recommendation_reason = normalize_optional_text(request.recommendation_reason);
    let selected_option_id =
        normalize_selected_option_id(request.selected_option_id, &option_cards)
            .or_else(|| {
                requested_recommendation
                    .as_deref()
                    .and_then(|label| find_option_by_label(&option_cards, label))
                    .map(|option| option.id.clone())
            })
            .or_else(|| derive_selected_option_id(&option_cards));
    let recommendation = requested_recommendation.or_else(|| {
        selected_option_id
            .as_ref()
            .and_then(|option_id| find_option_by_id(&option_cards, option_id))
            .map(|option| option.label.clone())
            .or_else(|| derive_recommendation(&option_cards))
    });
    let comparison_summary = normalize_optional_text(request.comparison_summary).or_else(|| {
        derive_comparison_summary(&option_cards, &variables, selected_option_id.as_deref())
    });
    let recommendation_reason = requested_recommendation_reason
        .or_else(|| derive_recommendation_reason(&option_cards, selected_option_id.as_deref()))
        .or_else(|| comparison_summary.clone());
    let handoff_target = normalize_handoff_target(request.handoff_target);
    let execution_risk_level = normalize_execution_risk_level(request.execution_risk_level);
    let options = option_cards
        .iter()
        .map(|option| option.label.clone())
        .collect::<Vec<_>>();

    if mission_id.is_empty() {
        return Err(AppError::validation("Mission is required"));
    }
    if baseline.is_empty() {
        return Err(AppError::validation("Baseline is required"));
    }

    let repo = MissionRepository::new(db.clone());
    let mission = repo
        .get(&mission_id)?
        .ok_or_else(|| AppError::validation("Mission not found"))?;

    let scenario_run = SimulationScenarioRun {
        id: Uuid::new_v4().to_string(),
        mission_id,
        mission_title: mission.title,
        baseline,
        options,
        option_cards,
        variables,
        recommendation,
        recommendation_reason,
        comparison_summary,
        selected_option_id,
        handoff_target,
        execution_risk_level,
        created_at: Utc::now().to_rfc3339(),
    };
    let options_json = serde_json::to_string(&ScenarioRunPayload {
        option_cards: scenario_run.option_cards.clone(),
        variables: scenario_run.variables.clone(),
        recommendation_reason: scenario_run.recommendation_reason.clone(),
        comparison_summary: scenario_run.comparison_summary.clone(),
        selected_option_id: scenario_run.selected_option_id.clone(),
        handoff_target: Some(scenario_run.handoff_target.clone()),
        execution_risk_level: Some(scenario_run.execution_risk_level.clone()),
    })
    .map_err(AppError::from_json_error)?;

    db.execute(
        "INSERT INTO scenario_runs (
            id, mission_id, baseline, options_json, recommendation, created_at
        ) VALUES (?, ?, ?, ?, ?, ?)",
        &[
            &scenario_run.id as &dyn rusqlite::ToSql,
            &scenario_run.mission_id,
            &scenario_run.baseline,
            &options_json,
            &scenario_run.recommendation,
            &scenario_run.created_at,
        ],
    )?;

    record_scenario_governance_handoff(db, &scenario_run)?;

    Ok(scenario_run)
}

async fn run_external_saas_simulation_for_db(
    db: &Database,
    request: SimulationRunExternalSaasRequest,
) -> AppResult<SimulationExternalSaasRun> {
    let SimulationRunExternalSaasRequest {
        mission_id,
        provider,
        endpoint_url,
        input_json,
        target_remote_user_id,
        dry_run,
        confirmation_phrase,
        timeout_ms,
    } = request;
    let mission_id = mission_id.trim().to_string();
    if mission_id.is_empty() {
        return Err(AppError::validation("Mission is required"));
    }
    let repo = MissionRepository::new(db.clone());
    repo.get(&mission_id)?
        .ok_or_else(|| AppError::validation("Mission not found"))?;

    let provider = normalize_external_saas_provider(provider)?;
    let dry_run = dry_run.unwrap_or(true);
    let input = parse_external_saas_input(input_json)?;
    let target_remote_user_id = normalize_optional_text(target_remote_user_id);
    let request_preview = serde_json::to_string_pretty(&serde_json::json!({
        "provider": provider,
        "endpoint_url": endpoint_url,
        "input": input,
        "target_remote_user_id": target_remote_user_id,
        "dry_run": dry_run,
        "network_invocation": provider == "http_json" && !dry_run,
    }))
    .map_err(AppError::from_json_error)?;

    if provider == "http_json"
        && !dry_run
        && confirmation_phrase.as_deref().map(str::trim) != Some(EXTERNAL_SAAS_CONFIRM_PHRASE)
    {
        return Err(AppError::validation(format!(
            "non-dry-run external SaaS simulation requires confirmation phrase `{}`",
            EXTERNAL_SAAS_CONFIRM_PHRASE
        )));
    }

    let timeout_ms = normalize_external_saas_timeout(timeout_ms);
    let endpoint_url = normalize_external_saas_endpoint(&provider, endpoint_url)?;
    let (executed, network_invocation, response_status, response_body) = if dry_run {
        (
            false,
            false,
            None,
            serde_json::json!({
                "mode": "dry_run",
                "provider": provider,
                "endpoint_url": endpoint_url,
                "network_invocation": false,
                "request_preview": request_preview,
            })
            .to_string(),
        )
    } else if provider == "local_echo" {
        (
            true,
            false,
            Some(200),
            serde_json::json!({
                "provider": "local_echo",
                "network_invocation": false,
                "received_input": input,
                "simulated_at": Utc::now().to_rfc3339(),
            })
            .to_string(),
        )
    } else {
        let endpoint = endpoint_url
            .as_deref()
            .ok_or_else(|| AppError::validation("http_json provider requires endpoint_url"))?;
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(timeout_ms))
            .build()
            .map_err(|err| {
                AppError::runtime(format!("Failed to build SaaS adapter client: {}", err))
            })?;
        let response = client
            .post(endpoint)
            .json(&input)
            .send()
            .await
            .map_err(|err| {
                AppError::runtime(format!("External SaaS adapter request failed: {}", err))
            })?;
        let status = response.status().as_u16();
        let body = response.text().await.map_err(|err| {
            AppError::runtime(format!(
                "External SaaS adapter response read failed: {}",
                err
            ))
        })?;
        (
            true,
            true,
            Some(status),
            truncate_simulation_text(&body, EXTERNAL_SAAS_MAX_RESPONSE_CHARS),
        )
    };

    let run_id = Uuid::new_v4().to_string();
    let event_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let summary = if dry_run {
        format!(
            "External SaaS simulation dry-run prepared provider adapter `{}` without network execution.",
            provider
        )
    } else {
        format!(
            "External SaaS simulation adapter `{}` executed with network_invocation={}",
            provider, network_invocation
        )
    };
    let response = SimulationExternalSaasRun {
        run_id: run_id.clone(),
        mission_id: mission_id.clone(),
        engine: EXTERNAL_SAAS_SIMULATION_ENGINE_NAME.to_string(),
        created_at: Some(created_at.clone()),
        status: Some("completed".to_string()),
        provider: provider.clone(),
        endpoint_url,
        target_remote_user_id,
        dry_run,
        executed,
        network_invocation,
        request_preview,
        response_status,
        response_body,
        summary: summary.clone(),
        audit_event_id: Some(event_id.clone()),
    };
    let payload_json = serde_json::to_string(&response).map_err(AppError::from_json_error)?;
    let event_type = if dry_run {
        "external_saas_simulation_previewed"
    } else {
        "external_saas_simulation_completed"
    };

    db.execute(
        "INSERT INTO runs (id, mission_id, type, status, started_at, finished_at, summary, error_message)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            &run_id as &dyn rusqlite::ToSql,
            &mission_id,
            &"simulation",
            &"completed",
            &created_at,
            &created_at,
            &summary,
            &Option::<String>::None,
        ],
    )?;
    db.execute(
        "INSERT INTO run_events (id, run_id, mission_id, event_type, message, payload_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        &[
            &event_id as &dyn rusqlite::ToSql,
            &run_id,
            &mission_id,
            &event_type,
            &summary,
            &Some(payload_json),
            &created_at,
        ],
    )?;

    Ok(response)
}

fn run_high_fidelity_sandbox_simulation(
    db: &Database,
    request: SimulationRunHighFidelitySandboxRequest,
) -> AppResult<SimulationHighFidelitySandboxRun> {
    let SimulationRunHighFidelitySandboxRequest {
        mission_id,
        baseline,
        options,
        agents,
        rounds,
        variables,
        target_remote_user_id,
    } = request;
    let mission_id = mission_id.trim().to_string();
    let baseline = baseline.trim().to_string();
    let options = normalize_local_sandbox_options(options);
    let agents = normalize_local_sandbox_agents(agents);
    let rounds = normalize_local_sandbox_rounds(rounds);
    let target_remote_user_id = normalize_optional_text(target_remote_user_id);
    if mission_id.is_empty() {
        return Err(AppError::validation("Mission is required"));
    }
    if baseline.is_empty() {
        return Err(AppError::validation("Baseline is required"));
    }
    if options.is_empty() {
        return Err(AppError::validation(
            "At least one simulation option is required",
        ));
    }
    let repo = MissionRepository::new(db.clone());
    repo.get(&mission_id)?
        .ok_or_else(|| AppError::validation("Mission not found"))?;

    let turns = build_local_sandbox_turns(&mission_id, &baseline, &options, &agents, rounds);
    let option_scores = aggregate_local_sandbox_option_scores(&options, &turns);
    let recommendation =
        derive_local_sandbox_recommendation(&baseline, rounds, &option_scores, &turns)?;
    let run_id = Uuid::new_v4().to_string();
    let event_id = Uuid::new_v4().to_string();
    let base_run = SimulationLocalSandboxRun {
        run_id: run_id.clone(),
        mission_id: mission_id.clone(),
        engine: LOCAL_SANDBOX_ENGINE_NAME.to_string(),
        rounds,
        agents,
        turns,
        option_scores,
        recommendation,
        audit_event_id: None,
    };
    let world = build_high_fidelity_world(&base_run, variables);
    let created_at = Utc::now().to_rfc3339();
    let summary = format!(
        "High-fidelity local sandbox completed: {} recommended at {:.1}/100 with {} world event(s).",
        base_run.recommendation.option,
        base_run.recommendation.average_score,
        world.timeline.len()
    );
    let response = SimulationHighFidelitySandboxRun {
        run_id: run_id.clone(),
        mission_id: mission_id.clone(),
        engine: HIGH_FIDELITY_SANDBOX_ENGINE_NAME.to_string(),
        created_at: Some(created_at.clone()),
        status: Some("completed".to_string()),
        target_remote_user_id,
        base_run,
        world,
        summary: summary.clone(),
        audit_event_id: Some(event_id.clone()),
    };
    let payload_json = serde_json::to_string(&response).map_err(AppError::from_json_error)?;

    db.execute(
        "INSERT INTO runs (id, mission_id, type, status, started_at, finished_at, summary, error_message)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            &run_id as &dyn rusqlite::ToSql,
            &mission_id,
            &"simulation",
            &"completed",
            &created_at,
            &created_at,
            &summary,
            &Option::<String>::None,
        ],
    )?;
    db.execute(
        "INSERT INTO run_events (id, run_id, mission_id, event_type, message, payload_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        &[
            &event_id as &dyn rusqlite::ToSql,
            &run_id,
            &mission_id,
            &"high_fidelity_sandbox_completed",
            &summary,
            &Some(payload_json),
            &created_at,
        ],
    )?;

    Ok(response)
}

fn run_local_sandbox_simulation(
    db: &Database,
    request: SimulationRunLocalSandboxRequest,
) -> AppResult<SimulationLocalSandboxRun> {
    let mission_id = request.mission_id.trim().to_string();
    let baseline = request.baseline.trim().to_string();
    let options = normalize_local_sandbox_options(request.options);
    let agents = normalize_local_sandbox_agents(request.agents);
    let rounds = normalize_local_sandbox_rounds(request.rounds);

    if mission_id.is_empty() {
        return Err(AppError::validation("Mission is required"));
    }
    if baseline.is_empty() {
        return Err(AppError::validation("Baseline is required"));
    }
    if options.is_empty() {
        return Err(AppError::validation(
            "At least one simulation option is required",
        ));
    }

    let repo = MissionRepository::new(db.clone());
    repo.get(&mission_id)?
        .ok_or_else(|| AppError::validation("Mission not found"))?;

    let turns = build_local_sandbox_turns(&mission_id, &baseline, &options, &agents, rounds);
    let option_scores = aggregate_local_sandbox_option_scores(&options, &turns);
    let recommendation =
        derive_local_sandbox_recommendation(&baseline, rounds, &option_scores, &turns)?;
    let run_id = Uuid::new_v4().to_string();
    let event_id = Uuid::new_v4().to_string();
    let created_at = Utc::now().to_rfc3339();
    let summary = format!(
        "Local sandbox simulation completed: {} recommended at {:.1}/100 after {} rounds.",
        recommendation.option, recommendation.average_score, rounds
    );

    let mut response = SimulationLocalSandboxRun {
        run_id: run_id.clone(),
        mission_id: mission_id.clone(),
        engine: LOCAL_SANDBOX_ENGINE_NAME.to_string(),
        rounds,
        agents,
        turns,
        option_scores,
        recommendation,
        audit_event_id: Some(event_id.clone()),
    };
    let event_payload = serde_json::to_string(&response).map_err(AppError::from_json_error)?;

    db.execute(
        "INSERT INTO runs (id, mission_id, type, status, started_at, finished_at, summary, error_message)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            &run_id as &dyn rusqlite::ToSql,
            &mission_id,
            &"simulation",
            &"completed",
            &created_at,
            &created_at,
            &summary,
            &Option::<String>::None,
        ],
    )?;

    db.execute(
        "INSERT INTO run_events (id, run_id, mission_id, event_type, message, payload_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        &[
            &event_id as &dyn rusqlite::ToSql,
            &run_id,
            &mission_id,
            &"local_sandbox_simulation_completed",
            &format!(
                "Local sandbox simulation completed with recommendation: {}",
                response.recommendation.option
            ),
            &Some(event_payload),
            &created_at,
        ],
    )?;

    response.audit_event_id = Some(event_id);
    Ok(response)
}

fn list_local_sandbox_runs_for_db(
    db: &Database,
    request: SimulationLocalSandboxRunListRequest,
) -> AppResult<Vec<SimulationLocalSandboxRun>> {
    let mission_id = request
        .mission_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let limit = request
        .limit
        .unwrap_or(LOCAL_SANDBOX_HISTORY_DEFAULT_LIMIT)
        .clamp(1, LOCAL_SANDBOX_HISTORY_MAX_LIMIT);

    let payloads = db.with_connection(|conn| {
        let mut payloads = Vec::new();
        if let Some(mission_id) = mission_id.as_deref() {
            let mut stmt = conn.prepare(
                "SELECT payload_json
                 FROM run_events
                 WHERE event_type = 'local_sandbox_simulation_completed'
                   AND payload_json IS NOT NULL
                   AND mission_id = ?1
                 ORDER BY datetime(created_at) DESC, rowid DESC
                 LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![mission_id, limit as i64], |row| {
                row.get::<_, String>(0)
            })?;
            for row in rows {
                payloads.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT payload_json
                 FROM run_events
                 WHERE event_type = 'local_sandbox_simulation_completed'
                   AND payload_json IS NOT NULL
                 ORDER BY datetime(created_at) DESC, rowid DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit as i64], |row| row.get::<_, String>(0))?;
            for row in rows {
                payloads.push(row?);
            }
        }
        Ok(payloads)
    })?;

    payloads
        .into_iter()
        .map(|payload| serde_json::from_str(&payload).map_err(AppError::from_json_error))
        .collect()
}

fn list_external_saas_runs_for_db(
    db: &Database,
    request: SimulationCapabilityRunListRequest,
) -> AppResult<Vec<SimulationExternalSaasRun>> {
    let target_remote_user_id = normalize_optional_text(request.target_remote_user_id.clone());
    let limit = normalize_capability_run_list_limit(request.limit);
    let payload_limit = if target_remote_user_id.is_some() {
        LOCAL_SANDBOX_HISTORY_MAX_LIMIT
    } else {
        limit
    };
    let payload_request = SimulationCapabilityRunListRequest {
        limit: Some(payload_limit),
        ..request
    };
    let runs = list_capability_run_payloads(
        db,
        payload_request,
        &[
            "external_saas_simulation_completed",
            "external_saas_simulation_previewed",
        ],
    )?
    .into_iter()
    .map(|payload| decode_capability_run_payload(&payload))
    .collect::<AppResult<Vec<SimulationExternalSaasRun>>>()?;
    Ok(filter_capability_runs_by_target(
        runs,
        target_remote_user_id.as_deref(),
        limit,
        |run| run.target_remote_user_id.as_deref(),
    ))
}

fn list_high_fidelity_sandbox_runs_for_db(
    db: &Database,
    request: SimulationCapabilityRunListRequest,
) -> AppResult<Vec<SimulationHighFidelitySandboxRun>> {
    let target_remote_user_id = normalize_optional_text(request.target_remote_user_id.clone());
    let limit = normalize_capability_run_list_limit(request.limit);
    let payload_limit = if target_remote_user_id.is_some() {
        LOCAL_SANDBOX_HISTORY_MAX_LIMIT
    } else {
        limit
    };
    let payload_request = SimulationCapabilityRunListRequest {
        limit: Some(payload_limit),
        ..request
    };
    let runs =
        list_capability_run_payloads(db, payload_request, &["high_fidelity_sandbox_completed"])?
            .into_iter()
            .map(|payload| decode_capability_run_payload(&payload))
            .collect::<AppResult<Vec<SimulationHighFidelitySandboxRun>>>()?;
    Ok(filter_capability_runs_by_target(
        runs,
        target_remote_user_id.as_deref(),
        limit,
        |run| run.target_remote_user_id.as_deref(),
    ))
}

struct CapabilityRunPayload {
    payload_json: String,
    created_at: String,
    status: Option<String>,
}

fn normalize_capability_run_list_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(LOCAL_SANDBOX_HISTORY_DEFAULT_LIMIT)
        .clamp(1, LOCAL_SANDBOX_HISTORY_MAX_LIMIT)
}

fn filter_capability_runs_by_target<T, F>(
    runs: Vec<T>,
    target_remote_user_id: Option<&str>,
    limit: usize,
    target_for: F,
) -> Vec<T>
where
    F: Fn(&T) -> Option<&str>,
{
    runs.into_iter()
        .filter(|run| {
            target_remote_user_id
                .map(|target| target_for(run) == Some(target))
                .unwrap_or(true)
        })
        .take(limit)
        .collect()
}

fn list_capability_run_payloads(
    db: &Database,
    request: SimulationCapabilityRunListRequest,
    event_types: &[&str],
) -> AppResult<Vec<CapabilityRunPayload>> {
    let mission_id = request
        .mission_id
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let limit = normalize_capability_run_list_limit(request.limit);

    db.with_connection(|conn| {
        let mut rows_out = Vec::new();
        if event_types.len() == 1 {
            if let Some(mission_id) = mission_id.as_deref() {
                let mut stmt = conn.prepare(
                    "SELECT run_events.payload_json, run_events.created_at, runs.status
                     FROM run_events
                     LEFT JOIN runs ON runs.id = run_events.run_id
                     WHERE run_events.event_type = ?1
                       AND run_events.payload_json IS NOT NULL
                       AND run_events.mission_id = ?2
                     ORDER BY datetime(run_events.created_at) DESC, run_events.rowid DESC
                     LIMIT ?3",
                )?;
                let rows =
                    stmt.query_map(params![event_types[0], mission_id, limit as i64], |row| {
                        Ok(CapabilityRunPayload {
                            payload_json: row.get::<_, String>(0)?,
                            created_at: row.get::<_, String>(1)?,
                            status: row.get::<_, Option<String>>(2)?,
                        })
                    })?;
                for row in rows {
                    rows_out.push(row?);
                }
            } else {
                let mut stmt = conn.prepare(
                    "SELECT run_events.payload_json, run_events.created_at, runs.status
                     FROM run_events
                     LEFT JOIN runs ON runs.id = run_events.run_id
                     WHERE run_events.event_type = ?1
                       AND run_events.payload_json IS NOT NULL
                     ORDER BY datetime(run_events.created_at) DESC, run_events.rowid DESC
                     LIMIT ?2",
                )?;
                let rows = stmt.query_map(params![event_types[0], limit as i64], |row| {
                    Ok(CapabilityRunPayload {
                        payload_json: row.get::<_, String>(0)?,
                        created_at: row.get::<_, String>(1)?,
                        status: row.get::<_, Option<String>>(2)?,
                    })
                })?;
                for row in rows {
                    rows_out.push(row?);
                }
            }
        } else if let Some(mission_id) = mission_id.as_deref() {
            let mut stmt = conn.prepare(
                "SELECT run_events.payload_json, run_events.created_at, runs.status
                 FROM run_events
                 LEFT JOIN runs ON runs.id = run_events.run_id
                 WHERE run_events.event_type IN (?1, ?2)
                   AND run_events.payload_json IS NOT NULL
                   AND run_events.mission_id = ?3
                 ORDER BY datetime(run_events.created_at) DESC, run_events.rowid DESC
                 LIMIT ?4",
            )?;
            let rows = stmt.query_map(
                params![event_types[0], event_types[1], mission_id, limit as i64],
                |row| {
                    Ok(CapabilityRunPayload {
                        payload_json: row.get::<_, String>(0)?,
                        created_at: row.get::<_, String>(1)?,
                        status: row.get::<_, Option<String>>(2)?,
                    })
                },
            )?;
            for row in rows {
                rows_out.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT run_events.payload_json, run_events.created_at, runs.status
                 FROM run_events
                 LEFT JOIN runs ON runs.id = run_events.run_id
                 WHERE run_events.event_type IN (?1, ?2)
                   AND run_events.payload_json IS NOT NULL
                 ORDER BY datetime(run_events.created_at) DESC, run_events.rowid DESC
                 LIMIT ?3",
            )?;
            let rows = stmt.query_map(
                params![event_types[0], event_types[1], limit as i64],
                |row| {
                    Ok(CapabilityRunPayload {
                        payload_json: row.get::<_, String>(0)?,
                        created_at: row.get::<_, String>(1)?,
                        status: row.get::<_, Option<String>>(2)?,
                    })
                },
            )?;
            for row in rows {
                rows_out.push(row?);
            }
        }

        Ok(rows_out)
    })
}

fn decode_capability_run_payload<T>(payload: &CapabilityRunPayload) -> AppResult<T>
where
    T: for<'de> Deserialize<'de>,
{
    let mut value: serde_json::Value =
        serde_json::from_str(&payload.payload_json).map_err(AppError::from_json_error)?;
    if let Some(object) = value.as_object_mut() {
        object.insert(
            "created_at".to_string(),
            serde_json::Value::String(payload.created_at.clone()),
        );
        if let Some(status) = payload.status.as_deref() {
            object.insert(
                "status".to_string(),
                serde_json::Value::String(status.to_string()),
            );
        }
    }
    serde_json::from_value(value).map_err(AppError::from_json_error)
}

fn normalize_external_saas_provider(provider: String) -> AppResult<String> {
    let normalized = provider.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return Err(AppError::validation("external SaaS provider is required"));
    }
    if normalized == "local_echo" || normalized == "http_json" {
        return Ok(normalized);
    }
    Err(AppError::validation(format!(
        "external SaaS provider `{}` is not supported; use local_echo or http_json",
        normalized
    )))
}

fn parse_external_saas_input(input_json: Option<String>) -> AppResult<serde_json::Value> {
    let raw = input_json
        .unwrap_or_else(|| "{}".to_string())
        .trim()
        .to_string();
    if raw.is_empty() {
        return Ok(serde_json::json!({}));
    }
    serde_json::from_str(&raw).map_err(AppError::from_json_error)
}

fn normalize_external_saas_endpoint(
    provider: &str,
    endpoint_url: Option<String>,
) -> AppResult<Option<String>> {
    let endpoint_url = endpoint_url
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if provider == "local_echo" {
        return Ok(endpoint_url);
    }
    let endpoint = endpoint_url
        .ok_or_else(|| AppError::validation("http_json provider requires endpoint_url"))?;
    let parsed = reqwest::Url::parse(&endpoint)
        .map_err(|err| AppError::validation(format!("endpoint_url is invalid: {}", err)))?;
    if parsed.scheme() != "http" && parsed.scheme() != "https" {
        return Err(AppError::validation(
            "http_json endpoint_url must use http or https",
        ));
    }
    Ok(Some(endpoint))
}

fn normalize_external_saas_timeout(timeout_ms: Option<u64>) -> u64 {
    timeout_ms
        .unwrap_or(EXTERNAL_SAAS_DEFAULT_TIMEOUT_MS)
        .clamp(50, EXTERNAL_SAAS_MAX_TIMEOUT_MS)
}

fn truncate_simulation_text(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("\n...[truncated]");
    truncated
}

fn build_high_fidelity_world(
    base_run: &SimulationLocalSandboxRun,
    variables: Vec<ScenarioVariable>,
) -> SimulationHighFidelityWorld {
    let mut entities = Vec::new();
    for agent in &base_run.agents {
        entities.push(SimulationHighFidelityEntity {
            id: format!("agent-{}", slugify(&agent.name)),
            label: agent.name.clone(),
            kind: "agent".to_string(),
            state: format!("{} / {}", agent.role, agent.stance),
            risk_score: round_score(
                50.0 - stance_bias_score(&agent.stance) - role_bias_score(&agent.role),
            ),
        });
    }
    for option_score in &base_run.option_scores {
        entities.push(SimulationHighFidelityEntity {
            id: format!("option-{}", slugify(&option_score.option)),
            label: option_score.option.clone(),
            kind: "option".to_string(),
            state: if option_score.option == base_run.recommendation.option {
                "recommended".to_string()
            } else {
                "contender".to_string()
            },
            risk_score: round_score(100.0 - option_score.average_score),
        });
    }

    let variables = normalize_high_fidelity_variables(variables);
    let timeline = base_run
        .turns
        .iter()
        .enumerate()
        .map(|(index, turn)| SimulationHighFidelityTimelineEvent {
            tick: index + 1,
            round: turn.round,
            actor: turn.agent_name.clone(),
            option: turn.option.clone(),
            score: turn.score,
            score_delta: round_score(turn.score - 50.0),
            state_changes: vec![
                format!("{} evaluated {}", turn.agent_name, turn.option),
                format!("score_delta={:.1}", turn.score - 50.0),
            ],
        })
        .collect::<Vec<_>>();
    let event_graph = build_high_fidelity_event_graph(base_run, &variables);
    let option_metric_heatmap = build_high_fidelity_heatmap(base_run, &variables);

    SimulationHighFidelityWorld {
        entities,
        variables,
        timeline,
        event_graph,
        option_metric_heatmap,
    }
}

fn normalize_high_fidelity_variables(
    variables: Vec<ScenarioVariable>,
) -> Vec<SimulationHighFidelityVariable> {
    let mut normalized = variables
        .into_iter()
        .filter_map(|variable| {
            let label = variable.label.trim().to_string();
            if label.is_empty() {
                return None;
            }
            Some(SimulationHighFidelityVariable {
                id: if variable.id.trim().is_empty() {
                    slugify(&label)
                } else {
                    variable.id.trim().to_string()
                },
                label,
                current_value: variable.current_value.trim().to_string(),
                proposed_value: variable.proposed_value.trim().to_string(),
                impact: variable.impact.trim().to_string(),
                uncertainty: variable.uncertainty.trim().to_string(),
                pressure_score: round_score(
                    variable.impact_weight * 100.0 - variable.uncertainty_weight * 45.0,
                ),
            })
        })
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        normalized.push(SimulationHighFidelityVariable {
            id: "baseline-pressure".to_string(),
            label: "Baseline pressure".to_string(),
            current_value: "unmodeled".to_string(),
            proposed_value: "deterministic local estimate".to_string(),
            impact: "medium".to_string(),
            uncertainty: "medium".to_string(),
            pressure_score: 40.0,
        });
    }
    normalized
}

fn build_high_fidelity_event_graph(
    base_run: &SimulationLocalSandboxRun,
    variables: &[SimulationHighFidelityVariable],
) -> SimulationHighFidelityEventGraph {
    let mut nodes = Vec::new();
    nodes.push(SimulationHighFidelityGraphNode {
        id: "baseline".to_string(),
        label: "Baseline".to_string(),
        kind: "baseline".to_string(),
    });
    for option_score in &base_run.option_scores {
        nodes.push(SimulationHighFidelityGraphNode {
            id: format!("option-{}", slugify(&option_score.option)),
            label: option_score.option.clone(),
            kind: "option".to_string(),
        });
    }
    for variable in variables {
        nodes.push(SimulationHighFidelityGraphNode {
            id: format!("variable-{}", slugify(&variable.id)),
            label: variable.label.clone(),
            kind: "variable".to_string(),
        });
    }

    let mut edges = Vec::new();
    for option_score in &base_run.option_scores {
        let option_id = format!("option-{}", slugify(&option_score.option));
        edges.push(SimulationHighFidelityGraphEdge {
            from: "baseline".to_string(),
            to: option_id.clone(),
            label: "candidate_path".to_string(),
            weight: option_score.average_score,
        });
        for variable in variables {
            edges.push(SimulationHighFidelityGraphEdge {
                from: format!("variable-{}", slugify(&variable.id)),
                to: option_id.clone(),
                label: "pressure".to_string(),
                weight: variable.pressure_score,
            });
        }
    }

    SimulationHighFidelityEventGraph { nodes, edges }
}

fn build_high_fidelity_heatmap(
    base_run: &SimulationLocalSandboxRun,
    variables: &[SimulationHighFidelityVariable],
) -> Vec<SimulationHighFidelityMetricCell> {
    let variable_pressure = if variables.is_empty() {
        0.0
    } else {
        variables
            .iter()
            .map(|variable| variable.pressure_score)
            .sum::<f64>()
            / variables.len() as f64
    };
    let mut cells = Vec::new();
    for option_score in &base_run.option_scores {
        cells.push(SimulationHighFidelityMetricCell {
            option: option_score.option.clone(),
            metric: "average_score".to_string(),
            value: option_score.average_score,
        });
        cells.push(SimulationHighFidelityMetricCell {
            option: option_score.option.clone(),
            metric: "risk_pressure".to_string(),
            value: round_score(100.0 - option_score.average_score),
        });
        cells.push(SimulationHighFidelityMetricCell {
            option: option_score.option.clone(),
            metric: "variable_pressure".to_string(),
            value: round_score(variable_pressure),
        });
        cells.push(SimulationHighFidelityMetricCell {
            option: option_score.option.clone(),
            metric: "supporting_turns".to_string(),
            value: option_score.turn_count as f64,
        });
    }
    cells
}

fn normalize_local_sandbox_options(options: Vec<String>) -> Vec<String> {
    let mut normalized = Vec::new();

    for option in options.into_iter().map(|option| option.trim().to_string()) {
        if option.is_empty() {
            continue;
        }
        if !normalized.iter().any(|existing| existing == &option) {
            normalized.push(option);
        }
    }

    normalized
}

fn normalize_local_sandbox_agents(
    agents: Vec<SimulationSandboxAgentRequest>,
) -> Vec<SimulationLocalSandboxAgent> {
    let mut normalized = agents
        .into_iter()
        .filter_map(|agent| {
            let role = agent.role.trim().to_string();
            let stance = agent.stance.trim().to_string();
            let fallback_name = if !role.is_empty() {
                format!("{} reviewer", role)
            } else {
                "Sandbox agent".to_string()
            };
            let name = if agent.name.trim().is_empty() {
                fallback_name
            } else {
                agent.name.trim().to_string()
            };
            let role = if role.is_empty() {
                "General reviewer".to_string()
            } else {
                role
            };
            let stance = if stance.is_empty() {
                "balanced".to_string()
            } else {
                stance
            };

            if name.trim().is_empty() {
                None
            } else {
                Some(SimulationLocalSandboxAgent { name, role, stance })
            }
        })
        .collect::<Vec<_>>();

    if normalized.is_empty() {
        normalized = vec![
            SimulationLocalSandboxAgent {
                name: "Casey".to_string(),
                role: "Operations lead".to_string(),
                stance: "balanced".to_string(),
            },
            SimulationLocalSandboxAgent {
                name: "Rin".to_string(),
                role: "Risk reviewer".to_string(),
                stance: "skeptical".to_string(),
            },
            SimulationLocalSandboxAgent {
                name: "Sol".to_string(),
                role: "Growth strategist".to_string(),
                stance: "speed biased".to_string(),
            },
        ];
    }

    normalized
}

fn normalize_local_sandbox_rounds(rounds: Option<usize>) -> usize {
    rounds
        .unwrap_or(LOCAL_SANDBOX_DEFAULT_ROUNDS)
        .clamp(1, LOCAL_SANDBOX_MAX_ROUNDS)
}

fn build_local_sandbox_turns(
    mission_id: &str,
    baseline: &str,
    options: &[String],
    agents: &[SimulationLocalSandboxAgent],
    rounds: usize,
) -> Vec<SimulationLocalSandboxTurn> {
    let mut turns = Vec::new();

    for round in 1..=rounds {
        for agent in agents {
            for option in options {
                let score = score_local_sandbox_turn(mission_id, baseline, option, agent, round);
                turns.push(SimulationLocalSandboxTurn {
                    round,
                    option: option.clone(),
                    agent_name: agent.name.clone(),
                    agent_role: agent.role.clone(),
                    agent_stance: agent.stance.clone(),
                    score,
                    rationale: build_local_sandbox_rationale(baseline, option, agent, round, score),
                });
            }
        }
    }

    turns
}

fn score_local_sandbox_turn(
    mission_id: &str,
    baseline: &str,
    option: &str,
    agent: &SimulationLocalSandboxAgent,
    round: usize,
) -> f64 {
    let baseline_alignment = lexical_alignment_score(baseline, option) * 16.0;
    let role_bias = role_bias_score(&agent.role);
    let stance_bias = stance_bias_score(&agent.stance);
    let option_bias = option_bias_score(option);
    let round_bias = (round as f64 - 1.0) * 1.7;
    let deterministic_bias = deterministic_range_score(&[
        mission_id,
        baseline,
        option,
        &agent.name,
        &agent.role,
        &agent.stance,
        &round.to_string(),
    ]);

    round_score(
        50.0 + baseline_alignment
            + role_bias
            + stance_bias
            + option_bias
            + round_bias
            + deterministic_bias,
    )
}

fn build_local_sandbox_rationale(
    baseline: &str,
    option: &str,
    agent: &SimulationLocalSandboxAgent,
    round: usize,
    score: f64,
) -> String {
    let alignment = match lexical_alignment_score(baseline, option) {
        value if value >= 0.34 => "high",
        value if value >= 0.17 => "medium",
        _ => "low",
    };
    let posture = if option_bias_score(option) >= 0.0 {
        "pushes the mission forward"
    } else {
        "trades speed for caution"
    };
    let stance = if stance_bias_score(&agent.stance) >= 0.0 {
        "supports momentum"
    } else {
        "leans conservative"
    };

    format!(
        "{} acting as {} with a {} stance scored '{}' at {:.1}/100 in round {} because baseline alignment is {}, the option {}, and this agent perspective {}.",
        agent.name, agent.role, agent.stance, option, score, round, alignment, posture, stance
    )
}

fn aggregate_local_sandbox_option_scores(
    options: &[String],
    turns: &[SimulationLocalSandboxTurn],
) -> Vec<SimulationLocalSandboxOptionScore> {
    let mut aggregates = BTreeMap::<String, (f64, usize)>::new();

    for option in options {
        aggregates.insert(option.clone(), (0.0, 0));
    }

    for turn in turns {
        let entry = aggregates.entry(turn.option.clone()).or_insert((0.0, 0));
        entry.0 += turn.score;
        entry.1 += 1;
    }

    aggregates
        .into_iter()
        .map(
            |(option, (total_score, turn_count))| SimulationLocalSandboxOptionScore {
                option,
                average_score: if turn_count == 0 {
                    0.0
                } else {
                    round_score(total_score / turn_count as f64)
                },
                total_score: round_total_score(total_score),
                turn_count,
            },
        )
        .collect()
}

fn derive_local_sandbox_recommendation(
    baseline: &str,
    rounds: usize,
    option_scores: &[SimulationLocalSandboxOptionScore],
    turns: &[SimulationLocalSandboxTurn],
) -> AppResult<SimulationLocalSandboxRecommendation> {
    let recommendation = option_scores
        .iter()
        .max_by(|left, right| {
            left.average_score
                .total_cmp(&right.average_score)
                .then_with(|| left.total_score.total_cmp(&right.total_score))
                .then_with(|| right.option.cmp(&left.option))
        })
        .cloned()
        .ok_or_else(|| AppError::validation("At least one simulation option is required"))?;
    let supporting_turns = turns
        .iter()
        .filter(|turn| turn.option == recommendation.option)
        .count();

    Ok(SimulationLocalSandboxRecommendation {
        option: recommendation.option.clone(),
        average_score: recommendation.average_score,
        rationale: format!(
            "'{}' leads this built-in local engine because it averaged {:.1}/100 across {} deterministic turns over {} rounds against the baseline '{}'.",
            recommendation.option, recommendation.average_score, supporting_turns, rounds, baseline
        ),
    })
}

fn lexical_alignment_score(left: &str, right: &str) -> f64 {
    let left_tokens = tokenize_for_alignment(left);
    let right_tokens = tokenize_for_alignment(right);
    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }

    let shared = left_tokens
        .iter()
        .filter(|token| right_tokens.contains(*token))
        .count();

    shared as f64 / left_tokens.len().min(right_tokens.len()) as f64
}

fn tokenize_for_alignment(value: &str) -> Vec<String> {
    value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter_map(|token| {
            let token = token.trim().to_lowercase();
            if token.is_empty() { None } else { Some(token) }
        })
        .collect()
}

fn role_bias_score(role: &str) -> f64 {
    let normalized = role.to_lowercase();
    let mut score = 0.0;
    let weights = [
        ("finance", -6.0),
        ("risk", -5.0),
        ("security", -4.0),
        ("operations", 3.0),
        ("operator", 3.0),
        ("growth", 6.0),
        ("strategy", 4.0),
        ("strategist", 4.0),
        ("product", 3.0),
        ("engineering", 2.0),
        ("customer", 3.0),
        ("sales", 4.0),
    ];

    for (needle, weight) in weights {
        if normalized.contains(needle) {
            score += weight;
        }
    }

    score
}

fn stance_bias_score(stance: &str) -> f64 {
    let normalized = stance.to_lowercase();
    let mut score = 0.0;
    let weights = [
        ("skeptical", -7.0),
        ("risk", -4.0),
        ("cost", -4.0),
        ("conservative", -5.0),
        ("balanced", 0.0),
        ("quality", 2.0),
        ("optimistic", 4.0),
        ("speed", 5.0),
        ("aggressive", 6.0),
        ("biased", 1.0),
    ];

    for (needle, weight) in weights {
        if normalized.contains(needle) {
            score += weight;
        }
    }

    score
}

fn option_bias_score(option: &str) -> f64 {
    let normalized = option.to_lowercase();
    let mut score = 0.0;
    let weights = [
        ("accelerate", 6.0),
        ("add", 4.0),
        ("increase", 4.0),
        ("protect", 3.0),
        ("support", 2.0),
        ("freeze", -2.0),
        ("delay", -6.0),
        ("reduce", -2.0),
        ("hold", -2.0),
    ];

    for (needle, weight) in weights {
        if normalized.contains(needle) {
            score += weight;
        }
    }

    score
}

fn deterministic_range_score(parts: &[&str]) -> f64 {
    let mut hash = 1_469_598_103_934_665_603_u64;

    for part in parts {
        for byte in part.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(1_099_511_628_211);
        }
        hash ^= 0xff;
        hash = hash.wrapping_mul(1_099_511_628_211);
    }

    (hash % 260) as f64 / 10.0 - 13.0
}

fn record_scenario_governance_handoff(
    db: &Database,
    scenario_run: &SimulationScenarioRun,
) -> AppResult<()> {
    let run_id = Uuid::new_v4().to_string();
    let created_at = scenario_run.created_at.clone();
    let selected_label = selected_option_label(scenario_run);
    let reason = scenario_run
        .recommendation_reason
        .clone()
        .or_else(|| scenario_run.comparison_summary.clone())
        .unwrap_or_else(|| format!("{selected_label} is the current scenario recommendation."));
    let run_summary = format!("Simulation scenario saved: {selected_label}. {reason}");

    db.execute(
        "INSERT INTO runs (id, mission_id, type, status, started_at, finished_at, summary, error_message)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        &[
            &run_id as &dyn rusqlite::ToSql,
            &scenario_run.mission_id,
            &"simulation",
            &"completed",
            &created_at,
            &created_at,
            &run_summary,
            &Option::<String>::None,
        ],
    )?;

    let event_payload = serde_json::json!({
        "scenario_run_id": scenario_run.id,
        "selected_option_id": scenario_run.selected_option_id,
        "recommendation": scenario_run.recommendation,
        "recommendation_reason": scenario_run.recommendation_reason,
        "comparison_summary": scenario_run.comparison_summary,
        "handoff_target": scenario_run.handoff_target,
        "execution_risk_level": scenario_run.execution_risk_level,
        "option_count": scenario_run.option_cards.len(),
        "variable_count": scenario_run.variables.len(),
    })
    .to_string();
    db.execute(
        "INSERT INTO run_events (id, run_id, mission_id, event_type, message, payload_json, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        &[
            &Uuid::new_v4().to_string() as &dyn rusqlite::ToSql,
            &run_id,
            &scenario_run.mission_id,
            &"scenario_saved",
            &format!("Saved scenario recommendation: {selected_label}"),
            &Some(event_payload),
            &created_at,
        ],
    )?;

    if should_create_council_handoff(&scenario_run.handoff_target) {
        db.execute(
            "INSERT INTO council_steps (id, mission_id, run_id, role, status, input_summary, output_summary, review_note, created_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &Uuid::new_v4().to_string() as &dyn rusqlite::ToSql,
                &scenario_run.mission_id,
                &run_id,
                &"Scenario Reviewer",
                &"pending",
                &format!("Review scenario recommendation: {selected_label}"),
                &Option::<String>::None,
                &Some(reason.clone()),
                &created_at,
                &created_at,
            ],
        )?;
    }

    let execution_payload = serde_json::json!({
        "action": "review_scenario_recommendation",
        "scenario_run_id": scenario_run.id,
        "selected_option_id": scenario_run.selected_option_id,
        "selected_option_label": selected_label,
        "recommendation_reason": scenario_run.recommendation_reason,
        "comparison_summary": scenario_run.comparison_summary,
        "handoff_target": scenario_run.handoff_target,
        "execution_risk_level": scenario_run.execution_risk_level,
    })
    .to_string();
    if should_create_execution_handoff(&scenario_run.handoff_target) {
        db.execute(
            "INSERT INTO execution_steps (
            id, mission_id, run_id, title, mode, risk_level, status,
            input_payload, output_summary, created_at, updated_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &Uuid::new_v4().to_string() as &dyn rusqlite::ToSql,
                &scenario_run.mission_id,
                &run_id,
                &format!("Review scenario recommendation: {selected_label}"),
                &"api",
                &scenario_run.execution_risk_level,
                &execution_status_for_risk(&scenario_run.execution_risk_level),
                &Some(execution_payload),
                &Option::<String>::None,
                &created_at,
                &created_at,
            ],
        )?;
    }

    Ok(())
}

fn list_scenario_runs(db: &Database, mission_id: &str) -> AppResult<Vec<SimulationScenarioRun>> {
    db.with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT
                sr.id,
                sr.mission_id,
                m.title,
                sr.baseline,
                sr.options_json,
                sr.recommendation,
                sr.created_at
             FROM scenario_runs sr
             INNER JOIN missions m ON m.id = sr.mission_id
             WHERE sr.mission_id = ?1
             ORDER BY sr.created_at DESC, sr.rowid DESC",
        )?;

        let rows = stmt.query_map(params![mission_id], |row| {
            let options_json: String = row.get(4)?;
            let payload = parse_scenario_payload(&options_json);
            let variables = normalize_variables(payload.variables);
            let mut option_cards = normalize_option_cards(payload.option_cards, Vec::new());
            option_cards = derive_option_scores(option_cards, &variables);
            let stored_recommendation = normalize_optional_text(row.get(5)?);
            let selected_option_id =
                normalize_selected_option_id(payload.selected_option_id, &option_cards)
                    .or_else(|| {
                        stored_recommendation
                            .as_deref()
                            .and_then(|label| find_option_by_label(&option_cards, label))
                            .map(|option| option.id.clone())
                    })
                    .or_else(|| derive_selected_option_id(&option_cards));
            let recommendation = stored_recommendation.or_else(|| {
                selected_option_id
                    .as_ref()
                    .and_then(|option_id| find_option_by_id(&option_cards, option_id))
                    .map(|option| option.label.clone())
                    .or_else(|| derive_recommendation(&option_cards))
            });
            let comparison_summary =
                normalize_optional_text(payload.comparison_summary).or_else(|| {
                    derive_comparison_summary(
                        &option_cards,
                        &variables,
                        selected_option_id.as_deref(),
                    )
                });
            let recommendation_reason = normalize_optional_text(payload.recommendation_reason)
                .or_else(|| {
                    derive_recommendation_reason(&option_cards, selected_option_id.as_deref())
                })
                .or_else(|| comparison_summary.clone());
            let handoff_target = normalize_handoff_target(payload.handoff_target);
            let execution_risk_level = normalize_execution_risk_level(payload.execution_risk_level);
            let options = option_cards
                .iter()
                .map(|option| option.label.clone())
                .collect::<Vec<_>>();

            Ok(SimulationScenarioRun {
                id: row.get(0)?,
                mission_id: row.get(1)?,
                mission_title: row.get(2)?,
                baseline: row.get(3)?,
                options,
                option_cards,
                variables,
                recommendation,
                recommendation_reason,
                comparison_summary,
                selected_option_id,
                handoff_target,
                execution_risk_level,
                created_at: row.get(6)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
    })
}

fn compare_scenario_runs(db: &Database, mission_id: &str) -> AppResult<SimulationComparisonMatrix> {
    let mission_id = mission_id.trim();
    if mission_id.is_empty() {
        return Err(AppError::validation("Mission is required"));
    }

    let repo = MissionRepository::new(db.clone());
    let mission = repo
        .get(mission_id)?
        .ok_or_else(|| AppError::validation("Mission not found"))?;
    let scenario_runs = list_scenario_runs(db, mission_id)?;

    Ok(build_simulation_comparison_matrix(
        mission.id,
        mission.title,
        scenario_runs,
    ))
}

fn build_simulation_comparison_matrix(
    mission_id: String,
    mission_title: String,
    scenario_runs: Vec<SimulationScenarioRun>,
) -> SimulationComparisonMatrix {
    let scenarios = scenario_runs
        .iter()
        .map(build_comparison_scenario)
        .collect::<Vec<_>>();
    let variable_axes = synthesize_variable_axes(&scenario_runs);
    let option_patterns = synthesize_option_patterns(&scenario_runs);
    let path_evolution = synthesize_path_evolution(&scenario_runs);
    let summary = build_comparison_summary(&scenarios, &variable_axes, &option_patterns);

    SimulationComparisonMatrix {
        mission_id,
        mission_title,
        scenario_count: scenarios.len(),
        scenarios,
        variable_axes,
        option_patterns,
        path_evolution,
        summary,
    }
}

fn build_comparison_scenario(run: &SimulationScenarioRun) -> SimulationComparisonScenario {
    SimulationComparisonScenario {
        scenario_run_id: run.id.clone(),
        created_at: run.created_at.clone(),
        selected_option_id: run.selected_option_id.clone(),
        selected_option_label: selected_option_label(run),
        recommendation: run.recommendation.clone(),
        comparison_summary: scenario_narrative(run),
        average_option_score: average_option_score(&run.option_cards),
    }
}

fn synthesize_variable_axes(
    scenario_runs: &[SimulationScenarioRun],
) -> Vec<SimulationVariableAxis> {
    #[derive(Default)]
    struct VariableAxisAccumulator {
        appearance_count: usize,
        values: Vec<String>,
        impacts: Vec<String>,
        uncertainties: Vec<String>,
    }

    let mut axes = BTreeMap::<String, VariableAxisAccumulator>::new();

    for run in scenario_runs {
        for variable in &run.variables {
            let entry = axes.entry(variable.label.clone()).or_default();
            entry.appearance_count += 1;
            push_unique(&mut entry.values, format_variable_transition(variable));
            push_unique(&mut entry.impacts, variable.impact.clone());
            push_unique(&mut entry.uncertainties, variable.uncertainty.clone());
        }
    }

    let mut collected = axes.into_iter().collect::<Vec<_>>();
    collected.sort_by(|(left_label, left), (right_label, right)| {
        right
            .appearance_count
            .cmp(&left.appearance_count)
            .then_with(|| left_label.cmp(right_label))
    });

    collected
        .into_iter()
        .map(|(label, axis)| SimulationVariableAxis {
            label,
            values: axis.values,
            impacts: axis.impacts,
            uncertainties: axis.uncertainties,
        })
        .collect()
}

fn synthesize_option_patterns(
    scenario_runs: &[SimulationScenarioRun],
) -> Vec<SimulationOptionPattern> {
    #[derive(Default)]
    struct OptionPatternAccumulator {
        appearance_count: usize,
        selected_count: usize,
        score_total: f64,
        latest_time_horizon: Option<String>,
    }

    let mut patterns = BTreeMap::<String, OptionPatternAccumulator>::new();

    for run in scenario_runs {
        let selected_label = selected_option_label_opt(run);
        for option in &run.option_cards {
            let entry = patterns.entry(option.label.clone()).or_default();
            entry.appearance_count += 1;
            entry.score_total += option.score;
            if entry.latest_time_horizon.is_none() {
                entry.latest_time_horizon = Some(option.time_horizon.clone());
            }
            if selected_label
                .as_deref()
                .is_some_and(|label| label == option.label)
            {
                entry.selected_count += 1;
            }
        }
    }

    let mut collected = patterns
        .into_iter()
        .map(|(label, pattern)| SimulationOptionPattern {
            label,
            appearance_count: pattern.appearance_count,
            selected_count: pattern.selected_count,
            average_score: round_score(pattern.score_total / pattern.appearance_count as f64),
            latest_time_horizon: pattern
                .latest_time_horizon
                .unwrap_or_else(|| default_time_horizon().to_string()),
        })
        .collect::<Vec<_>>();

    collected.sort_by(|left, right| {
        right
            .selected_count
            .cmp(&left.selected_count)
            .then_with(|| right.appearance_count.cmp(&left.appearance_count))
            .then_with(|| {
                right
                    .average_score
                    .partial_cmp(&left.average_score)
                    .unwrap_or(Ordering::Equal)
            })
            .then_with(|| left.label.cmp(&right.label))
    });

    collected
}

fn synthesize_path_evolution(
    scenario_runs: &[SimulationScenarioRun],
) -> Vec<SimulationPathEvolutionStep> {
    scenario_runs
        .iter()
        .map(|run| SimulationPathEvolutionStep {
            scenario_run_id: run.id.clone(),
            created_at: run.created_at.clone(),
            selected_option_label: selected_option_label(run),
            score: selected_option_score(run),
            variable_changes: sort_variables(&run.variables)
                .into_iter()
                .map(format_variable_change)
                .collect(),
            narrative: scenario_narrative(run),
        })
        .collect()
}

fn build_comparison_summary(
    scenarios: &[SimulationComparisonScenario],
    variable_axes: &[SimulationVariableAxis],
    option_patterns: &[SimulationOptionPattern],
) -> String {
    match scenarios {
        [] => "No saved scenarios yet for this mission.".to_string(),
        [_] => "Only one saved scenario is available, so the comparison captures the current path without cross-scenario drift.".to_string(),
        _ => {
            let latest = &scenarios[0];
            let earliest = scenarios.last().expect("comparison summary requires scenarios");
            let path_summary = if latest.selected_option_label == earliest.selected_option_label {
                format!(
                    "the selected path stayed on {}",
                    latest.selected_option_label
                )
            } else {
                format!(
                    "the selected path moved from {} to {}",
                    earliest.selected_option_label, latest.selected_option_label
                )
            };

            let variable_summary = if variable_axes.is_empty() {
                "No scenario variables shifted across the saved runs.".to_string()
            } else {
                let highlighted = variable_axes
                    .iter()
                    .filter(|axis| axis.values.len() > 1)
                    .take(2)
                    .map(|axis| axis.label.clone())
                    .collect::<Vec<_>>();
                let labels = if highlighted.is_empty() {
                    variable_axes
                        .iter()
                        .take(2)
                        .map(|axis| axis.label.clone())
                        .collect::<Vec<_>>()
                } else {
                    highlighted
                };

                format!("The main moving variables were {}.", labels.join(" and "))
            };

            let option_summary = option_patterns
                .first()
                .map(|pattern| {
                    format!(
                        "{} appeared {} time(s) and was selected {} time(s).",
                        pattern.label, pattern.appearance_count, pattern.selected_count
                    )
                })
                .unwrap_or_else(|| "No option pattern data was available.".to_string());

            format!(
                "Across {} scenarios, {}. {} {}",
                scenarios.len(),
                path_summary,
                variable_summary,
                option_summary,
            )
        }
    }
}

fn average_option_score(option_cards: &[ScenarioOptionCard]) -> f64 {
    if option_cards.is_empty() {
        return 0.0;
    }

    round_score(
        option_cards.iter().map(|option| option.score).sum::<f64>() / option_cards.len() as f64,
    )
}

fn selected_option_label(run: &SimulationScenarioRun) -> String {
    selected_option_label_opt(run).unwrap_or_else(|| "No selected option".to_string())
}

fn selected_option_label_opt(run: &SimulationScenarioRun) -> Option<String> {
    selected_option_for_run(run)
        .map(|option| option.label.clone())
        .or_else(|| run.recommendation.clone())
        .or_else(|| run.options.first().cloned())
}

fn selected_option_score(run: &SimulationScenarioRun) -> f64 {
    selected_option_for_run(run)
        .map(|option| round_score(option.score))
        .unwrap_or_default()
}

fn selected_option_for_run(run: &SimulationScenarioRun) -> Option<&ScenarioOptionCard> {
    run.selected_option_id
        .as_deref()
        .and_then(|option_id| find_option_by_id(&run.option_cards, option_id))
        .or_else(|| {
            run.recommendation
                .as_deref()
                .and_then(|label| find_option_by_label(&run.option_cards, label))
        })
        .or_else(|| {
            run.option_cards
                .iter()
                .max_by(|left, right| option_rank_key(left).cmp(&option_rank_key(right)))
        })
}

fn scenario_narrative(run: &SimulationScenarioRun) -> String {
    run.comparison_summary
        .clone()
        .filter(|summary| !summary.is_empty())
        .unwrap_or_else(|| {
            let selected_option_label = selected_option_label(run);
            if run.variables.is_empty() {
                format!("{selected_option_label} remained the active recommendation.")
            } else {
                format!(
                    "{selected_option_label} remained the active recommendation while {} shifted.",
                    summarize_variable_labels(&run.variables)
                )
            }
        })
}

fn summarize_variable_labels(variables: &[ScenarioVariable]) -> String {
    let labels = sort_variables(variables)
        .into_iter()
        .take(2)
        .map(|variable| variable.label.clone())
        .collect::<Vec<_>>();

    match labels.as_slice() {
        [] => "no tracked variables".to_string(),
        [label] => label.clone(),
        [first, second] => format!("{first} and {second}"),
        _ => labels.join(", "),
    }
}

fn sort_variables(variables: &[ScenarioVariable]) -> Vec<&ScenarioVariable> {
    let mut variables = variables.iter().collect::<Vec<_>>();
    variables.sort_by(|left, right| {
        level_rank(&right.impact)
            .cmp(&level_rank(&left.impact))
            .then_with(|| level_rank(&left.uncertainty).cmp(&level_rank(&right.uncertainty)))
            .then_with(|| left.label.cmp(&right.label))
    });
    variables
}

fn format_variable_transition(variable: &ScenarioVariable) -> String {
    format!("{} -> {}", variable.current_value, variable.proposed_value)
}

fn format_variable_change(variable: &ScenarioVariable) -> String {
    format!(
        "{}: {} -> {}",
        variable.label, variable.current_value, variable.proposed_value
    )
}

fn push_unique(values: &mut Vec<String>, value: String) {
    if !value.is_empty() && !values.iter().any(|current| current == &value) {
        values.push(value);
    }
}

fn increment_count(counts: &mut [(String, usize)], key: &str) {
    if let Some((_, count)) = counts.iter_mut().find(|(current, _)| current == key) {
        *count += 1;
    }
}

fn normalize_option_cards(
    option_cards: Vec<ScenarioOptionCard>,
    legacy_options: Vec<String>,
) -> Vec<ScenarioOptionCard> {
    let mut cards = option_cards
        .into_iter()
        .filter_map(normalize_option_card)
        .collect::<Vec<_>>();

    if cards.is_empty() {
        cards = legacy_options
            .into_iter()
            .map(|option| option.trim().to_string())
            .filter(|option| !option.is_empty())
            .enumerate()
            .map(|(index, label)| ScenarioOptionCard {
                id: format!("option-{}", index + 1),
                label,
                assumptions: Vec::new(),
                expected_benefits: Vec::new(),
                risks: Vec::new(),
                projected_outcomes: Vec::new(),
                score: 0.0,
                time_horizon: default_time_horizon().to_string(),
                confidence: "medium".to_string(),
            })
            .collect();
    }

    cards
}

fn normalize_option_card(mut option: ScenarioOptionCard) -> Option<ScenarioOptionCard> {
    option.id = option.id.trim().to_string();
    option.label = option.label.trim().to_string();
    option.assumptions = normalize_lines(option.assumptions);
    option.expected_benefits = normalize_lines(option.expected_benefits);
    option.risks = normalize_lines(option.risks);
    option.projected_outcomes = normalize_lines(option.projected_outcomes);
    if option.projected_outcomes.is_empty() {
        option.projected_outcomes = derive_projected_outcomes(&option);
    }
    option.score = normalize_score(option.score);
    option.time_horizon = normalize_time_horizon(&option.time_horizon).to_string();
    option.confidence = normalize_confidence(&option.confidence).to_string();

    if option.label.is_empty() {
        return None;
    }
    if option.id.is_empty() {
        option.id = format!("option-{}", slugify(&option.label));
    }

    Some(option)
}

fn normalize_variables(variables: Vec<ScenarioVariable>) -> Vec<ScenarioVariable> {
    variables
        .into_iter()
        .filter_map(normalize_variable)
        .collect()
}

fn normalize_variable(mut variable: ScenarioVariable) -> Option<ScenarioVariable> {
    variable.id = variable.id.trim().to_string();
    variable.label = variable.label.trim().to_string();
    variable.current_value = variable.current_value.trim().to_string();
    variable.proposed_value = variable.proposed_value.trim().to_string();
    variable.impact_weight = normalize_weight(variable.impact_weight, &variable.impact);
    variable.uncertainty_weight =
        normalize_weight(variable.uncertainty_weight, &variable.uncertainty);
    variable.impact = level_from_weight(variable.impact_weight).to_string();
    variable.uncertainty = level_from_weight(variable.uncertainty_weight).to_string();

    if variable.label.is_empty()
        || variable.current_value.is_empty()
        || variable.proposed_value.is_empty()
    {
        return None;
    }
    if variable.id.is_empty() {
        variable.id = format!("variable-{}", slugify(&variable.label));
    }

    Some(variable)
}

fn normalize_lines(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalize_confidence(value: &str) -> &'static str {
    match value.trim().to_lowercase().as_str() {
        "low" => "low",
        "high" => "high",
        _ => "medium",
    }
}

fn normalize_level(value: &str) -> &'static str {
    match value.trim().to_lowercase().as_str() {
        "low" => "low",
        "high" => "high",
        _ => "medium",
    }
}

fn normalize_weight(value: f64, fallback_level: &str) -> f64 {
    if value.is_finite() && value > 0.0 {
        value.clamp(0.0, 100.0)
    } else {
        weight_for_level(normalize_level(fallback_level))
    }
}

fn weight_for_level(level: &str) -> f64 {
    match level {
        "high" => 85.0,
        "low" => 25.0,
        _ => 55.0,
    }
}

fn level_from_weight(value: f64) -> &'static str {
    if value >= 67.0 {
        "high"
    } else if value <= 33.0 {
        "low"
    } else {
        "medium"
    }
}

fn normalize_handoff_target(value: Option<String>) -> String {
    match normalize_optional_text(value)
        .unwrap_or_else(|| "council_and_execution".to_string())
        .as_str()
    {
        "council_only" => "council_only".to_string(),
        "execution_only" => "execution_only".to_string(),
        "timeline_only" => "timeline_only".to_string(),
        _ => "council_and_execution".to_string(),
    }
}

fn normalize_execution_risk_level(value: Option<String>) -> String {
    match normalize_optional_text(value)
        .unwrap_or_else(|| "medium".to_string())
        .as_str()
    {
        "low" => "low".to_string(),
        "high" => "high".to_string(),
        _ => "medium".to_string(),
    }
}

fn should_create_council_handoff(handoff_target: &str) -> bool {
    matches!(handoff_target, "council_and_execution" | "council_only")
}

fn should_create_execution_handoff(handoff_target: &str) -> bool {
    matches!(handoff_target, "council_and_execution" | "execution_only")
}

fn execution_status_for_risk(risk_level: &str) -> &'static str {
    if risk_level == "high" {
        "awaiting_approval"
    } else {
        "pending"
    }
}

fn normalize_time_horizon(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        default_time_horizon()
    } else {
        trimmed
    }
}

fn default_time_horizon() -> &'static str {
    "near-term"
}

fn normalize_score(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value.clamp(0.0, 100.0)
    } else {
        0.0
    }
}

fn derive_recommendation(option_cards: &[ScenarioOptionCard]) -> Option<String> {
    option_cards
        .iter()
        .max_by(|left, right| option_rank_key(left).cmp(&option_rank_key(right)))
        .map(|option| option.label.clone())
}

fn derive_recommendation_reason(
    option_cards: &[ScenarioOptionCard],
    selected_option_id: Option<&str>,
) -> Option<String> {
    let selected = selected_option_id
        .and_then(|option_id| find_option_by_id(option_cards, option_id))
        .or_else(|| {
            option_cards
                .iter()
                .max_by(|left, right| option_rank_key(left).cmp(&option_rank_key(right)))
        })?;
    let strongest_outcome = selected
        .projected_outcomes
        .first()
        .or_else(|| selected.expected_benefits.first())
        .map(String::as_str)
        .unwrap_or("it creates the clearest next move");

    Some(format!(
        "Recommend {} because it scores {:.1}/100, fits a {} decision window, and shows the strongest immediate outcome: {}",
        selected.label, selected.score, selected.time_horizon, strongest_outcome
    ))
}

fn confidence_rank(value: &str) -> usize {
    match value {
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn option_rank_key(option: &ScenarioOptionCard) -> (i64, usize, usize, usize, Reverse<String>) {
    (
        scaled_score(option.score),
        confidence_rank(&option.confidence),
        option.expected_benefits.len() + option.projected_outcomes.len(),
        usize::MAX - option.risks.len(),
        Reverse(option.id.clone()),
    )
}

fn scaled_score(value: f64) -> i64 {
    (value * 10.0).round() as i64
}

fn parse_scenario_payload(options_json: &str) -> ScenarioRunPayload {
    serde_json::from_str::<ScenarioRunPayload>(options_json).unwrap_or_else(|_| {
        serde_json::from_str::<Vec<ScenarioOptionCard>>(options_json)
            .map(|option_cards| ScenarioRunPayload {
                option_cards: normalize_option_cards(option_cards, Vec::new()),
                variables: Vec::new(),
                recommendation_reason: None,
                comparison_summary: None,
                selected_option_id: None,
                handoff_target: None,
                execution_risk_level: None,
            })
            .or_else(|_| {
                serde_json::from_str::<Vec<String>>(options_json).map(|options| {
                    ScenarioRunPayload {
                        option_cards: normalize_option_cards(Vec::new(), options),
                        variables: Vec::new(),
                        recommendation_reason: None,
                        comparison_summary: None,
                        selected_option_id: None,
                        handoff_target: None,
                        execution_risk_level: None,
                    }
                })
            })
            .unwrap_or(ScenarioRunPayload {
                option_cards: Vec::new(),
                variables: Vec::new(),
                recommendation_reason: None,
                comparison_summary: None,
                selected_option_id: None,
                handoff_target: None,
                execution_risk_level: None,
            })
    })
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value
        .map(|current| current.trim().to_string())
        .filter(|current| !current.is_empty())
}

fn normalize_selected_option_id(
    selected_option_id: Option<String>,
    option_cards: &[ScenarioOptionCard],
) -> Option<String> {
    normalize_optional_text(selected_option_id)
        .and_then(|normalized| find_option_by_id(option_cards, &normalized).map(|_| normalized))
}

fn derive_selected_option_id(option_cards: &[ScenarioOptionCard]) -> Option<String> {
    option_cards
        .iter()
        .max_by(|left, right| option_rank_key(left).cmp(&option_rank_key(right)))
        .map(|option| option.id.clone())
}

fn find_option_by_id<'a>(
    option_cards: &'a [ScenarioOptionCard],
    option_id: &str,
) -> Option<&'a ScenarioOptionCard> {
    option_cards.iter().find(|option| option.id == option_id)
}

fn find_option_by_label<'a>(
    option_cards: &'a [ScenarioOptionCard],
    label: &str,
) -> Option<&'a ScenarioOptionCard> {
    option_cards
        .iter()
        .find(|option| option.label.eq_ignore_ascii_case(label))
}

fn derive_projected_outcomes(option: &ScenarioOptionCard) -> Vec<String> {
    let mut outcomes = option
        .expected_benefits
        .iter()
        .take(2)
        .map(|benefit| format!("Expected upside: {benefit}"))
        .collect::<Vec<_>>();

    if outcomes.is_empty() {
        outcomes = option
            .risks
            .iter()
            .take(1)
            .map(|risk| format!("Primary tradeoff: {risk}"))
            .collect();
    }

    outcomes
}

fn derive_option_scores(
    mut option_cards: Vec<ScenarioOptionCard>,
    variables: &[ScenarioVariable],
) -> Vec<ScenarioOptionCard> {
    if option_cards.is_empty() {
        return option_cards;
    }
    if option_cards.iter().all(|option| option.score > 0.0) {
        for option in &mut option_cards {
            option.score = round_score(option.score.clamp(0.0, 100.0));
        }
        return option_cards;
    }

    let aggregate_variable_pressure = variables.iter().map(variable_pressure).sum::<f64>();
    let average_variable_pressure = if variables.is_empty() {
        0.0
    } else {
        aggregate_variable_pressure / variables.len() as f64
    };

    let raw_scores = option_cards
        .iter()
        .map(|option| raw_option_score(option, average_variable_pressure))
        .collect::<Vec<_>>();
    let normalized_scores = normalize_raw_scores(&raw_scores);

    for (option, score) in option_cards.iter_mut().zip(normalized_scores) {
        option.score = score;
    }

    option_cards
}

fn raw_option_score(option: &ScenarioOptionCard, average_variable_pressure: f64) -> f64 {
    let confidence_weight = match option.confidence.as_str() {
        "high" => 10.0,
        "medium" => 4.0,
        "low" => -4.0,
        _ => 0.0,
    };
    let posture = option_posture(option);

    50.0 + confidence_weight
        + option.expected_benefits.len() as f64 * 8.0
        + option.projected_outcomes.len() as f64 * 5.0
        + option.assumptions.len() as f64 * 2.0
        - option.risks.len() as f64 * 7.0
        + posture * average_variable_pressure * 14.0
}

fn normalize_raw_scores(raw_scores: &[f64]) -> Vec<f64> {
    if raw_scores.is_empty() {
        return Vec::new();
    }

    let min_score = raw_scores.iter().copied().fold(f64::INFINITY, f64::min);
    let max_score = raw_scores.iter().copied().fold(f64::NEG_INFINITY, f64::max);

    if (max_score - min_score).abs() < f64::EPSILON {
        return vec![100.0; raw_scores.len()];
    }

    raw_scores
        .iter()
        .map(|score| round_score(((score - min_score) / (max_score - min_score)) * 100.0))
        .collect()
}

fn round_score(value: f64) -> f64 {
    ((value * 10.0).round() / 10.0).clamp(0.0, 100.0)
}

fn round_total_score(value: f64) -> f64 {
    (value * 10.0).round() / 10.0
}

fn variable_pressure(variable: &ScenarioVariable) -> f64 {
    let impact_weight = (variable.impact_weight / 100.0).clamp(0.0, 1.0);
    let uncertainty_penalty = ((variable.uncertainty_weight / 100.0) * 0.45).clamp(0.0, 0.45);

    (impact_weight - uncertainty_penalty).max(0.1_f64)
}

fn option_posture(option: &ScenarioOptionCard) -> f64 {
    let text = format!(
        "{} {} {} {}",
        option.label,
        option.expected_benefits.join(" "),
        option.projected_outcomes.join(" "),
        option.assumptions.join(" "),
    )
    .to_lowercase();

    let assertive_terms = [
        "accelerate",
        "add",
        "expand",
        "increase",
        "improve",
        "protect",
        "raise",
        "hire",
        "advance",
        "before",
    ];
    let cautious_terms = [
        "delay", "defer", "freeze", "wait", "pause", "reduce", "absorb", "hold", "slip", "push",
    ];

    let assertive_hits = assertive_terms
        .iter()
        .filter(|term| text.contains(**term))
        .count() as f64;
    let cautious_hits = cautious_terms
        .iter()
        .filter(|term| text.contains(**term))
        .count() as f64;

    (assertive_hits - cautious_hits).clamp(-2.0, 2.0) / 2.0
}

fn derive_comparison_summary(
    option_cards: &[ScenarioOptionCard],
    variables: &[ScenarioVariable],
    selected_option_id: Option<&str>,
) -> Option<String> {
    let mut ranked = option_cards.iter().collect::<Vec<_>>();
    ranked.sort_by_key(|option| Reverse(option_rank_key(option)));

    let selected = selected_option_id
        .and_then(|option_id| ranked.iter().copied().find(|option| option.id == option_id))
        .or_else(|| ranked.first().copied())?;
    let runner_up = ranked.into_iter().find(|option| option.id != selected.id);
    let variable_summary = summarize_variables(variables);

    let summary = if let Some(next_best) = runner_up {
        format!(
            "{} leads at {:.1}/100 versus {} at {:.1}/100 over a {} horizon, combining {} expected benefit(s) with {} modeled risk(s).{}",
            selected.label,
            selected.score,
            next_best.label,
            next_best.score,
            selected.time_horizon,
            selected.expected_benefits.len() + selected.projected_outcomes.len(),
            selected.risks.len(),
            variable_summary,
        )
    } else {
        format!(
            "{} is the only modeled option and scores {:.1}/100 for the {} horizon.{}",
            selected.label, selected.score, selected.time_horizon, variable_summary,
        )
    };

    Some(summary)
}

fn summarize_variables(variables: &[ScenarioVariable]) -> String {
    if variables.is_empty() {
        return String::new();
    }

    let mut ranked = variables.iter().collect::<Vec<_>>();
    ranked.sort_by_key(|variable| {
        (
            usize::MAX - level_rank(&variable.impact),
            level_rank(&variable.uncertainty),
        )
    });

    let summary = ranked
        .into_iter()
        .take(2)
        .map(|variable| {
            format!(
                "{} ({} impact, {} uncertainty)",
                variable.label, variable.impact, variable.uncertainty
            )
        })
        .collect::<Vec<_>>()
        .join("; ");

    format!(" Key swing variables: {summary}.")
}

fn level_rank(value: &str) -> usize {
    match value {
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn slugify(value: &str) -> String {
    let mut slug = String::new();
    let mut last_was_dash = false;

    for ch in value.chars().flat_map(|ch| ch.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let trimmed = slug.trim_matches('-').to_string();
    if trimmed.is_empty() {
        "item".to_string()
    } else {
        trimmed
    }
}

fn include_in_recent_runs(run_type: &RunType) -> bool {
    matches!(
        run_type,
        RunType::Research | RunType::Simulation | RunType::Execution
    )
}

fn run_type_key(value: &RunType) -> &'static str {
    match value {
        RunType::Research => "research",
        RunType::Simulation => "simulation",
        RunType::Council => "council",
        RunType::Execution => "execution",
        RunType::Growth => "growth",
    }
}

fn run_status_key(value: &RunStatus) -> &'static str {
    match value {
        RunStatus::Queued => "queued",
        RunStatus::Running => "running",
        RunStatus::Completed => "completed",
        RunStatus::Failed => "failed",
        RunStatus::Cancelled => "cancelled",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::{CreateMissionInput, Database};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct StoredScenarioRunPayload {
        #[serde(default)]
        option_cards: Vec<ScenarioOptionCard>,
        #[serde(default)]
        variables: Vec<ScenarioVariable>,
        recommendation_reason: Option<String>,
        comparison_summary: Option<String>,
        selected_option_id: Option<String>,
    }

    fn sample_input(title: &str) -> CreateMissionInput {
        CreateMissionInput {
            title: title.to_string(),
            goal: format!("{title} goal"),
            constraints: vec![],
            success_criteria: vec!["deliver result".to_string()],
            priority: MissionPriority::Medium,
        }
    }

    #[test]
    fn build_simulation_overview_aggregates_real_run_data_and_recent_items() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());

        let alpha = repo
            .create(sample_input("Alpha mission"))
            .expect("first mission should be created");
        let beta = repo
            .create(sample_input("Beta mission"))
            .expect("second mission should be created");

        update_mission_status(
            &db,
            &alpha.id,
            "simulating",
            "2026-04-21T12:00:00Z",
            "medium",
        );
        update_mission_status(&db, &beta.id, "executing", "2026-04-22T08:00:00Z", "high");

        insert_run(
            &db,
            "run-sim",
            &alpha.id,
            "simulation",
            "completed",
            Some("2026-04-20T10:00:00Z"),
            Some("2026-04-20T10:30:00Z"),
            Some("Simulation completed"),
            None,
        );
        insert_run(
            &db,
            "run-research",
            &alpha.id,
            "research",
            "running",
            Some("2026-04-21T09:00:00Z"),
            None,
            Some("Research in progress"),
            None,
        );
        insert_run(
            &db,
            "run-execution",
            &beta.id,
            "execution",
            "failed",
            Some("2026-04-22T07:00:00Z"),
            Some("2026-04-22T07:15:00Z"),
            Some("Execution failed"),
            Some("tool exited with 1"),
        );
        insert_run(
            &db,
            "run-council",
            &beta.id,
            "council",
            "completed",
            Some("2026-04-19T08:00:00Z"),
            Some("2026-04-19T08:20:00Z"),
            Some("Council completed"),
            None,
        );

        let overview = build_simulation_overview(&repo).expect("overview should build");

        assert_eq!(
            overview.summary,
            SimulationOverviewSummary {
                total_missions: 2,
                active_missions: 2,
                simulating_missions: 1,
                missions_with_runs: 2,
                total_runs: 4,
                simulation_runs: 1,
            }
        );
        assert_eq!(count_for(&overview.counts_by_type, "simulation"), 1);
        assert_eq!(count_for(&overview.counts_by_type, "research"), 1);
        assert_eq!(count_for(&overview.counts_by_type, "execution"), 1);
        assert_eq!(count_for(&overview.counts_by_type, "council"), 1);
        assert_eq!(count_for(&overview.counts_by_type, "growth"), 0);
        assert_eq!(count_for(&overview.counts_by_status, "queued"), 0);
        assert_eq!(count_for(&overview.counts_by_status, "running"), 1);
        assert_eq!(count_for(&overview.counts_by_status, "completed"), 2);
        assert_eq!(count_for(&overview.counts_by_status, "failed"), 1);
        assert_eq!(count_for(&overview.counts_by_status, "cancelled"), 0);

        let recent_ids = overview
            .recent_runs
            .iter()
            .map(|run| run.run_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(recent_ids, vec!["run-execution", "run-research", "run-sim"]);
        assert_eq!(overview.recent_runs[0].mission_title, "Beta mission");
        assert_eq!(
            overview.recent_runs[0].mission_status,
            MissionStatus::Executing
        );
        assert_eq!(
            overview.recent_runs[0].mission_priority,
            MissionPriority::High
        );
        assert_eq!(
            overview.recent_runs[0].summary.as_deref(),
            Some("Execution failed")
        );
    }

    #[test]
    fn build_simulation_overview_reports_real_counts_when_no_simulation_runs_exist() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());

        let mission = repo
            .create(sample_input("Research mission"))
            .expect("mission should be created");
        update_mission_status(
            &db,
            &mission.id,
            "planning",
            "2026-04-22T10:00:00Z",
            "medium",
        );
        insert_run(
            &db,
            "run-research",
            &mission.id,
            "research",
            "completed",
            Some("2026-04-22T08:00:00Z"),
            Some("2026-04-22T08:20:00Z"),
            Some("Research completed"),
            None,
        );
        insert_run(
            &db,
            "run-execution",
            &mission.id,
            "execution",
            "queued",
            Some("2026-04-22T09:00:00Z"),
            None,
            Some("Execution queued"),
            None,
        );

        let overview = build_simulation_overview(&repo).expect("overview should build");

        assert_eq!(overview.summary.total_missions, 1);
        assert_eq!(overview.summary.total_runs, 2);
        assert_eq!(overview.summary.simulation_runs, 0);
        assert_eq!(count_for(&overview.counts_by_type, "simulation"), 0);
        assert_eq!(count_for(&overview.counts_by_type, "research"), 1);
        assert_eq!(count_for(&overview.counts_by_type, "execution"), 1);
        assert_eq!(overview.recent_runs.len(), 2);
        assert!(
            overview
                .recent_runs
                .iter()
                .all(|run| matches!(run.run_type, RunType::Research | RunType::Execution))
        );
    }

    #[test]
    fn create_scenario_run_persists_structured_record_for_a_mission() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Scenario mission"))
            .expect("mission should be created");

        let created = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: mission.id.clone(),
                baseline: "Keep current staffing plan".to_string(),
                options: vec![
                    "Increase budget by 10%".to_string(),
                    "Delay rollout by two weeks".to_string(),
                ],
                option_cards: vec![],
                variables: vec![],
                recommendation: Some("Increase budget by 10%".to_string()),
                recommendation_reason: None,
                comparison_summary: None,
                selected_option_id: None,
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("scenario run should be created");

        assert_eq!(created.mission_id, mission.id);
        assert_eq!(created.mission_title, "Scenario mission");
        assert_eq!(created.baseline, "Keep current staffing plan");
        assert_eq!(
            created.options,
            vec![
                "Increase budget by 10%".to_string(),
                "Delay rollout by two weeks".to_string()
            ]
        );
        assert_eq!(
            created.recommendation.as_deref(),
            Some("Increase budget by 10%")
        );
        assert!(!created.id.is_empty());
        assert!(!created.created_at.is_empty());

        let persisted = db
            .query_row(
                "SELECT baseline, options_json, recommendation FROM scenario_runs WHERE id = ?1",
                &[&created.id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .expect("scenario run should be persisted");

        assert_eq!(persisted.0, "Keep current staffing plan");
        let persisted = serde_json::from_str::<StoredScenarioRunPayload>(&persisted.1)
            .expect("options json should parse as payload");
        assert_eq!(
            persisted
                .option_cards
                .into_iter()
                .map(|option| option.label)
                .collect::<Vec<_>>(),
            vec![
                "Increase budget by 10%".to_string(),
                "Delay rollout by two weeks".to_string()
            ]
        );
        assert_eq!(persisted.variables.len(), 0);
        assert_eq!(persisted.selected_option_id.as_deref(), Some("option-1"));
        assert_eq!(
            created.recommendation.as_deref(),
            Some("Increase budget by 10%")
        );
    }

    #[test]
    fn create_scenario_run_accepts_structured_option_cards_and_derives_recommendation() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Structured scenario mission"))
            .expect("mission should be created");

        let created = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: mission.id.clone(),
                baseline: "Keep current plan".to_string(),
                options: vec![],
                option_cards: vec![
                    ScenarioOptionCard {
                        id: "option-low".to_string(),
                        label: "Delay rollout".to_string(),
                        assumptions: vec!["Market remains stable".to_string()],
                        expected_benefits: vec!["More QA time".to_string()],
                        risks: vec!["Competitor moves first".to_string()],
                        projected_outcomes: vec![],
                        score: 0.0,
                        time_horizon: String::new(),
                        confidence: "medium".to_string(),
                    },
                    ScenarioOptionCard {
                        id: "option-high".to_string(),
                        label: "Increase budget".to_string(),
                        assumptions: vec!["Extra budget is approved".to_string()],
                        expected_benefits: vec!["Higher launch quality".to_string()],
                        risks: vec!["Higher burn".to_string()],
                        projected_outcomes: vec![],
                        score: 0.0,
                        time_horizon: String::new(),
                        confidence: "high".to_string(),
                    },
                ],
                variables: vec![],
                recommendation: None,
                recommendation_reason: None,
                comparison_summary: None,
                selected_option_id: None,
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("scenario run should be created");

        assert_eq!(
            created.options,
            vec!["Delay rollout".to_string(), "Increase budget".to_string()]
        );
        assert_eq!(created.option_cards.len(), 2);
        assert_eq!(created.option_cards[1].confidence, "high");
        assert_eq!(created.recommendation.as_deref(), Some("Increase budget"));
        assert_eq!(created.selected_option_id.as_deref(), Some("option-high"));

        let persisted_json = db
            .query_row(
                "SELECT options_json FROM scenario_runs WHERE id = ?1",
                &[&created.id],
                |row| row.get::<_, String>(0),
            )
            .expect("scenario options should persist");
        let persisted = serde_json::from_str::<StoredScenarioRunPayload>(&persisted_json)
            .expect("options json should store structured payload");
        assert_eq!(persisted.option_cards[0].label, "Delay rollout");
        assert_eq!(
            persisted.option_cards[1].expected_benefits,
            vec!["Higher launch quality"]
        );
        assert_eq!(persisted.selected_option_id.as_deref(), Some("option-high"));
    }

    #[test]
    fn create_scenario_run_persists_variables_and_comparison_metadata() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Variable scenario mission"))
            .expect("mission should be created");

        let created = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: mission.id.clone(),
                baseline: "Keep the current launch team and timeline".to_string(),
                options: vec![],
                option_cards: vec![
                    ScenarioOptionCard {
                        id: "hire-contractors".to_string(),
                        label: "Add contract analysts".to_string(),
                        assumptions: vec!["Contractors can start next sprint".to_string()],
                        expected_benefits: vec!["Increase research throughput".to_string()],
                        risks: vec!["Onboarding overhead".to_string()],
                        projected_outcomes: vec![
                            "Reduce backlog by roughly one sprint".to_string(),
                        ],
                        score: 0.0,
                        time_horizon: "next 30 days".to_string(),
                        confidence: "high".to_string(),
                    },
                    ScenarioOptionCard {
                        id: "freeze-scope".to_string(),
                        label: "Freeze scope and absorb delays".to_string(),
                        assumptions: vec!["Stakeholders accept reduced scope".to_string()],
                        expected_benefits: vec!["Protect current budget".to_string()],
                        risks: vec!["Miss revenue window".to_string()],
                        projected_outcomes: vec!["Keep the same staffing level".to_string()],
                        score: 0.0,
                        time_horizon: "next quarter".to_string(),
                        confidence: "medium".to_string(),
                    },
                ],
                variables: vec![
                    ScenarioVariable {
                        id: "bandwidth".to_string(),
                        label: "Analyst bandwidth".to_string(),
                        current_value: "2 analysts covering launch research".to_string(),
                        proposed_value: "4 analysts covering launch research".to_string(),
                        impact: "high".to_string(),
                        uncertainty: "low".to_string(),
                        impact_weight: 0.0,
                        uncertainty_weight: 0.0,
                    },
                    ScenarioVariable {
                        id: "window".to_string(),
                        label: "Revenue window".to_string(),
                        current_value: "Launch slips past the conference".to_string(),
                        proposed_value: "Launch lands before the conference".to_string(),
                        impact: "high".to_string(),
                        uncertainty: "medium".to_string(),
                        impact_weight: 0.0,
                        uncertainty_weight: 0.0,
                    },
                ],
                recommendation: None,
                recommendation_reason: None,
                comparison_summary: None,
                selected_option_id: None,
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("scenario run should be created");

        assert_eq!(created.variables.len(), 2);
        assert_eq!(
            created.selected_option_id.as_deref(),
            Some("hire-contractors")
        );
        assert_eq!(
            created.recommendation.as_deref(),
            Some("Add contract analysts")
        );
        assert!(
            created
                .comparison_summary
                .as_deref()
                .unwrap_or_default()
                .contains("Analyst bandwidth")
        );
        assert!(
            created
                .option_cards
                .iter()
                .all(|option| option.score >= 0.0)
        );
        assert!(
            created
                .option_cards
                .iter()
                .all(|option| option.score <= 100.0)
        );

        let persisted_json = db
            .query_row(
                "SELECT options_json FROM scenario_runs WHERE id = ?1",
                &[&created.id],
                |row| row.get::<_, String>(0),
            )
            .expect("scenario payload should persist");
        let persisted = serde_json::from_str::<StoredScenarioRunPayload>(&persisted_json)
            .expect("scenario payload should deserialize");
        assert_eq!(persisted.option_cards.len(), 2);
        assert_eq!(persisted.variables.len(), 2);
        assert_eq!(
            persisted.selected_option_id.as_deref(),
            Some("hire-contractors")
        );
        assert!(
            persisted
                .comparison_summary
                .as_deref()
                .unwrap_or_default()
                .contains("Revenue window")
        );
    }

    #[test]
    fn scoring_formula_templates_can_be_listed_saved_and_reused() {
        let db = Database::in_memory().expect("database should initialize");

        let defaults = list_scoring_formula_templates_for_db(&db)
            .expect("default formula templates should list");
        assert!(defaults.iter().any(|template| template.id == "balanced"));

        let saved = save_scoring_formula_template_for_db(
            &db,
            SimulationSaveScoringFormulaTemplateRequest {
                id: Some("risk-sensitive".to_string()),
                name: "Risk Sensitive".to_string(),
                base_score: 52.0,
                impact_multiplier: 14.0,
                uncertainty_penalty: 24.0,
                description: Some(
                    "Penalize uncertain scenario paths more aggressively.".to_string(),
                ),
            },
        )
        .expect("formula template should save");
        assert_eq!(saved.id, "risk-sensitive");
        assert_eq!(saved.uncertainty_penalty, 24.0);

        let updated = save_scoring_formula_template_for_db(
            &db,
            SimulationSaveScoringFormulaTemplateRequest {
                id: Some("risk-sensitive".to_string()),
                name: "Risk Sensitive v2".to_string(),
                base_score: 54.0,
                impact_multiplier: 12.0,
                uncertainty_penalty: 28.0,
                description: Some("Updated risk-sensitive formula.".to_string()),
            },
        )
        .expect("formula template should update");
        assert_eq!(updated.name, "Risk Sensitive v2");
        assert_eq!(updated.base_score, 54.0);

        let templates = list_scoring_formula_templates_for_db(&db)
            .expect("saved formula templates should list");
        assert_eq!(
            templates
                .iter()
                .filter(|template| template.id == "risk-sensitive")
                .count(),
            1
        );
        assert!(templates.iter().any(|template| {
            template.id == "risk-sensitive" && template.description.contains("Updated")
        }));
    }

    #[test]
    fn handoff_policy_templates_can_be_listed_saved_and_reused() {
        let db = Database::in_memory().expect("database should initialize");

        let defaults =
            list_handoff_policy_templates_for_db(&db).expect("default templates should list");
        assert!(
            defaults
                .iter()
                .any(|template| template.id == "council-and-execution")
        );

        let saved = save_handoff_policy_template_for_db(
            &db,
            SimulationSaveHandoffPolicyTemplateRequest {
                id: Some("approval-gate".to_string()),
                name: "Approval Gate".to_string(),
                handoff_target: "execution_only".to_string(),
                execution_risk_level: "high".to_string(),
                description: Some("Route directly into high-risk execution approval.".to_string()),
            },
        )
        .expect("template should save");
        assert_eq!(saved.id, "approval-gate");
        assert_eq!(saved.handoff_target, "execution_only");
        assert_eq!(saved.execution_risk_level, "high");

        let updated = save_handoff_policy_template_for_db(
            &db,
            SimulationSaveHandoffPolicyTemplateRequest {
                id: Some("approval-gate".to_string()),
                name: "Council Gate".to_string(),
                handoff_target: "council_only".to_string(),
                execution_risk_level: "medium".to_string(),
                description: Some("Reusable council-only review policy.".to_string()),
            },
        )
        .expect("template should update");
        assert_eq!(updated.name, "Council Gate");
        assert_eq!(updated.handoff_target, "council_only");

        let templates =
            list_handoff_policy_templates_for_db(&db).expect("saved templates should list");
        assert_eq!(
            templates
                .iter()
                .filter(|template| template.id == "approval-gate")
                .count(),
            1
        );
        assert!(templates.iter().any(|template| {
            template.id == "approval-gate" && template.description.contains("council-only")
        }));
    }

    #[test]
    fn template_bundle_preflight_reports_create_and_update_without_writing_audit_log() {
        let db = Database::in_memory().expect("database should initialize");
        save_handoff_policy_template_for_db(
            &db,
            SimulationSaveHandoffPolicyTemplateRequest {
                id: Some("existing-policy".to_string()),
                name: "Existing policy".to_string(),
                handoff_target: "council_only".to_string(),
                execution_risk_level: "medium".to_string(),
                description: Some("Before".to_string()),
            },
        )
        .expect("existing template should save");
        let bundle = SimulationTemplateBundle {
            schema_version: 1,
            exported_at: "2026-04-27T00:00:00Z".to_string(),
            handoff_policy_templates: vec![
                SimulationHandoffPolicyTemplate {
                    id: "existing-policy".to_string(),
                    name: "Existing policy updated".to_string(),
                    handoff_target: "execution_only".to_string(),
                    execution_risk_level: "high".to_string(),
                    description: "After".to_string(),
                    updated_at: "2026-04-27T00:00:00Z".to_string(),
                },
                SimulationHandoffPolicyTemplate {
                    id: "new-policy".to_string(),
                    name: "New policy".to_string(),
                    handoff_target: "timeline_only".to_string(),
                    execution_risk_level: "low".to_string(),
                    description: "New".to_string(),
                    updated_at: "2026-04-27T00:00:00Z".to_string(),
                },
            ],
            scoring_formula_templates: vec![],
        };

        let preflight = preflight_template_bundle_import_for_db(
            &db,
            SimulationImportTemplateBundleRequest {
                bundle_json: serde_json::to_string(&bundle).expect("bundle should serialize"),
            },
        )
        .expect("preflight should succeed");

        assert_eq!(preflight.handoff_policy_templates.create_count, 1);
        assert_eq!(preflight.handoff_policy_templates.update_count, 1);
        assert_eq!(preflight.scoring_formula_templates.create_count, 0);
        assert_eq!(preflight.total_count, 2);
        assert!(
            preflight
                .conflicts
                .iter()
                .any(|item| item.id == "existing-policy")
        );
        assert!(
            list_template_bundle_audit_log_for_db(&db)
                .expect("audit should load")
                .is_empty()
        );
        let templates = list_handoff_policy_templates_for_db(&db).expect("templates should load");
        assert!(
            templates.iter().any(
                |template| template.id == "existing-policy" && template.description == "Before"
            )
        );
    }

    #[test]
    fn template_bundle_export_and_import_record_local_audit_events() {
        let db = Database::in_memory().expect("database should initialize");
        save_handoff_policy_template_for_db(
            &db,
            SimulationSaveHandoffPolicyTemplateRequest {
                id: Some("audited-policy".to_string()),
                name: "Audited policy".to_string(),
                handoff_target: "execution_only".to_string(),
                execution_risk_level: "high".to_string(),
                description: Some("Audit this policy".to_string()),
            },
        )
        .expect("policy template should save");

        let exported = export_template_bundle_for_db(&db).expect("bundle should export");
        import_template_bundle_for_db(
            &db,
            SimulationImportTemplateBundleRequest {
                bundle_json: serde_json::to_string(&exported).expect("bundle should serialize"),
            },
        )
        .expect("bundle should import");

        let audit = list_template_bundle_audit_log_for_db(&db).expect("audit log should load");
        assert_eq!(audit.len(), 2);
        assert_eq!(audit[0].action, "import");
        assert_eq!(audit[1].action, "export");
        assert!(audit[0].handoff_policy_template_count >= 1);
        assert!(audit[1].handoff_policy_template_count >= 1);
        assert_eq!(audit[0].actor, "local-operator");
    }

    #[test]
    fn template_bundle_audit_export_returns_recent_events_without_mutating_log() {
        let db = Database::in_memory().expect("database should initialize");
        save_handoff_policy_template_for_db(
            &db,
            SimulationSaveHandoffPolicyTemplateRequest {
                id: Some("audit-export-policy".to_string()),
                name: "Audit export policy".to_string(),
                handoff_target: "execution_only".to_string(),
                execution_risk_level: "high".to_string(),
                description: Some("Verify audit export is read-only.".to_string()),
            },
        )
        .expect("policy template should save");
        save_scoring_formula_template_for_db(
            &db,
            SimulationSaveScoringFormulaTemplateRequest {
                id: Some("audit-export-formula".to_string()),
                name: "Audit export formula".to_string(),
                base_score: 61.0,
                impact_multiplier: 21.0,
                uncertainty_penalty: 9.0,
                description: Some("Verify audit export leaves formulas untouched.".to_string()),
            },
        )
        .expect("formula template should save");

        let stored_log = vec![
            sample_template_bundle_audit_entry("event-oldest", "import", "2026-04-22T08:00:00Z"),
            sample_template_bundle_audit_entry("event-newest", "export", "2026-04-24T08:00:00Z"),
            sample_template_bundle_audit_entry("event-middle", "import", "2026-04-23T08:00:00Z"),
        ];
        seed_template_bundle_audit_log(&db, stored_log.clone());

        let handoff_before =
            list_handoff_policy_templates_for_db(&db).expect("handoff templates should load");
        let formulas_before =
            list_scoring_formula_templates_for_db(&db).expect("formula templates should load");

        let exported = export_template_bundle_audit_log_for_db(
            &db,
            SimulationExportTemplateBundleAuditLogRequest { limit: Some(2) },
        )
        .expect("audit export should succeed");

        assert_eq!(exported.total, 3);
        assert_eq!(exported.exported_count, 2);
        assert_eq!(
            exported
                .events
                .iter()
                .map(|event| event.id.as_str())
                .collect::<Vec<_>>(),
            vec!["event-newest", "event-middle"]
        );
        assert_eq!(
            list_template_bundle_audit_log_for_db(&db).expect("audit log should still load"),
            stored_log
        );
        assert_eq!(
            list_handoff_policy_templates_for_db(&db).expect("handoff templates should reload"),
            handoff_before
        );
        assert_eq!(
            list_scoring_formula_templates_for_db(&db).expect("formula templates should reload"),
            formulas_before
        );
    }

    #[test]
    fn template_bundle_audit_export_uses_default_limit_and_caps_requested_limit() {
        let db = Database::in_memory().expect("database should initialize");
        let stored_log = (0..60)
            .map(|index| {
                sample_template_bundle_audit_entry(
                    &format!("event-{index:02}"),
                    if index % 2 == 0 { "export" } else { "import" },
                    &format!("2026-04-{day:02}T08:00:00Z", day = (index % 28) + 1),
                )
            })
            .collect::<Vec<_>>();
        seed_template_bundle_audit_log(&db, stored_log);

        let exported_default = export_template_bundle_audit_log_for_db(
            &db,
            SimulationExportTemplateBundleAuditLogRequest { limit: None },
        )
        .expect("default audit export should succeed");
        assert_eq!(exported_default.exported_count, 20);

        let exported_capped = export_template_bundle_audit_log_for_db(
            &db,
            SimulationExportTemplateBundleAuditLogRequest { limit: Some(999) },
        )
        .expect("capped audit export should succeed");
        assert_eq!(exported_capped.total, 60);
        assert_eq!(exported_capped.exported_count, 50);
    }

    #[test]
    fn template_bundles_can_be_exported_and_imported_for_manual_team_sharing() {
        let source = Database::in_memory().expect("source database should initialize");
        save_handoff_policy_template_for_db(
            &source,
            SimulationSaveHandoffPolicyTemplateRequest {
                id: Some("shared-execution-gate".to_string()),
                name: "Shared Execution Gate".to_string(),
                handoff_target: "execution_only".to_string(),
                execution_risk_level: "high".to_string(),
                description: Some("Shared policy for high-risk execution review.".to_string()),
            },
        )
        .expect("shared handoff template should save");
        save_scoring_formula_template_for_db(
            &source,
            SimulationSaveScoringFormulaTemplateRequest {
                id: Some("shared-impact-model".to_string()),
                name: "Shared Impact Model".to_string(),
                base_score: 60.0,
                impact_multiplier: 25.0,
                uncertainty_penalty: 8.0,
                description: Some("Shared formula for impact-forward decisions.".to_string()),
            },
        )
        .expect("shared formula template should save");

        let exported = export_template_bundle_for_db(&source).expect("bundle should export");
        let bundle_json = serde_json::to_string(&exported).expect("bundle should serialize");
        assert!(bundle_json.contains("shared-execution-gate"));
        assert!(bundle_json.contains("shared-impact-model"));

        let target = Database::in_memory().expect("target database should initialize");
        let imported = import_template_bundle_for_db(
            &target,
            SimulationImportTemplateBundleRequest { bundle_json },
        )
        .expect("bundle should import");

        assert!(imported.imported_handoff_policy_templates >= 1);
        assert!(imported.imported_scoring_formula_templates >= 1);
        assert!(
            imported
                .handoff_policy_templates
                .iter()
                .any(|template| template.id == "shared-execution-gate")
        );
        assert!(
            imported
                .scoring_formula_templates
                .iter()
                .any(|template| template.id == "shared-impact-model")
        );
    }

    #[test]
    fn create_scenario_run_respects_configurable_handoff_policy() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let council_mission = repo
            .create(sample_input("Council-only scenario mission"))
            .expect("mission should be created");
        let execution_mission = repo
            .create(sample_input("Execution-only scenario mission"))
            .expect("mission should be created");

        let council_only = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: council_mission.id.clone(),
                baseline: "Review the safest launch path".to_string(),
                options: vec!["Run a constrained pilot".to_string()],
                option_cards: vec![],
                variables: vec![],
                recommendation: Some("Run a constrained pilot".to_string()),
                recommendation_reason: Some(
                    "Council should review the constrained pilot before execution.".to_string(),
                ),
                comparison_summary: None,
                selected_option_id: None,
                handoff_target: Some("council_only".to_string()),
                execution_risk_level: Some("high".to_string()),
            },
        )
        .expect("council-only scenario should save");
        assert_eq!(council_only.handoff_target, "council_only");
        assert_eq!(council_only.execution_risk_level, "high");

        let council_steps = db
            .query_row(
                "SELECT COUNT(*) FROM council_steps WHERE mission_id = ?1 AND role = 'Scenario Reviewer'",
                &[&council_mission.id],
                |row| row.get::<_, i64>(0),
            )
            .expect("council steps should count");
        let execution_steps = db
            .query_row(
                "SELECT COUNT(*) FROM execution_steps WHERE mission_id = ?1",
                &[&council_mission.id],
                |row| row.get::<_, i64>(0),
            )
            .expect("execution steps should count");
        assert_eq!(council_steps, 1);
        assert_eq!(execution_steps, 0);

        let execution_only = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: execution_mission.id.clone(),
                baseline: "Prepare an execution-review-only path".to_string(),
                options: vec!["Ship with approval guardrails".to_string()],
                option_cards: vec![],
                variables: vec![],
                recommendation: Some("Ship with approval guardrails".to_string()),
                recommendation_reason: Some(
                    "Execution review needs explicit approval before this path can run."
                        .to_string(),
                ),
                comparison_summary: None,
                selected_option_id: None,
                handoff_target: Some("execution_only".to_string()),
                execution_risk_level: Some("high".to_string()),
            },
        )
        .expect("execution-only scenario should save");
        assert_eq!(execution_only.handoff_target, "execution_only");
        assert_eq!(execution_only.execution_risk_level, "high");

        let council_steps = db
            .query_row(
                "SELECT COUNT(*) FROM council_steps WHERE mission_id = ?1",
                &[&execution_mission.id],
                |row| row.get::<_, i64>(0),
            )
            .expect("council steps should count");
        let execution_policy = db
            .query_row(
                "SELECT risk_level, status, input_payload FROM execution_steps WHERE mission_id = ?1",
                &[&execution_mission.id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .expect("execution policy should load");
        assert_eq!(council_steps, 0);
        assert_eq!(execution_policy.0, "high");
        assert_eq!(execution_policy.1, "awaiting_approval");
        assert!(
            execution_policy
                .2
                .as_deref()
                .unwrap_or_default()
                .contains("execution_only")
        );
    }

    #[test]
    fn create_scenario_run_records_rationale_numeric_variables_and_governance_handoff() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Scenario handoff mission"))
            .expect("mission should be created");

        let created = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: mission.id.clone(),
                baseline: "Keep a conservative rollout plan".to_string(),
                options: vec![],
                option_cards: vec![
                    ScenarioOptionCard {
                        id: "guided-pilot".to_string(),
                        label: "Run a guided pilot".to_string(),
                        assumptions: vec!["Two design partners can join this sprint".to_string()],
                        expected_benefits: vec!["Validate adoption before launch".to_string()],
                        risks: vec!["Pilot coordination adds overhead".to_string()],
                        projected_outcomes: vec!["Sharper launch criteria".to_string()],
                        score: 0.0,
                        time_horizon: "next 30 days".to_string(),
                        confidence: "high".to_string(),
                    },
                    ScenarioOptionCard {
                        id: "big-bang".to_string(),
                        label: "Launch broadly now".to_string(),
                        assumptions: vec!["Current enablement is enough".to_string()],
                        expected_benefits: vec!["Faster market signal".to_string()],
                        risks: vec!["Support load spikes".to_string()],
                        projected_outcomes: vec!["Immediate usage spike".to_string()],
                        score: 0.0,
                        time_horizon: "next 2 weeks".to_string(),
                        confidence: "medium".to_string(),
                    },
                ],
                variables: vec![ScenarioVariable {
                    id: "support-load".to_string(),
                    label: "Support load".to_string(),
                    current_value: "Two support engineers".to_string(),
                    proposed_value: "Four support engineers plus onboarding docs".to_string(),
                    impact: "medium".to_string(),
                    uncertainty: "medium".to_string(),
                    impact_weight: 88.0,
                    uncertainty_weight: 22.0,
                }],
                recommendation: Some("Run a guided pilot".to_string()),
                recommendation_reason: Some(
                    "Run a guided pilot because it keeps adoption evidence auditable before launch."
                        .to_string(),
                ),
                comparison_summary: Some(
                    "Guided pilot wins because it lowers launch risk while preserving momentum."
                        .to_string(),
                ),
                selected_option_id: Some("guided-pilot".to_string()),
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("scenario run should be created");

        assert_eq!(
            created.recommendation.as_deref(),
            Some("Run a guided pilot")
        );
        assert_eq!(
            created.recommendation_reason.as_deref(),
            Some("Run a guided pilot because it keeps adoption evidence auditable before launch.")
        );
        assert_eq!(created.variables[0].impact_weight, 88.0);
        assert_eq!(created.variables[0].uncertainty_weight, 22.0);

        let persisted_json = db
            .query_row(
                "SELECT options_json FROM scenario_runs WHERE id = ?1",
                &[&created.id],
                |row| row.get::<_, String>(0),
            )
            .expect("scenario payload should persist");
        let persisted = serde_json::from_str::<StoredScenarioRunPayload>(&persisted_json)
            .expect("scenario payload should deserialize");
        assert_eq!(persisted.variables[0].impact_weight, 88.0);
        assert_eq!(persisted.variables[0].uncertainty_weight, 22.0);
        assert_eq!(
            persisted.recommendation_reason.as_deref(),
            Some("Run a guided pilot because it keeps adoption evidence auditable before launch.")
        );

        let run_count = db
            .query_row(
                "SELECT COUNT(*) FROM runs WHERE mission_id = ?1 AND type = 'simulation' AND status = 'completed' AND summary LIKE '%Run a guided pilot%'",
                &[&mission.id],
                |row| row.get::<_, i64>(0),
            )
            .expect("simulation run should be counted");
        assert_eq!(run_count, 1);

        let event_count = db
            .query_row(
                "SELECT COUNT(*) FROM run_events WHERE mission_id = ?1 AND event_type = 'scenario_saved' AND message LIKE '%Run a guided pilot%'",
                &[&mission.id],
                |row| row.get::<_, i64>(0),
            )
            .expect("scenario event should be counted");
        assert_eq!(event_count, 1);

        let council_count = db
            .query_row(
                "SELECT COUNT(*) FROM council_steps WHERE mission_id = ?1 AND role = 'Scenario Reviewer' AND status = 'pending' AND input_summary LIKE '%Run a guided pilot%'",
                &[&mission.id],
                |row| row.get::<_, i64>(0),
            )
            .expect("council handoff should be counted");
        assert_eq!(council_count, 1);

        let execution_count = db
            .query_row(
                "SELECT COUNT(*) FROM execution_steps WHERE mission_id = ?1 AND mode = 'api' AND status = 'pending' AND title LIKE '%Review scenario recommendation%' AND input_payload LIKE '%guided-pilot%'",
                &[&mission.id],
                |row| row.get::<_, i64>(0),
            )
            .expect("execution handoff should be counted");
        assert_eq!(execution_count, 1);
    }

    #[test]
    fn create_scenario_run_derives_comparison_summary_and_normalized_scores() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Derived comparison mission"))
            .expect("mission should be created");

        let created = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: mission.id.clone(),
                baseline: "Keep the release plan unchanged".to_string(),
                options: vec![],
                option_cards: vec![
                    ScenarioOptionCard {
                        id: "accelerate-qa".to_string(),
                        label: "Accelerate QA staffing".to_string(),
                        assumptions: vec!["External testers are available".to_string()],
                        expected_benefits: vec![
                            "Improve defect coverage before launch".to_string(),
                            "Protect the launch date".to_string(),
                        ],
                        risks: vec!["Higher near-term spend".to_string()],
                        projected_outcomes: vec![
                            "Raise confidence in the release train".to_string(),
                        ],
                        score: 0.0,
                        time_horizon: "6 weeks".to_string(),
                        confidence: "high".to_string(),
                    },
                    ScenarioOptionCard {
                        id: "delay-launch".to_string(),
                        label: "Delay the launch date".to_string(),
                        assumptions: vec!["Customers tolerate the slip".to_string()],
                        expected_benefits: vec!["More test time".to_string()],
                        risks: vec![
                            "Lose the conference reveal".to_string(),
                            "Reduce pipeline momentum".to_string(),
                        ],
                        projected_outcomes: vec!["Push revenue to the next quarter".to_string()],
                        score: 0.0,
                        time_horizon: "next quarter".to_string(),
                        confidence: "medium".to_string(),
                    },
                ],
                variables: vec![
                    ScenarioVariable {
                        id: "quality".to_string(),
                        label: "Release quality".to_string(),
                        current_value: "Defect escape rate remains elevated".to_string(),
                        proposed_value: "Defect escape rate drops before launch".to_string(),
                        impact: "high".to_string(),
                        uncertainty: "low".to_string(),
                        impact_weight: 0.0,
                        uncertainty_weight: 0.0,
                    },
                    ScenarioVariable {
                        id: "timeline".to_string(),
                        label: "Launch timing".to_string(),
                        current_value: "Miss the conference window".to_string(),
                        proposed_value: "Hit the conference window".to_string(),
                        impact: "high".to_string(),
                        uncertainty: "medium".to_string(),
                        impact_weight: 0.0,
                        uncertainty_weight: 0.0,
                    },
                ],
                recommendation: None,
                recommendation_reason: None,
                comparison_summary: None,
                selected_option_id: None,
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("scenario run should be created");

        assert_eq!(created.selected_option_id.as_deref(), Some("accelerate-qa"));
        assert_eq!(
            created.recommendation.as_deref(),
            Some("Accelerate QA staffing")
        );
        assert!(
            created
                .comparison_summary
                .as_deref()
                .unwrap_or_default()
                .contains("Launch timing")
        );
        assert!(created.option_cards[0].score > created.option_cards[1].score);
        assert!(created.option_cards[0].score <= 100.0);
        assert!(created.option_cards[1].score >= 0.0);
    }

    #[test]
    fn list_scenario_runs_returns_only_requested_mission_in_reverse_chronological_order() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let alpha = repo
            .create(sample_input("Alpha scenario"))
            .expect("alpha mission should be created");
        let beta = repo
            .create(sample_input("Beta scenario"))
            .expect("beta mission should be created");

        let first = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: alpha.id.clone(),
                baseline: "Baseline one".to_string(),
                options: vec!["Option A".to_string()],
                option_cards: vec![],
                variables: vec![],
                recommendation: Some("Option A".to_string()),
                recommendation_reason: None,
                comparison_summary: None,
                selected_option_id: None,
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("first scenario should be created");
        let second = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: alpha.id.clone(),
                baseline: "Baseline two".to_string(),
                options: vec!["Option B".to_string(), "Option C".to_string()],
                option_cards: vec![],
                variables: vec![],
                recommendation: None,
                recommendation_reason: None,
                comparison_summary: None,
                selected_option_id: None,
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("second scenario should be created");
        let _other = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: beta.id.clone(),
                baseline: "Other baseline".to_string(),
                options: vec!["Other option".to_string()],
                option_cards: vec![],
                variables: vec![],
                recommendation: Some("Other option".to_string()),
                recommendation_reason: None,
                comparison_summary: None,
                selected_option_id: None,
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("other mission scenario should be created");

        let runs = list_scenario_runs(&db, &alpha.id).expect("scenario runs should load");

        assert_eq!(runs.len(), 2);
        assert_eq!(
            runs.iter().map(|run| run.id.as_str()).collect::<Vec<_>>(),
            vec![second.id.as_str(), first.id.as_str()]
        );
        assert!(runs.iter().all(|run| run.mission_id == alpha.id));
        assert!(runs.iter().all(|run| run.mission_title == "Alpha scenario"));
        assert_eq!(
            runs[0].options,
            vec!["Option B".to_string(), "Option C".to_string()]
        );
        assert_eq!(runs[1].recommendation.as_deref(), Some("Option A"));
    }

    #[test]
    fn compare_scenario_runs_synthesizes_matrix_axes_patterns_and_legacy_rows() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Comparison mission"))
            .expect("mission should be created");

        let oldest = insert_legacy_scenario_run(
            &db,
            &mission.id,
            "Keep the current go-to-market plan",
            vec![
                "Hold launch scope".to_string(),
                "Add inventory buffer".to_string(),
            ],
            Some("Hold launch scope"),
            "2026-04-20T08:00:00Z",
        );
        let middle = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: mission.id.clone(),
                baseline: "Increase sell-in support before launch".to_string(),
                options: vec![],
                option_cards: vec![
                    ScenarioOptionCard {
                        id: "inventory-buffer".to_string(),
                        label: "Add inventory buffer".to_string(),
                        assumptions: vec!["Warehouse overflow is available".to_string()],
                        expected_benefits: vec!["Protect against channel delays".to_string()],
                        risks: vec!["Higher carrying cost".to_string()],
                        projected_outcomes: vec!["Absorb supplier variance".to_string()],
                        score: 81.0,
                        time_horizon: "next quarter".to_string(),
                        confidence: "high".to_string(),
                    },
                    ScenarioOptionCard {
                        id: "channel-shift".to_string(),
                        label: "Shift demand to partners".to_string(),
                        assumptions: vec!["Partners can absorb demand".to_string()],
                        expected_benefits: vec!["Reduce direct fulfillment pressure".to_string()],
                        risks: vec!["Partner coverage remains uneven".to_string()],
                        projected_outcomes: vec!["Smooth order intake".to_string()],
                        score: 62.0,
                        time_horizon: "next quarter".to_string(),
                        confidence: "medium".to_string(),
                    },
                ],
                variables: vec![ScenarioVariable {
                    id: "supply".to_string(),
                    label: "Supplier lead time".to_string(),
                    current_value: "6 weeks".to_string(),
                    proposed_value: "4 weeks".to_string(),
                    impact: "high".to_string(),
                    uncertainty: "medium".to_string(),
                    impact_weight: 0.0,
                    uncertainty_weight: 0.0,
                }],
                recommendation: None,
                recommendation_reason: None,
                comparison_summary: Some(
                    "Buffering inventory offsets the current supply delay.".to_string(),
                ),
                selected_option_id: Some("inventory-buffer".to_string()),
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("middle scenario should be created");
        update_scenario_created_at(&db, &middle.id, "2026-04-21T08:00:00Z");

        let newest = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: mission.id.clone(),
                baseline: "Push enablement earlier and protect the channel launch".to_string(),
                options: vec![],
                option_cards: vec![
                    ScenarioOptionCard {
                        id: "inventory-buffer-v2".to_string(),
                        label: "Add inventory buffer".to_string(),
                        assumptions: vec!["Overflow space remains available".to_string()],
                        expected_benefits: vec!["Protect launch availability".to_string()],
                        risks: vec!["Ties up working capital".to_string()],
                        projected_outcomes: vec!["Keep channels stocked".to_string()],
                        score: 74.0,
                        time_horizon: "45 days".to_string(),
                        confidence: "medium".to_string(),
                    },
                    ScenarioOptionCard {
                        id: "enablement".to_string(),
                        label: "Accelerate partner enablement".to_string(),
                        assumptions: vec!["Field training content is ready".to_string()],
                        expected_benefits: vec!["Improve channel readiness".to_string()],
                        risks: vec!["Higher launch coordination load".to_string()],
                        projected_outcomes: vec!["Raise partner close rates".to_string()],
                        score: 88.0,
                        time_horizon: "30 days".to_string(),
                        confidence: "high".to_string(),
                    },
                ],
                variables: vec![
                    ScenarioVariable {
                        id: "supply".to_string(),
                        label: "Supplier lead time".to_string(),
                        current_value: "6 weeks".to_string(),
                        proposed_value: "3 weeks".to_string(),
                        impact: "high".to_string(),
                        uncertainty: "low".to_string(),
                        impact_weight: 0.0,
                        uncertainty_weight: 0.0,
                    },
                    ScenarioVariable {
                        id: "readiness".to_string(),
                        label: "Partner readiness".to_string(),
                        current_value: "Field coverage is inconsistent".to_string(),
                        proposed_value: "Field coverage is launch-ready".to_string(),
                        impact: "medium".to_string(),
                        uncertainty: "medium".to_string(),
                        impact_weight: 0.0,
                        uncertainty_weight: 0.0,
                    },
                ],
                recommendation: Some("Accelerate partner enablement".to_string()),
                recommendation_reason: None,
                comparison_summary: Some(
                    "Enablement becomes the leading path once partner readiness improves."
                        .to_string(),
                ),
                selected_option_id: Some("enablement".to_string()),
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("newest scenario should be created");
        update_scenario_created_at(&db, &newest.id, "2026-04-22T08:00:00Z");

        let matrix = compare_scenario_runs(&db, &mission.id).expect("matrix should build");

        assert_eq!(matrix.mission_id, mission.id);
        assert_eq!(matrix.mission_title, "Comparison mission");
        assert_eq!(matrix.scenario_count, 3);
        assert_eq!(
            matrix
                .scenarios
                .iter()
                .map(|scenario| scenario.scenario_run_id.as_str())
                .collect::<Vec<_>>(),
            vec![newest.id.as_str(), middle.id.as_str(), oldest.as_str()]
        );
        assert_eq!(
            matrix
                .scenarios
                .iter()
                .map(|scenario| scenario.selected_option_label.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Accelerate partner enablement",
                "Add inventory buffer",
                "Hold launch scope",
            ]
        );
        assert_eq!(matrix.variable_axes.len(), 2);
        assert_eq!(matrix.variable_axes[0].label, "Supplier lead time");
        assert_eq!(
            matrix.variable_axes[0].values,
            vec![
                "6 weeks -> 3 weeks".to_string(),
                "6 weeks -> 4 weeks".to_string()
            ]
        );
        assert_eq!(
            matrix.variable_axes[0].uncertainties,
            vec!["low".to_string(), "medium".to_string()]
        );
        let inventory_pattern = matrix
            .option_patterns
            .iter()
            .find(|pattern| pattern.label == "Add inventory buffer")
            .expect("inventory pattern should exist");
        assert_eq!(inventory_pattern.appearance_count, 3);
        assert_eq!(inventory_pattern.selected_count, 1);
        assert_eq!(inventory_pattern.average_score, 85.0);
        assert_eq!(inventory_pattern.latest_time_horizon, "45 days");
        assert!(matrix.summary.contains("Across 3 scenarios"));
        assert!(matrix.summary.contains("Supplier lead time"));
        assert!(matrix.summary.contains("Accelerate partner enablement"));
    }

    #[test]
    fn compare_scenario_runs_orders_path_evolution_in_reverse_chronological_order() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Evolution mission"))
            .expect("mission should be created");

        let oldest = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: mission.id.clone(),
                baseline: "Oldest baseline".to_string(),
                options: vec![],
                option_cards: vec![ScenarioOptionCard {
                    id: "oldest-option".to_string(),
                    label: "Stabilize the current path".to_string(),
                    assumptions: vec![],
                    expected_benefits: vec!["Hold launch quality".to_string()],
                    risks: vec!["Recovery remains slow".to_string()],
                    projected_outcomes: vec!["Keep the current schedule".to_string()],
                    score: 51.0,
                    time_horizon: "60 days".to_string(),
                    confidence: "medium".to_string(),
                }],
                variables: vec![ScenarioVariable {
                    id: "capacity".to_string(),
                    label: "Launch capacity".to_string(),
                    current_value: "Coverage is thin".to_string(),
                    proposed_value: "Coverage stays thin".to_string(),
                    impact: "medium".to_string(),
                    uncertainty: "medium".to_string(),
                    impact_weight: 0.0,
                    uncertainty_weight: 0.0,
                }],
                recommendation: Some("Stabilize the current path".to_string()),
                recommendation_reason: None,
                comparison_summary: Some("The initial scenario held the line.".to_string()),
                selected_option_id: Some("oldest-option".to_string()),
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("oldest scenario should be created");
        update_scenario_created_at(&db, &oldest.id, "2026-04-20T07:00:00Z");

        let middle = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: mission.id.clone(),
                baseline: "Middle baseline".to_string(),
                options: vec![],
                option_cards: vec![ScenarioOptionCard {
                    id: "middle-option".to_string(),
                    label: "Add contingency coverage".to_string(),
                    assumptions: vec![],
                    expected_benefits: vec!["Reduce launch strain".to_string()],
                    risks: vec!["Temporary cost increase".to_string()],
                    projected_outcomes: vec!["Build extra slack".to_string()],
                    score: 73.0,
                    time_horizon: "45 days".to_string(),
                    confidence: "high".to_string(),
                }],
                variables: vec![ScenarioVariable {
                    id: "capacity".to_string(),
                    label: "Launch capacity".to_string(),
                    current_value: "Coverage is thin".to_string(),
                    proposed_value: "Coverage improves modestly".to_string(),
                    impact: "high".to_string(),
                    uncertainty: "medium".to_string(),
                    impact_weight: 0.0,
                    uncertainty_weight: 0.0,
                }],
                recommendation: Some("Add contingency coverage".to_string()),
                recommendation_reason: None,
                comparison_summary: Some(
                    "Coverage improves in the mid-course adjustment.".to_string(),
                ),
                selected_option_id: Some("middle-option".to_string()),
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("middle scenario should be created");
        update_scenario_created_at(&db, &middle.id, "2026-04-21T07:00:00Z");

        let newest = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: mission.id.clone(),
                baseline: "Newest baseline".to_string(),
                options: vec![],
                option_cards: vec![ScenarioOptionCard {
                    id: "newest-option".to_string(),
                    label: "Accelerate specialist support".to_string(),
                    assumptions: vec![],
                    expected_benefits: vec!["Recover launch confidence".to_string()],
                    risks: vec!["Coordination overhead".to_string()],
                    projected_outcomes: vec!["Raise execution tempo".to_string()],
                    score: 92.0,
                    time_horizon: "30 days".to_string(),
                    confidence: "high".to_string(),
                }],
                variables: vec![ScenarioVariable {
                    id: "capacity".to_string(),
                    label: "Launch capacity".to_string(),
                    current_value: "Coverage is thin".to_string(),
                    proposed_value: "Coverage is fully staffed".to_string(),
                    impact: "high".to_string(),
                    uncertainty: "low".to_string(),
                    impact_weight: 0.0,
                    uncertainty_weight: 0.0,
                }],
                recommendation: Some("Accelerate specialist support".to_string()),
                recommendation_reason: None,
                comparison_summary: Some(
                    "The latest path fully staffs the launch lane.".to_string(),
                ),
                selected_option_id: Some("newest-option".to_string()),
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("newest scenario should be created");
        update_scenario_created_at(&db, &newest.id, "2026-04-22T07:00:00Z");

        let matrix = compare_scenario_runs(&db, &mission.id).expect("matrix should build");

        assert_eq!(
            matrix
                .path_evolution
                .iter()
                .map(|step| step.scenario_run_id.as_str())
                .collect::<Vec<_>>(),
            vec![newest.id.as_str(), middle.id.as_str(), oldest.id.as_str()]
        );
        assert_eq!(
            matrix
                .path_evolution
                .iter()
                .map(|step| step.selected_option_label.as_str())
                .collect::<Vec<_>>(),
            vec![
                "Accelerate specialist support",
                "Add contingency coverage",
                "Stabilize the current path",
            ]
        );
        assert_eq!(matrix.path_evolution[0].score, 92.0);
        assert_eq!(
            matrix.path_evolution[0].variable_changes,
            vec!["Launch capacity: Coverage is thin -> Coverage is fully staffed".to_string()]
        );
        assert_eq!(
            matrix.path_evolution[1].narrative,
            "Coverage improves in the mid-course adjustment."
        );
    }

    #[test]
    fn compare_scenario_runs_returns_empty_matrix_when_mission_has_no_saved_scenarios() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Empty comparison mission"))
            .expect("mission should be created");

        let matrix = compare_scenario_runs(&db, &mission.id).expect("matrix should build");

        assert_eq!(matrix.mission_id, mission.id);
        assert_eq!(matrix.mission_title, "Empty comparison mission");
        assert_eq!(matrix.scenario_count, 0);
        assert!(matrix.scenarios.is_empty());
        assert!(matrix.variable_axes.is_empty());
        assert!(matrix.option_patterns.is_empty());
        assert!(matrix.path_evolution.is_empty());
        assert_eq!(matrix.summary, "No saved scenarios yet for this mission.");
    }

    #[test]
    fn compare_scenario_runs_handles_single_scenario_without_cross_run_history() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Single comparison mission"))
            .expect("mission should be created");

        let created = create_scenario_run(
            &db,
            SimulationCreateScenarioRunRequest {
                mission_id: mission.id.clone(),
                baseline: "Keep the current launch structure".to_string(),
                options: vec![],
                option_cards: vec![ScenarioOptionCard {
                    id: "support".to_string(),
                    label: "Add support contractors".to_string(),
                    assumptions: vec!["Contractors can start immediately".to_string()],
                    expected_benefits: vec!["Increase launch throughput".to_string()],
                    risks: vec!["Onboarding load".to_string()],
                    projected_outcomes: vec!["Shrink the backlog".to_string()],
                    score: 84.0,
                    time_horizon: "30 days".to_string(),
                    confidence: "high".to_string(),
                }],
                variables: vec![ScenarioVariable {
                    id: "capacity".to_string(),
                    label: "Launch capacity".to_string(),
                    current_value: "Coverage is constrained".to_string(),
                    proposed_value: "Coverage expands with contractors".to_string(),
                    impact: "high".to_string(),
                    uncertainty: "medium".to_string(),
                    impact_weight: 0.0,
                    uncertainty_weight: 0.0,
                }],
                recommendation: Some("Add support contractors".to_string()),
                recommendation_reason: None,
                comparison_summary: Some(
                    "The modeled path expands launch capacity with contractor support.".to_string(),
                ),
                selected_option_id: Some("support".to_string()),
                handoff_target: None,
                execution_risk_level: None,
            },
        )
        .expect("scenario should be created");
        update_scenario_created_at(&db, &created.id, "2026-04-22T09:00:00Z");

        let matrix = compare_scenario_runs(&db, &mission.id).expect("matrix should build");

        assert_eq!(matrix.scenario_count, 1);
        assert_eq!(matrix.scenarios.len(), 1);
        assert_eq!(matrix.path_evolution.len(), 1);
        assert_eq!(
            matrix.path_evolution[0].selected_option_label,
            "Add support contractors"
        );
        assert_eq!(matrix.path_evolution[0].score, 84.0);
        assert_eq!(matrix.option_patterns.len(), 1);
        assert_eq!(matrix.option_patterns[0].appearance_count, 1);
        assert_eq!(matrix.option_patterns[0].selected_count, 1);
        assert_eq!(
            matrix.summary,
            "Only one saved scenario is available, so the comparison captures the current path without cross-scenario drift."
        );
    }

    #[test]
    fn local_multi_agent_sandbox_records_run_event_and_scores_options() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Local sandbox mission"))
            .expect("mission should be created");

        let response = run_local_sandbox_simulation(
            &db,
            SimulationRunLocalSandboxRequest {
                mission_id: mission.id.clone(),
                baseline:
                    "Keep the current launch plan unless a better deterministic path emerges."
                        .to_string(),
                options: vec![
                    "Add support specialists for the launch window".to_string(),
                    "Delay launch by two weeks to reduce execution risk".to_string(),
                    "Keep the launch date and freeze non-critical scope".to_string(),
                ],
                agents: vec![
                    SimulationSandboxAgentRequest {
                        name: "Avery".to_string(),
                        role: "Operations lead".to_string(),
                        stance: "risk aware".to_string(),
                    },
                    SimulationSandboxAgentRequest {
                        name: "Mika".to_string(),
                        role: "Growth strategist".to_string(),
                        stance: "speed biased".to_string(),
                    },
                    SimulationSandboxAgentRequest {
                        name: "Jun".to_string(),
                        role: "Finance reviewer".to_string(),
                        stance: "cost skeptical".to_string(),
                    },
                ],
                rounds: Some(4),
            },
        )
        .expect("local sandbox simulation should complete");

        assert_eq!(response.mission_id, mission.id);
        assert_eq!(response.engine, "local_deterministic_multi_agent_sandbox");
        assert_eq!(response.rounds, 4);
        assert_eq!(response.agents.len(), 3);
        assert_eq!(response.turns.len(), 36);
        assert_eq!(response.option_scores.len(), 3);
        assert!(response.audit_event_id.is_some());
        assert!(
            response
                .turns
                .iter()
                .all(|turn| !turn.rationale.trim().is_empty())
        );

        let recommended = response
            .option_scores
            .iter()
            .max_by(|left, right| left.average_score.total_cmp(&right.average_score))
            .expect("at least one option score");
        assert_eq!(response.recommendation.option, recommended.option);
        assert_eq!(
            response.recommendation.average_score,
            recommended.average_score
        );

        let persisted_run = db
            .query_row(
                "SELECT type, status, summary FROM runs WHERE id = ?1",
                &[&response.run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .expect("simulation run should persist");
        assert_eq!(persisted_run.0, "simulation");
        assert_eq!(persisted_run.1, "completed");
        assert!(
            persisted_run
                .2
                .as_deref()
                .unwrap_or_default()
                .contains(&response.recommendation.option)
        );

        let persisted_event = db
            .query_row(
                "SELECT run_id, event_type, payload_json FROM run_events WHERE id = ?1",
                &[&response
                    .audit_event_id
                    .clone()
                    .expect("audit event id should exist")],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .expect("simulation event should persist");
        assert_eq!(persisted_event.0, response.run_id);
        assert_eq!(
            persisted_event.1,
            "local_sandbox_simulation_completed".to_string()
        );
        let payload = persisted_event
            .2
            .expect("event payload should persist for local sandbox");
        assert!(payload.contains("local_deterministic_multi_agent_sandbox"));
        assert!(payload.contains("Add support specialists"));
    }

    #[test]
    fn local_multi_agent_sandbox_history_lists_recent_runs_for_mission() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Sandbox history mission"))
            .expect("mission should be created");
        let other_mission = repo
            .create(sample_input("Other sandbox history mission"))
            .expect("other mission should be created");

        let first = run_local_sandbox_simulation(
            &db,
            SimulationRunLocalSandboxRequest {
                mission_id: mission.id.clone(),
                baseline: "Baseline".to_string(),
                options: vec!["Option A".to_string(), "Option B".to_string()],
                agents: Vec::new(),
                rounds: Some(1),
            },
        )
        .expect("first local sandbox should run");
        let second = run_local_sandbox_simulation(
            &db,
            SimulationRunLocalSandboxRequest {
                mission_id: mission.id.clone(),
                baseline: "Updated baseline".to_string(),
                options: vec!["Option C".to_string(), "Option D".to_string()],
                agents: Vec::new(),
                rounds: Some(1),
            },
        )
        .expect("second local sandbox should run");
        run_local_sandbox_simulation(
            &db,
            SimulationRunLocalSandboxRequest {
                mission_id: other_mission.id,
                baseline: "Other baseline".to_string(),
                options: vec!["Other A".to_string(), "Other B".to_string()],
                agents: Vec::new(),
                rounds: Some(1),
            },
        )
        .expect("other local sandbox should run");

        let history = list_local_sandbox_runs_for_db(
            &db,
            SimulationLocalSandboxRunListRequest {
                mission_id: Some(mission.id),
                limit: Some(10),
            },
        )
        .expect("history should load");

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].run_id, second.run_id);
        assert_eq!(history[1].run_id, first.run_id);
        assert!(
            history
                .iter()
                .all(|run| run.engine == LOCAL_SANDBOX_ENGINE_NAME)
        );
    }

    #[test]
    fn local_multi_agent_sandbox_uses_default_rounds_and_caps_requested_rounds() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("Sandbox bounds mission"))
            .expect("mission should be created");

        let default_rounds = run_local_sandbox_simulation(
            &db,
            SimulationRunLocalSandboxRequest {
                mission_id: mission.id.clone(),
                baseline: "Baseline".to_string(),
                options: vec!["Option A".to_string(), "Option B".to_string()],
                agents: vec![SimulationSandboxAgentRequest {
                    name: "Ari".to_string(),
                    role: "Operator".to_string(),
                    stance: "balanced".to_string(),
                }],
                rounds: None,
            },
        )
        .expect("default rounds simulation should succeed");
        assert_eq!(default_rounds.rounds, LOCAL_SANDBOX_DEFAULT_ROUNDS);
        assert_eq!(default_rounds.turns.len(), LOCAL_SANDBOX_DEFAULT_ROUNDS * 2);

        let capped_rounds = run_local_sandbox_simulation(
            &db,
            SimulationRunLocalSandboxRequest {
                mission_id: mission.id,
                baseline: "Baseline".to_string(),
                options: vec!["Option A".to_string(), "Option B".to_string()],
                agents: vec![SimulationSandboxAgentRequest {
                    name: "Ari".to_string(),
                    role: "Operator".to_string(),
                    stance: "balanced".to_string(),
                }],
                rounds: Some(999),
            },
        )
        .expect("capped rounds simulation should succeed");
        assert_eq!(capped_rounds.rounds, LOCAL_SANDBOX_MAX_ROUNDS);
        assert_eq!(capped_rounds.turns.len(), LOCAL_SANDBOX_MAX_ROUNDS * 2);
    }

    fn count_for(counts: &[SimulationCount], key: &str) -> usize {
        counts
            .iter()
            .find(|count| count.key == key)
            .map(|count| count.count)
            .unwrap_or_default()
    }

    fn update_mission_status(
        db: &Database,
        mission_id: &str,
        status: &str,
        last_activity_at: &str,
        priority: &str,
    ) {
        db.execute(
            "UPDATE missions SET status = ?, priority = ?, last_activity_at = ? WHERE id = ?",
            &[
                &status as &dyn rusqlite::ToSql,
                &priority,
                &last_activity_at,
                &mission_id,
            ],
        )
        .expect("mission should update");
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_run(
        db: &Database,
        run_id: &str,
        mission_id: &str,
        run_type: &str,
        status: &str,
        started_at: Option<&str>,
        finished_at: Option<&str>,
        summary: Option<&str>,
        error_message: Option<&str>,
    ) {
        db.execute(
            "INSERT INTO runs (
                id, mission_id, type, status, started_at, finished_at, summary, error_message
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            &[
                &run_id as &dyn rusqlite::ToSql,
                &mission_id,
                &run_type,
                &status,
                &started_at,
                &finished_at,
                &summary,
                &error_message,
            ],
        )
        .expect("run should insert");
    }

    fn update_scenario_created_at(db: &Database, scenario_run_id: &str, created_at: &str) {
        db.execute(
            "UPDATE scenario_runs SET created_at = ? WHERE id = ?",
            &[&created_at as &dyn rusqlite::ToSql, &scenario_run_id],
        )
        .expect("scenario run timestamp should update");
    }

    fn insert_legacy_scenario_run(
        db: &Database,
        mission_id: &str,
        baseline: &str,
        options: Vec<String>,
        recommendation: Option<&str>,
        created_at: &str,
    ) -> String {
        let scenario_run_id = Uuid::new_v4().to_string();
        let options_json =
            serde_json::to_string(&options).expect("legacy options json should serialize");
        db.execute(
            "INSERT INTO scenario_runs (
                id, mission_id, baseline, options_json, recommendation, created_at
            ) VALUES (?, ?, ?, ?, ?, ?)",
            &[
                &scenario_run_id as &dyn rusqlite::ToSql,
                &mission_id,
                &baseline,
                &options_json,
                &recommendation,
                &created_at,
            ],
        )
        .expect("legacy scenario should insert");
        scenario_run_id
    }

    fn seed_template_bundle_audit_log(
        db: &Database,
        entries: Vec<SimulationTemplateBundleAuditEntry>,
    ) {
        let now = "2026-04-27T00:00:00Z".to_string();
        let value_json = serde_json::to_string(&entries).expect("audit log should serialize");
        db.execute(
            "INSERT INTO app_settings (key, value_json, updated_at)
             VALUES (?, ?, ?)
             ON CONFLICT(key) DO UPDATE SET
                value_json = excluded.value_json,
                updated_at = excluded.updated_at",
            &[
                &TEMPLATE_BUNDLE_AUDIT_LOG_KEY as &dyn rusqlite::ToSql,
                &value_json,
                &now,
            ],
        )
        .expect("audit log should seed");
    }

    fn sample_template_bundle_audit_entry(
        id: &str,
        action: &str,
        occurred_at: &str,
    ) -> SimulationTemplateBundleAuditEntry {
        SimulationTemplateBundleAuditEntry {
            id: id.to_string(),
            action: action.to_string(),
            actor: "local-operator".to_string(),
            handoff_policy_template_count: 2,
            scoring_formula_template_count: 1,
            note: format!("Audit entry {id}"),
            occurred_at: occurred_at.to_string(),
        }
    }

    #[tokio::test]
    async fn external_saas_simulation_local_echo_persists_adapter_run() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("External SaaS simulation mission"))
            .expect("mission should be created");

        let response = run_external_saas_simulation_for_db(
            &db,
            SimulationRunExternalSaasRequest {
                mission_id: mission.id.clone(),
                provider: "local_echo".to_string(),
                endpoint_url: None,
                input_json: Some("{\"scenario\":\"upgrade\",\"reward_hint\":1}".to_string()),
                target_remote_user_id: Some("  remote-user-42  ".to_string()),
                dry_run: Some(false),
                confirmation_phrase: None,
                timeout_ms: None,
            },
        )
        .await
        .expect("local echo SaaS adapter should run");

        assert_eq!(response.provider, "local_echo");
        assert!(response.executed);
        assert!(!response.dry_run);
        assert!(!response.network_invocation);
        assert_eq!(
            response.target_remote_user_id.as_deref(),
            Some("remote-user-42")
        );
        assert!(response.response_body.contains("upgrade"));
        assert!(response.audit_event_id.is_some());

        let event_type = db
            .query_row(
                "SELECT event_type FROM run_events WHERE id = ?1",
                &[&response.audit_event_id.clone().expect("event id")],
                |row| row.get::<_, String>(0),
            )
            .expect("event should persist");
        assert_eq!(event_type, "external_saas_simulation_completed");

        let persisted_payload = db
            .query_row(
                "SELECT payload_json FROM run_events WHERE id = ?1",
                &[&response.audit_event_id.clone().expect("event id")],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("event payload should persist")
            .expect("payload should exist");
        let persisted: SimulationExternalSaasRun =
            serde_json::from_str(&persisted_payload).expect("persisted payload should decode");
        assert_eq!(
            persisted.target_remote_user_id.as_deref(),
            Some("remote-user-42")
        );
    }

    #[tokio::test]
    async fn external_saas_http_json_rejects_non_dry_run_without_confirmation() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("HTTP SaaS confirmation mission"))
            .expect("mission should be created");

        let error = run_external_saas_simulation_for_db(
            &db,
            SimulationRunExternalSaasRequest {
                mission_id: mission.id,
                provider: "http_json".to_string(),
                endpoint_url: Some("https://example.invalid/simulate".to_string()),
                input_json: Some("{\"scenario\":\"external\"}".to_string()),
                target_remote_user_id: None,
                dry_run: Some(false),
                confirmation_phrase: None,
                timeout_ms: None,
            },
        )
        .await
        .expect_err("external HTTP adapter should require confirmation");

        assert_eq!(error.code, "validation_error");
        assert!(error.message.contains("RUN EXTERNAL SAAS SIMULATION"));
    }

    #[tokio::test]
    async fn external_saas_http_json_dry_run_persists_preview_without_network() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("HTTP SaaS dry-run mission"))
            .expect("mission should be created");

        let response = run_external_saas_simulation_for_db(
            &db,
            SimulationRunExternalSaasRequest {
                mission_id: mission.id.clone(),
                provider: "http_json".to_string(),
                endpoint_url: Some("https://example.invalid/simulate".to_string()),
                input_json: Some(r#"{"scenario":"external-preview"}"#.to_string()),
                target_remote_user_id: Some("   dry-run-remote   ".to_string()),
                dry_run: Some(true),
                confirmation_phrase: None,
                timeout_ms: None,
            },
        )
        .await
        .expect("HTTP SaaS dry-run should not require confirmation or network");

        assert_eq!(response.provider, "http_json");
        assert!(response.dry_run);
        assert!(!response.executed);
        assert!(!response.network_invocation);
        assert_eq!(response.response_status, None);
        assert_eq!(
            response.endpoint_url.as_deref(),
            Some("https://example.invalid/simulate")
        );
        assert_eq!(
            response.target_remote_user_id.as_deref(),
            Some("dry-run-remote")
        );
        assert!(response.request_preview.contains("external-preview"));

        let body: serde_json::Value =
            serde_json::from_str(&response.response_body).expect("dry-run body should be JSON");
        assert_eq!(
            body.get("mode").and_then(serde_json::Value::as_str),
            Some("dry_run")
        );
        assert_eq!(
            body.get("network_invocation")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );

        let event_type = db
            .query_row(
                "SELECT event_type FROM run_events WHERE id = ?1",
                &[&response.audit_event_id.clone().expect("event id")],
                |row| row.get::<_, String>(0),
            )
            .expect("preview event should persist");
        assert_eq!(event_type, "external_saas_simulation_previewed");

        let persisted_payload = db
            .query_row(
                "SELECT payload_json FROM run_events WHERE id = ?1",
                &[&response.audit_event_id.clone().expect("event id")],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("preview payload should persist")
            .expect("payload should exist");
        let persisted: SimulationExternalSaasRun =
            serde_json::from_str(&persisted_payload).expect("persisted payload should decode");
        assert_eq!(
            persisted.target_remote_user_id.as_deref(),
            Some("dry-run-remote")
        );
    }

    #[tokio::test]
    async fn external_saas_history_lists_recent_runs_for_selected_mission() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("External SaaS history mission"))
            .expect("mission should be created");
        let other_mission = repo
            .create(sample_input("Other external SaaS history mission"))
            .expect("other mission should be created");

        let first = run_external_saas_simulation_for_db(
            &db,
            SimulationRunExternalSaasRequest {
                mission_id: mission.id.clone(),
                provider: "local_echo".to_string(),
                endpoint_url: None,
                input_json: Some(r#"{"scenario":"first"}"#.to_string()),
                target_remote_user_id: Some(" first-remote ".to_string()),
                dry_run: Some(false),
                confirmation_phrase: None,
                timeout_ms: None,
            },
        )
        .await
        .expect("first external SaaS run should persist");
        let second = run_external_saas_simulation_for_db(
            &db,
            SimulationRunExternalSaasRequest {
                mission_id: mission.id.clone(),
                provider: "http_json".to_string(),
                endpoint_url: Some("https://example.invalid/simulate".to_string()),
                input_json: Some(r#"{"scenario":"second"}"#.to_string()),
                target_remote_user_id: Some("".to_string()),
                dry_run: Some(true),
                confirmation_phrase: None,
                timeout_ms: None,
            },
        )
        .await
        .expect("second external SaaS run should persist");
        run_external_saas_simulation_for_db(
            &db,
            SimulationRunExternalSaasRequest {
                mission_id: other_mission.id,
                provider: "local_echo".to_string(),
                endpoint_url: None,
                input_json: Some(r#"{"scenario":"other"}"#.to_string()),
                target_remote_user_id: Some("other-remote".to_string()),
                dry_run: Some(false),
                confirmation_phrase: None,
                timeout_ms: None,
            },
        )
        .await
        .expect("other mission external SaaS run should persist");

        let history = list_external_saas_runs_for_db(
            &db,
            SimulationCapabilityRunListRequest {
                mission_id: Some(mission.id.clone()),
                limit: Some(10),
                target_remote_user_id: None,
            },
        )
        .expect("external SaaS history should load");

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].run_id, second.run_id);
        assert_eq!(history[1].run_id, first.run_id);
        assert!(
            history
                .iter()
                .all(|run| run.engine == EXTERNAL_SAAS_SIMULATION_ENGINE_NAME)
        );
        assert_eq!(history[0].target_remote_user_id, None);
        assert_eq!(
            history[1].target_remote_user_id.as_deref(),
            Some("first-remote")
        );

        let filtered = list_external_saas_runs_for_db(
            &db,
            SimulationCapabilityRunListRequest {
                mission_id: Some(mission.id.clone()),
                limit: Some(1),
                target_remote_user_id: Some("  first-remote  ".to_string()),
            },
        )
        .expect("external SaaS history should filter by target remote user");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].run_id, first.run_id);
        assert_eq!(
            filtered[0].target_remote_user_id.as_deref(),
            Some("first-remote")
        );

        let unmatched = list_external_saas_runs_for_db(
            &db,
            SimulationCapabilityRunListRequest {
                mission_id: Some(mission.id),
                limit: Some(10),
                target_remote_user_id: Some("missing-remote".to_string()),
            },
        )
        .expect("external SaaS history should return empty for unknown target remote user");
        assert!(unmatched.is_empty());
    }

    #[test]
    fn high_fidelity_sandbox_builds_world_model_and_persists_event() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("High fidelity sandbox mission"))
            .expect("mission should be created");

        let response = run_high_fidelity_sandbox_simulation(
            &db,
            SimulationRunHighFidelitySandboxRequest {
                mission_id: mission.id.clone(),
                baseline: "Protect launch quality while increasing support coverage".to_string(),
                options: vec![
                    "Add support specialists during launch".to_string(),
                    "Delay launch to reduce operational risk".to_string(),
                ],
                agents: vec![SimulationSandboxAgentRequest {
                    name: "Ari".to_string(),
                    role: "Operations lead".to_string(),
                    stance: "quality balanced".to_string(),
                }],
                rounds: Some(2),
                variables: vec![ScenarioVariable {
                    id: "coverage".to_string(),
                    label: "Support coverage".to_string(),
                    current_value: "weekday only".to_string(),
                    proposed_value: "launch window".to_string(),
                    impact: "high".to_string(),
                    uncertainty: "medium".to_string(),
                    impact_weight: 0.8,
                    uncertainty_weight: 0.3,
                }],
                target_remote_user_id: None,
            },
        )
        .expect("high fidelity sandbox should run");

        assert_eq!(response.engine, "local_high_fidelity_world_sandbox");
        assert_eq!(response.base_run.turns.len(), 4);
        assert!(!response.world.entities.is_empty());
        assert!(!response.world.variables.is_empty());
        assert_eq!(response.world.timeline.len(), response.base_run.turns.len());
        assert!(!response.world.event_graph.nodes.is_empty());
        assert!(!response.world.event_graph.edges.is_empty());
        assert!(!response.world.option_metric_heatmap.is_empty());
        assert!(response.audit_event_id.is_some());

        let payload = db
            .query_row(
                "SELECT payload_json FROM run_events WHERE id = ?1",
                &[&response.audit_event_id.clone().expect("event id")],
                |row| row.get::<_, Option<String>>(0),
            )
            .expect("event should persist")
            .expect("payload should exist");
        assert!(payload.contains("event_graph"));
        assert!(payload.contains("option_metric_heatmap"));
    }

    #[test]
    fn high_fidelity_sandbox_history_lists_newest_runs_and_applies_limit() {
        let db = Database::in_memory().expect("database should initialize");
        let repo = MissionRepository::new(db.clone());
        let mission = repo
            .create(sample_input("High fidelity history mission"))
            .expect("mission should be created");

        let first = run_high_fidelity_sandbox_simulation(
            &db,
            SimulationRunHighFidelitySandboxRequest {
                mission_id: mission.id.clone(),
                baseline: "Protect quality".to_string(),
                options: vec![
                    "Add launch support".to_string(),
                    "Delay rollout".to_string(),
                ],
                agents: Vec::new(),
                rounds: Some(1),
                variables: Vec::new(),
                target_remote_user_id: Some("  first-remote-hf  ".to_string()),
            },
        )
        .expect("first high fidelity run should persist");
        let second = run_high_fidelity_sandbox_simulation(
            &db,
            SimulationRunHighFidelitySandboxRequest {
                mission_id: mission.id.clone(),
                baseline: "Increase support coverage".to_string(),
                options: vec!["Add specialists".to_string(), "Freeze scope".to_string()],
                agents: Vec::new(),
                rounds: Some(1),
                variables: Vec::new(),
                target_remote_user_id: Some(" second-remote-hf ".to_string()),
            },
        )
        .expect("second high fidelity run should persist");
        run_local_sandbox_simulation(
            &db,
            SimulationRunLocalSandboxRequest {
                mission_id: mission.id.clone(),
                baseline: "Local sandbox baseline".to_string(),
                options: vec!["Option A".to_string(), "Option B".to_string()],
                agents: Vec::new(),
                rounds: Some(1),
            },
        )
        .expect("local sandbox run should not leak into high fidelity history");

        let history = list_high_fidelity_sandbox_runs_for_db(
            &db,
            SimulationCapabilityRunListRequest {
                mission_id: Some(mission.id.clone()),
                limit: Some(1),
                target_remote_user_id: None,
            },
        )
        .expect("high fidelity history should load");

        assert_eq!(history.len(), 1);
        assert_eq!(history[0].run_id, second.run_id);
        assert_ne!(history[0].run_id, first.run_id);
        assert_eq!(history[0].engine, HIGH_FIDELITY_SANDBOX_ENGINE_NAME);
        assert_eq!(
            history[0].target_remote_user_id.as_deref(),
            Some("second-remote-hf")
        );

        let filtered = list_high_fidelity_sandbox_runs_for_db(
            &db,
            SimulationCapabilityRunListRequest {
                mission_id: Some(mission.id),
                limit: Some(1),
                target_remote_user_id: Some(" first-remote-hf ".to_string()),
            },
        )
        .expect("high fidelity history should filter by target remote user before limit");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].run_id, first.run_id);
        assert_eq!(
            filtered[0].target_remote_user_id.as_deref(),
            Some("first-remote-hf")
        );
    }

    #[test]
    fn decode_capability_run_payload_defaults_missing_target_remote_user_id() {
        let decoded =
            decode_capability_run_payload::<SimulationExternalSaasRun>(&CapabilityRunPayload {
                payload_json: serde_json::json!({
                    "run_id": "run-123",
                    "mission_id": "mission-123",
                    "engine": EXTERNAL_SAAS_SIMULATION_ENGINE_NAME,
                    "provider": "local_echo",
                    "endpoint_url": null,
                    "dry_run": true,
                    "executed": false,
                    "network_invocation": false,
                    "request_preview": "{}",
                    "response_status": null,
                    "response_body": "{}",
                    "summary": "legacy payload",
                    "audit_event_id": null
                })
                .to_string(),
                created_at: "2026-01-01T00:00:00Z".to_string(),
                status: Some("completed".to_string()),
            })
            .expect("legacy payload should decode");

        assert_eq!(decoded.target_remote_user_id, None);
    }
}
