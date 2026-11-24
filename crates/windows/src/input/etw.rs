use super::raw_input::RawFrame;
use super::{InputError, InputSource};
use crate::capture_profile::capture_profile_ready;
use remote_buddy_core::input::parse_report;
use remote_buddy_core::protocol::{AtvvNotification, decode_control};
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, ERROR_MORE_DATA, ERROR_SUCCESS};
use windows::Win32::System::Diagnostics::Etw::{
    CloseTrace, ControlTraceW, EVENT_CONTROL_CODE_ENABLE_PROVIDER, EVENT_RECORD,
    EVENT_TRACE_CONTROL_FLUSH, EVENT_TRACE_CONTROL_STOP, EVENT_TRACE_LOGFILEW,
    EVENT_TRACE_PROPERTIES, EnableTraceEx2, OpenTraceW, PROCESS_TRACE_MODE_EVENT_RECORD,
    PROCESS_TRACE_MODE_REAL_TIME, PROPERTY_DATA_DESCRIPTOR, ProcessTrace, QueryAllTracesW,
    StartTraceW, TdhGetProperty, TdhGetPropertySize, WNODE_FLAG_TRACED_GUID,
};
use windows::core::{GUID, PCWSTR, PWSTR};

const BTHPORT_PROVIDER: GUID = GUID::from_u128(0x8a1f9517_3a8c_4a9e_a018_4f17a200f277);
const HCIRAW_KEYWORD: u64 = 0x8000000000000000;
const EVENT_ID_HCI_RAW: u16 = 402;
const BIP_HCI_EVENT: u8 = 2;
const BIP_ACL_RECEIVED: u8 = 3;
const BIP_ACL_SENT: u8 = 4;
const ETW_SESSION_PREFIX: &str = "RemoteBuddyBthEtw";
const ETW_SESSION_NAME: &str = "RemoteBuddyBthEtwLive";
const MAX_STALE_SESSIONS: usize = 64;
const STALE_SESSION_NAME_BYTES: usize = 2048;
const PROBE_LIFETIME: Duration = Duration::from_secs(15);
const BIP_TYPE_PROPERTY: &[u16] = &[0x42, 0x49, 0x50, 0x5F, 0x54, 0x79, 0x70, 0x65, 0];
const BIP_DATA_PROPERTY: &[u16] = &[0x42, 0x49, 0x50, 0x5F, 0x44, 0x61, 0x74, 0x61, 0];
const BIP_DATA_LEN_PROPERTY: &[u16] = &[
    0x42, 0x49, 0x50, 0x5F, 0x44, 0x61, 0x74, 0x61, 0x4C, 0x65, 0x6E, 0,
];

#[derive(Debug, Clone)]
pub struct EtwStatus {
    pub state: String,
    pub detail: String,
    pub bound_devices: usize,
    pub reports: u64,
    pub packets: u64,
    pub redacted: u64,
    pub sensitive_data: bool,
    pub setup_required: bool,
    pub last_event: Option<String>,
    pub last_error: Option<String>,
}

impl Default for EtwStatus {
    fn default() -> Self {
        Self {
            state: "starting".to_string(),
            detail: "正在启动 BthPort ETW 采集".to_string(),
            bound_devices: 0,
            reports: 0,
            packets: 0,
            redacted: 0,
            sensitive_data: false,
            setup_required: false,
            last_event: None,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone)]
struct Probe {
    device_id: String,
    model_type: String,
    request: Vec<u8>,
    created: Instant,
}

#[derive(Debug, Clone)]
struct Binding {
    device_id: String,
    model_type: String,
}

#[derive(Clone)]
struct AtvvWatch {
    device_id: String,
    attribute_handle: u16,
    sender: UnboundedSender<()>,
}

struct Shared {
    sender: UnboundedSender<RawFrame>,
    status: Arc<RwLock<EtwStatus>>,
    probes: Mutex<Vec<Probe>>,
    pending: Mutex<HashMap<u16, Probe>>,
    bindings: Mutex<HashMap<u16, Binding>>,
    fallback: Mutex<Option<Binding>>,
    atvv_watch: Mutex<Option<AtvvWatch>>,
    packets: AtomicU64,
    redacted: AtomicU64,
    sensitive_data: bool,
    last_event: Mutex<Option<String>>,
}

struct Session {
    properties: Box<[usize]>,
    logger_name: Vec<u16>,
    trace_handle: windows::Win32::System::Diagnostics::Etw::PROCESSTRACE_HANDLE,
    session_handle: windows::Win32::System::Diagnostics::Etw::CONTROLTRACE_HANDLE,
    thread: Option<std::thread::JoinHandle<()>>,
    flush_thread: Option<std::thread::JoinHandle<()>>,
}

pub struct EtwInputListener {
    shared: Arc<Shared>,
    session: Arc<Mutex<Option<Session>>>,
    stopped: Arc<AtomicBool>,
    refs: Arc<AtomicUsize>,
}

impl Clone for EtwInputListener {
    fn clone(&self) -> Self {
        self.refs.fetch_add(1, Ordering::SeqCst);
        Self {
            shared: self.shared.clone(),
            session: self.session.clone(),
            stopped: self.stopped.clone(),
            refs: self.refs.clone(),
        }
    }
}

impl EtwInputListener {
    pub fn spawn(
        sender: UnboundedSender<RawFrame>,
        status: Arc<RwLock<EtwStatus>>,
    ) -> Result<Self, InputError> {
        let sensitive_data = capture_profile_ready();
        if let Ok(mut status) = status.write() {
            status.sensitive_data = sensitive_data;
            status.setup_required = !sensitive_data;
            if !sensitive_data {
                status.state = "needsSetup".to_string();
                status.detail =
                    "Windows 蓝牙 ETW 缺少完整载荷；请启用按键增强后重启蓝牙".to_string();
            }
        }
        let shared = Arc::new(Shared {
            sender,
            status,
            probes: Mutex::new(Vec::new()),
            pending: Mutex::new(HashMap::new()),
            bindings: Mutex::new(HashMap::new()),
            fallback: Mutex::new(None),
            atvv_watch: Mutex::new(None),
            packets: AtomicU64::new(0),
            redacted: AtomicU64::new(0),
            sensitive_data,
            last_event: Mutex::new(None),
        });
        let session = Arc::new(Mutex::new(None));
        let stopped = Arc::new(AtomicBool::new(false));
        let listener = Self {
            shared: shared.clone(),
            session: session.clone(),
            stopped: stopped.clone(),
            refs: Arc::new(AtomicUsize::new(1)),
        };
        listener.start_session()?;
        Ok(listener)
    }

