# 贡献指南

感谢参与 Remote Buddy。提交代码、文档、测试或问题报告前，请先阅读本文件。

## 开发环境

- Windows 10 19041+ 或 Windows 11，仅 x64；
- Visual Studio 2022 Build Tools，含 C++ 桌面开发和 Windows SDK；
- Rust stable 1.89+，目标 `x86_64-pc-windows-msvc`；
- Node.js 22+、pnpm；
- Microsoft Edge WebView2 Runtime；
- 硬件验证需要已配对的 G9N9N 遥控器；
- 语音验证需要 VB-CABLE。

## 提交前检查

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
pnpm --dir ui test
pnpm --dir ui build
```

涉及窗口、托盘或安装包时，还应运行：

```powershell
.\scripts\validate.ps1
.\scripts\build.ps1 -Configuration Release
```

## 代码约定

- 使用 Rust 2024 edition 和项目现有错误处理风格；
- 核心协议、配置、手势逻辑放在 `crates/core`，保持不依赖 Tauri；
- Windows API 调用集中在 `crates/windows`；
- UI 与 Rust 快照的类型定义同步更新 `ui/src/lib/types.ts`；
- 不为通过检查而伪造设备状态；
- 修复问题时优先处理根因，避免只改界面显示；
- 新增可见功能时同步更新 README、功能说明或用户手册。

## 按键和协议修改

修改 BthPort ETW、ATVV、ADPCM 或按键报告解码时，请：

1. 阅读 [协议记录](docs/PROTOCOL.md) 和 [架构说明](docs/ARCHITECTURE.md)；
2. 为纯编解码逻辑增加单元测试；
3. 说明是否在真实遥控器上验证；
4. 记录 Windows 版本、权限状态、蓝牙适配器和输入来源；
5. 不要用“只连接了一台设备”或时间 proximity 作为设备绑定依据。

不要在没有新证据的情况下用 HID/Frida 路径替换 BthPort ETW 主路径。

## 硬件验证记录

硬件行为与编译测试结果分开记录。至少包含：

- Windows 版本和 build；
- 程序是否以管理员权限运行；
- 遥控器型号；
- 按键采集来源；
- 事件是否到达映射和系统动作；
- 失败时的最小复现步骤。

## 隐私

不要提交：

- `%APPDATA%\RemoteBuddy\config.json` 或备份；
- 应用日志；
- ETL/XML 捕获；
- 诊断包；
- 含真实蓝牙地址、用户名或机器路径的截图；
- 访问令牌、证书私钥或凭据。

截图必须遮盖蓝牙地址和设备名。

## 安全与依赖

- 不要扩大外部链接白名单；
- 不要在前端直接执行任意命令；
- 不要引入不必要的网络请求；
- 不要求普通权限即可完成的操作，不应强制提权；
- 新增依赖时说明用途、许可证和体积影响。

安全问题报告方式见 [SECURITY.md](SECURITY.md)。

## 提交与许可

提交贡献即表示你同意将贡献按本项目的 GPL-3.0-only 许可证分发，并确认你有权提交相关代码或素材。

请保持提交范围聚焦。一个提交尽量只解决一个问题，并在说明中写明：

- 修改内容；
- 修改原因；
- 已验证和未验证的范围。
