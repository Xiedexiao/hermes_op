# Hermes Native CUA Rewrite Track

## 1. 文档目标

本文档说明 `native_cua_*` 在 Hermes Native CUA rewrite 中的角色。它描述的是 Hermes-native 的本地会话底座，而不是外部桌面桥接，也不是 OSWorld / SOTA 证据文档。

`TuriX-CUA` 仍然保留为外部兼容/参考 bridge，用来描述外部桌面能力基准与 handoff 约束；`native_cua_*` 则是 Hermes-native rewrite track，用来承载本地 session / probe / observe / action / audit substrate。

## 2. 当前交付物

当前交付物已经从单一动作底座升级为 Hermes-native 的本地 CUA loop：

- session 可以启动、复用和追踪
- probe 可以判断本地底座是否可用
- observe 可以生成结构化观察结果
- action 可以在受控边界内执行
- planner 可以生成本地 deterministic plan，并选择技能目录中的候选 skill
- step runner 可以接收 TuriX-compatible actor action JSON，翻译为 Hermes native action 并执行/审计；公开动作目录以 `native_cua_prepare_model_turn` 返回的 `action_catalog` 为准，执行层还会归一化少量别名（例如 `double_click`）
- memory 可以处理 `record_info`，把文本保存到系统临时目录下的本地记忆文件并写入审计
- history 可以保存 Brain/Actor/Controller step record
- trajectory 可以导出 plan / step / memory / audit JSON 或 JSONL
- audit 可以记录、列表和导出
- model runtime seam 可以生成 Brain / Actor / Planner / Memory 的提示包，并把真实模型输出回写到同一 guarded loop
- `native_cua_preview_model_route` 可以在创建 session 前预览 `auto` / `custom` 实际会选择的 provider、model、base_url、api_key_ref、难度和原因，避免 Auto 模式黑箱启动
- `native_cua_invoke_model` 可以把已准备好的 Brain / Actor / Planner / Memory turn 发送到真实 provider，默认只做 dry-run 预览，并在非 dry-run 时强制要求精确确认短语 `INVOKE NATIVE CUA MODEL`

这一层把 TuriX-CUA 展示的 Brain / Actor / Planner / Memory / Controller loop 改写为 Hermes-native 的本地闭环。它仍然不声称 OSWorld / SOTA 成绩；真实 live 桌面动作仍受本机平台工具、系统权限和确认短语约束。

## 3. 命令契约

### `native_cua_probe`

检查 Hermes-native CUA substrate 的健康状态、会话承载能力、审计 sink 可用性和当前能力边界。它回答的是“本地底座是否可用”，不是“外部 GUI 是否已经可控”。

### `native_cua_start_session`

启动一个新的本地会话，作为后续 observe、action 和 audit 的锚点。会话应该可关联到任务上下文，并保持可追踪、可回放。开始任务时支持 `model_mode=auto` 或 `model_mode=custom`：`auto` 会根据任务文本的长度和复杂度信号归类为 `easy` / `standard` / `hard`，然后从桌面端配置的 Auto model router 选择对应模型；`custom` 会把本次任务选择的 `provider`、`model`、`base_url`、`api_key_ref` 存到 session，并在后续 `native_cua_invoke_model` 未显式覆盖时优先使用。

### `native_cua_preview_model_route`

在不创建、不恢复、不写入 session 的前提下，使用与 `native_cua_start_session` 相同的模型选择逻辑预览任务会走的模型路由。请求包含 `task`、`model_mode`（`auto` / `custom`）以及可选 `provider`、`model`、`base_url`、`api_key_ref`；响应包含最终 `model_mode`、`provider`、`model`、`base_url`、`api_key_ref`、`model_difficulty`、`model_selection_reason` 和 summary。Runtime UI 的 “Preview model route” 按钮使用该命令让用户在开始任务前看到 Auto router 的实际档位和模型选择；它不调用模型、不执行桌面动作，也不产生 benchmark 证据。

### `native_cua_observe`

读取当前会话的安全观察面，输出结构化事实、快照或摘要。这个命令应保持只读，不应隐式执行动作，也不应把外部 bridge 的成功结果伪装成本地能力。

### `native_cua_execute_action`

在当前会话内执行一个受控动作，并把结果写入审计轨道。命令必须尊重权限、前置条件和可审计性要求；如果无法安全执行，就必须失败，而不是切换到另一个未声明的路径。

### `native_cua_list_audit_events`

列出本地会话或底座范围内的审计事件，供人类复核、回放和后续记忆沉淀使用。这个命令只读取审计，不改变审计。

### `native_cua_export_audit_events`

导出审计事件，通常用于 `json` 或 `jsonl` 格式的离线检查、归档和共享。导出只能证明本地记录过什么，不能自动升级为 OSWorld / SOTA 证据。

### `native_cua_plan_task`

在已有 session 上生成 Hermes-native 的本地 deterministic plan。输入可以包含任务文本、技能目录 metadata 和最大步骤数；输出包含 plan steps、selected skills、iteration info 和摘要。它对应 TuriX Planner 的本地可审计改写，不调用外部模型，也不伪装为 benchmark 成绩。

### `native_cua_run_step`

