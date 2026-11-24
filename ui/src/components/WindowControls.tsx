import { useEffect, useState } from "react";
import { Copy, Minus, Square, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { isTauriRuntime } from "../lib/ipc";
import { useAppStore } from "../store";
import { useI18n } from "../lib/i18n";
import { toggleWindowMaximize } from "../lib/window";

async function run(action: "minimize" | "close") {
  if (!isTauriRuntime()) return;
  const window = getCurrentWindow();
  try {
    if (action === "minimize") {
      await window.minimize();
    } else {
      await window.close();
    }
  } catch (error) {
    useAppStore
      .getState()
      .setBackendError(`窗口操作失败：${String(error)}`);
  }
}

export function WindowControls() {
  const { t } = useI18n();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    if (!isTauriRuntime()) return;
    const window = getCurrentWindow();
    let disposed = false;
    const sync = () => {
      void window
        .isMaximized()
        .then((value) => {
          if (!disposed) setMaximized(value);
        })
        .catch(() => undefined);
    };
    sync();
    let unlisten: (() => void) | undefined;
    void window.onResized(sync).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  return (
    <div className="window-controls" aria-label={t("窗口控制", "Window controls")}>
      <button
        type="button"
        className="window-control"
        onClick={() => void run("minimize")}
        aria-label={t("最小化", "Minimize")}
        title={t("最小化", "Minimize")}
      >
        <Minus size={14} />
      </button>
      <button
        type="button"
        className="window-control"
        onClick={() => {
          void toggleWindowMaximize()
            .then((value) => {
              if (value !== null) setMaximized(value);
            })
            .catch((error: unknown) => {
              useAppStore
                .getState()
                .setBackendError(`窗口操作失败：${String(error)}`);
            });
        }}
        aria-label={maximized ? t("还原", "Restore") : t("最大化", "Maximize")}
        title={maximized ? t("还原", "Restore") : t("最大化", "Maximize")}
      >
        {maximized ? <Copy size={12} /> : <Square size={12} />}
      </button>
      <button
        type="button"
        className="window-control"
        data-kind="close"
        onClick={() => void run("close")}
        aria-label={t("关闭", "Close")}
        title={t("关闭", "Close")}
      >
        <X size={14} />
      </button>
    </div>
  );
}
