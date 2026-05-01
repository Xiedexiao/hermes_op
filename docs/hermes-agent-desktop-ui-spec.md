# Hermes Operator 界面与交互规格

## 1. 文档目标

本文档定义桌面端 IA、导航、页面结构、关键组件、状态与交互规则，目标是让 AI 编程工具可以直接开始生成页面、组件和状态管理代码。

## 2. 信息架构

主导航固定为：

1. Home
2. Missions
3. Operate
4. Knowledge
5. Simulation
6. Skills
7. Voice
8. Settings

全局辅助入口：

- 全局搜索
- 通知中心
- Runtime 状态
- 当前活跃 Mission 指示器

## 3. 全局布局

```text
┌─────────────────────────────────────────────────────────────┐
│ Top Bar: search / runtime / notifications / theme / user   │
├───────────────┬──────────────────────────────┬──────────────┤
│ Sidebar       │ Main Content                 │ Right Panel  │
│ - Home        │ route content                │ contextual   │
│ - Missions    │                              │ drawer/panel │
│ - Operate     │                              │              │
│ - Knowledge   │                              │              │
│ - Simulation  │                              │              │
│ - Skills      │                              │              │
│ - Settings    │                              │              │
└───────────────┴──────────────────────────────┴──────────────┘
```

### 布局规则

- Sidebar 固定宽度
- Main Content 是主工作区
- Right Panel 按页面上下文切换
- 小屏宽度下 Right Panel 变为 Drawer

## 4. 全局组件

## 4.1 AppShell

职责：

- 布局骨架
- 路由承载
- runtime 状态轮询
- 通知展示

## 4.2 GlobalSearch

职责：

- 搜 Mission
- 搜 Knowledge
- 搜 Memory

首版可先做命令面板式弹层。

## 4.3 RuntimeBadge

展示：

- Agent Engine 状态
- 应用运行状态
- 当前是否有运行中的 Run

颜色规则：

- 绿色：正常
- 黄色：运行中但部分依赖异常
- 红色：不可用

## 4.4 NotificationCenter

展示：

- 待审批执行项
- Run 失败
- Run 完成

## 5. 页面规格

## 5.1 Home

### 页面目标

作为应用进入后的总览页，展示当前系统状态、活跃任务和待处理事项。

### 页面模块

1. Runtime Health Card
2. Quick Actions
3. Active Missions
4. Pending Approvals
5. Recent Artifacts
6. Recent Memories

### 数据依赖

- `app_get_bootstrap`，包含 `active_mission` 恢复入口
- `mission_list`

### 交互

- 点击 Active Mission / 恢复未完成 Mission 进入详情
- 点击 Pending Approval 进入执行计划面板
- Quick Action 支持：
  - 新建 Mission
  - 导入文件
  - 打开 Settings

### 空态

- 没有 Mission 时，显示新建引导

### 错误态

- bootstrap 失败时显示重试按钮

## 5.2 Missions List

### 页面目标

作为主任务入口，管理所有 Mission。

### 布局

左侧：

- 筛选器
- 搜索框
- Mission 列表

中间：

- 当前选中 Mission 概览

### 列表项字段

- 标题
- 状态
- 优先级
- 最后活动时间
- 是否 pinned

### 交互

- 新建 Mission
- 状态筛选
- 搜索
- pin/unpin
- archive

### 空态

- 首次使用引导创建第一个 Mission

## 5.3 Mission Detail

### 页面目标

承载一个 Mission 的全生命周期工作空间。

### 顶部区域

- 标题
- 状态标签
- 优先级
- 主操作按钮
  - 开始研究
  - 开始推演
  - 生成计划
  - 执行
  - 暂停

### 主标签页

1. `Overview`
2. `Context`
3. `Operate`
4. `Council`
5. `Runs`
6. `Artifacts`
7. `Memory`

### Overview

展示：

- 目标
- 约束
- 成功标准
- 关键摘要
- 最近一次运行情况

### Context

展示：

- 已附加文件
- URL
- Notes
- 记忆建议
- 研究引用

交互：

- 添加 note
- 导入文件
- 关联历史记忆

### Operate

展示：

- 当前目标应用
- 当前操作计划
- 当前执行步骤
- 最近一次操作结果

交互：

- 开始执行
- 暂停
- 重试
- 跳过
- 在 Step Inspector 写入用户批注；批注会进入 execution input payload，运行中步骤会暂停等待复核

