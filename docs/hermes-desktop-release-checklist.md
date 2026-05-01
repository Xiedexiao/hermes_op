# Hermes Desktop Release Checklist

Last updated: 2026-04-30

This checklist captures the local release gate for `hermes-desktop` after the desktop capability-closure work.

## Automated Release Gate

Run from `/home/xiedex/code/hermes-agent_rl/hermes-desktop` unless noted otherwise:

```bash
cargo test
cargo clippy --lib -- -D warnings
cargo clippy --all-targets -- -D warnings
cd ui && npm test && npm run typecheck && npm run build
cd .. && cargo tauri build
```

Expected automated outputs:

- Rust unit, CLI, integration, and doc tests pass with zero failures.
- Rust Clippy passes for both lib and all targets with `-D warnings`.
- UI Node tests pass through `npm test`; this includes capability wrapper / Tauri command-registration drift checks.
- UI typecheck and Vite production build pass.
- Tauri release build produces Linux `deb`, `rpm`, and `AppImage` bundles under `target/release/bundle/`.

## Current Verified Bundle Outputs

A local `cargo tauri build` completed successfully and produced:

- `target/release/bundle/deb/Hermes Operator_0.1.0_amd64.deb`
- `target/release/bundle/rpm/Hermes Operator-0.1.0-1.x86_64.rpm`
- `target/release/bundle/appimage/Hermes Operator_0.1.0_amd64.AppImage`

Current local SHA256 checksums:

```text
3e007cb0de7488bb0326a800fbd144f7ecc88b50f2bca0d71e0b7b43e6c37463  target/release/bundle/deb/Hermes Operator_0.1.0_amd64.deb
7dabcdcab02ed1ffc23e1dc95c6ef854146bcc4df6d0b29701caceb2c8c6d2a6  target/release/bundle/rpm/Hermes Operator-0.1.0-1.x86_64.rpm
b6f765ad1aa2fe96c73036548f2aae7728fb1f81e4e260ee605b7a23d72ee9e5  target/release/bundle/appimage/Hermes Operator_0.1.0_amd64.AppImage
```

`dpkg-deb --info` confirms Debian metadata has the expected package, version, maintainer, homepage, dependencies, and non-empty description. `dpkg-deb --contents` confirms `/usr/bin/hermes-desktop`, `/usr/bin/hermes-operator-cli`, the desktop entry, and hicolor icons are included. AppImage extraction confirms `AppRun`, the desktop entry, app icon, bundled libraries, and both binaries are present.

A non-GUI release-binary smoke test also completed successfully:

```bash
target/release/hermes-desktop --engine-daemon --profile release-smoke --data-dir /tmp/hermes-release-smoke-... --once
```

Expected smoke output: creates `hermes.db` plus `engine.heartbeat.json` with `profile: "release-smoke"`.

The AppImage wrapper was also smoke-tested without launching a GUI by using `APPIMAGE_EXTRACT_AND_RUN=1`:

```bash
APPIMAGE_EXTRACT_AND_RUN=1 "target/release/bundle/appimage/Hermes Operator_0.1.0_amd64.AppImage" --engine-daemon --profile appimage-smoke --data-dir /tmp/hermes-appimage-smoke-... --once
```

Expected smoke output: creates `engine.heartbeat.json` with `profile: "appimage-smoke"`.

## Manual Smoke Gate

Run these before publishing artifacts outside the local machine:

- Install or launch exactly one produced Linux artifact on a clean desktop session.
- Confirm the app opens the main Hermes Desktop window and initializes SQLite state without panic.
- Confirm Runtime, Skills, and Simulation pages load without blank screens.
- Confirm Remote Skill Marketplace can load a local file manifest and install an inline-content skill.
- Confirm GUI automation dry-run works; only attempt non-dry-run with an OS GUI session, installed allowlisted executor, and explicit `RUN DESKTOP ACTION` confirmation.
- Confirm External SaaS `local_echo` works offline; only attempt `http_json` non-dry-run with a real endpoint, credentials/network if needed, and explicit `RUN EXTERNAL SAAS SIMULATION` confirmation.
- Confirm local RL training accepts trajectory JSONL, produces a persisted artifact, and can filter local RL job history by `target_remote_user_id`; do not represent this as large-model RLHF, distributed training, or remote-user delivery.
- Confirm Agent Exchange can save a Future Remote User, use it for an outbound draft, filter by remote user id, export/download a scoped bundle with `remote_users`, and re-import an old bundle without `remote_users`.
- Confirm marketplace install history, Runtime adapter audit list/export, simulation run history, and local RL job history can filter by `target_remote_user_id`; these filters must remain local history filters only.
- Confirm marketplace install/history, GUI automation audit, simulation run history/evidence, and local RL job/artifact exports include optional `target_remote_user_id` only as future routing metadata; when Marketplace audit, Runtime adapter audit handoff, Simulation capability evidence, or Local RL artifact export target is selected from local Agent Exchange, confirm the exported/downloaded envelope includes `target_remote_user_profile` snapshot and still does not claim remote delivery.
- Confirm Runtime adapter audit handoff, Hermes Native CUA audit payload, and TuriX bridge audit payload downloads remain local review files; Native CUA and TuriX payload downloads must not be described as future remote-user handoff envelopes unless a profile-aware envelope is added later.

## Release Safety Boundaries

Keep these backend gates intact for release candidates:

- Non-dry-run desktop action / GUI macro requires backend confirmation phrase and allowlisted executor validation.
- Non-dry-run `http_json` external SaaS simulation requires backend confirmation phrase and validated `http`/`https` endpoint.
- Marketplace install consumes manifest-provided `source_url` or inline `content`; it does not execute remote code.
- Agent Exchange Future Remote Users are local routing metadata only; no release may claim remote account provisioning, realtime cross-user transport, remote delivery receipts, or remote agent discovery until those systems exist.
- `target_remote_user_id` in persisted local runs/history/audits, local history filters, or exported audit/evidence artifacts is not proof of remote marketplace activity, remote delivery, remote GUI execution, or remote RLHF infrastructure.
- Backend `voice_*_stub` commands are legacy compatibility wrappers only; new UI should keep using non-stub voice client functions.

## Known Packaging Note

`cargo tauri build` currently succeeds but emits a Tauri bundler warning while patching bundle type metadata:

- `__TAURI_BUNDLE_TYPE variable not found in binary`

The current app does not ship updater-plugin behavior, and the bundles are still produced. If an updater flow is added later, revisit this warning by aligning Tauri CLI/crate versions and checking Tauri bundler symbol generation before publishing updater-enabled packages.
