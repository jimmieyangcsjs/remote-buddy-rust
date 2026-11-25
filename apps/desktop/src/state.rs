use crate::input::spawn_input_router;
use crate::logger::{AppLogger, LogEntry};
use crate::session::BleBackend;
use remote_buddy_core::config::{
    ActionConfig, AppConfig, DeviceConfig, MappingConfig, MediaControl, VoiceMode,
    default_config_path, default_mappings, load_config, save_config_with_backup,
};
use remote_buddy_core::device::{
    ConnectionSnapshot, DeviceCandidate, DeviceState, SupervisorHandle, spawn_supervisor,
    upsert_candidate,
};
use remote_buddy_core::input::{RemoteButton, parse_shortcut};
use remote_buddy_windows::input::{
    EtwInputListener, EtwStatus, HidReportListener, HidReportLogLevel, RawInputListener,
    spawn_hid_report_listener, spawn_raw_input_listener,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock as StdRwLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceView {
    pub id: String,
    pub name: String,
    pub model: String,
    pub address: String,
    pub container_id: String,
    pub battery: i32,
    pub connection: String,
    pub last_seen: String,
    pub is_default: bool,
    pub voice_mode: String,
    pub gain: f32,
    pub mappings: Vec<MappingView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingView {
    pub button: String,
    pub label: String,
    pub click: String,
    pub double_click: String,
    pub long_press: String,
    pub raw: MappingConfig,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsView {
    pub language: String,
    pub launch_at_startup: bool,
    pub silent_start: bool,
    pub minimize_to_tray: bool,
    pub run_as_admin: bool,
    pub running_as_admin: bool,
    pub input_tool: String,
    pub trigger_mode: String,
    pub shortcut: String,
    pub audio_input: String,
    pub audio_output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HidTapView {
    pub state: String,
    pub detail: String,
    pub source: String,
    pub packets: u64,
    pub redacted: u64,
    pub sensitive_data: bool,
    pub setup_required: bool,
    pub last_event: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioStatusView {
    pub state: String,
    pub detail: String,
    pub devices: Vec<String>,
    pub input_devices: Vec<String>,
    pub output_ready: bool,
    pub input_ready: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceDiscoveryView {
    pub state: String,
    pub detail: String,
    pub candidates: Vec<DeviceCandidate>,
}

impl DeviceDiscoveryView {
    fn pending() -> Self {
        Self {
            state: "pending".to_string(),
            detail: "等待扫描 Windows 蓝牙列表".to_string(),
            candidates: Vec::new(),
        }
    }
}

struct CachedAudioStatus {
    refreshed_at: Instant,
    status: AudioStatusView,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSnapshot {
    pub version: String,
    pub config_path: String,
    pub devices: Vec<DeviceView>,
    pub selected_device_id: Option<String>,
    pub connection: ConnectionSnapshot,
    pub voice_active: bool,
    pub hid_tap: HidTapView,
    pub audio: AudioStatusView,
    pub discovery: DeviceDiscoveryView,
    pub settings: SettingsView,
    pub logs: Vec<LogEntry>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsUpdate {
    pub language: String,
    pub launch_at_startup: bool,
    pub silent_start: bool,
    pub minimize_to_tray: bool,
    pub run_as_admin: bool,
    pub input_tool: String,
    pub trigger_mode: String,
    pub shortcut: String,
    pub audio_input: String,
    pub audio_output: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSettingsUpdate {
    pub gain: f32,
    pub voice_mode: VoiceMode,
}

pub struct AppState {
    pub config: Arc<RwLock<AppConfig>>,
    pub config_path: PathBuf,
    pub supervisor: SupervisorHandle,
    pub logger: AppLogger,
    pub voice_active: Arc<AtomicBool>,
    pub voice_gain: Arc<AtomicU32>,
    #[allow(dead_code)]
    pub raw_input: Mutex<Option<RawInputListener>>,
    #[allow(dead_code)]
    pub hid_report: Mutex<Option<HidReportListener>>,
    pub etw_status: Arc<StdRwLock<EtwStatus>>,
    pub etw_listener: Mutex<Option<EtwInputListener>>,
    pub minimize_to_tray: AtomicBool,
    discovery: StdRwLock<DeviceDiscoveryView>,
    audio_cache: StdRwLock<Option<CachedAudioStatus>>,
}

impl AppState {
    pub fn new(app: AppHandle) -> Self {
        let logger = AppLogger::new(app.clone());
        let config_path = default_config_path();
        let mut config = load_initial_config(&config_path, &logger);
        let before_discovery = config.clone();
        let discovery = discover_and_merge(&mut config, &logger);
        if config != before_discovery
            && let Err(error) = save_config_with_backup(&config_path, &config)
        {
            logger.warn(format!("保存发现到的设备失败：{error}"));
        }
        let minimize_to_tray = config.minimize_to_tray;
        let initial_device = config
            .selected_device_id
            .clone()
            .or_else(|| config.devices.first().map(|device| device.id.clone()));
        let initial_gain = config
            .selected_device_id
            .as_deref()
            .and_then(|id| config.device(id))
            .or_else(|| config.devices.first())
            .map(|device| device.gain)
            .unwrap_or(3.62)
            .clamp(0.1, 16.0);
        let config = Arc::new(RwLock::new(config));
        let voice_active = Arc::new(AtomicBool::new(false));
        let voice_gain = Arc::new(AtomicU32::new(initial_gain.to_bits()));
        let etw_status = Arc::new(StdRwLock::new(EtwStatus::default()));

        let (supervisor, raw_input, hid_report, etw_listener) = tauri::async_runtime::block_on(
            async {
                let (frame_tx, frame_rx) = tokio::sync::mpsc::unbounded_channel();
                let raw_input = spawn_raw_input_listener(frame_tx.clone()).ok();
                let hid_logger = logger.clone();
                let hid_report =
                    match spawn_hid_report_listener(frame_tx.clone(), move |level, message| {
                        match level {
                            HidReportLogLevel::Info => hid_logger.info(message),
                            HidReportLogLevel::Warn => hid_logger.warn(message),
                        }
                    }) {
                        Ok(listener) => {
                            logger.info("HID 报告采集已启动，等待遥控器按键");
                            Some(listener)
                        }
                        Err(error) => {
                            logger.warn(format!("HID 报告采集启动失败：{error}"));
                            None
                        }
                    };
                let etw_listener = match EtwInputListener::spawn(frame_tx, etw_status.clone()) {
                    Ok(listener) => {
                        if remote_buddy_windows::capture_profile::capture_profile_ready() {
                            logger.info("BthPort ETW 按键采集已启动");
                        } else {
                            logger.warn(
                                "按键增强未启用，BthPort ETW 全局会话保持关闭，避免影响其他蓝牙设备",
                            );
                        }
                        Some(listener)
                    }
                    Err(error) => {
                        logger.warn(format!("BthPort ETW 启动失败：{error}"));
                        None
                    }
                };

                let backend = BleBackend::new(
                    config.clone(),
                    logger.clone(),
                    app.clone(),
                    voice_active.clone(),
                    voice_gain.clone(),
                    etw_listener.clone(),
                );
                let supervisor = spawn_supervisor(backend, initial_device);
                let mut states = supervisor.subscribe();
                let app_for_states = app.clone();
                tauri::async_runtime::spawn(async move {
                    while states.changed().await.is_ok() {
                        let snapshot = states.borrow().clone();
                        crate::tray::update_connection(&app_for_states, snapshot.state);
                        let _ = app_for_states.emit("device-state-changed", snapshot);
                    }
                });

                spawn_input_router(app.clone(), config.clone(), logger.clone(), frame_rx);
                (supervisor, raw_input, hid_report, etw_listener)
            },
        );

        Self {
            config,
            config_path,
            supervisor,
            logger,
            voice_active,
            voice_gain,
            raw_input: Mutex::new(raw_input),
            hid_report: Mutex::new(hid_report),
            etw_status,
            etw_listener: Mutex::new(etw_listener),
            minimize_to_tray: AtomicBool::new(minimize_to_tray),
            discovery: StdRwLock::new(discovery),
            audio_cache: StdRwLock::new(None),
        }
    }

    pub async fn snapshot(&self) -> AppSnapshot {
        let etw_ready = self
            .etw_listener
            .lock()
            .map(|listener| listener.is_some())
            .unwrap_or(false);
        let etw_status = self
            .etw_status
            .read()
            .map(|status| status.clone())
            .unwrap_or_default();
        let capture_configured = remote_buddy_windows::capture_profile::capture_profile_ready();
        let config = self.config.read().await;
        let connection = self.supervisor.state();
        let selected = config
            .selected_device_id
            .clone()
            .or_else(|| config.devices.first().map(|device| device.id.clone()));
        let devices = config
            .devices
            .iter()
            .map(|device| {
                let is_selected = selected.as_deref() == Some(device.id.as_str());
                let state = if is_selected {
                    connection.state
                } else {
                    DeviceState::Disconnected
                };
                DeviceView::from_config(device, is_selected, state)
            })
            .collect::<Vec<_>>();

        let audio = self.audio_status(false);
        let discovery = self
            .discovery
            .read()
            .map(|discovery| discovery.clone())
            .unwrap_or_else(|_| DeviceDiscoveryView::pending());

        AppSnapshot {
            version: env!("CARGO_PKG_VERSION").to_string(),
            config_path: self.config_path.display().to_string(),
            devices,
            selected_device_id: selected,
            connection,
            voice_active: self.voice_active.load(Ordering::Relaxed),
            hid_tap: HidTapView {
                state: if capture_configured && !etw_status.sensitive_data {
                    "restartRequired".to_string()
                } else if etw_ready {
                    etw_status.state.clone()
                } else {
                    "unavailable".to_string()
                },
                detail: if capture_configured && !etw_status.sensitive_data {
                    "按键增强已配置，但 Windows 蓝牙驱动尚未重新加载。请关闭并重新打开蓝牙，或重启电脑。"
                        .to_string()
                } else if etw_ready {
                    format!(
                        "{}；事件 {} · 已绑定 {} 台 · 按键报告 {} · 被省略 {}",
                        etw_status.detail,
                        etw_status.packets,
                        etw_status.bound_devices,
                        etw_status.reports,
                        etw_status.redacted
                    )
                } else {
                    "BthPort ETW 未启动；普通键将仅依赖 Raw Input 兼容路径".to_string()
                },
                source: "BthPort ETW".to_string(),
                packets: etw_status.packets,
                redacted: etw_status.redacted,
                sensitive_data: etw_status.sensitive_data,
                setup_required: !capture_configured,
                last_event: etw_status.last_event,
            },
            audio,
            discovery,
            settings: SettingsView {
                language: config.language.clone(),
                launch_at_startup: config.launch_at_startup,
                silent_start: config.silent_start,
                minimize_to_tray: config.minimize_to_tray,
                run_as_admin: config.run_as_admin,
                running_as_admin: remote_buddy_windows::elevation::is_elevated(),
                input_tool: config.input_tool.tool.clone(),
                trigger_mode: config.input_tool.trigger_mode.clone(),
                shortcut: config.input_tool.shortcut.clone(),
                audio_input: config.input_tool.preferred_audio_input_device.clone(),
                audio_output: config.input_tool.preferred_audio_output_device.clone(),
            },
            logs: self.logger.entries(),
        }
    }

    fn audio_status(&self, force: bool) -> AudioStatusView {
        if !force
            && let Ok(cache) = self.audio_cache.read()
            && let Some(cached) = cache.as_ref()
            && cached.refreshed_at.elapsed() < Duration::from_secs(3)
        {
            return cached.status.clone();
        }

        let status = match (
            remote_buddy_windows::audio::list_output_devices(),
            remote_buddy_windows::audio::list_input_devices(),
        ) {
            (Ok(outputs), Ok(inputs)) => {
                let output_ready = outputs.iter().any(|device| {
                    let name = device.name.to_ascii_lowercase();
                    name.contains("cable input")
                        || (name.contains("cable") && name.contains("input"))
                });
                let input_ready = inputs.iter().any(|device| {
                    let name = device.name.to_ascii_lowercase();
                    name.contains("cable output")
                        || (name.contains("cable") && name.contains("output"))
                });
                let detail = match (output_ready, input_ready) {
                    (true, true) => "CABLE Input 与 CABLE Output 均已就绪".to_string(),
                    (true, false) => {
                        "已找到 CABLE Input，但未找到 CABLE Output 录音端点".to_string()
                    }
                    (false, true) => {
                        "已找到 CABLE Output，但未找到 CABLE Input 播放端点".to_string()
                    }
                    (false, false) => "未检测到 CABLE Input / CABLE Output".to_string(),
                };
                AudioStatusView {
                    state: if output_ready && input_ready {
                        "ready"
                    } else {
                        "degraded"
                    }
                    .to_string(),
                    detail,
                    devices: outputs.into_iter().map(|device| device.name).collect(),
                    input_devices: inputs.into_iter().map(|device| device.name).collect(),
                    output_ready,
                    input_ready,
                }
            }
            (Err(error), _) | (_, Err(error)) => AudioStatusView {
                state: "error".to_string(),
                detail: error.to_string(),
                devices: Vec::new(),
                input_devices: Vec::new(),
                output_ready: false,
                input_ready: false,
            },
        };
        if let Ok(mut cache) = self.audio_cache.write() {
            *cache = Some(CachedAudioStatus {
                refreshed_at: Instant::now(),
                status: status.clone(),
            });
        }
        status
    }

    pub async fn discover_devices(&self) -> Result<AppSnapshot, String> {
        let scanned =
            tauri::async_runtime::spawn_blocking(remote_buddy_windows::ble::list_paired_remotes)
                .await
                .map_err(|error| format!("蓝牙扫描任务失败：{error}"))?
                .map_err(|error| format!("无法读取 Windows 蓝牙设备：{error}"))?;

        let view = discovery_view(&scanned);
        let mut next = self.config.read().await.clone();
        let before = next.clone();
        for candidate in &scanned {
            let _ = upsert_candidate(&mut next, candidate);
        }
        let changed = next != before;
        let selected = next
            .selected_device_id
            .clone()
            .or_else(|| next.devices.first().map(|device| device.id.clone()));
        if changed {
            save_config_with_backup(&self.config_path, &next)
                .map_err(|error| format!("保存发现到的设备失败：{error}"))?;
            *self.config.write().await = next;
        }
        if let Ok(mut discovery) = self.discovery.write() {
            *discovery = view.clone();
        }
        if changed
            && self.supervisor.state().device_id.is_none()
            && let Some(selected) = selected.clone()
        {
            self.supervisor
                .connect(selected)
                .await
                .map_err(|error| error.to_string())?;
        }
        self.logger.info(view.detail.clone());
        Ok(self.snapshot().await)
    }

    pub async fn save(&self) -> Result<(), String> {
        let config = self.config.read().await.clone();
        save_config_with_backup(&self.config_path, &config).map_err(|error| error.to_string())
    }

    pub async fn connect(&self, device_id: &str) -> Result<AppSnapshot, String> {
        {
            let mut config = self.config.write().await;
            if config.device(device_id).is_none() {
                return Err(format!("设备 {device_id} 不在配置中"));
            }
            config.selected_device_id = Some(device_id.to_string());
        }
        self.save().await?;
        self.supervisor
            .connect(device_id)
            .await
            .map_err(|error| error.to_string())?;
        self.logger.info(format!("用户请求连接设备 {device_id}"));
        Ok(self.snapshot().await)
    }

    pub async fn disconnect(&self) -> Result<AppSnapshot, String> {
        self.supervisor
            .disconnect()
            .await
            .map_err(|error| error.to_string())?;
        self.logger.warn("用户手动断开设备连接");
        Ok(self.snapshot().await)
    }

    pub async fn retry(&self) -> Result<AppSnapshot, String> {
        self.supervisor
            .retry_now()
            .await
            .map_err(|error| error.to_string())?;
        self.logger.info("用户触发立即重连");
        Ok(self.snapshot().await)
    }

    pub async fn select_device(&self, device_id: &str) -> Result<AppSnapshot, String> {
        {
            let mut config = self.config.write().await;
            if config.device(device_id).is_none() {
                return Err(format!("设备 {device_id} 不在配置中"));
            }
            config.selected_device_id = Some(device_id.to_string());
        }
        self.save().await?;
        Ok(self.snapshot().await)
    }

    pub async fn save_mapping(
        &self,
        device_id: &str,
        mut mapping: MappingConfig,
    ) -> Result<AppSnapshot, String> {
        let button = RemoteButton::from_key(&mapping.button)
            .filter(|button| *button != RemoteButton::Unknown)
            .ok_or_else(|| format!("未知按键：{}", mapping.button))?;
        mapping.button = button.as_str().to_string();
        validate_action(&mapping.click)?;
        validate_action(&mapping.double_click)?;
        validate_action(&mapping.long_press)?;

        {
            let mut config = self.config.write().await;
            let device = config
                .device_mut(device_id)
                .ok_or_else(|| format!("设备 {device_id} 不在配置中"))?;
            if let Some(existing) = device
                .mappings
                .iter_mut()
                .find(|existing| existing.button.eq_ignore_ascii_case(&mapping.button))
            {
                *existing = mapping;
            } else {
                device.mappings.push(mapping);
            }
        }
        self.save().await?;
        self.logger.info(format!("已保存 {device_id} 的按键映射"));
        Ok(self.snapshot().await)
    }

    pub async fn reset_mappings(&self, device_id: &str) -> Result<AppSnapshot, String> {
        {
            let mut config = self.config.write().await;
            let device = config
                .device_mut(device_id)
                .ok_or_else(|| format!("设备 {device_id} 不在配置中"))?;
            restore_default_mappings(device);
        }
        self.save().await?;
        self.logger
            .info(format!("已恢复 {device_id} 的默认按键映射"));
        Ok(self.snapshot().await)
    }

    pub async fn save_settings(&self, update: SettingsUpdate) -> Result<AppSnapshot, String> {
        if update.trigger_mode != "HoldToTalk"
            && update.trigger_mode != "ClickToToggle"
            && update.trigger_mode != "SmartToggle"
        {
            return Err(format!("不支持的触发模式：{}", update.trigger_mode));
        }
        if parse_shortcut(update.shortcut.trim()).is_none() {
            return Err(format!("无效输入法快捷键：{}", update.shortcut));
        }
        if update.audio_output.trim().is_empty() {
            return Err("音频输出设备不能为空".to_string());
        }

        let minimize_to_tray = update.minimize_to_tray;
        {
            let mut config = self.config.write().await;
            let mut next = config.clone();
            next.language = update.language;
            next.launch_at_startup = update.launch_at_startup;
            next.silent_start = update.silent_start;
            next.minimize_to_tray = minimize_to_tray;
            next.run_as_admin = update.run_as_admin;
            next.input_tool.tool = update.input_tool;
            next.input_tool.trigger_mode = update.trigger_mode;
            next.input_tool.shortcut = update.shortcut;
            next.input_tool.preferred_audio_input_device = update.audio_input;
            next.input_tool.preferred_audio_output_device = update.audio_output;
            save_config_with_backup(&self.config_path, &next).map_err(|error| error.to_string())?;
            *config = next;
        }
        self.minimize_to_tray
            .store(minimize_to_tray, Ordering::Relaxed);
        self.logger.info("设置已保存");
        Ok(self.snapshot().await)
    }

    pub async fn save_voice_settings(
        &self,
        device_id: &str,
        update: VoiceSettingsUpdate,
    ) -> Result<AppSnapshot, String> {
        if !update.gain.is_finite() || !(0.1..=16.0).contains(&update.gain) {
            return Err("语音增益必须在 0.1 到 16.0 之间".to_string());
        }
        {
            let mut config = self.config.write().await;
            let device = config
                .device_mut(device_id)
                .ok_or_else(|| format!("设备 {device_id} 不在配置中"))?;
            device.gain = update.gain;
            device.voice_mode = update.voice_mode;
        }
        self.voice_gain
            .store(update.gain.to_bits(), Ordering::Relaxed);
        self.save().await?;
        self.logger.info(format!("已保存 {device_id} 的语音设置"));
        Ok(self.snapshot().await)
    }

    pub async fn test_audio_output(&self) -> Result<String, String> {
        let device = {
            let config = self.config.read().await;
            config.input_tool.preferred_audio_output_device.clone()
        };
        let device_for_task = device.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let output = remote_buddy_windows::audio::open_output(&device_for_task, 0.25)
                .map_err(|error| error.to_string())?;
            let mut samples = Vec::with_capacity(16_000 * 2 / 5);
            for index in 0..(16_000 * 2 / 5) {
                let phase = std::f32::consts::TAU * 440.0 * index as f32 / 16_000.0;
                let edge = (index as f32 / 1_600.0).min(1.0)
                    * (((16_000 * 2 / 5 - index) as f32) / 1_600.0).min(1.0);
                samples.push((phase.sin() * edge * 0.35 * f32::from(i16::MAX)) as i16);
            }
            output.push_pcm16(&samples);
            std::thread::sleep(std::time::Duration::from_millis(550));
            Ok::<_, String>(device_for_task)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    pub async fn export_diagnostics(&self) -> Result<String, String> {
        let snapshot = self.snapshot().await;
        let config = self.config.read().await.clone();
        let directory = self
            .config_path
            .parent()
            .map(|parent| parent.join("diagnostics"))
            .ok_or_else(|| "无法定位配置目录".to_string())?;
        let system = serde_json::json!({
            "appVersion": env!("CARGO_PKG_VERSION"),
            "targetOs": std::env::consts::OS,
            "targetArch": std::env::consts::ARCH,
            "exportedAt": chrono::Local::now().to_rfc3339(),
            "configPath": snapshot.config_path,
            "connection": snapshot.connection,
            "audio": snapshot.audio,
            "settings": snapshot.settings,
        });
        let log_path = self
            .config_path
            .parent()
            .map(|parent| parent.join("logs").join("rust-app.log"));
        let logs = log_path
            .as_deref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .unwrap_or_else(|| {
                snapshot
                    .logs
                    .iter()
                    .map(|entry| format!("{} {} {}\n", entry.time, entry.level, entry.message))
                    .collect()
            });
        let hid_tap = serde_json::to_value(&snapshot.hid_tap).map_err(|error| error.to_string())?;
        let path = crate::diagnostics::export(&directory, &system, &config, &logs, &hid_tap)?;
        self.logger
            .info(format!("诊断信息已导出到 {}", path.display()));
        Ok(path.display().to_string())
    }

    #[allow(dead_code)]
    pub fn reset_raw_input(&self) {
        if let Ok(mut listener) = self.raw_input.lock() {
            *listener = None;
        }
    }
}

fn discover_and_merge(config: &mut AppConfig, logger: &AppLogger) -> DeviceDiscoveryView {
    match remote_buddy_windows::ble::list_paired_remotes() {
        Ok(candidates) => {
            let before = config.clone();
            for candidate in &candidates {
                let _ = upsert_candidate(config, candidate);
            }
            let view = discovery_view(&candidates);
            if *config != before {
                logger.info(format!(
                    "{}；已自动导入 {} 台受支持的遥控器",
                    view.detail,
                    candidates.len()
                ));
            } else {
                logger.info(view.detail.clone());
            }
            view
        }
        Err(error) => {
            let detail = format!("无法读取 Windows 蓝牙设备：{error}");
            logger.warn(detail.clone());
            DeviceDiscoveryView {
                state: "error".to_string(),
                detail,
                candidates: Vec::new(),
            }
        }
    }
}

fn discovery_view(candidates: &[DeviceCandidate]) -> DeviceDiscoveryView {
    if candidates.is_empty() {
        return DeviceDiscoveryView {
            state: "empty".to_string(),
            detail:
                "Windows 蓝牙列表中没有受支持的遥控器。请先在系统设置中配对 Chromecast Remote，然后重新扫描。"
                    .to_string(),
            candidates: Vec::new(),
        };
    }
    let connected = candidates
        .iter()
        .filter(|device| device.is_connected)
        .count();
    DeviceDiscoveryView {
        state: "ready".to_string(),
        detail: if connected > 0 {
            format!(
                "Windows 返回 {} 台受支持的遥控器，其中 {} 台当前已连接",
                candidates.len(),
                connected
            )
        } else {
            format!(
                "Windows 返回 {} 台受支持的遥控器，目前均未连接",
                candidates.len()
            )
        },
        candidates: candidates.to_vec(),
    }
}

fn validate_action(action: &ActionConfig) -> Result<(), String> {
    match action {
        ActionConfig::Shortcut { keys } if parse_shortcut(keys).is_none() => {
            Err(format!("无效快捷键：{keys}"))
        }
        ActionConfig::Shortcut { .. } | ActionConfig::None | ActionConfig::Text { .. } => Ok(()),
        ActionConfig::Media { .. } => Ok(()),
        ActionConfig::LaunchApp { path, .. } if path.trim().is_empty() => {
            Err("切换应用需要选择或填写程序路径".to_string())
        }
        ActionConfig::LaunchApp { .. } => Ok(()),
        ActionConfig::ContinuousRepeat { interval_ms } if !(20..=1_000).contains(interval_ms) => {
            Err("连发间隔必须在 20 到 1000 ms 之间".to_string())
        }
        ActionConfig::ContinuousRepeat { .. } => Ok(()),
    }
}

impl DeviceView {
    fn from_config(device: &DeviceConfig, is_default: bool, state: DeviceState) -> Self {
        Self {
            id: device.id.clone(),
            name: device.name.clone(),
            model: device.model_type.clone(),
            address: device.formatted_address.clone(),
            container_id: device.container_id.clone().unwrap_or_default(),
            battery: -1,
            connection: state.as_str().to_string(),
            last_seen: if state == DeviceState::Ready {
                "刚刚".to_string()
            } else {
                "—".to_string()
            },
            is_default,
            voice_mode: voice_mode_key(device.voice_mode).to_string(),
            gain: device.gain,
            mappings: device
                .mappings
                .iter()
                .map(|mapping| MappingView {
                    button: mapping.button.clone(),
                    label: button_label(&mapping.button),
                    click: action_label(&mapping.click),
                    double_click: action_label(&mapping.double_click),
                    long_press: action_label(&mapping.long_press),
                    raw: mapping.clone(),
                })
                .collect(),
        }
    }
}

pub fn restore_default_mappings(device: &mut DeviceConfig) {
    device.mappings = default_mappings(&device.model_type);
}

fn load_initial_config(path: &PathBuf, logger: &AppLogger) -> AppConfig {
    let raw = std::fs::read_to_string(path).ok();
    let config = match raw
        .as_deref()
        .map(remote_buddy_core::config::parse_and_migrate)
    {
        Some(Ok(config)) => config,
        Some(Err(error)) => {
            logger.error(format!("配置读取失败，已使用默认配置：{error}"));
            AppConfig::default()
        }
        None => load_config(path).unwrap_or_else(|error| {
            logger.error(format!("配置加载失败：{error}"));
            AppConfig::default()
        }),
    };

    if let Some(raw) = raw
        && !raw.contains("schemaVersion")
        && !raw.contains("schema_version")
        && let Err(error) = save_config_with_backup(path, &config)
    {
        logger.warn(format!("迁移配置保存失败：{error}"));
    }
    config
}

pub fn action_label(action: &ActionConfig) -> String {
    match action {
        ActionConfig::None => "无动作".to_string(),
        ActionConfig::Shortcut { keys } => keys.clone(),
        ActionConfig::Text { text } => format!("文本 · {text}"),
        ActionConfig::Media { control } => match control {
            MediaControl::VolumeUp => "音量 +".to_string(),
            MediaControl::VolumeDown => "音量 -".to_string(),
            MediaControl::Mute => "静音".to_string(),
            MediaControl::PlayPause => "播放/暂停".to_string(),
        },
        ActionConfig::LaunchApp { path, name } => {
            let label = if name.trim().is_empty() {
                std::path::Path::new(path)
                    .file_stem()
                    .map(|value| value.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.clone())
            } else {
                name.clone()
            };
            format!("切换应用 · {label}")
        }
        ActionConfig::ContinuousRepeat { interval_ms } => {
            format!("连发 · {interval_ms} ms")
        }
    }
}

fn button_label(button: &str) -> String {
    match button {
        "Up" => "上".to_string(),
        "Down" => "下".to_string(),
        "Left" => "左".to_string(),
        "Right" => "右".to_string(),
        "Ok" => "确认".to_string(),
        "Back" => "返回".to_string(),
        "Home" => "主页".to_string(),
        "Menu" => "菜单".to_string(),
        "VolumeUp" => "音量 +".to_string(),
        "VolumeDown" => "音量 -".to_string(),
        "Mute" => "静音".to_string(),
        "PlayPause" => "播放/暂停".to_string(),
        "Power" => "电源".to_string(),
        "YouTube" => "YouTube".to_string(),
        "Netflix" => "Netflix".to_string(),
        "LiveTv" => "电视".to_string(),
        "Input" => "输入源".to_string(),
        "Custom" => "自定义".to_string(),
        other => other.to_string(),
    }
}

fn voice_mode_key(mode: VoiceMode) -> &'static str {
    match mode {
        VoiceMode::SmartToggle => "smartToggle",
        VoiceMode::HoldToTalk => "holdToTalk",
        VoiceMode::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voice_mode_wire_values_match_the_typescript_contract() {
        assert_eq!(voice_mode_key(VoiceMode::SmartToggle), "smartToggle");
        assert_eq!(voice_mode_key(VoiceMode::HoldToTalk), "holdToTalk");
        assert_eq!(voice_mode_key(VoiceMode::Unknown), "unknown");
    }
}
