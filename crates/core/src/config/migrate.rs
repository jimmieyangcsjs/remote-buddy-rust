use super::defaults::{fill_missing_defaults, migrate_long_press_defaults};
use super::model::{
    ActionConfig, AppConfig, CURRENT_SCHEMA_VERSION, DeviceConfig, InputToolConfig, LongPressMode,
    MappingConfig, MediaControl, VoiceMode,
};
use atomic_write_file::AtomicWriteFile;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("configuration JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("configuration version {0} is not supported")]
    UnsupportedVersion(u64),
    #[error("configuration IO failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("configuration root must be a JSON object")]
    RootNotObject,
}

pub fn parse_and_migrate(raw: &str) -> Result<AppConfig, ConfigError> {
    let value: Value = serde_json::from_str(raw)?;
    let object = value.as_object().ok_or(ConfigError::RootNotObject)?;
    let version = object
        .get("schemaVersion")
        .or_else(|| object.get("schema_version"))
        .and_then(Value::as_u64);

    match version {
        Some(version) if version == u64::from(CURRENT_SCHEMA_VERSION) => {
            let mut config: AppConfig = serde_json::from_value(value)?;
            config.schema_version = CURRENT_SCHEMA_VERSION;
            Ok(config)
        }
        Some(2) => {
            let mut config: AppConfig = serde_json::from_value(value)?;
            migrate_v2_to_v3(&mut config);
            Ok(config)
        }
        Some(3) => {
            let mut config: AppConfig = serde_json::from_value(value)?;
            config.schema_version = CURRENT_SCHEMA_VERSION;
            Ok(config)
        }
        Some(version) => Err(ConfigError::UnsupportedVersion(version)),
        None => migrate_v1(object),
    }
}

fn migrate_v2_to_v3(config: &mut AppConfig) {
    for device in &mut config.devices {
        migrate_long_press_defaults(&mut device.mappings);
    }
    config.schema_version = CURRENT_SCHEMA_VERSION;
}