    /// Register a one-shot GATT service query whose ATT request identifies
    /// the ACL connection of this device. Returns the 128-bit marker bytes
    /// that must be used for the query.
    pub fn register_probe(
        &self,
        device_id: &str,
        model_type: &str,
        address: u64,
    ) -> Option<[u8; 16]> {
        let now = Instant::now();
        if let Ok(mut probes) = self.shared.probes.lock() {
            probes.retain(|probe| now.duration_since(probe.created) < PROBE_LIFETIME);
        }
        let marker = marker_bytes(address);
        let mut request = vec![0x06, 0x01, 0x00, 0xFF, 0xFF, 0x00, 0x28];
        request.extend_from_slice(&marker);
        if let Ok(mut probes) = self.shared.probes.lock() {
            probes.retain(|probe| probe.device_id != device_id);
            probes.push(Probe {
                device_id: device_id.to_string(),
                model_type: model_type.to_string(),
                request,
                created: now,
            });
        }
        Some(marker)
    }

    pub fn status(&self) -> EtwStatus {
        self.shared
            .status
            .read()
            .map(|status| status.clone())
            .unwrap_or_default()
    }

    pub fn bound_devices(&self) -> usize {
        self.shared
            .bindings
            .lock()
            .map(|bindings| bindings.len())
            .unwrap_or(0)
    }

    pub fn is_device_bound(&self, device_id: &str) -> bool {
        self.shared
            .bindings
            .lock()
            .map(|bindings| {
                bindings
                    .values()
                    .any(|binding| binding.device_id.eq_ignore_ascii_case(device_id))
            })
            .unwrap_or(false)
    }

    /// Bind on the first HID notification if the GATT service probe does not
    /// complete. Only the currently selected remote is armed, so an unrelated
    /// Bluetooth notification cannot be attributed to it.
    pub fn arm_fallback_binding(&self, device_id: &str, model_type: &str) {
        if let Ok(mut fallback) = self.shared.fallback.lock() {
            *fallback = Some(Binding {
                device_id: device_id.to_string(),
                model_type: model_type.to_string(),
            });
        }
    }

    /// Watch the ATVV control characteristic directly from BthPort ETW. Some
    /// Windows/Bluetooth driver combinations deliver the characteristic
    /// notifications to the raw trace but not to the WinRT ValueChanged
    /// handler, so the voice key needs this independent fallback.
    pub fn watch_atvv_control(
        &self,
        device_id: &str,
        attribute_handle: u16,
        sender: UnboundedSender<()>,
    ) {
        if let Ok(mut watch) = self.shared.atvv_watch.lock() {
            *watch = Some(AtvvWatch {
                device_id: device_id.to_string(),
                attribute_handle,
                sender,
            });
        }
    }

    fn start_session(&self) -> Result<(), InputError> {
        let shared = self.shared.clone();
        let session_slot = self.session.clone();
        let stopped = self.stopped.clone();
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);
        let thread = std::thread::Builder::new()
            .name("remote-buddy-etw".to_string())
            .spawn(move || run_session(shared, session_slot, stopped, ready_tx))
            .map_err(|_| InputError::ThreadStart)?;
        match ready_rx.recv() {
            Ok(Ok(())) => {
                if let Ok(mut slot) = self.session.lock()
                    && let Some(session) = slot.as_mut()
                {
                    session.thread = Some(thread);
                }
                Ok(())
            }
            Ok(Err(error)) => {
                let _ = thread.join();
                Err(InputError::Etw(error))
            }
            Err(_) => Err(InputError::ThreadStart),
        }
    }
}

