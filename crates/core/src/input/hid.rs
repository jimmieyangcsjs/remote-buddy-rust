use super::button::{ButtonPhase, RemoteButton};
use std::collections::BTreeSet;

const CHROMECAST_CONSUMER_USAGES: &[(u16, RemoteButton)] = &[
    (0x019E, RemoteButton::Power),
    (0x00CD, RemoteButton::PlayPause),
    (0x0042, RemoteButton::Up),
    (0x0043, RemoteButton::Down),
    (0x0044, RemoteButton::Left),
    (0x0045, RemoteButton::Right),
    (0x0041, RemoteButton::Ok),
    (0x00E2, RemoteButton::Mute),
    (0x0221, RemoteButton::Menu),
    (0x0223, RemoteButton::Home),
    (0x0224, RemoteButton::Back),
    (0x00E9, RemoteButton::VolumeUp),
    (0x00EA, RemoteButton::VolumeDown),
    (0x0077, RemoteButton::YouTube),
    (0x0078, RemoteButton::Netflix),
    (0x0079, RemoteButton::Custom),
    (0x0189, RemoteButton::Input),
    (0x0089, RemoteButton::Input),
];

const CHROMECAST_INDEXED_USAGES: [u16; 17] = [
    0x019E, 0x00CD, 0x0042, 0x0043, 0x0044, 0x0045, 0x0041, 0x00E2, 0x0221, 0x0223, 0x0224, 0x00E9,
    0x00EA, 0x0077, 0x0078, 0x0079, 0x0089,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportButtons {
    pub report_id: u8,
    pub buttons: Vec<RemoteButton>,
    pub release: bool,
}

pub fn parse_report(bytes: &[u8], model_type: &str) -> Option<ReportButtons> {
    if bytes.is_empty() {
        return None;
    }

    if model_type.eq_ignore_ascii_case("GoogleG9N9N")
        || model_type.to_ascii_lowercase().contains("chromecast")
        || model_type.to_ascii_lowercase().contains("g9n9n")
        || model_type.to_ascii_lowercase().contains("gy3le")
    {
        parse_chromecast(bytes)
    } else {
        parse_generic_consumer(bytes)
    }
}

fn parse_chromecast(bytes: &[u8]) -> Option<ReportButtons> {
    match bytes.len() {
        1 => None,
        2 => {
            let usage = u16::from_le_bytes([bytes[0], bytes[1]]);
            usage_report(2, usage)
        }
        3 => match bytes[0] {
            1 => index_report(1, &bytes[1..]),
            2 => usage_report(2, u16::from_le_bytes([bytes[1], bytes[2]])),
            _ => None,
        },
        8 => {
            if bytes[1..].iter().any(|value| *value != 0) {
                return None;
            }
            index_report(0, &bytes[..1])
        }
        _ => parse_generic_consumer(bytes),
    }
}

fn parse_generic_consumer(bytes: &[u8]) -> Option<ReportButtons> {
    match bytes.len() {
        1 => index_report(1, bytes),
        2 => usage_report(2, u16::from_le_bytes([bytes[0], bytes[1]])),
        3 if bytes[0] == 1 => index_report(1, &bytes[1..]),
        3 if bytes[0] == 2 => usage_report(2, u16::from_le_bytes([bytes[1], bytes[2]])),
        4..=8 if bytes.len().is_multiple_of(2) => {
            let mut buttons = Vec::new();
            let mut non_zero = false;
            for pair in bytes.as_chunks::<2>().0 {
                let usage = u16::from_le_bytes([pair[0], pair[1]]);
                if usage != 0 {
                    non_zero = true;
                    buttons.extend(button_for_usage(usage));
                }
            }
            Some(ReportButtons {
                report_id: 2,
                release: !non_zero,
                buttons,
            })
        }
        _ => None,
    }
}

fn usage_report(report_id: u8, usage: u16) -> Option<ReportButtons> {
    if usage == 0 {
        return Some(ReportButtons {
            report_id,
            buttons: Vec::new(),
            release: true,
        });
    }
    let button = button_for_usage(usage);
    if button.is_empty() {
        return None;
    }
    Some(ReportButtons {
        report_id,
        buttons: button,
        release: false,
    })
}

fn index_report(report_id: u8, indexes: &[u8]) -> Option<ReportButtons> {
    let mut buttons = Vec::new();
    let mut saw_valid = false;
    for index in indexes.iter().copied() {
        if index == 0 {
            continue;
        }
        let usage = *CHROMECAST_INDEXED_USAGES.get(usize::from(index - 1))?;
        saw_valid = true;
        buttons.extend(button_for_usage(usage));
    }
    Some(ReportButtons {
        report_id,
        buttons,
        release: !saw_valid,
    })
}

fn button_for_usage(usage: u16) -> Vec<RemoteButton> {
    if let Some((_, button)) = CHROMECAST_CONSUMER_USAGES
        .iter()
        .find(|(candidate, _)| *candidate == usage)
    {
        return vec![*button];
    }

    match usage {
        0x0052 | 0x0026 => vec![RemoteButton::Up],
        0x0051 => vec![RemoteButton::Down],
        0x0050 | 0x0025 => vec![RemoteButton::Left],
        0x004F | 0x0027 => vec![RemoteButton::Right],
        0x0028 | 0x000D => vec![RemoteButton::Ok],
        0x00F1 | 0x001B | 0x00A6 => vec![RemoteButton::Back],
        0x004A | 0x0024 | 0x00AC => vec![RemoteButton::Home],
        0x0065 | 0x005D => vec![RemoteButton::Menu],
        0x0080 | 0x00AF => vec![RemoteButton::VolumeUp],
        0x0081 | 0x00AE => vec![RemoteButton::VolumeDown],
        0x00AD => vec![RemoteButton::Mute],
        0x00B3 => vec![RemoteButton::PlayPause],
        0x0035 | 0x00C0 => vec![RemoteButton::LiveTv],
        0x0066 => vec![RemoteButton::Power],
        _ => Vec::new(),
    }
}

#[derive(Debug, Default)]
pub struct ReportDecoder {
    pressed: BTreeSet<RemoteButton>,
}

impl ReportDecoder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) -> Vec<(RemoteButton, ButtonPhase)> {
        let releases = self
            .pressed
            .iter()
            .copied()
            .map(|button| (button, ButtonPhase::Up))
            .collect();
        self.pressed.clear();
        releases
    }

    pub fn update(&mut self, report: &ReportButtons) -> Vec<(RemoteButton, ButtonPhase)> {
        let current: BTreeSet<_> = report.buttons.iter().copied().collect();
        let mut events = Vec::new();
        for button in self.pressed.difference(&current).copied() {
            events.push((button, ButtonPhase::Up));
        }
        for button in current.difference(&self.pressed).copied() {
            events.push((button, ButtonPhase::Down));
        }
        self.pressed = current;
        events
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_chromecast_three_byte_consumer_report() {
        let report = parse_report(&[0x02, 0x42, 0x00], "GoogleG9N9N").unwrap();
        assert_eq!(report.buttons, vec![RemoteButton::Up]);
        assert!(!report.release);
    }

    #[test]
    fn parses_indexed_eight_byte_report() {
        let report = parse_report(&[7, 0, 0, 0, 0, 0, 0, 0], "GoogleG9N9N").unwrap();
        assert_eq!(report.buttons, vec![RemoteButton::Ok]);
    }

    #[test]
    fn rejects_ambiguous_unknown_two_byte_usage() {
        assert!(parse_report(&[0x00, 0x02], "GoogleG9N9N").is_none());
    }

    #[test]
    fn zero_reports_release_all_pressed_buttons() {
        let mut decoder = ReportDecoder::new();
        let down = parse_report(&[0x02, 0x41, 0x00], "GoogleG9N9N").unwrap();
        assert_eq!(
            decoder.update(&down),
            vec![(RemoteButton::Ok, ButtonPhase::Down)]
        );
        let release = parse_report(&[0x02, 0x00, 0x00], "GoogleG9N9N").unwrap();
        assert_eq!(
            decoder.update(&release),
            vec![(RemoteButton::Ok, ButtonPhase::Up)]
        );
    }

    #[test]
    fn rejects_a_media_key_fragment_as_a_button_index() {
        assert!(parse_report(&[0x08], "GoogleG9N9N").is_none());
    }

    #[test]
    fn parses_the_real_mute_key_as_an_eight_byte_index_report() {
        let report = parse_report(&[0x08, 0, 0, 0, 0, 0, 0, 0], "GoogleG9N9N").unwrap();
        assert_eq!(report.buttons, vec![RemoteButton::Mute]);
        assert!(!report.release);
    }
}
