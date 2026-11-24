# Architecture

## Goals

- Keep device connection and automatic reconnect independent of window visibility.
- Keep ordinary-button capture, voice capture, and action injection as separate failure domains.
- Make every input source identifiable in diagnostics.
- Preserve the legacy configuration during migration.
- Keep the domain layer testable without Windows APIs.
- Keep window state and device state independent so tray mode does not interrupt input.
- Target Windows x64 only, avoiding 32-bit platform branches in the release build.

## Layers

```text
UI (React)
  |
  | Tauri commands and events
  v
apps/desktop
  |-- window, tray, commands
  |-- AppState and snapshots
  |-- input router and gesture state
  |-- voice session state machine
  |
  +--> crates/core
  |      config, protocol, ADPCM, gestures, supervisor, mappings
  |
  +--> crates/windows
         BLE/GATT, ATVV transport, WASAPI, Raw Input, SendInput, ETW
```

`crates/core` does not depend on Tauri or the Windows crate. `crates/windows` owns all platform calls. `apps/desktop` is the composition root.

## Runtime startup

1. Tauri creates `AppState`.
2. `AppState` loads and migrates the config.
3. A shared unbounded channel is created for `RawFrame` values.
4. The Raw Input listener is started as a fallback.
5. The BthPort ETW listener is started and retained in `AppState`.
6. The BLE backend receives an optional clone of the ETW listener.
7. The device supervisor starts and attempts the selected device.
8. The unified input router consumes Raw Input and ETW frames.
9. Window move, resize, maximize, restore, and DPI changes schedule a WebView layout repair.

The ETW status and the input listener are separate from the BLE connection state. A connected ATVV voice link does not prove that ordinary-button capture is working.

## Connection supervisor

`spawn_supervisor` owns the desired device ID, connection attempts, backoff, and the active session.

States are published through a `watch` channel:

- `Disconnected`
- `Connecting`
- `Ready`
- `Degraded`

The UI subscribes to `device-state-changed`. A background connection failure does not require the main window to be visible.

The supervisor cancels the current connection future on disconnect, retry, or device switch. A session ending triggers the same reconnect logic as an initial failure.

## Voice path

```text
G9N9N voice key
  -> ATVV Control notification (StartSearch)
  -> input-tool shortcut
  -> MicOpen
  -> AudioStart
  -> ATVV audio notifications
  -> IMA ADPCM decode
  -> 16 kHz PCM
  -> cpal/WASAPI output
  -> CABLE Input
  -> input tool microphone
```

The voice state machine handles:

- short press versus long press in SmartToggle mode,
- hands-free reopen after `AudioStop`,
- a five-minute hands-free limit,
- periodic mic extension,
- release of a held input-tool shortcut on stop or disconnect.

Audio and voice state are reported through `voice-state-changed`.

The audio output enumerates WASAPI endpoints through `cpal`, prefers `CABLE Input`, decodes IMA ADPCM to 16 kHz mono PCM, and reports actual RMS and peak values. The process does not persist audio or transcripts.

## Ordinary-button path

```text
BthPort ETW Event 402
  -> L2CAP/ATT notification
  -> device-bound 8-byte report
  -> RawFrame { source: Etw }
  -> report decoder
  -> per-device ReportDecoder
  -> gesture recognizer
  -> mapping lookup
  -> SendInput / media action
```

Raw Input uses the same `RawFrame` and input router. The router keeps:

- a report decoder and gesture recognizer per device,
- held-button state per `(device, button)`,
- a short deduplication window for duplicate sources,
- the last source for gesture attribution.

The source is surfaced in the UI as `Raw Input`, `BthPort ETW`, or `HID tap`.

## Configuration

`AppConfig` uses schema version 4 and serde `flatten` extension maps. Unknown JSON keys are preserved on load/save.

Migration from the legacy V1 format:

- maps legacy action strings to typed `ActionConfig`,
- fills missing default mappings,
- preserves configured non-empty actions,
- adds the `CABLE Input` output default,
- adds per-key long-press mode and preferred audio input/output fields,
- writes atomically and keeps a one-time backup.

## UI boundaries

The React UI does not know how Windows delivers a key. It consumes:

- `get_app_state`,
- `device-state-changed`,
- `button-event`,
- `voice-state-changed`,
- `log-appended`,
- `window-hidden-to-tray`.

Commands are defined in `apps/desktop/src/lib.rs` and serialized through `ui/src/lib/types.ts`. When a Rust snapshot changes, update the TypeScript type at the same time.

## Failure isolation

- ETW unavailable: Raw Input remains active; UI reports degraded/unavailable.
- BLE disconnected: ordinary-button capture can remain available; voice cannot.
- Audio output failure: voice session stops and emits an error event.
- Invalid mapping: rejected before config save.
- Legacy config parse failure: default config is used and the error is logged.

## Application activation

The app switcher enumerates visible, unowned top-level windows and maps them to executable paths. Activation follows this sequence:

1. match an existing window by normalized executable path and optional name tokens;
2. restore only minimized windows;
3. attach the foreground and target input threads when Windows requires it;
4. request foreground activation, with a temporary topmost transition and `SwitchToThisWindow` fallback;
5. launch the configured executable if no window exists.

This path is designed for overlays such as Magpie and for targets running at a different integrity level. Elevated targets require Remote Buddy itself to run elevated.

## Window layout repair

The custom title bar uses native maximize/restore state rather than a JavaScript-only layout toggle. After move, resize, maximize, restore, or scale-factor changes, the Rust host re-reads the Win32 client rectangle and sets the WebView bounds to that exact size on a short schedule. This avoids stale client bounds on secondary displays and after rapid maximize transitions.

## Release profile

The workspace builds for `x86_64-pc-windows-msvc` only. Release builds use fat LTO, one codegen unit, `opt-level = 3`, abort-on-panic, symbol stripping, and the static MSVC runtime.
