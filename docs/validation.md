# Remote Buddy validation

## Environment

- Rust: rustc 1.100.0 (0ef1bc386 2026-11-09)
- Cargo: cargo 1.100.0 (3acc79c0c 2026-11-05)
- Node: v24.22.3
- pnpm: 11.25.0
- OS: Microsoft Windows NT 10.0.19044.0
- WebView2: 157.0.4341.60
- Generated: 2026-11-25 10:54:00 +08:00

## Automated checks

| Check | Result | Seconds | Detail |
| --- | --- | ---: | --- |
| cargo fmt | PASS | 0.8 |  |
| cargo test | PASS | 8 |  |
| cargo clippy | PASS | 5.2 |  |
| UI tests | PASS | 1.6 | 23 tests |
| UI build | PASS | 2.9 |  |

## Release artifacts

| Artifact | Path | MiB | SHA-256 | Signature |
| --- | --- | ---: | --- | --- |
| Application EXE | `target\release\remote-buddy-desktop.exe` | 5.81 | 972478FCE92976B369C4210F25B6985B848085A10D008F41026E047AA5E9961F | NotSigned |
| NSIS installer | `target\release\bundle\nsis\Remote Buddy_0.3.0_x64-setup.exe` | 3.72 | 25DD27CCE2BE406EE561F456A4EAAAFC8EAFDC8766B2F2998A17F2BC6C044880 | NotSigned |

Artifacts are produced by `build.ps1`; validation does not rebuild them.

## Startup validation

| Check | Result | Detail |
| --- | --- | --- |
| Administrator task creation | PASS | `schtasks /Create /XML` returned success |
| Task principal | PASS | `HighestAvailable`, interactive current-user logon |
| Battery policy | PASS | Start and continue while on battery are enabled |
| Task action | PASS | Installed EXE with `--autostart --silent` |
| Silent startup | PASS | Manual task run starts the process without a visible `Remote Buddy` main window |
| Normal startup | PASS | Main window title is `Remote Buddy` |
| Current-user fallback | PASS | HKCU Run value points to the installed EXE with `--autostart --silent` |
| Disable cleanup | PASS | Administrator task and HKCU Run value are both removed |

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
- [ ] Autostart launches after a real sign-out/sign-in on the target account
- [ ] Idle for 10+ minutes without periodic GATT probing or forced reconnects
- [ ] Intel Windows 11: Bluetooth mouse remains stable after reboot
- [ ] Bluetooth radio off/on and sleep/resume recovery