pub fn save_atomic(path: &Path, config: &AppConfig) -> Result<(), ConfigError> {
    let bytes = serde_json::to_vec_pretty(config)?;
    let mut file = AtomicWriteFile::open(path)?;
    file.write_all(&bytes)?;
    file.commit()?;
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct V1Config {
    #[serde(default)]
    language: String,
    #[serde(default)]
    launch_at_startup: bool,
    #[serde(default = "default_true")]
    minimize_to_tray_on_close: bool,
    #[serde(default)]
    has_completed_onboarding: bool,
    #[serde(default)]
    input_tool: V1InputTool,
    #[serde(default)]
    devices: Vec<V1Device>,
    #[serde(default)]
    selected_device_id: Option<String>,
    #[serde(default)]
    app_integrations: BTreeMap<String, Value>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct V1InputTool {
    #[serde(default = "default_tool")]
    tool: String,
    #[serde(default = "default_trigger")]
    trigger_mode: String,
    #[serde(default = "default_shortcut")]
    shortcut: String,
    #[serde(default = "default_audio_device")]
    preferred_audio_input_device: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct V1Device {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    model_type: String,
    #[serde(default)]
    container_id: Option<String>,
    #[serde(default)]
    bluetooth_address: u64,
    #[serde(default)]
    formatted_address: String,
    #[serde(default)]
    gain: f32,
    #[serde(default)]
    voice_button_mode: String,
    #[serde(default)]
    is_smart_toggle_mode: bool,
    #[serde(default)]
    is_hold_to_talk_mode: bool,
    #[serde(default)]
    mappings: Vec<V1Mapping>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct V1Mapping {
    #[serde(default)]
    button: String,
    #[serde(default)]
    click_action: String,
    #[serde(default)]
    click_shortcut: Option<String>,
    #[serde(default)]
    click_text: Option<String>,
    #[serde(default)]
    click_media: String,
    #[serde(default)]
    double_click_action: String,
    #[serde(default)]
    double_click_shortcut: Option<String>,
    #[serde(default)]
    double_click_text: Option<String>,
    #[serde(default)]
    double_click_media: String,
    #[serde(default)]
    long_press_action: String,
    #[serde(default)]
    long_press_shortcut: Option<String>,
    #[serde(default)]
    long_press_text: Option<String>,
    #[serde(default)]
    long_press_media: String,
}

fn migrate_v1(object: &Map<String, Value>) -> Result<AppConfig, ConfigError> {
    let v1: V1Config = serde_json::from_value(Value::Object(object.clone()))?;
    let known = [
        "Version",
        "Language",
        "LaunchAtStartup",
        "MinimizeToTrayOnClose",
        "HasCompletedOnboarding",
        "InputTool",
        "Devices",
        "SelectedDeviceId",
        "AppIntegrations",
    ];
    let mut extensions = Map::new();
    for (key, value) in object {
        if !known.contains(&key.as_str()) {
            extensions.insert(key.clone(), value.clone());
        }
    }

    let integrations = v1
        .app_integrations
        .into_iter()
        .filter_map(|(key, value)| value.as_bool().map(|value| (key, value)))
        .collect();

    let devices = v1
        .devices
        .into_iter()
        .map(|device| {
            let voice_mode = voice_mode(&device);
            let mut mappings: Vec<MappingConfig> = device
                .mappings
                .into_iter()
                .map(|mapping| MappingConfig {
                    button: mapping.button,
                    click: action(
                        &mapping.click_action,
                        mapping.click_shortcut,
                        mapping.click_text,
                        &mapping.click_media,
                    ),
                    double_click: action(
                        &mapping.double_click_action,
                        mapping.double_click_shortcut,
                        mapping.double_click_text,
                        &mapping.double_click_media,
                    ),
                    long_press: action(
                        &mapping.long_press_action,
                        mapping.long_press_shortcut,
                        mapping.long_press_text,
                        &mapping.long_press_media,
                    ),
                    long_press_mode: LongPressMode::OneShot,
                    extensions: Map::new(),
                })
                .collect();
            fill_missing_defaults(&mut mappings);
            migrate_long_press_defaults(&mut mappings);
            DeviceConfig {
                id: device.id,
                name: device.name,
                model_type: device.model_type,
                container_id: device.container_id,
                bluetooth_address: device.bluetooth_address,
                formatted_address: device.formatted_address,
                voice_mode,
                gain: if device.gain > 0.0 { device.gain } else { 1.0 },
                mappings,
                extensions: Map::new(),
            }
        })
        .collect();

    Ok(AppConfig {
        schema_version: CURRENT_SCHEMA_VERSION,
        language: if v1.language.is_empty() {
            "system".to_string()
        } else {
            v1.language
        },
        launch_at_startup: v1.launch_at_startup,
        minimize_to_tray: v1.minimize_to_tray_on_close,
        run_as_admin: true,
        has_completed_onboarding: v1.has_completed_onboarding,
        selected_device_id: v1.selected_device_id,
        devices,
        input_tool: InputToolConfig {
            tool: v1.input_tool.tool,
            trigger_mode: v1.input_tool.trigger_mode,
            shortcut: v1.input_tool.shortcut,
            preferred_audio_input_device: v1.input_tool.preferred_audio_input_device,
            preferred_audio_output_device: "CABLE Input".to_string(),
        },
        integrations,
        extensions,
    })
}

fn voice_mode(device: &V1Device) -> VoiceMode {
    if device.is_smart_toggle_mode || device.voice_button_mode.eq_ignore_ascii_case("SmartToggle") {
        VoiceMode::SmartToggle
    } else if device.is_hold_to_talk_mode
        || device.voice_button_mode.eq_ignore_ascii_case("HoldToTalk")
    {
        VoiceMode::HoldToTalk
    } else {
        VoiceMode::Unknown
    }
}

fn action(
    action: &str,
    shortcut: Option<String>,
    text: Option<String>,
    media: &str,
) -> ActionConfig {
    match action {
        "KeyboardShortcut" => shortcut
            .filter(|value| !value.is_empty())
            .map(|keys| ActionConfig::Shortcut { keys })
            .unwrap_or(ActionConfig::None),
        "Text" => text
            .filter(|value| !value.is_empty())
            .map(|text| ActionConfig::Text { text })
            .unwrap_or(ActionConfig::None),
        "MediaControl" => match media {
            "VolumeUp" => ActionConfig::Media {
                control: MediaControl::VolumeUp,
            },
            "VolumeDown" => ActionConfig::Media {
                control: MediaControl::VolumeDown,
            },
            "Mute" => ActionConfig::Media {
                control: MediaControl::Mute,
            },
            "PlayPause" => ActionConfig::Media {
                control: MediaControl::PlayPause,
            },
            _ => ActionConfig::None,
        },
        "ContinuousRepeat" => ActionConfig::ContinuousRepeat { interval_ms: 80 },
        _ => ActionConfig::None,
    }
}

fn default_true() -> bool {
    true
}

fn default_tool() -> String {
    "WeChatInput".to_string()
}

fn default_trigger() -> String {
    "HoldToTalk".to_string()
}

fn default_shortcut() -> String {
    "Ctrl+F5".to_string()
}

fn default_audio_device() -> String {
    "CABLE Output".to_string()
}
