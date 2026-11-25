export type PageId =
  | "dashboard"
  | "devices"
  | "mapping"
  | "voice"
  | "diagnostics"
  | "settings";

export type ConnectionState = "ready" | "connecting" | "disconnected" | "degraded";

export type MediaControl = "volumeUp" | "volumeDown" | "mute" | "playPause";
export type LongPressMode = "oneShot" | "hold";

export type ActionConfig =
  | { type: "none" }
  | { type: "shortcut"; keys: string }
  | { type: "text"; text: string }
  | { type: "media"; control: MediaControl }
  | { type: "launchApp"; path: string; name: string }
  | { type: "continuousRepeat"; intervalMs: number };

export interface RawMapping {
  button: string;
  click: ActionConfig;
  doubleClick: ActionConfig;
  longPress: ActionConfig;
  longPressMode: LongPressMode;
  extensions?: Record<string, unknown>;
}

export interface KeyMapping {
  button: string;
  label: string;
  click: string;
  doubleClick: string;
  longPress: string;
  raw: RawMapping;
}

export interface DeviceInfo {
  id: string;
  name: string;
  model: string;
  address: string;
  containerId: string;
  battery: number;
  connection: ConnectionState;
  lastSeen: string;
  isDefault: boolean;
  voiceMode: "smartToggle" | "holdToTalk" | "unknown";
  gain: number;
  mappings: KeyMapping[];
}

export interface ConnectionSnapshot {
  state: ConnectionState;
  deviceId: string | null;
  failures: number;
  attempt: number;
  lastError: string | null;
}

export interface SettingsView {
  language: string;
  launchAtStartup: boolean;
  silentStart: boolean;
  minimizeToTray: boolean;
  runAsAdmin: boolean;
  runningAsAdmin: boolean;
  inputTool: string;
  triggerMode: string;
  shortcut: string;
  audioInput: string;
  audioOutput: string;
}

export interface HidTapView {
  state: string;
  detail: string;
  source: string;
  packets: number;
  redacted: number;
  sensitiveData: boolean;
  setupRequired: boolean;
  lastEvent: string | null;
}

export interface AudioStatusView {
  state: string;
  detail: string;
  devices: string[];
  inputDevices: string[];
  outputReady: boolean;
  inputReady: boolean;
}

export interface DeviceCandidate {
  id: string;
  name: string;
  modelType: string;
  bluetoothAddress: number;
  formattedAddress: string;
  containerId: string | null;
  isConnected: boolean;
}

export interface RunningApplication {
  name: string;
  path: string;
  windowTitle: string;
  isForeground: boolean;
}

export interface DeviceDiscoveryView {
  state: "pending" | "ready" | "empty" | "error" | string;
  detail: string;
  candidates: DeviceCandidate[];
}

export interface KeyEvent {
  id: string;
  time: string;
  button: string;
  action: string;
  source: string;
}

export interface InputEvent {
  id: string;
  time: string;
  button: string;
  phase: "down" | "up" | "repeat";
  action: string;
  source: string;
}

export interface LogEntry {
  time: string;
  level: "INFO" | "WARN" | "ERROR";
  message: string;
}

export interface VoiceStateEvent {
  active: boolean;
  handsFree: boolean;
  phase: string;
  error: string | null;
}

export interface AudioLevel {
  rms: number;
  peak: number;
}

export interface VoiceLevelEvent extends AudioLevel {
  sequence: number;
}

export interface AppSnapshot {
  version: string;
  configPath: string;
  devices: DeviceInfo[];
  selectedDeviceId: string | null;
  connection: ConnectionSnapshot;
  voiceActive: boolean;
  hidTap: HidTapView;
  audio: AudioStatusView;
  discovery: DeviceDiscoveryView;
  settings: SettingsView;
  logs: LogEntry[];
}

export interface SettingsSaveResult {
  snapshot: AppSnapshot;
  warning: string | null;
}
