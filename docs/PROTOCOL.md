# Protocol and device notes

## Chromecast G9N9N identity

```text
VID: 18D1
PID: 9450
Model: Google G9N9N
Example address used in documentation: 11:22:33:44:55:66
Windows BluetoothAddress: 18838586676582
```

The remote exposes standard GATT services and a vendor ATVV service. The Windows HID-over-GATT driver owns the HID service on the test machine, so the application must not depend on reading that service directly.

The real address of a user's remote is local data. It must be redacted from screenshots, logs, ETL captures, and configuration examples before publication.

## ATVV service

```text
Service: AB5E0001-5A21-4F05-BC7D-AF01F617B664
TX:      AB5E0002-5A21-4F05-BC7D-AF01F617B664
Audio:   AB5E0003-5A21-4F05-BC7D-AF01F617B664
Control: AB5E0004-5A21-4F05-BC7D-AF01F617B664
```

The audio and control characteristics use notifications. The application writes the input-tool shortcut and then sends the ATVV control packets:

```text
Get capabilities: 0A 01 00 00 03 03
Mic open:         0C 00
Mic close:        0D <session>
Mic extend:       0E <session>
```

Notifications decoded by `crates/core/src/protocol/atvv.rs`:

```text
0x0B  capabilities
0x04  audio start (interaction, codec, session ID)
0x08  start search / voice key
0x0A  audio sync (predictor, step index)
0x0C  mic refused
0x00  audio stop
```

Audio is IMA ADPCM at 16 kHz in the tested device path. The decoder is in `crates/core/src/audio/adpcm.rs`.

## BthPort ETW

Provider:

```text
{8A1F9517-3A8C-4A9E-A018-4F17A200F277}
```

The listener enables the HCIRAW keyword and filters Event ID 402.

Properties used:

```text
BIP_Type
BIP_Data
BIP_DataLen
```

Observed packet:

```text
10 20         HCI ACL handle 0x2010
0F 00         ACL data length = 15
0B 00         L2CAP length = 11
04 00         ATT fixed channel
1B            ATT Handle Value Notification
29 00         attribute handle 0x0029
03 00 00 00 00 00 00 00
```

The last eight bytes are the HID report. A non-zero first byte is the key index; all-zero is release.

The code accepts `0x1B` and `0x1D` notifications and does not require the HID service to be opened.

## Button report map

| Button | Index | Consumer Usage |
| --- | ---: | ---: |
| Power | `01` | `019E` |
| Play / pause | `02` | `00CD` |
| Up | `03` | `0042` |
| Down | `04` | `0043` |
| Left | `05` | `0044` |
| Right | `06` | `0045` |
| Ok | `07` | `0041` |
| Mute | `08` | `00E2` |
| Menu | `09` | `0221` |
| Home | `0A` | `0223` |
| Back | `0B` | `0224` |
| Volume up | `0C` | `00E9` |
| Volume down | `0D` | `00EA` |
| YouTube | `0E` | `0077` |
| Netflix | `0F` | `0078` |
| Custom | `10` | `0079` |
| Input | `11` | `0089` / `0189` |

Up, Down, Left, Right, Ok, Home, Back, release, and the media/application keys used by the current release have been exercised in the development environment. Report maps can differ between remote firmware revisions, so a new model should be captured and verified instead of assumed compatible.

The decoder also accepts supported two-byte consumer usages and three-byte report-ID forms. An unknown two-byte usage is rejected rather than guessed, because `00 02` can be related to voice/session traffic and must not become Play/Pause.

## ACL binding

ETW reports do not include the Bluetooth device address. The application must bind an ACL connection handle to a configured device.

Current design:

1. Register a random 128-bit marker for the device.
2. Ask WinRT to query that service UUID.
3. Observe the outgoing ATT Find By Type Value request in ETW.
4. Bind the ACL handle when the response arrives.

The implementation is active in the current release. Never bind only by time proximity or by “the only connection” if multiple remotes may be present. The GUID conversion in `guid_from_marker` must continue to be checked against the outgoing ATT bytes when the Windows Bluetooth stack changes.

## Windows HID behavior

- The HID driver may expose a Consumer Control interface without delivering useful Raw Input reports.
- The HID Report characteristic can return `AccessDenied` to a normal application.
- BthPort ETW can still observe the ATT notification because it sees Bluetooth traffic before the HID consumer path.

## Sensitive ETW payloads

Windows can omit the complete report from Event 402 unless BthPort diagnostic values are enabled:

```text
HKLM\SYSTEM\CurrentControlSet\Services\BthPort\Parameters
  EtwLogSensitiveData = 1
  MaxEtwBytes = 1024
  EtwDropLargeEvents = 0
```

The application exposes a user-triggered UAC action for this setup. The values affect only local ETW visibility and must not be treated as a general Bluetooth security bypass.

These facts justify using ETW as the primary ordinary-button source and Raw Input only as a fallback for devices that Windows exposes normally.