### Council

展示：

- 角色列
- 当前步骤状态
- 被打回的原因

### Runs

展示：

- Run 列表
- Timeline
- 事件流

### Artifacts

展示：

- 生成的文档、图片、计划、文件

交互：

- 打开
- 复制路径
- 关联到上下文

### Memory

展示：

- 与当前 Mission 相关的历史记录
- 系统推荐召回

## 5.4 Operate

### 页面目标

把桌面软件操作能力做成独立的一等页面，而不是隐藏在 Mission 日志中。

### 页面模块

1. Current Target App
2. Current Operation Queue
3. Step Inspector
4. Approval Sheet Entry
5. Recent Run Evidence

### 交互

- 选择目标 Mission
- 查看当前操作目标
- 手动确认/跳过步骤
- 打开最近证据截图或日志
- 在 Step Inspector 为执行步骤写入用户批注
- 对 `desktop` mode step 生成 desktop handoff checklist 与 runtime prompt，不直接执行 GUI 自动化
- Desktop handoff package 支持复制 prompt、导出 JSON/Markdown，包含 step/run/mission/risk/status/input/checklist/review guidance
- 对已 prepared 的 desktop handoff 记录人工 reviewed 事件和 review note
- 展示 Desktop Handoff Queue，区分 needs handoff、prepared 与 reviewed 状态
- 在 Timeline 区域导出 Mission trajectory JSONL，用于本地研究/回放数据准备

### 空态

- 没有活动 Mission 时显示引导

## 5.5 Knowledge

### 页面目标

导入、管理、搜索知识源。

### 页面模块

1. Import Area
2. Source List
3. Search Box
4. Search Results
5. Evidence Preview

### 交互

- 导入文件/文件夹
  - 文件夹导入是本地-only connector：用户输入本机 folder path、recursive、max files 和 title prefix；仅导入 UTF-8 text-like files，不访问远端。
- URL 模式可先运行 Fetch URL preview：抓取网页 title/summary 回填表单，但不自动写库；用户确认 Attach URL 后才附加到 Mission。
- 搜索
- 查看原文片段
- 附加到 Mission

### 列表字段

- 标题
- 类型
- 索引状态
- chunk 数量
- 更新时间

## 5.6 Simulation

### 页面目标

对一个 Mission 做 mission-scoped scenario run 记录、比较和回看。当前页面是文本优先的 scenario editor，不是完整的外部变量模拟器；已保存 runs 会组织成 comparison matrix 和 path evolution 视图。

### 页面模块

1. Mission Selector
2. Baseline Input
3. Option Input
4. Option Card Grid
5. Recommendation Input / Result
6. Scenario History
7. Comparison Matrix
8. Path Evolution View
9. Overview Metrics
10. Run Type Mix
11. Status Mix
12. Recent Mission Runs

### 交互

- 选择 Mission
- 输入 baseline
- 逐行输入 option，再由后端结构化为 option cards
- 保存 scenario run
- 查看该 Mission 的历史 runs
- 查看已保存 runs 的 comparison matrix
- 查看 path evolution
- 查看 recommendation 的当前结果与 explanation card
- 调整 impact / uncertainty 滑块并观察 option scoring 变化
- 保存后按 handoff policy 自动送入 Council / Execution review handoff

### 空态

- 未选择 Mission 时显示引导

### 当前已落地

- 页面已经显示 Mission 选择器、baseline 输入、option 输入和 recommendation reason 输入
- 页面已经提供独立变量注入控件，并用 impact / uncertainty 滑块与 scoring formula 控件驱动 option scoring
- 保存后会刷新历史 scenario runs，并展示 option cards、confidence、recommendation 和 explanation card
- 页面下半区已经显示 overview metrics、run type mix、status mix 和 recent mission runs
- 页面已经显示 comparison matrix 和 path evolution
- 保存 scenario run 后会按 handoff policy 创建 Council review、Execution review、两者同时创建或只记录 timeline
- Handoff policy 支持选择内置模板、保存当前策略为模板、跨 Mission 复用
- Scoring formula 支持调整 base score、impact multiplier 和 uncertainty penalty
- Scoring formula 支持选择内置模板、保存当前公式为模板、跨 Mission 本地复用
- 页面支持导出/导入 Template Sharing Bundle，把 handoff policy 与 scoring formula templates 作为 JSON 手动共享
- Template Sharing Bundle 面板显示最近本地 export/import audit log
- 可导出本地 bundle audit log 为只读 JSON，供手动审计归档
- 导入前可运行 preflight，预览新增、覆盖、未变化数量和冲突列表

