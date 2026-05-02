# Hermes Desktop 打包指南

更新时间：2026-05-01

这份文档只写怎么把 `hermes-desktop` 打包成软件。

当前项目是 **Tauri v2 桌面应用**。现在只配置了桌面端：

- Linux
- macOS
- Windows

Android / iOS 还没有配置，暂时不能打包。

## 1. 最短打包命令

进入项目：

```bash
cd /home/xiedex/code/hermes-agent_rl/hermes-desktop
```

先构建前端：

```bash
cd ui
npm ci
npm run build
cd ..
```

再打包桌面软件：

```bash
cargo tauri build
```

打包产物都在：

```text
hermes-desktop/target/release/bundle/
```

## 2. 打包前最好先检查一次

正式发包前跑这些：

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

cargo tauri build
```

如果只是本地自己试用，最少跑：

```bash
cd ui
npm ci
npm run build
cd ..
cargo tauri build
```

## 3. Linux 怎么打包

必须在 Linux 机器上打包。

Ubuntu / Debian 先装依赖：

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
  librsvg2-dev \
  rpm
```

然后执行：

```bash
cd /home/xiedex/code/hermes-agent_rl/hermes-desktop
cd ui
npm ci
npm run build
cd ..
cargo tauri build
```

Linux 打包后会生成这些文件：

```text
target/release/bundle/deb/*.deb
target/release/bundle/rpm/*.rpm
target/release/bundle/appimage/*.AppImage
```

当前项目之前打出来的文件名类似：

```text
Hermes Operator_0.1.0_amd64.deb
Hermes Operator-0.1.0-1.x86_64.rpm
Hermes Operator_0.1.0_amd64.AppImage
```

查看实际产物：

```bash
find target/release/bundle -maxdepth 3 -type f | sort
```

## 4. macOS 怎么打包

必须在 Mac 上打包。

先安装 Xcode 命令行工具：

```bash
xcode-select --install
```

然后执行：

```bash
cd /path/to/hermes-agent_rl/hermes-desktop
cd ui
npm ci
npm run build
cd ..
cargo tauri build
```

macOS 打包后会生成：

```text
target/release/bundle/macos/*.app
target/release/bundle/dmg/*.dmg
```

如果只是自己用，可以直接用未签名的 `.app` 或 `.dmg`。

如果要发给别人正式安装，需要 Apple 开发者账号，并做：

- Developer ID 签名
- notarization 公证

没有签名和公证，别人打开时可能会被 macOS Gatekeeper 拦截。

## 5. Windows 怎么打包

必须在 Windows 机器上打包。

先安装：

- Rust
- Node.js LTS
- Microsoft C++ Build Tools
- Microsoft Edge WebView2 Runtime

然后用 PowerShell 执行：

```powershell
cd C:\path\to\hermes-agent_rl\hermes-desktop

cd ui
npm ci
npm run build
cd ..

cargo tauri build
```

Windows 打包后会生成：

```text
target\release\bundle\msi\*.msi
target\release\bundle\nsis\*-setup.exe
```

也会有主程序：

```text
target\release\hermes-desktop.exe
target\release\hermes-operator-cli.exe
```

查看实际产物：

```powershell
Get-ChildItem -Recurse target\release\bundle
```

如果要公开发布 Windows 安装包，最好做代码签名。没签名也能打包，但用户下载后可能遇到 SmartScreen 警告。

## 6. 三个平台总结

| 系统 | 在哪里打包 | 主要命令 | 产物 |
| --- | --- | --- | --- |
| Linux | Linux | `cargo tauri build` | `.deb` / `.rpm` / `.AppImage` |
| macOS | Mac | `cargo tauri build` | `.app` / `.dmg` |
| Windows | Windows | `cargo tauri build` | `.msi` / `.exe` |

重点：**哪个系统的软件，就尽量在哪个系统上打包。**

## 7. 打包成功后怎么检查

Linux / macOS 可以跑：

```bash
target/release/hermes-desktop \
  --engine-daemon \
  --profile smoke \
  --data-dir /tmp/hermes-smoke \
  --once
```

Windows PowerShell 可以跑：

```powershell
.\target\release\hermes-desktop.exe `
  --engine-daemon `
  --profile smoke `
  --data-dir $env:TEMP\hermes-smoke `
  --once
```

然后再手动打开安装包，确认：

- 软件窗口能打开
- Runtime 页面能打开
- Skills 页面能打开
- Simulation 页面能打开
- Agent Exchange 页面能打开

## 8. 生成校验文件

Linux：

```bash
sha256sum target/release/bundle/*/* > target/release/bundle/SHA256SUMS
```

macOS：

```bash
shasum -a 256 target/release/bundle/dmg/*.dmg
```

Windows：

```powershell
Get-FileHash target\release\bundle\*\* -Algorithm SHA256
```

## 9. 最常用流程

平时你只需要记住这个：

```bash
cd /home/xiedex/code/hermes-agent_rl/hermes-desktop

cd ui
npm ci
npm run build
cd ..

cargo tauri build
```

然后去这里拿安装包：

```text
target/release/bundle/
```

