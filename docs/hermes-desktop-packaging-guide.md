# Hermes Desktop Packaging Guide

Last updated: 2026-05-01

This guide explains how to package `hermes-desktop` into installable software for the supported desktop systems: Linux, macOS, and Windows. The current app is a Tauri v2 desktop app. Android and iOS are not configured in this repository yet, so they are listed only as future work.

Repository layout used by this guide:

- App root: `/home/xiedex/code/hermes-agent_rl/hermes-desktop`
- Tauri config: `hermes-desktop/tauri.conf.json`
- Rust package: `hermes-desktop/Cargo.toml`
- UI package: `hermes-desktop/ui/package.json`
- UI output: `hermes-desktop/ui/dist`
- Tauri bundle output: `hermes-desktop/target/release/bundle/`

Current product metadata:

- Product name: `Hermes Operator`
- App identifier: `ai.hermes.operator`
- Version: `0.1.0`
- Bundle targets: `all`
- Main desktop binary: `hermes-desktop`
- Included CLI binary: `hermes-operator-cli`

## 1. Release Rule

Build on the target operating system whenever possible.

Tauri can cross-compile some binaries, but installers, code signing, notarization, system webview dependencies, and smoke tests are platform-specific. The reliable release pattern is a CI matrix or three local machines:

- Linux runner builds Linux packages.
- macOS runner builds `.app` / `.dmg` and performs Apple signing/notarization.
- Windows runner builds `.msi` / NSIS setup executable and performs Windows signing.

Do not publish artifacts that were not smoke-tested on the target OS.

## 2. Shared Prerequisites

Install these on every build machine:

1. Rust stable:

```bash
rustup update stable
rustup default stable
```

1. Node.js LTS and npm.

1. Tauri CLI v2:

```bash
cargo install tauri-cli --version "^2" --locked
```

1. Frontend dependencies:

```bash
cd /home/xiedex/code/hermes-agent_rl/hermes-desktop/ui
npm ci
```

The project has `package-lock.json`, so use `npm ci` for repeatable release builds.

## 3. Version Checklist

Before packaging a release, keep these values aligned:

- `hermes-desktop/tauri.conf.json` -> `version`
- `hermes-desktop/Cargo.toml` -> `[package].version`
- `hermes-desktop/ui/package.json` -> `version`
- Release notes / artifact names / checksum manifest

Tauri prefers the version in `tauri.conf.json` when present. This repository currently sets `0.1.0` in both Tauri and Rust metadata.

## 4. Preflight Verification

Run this from `hermes-desktop` before building packages:

```bash
cd /home/xiedex/code/hermes-agent_rl/hermes-desktop

cargo test
cargo clippy --lib -- -D warnings
cargo clippy --all-targets -- -D warnings

cd ui
npm test
npm run typecheck
npm run build
cd ..
```

The Tauri config contains `beforeBuildCommand: "npm run build"`, while this repository keeps `package.json` under `ui/`. The known-good local release flow is to build the UI explicitly with `cd ui && npm run build` before `cargo tauri build`. If a future Tauri CLI invocation fails because it runs the frontend command from the app root, change the config command to `cd ui && npm run build` or run the equivalent command in CI before bundling.

## 5. Linux Packaging

### Linux Prerequisites

On Debian/Ubuntu build machines, install Tauri's Linux system dependencies:

```bash
sudo apt update
sudo apt install -y \
  libwebkit2gtk-4.1-dev \
  build-essential \
  curl \
  wget \
  file \
  libxdo-dev \
  libssl-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
```

For RPM output, install RPM tooling if your distro does not already include it:

```bash
sudo apt install -y rpm
```

Use the equivalent WebKitGTK 4.1, compiler, OpenSSL, appindicator, and librsvg packages on Fedora, Arch, openSUSE, Alpine, or NixOS.

### Build Linux Artifacts

