pub mod etw;
pub mod hid_report;
pub mod raw_input;
pub mod send_input;

use remote_buddy_core::input::{ButtonPhase, RemoteButton};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InputSource {
    Hid,
    HidTap,
    Etw,
    RawInput,
}

impl InputSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hid => "HID report",
            Self::HidTap => "HID tap",
            Self::Etw => "BthPort ETW",
            Self::RawInput => "Raw Input",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RawButtonEvent {
    pub device_id: String,
    pub button: RemoteButton,
    pub phase: ButtonPhase,
    pub source: InputSource,
    pub captured_at_ms: u64,
    pub raw_report: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum InputError {
    #[error("input listener thread failed to start")]
    ThreadStart,
    #[error("invalid shortcut: {0}")]
    InvalidShortcut(String),
    #[error("application action failed: {0}")]
    Application(String),
    #[error("Windows API failed: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("Bluetooth ETW failed: {0}")]
    Etw(String),
}

pub use etw::{EtwInputListener, EtwStatus, guid_from_marker};
pub use hid_report::{HidReportListener, HidReportLogLevel, spawn_hid_report_listener};
pub use raw_input::{RawFrame, RawInputListener, spawn_raw_input_listener};
pub use send_input::{
    execute_action, execute_action_down, execute_action_up, send_shortcut, send_shortcut_down,
    send_shortcut_text, send_shortcut_up,
};
