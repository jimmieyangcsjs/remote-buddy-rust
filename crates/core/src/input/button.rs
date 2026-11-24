use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RemoteButton {
    Up,
    Down,
    Left,
    Right,
    Ok,
    Back,
    Home,
    Menu,
    VolumeUp,
    VolumeDown,
    Mute,
    PlayPause,
    Power,
    YouTube,
    Netflix,
    LiveTv,
    Input,
    Custom,
    Unknown,
}

impl RemoteButton {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Up => "Up",
            Self::Down => "Down",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Ok => "Ok",
            Self::Back => "Back",
            Self::Home => "Home",
            Self::Menu => "Menu",
            Self::VolumeUp => "VolumeUp",
            Self::VolumeDown => "VolumeDown",
            Self::Mute => "Mute",
            Self::PlayPause => "PlayPause",
            Self::Power => "Power",
            Self::YouTube => "YouTube",
            Self::Netflix => "Netflix",
            Self::LiveTv => "LiveTv",
            Self::Input => "Input",
            Self::Custom => "Custom",
            Self::Unknown => "Unknown",
        }
    }

    pub fn from_key(value: &str) -> Option<Self> {
        Some(match value {
            "Up" => Self::Up,
            "Down" => Self::Down,
            "Left" => Self::Left,
            "Right" => Self::Right,
            "Ok" => Self::Ok,
            "Back" => Self::Back,
            "Home" => Self::Home,
            "Menu" => Self::Menu,
            "VolumeUp" => Self::VolumeUp,
            "VolumeDown" => Self::VolumeDown,
            "Mute" => Self::Mute,
            "PlayPause" => Self::PlayPause,
            "Power" => Self::Power,
            "YouTube" => Self::YouTube,
            "Netflix" => Self::Netflix,
            "LiveTv" => Self::LiveTv,
            "Input" => Self::Input,
            "Custom" => Self::Custom,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ButtonPhase {
    Down,
    Up,
    Repeat,
}
