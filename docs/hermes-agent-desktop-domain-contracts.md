# Hermes Operator 领域模型与契约文档

## 1. 文档目标

本文档定义核心领域对象、状态机、SQLite 表设计、Tauri IPC 契约和 Agent Engine 契约。AI 编程工具必须先按照本文档建立类型和接口，再实现页面和业务逻辑。

## 2. 核心领域对象

## 2.1 AppSettings

```ts
type ThemeMode = "system" | "light" | "dark";

interface AppSettings {
  themeMode: ThemeMode;
  language: "zh-CN" | "en-US";
  launchAtLogin: boolean;
  defaultWorkspacePath: string;
  logLevel: "debug" | "info" | "warn" | "error";
  requireApprovalForRisk: "high" | "medium" | "never";
}
```

## 2.2 RuntimeSettings

```ts
interface RuntimeSettings {
  provider: "openai" | "anthropic" | "deepseek" | "ollama" | "openrouter";
  model: string;
  baseUrl?: string;
  apiKeyRef?: string;
  engineProfile?: string;
  agentEngineEnabled: boolean;
}
```

## 2.3 Mission

```ts
type MissionStatus =
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

interface Mission {
  id: string;
  title: string;
  goal: string;
  constraints: string[];
  successCriteria: string[];
  status: MissionStatus;
  priority: "low" | "medium" | "high";
  pinned: boolean;
  createdAt: string;
  updatedAt: string;
  lastActivityAt: string;
}
```

## 2.4 MissionContextItem

```ts
type ContextItemType =
  | "file"
  | "url"
  | "note"
  | "memory"
  | "knowledge_result"
  | "artifact";

interface MissionContextItem {
  id: string;
  missionId: string;
  type: ContextItemType;
  title: string;
  contentPreview?: string;
  sourceUri?: string;
  pinned: boolean;
  createdAt: string;
}
```

## 2.5 Run

```ts
type RunType = "research" | "simulation" | "council" | "execution" | "growth";
type RunStatus = "queued" | "running" | "completed" | "failed" | "cancelled";

interface Run {
  id: string;
  missionId: string;
  type: RunType;
  status: RunStatus;
  startedAt?: string;
  finishedAt?: string;
  summary?: string;
  errorMessage?: string;
}
```

## 2.6 Artifact

```ts
type ArtifactType =
  | "markdown"
  | "report"
  | "plan"
  | "json"
  | "text"
  | "image"
  | "file";

interface Artifact {
  id: string;
  missionId: string;
  runId?: string;
  type: ArtifactType;
  title: string;
  path: string;
  mimeType?: string;
  createdAt: string;
}
```

## 2.7 KnowledgeSource

```ts
type KnowledgeSourceType = "file" | "folder" | "url" | "manual";
type KnowledgeIndexStatus = "pending" | "indexing" | "ready" | "failed";

interface KnowledgeSource {
  id: string;
  type: KnowledgeSourceType;
  title: string;
  sourceUri: string;
  indexStatus: KnowledgeIndexStatus;
  chunkCount: number;
  createdAt: string;
  updatedAt: string;
}
```

## 2.8 MemoryRecord

```ts
type MemoryScope = "user" | "project" | "mission";

interface MemoryRecord {
  id: string;
  scope: MemoryScope;
  scopeRef: string;
  title: string;
  content: string;
  sourceType: "mission_note" | "run_event" | "manual" | "artifact";
  importance: "low" | "medium" | "high";
  createdAt: string;
}
```

## 2.9 ScenarioRun

```ts
interface ScenarioOption {
  id: string;
  label: string;
  assumptions: string[];
  expectedBenefits: string[];
  risks: string[];
  confidence: "low" | "medium" | "high";
}

interface ScenarioRun {
  id: string;
  missionId: string;
  baseline: string;
  options: ScenarioOption[];
  recommendation?: string;
  createdAt: string;
}
```

## 2.10 CouncilStep

```ts
type CouncilRole =
  | "scout"
  | "analyst"
  | "critic"
  | "planner"
  | "executor"
  | "reviewer";

type CouncilStepStatus = "pending" | "running" | "completed" | "rejected" | "failed";

interface CouncilStep {
  id: string;
  missionId: string;
  runId: string;
  role: CouncilRole;
  status: CouncilStepStatus;
  inputSummary?: string;
  outputSummary?: string;
  reviewNote?: string;
  createdAt: string;
  updatedAt: string;
}
```

## 2.11 ExecutionStep

```ts
type ExecutionMode = "api" | "cli" | "browser" | "desktop";
type RiskLevel = "low" | "medium" | "high";
type ExecutionStepStatus =
  | "pending"
  | "awaiting_approval"
  | "running"
  | "completed"
  | "failed"
  | "skipped";

interface ExecutionStep {
  id: string;
  missionId: string;
  runId: string;
  title: string;
  mode: ExecutionMode;
  riskLevel: RiskLevel;
  status: ExecutionStepStatus;
  inputPayload?: string; // may include user_notes and latest_user_note
  outputSummary?: string;
  createdAt: string;
  updatedAt: string;
}
```

### `execution_add_step_note`

请求：

```json
{
  "id": "execution-step-id",
  "note": "Wait for human review before continuing",
  "pause_before_continue": true
}
```

响应：更新后的 `ExecutionStep`。批注会追加到 `inputPayload.user_notes`，记录 `step_note_added` timeline event；当目标步骤为 `running` 且 `pause_before_continue` 为 true 时，步骤会切换到 `paused`。

### `execution_list_desktop_handoff_queue`

请求：

```json
{
  "mission_id": "mission-001"
}
```

响应：desktop mode execution steps 的 handoff queue，每项包含 `step`、`handoff_prepared`、`prepared_event_count`、`latest_prepared_at`、`handoff_reviewed`、`reviewed_event_count` 和 `latest_reviewed_at`。该 read-model 只反映本地 handoff 准备/人工复核状态，不代表 GUI runtime 已执行。

### `execution_mark_desktop_handoff_reviewed`

请求：

```json
{
  "run_id": "run-001",
  "step_id": "execution-step-id",
  "review_note": "Human checked target window and inputs"
}
```

响应：空成功响应。该命令只为已经 prepared 的 `desktop` step 记录 `desktop_handoff_reviewed` timeline event，payload 精确包含 `step_id` 和人工 note；它不执行 GUI 自动化。

### `execution_prepare_desktop_handoff`

请求：

```json
{
  "id": "desktop-execution-step-id"
}
```

响应：

```json
{
  "step_id": "desktop-execution-step-id",
  "mission_id": "mission-001",
  "run_id": "run-001",
  "title": "Open target app",
  "status": "awaiting_approval",
  "risk_level": "high",
  "automatic_execution": false,
  "reason": "Desktop GUI runtime is not connected...",
  "checklist": ["Confirm approval before touching the desktop target."],
  "input_payload": {},
  "handoff_prompt": "desktop_handoff\tstep=..."
}
```

