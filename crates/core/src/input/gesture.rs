use super::button::{ButtonPhase, RemoteButton};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const DOUBLE_CLICK_WINDOW_MS: u64 = 350;
pub const LONG_PRESS_MS: u64 = 700;
pub const REPEAT_INTERVAL_MS: u64 = 80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Gesture {
    Click,
    DoubleClick,
    LongPress,
    Repeat,
}

#[derive(Debug, Default)]
struct ButtonState {
    down_at: Option<u64>,
    last_click_at: Option<u64>,
    pending_click: bool,
    next_repeat_at: Option<u64>,
}

#[derive(Debug, Default)]
pub struct GestureRecognizer {
    buttons: HashMap<RemoteButton, ButtonState>,
}

impl GestureRecognizer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn on_event(
        &mut self,
        button: RemoteButton,
        phase: ButtonPhase,
        now_ms: u64,
        double_click_enabled: bool,
    ) -> Vec<Gesture> {
        let state = self.buttons.entry(button).or_default();
        match phase {
            ButtonPhase::Down => {
                state.down_at = Some(now_ms);
                state.next_repeat_at = Some(now_ms + LONG_PRESS_MS);
                Vec::new()
            }
            ButtonPhase::Repeat => {
                if state.next_repeat_at.is_none_or(|next| now_ms >= next) {
                    state.next_repeat_at = Some(now_ms + REPEAT_INTERVAL_MS);
                    vec![Gesture::Repeat]
                } else {
                    Vec::new()
                }
            }
            ButtonPhase::Up => {
                let held_for = state.down_at.map_or(0, |down| now_ms.saturating_sub(down));
                state.down_at = None;
                state.next_repeat_at = None;
                if held_for >= LONG_PRESS_MS {
                    state.pending_click = false;
                    state.last_click_at = None;
                    return vec![Gesture::LongPress];
                }

                let is_double = double_click_enabled
                    && state
                        .last_click_at
                        .is_some_and(|last| now_ms.saturating_sub(last) <= DOUBLE_CLICK_WINDOW_MS);
                if is_double {
                    state.pending_click = false;
                    state.last_click_at = None;
                    vec![Gesture::DoubleClick]
                } else if !double_click_enabled {
                    state.pending_click = false;
                    state.last_click_at = None;
                    vec![Gesture::Click]
                } else {
                    state.pending_click = true;
                    state.last_click_at = Some(now_ms);
                    Vec::new()
                }
            }
        }
    }

    pub fn poll(&mut self, now_ms: u64) -> Vec<Gesture> {
        self.poll_events(now_ms)
            .into_iter()
            .map(|(_, gesture)| gesture)
            .collect()
    }

    pub fn poll_events(&mut self, now_ms: u64) -> Vec<(RemoteButton, Gesture)> {
        let mut gestures = Vec::new();
        for (button, state) in &mut self.buttons {
            if state.pending_click
                && state
                    .last_click_at
                    .is_some_and(|last| now_ms.saturating_sub(last) >= DOUBLE_CLICK_WINDOW_MS)
            {
                state.pending_click = false;
                state.last_click_at = None;
                gestures.push((*button, Gesture::Click));
            }
        }
        gestures
    }
}
