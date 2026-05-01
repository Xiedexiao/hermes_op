/**
 * Tauri API 调用封装
 *
 * 提供与 Rust 后端命令交互的类型安全接口
 */

import { invoke } from "@tauri-apps/api/core";

// ============ 类型定义 ============

export interface AppSettings {
  theme_mode?: string;
  language?: string;
  launch_at_login?: boolean;
  default_workspace_path?: string;
  log_level?: string;
  require_approval_for_risk?: string;
}

export interface NativeCuaModelProfileSettings {
  provider?: string | null;
  model?: string | null;
  base_url?: string | null;
  api_key_ref?: string | null;
}

export interface NativeCuaAutoModelSettings {
  easy?: NativeCuaModelProfileSettings | null;
  standard?: NativeCuaModelProfileSettings | null;
  hard?: NativeCuaModelProfileSettings | null;
}

export interface RuntimeSettings {
  provider?: string;
  model?: string;
  base_url?: string;
  api_key_ref?: string;
  engine_profile?: string;
  agent_engine_enabled?: boolean;
  busy_input_mode?: string;
  native_cua_auto_models?: NativeCuaAutoModelSettings | null;
}

export interface EngineStatus {
  running: boolean;
  profile: string | null;
  pid: number | null;
  started_at?: string | null;
  last_heartbeat_at?: string | null;
  queued_background_runs?: number;
  awaiting_approval_steps?: number;
  last_error: string | null;
}

export interface AppRuntimeStatus {
  installed: boolean;
  running: boolean;
  version: string | null;
}

export interface ForegroundSnapshot {
  active: boolean;
  state: string;
  session_id: string | null;
  run_id: string | null;
  cancel_state: string | null;
  pending_count: number;
  interrupt_count: number;
  updated_at: string;
}

interface RawBootstrapPayload {
  app_settings: AppSettings;
  runtime_settings: RuntimeSettings;
  engine_status: EngineStatus;
  hermes_status: AppRuntimeStatus;
  foreground_snapshot: ForegroundSnapshot;
  active_session?: ActiveSessionSelection | null;
  active_mission?: Mission | null;
  summary: {
    active_mission_count: number;
    pending_approval_count: number;
    recent_session_count: number;
    has_recent_session: boolean;
  };
}

interface RawRuntimeStatusResponse {
  engine: {
    running: boolean;
    profile: string | null;
    pid: number | null;
    started_at?: string | null;
    last_heartbeat_at?: string | null;
    queued_background_runs?: number;
    awaiting_approval_steps?: number;
    last_error?: string | null;
  };
  hermes: {
    installed: boolean;
    running: boolean;
    version?: string | null;
  };
  foreground_snapshot: ForegroundSnapshot;
}

export interface BootstrapPayload {
  app_settings: AppSettings;
  runtime_settings: RuntimeSettings;
  engine_status: EngineStatus;
  app_runtime_status: AppRuntimeStatus;
  foreground_snapshot: ForegroundSnapshot;
  active_session?: ActiveSessionSelection | null;
  active_mission?: Mission | null;
  summary: {
    active_mission_count: number;
    pending_approval_count: number;
    recent_session_count: number;
    has_recent_session: boolean;
  };
}

export interface RuntimeStatusResponse {
  engine: {
    running: boolean;
    profile: string | null;
    pid: number | null;
    started_at?: string | null;
    last_heartbeat_at?: string | null;
    queued_background_runs?: number;
    awaiting_approval_steps?: number;
    last_error?: string | null;
  };
  appRuntime: {
    installed: boolean;
    running: boolean;
    version: string | null;
  };
  foreground: ForegroundSnapshot;
}


export interface SkillToolRequest {
  command: string;
  args?: string[];
  cwd?: string | null;
  timeout_ms?: number | null;
}

export interface SkillToolResponse {
  exit_code: number;
  stdout: string;
  stderr: string;
  duration_ms: number;
  timed_out: boolean;
  audit_message: string;
}

export interface DesktopExecutorProbe {
  platform: string;
  session_type?: string | null;
  display?: string | null;
  wayland_display?: string | null;
  has_graphical_session: boolean;
  tool_availability: Record<string, boolean>;
}

export interface DesktopActionRequest {
  executor: string;
  args?: string[];
  dry_run?: boolean | null;
  confirmation_phrase?: string | null;
}

export interface DesktopActionResponse {
  executed: boolean;
  planned_command: string[];
  exit_code?: number | null;
  stdout: string;
  stderr: string;
  duration_ms: number;
  timed_out: boolean;
  audit_message: string;
}

export interface GuiAutomationStepRequest {
  label?: string | null;
  executor: string;
  args?: string[];
}

export interface GuiAutomationRequest {
  steps: GuiAutomationStepRequest[];
  dry_run?: boolean | null;
  confirmation_phrase?: string | null;
  stop_on_error?: boolean | null;
  target_remote_user_id?: string | null;
}

export interface GuiAutomationStepResult {
  index: number;
  label?: string | null;
  status: string;
  response: DesktopActionResponse;
}

export interface GuiAutomationResponse {
  executed: boolean;
  dry_run: boolean;
  target_remote_user_id?: string | null;
  step_count: number;
  completed_count: number;
  planned_commands: string[][];
  results: GuiAutomationStepResult[];
  audit_message: string;
}

export interface TrajectorySummaryRequest {
  jsonl: string;
}

export interface TrajectorySummaryResponse {
  line_count: number;
  kind_counts: Record<string, number>;
  source_counts: Record<string, number>;
  reward_hint_count: number;
  invalid_line_count: number;
}

export interface RuntimeAdapterAuditEvent {
  id: string;
  occurred_at: string;
  kind: string;
  status: string;
  summary: string;
  duration_ms?: number | null;
  timed_out: boolean;
  target?: string | null;
  target_remote_user_id?: string | null;
  exit_code?: number | null;
}

export interface RuntimeAdapterAuditListRequest {
  limit?: number | null;
  kind?: string | null;
  status?: string | null;
  target_remote_user_id?: string | null;
}

export interface RuntimeAdapterAuditExportRequest {
  limit?: number | null;
  kind?: string | null;
  status?: string | null;
  format?: string | null;
  target_remote_user_id?: string | null;
}

export interface RuntimeAdapterAuditExportResponse {
  total: number;
  exported_count: number;
  format: string;
  payload: string;
  events: RuntimeAdapterAuditEvent[];
}

export interface TurixCuaProbeRequest {
  repo_path?: string | null;
}

export interface TurixCuaConfigSummary {
  task_present: boolean;
  resume: boolean;
  agent_id_present: boolean;
  has_template_api_key: boolean;
}

export interface TurixCuaProbe {
  status: string;
  repo_path: string;
  repo_exists: boolean;
  config_path: string;
  config_exists: boolean;
  run_script_path: string;
  run_script_exists: boolean;
  run_script_ready: boolean;
  python_entry_path: string;
  python_entry_exists: boolean;
  requirements_path: string;
  requirements_exists: boolean;
  permission_hints: string[];
  notes: string[];
  config_summary?: TurixCuaConfigSummary | null;
}

export interface TurixCuaRunRequest {
  repo_path?: string | null;
  task?: string | null;
  resume_agent_id?: string | null;
  config_path?: string | null;
  launcher?: string | null;
  dry_run?: boolean | null;
}

export interface TurixCuaRunResponse {
  executed: boolean;
  dry_run: boolean;
  launcher: string;
  repo_path: string;
  base_config_path: string;
  derived_config_path?: string | null;
  planned_command: string[];
  pid?: number | null;
  audit_message: string;
}

export interface TurixCuaAuditEvent {
  id: string;
  occurred_at: string;
  action: string;
  status: string;
  launcher: string;
  dry_run: boolean;
  summary: string;
  repo_path: string;
  command: string[];
  resume_agent_id?: string | null;
  pid?: number | null;
}

export interface TurixCuaAuditListRequest {
  limit?: number | null;
  action?: string | null;
  status?: string | null;
}

export type TurixCuaAuditExportFormat = "json" | "jsonl";

export interface TurixCuaAuditExportRequest {
  limit?: number | null;
  action?: string | null;
  status?: string | null;
  format: TurixCuaAuditExportFormat;
}

export interface TurixCuaAuditExportResponse {
  total: number;
  exported_count: number;
  format: string;
  payload: string;
  events: TurixCuaAuditEvent[];
}

export interface NativeCuaProbe {
  readiness: string;
  available: boolean;
  platform?: string | null;
  safety_mode?: string | null;
  active_session_id?: string | null;
  notes: string[];
  warnings: string[];
  capabilities: string[];
}

export interface NativeCuaStartSessionRequest {
  task: string;
  session_id?: string | null;
  model_mode?: "auto" | "custom" | string | null;
  provider?: string | null;
  model?: string | null;
  base_url?: string | null;
  api_key_ref?: string | null;
}

export interface NativeCuaPreviewModelRouteRequest {
  task: string;
  model_mode?: "auto" | "custom" | string | null;
  provider?: string | null;
  model?: string | null;
  base_url?: string | null;
  api_key_ref?: string | null;
}

export interface NativeCuaModelRoutePreview {
  model_mode: string;
  provider?: string | null;
  model?: string | null;
  base_url?: string | null;
  api_key_ref?: string | null;
  model_difficulty?: string | null;
  model_selection_reason?: string | null;
  summary: string;
}

export interface NativeCuaSessionResponse {
  session_id: string;
  status: string;
  task?: string | null;
  resumed: boolean;
  created_at?: string | null;
  updated_at?: string | null;
  summary?: string | null;
  model_mode?: string | null;
  provider?: string | null;
  model?: string | null;
  base_url?: string | null;
  api_key_ref?: string | null;
  model_difficulty?: string | null;
  model_selection_reason?: string | null;
}

export interface NativeCuaObserveRequest {
  session_id?: string | null;
  dry_run?: boolean | null;
  capture_screenshot?: boolean | null;
}

export interface NativeCuaObserveResponse {
  session_id: string;
  dry_run: boolean;
  capture_screenshot: boolean;
  screenshot_captured?: boolean | null;
  screenshot_path?: string | null;
  status: string;
  summary?: string | null;
  observation?: unknown;
}

export type NativeCuaActionType =
  | "wait"
  | "done"
  | "click"
  | "double_click"
  | "right_click"
  | "type_text"
  | "press_key"
  | "hotkey"
  | "launch_app"
  | "move_pointer"
  | "drag_pointer"
  | "scroll"
  | "run_apple_script";

export interface NativeCuaExecuteActionRequest {
  session_id?: string | null;
  action_type: NativeCuaActionType;
  text?: string | null;
  key?: string | null;
  modifiers?: string[] | null;
  app?: string | null;
  x?: number | null;
  y?: number | null;
  dx?: number | null;
  dy?: number | null;
  dry_run?: boolean | null;
  confirmation_phrase?: string | null;
}

export interface NativeCuaExecuteActionResponse {
  session_id: string;
  action_type: string;
  dry_run: boolean;
  executed: boolean;
  status: string;
  summary?: string | null;
  audit_message?: string | null;
  planned_command?: string[];
}

export interface NativeCuaSkillMetadata {
  name: string;
  description: string;
}

export interface NativeCuaPlanTaskRequest {
  session_id?: string | null;
  task?: string | null;
  skill_catalog?: NativeCuaSkillMetadata[] | null;
  max_steps?: number | null;
}

export interface NativeCuaPlanStep {
  index: number;
  goal: string;
  suggested_action: string;
  status: string;
}

export interface NativeCuaPlanResponse {
  session_id: string;
  task: string;
  created_at: string;
  updated_at: string;
  source: string;
  status: string;
  selected_skills: string[];
  iteration_info: unknown;
  steps: NativeCuaPlanStep[];
  summary: string;
}

export interface NativeCuaRunStepRequest {
  session_id?: string | null;
  dry_run?: boolean | null;
  capture_screenshot?: boolean | null;
  brain_state?: unknown;
  actions?: unknown[] | null;
  max_actions?: number | null;
  confirmation_phrase?: string | null;
}

export interface NativeCuaMemoryRecord {
  id: string;
  session_id: string;
  file_name: string;
  text: string;
  path: string;
  screenshot_path?: string | null;
  created_at: string;
}