约束：仅支持 `mode = desktop` 的 execution step；该命令只生成可审计 handoff prompt/checklist，并记录 `desktop_handoff_prepared` timeline event，不执行 GUI 自动化。

## 3. 状态机

## 3.1 Mission 状态机

```text
draft
  -> researching
  -> simulating
  -> planning
  -> awaiting_approval
  -> executing
  -> completed

draft/researching/simulating/planning/executing
  -> paused

paused
  -> researching/simulating/planning/executing

any active status
  -> failed

completed/failed
  -> archived
```

### 约束

- `completed` 不能回到 `draft`
- `archived` 只能查看，不能继续执行
- `awaiting_approval` 只能进入 `executing` 或 `paused`

## 3.2 Run 状态机

```text
queued -> running -> completed
queued -> running -> failed
queued -> cancelled
running -> cancelled
```

## 3.3 ExecutionStep 状态机

```text
pending -> awaiting_approval -> running -> completed
pending -> running -> completed
running -> failed
awaiting_approval -> skipped
pending -> skipped
```

## 4. SQLite 表设计

## 4.1 `app_settings`

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `key` | TEXT PRIMARY KEY | 配置键 |
| `value_json` | TEXT NOT NULL | JSON 序列化值 |
| `updated_at` | TEXT NOT NULL | 更新时间 |

## 4.2 `missions`

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `id` | TEXT PRIMARY KEY | UUID |
| `title` | TEXT NOT NULL | 标题 |
| `goal` | TEXT NOT NULL | 目标 |
| `constraints_json` | TEXT NOT NULL | 约束数组 |
| `success_criteria_json` | TEXT NOT NULL | 成功标准数组 |
| `status` | TEXT NOT NULL | 状态 |
| `priority` | TEXT NOT NULL | 优先级 |
| `pinned` | INTEGER NOT NULL | 0/1 |
| `created_at` | TEXT NOT NULL | 创建时间 |
| `updated_at` | TEXT NOT NULL | 更新时间 |
| `last_activity_at` | TEXT NOT NULL | 最后活动时间 |

索引：

- `idx_missions_status`
- `idx_missions_last_activity_at`

## 4.3 `mission_context_items`

| 字段 | 类型 |
| --- | --- |
| `id` | TEXT PRIMARY KEY |
| `mission_id` | TEXT NOT NULL |
| `type` | TEXT NOT NULL |
| `title` | TEXT NOT NULL |
| `content_preview` | TEXT |
| `source_uri` | TEXT |
| `pinned` | INTEGER NOT NULL |
| `created_at` | TEXT NOT NULL |

## 4.4 `runs`

| 字段 | 类型 |
| --- | --- |
| `id` | TEXT PRIMARY KEY |
| `mission_id` | TEXT NOT NULL |
| `type` | TEXT NOT NULL |
| `status` | TEXT NOT NULL |
| `started_at` | TEXT |
| `finished_at` | TEXT |
| `summary` | TEXT |
| `error_message` | TEXT |

索引：

- `idx_runs_mission_id`
- `idx_runs_status`

## 4.5 `run_events`

| 字段 | 类型 |
| --- | --- |
| `id` | TEXT PRIMARY KEY |
| `run_id` | TEXT NOT NULL |
| `mission_id` | TEXT NOT NULL |
| `event_type` | TEXT NOT NULL |
| `message` | TEXT NOT NULL |
| `payload_json` | TEXT |
| `created_at` | TEXT NOT NULL |

## 4.6 `artifacts`

| 字段 | 类型 |
| --- | --- |
| `id` | TEXT PRIMARY KEY |
| `mission_id` | TEXT NOT NULL |
| `run_id` | TEXT |
| `type` | TEXT NOT NULL |
| `title` | TEXT NOT NULL |
| `path` | TEXT NOT NULL |
| `mime_type` | TEXT |
| `created_at` | TEXT NOT NULL |

## 4.7 `knowledge_sources`

| 字段 | 类型 |
| --- | --- |
| `id` | TEXT PRIMARY KEY |
| `type` | TEXT NOT NULL |
| `title` | TEXT NOT NULL |
| `source_uri` | TEXT NOT NULL |
| `index_status` | TEXT NOT NULL |
| `chunk_count` | INTEGER NOT NULL |
| `created_at` | TEXT NOT NULL |
| `updated_at` | TEXT NOT NULL |

## 4.8 `knowledge_chunks`

| 字段 | 类型 |
| --- | --- |
| `id` | TEXT PRIMARY KEY |
| `source_id` | TEXT NOT NULL |
| `chunk_index` | INTEGER NOT NULL |
| `content` | TEXT NOT NULL |
| `metadata_json` | TEXT |

## 4.9 `memory_records`

| 字段 | 类型 |
| --- | --- |
| `id` | TEXT PRIMARY KEY |
| `scope` | TEXT NOT NULL |
| `scope_ref` | TEXT NOT NULL |
| `title` | TEXT NOT NULL |
| `content` | TEXT NOT NULL |
| `source_type` | TEXT NOT NULL |
| `importance` | TEXT NOT NULL |
| `created_at` | TEXT NOT NULL |

## 4.10 `scenario_runs`

| 字段 | 类型 |
| --- | --- |
| `id` | TEXT PRIMARY KEY |
| `mission_id` | TEXT NOT NULL |
| `baseline` | TEXT NOT NULL |
| `options_json` | TEXT NOT NULL |
| `recommendation` | TEXT |
| `created_at` | TEXT NOT NULL |

## 4.11 `council_steps`

| 字段 | 类型 |
| --- | --- |
| `id` | TEXT PRIMARY KEY |
| `mission_id` | TEXT NOT NULL |
| `run_id` | TEXT NOT NULL |
| `role` | TEXT NOT NULL |
| `status` | TEXT NOT NULL |
| `input_summary` | TEXT |
| `output_summary` | TEXT |
| `review_note` | TEXT |
| `created_at` | TEXT NOT NULL |
| `updated_at` | TEXT NOT NULL |

## 4.12 `execution_steps`

| 字段 | 类型 |
| --- | --- |
| `id` | TEXT PRIMARY KEY |
| `mission_id` | TEXT NOT NULL |
| `run_id` | TEXT NOT NULL |
| `title` | TEXT NOT NULL |
| `mode` | TEXT NOT NULL |
| `risk_level` | TEXT NOT NULL |
| `status` | TEXT NOT NULL |
| `input_payload` | TEXT |
| `output_summary` | TEXT |
| `created_at` | TEXT NOT NULL |
| `updated_at` | TEXT NOT NULL |

## 5. Rust 侧内部接口

## 5.1 `BootstrapService`

```rust
pub trait BootstrapService {
    fn get_bootstrap(&self) -> Result<BootstrapPayload, AppError>;
}
```

## 5.2 `MissionService`

