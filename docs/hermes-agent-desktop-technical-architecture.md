# Hermes Operator 技术架构规格

## 1. 文档目标

本文档定义 `Hermes Operator` 的技术实现边界、运行拓扑、模块划分、存储策略和集成方式，供 AI 编程工具在正式写代码前统一结构认知。

## 2. 架构目标

系统必须同时满足以下目标：

- 桌面本地优先
- 桌面软件操作是一等公民
- 所有核心代码绿地重写
- 前后端边界清晰
- Hermes 功能等价能力可渐进实现
- 可离线开发和契约先行开发
- 后续可扩展到知识层、记忆层、推演层和执行层

## 3. 关键架构决策

### 3.0 桌面技术栈固定为 Rust + Tauri

这是硬约束，不是建议。

固定内容：

- 桌面主进程：`Rust`
- 桌面应用壳：`Tauri 2`

禁止替换为：

- Electron
- Flutter Desktop
- Qt
- Neutralino
- 其他非 Tauri 桌面壳方案

说明：

- Tauri 视图层可以使用 Web UI 技术，但不改变桌面技术栈选择。
- `Rust + Tauri` 的决策优先级高于任何后续实现偏好。

### 3.1 桌面壳继续采用 Tauri 2

原因：

- 当前已有 `hermes-desktop` 基础壳
- Rust 主进程适合做系统能力、进程管理、文件系统和数据库封装
- 包体和性能更适合桌面工具

### 3.2 前端升级为 Tauri 内嵌 Web UI

当前 `src/frontend/index.html` 只适合演示，不适合复杂工作台。必须升级为结构化的 Tauri 内嵌 Web UI。

建议前端视图层：

- React 19
- TypeScript
- Vite
- React Router
- Zustand
- TanStack Query
- 原生 CSS Variables + 模块化样式

不建议首版引入过重 UI 框架。

注意：

- 这里的 React/TypeScript/Vite 只是 Tauri 的 UI 层选择。
- 桌面技术栈本身仍然是 `Rust + Tauri`。

### 3.3 Rust 负责“壳 + 状态 + 系统能力 + 本地数据”

Rust 侧职责：

- 应用启动
- 命令暴露
- 系统托盘
- 通知
- SQLite
- 日志
- 进程管理
- Agent Engine 生命周期
- 本地文件与执行权限控制

### 3.4 Rust 绿地重写 Agent Core

桌面应用代码不接入 `hermes-agent` 现有运行时，也不自建 Python 中间层。

正确边界是：

- `Rust + Tauri` 构成产品自身的桌面技术栈
- Agent Core 由 Rust 从零实现
- `hermes-agent` 只作为功能与行为参考基线
- `TuriX-CUA` 只作为桌面软件操作 benchmark

Rust 不直接在 command 内堆大段业务逻辑，而是把能力实现沉入本地 Agent Core：

- 任务理解
- 工具编排
- 计划生成
- 执行调度
- 状态管理

## 4. 运行拓扑

```text
┌──────────────────────────────┐
│ React UI (desktop frontend)  │
└──────────────┬───────────────┘
               │ Tauri invoke
┌──────────────▼───────────────┐
│ Rust App Core                │
│ - commands                   │
│ - services                   │
│ - repositories               │
│ - app state                  │
│ - agent engine               │
└───────┬───────────┬──────────┘
        │           │
        │           └──────────────┐
        │                          │
┌───────▼────────┐        ┌────────▼────────────────┐
│ SQLite / FS    │        │ Rust Agent Core         │
│ - app.db       │        │ - planner               │
│ - artifacts/   │        │ - tool runner           │
│ - cache/       │        │ - state machine         │
│ - logs/        │        │ - operation engine      │
└────────────────┘        └─────────────────────────┘
```

## 5. 模块边界

## 5.1 Frontend

职责：

- 页面渲染
- 表单
- 状态展示
- 用户交互
- 命令调用
- 轮询与订阅

禁止职责：

- 直接读写本地文件
- 直接起子进程
- 直接连本地数据库
- 直接访问 Agent Core 内部实现

## 5.2 Rust Commands

职责：

- 输入校验
- 调服务层
- 统一返回 `Result<T, AppError>`
- 保持 IPC 契约稳定

禁止职责：

- 写大量业务逻辑
- 直接做复杂聚合
- 直接写 SQL

## 5.3 Services

职责：

- 业务编排
- 权限和状态判断
- 任务生命周期管理
- 多 repository / adapter 组合

## 5.4 Repositories

职责：

- 单一聚合根的数据读写
- 隐藏 SQL 细节

## 5.5 Adapters

职责：

- 文件系统
- 系统通知
- shell / CLI 执行
- Agent engine 封装

## 5.6 Agent Engine

职责：

- 任务理解
- 工具选择与编排
- 执行调度
- 状态查询
- 与本地日志/事件流对接

## 6. 本地存储布局

建议统一使用用户目录下的独立工作空间：

```text
~/.hermes-desktop/
  app.db
  config.json
  logs/
    app.log
    agent-engine.log
  artifacts/
    <mission-id>/
  cache/
    documents/
    search/
  knowledge/
    imports/
  runtime/
    engine.state
    engine.lock
```