export interface NativeCuaStepActionResult {
  action_name: string;
  raw_action: unknown;
  status: string;
  summary: string;
  native_result?: NativeCuaExecuteActionResponse | null;
  memory_record?: NativeCuaMemoryRecord | null;
  is_done: boolean;
}

export interface NativeCuaStepRecord {
  id: string;
  session_id: string;
  step_index: number;
  occurred_at: string;
  status: string;
  brain_state: unknown;
  observation?: NativeCuaObserveResponse | null;
  actions: NativeCuaStepActionResult[];
  final_result?: string | null;
  summary: string;
}

export interface NativeCuaRunStepResponse {
  session_id: string;
  step: NativeCuaStepRecord;
  history_len: number;
  done: boolean;
  summary: string;
}

export interface NativeCuaHistoryListRequest {
  session_id?: string | null;
  limit?: number | null;
  status?: string | null;
}

export interface NativeCuaRecordInfoRequest {
  session_id?: string | null;
  text: string;
  file_name: string;
  screenshot_path?: string | null;
}

export interface NativeCuaTrajectoryExportRequest {
  session_id?: string | null;
  format?: NativeCuaAuditExportFormat | null;
  include_audit?: boolean | null;
}

export interface NativeCuaTrajectoryExportResponse {
  session_id?: string | null;
  format: string;
  exported_count: number;
  payload: string;
}

export type NativeCuaModelRole = "brain" | "actor" | "planner" | "memory";

export interface NativeCuaModelTurnRequest {
  session_id?: string | null;
  role: NativeCuaModelRole;
  include_screenshot_data_url?: boolean | null;
  max_history?: number | null;
  extra_context?: string | null;
}

export interface NativeCuaPromptMessage {
  role: string;
  content: string;
  attachments: unknown[];
}

export interface NativeCuaModelTurnResponse {
  id: string;
  session_id: string;
  role: NativeCuaModelRole;
  provider?: string | null;
  model?: string | null;
  messages: NativeCuaPromptMessage[];
  response_schema: unknown;
  action_catalog: string[];
  created_at: string;
  summary: string;
}

export interface NativeCuaInvokeModelRequest {
  session_id?: string | null;
  role: NativeCuaModelRole;
  provider?: string | null;
  model?: string | null;
  base_url?: string | null;
  api_key_ref?: string | null;
  dry_run?: boolean | null;
  apply_output?: boolean | null;
  capture_screenshot?: boolean | null;
  extra_context?: string | null;
  model_confirmation_phrase?: string | null;
  action_confirmation_phrase?: string | null;
}

export interface NativeCuaApplyModelOutputRequest {
  session_id?: string | null;
  role: NativeCuaModelRole;
  output: unknown;
  dry_run?: boolean | null;
  capture_screenshot?: boolean | null;
  confirmation_phrase?: string | null;
}

export interface NativeCuaApplyModelOutputResponse {
  session_id: string;
  role: NativeCuaModelRole;
  status: string;
  output: unknown;
  step_result?: NativeCuaRunStepResponse | null;
  summary: string;
}

export interface NativeCuaInvokeModelResponse {
  session_id: string;
  role: NativeCuaModelRole;
  provider?: string | null;
  model?: string | null;
  dry_run: boolean;
  requested: boolean;
  status: string;
  prompt_turn: NativeCuaModelTurnResponse;
  http_request_preview: unknown;
  raw_output?: unknown;
  parsed_output?: unknown;
  apply_result?: NativeCuaApplyModelOutputResponse | null;
  summary: string;
}

export interface NativeCuaAuditEvent {
  id: string;
  occurred_at: string;
  event_type: string;
  status: string;
  session_id?: string | null;
  dry_run?: boolean | null;
  summary: string;
}

export interface NativeCuaAuditListRequest {
  limit?: number | null;
  session_id?: string | null;
  event_type?: string | null;
  status?: string | null;
}

export type NativeCuaAuditExportFormat = "json" | "jsonl";

export interface NativeCuaAuditExportRequest {
  limit?: number | null;
  session_id?: string | null;
  event_type?: string | null;
  status?: string | null;
  format: NativeCuaAuditExportFormat;
}

export interface NativeCuaAuditExportResponse {
  total: number;
  exported_count: number;
  format: string;
  payload: string;
  events: NativeCuaAuditEvent[];
}

export type AgentExchangeDirection = "inbound" | "outbound";
export type AgentExchangeMessageStatus = "draft" | "sent" | "received" | "archived";
export type AgentExchangeRemoteUserStatus = "active" | "paused" | "blocked";

export interface AgentExchangeMessage {
  id: string;
  thread_id: string;
  direction: AgentExchangeDirection;
  local_agent_id: string;
  remote_agent_id: string;
  remote_user_id?: string | null;
  subject?: string | null;
  body: string;
  payload_json?: unknown | null;
  status: AgentExchangeMessageStatus;
  source_message_id?: string | null;
  created_at: string;
  updated_at: string;
}

export interface AgentExchangeRemoteUser {
  user_id: string;
  display_name: string;
  default_agent_id: string;
  transport_label?: string | null;
  route_hint?: string | null;
  status: AgentExchangeRemoteUserStatus;
  created_at: string;
  updated_at: string;
}

export interface AgentExchangeState {
  schema_version: number;
  messages: AgentExchangeMessage[];
  remote_users: AgentExchangeRemoteUser[];
  last_imported_at?: string | null;
  last_exported_at?: string | null;
}

export interface AgentExchangeBundle {
  schema_version: number;
  exported_at: string;
  messages: AgentExchangeMessage[];
  remote_users: AgentExchangeRemoteUser[];
}

export interface AgentExchangeListRequest {
  direction?: AgentExchangeDirection | null;
  status?: AgentExchangeMessageStatus | null;
  thread_id?: string | null;
  remote_agent_id?: string | null;
  remote_user_id?: string | null;
  limit?: number | null;
}

export interface AgentExchangeDraftOutboundRequest {
  local_agent_id: string;
  remote_agent_id: string;
  remote_user_id?: string | null;
  thread_id?: string | null;
  subject?: string | null;
  body: string;
  payload_json?: unknown | null;
}

export interface AgentExchangeIngestInboundRequest {
  local_agent_id: string;
  remote_agent_id: string;
  remote_user_id?: string | null;
  thread_id?: string | null;
  subject?: string | null;
  body: string;
  payload_json?: unknown | null;
  source_message_id?: string | null;
}

export interface AgentExchangeExportBundleRequest extends AgentExchangeListRequest {}

export interface AgentExchangeListRemoteUsersRequest {
  query?: string | null;
  status?: AgentExchangeRemoteUserStatus | null;
  limit?: number | null;
}

export interface AgentExchangeUpsertRemoteUserRequest {
  user_id: string;
  display_name: string;
  default_agent_id: string;
  transport_label?: string | null;
  route_hint?: string | null;
  status: AgentExchangeRemoteUserStatus;
}

export interface AgentExchangeDeleteRemoteUserRequest {
  user_id: string;
}

export interface AgentExchangeImportBundleRequest {
  bundle: AgentExchangeBundle;
  local_agent_id?: string | null;
  as_inbound?: boolean | null;
}

export interface AgentExchangeImportBundleResponse {
  state: AgentExchangeState;
  imported_count: number;
  skipped_count: number;
}

export interface AgentExchangeUpdateMessageStatusRequest {
  message_id: string;
  status: AgentExchangeMessageStatus;
}

export interface AgentExchangeDeleteMessageRequest {
  message_id: string;
}

export interface AgentExchangeRunFolderSyncRequest {
  path: string;
  local_agent_id?: string | null;
  as_inbound?: boolean | null;
}

export interface AgentExchangeRunFolderSyncResponse {
  state: AgentExchangeState;
  imported_count: number;
  skipped_count: number;
  exported_count: number;
  path: string;
  synced_at: string;
}

export type TeamRole = "owner" | "admin" | "editor" | "viewer";

export interface TeamMember {
  id: string;
  role: TeamRole;
  updated_at: string;
}

export interface TeamRolePolicy {
  role: TeamRole;
  allowed: string[];
}

export interface TeamAuditEvent {
  id: string;
  at: string;
  action: string;
  actor_member_id: string;
  subject_member_id?: string | null;
  detail: string;
}

export interface TeamSyncState {
  schema_version: number;
  members: TeamMember[];
  roles: TeamRolePolicy[];
  audit_events: TeamAuditEvent[];
  last_synced_at?: string | null;
}

export interface TeamSyncUpsertMemberRequest {
  actor_member_id: string;
  member_id: string;
  role: TeamRole;
}

export interface TeamSyncCheckAccessRequest {
  actor_member_id: string;
  resource: string;
  action: string;
}

export interface TeamSyncAccessDecision {
  allowed: boolean;
  reason: string;
}

export interface TeamSyncBundle {
  schema_version: number;
  exported_at: string;
  members: TeamMember[];
  roles: TeamRolePolicy[];
  audit_events: TeamAuditEvent[];
}

export type TeamSyncAuditExportFormat = "json" | "jsonl";

export interface TeamSyncExportAuditRequest {
  actor_member_id: string;
  actor?: string | null;
  action?: string | null;
  limit?: number | null;
  format: TeamSyncAuditExportFormat;
}

export interface TeamSyncExportAuditResponse {
  total: number;
  exported_count: number;
  payload: string;
  events: TeamAuditEvent[];
}

export interface TeamSyncExportBundleRequest {
  actor_member_id: string;
}

export interface TeamSyncImportBundleRequest {
  actor_member_id: string;
  bundle: TeamSyncBundle;
}

export interface TeamSyncRunFolderSyncRequest {
  actor_member_id: string;
  file_path?: string | null;
}

export interface TeamSyncRunFolderSyncResponse {
  state: TeamSyncState;
  bundle?: TeamSyncBundle | null;
}

export type MissionStatus =
  | "draft"
  | "researching"
  | "simulating"
  | "planning"
  | "awaiting_approval"
  | "executing"
  | "paused"
  | "completed"
  | "failed"
  | "archived";

export type MissionPriority = "low" | "medium" | "high";

export interface Mission {
  id: string;
  title: string;
  goal: string;
  constraints: string[];
  success_criteria: string[];
  status: MissionStatus;
  priority: MissionPriority;
  pinned: boolean;
  created_at: string;
  updated_at: string;
  last_activity_at: string;
}

export interface MissionContextItem {
  id: string;
  mission_id: string;
  type: "file" | "url" | "note" | "memory" | "knowledge_result" | "artifact";
  title: string;
  content_preview?: string | null;
  source_uri?: string | null;
  pinned: boolean;
  created_at: string;
}

export interface Run {
  id: string;
  mission_id: string;
  type: "research" | "simulation" | "council" | "execution" | "growth";
  status: "queued" | "running" | "completed" | "failed" | "cancelled";
  started_at?: string | null;
  finished_at?: string | null;
  summary?: string | null;
  error_message?: string | null;
}

export interface Artifact {
  id: string;
  mission_id: string;
  run_id?: string | null;
  type: "markdown" | "report" | "plan" | "json" | "text" | "image" | "file";
  title: string;
  path: string;
  mime_type?: string | null;
  created_at: string;
}

export interface MissionDetail {
  mission: Mission;
  context_items: MissionContextItem[];
  runs: Run[];
  artifacts: Artifact[];
}

export interface MissionListRequest {
  query?: string;
  status?: MissionStatus;
  limit?: number;
}

export interface MissionCreateRequest {
  title: string;
  goal: string;
  constraints: string[];
  success_criteria: string[];
  priority: MissionPriority;
}

export interface MissionUpdateRequest {
  id: string;
  title: string;
  goal: string;
  constraints: string[];
  success_criteria: string[];
  priority: MissionPriority;
}

export interface MissionPinnedRequest {
  id: string;
  pinned: boolean;
}

export interface MissionStatusRequest {
  id: string;
  status: MissionStatus;
}

export interface GeneratedMissionPlan {
  run: Run;
  steps: ExecutionStep[];
}

