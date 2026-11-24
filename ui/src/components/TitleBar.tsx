import type { PointerEvent as ReactPointerEvent } from "react";
import { Circle } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { WindowControls } from "./WindowControls";
import { useAppStore } from "../store";
import { connectionPresentation } from "../lib/presentation";
import { isTauriRuntime } from "../lib/ipc";
import { useI18n } from "../lib/i18n";
import { toggleWindowMaximize } from "../lib/window";

const labels: Record<string, { zh: string; en: string }> = {
  dashboard: { zh: "信号总览", en: "Signal overview" },
  devices: { zh: "设备管理", en: "Device registry" },
  mapping: { zh: "按键映射", en: "Button mapping" },
  voice: { zh: "语音链路", en: "Voice route" },
  diagnostics: { zh: "运行诊断", en: "Runtime diagnostics" },
  settings: { zh: "偏好设置", en: "Preferences" },
};

export function TitleBar() {
  const { language, t } = useI18n();
  const activePage = useAppStore((state) => state.activePage);
  const connection = useAppStore((state) => state.connection);
  const detail = useAppStore((state) => state.connectionDetail);
  const devices = useAppStore((state) => state.devices);
  const selectedDeviceId = useAppStore((state) => state.selectedDeviceId);
  const device = devices.find((item) => item.id === selectedDeviceId) ?? devices[0];
  const presentation = connectionPresentation(
    connection,
    device?.name,
    detail?.lastError,
    language,
  );

  const beginDrag = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (
      event.button !== 0 ||
      !isTauriRuntime() ||
      (event.target as HTMLElement).closest("button, a, input, select")
    ) {
      return;
    }

    const originX = event.clientX;
    const originY = event.clientY;
    const cleanup = () => {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", cleanup);
      window.removeEventListener("pointercancel", cleanup);
    };
    const onPointerMove = (moveEvent: PointerEvent) => {
      if (
        Math.hypot(moveEvent.clientX - originX, moveEvent.clientY - originY) <
        5
      ) {
        return;
      }
      cleanup();
      void getCurrentWindow().startDragging().catch(() => undefined);
    };

    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", cleanup, { once: true });
    window.addEventListener("pointercancel", cleanup, { once: true });
  };

  return (
    <header className="titlebar">
      <div
        className="titlebar-drag"
        onPointerDown={beginDrag}
        onDoubleClick={() => {
          if (isTauriRuntime()) {
            void toggleWindowMaximize().catch(() => undefined);
          }
        }}
      >
        <span className="titlebar-crumb">
          {t("遥控助手", "Remote Buddy")}
        </span>
        <span className="titlebar-sep" />
        <span className="titlebar-crumb">
          {language === "en" ? labels[activePage].en : labels[activePage].zh}
        </span>
        <span className="titlebar-live">
          <Circle
            size={6}
            fill={
              connection === "ready"
                ? "#6E9AFF"
                : connection === "connecting"
                  ? "#D99A4E"
                  : "#D96770"
            }
            stroke="none"
          />
          {presentation.titlebar}
        </span>
      </div>
      <WindowControls />
    </header>
  );
}