### 非本地闭环能力

- 已接入内置 Local Multi-Agent Sandbox，可运行 deterministic agents/rounds/options 并写入 completed simulation run 与 audit event；页面展示最近 sandbox run history，并可选中历史 run 查看 provider 状态、agents、turn-by-turn replay、scoreboard 与 recommendation，还可复制/导出完整 replay JSON。
- Template Sharing Bundle 已支持手动团队共享、本地 audit log、audit JSON export；Runtime 页提供本地 Team Governance RBAC/audit/bundle sync 面板。
- 外部仿真 SaaS 已以 `local_echo` / `http_json` provider adapter 接入；高保真沙盘已以本地 world model 接入。远端团队服务仍是可选外部集成；真实 HTTP SaaS 调用仍依赖 endpoint/凭证/网络权限，不伪造结果。

## 5.7 Skills

### 页面目标

管理本地发现的 `SKILL.md`，让技能从“可见/可启用”推进到“可渲染为 runtime invocation payload，并可进入受限本地 runtime validation”。

### 当前已落地

- Skills 页面展示本地发现的 skills、来源路径和启用状态
- 支持启用/停用 skill，禁用状态不会生成可调用 payload
- 支持为启用 skill 输入本次 invocation instruction
- 支持调用 `skills_invoke`，读取对应 `SKILL.md` 并渲染 slash-command 风格 runtime payload
- 支持把 invocation payload 保存到最近 session，作为 `skill_invocation` 来源的 system message
- 支持查看选中 session 中已保存的 skill invocation context，用于本地重放/复核
- 支持调用 `skills_execute_runtime` 生成 dry-run execution package，或通过 Runtime Adapter allowlisted `printf` 本地执行 validation 并展示 exit/stdout/stderr/audit 结果
- Skill invocation result、runtime package 和 session replay payload 支持 copy/download JSON/Markdown 或页面内查看；不会调用付费模型 provider
- Skill Evolution Inbox 可展示、创建、接受和拒绝本地 skill 改进候选
- 自动候选生成可从 failed runs、failed steps 和 failed events 中提取失败模式，并对强匹配 skill 生成 `refine` 候选

### 非本地闭环能力

- 已完成本地 skill invocation payload adapter；payload 可保存为 session context，也可进入受限 runtime validation package。
- Runtime Adapter 已能执行受限 allowlisted 本地 skill-tool command，并记录审计；Skills 页面现在只开放 `printf`/`echo` 验证闭环，这不等同于任意 skill payload 自动送入真实模型/工具 runtime。
- Remote Skill Marketplace 已接入 manifest list/install；更完整的工具 sandbox、权限隔离和远端团队同步仍是扩展边界。

## 5.8 Voice

### 页面目标

提供本地 text-only voice 工作流，把手动 transcript 录入、待播报文本队列和历史审计做成可用闭环，同时明确它不是麦克风录音、音频 STT 或音频 TTS。

### 当前已落地

- Voice 页面可以启用/停用本地 voice workflow。
- `voice_list_providers` 返回 provider catalog；当前只有 `local-text-capture` 与 `local-speak-queue`，二者都标记 `local_only=true`、`supports_audio_input=false`、`supports_audio_output=false`。
- STT/TTS provider 设置改为从 catalog 选择，避免用户误以为可以输入任意真实音频 provider。
- 页面顶部和 provider cards 明确展示 runtime boundary：不会采集麦克风，也不会合成音频。
- Local Transcript 会把用户输入文本作为 transcript 持久化到 SQLite history，可记录 source、language、word count。
- Speak Queue 会把待播报文本持久化为 queued speech item，并支持手动处理下一个队列项为 spoken。
- 旧的 `voice_transcribe_stub` / `voice_speak_stub` Tauri 命令只作为兼容 wrapper 保留；新 UI 不再调用这些 wrapper。

### 仍未完成

- 未接入真实麦克风采集、音频文件解码、远端 STT provider 或本地音频 TTS provider。
- 未实现消息平台语音附件收发；当前只处理桌面端手动文本输入与本地队列。

## 5.9 Settings

### 页面目标

管理应用、运行时与工作区设置。

### 子分组

1. App
2. Runtime
3. Workspace
4. Diagnostics

