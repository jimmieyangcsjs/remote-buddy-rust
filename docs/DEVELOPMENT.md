# 开发与构建

## 1. 目标平台

- Windows 10 19041+ 或 Windows 11；
- 仅 x64（`x86_64-pc-windows-msvc`）；
- Rust 2024 edition；
- 不构建 32 位版本。

macOS、Linux、Windows ARM64 和 Windows 32 位不在当前支持范围内。

## 2. 环境准备

安装：

1. Visual Studio 2022 Build Tools；
2. “使用 C++ 的桌面开发”工作负载；
3. Windows 10/11 SDK；
4. Rust stable 1.89+；
5. Node.js 22+；
6. pnpm；
7. Microsoft Edge WebView2 Runtime；
8. 语音开发需要 VB-CABLE。

检查工具链：

```powershell
rustup show
rustc --version
cargo --version
node --version
pnpm --version
```

## 3. 初始化

```powershell
pnpm --dir ui install
cargo check --workspace
```

仓库中的 `.cargo/config.toml` 可能配置了镜像源。切换网络环境后，可删除或替换该配置，并确保 `Cargo.lock` 中的依赖能够正常解析。

## 4. 常用命令

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir ui test
pnpm --dir ui build
```

开发模式：

```powershell
pnpm --dir ui dev
cargo tauri dev
```

没有全局 Tauri CLI 时：

```powershell
pnpm dlx "@tauri-apps/cli@^2" dev
```

## 5. 验证与打包

完整自动化验证：

```powershell
.\scripts\validate.ps1 -Report docs\validation.md
```

构建 Release EXE 和 NSIS 安装包：

```powershell
.\scripts\build.ps1 -Configuration Release
```

只构建 Rust，不生成安装包：

```powershell
.\scripts\build.ps1 -Configuration Release -SkipBundle
```

手动构建正式 EXE 时必须启用 `custom-protocol`：

```powershell
cargo build --release -p remote-buddy-desktop --features custom-protocol
```

输出：

```text
target\release\remote-buddy-desktop.exe
target\release\bundle\nsis\Remote Buddy_0.3.0_x64-setup.exe
```

## 6. Release 配置

`Cargo.toml` 中的 release profile：

- `lto = "fat"`
- `codegen-units = 1`
- `opt-level = 3`
- `panic = "abort"`
- `strip = true`

Tauri 配置使用：

- NSIS 安装包；
- 计算机范围安装，默认 `C:\Program Files\Remote Buddy`；
- 安装前清理旧版按用户安装；
- WebView2 静默引导；
- 自定义 CSP；
- 本地资源协议；
- 禁用不需要的 WebView 后台网络功能。

## 7. 代码结构

```text
crates/core
  config/        配置模型、默认值、迁移和原子写入
  device/        BLE 候选、重连监督器
  input/         按键、手势、快捷键
  protocol/      ATVV 控制包
  audio/         ADPCM、PCM 流、电平计算

crates/windows
  ble/           Windows BLE/GATT
  input/         ETW、Raw Input、SendInput
  audio.rs       WASAPI 输出和端点枚举
  apps.rs        窗口枚举、应用激活
  elevation.rs   UAC 提权
  capture_profile.rs  BthPort ETW 配置

apps/desktop
  lib.rs         Tauri 命令、窗口布局修复
  state.rs       应用快照和配置保存
  input.rs       按键路由和手势
  session.rs     语音状态机
  tray.rs        托盘菜单

ui
  React 19、TypeScript、Vite、Tailwind CSS
```

## 8. 测试策略

### 自动化测试

- `crates/core`：配置迁移、ADPCM、手势、协议编解码；
- `crates/windows`：按键报告解码、应用匹配；
- `ui`：状态、展示层、快捷键和电平格式；
- Rust workspace 编译、Clippy 和格式检查。

### 硬件测试

硬件测试需要真实 G9N9N 遥控器，至少覆盖：

- Windows 蓝牙配对；
- 自动发现和连接；
- BthPort ETW 启动与设备绑定；
- 单击、双击、长按、连续重复；
- 方向键、OK、返回、主页、菜单；
- 音量、静音、播放/暂停、电源；
- YouTube / Netflix / Input 等应用键；
- 松开事件和去重；
- ATVV 语音触发和音频电平；
- 托盘隐藏 10 分钟后的重连；
- 蓝牙开关、睡眠/唤醒；
- 双显示器和 DPI 变化；
- Magpie 等覆盖层下的应用切换；
- 管理员目标应用切换。

测试记录必须区分：

- 编译通过；
- 自动化测试通过；
- 在当前电脑实机通过；
- 尚未在 Windows 10 或其他适配器上验证。

## 9. 关键约束

- 不要用 HID/Frida 路径替换 BthPort ETW 主路径，除非有新证据证明其在目标系统可行。
- 不得只按时间顺序或“只有一台设备”来绑定蓝牙 ACL。
- 完整按键报告是 8 字节；不要把一字节媒体片段误判为 Mute。
- 关闭窗口到托盘时，设备连接必须继续工作。
- 改变窗口大小时，WebView 客户区必须与 Win32 client rect 一致。
- 应用切换只能恢复最小化窗口，不应无条件还原最大化的目标窗口。
- 用户保存映射后必须立即生效。

## 10. 常见构建问题

### 找不到 `cl.exe`

运行 `build.ps1` 会自动进入 MSVC 环境；手动构建时可先执行：

```powershell
& 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\Common7\Tools\Launch-VsDevShell.ps1' `
  -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
```

### WebView 打开空白页

确认正式构建带有 `custom-protocol`，否则程序会尝试连接开发服务器 `localhost:1420`。

### 安装包未生成

NSIS 只在 Release 构建中生成。确认：

```powershell
.\scripts\build.ps1 -Configuration Release
```

并检查 `scripts/build.ps1` 中的 Tauri CLI 路径。

## 11. 仓库卫生

不要提交：

- `target/`；
- `ui/node_modules/`；
- `ui/dist/`；
- 日志、ETL、XML、CSV、诊断包；
- `%APPDATA%\RemoteBuddy` 下的配置；
- 含真实蓝牙地址或用户名的截图；
- 访问令牌和证书私钥。

提交前运行：

```powershell
git status --short
git diff --check
```
