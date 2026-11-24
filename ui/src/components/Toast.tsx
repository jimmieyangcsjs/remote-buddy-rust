import { useEffect } from "react";
import { AlertCircle, CheckCircle2, Info, X } from "lucide-react";
import { useAppStore } from "../store";
import { useI18n } from "../lib/i18n";

export function Toast() {
  const { t } = useI18n();
  const toast = useAppStore((state) => state.toast);
  const dismissToast = useAppStore((state) => state.dismissToast);

  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(dismissToast, 4200);
    return () => window.clearTimeout(timer);
  }, [toast, dismissToast]);

  if (!toast) return null;
  const Icon =
    toast.tone === "error"
      ? AlertCircle
      : toast.tone === "info"
        ? Info
        : CheckCircle2;

  return (
    <div className="toast-viewport" aria-live="polite" aria-atomic="true">
      <div className="toast" data-tone={toast.tone} role="status">
        <span className="toast-icon">
          <Icon size={17} />
        </span>
        <span className="toast-message">{toast.message}</span>
        <button
          type="button"
          className="toast-close"
          aria-label={t("关闭提示", "Dismiss notification")}
          title={t("关闭提示", "Dismiss notification")}
          onClick={dismissToast}
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}
