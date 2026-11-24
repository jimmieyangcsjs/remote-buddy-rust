use super::model::{ActionConfig, LongPressMode, MappingConfig, MediaControl};

const CONTINUOUS_REPEAT_BUTTONS: &[&str] =
    &["Up", "Down", "Left", "Right", "VolumeUp", "VolumeDown"];

pub fn default_mappings(_model_type: &str) -> Vec<MappingConfig> {
    vec![
        shortcut("Up", "Up", true),
        shortcut("Down", "Down", true),
        shortcut("Left", "Left", true),
        shortcut("Right", "Right", true),
        shortcut("Ok", "Enter", true),
        shortcut("Back", "Escape", false),
        shortcut("Home", "Win+Tab", false),
        shortcut("Menu", "Apps", false),
        media("VolumeUp", MediaControl::VolumeUp, true),
        media("VolumeDown", MediaControl::VolumeDown, true),
        media("Mute", MediaControl::Mute, false),
        media("PlayPause", MediaControl::PlayPause, false),
        shortcut("Power", "Backspace", true),
        media("Input", MediaControl::PlayPause, false),
        shortcut("YouTube", "Ctrl+R", false),
        shortcut("Netflix", "Ctrl+W", false),
        none("LiveTv"),
        none("Custom"),
    ]
}

pub(crate) fn fill_missing_defaults(existing: &mut Vec<MappingConfig>) {
    for default in default_mappings("GoogleG9N9N") {
        if let Some(current) = existing
            .iter_mut()
            .find(|mapping| mapping.button.eq_ignore_ascii_case(&default.button))
        {
            if current.click == ActionConfig::None {
                current.click = default.click;
            }
            if current.double_click == ActionConfig::None {
                current.double_click = default.double_click;
            }
            if current.long_press == ActionConfig::None {
                current.long_press = default.long_press;
            }
        } else {
            existing.push(default);
        }
    }
}

fn shortcut(button: &str, keys: &str, repeat: bool) -> MappingConfig {
    let click = ActionConfig::Shortcut {
        keys: keys.to_string(),
    };
    MappingConfig {
        button: button.to_string(),
        long_press: long_press_action(button, &click, repeat),
        click,
        double_click: ActionConfig::None,
        long_press_mode: LongPressMode::OneShot,
        extensions: Default::default(),
    }
}

fn media(button: &str, control: MediaControl, repeat: bool) -> MappingConfig {
    let click = ActionConfig::Media { control };
    MappingConfig {
        button: button.to_string(),
        long_press: long_press_action(button, &click, repeat),
        click,
        double_click: ActionConfig::None,
        long_press_mode: LongPressMode::OneShot,
        extensions: Default::default(),
    }
}

fn none(button: &str) -> MappingConfig {
    MappingConfig::none(button)
}

fn long_press_action(button: &str, click: &ActionConfig, repeat: bool) -> ActionConfig {
    if repeat && supports_continuous_repeat(button) {
        ActionConfig::ContinuousRepeat { interval_ms: 80 }
    } else {
        click.clone()
    }
}

pub(crate) fn supports_continuous_repeat(button: &str) -> bool {
    CONTINUOUS_REPEAT_BUTTONS
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(button))
}

pub(crate) fn migrate_long_press_defaults(mappings: &mut [MappingConfig]) {
    for mapping in mappings {
        if !supports_continuous_repeat(&mapping.button)
            && mapping.long_press == (ActionConfig::ContinuousRepeat { interval_ms: 80 })
        {
            mapping.long_press = mapping.click.clone();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_every_documented_g9n9n_key() {
        let mappings = default_mappings("GoogleG9N9N");
        for button in [
            "Up",
            "Down",
            "Left",
            "Right",
            "Ok",
            "Back",
            "Home",
            "Menu",
            "VolumeUp",
            "VolumeDown",
            "Mute",
            "PlayPause",
            "Power",
            "YouTube",
            "Netflix",
            "LiveTv",
            "Input",
        ] {
            assert!(
                mappings.iter().any(|mapping| mapping.button == button),
                "missing default for {button}"
            );
        }
    }

    #[test]
    fn missing_click_actions_are_filled_without_overwriting_long_press() {
        let mut existing = vec![MappingConfig {
            button: "Power".to_string(),
            click: ActionConfig::None,
            double_click: ActionConfig::None,
            long_press: ActionConfig::ContinuousRepeat { interval_ms: 40 },
            long_press_mode: LongPressMode::OneShot,
            extensions: Default::default(),
        }];

        fill_missing_defaults(&mut existing);

        let power = existing
            .iter()
            .find(|mapping| mapping.button == "Power")
            .unwrap();
        assert_eq!(
            power.click,
            ActionConfig::Shortcut {
                keys: "Backspace".to_string()
            }
        );
        assert_eq!(
            power.long_press,
            ActionConfig::ContinuousRepeat { interval_ms: 40 }
        );
        assert_eq!(existing.len(), 18);
    }

    #[test]
    fn only_navigation_and_volume_keys_use_continuous_repeat_by_default() {
        let mappings = default_mappings("GoogleG9N9N");
        for mapping in &mappings {
            if matches!(
                mapping.button.as_str(),
                "Up" | "Down" | "Left" | "Right" | "VolumeUp" | "VolumeDown"
            ) {
                assert_eq!(
                    mapping.long_press,
                    ActionConfig::ContinuousRepeat { interval_ms: 80 }
                );
            } else {
                assert_eq!(
                    mapping.long_press, mapping.click,
                    "{} should repeat its click action once on long press",
                    mapping.button
                );
            }
        }
    }
}
