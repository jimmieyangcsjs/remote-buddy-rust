import { useState } from "react";
import { Bluetooth, Check, RefreshCw, Signal } from "lucide-react";
import { Button } from "../components/Button";
import { PageHeader } from "../components/PageHeader";
import { StatusPill } from "../components/StatusPill";
import { useAppStore } from "../store";
import {
  connectDevice,
  discoverDevices,
  disconnectDevice,
  selectDevice,
} from "../lib/ipc";
import { connectionPresentation } from "../lib/presentation";
import { useI18n } from "../lib/i18n";

export function DevicesPage() {
  const { language, t } = useI18n();
  const devices = useAppStore((state) => state.devices);
  const selectedDeviceId = useAppStore((state) => state.selectedDeviceId);
  const connection = useAppStore((state) => state.connection);
  const connectionDetail = useAppStore((state) => state.connectionDetail);
  const discovery = useAppStore((state) => state.discovery);
  const hydrate = useAppStore((state) => state.hydrate);
  const setBackendError = useAppStore((state) => state.setBackendError);
  const showToast = useAppStore((state) => state.showToast);
  const [scanning, setScanning] = useState(false);
  const selected =
    devices.find((device) => device.id === selectedDeviceId) ?? devices[0];
  const presentation = connectionPresentation(
    connection,
    selected?.name,
    connectionDetail?.lastError,
    language,
  );
  const discovered = discovery?.candidates.find(
    (candidate) => candidate.formattedAddress === selected?.address,
  );

  const refresh = () => {
    setScanning(true);
    void discoverDevices()
      .then(hydrate)
      .catch((error: unknown) => setBackendError(String(error)))
      .finally(() => setScanning(false));
  };

  const choose = (id: string) => {
    void selectDevice(id)
      .then(hydrate)
      .catch((error: unknown) => setBackendError(String(error)));
  };

  const connect = () => {
    if (!selected) return;
    void connectDevice(selected.id)
      .then((snapshot) => {
        hydrate(snapshot);
        showToast(
          t(
            `正在连接 ${selected.name}，连接状态会在后台持续更新。`,
            `Connecting to ${selected.name}. Status will update in the background.`,
          ),
          "info",
        );
      })
      .catch((error: unknown) => setBackendError(String(error)));
  };

  const disconnect = () => {
    const confirmed = window.confirm(
      t(
        "断开后不会自动重连，需要手动点击连接。确定断开吗？",
        "Automatic reconnect is disabled after a manual disconnect. Disconnect now?",
      ),
    );
    if (!confirmed) return;
    void disconnectDevice()
      .then((snapshot) => {
        hydrate(snapshot);
        showToast(t("已手动断开遥控器。", "Remote disconnected manually."), "info");
      })
      .catch((error: unknown) => setBackendError(String(error)));
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow={t("设备管理", "Device registry")}
        title={t("设备与连接", "Devices and connection")}
        subtitle={t(
          "程序会自动读取 Windows 已配对设备。先在系统设置中配对 Chromecast Remote，再在这里扫描并连接。",
          "Paired Windows devices are detected automatically. Pair the Chromecast Remote in Windows Settings, then scan and connect here.",
        )}
        actions={
          <Button
            variant="ghost"
            icon={<RefreshCw size={14} />}
            onClick={refresh}
            disabled={scanning}
          >
            {scanning
              ? t("扫描中", "Scanning")
              : t("扫描 Windows 设备", "Scan Windows devices")}
          </Button>
        }
      />

      <div className="discovery-note" data-state={discovery?.state ?? "pending"}>
        <Bluetooth size={15} />
        <span>
          {discovery?.detail ??
            t("尚未扫描 Windows 蓝牙列表。", "The Windows Bluetooth list has not been scanned.")}
        </span>
      </div>

      <div className="split-layout">
        <section className="panel">
          <div className="panel-head">
            <div>
              <p className="panel-label">{t("已保存设备", "Saved devices")}</p>
              <h2 className="panel-title" style={{ marginTop: 6 }}>
                {t(`${devices.length} 个设备`, `${devices.length} device(s)`)}
              </h2>
            </div>
            <StatusPill tone="neutral">{t("本机", "LOCAL")}</StatusPill>
          </div>
          <div className="device-list">
            {devices.length === 0 ? (
              <div className="empty-note">
                <p style={{ margin: "0 0 10px" }}>
                  {t("没有找到可用遥控器。请确认：", "No supported remote was found. Check the following:")}
                </p>
                <ol className="setup-steps">
                  <li>
                    {t(
                      "Windows 设置 → 蓝牙和设备 → 添加设备，完成 Chromecast Remote 配对。",
                      "Open Windows Settings → Bluetooth & devices → Add device and pair the Chromecast Remote.",
                    )}
                  </li>
                  <li>
                    {t(
                      "短按遥控器按键唤醒它，确保 Windows 显示“已连接”。",
                      "Press a remote button to wake it and make sure Windows shows it as connected.",
                    )}
                  </li>
                  <li>
                    {t(
                      "点击“扫描 Windows 设备”，程序会自动导入受支持的遥控器。",
                      "Click Scan Windows devices to import supported remotes.",
                    )}
                  </li>
                </ol>
              </div>
            ) : (
              devices.map((device) => (
                <button
                  key={device.id}
                  className="device-row"
                  data-selected={device.id === selectedDeviceId}
                  onClick={() => choose(device.id)}
                  style={{ width: "100%", textAlign: "left" }}
                >
                  <span className="device-glyph">
                    <Bluetooth size={17} />
                  </span>
                  <span className="device-copy">
                    <span className="device-name">{device.name}</span>
                    <span className="device-meta" style={{ display: "block" }}>
                      {device.model} · {device.address}
                    </span>
                  </span>
                  <span className="device-actions">
                    {device.connection === "ready" ? (
                      <StatusPill tone="signal">{t("在线", "LIVE")}</StatusPill>
                    ) : device.id === selectedDeviceId &&
                      connection === "connecting" ? (
                      <StatusPill tone="amber" pulse>
                        {t("连接中", "Connecting")}
                      </StatusPill>
                    ) : null}
                    {device.id === selectedDeviceId ? (
                      <Check size={15} color="var(--signal-strong)" />
                    ) : null}
                  </span>
                </button>
              ))
            )}
          </div>
        </section>

        <section className="panel">
          <div className="panel-head">
            <div>
              <p className="panel-label">{t("当前设备", "Selected device")}</p>
              <h2 className="panel-title" style={{ marginTop: 6 }}>
                {selected?.name ?? t("未选择设备", "No device selected")}
              </h2>
            </div>
            <StatusPill tone={presentation.tone} pulse={presentation.pulse}>
              {presentation.badge}
            </StatusPill>
          </div>
          <div className="metric-list">
            <div className="metric-row">
              <span className="metric-name">{t("型号", "Model")}</span>
              <span className="metric-value" style={{ fontSize: 15 }}>
                {selected?.model ?? "—"}
              </span>
            </div>
            <div className="metric-row">
              <span className="metric-name">{t("蓝牙地址", "Bluetooth address")}</span>
              <span className="metric-value" style={{ fontSize: 13 }}>
                {selected?.address ?? "—"}
              </span>
            </div>
            <div className="metric-row">
              <span className="metric-name">{t("容器 ID", "Container ID")}</span>
              <span className="metric-value" style={{ fontSize: 10 }}>
                {selected?.containerId || "—"}
              </span>
            </div>
            <div className="metric-row">
              <span className="metric-name">{t("Windows 蓝牙", "Windows Bluetooth")}</span>
              <span className="metric-value" style={{ fontSize: 13 }}>
                {discovered
                  ? discovered.isConnected
                    ? t("已连接", "Connected")
                    : t("已配对，未连接", "Paired, not connected")
                  : t("未在本次扫描中返回", "Not returned in this scan")}
              </span>
            </div>
            <div className="metric-row">
              <span className="metric-name">{t("语音模式", "Voice mode")}</span>
              <span className="metric-value" style={{ fontSize: 13 }}>
                {voiceModeLabel(selected?.voiceMode, language)}
              </span>
            </div>
            <div className="metric-row">
              <span className="metric-name">{t("电量", "Battery")}</span>
              <span className="metric-value">
                {selected && selected.battery >= 0 ? selected.battery : "—"}
                {selected && selected.battery >= 0 ? (
                  <span className="metric-unit">%</span>
                ) : null}
              </span>
            </div>
            <div className="metric-row">
              <span className="metric-name">{t("最近活动", "Last activity")}</span>
              <span className="metric-value" style={{ fontSize: 13 }}>
                {selected?.lastSeen ?? "—"}
              </span>
            </div>
          </div>
          <div style={{ display: "flex", gap: 9, marginTop: 18 }}>
            <Button
              variant="primary"
              icon={<Signal size={14} />}
              onClick={connect}
              disabled={!selected}
              style={{ flex: 1 }}
            >
              {connection === "connecting"
                ? t("连接中", "Connecting")
                : t("连接设备", "Connect")}
            </Button>
            <Button
              variant="ghost"
              onClick={disconnect}
              disabled={!selected}
              style={{ flex: 1 }}
            >
              {t("断开", "Disconnect")}
            </Button>
          </div>
          <div className="empty-note" style={{ marginTop: 14 }}>
            {connectionDetail?.lastError
              ? t(
                  `最近错误：${connectionDetail.lastError}`,
                  `Last error: ${connectionDetail.lastError}`,
                )
              : t(
                  "后台会在设备休眠、系统唤醒或蓝牙短暂断开后自动重连；手动断开不会自动恢复。",
                  "The background service reconnects after sleep, resume, or a brief Bluetooth drop. A manual disconnect is not restored automatically.",
                )}
          </div>
        </section>
      </div>
    </div>
  );
}

function voiceModeLabel(
  mode: "smartToggle" | "holdToTalk" | "unknown" | undefined,
  language: "zh" | "en",
) {
  if (language === "en") {
    return { smartToggle: "Smart toggle", holdToTalk: "Hold to talk", unknown: "Unknown" }[
      mode ?? "unknown"
    ];
  }
  return { smartToggle: "智能免提", holdToTalk: "按住说话", unknown: "未知" }[
    mode ?? "unknown"
  ];
}
