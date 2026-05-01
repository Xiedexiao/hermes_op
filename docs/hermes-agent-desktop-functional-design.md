# Hermes Operator 功能设计文档

## 1. 文档目标

本文档在产品设计基础上，进一步定义 `Hermes Operator` 的功能结构、关键流程、核心对象、阶段范围与非功能要求。本文档强调“能做什么”和“如何协同”，不展开到代码级实现细节。

## 2. 功能设计原则

### 2.1 Mission 优先

系统围绕 `Mission` 组织，而不是围绕单轮对话组织。每次任务都应具备完整上下文、可见状态与可回放过程。

### 2.2 可见优于自动

多 Agent 协作、推演结果、执行计划、审批节点都要可视化。用户必须知道系统为什么这么做。

### 2.3 稳定优先于炫技

在产品定义层，桌面软件操作必须是一等公民能力；在执行引擎层，仍然优先选择更稳定的执行通道。

执行路径默认优先：

1. API
2. CLI
3. 浏览器自动化
4. 桌面 GUI 自动化

只在必要时进入更脆弱的执行方式。

### 2.4 默认沉淀长期资产

完成一次任务后，系统默认沉淀可复用资产：

- 记忆
- 产物
- 模板
- 技能建议
- 风险与失败经验

## 3. 功能来源映射

| 功能域 | 主要参考项目 | 级别 | 设计引入方式 |
| --- | --- | --- | --- |
| Agent 内核 | `hermes-agent` | 复刻基线 | 用 Rust 绿地重写其任务理解、技能编排、工具调用和后台任务能力 |
| 桌面执行 | `TuriX-CUA` | 重点 benchmark | 桌面 GUI 能力基准与外部 runtime bridge 参考，不把 benchmark 伪装成 Hermes 原生 parity 层 |
| 统一检索 | `onyx` | 增强模块 | 知识源接入、RAG、深度研究、Artifacts |
| 长时记忆 | `mempalace` | 增强模块 | 项目记忆、会话检索、逐字级原因回溯 |
| 多 Agent 治理 | `edict` | 增强模块 | 计划、审议、派发、看板、人工干预 |
| 推演引擎 | `MiroFish` | 增强模块 | 变量建模、情境模拟、方案对比 |
| 学习闭环 | `DeepTutor` | 增强模块 | 任务复盘、成长建议、流程模板化 |

## 4. 功能总览

首版系统拆分为 8 个一级模块：

1. Desktop Shell
2. Mission Workspace
3. Knowledge Fabric
4. Scenario Sandbox
5. Council Orchestrator
6. Desktop Operations Engine
7. Memory Palace
8. Growth Engine

其中：

- Rust 重写的 Hermes 等价 Agent Core 驱动的桌面操作链路是主能力
- `Knowledge / Memory / Council / Scenario` 都是围绕主能力叠加的增强层

## 5. 模块设计

### 5.1 Desktop Shell

### 目标

为用户提供桌面级启动、驻留、通知与全局入口能力。

### 核心功能

- 应用主窗口
- 系统托盘
- 全局快捷唤起
- 本地服务状态检查
- 模型与执行器状态提示
- 通知中心
- 任务完成与审批提醒

### 关键要求

- 应用关闭主窗口后可选择继续在后台运行
- 托盘需显示当前活跃 Mission 数、待审批数、失败数
- 通知点击后应直接跳转到对应 Mission 的相关节点

### MVP 边界

- 必须支持主窗口、托盘、通知
- 暂不要求复杂多窗口布局

### 5.2 Mission Workspace

### 目标

作为用户的主工作台，承载一个 Mission 的全生命周期。

### 核心功能

- 创建 Mission
- 输入目标、约束、成功标准
- 附加文件、链接、笔记、历史 Mission
- 任务时间线
- 当前阶段展示
- 产物侧栏
- 用户评论与批注
- 任务回放

### 页面结构建议

- 左栏：Mission 列表
- 中栏：对话与时间线
- 右栏：上下文、产物、审议、执行计划

