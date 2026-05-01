# Scenario Handoff Closure Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Close the remaining Simulation Sandbox gaps by adding numeric variable sliders, explicit recommendation rationale, and automatic Council/Execution handoff for saved scenario results.

**Architecture:** Extend the existing JSON-backed scenario payload instead of adding a new SQLite migration, preserving legacy scenario rows. On scenario save, create a completed simulation run, timeline event, pending Council review step, and pending Execution step so sandbox outputs enter the existing governance and operate surfaces. Keep UI changes inside the current `SimulationPage` and Tauri client types.

**Tech Stack:** Rust, Tauri commands, SQLite, React, TypeScript, Vite.

---

## File Structure

- Modify: `hermes-desktop/src/commands/simulation.rs` — add scenario rationale fields, numeric variable normalization/scoring, completed run + Council/Execution handoff, and Rust unit tests.
- Modify: `hermes-desktop/ui/src/lib/tauri.ts` — expose new scenario variable and recommendation fields to TypeScript.
- Modify: `hermes-desktop/ui/src/routes/SimulationPage.tsx` — replace select-only variable levels with sliders, send recommendation result and rationale separately, and show handoff/reason cards.
- Modify: `hermes-desktop/ui/src/routes/SimulationPage.css` — style sliders, explanation card, and handoff callout.
- Modify: `docs/hermes-agent-desktop-functional-design.md`, `docs/hermes-agent-desktop-ui-spec.md`, `docs/hermes-agent-desktop-delivery-plan.md` — update stale open-work notes to match implemented behavior and leave only larger later work out of scope.
- Modify: `.omx/plans/task_plan.md`, `.omx/plans/notes.md` — track findings and execution status.

## Tasks

### Task 1: Backend Scenario Semantics and Handoff

**Files:**
- Modify: `hermes-desktop/src/commands/simulation.rs`

- [x] Step 1: Add a failing Rust test named `create_scenario_run_records_rationale_numeric_variables_and_governance_handoff` that creates a mission, saves a scenario with `recommendation_reason`, numeric `impact_weight`/`uncertainty_weight`, and asserts: scenario payload preserves rationale/weights; `runs` has a completed `simulation` run; `run_events` has `scenario_saved`; `council_steps` has a pending Scenario Reviewer row; `execution_steps` has a pending API review row.
- [x] Step 2: Run `cd hermes-desktop && cargo test create_scenario_run_records_rationale_numeric_variables_and_governance_handoff --lib` and confirm it fails because the fields/handoff are absent.
- [x] Step 3: Extend `SimulationCreateScenarioRunRequest`, `SimulationScenarioRun`, `ScenarioVariable`, and `ScenarioRunPayload` with `recommendation_reason`, `impact_weight`, and `uncertainty_weight`, using serde defaults for backward compatibility.
- [x] Step 4: Normalize numeric weights to 0–100, derive string levels from weights when needed, and update option scoring to use weights before coarse labels.
- [x] Step 5: In `create_scenario_run`, keep `recommendation` as the selected option label and derive/store a separate rationale string.
- [x] Step 6: After inserting `scenario_runs`, insert a completed `runs` row with type `simulation`, record a `scenario_saved` event, insert a pending `council_steps` Scenario Reviewer row, and insert a pending `execution_steps` API row with the selected recommendation payload.
- [x] Step 7: Run the targeted Rust test and confirm it passes.

### Task 2: Frontend Sliders and Rationale Card

**Files:**
- Modify: `hermes-desktop/ui/src/lib/tauri.ts`
- Modify: `hermes-desktop/ui/src/routes/SimulationPage.tsx`
- Modify: `hermes-desktop/ui/src/routes/SimulationPage.css`

- [x] Step 1: Update TypeScript scenario types with `recommendation_reason`, `impact_weight`, and `uncertainty_weight`.
- [x] Step 2: Replace the select-only Impact/Uncertainty controls with range sliders that also display low/medium/high labels.
- [x] Step 3: Send `recommendation` as the selected option label and `recommendation_reason` as the editable explanation text.
- [x] Step 4: Add a visible recommendation explanation card and automatic Council/Execution handoff callout to the save form and saved history cards.
- [x] Step 5: Run `cd hermes-desktop/ui && npm run typecheck` after edits.

### Task 3: Documentation and Verification

**Files:**
- Modify: `docs/hermes-agent-desktop-functional-design.md`
- Modify: `docs/hermes-agent-desktop-ui-spec.md`
- Modify: `docs/hermes-agent-desktop-delivery-plan.md`
- Modify: `.omx/plans/task_plan.md`
- Modify: `.omx/plans/notes.md`

- [x] Step 1: Update Simulation “currently implemented” sections to include variable sliders/weights, comparison matrix, path evolution, rationale cards, and Council/Execution handoff.
- [x] Step 2: Update stale “still unfinished” lists so they do not claim completed work remains missing.
- [x] Step 3: Run `cd hermes-desktop && cargo test create_scenario_run_records_rationale_numeric_variables_and_governance_handoff --lib`.
- [x] Step 4: Run `cd hermes-desktop && cargo test simulation --lib`.
- [x] Step 5: Run `cd hermes-desktop/ui && npm run typecheck`.
- [x] Step 6: Run `cd hermes-desktop/ui && npm run build`.

## Self-Review

- Spec coverage: covers every still-actionable Simulation gap explicitly named in the design docs: variable model/control, mission comparison/path evolution doc drift, recommendation rationale card, and Council/Execution handoff.
- Open-work scan: no `TODO`, `TBD`, or open-ended “implement later” steps are present.
- Type consistency: new fields use snake_case to match existing Rust/serde and TypeScript payload conventions.
