use crate::config::{AppConfig, DeviceConfig, VoiceMode, default_mappings};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceCandidate {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub bluetooth_address: u64,
    pub formatted_address: String,
    pub container_id: Option<String>,
    pub is_connected: bool,
}

impl DeviceCandidate {
    pub fn new(address: u64, name: impl Into<String>, is_connected: bool) -> Self {
        let name = name.into();
        Self {
            id: candidate_id(address),
            model_type: model_type_for_name(&name),
            formatted_address: format_bluetooth_address(address),
            bluetooth_address: address,
            container_id: None,
            is_connected,
            name,
        }
    }
}

pub fn candidate_id(address: u64) -> String {
    format!("{address:012X}")
}

pub fn address_from_device_id(device_id: &str) -> Option<u64> {
    let compact = device_id.rsplit('-').next()?.replace(':', "");
    if compact.len() != 12 || !compact.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    u64::from_str_radix(&compact, 16)
        .ok()
        .filter(|value| *value != 0)
}

pub fn format_bluetooth_address(address: u64) -> String {
    let bytes = address.to_be_bytes();
    format!(
        "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
        bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7]
    )
}

pub fn model_type_for_name(name: &str) -> String {
    if supported_remote_name(name) {
        "GoogleG9N9N".to_string()
    } else {
        "GenericBle".to_string()
    }
}

pub fn supported_remote_name(name: &str) -> bool {
    let normalized = name.to_ascii_lowercase();
    [
        "chromecast",
        "g9n9n",
        "gy3le",
        "google tv",
        "android tv",
        "voice remote",
    ]
    .iter()
    .any(|marker| normalized.contains(marker))
        || normalized.contains("remote")
}

