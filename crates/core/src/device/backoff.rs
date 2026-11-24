use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeviceState {
    Disconnected,
    Connecting,
    Ready,
    Degraded,
    ManualDisconnected,
}

impl DeviceState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disconnected => "disconnected",
            Self::Connecting => "connecting",
            Self::Ready => "ready",
            Self::Degraded => "degraded",
            Self::ManualDisconnected => "disconnected",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackoffPolicy {
    steps: Vec<Duration>,
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        Self {
            steps: vec![
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(4),
                Duration::from_secs(8),
                Duration::from_secs(15),
                Duration::from_secs(30),
            ],
        }
    }
}

impl BackoffPolicy {
    pub fn delay(&self, failures: u32) -> Duration {
        if self.steps.is_empty() {
            return Duration::from_secs(1);
        }
        let index = usize::try_from(failures)
            .unwrap_or(usize::MAX)
            .min(self.steps.len() - 1);
        self.steps[index]
    }

    pub fn steps(&self) -> &[Duration] {
        &self.steps
    }
}