### Mission 状态

- `draft`
- `researching`
- `simulating`
- `planning`
- `awaiting_approval`
- `executing`
- `paused`
- `completed`
- `failed`
- `archived`

### 关键要求

- 一个 Mission 内允许多模式切换，但必须保持同一上下文连续性
- 应用启动时会恢复 pinned 或最近活跃的未完成 Mission，避免重启后丢失任务指向
- 所有关键节点均可插入“用户批注”并影响后续执行；Operate Step Inspector 已可把批注写入 execution step input payload、记录 timeline event，且运行中步骤会先暂停再等待人工复核

### 受 `DeepTutor` 启发的设计点

- 同一任务在研究、规划、执行、复盘阶段使用统一线程
- 避免用户在多个工具之间重复粘贴上下文

### 5.3 Knowledge Fabric

### 目标

把本地资料、外部网页和连接器内容变成可检索、可引用、可追溯的知识层。

### 核心功能

- 本地文件导入
  - PDF
  - DOCX
  - Markdown
  - TXT
  - 表格
- 文件夹级知识集
  - 当前已提供本地 folder import：递归/非递归扫描 UTF-8 `.md` / `.markdown` / `.txt` / `.json` / `.csv`，按 max files 批量写入 Mission context 与 KnowledgeSource/chunks。
- Web Search
- URL 抓取与归档
  - 当前已提供本地 `knowledge_fetch_url_preview` connector：只允许 `http/https`、8s timeout、128 KiB body cap、简单 HTML title/text 摘要；fetch preview 不自动入库，用户确认 Attach URL 后才写入 Mission context 与 KnowledgeSource。
- 连接器接入
  - 首版建议先做轻量连接器
  - 本地文件系统
  - Google Drive / Notion / GitHub 可作为后续阶段
- 检索结果证据卡片
- 深度研究报告生成

### 关键能力

- 语义检索 + 关键词检索
- 引用来源高亮
- 支持 Mission 级临时知识集
- 支持项目级长期知识集

### MVP 边界

- 本地文件导入与网页检索必须有
- 企业级连接器在后续阶段扩展

### 受 `Onyx` 启发的设计点

- 检索不仅返回答案，还返回证据、来源与可下载产物
- 研究工作流是一个显式模式，而不是隐式的长回答

### 5.4 Scenario Sandbox

### 目标

在 Mission 级决策前，用 baseline + option cards 做轻量、可追溯的文本推演。当前版本已经能保存 scenario run，并提供变量权重滑块、recommendation rationale、mission-level comparison matrix、path evolution 视图和内置 deterministic Local Multi-Agent Sandbox。Sandbox 会真实生成多 agent/多轮 turns、option scores、recommendation，并写入 completed simulation run 与 audit event；历史 runs 可按 Mission 选中回放完整 agents/turns/scoreboard/recommendation。外部 SaaS 仿真已接入受控 provider adapter：`local_echo` 可离线执行，`http_json` 可 dry-run 预览，真实 HTTP 调用必须显式提供确认短语和 endpoint。高保真沙盘以本地 deterministic world model 落地，包含 entities、variables、timeline、event graph 与 option heatmap。

### 核心功能

- 选择 Mission
- 记录 baseline（当前方案或现状）
- 记录候选 option
- 当前 UI 先以一行一个 option 的文本输入为主，后端会把它们结构化成 option cards
- option card 支持并自动生成 `assumptions`、`expected_benefits`、`risks`、`projected_outcomes`、`score`、`confidence`
- 对比 baseline 与各 option 的差异
- 输出 recommendation 与 recommendation rationale
  - rationale 可由用户手动填写
  - 也可按 option card score / confidence / projected outcome 自动归因
- 保留 scenario run 历史，便于回看和复盘

### 交互形态

- baseline 与 option cards 分栏展示
- option card 卡片式对比
- 结果侧展示 option 的结构化详情，而不是通用模拟参数面板
- recommendation 区域展示推荐结论、解释理由和治理交接状态

