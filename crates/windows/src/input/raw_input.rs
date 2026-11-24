use super::{InputError, InputSource};
use std::collections::HashMap;
use std::ffi::c_void;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
use windows::Win32::Foundation::{HANDLE, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, RAWHID, RAWINPUTDEVICE, RAWINPUTHEADER, RAWKEYBOARD,
    RID_INPUT, RIDEV_DEVNOTIFY, RIDEV_INPUTSINK, RIDEV_REMOVE, RIDI_DEVICENAME, RIM_TYPEHID,
    RIM_TYPEKEYBOARD, RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetMessageW,
    GetWindowLongPtrW, MSG, PostMessageW, RegisterClassW, SetWindowLongPtrW, TranslateMessage,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_INPUT, WNDCLASS_STYLES, WNDCLASSW,
};
use windows::core::{PCWSTR, w};

const WINDOW_CLASS: PCWSTR = w!("RemoteBuddy.RawInput");

#[derive(Debug, Clone)]
pub struct RawFrame {
    pub device_path: String,
    pub bytes: Vec<u8>,
    pub vkey: Option<u16>,
    pub make_code: Option<u16>,
    pub is_down: Option<bool>,
    pub captured_at_ms: u64,
    pub source: InputSource,
}

pub struct RawInputListener {
    hwnd: isize,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for RawInputListener {
    fn drop(&mut self) {
        if self.hwnd != 0 {
            let _ = unsafe {
                PostMessageW(
                    Some(HWND(self.hwnd as *mut c_void)),
                    WM_CLOSE,
                    WPARAM(0),
                    LPARAM(0),
                )
            };
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

pub fn spawn_raw_input_listener(
    sender: UnboundedSender<RawFrame>,
) -> Result<RawInputListener, InputError> {
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let thread = std::thread::Builder::new()
        .name("remote-buddy-raw-input".to_string())
        .spawn(move || run_listener(sender, ready_tx))
        .map_err(|_| InputError::ThreadStart)?;

    match ready_rx.recv() {
        Ok(Ok(hwnd)) => Ok(RawInputListener {
            hwnd,
            thread: Some(thread),
        }),
        Ok(Err(error)) => {
            let _ = thread.join();
            Err(InputError::Windows(windows::core::Error::new(
                windows::core::HRESULT(0x80004005u32 as i32),
                error,
            )))
        }
        Err(_) => {
            let _ = thread.join();
            Err(InputError::ThreadStart)
        }
    }
}

fn run_listener(
    sender: UnboundedSender<RawFrame>,
    ready: std::sync::mpsc::SyncSender<Result<isize, String>>,
) {
    unsafe {
        let instance = match GetModuleHandleW(None) {
            Ok(module) => HINSTANCE(module.0),
            Err(error) => {
                let _ = ready.send(Err(error.to_string()));
                return;
            }
        };
        let class = WNDCLASSW {
            style: WNDCLASS_STYLES(0),
            lpfnWndProc: Some(raw_window_proc),
            hInstance: instance,
            lpszClassName: WINDOW_CLASS,
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            let _ = ready.send(Err(windows::core::Error::from_win32().to_string()));
            return;
        }

        let context = Box::into_raw(Box::new(RawInputContext {
            sender,
            paths: HashMap::new(),
        }));
        let window = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            WINDOW_CLASS,
            w!("Remote Buddy Input"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            Some(context.cast()),
        ) {
            Ok(window) => window,
            Err(error) => {
                drop(Box::from_raw(context));
                let _ = ready.send(Err(error.to_string()));
                return;
            }
        };
        SetWindowLongPtrW(window, GWLP_USERDATA, context as isize);

        let devices = [
            RAWINPUTDEVICE {
                usUsagePage: 0x0C,
                usUsage: 0x01,
                dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                hwndTarget: window,
            },
            RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x06,
                dwFlags: RIDEV_INPUTSINK | RIDEV_DEVNOTIFY,
                hwndTarget: window,
            },
        ];
        if let Err(error) =
            RegisterRawInputDevices(&devices, std::mem::size_of::<RAWINPUTDEVICE>() as u32)
        {
            DestroyWindow(window).ok();
            drop(Box::from_raw(context));
            let _ = ready.send(Err(error.to_string()));
            return;
        }
        if ready.send(Ok(window.0 as isize)).is_err() {
            DestroyWindow(window).ok();
            drop(Box::from_raw(context));
            return;
        }

        let mut message = MSG::default();
        loop {
            let result = GetMessageW(&mut message, None, 0, 0);
            if result.0 == 0 || result.0 == -1 {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }

        let removal = [
            RAWINPUTDEVICE {
                usUsagePage: 0x0C,
                usUsage: 0x01,
                dwFlags: RIDEV_REMOVE,
                hwndTarget: HWND::default(),
            },
            RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x06,
                dwFlags: RIDEV_REMOVE,
                hwndTarget: HWND::default(),
            },
        ];
        let _ = RegisterRawInputDevices(&removal, std::mem::size_of::<RAWINPUTDEVICE>() as u32);
        drop(Box::from_raw(context));
    }
}

struct RawInputContext {
    sender: UnboundedSender<RawFrame>,
    paths: HashMap<isize, String>,
}

impl RawInputContext {
    #[allow(unsafe_op_in_unsafe_fn)]
    unsafe fn process(&mut self, lparam: LPARAM) {
        let header_size = std::mem::size_of::<RAWINPUTHEADER>() as u32;
        let mut size = 0_u32;
        let probe = GetRawInputData(
            windows::Win32::UI::Input::HRAWINPUT(lparam.0 as *mut c_void),
            RID_INPUT,
            None,
            &mut size,
            header_size,
        );
        if probe == u32::MAX || size == 0 {
            return;
        }

        let mut buffer = vec![0_u8; size as usize];
        let read = GetRawInputData(
            windows::Win32::UI::Input::HRAWINPUT(lparam.0 as *mut c_void),
            RID_INPUT,
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
            header_size,
        );
        if read == u32::MAX || read > buffer.len() as u32 {
            return;
        }

        let header = std::ptr::read_unaligned(buffer.as_ptr().cast::<RAWINPUTHEADER>());
        if header.hDevice.is_invalid() || header.hDevice.0.is_null() {
            return;
        }
        let handle_key = header.hDevice.0 as isize;
        let path = if let Some(path) = self.paths.get(&handle_key) {
            path.clone()
        } else {
            let Some(path) = device_path(header.hDevice) else {
                return;
            };
            if !is_supported_remote_path(&path) {
                return;
            }
            self.paths.insert(handle_key, path.clone());
            path
        };
        if !is_supported_remote_path(&path) {
            return;
        }

        if header.dwType == RIM_TYPEHID.0 {
            let hid = std::ptr::read_unaligned(
                buffer
                    .as_ptr()
                    .add(std::mem::size_of::<RAWINPUTHEADER>())
                    .cast::<RAWHID>(),
            );
            if hid.dwSizeHid == 0 || hid.dwCount == 0 || hid.dwSizeHid > 64 || hid.dwCount > 64 {
                return;
            }
            let report_size = hid.dwSizeHid as usize;
            let total = report_size.saturating_mul(hid.dwCount as usize);
            let data_start =
                std::mem::size_of::<RAWINPUTHEADER>() + std::mem::offset_of!(RAWHID, bRawData);
            if data_start + total > buffer.len() {
                return;
            }
            let captured_at_ms = now_ms();
            for index in 0..hid.dwCount as usize {
                let start = data_start + index * report_size;
                let bytes = buffer[start..start + report_size].to_vec();
                let _ = self.sender.send(RawFrame {
                    device_path: path.clone(),
                    bytes,
                    vkey: None,
                    make_code: None,
                    is_down: None,
                    captured_at_ms,
                    source: InputSource::RawInput,
                });
            }
        } else if header.dwType == RIM_TYPEKEYBOARD.0 {
            let keyboard = std::ptr::read_unaligned(
                buffer
                    .as_ptr()
                    .add(std::mem::size_of::<RAWINPUTHEADER>())
                    .cast::<RAWKEYBOARD>(),
            );
            if keyboard.VKey == 0 {
                return;
            }
            let _ = self.sender.send(RawFrame {
                device_path: path,
                bytes: Vec::new(),
                vkey: Some(keyboard.VKey),
                make_code: Some(keyboard.MakeCode),
                is_down: Some(keyboard.Flags & 1 == 0),
                captured_at_ms: now_ms(),
                source: InputSource::RawInput,
            });
        }
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe extern "system" fn raw_window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        WM_INPUT => {
            let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut RawInputContext;
            if !pointer.is_null() {
                unsafe { (*pointer).process(lparam) };
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = unsafe { DestroyWindow(hwnd) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
}

#[allow(unsafe_op_in_unsafe_fn)]
unsafe fn device_path(device: HANDLE) -> Option<String> {
    let mut length = 0_u32;
    GetRawInputDeviceInfoW(Some(device), RIDI_DEVICENAME, None, &mut length);
    if length == 0 || length > 4096 {
        return None;
    }
    let mut buffer = vec![0_u16; length as usize + 1];
    let result = GetRawInputDeviceInfoW(
        Some(device),
        RIDI_DEVICENAME,
        Some(buffer.as_mut_ptr().cast()),
        &mut length,
    );
    if result == u32::MAX || result == 0 {
        return None;
    }
    let end = buffer
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..end]))
}

pub fn is_supported_remote_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.contains("vid&0118d1")
        || lower.contains("vid&0218d1")
        || lower.contains("vid_18d1")
        || lower.contains("pid&9450")
        || lower.contains("pid_9450")
        || lower.contains("vid&022717")
        || lower.contains("vid_2717")
        || lower.contains("t1-remote")
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}
