use windows::Devices::Bluetooth::BluetoothLEDevice;
use windows::Devices::Enumeration::DeviceInformation;

fn dump_selector(label: &str, selector: &windows::core::HSTRING) {
    let Ok(found) = DeviceInformation::FindAllAsyncAqsFilter(selector).and_then(|op| op.get())
    else {
        println!("{label}: enumeration failed");
        return;
    };
    println!("{label}: {} devices", found.Size().unwrap_or(0));
    for index in 0..found.Size().unwrap_or(0) {
        let Ok(info) = found.GetAt(index) else {
            continue;
        };
        let name = info
            .Name()
            .map(|value| value.to_string_lossy())
            .unwrap_or_default();
        let id = info
            .Id()
            .map(|value| value.to_string_lossy())
            .unwrap_or_default();
        let paired = info
            .Pairing()
            .and_then(|pairing| pairing.IsPaired())
            .unwrap_or(false);
        let address = BluetoothLEDevice::FromIdAsync(&info.Id().unwrap_or_default())
            .and_then(|operation| operation.get())
            .ok()
            .and_then(|device| device.BluetoothAddress().ok());
        println!("  {name} | paired={paired} | address={address:?} | {id}");
    }
}

fn main() {
    if let Ok(selector) = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true) {
        dump_selector("paired BLE selector", &selector);
    }
    if let Ok(selector) = BluetoothLEDevice::GetDeviceSelector() {
        dump_selector("all BLE selector", &selector);
    }
    match remote_buddy_windows::ble::list_paired_remotes() {
        Ok(devices) if devices.is_empty() => {
            println!("No paired Chromecast/Google TV remotes were found.");
        }
        Ok(devices) => {
            for device in devices {
                println!(
                    "{} | {} | {} | connected={}",
                    device.id, device.name, device.formatted_address, device.is_connected
                );
            }
        }
        Err(error) => {
            eprintln!("Paired-device discovery failed: {error}");
            std::process::exit(1);
        }
    }
}