### 当前已落地

- 可以为 Mission 保存 scenario run
- 可以按 Mission 查看历史 scenario runs
- 可以在保存后看到 option cards、confidence、recommendation 和 recommendation rationale
- Simulation 页面也会展示 overview、run type mix、status mix 和 recent runs
- 已保存 scenario runs 会按 Mission 聚合为 comparison matrix
- Path evolution 视图会展示同一 Mission 的方案演化路径
- 变量注入已具备独立变量结构、impact/uncertainty 数值权重和滑块 UI
- 保存 scenario run 会按 handoff policy 自动生成 completed simulation run、timeline event、Scenario Reviewer Council step 和/或 Execution review step
- Handoff policy 支持内置模板、保存自定义模板，并可跨 Mission 复用
- 变量评分公式支持用户在页面调整 base score、impact multiplier 和 uncertainty penalty，生成后的 option scores 会随 scenario run 持久化
- Scoring formula 支持内置模板、自定义保存和跨 Mission 本地复用
- Handoff policy templates 与 scoring formula templates 支持导出/导入 portable JSON bundle，可用于团队手动共享

### 已落地后的扩展边界

- Template bundle 已记录本地 export/import audit log；本地 Team Governance 已提供 RBAC、audit、bundle export/import、共享目录 JSON sync 和前端 audit filter/export preview。
- Runtime Adapter 已记录本地持久化审计事件，并可在 Runtime 页面按 kind/status 查看与导出 `json` / `jsonl`。

### 非本地闭环能力

- 已完成内置 Local Multi-Agent Sandbox、受控 External SaaS provider adapter、High-Fidelity Local Sandbox world model；其中 `http_json` 真实外呼仍依赖用户提供 endpoint/凭证/网络权限，系统不会伪造外部 SaaS 结果。
- Handoff/scoring template bundle 已支持手动团队共享、本地 audit log、只读 audit JSON export、import preflight，以及本地 Team Governance RBAC/audit/bundle sync；远端团队协作服务仍是可选外部集成。

### 关键要求

- 必须明确标记“推演结论不是事实，只是基于当前信息的策略模拟”
- 必须展示推演的关键假设
- 必须说明 recommendation 是手动输入还是基于 option cards 归因

### MVP 边界

- 首版只做 text-first、Mission 级 scenario runs
- 优先支持 baseline + option cards + recommendation 兜底
- 高保真数字沙盘与复杂可视化留到后续版本

### 受 `MiroFish` 启发的设计点

- 不是只生成建议列表，而是对 baseline 和 option cards 做可解释对比
- 不是只给 recommendation，而是说明推荐由哪些 `assumptions` / `benefits` / `risks` / `confidence` 支撑
- 不是做泛化模拟引擎，而是先把变量注入做成可审阅的结构化文本
- 已把同一 Mission 的 saved scenario runs 做成 comparison matrix 和 path evolution 回看，单次 run 详情仍保留 option card 级解释

### 5.5 Council Orchestrator

### 目标

提供多 Agent 的任务治理层，让复杂任务的拆解、审议、执行与复盘可见、可控、可干预。

### 默认角色建议

- `Scout`：资料搜集
- `Analyst`：问题拆解
- `Critic`：反方审查与风险提示
- `Planner`：任务计划与里程碑
- `Executor`：执行步骤落地
- `Reviewer`：结果验收

### 核心功能

- 任务拆解
- 角色分工
- 审议意见流
- 计划打回重做
- 执行阶段看板
- 中断、暂停、恢复、重试
- Agent 健康状态展示
- 成本与耗时展示

### 看板视图

建议列：

- 待理解
- 待规划
- 待审议
- 待审批
- 执行中
- 已完成
- 异常

### 关键要求

- 用户可以查看某一步为什么被打回
- 用户可以选择跳过某个 Agent 或手动修改计划
- 所有 Agent 产出都必须带时间戳与责任角色

### MVP 边界

- 首版采用固定角色模版即可
- 角色自定义市场可后置

