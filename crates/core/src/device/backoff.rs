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
                Duration::from_secs(3),
                Duration::from_secs(15),
                Duration::from_secs(30),
                Duration::from_secs(60),
                Duration::from_secs(120),
                Duration::from_secs(300),
            ],
        }
    }
}

impl BackoffPolicy {
    pub fn from_steps(steps: impl IntoIterator<Item = Duration>) -> Self {
        let mut steps: Vec<Duration> = steps.into_iter().collect();
        if steps.is_empty() {
            steps.push(Duration::from_secs(3));
        }
        Self { steps }
    }

    pub fn delay(&self, failures: u32) -> Duration {
        let index = usize::try_from(failures)
            .unwrap_or(usize::MAX)
            .min(self.steps.len() - 1);
        self.steps[index]
    }

    pub fn steps(&self) -> &[Duration] {
        &self.steps
    }
}
