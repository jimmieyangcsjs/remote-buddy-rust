use super::backoff::{BackoffPolicy, DeviceState};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tokio::time::sleep;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);

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
        let mut failures = 0_u32;
        let mut last_error = None;

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
                        desired = Some(next);
                        failures = 0;
                        last_error = None;
                    }
                    Some(SupervisorCommand::RetryNow) => {}
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
                                desired = Some(next);
                                failures = 0;
                                last_error = None;
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
                    failures = 0;
                    last_error = None;
                    publish(
                        &state_tx,
                        ConnectionSnapshot {
                            state: DeviceState::Ready,
                            device_id: Some(device_id.clone()),
                            failures: 0,
                            attempt: 0,
                            last_error: None,
                        },
                    );

                    let mut closed = Box::pin(session.wait_closed());
                    let reason = tokio::select! {
                        command = command_rx.recv() => match command {
                            Some(SupervisorCommand::Connect(next)) => {
                                desired = Some(next);
                                failures = 0;
                                last_error = None;
                                None
                            }
                            Some(SupervisorCommand::Disconnect) => {
                                desired = None;
                                failures = 0;
                                last_error = None;
                                None
                            }
                            Some(SupervisorCommand::RetryNow) => {
                                failures = 0;
                                last_error = None;
                                Some("manual retry".to_string())
                            }
                            Some(SupervisorCommand::Shutdown) | None => return,
                        },
                        reason = &mut closed => Some(reason),
                    };

                    let Some(reason) = reason else {
                        continue;
                    };
                    failures = 1;
                    last_error = Some(reason.clone());
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
                }
            }

            let delay = policy.delay(failures.saturating_sub(1));
            let mut waiting = Box::pin(sleep(delay));
            tokio::select! {
                command = command_rx.recv() => match command {
                    Some(SupervisorCommand::Connect(next)) => {
                        desired = Some(next);
                        failures = 0;
                        last_error = None;
                    }
                    Some(SupervisorCommand::Disconnect) => {
                        desired = None;
                        failures = 0;
                        last_error = None;
                    }
                    Some(SupervisorCommand::RetryNow) => {}
                    Some(SupervisorCommand::Shutdown) | None => return,
                },
                _ = &mut waiting => {}
            }
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
    use tokio::time::timeout;

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
    }

    #[async_trait]
    impl LinkBackend for MockBackend {
        type Session = MockSession;

        async fn connect(&self, _device_id: &str) -> Result<Self::Session, String> {
            self.attempts.fetch_add(1, Ordering::SeqCst);
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

    #[tokio::test]
    async fn reconnects_after_unexpected_disconnect() {
        let backend = MockBackend {
            attempts: Arc::new(AtomicUsize::new(0)),
            closers: Arc::new(Mutex::new(Vec::new())),
        };
        let handle = spawn_supervisor(backend.clone(), Some("device-1".to_string()));
        let mut states = handle.subscribe();
        wait_for(&mut states, DeviceState::Ready).await;
        let closer = backend.closers.lock().unwrap().remove(0);
        closer.send("remote slept".to_string()).unwrap();
        wait_for(&mut states, DeviceState::Degraded).await;
        wait_for(&mut states, DeviceState::Ready).await;
        assert!(backend.attempts.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn manual_disconnect_suppresses_reconnect() {
        let backend = MockBackend {
            attempts: Arc::new(AtomicUsize::new(0)),
            closers: Arc::new(Mutex::new(Vec::new())),
        };
        let handle = spawn_supervisor(backend.clone(), Some("device-1".to_string()));
        let mut states = handle.subscribe();
        wait_for(&mut states, DeviceState::Ready).await;
        handle.disconnect().await.unwrap();
        wait_for(&mut states, DeviceState::Disconnected).await;
        sleep(Duration::from_millis(1200)).await;
        assert_eq!(backend.attempts.load(Ordering::SeqCst), 1);
    }
}
