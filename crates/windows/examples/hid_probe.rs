use std::sync::mpsc;
use std::time::Duration;
use windows::Devices::Bluetooth::BluetoothLEDevice;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristic, GattClientCharacteristicConfigurationDescriptorValue,
    GattCommunicationStatus,
};
use windows::Devices::Enumeration::DeviceInformation;
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::DataReader;
use windows::core::{GUID, Ref};

const HID_SERVICE: GUID = GUID::from_u128(0x00001812_0000_1000_8000_00805f9b34fb);
const REPORT_CHARACTERISTIC: GUID = GUID::from_u128(0x00002a4d_0000_1000_8000_00805f9b34fb);

fn main() {
    let address = std::env::args()
        .nth(1)
        .map(|value| {
            u64::from_str_radix(value.trim_start_matches("0x"), 16).expect("invalid address")
        })
        .unwrap_or(0x112233445566);

    let device = match BluetoothLEDevice::FromBluetoothAddressAsync(address)
        .expect("FromBluetoothAddressAsync")
        .get()
    {
        Ok(device) => device,
        Err(error) => {
            println!("BluetoothLEDevice lookup failed: {error:?}");
            let selector = BluetoothLEDevice::GetDeviceSelector().expect("selector");
            let found = DeviceInformation::FindAllAsyncAqsFilter(&selector)
                .expect("find devices")
                .get()
                .expect("device list");
            println!("paired/known devices: {}", found.Size().unwrap_or(0));
            for index in 0..found.Size().unwrap_or(0) {
                let info = found.GetAt(index).expect("device info");
                println!(
                    "  {} | {} | paired={:?}",
                    info.Id()
                        .map(|value| value.to_string_lossy())
                        .unwrap_or_default(),
                    info.Name()
                        .map(|value| value.to_string_lossy())
                        .unwrap_or_default(),
                    info.Pairing().and_then(|pairing| pairing.IsPaired()),
                );
            }
            return;
        }
    };
    println!(
        "device: {}",
        device
            .Name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default()
    );

    let all_services = device
        .GetGattServicesWithCacheModeAsync(
            windows::Devices::Bluetooth::BluetoothCacheMode::Uncached,
        )
        .expect("GetGattServices")
        .get()
        .expect("all services result");
    println!("all services status: {:?}", all_services.Status());
    let all = all_services.Services().expect("all service list");
    println!("all services: {}", all.Size().unwrap_or(0));
    for index in 0..all.Size().unwrap_or(0) {
        let service = all.GetAt(index).expect("all service");
        println!("  service: {:?}", service.Uuid().unwrap_or_default());
    }

    let result = device
        .GetGattServicesForUuidWithCacheModeAsync(
            HID_SERVICE,
            windows::Devices::Bluetooth::BluetoothCacheMode::Uncached,
        )
        .expect("GetGattServicesForUuid")
        .get()
        .expect("service result");
    println!("service status: {:?}", result.Status());
    let services = result.Services().expect("services");
    println!("services: {}", services.Size().unwrap_or(0));
    for index in 0..services.Size().unwrap_or(0) {
        let service = services.GetAt(index).expect("service");
        let chars = service
            .GetCharacteristicsWithCacheModeAsync(
                windows::Devices::Bluetooth::BluetoothCacheMode::Uncached,
            )
            .expect("chars")
            .get()
            .expect("char result");
        println!("characteristics status: {:?}", chars.Status());
        let list = chars.Characteristics().expect("char list");
        for char_index in 0..list.Size().unwrap_or(0) {
            let characteristic = list.GetAt(char_index).expect("char");
            let uuid = characteristic.Uuid().expect("uuid");
            println!(
                "char {char_index}: {uuid:?} props {:?}",
                characteristic.CharacteristicProperties()
            );
            let descriptors = characteristic
                .GetDescriptorsAsync()
                .expect("descs")
                .get()
                .expect("desc result");
            if let Ok(descriptors) = descriptors.Descriptors() {
                for descriptor_index in 0..descriptors.Size().unwrap_or(0) {
                    let descriptor = descriptors.GetAt(descriptor_index).expect("descriptor");
                    println!("  descriptor: {:?}", descriptor.Uuid().unwrap_or_default());
                }
            }
        }
    }

    let services = match device
        .GetGattServicesForUuidWithCacheModeAsync(
            HID_SERVICE,
            windows::Devices::Bluetooth::BluetoothCacheMode::Uncached,
        )
        .and_then(|operation| operation.get())
    {
        Ok(result) if result.Status().ok() == Some(GattCommunicationStatus::Success) => {
            result.Services().expect("services")
        }
        other => {
            println!("service reopen failed: {other:?}");
            return;
        }
    };

    let mut subscriptions = Vec::new();
    for index in 0..services.Size().unwrap_or(0) {
        let service = services.GetAt(index).expect("service");
        let chars = match service
            .GetCharacteristicsWithCacheModeAsync(
                windows::Devices::Bluetooth::BluetoothCacheMode::Uncached,
            )
            .and_then(|operation| operation.get())
        {
            Ok(chars) => chars,
            Err(error) => {
                println!("characteristics failed: {error:?}");
                continue;
            }
        };
        let list = match chars.Characteristics() {
            Ok(list) => list,
            Err(error) => {
                println!("characteristic list failed: {error:?}");
                continue;
            }
        };
        for char_index in 0..list.Size().unwrap_or(0) {
            let characteristic = list.GetAt(char_index).expect("char");
            if characteristic.Uuid().ok() != Some(REPORT_CHARACTERISTIC) {
                continue;
            }
            let (sender, receiver) = mpsc::channel::<Vec<u8>>();
            let handler = TypedEventHandler::new(
                move |_sender: Ref<'_, GattCharacteristic>,
                      args: Ref<
                    '_,
                    windows::Devices::Bluetooth::GenericAttributeProfile::GattValueChangedEventArgs,
                >| {
                    if let Ok(args) = args.ok()
                        && let Ok(buffer) = args.CharacteristicValue()
                        && let Ok(reader) = DataReader::FromBuffer(&buffer)
                    {
                        let mut bytes =
                            vec![0_u8; reader.UnconsumedBufferLength().unwrap_or(0) as usize];
                        if reader.ReadBytes(&mut bytes).is_ok() {
                            let _ = sender.send(bytes);
                        }
                    }
                    Ok(())
                },
            );
            let token = characteristic.ValueChanged(&handler).expect("ValueChanged");
            let status = characteristic
                .WriteClientCharacteristicConfigurationDescriptorAsync(
                    GattClientCharacteristicConfigurationDescriptorValue::Notify,
                )
                .expect("subscribe")
                .get()
                .expect("subscribe result");
            println!("subscribe {char_index}: {status:?}");
            subscriptions.push((characteristic, token, receiver));
        }
    }

    if subscriptions.is_empty() {
        println!("no Report characteristic subscribed");
        return;
    }
    println!("READY: press remote keys for 60 seconds");
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        let mut received = false;
        for (_, _, receiver) in &subscriptions {
            while let Ok(bytes) = receiver.try_recv() {
                println!(
                    "report: {}",
                    bytes
                        .iter()
                        .map(|b| format!("{b:02X}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                received = true;
            }
        }
        if !received {
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    for (characteristic, token, _) in subscriptions {
        let _ = characteristic.RemoveValueChanged(token);
    }
}
