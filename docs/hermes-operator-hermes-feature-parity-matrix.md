# Hermes Operator 对标 Hermes Agent 功能复刻矩阵

## 1. 文档目标

本文档把 `repos/hermes-agent` 视为**功能与行为对标基线**，将需要复刻的能力逐项拆开，作为 `Hermes Operator` 的强约束范围文档。

本文件的用途不是解释产品方向，而是回答一个更具体的问题：

**`hermes-agent` 到底有哪些能力，`Hermes Operator` 必须逐项复刻哪些内容，以及这些内容做到什么程度才算真正等价。**

## 2. 顶层规则

### 2.1 代码规则

- 不复用 `repos/hermes-agent` 任何现有代码
- 只复用其功能、行为、交互和配置思路
- 所有实现必须用 `Rust + Tauri` 体系重新完成

### 2.2 范围规则

- 本文档中的所有条目都属于**必须进入范围**
- 可以按阶段实现
- 但不允许把任何条目标记为“永久不做”

### 2.3 等价规则

“一笔一笔复刻”不等于只做名字相似的功能，而是要求至少达到以下三层：

1. **功能等价**
   - 用户可以完成同类任务
2. **行为等价**
   - 命令、交互、状态流转、恢复逻辑尽量一致
3. **持久化等价**
   - 相关配置、会话、记忆、计划、运行状态能被保存与恢复

对于桌面软件操作能力，还要额外满足：

4. **TuriX 超越目标**
   - 成功率
   - 稳定性
   - 长链任务能力
   - 失败恢复能力
   - 中文办公软件适配
   - 安全审批与可控性
   - 执行速度

## 3. 参考来源

当前矩阵基于以下公开材料抽取：

- `repos/hermes-agent/README.md`
- `repos/hermes-agent/website/docs/getting-started/*`
- `repos/hermes-agent/website/docs/user-guide/*`
- `repos/hermes-agent/website/docs/reference/*`
- `repos/hermes-agent/website/docs/guides/*`
- `repos/hermes-agent/website/docs/developer-guide/*`

这些文档描述的是行为面。真正实现时，仍需继续读代码做细化补完。

## 4. 复刻分层

为了方便实现，把所有 Hermes 功能拆成四层：

- `Layer A`：基础入口与核心运行
- `Layer B`：使用中频能力
- `Layer C`：高级平台能力
- `Layer D`：研究与扩展能力

四层都必须做，只是实现顺序不同。

## 5. Feature Parity Matrix

| Domain | Hermes 功能面 | 必须复刻的具体点 | 等价要求 | 推荐阶段 |
| --- | --- | --- | --- | --- |
| A1 | 安装与初始化 | 安装、首次启动、配置目录初始化、升级、诊断 | 能完成首次可用、升级、诊断与修复建议 | Phase 1 |
| A2 | 核心 CLI/TUI | 交互式聊天、单次查询、继续会话、恢复会话、带模型/Provider/Toolsets/Skills 启动 | 行为、参数入口、恢复流程等价 | Phase 1-2 |
| A3 | 会话生命周期 | 自动保存 session、resume、continue、按 ID/标题恢复、会话 recap | 会话持久化与恢复行为等价 | Phase 2 |
| A4 | Slash Commands | `/help`、`/model`、`/tools`、`/skills`、`/voice`、`/title`、`/background` 等命令体系 | 命令体系和交互路径尽量等价 | Phase 2-3 |
| A5 | 基础配置系统 | `config.yaml`、`.env`、OAuth/Auth、优先级覆盖、命令配置修改 | 配置结构与覆盖优先级等价 | Phase 1-2 |
| A6 | Provider / Model 管理 | 多模型提供商、provider 切换、自定义 endpoint、模型选择 | 使用体验与配置面等价 | Phase 2 |
| A7 | Terminal Backends | local、docker、ssh、modal、daytona、singularity 类后端能力 | 行为面分阶段等价；云后端可后置但必须进入范围 | Phase 3-5 |
| A8 | Tool / Toolset System | 工具发现、启停、工具集配置、工具说明与限制 | 工具/工具集作为一等能力存在 | Phase 2-3 |
| B1 | Skills System | skills list/search/install/browse/use/config/create，skills 自动变 slash commands | 技能加载与使用模型等价 | Phase 3 |
| B2 | Persistent Memory | MEMORY/USER、长期记忆、用户偏好、历史决策保留 | 至少达到功能与持久化等价 | Phase 3 |
| B3 | Self-improving Loop | 任务后技能生成、技能自我改进、记忆提示/沉淀 | 必须进入范围；可分阶段递进实现 | Phase 4-5 |
| B4 | Session Search | 全文检索、跨会话搜索、摘要辅助召回 | 检索与召回路径等价 | Phase 3 |
| B5 | Personalities / Profiles | personality、profiles、title、session naming | 对话人格与会话身份机制等价 | Phase 2-3 |
| B6 | Worktrees / Parallel Work | worktree 模式、背景任务、隔离会话、并行工作流 | 能力与任务路径等价 | Phase 4 |
| B7 | Checkpoints / Rollback | 会话检查点、恢复、回滚能力 | 状态保存与恢复行为等价 | Phase 4 |
| C1 | Messaging Gateway | Telegram、Discord、Slack、WhatsApp、Signal、Email 等多渠道入口 | 多渠道架构和至少核心渠道的行为等价 | Phase 4-5 |
| C2 | Voice Mode | CLI 录音、语音转文字、TTS、消息平台语音能力 | 交互路径与核心能力等价 | Phase 4-5 |
| C3 | MCP Integration | MCP server 配置、工具过滤、resources/prompts 控制、reload | 能力面和安全控制等价 | Phase 4 |
| C4 | Cron Scheduling | 自然语言计划、cron 表达式、任务创建/编辑/暂停/立即运行、deliver 渠道 | 调度与交付行为等价 | Phase 4-5 |
| C5 | Security Model | 审批、容器隔离、远程执行边界、权限控制、审计 | 安全模型不能弱于 Hermes | Phase 3-5 |
| C6 | Context Files | `SOUL.md`、项目上下文文件、system prompt 拼装影响 | 项目/身份上下文能力等价 | Phase 3 |
| C7 | Optional Skills / Catalog | 可选技能目录、官方技能、搜索、安装和发现 | 技能目录与扩展机制进入范围 | Phase 4-5 |
| D1 | ACP / Editor Integration | ACP 服务、编辑器接入 | 接口与接入能力进入范围 | Phase 5 |
| D2 | Python Library / Programmatic Control | 以编程方式调用 Agent 的能力 | 行为和控制面进入范围 | Phase 5 |
| D3 | RL / Research Features | 轨迹生成、trajectory compression、Atropos/Tinker 相关能力 | 进入范围，但后置到研究层 | Phase 5+ |
| D4 | Migration Utilities | 从 OpenClaw 等环境迁移配置、技能、记忆、命令允许列表 | 工具链与迁移能力进入范围 | Phase 5 |

