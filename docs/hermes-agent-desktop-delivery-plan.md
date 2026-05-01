# Hermes Operator 交付路线图

## 1. 文档目标

本文档把产品拆成可执行的阶段性里程碑，给 AI 编程工具一个稳定的开发顺序。每个阶段都给出：

- 目标
- 依赖
- 主要文件范围
- 验收标准
- 不做什么

## 2. 总体原则

- 一阶段只解决一类问题
- 所有核心代码绿地重写
- 先基础设施，后复杂工作流
- 先稳定契约，后真实能力接入
- 先本地闭环，后外部整合
- 桌面软件操作能力必须比知识增强能力更早落地

## 3. 里程碑总览

| Phase | 名称 | 目标 |
| --- | --- | --- |
| 1 | Foundation | 把当前桌面壳升级为可持续开发的工程骨架 |
| 2 | Mission + Operate Baseline | 建立 Mission 主工作流与桌面操作主通路 |
| 3 | Knowledge + Memory | 建立资料导入、检索、召回 |
| 4 | Council + Advanced Execution | 建立计划、审议、审批、执行 |
| 5 | Scenario + Growth | 建立推演和成长闭环 |

## 4. Phase 1: Foundation

### 目标

把 `hermes-desktop` 从 demo 壳升级成真正的桌面应用基础设施。

### 主要工作

- 前端从单 HTML 升级到 React + TypeScript + Vite
- Rust 建立 command/service/storage 边界
- 建立 SQLite
- 建立 AppState
- 建立 Rust Agent Core 管理边界；首版可用稳定契约和保守本地响应，后续再接入复杂外部能力
- 建立 Home 与 Settings 两个基础页

### 主要文件

- `hermes-desktop/tauri.conf.json`
- `hermes-desktop/Cargo.toml`
- `hermes-desktop/src/commands/*`
- `hermes-desktop/src/backend/app_state.rs`
- `hermes-desktop/src/backend/storage/*`
- `hermes-desktop/ui/*`

### 验收标准

- 应用可以启动
- React UI 正常加载
- `app_get_bootstrap` 可用
- `settings_get/settings_save` 可用
- SQLite 能创建
- Agent Core 状态可见

### 不做什么

- 不做 Mission
- 不做 Knowledge 搜索
- 不做 Council
- Phase 1 不做 GUI automation；2026-04-28 后已通过 Runtime Adapter GUI macro 以 allowlist + confirmation gate 的方式补齐

## 5. Phase 2: Mission + Operate Baseline

### 目标

建立 Mission 作为一等领域对象，并把桌面软件操作做成独立主通路。

### 主要工作

- Mission 数据表
- Mission CRUD 命令
- Mission 列表页
- Mission 详情页
- Operate 页面
- 基础操作步骤模型
- Overview / Context / Runs / Artifacts tabs
- Timeline 视图
- Bootstrap 恢复 pinned 优先、否则最近活跃的未完成 Mission

### 主要文件

- `src/backend/domain/mission.rs`
- `src/backend/domain/execution.rs`
- `src/backend/services/mission_service.rs`
- `src/backend/services/execution_service.rs`
- `src/backend/storage/repositories/mission_repo.rs`
- `src/commands/mission.rs`
- `ui/src/features/missions/*`
- `ui/src/features/execution/*`
- `ui/src/routes/missions/*`

### 验收标准

- Mission 可创建、编辑、归档
- Mission 列表和详情可用
- Operate 页面可展示一个 Mission 的当前操作状态
- 能附加 note/context，并持久化到 Mission context
- 能显示 run 空态和 timeline 空态

### 不做什么

- 不接入真实研究引擎
- 不接入记忆系统

## 6. Phase 3: Knowledge + Memory

### 目标

建立资料进入系统后的索引和召回能力。

### 主要工作

- 文件导入
- 本地文件夹导入：UTF-8 `.md` / `.markdown` / `.txt` / `.json` / `.csv`，支持 recursive、max files、title prefix，并复用 Mission context + KnowledgeSource/chunk 入库
- URL fetch preview connector：`http/https` only、timeout、body cap、HTML title/summary extraction，用户确认后再 Attach URL 入库
- KnowledgeSource / Chunk 数据表
- 文本提取和基础 chunking；当前本地实现以轻量规则切片为主，不声明完整语义 RAG
- 搜索页
- MemoryRecord 数据表
- Mission 内记忆推荐面板

### 主要文件

- `src/backend/domain/knowledge.rs`
- `src/backend/domain/memory.rs`
- `src/backend/services/knowledge_service.rs`
- `src/backend/services/memory_service.rs`
- `src/commands/knowledge.rs`
- `src/commands/memory.rs`
- `ui/src/features/knowledge/*`
- `ui/src/features/memory/*`

### 验收标准

