use remote_buddy_core::config::{ActionConfig, LongPressMode, VoiceMode, parse_and_migrate};

const V1: &str = include_str!("fixtures/config-v1.json");

#[test]
fn migrates_v1_device_and_mapping() {
    let config = parse_and_migrate(V1).expect("v1 config should migrate");
    assert_eq!(config.schema_version, 4);
    assert_eq!(config.devices.len(), 1);
    assert_eq!(config.devices[0].name, "Chromecast Remote");
    assert_eq!(config.devices[0].voice_mode, VoiceMode::SmartToggle);
    assert_eq!(config.devices[0].mappings[0].button, "Up");
    assert_eq!(
        config.devices[0].mappings[0].click,
        ActionConfig::Shortcut {
            keys: "Up".to_string()
        }
    );
    assert_eq!(
        config.devices[0].mappings[1].click,
        ActionConfig::Shortcut {
            keys: "Backspace".to_string()
        }
    );
    assert_eq!(
        config.devices[0].mappings[1].long_press,
        ActionConfig::Shortcut {
            keys: "Backspace".to_string()
        }
    );
    assert_eq!(config.devices[0].mappings.len(), 18);
}

#[test]
fn keeps_unknown_top_level_fields() {
    let config = parse_and_migrate(V1).expect("v1 config should migrate");
    assert_eq!(
        config.extensions.get("FutureField"),
        Some(&serde_json::json!({"keep": true}))
    );
}

#[test]
fn rejects_unknown_schema_versions() {
    let raw = r#"{"schemaVersion": 99}"#;
    let error = parse_and_migrate(raw).expect_err("future schema should be rejected");
    assert!(error.to_string().contains("99"));
}

#[test]
fn round_trips_current_schema() {
    let config = parse_and_migrate(V1).expect("v1 config should migrate");
    let json = serde_json::to_string_pretty(&config).expect("config should serialize");
    let parsed = parse_and_migrate(&json).expect("current config should parse");
    assert_eq!(config, parsed);
}

#[test]
fn migrates_v2_long_press_repeats_to_one_shot_actions() {
    let mut legacy = parse_and_migrate(V1).expect("v1 config should migrate");
    legacy.schema_version = 2;
    legacy.devices[0].mappings[0].long_press = ActionConfig::ContinuousRepeat { interval_ms: 80 };
    legacy.devices[0].mappings[1].long_press = ActionConfig::ContinuousRepeat { interval_ms: 80 };
    let json = serde_json::to_string(&legacy).expect("legacy config should serialize");

    let migrated = parse_and_migrate(&json).expect("v2 config should migrate");

    assert_eq!(migrated.schema_version, 4);
    assert_eq!(
        migrated.devices[0].mappings[0].long_press,
        ActionConfig::ContinuousRepeat { interval_ms: 80 }
    );
    assert_eq!(
        migrated.devices[0].mappings[1].long_press,
        ActionConfig::Shortcut {
            keys: "Backspace".to_string()
        }
    );
}

#[test]
fn preserves_custom_long_press_repeat_intervals_during_migration() {
    let mut legacy = parse_and_migrate(V1).expect("v1 config should migrate");
    legacy.schema_version = 2;
    legacy.devices[0].mappings[1].long_press = ActionConfig::ContinuousRepeat { interval_ms: 40 };
    let json = serde_json::to_string(&legacy).expect("legacy config should serialize");

    let migrated = parse_and_migrate(&json).expect("v2 config should migrate");

    assert_eq!(
        migrated.devices[0].mappings[1].long_press,
        ActionConfig::ContinuousRepeat { interval_ms: 40 }
    );
}

#[test]
fn serializes_repeat_interval_with_the_ui_field_name() {
    let action = ActionConfig::ContinuousRepeat { interval_ms: 80 };
    let value = serde_json::to_value(&action).expect("action should serialize");

    assert_eq!(
        value,
        serde_json::json!({"type": "continuousRepeat", "intervalMs": 80})
    );
    assert_eq!(
        serde_json::from_value::<ActionConfig>(value).expect("camelCase should parse"),
        action
    );
}

#[test]
fn round_trips_application_switch_actions() {
    let action = ActionConfig::LaunchApp {
        path: r"C:\Program Files\Tencent\Weixin\Weixin.exe".to_string(),
        name: "微信".to_string(),
    };
    let value = serde_json::to_value(&action).expect("action should serialize");

    assert_eq!(value["type"], "launchApp");
    assert_eq!(value["name"], "微信");
    assert_eq!(
        serde_json::from_value::<ActionConfig>(value).expect("application action should parse"),
        action
    );
}

#[test]
fn accepts_the_legacy_repeat_interval_field() {
    let action: ActionConfig = serde_json::from_value(serde_json::json!({
        "type": "continuousRepeat",
        "interval_ms": 40
    }))
    .expect("legacy snake_case should parse");

    assert_eq!(action, ActionConfig::ContinuousRepeat { interval_ms: 40 });
}

#[test]
fn migrates_v3_mappings_to_one_shot_long_press() {
    let config = parse_and_migrate(V1).expect("v1 config should migrate");
    let mut legacy = serde_json::to_value(&config).expect("config should serialize");
    legacy["schemaVersion"] = serde_json::json!(3);
    for mapping in legacy["devices"][0]["mappings"]
        .as_array_mut()
        .expect("mappings should be an array")
    {
        mapping
            .as_object_mut()
            .expect("mapping should be an object")
            .remove("longPressMode");
    }

    let migrated =
        parse_and_migrate(&serde_json::to_string(&legacy).unwrap()).expect("v3 should migrate");

    assert_eq!(migrated.schema_version, 4);
    assert!(
        migrated.devices[0]
            .mappings
            .iter()
            .all(|mapping| mapping.long_press_mode == LongPressMode::OneShot)
    );
}

#[test]
fn serializes_long_press_mode_for_the_ui() {
    let mut config = parse_and_migrate(V1).expect("v1 config should migrate");
    config.devices[0].mappings[0].long_press_mode = LongPressMode::Hold;
    let value = serde_json::to_value(&config).expect("config should serialize");

    assert_eq!(
        value["devices"][0]["mappings"][0]["longPressMode"],
        serde_json::json!("hold")
    );
}