## 6. 按能力域展开的实现清单

## 6.1 安装、启动、更新、诊断

必须复刻：

- 首次启动即可完成配置引导
- `setup` 类初始化流
- `update`
- `doctor`
- 配置检查与配置迁移
- 运行时环境诊断

验收标准：

- 新用户可以从零到可用完成完整初始化
- 老版本配置可以被迁移
- 常见错误能被明确诊断而不是静默失败

## 6.2 CLI/TUI 体验

必须复刻：

- 真正的终端交互界面，不是假的输入框
- 多行输入
- slash command 自动补全
- 会话历史展示
- 中断并重定向当前任务
- 流式工具输出
- resume / continue 机制

验收标准：

- 用户可以只使用 CLI 完成 Hermes 等价工作流
- 终端体验不能退化成“桌面 UI 上的一层伪终端”

## 6.3 Slash Commands 体系

必须复刻：

- 内建命令
- 动态技能命令
- 自定义 quick commands
- 配置与模型切换命令
- session title / personality / tool listing / usage 等元命令

验收标准：

- 命令体系可扩展
- 新技能自动挂接为命令

## 6.4 Session Persistence

必须复刻：

- 会话自动保存
- resume by latest / id / title
- session title
- session recap
- source tagging
- session search

验收标准：

- 会话不因退出而丢失
- 用户能用标题与 ID 找回过去的工作

## 6.5 配置系统

必须复刻：

- `config.yaml`
- `.env`
- 配置优先级
- `config set/get/edit/check/migrate`
- provider / model / terminal / skill 相关配置

验收标准：

- 配置修改不需要手改多个文件
- secrets 与非 secrets 有清晰边界

## 6.6 Provider / Model 能力

必须复刻：

- 多 provider
- model 切换
- custom endpoint
- provider 级配置
- reasoning / context 相关控制

验收标准：

- 用户可以在不改代码的情况下切换模型提供商

## 6.7 Tool / Toolset 能力

必须复刻：

- 工具注册
- 工具启停
- toolsets 分组
- 工具说明
- 工具可见性与可用性
- 多执行后端

验收标准：

- 工具不是散落功能，而是结构化系统

## 6.8 Skills 能力

必须复刻：

- bundled skills
- skills list / search / install / browse
- 动态加载 skill 内容
- skill config
- custom skills
- optional skill catalog

验收标准：

- skill 不只是文档，而是可被 Agent 真正装载与调用的能力单元

## 6.9 Memory + Self-improvement

必须复刻：

- 长期记忆
- 用户模型
- 历史搜索
- 自动沉淀
- 任务后技能生成
- 技能自我改进

验收标准：

- 不能只做“会话历史列表”
- 必须体现持续学习与持续个性化

## 6.10 Messaging Gateway

必须复刻：

- 网关进程
- 多渠道会话连续性
- 至少核心渠道优先落地
- 每个渠道的 session source 标注

验收标准：

- CLI 不是唯一入口
- 同一 Agent 能跨渠道连续工作