```rust
pub trait MissionService {
    fn list(&self, filter: MissionListFilter) -> Result<Vec<Mission>, AppError>;
    fn create(&self, input: CreateMissionInput) -> Result<Mission, AppError>;
    fn get(&self, id: &str) -> Result<MissionDetail, AppError>;
    fn update(&self, id: &str, input: UpdateMissionInput) -> Result<Mission, AppError>;
    fn archive(&self, id: &str) -> Result<(), AppError>;
}
```

## 5.3 `KnowledgeService`

```rust
pub trait KnowledgeService {
    fn import_files(&self, paths: Vec<String>) -> Result<ImportJob, AppError>;
    fn list_sources(&self) -> Result<Vec<KnowledgeSource>, AppError>;
    fn search(&self, query: SearchQuery) -> Result<SearchResultPage, AppError>;
}
```

## 5.4 `AgentEngineService`

```rust
pub trait AgentEngineService {
    fn status(&self) -> Result<AgentEngineStatus, AppError>;
    fn start(&self) -> Result<AgentEngineStatus, AppError>;
    fn stop(&self) -> Result<AgentEngineStatus, AppError>;
    fn restart(&self) -> Result<AgentEngineStatus, AppError>;
}
```

## 6. Tauri IPC 契约

命名规则：

- 查询：`*_get` / `*_list`
- 命令：`*_create` / `*_update` / `*_start` / `*_stop`
- 所有响应统一返回结构化 JSON

## 6.1 App

### `app_get_bootstrap`

请求：

```json
{}
```

响应：

```json
{
  "appSettings": {},
  "runtimeSettings": {},
  "engineStatus": {
    "running": false,
    "profile": null
  },
  "hermesStatus": {
    "installed": true,
    "running": false,
    "version": "0.8.0"
  },
  "activeMission": {
    "id": "mission-001",
    "title": "Launch recovery",
    "status": "paused",
    "priority": "high"
  },
  "summary": {
    "activeMissionCount": 0,
    "pendingApprovalCount": 0
  }
}
```

### `simulation_run_local_sandbox`

请求：

```json
{
  "mission_id": "mission-001",
  "baseline": "Keep current launch plan",
  "options": ["Run pilot", "Delay launch"],
  "agents": [
    { "role": "strategy", "stance": "optimistic", "name": "Strategy Agent" },
    { "role": "risk", "stance": "skeptical", "name": "Risk Agent" }
  ],
  "rounds": 3
}
```

响应：内置 deterministic multi-agent sandbox 结果，包含 `run_id`、`engine`、`rounds`、`agents`、逐轮 `turns`、`option_scores`、`recommendation` 和 `audit_event_id`。命令会真实写入 completed `runs` row 和 `local_sandbox_simulation_completed` run event。该能力是本地可运行仿真引擎，不依赖外部 SaaS。

### `simulation_list_local_sandbox_runs`

请求：

```json
{
  "mission_id": "mission-001",
  "limit": 5
}
```

响应：最近的 Local Multi-Agent Sandbox runs，按最新优先排序，可按 Mission 过滤。每项复用 `simulation_run_local_sandbox` 的结果结构，用于历史回放和 UI 审计。

### `simulation_export_template_bundle`

请求：

```json
{}
```

响应：

```json
{
  "schema_version": 1,
  "exported_at": "2026-04-27T00:00:00Z",
  "handoff_policy_templates": [],
  "scoring_formula_templates": []
}
```

### `simulation_import_template_bundle`

请求：

```json
{
  "bundle_json": "{...}"
}
```

响应：导入后的 handoff policy templates、scoring formula templates 和 imported counts。该契约用于手动团队共享；自动远端同步和 RBAC 仍属于团队服务集成。

### `simulation_preflight_template_bundle_import`

请求：

```json
{
  "bundle_json": "{...}"
}
```

响应：导入预检摘要，包括 handoff policy templates 与 scoring formula templates 的新增、覆盖、未变化数量，以及同 ID 更新冲突列表。该命令不写入模板、不写入 audit log。

### `simulation_list_template_bundle_audit_log`

请求：

```json
{}
```

响应：最近本地 template bundle export/import audit entries，包含 `action`、`actor`、模板数量和 `occurred_at`。该日志是本地审计线索，不等同于服务端 RBAC 审计。

### `simulation_export_template_bundle_audit_log`

请求：

```json
{
  "limit": 50
}
```

响应：只读 audit export JSON，包含 `total`、`exported_count` 和按最新优先排序的 `events`。该命令不写入新的 audit event、不修改模板，用于无服务端/RBAC 前提下的手动审计归档。

## 6.2 Team Sync Governance

### `team_sync_get_state`

响应：本地 team governance 状态，包含 members、role policies、audit events 和 last synced timestamp。状态存储于本机 SQLite `app_settings`，不需要额外迁移。

### `team_sync_upsert_member`

请求：

```json
{
  "actor_member_id": "local-owner",
  "member_id": "teammate",
  "role": "editor"
}
```

响应：更新后的 member，并记录本地 audit event。首次写入会 bootstrap owner。

### `team_sync_check_access`

请求：

```json
{
  "actor_member_id": "teammate",
  "resource": "bundle",
  "action": "export"
}
```

响应：`allowed` 和 `reason`。内置角色语义：owner 全权限；admin 可管理成员和 bundle；editor 可读取/导出/文件同步；viewer 只读。

### `team_sync_export_bundle` / `team_sync_import_bundle` / `team_sync_run_folder_sync`

响应：本地 JSON bundle 导出、导入和可选共享目录 JSON 文件同步。该能力提供真实本地 RBAC + audit + bundle sync；云端服务端 RBAC 可作为后续远端适配器，不是当前本地闭环的前置条件。

### `team_sync_export_audit`

请求：

```json
{
  "actor_member_id": "local-owner",
  "actor": "local-owner",
  "action": "upsert_member",
  "limit": 50,
  "format": "jsonl"
}
```

响应：本地 Team Governance audit events 的 `total`、`exported_count`、`payload` 和 `events`。命令复用 `bundle:export` 权限语义，viewer 被拒绝，editor/admin/owner 可导出；导出动作自身会写入本地 audit event，便于手动审计归档。

## 6.3 Runtime Adapters

本节把两类契约分开：

- `runtime_adapter_*` 是 Hermes 本地 runtime adapter 契约，只负责本地进程执行、dry-run 探测和审计记录
- `turix_cua_*` 是外部 `TuriX-CUA` runtime bridge 契约，只描述外部 runtime handoff、launch 约束和权限前置条件，不是 Hermes 原生 GUI capability
- `native_cua_*` 是 Hermes-native rewrite track 契约，只负责本地 session / probe / observe / action / audit substrate，不代表 OSWorld / SOTA 证据

Hermes 原生安全闭环始终留在本地并保持可审计。任何 `OSWorld`、`SOTA` 或真实 GUI 能力判断，都必须来自真实 OS / browser runtime 与真实系统权限，不能由文档、静态契约或文字描述模拟出来。

### `runtime_adapter_execute_skill_tool`

请求：

