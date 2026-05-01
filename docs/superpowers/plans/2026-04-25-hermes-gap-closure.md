# Hermes Gap Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the highest-confidence missing or incomplete product surfaces in `hermes-desktop`: real engine runtime, real workspace diagnostics, real local voice workflow, and mission playbook/growth suggestions.

**Architecture:** Keep the scope inside the existing Rust + Tauri + React boundaries. Reuse current mission/run/execution/session storage instead of adding new dependencies or parallel data models. Replace incomplete UI blocks with real read models backed by fresh commands, and keep backward compatibility for existing exported voice/runtime surfaces where possible.

**Tech Stack:** Rust, Tauri commands, SQLite, React, TypeScript, Zustand

---

### Task 1: Real Engine Runtime And Diagnostics

**Files:**
- Modify: `hermes-desktop/src/backend/agent_core/mod.rs`
- Modify: `hermes-desktop/src/backend/agent_core/process_manager.rs`
- Create: `hermes-desktop/src/backend/agent_core/daemon.rs`
- Modify: `hermes-desktop/src/backend/mod.rs`
- Modify: `hermes-desktop/src/commands/app.rs`
- Modify: `hermes-desktop/src/commands/runtime.rs`
- Modify: `hermes-desktop/src/main.rs`
- Modify: `hermes-desktop/src/lib.rs`
- Modify: `hermes-desktop/ui/src/lib/tauri.ts`
- Modify: `hermes-desktop/ui/src/routes/RuntimePage.tsx`
- Modify: `hermes-desktop/ui/src/routes/RuntimePage.css`
- Modify: `hermes-desktop/ui/src/routes/SettingsPage.tsx`

- [x] Add failing Rust tests for engine heartbeat/daemon snapshot parsing and diagnostics payload generation.
- [x] Replace the temporary `sleep 3600` engine process with an app-owned daemon mode launched from the current desktop binary.
- [x] Persist and read engine heartbeat metadata so runtime/status surfaces expose real activity.
- [x] Add a diagnostics/workspace command that reports real app paths, counts, runtime heartbeat, and recent logs.
- [x] Wire diagnostics and enriched runtime fields into the existing Runtime and Settings pages.

### Task 2: Voice Workflow With Real Local Semantics

**Files:**
- Modify: `hermes-desktop/src/commands/voice.rs`
- Modify: `hermes-desktop/src/commands/mod.rs`
- Modify: `hermes-desktop/src/main.rs`
- Modify: `hermes-desktop/src/lib.rs`
- Modify: `hermes-desktop/ui/src/lib/tauri.ts`
- Create: `hermes-desktop/ui/src/routes/VoicePage.tsx`
- Create: `hermes-desktop/ui/src/routes/VoicePage.css`
- Modify: `hermes-desktop/ui/src/app/App.tsx`
- Modify: `hermes-desktop/ui/src/components/SidebarNav.tsx`

- [x] Add failing Rust tests for richer voice summary/history/queue behavior.
- [x] Implement real local voice commands and keep backward-compatible wrappers for old compatibility exports if needed.
- [x] Expose voice status/history in Tauri bindings.
- [x] Add a visible desktop voice page so the workflow exists beyond CLI-only slash commands.

### Task 3: Mission Playbook Growth And Real Operate Evidence

**Files:**
- Create: `hermes-desktop/src/commands/playbook.rs`
- Modify: `hermes-desktop/src/commands/mod.rs`
- Modify: `hermes-desktop/src/main.rs`
- Modify: `hermes-desktop/src/lib.rs`
- Modify: `hermes-desktop/ui/src/lib/tauri.ts`
- Modify: `hermes-desktop/ui/src/routes/MissionsPage.tsx`
- Modify: `hermes-desktop/ui/src/routes/MissionsPage.css`
- Modify: `hermes-desktop/ui/src/routes/OperatePage.tsx`

- [x] Add failing Rust tests for mission playbook suggestion generation from runs, steps, memory, scenarios, and timeline evidence.
- [x] Implement a self-contained playbook/growth read model over the existing mission database.
- [x] Replace hard-coded Operate “recent evidence” content with mission-backed evidence cards.
- [x] Show playbook suggestions in the Missions detail workflow.

### Task 4: Verification

**Files:**
- Modify: `docs/superpowers/plans/2026-04-25-hermes-gap-closure.md`

- [x] Run targeted Rust tests for the touched modules first.
- [x] Run full `cargo test` in `hermes-desktop`.
- [x] Run `npm run typecheck` and `npm run build` in `hermes-desktop/ui`.
- [x] Update the plan with any gaps or residual risks discovered during verification.
