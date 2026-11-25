use crate::input::InputEvent;
use crate::logger::AppLogger;
use async_trait::async_trait;
use remote_buddy_core::audio::{AtvvAudioDecoder, AudioLevelAccumulator};
use remote_buddy_core::config::{AppConfig, DeviceConfig};
use remote_buddy_core::device::{LinkBackend, LinkSession};
use remote_buddy_core::protocol::{AtvvControl, AtvvNotification};
use remote_buddy_windows::audio::{AudioError, AudioOutput, open_output_with_gain};
use remote_buddy_windows::ble::atvv_transport::AtvvTransport;
use remote_buddy_windows::input::EtwInputListener;
use remote_buddy_windows::input::{send_shortcut_down, send_shortcut_text, send_shortcut_up};
use serde::Serialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::{RwLock, mpsc::UnboundedReceiver};

const HOLD_THRESHOLD: Duration = Duration::from_millis(700);
const KEEPALIVE_INTERVAL: Duration = Duration::from_secs(20);
const MAX_HANDS_FREE: Duration = Duration::from_secs(300);
const LEVEL_EMIT_INTERVAL: Duration = Duration::from_millis(50);
const VOICE_TRIGGER_DEDUP: Duration = Duration::from_millis(700);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceStateEvent {
    pub active: bool,
    pub hands_free: bool,
    pub phase: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceLevelEvent {
    pub rms: f32,
    pub peak: f32,
    pub sequence: u64,
}

#[derive(Clone)]
pub struct BleBackend {
    config: Arc<RwLock<AppConfig>>,
    logger: AppLogger,
    app: AppHandle,
    voice_active: Arc<AtomicBool>,
    voice_gain: Arc<AtomicU32>,
    etw: Option<EtwInputListener>,
}

impl BleBackend {
    pub fn new(
        config: Arc<RwLock<AppConfig>>,
        logger: AppLogger,
        app: AppHandle,
        voice_active: Arc<AtomicBool>,
        voice_gain: Arc<AtomicU32>,
        etw: Option<EtwInputListener>,
    ) -> Self {
        Self {
            config,
            logger,
            app,
            voice_active,
            voice_gain,
            etw,
        }
    }
}

#[async_trait]
impl LinkBackend for BleBackend {
    type Session = BleSession;

    async fn connect(&self, device_id: &str) -> Result<Self::Session, String> {
        let device = {
            let config = self.config.read().await;
            config
                .device(device_id)
                .cloned()
                .ok_or_else(|| format!("设备 {device_id} 不在配置中"))?
        };
        self.voice_gain
            .store(device.gain.clamp(0.1, 16.0).to_bits(), Ordering::Relaxed);

        self.logger.info(format!(
            "正在连接 {}（{}）",
            device.name, device.formatted_address
        ));
        let mut transport = AtvvTransport::connect(device.bluetooth_address)
            .await
            .map_err(|error| error.to_string())?;
        let notifications = transport
            .take_notifications()
            .ok_or_else(|| "ATVV 控制通知通道不可用".to_string())?;
        let audio_chunks = transport
            .take_audio_chunks()
            .ok_or_else(|| "ATVV 音频通知通道不可用".to_string())?;
        let (voice_fallback_tx, voice_fallback_rx) = tokio::sync::mpsc::unbounded_channel();

        self.logger
            .info(format!("{} 已连接，ATVV 服务就绪", device.name));
        if transport.maintains_connection() {
            self.logger
                .info(format!("{} 已启用 Windows GATT 连接保活", device.name));
        } else {
            self.logger.warn(format!(
                "{} 的蓝牙驱动不支持 GATT 长连接保活，将只在系统报告断线时恢复",
                device.name
            ));
        }

        if let Some(handle) = transport.control_handle() {
            self.logger.info(format!(
                "ATVV 控制句柄 0x{handle:04X}，已启用语音键 ETW 兜底"
            ));
            if let Some(etw) = self.etw.clone() {
                etw.watch_atvv_control(&device.id, handle, voice_fallback_tx);
            }
        } else {
            self.logger
                .warn("无法读取 ATVV 控制句柄，语音键 ETW 兜底不可用");
        }

        if let Some(etw) = self.etw.clone() {
            etw.arm_fallback_binding(&device.id, &device.model_type);
            self.logger
                .info("BthPort ETW 使用按键通知完成绑定，不再循环发起 GATT 探测连接");
        }

        Ok(BleSession {
            device,
            config: self.config.clone(),
            transport,
            notifications,
            audio_chunks,
            voice_fallback_triggers: voice_fallback_rx,
            decoder: AtvvAudioDecoder::new(120),
            audio: None,
            active: false,
            hands_free: false,
            awaiting_reopen: false,
            pressed_at: None,
            started_at: None,
            session_id: 0,
            shortcut_held: None,
            voice_shortcut_sent: false,
            voice_input_down: false,
            last_voice_trigger: None,
            input_sequence: 0,
            last_keepalive: Instant::now(),
            level_accumulator: AudioLevelAccumulator::default(),
            last_level_emit: Instant::now(),
            level_sequence: 0,
            app: self.app.clone(),
            logger: self.logger.clone(),
            voice_active: self.voice_active.clone(),
            voice_gain: self.voice_gain.clone(),
        })
    }
}

pub struct BleSession {
    device: DeviceConfig,
    config: Arc<RwLock<AppConfig>>,
    transport: AtvvTransport,
    notifications: UnboundedReceiver<AtvvNotification>,
    audio_chunks: UnboundedReceiver<Vec<u8>>,
    voice_fallback_triggers: UnboundedReceiver<()>,
    decoder: AtvvAudioDecoder,
    audio: Option<AudioOutput>,
    active: bool,
    hands_free: bool,
    awaiting_reopen: bool,
    pressed_at: Option<Instant>,
    started_at: Option<Instant>,
    session_id: u8,
    shortcut_held: Option<String>,
    voice_shortcut_sent: bool,
    voice_input_down: bool,
    last_voice_trigger: Option<Instant>,
    input_sequence: u64,
    last_keepalive: Instant,
    level_accumulator: AudioLevelAccumulator,
    last_level_emit: Instant,
    level_sequence: u64,
    app: AppHandle,
    logger: AppLogger,
    voice_active: Arc<AtomicBool>,
    voice_gain: Arc<AtomicU32>,
}

#[async_trait]
impl LinkSession for BleSession {
    async fn wait_closed(&mut self) -> String {
        enum Event {
            Timer,
            Notification(Option<AtvvNotification>),
            Audio(Option<Vec<u8>>),
            VoiceFallback,
        }

        loop {
            let event = tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(2)) => Event::Timer,
                notification = self.notifications.recv() => Event::Notification(notification),
                chunk = self.audio_chunks.recv() => Event::Audio(chunk),
                Some(()) = self.voice_fallback_triggers.recv() => Event::VoiceFallback,
            };
            match event {
                Event::Timer => {
                    if !self.transport.is_alive() {
                        self.shutdown_voice("蓝牙连接已断开");
                        return "remote disconnected from Windows Bluetooth".to_string();
                    }
                    self.tick().await;
                }
                Event::Notification(Some(notification)) => {
                    self.handle_notification(notification).await;
                }
                Event::Notification(None) => {
                    self.shutdown_voice("ATVV 控制通道已关闭");
                    return "ATVV control channel closed".to_string();
                }
                Event::Audio(Some(chunk)) => {
                    if self.active {
                        let pcm = self.decoder.decode(&chunk);
                        if !pcm.is_empty() {
                            if let Some(audio) = &self.audio {
                                audio.push_pcm16(&pcm);
                            }
                            self.level_accumulator.push_pcm16(&pcm);
                            if self.last_level_emit.elapsed() >= LEVEL_EMIT_INTERVAL {
                                self.emit_accumulated_level();
                            }
                        }
                    }
                }
                Event::Audio(None) => {}
                Event::VoiceFallback => {
                    self.on_voice_trigger("ETW 兜底").await;
                }
            }
        }
    }
}

