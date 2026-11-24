use super::InputError;
use remote_buddy_core::config::{ActionConfig, MediaControl};
use remote_buddy_core::input::{ParsedShortcut, ShortcutKey, parse_shortcut};
use std::time::Duration;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBD_EVENT_FLAGS, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY,
    KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAP_VIRTUAL_KEY_TYPE, MapVirtualKeyW, SendInput,
    VIRTUAL_KEY, VK_APPS, VK_BACK, VK_CONTROL, VK_DELETE, VK_DOWN, VK_END, VK_ESCAPE, VK_HOME,
    VK_INSERT, VK_LEFT, VK_LWIN, VK_MEDIA_PLAY_PAUSE, VK_MENU, VK_NEXT, VK_PRIOR, VK_RETURN,
    VK_RIGHT, VK_SHIFT, VK_SPACE, VK_TAB, VK_UP, VK_VOLUME_DOWN, VK_VOLUME_MUTE, VK_VOLUME_UP,
};

const SYNTHETIC_EVENT_MARKER: usize = 0x5649_4201;

pub fn send_shortcut_text(value: &str) -> Result<(), InputError> {
    let shortcut =
        parse_shortcut(value).ok_or_else(|| InputError::InvalidShortcut(value.to_string()))?;
    send_shortcut(shortcut)
}

pub fn send_shortcut_down(value: &str) -> Result<(), InputError> {
    let shortcut =
        parse_shortcut(value).ok_or_else(|| InputError::InvalidShortcut(value.to_string()))?;
    send_parsed_shortcut_down(shortcut)
}

fn send_parsed_shortcut_down(shortcut: ParsedShortcut) -> Result<(), InputError> {
    let mut inputs = modifier_inputs(shortcut, false);
    inputs.push(key_input(virtual_key(shortcut.key), false));
    send(&inputs)
}

pub fn send_shortcut_up(value: &str) -> Result<(), InputError> {
    let shortcut =
        parse_shortcut(value).ok_or_else(|| InputError::InvalidShortcut(value.to_string()))?;
    send_parsed_shortcut_up(shortcut)
}

fn send_parsed_shortcut_up(shortcut: ParsedShortcut) -> Result<(), InputError> {
    let mut inputs = vec![key_input(virtual_key(shortcut.key), true)];
    inputs.extend(modifier_inputs(shortcut, true).into_iter().rev());
    send(&inputs)
}

pub fn send_shortcut(shortcut: ParsedShortcut) -> Result<(), InputError> {
    let key = virtual_key(shortcut.key);

    let mut inputs = Vec::with_capacity(10);
    inputs.extend(modifier_inputs(shortcut, false));
    inputs.push(key_input(key, false));
    inputs.push(key_input(key, true));
    inputs.extend(modifier_inputs(shortcut, true).into_iter().rev());
    send(&inputs)
}

pub fn unlock_foreground() -> Result<(), InputError> {
    send(&[key_input(VK_MENU, false)])?;
    std::thread::sleep(Duration::from_millis(12));
    send(&[key_input(VK_MENU, true)])
}

pub fn execute_action(action: &ActionConfig) -> Result<(), InputError> {
    match action {
        ActionConfig::None | ActionConfig::ContinuousRepeat { .. } => Ok(()),
        ActionConfig::Shortcut { keys } => send_shortcut_text(keys),
        ActionConfig::Text { text } => send_text(text),
        ActionConfig::Media { control } => send_shortcut(media_shortcut(*control)),
        ActionConfig::LaunchApp { path, name } => crate::apps::focus_or_launch_named(path, name)
            .map_err(|error| InputError::Application(error.to_string())),
    }
}

pub fn execute_action_down(action: &ActionConfig) -> Result<(), InputError> {
    match action {
        ActionConfig::None | ActionConfig::ContinuousRepeat { .. } => Ok(()),
        ActionConfig::Shortcut { keys } => send_shortcut_down(keys),
        ActionConfig::Text { text } => send_text(text),
        ActionConfig::Media { control } => send_parsed_shortcut_down(media_shortcut(*control)),
        ActionConfig::LaunchApp { .. } => Ok(()),
    }
}

pub fn execute_action_up(action: &ActionConfig) -> Result<(), InputError> {
    match action {
        ActionConfig::None
        | ActionConfig::ContinuousRepeat { .. }
        | ActionConfig::Text { .. }
        | ActionConfig::LaunchApp { .. } => Ok(()),
        ActionConfig::Shortcut { keys } => send_shortcut_up(keys),
        ActionConfig::Media { control } => send_parsed_shortcut_up(media_shortcut(*control)),
    }
}