```json
{
  "command": "echo",
  "args": ["hello"],
  "timeout_ms": 5000
}
```

响应：本地 allowlisted process execution 的 `exit_code`、`stdout`、`stderr`、`duration_ms`、`timed_out` 和 audit message。命令不调用 LLM，不走 shell，不允许任意二进制；`timeout_ms` 会被归一化并强制执行，超时进程会被终止。`cat` 仅允许相对路径普通文件，禁止绝对路径/`..` 穿越/目录读取，并限制单文件 64 KiB；拒绝也会进入 Runtime Adapter audit log。

### `runtime_adapter_probe_desktop_executor` / `runtime_adapter_execute_desktop_action`

响应：桌面 session/tool availability 探测，以及默认 dry-run 的桌面 action plan。只有当显式 `dry_run=false` 且 executor 属于平台 allowlist 并存在于 PATH 时才执行本地命令；执行路径也会返回 `timed_out`。该契约只面向 Hermes 本地、可审计的命令型桌面动作，不承担真实 GUI 会话桥接；真实 GUI 桥接在下方的外部 runtime bridge 契约里单独定义。

### `turix_cua_probe`

响应：外部 `TuriX-CUA` runtime bridge 的连接状态、宿主平台、启动/运行前置条件、最近一次 handoff 结果和可用 GUI 能力列表。该状态只描述外部 bridge，不代表 Hermes 原生安全闭环已经获得 GUI 控制权，也不代表 Hermes 核心具备原生 GUI capability。

### `turix_cua_plan_command`

请求：

```json
{
  "mission_id": "mission-001",
  "step_id": "desktop-execution-step-id"
}
```

响应：发给外部 runtime bridge 的 handoff 包，包含 `bridge_state`、`permission_prereqs`、`checklist`、`handoff_prompt` 和 `risk_notes`。该命令只准备真实执行所需的外部交接与 launch 约束，不模拟成功、不伪造权限。

### `turix_cua_run`

请求：

```json
{
  "step_id": "desktop-execution-step-id",
  "action": "click"
}
```

响应：真实 bridge 执行结果，允许返回 `succeeded`、`permission_denied`、`bridge_unavailable`、`dry_run` 等状态。仅当外部 runtime bridge 已连接且系统权限真实存在时才返回可执行结果；否则必须失败，不得降级成“看起来执行过”。这个契约只描述外部 runtime handoff 与 launch 结果，不声明 Hermes 本地闭环已经获得 GUI 控制权。

### 外部 GUI 现实约束

- `OSWorld`、`SOTA`、真实 GUI 成功率和真实桌面交互结果，只能由实际 OS / browser runtime 与权限环境给出
- 文档可以约束契约、定义状态和失败模式，但不能把 benchmark 伪装成已落地能力
- 如果外部 runtime、Accessibility、屏幕录制或窗口控制权限不存在，契约只能返回失败、未连接或待交接状态

更完整的边界说明见 [TuriX-CUA Runtime Bridge 契约说明](./turix-cua-runtime-bridge.md)。

### `runtime_adapter_summarize_trajectory_jsonl`

响应：对本地 trajectory JSONL 的 kind/source/reward-hint/invalid-line summary。该能力提供研究特征摘要，不训练模型。

### `runtime_adapter_list_audit_events` / `runtime_adapter_export_audit_events`

请求：

```json
{
  "limit": 25,
  "kind": "skill_tool",
  "status": "succeeded",
  "target_remote_user_id": "future-user-001",
  "format": "jsonl"
}
```

响应：本地 Runtime Adapter 审计事件列表或导出 payload，事件包含 `id`、`occurred_at`、`kind`、`status`、`summary`、`duration_ms`、`timed_out`、`target`、`target_remote_user_id` 和 `exit_code`。`target_remote_user_id` 会 trim/empty-to-null，存在时只返回该 future remote user routing metadata 匹配的本地审计事件；该过滤同样适用于 export `total` / `exported_count`。日志复用 SQLite `app_settings`，覆盖 skill-tool、desktop action、GUI automation 和 trajectory summary 调用；导出支持 `json` / `jsonl`，不写入远端服务，也不改变 allowlist，不表示 remote delivery 或 remote GUI execution。

## 6.4 Native CUA Rewrite Track

`native_cua_*` 是 Hermes-native rewrite track 的本地契约面，目标是安全、可审计、可回放的 session / probe / observe / action / audit substrate。它不承诺 OSWorld / SOTA 证据，也不把外部 `TuriX-CUA` bridge 的可用性当作 Hermes 原生能力。

当前交付物：

- 本地 session 管理
- 安全 probe
- 只读 observe
- 受控 action execute
- 审计事件列表与导出
- 本地 deterministic planner
- TuriX-compatible actor action JSON 翻译与 step runner
- `record_info` memory 文件落地
- step history 与 trajectory JSON/JSONL 导出

扩展点：

- `native_cua_observe` 是 VLM Brain / Memory 的输入点
- `native_cua_execute_action` 是单动作 Actor 输出点
- `native_cua_plan_task` 是 Planner 输出点
- `native_cua_run_step` 是 Brain / Actor / Controller loop 的执行入口
- `native_cua_record_info` 是 Memory 的落地点
- `native_cua_export_trajectory` 是 RL/trajectory 研究层的数据出口
- `native_cua_preview_model_route` 是开始任务前的模型路由预检口
- `native_cua_prepare_model_turn` 是真实模型 runtime 的 prompt/schema 输出口
- `native_cua_invoke_model` 是真实模型 invocation 与 provider payload 入口
- `native_cua_apply_model_output` 是真实模型 JSON 输出回写入口
- Planner、Memory、Council 可以围绕同一 session 共享上下文，但不应绕过审计轨道

### `native_cua_probe`

返回 Hermes-native CUA substrate 的健康状态、会话服务可用性、审计 sink 状态和当前能力边界。这个命令只回答“本地底座是否可用”，不回答“外部 GUI 是否已经可控”。

### `native_cua_start_session`

请求：`task`、可选 `session_id`、可选 `model_mode`（`auto` / `custom`）、可选 `provider`、`model`、`base_url`、`api_key_ref`。响应：`session_id`、`status`、`task`、`resumed`、时间戳、summary，以及 session 级模型选择字段。该命令只创建或恢复本地 session，不隐式观察屏幕、不执行动作，也不声明 benchmark 能力。`model_mode=auto` 表示后端按任务难度选择 `easy` / `standard` / `hard` 的 Auto model router 档位，并把选择结果、难度和原因写入 session；`model_mode=custom` 表示将本次任务选择的模型配置写入 session，并在 `native_cua_invoke_model` 未显式覆盖时优先于桌面默认值。

### `native_cua_preview_model_route`