pub fn upsert_candidate(
    config: &mut AppConfig,
    candidate: &DeviceCandidate,
) -> Option<(String, bool)> {
    if candidate.bluetooth_address == 0 {
        return None;
    }
    let requested_id = if candidate.id.trim().is_empty() {
        candidate_id(candidate.bluetooth_address)
    } else {
        candidate.id.trim().to_ascii_uppercase()
    };

    if let Some(existing) = config.devices.iter_mut().find(|device| {
        device.bluetooth_address == candidate.bluetooth_address
            || device.id.eq_ignore_ascii_case(&requested_id)
    }) {
        if !candidate.name.trim().is_empty() {
            existing.name = candidate.name.trim().to_string();
        }
        if !candidate.formatted_address.trim().is_empty() {
            existing.formatted_address = candidate.formatted_address.clone();
        }
        if candidate.container_id.is_some() {
            existing.container_id = candidate.container_id.clone();
        }
        if existing.model_type == "GenericBle" && candidate.model_type == "GoogleG9N9N" {
            existing.model_type = candidate.model_type.clone();
            existing.voice_mode = VoiceMode::SmartToggle;
        }
        return Some((existing.id.clone(), false));
    }

    let model_type = if candidate.model_type.trim().is_empty() {
        model_type_for_name(&candidate.name)
    } else {
        candidate.model_type.clone()
    };
    let voice_mode = if model_type == "GoogleG9N9N" {
        VoiceMode::SmartToggle
    } else {
        VoiceMode::Unknown
    };
    config.devices.push(DeviceConfig {
        id: requested_id.clone(),
        name: if candidate.name.trim().is_empty() {
            "Bluetooth Remote".to_string()
        } else {
            candidate.name.trim().to_string()
        },
        mappings: default_mappings(&model_type),
        model_type,
        container_id: candidate.container_id.clone(),
        bluetooth_address: candidate.bluetooth_address,
        formatted_address: if candidate.formatted_address.trim().is_empty() {
            format_bluetooth_address(candidate.bluetooth_address)
        } else {
            candidate.formatted_address.clone()
        },
        voice_mode,
        gain: 3.62,
        extensions: Default::default(),
    });

    let selected_is_valid = config
        .selected_device_id
        .as_deref()
        .is_some_and(|id| config.device(id).is_some());
    if !selected_is_valid {
        config.selected_device_id = Some(requested_id.clone());
    }
    Some((requested_id, true))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ActionConfig, AppConfig, DeviceConfig, VoiceMode};

    #[test]
    fn formats_the_windows_bluetooth_address_as_six_octets() {
        assert_eq!(
            format_bluetooth_address(0x112233445566),
            "11:22:33:44:55:66"
        );
        assert_eq!(candidate_id(0x112233445566), "112233445566");
    }

    #[test]
    fn extracts_the_bluetooth_address_from_a_windows_device_id() {
        assert_eq!(
            address_from_device_id("BluetoothLE#BluetoothLE00:11:22:33:44:55-11:22:33:44:55:66"),
            Some(0x112233445566)
        );
        assert_eq!(address_from_device_id("not-a-bluetooth-device"), None);
    }

    #[test]
    fn recognizes_chromecast_remote_names() {
        assert!(supported_remote_name("Chromecast Remote"));
        assert!(supported_remote_name("Google G9N9N Remote"));
        assert!(!supported_remote_name("Bluetooth Mouse"));
        assert_eq!(model_type_for_name("Chromecast Remote"), "GoogleG9N9N");
        assert_eq!(model_type_for_name("Bluetooth Mouse"), "GenericBle");
    }

    #[test]
    fn imports_a_paired_chromecast_with_default_mappings_and_selects_it() {
        let mut config = AppConfig::default();
        let candidate = DeviceCandidate::new(0x112233445566, "Chromecast Remote", true);

        let (id, inserted) = upsert_candidate(&mut config, &candidate).expect("candidate");

        assert_eq!(id, "112233445566");
        assert!(inserted);
        assert_eq!(config.selected_device_id.as_deref(), Some("112233445566"));
        let device = config.device("112233445566").expect("imported device");
        assert_eq!(device.name, "Chromecast Remote");
        assert_eq!(device.formatted_address, "11:22:33:44:55:66");
        assert_eq!(device.model_type, "GoogleG9N9N");
        assert_eq!(device.voice_mode, VoiceMode::SmartToggle);
        assert!(device.mappings.iter().any(|mapping| mapping.button == "Ok"));
    }

    #[test]
    fn updates_an_existing_device_without_overwriting_custom_mappings() {
        let existing = DeviceConfig {
            id: "MY-REMOTE".to_string(),
            name: "Old name".to_string(),
            model_type: "GoogleG9N9N".to_string(),
            container_id: None,
            bluetooth_address: 0x112233445566,
            formatted_address: "11:22:33:44:55:66".to_string(),
            voice_mode: VoiceMode::HoldToTalk,
            gain: 4.0,
            mappings: vec![crate::config::MappingConfig {
                button: "Ok".to_string(),
                click: ActionConfig::Shortcut {
                    keys: "Ctrl+Enter".to_string(),
                },
                double_click: ActionConfig::None,
                long_press: ActionConfig::None,
                long_press_mode: crate::config::LongPressMode::OneShot,
                extensions: Default::default(),
            }],
            extensions: Default::default(),
        };
        let mut config = AppConfig {
            selected_device_id: Some("MY-REMOTE".to_string()),
            devices: vec![existing],
            ..AppConfig::default()
        };
        let candidate = DeviceCandidate::new(0x112233445566, "Chromecast Remote", false);

        let (id, inserted) = upsert_candidate(&mut config, &candidate).expect("candidate");

        assert_eq!(id, "MY-REMOTE");
        assert!(!inserted);
        assert_eq!(config.devices.len(), 1);
        let device = config.device("MY-REMOTE").expect("existing device");
        assert_eq!(device.name, "Chromecast Remote");
        assert_eq!(device.gain, 4.0);
        assert_eq!(device.voice_mode, VoiceMode::HoldToTalk);
        assert_eq!(
            device.mappings[0].click,
            ActionConfig::Shortcut {
                keys: "Ctrl+Enter".to_string()
            }
        );
    }
}