impl Drop for EtwInputListener {
    fn drop(&mut self) {
        if self.refs.fetch_sub(1, Ordering::SeqCst) != 1 {
            return;
        }
        if self.stopped.swap(true, Ordering::SeqCst) {
            return;
        }
        let session = self.session.lock().ok().and_then(|mut slot| slot.take());
        if let Some(mut session) = session {
            unsafe {
                let _ = CloseTrace(session.trace_handle);
                let _ = ControlTraceW(
                    session.session_handle,
                    PCWSTR(session.logger_name.as_ptr()),
                    session
                        .properties
                        .as_mut_ptr()
                        .cast::<EVENT_TRACE_PROPERTIES>(),
                    EVENT_TRACE_CONTROL_STOP,
                );
            }
            if let Some(thread) = session.flush_thread.take() {
                let _ = thread.join();
            }
            if let Some(thread) = session.thread.take() {
                let _ = thread.join();
            }
        }
    }
}

fn run_session(
    shared: Arc<Shared>,
    session_slot: Arc<Mutex<Option<Session>>>,
    stopped: Arc<AtomicBool>,
    ready: std::sync::mpsc::SyncSender<Result<(), String>>,
) {
    let cleanup_warning = cleanup_stale_sessions(&shared);
    let name = wide(ETW_SESSION_NAME);
    let properties_size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() + name.len() * 2;
    let words = properties_size.div_ceil(std::mem::size_of::<usize>());
    let mut properties = vec![0usize; words].into_boxed_slice();
    let props = properties.as_mut_ptr().cast::<EVENT_TRACE_PROPERTIES>();

    unsafe {
        (*props).Wnode.BufferSize = (properties.len() * std::mem::size_of::<usize>()) as u32;
        (*props).Wnode.Flags = WNODE_FLAG_TRACED_GUID;
        (*props).Wnode.ClientContext = 1;
        (*props).BufferSize = 64;
        (*props).MinimumBuffers = 4;
        (*props).MaximumBuffers = 16;
        (*props).LogFileMode = PROCESS_TRACE_MODE_REAL_TIME;
        (*props).FlushTimer = 1;
        (*props).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
        (*props).LogFileNameOffset = 0;
        std::ptr::copy_nonoverlapping(
            name.as_ptr(),
            properties
                .as_mut_ptr()
                .cast::<u8>()
                .add(std::mem::size_of::<EVENT_TRACE_PROPERTIES>())
                .cast::<u16>(),
            name.len(),
        );
    }

    let mut session_handle = Default::default();
    let mut start = unsafe { StartTraceW(&mut session_handle, PCWSTR(name.as_ptr()), props) };
    if start == ERROR_ALREADY_EXISTS {
        unsafe {
            let _ = ControlTraceW(
                Default::default(),
                PCWSTR(name.as_ptr()),
                props,
                EVENT_TRACE_CONTROL_STOP,
            );
        }
        start = unsafe { StartTraceW(&mut session_handle, PCWSTR(name.as_ptr()), props) };
    }
    if start != ERROR_SUCCESS {
        let message = format!("StartTraceW failed: {}", start.0);
        set_error(&shared, &message);
        let _ = ready.send(Err(message));
        return;
    }

    let enable = unsafe {
        EnableTraceEx2(
            session_handle,
            &BTHPORT_PROVIDER,
            EVENT_CONTROL_CODE_ENABLE_PROVIDER.0,
            4,
            HCIRAW_KEYWORD,
            0,
            0,
            None,
        )
    };
    if enable != ERROR_SUCCESS {
        let message = format!("EnableTraceEx2 failed: {}", enable.0);
        unsafe {
            let _ = ControlTraceW(
                session_handle,
                PCWSTR(name.as_ptr()),
                props,
                EVENT_TRACE_CONTROL_STOP,
            );
        }
        set_error(&shared, &message);
        let _ = ready.send(Err(message));
        return;
    }

    let mut logfile = EVENT_TRACE_LOGFILEW {
        LoggerName: PWSTR(name.as_ptr() as *mut u16),
        ..Default::default()
    };
    logfile.Anonymous1.ProcessTraceMode =
        PROCESS_TRACE_MODE_REAL_TIME | PROCESS_TRACE_MODE_EVENT_RECORD;
    logfile.Anonymous2.EventRecordCallback = Some(event_callback);
    logfile.Context = Arc::as_ptr(&shared) as *mut c_void;
    let trace_handle = unsafe { OpenTraceW(&mut logfile) };
    if trace_handle.Value == u64::MAX {
        let message = "OpenTraceW failed".to_string();
        unsafe {
            let _ = ControlTraceW(
                session_handle,
                PCWSTR(name.as_ptr()),
                props,
                EVENT_TRACE_CONTROL_STOP,
            );
        }
        set_error(&shared, &message);
        let _ = ready.send(Err(message));
        return;
    }

    {
        let flush_thread = spawn_flush_thread(session_handle, name.clone(), stopped.clone());
        let mut slot = session_slot.lock().expect("etw session lock");
        *slot = Some(Session {
            properties,
            trace_handle,
            session_handle,
            thread: None,
            flush_thread: Some(flush_thread),
            logger_name: name.clone(),
        });
    }
    if let Ok(mut status) = shared.status.write() {
        status.state = if shared.sensitive_data {
            "degraded"
        } else {
            "needsSetup"
        }
        .to_string();
        status.detail = if shared.sensitive_data {
            "BthPort ETW 已启动，等待设备连接和 GATT 绑定".to_string()
        } else {
            "ETW 会话已启动，但 Windows 尚未提供按键载荷".to_string()
        };
        status.last_error = cleanup_warning;
    }
    let _ = ready.send(Ok(()));

    unsafe {
        let _ = ProcessTrace(&[trace_handle], None, None);
    }
    unsafe {
        let _ = CloseTrace(trace_handle);
        let _ = ControlTraceW(
            session_handle,
            PCWSTR(name.as_ptr()),
            props,
            EVENT_TRACE_CONTROL_STOP,
        );
    }
    if !stopped.load(Ordering::SeqCst)
        && let Ok(mut status) = shared.status.write()
    {
        status.state = "unavailable".to_string();
        status.detail = "BthPort ETW 实时会话已结束".to_string();
    }
}