请求：`task`、可选 `model_mode`（`auto` / `custom`）、可选 `provider`、`model`、`base_url`、`api_key_ref`。响应：`model_mode`、可选 `provider`、`model`、`base_url`、`api_key_ref`、可选 `model_difficulty`、可选 `model_selection_reason` 和 `summary`。该命令复用 `native_cua_start_session` 的模型选择逻辑，但不创建 session、不更新 active session、不写审计；它只用于 Runtime UI 在开始任务前预览 Auto router 或 Custom 配置的实际模型路由。

### `native_cua_observe`

读取当前会话的安全观察结果，生成可供上层推理使用的结构化事实、快照或摘要。它是只读入口，不应隐式执行动作，也不应把外部 benchmark 结果包装成已完成能力。

### `native_cua_execute_action`

在当前会话上下文内执行一个受控动作，并把动作结果写入审计轨道。该命令必须失败于权限不足、前置条件缺失或执行不可审计的情况，不得静默回退到其他桥接路径。执行层会归一化少量动作别名（例如 `double_click`）和坐标尺度（`0..1` 或 TuriX `0..1000`），但公开的模型动作目录仍以 `native_cua_prepare_model_turn` 返回的 `action_catalog` 为准。

### `native_cua_list_audit_events`

列出 native CUA 会话或底座范围内的审计事件，供人类复核、回放和后续记忆沉淀使用。它是审计读取面，不是执行面。

### `native_cua_export_audit_events`

导出 native CUA 审计事件，通常以 `json` 或 `jsonl` 形式用于归档、分享或离线检查。导出只能陈述本地发生过什么，不能被当作 OSWorld / SOTA 证据。

### `native_cua_plan_task`

请求：`session_id`、可选 `task`、可选 `skill_catalog`、可选 `max_steps`。响应：本地 deterministic plan，包含 `steps`、`selected_skills`、`iteration_info`、`source` 和 summary。该命令是 TuriX Planner 的 Hermes-native 可审计改写，不调用外部 TuriX runtime。

### `native_cua_run_step`

请求：`session_id`、`dry_run`、`capture_screenshot`、可选 `brain_state`、可选 TuriX-compatible `actions`、`max_actions` 和确认短语。响应：step record，包含 observation、brain_state、action results、done/final_result 和 history length。公开 actor action key：`done`、`input_text`、`open_app`、`run_apple_script`、`Hotkey`、`multi_Hotkey`、`Click`、`RightSingle`、`Drag`、`move_mouse`、`scroll_up`、`scroll_down`、`record_info`、`wait`；执行层还会归一化少量别名（例如 `double_click`）。坐标参数接受 Hermes 原生 `0..1` 归一化值，也接受 TuriX actor 常见 `0..1000` thousandth-scale 输出，并在平台命令规划前统一转成 `0..1`。默认 dry-run；非 dry-run 必须复用 `RUN NATIVE CUA ACTION`。

### `native_cua_list_history`

请求：可选 `session_id`、`limit`、`status`。响应：按新到旧排列的 step history，用于 UI 回放、审计和 trajectory export。

### `native_cua_record_info`

请求：`session_id`、`text`、`file_name`、可选 `screenshot_path`。响应：memory record，包含本地文件路径。该命令对应 TuriX `record_info`，写入系统临时目录下的 `hermes-native-cua/records/<session>/...txt`，以及 `app_settings` memory records。

### `native_cua_export_trajectory`

请求：可选 `session_id`、`format`、`include_audit`。响应：包含 plan、step、memory 和可选 audit 的 `json` / `jsonl` payload。该命令只导出本地轨迹，不训练 RL 模型，也不声明研究结论。

### `native_cua_prepare_model_turn`

请求：`session_id`、`role`（`brain` / `actor` / `planner` / `memory`）、可选 `include_screenshot_data_url`、`max_history`、`extra_context`。响应：可送入真实模型 runtime 的 messages、response_schema、action_catalog 和 summary。当前实现会附带本地 `screenshot_path` 引用，而不是内嵌 base64 数据；该命令只准备 prompt，不调用模型。

### `native_cua_invoke_model`

请求：`session_id`、`role`（`brain` / `actor` / `planner` / `memory`）、可选 `provider`、`model`、`base_url`、`api_key_ref`、`dry_run`（默认 `true`）、`apply_output`、`capture_screenshot`、`extra_context`、`model_confirmation_phrase` 和 `action_confirmation_phrase`。响应：`session_id`、`role`、`provider`、`model`、`dry_run`、`requested`、`status`、`prompt_turn`、`http_request_preview`、可选 `raw_output`、可选 `parsed_output`、可选 `apply_result` 和 `summary`。如果请求未显式覆盖 provider/model/base_url/api_key_ref，命令会先读取 session 级 custom 模型配置；没有 session custom 配置时，再读取桌面端通过 `settings_save` 写入的 `app_settings.runtime` 默认模型配置。`base_url` 可以是 provider 根地址，也可以是常见 `/v1` endpoint，后端会避免重复拼接 API version。默认只做 dry-run 预览；非 dry-run 必须精确匹配 `INVOKE NATIVE CUA MODEL`，因为该命令可能调用外部或付费模型端点。支持 OpenAI-compatible providers（OpenAI / OpenRouter / DeepSeek）、Anthropic 和 Ollama。`apply_output=true` 时，actor JSON 会被回写到 `native_cua_run_step`；其他角色输出可以继续作为 model turn record 存档。这个契约只描述真实模型 invocation plumbing，不声明 OSWorld / SOTA 证据；桌面 live action 仍然必须另行满足 `RUN NATIVE CUA ACTION`。

### `native_cua_apply_model_output`

请求：`session_id`、`role`、模型 `output` JSON、`dry_run`、`capture_screenshot`、可选确认短语。响应：model output 存档状态；当 `role=actor` 时，还返回通过 `native_cua_run_step` 产生的 step_result。非 dry-run actor output 必须复用 `RUN NATIVE CUA ACTION`。

### 边界说明

- `TuriX-CUA` 保持为外部兼容/参考 bridge
- `native_cua_*` 保持为 Hermes-native rewrite track
- 当前 deliverable 是安全的本地 session / probe / observe / action / plan / step / memory / history / trajectory / audit loop
- 未来真实 VLM Brain / Actor / Planner / Memory 可以接在 `native_cua_plan_task`、`native_cua_observe`、`native_cua_run_step` 与 `native_cua_execute_action` 之上
- 任何桌面 GUI benchmark 结论仍必须来自真实运行与真实权限环境，而不是文档声明

## 6.5 Skills

### `skills_invoke`

请求：

```json
{
  "name": "plan",
  "instruction": "Draft a launch plan"
}
```

响应：

```json
{
  "name": "plan",
  "display_name": "Plan Designer",
  "command": "/plan-designer",
  "source": "codex",
  "path": "/home/user/.codex/skills/plan/SKILL.md",
  "instruction": "Draft a launch plan",
  "rendered_prompt": "skill\tcommand=/plan-designer..."
}
```

约束：

