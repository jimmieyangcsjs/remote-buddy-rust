import { useEffect } from "react";
import { Sidebar } from "./components/Sidebar";
import { TitleBar } from "./components/TitleBar";
import { Toast } from "./components/Toast";
import { DashboardPage } from "./pages/DashboardPage";
import { DevicesPage } from "./pages/DevicesPage";
import { DiagnosticsPage } from "./pages/DiagnosticsPage";
import { MappingPage } from "./pages/MappingPage";
import { SettingsPage } from "./pages/SettingsPage";
import { VoicePage } from "./pages/VoicePage";
import { useAppStore } from "./store";
import { getAppState, isTauriRuntime, subscribeBackend } from "./lib/ipc";
import { useI18n } from "./lib/i18n";
import type { PageId } from "./lib/types";

const pageShortcuts: Record<string, PageId> = {
  "1": "dashboard",
  "2": "devices",
  "3": "mapping",
  "4": "voice",
  "5": "diagnostics",
  "6": "settings",
};

export default function App() {
  const { t } = useI18n();
  const activePage = useAppStore((state) => state.activePage);
  const backendError = useAppStore((state) => state.backendError);

  useEffect(() => {
    let disposed = false;
    let unsubscribe: (() => void)[] = [];
    const store = useAppStore.getState();

    if (!isTauriRuntime()) {
      store.setBackendError(
        t(
          "当前为浏览器预览模式，请通过 Tauri 桌面程序运行。",
          "This is a browser preview. Run the Tauri desktop app for device access.",
        ),
      );
      return;
    }

    void getAppState()
      .then((snapshot) => {
        if (!disposed) store.hydrate(snapshot);
      })
      .catch((error: unknown) => {
        if (!disposed) store.setBackendError(String(error));
      });

    void subscribeBackend({
      onConnection: (snapshot) => useAppStore.getState().applyConnection(snapshot),
      onInput: (event) => useAppStore.getState().applyInput(event),
      onButton: (event) => useAppStore.getState().addEvent(event),
      onVoice: (event) => useAppStore.getState().applyVoice(event),
      onVoiceLevel: (event) =>
        useAppStore.getState().applyVoiceLevel(event),
      onLog: (entry) => useAppStore.getState().addLog(entry),
      onNavigate: (page) => useAppStore.getState().setActivePage(page),
    })
      .then((listeners) => {
        if (disposed) listeners.forEach((listener) => listener());
        else unsubscribe = listeners;
      })
      .catch((error: unknown) => {
        if (!disposed) useAppStore.getState().setBackendError(String(error));
      });

    return () => {
      disposed = true;
      unsubscribe.forEach((listener) => listener());
    };
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat || event.ctrlKey || event.altKey || event.metaKey) return;
      const target = event.target as HTMLElement | null;
      const editing =
        target?.matches("input, select, textarea, button, [contenteditable]") ??
        false;

      if (event.key === "Escape") {
        useAppStore.getState().dismissToast();
        return;
      }
      if (editing) return;
      const page = pageShortcuts[event.key];
      if (page) {
        event.preventDefault();
        useAppStore.getState().setActivePage(page);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  return (
    <div className="app-frame">
      <Sidebar />
      <div className="workspace">
        <TitleBar />
        <main className="content">
          {backendError ? (
            <div className="empty-note" style={{ marginBottom: 14 }}>
              {backendError}
            </div>
          ) : null}
          {activePage === "dashboard" ? <DashboardPage /> : null}
          {activePage === "devices" ? <DevicesPage /> : null}
          {activePage === "mapping" ? <MappingPage /> : null}
          {activePage === "voice" ? <VoicePage /> : null}
          {activePage === "diagnostics" ? <DiagnosticsPage /> : null}
          {activePage === "settings" ? <SettingsPage /> : null}
        </main>
      </div>
      <Toast />
    </div>
  );
}