fn cleanup_stale_sessions(shared: &Shared) -> Option<String> {
    let properties_size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>();
    let buffer_size = properties_size + STALE_SESSION_NAME_BYTES;
    let words = buffer_size.div_ceil(std::mem::size_of::<usize>());
    let mut buffers = (0..MAX_STALE_SESSIONS)
        .map(|_| vec![0usize; words].into_boxed_slice())
        .collect::<Vec<_>>();
    let mut pointers = buffers
        .iter_mut()
        .map(|buffer| buffer.as_mut_ptr().cast::<EVENT_TRACE_PROPERTIES>())
        .collect::<Vec<_>>();

    for pointer in &pointers {
        unsafe {
            let properties = &mut **pointer;
            properties.Wnode.BufferSize = buffer_size as u32;
            properties.LoggerNameOffset = properties_size as u32;
        }
    }

    let mut count = 0u32;
    let query = unsafe { QueryAllTracesW(&mut pointers, &mut count) };
    if query != ERROR_SUCCESS && query != ERROR_MORE_DATA {
        let warning = format!("查询残留 ETW 会话失败：{}", query.0);
        if let Ok(mut status) = shared.status.write() {
            status.last_error = Some(warning.clone());
        }
        return Some(warning);
    }

    let prefix = ETW_SESSION_PREFIX.encode_utf16().collect::<Vec<_>>();
    let mut first_error = None;
    for pointer in pointers.iter().take(count as usize) {
        let pointer = *pointer;
        let properties = unsafe { &*pointer };
        let offset = properties.LoggerNameOffset as usize;
        if offset < properties_size || offset + std::mem::size_of::<u16>() > buffer_size {
            continue;
        }

        let max_units = (buffer_size - offset) / std::mem::size_of::<u16>();
        let name_pointer = unsafe { (pointer as *const u8).add(offset).cast::<u16>() };
        let units = unsafe { std::slice::from_raw_parts(name_pointer, max_units) };
        let Some(end) = units.iter().position(|unit| *unit == 0) else {
            continue;
        };
        let name = &units[..end];
        if !name.starts_with(&prefix) {
            continue;
        }

        let result = unsafe {
            ControlTraceW(
                Default::default(),
                PCWSTR(name_pointer),
                pointer,
                EVENT_TRACE_CONTROL_STOP,
            )
        };
        if result != ERROR_SUCCESS && first_error.is_none() {
            first_error = Some(format!(
                "停止残留 ETW 会话 {} 失败：{}",
                String::from_utf16_lossy(name),
                result.0
            ));
        }
    }

    if let Some(warning) = first_error {
        if let Ok(mut status) = shared.status.write() {
            status.last_error = Some(warning.clone());
        }
        Some(warning)
    } else {
        None
    }
}

fn spawn_flush_thread(
    session_handle: windows::Win32::System::Diagnostics::Etw::CONTROLTRACE_HANDLE,
    name: Vec<u16>,
    stopped: Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    std::thread::Builder::new()
        .name("remote-buddy-etw-flush".to_string())
        .spawn(move || {
            let size = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() + name.len() * 2;
            let words = size.div_ceil(std::mem::size_of::<usize>());
            let mut properties = vec![0usize; words].into_boxed_slice();
            let props = properties.as_mut_ptr().cast::<EVENT_TRACE_PROPERTIES>();
            unsafe {
                (*props).Wnode.BufferSize =
                    (properties.len() * std::mem::size_of::<usize>()) as u32;
                (*props).LoggerNameOffset = std::mem::size_of::<EVENT_TRACE_PROPERTIES>() as u32;
                std::ptr::copy_nonoverlapping(
                    name.as_ptr(),
                    properties
                        .as_mut_ptr()
                        .cast::<u8>()
                        .add(std::mem::size_of::<EVENT_TRACE_PROPERTIES>())
                        .cast::<u16>(),
                    name.len(),
                );
            }
            while !stopped.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(100));
                let _ = unsafe {
                    ControlTraceW(
                        session_handle,
                        PCWSTR::null(),
                        props,
                        EVENT_TRACE_CONTROL_FLUSH,
                    )
                };
            }
        })
        .unwrap_or_else(|_| std::thread::spawn(|| {}))
}