### 受 `edict` 启发的设计点

- 多 Agent 最大价值不是并发，而是制衡与审议
- 看板与时间线是建立信任的必要 UI

### 5.6 Desktop Operations Engine

### 目标

把“操作桌面软件”提升为独立、一等公民的产品能力，并把计划变成真实动作，覆盖终端、浏览器和桌面应用。桌面 GUI 相关能力分成两条线：`TuriX-CUA` 作为外部兼容/参考 bridge，`native_cua_*` 作为 Hermes-native rewrite track。

### 执行层级

1. API 执行
2. CLI 执行
3. 浏览器自动化
4. 桌面 GUI 自动化

### 核心功能

- 当前目标软件展示
- 执行计划生成
- 动作风险分级
- 高风险动作审批
- 执行过程日志
- 截图与结果证据
- 失败恢复与回退建议

### 动作风险等级

- `low`
  - 读文件
  - 搜索网页
  - 草稿生成
- `medium`
  - 修改本地草稿文件
  - 填写表单但不提交
- `high`
  - 发送消息
  - 提交审批
  - 删除或覆盖文件
  - 对外发布

### GUI 执行能力

- 打开应用
- 切换窗口
- 识别 UI 元素
- 填写文本
- 点击按钮
- 复制与粘贴
- 获取屏幕结果

### 当前已落地

- Operate 页面已展示 Mission 当前执行队列、恢复队列和 Step Inspector
- Step Inspector 可写入用户批注，并把运行中的步骤暂停到恢复队列
- CLI step 已有真实本地 runner；desktop step 可生成 auditable desktop handoff prompt 与 checklist
- Desktop handoff 会记录 `desktop_handoff_prepared` 与人工 `desktop_handoff_reviewed` timeline event，并在 Operate 中形成 prepared/reviewed queue；这只是 bridge 准备和审计，不是自动 GUI 操作
- Model runtime 走独立的 prepare → invoke → apply 路径：`native_cua_prepare_model_turn` 只产出 turn，Runtime / Native CUA 页面可以直接配置并保存 provider/model/base_url/api_key_ref 到桌面 runtime settings；开始 Native CUA 任务时可以选择 Auto（按任务难度选择 easy/standard/hard 模型档位）或 Custom（本任务独立模型配置），`native_cua_invoke_model` 按“请求显式覆盖 → session auto/custom → 桌面默认 → provider 默认”的顺序解析，并默认只做 dry-run 预览；非 dry-run 时要求精确确认短语 `INVOKE NATIVE CUA MODEL`，`apply_output=true` 时 actor JSON 可以回灌到 `native_cua_run_step`

### 关键要求

- 桌面软件操作必须在 UI 中有独立可见区域，而不是隐藏在日志中
- 高风险动作默认需要用户确认
- 每个执行步骤必须可回放
- 每次失败需记录失败位置与重试策略
- 用户批注必须作为执行上下文的一部分被记录，不能只停留在前端临时状态
- Runtime adapter 已支持 allowlisted skill-tool execution 的强制 timeout、桌面 executor 探测和默认 dry-run desktop action；显式非 dry-run 仍受平台 allowlist 与 PATH availability 约束，UI 还要求 checkbox + `RUN DESKTOP ACTION` 确认短语。
- 真实模型调用是另一条受控边界：`native_cua_invoke_model` 可以构建 provider payload 并默认做 dry-run preview；桌面端保存的模型配置只影响默认 provider/model/base_url/api_key_ref，不会绕过确认短语、API key 环境变量解析或动作审批。如果要发往外部或付费模型端点，必须精确确认 `INVOKE NATIVE CUA MODEL`，而桌面 live action 仍然单独要求 `RUN NATIVE CUA ACTION`。

### TuriX 边界说明