- 文件能导入
- 搜索能返回结果
- 结果能附加到 Mission
- 能从 Memory 面板看到历史文本记录

### 不做什么

- 不做完整 Onyx 连接器
- 不做 MemPalace 深度集成

## 7. Phase 4: Council + Advanced Execution

### 目标

建立从计划到执行的中间治理层。

### 主要工作

- CouncilStep 数据表
- ExecutionStep 数据表
- 计划生成 run
- Council Board
- Approval Sheet
- CLI 执行 adapter
- Browser/desktop handoff adapter 边界；CLI 已有本地 runner，桌面动作保持 probe/dry-run/allowlisted explicit execution，不声明任意 GUI 自动化

### 主要文件

- `src/backend/domain/council.rs`
- `src/backend/domain/execution.rs`
- `src/backend/services/council_service.rs`
- `src/backend/services/execution_service.rs`
- `src/backend/adapters/shell_runner.rs`
- `src/commands/runtime.rs`
- `ui/src/features/council/*`
- `ui/src/features/execution/*`

### 验收标准

- Mission 可以生成计划
- Council 看板能展示步骤
- 高风险执行能审批
- 执行日志能写入 Runs
- Operate Step Inspector 能写入用户批注，并把运行中步骤暂停到恢复队列
- Desktop mode step 能生成 desktop handoff checklist/runtime prompt，记录 prepared/reviewed timeline events，并进入 desktop handoff queue，但不伪装 GUI 自动化
- Mission 能导出本地 trajectory JSONL，聚合 runs、execution steps、run events 和可选 session messages

### 不做什么

- 桌面 GUI 自动化先以 executor probe、dry-run action 和 allowlisted explicit execution 交付，不做隐式任意 GUI 操作

## 8. Phase 5: Scenario + Growth

### 目标

补齐创新差异化：先把 Mission 级 scenario run 记录、baseline / option cards 比较和 recommendation 兜底做成真实可用的首个增量，再把已保存 runs 的 mission-level comparison matrix 和 path evolution 补出来，然后用 Skill Evolution Inbox 承接会话证据并保持保守归因。

### 主要工作

- ScenarioRun 数据表
- Mission 级 scenario run 输入与比较 UI
- baseline / option cards 输入与历史回放
- 已保存 scenario runs 的 mission-level comparison matrix
- 已保存 scenario runs 的 path evolution 视图
- 结构化 option card 字段：`assumptions` / `expected_benefits` / `risks` / `confidence`
- recommendation 输出与兜底
- Mission 级 scenario run 历史回放
- Growth/Playbook 建议
- Mission 完成后自动复盘
- Skill Evolution Inbox：把会话证据、工具错误、人工观察沉淀为可评审的 skill 改进候选
- 自动候选生成：从 failed runs、failed steps 和 failed run events 中提取可复用失败模式，作为 Growth 的第一批候选输入
- 候选归因：当失败模式与本地已发现 skill 明显匹配时，优先生成 `refine` 候选而不是一律 `create`

### 主要文件

- `src/backend/domain/scenario.rs`
- `src/backend/services/scenario_service.rs`
- `src/commands/skill_evolution.rs`
- `ui/src/features/simulation/*`
- `ui/src/features/skills/*`

### 验收标准

- 一个 Mission 能保存 baseline 和 2 到 3 个 option card
- option card 能保留 `assumptions` / `expected_benefits` / `risks` / `confidence`
- 能输出 recommendation 和 recommendation rationale，并说明来源是手动输入还是按 option card 归因
- 能通过 impact / uncertainty 滑块注入变量权重
- 能配置 scenario handoff route：Council + Execution、Council only、Execution only、Timeline only
- 高风险 Execution handoff 会进入 awaiting approval 状态，低/中风险进入 pending review
- 能保存和复用 scenario handoff policy templates
- 能在 Simulation 页面调整 scoring formula，并把生成的 option scores 随 scenario run 持久化
- 能保存和复用本地 scoring formula templates
- 能导出/导入 handoff policy + scoring formula template bundle，用于团队手动共享
- 能导出本地 template bundle audit log 为只读 JSON，用于手动审计归档
- Template bundle export/import 会记录本地 audit log，并在 Simulation 页面展示最近记录
- Template bundle import 支持 preflight，导入前展示新增、覆盖、未变化数量和冲突列表
- 能回看 scenario run 历史
- 能查看同一 Mission 的 comparison matrix 和 path evolution
- Simulation 页面能展示 overview、run type mix、status mix 和 recent runs
- 能生成至少一种 playbook suggestion
- Skills 页面能展示、创建、接受和拒绝本地 skill evolution candidates
- 候选 skill 改进必须先进入验证/评审状态，不直接自动改写 `SKILL.md`
- 能从现有运行轨迹一键生成去重后的候选 skill 改进
- 自动生成阶段能对强匹配的现有 skill 做保守归因，生成 `refine` 候选
- 能对启用的本地 `SKILL.md` 渲染 runtime invocation payload，并拒绝禁用 skill 的 invocation
- 能把 skill invocation payload 保存为本地 session message，作为后续 runtime 上下文输入
- 能按 session 查看已保存的 skill invocation context，但不执行真实模型/工具 runtime