### 6.1 存储原则

- 配置和业务数据分离
- 日志独立目录
- Artifact 按 Mission 隔离
- Agent Engine 状态文件独立

## 7. 目标工程结构

## 7.1 Rust

```text
src/
  commands/
  backend/
    domain/
    services/
    storage/
    adapters/
    hermes/
```

### 结构解释

- `commands/`: Tauri 命令入口
- `domain/`: 领域对象与枚举
- `services/`: 业务编排
- `storage/`: SQLite 初始化和仓储
- `adapters/`: 外部依赖与系统能力封装
- `agent_core/`: Rust Agent Core 管理与调用封装

## 7.2 Frontend

```text
ui/src/
  app/
  routes/
  features/
  components/
  lib/
  hooks/
  store/
  styles/
```

### 结构解释

- `app/`: 根入口和 layout
- `routes/`: 页面级组件
- `features/`: Mission、Knowledge、Memory 等业务模块
- `components/`: 共享组件
- `lib/`: invoke client、formatters、constants
- `store/`: Zustand store

## 8. 配置管理策略

配置分三层：

1. `AppSettings`
   - 桌面壳级配置
   - 主题、默认工作区、是否开机启动、日志级别

2. `RuntimeSettings`
   - 模型提供商、默认模型、Base URL、API Key 引用

3. `WorkspaceSettings`
   - 当前工作目录、默认知识集、是否启用记忆、执行审批级别

### 安全要求

- API Key 不应长期明文散落在多个文件
- 首版如未接入系统钥匙串，至少集中存储并单点访问
- UI 返回配置时需脱敏

## 9. 日志与可观测性

日志分三类：

1. `app.log`
   - UI 启动、Rust command、错误

2. `agent-engine.log`
   - Agent Core 启动、任务执行、外部引擎调用

3. `run event stream`
   - 每个 Mission/Run 的结构化事件流，存数据库

结构化事件最少包含：

- `event_id`
- `mission_id`
- `run_id`
- `event_type`
- `message`
- `payload_json`
- `created_at`

## 10. 错误处理策略

统一错误模型：

- `validation_error`
- `not_found`
- `conflict`
- `runtime_unavailable`
- `external_dependency_failed`
- `permission_denied`
- `unknown_error`

前端必须区分：

- 可重试错误
- 需要用户修正配置的错误
- 需要开发者排查的错误

## 11. 后台任务与并发

### 11.1 Rust 侧

Rust 负责：

- Agent Core 生命周期
- 本地轮询
- 结构化事件写入
- 简单异步任务
- 工具调用与执行调度

### 11.2 约束

- 同一个 Mission 同时只允许一个主 Run 处于 `running`
- Knowledge import 可以并发，但要有 job id
- 执行流必须带取消能力

## 12. 与参考项目的参考策略

### 12.1 `hermes-agent`

参考策略：

- 这是功能与行为复刻基线
- 不接入、不调用、不复用现有实现
- 逐项拆解其功能面、配置模型、交互路径和能力边界
- 用 Rust 重新实现等价能力

### 12.2 `mempalace`

参考策略：

- 首版先定义 Memory 接口
- 本地记忆可以先用自建 sqlite + 原文存储
- 后续再对接 MemPalace 高保真检索

### 12.3 `onyx`

参考策略：

- 吸收其知识层产品设计
- 首版不直接搬完整后端
- 用轻量本地知识导入 + 检索替代

### 12.4 `TuriX-CUA`

参考策略：

- 作为桌面软件操作能力的重点 benchmark
- 功能与行为参考，但不复用实现
- 在任务成功率、长链稳定性、失败恢复、覆盖面和中文办公软件适配上作为超越目标

### 12.5 `edict`

参考策略：

- 采用可视化 Council 思路
- 不照搬全部角色制度命名

### 12.6 `MiroFish`

参考策略：

- 先做文本化多方案推演
- 后续再做更深模拟

### 12.7 `DeepTutor`

参考策略：

- 采用统一 Mission thread
- 引入成长与复盘资产化思路

## 13. 非功能要求

### 13.1 可恢复性

- 应用重启后通过 bootstrap 恢复 pinned 优先、否则最近活跃的非终态 Mission
- Agent Engine 崩溃后可重启
- 未完成 Run 要保留状态和日志

### 13.2 可迁移性

- 业务数据尽量集中在 SQLite 和标准文件目录
- 不把核心状态分散在大量 JSON 文件中

### 13.3 可测试性

- service 层可独立测试
- repository 层可用临时 sqlite 测试
- Agent Engine 可集成测试
- frontend 命令调用可使用测试替身

## 14. 架构结论

这不是一个“单页 Tauri 小工具”，而是一个以 Rust 绿地重写的 Hermes 等价 Agent Core 为基础、并以超越 TuriX 为目标的本地桌面执行平台。技术架构必须从第一阶段就支持以下事实：

- 会有多条 Mission 并行
- 会有长期数据
- 会有本地 Agent Engine
- 会有复杂状态机
- 会有未来的 GUI automation 和推演能力

因此，最重要的实现原则是：

**壳、状态、存储、契约、Agent Core 边界先行，功能后置。**