- Hermes 原生安全闭环只覆盖 API / CLI / Browser / 本地 dry-run / 可审计 handoff，所有执行与审计都留在本地。
- `TuriX-CUA` 在这里是桌面能力 benchmark，也是外部 runtime bridge 的参考，bridge 只承担外部 handoff / launch 约束，不是 Hermes 原生 GUI capability，也不是本地 parity 层。
- 真实 GUI 依赖 Accessibility、屏幕录制、窗口控制等系统权限；没有这些权限时，只能保留 handoff，不得伪装成已执行。
- OSWorld、SOTA 和真实 GUI 成功率只能来自外部环境与真实运行结果，不能在文档里按“已实现能力”处理。更细的 bridge 契约见 [TuriX-CUA Runtime Bridge 契约说明](./turix-cua-runtime-bridge.md)。

### Native CUA Rewrite Track

- `native_cua_*` 是 Hermes-native rewrite track，用来承载安全的本地 session / probe / observe / action / plan / step / memory / history / trajectory / audit loop。
- 这条轨道的当前交付物是“可审计、可回放、可 UI 操作、可本地验证”的 CUA loop，而不是 OSWorld / SOTA 证据或桌面 GUI 终局 parity。
- `native_cua_plan_task`、`native_cua_observe`、`native_cua_run_step`、`native_cua_execute_action`、`native_cua_record_info`、`native_cua_export_trajectory`、`native_cua_preview_model_route`、`native_cua_prepare_model_turn`、`native_cua_invoke_model` 和 `native_cua_apply_model_output` 共同构成 Hermes-native 的 Brain / Actor / Planner / Memory / Controller / Model Runtime 接入面。
- 详细命令契约见 [Hermes Native CUA Rewrite Track](./native-cua-rewrite.md)。

### 受 `hermes-agent + TuriX-CUA` 启发的设计点

- Rust 重写的 Hermes 等价 Agent Core 负责理解任务、生成操作计划和调度动作
- `TuriX-CUA` 只在外部 bridge 形态下承担最后一公里 GUI 执行参考，不在 Hermes 原生闭环内假定已接通
- Hermes-native 的 `native_cua_*` 轨道负责把会话、计划、观察、动作、记忆、历史、轨迹和审计沉淀为可组合的本地能力，而不是把桌面能力绑定到外部 bridge 的成败上

### 5.7 Memory Palace

### 目标

记录完整工作上下文，让系统具备长期连续性。

### 核心功能

- 自动沉淀 Mission 对话与关键节点
- 保存执行前后决策原因
- 按项目、主题、人物建立记忆分区
- 全局记忆检索
- Mission 启动时上下文唤醒
- 记忆固定与屏蔽
- 敏感记忆删除

### 记忆内容类型

- 用户目标与偏好
- 历史决策原因
- 失败案例
- 常用资料
- 反复执行流程
- 与特定项目相关的上下文

### 检索入口

- Mission 内“找回历史类似任务”
- 全局搜索“我们之前为什么这样做”
- 任务创建时自动推荐相关记忆

### 关键要求

- 支持逐字级原始记录检索
- 摘要只能作为导航层，不能替代原文
- 用户需能看见哪些内容被纳入记忆

### 受 `mempalace` 启发的设计点

- 高保真存储优先于过早抽象
- “为什么”与“怎么失败的”比“最终结论”更重要

### 5.8 Growth Engine

### 目标

让每次任务执行后，都能转化为未来更高质量的任务能力。

### 核心功能

- 任务复盘摘要
- 经验点提取
- 流程模板建议
- 技能候选建议
- 用户偏好更新
- 类似任务推荐流程

### 输出形态

- `Playbook`
  - 一类任务的标准流程
- `Skill Suggestion`
  - 可安装或可生成的新技能
- `Personal Preference Update`
  - 对用户风格、约束、偏好的更新
- `Learning Card`
  - 针对用户工作方式的改进建议

### 当前已落地

