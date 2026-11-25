# Remote Buddy validation

## Environment

- Rust: rustc 1.100.0 (0ef1bc386 2026-11-09)
- Cargo: cargo 1.100.0 (3acc79c0c 2026-11-05)
- Node: v24.22.3
- pnpm: 11.25.0
- OS: Microsoft Windows NT 10.0.19044.0
- WebView2: 157.0.4341.60
- Generated: 2026-11-25 09:35:45 +08:00

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
| Application EXE | `target\release\remote-buddy-desktop.exe` | 5.83 | AD07BADEFEA1947162428F5D036400BC1DB5BF9B4126D5FCEB10A282EA61D0CF | NotSigned |
| NSIS installer | `target\release\bundle\nsis\Remote Buddy_0.2.0_x64-setup.exe` | 3.72 | A7EAA501BFE96CE02B2E58594C2842909E55B17C52ACAEB07284DC2FD2D43973 | NotSigned |

Artifacts are produced by `build.ps1`; validation does not rebuild them.

## Hardware acceptance

Hardware checks are recorded separately after running on a paired G9N9N.

- [ ] Pair the G9N9N through Windows Settings
- [ ] Startup with button capture disabled leaves the global BthPort ETW session off
- [ ] First remote button notification binds the ACL handle without a GATT probe loop
- [ ] Windows GATT keepalive is active, or the log reports that the driver does not support it
- [ ] No automatic reconnect loop after an initial connection failure
- [ ] Up / Down / Left / Right / Ok / Back / Home
- [ ] Menu / volume / mute / play-pause / power / app keys
- [ ] No duplicate Raw Input + ETW action
- [ ] SmartToggle short press and HoldToTalk long press
- [ ] Audio reaches CABLE Input and live gain changes are audible
- [ ] Autostart launches the application after sign-in
- [ ] Idle for 10+ minutes without periodic GATT probing or forced reconnects
- [ ] Intel Windows 11: Bluetooth mouse remains stable after reboot
- [ ] Bluetooth radio off/on and sleep/resume recovery
