use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::time::{Duration, Instant};

use windows::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows::Win32::Devices::HumanInterfaceDevice::{
    HIDD_ATTRIBUTES, HIDP_CAPS, HidD_FreePreparsedData, HidD_GetAttributes, HidD_GetPreparsedData,
    HidD_GetProductString, HidP_GetCaps, PHIDP_PREPARSED_DATA,
};
use windows::Win32::Foundation::{CloseHandle, GENERIC_READ};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, ReadFile,
};
use windows::core::PCWSTR;

fn main() {
    let paths = hid_interface_paths().expect("enumerate HID interfaces");
    println!("HID interfaces: {}", paths.len());
    for path in paths {
        let lower = path.to_ascii_lowercase();
        if !lower.contains("vid_18d1") && !lower.contains("vid&0118d1") {
            continue;
        }
        println!("candidate: {path}");
        probe(&path);
    }
}

fn probe(path: &str) {
    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    for access in [GENERIC_READ] {
        let handle = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                access.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_ATTRIBUTE_NORMAL,
                None,
            )
        };
        let handle = match handle {
            Ok(handle) => handle,
            Err(error) => {
                println!("  open 0x{:08X} failed: {error}", access.0);
                continue;
            }
        };

        let mut attributes = HIDD_ATTRIBUTES {
            Size: std::mem::size_of::<HIDD_ATTRIBUTES>() as u32,
            ..Default::default()
        };
        let attributes_ok = unsafe { HidD_GetAttributes(handle, &mut attributes) };
        let mut product = [0u16; 256];
        let product_ok = unsafe { HidD_GetProductString(handle, product.as_mut_ptr().cast(), 512) };
        let product = String::from_utf16_lossy(
            &product[..product.iter().position(|value| *value == 0).unwrap_or(0)],
        );
        println!(
            "  open 0x{:08X}: attrs={attributes_ok} vid=0x{:04X} pid=0x{:04X} product={product:?} productOk={product_ok}",
            access.0, attributes.VendorID, attributes.ProductID
        );

        let mut preparsed = PHIDP_PREPARSED_DATA::default();
        if !unsafe { HidD_GetPreparsedData(handle, &mut preparsed) } {
            println!("  preparsed failed: {}", windows::core::Error::from_win32());
        } else {
            let mut caps = HIDP_CAPS::default();
            let status = unsafe { HidP_GetCaps(preparsed, &mut caps) };
            println!(
                "  caps status=0x{:08X} page=0x{:04X} usage=0x{:04X} input={} output={} feature={}",
                status.0,
                caps.UsagePage,
                caps.Usage,
                caps.InputReportByteLength,
                caps.OutputReportByteLength,
                caps.FeatureReportByteLength
            );
            let _ = unsafe { HidD_FreePreparsedData(preparsed) };
        }

        if access == GENERIC_READ {
            println!("  READY: press remote keys for 20 seconds");
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut buffer = [0u8; 64];
            while Instant::now() < deadline {
                let mut read = 0u32;
                let result =
                    unsafe { ReadFile(handle, Some(&mut buffer[..]), Some(&mut read), None) };
                match result {
                    Ok(()) => println!(
                        "  report: {}",
                        buffer[..read as usize]
                            .iter()
                            .map(|byte| format!("{byte:02X}"))
                            .collect::<Vec<_>>()
                            .join(" ")
                    ),
                    Err(error) => {
                        println!("  read failed: {error}");
                        std::thread::sleep(Duration::from_millis(300));
                    }
                }
            }
        }
        let _ = unsafe { CloseHandle(handle) };
        break;
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