- Skills 页面可以管理本地发现的 `SKILL.md` 启用状态
- 启用的 skill 可以被渲染为 runtime invocation payload，payload 包含 command、instruction、来源路径和完整 skill instructions
- Skill invocation payload 可以保存到本地 session message，并能按 session 查看/重放；`skills_execute_runtime` 可生成 dry-run execution package，或通过 Runtime Adapter 的 `printf`/`echo` allowlist 做本地 validation 并记录审计
- 禁用的 skill 会被拒绝 invocation，避免 UI 与 runtime 候选能力不一致
- Skill Evolution Inbox 可以把失败证据沉淀为待评审的 skill 改进候选
- Mission 运行轨迹可导出为 JSONL trajectory dataset，用于离线研究、回放或未来 RL 数据准备

### 关键要求

- 复盘不能打断主任务交付
- 复盘以轻量、半自动方式进行
- 本地 skill payload adapter 不等同于任意模型/工具执行；当前闭环只允许受限本地 validation command，进一步 sandbox、审批和工具解释仍必须由 runtime 层扩展

### 受 `DeepTutor` 启发的设计点

- Agent 不只帮用户完成任务，还帮助用户成长为更强的任务执行者

### 5.9 Voice Workflow

### 目标

提供可审计的本地 voice-like 工作流：用户手动输入 transcript，系统归一化并持久化；助手输出文本进入 speak queue，用户或外层 runtime 再显式标记为已处理。该模块当前不声明真实音频能力。

### 当前已落地

- `voice_list_providers` 暴露本地 provider catalog，并为每个 provider 返回 `interaction_model`、`supports_audio_input`、`supports_audio_output`、`compatibility_aliases` 和 `runtime_boundary`。
- `local-text-capture` 是手动文本录入 provider，不采集麦克风、不做音频 STT。
- `local-speak-queue` 是文本队列 provider，不合成音频、不播放声音。
- `voice_transcribe`、`voice_speak`、`voice_list_history` 和 `voice_process_speak_queue` 已形成 SQLite-backed 本地闭环。
- `voice_transcribe_stub` / `voice_speak_stub` 只为旧调用方兼容保留，内部转发到真实本地 workflow，并标记为 compatibility wrapper。

### 后续扩展边界

- 真实麦克风录音、音频 STT、音频 TTS、消息平台语音附件和 provider 凭证管理必须作为新 provider/runtime lane 单独设计。
- 未实现前不得把当前 Voice 页面表述为音频语音助手或可用的 STT/TTS provider。

## 6. 核心用户流程

### 6.1 标准流程

1. 用户创建 Mission
2. 输入任务目标与约束
3. 系统确定目标桌面软件或操作上下文
4. Desktop Operations Engine 形成操作主通路
5. 系统建议导入相关资料与历史记忆
6. Knowledge Fabric 完成检索与研究
7. 若任务复杂，进入 Scenario Sandbox 推演
8. Council Orchestrator 形成计划并审议
9. 用户确认执行计划
10. Desktop Operations Engine 执行动作
11. 产物输出并回传
12. Memory Palace 自动归档
13. Growth Engine 产出复盘与模板建议

### 6.2 快捷流程

适用于低复杂任务：

1. 用户创建 Mission
2. 系统直接给出计划
3. 用户确认
4. 执行并归档

### 进入快捷流程的条件

- 任务风险低
- 不需要多方案推演
- 无对外不可逆动作

### 6.3 审批流程

对于高风险动作：

1. 系统生成执行清单
2. 标出高风险步骤
3. 用户逐项确认或整体确认
4. 系统执行
5. 结果回写日志

## 7. 数据对象设计

建议在产品层定义以下核心对象：

### 7.1 Mission

- `id`
- `title`
- `goal`
- `constraints`
- `status`
- `priority`
- `owner`
- `created_at`
- `updated_at`

### 7.2 Context Source

- `id`
- `type`
  - file
  - url
  - note
  - connector_item
  - memory_reference
- `title`
- `uri`
- `summary`

### 7.3 Simulation Scenario

当前实现先落地：

- `id`
- `mission_id`
- `baseline`
- `options`
- `option_cards`
- `recommendation`
- `recommendation_reason`
- `variables`
- `comparison_summary`
- `selected_option_id`

