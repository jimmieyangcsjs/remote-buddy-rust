use super::raw_input::{RawFrame, is_supported_remote_path};
use super::{InputError, InputSource};
use std::collections::{HashMap, HashSet};
use std::ffi::{OsStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HIDP_CAPS, HidD_FreePreparsedData, HidD_GetPreparsedData, HidP_GetCaps, PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_IO_PENDING, GENERIC_READ, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_FLAG_OVERLAPPED, FILE_SHARE_READ, FILE_SHARE_WRITE,
    OPEN_EXISTING, ReadFile,
};
use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForSingleObject};
use windows::core::PCWSTR;

const CONSUMER_PAGE: u16 = 0x0C;
const CONSUMER_CONTROL: u16 = 0x01;
const HIDP_STATUS_SUCCESS: i32 = 0x0011_0000;
const SCAN_INTERVAL: Duration = Duration::from_secs(2);
const WAIT_SLICE_MS: u32 = 250;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HidReportLogLevel {
    Info,
    Warn,
}

type LogSink = Arc<dyn Fn(HidReportLogLevel, String) + Send + Sync>;

pub struct HidReportListener {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Drop for HidReportListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn spawn_hid_report_listener(
    sender: UnboundedSender<RawFrame>,
    log: impl Fn(HidReportLogLevel, String) + Send + Sync + 'static,
) -> Result<HidReportListener, InputError> {
    let log: LogSink = Arc::new(log);
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = stop.clone();
    let thread = thread::Builder::new()
        .name("remote-buddy-hid-report".to_string())
        .spawn(move || scan_loop(sender, thread_stop, log))
        .map_err(|_| InputError::ThreadStart)?;
    Ok(HidReportListener {
        stop,
        thread: Some(thread),
    })
}

fn scan_loop(sender: UnboundedSender<RawFrame>, stop: Arc<AtomicBool>, log: LogSink) {
    let mut readers: HashMap<String, JoinHandle<()>> = HashMap::new();
    let mut inspected: HashSet<String> = HashSet::new();
    while !stop.load(Ordering::SeqCst) {
        match hid_interface_paths() {
            Ok(paths) => {
                readers.retain(|_, reader| !reader.is_finished());
                for path in paths {
                    if !is_supported_remote_path(&path) {
                        continue;
                    }
                    if readers.contains_key(&path) {
                        continue;
                    }
                    if inspected.insert(path.clone()) {
                        log(
                            HidReportLogLevel::Info,
                            format!("发现遥控器 HID 接口：{path}"),
                        );
                    }
                    let Some(reader) =
                        start_reader(&path, sender.clone(), stop.clone(), log.clone())
                    else {
                        continue;
                    };
                    readers.insert(path, reader);
                }
            }
            Err(error) => log(
                HidReportLogLevel::Warn,
                format!("枚举 HID 接口失败：{error}"),
            ),
        }

        let scan_until = SystemTime::now() + SCAN_INTERVAL;
        while !stop.load(Ordering::SeqCst) && SystemTime::now() < scan_until {
            thread::sleep(Duration::from_millis(100));
        }
    }
    for (_, reader) in readers {
        let _ = reader.join();
    }
}

fn start_reader(
    path: &str,
    sender: UnboundedSender<RawFrame>,
    stop: Arc<AtomicBool>,
    log: LogSink,
) -> Option<JoinHandle<()>> {
    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_READ.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL | FILE_FLAG_OVERLAPPED,
            None,
        )
    };
    let handle = match handle {
        Ok(handle) => handle,
        Err(error) => {
            log(
                HidReportLogLevel::Warn,
                format!("打开 HID 接口失败：{path} · {error}"),
            );
            return None;
        }
    };

    let caps = match read_caps(handle) {
        Some(caps) => caps,
        None => {
            log(
                HidReportLogLevel::Warn,
                format!("读取 HID 能力失败：{path}"),
            );
            unsafe {
                let _ = CloseHandle(handle);
            }
            return None;
        }
    };
    if caps.UsagePage != CONSUMER_PAGE
        || caps.Usage != CONSUMER_CONTROL
        || caps.InputReportByteLength == 0
    {
        log(
            HidReportLogLevel::Info,
            format!(
                "跳过非 Consumer Control HID 接口：{path} · usagePage=0x{:04X} usage=0x{:04X} report={}",
                caps.UsagePage, caps.Usage, caps.InputReportByteLength
            ),
        );
        unsafe {
            let _ = CloseHandle(handle);
        }
        return None;
    }

    let path = path.to_string();
    let handle_value = handle.0 as usize;
    thread::Builder::new()
        .name(format!(
            "remote-buddy-hid-{}",
            path.replace(|character: char| !character.is_ascii_alphanumeric(), "")
        ))
        .spawn(move || {
            log(
                HidReportLogLevel::Info,
                format!(
                    "HID Consumer Control 已打开：{path} · 输入报告 {} 字节",
                    caps.InputReportByteLength
                ),
            );
            read_loop(
                path,
                HANDLE(handle_value as *mut c_void),
                caps.InputReportByteLength,
                sender,
                stop,
                log,
            )
        })
        .ok()
}

fn read_caps(handle: HANDLE) -> Option<HIDP_CAPS> {
    let mut preparsed = PHIDP_PREPARSED_DATA::default();
    if !unsafe { HidD_GetPreparsedData(handle, &mut preparsed) } {
        return None;
    }
    let mut caps = HIDP_CAPS::default();
    let status = unsafe { HidP_GetCaps(preparsed, &mut caps) };
    let _ = unsafe { HidD_FreePreparsedData(preparsed) };
    (status.0 == HIDP_STATUS_SUCCESS).then_some(caps)
}

