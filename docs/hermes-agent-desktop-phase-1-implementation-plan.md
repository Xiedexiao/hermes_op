# Hermes Operator Phase 1 Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把当前 `hermes-desktop` 从静态 demo 壳升级成可持续开发的桌面应用基础设施，为后续 Mission、Operate、Knowledge、Council 和 Scenario 模块提供稳定地基。

**Architecture:** 桌面端技术栈固定为 Tauri 2 + Rust 主进程，新增结构化 `commands/services/storage` 边界，并把 Tauri 内嵌视图层升级到 React + TypeScript + Vite。首阶段只交付壳、状态、设置、运行时和本地 Rust Agent Core 原型，不做 Mission 业务。

**Tech Stack:** Tauri 2, Rust, React, TypeScript, Vite, Zustand, TanStack Query, SQLite

**Stack Constraint:** Desktop shell and desktop runtime must remain `Rust + Tauri`. Do not substitute Electron, Flutter, or any other desktop container.

---

## File Structure

### 保留文件

- `hermes-desktop/src/main.rs`
- `hermes-desktop/src/lib.rs`
- `hermes-desktop/src/backend/env.rs`
- `hermes-desktop/src/backend/installer.rs`

### 需要重构或新增的 Rust 文件

- Create: `hermes-desktop/src/commands/mod.rs`
- Create: `hermes-desktop/src/commands/app.rs`
- Create: `hermes-desktop/src/commands/settings.rs`
- Create: `hermes-desktop/src/commands/runtime.rs`
- Create: `hermes-desktop/src/backend/app_state.rs`
- Create: `hermes-desktop/src/backend/errors.rs`
- Create: `hermes-desktop/src/backend/storage/mod.rs`
- Create: `hermes-desktop/src/backend/storage/sqlite.rs`
- Create: `hermes-desktop/src/backend/storage/migrations.rs`
- Create: `hermes-desktop/src/backend/agent_core/mod.rs`
- Create: `hermes-desktop/src/backend/agent_core/process_manager.rs`
- Create: `hermes-desktop/src/backend/agent_core/engine_service.rs`
- Modify: `hermes-desktop/src/backend/mod.rs`
- Modify: `hermes-desktop/src/commands.rs`
- Modify: `hermes-desktop/src/main.rs`

### 需要新增的前端文件

- Create: `hermes-desktop/ui/package.json`
- Create: `hermes-desktop/ui/tsconfig.json`
- Create: `hermes-desktop/ui/vite.config.ts`
- Create: `hermes-desktop/ui/index.html`
- Create: `hermes-desktop/ui/src/main.tsx`
- Create: `hermes-desktop/ui/src/app/App.tsx`
- Create: `hermes-desktop/ui/src/app/AppShell.tsx`
- Create: `hermes-desktop/ui/src/routes/HomePage.tsx`
- Create: `hermes-desktop/ui/src/routes/SettingsPage.tsx`
- Create: `hermes-desktop/ui/src/routes/RuntimePage.tsx`
- Create: `hermes-desktop/ui/src/components/SidebarNav.tsx`
- Create: `hermes-desktop/ui/src/components/TopBar.tsx`
- Create: `hermes-desktop/ui/src/components/StatusBadge.tsx`
- Create: `hermes-desktop/ui/src/lib/tauri.ts`
- Create: `hermes-desktop/ui/src/store/appStore.ts`
- Create: `hermes-desktop/ui/src/store/runtimeStore.ts`
- Create: `hermes-desktop/ui/src/styles/global.css`

### 测试文件

- Create: `hermes-desktop/src/backend/storage/sqlite_tests.rs`
- Create: `hermes-desktop/src/backend/agent_core/engine_service_tests.rs`
- Create: `hermes-desktop/ui/src/lib/tauri.test.ts`

## Task 1: 前端工程化迁移

**Files:**

- Create: `hermes-desktop/ui/package.json`
- Create: `hermes-desktop/ui/tsconfig.json`
- Create: `hermes-desktop/ui/vite.config.ts`
- Create: `hermes-desktop/ui/index.html`
- Create: `hermes-desktop/ui/src/main.tsx`
- Create: `hermes-desktop/ui/src/app/App.tsx`
- Create: `hermes-desktop/ui/src/styles/global.css`
- Modify: `hermes-desktop/tauri.conf.json`

- [ ] **Step 1: 创建 Vite + React 前端骨架**

写入最小前端入口结构，确保应用可以被 Tauri 加载。

- [ ] **Step 2: 修改 `tauri.conf.json` 指向新的前端构建目录**

目标改为开发时走前端 dev server，生产时走 `ui/dist`。