### 当前本地闭环

- Scoring formula templates 已可本地复用、bundle 手动共享，并可进入 Local Multi-Agent Sandbox 做 deterministic 多 agent/多轮评分。
- Handoff policy templates 已可本地复用、通过 bundle 手动共享并记录本地 audit log；Local Team Governance 提供 RBAC、audit、bundle import/export 和共享目录 JSON sync。
- Skill invocation payload 已可本地渲染、保存到 session，并可通过 Runtime Adapter 执行受限 allowlisted 本地 skill-tool command。
- Scenario 结果可进入治理、执行准备队列、desktop handoff 和 Local Multi-Agent Sandbox；桌面动作通过 Runtime Adapter probe/dry-run/allowlisted explicit command 执行，避免隐式任意 GUI 操作。

## 9. 每阶段通用验收清单

每个阶段都必须满足：

- 命令契约清晰
- 至少一条 happy path
- 空态
- 加载态
- 错误态
- 单元测试
- 至少一条集成测试
- 文档更新

## 10. 推荐开发节奏

### 单阶段内部顺序

1. 定义 domain model
2. 建表 / 迁移
3. 实现 repository
4. 实现 service
5. 暴露 command / API
6. 写前端 invoke client
7. 写 store
8. 写页面
9. 补测试
10. 补文档

## 11. AI 编程工具执行提示

- 不要跨 Phase 同时推进多个大模块
- 优先保持接口稳定
- Rust 自研 Agent Core 优先保持可替换契约；外部模型、GUI、SaaS provider 先通过受控 adapter 接入，不用假成功掩盖边界
- 页面可以先连真实契约的空态/受限本地响应；示例数据必须标注为示例，不得当作已完成能力
- 真实执行器与桌面操作引擎必须逐步接入 probe/dry-run/allowlist/confirmation/audit，不得从示例按钮直接宣称完整自动化

## 12. 路线图结论

这个项目的关键，不是“先做最酷的功能”，而是：

**先把这个桌面 Agent 平台的骨架做对。**

只有骨架对了，后面的 Knowledge、Council、Scenario 和 Growth 才不会互相打架。


## Current Local Integration Closure

- Local Multi-Agent Sandbox：本地 deterministic agents/rounds/options，写入 completed simulation run 和 audit event，并支持可选中 run 的完整历史回放。
- Local Team Governance：SQLite-backed members/roles/audit/bundle sync，支持 RBAC check、本地 JSON 文件同步、前端 audit filter/export preview，以及后端 RBAC 审计导出。
- Runtime Adapters：allowlisted skill-tool process execution with enforced timeout、desktop executor probe/dry-run action、非 dry-run UI confirmation gate、用户自定义 trajectory JSONL summary、本地持久化 audit list/export，以及 `cat` 文件路径/大小安全限制。
- Trajectory review：Mission trajectory JSONL 导出后可本地解析 kind/source/reward_hint/invalid 摘要、最近行预览和剪贴板复制，用于 dataset review / replay preparation。Simulation sandbox replay 也可导出/复制完整 JSON。


## 2026-04-28 Closure Update

- Remote skill marketplace、GUI automation macro、external SaaS simulation adapter、high-fidelity local sandbox、local tabular RL training 已进入桌面端闭环。
- 仍需外部条件的部分：真实 SaaS 账号/endpoint/凭证、OS 图形会话权限、非 allowlisted GUI executor、远端团队云服务、大模型/RLHF 训练集群。
- 验收重点：所有非 dry-run GUI/HTTP 外呼路径必须继续保留 backend confirmation phrase、allowlist 或 provider validation，不能用 UI 文案替代后端拒绝。

## 2026-04-29 Verification Hardening Update

- UI verification now includes `npm test`, backed by Node native tests under `hermes-desktop/ui/tests/*.test.mjs`.
- Frontend contract coverage guards the newly added Tauri wrappers for remote skill marketplace, GUI automation macro, external SaaS simulation, high-fidelity sandbox, and local RL training against Rust `invoke_handler` registration drift.
- Backend regression coverage now includes marketplace inline-content install, `http_json` SaaS dry-run preview without network invocation, and GUI automation macro over-limit rejection/audit.
- Standard pre-handoff verification command set: `cargo test`, `cargo clippy --lib -- -D warnings`, `cargo clippy --all-targets -- -D warnings`, `npm test`, `npm run typecheck`, and `npm run build`.