```bash
cd /home/xiedex/code/hermes-agent_rl/hermes-desktop

cd ui
npm ci
npm run build
cd ..

cargo tauri build
```

Expected Linux outputs with the current `targets: "all"` config:

```text
target/release/hermes-desktop
target/release/hermes-operator-cli
target/release/bundle/deb/*.deb
target/release/bundle/rpm/*.rpm
target/release/bundle/appimage/*.AppImage
```

The last verified local build produced:

```text
target/release/bundle/deb/Hermes Operator_0.1.0_amd64.deb
target/release/bundle/rpm/Hermes Operator-0.1.0-1.x86_64.rpm
target/release/bundle/appimage/Hermes Operator_0.1.0_amd64.AppImage
```

List the actual files after each build:

```bash
find target/release/bundle -maxdepth 3 -type f | sort
```

### Linux Smoke Test

Run the release binary without a GUI:

```bash
tmpdir="$(mktemp -d)"
target/release/hermes-desktop \
  --engine-daemon \
  --profile release-smoke \
  --data-dir "$tmpdir" \
  --once

test -f "$tmpdir/hermes.db"
test -f "$tmpdir/engine.heartbeat.json"
```

Smoke-test the AppImage wrapper:

```bash
tmpdir="$(mktemp -d)"
APPIMAGE_EXTRACT_AND_RUN=1 \
  "target/release/bundle/appimage/Hermes Operator_0.1.0_amd64.AppImage" \
  --engine-daemon \
  --profile appimage-smoke \
  --data-dir "$tmpdir" \
  --once

test -f "$tmpdir/engine.heartbeat.json"
```

Inspect Debian package contents:

```bash
dpkg-deb --info "target/release/bundle/deb/Hermes Operator_0.1.0_amd64.deb"
dpkg-deb --contents "target/release/bundle/deb/Hermes Operator_0.1.0_amd64.deb" | less
```

Expected package contents include:

- `/usr/bin/hermes-desktop`
- `/usr/bin/hermes-operator-cli`
- desktop entry
- hicolor icons

### Linux Checksums

```bash
sha256sum target/release/bundle/deb/*.deb \
  target/release/bundle/rpm/*.rpm \
  target/release/bundle/appimage/*.AppImage \
  > target/release/bundle/SHA256SUMS
```

### Linux Signing Notes

Linux signing depends on how you publish:

- Direct file download: publish `SHA256SUMS`, and optionally sign the checksum file with GPG.
- APT repository: sign repository metadata with your package/repository GPG key.
- RPM repository: sign RPM packages or repository metadata according to your distro policy.
- AppImage: publish checksums at minimum; add signature/update metadata only if you implement an updater flow.

## 6. macOS Packaging

### macOS Prerequisites

Build on macOS. Install:

```bash
xcode-select --install
```

For App Store or signed public distribution, you also need:

- Apple Developer account.
- Developer ID Application certificate for direct distribution outside the Mac App Store.
- Apple signing/notarization credentials.

### Build macOS Artifacts

On the target Mac:

```bash
cd /home/xiedex/code/hermes-agent_rl/hermes-desktop

cd ui
npm ci
npm run build
cd ..

cargo tauri build
```

Expected macOS outputs:

```text
target/release/hermes-desktop
target/release/hermes-operator-cli
target/release/bundle/macos/*.app
target/release/bundle/dmg/*.dmg
```

The exact `.dmg` name includes the product name, version, and sometimes architecture. Always inspect:

```bash
find target/release/bundle -maxdepth 3 -type f -o -type d | sort
```

### Apple Silicon and Intel

For a native Apple Silicon build:

```bash
rustup target add aarch64-apple-darwin
cargo tauri build --target aarch64-apple-darwin
```

For a native Intel build on Intel macOS:

```bash
rustup target add x86_64-apple-darwin
cargo tauri build --target x86_64-apple-darwin
```

For universal builds, use a macOS runner with both Rust targets installed and validate the current Tauri CLI behavior for your environment:

