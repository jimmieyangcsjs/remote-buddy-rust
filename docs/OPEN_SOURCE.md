# 开源与合规说明

本文说明 Remote Buddy 的许可证、参考项目、AI 辅助开发、第三方组件和再分发注意事项。

## 1. 项目许可证

Remote Buddy 采用：

```text
GNU General Public License v3.0 only
SPDX-License-Identifier: GPL-3.0-only
```

完整许可证文本见仓库根目录的 [LICENSE](../LICENSE)。

### 你可以

- 在个人或商业环境中使用本程序；
- 阅读、研究、修改源码；
- 分发原始版本或修改后的版本；
- 将本项目用于其他自由软件项目。

### 分发时必须

- 向接收者提供 GPL-3.0-only 许可证文本；
- 提供对应的完整源码，或按 GPL 要求提供可取得源码的有效方式；
- 对修改过的文件保留清楚的修改说明；
- 不得对接收者施加 GPL 不允许的额外限制；
- 不得将本项目代码作为专有闭源产品分发。

### 不提供担保

本项目按“现状”提供，不附带任何明示或默示担保，包括适销性、特定用途适用性和不侵权担保。使用者自行承担设备、数据、系统配置和第三方软件兼容性风险。

## 2. AI 辅助开发声明

本项目在需求分析、架构设计、代码实现、测试、界面设计、问题定位和文档整理过程中使用了 **DeepSeek** 进行 AI 辅助编程。

项目维护者的责任包括：

- 审核提交的代码和文档；
- 运行自动化测试与构建；
- 在可用硬件和系统环境中验证关键路径；
- 决定发布内容和版本；
- 对发布包及运行结果负责。

DeepSeek 是独立第三方服务，本项目：

- 不隶属于 DeepSeek；
- 未获得 DeepSeek 的官方认证、赞助或背书；
- 不声称 AI 生成的代码天然正确或适合所有环境。

AI 辅助产生的代码在本仓库中同样受 GPL-3.0-only 约束。

## 3. 参考项目

本项目在产品目标、设备协议研究和使用流程上参考：

[zfdang/remote-buddy-windows](https://github.com/zfdang/remote-buddy-windows)

根据该仓库的公开信息，其许可证为 GNU General Public License v3.0。

Rust + Tauri 2 实现为本项目独立开发。上游项目的：

- 源代码；
- 文档；
- 项目名称；
- 图标和品牌材料；
- 历史提交和作者权利；

仍归其原作者及贡献者所有。本项目不宣称获得上游作者、Google 或任何设备厂商的官方认可。

如果后续引入或参考了上游的具体代码片段，应在对应文件或提交中保留来源、版权和许可证说明。

## 4. 主要第三方组件

下表列出主要直接依赖。完整依赖清单和精确版本以
[Cargo.lock](../Cargo.lock)、[ui/pnpm-lock.yaml](../ui/pnpm-lock.yaml) 以及各包内许可证为准。

### 4.1 Rust 与桌面框架

| 组件 | 用途 | 主要许可证 |
| --- | --- | --- |
| Tauri 2 | 桌面窗口、IPC、托盘、打包 | MIT 或 Apache-2.0 |
| Tokio | 异步运行时 | MIT |
| Serde / serde_json | 配置和 IPC 序列化 | MIT 或 Apache-2.0 |
| windows-rs | Windows API 绑定 | MIT 或 Apache-2.0 |
| cpal | WASAPI 音频输出 | Apache-2.0 |
| thiserror | 错误类型 | MIT 或 Apache-2.0 |
| tracing | 日志与诊断 | MIT |
| zip | 诊断包压缩 | MIT |
| chrono | 时间戳 | MIT 或 Apache-2.0 |
| crossbeam-channel | 跨线程事件通道 | MIT 或 Apache-2.0 |
| atomic-write-file | 配置原子写入 | MIT |
| tauri-plugin-single-instance | 单实例运行 | MIT 或 Apache-2.0 |

### 4.2 前端

| 组件 | 用途 | 主要许可证 |
| --- | --- | --- |
| React / React DOM | 用户界面 | MIT |
| TypeScript | 类型检查 | Apache-2.0 |
| Vite | 前端构建 | MIT |
| Tailwind CSS | 样式系统 | MIT |
| Radix UI | 无障碍交互组件 | MIT |
| Lucide React | 图标 | ISC |
| Motion | 动画 | MIT |
| Zustand | 前端状态管理 | MIT |
| Manrope / JetBrains Mono | 界面字体 | SIL Open Font License 1.1 |

### 4.3 外部软件

| 软件 | 关系 | 说明 |
| --- | --- | --- |
| VB-CABLE | 用户自行安装 | VB-Audio 的第三方虚拟音频设备，不随本仓库或安装包分发 |
| Microsoft Edge WebView2 Runtime | 系统/用户环境 | 安装包仅内嵌微软提供的静默引导程序；运行时由微软授权 |
| 微信输入法 / 搜狗输入法 | 用户自行安装 | 负责语音识别和文本上屏，本项目不包含其代码 |

## 5. 构建与源码完整性

发布包由仓库内源码和以下公开脚本构建：

```text
scripts/build.ps1
scripts/validate.ps1
```

构建要求、依赖版本和验证步骤见 [DEVELOPMENT.md](DEVELOPMENT.md)。

发布包中的前端资源由 `ui` 目录构建，Rust 二进制由 workspace 源码构建。GPL 要求的对应源码可通过本仓库获取。

## 6. 隐私与数据处理

- 程序不使用账号；
- 不包含遥测 SDK；
- 不向开发者服务器发送音频、转写文本、按键记录或配置；
- 语音仅在本机内存和 VB-CABLE 之间流转；
- 日志、配置和诊断包保存在本机。

详细说明见 [PRIVACY.md](../PRIVACY.md)。

## 7. 商标与免责声明

本项目名称、图标和界面为本项目独立设计。

以下名称或商标归其各自权利人所有：

- Google、Chromecast、Google TV、YouTube；
- Netflix；
- Microsoft、Windows、WebView2；
- 微信、微信输入法；
- 搜狗输入法；
- VB-Audio、VB-CABLE；
- 其他在界面或文档中出现的第三方名称。

本项目：

- 不是 Google 官方产品；
- 不是 Netflix、Microsoft 或腾讯官方产品；
- 未获得上述公司的认证、赞助或背书；
- 不提供设备固件、驱动修复或绕过系统安全策略的功能。

## 8. 安全问题

安全问题报告方式见 [SECURITY.md](../SECURITY.md)。请不要在公开 Issue 中粘贴：

- 完整蓝牙地址；
- 本机用户名或路径；
- 配置备份；
- 未脱敏诊断包；
- 访问令牌或其他凭据。
