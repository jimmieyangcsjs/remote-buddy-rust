pub mod backoff;
pub mod discovery;
pub mod supervisor;

pub use backoff::{BackoffPolicy, DeviceState};
pub use discovery::{
    DeviceCandidate, address_from_device_id, format_bluetooth_address, model_type_for_name,
    supported_remote_name, upsert_candidate,
};
pub use supervisor::{
    ConnectionSnapshot, LinkBackend, LinkSession, SupervisorCommand, SupervisorHandle,
    spawn_supervisor,
};
