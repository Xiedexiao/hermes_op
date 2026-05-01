# Notes: Hermes Product Research and Completion

## Sources

### Workspace

- Root: `/home/xiedex/code/hermes-agent_rl`
- Product directories: `docs`, `hermes-desktop`
- Reference repositories: `repos/*`

### Product Docs

- `docs/hermes-agent-desktop-functional-design.md`: MVP includes Mission Workspace, local knowledge, memory, plan generation, CLI/browser execution, Council/approval, mission history search, and basic review/playbook generation.
- `docs/hermes-agent-desktop-product-design.md`: MVP requires basic task review and experience sedimentation; V1.1 adds scenario comparison, scheduled/background tasks, cross-channel notification, and workflow template market.
- `docs/hermes-agent-desktop-delivery-plan.md`: Phase 5 includes Scenario + Growth with scenario runs, recommendation output, and playbook suggestions.
- `docs/hermes-agent-desktop-ui-spec.md`: Phase 5 UI surfaces are Simulation and Skills.

### Current Hermes Desktop Status

- Rust backend already includes domains/commands for Missions, Sessions, Knowledge, Memory, Council, Execution, Gateway, Parity, Skills, Simulation, Voice, and terminal backends.
- Source-level unfinished markers are mostly intentional stubs: Agent Core stub in docs/backend comments and voice workflow stubs in `commands/voice.rs` / CLI slash help.
- Verification baseline passed: `cargo test` passed 105 library tests, 50 CLI tests, 24 integration tests, plus doc-tests; `npm run typecheck` passed; `npm run build` passed.
- Practical product gap is not build health. It is Phase 5 Growth/Playbook/Learning-card style productization and replacing/augmenting stubs with validated loops.

### Reference Repositories

- `DeepTutor`: unified chat workspace, mode switching, guided learning, knowledge hub, persistent memory, TutorBots, CLI/SDK. Useful for Hermes onboarding/setup tour, mode continuity, and personal agent memory.
- `MiroFish`: multi-agent scenario simulation with seed extraction, digital-world construction, variable injection, and report generation. Useful for Hermes Simulation: option cards, assumptions, risks, recommendations.
- `TuriX-CUA`: desktop action agent with skills, MCP readiness, hot-swappable model config, and GUI action benchmark orientation. Useful for Hermes Operate/Desktop GUI execution later.
- `edict`: institution-style multi-agent governance with planning, review veto, dispatch, Kanban, intervention, audit, health monitoring, model/skill controls. Useful for Hermes Council/Approval and observability.
- `hermes-agent`: upstream target for CLI/TUI parity, gateway, skills, memory, cron, terminal backends, session search, self-improving loop, and RL trajectory generation.
- `mempalace`: local raw/verbatim memory, palace-style organization, MCP tools, honest benchmark distinction between raw and compressed modes. Useful for Hermes memory/search design.
- `onyx`: RAG/connectors/deep research/actions/MCP/voice/artifacts/deployment modes. Useful for Hermes Knowledge and connector roadmap.

### Paper: arXiv 2604.08377 SkillClaw

- Title: "SkillClaw: Let Skills Evolve Collectively with Agentic Evolver" by Ziyu Ma et al., submitted April 9, 2026.
- Core loop: Multi-user interaction -> session collection -> skill evolution -> skill synchronization.
- Evidence model: preserve prompt, tool calls/actions, intermediate feedback/errors, user responses, and final response; extract skills referenced, tool errors, and coarse quality.
- Evolution actions: refine an existing skill, create a new skill, or skip when evidence is weak.
- Validation gate: candidate skill updates are evaluated before deployment; only accepted updates enter the shared skill pool.
- Reported gains on WildClawBench Day 1 to Day 6: Social Interaction +6.33 abs, Search & Retrieval +11.82 abs, Creative Synthesis +10.23 abs, Safety & Alignment +8.00 abs.
- Strong product takeaway: add a local, auditable Skill Evolution Inbox first. It can collect session/run evidence, group by skill/capability, create candidate recommendations, and require validation before installing/updating skills.

## Synthesized Findings

### Recommended Product Slice

Build a small local "Skill Evolution" slice inside Skills/Growth:

- Add persistent records for skill evolution candidates with status, evidence summary, recommended action, target skill, confidence, and validation notes.
- Add backend commands to list/create/update these candidates.
- Seed candidate generation from existing mission/run/session data initially, without autonomous LLM editing.
- Add a Skills page panel showing pending/accepted/rejected candidate improvements.
- Later wire the panel to actual SKILL.md editing and validation runs.

### Why This Slice

- Aligns with Hermes Agent's self-improving loop and docs Phase 5 Growth/Playbook/Learning-card goals.
- Reuses current Skills, Sessions, Runs, Memory, and Simulation surfaces.
- Avoids premature autonomous skill mutation; keeps candidate updates auditable and user-approvable.
- Converts the SkillClaw paper into product value without requiring multi-user cloud sync on day one.
