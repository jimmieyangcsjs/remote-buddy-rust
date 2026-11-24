# Diagnostics

## Application log

```text
%APPDATA%\RemoteBuddy\logs\rust-app.log
```

The UI also shows the last 500 log entries and exports a ZIP package under `%APPDATA%\RemoteBuddy\diagnostics\`.

```text
system.json            app version, OS/arch, connection, audio and settings snapshot
config.redacted.json   configuration with device IDs, names and addresses removed
logs/app.log           rolling application log, or the in-memory log fallback
hid-tap-status.json    current BthPort ETW input-source state
```

The `system.json` and log entries can still contain local device names or Bluetooth addresses. Treat every exported package as private.

Run the repeatable automated checks with:

```powershell
.\scripts\validate.ps1 -Report docs\validation.md
```

## State to inspect first

### Windows device discovery

The discovery snapshot records one of:

- `pending`: startup has not completed the Windows BLE scan.
- `ready`: at least one supported remote was returned and imported.
- `empty`: Windows returned no supported remote.
- `error`: WinRT enumeration failed; the detail contains the Windows error.

The paired-device selector can return a device whose WinRT `Pairing().IsPaired()` property is false. Treat the selector result as authoritative when the friendly name and Bluetooth address are valid.

### Input and audio

The dashboard diagnostics card now represents **按键采集**, not HID tap:

- `starting`: ETW listener is being created.
- `ready`: ETW is running and bound to at least one device.
- `degraded`: ETW is running but no device is bound.
- `unavailable`: ETW did not start or the listener is absent.

The detail line includes the bound device count and report count. A report count above zero without action output means the failure is downstream in decoding, mapping, or SendInput.

## ETW capture notes

BthPort provider:

```text
{8A1F9517-3A8C-4A9E-A018-4F17A200F277}
```

Event ID 402, HCIRAW keyword.

An ETL capture can be rendered to XML with the Windows SDK:

```powershell
tracerpt .\bth-keys.etl -of XML -o .\bth-keys.xml
```

Search the XML for `BIP_Data`. Ignore the repeated `1B 3F 00 00 02` short notification when diagnosing ordinary buttons; it is not the 8-byte report.

Useful 8-byte examples:

```text
10200F000B0004001B29000300000000000000  Up
10200F000B0004001B29000400000000000000  Down
10200F000B0004001B29000500000000000000  Left
10200F000B0004001B29000600000000000000  Right
10200F000B0004001B29000700000000000000  Ok
10200F000B0004001B29000A00000000000000  Home
10200F000B0004001B29000B00000000000000  Back
10200F000B0004001B29000000000000000000  release
```

## What proves what

| Observation | Conclusion |
| --- | --- |
| Event 402 with complete 8-byte report | Bluetooth/HID notification reached the OS |
| ETW status `ready`, reports increase | Listener and device binding are working |
| Report but no `button-event` | Decoder, mapping, or input router problem |
| `button-event` but no system action | SendInput/media execution problem |
| Voice works while ordinary keys fail | ATVV path is independent of HID/ETW |
| Raw Input empty but ETW reports present | Expected for this G9N9N on the test machine |

## HID access constraints

On the tested Windows configuration, the HID-over-GATT service is owned by the Windows driver. Direct reads from the HID Report characteristic can return an access error to a normal application, while BthPort ETW still observes the ATT notification before the HID consumer path.

This is why BthPort ETW is the primary ordinary-button source and Raw Input is only a fallback. Direct HID access should not be reintroduced as a primary fix without new evidence from the target Windows build and Bluetooth driver.

## Performance evidence

An interrupt trace on one affected development machine showed a very high interrupt rate, with Bluetooth/USB and screen-sharing driver paths under suspicion. The available evidence did not attribute the interrupt load to the Remote Buddy process. Rust reduces application overhead; it does not repair a faulty or misconfigured driver.

Keep driver traces separate from input-path traces when comparing before/after behavior.