- `name` 可使用 skill name、display name 或 slash-command 风格 selector；后端会归一化为 command key。
- 只读取本地发现的 `SKILL.md` 并渲染 runtime-ready prompt payload。
- 已禁用 skill 必须返回 `validation_error`，不得作为 runtime 候选能力调用。
- 该契约不执行模型、不运行工具、不绕过 sandbox；真实执行、安全审批和审计仍属于 runtime adapter。

### `skills_invoke_into_session`

请求：

```json
{
  "name": "plan",
  "instruction": "Draft a launch plan",
  "session_id": "session-001"
}
```

响应：渲染后的 skill invocation payload 与写入的 session message。该命令把 payload 保存为 `system` role、`skill_invocation` source 的本地 session message，不调用模型或工具。

### `skills_execute_runtime`

请求：

```json
{
  "name": "plan",
  "instruction": "Draft a launch plan",
  "session_id": "session-001",
  "save_to_session": true,
  "dry_run": false,
  "tool_command": "printf",
  "timeout_ms": 1000
}
```

响应：

```json
{
  "invocation": {
    "name": "plan",
    "display_name": "Plan Designer",
    "command": "/plan",
    "source": "codex",
    "path": "/home/user/.codex/skills/plan/SKILL.md",
    "instruction": "Draft a launch plan",
    "rendered_prompt": "skill\tcommand=/plan..."
  },
  "execution_package": {
    "command": "printf",
    "args": ["%s", "{\"kind\":\"skill_runtime_execution_package\"...}"],
    "cwd": null,
    "timeout_ms": 1000,
    "preview": "{\"kind\":\"skill_runtime_execution_package\",\"marker\":\"skill-runtime\"...}"
  },
  "executed": true,
  "dry_run": false,
  "runtime_result": {
    "exit_code": 0,
    "stdout": "{\"kind\":\"skill_runtime_execution_package\"...}",
    "stderr": "",
    "duration_ms": 3,
    "timed_out": false,
    "audit_message": "skill tool executed allowlisted command `printf` within normalized timeout 1000 ms"
  },
  "session_message": null,
  "summary": "executed safe local skill runtime validation through runtime adapter"
}
```

约束：

- 先复用 `skills_invoke` 的 enabled-skill lookup 与 `SKILL.md` payload 渲染；禁用 skill 会在进入 runtime adapter 前拒绝。
- `dry_run` 默认 `true`，只生成 execution package，不执行本地工具。
- 当前仅允许 `printf` / `echo` 本地验证命令，并委托 `runtime_adapter_execute_skill_tool` 的 allowlist、timeout、stdout/stderr capture 与 audit log。
- `save_to_session=true` 时必须传 `session_id`，并把同一 invocation prompt 保存为 `skill_invocation` session context，便于 replay/review。
- 该命令不调用模型、不访问付费 provider、不解释或自动执行 `SKILL.md` 内任意工具指令；它只把 enabled skill payload 接入受限本地 runtime validation 闭环。

### `skills_list_session_invocations`

请求：

```json
{
  "session_id": "session-001",
  "limit": 8
}
```

响应：该 session 中已保存的 skill invocation context messages，按最新优先排序，只返回本地标记为 `skill_invocation` 的消息。该命令用于查看/重放上下文，不调用真实 skill runtime、模型或工具。

## 6.6 Voice

### `voice_list_providers`

请求：无。

响应：本地 Voice provider catalog。当前只包含：

- `local-text-capture`：`kind="stt"`，`interaction_model="manual_text_input"`，`supports_audio_input=false`，`supports_audio_output=false`。
- `local-speak-queue`：`kind="tts"`，`interaction_model="queued_text_output"`，`supports_audio_input=false`，`supports_audio_output=false`。

每个 provider 都会返回 `local_only`、`transport`、`capabilities`、`compatibility_aliases`、`runtime_boundary` 和 `notes`。`runtime_boundary` 必须明确说明当前 workflow 不采集麦克风，也不合成音频。

### `voice_status` / `voice_update_settings` / `voice_set_enabled`

响应或保存 `VoiceSettings`：`enabled`、`stt_provider`、`tts_provider`、`transcription_language`、`preferred_voice`、`auto_speak_transcripts` 和 `updated_at`。Provider ID 必须来自 `voice_list_providers`；历史 `stub-local` 只作为兼容 alias 归一化到当前本地 provider，不作为新 UI 展示项。

### `voice_transcribe`

请求：`text`、可选 `source`、可选 `language`、可选 `auto_queue_for_speech`。

响应：归一化后的 `transcript`、`provider`、`normalized_transcript`、`source`、`language`、`word_count` 和 `queued_for_speech`。该命令只处理用户提供的文本，不读取麦克风、不解析音频文件、不调用 STT provider。

### `voice_speak` / `voice_process_speak_queue`

`voice_speak` 请求：`text`、可选 `voice`、可选 `origin`；响应 queued speech item 的 `id`、`provider`、`text`、`status`、`voice`、`origin` 和 `created_at`。

`voice_process_speak_queue` 请求：可选 `mark_status`（`spoken` 或 `completed`）；响应被处理的队列项和剩余 queued 数。该流程只把文本队列项标记为已处理，不合成音频、不播放声音。

### 兼容命令

`voice_transcribe_stub` 和 `voice_speak_stub` 仅为旧前端/脚本兼容保留，内部转发到 `voice_transcribe` / `voice_speak`，并使用 `compatibility-wrapper` source/origin；新代码应调用非 stub 命令。


## 6.7 Trajectory

### `trajectory_export_dataset`

请求：

```json
{
  "mission_id": "mission-001",
  "include_session_messages": true
}
```

响应：

```json
{
  "schema_version": 1,
  "exported_at": "2026-04-27T00:00:00Z",
  "mission_id": "mission-001",
  "item_count": 4,
  "jsonl": "{\"kind\":\"run\"}\n"
}
```

约束：导出已有本地 runs、execution steps、run events 和可选 session messages 为 JSONL；不训练 RL 模型，也不声称生成研究结论。

## 6.8 Settings

### `settings_get`

响应：

```json
{
  "app": {},
  "runtime": {}
}
```

### `settings_save`

请求：

```json
{
  "app": {},
  "runtime": {
    "provider": "openai",
    "model": "gpt-4o",
    "base_url": "https://api.openai.com/v1",
    "api_key_ref": "OPENAI_API_KEY",
    "native_cua_auto_models": {
      "easy": { "provider": "openai", "model": "gpt-4o-mini", "base_url": "https://api.openai.com/v1" },
      "standard": { "provider": "openai", "model": "gpt-4o", "base_url": "https://api.openai.com/v1" },
      "hard": { "provider": "openrouter", "model": "anthropic/claude-opus-4", "base_url": "https://openrouter.ai/api/v1", "api_key_ref": "OPENROUTER_API_KEY" }
    }
  }
}
```

`runtime` 同时是桌面端模型配置入口；Runtime / Native CUA 页面保存的 provider、model、base_url、api_key_ref 与 `native_cua_auto_models.easy/standard/hard` 会写入这里。Auto 模式按任务难度选择对应档位；非 Auto 或请求显式覆盖时仍可直接指定模型。`api_key_ref` 表示环境变量名，不建议保存原始密钥明文。

