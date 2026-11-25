import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppSnapshot,
  ConnectionSnapshot,
  InputEvent,
  KeyEvent,
  LogEntry,
  PageId,
  RawMapping,
  RunningApplication,
  SettingsView,
  VoiceLevelEvent,
  VoiceStateEvent,
} from "./types";

export function isTauriRuntime() {
  return "__TAURI_INTERNALS__" in window;
}

export function getAppState() {
  return invoke<AppSnapshot>("get_app_state");
}

export function discoverDevices() {
  return invoke<AppSnapshot>("discover_devices");
}

export function connectDevice(deviceId: string) {
  return invoke<AppSnapshot>("connect_device", { deviceId });
}

export function disconnectDevice() {
  return invoke<AppSnapshot>("disconnect_device");
}

export function retryDevice() {
  return invoke<AppSnapshot>("retry_device");
}

export function selectDevice(deviceId: string) {
  return invoke<AppSnapshot>("select_device", { deviceId });
}

export function saveMapping(deviceId: string, raw: RawMapping) {
  return invoke<AppSnapshot>("save_mapping", { deviceId, mapping: raw });
}

export function restoreDefaultMappings(deviceId: string) {
  return invoke<AppSnapshot>("restore_default_mappings", { deviceId });
}

export function saveSettings(update: SettingsView) {
  return invoke<AppSnapshot>("save_settings", { update });
}

export function saveVoiceSettings(
  deviceId: string,
  update: { gain: number; voiceMode: string },
) {
  return invoke<AppSnapshot>("save_voice_settings", { deviceId, update });
}

export function testAudioOutput() {
  return invoke<string>("test_audio_output");
}

export function exportDiagnostics() {
  return invoke<string>("export_diagnostics");
}

export function openExternalUrl(url: string) {
  return invoke<void>("open_external_url", { url });
}

export function enableButtonCapture() {
  return invoke<void>("enable_button_capture");
}

export function disableButtonCapture() {
  return invoke<void>("disable_button_capture");
}

export function restartAsAdministrator() {
  return invoke<void>("restart_as_administrator");
}

export function listRunningApplications() {
  return invoke<RunningApplication[]>("list_running_applications");
}

export interface BackendHandlers {
  onConnection: (snapshot: ConnectionSnapshot) => void;
  onInput: (event: InputEvent) => void;
  onButton: (event: KeyEvent) => void;
  onVoice: (event: VoiceStateEvent) => void;
  onVoiceLevel: (event: VoiceLevelEvent) => void;
  onLog: (entry: LogEntry) => void;
  onNavigate: (page: PageId) => void;
}

export async function subscribeBackend(handlers: BackendHandlers) {
  const pages = new Set<PageId>([
    "dashboard",
    "devices",
    "mapping",
    "voice",
    "diagnostics",
    "settings",
  ]);
  const listeners = await Promise.all([
    listen<ConnectionSnapshot>("device-state-changed", (event) =>
      handlers.onConnection(event.payload),
    ),
    listen<InputEvent>("input-event", (event) => handlers.onInput(event.payload)),
    listen<KeyEvent>("button-event", (event) => handlers.onButton(event.payload)),
    listen<VoiceStateEvent>("voice-state-changed", (event) =>
      handlers.onVoice(event.payload),
    ),
    listen<VoiceLevelEvent>("voice-level-changed", (event) =>
      handlers.onVoiceLevel(event.payload),
    ),
    listen<LogEntry>("log-appended", (event) => handlers.onLog(event.payload)),
    listen<string>("navigate-page", (event) => {
      const page = event.payload as PageId;
      if (pages.has(page)) handlers.onNavigate(page);
    }),
  ]);
  return listeners as UnlistenFn[];
}