其中：

- `option_cards` 承载 `assumptions`、`expected_benefits`、`risks`、`projected_outcomes`、`score`、`confidence`
- `variables` 承载 `current_value`、`proposed_value`、`impact_weight`、`uncertainty_weight`，并派生 low/medium/high 标签
- `recommendation` 保留被选 option 的结论标签，`recommendation_reason` 保留解释卡文本
- 保存 scenario 时会按可复用 handoff policy 同步写入 mission timeline、Council review 和/或 Execution review handoff
- `risk_score` 和外部仿真引擎输出仍属于后续增强字段

### 7.4 Council Task

- `id`
- `mission_id`
- `role`
- `status`
- `input`
- `output`
- `review_notes`

### 7.5 Execution Run

- `id`
- `mission_id`
- `plan`
- `steps`
- `risk_level`
- `status`
- `artifacts`
- `logs`

### 7.6 Memory Record

- `id`
- `scope`
  - user
  - project
  - mission
- `content`
- `source_link`
- `importance`
- `visibility`

### 7.7 Playbook

- `id`
- `title`
- `trigger_conditions`
- `steps`
- `related_missions`
- `confidence`

## 8. MVP 功能清单

### P0 基础可用版

- Desktop Shell
- Mission Workspace
- 本地文件知识接入
- 基础检索
- 基础记忆沉淀
- 单 Agent 或轻量多 Agent 计划生成
- CLI 与浏览器执行

### P1 首个可差异化版本

- Council 看板
- 审议与审批流程
- 桌面 GUI 执行
- 任务回放
- Mission 历史检索
- 基础复盘与 Playbook 生成

### P2 强化差异化版本

- Scenario Sandbox
- 多方案推演
- 变量注入
- 后台定时任务
- 跨渠道通知

## 9. 非功能要求

### 9.1 隐私与安全

- 敏感资料默认本地存储
- 高风险动作强制审批
- 执行日志可审计
- 记忆内容支持删除与屏蔽

### 9.2 可恢复性

- Mission 中断后可恢复
- 执行失败后可从步骤重试
- 应用重启后恢复未完成 Mission：`app_get_bootstrap` 会返回 pinned 优先、否则最近活跃的非终态 Mission，Home 和右侧 Context 可直接继续打开

### 9.3 可观测性

- 任务状态可见
- Agent 状态可见
- 执行日志可见
- 推演假设可见
- 成本与时长可见

### 9.4 性能

- 桌面主界面首屏应快速可用
- 大任务的研究、推演、执行允许后台异步进行
- 检索结果应优先返回摘要，再补全文档级细节

## 10. 推荐实现边界

虽然本文不是技术架构文档，但为了保证功能设计可落地，建议采用如下边界：

- 桌面壳：固定采用 `Tauri 2`
- 桌面主进程与本地协调层：固定采用 `Rust`
- 前端：Tauri 内嵌的 React / TypeScript 工作台
- Agent 与工具执行：用 Rust 绿地重写 `hermes-agent` 的功能与行为
- 检索与研究：吸收 `onyx` 的知识层设计思路
- 记忆层：参考 `mempalace` 的高保真记忆策略
- GUI 执行：以 `TuriX-CUA` 作为桌面能力 benchmark 和外部 runtime bridge 参考，真实 GUI 仅在系统权限与 bridge 接通时落地，不把 benchmark 当作本地原生 parity；本地闭环的执行与审计仍然只属于 Hermes runtime adapter
- 治理看板：借鉴 `edict`
- 推演模块：借鉴 `MiroFish`

原则是“抽能力，不硬搬产品”，同时桌面端技术栈不再开放讨论，默认锁定为 `Rust + Tauri`。

## 11. 明确不做

为了防止首版失控，以下内容不建议进入首版：

- 完整企业管理后台
- 重型多租户权限体系
- 大量垂直行业模版
- 高保真 3D 或复杂沙盘可视化
- 与所有第三方 SaaS 的一次性深度打通

