use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const PIPE_PREFIX: &str = r"\\.\pipe\RemoteBuddyHidTap";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TapReport {
    pub device_id: String,
    pub bytes: Vec<u8>,
    pub captured_at_ms: u64,
}

#[derive(Debug, Error)]
pub enum TapError {
    #[error("HID tap requires administrator privileges")]
    NotElevated,
    #[error("HID over GATT module was not found")]
    ModuleNotFound,
    #[error("symbol resolution failed: {0}")]
    Symbol(String),
    #[error("injection failed: {0}")]
    Injection(String),
    #[error("IPC failed: {0}")]
    Ipc(String),
}