fn set_error(shared: &Shared, message: &str) {
    if let Ok(mut status) = shared.status.write() {
        status.state = "unavailable".to_string();
        status.detail = message.to_string();
        status.last_error = Some(message.to_string());
    }
}

unsafe extern "system" fn event_callback(record: *mut EVENT_RECORD) {
    if record.is_null() {
        return;
    }
    let record = unsafe { &*record };
    if record.EventHeader.ProviderId != BTHPORT_PROVIDER
        || record.EventHeader.EventDescriptor.Id != EVENT_ID_HCI_RAW
        || record.UserContext.is_null()
    {
        return;
    }
    let shared = unsafe { &*(record.UserContext as *const Shared) };
    let packet_count = shared.packets.fetch_add(1, AtomicOrdering::Relaxed) + 1;
    let (kind, data) = match (
        property_u8(record, BIP_TYPE_PROPERTY),
        property(record, BIP_DATA_PROPERTY),
    ) {
        (Some(kind), Some(data)) => {
            let length = property_u32(record, BIP_DATA_LEN_PROPERTY)
                .map(|value| value as usize)
                .unwrap_or(data.len());
            if length > data.len() {
                return;
            }
            (kind, data[..length].to_vec())
        }
        _ if record.EventHeader.EventDescriptor.Version == 0 => {
            if record.UserData.is_null() || record.UserDataLength <= 8 {
                return;
            }
            let data = unsafe {
                std::slice::from_raw_parts(
                    record.UserData as *const u8,
                    record.UserDataLength as usize,
                )
            };
            let declared = u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize;
            if declared != data.len().saturating_sub(8) {
                return;
            }
            (data[3], data[8..].to_vec())
        }
        _ => return,
    };
    if let Ok(mut last_event) = shared.last_event.lock()
        && packet_count <= 5
    {
        *last_event = Some(format!(
            "Event 402 #{packet_count} · kind={kind} · {} 字节",
            data.len()
        ));
    }
    let packet = data.as_slice();
    match kind {
        BIP_ACL_SENT => handle_sent(shared, packet),
        BIP_ACL_RECEIVED => handle_received(shared, packet),
        BIP_HCI_EVENT => handle_hci_event(shared, packet),
        _ => {}
    }
}

fn handle_sent(shared: &Shared, packet: &[u8]) {
    let Some((connection, att, _incomplete)) = att_packet(packet) else {
        return;
    };
    let now = Instant::now();
    let probe = shared.probes.lock().ok().and_then(|mut probes| {
        probes.retain(|probe| now.duration_since(probe.created) < PROBE_LIFETIME);
        probes
            .iter()
            .find(|probe| att == probe.request.as_slice())
            .cloned()
    });
    let Some(probe) = probe else {
        return;
    };
    if let Ok(mut pending) = shared.pending.lock() {
        pending.insert(connection, probe.clone());
    }
    if let Ok(mut bindings) = shared.bindings.lock() {
        bindings.retain(|_, binding| binding.device_id != probe.device_id);
    }
}