```bash
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo tauri build --target universal-apple-darwin
```

If universal bundling fails, produce separate Apple Silicon and Intel artifacts and label them explicitly.

### macOS Signing and Notarization

Unsigned `.app` / `.dmg` artifacts are acceptable only for local development or internal testing where the tester understands Gatekeeper warnings.

For public downloads outside the Mac App Store:

1. Install the Developer ID Application certificate in the macOS keychain.
1. Configure Tauri signing identity through `tauri.conf.json` under `bundle.macOS.signingIdentity`, or use environment variables supported by Tauri.
1. Configure notarization credentials, commonly:

```bash
export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Org (TEAMID)"
export APPLE_ID="release@example.com"
export APPLE_PASSWORD="app-specific-password-or-keychain-profile"
export APPLE_TEAM_ID="TEAMID"
```

1. Run:

```bash
cargo tauri build
```

Notarization requires Apple services and can fail for certificate, entitlement, or network reasons. Do not publish macOS direct-download artifacts until Gatekeeper accepts the downloaded `.dmg` on a clean Mac.

### macOS Smoke Test

Run the binary without opening the full GUI:

```bash
tmpdir="$(mktemp -d)"
target/release/hermes-desktop \
  --engine-daemon \
  --profile macos-smoke \
  --data-dir "$tmpdir" \
  --once

test -f "$tmpdir/hermes.db"
test -f "$tmpdir/engine.heartbeat.json"
```

After installing or mounting the `.dmg`, also run a GUI smoke test:

- Open `Hermes Operator.app`.
- Confirm the main window renders.
- Confirm Runtime, Skills, Simulation, and Agent Exchange pages do not blank-screen.
- Confirm no macOS permission prompts are misrepresented as successful GUI automation.

### macOS Checksums

```bash
shasum -a 256 target/release/bundle/dmg/*.dmg \
  > target/release/bundle/SHA256SUMS-macos
```

## 7. Windows Packaging

### Windows Prerequisites

Build on Windows for the reliable release path. Install:

- Microsoft C++ Build Tools with `Desktop development with C++`.
- Microsoft Edge WebView2 Runtime.
- Rust via `rustup`.
- Node.js LTS and npm.
- VBSCRIPT Windows optional feature if building MSI installers.

Tauri Windows installers can be:

- `.msi` via WiX Toolset v3.
- `*-setup.exe` via NSIS.

If MSI or NSIS bundling fails, check the specific Tauri error and install the missing installer toolchain.

### Build Windows Artifacts

Use PowerShell:

```powershell
cd C:\path\to\hermes-agent_rl\hermes-desktop

cd ui
npm ci
npm run build
cd ..

cargo tauri build
```

Expected Windows outputs:

```text
target\release\hermes-desktop.exe
target\release\hermes-operator-cli.exe
target\release\bundle\msi\*.msi
target\release\bundle\nsis\*-setup.exe
```

The exact installer set depends on Tauri target resolution and installed bundler tooling. Inspect:

```powershell
Get-ChildItem -Recurse target\release\bundle
```

### Windows Code Signing

Unsigned Windows installers can run, but browser-downloaded installers may trigger SmartScreen warnings.

For public distribution:

- Use an OV/EV code-signing certificate, Azure Key Vault, or Azure Trusted Signing.
- Configure Tauri Windows signing through `bundle.windows.signCommand` or the official Tauri Windows signing environment.
- If cross-compiling Windows installers from Linux or macOS, use a custom signing command; the default signing flow is Windows-oriented.

Do not publish unsigned Windows installers as production releases unless the release notes explicitly state they are unsigned internal/testing artifacts.

### Windows Smoke Test

Run the binary without opening the full GUI:

```powershell
$tmp = Join-Path $env:TEMP ("hermes-windows-smoke-" + [guid]::NewGuid())
New-Item -ItemType Directory -Path $tmp | Out-Null

.\target\release\hermes-desktop.exe `
  --engine-daemon `
  --profile windows-smoke `
  --data-dir $tmp `
  --once

Test-Path (Join-Path $tmp "hermes.db")
Test-Path (Join-Path $tmp "engine.heartbeat.json")
```

