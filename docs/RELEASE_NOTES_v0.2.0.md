# Remote Buddy v0.2.0

这是 Remote Buddy 的首个完整公开版本。项目由 DeepSeek 辅助完成需求分析、架构设计、代码实现、测试和文档整理，所有提交、构建和发布由维护者审核。

## 主要更新

- 完整中文文档、界面截图、隐私说明、安全策略和 GPL-3.0 开源合规说明。
- 按键映射保存后立即生效。
- 长按支持“触发一次”和“按住保持、松开释放”。
- 修复 Magpie 位于前台时无法激活 ChatGPT、Argus 等应用的问题。
- 修复双显示器最大化、标题栏双击还原和 WebView 布局异常。
- 新增管理员权限启动/即时提权。
- 重绘应用、托盘和遥控器图标，优化中文界面和托盘联动状态。
- 显示真实 RMS / PEAK 音频电平。
- 仅发布 Windows x64，使用 LTO、符号剥离和静态 MSVC 运行库。
- 蓝牙连接改为 GATT 保活，移除周期性 GATT 探测和无限强制重连。
- 按键增强支持原始值备份与一键恢复，改善 Intel 蓝牙环境下的兼容性。

## 安装

推荐下载并运行：

```text
Remote Buddy_0.2.0_x64-setup.exe
```

独立版：

```text
remote-buddy-desktop.exe
```

安装版默认安装到 `C:\Program Files\Remote Buddy`，需要管理员权限，并会在升级时清理旧版按用户安装。独立版要求系统已安装 WebView2。安装版内嵌 WebView2 静默引导程序。

## 首次使用

1. 在 Windows 设置中配对 `Chromecast Remote`；
2. 启动 Remote Buddy，在“设备”页扫描并连接；
3. 在“诊断”页点击“启用按键增强”，同意 UAC，然后重新加载蓝牙或重启；
4. 在“按键映射”页配置单击、双击、长按动作；
5. 语音输入需要安装 VB-CABLE，并在输入法中选择 `CABLE Output` 作为麦克风。

详细步骤见 [用户手册](USER_GUIDE.md)。

## 系统要求

- Windows 10 19041+ 或 Windows 11；
- 仅 x64；
- WebView2；
- 语音功能需要 VB-CABLE 和受支持的输入法。

本版本面向 Windows 10 19041+ 与 Windows 11 x64。当前自动化验证在 Windows 10 19044 x64 上完成；Windows 11 使用相同目标和构建配置，但建议根据“诊断”页逐项确认蓝牙适配器与 ETW 行为。

## 已知限制

- 普通实体按键首次需要 BthPort ETW 配置并重新加载蓝牙。
- 不同蓝牙适配器、驱动和企业策略可能限制 ETW。
- 语音识别由第三方输入法完成，本项目不包含识别模型。
- VB-CABLE 不随项目分发。
- 切换高权限目标应用时，Remote Buddy 也需要管理员权限。
- 当前安装包未签名，SmartScreen 可能提示未知发布者。

## 验证

- Rust workspace 测试：76 项通过。
- UI 测试：23 项通过。
- UI 生产构建：通过。
- Release EXE 与 NSIS 安装包：由本仓库源码构建。

自动化结果见 [验证记录](validation.md)。

## 开源与致谢

- 许可证：GPL-3.0-only。
- 参考项目：[zfdang/remote-buddy-windows](https://github.com/zfdang/remote-buddy-windows)。
- AI 辅助开发：DeepSeek。

本项目不是 Google 官方产品，也不代表上游项目作者。

## 校验

下载 `SHA256SUMS.txt` 后，在 PowerShell 中执行：

```powershell
Get-FileHash '.\Remote Buddy_0.2.0_x64-setup.exe' -Algorithm SHA256
```

将结果与 `SHA256SUMS.txt` 比较。