fn handle_received(shared: &Shared, packet: &[u8]) {
    let Some((connection, att, incomplete)) = att_packet(packet) else {
        return;
    };
    if att.len() < 3 {
        return;
    }
    if incomplete {
        if shared
            .fallback
            .lock()
            .map(|fallback| fallback.is_some())
            .unwrap_or(false)
        {
            mark_incomplete_report(shared);
        }
        return;
    }

    let mut binding = shared
        .bindings
        .lock()
        .ok()
        .and_then(|bindings| bindings.get(&connection).cloned());
    if binding.is_none() {
        let pending = shared
            .pending
            .lock()
            .ok()
            .and_then(|mut pending| pending.remove(&connection));
        if let Some(probe) = pending {
            binding = Some(Binding {
                device_id: probe.device_id.clone(),
                model_type: probe.model_type.clone(),
            });
            if let Ok(mut bindings) = shared.bindings.lock() {
                bindings.retain(|_, existing| existing.device_id != probe.device_id);
                bindings.insert(connection, binding.clone().expect("binding"));
            }
            if let Ok(mut probes) = shared.probes.lock() {
                probes.retain(|candidate| candidate.device_id != probe.device_id);
            }
            update_bound_status(shared);
        }
    }
    if binding.is_none()
        && let Some(fallback) = bind_from_hid_notification(shared, connection, att)
    {
        binding = Some(fallback);
    }

    let Some(value) = notification_value(att) else {
        return;
    };
    forward_atvv_start_search(shared, att, value, binding.as_ref());
    let Some(binding) = binding else {
        return;
    };
    let Some(_report) = parse_report(value, &binding.model_type) else {
        if let Ok(mut status) = shared.status.write() {
            status.last_event = Some(format!(
                "未识别的 HID 通知 · attr=0x{:04X} · {}",
                u16::from_le_bytes([att[1], att[2]]),
                value
                    .iter()
                    .take(16)
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
        return;
    };
    let bytes = value.to_vec();
    if let Ok(mut status) = shared.status.write() {
        status.state = "ready".to_string();
        status.reports = status.reports.saturating_add(1);
        status.detail = format!(
            "BthPort ETW 已绑定 {} 台设备，累计 {} 条按键报告",
            shared
                .bindings
                .lock()
                .map(|bindings| bindings.len())
                .unwrap_or(0),
            status.reports
        );
        status.packets = shared.packets.load(AtomicOrdering::Relaxed);
        status.redacted = shared.redacted.load(AtomicOrdering::Relaxed);
        status.sensitive_data = shared.sensitive_data;
        status.setup_required = !shared.sensitive_data;
        status.last_event = shared
            .last_event
            .lock()
            .ok()
            .and_then(|event| event.clone());
    }
    let _ = shared.sender.send(RawFrame {
        device_path: format!("etw:{}", binding.device_id),
        bytes,
        vkey: None,
        make_code: None,
        is_down: None,
        captured_at_ms: now_ms(),
        source: InputSource::Etw,
    });
}

fn forward_atvv_start_search(shared: &Shared, att: &[u8], value: &[u8], binding: Option<&Binding>) {
    if att.len() < 3 {
        return;
    }
    let Some(watch) = shared
        .atvv_watch
        .lock()
        .ok()
        .and_then(|watch| watch.as_ref().cloned())
    else {
        return;
    };
    let handle = u16::from_le_bytes([att[1], att[2]]);
    if handle != watch.attribute_handle {
        return;
    }
    if binding.is_some_and(|binding| {
        !binding
            .device_id
            .eq_ignore_ascii_case(watch.device_id.as_str())
    }) {
        return;
    }
    if decode_control(value).ok().flatten() == Some(AtvvNotification::StartSearch) {
        let _ = watch.sender.send(());
    }
}

fn mark_incomplete_report(shared: &Shared) {
    let redacted = shared.redacted.fetch_add(1, AtomicOrdering::Relaxed) + 1;
    if let Ok(mut status) = shared.status.write() {
        status.redacted = redacted;
        status.state = if shared.sensitive_data {
            "restartRequired"
        } else {
            "needsSetup"
        }
        .to_string();
        status.setup_required = !shared.sensitive_data;
        status.detail = if shared.sensitive_data {
            "Windows 仍在省略蓝牙按键载荷。请关闭并重新打开蓝牙，或重启电脑后再试。".to_string()
        } else {
            "Windows 只返回了 ATT 头，按键载荷被系统省略；请启用按键增强".to_string()
        };
        status.last_event = shared
            .last_event
            .lock()
            .ok()
            .and_then(|event| event.clone());
        status.packets = shared.packets.load(AtomicOrdering::Relaxed);
    }
}

fn bind_from_hid_notification(shared: &Shared, connection: u16, att: &[u8]) -> Option<Binding> {
    if att.len() < 3 || u16::from_le_bytes([att[1], att[2]]) != 0x0029 {
        return None;
    }
    let value = notification_value(att)?;
    let fallback = shared.fallback.lock().ok()?.clone()?;
    parse_report(value, &fallback.model_type)?;
    {
        let mut bindings = shared.bindings.lock().ok()?;
        bindings.retain(|_, binding| binding.device_id != fallback.device_id);
        bindings.insert(connection, fallback.clone());
    }
    if let Ok(mut pending) = shared.pending.lock() {
        pending.remove(&connection);
    }
    if let Ok(mut probes) = shared.probes.lock() {
        probes.retain(|probe| probe.device_id != fallback.device_id);
    }
    update_bound_status(shared);
    Some(fallback)
}

fn handle_hci_event(shared: &Shared, packet: &[u8]) {
    if packet.len() < 6 {
        return;
    }
    if packet[0] != 0x05 || packet[1] != 0x04 {
        return;
    }
    let connection = u16::from_le_bytes([packet[3], packet[4]]) & 0x0FFF;
    if let Ok(mut pending) = shared.pending.lock() {
        pending.remove(&connection);
    }
    if let Ok(mut bindings) = shared.bindings.lock() {
        bindings.remove(&connection);
    }
    update_bound_status(shared);
}

fn update_bound_status(shared: &Shared) {
    let bound = shared
        .bindings
        .lock()
        .map(|bindings| bindings.len())
        .unwrap_or(0);
    if let Ok(mut status) = shared.status.write() {
        status.bound_devices = bound;
        status.packets = shared.packets.load(AtomicOrdering::Relaxed);
        status.redacted = shared.redacted.load(AtomicOrdering::Relaxed);
        status.sensitive_data = shared.sensitive_data;
        status.setup_required = !shared.sensitive_data;
        status.state = if !shared.sensitive_data {
            "needsSetup"
        } else if bound > 0 {
            "ready"
        } else {
            "degraded"
        }
        .to_string();
        status.detail = if !shared.sensitive_data {
            "ETW 已绑定设备，但 Windows 尚未提供按键载荷；请启用按键增强".to_string()
        } else if bound > 0 {
            format!("BthPort ETW 已绑定 {bound} 台设备，等待按键")
        } else {
            "BthPort ETW 已启动，等待设备绑定".to_string()
        };
        status.last_event = shared
            .last_event
            .lock()
            .ok()
            .and_then(|event| event.clone());
    }
}

fn notification_value(att: &[u8]) -> Option<&[u8]> {
    match att[0] {
        0x1B | 0x1D if att.len() >= 3 => Some(&att[3..]),
        _ => None,
    }
}

fn att_packet(packet: &[u8]) -> Option<(u16, &[u8], bool)> {
    if packet.len() < 8 {
        return None;
    }
    let header = u16::from_le_bytes([packet[0], packet[1]]);
    let connection = header & 0x0FFF;
    if connection > 0x0EFF || (header & 0xD000) != 0 {
        return None;
    }
    let acl_length = u16::from_le_bytes([packet[2], packet[3]]) as usize;
    let l2cap_length = u16::from_le_bytes([packet[4], packet[5]]) as usize;
    let cid = u16::from_le_bytes([packet[6], packet[7]]);
    if cid != 4 || acl_length != l2cap_length + 4 {
        return None;
    }
    let available = packet.len().saturating_sub(8);
    let incomplete = available < l2cap_length;
    let end = 8 + available.min(l2cap_length);
    Some((connection, &packet[8..end], incomplete))
}

fn property_size(record: &EVENT_RECORD, name: &[u16]) -> Option<usize> {
    let descriptor = PROPERTY_DATA_DESCRIPTOR {
        PropertyName: name.as_ptr() as u64,
        ArrayIndex: u32::MAX,
        Reserved: 0,
    };
    let mut size = 0u32;
    let status = unsafe {
        TdhGetPropertySize(
            record as *const EVENT_RECORD,
            None,
            &[descriptor],
            &mut size,
        )
    };
    (status == 0 && size != 0 && size <= 65_536).then_some(size as usize)
}

fn property_into(record: &EVENT_RECORD, name: &[u16], buffer: &mut [u8]) -> bool {
    let descriptor = PROPERTY_DATA_DESCRIPTOR {
        PropertyName: name.as_ptr() as u64,
        ArrayIndex: u32::MAX,
        Reserved: 0,
    };
    let status =
        unsafe { TdhGetProperty(record as *const EVENT_RECORD, None, &[descriptor], buffer) };
    status == 0
}

fn property(record: &EVENT_RECORD, name: &[u16]) -> Option<Vec<u8>> {
    let size = property_size(record, name)?;
    let mut buffer = vec![0u8; size];
    property_into(record, name, &mut buffer).then_some(buffer)
}

fn property_u8(record: &EVENT_RECORD, name: &[u16]) -> Option<u8> {
    let mut buffer = [0u8; 1];
    property_into(record, name, &mut buffer).then_some(buffer[0])
}

fn property_u32(record: &EVENT_RECORD, name: &[u16]) -> Option<u32> {
    let mut buffer = [0u8; 4];
    property_into(record, name, &mut buffer).then_some(u32::from_le_bytes(buffer))
}

fn marker_bytes(address: u64) -> [u8; 16] {
    let mut marker = [0u8; 16];
    let seed = address ^ (now_ms() << 17) ^ (std::process::id() as u64).rotate_left(31);
    let mut value = seed | 1;
    for chunk in marker.chunks_mut(8) {
        value ^= value << 13;
        value ^= value >> 7;
        value ^= value << 17;
        chunk.copy_from_slice(&value.to_le_bytes());
    }
    marker[0] |= 0x80;
    marker
}

pub fn guid_from_marker(marker: [u8; 16]) -> GUID {
    let mut rfc = marker;
    rfc.reverse();
    GUID::from_u128(u128::from_be_bytes(rfc))
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_guid_round_trips_to_the_att_byte_order() {
        let marker = [
            0x80, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ];
        let guid = guid_from_marker(marker);
        assert_eq!(guid.to_u128().to_le_bytes(), marker);
    }

    #[test]
    fn decodes_the_captured_bthport_acl_packet() {
        let packet = [
            0x10, 0x20, 0x0F, 0x00, 0x0B, 0x00, 0x04, 0x00, 0x1B, 0x29, 0x00, 0x03, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00,
        ];
        let (connection, att, incomplete) = att_packet(&packet).expect("sample packet");
        assert_eq!(connection, 0x0010);
        assert!(!incomplete);
        assert_eq!(
            att,
            &[
                0x1B, 0x29, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00
            ]
        );
        assert_eq!(
            notification_value(att),
            Some(&[0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00][..])
        );
    }

    #[test]
    fn marks_truncated_acl_packets_as_incomplete() {
        let packet = [
            0x10, 0x20, 0x0F, 0x00, 0x0B, 0x00, 0x04, 0x00, 0x1B, 0x29, 0x00,
        ];
        let (connection, att, incomplete) = att_packet(&packet).expect("truncated packet");
        assert_eq!(connection, 0x0010);
        assert_eq!(att, &[0x1B, 0x29, 0x00]);
        assert!(incomplete);
    }

    #[test]
    fn binds_the_armed_device_from_a_real_hid_notification() {
        let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let shared = Shared {
            sender,
            status: Arc::new(RwLock::new(EtwStatus::default())),
            probes: Mutex::new(Vec::new()),
            pending: Mutex::new(HashMap::new()),
            bindings: Mutex::new(HashMap::new()),
            fallback: Mutex::new(Some(Binding {
                device_id: "112233445566".to_string(),
                model_type: "GoogleG9N9N".to_string(),
            })),
            atvv_watch: Mutex::new(None),
            packets: AtomicU64::new(0),
            redacted: AtomicU64::new(0),
            sensitive_data: true,
            last_event: Mutex::new(None),
        };
        let notification = [
            0x1B, 0x29, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        let binding = bind_from_hid_notification(&shared, 0x0E02, &notification)
            .expect("HID notification should bind the armed device");

        assert_eq!(binding.device_id, "112233445566");
        assert_eq!(
            shared
                .bindings
                .lock()
                .unwrap()
                .get(&0x0E02)
                .unwrap()
                .device_id,
            "112233445566"
        );
        assert_eq!(shared.status.read().unwrap().bound_devices, 1);
    }

    #[test]
    fn does_not_bind_from_a_non_hid_notification() {
        let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let shared = Shared {
            sender,
            status: Arc::new(RwLock::new(EtwStatus::default())),
            probes: Mutex::new(Vec::new()),
            pending: Mutex::new(HashMap::new()),
            bindings: Mutex::new(HashMap::new()),
            fallback: Mutex::new(Some(Binding {
                device_id: "112233445566".to_string(),
                model_type: "GoogleG9N9N".to_string(),
            })),
            atvv_watch: Mutex::new(None),
            packets: AtomicU64::new(0),
            redacted: AtomicU64::new(0),
            sensitive_data: true,
            last_event: Mutex::new(None),
        };
        let audio_notification = [0x1B, 0x3F, 0x00, 0x03, 0x00, 0x00, 0x00, 0x00];

        assert!(bind_from_hid_notification(&shared, 0x0E02, &audio_notification).is_none());
    }

    #[test]
    fn reports_a_pending_driver_reload_when_the_payload_is_truncated() {
        let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let shared = Shared {
            sender,
            status: Arc::new(RwLock::new(EtwStatus::default())),
            probes: Mutex::new(Vec::new()),
            pending: Mutex::new(HashMap::new()),
            bindings: Mutex::new(HashMap::new()),
            fallback: Mutex::new(Some(Binding {
                device_id: "112233445566".to_string(),
                model_type: "GoogleG9N9N".to_string(),
            })),
            atvv_watch: Mutex::new(None),
            packets: AtomicU64::new(0),
            redacted: AtomicU64::new(0),
            sensitive_data: true,
            last_event: Mutex::new(None),
        };
        let packet = [
            0x10, 0x20, 0x0F, 0x00, 0x0B, 0x00, 0x04, 0x00, 0x1B, 0x29, 0x00,
        ];

        handle_received(&shared, &packet);

        let status = shared.status.read().unwrap();
        assert_eq!(status.state, "restartRequired");
        assert_eq!(status.redacted, 1);
        assert!(!status.setup_required);
    }

    #[test]
    fn forwards_start_search_only_from_the_watched_control_handle() {
        let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let (voice_sender, mut voice_receiver) = tokio::sync::mpsc::unbounded_channel();
        let shared = Shared {
            sender,
            status: Arc::new(RwLock::new(EtwStatus::default())),
            probes: Mutex::new(Vec::new()),
            pending: Mutex::new(HashMap::new()),
            bindings: Mutex::new(HashMap::new()),
            fallback: Mutex::new(None),
            atvv_watch: Mutex::new(Some(AtvvWatch {
                device_id: "112233445566".to_string(),
                attribute_handle: 0x003F,
                sender: voice_sender,
            })),
            packets: AtomicU64::new(0),
            redacted: AtomicU64::new(0),
            sensitive_data: true,
            last_event: Mutex::new(None),
        };
        let start_search = [0x1B, 0x3F, 0x00, 0x08];

        forward_atvv_start_search(
            &shared,
            &start_search,
            notification_value(&start_search).unwrap(),
            None,
        );
        assert_eq!(voice_receiver.try_recv(), Ok(()));

        let other_handle = [0x1B, 0x40, 0x00, 0x08];
        forward_atvv_start_search(
            &shared,
            &other_handle,
            notification_value(&other_handle).unwrap(),
            None,
        );
        assert!(voice_receiver.try_recv().is_err());
    }

    #[test]
    fn dropping_one_listener_clone_keeps_the_shared_session_running() {
        let (sender, _receiver) = tokio::sync::mpsc::unbounded_channel();
        let stopped = Arc::new(AtomicBool::new(false));
        let listener = EtwInputListener {
            shared: Arc::new(Shared {
                sender,
                status: Arc::new(RwLock::new(EtwStatus::default())),
                probes: Mutex::new(Vec::new()),
                pending: Mutex::new(HashMap::new()),
                bindings: Mutex::new(HashMap::new()),
                fallback: Mutex::new(None),
                atvv_watch: Mutex::new(None),
                packets: AtomicU64::new(0),
                redacted: AtomicU64::new(0),
                sensitive_data: true,
                last_event: Mutex::new(None),
            }),
            session: Arc::new(Mutex::new(None)),
            stopped: stopped.clone(),
            refs: Arc::new(AtomicUsize::new(1)),
        };

        drop(listener.clone());
        assert!(!stopped.load(Ordering::SeqCst));

        drop(listener);
        assert!(stopped.load(Ordering::SeqCst));
    }
}