impl BleSession {
    async fn handle_notification(&mut self, notification: AtvvNotification) {
        match notification {
            AtvvNotification::StartSearch => self.on_voice_trigger("ATVV").await,
            AtvvNotification::AudioStart {
                codec,
                session_id,
                interaction: _,
            } => self.on_audio_start(codec, session_id).await,
            AtvvNotification::AudioStop => self.on_audio_stop().await,
            AtvvNotification::AudioSync {
                predictor,
                step_index,
            } => {
                self.decoder.synchronize(predictor, step_index);
            }
            AtvvNotification::Capabilities(capabilities) => {
                self.decoder
                    .set_frame_size(usize::from(capabilities.frame_size));
                self.logger.info(format!(
                    "ATVV 能力：版本 0x{:04X}，帧大小 {}",
                    capabilities.version, capabilities.frame_size
                ));
            }
            AtvvNotification::MicRefused => {
                self.logger.warn("遥控器拒绝了麦克风打开请求");
                self.stop_voice(None).await;
            }
        }
    }

    async fn on_voice_trigger(&mut self, source: &str) {
        if self
            .last_voice_trigger
            .is_some_and(|triggered| triggered.elapsed() < VOICE_TRIGGER_DEDUP)
        {
            return;
        }
        self.last_voice_trigger = Some(Instant::now());
        self.logger.info(format!("检测到语音键触发（{source}）"));
        self.on_start_search().await;
    }

