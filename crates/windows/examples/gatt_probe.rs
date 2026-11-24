use std::ffi::{OsStr, c_void};
use std::os::windows::ffi::OsStrExt;
use std::sync::mpsc;
use std::time::Duration;
use windows::Win32::Devices::Bluetooth::{
    BLUETOOTH_GATT_FLAG_NONE, BLUETOOTH_GATT_VALUE_CHANGED_EVENT,
    BLUETOOTH_GATT_VALUE_CHANGED_EVENT_REGISTRATION, BTH_LE_GATT_CHARACTERISTIC,
    BTH_LE_GATT_EVENT_TYPE, BTH_LE_GATT_SERVICE, BluetoothGATTGetCharacteristics,
    BluetoothGATTGetServices, BluetoothGATTRegisterEvent, BluetoothGATTUnregisterEvent,
    CharacteristicValueChangedEvent, GUID_BLUETOOTH_GATT_SERVICE_DEVICE_INTERFACE,
};
use windows::Win32::Devices::DeviceAndDriverInstallation::{
    DIGCF_DEVICEINTERFACE, DIGCF_PRESENT, HDEVINFO, SP_DEVICE_INTERFACE_DATA,
    SP_DEVICE_INTERFACE_DETAIL_DATA_W, SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInterfaces,
    SetupDiGetClassDevsW, SetupDiGetDeviceInterfaceDetailW,
};
use windows::Win32::Foundation::{
    CloseHandle, ERROR_MORE_DATA, GENERIC_READ, GENERIC_WRITE, HANDLE,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FILE_ATTRIBUTE_NORMAL, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING,
};
use windows::core::PCWSTR;

const HID_SERVICE: u16 = 0x1812;
const REPORT_CHARACTERISTIC: u16 = 0x2A4D;

fn main() {
    let paths = interface_paths().expect("enumerate GATT interfaces");
    println!("GATT service interfaces: {}", paths.len());
    for path in &paths {
        println!("path: {path}");
    }
    for path in paths {
        if !path.to_ascii_lowercase().contains("4754514c6b3a") {
            continue;
        }
        println!("opening: {path}");
        match probe(&path) {
            Ok(()) => return,
            Err(error) => println!("probe failed: {error:?}"),
        }
    }
    println!("no matching GATT interface");
}

