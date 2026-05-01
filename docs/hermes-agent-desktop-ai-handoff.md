# Hermes Operator AI 开工总览

## 1. 目的

本文档是给 AI 编程工具的总入口文档。它回答五个问题：

1. 先看哪些文档。
2. 这个桌面产品最终要长成什么结构。
3. 现有 `hermes-desktop` 壳子能参考什么、必须重写什么。
4. 第一阶段应该从哪里开始动手。
5. 每一阶段做到什么算完成。

如果你是 AI 编程工具，默认按本文档指定的阅读顺序执行，不要跳过架构文档直接开始堆功能。

## 2. 命名决策

最终产品名采用：

`Hermes Operator`

命名理由：

- `Hermes` 直接继承 `hermes-agent` 的产品认知，但不表示复用其现有代码。
- `Operator` 明确传达“操作桌面软件”和“代用户执行任务”是产品的一等公民能力。
- 相比 `Hermes Desktop`，`Hermes Operator` 更少“桌面壳工具”的感觉，更强“桌面执行系统”的定位。

工作区内现有文档文件名暂时仍沿用 `hermes-agent-desktop-*` 命名，避免一次性重命名所有文件导致引用失效；后续如需要，可统一做文件名迁移。

## 3. 推荐阅读顺序

1. [产品设计文档](/home/xiedex/code/hermes-agent_rl/docs/hermes-agent-desktop-product-design.md)
2. [Hermes 功能复刻矩阵](/home/xiedex/code/hermes-agent_rl/docs/hermes-operator-hermes-feature-parity-matrix.md)
3. [功能设计文档](/home/xiedex/code/hermes-agent_rl/docs/hermes-agent-desktop-functional-design.md)
4. [技术架构规格](/home/xiedex/code/hermes-agent_rl/docs/hermes-agent-desktop-technical-architecture.md)
5. [领域模型与契约文档](/home/xiedex/code/hermes-agent_rl/docs/hermes-agent-desktop-domain-contracts.md)
6. [界面与交互规格](/home/xiedex/code/hermes-agent_rl/docs/hermes-agent-desktop-ui-spec.md)
7. [交付路线图](/home/xiedex/code/hermes-agent_rl/docs/hermes-agent-desktop-delivery-plan.md)
8. [Phase 1 实施计划](/home/xiedex/code/hermes-agent_rl/docs/hermes-agent-desktop-phase-1-implementation-plan.md)

## 4. 当前工作区基线

当前工作区中，`hermes-desktop` 已存在一个非常轻量的桌面壳：

- Tauri 2 + Rust 主进程
- `src/backend/` 下已有基础模块
  - `env.rs`
  - `installer.rs`
  - `config.rs`
  - `hermes.rs`
- `src/frontend/index.html` 是单文件静态页面
- 目前只覆盖：
  - 环境检测
  - Hermes 安装/卸载/升级
  - Hermes 启停
  - 简单配置读取/保存

这意味着：

- 当前工作区已经从最初 demo 壳演进为结构化 Rust + Tauri + React 应用。
- Mission、本地 SQLite、Knowledge、Memory、Council、Execution、Simulation、Skills/Growth、Voice、Gateway/Parity 等核心表面已经具备可测试实现。
- Simulation Sandbox 已具备 Mission 级 scenario runs、变量权重滑块、option cards、recommendation rationale、comparison matrix、path evolution，以及可配置、可复用模板化的 Council / Execution review handoff。
- 仍应把外部 GUI 自动化、多 Agent 数字沙盘、真实外部仿真引擎和更深 RL/trajectory 能力视为后续大型集成，而不是 Phase 1/5 的未闭合缺口。

## 5. 对 AI 编程工具的硬性要求

### 5.1 技术栈强制约束

桌面端技术栈固定为：

- `Rust`
- `Tauri 2`

解释：

- `Rust` 是桌面主进程、系统能力层、状态层和本地数据层的唯一主语言。
- `Tauri 2` 是唯一桌面壳方案。
- 不允许把桌面壳替换为 Electron、Flutter、Qt、Neutralino 或其他方案。

补充说明：

- Tauri 的前端视图层仍可采用 Web UI，但它只是 Tauri 内部界面层，不改变“桌面技术栈 = Rust + Tauri”的决策。

### 5.2 绿地重写约束

`repos/` 下所有项目都只作为参考样本，不复用任何现有代码。

其中：

- `hermes-agent` 是功能与行为等价基线
- `TuriX-CUA` 是桌面软件操作 benchmark
- 其他项目是增强能力参考

### 5.3 先打地基，再做功能

禁止一上来就做 Mission 页面或复杂工作流。必须先完成：

- 前端工程化
- Rust 应用状态层
- 本地存储层
- IPC 契约层
- Rust Agent Core 管理层