export type ExecutionMode = "api" | "cli" | "browser" | "desktop";
export type RiskLevel = "low" | "medium" | "high";
export type ExecutionStepStatus =
  | "pending"
  | "awaiting_approval"
  | "running"
  | "paused"
  | "completed"
  | "failed"
  | "skipped";

export interface ExecutionStep {
  id: string;
  mission_id: string;
  run_id: string;
  title: string;
  mode: ExecutionMode;
  risk_level: RiskLevel;
  status: ExecutionStepStatus;
  input_payload?: string | null;
  output_summary?: string | null;
  created_at: string;
  updated_at: string;
}

export interface ExecutionRunCliStepRequest {
  id: string;
  cwd?: string | null;
}

export interface ExecutionStepNoteRequest {
  id: string;
  note: string;
  pause_before_continue?: boolean | null;
}

export interface ExecutionPrepareDesktopHandoffRequest {
  id: string;
}

export interface ExecutionMarkDesktopHandoffReviewedRequest {
  run_id: string;
  step_id: string;
  review_note?: string | null;
}

export interface ExecutionDesktopHandoff {
  step_id: string;
  mission_id: string;
  run_id: string;
  title: string;
  status: string;
  risk_level: string;
  automatic_execution: boolean;
  reason: string;
  checklist: string[];
  input_payload?: unknown;
  handoff_prompt: string;
}

export interface ExecutionDesktopHandoffQueueRequest {
  mission_id?: string | null;
}

export interface ExecutionDesktopHandoffQueueItem {
  step: ExecutionStep;
  handoff_prepared: boolean;
  prepared_event_count: number;
  latest_prepared_at?: string | null;
  handoff_reviewed: boolean;
  reviewed_event_count: number;
  latest_reviewed_at?: string | null;
}

export interface SettingsPayload {
  app: AppSettings;
  runtime: RuntimeSettings;
}

export interface SaveSettingsRequest {
  app?: AppSettings;
  runtime?: RuntimeSettings;
}

export interface SaveSettingsResponse {
  ok: boolean;
}

export type SessionSource =
  | "cli"
  | "desktop"
  | "telegram"
  | "discord"
  | "slack"
  | "whatsapp"
  | "signal"
  | "email"
  | "cron"
  | "unknown";

export interface Session {
  id: string;
  source: SessionSource;
  title: string;
  model_name?: string | null;
  parent_session_id?: string | null;
  started_at: string;
  updated_at: string;
  ended_at?: string | null;
}

export interface ActiveSessionSelection {
  session: Session;
  reason: string;
  activated_at: string;
}

export type SessionMessageRole =
  | "user"
  | "assistant"
  | "system"
  | "tool"
  | "note";

export interface SessionMessage {
  id: string;
  session_id: string;
  role: SessionMessageRole;
  content: string;
  source: string;
  created_at: string;
}

export interface SessionRenameRequest {
  id: string;
  title: string;
}

export interface SessionActivateRequest {
  id: string;
  reason?: string | null;
}

export interface SessionMessageListRequest {
  session_id: string;
  limit?: number | null;
  role?: SessionMessageRole | null;
  query?: string | null;
}

export interface SessionReplaySnapshotRequest {
  session_id?: string | null;
  limit?: number | null;
}

export interface SessionMessageCreateRequest {
  session_id: string;
  role: SessionMessageRole;
  content: string;
  source?: string | null;
}

export interface SessionReplaySnapshot {
  resolved_via: "session_id" | "active_session" | "latest_session" | "none";
  session?: Session | null;
  active_selection?: ActiveSessionSelection | null;
  messages: SessionMessage[];
}

export interface TerminalBackendProfile {
  id: string;
  kind: string;
  display_name: string;
  enabled: boolean;
  config: Record<string, unknown>;
  created_at: string;
  updated_at: string;
}

export interface TerminalBackendSaveProfileRequest {
  id?: string | null;
  kind: string;
  display_name: string;
  enabled: boolean;
  config: Record<string, unknown>;
}

export interface TerminalBackendStatus {
  id: string;
  kind: string;
  display_name: string;
  enabled: boolean;
  availability: string;
  configured: boolean;
  testable: boolean;
  required_command?: string | null;
  message: string;
}

export interface TerminalBackendTestResult {
  id: string;
  kind: string;
  status: string;
  availability: string;
  message: string;
}

export type GatewaySource =
  | "telegram"
  | "discord"
  | "slack"
  | "whatsapp"
  | "signal"
  | "email";

export type GatewayMessageDirection = "inbound" | "outbound";

export interface GatewayConversation {
  id: string;
  source: GatewaySource;
  external_conversation_id: string;
  external_thread_id: string;
  channel_name?: string | null;
  participant_display?: string | null;
  session_id: string;
  last_message_at: string;
  created_at: string;
}

export interface GatewayMessage {
  id: string;
  conversation_id: string;
  session_id: string;
  source: GatewaySource;
  external_message_id: string;
  direction: GatewayMessageDirection;
  sender_id?: string | null;
  sender_display?: string | null;
  subject?: string | null;
  body: string;
  payload_json?: unknown;
  received_at: string;
}

export interface GatewayIngestedMessage {
  conversation: GatewayConversation;
  message: GatewayMessage;
}

export interface GatewayIngestMessageRequest {
  source: GatewaySource;
  external_conversation_id: string;
  external_thread_id?: string | null;
  external_message_id: string;
  channel_name?: string | null;
  participant_display?: string | null;
  direction?: GatewayMessageDirection | null;
  sender_id?: string | null;
  sender_display?: string | null;
  subject?: string | null;
  body: string;
  payload_json?: unknown;
  received_at?: string | null;
}

export interface GatewayListRecentConversationsRequest {
  limit?: number;
}

export interface GatewayListRecentMessagesRequest {
  conversation_id?: string | null;
  session_id?: string | null;
  limit?: number;
}

export interface SkillListItem {
  name: string;
  display_name: string;
  source: string;
  path: string;
  enabled: boolean;
}

export interface KnowledgeFeedItem {
  id: string;
  mission_id: string;
  mission_title: string;
  source_kind: string;
  item_type: string;
  title: string;
  preview?: string | null;
  source?: string | null;
  path?: string | null;
  created_at: string;
}

export interface KnowledgeSource {
  id: string;
  type: "file" | "url" | "note" | "memory" | "knowledge_result" | "artifact";
  title: string;
  source_uri: string;
  index_status: string;
  chunk_count: number;
  updated_at: string;
}

export interface KnowledgeListRequest {
  query?: string;
}

export interface KnowledgeImportRequest {
  mission_id: string;
  type: "file" | "url" | "note";
  title: string;
  content_preview?: string | null;
  source_uri?: string | null;
}

export interface KnowledgeFolderImportRequest {
  mission_id: string;
  folder_path: string;
  title_prefix?: string | null;
  recursive?: boolean | null;
  max_files?: number | null;
}

export interface KnowledgeFolderImportResponse {
  imported_count: number;
  skipped_count: number;
  items: MissionContextItem[];
  summary: string;
}

export interface KnowledgeUrlPreviewRequest {
  url: string;
}

export interface KnowledgeUrlPreviewResponse {
  url: string;
  status: number;
  content_type?: string | null;
  title?: string | null;
  preview: string;
  fetched_at: string;
  truncated: boolean;
}

export interface SimulationCount {
  key: string;
  count: number;
}

export interface SimulationOverviewSummary {
  total_missions: number;
  active_missions: number;
  simulating_missions: number;
  missions_with_runs: number;
  total_runs: number;
  simulation_runs: number;
}

export interface SimulationRecentRun {
  run_id: string;
  mission_id: string;
  mission_title: string;
  mission_status: MissionStatus;
  mission_priority: MissionPriority;
  mission_last_activity_at: string;
  run_type: Run["type"];
  run_status: Run["status"];
  started_at?: string | null;
  finished_at?: string | null;
  activity_at: string;
  run_activity_at?: string | null;
  summary?: string | null;
  error_message?: string | null;
}

export interface SimulationOverview {
  summary: SimulationOverviewSummary;
  counts_by_type: SimulationCount[];
  counts_by_status: SimulationCount[];
  recent_runs: SimulationRecentRun[];
}

export interface ScenarioVariable {
  id: string;
  label: string;
  current_value: string;
  proposed_value: string;
  impact: string;
  uncertainty: string;
  impact_weight: number;
  uncertainty_weight: number;
}

export interface ScenarioRun {
  id: string;
  mission_id: string;
  mission_title: string;
  baseline: string;
  options: string[];
  variables?: ScenarioVariable[];
  option_cards?: ScenarioOptionCard[];
  recommendation?: string | null;
  recommendation_reason?: string | null;
  comparison_summary?: string | null;
  selected_option_id?: string | null;
  handoff_target?: string | null;
  execution_risk_level?: string | null;
  created_at: string;
}

export interface ScenarioOptionCard {
  id: string;
  label: string;
  assumptions: string[];
  expected_benefits: string[];
  risks: string[];
  projected_outcomes: string[];
  score: number;
  time_horizon: string;
  confidence: "low" | "medium" | "high" | string;
}

export interface SimulationCreateScenarioRunRequest {
  mission_id: string;
  baseline: string;
  options: string[];
  variables: ScenarioVariable[];
  option_cards?: ScenarioOptionCard[];
  recommendation?: string | null;
  recommendation_reason?: string | null;
  comparison_summary: string | null;
  selected_option_id: string | null;
  handoff_target?: string | null;
  execution_risk_level?: string | null;
}
export interface SimulationHandoffPolicyTemplate {
  id: string;
  name: string;
  handoff_target: string;
  execution_risk_level: string;
  description: string;
  updated_at: string;
}

export interface SimulationSaveHandoffPolicyTemplateRequest {
  id?: string | null;
  name: string;
  handoff_target: string;
  execution_risk_level: string;
  description?: string | null;
}

export interface SimulationScoringFormulaTemplate {
  id: string;
  name: string;
  base_score: number;
  impact_multiplier: number;
  uncertainty_penalty: number;
  description: string;
  updated_at: string;
}

export interface SimulationSaveScoringFormulaTemplateRequest {
  id?: string | null;
  name: string;
  base_score: number;
  impact_multiplier: number;
  uncertainty_penalty: number;
  description?: string | null;
}

export interface SimulationTemplateBundle {
  schema_version: number;
  exported_at: string;
  handoff_policy_templates: SimulationHandoffPolicyTemplate[];
  scoring_formula_templates: SimulationScoringFormulaTemplate[];
}

export interface SimulationImportTemplateBundleRequest {
  bundle_json: string;
}

export interface SimulationImportTemplateBundleResponse {
  imported_handoff_policy_templates: number;
  imported_scoring_formula_templates: number;
  handoff_policy_templates: SimulationHandoffPolicyTemplate[];
  scoring_formula_templates: SimulationScoringFormulaTemplate[];
}

export interface SimulationTemplateBundleAuditEntry {
  id: string;
  action: string;
  actor: string;
  handoff_policy_template_count: number;
  scoring_formula_template_count: number;
  note: string;
  occurred_at: string;
}

export interface SimulationExportTemplateBundleAuditLogRequest {
  limit?: number | null;
}

export interface SimulationExportTemplateBundleAuditLogResponse {
  total: number;
  exported_count: number;
  events: SimulationTemplateBundleAuditEntry[];
}

export interface SimulationTemplateBundlePreflightSection {
  create_count: number;
  update_count: number;
  unchanged_count: number;
}

export interface SimulationTemplateBundleConflict {
  id: string;
  template_type: string;
  existing_name: string;
  incoming_name: string;
}

export interface SimulationTemplateBundlePreflightResponse {
  schema_version: number;
  total_count: number;
  handoff_policy_templates: SimulationTemplateBundlePreflightSection;
  scoring_formula_templates: SimulationTemplateBundlePreflightSection;
  conflicts: SimulationTemplateBundleConflict[];
}


export interface SimulationLocalSandboxRunListRequest {
  mission_id?: string | null;
  limit?: number | null;
}

export interface SimulationCapabilityRunListRequest {
  mission_id?: string | null;
  limit?: number | null;
  target_remote_user_id?: string | null;
}

export interface SimulationSandboxAgentRequest {
  name?: string | null;
  role: string;
  stance: string;
}

