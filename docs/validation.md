# Remote Buddy validation

## Environment

- Rust: rustc 1.100.0 (0ef1bc386 2026-11-09)
- Cargo: cargo 1.100.0 (3acc79c0c 2026-11-05)
- Node: v24.22.3
- pnpm: 11.25.0
- OS: Microsoft Windows NT 10.0.19044.0
- WebView2: 157.0.4341.60
- Generated: 2026-11-24 16:11:53 +08:00

## Automated checks

| Check | Result | Seconds | Detail |
| --- | --- | ---: | --- |
| cargo fmt | PASS | 0.9 |  |
| cargo test | PASS | 5 |  |
| cargo clippy | PASS | 1.9 |  |
| UI tests | PASS | 3.7 |  |
| UI build | PASS | 6.3 |  |

## Release artifacts

| Artifact | Path | MiB | SHA-256 | Signature |
| --- | --- | ---: | --- | --- |
| Application EXE | `target\release\remote-buddy-desktop.exe` | 5.82 | 1A653482A9D082325E2904E947AAD218E0EF8E8D8475CA0E92B8A470BCA8B5EB | NotSigned |
| NSIS installer | `target\release\bundle\nsis\Remote Buddy_0.2.0_x64-setup.exe` | 3.72 | F7DB69F65CCD25EBBEFEBC17F9CF861A00DE730D7A90B42C3A60E62CBE1EE24B | NotSigned |

Artifacts are produced by `build.ps1`; validation does not rebuild them.

## Hardware acceptance

Hardware checks are recorded separately after running on a paired G9N9N.

- [ ] Pair the G9N9N through Windows Settings
- [ ] ETW starts without elevation
- [ ] GATT probe binds the ACL handle
- [ ] Up / Down / Left / Right / Ok / Back / Home
- [ ] Menu / volume / mute / play-pause / power / app keys
- [ ] No duplicate Raw Input + ETW action
- [ ] SmartToggle short press and HoldToTalk long press
- [ ] Audio reaches CABLE Input and live gain changes are audible
- [ ] Autostart launches the application after sign-in
- [ ] Tray reconnect after 10+ minutes idle
- [ ] Bluetooth radio off/on and sleep/resume recovery
