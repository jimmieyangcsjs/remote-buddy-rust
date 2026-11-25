# Remote Buddy v0.2.1

本次更新集中解决蓝牙连接稳定性、Intel 蓝牙兼容性和安装升级路径问题。

## 本次更新

### 蓝牙连接改为保活

- 连接成功后启用 Windows GATT 连接保活，不再周期性主动断开重连。
- 移除连接建立后的每秒 GATT 探测循环，减少对同一蓝牙适配器上鼠标和键盘的干扰。
- 首次连接失败时停止自动重试，等待用户手动重连。
- 稳定连接超过 60 秒后意外断开，最多执行一次受控恢复；恢复失败后停止自动尝试。
- 启动扫描时先按设备名筛选，再打开受支持的 BLE 遥控器，避免短暂访问已配对的蓝牙鼠标。

### 按键增强与蓝牙兼容恢复

- BthPort 全局 ETW 会话只在启用按键增强时启动。
- 启用按键增强前自动备份原始注册表值到 `%APPDATA%\RemoteBuddy\capture-profile.backup.json`。
- 新增“恢复蓝牙默认设置”，可从诊断页或托盘菜单调用。
- 有备份时精确恢复原值；没有备份时只删除与本程序写入值完全一致的项，不覆盖其他软件配置。
- 文档增加 Intel 蓝牙 Windows 11 环境下蓝牙鼠标断连、驱动告警和反复重连的处理步骤。

### 安装与升级

- NSIS 安装包改为计算机范围安装，默认路径为 `C:\Program Files\Remote Buddy`。
- 安装和升级需要管理员权限。
- 安装器会自动关闭旧的 Remote Buddy 进程，清理旧的 `%LOCALAPPDATA%\Remote Buddy` 副本、快捷方式和 HKCU 卸载项。
- 升级后桌面快捷方式自动指向新的 Program Files 安装位置。

## 安装

推荐下载并运行：

```text
Remote-Buddy_0.2.1_x64-setup.exe
```

独立版：

```text
remote-buddy-desktop.exe
```

安装版默认安装到 `C:\Program Files\Remote Buddy`，内嵌 WebView2 静默引导程序。独立版要求系统已安装 WebView2。

## 从旧版本升级

1. 关闭正在运行的 Remote Buddy；
2. 运行 `Remote-Buddy_0.2.1_x64-setup.exe` 并同意 UAC；
3. 安装器会清理旧版按用户安装；
4. 启动新版，在“设备”页连接遥控器；
5. Intel 蓝牙 Windows 11 用户如遇到蓝牙鼠标异常，先在“诊断”页点击“恢复蓝牙默认设置”，重启电脑确认鼠标稳定后，再按需重新启用按键增强。

恢复蓝牙默认设置后，普通遥控器按键可能暂时不可用；需要时重新启用按键增强即可。

## 验证

- `cargo fmt`：通过
- `cargo test --workspace`：通过
- `cargo clippy --workspace --all-targets -- -D warnings`：通过
- UI 测试：23 项通过
- UI 生产构建：通过
- Release EXE 与 NSIS 安装包：构建成功

自动化结果见 [验证记录](validation.md)。

## 发布资产

- `Remote-Buddy_0.2.1_x64-setup.exe`：x64 NSIS 安装包，推荐使用。
- `remote-buddy-desktop.exe`：独立运行版。
- `SHA256SUMS.txt`：发布文件 SHA-256 校验值。

## 校验

下载 `SHA256SUMS.txt` 后，在 PowerShell 中执行：

```powershell
Get-FileHash '.\Remote-Buddy_0.2.1_x64-setup.exe' -Algorithm SHA256
```

将结果与 `SHA256SUMS.txt` 比较。

## 系统要求

- Windows 10 19041+ 或 Windows 11；
- 仅 x64；
- WebView2；
- 语音功能需要 VB-CABLE 和受支持的输入法。

## 已知限制

- 普通实体按键首次需要 BthPort ETW 配置并重新加载蓝牙。
- 不同蓝牙适配器、驱动和企业策略可能限制 ETW。
- 语音识别由第三方输入法完成，本项目不包含识别模型。
- VB-CABLE 不随项目分发。
- 当前安装包未签名，SmartScreen 可能提示未知发布者。

## 开源与致谢

- 许可证：GPL-3.0-only。
- 参考项目：[zfdang/remote-buddy-windows](https://github.com/zfdang/remote-buddy-windows)。
- AI 辅助开发：DeepSeek。

本项目不是 Google 官方产品，也不代表上游项目作者。