export interface SimulationRunLocalSandboxRequest {
  mission_id: string;
  baseline: string;
  options: string[];
  agents?: SimulationSandboxAgentRequest[];
  rounds?: number | null;
}

export interface SimulationLocalSandboxAgent {
  name: string;
  role: string;
  stance: string;
}

export interface SimulationLocalSandboxTurn {
  round: number;
  option: string;
  agent_name: string;
  agent_role: string;
  agent_stance: string;
  score: number;
  rationale: string;
}

export interface SimulationLocalSandboxOptionScore {
  option: string;
  average_score: number;
  total_score: number;
  turn_count: number;
}

export interface SimulationLocalSandboxRecommendation {
  option: string;
  average_score: number;
  rationale: string;
}

export interface SimulationLocalSandboxRun {
  run_id: string;
  mission_id: string;
  engine: string;
  rounds: number;
  agents: SimulationLocalSandboxAgent[];
  turns: SimulationLocalSandboxTurn[];
  option_scores: SimulationLocalSandboxOptionScore[];
  recommendation: SimulationLocalSandboxRecommendation;
  audit_event_id?: string | null;
}

export interface SimulationRunExternalSaasRequest {
  mission_id: string;
  provider: string;
  endpoint_url?: string | null;
  input_json?: string | null;
  dry_run?: boolean | null;
  confirmation_phrase?: string | null;
  target_remote_user_id?: string | null;
  timeout_ms?: number | null;
}

export interface SimulationExternalSaasRun {
  run_id: string;
  mission_id: string;
  engine: string;
  target_remote_user_id?: string | null;
  provider: string;
  endpoint_url?: string | null;
  dry_run: boolean;
  executed: boolean;
  network_invocation: boolean;
  request_preview: string;
  response_status?: number | null;
  response_body: string;
  summary: string;
  audit_event_id?: string | null;
}

export interface SimulationExternalSaasRunHistoryItem extends SimulationExternalSaasRun {
  created_at?: string | null;
  status?: string | null;
}

export interface SimulationRunHighFidelitySandboxRequest {
  mission_id: string;
  baseline: string;
  options: string[];
  agents?: SimulationSandboxAgentRequest[];
  rounds?: number | null;
  variables?: ScenarioVariable[];
  target_remote_user_id?: string | null;
}

export interface SimulationHighFidelityEntity {
  id: string;
  label: string;
  kind: string;
  state: string;
  risk_score: number;
}

export interface SimulationHighFidelityVariable {
  id: string;
  label: string;
  current_value: string;
  proposed_value: string;
  impact: string;
  uncertainty: string;
  pressure_score: number;
}

export interface SimulationHighFidelityTimelineEvent {
  tick: number;
  round: number;
  actor: string;
  option: string;
  score: number;
  score_delta: number;
  state_changes: string[];
}

export interface SimulationHighFidelityGraphNode {
  id: string;
  label: string;
  kind: string;
}

export interface SimulationHighFidelityGraphEdge {
  from: string;
  to: string;
  label: string;
  weight: number;
}

export interface SimulationHighFidelityEventGraph {
  nodes: SimulationHighFidelityGraphNode[];
  edges: SimulationHighFidelityGraphEdge[];
}

export interface SimulationHighFidelityMetricCell {
  option: string;
  metric: string;
  value: number;
}

export interface SimulationHighFidelityWorld {
  entities: SimulationHighFidelityEntity[];
  variables: SimulationHighFidelityVariable[];
  timeline: SimulationHighFidelityTimelineEvent[];
  event_graph: SimulationHighFidelityEventGraph;
  option_metric_heatmap: SimulationHighFidelityMetricCell[];
}

export interface SimulationHighFidelitySandboxRun {
  run_id: string;
  mission_id: string;
  engine: string;
  target_remote_user_id?: string | null;
  base_run: SimulationLocalSandboxRun;
  world: SimulationHighFidelityWorld;
  summary: string;
  audit_event_id?: string | null;
}

export interface SimulationHighFidelitySandboxRunHistoryItem
  extends SimulationHighFidelitySandboxRun {
  created_at?: string | null;
  status?: string | null;
}

export interface SimulationComparisonRequest {
  mission_id: string;
}

export interface SimulationComparisonScenario {
  scenario_run_id: string;
  created_at: string;
  selected_option_id?: string | null;
  selected_option_label?: string | null;
  recommendation?: string | null;
  recommendation_reason?: string | null;
  comparison_summary?: string | null;
  average_option_score: number;
}

export interface SimulationVariableAxis {
  label: string;
  values: string[];
  impacts: string[];
  uncertainties: string[];
}

export interface SimulationOptionPattern {
  label: string;
  appearance_count: number;
  selected_count: number;
  average_score: number;
  latest_time_horizon: string;
}

export interface SimulationPathEvolutionStep {
  scenario_run_id: string;
  created_at: string;
  selected_option_label: string;
  score: number;
  variable_changes: string[];
  narrative: string;
}

export interface SimulationComparisonMatrix {
  mission_id: string;
  mission_title: string;
  scenario_count: number;
  scenarios: SimulationComparisonScenario[];
  variable_axes: SimulationVariableAxis[];
  option_patterns: SimulationOptionPattern[];
  path_evolution: SimulationPathEvolutionStep[];
  summary: string;
}

export interface GlobalSearchResult {
  id: string;
  kind: string;
  title: string;
  detail: string;
  route: string;
}

export interface NotificationItem {
  id: string;
  kind: string;
  title: string;
  message: string;
  mission_id?: string | null;
  route: string;
  created_at: string;
}

export interface RunEventItem {
  id: string;
  mission_id: string;
  run_id: string;
  event_type: string;
  message: string;
  payload_json?: string | null;
  created_at: string;
}

export interface MemoryRecordItem {
  id: string;
  scope: string;
  scope_ref: string;
  title: string;
  content: string;
  source_type: string;
  importance: string;
  created_at: string;
}

export interface MemoryRecordListRequest {
  scope?: string;
  scope_ref?: string;
}

export interface MemoryRecordCreateRequest {
  scope: string;
  scope_ref: string;
  title: string;
  content: string;
  source_type: string;
  importance: string;
}

export interface MemoryRecordSearchRequest {
  query: string;
  limit?: number;
}

export interface CouncilStepItem {
  id: string;
  mission_id: string;
  run_id: string;
  role: string;
  status: string;
  input_summary?: string | null;
  output_summary?: string | null;
  review_note?: string | null;
  created_at: string;
  updated_at: string;
}

export interface CouncilStepCreateRequest {
  mission_id: string;
  run_id: string;
  role: string;
  status: string;
  input_summary?: string | null;
  output_summary?: string | null;
  review_note?: string | null;
}

export interface GlobalSearchRequest {
  query: string;
}

export interface SkillSetEnabledRequest {
  name: string;
  enabled: boolean;
}

export interface SkillSearchRequest {
  query: string;
  limit?: number;
}

export interface SkillViewRequest {
  name: string;
}

export interface SkillInstallRequest {
  name: string;
  title?: string | null;
  description?: string | null;
  content?: string | null;
  force?: boolean;
}

export interface SkillMarketplaceListRequest {
  manifest_url: string;
  limit?: number | null;
}

export interface SkillMarketplaceInstallRequest {
  manifest_url: string;
  name: string;
  force?: boolean | null;
  target_remote_user_id?: string | null;
}

export interface SkillMarketplaceInstallHistoryListRequest {
  limit?: number | null;
  marketplace_id?: string | null;
  skill_name?: string | null;
  target_remote_user_id?: string | null;
}

export interface SkillMarketplaceEntry {
  name: string;
  title: string;
  description: string;
  source_url?: string | null;
  content?: string | null;
  tags: string[];
}

export interface SkillMarketplaceCatalog {
  schema_version: number;
  marketplace_id: string;
  manifest_url: string;
  skills: SkillMarketplaceEntry[];
}

export interface SkillMarketplaceInstallResult {
  marketplace_id: string;
  manifest_url: string;
  target_remote_user_id?: string | null;
  entry: SkillMarketplaceEntry;
  installed_skill: SkillDetailItem;
}

export interface SkillMarketplaceInstallHistoryItem {
  id: string;
  marketplace_id: string;
  skill_name: string;
  display_name: string;
  manifest_url: string;
  source_url?: string | null;
  target_remote_user_id?: string | null;
  content_source_summary: string;
  installed_skill_name: string;
  installed_at: string;
}

export interface SkillInvokeRequest {
  name: string;
  instruction?: string | null;
}

export interface SkillInvocationPayload {
  name: string;
  display_name: string;
  command: string;
  source: string;
  path: string;
  instruction?: string | null;
  rendered_prompt: string;
}

export interface SkillInvokeSessionRequest {
  name: string;
  instruction?: string | null;
  session_id: string;
}

export interface SkillSessionInvocationListRequest {
  session_id: string;
  limit?: number | null;
}

export interface SkillSessionInvocationResult {
  session_id: string;
  invocation: SkillInvocationPayload;
  message: SessionMessage;
}

export interface SkillRuntimeExecuteRequest {
  name: string;
  instruction?: string | null;
  session_id?: string | null;
  save_to_session?: boolean | null;
  dry_run?: boolean | null;
  tool_command?: string | null;
  timeout_ms?: number | null;
}

export interface SkillRuntimeExecutionPackage {
  command: string;
  args: string[];
  cwd?: string | null;
  timeout_ms: number;
  preview: string;
}

export interface SkillRuntimeExecutionResult {
  invocation: SkillInvocationPayload;
  execution_package: SkillRuntimeExecutionPackage;
  executed: boolean;
  dry_run: boolean;
  runtime_result?: SkillToolResponse | null;
  session_message?: SessionMessage | null;
  summary: string;
}

export type SkillEvolutionAction = "refine" | "create" | "skip";
export type SkillEvolutionStatus = "pending" | "accepted" | "rejected";
export type SkillEvolutionConfidence = "low" | "medium" | "high";

export interface SkillEvolutionSourceRef {
  kind: string;
  id: string;
  title?: string | null;
}

export interface SkillEvolutionCandidate {
  id: string;
  target_skill_name?: string | null;
  action: SkillEvolutionAction;
  status: SkillEvolutionStatus;
  evidence_summary: string;
  recommended_change: string;
  confidence: SkillEvolutionConfidence;
  source_refs: SkillEvolutionSourceRef[];
  validation_notes?: string | null;
  created_at: string;
  updated_at: string;
}

export interface SkillEvolutionCandidateListRequest {
  status?: SkillEvolutionStatus | null;
  limit?: number;
}

export interface SkillEvolutionCandidateGenerateRequest {
  limit?: number;
}

export interface SkillEvolutionCandidateCreateRequest {
  target_skill_name?: string | null;
  action: SkillEvolutionAction;
  evidence_summary: string;
  recommended_change: string;
  confidence: SkillEvolutionConfidence;
  source_refs?: SkillEvolutionSourceRef[];
  validation_notes?: string | null;
}

export interface SkillEvolutionCandidateSetStatusRequest {
  id: string;
  status: SkillEvolutionStatus;
  validation_notes?: string | null;
}

export interface SkillDetailItem {
  name: string;
  display_name: string;
  description?: string | null;
  source: string;
  path: string;
  enabled: boolean;
  content: string;
}

export type VoiceProviderKind = "stt" | "tts";

export interface VoiceProvider {
  id: string;
  label: string;
  kind: VoiceProviderKind;
  local_only: boolean;
  transport: string;
  interaction_model: string;
  supports_audio_input: boolean;
  supports_audio_output: boolean;
  capabilities: string[];
  compatibility_aliases: string[];
  runtime_boundary: string;
  notes: string;
}

export interface VoiceSettings {
  enabled: boolean;
  stt_provider: string;
  tts_provider: string;
  updated_at: string;
  transcription_language: string;
  preferred_voice?: string | null;
  auto_speak_transcripts: boolean;
}

export interface VoiceSummary {
  enabled: boolean;
  stt_provider: string;
  tts_provider: string;
  updated_at: string;
  transcription_count: number;
  queued_speak_count: number;
  history_count: number;
  pending_speak_count: number;
  last_transcript?: string | null;
  last_spoken_text?: string | null;
  last_event_kind?: string | null;
  last_event_at?: string | null;
}

