use std::sync::mpsc;
use std::time::Duration;
use windows::Devices::Bluetooth::BluetoothLEDevice;
use windows::Devices::Bluetooth::GenericAttributeProfile::{
    GattCharacteristic, GattClientCharacteristicConfigurationDescriptorValue,
    GattCommunicationStatus,
};
use windows::Foundation::TypedEventHandler;
use windows::Storage::Streams::DataReader;
use windows::core::Ref;

fn main() {
    let device = BluetoothLEDevice::FromBluetoothAddressAsync(0x112233445566)
        .expect("lookup")
        .get()
        .expect("device");
    println!(
        "device: {}",
        device
            .Name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default()
    );
    let result = device
        .GetGattServicesWithCacheModeAsync(
            windows::Devices::Bluetooth::BluetoothCacheMode::Uncached,
        )
        .expect("services")
        .get()
        .expect("service result");
    println!("service status: {:?}", result.Status());
    let services = result.Services().expect("service list");
    let mut subscriptions = Vec::new();
    for index in 0..services.Size().unwrap_or(0) {
        let service = services.GetAt(index).expect("service");
        let service_uuid = service.Uuid().unwrap_or_default();
        println!("service: {service_uuid:?}");
        let chars = match service
            .GetCharacteristicsWithCacheModeAsync(
                windows::Devices::Bluetooth::BluetoothCacheMode::Uncached,
            )
            .and_then(|operation| operation.get())
        {
            Ok(chars) => chars,
            Err(error) => {
                println!("  characteristics error: {error:?}");
                continue;
            }
        };
        println!("  status: {:?}", chars.Status());
        let list = match chars.Characteristics() {
            Ok(list) => list,
            Err(error) => {
                println!("  list error: {error:?}");
                continue;
            }
        };
        for char_index in 0..list.Size().unwrap_or(0) {
            let characteristic = list.GetAt(char_index).expect("char");
            let uuid = characteristic.Uuid().unwrap_or_default();
            let properties = characteristic
                .CharacteristicProperties()
                .map(|value| value.0)
                .unwrap_or(0);
            println!("  char {uuid:?} props=0x{properties:X}");
            if properties & 0x30 == 0 {
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
            let token = characteristic.ValueChanged(&handler).expect("handler");
            let value = if properties & 0x10 != 0 {
                GattClientCharacteristicConfigurationDescriptorValue::Notify
            } else {
                GattClientCharacteristicConfigurationDescriptorValue::Indicate
            };
            let status = characteristic
                .WriteClientCharacteristicConfigurationDescriptorAsync(value)
                .expect("subscribe")
                .get()
                .expect("subscribe result");
            println!("  subscribe {uuid:?}: {status:?}");
            if status == GattCommunicationStatus::Success {
                subscriptions.push((service_uuid, uuid, characteristic, token, receiver));
            } else {
                let _ = characteristic.RemoveValueChanged(token);
            }
        }
    }

    println!("READY: press remote keys for 90 seconds");
    let deadline = std::time::Instant::now() + Duration::from_secs(90);
    while std::time::Instant::now() < deadline {
        let mut any = false;
        for (service, uuid, _, _, receiver) in &subscriptions {
            while let Ok(bytes) = receiver.try_recv() {
                println!(
                    "notify service={service:?} char={uuid:?} data={}",
                    bytes
                        .iter()
                        .map(|byte| format!("{byte:02X}"))
                        .collect::<Vec<_>>()
                        .join(" ")
                );
                any = true;
            }
        }
        if !any {
            std::thread::sleep(Duration::from_millis(30));
        }
    }
    for (_, _, characteristic, token, _) in subscriptions {
        let _ = characteristic.RemoveValueChanged(token);
    }
}