- [ ] **Step 3: 写最小 App 组件**

要求页面只展示：

- 应用标题
- “Home / Runtime / Settings” 三个导航
- 一个基础内容区

- [ ] **Step 4: 本地运行前端**

Run: `cd hermes-desktop/ui && npm install && npm run dev`

Expected: Vite 正常启动，无 TypeScript 报错。

- [ ] **Step 5: 提交前端工程骨架**

```bash
git add hermes-desktop/ui hermes-desktop/tauri.conf.json
git commit -m "feat: scaffold React frontend for Hermes Operator"
```

## Task 2: 建立 Rust 应用状态与错误模型

**Files:**

- Create: `hermes-desktop/src/backend/app_state.rs`
- Create: `hermes-desktop/src/backend/errors.rs`
- Modify: `hermes-desktop/src/backend/mod.rs`
- Modify: `hermes-desktop/src/lib.rs`

- [ ] **Step 1: 定义统一 `AppError`**

错误类型至少包含：

- `Validation`
- `Storage`
- `Runtime`
- `HermesRuntime`
- `Io`
- `Unknown`

- [ ] **Step 2: 定义 `AppState`**

至少包含：

- sqlite 路径
- 配置目录路径
- logs 路径
- Agent Core 运行态

- [ ] **Step 3: 在 `lib.rs` 导出状态和错误模块**

要求后续 command 可以共享这些类型。

- [ ] **Step 4: `cargo check`**

Run: `cd hermes-desktop && cargo check`

Expected: 编译通过。

- [ ] **Step 5: 提交 Rust 状态骨架**

```bash
git add hermes-desktop/src/backend hermes-desktop/src/lib.rs
git commit -m "feat: add app state and error model"
```

## Task 3: 建立 SQLite 初始化层

**Files:**

- Create: `hermes-desktop/src/backend/storage/mod.rs`
- Create: `hermes-desktop/src/backend/storage/sqlite.rs`
- Create: `hermes-desktop/src/backend/storage/migrations.rs`
- Create: `hermes-desktop/src/backend/storage/sqlite_tests.rs`
- Modify: `hermes-desktop/Cargo.toml`

- [ ] **Step 1: 引入 SQLite 依赖**

在 `Cargo.toml` 中加入本地 sqlite 所需依赖。

- [ ] **Step 2: 编写数据库初始化逻辑**

要求首次启动自动创建：

- `app_settings`
- `missions`
- `runs`
- `run_events`

当前只要求建表，不要求全部业务接入。

- [ ] **Step 3: 写初始化测试**

测试目标：

- 临时数据库可创建
- 核心表存在

- [ ] **Step 4: 运行 Rust 测试**

Run: `cd hermes-desktop && cargo test sqlite`

Expected: sqlite 初始化相关测试通过。

- [ ] **Step 5: 提交存储层**

```bash
git add hermes-desktop/src/backend/storage hermes-desktop/Cargo.toml hermes-desktop/Cargo.lock
git commit -m "feat: add sqlite bootstrap layer"
```

## Task 4: 拆分命令层并实现 bootstrap/settings/runtime 契约

**Files:**

- Create: `hermes-desktop/src/commands/mod.rs`
- Create: `hermes-desktop/src/commands/app.rs`
- Create: `hermes-desktop/src/commands/settings.rs`
- Create: `hermes-desktop/src/commands/runtime.rs`
- Modify: `hermes-desktop/src/commands.rs`
- Modify: `hermes-desktop/src/main.rs`

- [ ] **Step 1: 创建 `app_get_bootstrap` 命令**

要求返回：

- app settings
- runtime settings
- Hermes status
- runtime status
- summary payload

- [ ] **Step 2: 重构现有 `load_config/save_config` 为 settings 命令**

要求命令名与文档契约一致。

- [ ] **Step 3: 增加 `runtime_get_status/start/stop/restart`**

当前 Agent Core 可以先返回原型状态。

- [ ] **Step 4: 在 `main.rs` 注册新命令**

保留已有命令的兼容性时，可增加过渡层，但 UI 只消费新命令。

- [ ] **Step 5: `cargo check` 验证命令层**

Run: `cd hermes-desktop && cargo check`

Expected: 无命令注册错误。

- [ ] **Step 6: 提交命令层**

```bash
git add hermes-desktop/src/commands hermes-desktop/src/main.rs
git commit -m "feat: add structured tauri command modules"
```

## Task 5: 建立本地 Rust Agent Core 原型

**Files:**