export interface VoiceSetEnabledRequest {
  enabled: boolean;
}

export interface VoiceUpdateSettingsRequest {
  enabled?: boolean | null;
  stt_provider?: string | null;
  tts_provider?: string | null;
  transcription_language?: string | null;
  preferred_voice?: string | null;
  auto_speak_transcripts?: boolean | null;
}

export interface VoiceTranscribeRequest {
  text: string;
  source?: string | null;
  language?: string | null;
  auto_queue_for_speech?: boolean | null;
}


export interface VoiceTranscriptionResult {
  transcript: string;
  normalized_transcript: string;
  provider: string;
  source: string;
  language: string;
  word_count: number;
  queued_for_speech: boolean;
}

export interface VoiceSpeakRequest {
  text: string;
  voice?: string | null;
  origin?: string | null;
}


export interface VoiceSpeakResult {
  queued: boolean;
  provider: string;
  text: string;
  id: string;
  status: string;
  voice?: string | null;
  origin: string;
  created_at: string;
}

export interface VoiceHistoryListRequest {
  kind?: string | null;
  limit?: number | null;
  include_payload?: boolean;
}

export interface VoiceHistoryItem {
  id: string;
  kind: string;
  provider: string;
  text: string;
  created_at: string;
  updated_at: string;
  status: string;
  source?: string | null;
  language?: string | null;
  voice?: string | null;
  origin?: string | null;
  word_count?: number | null;
  payload_text?: string | null;
}

export interface VoiceHistoryListResponse {
  total: number;
  items: VoiceHistoryItem[];
}

export interface VoiceProcessSpeakQueueRequest {
  mark_status?: string | null;
}

export interface VoiceProcessSpeakQueueResponse {
  processed: boolean;
  item: VoiceHistoryItem;
  remaining: number;
}

export interface WorkspacePathsPayload {
  config_dir: string;
  data_dir: string;
  log_dir: string;
  db_path: string;
  control_api_url: string;
  default_workspace_path?: string | null;
}

export interface WorkspaceStatusPayload {
  config_exists: boolean;
  database_exists: boolean;
  default_workspace_exists: boolean;
  engine_last_heartbeat_at?: string | null;
  engine_queued_background_runs: number;
  engine_awaiting_approval_steps: number;
  foreground_updated_at: string;
  cron_last_heartbeat_at?: string | null;
}

export interface WorkspaceCountsPayload {
  missions: number;
  sessions: number;
  knowledge_sources: number;
  memory_records: number;
  run_events: number;
  cron_jobs: number;
}

export interface WorkspaceLogFilePayload {
  name: string;
  path: string;
  size_bytes: number;
}

export interface WorkspaceDiagnosticsPayload {
  paths: WorkspacePathsPayload;
  status: WorkspaceStatusPayload;
  counts: WorkspaceCountsPayload;
  recent_logs: WorkspaceLogFilePayload[];
}

export interface MissionPlaybookSuggestion {
  id: string;
  kind: string;
  priority: string;
  title: string;
  rationale: string;
  actions: string[];
  evidence_ids: string[];
}

export interface MissionPlaybookEvidenceCard {
  id: string;
  category: string;
  title: string;
  summary: string;
  bullets: string[];
  source_refs: string[];
}

export interface MissionPlaybook {
  mission_id: string;
  mission_title: string;
  generated_at: string;
  summary: string;
  suggestions: MissionPlaybookSuggestion[];
  evidence_cards: MissionPlaybookEvidenceCard[];
}

export interface TrajectoryExportRequest {
  mission_id?: string | null;
  include_session_messages?: boolean | null;
}

export interface TrajectoryDatasetExport {
  schema_version: number;
  exported_at: string;
  mission_id?: string | null;
  item_count: number;
  jsonl: string;
}

export interface TrajectoryRlTrainingRequest {
  jsonl: string;
  epochs?: number | null;
  alpha?: number | null;
  gamma?: number | null;
  job_name?: string | null;
  target_remote_user_id?: string | null;
}

export interface TrajectoryRlTrainingJobListRequest {
  limit?: number | null;
  target_remote_user_id?: string | null;
}

export interface TrajectoryRlPolicyEntry {
  state: string;
  action: string;
  q_value: number;
  visits: number;
  average_reward: number;
}

export interface TrajectoryRlTrainingResult {
  job_id: string;
  job_name?: string | null;
  target_remote_user_id?: string | null;
  trained_at: string;
  input_line_count: number;
  valid_transition_count: number;
  invalid_line_count: number;
  episode_count: number;
  epochs: number;
  alpha: number;
  gamma: number;
  average_reward: number;
  policy: TrajectoryRlPolicyEntry[];
  artifact_json: string;
  summary: string;
}

export interface ParityCatalog {
  providers: ParityProviderCatalog[];
  active_provider: string;
  active_model: string;
  tool_visibility_options: string[];
  cron_status_options: string[];
  mcp_filter_modes: string[];
}

export interface ParityProviderCatalog {
  id: string;
  display_name: string;
  supports_custom_endpoint: boolean;
  models: ParityModelCatalogEntry[];
}

export interface ParityModelCatalogEntry {
  id: string;
  display_name: string;
  recommended: boolean;
}

export interface ParityProviderSelection {
  provider: string;
  model: string;
  base_url?: string | null;
}

export interface ParityRuntimeReadiness {
  provider: string;
  model: string;
  base_url?: string | null;
  api_key_ref?: string | null;
  api_key_ref_configured: boolean;
  uses_custom_endpoint: boolean;
  can_authenticate: boolean;
  auth: {
    kind: string;
    label: string;
    env_var?: string | null;
    available: boolean;
  };
  sources: Array<{
    kind: string;
    label: string;
    env_var?: string | null;
    available: boolean;
  }>;
  status: string;
  message: string;
}

export interface ParitySaveProviderSelectionRequest {
  provider: string;
  model: string;
  base_url?: string | null;
}

export type ParityProviderSelectionRequest = ParitySaveProviderSelectionRequest;

export interface ParityToolMetadata {
  name: string;
  description: string;
  visible: boolean;
  enabled: boolean;
  availability: string;
}

export interface ParityToolset {
  id: string;
  name: string;
  description?: string | null;
  enabled: boolean;
  source: string;
  tools: ParityToolMetadata[];
  created_at: string;
  updated_at: string;
}

export interface ParityToolsetSaveRequest {
  id?: string | null;
  name: string;
  description?: string | null;
  enabled: boolean;
  source?: string | null;
  tools: ParityToolMetadata[];
}

export interface ParityCronJob {
  id: string;
  name: string;
  schedule: string;
  prompt: string;
  deliver_to?: string | null;
  enabled: boolean;
  status: string;
  last_run_requested_at?: string | null;
  last_run_status?: string | null;
  run_count: number;
  paused_at?: string | null;
  created_at: string;
  updated_at: string;
}

export interface ParityCronCreateRequest {
  name: string;
  schedule: string;
  prompt: string;
  deliver_to?: string | null;
  enabled: boolean;
}

export interface ParityCronRuntimeStatus {
  status: string;
  worker_started_at?: string | null;
  last_heartbeat_at?: string | null;
  last_poll_started_at?: string | null;
  last_poll_completed_at?: string | null;
  last_error?: string | null;
  last_dispatch_count: number;
}

export interface ParityCronDispatchOutcome {
  job_id: string;
  job_name: string;
  reason: string;
  status: string;
  session_id: string;
  mission_id: string;
  run_id: string;
  run_event_id: string;
  dispatched_at: string;
}

export interface ParityCronRuntimeTickResult {
  checked_jobs: number;
  dispatched_jobs: number;
  heartbeat_at: string;
  dispatches: ParityCronDispatchOutcome[];
}

export interface ParityCronSetEnabledRequest {
  id: string;
  enabled: boolean;
}

export interface ParityCronRunNowRequest {
  id: string;
}

