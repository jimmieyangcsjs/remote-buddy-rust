import { create } from "zustand";
import { VOICE_WAVEFORM_BARS } from "./lib/audio-level";
import type {
  AppSnapshot,
  AudioLevel,
  AudioStatusView,
  ConnectionSnapshot,
  ConnectionState,
  DeviceDiscoveryView,
  DeviceInfo,
  HidTapView,
  InputEvent,
  KeyEvent,
  LogEntry,
  PageId,
  SettingsView,
  VoiceLevelEvent,
  VoiceStateEvent,
} from "./lib/types";

interface AppStore {
  activePage: PageId;
  setActivePage: (page: PageId) => void;
  devices: DeviceInfo[];
  selectedDeviceId: string;
  connection: ConnectionState;
  connectionDetail: ConnectionSnapshot | null;
  battery: number;
  latencyMs: number;
  version: string;
  configPath: string;
  settings: SettingsView | null;
  language: string;
  hidTap: HidTapView | null;
  audio: AudioStatusView | null;
  discovery: DeviceDiscoveryView | null;
  voiceActive: boolean;
  voiceHandsFree: boolean;
  voicePhase: string;
  voiceError: string | null;
  voiceLevel: AudioLevel;
  voiceLevelHistory: number[];
  voiceGain: number;
  lastInput: InputEvent | null;
  pressedButtons: string[];
  events: KeyEvent[];
  logs: LogEntry[];
  backendReady: boolean;
  backendError: string | null;
  toast: AppToast | null;
  hydrate: (snapshot: AppSnapshot) => void;
  applyConnection: (snapshot: ConnectionSnapshot) => void;
  applyVoice: (event: VoiceStateEvent) => void;
  applyVoiceLevel: (event: VoiceLevelEvent) => void;
  applyInput: (event: InputEvent) => void;
  setSelectedDevice: (id: string) => void;
  setConnection: (connection: ConnectionState) => void;
  setVoiceGain: (gain: number) => void;
  setBackendError: (error: string | null) => void;
  showToast: (message: string, tone?: AppToast["tone"]) => void;
  dismissToast: () => void;
  setLanguage: (language: string) => void;
  addEvent: (event: KeyEvent) => void;
  addLog: (entry: LogEntry) => void;
}

export interface AppToast {
  id: number;
  message: string;
  tone: "success" | "error" | "info";
}

let toastSequence = 0;

export const useAppStore = create<AppStore>((set) => ({
  activePage: "dashboard",
  setActivePage: (activePage) => set({ activePage }),
  devices: [],
  selectedDeviceId: "",
  connection: "disconnected",
  connectionDetail: null,
  battery: -1,
  latencyMs: 0,
  version: "0.2.0",
  configPath: "",
  settings: null,
  language: "zh-CN",
  hidTap: null,
  audio: null,
  discovery: null,
  voiceActive: false,
  voiceHandsFree: false,
  voicePhase: "idle",
  voiceError: null,
  voiceLevel: { rms: 0, peak: 0 },
  voiceLevelHistory: [],
  voiceGain: 3.62,
  lastInput: null,
  pressedButtons: [],
  events: [],
  logs: [],
  backendReady: false,
  backendError: null,
  toast: null,
  hydrate: (snapshot) => {
    const selected =
      snapshot.devices.find((device) => device.id === snapshot.selectedDeviceId) ??
      snapshot.devices[0];
    set({
      devices: snapshot.devices,
      selectedDeviceId: selected?.id ?? "",
      connection: snapshot.connection.state,
      connectionDetail: snapshot.connection,
      battery: selected?.battery ?? -1,
      version: snapshot.version,
      configPath: snapshot.configPath,
      settings: snapshot.settings,
      language: snapshot.settings.language,
      hidTap: snapshot.hidTap,
      audio: snapshot.audio,
      discovery: snapshot.discovery,
      voiceActive: snapshot.voiceActive,
      voiceGain: selected?.gain ?? 3.62,
      logs: snapshot.logs,
      backendReady: true,
      backendError: null,
    });
  },
  applyConnection: (snapshot) =>
    set((state) => ({
      connection: snapshot.state,
      connectionDetail: snapshot,
      devices: state.devices.map((device) => {
        if (device.id !== state.selectedDeviceId) return device;
        return {
          ...device,
          connection: snapshot.state,
          lastSeen:
            snapshot.state === "ready"
              ? "刚刚"
              : snapshot.lastError
                ? snapshot.lastError
                : device.lastSeen,
        };
      }),
    })),
  applyVoice: (event) =>
    set({
      voiceActive: event.active,
      voiceHandsFree: event.handsFree,
      voicePhase: event.phase,
      voiceError: event.error,
      ...(event.active
        ? {}
        : {
            voiceLevel: { rms: 0, peak: 0 },
            voiceLevelHistory: [],
          }),
    }),
  applyVoiceLevel: (event) =>
    set((state) => ({
      voiceLevel: { rms: event.rms, peak: event.peak },
      voiceLevelHistory: [...state.voiceLevelHistory, event.rms].slice(
        -VOICE_WAVEFORM_BARS,
      ),
    })),
  applyInput: (event) =>
    set((state) => {
      const pressed = new Set(state.pressedButtons);
      if (event.phase === "up") {
        pressed.delete(event.button);
      } else {
        pressed.add(event.button);
      }
      return {
        lastInput: event,
        pressedButtons: [...pressed],
      };
    }),
  setSelectedDevice: (selectedDeviceId) =>
    set((state) => {
      const selected = state.devices.find((device) => device.id === selectedDeviceId);
      return {
        selectedDeviceId,
        battery: selected?.battery ?? -1,
        voiceGain: selected?.gain ?? state.voiceGain,
      };
    }),
  setConnection: (connection) => set({ connection }),
  setVoiceGain: (voiceGain) => set({ voiceGain }),
  setBackendError: (backendError) => set({ backendError }),
  showToast: (message, tone = "success") =>
    set({
      toast: {
        id: (toastSequence += 1),
        message,
        tone,
      },
    }),
  dismissToast: () => set({ toast: null }),
  setLanguage: (language) => set({ language }),
  addEvent: (event) =>
    set((state) => ({ events: [event, ...state.events].slice(0, 10) })),
  addLog: (entry) =>
    set((state) => ({ logs: [...state.logs, entry].slice(-500) })),
}));