## 12. 功能设计总结

`Hermes Operator` 的功能设计核心，不是把“聊天、检索、自动化、记忆、仿真”几个词堆在一起，而是围绕一个稳定的工作对象 `Mission`，把 Rust 重写的 Hermes 等价 Agent Core 作为能力基线，把桌面软件操作作为主链，把其他项目能力作为增强层：

**理解任务 -> 操作桌面软件 -> 校验结果 -> 沉淀记忆 -> 形成成长资产**

如果产品设计强调“为什么做”，那么功能设计强调的是：

**系统必须始终围绕这条主链组织，任何功能如果不能增强这条主链，就不应进入首版。**


## 2026-04-28 Capability Closure Notes

- Remote Skill Marketplace：已通过 manifest-based `skills_marketplace_list` / `skills_marketplace_install` 落地，支持 `file` / `http` / `https` manifest 和 skill source，安装只写入本地 Hermes skills 目录；install history 可按 `target_remote_user_id` 做本地过滤。
- GUI automation：已通过 Runtime Adapter macro command 落地，接受任意 step sequence，但每一步仍经过平台 allowlist、dry-run 默认值、非 dry-run 确认短语和本地审计；GUI macro request/response/audit 可持久化 `target_remote_user_id` 作为 future remote user routing metadata，Runtime adapter audit list/export 也可按该字段本地过滤。
- External SaaS simulation：已通过 `local_echo` / `http_json` provider adapter 落地；dry-run 不外呼，真实 HTTP 调用需要确认短语，结果来自实际 response；run request/result/history payload 可持久化 `target_remote_user_id`，history list 可按该字段本地过滤。
- High-fidelity sandbox：已落地为本地 deterministic world model，不声称 3D/OSWorld/SOTA 沙盘；run request/result/history payload 可持久化 `target_remote_user_id`，history list 可按该字段本地过滤。
- Real RL training：已落地为本地 tabular TD/Q-learning baseline over trajectory JSONL，并持久化训练 job/artifact；job request/result/history/artifact 可携带 `target_remote_user_id`，job history list 也支持按该 future remote user routing metadata 本地过滤，但不声称训练大模型、获得 benchmark 质量或完成远端 RLHF。

## 2026-04-29 Future Remote User Routing Notes

- Agent Exchange 新增本地 Future Remote Users 目录，用于维护 `user_id`、display name、默认 agent、transport label、route hint 与 `active` / `paused` / `blocked` 状态。它是 remote-account routing readiness，不是远端账号系统或实时消息服务。
- Agent Exchange messages 和 bundle export/import 支持 `remote_user_id` scope；bundle 会携带相关 remote user profiles，import 会按 `updated_at` 合并 profile，同时保留消息去重。前端可下载 scoped bundle JSON，供用户通过批准的 out-of-band 路径手工交接。
- Skills marketplace install request/result/history、Runtime GUI automation request/response/audit、Simulation External SaaS 与 High-Fidelity run payload/history、Runtime local RL job/artifact，以及前端 marketplace audit / runtime adapter audit handoff / simulation evidence / local RL artifact export 都可以写入 `target_remote_user_id`。Marketplace history、Runtime adapter audit list/export、Simulation history 和 Local RL jobs history 可按该字段做本地过滤。当前前端导出的 marketplace audit、runtime adapter audit handoff、simulation evidence、local RL artifact envelope 会在目标来自本地 Agent Exchange future remote user picker 时附带 `target_remote_user_profile` snapshot；该 snapshot 只是把本地 profile 上下文带进证据包，不证明 remote delivery、remote marketplace account activity 或大规模 RLHF。
- Runtime Adapter audit handoff、Hermes Native CUA audit payload 和 TuriX bridge audit payload 均可本地下载。Native CUA / TuriX 下载只是 audit review payload，不是带 `target_remote_user_profile` 的 remote-user handoff envelope，也不证明 remote GUI execution、OSWorld/SOTA benchmark 能力或远端同步。