响应：

```json
{
  "ok": true
}
```

## 6.9 Runtime

### `runtime_get_status`
### `runtime_start_engine`
### `runtime_stop_engine`
### `runtime_restart_engine`

统一响应：

```json
{
  "engine": {
    "running": true,
    "profile": "default",
    "pid": 12345
  },
  "hermes": {
    "installed": true,
    "running": true
  }
}
```

## 6.10 Mission

### `mission_list`

请求：

```json
{
  "status": ["draft", "researching"],
  "query": "",
  "limit": 50
}
```

### `mission_create`

请求：

```json
{
  "title": "准备客户拜访方案",
  "goal": "基于现有资料生成明日拜访方案",
  "constraints": ["不得对外发送邮件"],
  "successCriteria": ["生成 Markdown 方案", "生成行动清单"],
  "priority": "high"
}
```

### `mission_get`

响应必须是聚合详情：

- mission
- context items
- runs
- artifacts
- council summary
- memory suggestions

## 6.11 Knowledge

### `knowledge_fetch_url_preview`

请求：

```json
{
  "url": "https://example.com/brief"
}
```

响应：URL preview connector 返回 `url`、HTTP `status`、`content_type`、提取出的 `title`、文本 `preview`、`fetched_at` 与 `truncated`。命令只允许 `http/https`，使用 8s timeout 与 128 KiB body cap；它只回填预览，不写入 Mission context，用户仍需通过 `knowledge_import` / Attach URL 确认入库。

### `knowledge_import_folder`

请求：

```json
{
  "mission_id": "mission-001",
  "folder_path": "/abs/path/notes",
  "recursive": true,
  "max_files": 20,
  "title_prefix": "Research"
}
```

响应：`imported_count`、`skipped_count`、导入的 `items` 和 `summary`。命令只读取本地目录下 UTF-8 text-like files（`.md`、`.markdown`、`.txt`、`.json`、`.csv`），拒绝 `http(s)`、非目录、空目录/无支持文件；`max_files` 默认 20，最大 100，单文件沿用 1 MiB 上限。

### `knowledge_import_files`

请求：

```json
{
  "paths": ["/abs/path/a.pdf", "/abs/path/b.md"]
}
```

### `knowledge_search`

请求：

```json
{
  "query": "客户历史偏好",
  "scope": "all",
  "limit": 10
}
```

## 6.12 Memory

### `memory_search`

请求：

```json
{
  "query": "为什么选择方案B",
  "scope": "project",
  "scopeRef": "default"
}
```

## 7. Agent Engine 契约

Agent Engine 仅供 Rust 内部调用，不供前端直连。

它不是外部 `hermes-agent` 运行时的包装层，而是本项目用 Rust 绿地实现的本地核心能力边界。

## 7.1 Health

内部返回结构：

```json
{
  "ok": true,
  "version": "0.1.0",
  "engineReady": true
}
```

## 7.2 Research

内部调用输入：

```json
{
  "missionId": "m_001",
  "goal": "分析客户背景",
  "contextItems": []
}
```

内部调用输出：

```json
{
  "runId": "r_001",
  "status": "queued"
}
```

## 7.3 Scenario

内部调用输入：

```json
{
  "missionId": "m_001",
  "baseline": "直接发送统一报价",
  "options": [
    "先做定制化方案",
    "先预约沟通再发方案"
  ]
}
```

## 7.4 Council

内部调用输入：

```json
{
  "missionId": "m_001",
  "goal": "形成执行计划",
  "roles": ["scout", "analyst", "critic", "planner"]
}
```

## 7.5 Execution

内部调用输入：

```json
{
  "missionId": "m_001",
  "artifacts": [],
  "constraints": []
}
```

内部调用输出：

```json
{
  "runId": "r_exec_001",
  "steps": [
    {
      "title": "生成本地 Markdown 文档",
      "mode": "cli",
      "riskLevel": "low",
      "status": "pending"
    }
  ]
}
```

## 8. 统一错误响应

Rust IPC 与 Agent Engine 统一错误字段：

```json
{
  "code": "validation_error",
  "message": "title 不能为空",
  "retryable": false,
  "details": {}
}
```

## 9. 事件流契约

前端 Timeline 和 Logs 统一消费 `RunEvent`：

```ts
interface RunEvent {
  id: string;
  missionId: string;
  runId: string;
  eventType:
    | "run_created"
    | "run_started"
    | "step_started"
    | "step_completed"
    | "approval_required"
    | "run_failed"
    | "run_completed";
  message: string;
  payload?: Record<string, unknown>;
  createdAt: string;
}
```

## 10. 契约结论

AI 编程工具的实现顺序必须是：

1. 定义类型
2. 定义表
3. 定义 Rust service trait
4. 定义 Tauri command 契约
5. 定义 Agent Engine schema
6. 再连接 UI

如果顺序反过来，后续大概率会进入“页面先跑起来、数据结构反复推倒”的状态。


## 2026-04-28 Added Command Contracts

### `skills_marketplace_list`

请求：`{ manifest_url, limit? }`。`manifest_url` 支持本地路径、`file://`、`http://`、`https://`。响应：`SkillMarketplaceCatalog`，包含 `schema_version`、`marketplace_id`、`manifest_url`、`skills[]`。每个 entry 包含 `name`、`title`、`description`、`source_url?`、`content?`、`tags[]`。只读取 manifest，不安装、不执行 skill。

### `skills_marketplace_install`

请求：`{ manifest_url, name, force?, target_remote_user_id? }`。命令先加载 marketplace manifest，再读取 inline `content` 或 `source_url` 的 `SKILL.md` 内容，复用本地 `skills_install` 校验和写入流程。`target_remote_user_id` 会 trim/empty-to-null 后写入 install result 和本地 install history row，仅作为 future remote user routing metadata。响应包含 marketplace entry、installed local skill detail 和可选 `target_remote_user_id`。该命令不执行 skill，不调用模型，不表示 remote marketplace account activity。

### `skills_marketplace_list_install_history`

请求：`{ limit?, marketplace_id?, skill_name?, target_remote_user_id? }`。默认从本地 `app_settings` 返回最新 marketplace install history；`marketplace_id` 精确匹配 marketplace，`skill_name` 可匹配 source skill name 或 installed skill name。`target_remote_user_id` 会 trim/empty-to-null，存在时只返回该 future remote user routing metadata 匹配的本地 history row。该过滤只是本地历史检索，不表示远端 marketplace 状态变更、远端账号活动或远端投递。

### `runtime_adapter_run_gui_automation`