    async fn on_start_search(&mut self) {
        if self.active && (self.hands_free || self.awaiting_reopen) {
            self.logger.info("再次按下语音键，结束免提收音");
            self.stop_voice(None).await;
            return;
        }
        if self.active {
            return;
        }

        self.emit_voice_key("down");
        if let Err(error) = self.open_audio().await {
            let message = audio_error_message(&error);
            self.logger.error(message.clone());
            self.emit_voice(false, false, "error", Some(message));
            self.emit_voice_key("up");
            return;
        }

        if let Err(error) = self.begin_input_shortcut().await {
            self.logger
                .error(format!("发送输入法语音快捷键失败：{error}"));
            self.stop_voice(Some(error)).await;
            return;
        }

        if let Err(error) = self.transport.write_control(&AtvvControl::MicOpen).await {
            self.logger.error(format!("发送 MicOpen 失败：{error}"));
            self.stop_voice(Some(error.to_string())).await;
            return;
        }

        self.active = true;
        self.hands_free = false;
        self.awaiting_reopen = false;
        self.pressed_at = Some(Instant::now());
        self.started_at = Some(Instant::now());
        self.last_keepalive = Instant::now();
        self.decoder.reset();
        self.logger.info("语音键按下，已打开麦克风");
        self.emit_voice(true, false, "opening", None);
    }

    async fn on_audio_start(&mut self, codec: u8, session_id: u8) {
        if codec != 0x02 {
            self.logger
                .warn(format!("不支持的 ATVV 音频编码 0x{codec:02X}"));
            self.stop_voice(Some("遥控器返回了非 16 kHz ADPCM 编码".to_string()))
                .await;
            return;
        }

        if !self.active {
            self.emit_voice_key("down");
            if let Err(error) = self.open_audio().await {
                self.logger.error(audio_error_message(&error));
                self.emit_voice_key("up");
                self.stop_voice(Some(error.to_string())).await;
                return;
            }
            self.active = true;
            self.started_at = Some(Instant::now());
            self.decoder.reset();
            if let Err(error) = self.begin_input_shortcut().await {
                self.logger.error(format!("启动输入法语音失败：{error}"));
                self.stop_voice(Some(error)).await;
                return;
            }
        }

        if self.awaiting_reopen {
            self.awaiting_reopen = false;
            self.hands_free = true;
            self.logger.info("语音免提已重新打开");
        }
        self.session_id = session_id;
        self.last_keepalive = Instant::now();
        self.emit_voice(true, self.hands_free, "streaming", None);
    }

    async fn begin_input_shortcut(&mut self) -> Result<(), String> {
        if self.voice_shortcut_sent {
            return Ok(());
        }
        let input_tool = self.config.read().await.input_tool.clone();
        let hold_to_talk = input_tool.trigger_mode.eq_ignore_ascii_case("HoldToTalk");
        if hold_to_talk {
            send_shortcut_down(&input_tool.shortcut).map_err(|error| error.to_string())?;
            self.shortcut_held = Some(input_tool.shortcut.clone());
        } else {
            send_shortcut_text(&input_tool.shortcut).map_err(|error| error.to_string())?;
        }
        self.voice_shortcut_sent = true;
        self.logger.info(format!(
            "已发送输入法语音快捷键：{}（{}）",
            input_tool.shortcut,
            if hold_to_talk {
                "按住说话"
            } else {
                "点击切换"
            }
        ));
        Ok(())
    }

    async fn on_audio_stop(&mut self) {
        if !self.active {
            return;
        }
        let short_press = self
            .pressed_at
            .is_some_and(|pressed| pressed.elapsed() < HOLD_THRESHOLD);
        if self.device.voice_mode == remote_buddy_core::config::VoiceMode::SmartToggle
            && short_press
            && !self.hands_free
            && !self.awaiting_reopen
        {
            self.awaiting_reopen = true;
            self.pressed_at = None;
            self.logger.info("短按松开，重新打开麦克风进入免提");
            match self.transport.write_control(&AtvvControl::MicOpen).await {
                Ok(()) => self.emit_voice(true, false, "reopening", None),
                Err(error) => {
                    self.logger.error(format!("免提重开失败：{error}"));
                    self.stop_voice(Some(error.to_string())).await;
                }
            }
            return;
        }

        self.stop_voice(None).await;
    }