- Create: `hermes-desktop/src/backend/agent_core/mod.rs`
- Create: `hermes-desktop/src/backend/agent_core/process_manager.rs`
- Create: `hermes-desktop/src/backend/agent_core/engine_service.rs`
- Create: `hermes-desktop/src/backend/agent_core/engine_service_tests.rs`

- [ ] **Step 1: 定义 `AgentEngineStatus` 结构**

字段至少包含：

- running
- pid
- profile
- lastError

- [ ] **Step 2: 实现 Rust `ProcessManager` 原型**

首版可以只支持：

- 检查 runtime 目录中的 `engine.state` / `engine.lock` 文件
- 初始化本地 Agent Engine 进程状态
- 启动本地 Rust Agent Core 原型
- 停止本地 Rust Agent Core 原型

- [ ] **Step 3: 写最小本地 Rust Agent Core 原型逻辑**

要求：

- 在没有真实 Agent Core 能力时也能返回结构化状态
- 不依赖外部 `hermes-agent` 进程
- 通过 Rust service 暴露健康状态

- [ ] **Step 4: 写 runtime service 测试**

测试目标：

- 原型状态可返回
- 无 Agent Core 能力时不会 panic

- [ ] **Step 5: 提交本地 Rust Agent Core 原型**

```bash
git add hermes-desktop/src/backend/agent_core
git commit -m "feat: add local rust agent core prototype"
```

## Task 6: 实现 Home / Runtime / Settings 页面

**Files:**

- Create: `hermes-desktop/ui/src/app/AppShell.tsx`
- Create: `hermes-desktop/ui/src/routes/HomePage.tsx`
- Create: `hermes-desktop/ui/src/routes/RuntimePage.tsx`
- Create: `hermes-desktop/ui/src/routes/SettingsPage.tsx`
- Create: `hermes-desktop/ui/src/components/SidebarNav.tsx`
- Create: `hermes-desktop/ui/src/components/TopBar.tsx`
- Create: `hermes-desktop/ui/src/components/StatusBadge.tsx`
- Create: `hermes-desktop/ui/src/lib/tauri.ts`
- Create: `hermes-desktop/ui/src/store/appStore.ts`
- Create: `hermes-desktop/ui/src/store/runtimeStore.ts`
- Create: `hermes-desktop/ui/src/lib/tauri.test.ts`

- [ ] **Step 1: 建立 `tauri.ts` 调用封装**

必须封装：

- `appGetBootstrap`
- `settingsGet`
- `settingsSave`
- `runtimeGetStatus`
- `runtimeStartEngine`
- `runtimeStopEngine`

- [ ] **Step 2: 建立 AppShell 和三页面路由**

要求：

- 左侧导航
- 顶部 runtime 状态
- 页面可切换

- [ ] **Step 3: Home 页接入 bootstrap**

展示：

- Agent Core 状态
- Runtime 状态
- summary payload

- [ ] **Step 4: Runtime 页接入启停命令**

展示：

- 当前状态
- 启动、停止、重启按钮
- 错误提示区

- [ ] **Step 5: Settings 页接入配置读写**

展示：

- provider
- model
- baseUrl
- default workspace

- [ ] **Step 6: 写 `tauri.ts` 测试**

要求至少校验：

- 调用参数映射正确
- 响应结构解析正确

- [ ] **Step 7: 提交 Phase 1 UI**

```bash
git add hermes-desktop/ui
git commit -m "feat: add foundation pages for Hermes Operator"
```

## Task 7: 端到端联调与文档回写

**Files:**

- Modify: `docs/hermes-agent-desktop-ai-handoff.md`
- Modify: `docs/hermes-agent-desktop-delivery-plan.md`

- [ ] **Step 1: 联调应用启动**

Run: `cd hermes-desktop && cargo run`

Expected: 桌面壳正常打开，前端渲染成功。

- [ ] **Step 2: 联调 bootstrap/settings/runtime 三条主路径**

验证：

- Home 能看到 bootstrap 数据
- Settings 能保存并重新加载
- Runtime 能展示 Agent Core 原型状态

- [ ] **Step 3: 回写文档中的偏差**

若实际命令名、文件路径、端口管理方式与文档不同，必须立刻回写文档。

- [ ] **Step 4: 提交联调修正**

```bash
git add docs hermes-desktop
git commit -m "docs: align phase 1 implementation with foundation contracts"
```

## 完成判定

Phase 1 完成必须同时满足：

- React 前端替代静态 HTML 成功
- Rust 拥有 `commands/services/storage` 基本边界
- SQLite 初始化成功
- Home / Runtime / Settings 三页面可用
- 本地 Rust Agent Core 原型可启动或可正确返回状态
- 文档与代码一致

若以上任意一条未满足，则 Phase 1 不能宣称完成。
