# Task Plan: Hermes Product Research and Completion

## Goal

Identify unfinished product work across `docs`, `hermes-desktop`, and reference projects in `repos`, study arXiv 2604.08377, then propose and implement a small verified product improvement after design approval.

## Phases

- [x] Phase 1: Plan and setup
- [x] Phase 2: Repository and product inventory
- [x] Phase 3: Paper research and product idea synthesis
- [x] Phase 4: Design proposal and approval gate
- [x] Phase 5: TDD implementation
- [x] Phase 6: Verification and final report

## Key Questions

1. What unfinished features are explicitly documented or visible in `hermes-desktop`?
2. Which ideas from the reference repos are most aligned with the current Hermes product?
3. What actionable product ideas from arXiv 2604.08377 can be implemented safely now?
4. What is the smallest high-value feature slice that can be tested and verified?

## Decisions Made

- Use `docs` and `hermes-desktop` as the product source of truth.
- Treat `repos` as reference implementations and idea sources unless docs indicate direct dependency.
- Do not modify production code until a focused design is proposed and approved.
- Verified that current code already covers many Phase 1-4 surfaces and passes Rust/frontend checks.
- Recommend implementing a Phase 5 "Skill Evolution Inbox" slice based on SkillClaw rather than replacing stable stubs blindly.
- Implemented Skill Evolution Inbox storage, commands, frontend API, Skills page UI, and delivery-plan documentation.
- Extended the inbox with deterministic candidate generation from failed runs, failed execution steps, and failed run events.
- Added conservative attribution so strong matches to existing discovered skills generate `refine` candidates instead of always `create`.
- Upgraded Simulation toward a MiroFish-style scenario sandbox with variable injection, option comparison, and recommendation explanation.
- Added mission-level scenario comparison matrix and path evolution synthesis so saved scenario runs can be compared across a Mission.
- Refreshed the roadmap docs to frame Phase 5 around session evidence and conservative attribution.

## Errors Encountered

- Root workspace is not a git repository; inspect child repositories individually.
- Broad TODO search initially included `target`/`node_modules`; repeated source-limited inspection.
- Full `cargo test` initially exposed a concurrent migration ledger race for the new migration; changed migration recording to `INSERT OR IGNORE`.

## Status

**Complete** - Skill Evolution Inbox and the comparison-ready MiroFish-style Simulation Sandbox slices are implemented and verified.