执行一个 Brain / Actor / Controller step。命令会先通过 observe 生成本地观察，然后接收 TuriX-compatible actor action JSON（公开目录以 `native_cua_prepare_model_turn` 返回的 `action_catalog` 为准；执行层还会归一化少量别名，例如 `double_click`），翻译为 Hermes native action、memory record 或 done 状态，并写入 history 与 audit。坐标参数兼容 Hermes 原生 `0..1` 归一化值，也兼容 TuriX actor 常见的 `0..1000` thousandth-scale 输出，并在进入平台执行计划前统一归一化。默认 dry-run；live step 复用 `RUN NATIVE CUA ACTION` 确认短语。

### `native_cua_list_history`

读取本地 step history，返回每一步的 brain state、observe result、action results、final result 和 summary，供 UI 回放与审计复核。

### `native_cua_record_info`

对应 TuriX 的 `record_info` 动作，把文本保存到系统临时目录下的 `hermes-native-cua/records/<session>/...txt`，同时写入 `app_settings` memory records 与 audit。

### `native_cua_export_trajectory`

导出完整本地 CUA trajectory，包含 plan、step history、memory records 和可选 audit events。支持 `json` / `jsonl`，用于离线复核、数据集准备和后续 RL/trajectory 研究接入；它本身不训练模型。

### `native_cua_prepare_model_turn`

为 Brain / Actor / Planner / Memory 生成可交给真实模型 runtime 的 prompt/messages/schema 包。响应包含系统提示、用户上下文、响应 schema、TuriX-compatible action catalog 和可选本地截图路径附件；当前实现只附 `screenshot_path` 引用，不内嵌 base64。这个命令不调用远端模型，也不消耗 API key；它是 Hermes-native CUA 与真实 VLM runtime 之间的明确边界。

### `native_cua_invoke_model`

把 `native_cua_prepare_model_turn` 产出的 Brain / Actor / Planner / Memory turn 发送到真实模型 provider，构建 provider payload 并返回 dry-run 预览或真实调用结果。支持的 provider target 包括 OpenAI-compatible providers（OpenAI / OpenRouter / DeepSeek）、Anthropic 和 Ollama。桌面 Runtime 页面现在可以直接加载/保存 `provider`、`model`、`base_url` 和 `api_key_ref` 到 SQLite `app_settings.runtime`；请求未显式覆盖且 session 没有 custom 模型配置时，`native_cua_invoke_model` 会读取这组桌面默认模型配置。开始任务若选择 custom 模式，session 级模型配置优先于桌面默认值；若选择 auto 模式，后端会把难度路由得到的 `provider` / `model` / `base_url` / `api_key_ref` 固化到 session，并返回 `model_difficulty` 与 `model_selection_reason` 供 UI 审计；用户也可以先调用 `native_cua_preview_model_route` 预览同一选择结果而不创建 session。`base_url` 兼容带 `/v1` 的桌面配置，不会重复拼接 `/v1/v1`。默认 `dry_run=true`，只输出请求预览，不会触发外部或付费端点；非 dry-run 必须精确匹配 `INVOKE NATIVE CUA MODEL`，因为这个命令会接触真实模型调用成本与外部网络边界。若 `apply_output=true`，actor JSON 可以直接回灌到 `native_cua_run_step`；其他角色输出则保留为 model-turn record。这个命令负责真实模型 invocation plumbing，不负责 OSWorld / SOTA 证明。桌面 live action 仍然必须另行满足 `RUN NATIVE CUA ACTION`。

### `native_cua_apply_model_output`

把真实模型返回的 JSON 输出回写到 Hermes-native CUA loop。`actor` 输出必须包含 `action` 数组，并会通过 `native_cua_run_step` 执行、审计和写 history；`brain` / `planner` / `memory` 输出先作为 model-turn record 存档。非 dry-run actor 输出仍必须满足 `RUN NATIVE CUA ACTION`。

## 4. 设计边界

- `TuriX-CUA` 是外部 compatibility/reference bridge，不是 Hermes-native rewrite track
- `native_cua_*` 是 Hermes-native rewrite track，不是外部 bridge
- 当前范围覆盖安全本地 session / probe / observe / action / plan / step / memory / history / trajectory / audit loop
- OSWorld、SOTA、真实 GUI 成功率和桌面可控性仍然必须来自真实运行环境

## 5. 可挂接的未来循环

当前已经具备不依赖外部 TuriX 进程的本地 loop 命令：`native_cua_plan_task`、`native_cua_run_step`、`native_cua_list_history`、`native_cua_record_info`、`native_cua_export_trajectory`、`native_cua_preview_model_route`、`native_cua_prepare_model_turn`、`native_cua_invoke_model`、`native_cua_apply_model_output`。后续如果接入真实 VLM，可沿用同一轨道：

- Brain 读 `native_cua_observe` / `native_cua_run_step` 中的 observation，做状态理解
- Planner 可以替换或增强 `native_cua_plan_task` 的 deterministic plan
- Actor 输出 TuriX-compatible action JSON 给 `native_cua_run_step`
- `native_cua_invoke_model` 负责把已准备好的 turn 送进真实 provider，并在需要时把 actor JSON 回灌到 `native_cua_run_step`
- Memory 通过 `record_info` / `native_cua_record_info` 沉淀可复用上下文
- Trajectory 通过 `native_cua_export_trajectory` 导出给后续研究层

这些循环共享同一个会话、history、memory 与审计轨道，而不是绕开它们。

## 6. 相关文档

- [Hermes Operator 功能设计文档](./hermes-agent-desktop-functional-design.md)
- [Hermes Operator 领域模型与契约文档](./hermes-agent-desktop-domain-contracts.md)
- [TuriX-CUA Runtime Bridge 契约说明](./turix-cua-runtime-bridge.md)
