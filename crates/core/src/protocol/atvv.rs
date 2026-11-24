use serde::{Deserialize, Serialize};

pub const OPCODE_CAPABILITIES: u8 = 0x0B;
pub const OPCODE_AUDIO_START: u8 = 0x04;
pub const OPCODE_START_SEARCH: u8 = 0x08;
pub const OPCODE_AUDIO_SYNC: u8 = 0x0A;
pub const OPCODE_MIC_REFUSED: u8 = 0x0C;
pub const OPCODE_AUDIO_STOP: u8 = 0x00;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AtvvControl {
    GetCapabilities,
    MicOpen,
    MicClose { session_id: u8 },
    MicExtend { session_id: u8 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AtvvNotification {
    Capabilities(Capabilities),
    AudioStart {
        codec: u8,
        session_id: u8,
        interaction: u8,
    },
    AudioStop,
    StartSearch,
    AudioSync {
        predictor: i16,
        step_index: u8,
    },
    MicRefused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub version: u16,
    pub codec: u8,
    pub interaction: u8,
    pub frame_size: u16,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum AtvvError {
    #[error("ATVV packet is empty")]
    Empty,
    #[error("ATVV packet is malformed for opcode 0x{0:02X}")]
    Malformed(u8),
}

pub fn encode_control(control: &AtvvControl) -> Vec<u8> {
    match control {
        AtvvControl::GetCapabilities => vec![0x0A, 0x01, 0x00, 0x00, 0x03, 0x03],
        AtvvControl::MicOpen => vec![0x0C, 0x00],
        AtvvControl::MicClose { session_id } => vec![0x0D, *session_id],
        AtvvControl::MicExtend { session_id } => vec![0x0E, *session_id],
    }
}

pub fn decode_control(bytes: &[u8]) -> Result<Option<AtvvNotification>, AtvvError> {
    let Some(opcode) = bytes.first().copied() else {
        return Err(AtvvError::Empty);
    };

    match opcode {
        OPCODE_CAPABILITIES => {
            if bytes.len() < 7 {
                return Err(AtvvError::Malformed(opcode));
            }
            Ok(Some(AtvvNotification::Capabilities(Capabilities {
                version: u16::from_le_bytes([bytes[1], bytes[2]]),
                codec: bytes[3],
                interaction: bytes[4],
                frame_size: u16::from_le_bytes([bytes[5], bytes[6]]),
            })))
        }
        OPCODE_AUDIO_START => {
            if bytes.len() < 4 {
                return Err(AtvvError::Malformed(opcode));
            }
            Ok(Some(AtvvNotification::AudioStart {
                interaction: bytes[1],
                codec: bytes[2],
                session_id: bytes[3],
            }))
        }
        OPCODE_START_SEARCH => Ok(Some(AtvvNotification::StartSearch)),
        OPCODE_AUDIO_SYNC => {
            if bytes.len() < 7 {
                return Err(AtvvError::Malformed(opcode));
            }
            Ok(Some(AtvvNotification::AudioSync {
                predictor: i16::from_be_bytes([bytes[4], bytes[5]]),
                step_index: bytes[6],
            }))
        }
        OPCODE_MIC_REFUSED => Ok(Some(AtvvNotification::MicRefused)),
        OPCODE_AUDIO_STOP => Ok(Some(AtvvNotification::AudioStop)),
        _ => Ok(None),
    }
}