fn media_shortcut(control: MediaControl) -> ParsedShortcut {
    ParsedShortcut {
        ctrl: false,
        alt: false,
        shift: false,
        win: false,
        key: match control {
            MediaControl::VolumeUp => ShortcutKey::MediaVolumeUp,
            MediaControl::VolumeDown => ShortcutKey::MediaVolumeDown,
            MediaControl::Mute => ShortcutKey::MediaMute,
            MediaControl::PlayPause => ShortcutKey::MediaPlayPause,
        },
    }
}

fn send_text(text: &str) -> Result<(), InputError> {
    let mut inputs = Vec::with_capacity(text.encode_utf16().count() * 2);
    for unit in text.encode_utf16() {
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: unit,
                    dwFlags: KEYEVENTF_UNICODE,
                    time: 0,
                    dwExtraInfo: SYNTHETIC_EVENT_MARKER,
                },
            },
        });
        inputs.push(INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: unit,
                    dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    time: 0,
                    dwExtraInfo: SYNTHETIC_EVENT_MARKER,
                },
            },
        });
    }
    send(&inputs)
}

fn key_input(key: VIRTUAL_KEY, up: bool) -> INPUT {
    let mut flags = if up {
        KEYEVENTF_KEYUP
    } else {
        KEYBD_EVENT_FLAGS(0)
    };
    if is_extended_key(key) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: key,
                wScan: unsafe { MapVirtualKeyW(u32::from(key.0), MAP_VIRTUAL_KEY_TYPE(0)) as u16 },
                dwFlags: flags,
                time: 0,
                dwExtraInfo: SYNTHETIC_EVENT_MARKER,
            },
        },
    }
}

fn modifier_inputs(shortcut: ParsedShortcut, up: bool) -> Vec<INPUT> {
    [
        (shortcut.ctrl, VK_CONTROL),
        (shortcut.alt, VK_MENU),
        (shortcut.shift, VK_SHIFT),
        (shortcut.win, VK_LWIN),
    ]
    .into_iter()
    .filter(|(enabled, _)| *enabled)
    .map(|(_, key)| key_input(key, up))
    .collect()
}

fn is_extended_key(key: VIRTUAL_KEY) -> bool {
    matches!(
        key,
        VK_LWIN
            | VK_APPS
            | VK_LEFT
            | VK_UP
            | VK_RIGHT
            | VK_DOWN
            | VK_PRIOR
            | VK_NEXT
            | VK_END
            | VK_HOME
            | VK_INSERT
            | VK_DELETE
            | VK_VOLUME_MUTE
            | VK_VOLUME_DOWN
            | VK_VOLUME_UP
            | VK_MEDIA_PLAY_PAUSE
    )
}

fn send(inputs: &[INPUT]) -> Result<(), InputError> {
    if inputs.is_empty() {
        return Ok(());
    }
    let sent = unsafe { SendInput(inputs, std::mem::size_of::<INPUT>() as i32) };
    if sent != inputs.len() as u32 {
        return Err(InputError::Windows(windows::core::Error::from_win32()));
    }
    Ok(())
}

fn virtual_key(key: ShortcutKey) -> VIRTUAL_KEY {
    match key {
        ShortcutKey::Up => VK_UP,
        ShortcutKey::Down => VK_DOWN,
        ShortcutKey::Left => VK_LEFT,
        ShortcutKey::Right => VK_RIGHT,
        ShortcutKey::Enter => VK_RETURN,
        ShortcutKey::Escape => VK_ESCAPE,
        ShortcutKey::Tab => VK_TAB,
        ShortcutKey::Space => VK_SPACE,
        ShortcutKey::Backspace => VK_BACK,
        ShortcutKey::Delete => VK_DELETE,
        ShortcutKey::Insert => VK_INSERT,
        ShortcutKey::Home => VK_HOME,
        ShortcutKey::End => VK_END,
        ShortcutKey::PageUp => VK_PRIOR,
        ShortcutKey::PageDown => VK_NEXT,
        ShortcutKey::Apps => VK_APPS,
        ShortcutKey::Function(number) => VIRTUAL_KEY(0x70 + u16::from(number.saturating_sub(1))),
        ShortcutKey::Letter(character) => VIRTUAL_KEY(character as u16),
        ShortcutKey::Digit(digit) => VIRTUAL_KEY(0x30 + u16::from(digit)),
        ShortcutKey::MediaVolumeUp => VK_VOLUME_UP,
        ShortcutKey::MediaVolumeDown => VK_VOLUME_DOWN,
        ShortcutKey::MediaMute => VK_VOLUME_MUTE,
        ShortcutKey::MediaPlayPause => VK_MEDIA_PLAY_PAUSE,
    }
}