Installer smoke:

- Install the `.msi` or `*-setup.exe` on a clean Windows VM.
- Launch Hermes Operator from Start Menu.
- Confirm the main Tauri window renders.
- Confirm Runtime, Skills, Simulation, and Agent Exchange pages load.
- Confirm WebView2 is present or installed by your deployment process.

### Windows Checksums

```powershell
Get-FileHash target\release\bundle\msi\*.msi -Algorithm SHA256
Get-FileHash target\release\bundle\nsis\*.exe -Algorithm SHA256
```

Write the hashes to a release manifest.

## 8. Mobile Targets

This repository is not currently configured for Android or iOS packaging.

Tauri v2 supports mobile targets, but mobile packaging requires additional configuration and dependencies:

- Android Studio.
- Android SDK / Platform Tools / Build Tools / Command-line Tools.
- Android NDK.
- `JAVA_HOME`, `ANDROID_HOME`, and `NDK_HOME`.
- Rust Android targets.
- For iOS, Xcode and Apple signing/provisioning.

Do not claim Android or iOS releases until the repository has explicit mobile Tauri configuration, mobile icons, permissions, signing profiles, and tested mobile build commands.

## 9. Publishing Checklist

Before uploading artifacts:

1. Run the preflight verification commands.
1. Build on the target OS.
1. Run non-GUI smoke tests.
1. Install one artifact on a clean VM or clean desktop session.
1. Run the manual UI smoke test.
1. Generate SHA256 checksums.
1. Sign/notarize where required for the release channel.
1. Keep safety boundary notes intact:

- Remote Skill Marketplace install writes local skills only.
- GUI automation non-dry-run remains allowlisted and confirmation-gated.
- External SaaS non-dry-run requires the explicit confirmation phrase and real endpoint.
- Local RL training is a tabular baseline over trajectory JSONL, not remote RLHF infrastructure.
- Agent Exchange future remote users are local routing metadata, not live remote accounts or delivery receipts.

## 10. Quick Commands

### Linux

```bash
cd /home/xiedex/code/hermes-agent_rl/hermes-desktop
cd ui && npm ci && npm test && npm run typecheck && npm run build
cd ..
cargo test
cargo clippy --lib -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo tauri build
find target/release/bundle -maxdepth 3 -type f | sort
sha256sum target/release/bundle/*/* > target/release/bundle/SHA256SUMS
```

### macOS

```bash
cd /path/to/hermes-agent_rl/hermes-desktop
cd ui && npm ci && npm test && npm run typecheck && npm run build
cd ..
cargo test
cargo clippy --lib -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo tauri build
find target/release/bundle -maxdepth 3 -print | sort
shasum -a 256 target/release/bundle/dmg/*.dmg
```

### Windows PowerShell

```powershell
cd C:\path\to\hermes-agent_rl\hermes-desktop
cd ui
npm ci
npm test
npm run typecheck
npm run build
cd ..
cargo test
cargo clippy --lib -- -D warnings
cargo clippy --all-targets -- -D warnings
cargo tauri build
Get-ChildItem -Recurse target\release\bundle
Get-FileHash target\release\bundle\*\* -Algorithm SHA256
```

## References

- Tauri v2 prerequisites: https://v2.tauri.app/start/prerequisites/
- Tauri v2 distribution overview: https://v2.tauri.app/distribute/
- Tauri v2 macOS signing/notarization: https://v2.tauri.app/distribute/sign/macos/
- Tauri v2 Windows installer: https://v2.tauri.app/distribute/windows-installer/
- Tauri v2 Windows signing: https://v2.tauri.app/distribute/sign/windows/
- Existing local release checklist: `docs/hermes-desktop-release-checklist.md`