fn read_loop(
    path: String,
    handle: HANDLE,
    report_length: u16,
    sender: UnboundedSender<RawFrame>,
    stop: Arc<AtomicBool>,
    log: LogSink,
) {
    let event = match unsafe { CreateEventW(None, true, false, None) } {
        Ok(event) => event,
        Err(error) => {
            log(
                HidReportLogLevel::Warn,
                format!("创建 HID 读取事件失败：{error}"),
            );
            unsafe {
                let _ = CloseHandle(handle);
            }
            return;
        }
    };
    let mut buffer = vec![0u8; usize::from(report_length)];
    let mut overlapped = OVERLAPPED {
        hEvent: event,
        ..Default::default()
    };
    let mut frames = 0_u32;

    loop {
        if stop.load(Ordering::SeqCst) {
            unsafe {
                let _ = CancelIoEx(handle, Some(&overlapped));
                let _ = WaitForSingleObject(event, 1_000);
            }
            break;
        }
        let _ = unsafe { ResetEvent(event) };
        let read = unsafe { ReadFile(handle, Some(&mut buffer[..]), None, Some(&mut overlapped)) };
        let pending = match read {
            Ok(()) => false,
            Err(error)
                if error.code() == windows::core::HRESULT::from_win32(ERROR_IO_PENDING.0) =>
            {
                true
            }
            Err(error) => {
                log(
                    HidReportLogLevel::Warn,
                    format!("读取 HID 报告失败：{path} · {error}"),
                );
                break;
            }
        };
        if pending {
            let mut completed = false;
            loop {
                if stop.load(Ordering::SeqCst) {
                    unsafe {
                        let _ = CancelIoEx(handle, Some(&overlapped));
                        let _ = WaitForSingleObject(event, 1_000);
                    }
                    break;
                }
                match unsafe { WaitForSingleObject(event, WAIT_SLICE_MS) } {
                    WAIT_OBJECT_0 => {
                        completed = true;
                        break;
                    }
                    WAIT_TIMEOUT => continue,
                    _ => break,
                }
            }
            if stop.load(Ordering::SeqCst) || !completed {
                break;
            }
        }

        let mut bytes_read = 0u32;
        if let Err(error) =
            unsafe { GetOverlappedResult(handle, &overlapped, &mut bytes_read, false) }
        {
            log(
                HidReportLogLevel::Warn,
                format!("读取 HID 报告完成状态失败：{path} · {error}"),
            );
            break;
        }
        let length = usize::try_from(bytes_read).unwrap_or(0);
        if length == 0 {
            continue;
        }
        let length = length.min(buffer.len());
        frames = frames.saturating_add(1);
        if frames <= 20 {
            log(
                HidReportLogLevel::Info,
                format!(
                    "HID 首帧 {}：{}",
                    frames,
                    buffer[..length]
                        .iter()
                        .map(|byte| format!("{byte:02X}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                ),
            );
        }
        let _ = sender.send(RawFrame {
            device_path: path.clone(),
            bytes: buffer[..length].to_vec(),
            vkey: None,
            make_code: None,
            is_down: None,
            captured_at_ms: now_ms(),
            source: InputSource::Hid,
        });
    }

    unsafe {
        let _ = CloseHandle(event);
        let _ = CloseHandle(handle);
    }
}

fn hid_interface_paths() -> windows::core::Result<Vec<String>> {
    let guid = unsafe { windows::Win32::Devices::HumanInterfaceDevice::HidD_GetHidGuid() };
    let info: HDEVINFO = unsafe {
        SetupDiGetClassDevsW(
            Some(&guid),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )?
    };
    let mut paths = Vec::new();
    let mut index = 0u32;
    loop {
        let mut data = SP_DEVICE_INTERFACE_DATA {
            cbSize: std::mem::size_of::<SP_DEVICE_INTERFACE_DATA>() as u32,
            ..Default::default()
        };
        if unsafe { SetupDiEnumDeviceInterfaces(info, None, &guid, index, &mut data) }.is_err() {
            break;
        }
        index += 1;
        let mut required = 0u32;
        let _ = unsafe {
            SetupDiGetDeviceInterfaceDetailW(info, &data, None, 0, Some(&mut required), None)
        };
        if required < std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32 {
            continue;
        }
        let mut buffer = vec![0u8; required as usize];
        let detail = buffer
            .as_mut_ptr()
            .cast::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>();
        unsafe {
            (*detail).cbSize = std::mem::size_of::<SP_DEVICE_INTERFACE_DETAIL_DATA_W>() as u32;
        }
        if unsafe {
            SetupDiGetDeviceInterfaceDetailW(
                info,
                &data,
                Some(detail),
                required,
                Some(&mut required),
                None,
            )
        }
        .is_err()
        {
            continue;
        }
        let path_ptr = unsafe { (*detail).DevicePath.as_ptr() };
        let length = (0..)
            .take_while(|offset| unsafe { *path_ptr.add(*offset) } != 0)
            .count();
        paths.push(String::from_utf16_lossy(unsafe {
            std::slice::from_raw_parts(path_ptr, length)
        }));
    }
    unsafe {
        let _ = SetupDiDestroyDeviceInfoList(info);
    }
    Ok(paths)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}