    async fn tick(&mut self) {
        if !self.active {
            return;
        }
        if self
            .started_at
            .is_some_and(|started| started.elapsed() >= MAX_HANDS_FREE)
        {
            self.logger.warn("语音会话达到 5 分钟上限，自动结束");
            self.stop_voice(None).await;
            return;
        }
        if self.last_keepalive.elapsed() >= KEEPALIVE_INTERVAL {
            self.last_keepalive = Instant::now();
            let _ = self
                .transport
                .write_control(&AtvvControl::MicExtend {
                    session_id: self.session_id,
                })
                .await;
        }
    }

    async fn stop_voice(&mut self, error: Option<String>) {
        if self.active {
            let _ = self
                .transport
                .write_control(&AtvvControl::MicClose {
                    session_id: self.session_id,
                })
                .await;
        }
        self.release_shortcut();
        self.emit_voice_key("up");
        self.audio.take();
        self.decoder.reset();
        self.active = false;
        self.hands_free = false;
        self.awaiting_reopen = false;
        self.pressed_at = None;
        self.started_at = None;
        self.session_id = 0;
        self.emit_voice(false, false, "idle", error);
    }

    fn shutdown_voice(&mut self, reason: &str) {
        let _ = self.transport.write_control_now(&AtvvControl::MicClose {
            session_id: self.session_id,
        });
        self.release_shortcut();
        self.emit_voice_key("up");
        self.audio.take();
        self.decoder.reset();
        self.active = false;
        self.hands_free = false;
        self.awaiting_reopen = false;
        self.emit_voice(false, false, "idle", Some(reason.to_string()));
        self.logger.warn(reason);
    }

    fn release_shortcut(&mut self) {
        if let Some(shortcut) = self.shortcut_held.take() {
            let _ = send_shortcut_up(&shortcut);
        }
        self.voice_shortcut_sent = false;
    }

    fn emit_voice_key(&mut self, phase: &str) {
        if phase == "down" {
            if self.voice_input_down {
                return;
            }
            self.voice_input_down = true;
        } else {
            if !self.voice_input_down {
                return;
            }
            self.voice_input_down = false;
        }
        self.input_sequence = self.input_sequence.saturating_add(1);
        let _ = self.app.emit(
            "input-event",
            InputEvent {
                id: format!("voice-{}", self.input_sequence),
                time: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                button: "Voice".to_string(),
                phase: phase.to_string(),
                action: "语音输入".to_string(),
                source: "ATVV".to_string(),
            },
        );
        if phase == "down" {
            crate::tray::record_action(&self.app, "Voice", "按下", "语音输入");
        }
    }

    async fn open_audio(&mut self) -> Result<(), AudioError> {
        self.audio.take();
        self.level_accumulator.reset();
        self.last_level_emit = Instant::now();
        let output_device = self
            .config
            .read()
            .await
            .input_tool
            .preferred_audio_output_device
            .clone();
        let audio = open_output_with_gain(&output_device, self.voice_gain.clone())?;
        self.audio = Some(audio);
        Ok(())
    }

    fn emit_voice(&mut self, active: bool, hands_free: bool, phase: &str, error: Option<String>) {
        self.voice_active.store(active, Ordering::Relaxed);
        if !active {
            self.emit_silence_level();
        }
        let _ = self.app.emit(
            "voice-state-changed",
            VoiceStateEvent {
                active,
                hands_free,
                phase: phase.to_string(),
                error,
            },
        );
    }

    fn emit_accumulated_level(&mut self) {
        let level = self.level_accumulator.level();
        self.level_accumulator.reset();
        self.last_level_emit = Instant::now();
        self.emit_level(level.rms, level.peak);
    }

    fn emit_silence_level(&mut self) {
        self.level_accumulator.reset();
        self.last_level_emit = Instant::now();
        self.emit_level(0.0, 0.0);
    }

    fn emit_level(&mut self, rms: f32, peak: f32) {
        self.level_sequence = self.level_sequence.wrapping_add(1);
        let _ = self.app.emit(
            "voice-level-changed",
            VoiceLevelEvent {
                rms,
                peak,
                sequence: self.level_sequence,
            },
        );
    }
}

impl Drop for BleSession {
    fn drop(&mut self) {
        self.shutdown_voice("设备会话已结束");
    }
}

fn audio_error_message(error: &AudioError) -> String {
    match error {
        AudioError::DeviceNotFound { name } => {
            format!("未找到音频输出设备“{name}”。请安装 VB-CABLE，或改用默认播放设备。")
        }
        _ => format!("音频输出启动失败：{error}"),
    }
}