### App

- 主题
- 语言
- 开机启动
- 默认工作目录

### Runtime

- Provider
- Model
- Base URL
- API Key
- Engine Profile
- Agent Engine 启停

### Workspace

- 默认知识集
- 默认审批等级
- 是否启用长期记忆

### Diagnostics

- 环境检测结果
- 日志路径
- 导出诊断包

## 6. 关键组件清单

| 组件 | 用途 |
| --- | --- |
| `SidebarNav` | 主导航 |
| `TopBar` | 全局操作区 |
| `StatusBadge` | runtime/mission 状态展示 |
| `MetricCard` | 首页指标卡 |
| `MissionCard` | Mission 列表项 |
| `MissionTimeline` | 时间线 |
| `ContextItemList` | 上下文列表 |
| `ArtifactList` | 产物列表 |
| `CouncilBoard` | 多角色编排看板 |
| `RunLogPanel` | 结构化事件流 |
| `ApprovalSheet` | 高风险执行审批 |
| `SearchResultCard` | Knowledge 搜索结果 |
| `MemoryRecallCard` | 记忆召回项 |
| `ScenarioRunnerForm` | Scenario baseline / option 输入 |
| `ScenarioOptionCardGrid` | 结构化方案比较 |
| `ScenarioRunList` | Mission 级 scenario 历史 |

## 7. 前端状态设计

前端 store 建议拆分：

- `appStore`
- `runtimeStore`
- `missionStore`
- `knowledgeStore`
- `memoryStore`
- `uiStore`

### `appStore`

- 当前主题
- 当前语言
- 全局初始化状态

### `runtimeStore`

- Agent Engine 状态
- 应用运行状态
- 最后检测时间

### `missionStore`

- 列表
- 当前选中 ID
- filter
- loading 状态

### `knowledgeStore`

- source 列表
- 搜索 query
- 搜索结果

### `uiStore`

- 当前右侧面板
- 通知弹层开关
- 全局搜索开关

## 8. 页面路由

建议：

- `/`
- `/missions`
- `/missions/:missionId`
- `/knowledge`
- `/simulation`
- `/skills`
- `/settings`

## 9. 关键交互流程

## 9.1 新建 Mission

1. Home 点击“新建 Mission”
2. 打开弹层
3. 输入标题、目标、约束、成功标准
4. 创建成功后自动跳转到 `/missions/:id`

## 9.2 导入资料并附加到 Mission

1. 在 Mission Detail 的 Context Tab 点击“导入文件”
2. 调用 `knowledge_import_files`
3. 导入成功后显示 source
4. 可一键附加为 context item

## 9.3 生成计划

1. 在 Mission Detail 点击“生成计划”
2. 创建 `planning` 类型 run
3. Runs Tab 出现新 run
4. Council Tab 展示对应步骤

## 9.4 执行审批

1. Execution 生成带高风险步骤
2. NotificationCenter 出现待审批
3. 用户打开 `ApprovalSheet`
4. 同意或跳过
5. 状态同步到 ExecutionStep

## 9.5 执行步骤批注

1. 用户在 Operate Step Inspector 输入批注
2. 调用 `execution_add_step_note`
3. 批注追加到 `ExecutionStep.inputPayload.user_notes`，并写入 `step_note_added` timeline event
4. 如果步骤正在运行，则状态切到 `paused`，避免继续无视人工复核意见

## 10. 视觉与交互原则

- 重点信息卡片化
- 状态标签明显
- 右侧面板承担上下文，不要把页面做成单列长文
- Timeline 和 Board 同时存在
- 所有异步动作必须有 loading 状态
- 所有空页必须有明确下一步动作

## 11. UI 开发优先级

### Phase 1

- AppShell
- Home
- Settings
- Runtime 状态组件

### Phase 2

- Mission List
- Mission Detail
- Operate 页面
- Timeline

### Phase 3

- Knowledge
- Memory 面板

### Phase 4

- Council Board
- ApprovalSheet

### Phase 5

- Simulation
- Skills

## 12. 界面规格结论

这个产品的 UI 不能只做成“左侧聊天列表 + 中间聊天区”。它必须体现三个维度：

- 当前任务在什么阶段
- 系统为什么这么做
- 用户此刻可以介入什么

因此，所有页面都要围绕：

**状态、证据、可介入点**

来设计，而不是只围绕“对话”来设计。


