use super::backoff::{BackoffPolicy, DeviceState};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, sleep};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const STABLE_RESET: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSnapshot {
    pub state: DeviceState,
    pub device_id: Option<String>,
    pub failures: u32,
    pub attempt: u32,
    pub last_error: Option<String>,
}

impl Default for ConnectionSnapshot {
    fn default() -> Self {
        Self {
            state: DeviceState::Disconnected,
            device_id: None,
            failures: 0,
            attempt: 0,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorCommand {
    Connect(String),
    Disconnect,
    RetryNow,
    Shutdown,
}

#[derive(Debug, thiserror::Error)]
pub enum SupervisorError {
    #[error("connection supervisor has stopped")]
    Closed,
}

#[async_trait]
pub trait LinkSession: Send + 'static {
    async fn wait_closed(&mut self) -> String;
}

#[async_trait]
pub trait LinkBackend: Send + Sync + 'static {
    type Session: LinkSession;

    async fn connect(&self, device_id: &str) -> Result<Self::Session, String>;
}

#[derive(Debug, Clone)]
pub struct SupervisorHandle {
    commands: mpsc::Sender<SupervisorCommand>,
    states: watch::Receiver<ConnectionSnapshot>,
}

impl SupervisorHandle {
    pub fn state(&self) -> ConnectionSnapshot {
        self.states.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<ConnectionSnapshot> {
        self.states.clone()
    }

    pub async fn connect(&self, device_id: impl Into<String>) -> Result<(), SupervisorError> {
        self.commands
            .send(SupervisorCommand::Connect(device_id.into()))
            .await
            .map_err(|_| SupervisorError::Closed)
    }

    pub async fn disconnect(&self) -> Result<(), SupervisorError> {
        self.commands
            .send(SupervisorCommand::Disconnect)
            .await
            .map_err(|_| SupervisorError::Closed)
    }

    pub async fn retry_now(&self) -> Result<(), SupervisorError> {
        self.commands
            .send(SupervisorCommand::RetryNow)
            .await
            .map_err(|_| SupervisorError::Closed)
    }

    pub async fn shutdown(&self) -> Result<(), SupervisorError> {
        self.commands
            .send(SupervisorCommand::Shutdown)
            .await
            .map_err(|_| SupervisorError::Closed)
    }
}

pub fn spawn_supervisor<B: LinkBackend>(
    backend: B,
    initial_device: Option<String>,
) -> SupervisorHandle {
    spawn_supervisor_with_policy(backend, initial_device, BackoffPolicy::default())
}

fn spawn_supervisor_with_policy<B: LinkBackend>(
    backend: B,
    initial_device: Option<String>,
    policy: BackoffPolicy,
) -> SupervisorHandle {
    let (command_tx, mut command_rx) = mpsc::channel(32);
    let initial = ConnectionSnapshot {
        state: if initial_device.is_some() {
            DeviceState::Connecting
        } else {
            DeviceState::Disconnected
        },
        device_id: initial_device.clone(),
        ..ConnectionSnapshot::default()
    };
    let (state_tx, state_rx) = watch::channel(initial);

    tokio::spawn(async move {
        let mut desired = initial_device;
        let mut last_device = desired.clone();
        let mut failures = 0_u32;
        let mut last_error = None;
        let mut first_attempt = true;
        let mut recovering = false;

        loop {
            let Some(device_id) = desired.clone() else {
                publish(
                    &state_tx,
                    ConnectionSnapshot {
                        state: DeviceState::Disconnected,
                        device_id: None,
                        failures,
                        attempt: 0,
                        last_error: last_error.clone(),
                    },
                );
                match command_rx.recv().await {
                    Some(SupervisorCommand::Connect(next)) => {
                        last_device = Some(next.clone());
                        desired = Some(next);
                        failures = 0;
                        last_error = None;
                        first_attempt = true;
                        recovering = false;
                    }
                    Some(SupervisorCommand::RetryNow) => {
                        if let Some(previous) = last_device.clone() {
                            desired = Some(previous);
                            failures = 0;
                            last_error = None;
                            first_attempt = true;
                            recovering = false;
                        }
                    }
                    Some(SupervisorCommand::Disconnect) => {}
                    Some(SupervisorCommand::Shutdown) | None => break,
                }
                continue;
            };

            publish(
                &state_tx,
                ConnectionSnapshot {
                    state: DeviceState::Connecting,
                    device_id: Some(device_id.clone()),
                    failures,
                    attempt: failures.saturating_add(1),
                    last_error: last_error.clone(),
                },
            );

            let mut connection = Box::pin(backend.connect(&device_id));
            let connection_result = loop {
                tokio::select! {
                    command = command_rx.recv() => {
                        match command {
                            Some(SupervisorCommand::Connect(next)) => {
                                last_device = Some(next.clone());
                                desired = Some(next);
                                failures = 0;
                                last_error = None;
                                first_attempt = true;
                                recovering = false;
                                break None;
                            }
                            Some(SupervisorCommand::Disconnect) => {
                                desired = None;
                                failures = 0;
                                last_error = None;
                                break None;
                            }
                            Some(SupervisorCommand::RetryNow) => {}
                            Some(SupervisorCommand::Shutdown) | None => return,
                        }
                    }
                    result = &mut connection => break Some(result),
                    _ = sleep(CONNECT_TIMEOUT) => {
                        break Some(Err("connection timed out".to_string()));
                    }
                }
            };

            let Some(connection_result) = connection_result else {
                continue;
            };

            match connection_result {
                Ok(mut session) => {
                    let connected_at = Instant::now();
                    first_attempt = false;
                    recovering = false;
                    publish(
                        &state_tx,
                        ConnectionSnapshot {
                            state: DeviceState::Ready,
                            device_id: Some(device_id.clone()),
                            failures,
                            attempt: 0,
                            last_error: None,
                        },
                    );

                    let mut closed = Box::pin(session.wait_closed());
                    let reason = loop {
                        tokio::select! {
                            command = command_rx.recv() => match command {
                                Some(SupervisorCommand::Connect(next)) => {
                                    last_device = Some(next.clone());
                                    desired = Some(next);
                                    failures = 0;
                                    last_error = None;
                                    first_attempt = true;
                                    recovering = false;
                                    break None;
                                }
                                Some(SupervisorCommand::Disconnect) => {
                                    desired = None;
                                    failures = 0;
                                    last_error = None;
                                    first_attempt = true;
                                    recovering = false;
                                    break None;
                                }
                                Some(SupervisorCommand::RetryNow) => {}
                                Some(SupervisorCommand::Shutdown) | None => return,
                            },
                            reason = &mut closed => break Some(reason),
                        }
                    };

                    let Some(reason) = reason else {
                        continue;
                    };
                    failures = 1;
                    last_error = Some(reason.clone());
                    let stable = connected_at.elapsed() >= STABLE_RESET;
                    publish(
                        &state_tx,
                        ConnectionSnapshot {
                            state: DeviceState::Degraded,
                            device_id: Some(device_id.clone()),
                            failures,
                            attempt: 1,
                            last_error: Some(reason),
                        },
                    );
                    if !stable {
                        desired = None;
                        last_error = Some(
                            "连路在稳定前中断，已停止自动重试；请确认蓝牙鼠标正常后手动重连"
                                .to_string(),
                        );
                        continue;
                    }
                }
                Err(error) => {
                    failures = failures.saturating_add(1);
                    last_error = Some(error.clone());
                    publish(
                        &state_tx,
                        ConnectionSnapshot {
                            state: DeviceState::Degraded,
                            device_id: Some(device_id.clone()),
                            failures,
                            attempt: failures,
                            last_error: Some(error),
                        },
                    );
                    if first_attempt || recovering {
                        desired = None;
                        first_attempt = false;
                        recovering = false;
                        continue;
                    }
                }
            }

            recovering = true;
            let delay = policy.delay(failures.saturating_sub(1));
            let mut waiting = Box::pin(sleep(delay));
            tokio::select! {
                command = command_rx.recv() => match command {
                    Some(SupervisorCommand::Connect(next)) => {
                        last_device = Some(next.clone());
                        desired = Some(next);
                        failures = 0;
                        last_error = None;
                        first_attempt = true;
                        recovering = false;
                    }
                    Some(SupervisorCommand::Disconnect) => {
                        desired = None;
                        failures = 0;
                        last_error = None;
                        first_attempt = true;
                        recovering = false;
                    }
                    Some(SupervisorCommand::RetryNow) => {
                        failures = 0;
                        last_error = None;
                    }
                    Some(SupervisorCommand::Shutdown) | None => return,
                },
                _ = &mut waiting => {}
            }
            if desired.is_none() {
                continue;
            }
            desired = last_device.clone();
        }
    });

    SupervisorHandle {
        commands: command_tx,
        states: state_rx,
    }
}

fn publish(states: &watch::Sender<ConnectionSnapshot>, snapshot: ConnectionSnapshot) {
    let _ = states.send(snapshot);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use tokio::time::{advance, timeout};

    struct MockSession {
        closed: tokio::sync::oneshot::Receiver<String>,
    }

    #[async_trait]
    impl LinkSession for MockSession {
        async fn wait_closed(&mut self) -> String {
            (&mut self.closed)
                .await
                .unwrap_or_else(|_| "session dropped".to_string())
        }
    }

    #[derive(Clone)]
    struct MockBackend {
        attempts: Arc<AtomicUsize>,
        closers: Arc<Mutex<Vec<tokio::sync::oneshot::Sender<String>>>>,
        fail_first: usize,
    }

    #[async_trait]
    impl LinkBackend for MockBackend {
        type Session = MockSession;

        async fn connect(&self, _device_id: &str) -> Result<Self::Session, String> {
            let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
            if attempt <= self.fail_first {
                return Err("remote unavailable".to_string());
            }
            let (tx, rx) = tokio::sync::oneshot::channel();
            self.closers.lock().unwrap().push(tx);
            Ok(MockSession { closed: rx })
        }
    }

    async fn wait_for(
        states: &mut watch::Receiver<ConnectionSnapshot>,
        state: DeviceState,
    ) -> ConnectionSnapshot {
        timeout(Duration::from_secs(4), async {
            loop {
                let snapshot = states.borrow().clone();
                if snapshot.state == state {
                    return snapshot;
                }
                states.changed().await.unwrap();
            }
        })
        .await
        .expect("state should be reached")
    }

    fn backend(fail_first: usize) -> MockBackend {
        MockBackend {
            attempts: Arc::new(AtomicUsize::new(0)),
            closers: Arc::new(Mutex::new(Vec::new())),
            fail_first,
        }
    }

    #[tokio::test(start_paused = true)]
    async fn reconnects_after_unexpected_disconnect() {
        let backend = backend(0);
        let handle = spawn_supervisor(backend.clone(), Some("device-1".to_string()));
        let mut states = handle.subscribe();
        wait_for(&mut states, DeviceState::Ready).await;
        advance(STABLE_RESET + Duration::from_secs(1)).await;
        let closer = backend.closers.lock().unwrap().remove(0);
        closer.send("remote slept".to_string()).unwrap();
        wait_for(&mut states, DeviceState::Degraded).await;
        wait_for(&mut states, DeviceState::Ready).await;
        assert!(backend.attempts.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test(start_paused = true)]
    async fn initial_failure_does_not_start_a_reconnect_storm() {
        let backend = backend(1);
        let handle = spawn_supervisor(backend.clone(), Some("device-1".to_string()));
        let mut states = handle.subscribe();
        wait_for(&mut states, DeviceState::Disconnected).await;
        advance(Duration::from_secs(120)).await;
        assert_eq!(backend.attempts.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(start_paused = true)]
    async fn recovery_is_limited_to_one_attempt_per_stable_session() {
        let backend = backend(0);
        let handle = spawn_supervisor(backend.clone(), Some("device-1".to_string()));
        let mut states = handle.subscribe();
        wait_for(&mut states, DeviceState::Ready).await;
        advance(STABLE_RESET + Duration::from_secs(1)).await;
        backend
            .closers
            .lock()
            .unwrap()
            .remove(0)
            .send("first drop".to_string())
            .unwrap();
        wait_for(&mut states, DeviceState::Degraded).await;
        wait_for(&mut states, DeviceState::Ready).await;
        assert_eq!(backend.attempts.load(Ordering::SeqCst), 2);

        backend
            .closers
            .lock()
            .unwrap()
            .remove(0)
            .send("short drop".to_string())
            .unwrap();
        wait_for(&mut states, DeviceState::Disconnected).await;
        advance(Duration::from_secs(300)).await;
        assert_eq!(backend.attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_now_keeps_a_healthy_session_alive() {
        let backend = backend(0);
        let handle = spawn_supervisor(backend.clone(), Some("device-1".to_string()));
        let mut states = handle.subscribe();
        wait_for(&mut states, DeviceState::Ready).await;
        handle.retry_now().await.unwrap();
        advance(Duration::from_secs(10)).await;
        assert_eq!(backend.attempts.load(Ordering::SeqCst), 1);
        assert_eq!(states.borrow().state, DeviceState::Ready);
    }

    #[tokio::test(start_paused = true)]
    async fn manual_disconnect_suppresses_reconnect() {
        let backend = backend(0);
        let handle = spawn_supervisor(backend.clone(), Some("device-1".to_string()));
        let mut states = handle.subscribe();
        wait_for(&mut states, DeviceState::Ready).await;
        handle.disconnect().await.unwrap();
        wait_for(&mut states, DeviceState::Disconnected).await;
        advance(Duration::from_secs(120)).await;
        assert_eq!(backend.attempts.load(Ordering::SeqCst), 1);
    }
}