### 5.4 前端不得直接访问文件系统和执行命令

所有系统能力必须经过：

`Frontend -> Tauri Command -> Rust Service -> Rust Agent Core`

禁止在前端写任何“直接操作系统”的逻辑。

### 5.5 领域对象先定义，再接 UI

先定义：

- `Mission`
- `Run`
- `Artifact`
- `KnowledgeSource`
- `MemoryRecord`
- `ScenarioRun`
- `CouncilStep`
- `ExecutionStep`

再做页面绑定。

### 5.6 优先做可替换的受控边界

对外部能力的首版接入，优先用稳定契约、受控 adapter 和明确能力标签：

- Rust Agent Core 可以先提供保守本地响应，但不得把示例/空态写成真实智能能力。
- Knowledge search 可以先查本地 SQLite + 文件索引，并明确轻量 chunking 不等同于完整语义 RAG。
- Scenario sandbox 可以先做文本化策略比较；当前本地闭环已扩展为 deterministic Local Multi-Agent Sandbox、External SaaS provider adapter（local_echo / confirmed http_json）和 High-Fidelity Local Sandbox world model；真实 SaaS 仍需要 endpoint/凭证/网络权限。

先跑通链路，再接入真实复杂能力；所有示例、dry-run、preview 和 handoff 都必须在 UI/文档中标清边界。

### 5.7 TuriX 超越目标

桌面软件操作能力不是“达到 TuriX 水平”就结束，而是要把 TuriX 作为重点 benchmark，并在以下维度持续超越：

- 任务成功率
- 跨应用覆盖
- 长链任务稳定性
- 失败恢复能力
- 中文办公软件适配
- 安全审批与可控性
- 执行速度

### 5.8 GUI 自动化最后接入

执行层优先级固定：

1. API
2. CLI
3. Browser automation
4. Desktop GUI automation

只有前三者无法完成时，才使用桌面 GUI 自动化。

说明：

- 在产品定义层，桌面软件操作是第一等公民能力，必须直接出现在 IA、页面结构和任务主链中。
- 在执行引擎层，依然优先选择更稳定的 API/CLI/Browser 路径，这是实现策略，不是产品降级。

## 6. 目标目录结构

AI 工具默认将 `hermes-desktop` 重构到如下结构：

```text
hermes-desktop/
  src/
    main.rs
    lib.rs
    commands/
      mod.rs
      app.rs
      settings.rs
      runtime.rs
      mission.rs
      knowledge.rs
      memory.rs
    backend/
      mod.rs
      app_state.rs
      errors.rs
      env.rs
      installer.rs
      agent_core/
        mod.rs
        process_manager.rs
        engine_service.rs
      domain/
        mod.rs
        app.rs
        settings.rs
        mission.rs
        knowledge.rs
        memory.rs
        scenario.rs
        council.rs
        execution.rs
      services/
        mod.rs
        bootstrap_service.rs
        settings_service.rs
        mission_service.rs
        knowledge_service.rs
        memory_service.rs
        scenario_service.rs
        council_service.rs
        execution_service.rs
      storage/
        mod.rs
        sqlite.rs
        migrations.rs
        repositories/
          mod.rs
          mission_repo.rs
          knowledge_repo.rs
          memory_repo.rs
      adapters/
        mod.rs
        filesystem.rs
        notifications.rs
        shell_runner.rs
        agent_engine.rs
  ui/
    package.json
    tsconfig.json
    vite.config.ts
    src/
      main.tsx
      app/
      routes/
      components/
      features/
      hooks/
      lib/
      store/
      styles/
  tests/
    integration/
    fixtures/
```

## 7. 文档和实现的对应关系

| 文档 | 作用 | AI 工具使用方式 |
| --- | --- | --- |
| 产品设计 | 解释产品为什么这样定义 | 用于守住目标用户和边界 |
| 功能设计 | 解释系统包含哪些模块 | 用于避免漏模块和功能越界 |
| 技术架构 | 解释模块边界和运行拓扑 | 用于定结构、定进程、定责任 |
| 领域与契约 | 解释数据模型和 IPC/Agent Engine 契约 | 用于先定义接口再写实现 |
| UI 规格 | 解释页面、组件、状态和交互 | 用于生成 UI 和状态管理 |
| 交付路线图 | 解释分阶段交付顺序 | 用于拆分任务和里程碑 |
| Phase 1 计划 | 解释第一批代码如何落地 | 用于直接开工 |

## 8. 分阶段启动策略

### Phase 1

目标：把当前静态壳升级为可持续开发的桌面应用骨架。

必须交付：

- React + TypeScript 前端工程
- Rust 应用状态和统一命令层
- SQLite 初始化
- 设置页、运行时页、Home 页
- Rust Agent Core 管理边界与保守本地响应