## 5.10 Runtime Integrations

### 当前已落地

- Runtime 页面展示 Local Runtime Adapters，可执行 allowlisted skill-tool echo、强制 timeout、探测 desktop executor、dry-run desktop action、受控 GUI automation macro；GUI macro 面板可输入 `target_remote_user_id` 并写入本地 request/response/audit。非 dry-run desktop action / macro 需要 checkbox + `RUN DESKTOP ACTION` 确认短语，前端会传入该确认短语，后端也会独立拒绝缺少确认短语的非 dry-run 请求，并仍可能被 session/platform 拒绝；页面也可通过用户可编辑 JSONL textarea 生成 trajectory summary 和本地 tabular RL training artifact。
- Runtime 页面展示 Runtime Adapter 本地审计浏览器，可按 kind/status/`target_remote_user_id` 刷新和导出 `json` / `jsonl`，并可复制或下载本地 handoff envelope JSON；审计来源为本机 SQLite settings；target 过滤只筛选本地审计 history，不证明 remote GUI execution 或 remote delivery。
- Runtime 页面展示 Hermes Native CUA audit 与 TuriX bridge audit 本地审计浏览器；两者可导出、复制和下载本地 audit payload。它们是本地 review payload，不携带 `target_remote_user_profile` envelope，也不证明 Hermes 已取得 OSWorld/SOTA GUI benchmark 能力或完成远端同步。
- Missions 页面 trajectory export 面板会本地解析已导出的 JSONL，显示 kind/source/reward_hint/invalid 摘要、最近行预览和复制按钮；Runtime 页面可对 JSONL 运行本地 tabular TD/Q-learning baseline training，但不声称训练大模型或获得 benchmark/SOTA 质量。
- Runtime 页面展示 Local Team Governance，可 bootstrap owner、upsert member、check RBAC、export/import local team bundle，并通过本地 JSON 文件路径运行 folder sync；Team audit events 可按 live state / exported bundle、actor、action 和文本搜索过滤预览，也可经后端 RBAC 导出 `json` / `jsonl` 并记录导出审计。
- 这些能力是真实本地执行/治理层；远端云服务需要真实 endpoint/凭证/网络权限，GUI macro 受 allowlist/确认门控约束，RL training 是本地 tabular baseline 而不是大模型训练。


## 2026-04-28 UI Addendum

- Skills 页面新增 Remote Skill Marketplace 面板，可加载 manifest、查看 entries/tags/source，并安装/更新到本地 skills。
- Runtime 页面新增 GUI automation macro JSON 面板和 Local RL training artifact 面板，分别复用 desktop action confirmation gate 与 trajectory JSONL；两者都可附加 future remote user routing metadata。Local RL jobs history 会复用同一个 Target remote user id 字段做本地历史过滤，并提供刷新按钮以拉取该 future remote user 上下文的 persisted jobs。
- Simulation 页面新增 External SaaS Simulation Adapter 与 High-Fidelity Local Sandbox 面板，显示 provider preview/response JSON 与 world model JSON；run request/history 和 capability evidence 均可携带 `target_remote_user_id`。

## 2026-04-29 UI Addendum

- Agent Exchange 页面新增 Future Remote Users 面板：本地维护 future remote user profile，支持保存、编辑、删除、填充 outbound draft、按 remote user 过滤 mailbox。页面必须持续说明 profile 是 routing metadata，不代表实时远端投递。
- Mailbox filters 新增 Remote user id；同一 scope 用于 message list 和 scoped bundle export/download。
- Skills marketplace install/history、Runtime GUI automation audit、Simulation External SaaS / High-Fidelity run history、Simulation capability evidence、Runtime local RL training job/artifact export 均新增或复用 Target remote user id 输入。字段名统一为 `target_remote_user_id`；Marketplace history、Runtime adapter audit list/export、Simulation run history 和 Local RL job history 会把该输入作为本地过滤条件。页面必须展示边界说明：这是 future remote user routing metadata，不代表 remote delivery、remote marketplace account activity 或 remote RLHF infrastructure。若用户通过本地 Agent Exchange future remote user picker 填充目标，Marketplace audit、Runtime adapter audit handoff、Simulation capability evidence、Local RL artifact export 还会写入 `target_remote_user_profile` snapshot，便于未来 handoff recipient 看到本地 profile 上下文；手动输入未匹配本地 profile 时该 snapshot 为 `null`。