## 6.11 Voice

必须复刻：

- CLI 录音入口
- 语音转文本
- TTS
- 消息平台语音支持

当前 Hermes Desktop 落地状态：

- 已实现本地 text-only Voice workflow：`local-text-capture` 记录手动输入 transcript，`local-speak-queue` 持久化待播报文本队列。
- 已提供 `voice_list_providers` catalog，并明确 `supports_audio_input=false`、`supports_audio_output=false`。
- 旧 `voice_*_stub` 命令仅作为 compatibility wrapper 保留，新 UI 使用非 stub 命令。
- 未实现真实麦克风录音、音频 STT、音频 TTS 或消息平台语音附件，因此本项仍不是 Hermes CLI Voice Mode 的完整 parity。

验收标准：

- 语音不是 demo，而是可用工作流

## 6.12 MCP

必须复刻：

- MCP server 配置
- include / exclude
- prompts / resources 开关
- reload
- 安全控制

验收标准：

- MCP 接入不是简单“能连”，而是可治理

## 6.13 Cron

必须复刻：

- 自然语言创建任务
- cron 表达式
- create / edit / pause / run now / list
- script + prompt 组合
- 多 deliver 通道

验收标准：

- 计划任务必须能真正无人值守运行

## 6.14 Security

必须复刻：

- 审批机制
- 执行隔离
- 高风险动作控制
- 渠道权限控制
- 日志与审计

验收标准：

- 安全模型不能因为桌面化而变弱

## 6.15 Editor / ACP / Programmatic Control

必须复刻：

- ACP 接入
- 程序式调用能力
- 可嵌入编辑器环境

验收标准：

- 不只是 GUI 产品，还能作为更大生态的一部分被调用

## 6.16 Research / RL / Data Generation

必须复刻：

- trajectory 生成
- trajectory 压缩
- RL / env 相关工作流

验收标准：

- 这些能力可以后置，但不能被从范围中删掉

## 7. 桌面操作能力的超越目标

由于 `TuriX-CUA` 是重点 benchmark，`Hermes Operator` 在桌面软件操作域必须设立单独质量门槛。

### 7.1 必须全面优化的维度

- 任务成功率
- 跨应用覆盖范围
- 长链任务稳定性
- 执行速度
- 失败恢复能力
- 中文办公软件适配
- 安全审批与可控性

### 7.2 文档约束

后续所有实施计划都必须显式回答：

1. 这一阶段复刻了 Hermes 的哪些具体功能？
2. 这一阶段让桌面操作能力在哪些维度逼近或超越 TuriX？

## 8. 实施要求

从现在开始，所有实施计划都必须引用本矩阵，不允许再写模糊表述，例如：

- “实现 Hermes 风格能力”
- “兼容类似 Hermes 的功能”
- “支持类 TuriX 的桌面操作”

必须改写成明确条目，例如：

- “实现 session resume by title”
- “实现 slash command autocomplete”
- “实现 cron create/edit/pause/run-now”
- “实现跨应用桌面操作步骤恢复”

## 9. 结论

`Hermes Operator` 不是一个“受 Hermes 启发的桌面 Agent”，而是：

**一个用 Rust + Tauri 绿地重写、对 `hermes-agent` 做功能与行为逐项复刻，并把 TuriX 作为桌面操作 benchmark 且目标全面超越的产品。**


## 2026-04-28 Capability Boundary Update

- Implemented locally/provider-backed: remote skill marketplace manifest list/install, GUI automation macro over allowlisted desktop actions, external SaaS simulation adapter (`local_echo` / confirmed `http_json`), high-fidelity deterministic world sandbox, local tabular RL training over trajectory JSONL.
- Still external-dependent: paid SaaS accounts, production endpoints, OS desktop permissions, non-allowlisted executors, high-fidelity 3D/benchmark sandboxes, large-scale model/RL training infrastructure.

## 2026-04-29 Future Remote User Routing Boundary

- Implemented locally: Agent Exchange Future Remote Users directory, `remote_user_id` message/bundle scoping, downloadable bundle JSON, bundle-carried remote user profiles, legacy bundle/mailbox compatibility, and local marketplace / GUI automation / simulation / RL records plus evidence exports with `target_remote_user_id`.
- Implemented local filters: marketplace install history, runtime adapter audit list/export, External SaaS run history, High-Fidelity sandbox run history, and local RL job history can filter by `target_remote_user_id`; these are local history filters only.
- Implemented local handoff/review exports: Marketplace audit, Runtime adapter audit handoff, Simulation capability evidence, and Local RL artifact exports can include `target_remote_user_profile` snapshots when the target is selected from local Agent Exchange. Runtime adapter handoff, Agent Exchange bundle, Native CUA audit payload, and TuriX bridge audit payload can be downloaded locally for review or out-of-band transfer, but these files are not remote delivery receipts.
- Still external-dependent: real remote user accounts, remote agent discovery, realtime cross-user transport, remote delivery receipts, remote marketplace account activity, and large-scale remote RLHF infrastructure.