### Phase 2

目标：建立 Mission 主工作流和桌面操作主通路。

必须交付：

- Mission CRUD
- Mission 列表与详情
- Timeline
- Artifact 基础模型
- 基础 Run 记录
- 基础桌面操作任务模型
- Operate 面板基础页

### Phase 3

目标：建立 Knowledge + Memory。

必须交付：

- 本地文件导入
- 文档索引
- 检索结果页
- 记忆召回

### Phase 4

目标：建立 Council + Execution。

必须交付：

- 计划生成
- 审议流
- 执行计划审批
- CLI / Browser adapter

### Phase 5

目标：建立 Scenario + Growth。

必须交付：

- 多方案推演
- Playbook 建议
- Learning card
- 任务复盘资产化

## 9. Definition of Done

AI 工具在任何阶段都不得用“页面能打开”作为完成标准。阶段完成必须同时满足：

- 数据模型稳定
- 命令契约可调用
- 至少一条主路径打通
- 有测试
- 有错误态
- 有空态
- 有日志
- 有文档更新

## 10. 开工顺序结论

如果你要立即开始，请直接按以下顺序操作：

1. 阅读 [技术架构规格](/home/xiedex/code/hermes-agent_rl/docs/hermes-agent-desktop-technical-architecture.md)
2. 阅读 [领域模型与契约文档](/home/xiedex/code/hermes-agent_rl/docs/hermes-agent-desktop-domain-contracts.md)
3. 阅读 [Phase 1 实施计划](/home/xiedex/code/hermes-agent_rl/docs/hermes-agent-desktop-phase-1-implementation-plan.md)
4. 只做 Phase 1，不提前偷跑 Phase 2

这套顺序的目的，是确保这个产品从第一行代码开始就是“以 `hermes-agent` 为脑、以桌面软件操作为主战场”的系统，而不是先写一堆页面，再回头重做基础设施。


## 2026-04-28 Capability Boundary Update

- Implemented locally/provider-backed: remote skill marketplace manifest list/install, GUI automation macro over allowlisted desktop actions, external SaaS simulation adapter (`local_echo` / confirmed `http_json`), high-fidelity deterministic world sandbox, local tabular RL training over trajectory JSONL.
- Still external-dependent: paid SaaS accounts, production endpoints, OS desktop permissions, non-allowlisted executors, high-fidelity 3D/benchmark sandboxes, large-scale model/RL training infrastructure.

## 2026-04-29 Verification Handoff Update

- `hermes-desktop/ui/package.json` now exposes `npm test` for Node native UI tests.
- `hermes-desktop/ui/tests/capabilityContracts.test.mjs` statically checks that the new capability wrappers invoke registered Rust Tauri command names and that the new UI surface does not export legacy `voice*Stub` wrappers.
- Continue treating backend compatibility `voice_*_stub` commands as legacy wrappers only; new UI should call `voice_transcribe` / `voice_speak` through non-stub client functions.
- Current full local verification set is Rust tests + lib/all-target Clippy + UI `npm test` + UI typecheck/build.

## 2026-04-29 Future Remote User Handoff Update

- Agent Exchange now has a local Future Remote Users directory for future remote-account routing readiness. The directory stores user id, display name, default remote agent id, transport label, route hint, status, and timestamps.
- `AgentExchangeBundle` carries `remote_users`; old bundles and persisted mailboxes without that field deserialize as an empty directory. Import merges profiles by `user_id` and keeps the newest `updated_at` profile.
- `remote_user_id` can scope Agent Exchange message list/export. Export includes profiles referenced by exported messages and an explicitly requested remote user profile even when no message is in scope, and the UI can download the scoped local bundle JSON for out-of-band handoff.
- `target_remote_user_id` is now persisted through local marketplace install request/result/history, GUI automation request/response/audit, External SaaS and High-Fidelity simulation run payload/history, and local RL training job/artifact exports.
- Marketplace install history, runtime adapter audit list/export, External SaaS run history, High-Fidelity sandbox run history, and local RL training job history can be filtered by `target_remote_user_id`. These are local history filters only.
- Marketplace audit export, runtime adapter audit handoff export/download, Simulation capability evidence export/download, and local RL artifact export/download can include `target_remote_user_profile` snapshots when the target comes from the local Agent Exchange directory. Native CUA and TuriX bridge audit exports can also be downloaded as local review payloads, but those payloads remain raw local audit exports rather than future remote-user handoff envelopes.
- Treat every `remote_user_id`, `target_remote_user_id`, and `target_remote_user_profile` field as future routing metadata only; none of them are evidence of remote account activity, remote delivery, remote GUI execution, benchmark quality, or remote RLHF infrastructure.