export interface ParityMcpServer {
  id: string;
  name: string;
  transport: string;
  endpoint: string;
  enabled: boolean;
  tool_filter_mode: string;
  allowed_tools: string[];
  blocked_tools: string[];
  resources_enabled: boolean;
  prompts_enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface ParityMcpServerRuntimeStatus {
  id: string;
  name: string;
  transport: string;
  endpoint: string;
  enabled: boolean;
  runtime_status: string;
  management_mode: string;
  pid?: number | null;
  last_started_at?: string | null;
  last_stopped_at?: string | null;
  last_reloaded_at?: string | null;
  last_exit_code?: number | null;
  last_error?: string | null;
  status_message?: string | null;
  updated_at: string;
}

export interface ParityMcpProbeResult {
  id: string;
  name: string;
  transport: string;
  endpoint: string;
  management_mode: string;
  tool_filter_mode: string;
  allowed_tool_count: number;
  blocked_tool_count: number;
  resources_enabled: boolean;
  prompts_enabled: boolean;
  handshake_status: string;
  handshake_reason: string;
  status: string;
  message: string;
  command_available?: boolean | null;
  url_valid?: boolean | null;
  parsed_command?: string | null;
  parsed_args?: string[];
  endpoint_scheme?: string | null;
  endpoint_host?: string | null;
  endpoint_detail?: string | null;
}

export interface ParityMcpUpsertRequest {
  id?: string | null;
  name: string;
  transport: string;
  endpoint: string;
  enabled: boolean;
  tool_filter_mode: string;
  allowed_tools: string[];
  blocked_tools: string[];
  resources_enabled: boolean;
  prompts_enabled: boolean;
}

export interface ParityQuickCommand {
  id: string;
  name: string;
  command: string;
  description?: string | null;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface ParityQuickCommandSaveRequest {
  id?: string | null;
  name: string;
  command: string;
  description?: string | null;
  enabled: boolean;
}

function isOptionalCommandMissing(error: unknown): boolean {
  if (!(error instanceof Error)) {
    return false;
  }

  const message = error.message.toLowerCase();
  return (
    message.includes("unknown command") ||
    message.includes("command not found") ||
    message.includes("not currently implemented") ||
    message.includes("is not defined on the invoke handler")
  );
}

async function invokeOptional<T>(
  command: string,
  args?: Record<string, unknown>,
): Promise<T | null> {
  try {
    return await invoke<T>(command, args);
  } catch (error) {
    if (isOptionalCommandMissing(error)) {
      return null;
    }
    throw error;
  }
}

// ============ API 函数 ============

/**
 * 获取引导数据
 */
export async function appGetBootstrap(): Promise<BootstrapPayload> {
  const raw = await invoke<RawBootstrapPayload>("app_get_bootstrap");
  return {
    app_settings: raw.app_settings,
    runtime_settings: raw.runtime_settings,
    engine_status: raw.engine_status,
    app_runtime_status: raw.hermes_status,
    foreground_snapshot: raw.foreground_snapshot,
    active_session: raw.active_session ?? null,
    active_mission: raw.active_mission ?? null,
    summary: raw.summary,
  };
}

export async function appGetWorkspaceDiagnostics(): Promise<WorkspaceDiagnosticsPayload> {
  return invoke<WorkspaceDiagnosticsPayload>("app_get_workspace_diagnostics");
}

/**
 * 获取设置
 */
export async function settingsGet(): Promise<SettingsPayload> {
  return invoke<SettingsPayload>("settings_get");
}

/**
 * 保存设置
 */
export async function settingsSave(
  request: SaveSettingsRequest,
): Promise<SaveSettingsResponse> {
  return invoke<SaveSettingsResponse>("settings_save", { request });
}

/**
 * 获取运行时状态
 */

export async function runtimeAdapterExecuteSkillTool(
  request: SkillToolRequest,
): Promise<SkillToolResponse> {
  return invoke<SkillToolResponse>("runtime_adapter_execute_skill_tool", { request });
}

export async function runtimeAdapterProbeDesktopExecutor(): Promise<DesktopExecutorProbe> {
  return invoke<DesktopExecutorProbe>("runtime_adapter_probe_desktop_executor");
}

export async function runtimeAdapterExecuteDesktopAction(
  request: DesktopActionRequest,
): Promise<DesktopActionResponse> {
  return invoke<DesktopActionResponse>("runtime_adapter_execute_desktop_action", { request });
}

export async function runtimeAdapterRunGuiAutomation(
  request: GuiAutomationRequest,
): Promise<GuiAutomationResponse> {
  return invoke<GuiAutomationResponse>("runtime_adapter_run_gui_automation", { request });
}

export async function runtimeAdapterSummarizeTrajectoryJsonl(
  request: TrajectorySummaryRequest,
): Promise<TrajectorySummaryResponse> {
  return invoke<TrajectorySummaryResponse>("runtime_adapter_summarize_trajectory_jsonl", { request });
}

export async function runtimeAdapterListAuditEvents(
  request: RuntimeAdapterAuditListRequest = {},
): Promise<RuntimeAdapterAuditEvent[]> {
  return invoke<RuntimeAdapterAuditEvent[]>("runtime_adapter_list_audit_events", { request });
}

export async function runtimeAdapterExportAuditEvents(
  request: RuntimeAdapterAuditExportRequest = {},
): Promise<RuntimeAdapterAuditExportResponse> {
  return invoke<RuntimeAdapterAuditExportResponse>("runtime_adapter_export_audit_events", { request });
}

export async function turixCuaProbe(
  request: TurixCuaProbeRequest = {},
): Promise<TurixCuaProbe> {
  return invoke<TurixCuaProbe>("turix_cua_probe", { request });
}

export async function turixCuaRun(
  request: TurixCuaRunRequest,
): Promise<TurixCuaRunResponse> {
  return invoke<TurixCuaRunResponse>("turix_cua_run", { request });
}

export async function turixCuaListAuditEvents(
  request: TurixCuaAuditListRequest = {},
): Promise<TurixCuaAuditEvent[]> {
  return invoke<TurixCuaAuditEvent[]>("turix_cua_list_audit_events", { request });
}

export async function turixCuaExportAuditEvents(
  request: TurixCuaAuditExportRequest,
): Promise<TurixCuaAuditExportResponse> {
  return invoke<TurixCuaAuditExportResponse>("turix_cua_export_audit_events", { request });
}

export async function nativeCuaProbe(): Promise<NativeCuaProbe> {
  return invoke<NativeCuaProbe>("native_cua_probe");
}

export async function nativeCuaStartSession(
  request: NativeCuaStartSessionRequest,
): Promise<NativeCuaSessionResponse> {
  return invoke<NativeCuaSessionResponse>("native_cua_start_session", { request });
}

export async function nativeCuaPreviewModelRoute(
  request: NativeCuaPreviewModelRouteRequest,
): Promise<NativeCuaModelRoutePreview> {
  return invoke<NativeCuaModelRoutePreview>("native_cua_preview_model_route", { request });
}

export async function nativeCuaObserve(
  request: NativeCuaObserveRequest = {},
): Promise<NativeCuaObserveResponse> {
  return invoke<NativeCuaObserveResponse>("native_cua_observe", { request });
}

export async function nativeCuaExecuteAction(
  request: NativeCuaExecuteActionRequest,
): Promise<NativeCuaExecuteActionResponse> {
  return invoke<NativeCuaExecuteActionResponse>("native_cua_execute_action", { request });
}

export async function nativeCuaListAuditEvents(
  request: NativeCuaAuditListRequest = {},
): Promise<NativeCuaAuditEvent[]> {
  return invoke<NativeCuaAuditEvent[]>("native_cua_list_audit_events", { request });
}

export async function nativeCuaExportAuditEvents(
  request: NativeCuaAuditExportRequest,
): Promise<NativeCuaAuditExportResponse> {
  return invoke<NativeCuaAuditExportResponse>("native_cua_export_audit_events", { request });
}

export async function nativeCuaPlanTask(
  request: NativeCuaPlanTaskRequest,
): Promise<NativeCuaPlanResponse> {
  return invoke<NativeCuaPlanResponse>("native_cua_plan_task", { request });
}

export async function nativeCuaRunStep(
  request: NativeCuaRunStepRequest,
): Promise<NativeCuaRunStepResponse> {
  return invoke<NativeCuaRunStepResponse>("native_cua_run_step", { request });
}

export async function nativeCuaListHistory(
  request: NativeCuaHistoryListRequest = {},
): Promise<NativeCuaStepRecord[]> {
  return invoke<NativeCuaStepRecord[]>("native_cua_list_history", { request });
}

export async function nativeCuaRecordInfo(
  request: NativeCuaRecordInfoRequest,
): Promise<NativeCuaMemoryRecord> {
  return invoke<NativeCuaMemoryRecord>("native_cua_record_info", { request });
}

export async function nativeCuaExportTrajectory(
  request: NativeCuaTrajectoryExportRequest,
): Promise<NativeCuaTrajectoryExportResponse> {
  return invoke<NativeCuaTrajectoryExportResponse>("native_cua_export_trajectory", { request });
}

export async function nativeCuaPrepareModelTurn(
  request: NativeCuaModelTurnRequest,
): Promise<NativeCuaModelTurnResponse> {
  return invoke<NativeCuaModelTurnResponse>("native_cua_prepare_model_turn", { request });
}

export async function nativeCuaInvokeModel(
  request: NativeCuaInvokeModelRequest,
): Promise<NativeCuaInvokeModelResponse> {
  return invoke<NativeCuaInvokeModelResponse>("native_cua_invoke_model", { request });
}

export async function nativeCuaApplyModelOutput(
  request: NativeCuaApplyModelOutputRequest,
): Promise<NativeCuaApplyModelOutputResponse> {
  return invoke<NativeCuaApplyModelOutputResponse>("native_cua_apply_model_output", { request });
}

export async function agentExchangeGetState(): Promise<AgentExchangeState> {
  return invoke<AgentExchangeState>("agent_exchange_get_state");
}

export async function agentExchangeListMessages(
  request: AgentExchangeListRequest = {},
): Promise<AgentExchangeMessage[]> {
  return invoke<AgentExchangeMessage[]>("agent_exchange_list_messages", { request });
}

export async function agentExchangeListRemoteUsers(
  request: AgentExchangeListRemoteUsersRequest = {},
): Promise<AgentExchangeRemoteUser[]> {
  return invoke<AgentExchangeRemoteUser[]>("agent_exchange_list_remote_users", { request });
}

export async function agentExchangeUpsertRemoteUser(
  request: AgentExchangeUpsertRemoteUserRequest,
): Promise<AgentExchangeRemoteUser> {
  return invoke<AgentExchangeRemoteUser>("agent_exchange_upsert_remote_user", { request });
}

export async function agentExchangeDeleteRemoteUser(
  request: AgentExchangeDeleteRemoteUserRequest,
): Promise<AgentExchangeState> {
  return invoke<AgentExchangeState>("agent_exchange_delete_remote_user", { request });
}

export async function agentExchangeDraftOutbound(
  request: AgentExchangeDraftOutboundRequest,
): Promise<AgentExchangeMessage> {
  return invoke<AgentExchangeMessage>("agent_exchange_draft_outbound", { request });
}

export async function agentExchangeIngestInbound(
  request: AgentExchangeIngestInboundRequest,
): Promise<AgentExchangeMessage> {
  return invoke<AgentExchangeMessage>("agent_exchange_ingest_inbound", { request });
}

export async function agentExchangeExportBundle(
  request: AgentExchangeExportBundleRequest = {},
): Promise<AgentExchangeBundle> {
  return invoke<AgentExchangeBundle>("agent_exchange_export_bundle", { request });
}

export async function agentExchangeImportBundle(
  request: AgentExchangeImportBundleRequest,
): Promise<AgentExchangeImportBundleResponse> {
  return invoke<AgentExchangeImportBundleResponse>("agent_exchange_import_bundle", { request });
}

export async function agentExchangeUpdateMessageStatus(
  request: AgentExchangeUpdateMessageStatusRequest,
): Promise<AgentExchangeMessage> {
  return invoke<AgentExchangeMessage>("agent_exchange_update_message_status", { request });
}

export async function agentExchangeDeleteMessage(
  request: AgentExchangeDeleteMessageRequest,
): Promise<AgentExchangeState> {
  return invoke<AgentExchangeState>("agent_exchange_delete_message", { request });
}

export async function agentExchangeRunFolderSync(
  request: AgentExchangeRunFolderSyncRequest,
): Promise<AgentExchangeRunFolderSyncResponse> {
  return invoke<AgentExchangeRunFolderSyncResponse>("agent_exchange_run_folder_sync", { request });
}

export async function teamSyncGetState(): Promise<TeamSyncState> {
  return invoke<TeamSyncState>("team_sync_get_state");
}

export async function teamSyncUpsertMember(
  request: TeamSyncUpsertMemberRequest,
): Promise<TeamMember> {
  return invoke<TeamMember>("team_sync_upsert_member", { request });
}

export async function teamSyncCheckAccess(
  request: TeamSyncCheckAccessRequest,
): Promise<TeamSyncAccessDecision> {
  return invoke<TeamSyncAccessDecision>("team_sync_check_access", { request });
}

export async function teamSyncExportBundle(
  request: TeamSyncExportBundleRequest,
): Promise<TeamSyncBundle> {
  return invoke<TeamSyncBundle>("team_sync_export_bundle", { request });
}

export async function teamSyncExportAudit(
  request: TeamSyncExportAuditRequest,
): Promise<TeamSyncExportAuditResponse> {
  return invoke<TeamSyncExportAuditResponse>("team_sync_export_audit", { request });
}

export async function teamSyncImportBundle(
  request: TeamSyncImportBundleRequest,
): Promise<TeamSyncState> {
  return invoke<TeamSyncState>("team_sync_import_bundle", { request });
}

export async function teamSyncRunFolderSync(
  request: TeamSyncRunFolderSyncRequest,
): Promise<TeamSyncRunFolderSyncResponse> {
  return invoke<TeamSyncRunFolderSyncResponse>("team_sync_run_folder_sync", { request });
}

export async function runtimeGetStatus(): Promise<RuntimeStatusResponse> {
  const raw = await invoke<RawRuntimeStatusResponse>("runtime_get_status");
  return {
    engine: {
      ...raw.engine,
      last_error: raw.engine.last_error ?? null,
    },
    appRuntime: {
      installed: raw.hermes.installed,
      running: raw.hermes.running,
      version: raw.hermes.version ?? null,
    },
    foreground: raw.foreground_snapshot,
  };
}

/**
 * 启动 Agent Engine
 */
export async function runtimeStartEngine(): Promise<RuntimeStatusResponse> {
  const raw = await invoke<RawRuntimeStatusResponse>("runtime_start_engine");
  return {
    engine: {
      ...raw.engine,
      last_error: raw.engine.last_error ?? null,
    },
    appRuntime: {
      installed: raw.hermes.installed,
      running: raw.hermes.running,
      version: raw.hermes.version ?? null,
    },
    foreground: raw.foreground_snapshot,
  };
}

/**
 * 停止 Agent Engine
 */
export async function runtimeStopEngine(): Promise<RuntimeStatusResponse> {
  const raw = await invoke<RawRuntimeStatusResponse>("runtime_stop_engine");
  return {
    engine: {
      ...raw.engine,
      last_error: raw.engine.last_error ?? null,
    },
    appRuntime: {
      installed: raw.hermes.installed,
      running: raw.hermes.running,
      version: raw.hermes.version ?? null,
    },
    foreground: raw.foreground_snapshot,
  };
}

/**
 * 重启 Agent Engine
 */
export async function runtimeRestartEngine(): Promise<RuntimeStatusResponse> {
  const raw = await invoke<RawRuntimeStatusResponse>("runtime_restart_engine");
  return {
    engine: {
      ...raw.engine,
      last_error: raw.engine.last_error ?? null,
    },
    appRuntime: {
      installed: raw.hermes.installed,
      running: raw.hermes.running,
      version: raw.hermes.version ?? null,
    },
    foreground: raw.foreground_snapshot,
  };
}

export async function missionList(
  request?: MissionListRequest,
): Promise<Mission[]> {
  return invoke<Mission[]>("mission_list", { request });
}

export async function missionCreate(
  request: MissionCreateRequest,
): Promise<Mission> {
  return invoke<Mission>("mission_create", { request });
}

export async function missionUpdate(
  request: MissionUpdateRequest,
): Promise<Mission> {
  return invoke<Mission>("mission_update", { request });
}

export async function missionSetPinned(
  request: MissionPinnedRequest,
): Promise<Mission> {
  return invoke<Mission>("mission_set_pinned", { request });
}

export async function missionSetStatus(
  request: MissionStatusRequest,
): Promise<Mission> {
  return invoke<Mission>("mission_set_status", { request });
}

export async function missionGeneratePlan(id: string): Promise<GeneratedMissionPlan> {
  return invoke<GeneratedMissionPlan>("mission_generate_plan", { request: { id } });
}

export async function missionGet(id: string): Promise<MissionDetail | null> {
  return invoke<MissionDetail | null>("mission_get", { id });
}

export async function executionListByMission(
  missionId: string,
): Promise<ExecutionStep[]> {
  return invoke<ExecutionStep[]>("execution_list_by_mission", {
    request: { mission_id: missionId },
  });
}

export async function executionAddStepNote(
  request: ExecutionStepNoteRequest,
): Promise<ExecutionStep> {
  return invoke<ExecutionStep>("execution_add_step_note", { request });
}

export async function executionPrepareDesktopHandoff(
  request: ExecutionPrepareDesktopHandoffRequest,
): Promise<ExecutionDesktopHandoff> {
  return invoke<ExecutionDesktopHandoff>("execution_prepare_desktop_handoff", { request });
}

export async function executionListDesktopHandoffQueue(
  request: ExecutionDesktopHandoffQueueRequest = {},
): Promise<ExecutionDesktopHandoffQueueItem[]> {
  return invoke<ExecutionDesktopHandoffQueueItem[]>("execution_list_desktop_handoff_queue", { request });
}

export async function executionMarkDesktopHandoffReviewed(
  request: ExecutionMarkDesktopHandoffReviewedRequest,
): Promise<void> {
  await invoke("execution_mark_desktop_handoff_reviewed", { request });
}

export async function executionApproveStep(id: string): Promise<ExecutionStep> {
  return invoke<ExecutionStep>("execution_approve_step", { request: { id } });
}

export async function executionStartStep(id: string): Promise<ExecutionStep> {
  return invoke<ExecutionStep>("execution_start_step", { request: { id } });
}

export async function executionPauseStep(id: string): Promise<ExecutionStep> {
  return invoke<ExecutionStep>("execution_pause_step", { request: { id } });
}

export async function executionCompleteStep(
  id: string,
  outputSummary?: string,
): Promise<ExecutionStep> {
  return invoke<ExecutionStep>("execution_complete_step", {
    request: { id, output_summary: outputSummary ?? null },
  });
}

export async function executionRetryStep(id: string): Promise<ExecutionStep> {
  return invoke<ExecutionStep>("execution_retry_step", { request: { id } });
}

export async function executionResumeStep(id: string): Promise<ExecutionStep> {
  return invoke<ExecutionStep>("execution_resume_step", { request: { id } });
}

export async function executionRerunStep(id: string): Promise<ExecutionStep> {
  return invoke<ExecutionStep>("execution_rerun_step", { request: { id } });
}

export async function executionConfirmSkipStep(
  id: string,
): Promise<ExecutionStep> {
  return invoke<ExecutionStep>("execution_confirm_skip_step", {
    request: { id },
  });
}

export async function executionRunCliStep(
  request: ExecutionRunCliStepRequest,
): Promise<ExecutionStep> {
  return invoke<ExecutionStep>("execution_run_cli_step", { request });
}

export async function sessionListRecent(limit = 20): Promise<Session[]> {
  return invoke<Session[]>("session_list_recent", {
    request: { limit },
  });
}

export async function sessionGet(id: string): Promise<Session | null> {
  return invoke<Session | null>("session_get", { id });
}

export async function sessionGetLatest(): Promise<Session | null> {
  return invoke<Session | null>("session_get_latest");
}

export async function sessionGetActive(): Promise<ActiveSessionSelection | null> {
  return invokeOptional<ActiveSessionSelection>("session_get_active");
}

export async function sessionContinueLatest(): Promise<Session | null> {
  return invoke<Session | null>("session_continue_latest");
}

export async function sessionActivate(
  request: SessionActivateRequest,
): Promise<ActiveSessionSelection> {
  return invoke<ActiveSessionSelection>("session_activate", { request });
}

export async function sessionClearActive(): Promise<boolean> {
  return invoke<boolean>("session_clear_active");
}

export async function sessionMessageList(
  request: SessionMessageListRequest,
): Promise<SessionMessage[]> {
  return invoke<SessionMessage[]>("session_message_list", { request });
}

export async function sessionReplaySnapshot(
  request?: SessionReplaySnapshotRequest | null,
): Promise<SessionReplaySnapshot> {
  return invoke<SessionReplaySnapshot>("session_replay_snapshot", {
    request: request ?? null,
  });
}

export async function sessionMessageCreate(
  request: SessionMessageCreateRequest,
): Promise<SessionMessage> {
  return invoke<SessionMessage>("session_message_create", { request });
}

export async function sessionResumeByTitle(title: string): Promise<Session | null> {
  const trimmed = title.trim();
  if (!trimmed) {
    return null;
  }

  return invokeOptional<Session | null>("session_resume_by_title", {
    request: { title: trimmed },
  });
}

export async function sessionSearch(
  query: string,
  limit = 20,
): Promise<Session[]> {
  const trimmed = query.trim();
  if (!trimmed) {
    return [];
  }

  return invoke<Session[]>("session_search", {
    request: { query: trimmed, limit },
  });
}

export async function sessionRename(
  request: SessionRenameRequest,
): Promise<Session> {
  return invoke<Session>("session_rename", { request });
}

export async function terminalBackendListProfiles(): Promise<TerminalBackendProfile[]> {
  return invoke<TerminalBackendProfile[]>("terminal_backend_list_profiles");
}

export async function terminalBackendSaveProfile(
  request: TerminalBackendSaveProfileRequest,
): Promise<TerminalBackendProfile> {
  return invoke<TerminalBackendProfile>("terminal_backend_save_profile", { request });
}

export async function terminalBackendListStatus(): Promise<TerminalBackendStatus[]> {
  return invoke<TerminalBackendStatus[]>("terminal_backend_list_status");
}

export async function terminalBackendTestProfile(
  id: string,
): Promise<TerminalBackendTestResult> {
  return invoke<TerminalBackendTestResult>("terminal_backend_test_profile", {
    request: { id },
  });
}

export async function gatewayIngestMessage(
  request: GatewayIngestMessageRequest,
): Promise<GatewayIngestedMessage> {
  return invoke<GatewayIngestedMessage>("gateway_ingest_message", { request });
}

export async function gatewayListRecentConversations(
  request?: GatewayListRecentConversationsRequest,
): Promise<GatewayConversation[]> {
  return invoke<GatewayConversation[]>("gateway_list_recent_conversations", {
    request,
  });
}

export async function gatewayListRecentMessages(
  request: GatewayListRecentMessagesRequest = {},
): Promise<GatewayMessage[]> {
  return invoke<GatewayMessage[]>("gateway_list_recent_messages", { request });
}

export async function skillsList(): Promise<SkillListItem[]> {
  return invoke<SkillListItem[]>("skills_list");
}

export async function skillsSearch(
  request: SkillSearchRequest,
): Promise<SkillListItem[]> {
  return invoke<SkillListItem[]>("skills_search", { request });
}

export async function skillsView(request: SkillViewRequest): Promise<SkillDetailItem> {
  return invoke<SkillDetailItem>("skills_view", { request });
}

export async function skillsInstall(
  request: SkillInstallRequest,
): Promise<SkillDetailItem> {
  return invoke<SkillDetailItem>("skills_install", { request });
}

export async function skillsMarketplaceList(
  request: SkillMarketplaceListRequest,
): Promise<SkillMarketplaceCatalog> {
  return invoke<SkillMarketplaceCatalog>("skills_marketplace_list", { request });
}

export async function skillsMarketplaceInstall(
  request: SkillMarketplaceInstallRequest,
): Promise<SkillMarketplaceInstallResult> {
  return invoke<SkillMarketplaceInstallResult>("skills_marketplace_install", { request });
}

export async function skillsMarketplaceListInstallHistory(
  request: SkillMarketplaceInstallHistoryListRequest = {},
): Promise<SkillMarketplaceInstallHistoryItem[]> {
  return invoke<SkillMarketplaceInstallHistoryItem[]>("skills_marketplace_list_install_history", { request });
}

export async function skillsInvoke(
  request: SkillInvokeRequest,
): Promise<SkillInvocationPayload> {
  return invoke<SkillInvocationPayload>("skills_invoke", { request });
}

export async function skillsInvokeIntoSession(
  request: SkillInvokeSessionRequest,
): Promise<SkillSessionInvocationResult> {
  return invoke<SkillSessionInvocationResult>("skills_invoke_into_session", { request });
}

export async function skillsExecuteRuntime(
  request: SkillRuntimeExecuteRequest,
): Promise<SkillRuntimeExecutionResult> {
  return invoke<SkillRuntimeExecutionResult>("skills_execute_runtime", { request });
}

export async function skillsListSessionInvocations(
  request: SkillSessionInvocationListRequest,
): Promise<SessionMessage[]> {
  return invoke<SessionMessage[]>("skills_list_session_invocations", { request });
}

export async function skillsSetEnabled(
  request: SkillSetEnabledRequest,
): Promise<void> {
  await invoke("skills_set_enabled", { request });
}

export async function skillEvolutionCandidateList(
  request: SkillEvolutionCandidateListRequest = {},
): Promise<SkillEvolutionCandidate[]> {
  return invoke<SkillEvolutionCandidate[]>("skill_evolution_candidate_list", {
    request,
  });
}

export async function skillEvolutionCandidateCreate(
  request: SkillEvolutionCandidateCreateRequest,
): Promise<SkillEvolutionCandidate> {
  return invoke<SkillEvolutionCandidate>("skill_evolution_candidate_create", {
    request,
  });
}

export async function skillEvolutionCandidateGenerate(
  request: SkillEvolutionCandidateGenerateRequest = {},
): Promise<SkillEvolutionCandidate[]> {
  return invoke<SkillEvolutionCandidate[]>("skill_evolution_candidate_generate", {
    request,
  });
}

export async function skillEvolutionCandidateSetStatus(
  request: SkillEvolutionCandidateSetStatusRequest,
): Promise<SkillEvolutionCandidate> {
  return invoke<SkillEvolutionCandidate>("skill_evolution_candidate_set_status", {
    request,
  });
}

export async function parityGetCatalog(): Promise<ParityCatalog> {
  return invoke<ParityCatalog>("parity_get_catalog");
}

export async function parityGetRuntimeReadiness(): Promise<ParityRuntimeReadiness> {
  return invoke<ParityRuntimeReadiness>("parity_get_runtime_readiness");
}

export async function paritySaveProviderSelection(
  request: ParityProviderSelectionRequest,
): Promise<ParityProviderSelection> {
  return invoke<ParityProviderSelection>("parity_save_provider_selection", { request });
}

export async function parityToolsetList(): Promise<ParityToolset[]> {
  return invoke<ParityToolset[]>("parity_toolset_list");
}

export async function parityToolsetSave(
  request: ParityToolsetSaveRequest,
): Promise<ParityToolset> {
  return invoke<ParityToolset>("parity_toolset_save", { request });
}

export async function parityCronList(): Promise<ParityCronJob[]> {
  return invoke<ParityCronJob[]>("parity_cron_list");
}

export async function parityCronCreate(
  request: ParityCronCreateRequest,
): Promise<ParityCronJob> {
  return invoke<ParityCronJob>("parity_cron_create", { request });
}

export async function parityCronRuntimeStatus(): Promise<ParityCronRuntimeStatus> {
  return invoke<ParityCronRuntimeStatus>("parity_cron_runtime_status");
}

export async function parityCronRuntimeTick(): Promise<ParityCronRuntimeTickResult> {
  return invoke<ParityCronRuntimeTickResult>("parity_cron_runtime_tick");
}

export async function parityCronSetEnabled(
  id: string,
  enabled: boolean,
): Promise<ParityCronJob> {
  return invoke<ParityCronJob>("parity_cron_set_enabled", {
    request: { id, enabled },
  });
}

export async function parityCronRunNow(id: string): Promise<ParityCronJob> {
  return invoke<ParityCronJob>("parity_cron_run_now", { request: { id } });
}

export async function parityMcpList(): Promise<ParityMcpServer[]> {
  return invoke<ParityMcpServer[]>("parity_mcp_list");
}

export async function parityMcpProbe(id: string): Promise<ParityMcpProbeResult> {
  return invoke<ParityMcpProbeResult>("parity_mcp_probe", { request: { id } });
}

export async function parityMcpUpsert(
  request: ParityMcpUpsertRequest,
): Promise<ParityMcpServer> {
  return invoke<ParityMcpServer>("parity_mcp_upsert", { request });
}

export async function parityMcpRuntimeListStatus(): Promise<ParityMcpServerRuntimeStatus[]> {
  return invoke<ParityMcpServerRuntimeStatus[]>("parity_mcp_runtime_list_status");
}

export async function parityMcpRuntimeStart(
  id: string,
): Promise<ParityMcpServerRuntimeStatus> {
  return invoke<ParityMcpServerRuntimeStatus>("parity_mcp_runtime_start", {
    request: { id },
  });
}

export async function parityMcpRuntimeStop(
  id: string,
): Promise<ParityMcpServerRuntimeStatus> {
  return invoke<ParityMcpServerRuntimeStatus>("parity_mcp_runtime_stop", {
    request: { id },
  });
}

export async function parityMcpRuntimeReload(
  id: string,
): Promise<ParityMcpServerRuntimeStatus> {
  return invoke<ParityMcpServerRuntimeStatus>("parity_mcp_runtime_reload", {
    request: { id },
  });
}

export async function parityQuickCommandList(): Promise<ParityQuickCommand[]> {
  return invoke<ParityQuickCommand[]>("parity_quick_command_list");
}

export async function parityQuickCommandSave(
  request: ParityQuickCommandSaveRequest,
): Promise<ParityQuickCommand> {
  return invoke<ParityQuickCommand>("parity_quick_command_save", { request });
}

export async function knowledgeList(
  request?: KnowledgeListRequest,
): Promise<KnowledgeFeedItem[]> {
  return invoke<KnowledgeFeedItem[]>("knowledge_list", { request });
}

export async function knowledgeSourceList(
  request?: KnowledgeListRequest,
): Promise<KnowledgeSource[]> {
  return invoke<KnowledgeSource[]>("knowledge_source_list", { request });
}

export async function knowledgeImport(
  request: KnowledgeImportRequest,
): Promise<MissionContextItem> {
  return invoke<MissionContextItem>("knowledge_import", { request });
}

export async function knowledgeImportFolder(
  request: KnowledgeFolderImportRequest,
): Promise<KnowledgeFolderImportResponse> {
  return invoke<KnowledgeFolderImportResponse>("knowledge_import_folder", {
    request,
  });
}

export async function knowledgeFetchUrlPreview(
  request: KnowledgeUrlPreviewRequest,
): Promise<KnowledgeUrlPreviewResponse> {
  return invoke<KnowledgeUrlPreviewResponse>("knowledge_fetch_url_preview", {
    request,
  });
}

export async function simulationGetOverview(): Promise<SimulationOverview> {
  return invoke<SimulationOverview>("simulation_get_overview");
}

export async function simulationCreateScenarioRun(
  request: SimulationCreateScenarioRunRequest,
): Promise<ScenarioRun> {
  return invoke<ScenarioRun>("simulation_create_scenario_run", { request });
}

export async function simulationListScenarioRuns(
  missionId: string,
): Promise<ScenarioRun[]> {
  return invoke<ScenarioRun[]>("simulation_list_scenario_runs", {
    request: { mission_id: missionId },
  });
}

export async function simulationGetComparisonMatrix(
  request: SimulationComparisonRequest,
): Promise<SimulationComparisonMatrix | null> {
  return invokeOptional<SimulationComparisonMatrix>(
    "simulation_get_comparison_matrix",
    { request },
  );
}


export async function simulationRunLocalSandbox(
  request: SimulationRunLocalSandboxRequest,
): Promise<SimulationLocalSandboxRun> {
  return invoke<SimulationLocalSandboxRun>("simulation_run_local_sandbox", { request });
}

export async function simulationListLocalSandboxRuns(
  request: SimulationLocalSandboxRunListRequest = {},
): Promise<SimulationLocalSandboxRun[]> {
  return invoke<SimulationLocalSandboxRun[]>("simulation_list_local_sandbox_runs", { request });
}

export async function simulationRunExternalSaas(
  request: SimulationRunExternalSaasRequest,
): Promise<SimulationExternalSaasRun> {
  return invoke<SimulationExternalSaasRun>("simulation_run_external_saas", { request });
}

export async function simulationListExternalSaasRuns(
  request: SimulationCapabilityRunListRequest = {},
): Promise<SimulationExternalSaasRunHistoryItem[]> {
  return invoke<SimulationExternalSaasRunHistoryItem[]>("simulation_list_external_saas_runs", { request });
}

export async function simulationRunHighFidelitySandbox(
  request: SimulationRunHighFidelitySandboxRequest,
): Promise<SimulationHighFidelitySandboxRun> {
  return invoke<SimulationHighFidelitySandboxRun>("simulation_run_high_fidelity_sandbox", { request });
}

export async function simulationListHighFidelitySandboxRuns(
  request: SimulationCapabilityRunListRequest = {},
): Promise<SimulationHighFidelitySandboxRunHistoryItem[]> {
  return invoke<SimulationHighFidelitySandboxRunHistoryItem[]>("simulation_list_high_fidelity_sandbox_runs", { request });
}

export async function simulationListHandoffPolicyTemplates(): Promise<SimulationHandoffPolicyTemplate[]> {
  return invoke<SimulationHandoffPolicyTemplate[]>("simulation_list_handoff_policy_templates");
}

export async function simulationSaveHandoffPolicyTemplate(
  request: SimulationSaveHandoffPolicyTemplateRequest,
): Promise<SimulationHandoffPolicyTemplate> {
  return invoke<SimulationHandoffPolicyTemplate>("simulation_save_handoff_policy_template", { request });
}

export async function simulationListScoringFormulaTemplates(): Promise<SimulationScoringFormulaTemplate[]> {
  return invoke<SimulationScoringFormulaTemplate[]>("simulation_list_scoring_formula_templates");
}

export async function simulationSaveScoringFormulaTemplate(
  request: SimulationSaveScoringFormulaTemplateRequest,
): Promise<SimulationScoringFormulaTemplate> {
  return invoke<SimulationScoringFormulaTemplate>("simulation_save_scoring_formula_template", { request });
}

export async function simulationExportTemplateBundle(): Promise<SimulationTemplateBundle> {
  return invoke<SimulationTemplateBundle>("simulation_export_template_bundle");
}

export async function simulationImportTemplateBundle(
  request: SimulationImportTemplateBundleRequest,
): Promise<SimulationImportTemplateBundleResponse> {
  return invoke<SimulationImportTemplateBundleResponse>("simulation_import_template_bundle", { request });
}

export async function simulationListTemplateBundleAuditLog(): Promise<SimulationTemplateBundleAuditEntry[]> {
  return invoke<SimulationTemplateBundleAuditEntry[]>("simulation_list_template_bundle_audit_log");
}

export async function simulationExportTemplateBundleAuditLog(
  request: SimulationExportTemplateBundleAuditLogRequest = {},
): Promise<SimulationExportTemplateBundleAuditLogResponse> {
  return invoke<SimulationExportTemplateBundleAuditLogResponse>("simulation_export_template_bundle_audit_log", { request });
}

export async function simulationPreflightTemplateBundleImport(
  request: SimulationImportTemplateBundleRequest,
): Promise<SimulationTemplateBundlePreflightResponse> {
  return invoke<SimulationTemplateBundlePreflightResponse>("simulation_preflight_template_bundle_import", { request });
}

export async function globalSearch(
  request: GlobalSearchRequest,
): Promise<GlobalSearchResult[]> {
  return invoke<GlobalSearchResult[]>("global_search", { request });
}

export async function notificationsList(): Promise<NotificationItem[]> {
  return invoke<NotificationItem[]>("notifications_list");
}

export async function runEventList(missionId: string): Promise<RunEventItem[]> {
  return invoke<RunEventItem[]>("run_event_list", {
    request: { mission_id: missionId },
  });
}

export async function trajectoryExportDataset(
  request: TrajectoryExportRequest = {},
): Promise<TrajectoryDatasetExport> {
  return invoke<TrajectoryDatasetExport>("trajectory_export_dataset", { request });
}

export async function trajectoryRunLocalRlTraining(
  request: TrajectoryRlTrainingRequest,
): Promise<TrajectoryRlTrainingResult> {
  return invoke<TrajectoryRlTrainingResult>("trajectory_run_local_rl_training", { request });
}

export async function trajectoryListLocalRlTrainingJobs(
  request: TrajectoryRlTrainingJobListRequest = {},
): Promise<TrajectoryRlTrainingResult[]> {
  return invoke<TrajectoryRlTrainingResult[]>("trajectory_list_local_rl_training_jobs", { request });
}

export async function memoryRecordList(
  request?: MemoryRecordListRequest,
): Promise<MemoryRecordItem[]> {
  return invoke<MemoryRecordItem[]>("memory_record_list", { request });
}

export async function memoryRecordCreate(
  request: MemoryRecordCreateRequest,
): Promise<MemoryRecordItem> {
  return invoke<MemoryRecordItem>("memory_record_create", { request });
}

export async function memoryRecordSearch(
  request: MemoryRecordSearchRequest,
): Promise<MemoryRecordItem[]> {
  return invoke<MemoryRecordItem[]>("memory_record_search", { request });
}

export async function voiceStatus(): Promise<VoiceSettings> {
  return invoke<VoiceSettings>("voice_status");
}

export async function voiceListProviders(): Promise<VoiceProvider[]> {
  return invoke<VoiceProvider[]>("voice_list_providers");
}

export async function voiceSetEnabled(
  request: VoiceSetEnabledRequest,
): Promise<VoiceSettings> {
  return invoke<VoiceSettings>("voice_set_enabled", { request });
}

export async function voiceUpdateSettings(
  request: VoiceUpdateSettingsRequest,
): Promise<VoiceSettings> {
  return invoke<VoiceSettings>("voice_update_settings", { request });
}

export async function voiceTranscribe(
  request: VoiceTranscribeRequest,
): Promise<VoiceTranscriptionResult> {
  return invoke<VoiceTranscriptionResult>("voice_transcribe", { request });
}


export async function voiceSpeak(
  request: VoiceSpeakRequest,
): Promise<VoiceSpeakResult> {
  return invoke<VoiceSpeakResult>("voice_speak", { request });
}


export async function voiceListHistory(
  request: VoiceHistoryListRequest,
): Promise<VoiceHistoryListResponse> {
  return invoke<VoiceHistoryListResponse>("voice_list_history", { request });
}

export async function voiceProcessSpeakQueue(
  request: VoiceProcessSpeakQueueRequest,
): Promise<VoiceProcessSpeakQueueResponse> {
  return invoke<VoiceProcessSpeakQueueResponse>("voice_process_speak_queue", { request });
}

export async function playbookGet(missionId: string): Promise<MissionPlaybook> {
  return invoke<MissionPlaybook>("playbook_get", {
    request: { mission_id: missionId },
  });
}

export async function councilStepList(missionId: string): Promise<CouncilStepItem[]> {
  return invoke<CouncilStepItem[]>("council_step_list", {
    request: { mission_id: missionId },
  });
}

export async function councilStepCreate(
  request: CouncilStepCreateRequest,
): Promise<CouncilStepItem> {
  return invoke<CouncilStepItem>("council_step_create", { request });
}

// 兼容旧命令
export async function checkEnvironment() {
  return invoke("check_environment");
}

export async function getHermesStatus() {
  return invoke("get_hermes_status");
}

export async function loadConfig() {
  return invoke("load_config");
}

export async function saveConfig(config: unknown) {
  return invoke("save_config", { config });
}