fn probe(path: &str) -> windows::core::Result<()> {
    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE,
            None,
            OPEN_EXISTING,
            FILE_ATTRIBUTE_NORMAL,
            None,
        )?
    };

    let services = match query_services(handle) {
        Ok(services) => services,
        Err(error) => {
            unsafe {
                let _ = CloseHandle(handle);
            }
            return Err(error);
        }
    };
    println!("services: {}", services.len());
    for service in &services {
        let uuid = unsafe {
            if service.ServiceUuid.IsShortUuid {
                service.ServiceUuid.Value.ShortUuid
            } else {
                0
            }
        };
        println!("  service: 0x{uuid:04X} attr {}", service.AttributeHandle);
    }

    let Some(hid) = services.iter().find(|service| unsafe {
        service.ServiceUuid.IsShortUuid && service.ServiceUuid.Value.ShortUuid == HID_SERVICE
    }) else {
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Err(windows::core::Error::new(
            windows::core::HRESULT(0x80004005u32 as i32),
            "HID service missing",
        ));
    };

    let characteristics = query_characteristics(handle, hid)?;
    println!("characteristics: {}", characteristics.len());
    let mut subscribed = None;
    for characteristic in &characteristics {
        let uuid = unsafe {
            if characteristic.CharacteristicUuid.IsShortUuid {
                characteristic.CharacteristicUuid.Value.ShortUuid
            } else {
                0
            }
        };
        println!(
            "  characteristic: 0x{uuid:04X} value {} notify={} indicate={}",
            characteristic.CharacteristicValueHandle,
            characteristic.IsNotifiable,
            characteristic.IsIndicatable
        );
        if uuid == REPORT_CHARACTERISTIC
            && (characteristic.IsNotifiable || characteristic.IsIndicatable)
        {
            subscribed = Some(*characteristic);
        }
    }

    let Some(characteristic) = subscribed else {
        unsafe {
            let _ = CloseHandle(handle);
        }
        return Err(windows::core::Error::new(
            windows::core::HRESULT(0x80004005u32 as i32),
            "no report characteristic",
        ));
    };

    let (sender, receiver) = mpsc::channel::<Vec<u8>>();
    let context = Box::into_raw(Box::new(sender));
    let registration = BLUETOOTH_GATT_VALUE_CHANGED_EVENT_REGISTRATION {
        NumCharacteristics: 1,
        Characteristics: [characteristic],
    };
    let mut event_handle = 0isize;
    unsafe {
        BluetoothGATTRegisterEvent(
            handle,
            CharacteristicValueChangedEvent,
            (&registration as *const BLUETOOTH_GATT_VALUE_CHANGED_EVENT_REGISTRATION).cast(),
            Some(on_value_changed),
            Some(context.cast()),
            &mut event_handle,
            BLUETOOTH_GATT_FLAG_NONE,
        )?;
    }
    println!("READY: press remote keys for 60 seconds");
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        match receiver.recv_timeout(Duration::from_millis(100)) {
            Ok(bytes) => {
                println!(
                    "report: {}",
                    bytes
                        .iter()
                        .map(|byte| format!("{byte:02X}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
    unsafe {
        let _ = BluetoothGATTUnregisterEvent(event_handle, BLUETOOTH_GATT_FLAG_NONE);
        drop(Box::from_raw(context));
        let _ = CloseHandle(handle);
    }
    Ok(())
}

unsafe extern "system" fn on_value_changed(
    event_type: BTH_LE_GATT_EVENT_TYPE,
    event_out_parameter: *const c_void,
    context: *const c_void,
) {
    if event_type != CharacteristicValueChangedEvent
        || event_out_parameter.is_null()
        || context.is_null()
    {
        return;
    }
    let event = unsafe { &*(event_out_parameter as *const BLUETOOTH_GATT_VALUE_CHANGED_EVENT) };
    if event.CharacteristicValue.is_null() || event.CharacteristicValueDataSize == 0 {
        return;
    }
    let value = unsafe { &*event.CharacteristicValue };
    let size = usize::try_from(value.DataSize)
        .unwrap_or(0)
        .min(event.CharacteristicValueDataSize);
    if size == 0 {
        return;
    }
    let bytes = unsafe { std::slice::from_raw_parts(value.Data.as_ptr(), size) }.to_vec();
    let sender = unsafe { &*(context as *const mpsc::Sender<Vec<u8>>) };
    let _ = sender.send(bytes);
}

fn query_services(handle: HANDLE) -> windows::core::Result<Vec<BTH_LE_GATT_SERVICE>> {
    let mut count = 0u16;
    let first =
        unsafe { BluetoothGATTGetServices(handle, None, &mut count, BLUETOOTH_GATT_FLAG_NONE) };
    if let Err(error) = first
        && error.code() != windows::core::HRESULT::from_win32(ERROR_MORE_DATA.0)
    {
        return Err(error);
    }
    let mut services = vec![BTH_LE_GATT_SERVICE::default(); usize::from(count)];
    let mut actual = 0u16;
    let second = unsafe {
        BluetoothGATTGetServices(
            handle,
            Some(&mut services),
            &mut actual,
            BLUETOOTH_GATT_FLAG_NONE,
        )
    };
    if let Err(error) = second
        && error.code() != windows::core::HRESULT::from_win32(ERROR_MORE_DATA.0)
    {
        return Err(error);
    }
    services.truncate(usize::from(actual.min(count)));
    Ok(services)
}

fn query_characteristics(
    handle: HANDLE,
    service: &BTH_LE_GATT_SERVICE,
) -> windows::core::Result<Vec<BTH_LE_GATT_CHARACTERISTIC>> {
    let mut count = 0u16;
    let first = unsafe {
        BluetoothGATTGetCharacteristics(
            handle,
            Some(service as *const BTH_LE_GATT_SERVICE),
            None,
            &mut count,
            BLUETOOTH_GATT_FLAG_NONE,
        )
    };
    if let Err(error) = first
        && error.code() != windows::core::HRESULT::from_win32(ERROR_MORE_DATA.0)
    {
        return Err(error);
    }
    let mut characteristics = vec![BTH_LE_GATT_CHARACTERISTIC::default(); usize::from(count)];
    let mut actual = 0u16;
    let second = unsafe {
        BluetoothGATTGetCharacteristics(
            handle,
            Some(service as *const BTH_LE_GATT_SERVICE),
            Some(&mut characteristics),
            &mut actual,
            BLUETOOTH_GATT_FLAG_NONE,
        )
    };
    if let Err(error) = second
        && error.code() != windows::core::HRESULT::from_win32(ERROR_MORE_DATA.0)
    {
        return Err(error);
    }
    characteristics.truncate(usize::from(actual.min(count)));
    Ok(characteristics)
}

fn interface_paths() -> windows::core::Result<Vec<String>> {
    let guid = GUID_BLUETOOTH_GATT_SERVICE_DEVICE_INTERFACE;
    let info = unsafe {
        SetupDiGetClassDevsW(
            Some(&guid),
            PCWSTR::null(),
            None,
            DIGCF_PRESENT | DIGCF_DEVICEINTERFACE,
        )?
    };
    let mut result = Vec::new();
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
        let path =
            String::from_utf16_lossy(unsafe { std::slice::from_raw_parts(path_ptr, length) });
        result.push(path);
    }
    unsafe {
        let _ = SetupDiDestroyDeviceInfoList(info);
    }
    let _ = info;
    let _ = HDEVINFO::default();
    Ok(result)
}
