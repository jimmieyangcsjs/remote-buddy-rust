import { useState } from "react";
import {
  Activity,
  AlertTriangle,
  Download,
  RefreshCw,
  ShieldCheck,
  Usb,
  Waves,
} from "lucide-react";
import { Button } from "../components/Button";
import { CableSetupGuide } from "../components/CableSetupGuide";
import { LogViewer } from "../components/LogViewer";
import { PageHeader } from "../components/PageHeader";
import { StatusPill } from "../components/StatusPill";
import { useAppStore } from "../store";
import {
  disableButtonCapture,
  discoverDevices,
  enableButtonCapture,
  exportDiagnostics,
} from "../lib/ipc";
import { webViewRuntime } from "../lib/presentation";
import { useI18n } from "../lib/i18n";

export function DiagnosticsPage() {
  const { language, t } = useI18n();
  const logs = useAppStore((state) => state.logs);
  const hidTap = useAppStore((state) => state.hidTap);
  const audio = useAppStore((state) => state.audio);
  const connection = useAppStore((state) => state.connection);
  const connectionDetail = useAppStore((state) => state.connectionDetail);
  const discovery = useAppStore((state) => state.discovery);
  const hydrate = useAppStore((state) => state.hydrate);
  const setBackendError = useAppStore((state) => state.setBackendError);
  const showToast = useAppStore((state) => state.showToast);
  const [busy, setBusy] = useState(false);
  const webview = webViewRuntime(navigator.userAgent, language);

  const refresh = () => {
    setBusy(true);
    void discoverDevices()
      .then((snapshot) => {
        hydrate(snapshot);
        showToast(
          t(
            "蓝牙、按键与音频状态已重新检测。",
            "Bluetooth, input, and audio status checked again.",
          ),
        );
      })
      .catch((error: unknown) => setBackendError(String(error)))
      .finally(() => setBusy(false));
  };

  const exportPackage = () => {
    setBusy(true);
    void exportDiagnostics()
      .then((path) =>
        showToast(
          t(`诊断信息已导出：${path}`, `Diagnostics exported: ${path}`),
        ),
      )
      .catch((error: unknown) => setBackendError(String(error)))
      .finally(() => setBusy(false));
  };

  const enableCapture = () => {
    setBusy(true);
    void enableButtonCapture()
      .then(() =>
        showToast(
          t(
            "已启动管理员配置。请在 UAC 中选择“是”，随后重新加载蓝牙或重启电脑。",
            "Administrator setup started. Approve the UAC prompt, then reload Bluetooth or restart Windows.",
          ),
          "info",
        ),
      )
      .catch((error: unknown) => setBackendError(String(error)))
      .finally(() => setBusy(false));
  };

  const restoreCapture = () => {
    const confirmed = window.confirm(
      t(
        "将恢复启用按键增强前的 BthPort 注册表参数，并停止全局蓝牙敏感日志。恢复后普通实体按键可能暂时不可用，需要重新启用。确定继续吗？",
        "This restores the BthPort values saved before button capture was enabled and stops global Bluetooth sensitive logging. Ordinary buttons may stop working until capture is enabled again. Continue?",
      ),
    );
    if (!confirmed) return;
    setBusy(true);
    void disableButtonCapture()
      .then(() =>
        showToast(
          t(
            "已启动管理员恢复流程。请在 UAC 中选择“是”，随后完全退出程序并重启电脑。",
            "Administrator restore started. Approve the UAC prompt, then quit the app completely and restart Windows.",
          ),
          "info",
        ),
      )
      .catch((error: unknown) => setBackendError(String(error)))
      .finally(() => setBusy(false));
  };

  const cards = [
    {
      icon: Usb,
      title: t("按键采集", "Button capture"),
      detail: hidTap?.detail ?? t("尚未读取按键采集状态", "Capture status not loaded"),
      status:
        hidTap?.state === "ready" ? t("已就绪", "READY") : t("检查", "CHECK"),
      tone: (hidTap?.state === "ready" ? "signal" : "amber") as
        | "signal"
        | "amber",
    },
    {
      icon: Waves,
      title: t("Windows 蓝牙发现", "Windows Bluetooth discovery"),
      detail:
        discovery?.detail ??
        t("尚未扫描 Windows 蓝牙列表", "Windows Bluetooth has not been scanned"),
      status:
        discovery?.state === "ready" ? t("已就绪", "READY") : t("检查", "CHECK"),
      tone: (discovery?.state === "ready" ? "signal" : "amber") as
        | "signal"
        | "amber",
    },
    {
      icon: ShieldCheck,
      title: "WebView2",
      detail: webview.detail,
      status: t("运行中", "ACTIVE"),
      tone: webview.tone,
    },
    {
      icon: Activity,
      title: "VB-CABLE",
      detail: audio?.detail ?? t("尚未读取音频状态", "Audio status not loaded"),
      status:
        audio?.state === "ready" ? t("已就绪", "READY") : t("检查", "CHECK"),
      tone: (audio?.state === "ready" ? "signal" : "amber") as
        | "signal"
        | "amber",
    },
    {
      icon: Activity,
      title: t("ATVV 语音链路", "ATVV voice route"),
      detail:
        connectionDetail?.lastError ??
        (connection === "ready"
          ? t(
              "GATT 服务已连接，等待语音键",
              "GATT service is connected; waiting for the Voice key",
            )
          : t(
              "设备连接后才会建立语音链路",
              "The voice route starts after the device connects",
            )),
      status:
        connection === "ready" ? t("已就绪", "READY") : t("等待", "WAIT"),
      tone: (connection === "ready" ? "signal" : "neutral") as
        | "signal"
        | "neutral",
    },
  ];

  return (
    <div className="page">
      <PageHeader
        eyebrow={t("运行诊断", "Runtime diagnostics")}
        title={t("每条链路都可见", "Every link in plain sight")}
        subtitle={t(
          "诊断信息只保存在本机。导出内容不包含音频、转写文本、剪贴板或遥测数据。",
          "Diagnostics stay on this computer. Exports contain no audio, transcripts, clipboard data, or telemetry.",
        )}
        actions={
          <>
            <Button
              variant="ghost"
              icon={<RefreshCw size={14} />}
              onClick={refresh}
              disabled={busy}
            >
              {busy ? t("检测中", "Checking") : t("重新检测", "Check again")}
            </Button>
            <Button
              variant="primary"
              icon={<Download size={14} />}
              onClick={exportPackage}
              disabled={busy}
            >
              {t("导出诊断包", "Export diagnostics")}
            </Button>
          </>
        }
      />

      {hidTap?.setupRequired ? (
        <section
          className="setup-callout"
          aria-label={t("实体按键增强配置", "Button capture setup")}
        >
          <span className="setup-callout-icon">
            <AlertTriangle size={18} />
          </span>
          <div className="setup-callout-copy">
            <p className="setup-callout-title">
              {t(
                "实体按键需要启用一次蓝牙报告增强",
                "Button reports need a one-time Bluetooth enhancement",
              )}
            </p>
            <p className="setup-callout-sub">
              {t(
                "Windows 默认只向 ETW 暴露蓝牙事件头，遥控器的普通按键载荷会被省略。点击右侧按钮会弹出 UAC，并先备份原值，再为本机 BthPort 驱动写入三项诊断参数；不会联网或上传数据。注意：该设置对整块蓝牙适配器生效，部分 Intel 蓝牙驱动会连带影响蓝牙鼠标，完成后如出现鼠标异常请立即恢复。",
                "Windows exposes only Bluetooth event headers to ETW by default, so ordinary button payloads are omitted. The button opens UAC, backs up the original values, then writes three local BthPort diagnostic values. No network access or upload occurs. Note: this applies to the whole Bluetooth adapter, and some Intel drivers may affect Bluetooth mice. Restore immediately if the mouse becomes unstable.",
              )}
            </p>
          </div>
          <Button
            variant="signal"
            size="sm"
            onClick={enableCapture}
            disabled={busy}
          >
            {t("启用按键增强", "Enable capture")}
          </Button>
        </section>
      ) : hidTap?.state === "restartRequired" ? (
        <section
          className="setup-callout"
          aria-label={t("需要重新加载蓝牙", "Bluetooth reload required")}
        >
          <span className="setup-callout-icon">
            <AlertTriangle size={18} />
          </span>
          <div className="setup-callout-copy">
            <p className="setup-callout-title">
              {t(
                "按键增强已配置，等待蓝牙驱动重新加载",
                "Button capture is configured; reload the Bluetooth driver",
              )}
            </p>
            <p className="setup-callout-sub">
              {t(
                "请关闭并重新打开 Windows 蓝牙，或重启电脑。完成后重新启动 Remote Buddy，实体按键就会同步点亮映射页的虚拟遥控器。",
                "Turn Windows Bluetooth off and on, or restart Windows. Then restart Remote Buddy so physical keys can light up the virtual remote.",
              )}
            </p>
          </div>
          <Button variant="ghost" size="sm" onClick={refresh} disabled={busy}>
            {t("再次检测", "Check again")}
          </Button>
        </section>
      ) : null}

      {hidTap?.sensitiveData ? (
        <section
          className="setup-callout"
          aria-label={t("蓝牙兼容性恢复", "Bluetooth compatibility restore")}
        >
          <span className="setup-callout-icon">
            <AlertTriangle size={18} />
          </span>
          <div className="setup-callout-copy">
            <p className="setup-callout-title">
              {t(
                "全局蓝牙敏感诊断日志正在生效",
                "Global Bluetooth sensitive diagnostics are active",
              )}
            </p>
            <p className="setup-callout-sub">
              {t(
                "此设置对整块蓝牙适配器生效，不只影响遥控器。Intel 蓝牙环境下，如果重启后蓝牙鼠标无法连接、设备管理器报错或反复重连，请点击右侧恢复原值并重启电脑；程序会优先使用启用前保存的原值。",
                "This setting applies to the whole Bluetooth adapter, not just the remote. On Intel Bluetooth, if a mouse cannot connect after reboot, shows a driver error, or reconnects repeatedly, restore the original values and restart Windows. Saved pre-enable values are restored first.",
              )}
            </p>
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={restoreCapture}
            disabled={busy}
          >
            {t("恢复蓝牙默认设置", "Restore Bluetooth defaults")}
          </Button>
        </section>
      ) : null}

      <div className="diagnostic-grid">
        {cards.map((card) => {
          const Icon = card.icon;
          return (
            <div className="diagnostic-card" key={card.title}>
              <span className="diagnostic-icon">
                <Icon size={16} />
              </span>
              <div className="diagnostic-copy">
                <p className="toggle-title">{card.title}</p>
                <p className="toggle-sub">{card.detail}</p>
              </div>
              <StatusPill tone={card.tone}>{card.status}</StatusPill>
            </div>
          );
        })}
      </div>

      <section className="panel" style={{ marginTop: 18 }}>
        <div className="panel-head">
          <div>
            <p className="panel-label">{t("按键采集明细", "Capture details")}</p>
            <h2 className="panel-title" style={{ marginTop: 6 }}>
              {t("按键采集计数", "Button capture counters")}
            </h2>
          </div>
          <StatusPill
            tone={
              hidTap?.setupRequired
                ? "amber"
                : hidTap?.state === "ready"
                  ? "signal"
                  : "neutral"
            }
          >
            {hidTap?.setupRequired
              ? t("需要配置", "SETUP")
              : hidTap?.state === "ready"
                ? t("已就绪", "READY")
                : t("等待", "WAIT")}
          </StatusPill>
        </div>
        <div className="metric-list">
          <div className="metric-row">
            <span className="metric-name">BthPort {t("事件", "events")}</span>
            <span className="metric-value">{hidTap?.packets ?? 0}</span>
          </div>
          <div className="metric-row">
            <span className="metric-name">{t("完整按键报告", "Full button reports")}</span>
            <span className="metric-value">{hidTap?.lastEvent ?? t("等待按键", "Waiting")}</span>
          </div>
          <div className="metric-row">
            <span className="metric-name">{t("被 Windows 省略", "Redacted by Windows")}</span>
            <span className="metric-value">{hidTap?.redacted ?? 0}</span>
          </div>
          <div className="metric-row">
            <span className="metric-name">{t("敏感载荷", "Sensitive payloads")}</span>
            <span className="metric-value metric-value-text">
              {hidTap?.sensitiveData
                ? t("已启用", "Enabled")
                : t("未启用", "Disabled")}
            </span>
          </div>
        </div>
      </section>

      <CableSetupGuide audio={audio} busy={busy} onRecheck={refresh} />

      <section className="panel log-panel">
        <div className="log-toolbar">
          <div>
            <p className="panel-label">{t("应用日志", "Application log")}</p>
            <h2 className="panel-title" style={{ marginTop: 6 }}>
              {t("最近事件", "Recent events")}
            </h2>
          </div>
          <div className="log-levels">
            <span className="kbd">INFO</span>
            <span className="kbd">WARN</span>
            <span className="kbd">ERROR</span>
          </div>
        </div>
        <LogViewer logs={logs} />
      </section>
    </div>
  );
}
