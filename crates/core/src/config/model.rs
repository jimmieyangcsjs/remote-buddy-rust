use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

pub const CURRENT_SCHEMA_VERSION: u32 = 4;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub schema_version: u32,
    pub language: String,
    pub launch_at_startup: bool,
    #[serde(default)]
    pub silent_start: bool,
    pub minimize_to_tray: bool,
    #[serde(default = "default_run_as_admin")]
    pub run_as_admin: bool,
    pub has_completed_onboarding: bool,
    pub selected_device_id: Option<String>,
    pub devices: Vec<DeviceConfig>,
    pub input_tool: InputToolConfig,
    pub integrations: BTreeMap<String, bool>,
    #[serde(flatten, default)]
    pub extensions: Map<String, Value>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            language: "zh-CN".to_string(),
            launch_at_startup: false,
            silent_start: false,
            minimize_to_tray: true,
            run_as_admin: true,
            has_completed_onboarding: false,
            selected_device_id: None,
            devices: Vec::new(),
            input_tool: InputToolConfig::default(),
            integrations: BTreeMap::new(),
            extensions: Map::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InputToolConfig {
    pub tool: String,
    pub trigger_mode: String,
    pub shortcut: String,
    pub preferred_audio_input_device: String,
    #[serde(default = "default_audio_output_device")]
    pub preferred_audio_output_device: String,
}

impl Default for InputToolConfig {
    fn default() -> Self {
        Self {
            tool: "WeChatInput".to_string(),
            trigger_mode: "HoldToTalk".to_string(),
            shortcut: "Ctrl+F5".to_string(),
            preferred_audio_input_device: "CABLE Output".to_string(),
            preferred_audio_output_device: "CABLE Input".to_string(),
        }
    }
}

fn default_audio_output_device() -> String {
    "CABLE Input".to_string()
}

fn default_run_as_admin() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceConfig {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub container_id: Option<String>,
    pub bluetooth_address: u64,
    pub formatted_address: String,
    pub voice_mode: VoiceMode,
    pub gain: f32,
    pub mappings: Vec<MappingConfig>,
    #[serde(flatten, default)]
    pub extensions: Map<String, Value>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceMode {
    SmartToggle,
    HoldToTalk,
    #[default]
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MappingConfig {
    pub button: String,
    pub click: ActionConfig,
    pub double_click: ActionConfig,
    pub long_press: ActionConfig,
    #[serde(default)]
    pub long_press_mode: LongPressMode,
    #[serde(flatten, default)]
    pub extensions: Map<String, Value>,
}

impl MappingConfig {
    pub fn none(button: impl Into<String>) -> Self {
        Self {
            button: button.into(),
            click: ActionConfig::None,
            double_click: ActionConfig::None,
            long_press: ActionConfig::None,
            long_press_mode: LongPressMode::OneShot,
            extensions: Map::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LongPressMode {
    #[default]
    OneShot,
    Hold,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ActionConfig {
    #[default]
    None,
    Shortcut {
        keys: String,
    },
    Text {
        text: String,
    },
    Media {
        control: MediaControl,
    },
    LaunchApp {
        path: String,
        #[serde(default)]
        name: String,
    },
    ContinuousRepeat {
        #[serde(rename = "intervalMs", alias = "interval_ms")]
        interval_ms: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaControl {
    VolumeUp,
    VolumeDown,
    Mute,
    PlayPause,
}

impl AppConfig {
    pub fn device(&self, id: &str) -> Option<&DeviceConfig> {
        self.devices
            .iter()
            .find(|device| device.id.eq_ignore_ascii_case(id))
    }

    pub fn device_mut(&mut self, id: &str) -> Option<&mut DeviceConfig> {
        self.devices
            .iter_mut()
            .find(|device| device.id.eq_ignore_ascii_case(id))
    }
}
