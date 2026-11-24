#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParsedShortcut {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    pub key: ShortcutKey,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutKey {
    Up,
    Down,
    Left,
    Right,
    Enter,
    Escape,
    Tab,
    Space,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    Apps,
    Function(u8),
    Letter(char),
    Digit(u8),
    MediaVolumeUp,
    MediaVolumeDown,
    MediaMute,
    MediaPlayPause,
}

pub fn parse_shortcut(value: &str) -> Option<ParsedShortcut> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut win = false;
    let mut key = None;

    for part in trimmed
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
    {
        match part.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => ctrl = true,
            "alt" => alt = true,
            "shift" => shift = true,
            "win" | "windows" | "meta" | "cmd" => win = true,
            _ => {
                if key.is_some() {
                    return None;
                }
                key = Some(parse_key(part)?);
            }
        }
    }

    Some(ParsedShortcut {
        ctrl,
        alt,
        shift,
        win,
        key: key?,
    })
}

fn parse_key(value: &str) -> Option<ShortcutKey> {
    let upper = value.to_ascii_uppercase();
    let key = match upper.as_str() {
        "UP" => ShortcutKey::Up,
        "DOWN" => ShortcutKey::Down,
        "LEFT" => ShortcutKey::Left,
        "RIGHT" => ShortcutKey::Right,
        "ENTER" | "RETURN" => ShortcutKey::Enter,
        "ESC" | "ESCAPE" => ShortcutKey::Escape,
        "TAB" => ShortcutKey::Tab,
        "SPACE" => ShortcutKey::Space,
        "BACKSPACE" | "BKSP" => ShortcutKey::Backspace,
        "DELETE" | "DEL" => ShortcutKey::Delete,
        "INSERT" | "INS" => ShortcutKey::Insert,
        "HOME" => ShortcutKey::Home,
        "END" => ShortcutKey::End,
        "PAGEUP" | "PGUP" => ShortcutKey::PageUp,
        "PAGEDOWN" | "PGDN" => ShortcutKey::PageDown,
        "APPS" | "CONTEXTMENU" => ShortcutKey::Apps,
        "MEDIAVOLUMEUP" | "VOLUMEUP" => ShortcutKey::MediaVolumeUp,
        "MEDIAVOLUMEDOWN" | "VOLUMEDOWN" => ShortcutKey::MediaVolumeDown,
        "MEDIAMUTE" | "MUTE" => ShortcutKey::MediaMute,
        "MEDIAPLAYPAUSE" | "PLAYPAUSE" => ShortcutKey::MediaPlayPause,
        _ => {
            if let Some(number) = upper
                .strip_prefix('F')
                .and_then(|number| number.parse::<u8>().ok())
                .filter(|number| (1..=24).contains(number))
            {
                ShortcutKey::Function(number)
            } else if value.chars().count() == 1 {
                let character = value.chars().next()?.to_ascii_uppercase();
                if character.is_ascii_alphabetic() {
                    ShortcutKey::Letter(character)
                } else if character.is_ascii_digit() {
                    ShortcutKey::Digit(character.to_digit(10)? as u8)
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
    };
    Some(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ctrl_function_key() {
        let shortcut = parse_shortcut("Ctrl+F5").unwrap();
        assert!(shortcut.ctrl);
        assert_eq!(shortcut.key, ShortcutKey::Function(5));
    }

    #[test]
    fn parses_win_tab_and_plain_direction() {
        assert!(parse_shortcut("Win+Tab").unwrap().win);
        assert_eq!(parse_shortcut("Up").unwrap().key, ShortcutKey::Up);
    }

    #[test]
    fn rejects_unknown_key() {
        assert!(parse_shortcut("Ctrl+NotAKey").is_none());
        assert!(parse_shortcut("A+B").is_none());
    }
}
