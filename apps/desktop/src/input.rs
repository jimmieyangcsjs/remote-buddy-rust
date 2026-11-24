use crate::logger::AppLogger;
use remote_buddy_core::config::{
    ActionConfig, AppConfig, LongPressMode, MappingConfig, MediaControl,
};
use remote_buddy_core::input::{
    ButtonPhase, Gesture, GestureRecognizer, RemoteButton, ReportDecoder, parse_report,
};
use remote_buddy_windows::input::{
    InputSource, RawFrame, execute_action, execute_action_down, execute_action_up,
};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::{RwLock, mpsc::UnboundedReceiver};

const DEDUP_WINDOW: Duration = Duration::from_millis(45);
const TICK: Duration = Duration::from_millis(20);
const REPEAT_AFTER: Duration = Duration::from_millis(700);
const DEFAULT_REPEAT_INTERVAL: Duration = Duration::from_millis(80);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyEvent {
    pub id: String,
    pub time: String,
    pub button: String,
    pub action: String,
    pub source: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InputEvent {
    pub id: String,
    pub time: String,
    pub button: String,
    pub phase: String,
    pub action: String,
    pub source: String,
}

#[derive(Default)]
struct DeviceInput {
    reports: ReportDecoder,
    gestures: GestureRecognizer,
    last_source: Option<InputSource>,
}

struct HeldButton {
    pressed_at: Instant,
    next_repeat: Instant,
    repeat_interval: Duration,
    repeat_enabled: bool,
    source: InputSource,
    hold_action: Option<ActionConfig>,
    hold_triggered: bool,
}

pub fn spawn_input_router(
    app: AppHandle,
    config: Arc<RwLock<AppConfig>>,
    logger: AppLogger,
    mut frames: UnboundedReceiver<RawFrame>,
) {
    tokio::spawn(async move {
        let mut devices: HashMap<String, DeviceInput> = HashMap::new();
        let mut held: HashMap<(String, RemoteButton), HeldButton> = HashMap::new();
        let mut last_emitted: HashMap<(String, RemoteButton, ButtonPhase), Instant> =
            HashMap::new();
        let input_trace = std::env::var_os("REMOTE_BUDDY_INPUT_TRACE").is_some();
        let mut interval = tokio::time::interval(TICK);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut sequence = 0_u64;
        let mut logged_inputs = 0_u32;
        let mut logged_raw_frames = 0_u32;
        let mut logged_trace_frames = 0_u32;
        let mut unknown_path_reported = false;

        loop {
            tokio::select! {
                Some(frame) = frames.recv() => {
                    let snapshot = config.read().await;
                    let Some((device_id, model_type)) = resolve_device(&snapshot, &frame.device_path) else {
                        if !unknown_path_reported {
                            logger.warn(format!(
                                "收到未配置遥控器的输入，已忽略：{}",
                                frame.device_path
                            ));
                            unknown_path_reported = true;
                        }
                        continue;
                    };
                    drop(snapshot);

                    let source = frame.source;
                    let phases = decode_frame(&mut devices, &device_id, &model_type, &frame);
                    if input_trace && logged_trace_frames < 5_000 {
                        logged_trace_frames += 1;
                        logger.info(format!(
                            "输入追踪 {} · {} · vkey={:?} down={:?} · 原始 {} · 解码 {:?}",
                            source.as_str(),
                            frame.device_path,
                            frame.vkey,
                            frame.is_down,
                            hex_bytes(&frame.bytes),
                            phases
                        ));
                    }
                    if logged_raw_frames < 20 {
                        logged_raw_frames += 1;
                        logger.info(format!(
                            "{} 首帧：{} · vkey={:?} down={:?} · {} 字节 · 解码 {} 个事件",
                            source.as_str(),
                            frame.device_path,
                            frame.vkey,
                            frame.is_down,
                            frame.bytes.len(),
                            phases.len()
                        ));
                    }
                    for (button, phase) in phases {
                        let key = (device_id.clone(), button, phase);
                        let now = Instant::now();
                        if last_emitted
                            .get(&key)
                            .is_some_and(|last| now.duration_since(*last) < DEDUP_WINDOW)
                        {
                            continue;
                        }
                        last_emitted.insert(key, now);
                        let phase_label = match phase {
                            ButtonPhase::Down => "down",
                            ButtonPhase::Up => "up",
                            ButtonPhase::Repeat => "repeat",
                        };
                        let action = mapping_for(&config, &device_id, button)
                            .await
                            .map(|mapping| describe_action(&mapping.click))
                            .unwrap_or_else(|| "无动作".to_string());
                        sequence = sequence.saturating_add(1);
                        let input_event = InputEvent {
                            id: format!("input-{sequence}"),
                            time: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
                            button: button.as_str().to_string(),
                            phase: phase_label.to_string(),
                            action: action.clone(),
                            source: source.as_str().to_string(),
                        };
                        if logged_inputs < 30 {
                            logged_inputs += 1;
                            logger.info(format!(
                                "遥控器输入：{} {} · {} · {}",
                                button.as_str(),
                                phase_label,
                                action,
                                source.as_str()
                            ));
                        }
                        let _ = app.emit("input-event", input_event);
                        handle_phase(
                            &app,
                            &config,
                            &logger,
                            &mut devices,
                            &mut held,
                            &mut sequence,
                            &device_id,
                            button,
                            phase,
                            now,
                            source,
                        )
                        .await;
                    }
                }
                _ = interval.tick() => {
                    let now = Instant::now();
                    let due_repeats = held
                        .iter()
                        .filter(|(_, state)| {
                            state.repeat_enabled
                                && now.duration_since(state.pressed_at) >= REPEAT_AFTER
                                && now >= state.next_repeat
                        })
                        .map(|(key, state)| (key.clone(), state.source))
                        .collect::<Vec<_>>();
                    for (key, _) in &due_repeats {
                        if let Some(state) = held.get_mut(key) {
                            state.next_repeat = now + state.repeat_interval;
                        }
                    }
                    for ((device_id, button), source) in due_repeats {
                        execute_gesture(
                            &app,
                            &config,
                            &logger,
                            &mut sequence,
                            &device_id,
                            button,
                            Gesture::Repeat,
                            source,
                        )
                        .await;
                    }

                    let due_holds = held
                        .iter()
                        .filter(|(_, state)| {
                            state.hold_action.is_some()
                                && !state.hold_triggered
                                && now.duration_since(state.pressed_at) >= REPEAT_AFTER
                        })
                        .map(|(key, state)| {
                            (
                                key.clone(),
                                state.source,
                                state.hold_action.clone().expect("hold action checked"),
                            )
                        })
                        .collect::<Vec<_>>();
                    for (key, source, action) in due_holds {
                        if let Some(state) = held.get_mut(&key) {
                            state.hold_triggered = true;
                        }
                        let (device_id, button) = key;
                        execute_hold_start(
                            &app,
                            &logger,
                            &mut sequence,
                            &device_id,
                            button,
                            &action,
                            source,
                        );
                    }

                    let now_ms = monotonic_ms();
                    let mut clicks = Vec::new();
                    for (device_id, state) in &mut devices {
                        let source = state.last_source.unwrap_or(InputSource::RawInput);
                        for (button, gesture) in state.gestures.poll_events(now_ms) {
                            clicks.push((device_id.clone(), button, gesture, source));
                        }
                    }
                    for (device_id, button, gesture, source) in clicks {
                        execute_gesture(
                            &app,
                            &config,
                            &logger,
                            &mut sequence,
                            &device_id,
                            button,
                            gesture,
                            source,
                        )
                        .await;
                    }

                    last_emitted.retain(|_, at| now.duration_since(*at) < Duration::from_secs(10));
                }
                else => break,
            }
        }
    });
}

#[allow(clippy::too_many_arguments)]
async fn handle_phase(
    app: &AppHandle,
    config: &Arc<RwLock<AppConfig>>,
    logger: &AppLogger,
    devices: &mut HashMap<String, DeviceInput>,
    held: &mut HashMap<(String, RemoteButton), HeldButton>,
    sequence: &mut u64,
    device_id: &str,
    button: RemoteButton,
    phase: ButtonPhase,
    now: Instant,
    source: InputSource,
) {
    let mapping = mapping_for(config, device_id, button).await;
    let double_click_enabled = double_click_enabled(mapping.as_ref());
    let hold_action = hold_action(mapping.as_ref());
    let state = devices.entry(device_id.to_string()).or_default();
    state.last_source = Some(source);
    let gestures = state
        .gestures
        .on_event(button, phase, monotonic_ms(), double_click_enabled);
    let key = (device_id.to_string(), button);
    let hold_release = if phase == ButtonPhase::Up {
        held.remove(&key)
    } else {
        None
    };
    match phase {
        ButtonPhase::Down => {
            let repeat_interval = repeat_interval(mapping.as_ref());
            let repeat_enabled = mapping.as_ref().is_some_and(|mapping| {
                matches!(mapping.long_press, ActionConfig::ContinuousRepeat { .. })
            });
            held.insert(
                key,
                HeldButton {
                    pressed_at: now,
                    next_repeat: now + REPEAT_AFTER,
                    repeat_interval,
                    repeat_enabled,
                    source,
                    hold_action,
                    hold_triggered: false,
                },
            );
        }
        ButtonPhase::Up => {}
        ButtonPhase::Repeat => {}
    }

    for gesture in gestures {
        if gesture == Gesture::LongPress
            && hold_release
                .as_ref()
                .is_some_and(|state| state.hold_action.is_some())
        {
            continue;
        }
        if gesture == Gesture::LongPress
            && mapping.as_ref().is_some_and(|mapping| {
                matches!(mapping.long_press, ActionConfig::ContinuousRepeat { .. })
            })
        {
            continue;
        }
        execute_gesture(
            app, config, logger, sequence, device_id, button, gesture, source,
        )
        .await;
    }

    if let Some(state) = hold_release
        && let Some(action) = state.hold_action
    {
        if state.hold_triggered {
            execute_hold_end(app, logger, sequence, device_id, button, &action, source);
        } else if now.duration_since(state.pressed_at) >= REPEAT_AFTER {
            execute_hold_start(app, logger, sequence, device_id, button, &action, source);
            execute_hold_end(app, logger, sequence, device_id, button, &action, source);
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn execute_gesture(
    app: &AppHandle,
    config: &Arc<RwLock<AppConfig>>,
    logger: &AppLogger,
    sequence: &mut u64,
    device_id: &str,
    button: RemoteButton,
    gesture: Gesture,
    source: InputSource,
) {
    let Some(mapping) = mapping_for(config, device_id, button).await else {
        return;
    };
    let action = match gesture {
        Gesture::Click => &mapping.click,
        Gesture::DoubleClick => &mapping.double_click,
        Gesture::LongPress => &mapping.long_press,
        Gesture::Repeat => {
            if matches!(mapping.long_press, ActionConfig::ContinuousRepeat { .. }) {
                &mapping.click
            } else {
                &mapping.long_press
            }
        }
    };
    if matches!(
        action,
        ActionConfig::None | ActionConfig::ContinuousRepeat { .. }
    ) {
        return;
    }

    let action_label = describe_action(action);
    if let Err(error) = execute_action(action) {
        logger.error(format!(
            "{} 的 {} 动作失败：{error}",
            button.as_str(),
            action_label
        ));
        return;
    }

    let gesture_name = gesture_label(gesture);
    emit_action_event(
        app,
        sequence,
        button,
        gesture_name,
        &action_label,
        source,
        gesture != Gesture::Repeat,
    );
}

#[allow(clippy::too_many_arguments)]
fn execute_hold_start(
    app: &AppHandle,
    logger: &AppLogger,
    sequence: &mut u64,
    device_id: &str,
    button: RemoteButton,
    action: &ActionConfig,
    source: InputSource,
) {
    let action_label = describe_action(action);
    if let Err(error) = execute_action_down(action) {
        logger.error(format!(
            "{device_id} 的 {} 长按按住动作失败：{error}",
            button.as_str()
        ));
        return;
    }

    emit_action_event(app, sequence, button, "长按", &action_label, source, true);
}

#[allow(clippy::too_many_arguments)]
fn execute_hold_end(
    app: &AppHandle,
    logger: &AppLogger,
    sequence: &mut u64,
    device_id: &str,
    button: RemoteButton,
    action: &ActionConfig,
    source: InputSource,
) {
    let action_label = describe_action(action);
    if let Err(error) = execute_action_up(action) {
        logger.error(format!(
            "{device_id} 的 {} 长按释放动作失败：{error}",
            button.as_str()
        ));
        return;
    }

    emit_action_event(
        app,
        sequence,
        button,
        "长按释放",
        &action_label,
        source,
        false,
    );
}

fn emit_action_event(
    app: &AppHandle,
    sequence: &mut u64,
    button: RemoteButton,
    gesture: &str,
    action_label: &str,
    source: InputSource,
    record_tray: bool,
) {
    *sequence = sequence.saturating_add(1);
    let event = KeyEvent {
        id: format!("{}-{sequence}", monotonic_ms()),
        time: chrono::Local::now().format("%H:%M:%S%.3f").to_string(),
        button: format!("{} · {gesture}", button.as_str()),
        action: action_label.to_string(),
        source: source.as_str().to_string(),
    };
    if record_tray {
        crate::tray::record_action(app, button.as_str(), gesture, action_label);
    }
    let _ = app.emit("button-event", event);
}

async fn mapping_for(
    config: &Arc<RwLock<AppConfig>>,
    device_id: &str,
    button: RemoteButton,
) -> Option<MappingConfig> {
    let config = config.read().await;
    config
        .device(device_id)?
        .mappings
        .iter()
        .find(|mapping| mapping.button.eq_ignore_ascii_case(button.as_str()))
        .cloned()
}

fn decode_frame(
    devices: &mut HashMap<String, DeviceInput>,
    device_id: &str,
    model_type: &str,
    frame: &RawFrame,
) -> Vec<(RemoteButton, ButtonPhase)> {
    if !frame.bytes.is_empty() {
        let Some(report) = parse_report(&frame.bytes, model_type) else {
            return Vec::new();
        };
        return devices
            .entry(device_id.to_string())
            .or_default()
            .reports
            .update(&report);
    }

    let Some(vkey) = frame.vkey else {
        return Vec::new();
    };
    let Some(button) = remote_button_for_vkey(vkey) else {
        return Vec::new();
    };
    let phase = if frame.is_down.unwrap_or(true) {
        ButtonPhase::Down
    } else {
        ButtonPhase::Up
    };
    vec![(button, phase)]
}

fn resolve_device(config: &AppConfig, path: &str) -> Option<(String, String)> {
    let lower = path.to_ascii_lowercase();
    for device in &config.devices {
        let id = device.id.to_ascii_lowercase();
        let address = device
            .formatted_address
            .replace(':', "")
            .to_ascii_lowercase();
        if (!id.is_empty() && lower.contains(&id))
            || (!address.is_empty() && lower.contains(&address))
        {
            return Some((device.id.clone(), device.model_type.clone()));
        }
    }

    let id = bluetooth_id_from_path(path)?;
    config
        .devices
        .iter()
        .find(|device| device.id.eq_ignore_ascii_case(&id))
        .map(|device| (device.id.clone(), device.model_type.clone()))
}

fn bluetooth_id_from_path(path: &str) -> Option<String> {
    let bytes = path.as_bytes();
    for start in 1..bytes.len().saturating_sub(12) {
        if bytes[start - 1] != b'_' {
            continue;
        }
        let candidate = &bytes[start..start + 12];
        let end = start + 12;
        if candidate.iter().all(u8::is_ascii_hexdigit)
            && (end == bytes.len() || !bytes[end].is_ascii_hexdigit())
        {
            return Some(String::from_utf8_lossy(candidate).to_ascii_uppercase());
        }
    }
    None
}

fn remote_button_for_vkey(vkey: u16) -> Option<RemoteButton> {
    Some(match vkey {
        0x26 => RemoteButton::Up,
        0x28 => RemoteButton::Down,
        0x25 => RemoteButton::Left,
        0x27 => RemoteButton::Right,
        0x0D => RemoteButton::Ok,
        0x1B => RemoteButton::Back,
        0x24 => RemoteButton::Home,
        0x5D => RemoteButton::Menu,
        0xAF => RemoteButton::VolumeUp,
        0xAE => RemoteButton::VolumeDown,
        0xAD => RemoteButton::Mute,
        0xB3 => RemoteButton::PlayPause,
        0xC0 => RemoteButton::LiveTv,
        0x08 => RemoteButton::Power,
        _ => return None,
    })
}

fn hex_bytes(bytes: &[u8]) -> String {
    if bytes.is_empty() {
        return "-".to_string();
    }
    bytes
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn describe_action(action: &ActionConfig) -> String {
    match action {
        ActionConfig::None => "无动作".to_string(),
        ActionConfig::Shortcut { keys } => keys.clone(),
        ActionConfig::Text { text } => format!("文本：{text}"),
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
            format!("切换应用：{label}")
        }
        ActionConfig::ContinuousRepeat { interval_ms } => {
            format!("连续重复 {interval_ms} ms")
        }
    }
}

fn double_click_enabled(mapping: Option<&MappingConfig>) -> bool {
    mapping.is_some_and(|mapping| !matches!(mapping.double_click, ActionConfig::None))
}

fn hold_action(mapping: Option<&MappingConfig>) -> Option<ActionConfig> {
    let mapping = mapping?;
    if mapping.long_press_mode != LongPressMode::Hold {
        return None;
    }
    match &mapping.long_press {
        ActionConfig::Shortcut { .. } | ActionConfig::Media { .. } => {
            Some(mapping.long_press.clone())
        }
        _ => None,
    }
}

fn repeat_interval(mapping: Option<&MappingConfig>) -> Duration {
    mapping
        .and_then(|mapping| match mapping.long_press {
            ActionConfig::ContinuousRepeat { interval_ms } => {
                Some(Duration::from_millis(interval_ms.clamp(20, 1_000)))
            }
            _ => None,
        })
        .unwrap_or(DEFAULT_REPEAT_INTERVAL)
}

fn gesture_label(gesture: Gesture) -> &'static str {
    match gesture {
        Gesture::Click => "单击",
        Gesture::DoubleClick => "双击",
        Gesture::LongPress => "长按",
        Gesture::Repeat => "连发",
    }
}

fn monotonic_ms() -> u64 {
    use std::sync::OnceLock;
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(double_click: ActionConfig, long_press: ActionConfig) -> MappingConfig {
        MappingConfig {
            button: "Up".to_string(),
            click: ActionConfig::Shortcut {
                keys: "Up".to_string(),
            },
            double_click,
            long_press,
            long_press_mode: LongPressMode::OneShot,
            extensions: Default::default(),
        }
    }

    #[test]
    fn double_click_wait_is_only_used_when_a_double_click_action_exists() {
        assert!(!double_click_enabled(Some(&mapping(
            ActionConfig::None,
            ActionConfig::None
        ))));
        assert!(double_click_enabled(Some(&mapping(
            ActionConfig::Text {
                text: "copy".to_string()
            },
            ActionConfig::None
        ))));
    }

    #[test]
    fn repeat_interval_comes_from_the_mapping_and_is_bounded() {
        assert_eq!(
            repeat_interval(Some(&mapping(
                ActionConfig::None,
                ActionConfig::ContinuousRepeat { interval_ms: 120 }
            ))),
            Duration::from_millis(120)
        );
        assert_eq!(
            repeat_interval(Some(&mapping(
                ActionConfig::None,
                ActionConfig::ContinuousRepeat { interval_ms: 5 }
            ))),
            Duration::from_millis(20)
        );
        assert_eq!(
            repeat_interval(Some(&mapping(ActionConfig::None, ActionConfig::None))),
            DEFAULT_REPEAT_INTERVAL
        );
    }

    #[test]
    fn hold_mode_only_applies_to_keys_that_can_stay_pressed() {
        let mut value = mapping(
            ActionConfig::None,
            ActionConfig::Shortcut {
                keys: "Ctrl+Shift+Space".to_string(),
            },
        );
        assert_eq!(hold_action(Some(&value)), None);

        value.long_press_mode = LongPressMode::Hold;
        assert_eq!(hold_action(Some(&value)), Some(value.long_press.clone()));

        value.long_press = ActionConfig::Text {
            text: "held text".to_string(),
        };
        assert_eq!(hold_action(Some(&value)), None);

        value.long_press = ActionConfig::ContinuousRepeat { interval_ms: 80 };
        assert_eq!(hold_action(Some(&value)), None);
    }
}
