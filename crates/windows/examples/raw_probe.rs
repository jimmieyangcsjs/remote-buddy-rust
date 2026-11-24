use std::ffi::c_void;
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::{
    GetRawInputData, GetRawInputDeviceInfoW, GetRawInputDeviceList, RAWHID, RAWINPUTDEVICE,
    RAWINPUTDEVICELIST, RAWINPUTHEADER, RAWKEYBOARD, RID_INPUT, RIDEV_INPUTSINK, RIDI_DEVICENAME,
    RIM_TYPEHID, RIM_TYPEKEYBOARD, RegisterRawInputDevices,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, MSG, RegisterClassW,
    TranslateMessage, WINDOW_EX_STYLE, WINDOW_STYLE, WM_INPUT, WNDCLASSW,
};
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("RemoteBuddy.RawProbe");

fn main() {
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None).unwrap().0);
        let class = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: CLASS,
            ..Default::default()
        };
        RegisterClassW(&class);
        let window = CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            w!("probe"),
            WINDOW_STYLE(0),
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance),
            None,
        )
        .unwrap();
        let devices = [
            RAWINPUTDEVICE {
                usUsagePage: 0x0C,
                usUsage: 0x01,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: window,
            },
            RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x06,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: window,
            },
        ];
        RegisterRawInputDevices(&devices, std::mem::size_of::<RAWINPUTDEVICE>() as u32).unwrap();
        println!("READY: press remote keys for 60 seconds");

        let mut count = 0u32;
        let list_result = GetRawInputDeviceList(
            None,
            &mut count,
            std::mem::size_of::<RAWINPUTDEVICELIST>() as u32,
        );
        println!("device list probe: {list_result} count={count}");
        if count > 0 {
            let mut list = vec![RAWINPUTDEVICELIST::default(); count as usize];
            let actual = GetRawInputDeviceList(
                Some(list.as_mut_ptr()),
                &mut count,
                std::mem::size_of::<RAWINPUTDEVICELIST>() as u32,
            );
            println!("device list: {actual}");
            for device in list.iter().take(actual as usize) {
                let mut length = 0u32;
                GetRawInputDeviceInfoW(Some(device.hDevice), RIDI_DEVICENAME, None, &mut length);
                let mut name = vec![0u16; length as usize + 1];
                let copied = GetRawInputDeviceInfoW(
                    Some(device.hDevice),
                    RIDI_DEVICENAME,
                    Some(name.as_mut_ptr().cast()),
                    &mut length,
                );
                if copied != u32::MAX {
                    println!(
                        "  device {:?}: {}",
                        device.dwType,
                        String::from_utf16_lossy(
                            &name[..name.iter().position(|v| *v == 0).unwrap_or(0)]
                        )
                    );
                }
            }
        }

        let mut message = MSG::default();
        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
        let _ = window;
    }
}

unsafe extern "system" fn proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message != WM_INPUT {
        return unsafe { DefWindowProcW(hwnd, message, wparam, lparam) };
    }
    let header_size = std::mem::size_of::<RAWINPUTHEADER>() as u32;
    let mut size = 0u32;
    let probe = unsafe {
        GetRawInputData(
            windows::Win32::UI::Input::HRAWINPUT(lparam.0 as *mut c_void),
            RID_INPUT,
            None,
            &mut size,
            header_size,
        )
    };
    if probe == u32::MAX || size == 0 {
        return LRESULT(0);
    }
    let mut buffer = vec![0u8; size as usize];
    let read = unsafe {
        GetRawInputData(
            windows::Win32::UI::Input::HRAWINPUT(lparam.0 as *mut c_void),
            RID_INPUT,
            Some(buffer.as_mut_ptr().cast()),
            &mut size,
            header_size,
        )
    };
    if read == u32::MAX {
        return LRESULT(0);
    }
    let header = unsafe { std::ptr::read_unaligned(buffer.as_ptr().cast::<RAWINPUTHEADER>()) };
    let mut length = 0u32;
    unsafe { GetRawInputDeviceInfoW(Some(header.hDevice), RIDI_DEVICENAME, None, &mut length) };
    let mut name = vec![0u16; length as usize + 1];
    unsafe {
        GetRawInputDeviceInfoW(
            Some(header.hDevice),
            RIDI_DEVICENAME,
            Some(name.as_mut_ptr().cast()),
            &mut length,
        )
    };
    let path =
        String::from_utf16_lossy(&name[..name.iter().position(|value| *value == 0).unwrap_or(0)]);
    if header.dwType == RIM_TYPEHID.0 {
        let hid = unsafe {
            std::ptr::read_unaligned(
                buffer
                    .as_ptr()
                    .add(std::mem::size_of::<RAWINPUTHEADER>())
                    .cast::<RAWHID>(),
            )
        };
        let start = std::mem::size_of::<RAWINPUTHEADER>() + std::mem::offset_of!(RAWHID, bRawData);
        let total = hid.dwSizeHid as usize * hid.dwCount as usize;
        if start + total <= buffer.len() {
            println!(
                "HID path={path} size={} count={} data={}",
                hid.dwSizeHid,
                hid.dwCount,
                buffer[start..start + total]
                    .iter()
                    .map(|byte| format!("{byte:02X}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
    } else if header.dwType == RIM_TYPEKEYBOARD.0 {
        let keyboard = unsafe {
            std::ptr::read_unaligned(
                buffer
                    .as_ptr()
                    .add(std::mem::size_of::<RAWINPUTHEADER>())
                    .cast::<RAWKEYBOARD>(),
            )
        };
        println!(
            "KEY path={path} vkey=0x{:02X} make=0x{:02X} flags=0x{:04X}",
            keyboard.VKey, keyboard.MakeCode, keyboard.Flags
        );
    } else if header.dwType == 0 {
        let mouse = unsafe {
            std::ptr::read_unaligned(
                buffer
                    .as_ptr()
                    .add(std::mem::size_of::<RAWINPUTHEADER>())
                    .cast::<windows::Win32::UI::Input::RAWMOUSE>(),
            )
        };
        println!("MOUSE path={path} flags=0x{:04X}", mouse.usFlags.0);
    }
    LRESULT(0)
}
