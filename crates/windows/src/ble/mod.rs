use remote_buddy_core::device::{DeviceCandidate, address_from_device_id, supported_remote_name};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothLEDevice};
use windows::Devices::Enumeration::DeviceInformation;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BleDevice {
    pub id: String,
    pub name: String,
    pub address: u64,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BleEvent {
    Added(BleDevice),
    Updated(BleDevice),
    Removed(String),
    RadioOff,
    RadioOn,
}

#[derive(Debug, Error)]
pub enum BleError {
    #[error("Bluetooth LE is not available on this system")]
    NotAvailable,
    #[error("WinRT call failed: {0}")]
    WinRt(#[from] windows::core::Error),
    #[error("device {0} was not found")]
    DeviceNotFound(String),
    #[error("GATT service {0} was not found")]
    ServiceNotFound(String),
    #[error("GATT characteristic {0} was not found")]
    CharacteristicNotFound(String),
    #[error("operation timed out")]
    Timeout,
    #[error("GATT operation failed: {0}")]
    Gatt(String),
    #[error("background task failed: {0}")]
    Join(String),
}

pub fn list_paired_remotes() -> Result<Vec<DeviceCandidate>, BleError> {
    let selector = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true)?;
    let mut devices = enumerate_remotes(&selector)?;
    if devices.is_empty()
        && let Ok(all_selector) = BluetoothLEDevice::GetDeviceSelector()
    {
        devices = enumerate_remotes(&all_selector)?;
    }
    devices.sort_by(|left, right| {
        right
            .is_connected
            .cmp(&left.is_connected)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(devices)
}

fn enumerate_remotes(selector: &windows::core::HSTRING) -> Result<Vec<DeviceCandidate>, BleError> {
    let found = DeviceInformation::FindAllAsyncAqsFilter(selector)?.get()?;
    let mut devices: Vec<DeviceCandidate> = Vec::new();
    for index in 0..found.Size()? {
        let Ok(info) = found.GetAt(index) else {
            continue;
        };
        let info_name = info
            .Name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default();
        let Ok(id) = info.Id() else {
            continue;
        };
        let opened = BluetoothLEDevice::FromIdAsync(&id).and_then(|operation| operation.get());
        let (device_name, address, is_connected) = match opened {
            Ok(device) => {
                let name = device
                    .Name()
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_default();
                let address = device.BluetoothAddress().ok();
                let connected =
                    device.ConnectionStatus() == Ok(BluetoothConnectionStatus::Connected);
                let _ = device.Close();
                (name, address, connected)
            }
            Err(_) => (String::new(), None, false),
        };
        let name = if info_name.trim().is_empty() {
            device_name
        } else {
            info_name
        };
        if !supported_remote_name(&name) {
            continue;
        }
        let Some(address) = address
            .filter(|address| *address != 0)
            .or_else(|| address_from_device_id(&id.to_string_lossy()))
        else {
            continue;
        };
        if let Some(existing) = devices
            .iter_mut()
            .find(|candidate| candidate.bluetooth_address == address)
        {
            existing.is_connected |= is_connected;
        } else {
            devices.push(DeviceCandidate::new(address, name, is_connected));
        }
    }
    Ok(devices)
}

pub mod atvv_transport {
    use super::BleError;
    use remote_buddy_core::protocol::{
        AtvvControl, AtvvNotification, decode_control, encode_control,
    };
    use std::time::Duration;
    use tokio::sync::mpsc;
    use windows::Devices::Bluetooth::GenericAttributeProfile::{
        GattCharacteristic, GattClientCharacteristicConfigurationDescriptorValue,
        GattCommunicationStatus, GattValueChangedEventArgs,
    };
    use windows::Devices::Bluetooth::{BluetoothConnectionStatus, BluetoothLEDevice};
    use windows::Foundation::TypedEventHandler;
    use windows::Security::Cryptography::CryptographicBuffer;
    use windows::Storage::Streams::DataReader;
    use windows::core::{GUID, HSTRING, Ref};

    const SERVICE_UUID: GUID = GUID::from_u128(0xAB5E0001_5A21_4F05_BC7D_AF01F617B664);
    const TX_UUID: GUID = GUID::from_u128(0xAB5E0002_5A21_4F05_BC7D_AF01F617B664);
    const AUDIO_UUID: GUID = GUID::from_u128(0xAB5E0003_5A21_4F05_BC7D_AF01F617B664);
    const CONTROL_UUID: GUID = GUID::from_u128(0xAB5E0004_5A21_4F05_BC7D_AF01F617B664);

    pub struct AtvvTransport {
        device: BluetoothLEDevice,
        tx: GattCharacteristic,
        control: GattCharacteristic,
        audio: GattCharacteristic,
        control_handle: Option<u16>,
        notifications: Option<mpsc::UnboundedReceiver<AtvvNotification>>,
        audio_chunks: Option<mpsc::UnboundedReceiver<Vec<u8>>>,
        handler_tokens: Vec<(GattCharacteristic, i64)>,
    }

    impl std::fmt::Debug for AtvvTransport {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter
                .debug_struct("AtvvTransport")
                .finish_non_exhaustive()
        }
    }

    impl AtvvTransport {
        pub async fn connect(address: u64) -> Result<Self, BleError> {
            tokio::task::spawn_blocking(move || connect_blocking(address))
                .await
                .map_err(|error| BleError::Join(error.to_string()))?
        }

        pub async fn write_control(&self, control: &AtvvControl) -> Result<(), BleError> {
            let bytes = encode_control(control);
            let characteristic = self.tx.clone();
            tokio::task::spawn_blocking(move || write_bytes(&characteristic, &bytes))
                .await
                .map_err(|error| BleError::Join(error.to_string()))?
        }

        pub fn write_control_now(&self, control: &AtvvControl) -> Result<(), BleError> {
            write_bytes(&self.tx, &encode_control(control))
        }

        pub fn take_notifications(&mut self) -> Option<mpsc::UnboundedReceiver<AtvvNotification>> {
            self.notifications.take()
        }

        pub fn take_audio_chunks(&mut self) -> Option<mpsc::UnboundedReceiver<Vec<u8>>> {
            self.audio_chunks.take()
        }

        pub fn is_alive(&self) -> bool {
            self.device.ConnectionStatus() == Ok(BluetoothConnectionStatus::Connected)
        }

        pub fn name(&self) -> String {
            self.device
                .Name()
                .map(|name| name.to_string_lossy())
                .unwrap_or_default()
        }

        pub fn control_handle(&self) -> Option<u16> {
            self.control_handle
        }

        pub async fn wait_closed(&self) -> String {
            loop {
                tokio::time::sleep(Duration::from_secs(2)).await;
                if !self.is_alive() {
                    return "remote disconnected from Windows Bluetooth".to_string();
                }
            }
        }
    }

    impl Drop for AtvvTransport {
        fn drop(&mut self) {
            for (characteristic, token) in self.handler_tokens.drain(..) {
                let _ = characteristic.RemoveValueChanged(token);
            }
            let _ = self
                .audio
                .WriteClientCharacteristicConfigurationDescriptorAsync(
                    GattClientCharacteristicConfigurationDescriptorValue::None,
                );
            let _ = self
                .control
                .WriteClientCharacteristicConfigurationDescriptorAsync(
                    GattClientCharacteristicConfigurationDescriptorValue::None,
                );
        }
    }

    fn connect_blocking(address: u64) -> Result<AtvvTransport, BleError> {
        let device = BluetoothLEDevice::FromBluetoothAddressAsync(address)?.get()?;
        let service_result = device
            .GetGattServicesForUuidWithCacheModeAsync(
                SERVICE_UUID,
                windows::Devices::Bluetooth::BluetoothCacheMode::Uncached,
            )?
            .get()?;
        if service_result.Status()? != GattCommunicationStatus::Success {
            return Err(BleError::ServiceNotFound(format!("{SERVICE_UUID:?}")));
        }
        let services = service_result.Services()?;
        if services.Size()? == 0 {
            return Err(BleError::ServiceNotFound(format!("{SERVICE_UUID:?}")));
        }
        let service = services.GetAt(0)?;
        let characters = service.GetCharacteristicsAsync()?.get()?;
        if characters.Status()? != GattCommunicationStatus::Success {
            return Err(BleError::Gatt(
                "characteristic discovery failed".to_string(),
            ));
        }

        let mut tx = None;
        let mut audio = None;
        let mut control = None;
        let list = characters.Characteristics()?;
        for index in 0..list.Size()? {
            let characteristic = list.GetAt(index)?;
            match characteristic.Uuid()? {
                TX_UUID => tx = Some(characteristic),
                AUDIO_UUID => audio = Some(characteristic),
                CONTROL_UUID => control = Some(characteristic),
                _ => {}
            }
        }
        let tx = tx.ok_or_else(|| BleError::CharacteristicNotFound(format!("{TX_UUID:?}")))?;
        let audio =
            audio.ok_or_else(|| BleError::CharacteristicNotFound(format!("{AUDIO_UUID:?}")))?;
        let control =
            control.ok_or_else(|| BleError::CharacteristicNotFound(format!("{CONTROL_UUID:?}")))?;
        let control_handle = control.AttributeHandle().ok();

        let (notification_tx, notifications) = mpsc::unbounded_channel();
        let control_handler = TypedEventHandler::new(
            move |_sender: Ref<'_, GattCharacteristic>,
                  args: Ref<'_, GattValueChangedEventArgs>| {
                if let Ok(args) = args.ok()
                    && let Ok(buffer) = args.CharacteristicValue()
                    && let Ok(bytes) = read_buffer(&buffer)
                    && let Ok(Some(notification)) = decode_control(&bytes)
                {
                    let _ = notification_tx.send(notification);
                }
                Ok(())
            },
        );
        let control_token = control.ValueChanged(&control_handler)?;

        let (audio_tx, audio_chunks) = mpsc::unbounded_channel();
        let audio_handler = TypedEventHandler::new(
            move |_sender: Ref<'_, GattCharacteristic>,
                  args: Ref<'_, GattValueChangedEventArgs>| {
                if let Ok(args) = args.ok()
                    && let Ok(buffer) = args.CharacteristicValue()
                    && let Ok(bytes) = read_buffer(&buffer)
                {
                    let _ = audio_tx.send(bytes);
                }
                Ok(())
            },
        );
        let audio_token = audio.ValueChanged(&audio_handler)?;

        for characteristic in [&audio, &control] {
            let status = characteristic
                .WriteClientCharacteristicConfigurationDescriptorAsync(
                    GattClientCharacteristicConfigurationDescriptorValue::Notify,
                )?
                .get()?;
            if status != GattCommunicationStatus::Success {
                return Err(BleError::Gatt(format!(
                    "notification subscription failed: {status:?}"
                )));
            }
        }

        let transport = AtvvTransport {
            device,
            tx,
            control: control.clone(),
            audio: audio.clone(),
            control_handle,
            notifications: Some(notifications),
            audio_chunks: Some(audio_chunks),
            handler_tokens: vec![(control, control_token), (audio, audio_token)],
        };
        write_bytes(
            &transport.tx,
            &encode_control(&AtvvControl::GetCapabilities),
        )?;
        Ok(transport)
    }

    fn write_bytes(characteristic: &GattCharacteristic, bytes: &[u8]) -> Result<(), BleError> {
        let buffer = CryptographicBuffer::CreateFromByteArray(bytes)?;
        let result = characteristic.WriteValueWithResultAsync(&buffer)?.get()?;
        if result.Status()? == GattCommunicationStatus::Success {
            Ok(())
        } else {
            Err(BleError::Gatt(format!(
                "write failed: {:?}",
                result.Status()?
            )))
        }
    }

    fn read_buffer(buffer: &windows::Storage::Streams::IBuffer) -> Result<Vec<u8>, BleError> {
        let reader = DataReader::FromBuffer(buffer)?;
        let mut bytes = vec![0_u8; reader.UnconsumedBufferLength()? as usize];
        reader.ReadBytes(&mut bytes)?;
        Ok(bytes)
    }

    pub fn parse_device_name(device: &BluetoothLEDevice) -> String {
        device
            .Name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_else(|_| String::new())
    }

    pub fn device_id(device: &BluetoothLEDevice) -> String {
        device
            .DeviceId()
            .unwrap_or_else(|_| HSTRING::new())
            .to_string_lossy()
    }
}

/// Ask the Windows GATT stack to query a random service UUID. The resulting
/// ATT request is observed by the BthPort ETW listener and binds the ACL
/// connection handle to the selected remote without touching the HID service.
pub fn request_gatt_probe(address: u64, marker: [u8; 16]) -> Result<(), BleError> {
    let guid = crate::input::guid_from_marker(marker);
    let device = BluetoothLEDevice::FromBluetoothAddressAsync(address)?.get()?;
    let _ = device
        .GetGattServicesForUuidWithCacheModeAsync(
            guid,
            windows::Devices::Bluetooth::BluetoothCacheMode::Uncached,
        )?
        .get()?;
    Ok(())
}