请求：`{ steps, dry_run?, confirmation_phrase?, stop_on_error?, target_remote_user_id? }`，其中每个 step 是 `{ label?, executor, args[] }`。默认 dry-run；非 dry-run 必须提供 `RUN DESKTOP ACTION`，且每一步仍复用 desktop action executor allowlist。`target_remote_user_id` 会 trim/empty-to-null 后写入响应和 runtime adapter audit event，仅作为 future remote user routing metadata。响应包含 planned commands、per-step desktop action response、completed count、`target_remote_user_id` 和 audit message。

### `simulation_run_external_saas`

请求：`{ mission_id, provider, endpoint_url?, input_json?, dry_run?, confirmation_phrase?, target_remote_user_id?, timeout_ms? }`。`provider=local_echo` 离线执行；`provider=http_json` dry-run 只预览，非 dry-run 必须提供 `RUN EXTERNAL SAAS SIMULATION` 和 http/https endpoint。`target_remote_user_id` 会 trim/empty-to-null 后写入 request preview、响应和 persisted run event payload。响应包含 request preview、actual response body/status、network_invocation 标记、`target_remote_user_id` 和 persisted run event id。

### `simulation_list_external_saas_runs`

请求：`{ mission_id?, limit?, target_remote_user_id? }`。返回本地 persisted External SaaS run history，包含 `external_saas_simulation_completed` 与 `external_saas_simulation_previewed` 事件。`target_remote_user_id` 会 trim/empty-to-null，存在时在返回 `limit` 前按 future remote user routing metadata 过滤本地 history payload；该过滤不表示远端 SaaS 执行、远端用户接收或远端账号活动。

### `simulation_run_high_fidelity_sandbox`

请求复用 local sandbox 的 mission/baseline/options/agents/rounds，并额外接受 `variables[]` 与 `target_remote_user_id?`。`target_remote_user_id` 会 trim/empty-to-null 后写入响应和 persisted run event payload。响应包含 base deterministic sandbox run、`target_remote_user_id` 与 `world`：entities、variables、timeline、event_graph、option_metric_heatmap。命令写入 completed simulation run 和 `high_fidelity_sandbox_completed` event。

### `simulation_list_high_fidelity_sandbox_runs`

请求：`{ mission_id?, limit?, target_remote_user_id? }`。返回本地 persisted High-Fidelity Sandbox run history。`target_remote_user_id` 会 trim/empty-to-null，存在时在返回 `limit` 前按 future remote user routing metadata 过滤本地 history payload。该能力仍是 deterministic local world model history 检索，不声明 3D/OSWorld/SOTA 沙盘能力，也不表示远端投递。

### `trajectory_run_local_rl_training`

请求：`{ jsonl, epochs?, alpha?, gamma?, job_name?, target_remote_user_id? }`。命令解析 trajectory JSONL，按 `trajectory_id` 构造 episode，并运行本地 tabular TD/Q-learning baseline update。`target_remote_user_id` 会 trim/empty-to-null 后写入响应、artifact JSON 和 persisted job history。响应包含 job id、valid/invalid counts、episode count、policy table、artifact JSON、`target_remote_user_id` 和 summary，并将 job artifact 持久化到 `app_settings`。该命令不训练大模型、不声明 benchmark 质量。

### `trajectory_list_local_rl_training_jobs`

请求：`{ limit?, target_remote_user_id? }`。默认从本地 `app_settings` 返回最新 local RL training jobs；`limit` 仍受本地 cap 限制。`target_remote_user_id` 会 trim/empty-to-null，存在时只返回该 future remote user routing metadata 匹配的 job history。该过滤只是本地历史检索，不表示远端用户已接收 artifact，也不表示 remote RLHF。

## 2026-04-29 Agent Exchange Remote User Contracts

Agent Exchange 是 future remote users / remote-account routing 的本地预留层，不是实时远端消息服务。所有数据仍写入本机 `app_settings`，bundle 通过 JSON 或共享文件路径手工传递。

### `agent_exchange_list_remote_users`

请求：`{ query?, status?, limit? }`。`status` 支持 `active` / `paused` / `blocked`。`query` 会大小写不敏感地匹配 `user_id`、`display_name`、`default_agent_id`、`transport_label` 和 `route_hint`。响应为按 `updated_at` 新到旧排序的 `AgentExchangeRemoteUser[]`。

### `agent_exchange_upsert_remote_user`

请求：`{ user_id, display_name, default_agent_id, transport_label?, route_hint?, status? }`。`user_id`、`display_name` 和 `default_agent_id` 必填且 trim；`status` 缺省为 `active`。更新已有 `user_id` 时保留 `created_at`，刷新 `updated_at`。

### `agent_exchange_delete_remote_user`

请求：`{ user_id }`。只删除本地 remote user profile，不删除已有 messages；历史 messages 上的 `remote_user_id` 保持为审计线索。

### Agent Exchange bundle / filter compatibility

`AgentExchangeState`、`AgentExchangeBundle` 和本地 mailbox 新增 `remote_users`。旧 mailbox / bundle 缺少该字段时必须按空数组读取。`agent_exchange_list_messages` 与 `agent_exchange_export_bundle` 新增 `remote_user_id` 精确过滤；bundle export 会包含导出消息引用到的 remote user profiles，也会在显式指定 `remote_user_id` 时包含该 profile，即使当前 scope 下没有消息。bundle import 会合并 remote user profiles，同一 `user_id` 按 `updated_at` 新者覆盖，消息去重规则不变。前端可把当前 scoped bundle 下载为 `agent-exchange-bundle.json`，该文件仍是本地 out-of-band handoff artifact，不是远端投递凭证。

### Evidence export metadata

`target_remote_user_id` 现在贯通以下本地记录面：Skills marketplace install request/result/history、Runtime GUI automation request/response/audit event、Simulation External SaaS run request/result/history payload、Simulation High-Fidelity sandbox request/result/history payload、Runtime local RL training request/result/job artifact，以及前端导出的 marketplace audit / runtime adapter audit handoff / simulation evidence / local RL artifact envelope。Marketplace install history、Runtime adapter audit list/export、Simulation External SaaS history、Simulation High-Fidelity Sandbox history 和 Local RL job history 均可按该字段做本地过滤。当前 UI 若从本地 Agent Exchange Future Remote Users 目录选择目标，导出的 marketplace audit、runtime adapter audit handoff、simulation evidence、local RL artifact envelope 还会附带 `target_remote_user_profile` snapshot（`user_id`、display name、默认 agent、transport label、route hint、status、timestamps），便于未来接收方理解本地路由上下文。该字段和 profile snapshot 都只是 future remote user routing metadata，不表示远端账号活动、远端 marketplace 状态变更、远端投递成功或远端 RLHF 基础设施。

Runtime adapter audit handoff envelope 可下载为 `runtime-adapter-audit-handoff.json`。Hermes Native CUA audit export 与 TuriX bridge audit export 也可从 Runtime 页面下载为本地 audit payload（`json` 或 `jsonl`），但它们仍是 raw local review payload，不包含 Agent Exchange `target_remote_user_profile` envelope，不表示 remote sync、remote GUI execution、OSWorld/SOTA benchmark 结果或跨用户投递完成。
