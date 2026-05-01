# Skill Evolution Inbox Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a local, auditable Skill Evolution Inbox that turns session evidence into reviewable candidate skill improvements.

**Architecture:** Store candidate evolution records in SQLite and expose them through focused Tauri commands. Keep candidate review separate from actual `SKILL.md` mutation so the first slice is safe, testable, and reversible. Apply conservative attribution so strong matches to existing discovered skills become `refine` candidates instead of always `create`. Render the inbox inside the existing Skills page using the current page layout and CSS conventions.

**Tech Stack:** Rust, Tauri commands, SQLite migrations, React + TypeScript, Vite.

---

## File Structure

- Create: `hermes-desktop/migrations/003_skill_evolution.sql` for the candidate table and indexes.
- Modify: `hermes-desktop/src/backend/storage/migrations.rs` to apply migration `003_skill_evolution`.
- Create: `hermes-desktop/src/commands/skill_evolution.rs` for candidate list/create/status commands and unit tests.
- Modify: `hermes-desktop/src/commands/mod.rs` to export the new command module.
- Modify: `hermes-desktop/src/lib.rs` to re-export Tauri command functions.
- Modify: `hermes-desktop/src/main.rs` to register Tauri commands.
- Modify: `hermes-desktop/ui/src/lib/tauri.ts` to add TypeScript types and invoke wrappers.
- Modify: `hermes-desktop/ui/src/routes/SkillsPage.tsx` to load and render candidates.
- Modify: `hermes-desktop/ui/src/routes/SkillsPage.css` to style the inbox within the existing visual system.
- Modify: `docs/hermes-agent-desktop-delivery-plan.md` to record the Phase 5 Growth mapping.
- Modify: `task_plan.md` after implementation and verification.

## Tasks

### Task 1: Persistent Candidate Storage

**Files:**

- Create: `hermes-desktop/migrations/003_skill_evolution.sql`
- Modify: `hermes-desktop/src/backend/storage/migrations.rs`
- Test: `hermes-desktop/src/commands/skill_evolution.rs`

- [ ] Step 1: Add a failing Rust test that creates a candidate and lists it from an in-memory database.
- [ ] Step 2: Run `cd hermes-desktop && cargo test skill_evolution --lib` and confirm the new test fails because the command module/table does not exist.
- [ ] Step 3: Add migration `003_skill_evolution` with `skill_evolution_candidates`.
- [ ] Step 4: Register the migration in `run_migrations`.
- [ ] Step 5: Implement the minimal command module functions needed for create/list.
- [ ] Step 6: Re-run `cd hermes-desktop && cargo test skill_evolution --lib` and confirm create/list passes.

### Task 2: Review Status Workflow

**Files:**

- Modify: `hermes-desktop/src/commands/skill_evolution.rs`

- [ ] Step 1: Add failing tests for accepted/rejected status updates and validation of illegal status/action values.
- [ ] Step 2: Run `cd hermes-desktop && cargo test skill_evolution --lib` and confirm expected failures.
- [ ] Step 3: Implement `skill_evolution_candidate_set_status_for_db`, status validation, action validation, confidence validation, and list filtering.
- [ ] Step 4: Re-run `cd hermes-desktop && cargo test skill_evolution --lib` and confirm the workflow tests pass.

### Task 3: Tauri Surface

**Files:**

- Modify: `hermes-desktop/src/commands/mod.rs`
- Modify: `hermes-desktop/src/lib.rs`
- Modify: `hermes-desktop/src/main.rs`

- [ ] Step 1: Export `skill_evolution_candidate_list`, `skill_evolution_candidate_create`, and `skill_evolution_candidate_set_status`.
- [ ] Step 2: Register the three functions in `tauri::generate_handler!`.
- [ ] Step 3: Run `cd hermes-desktop && cargo test skill_evolution --lib`.
- [ ] Step 4: Run `cd hermes-desktop && cargo test`.

### Task 4: Frontend API and Skills Page Inbox

**Files:**

- Modify: `hermes-desktop/ui/src/lib/tauri.ts`
- Modify: `hermes-desktop/ui/src/routes/SkillsPage.tsx`
- Modify: `hermes-desktop/ui/src/routes/SkillsPage.css`

- [ ] Step 1: Add TypeScript types and wrappers for candidate list/create/status update.
- [ ] Step 2: Load candidates alongside skills/toolsets.
- [ ] Step 3: Render an Evolution Inbox section with summary counts, pending candidate cards, source refs, validation notes, and accept/reject buttons.
- [ ] Step 4: Add a small manual candidate form so users can capture evidence without automatic skill mutation.
- [ ] Step 5: Run `cd hermes-desktop/ui && npm run typecheck`.
- [ ] Step 6: Run `cd hermes-desktop/ui && npm run build`.

### Task 5: Docs and Final Verification

**Files:**

- Modify: `docs/hermes-agent-desktop-delivery-plan.md`
- Modify: `task_plan.md`

- [ ] Step 1: Update the delivery plan Phase 5 section to mention Skill Evolution Inbox as the first Growth slice.
- [ ] Step 2: Update `task_plan.md` status.
- [ ] Step 3: Run `cd hermes-desktop && cargo test`.
- [ ] Step 4: Run `cd hermes-desktop/ui && npm run typecheck`.
- [ ] Step 5: Run `cd hermes-desktop/ui && npm run build`.

## Self-Review

- Spec coverage: the plan adds a candidate inbox, persistent records, review workflow, frontend panel, and docs mapping. It intentionally excludes automatic `SKILL.md` mutation and multi-user sync.
- Placeholder scan: no implementation placeholder is required for the agent executing this plan; each task has concrete paths and commands.
- Type consistency: all planned command names use the `skill_evolution_candidate_*` prefix and map to `skill_evolution_candidates` storage.
