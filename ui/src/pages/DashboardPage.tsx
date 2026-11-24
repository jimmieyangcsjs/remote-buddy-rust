import {
  ArrowUpRight,
  BatteryMedium,
  Cable,
  Radio,
  Zap,
} from "lucide-react";
import { Button } from "../components/Button";
import { PageHeader } from "../components/PageHeader";
import { RemoteIllustration } from "../components/RemoteIllustration";
import { StatusPill } from "../components/StatusPill";
import { Waveform } from "../components/Waveform";
import { useAppStore } from "../store";
import {
  connectDevice,
  discoverDevices,
} from "../lib/ipc";
import { formatDbfs } from "../lib/audio-level";
import {
  connectionPresentation,
  inputToolLabel,
  shortcutLabel,
  triggerModeLabel,
  voiceStatus,
} from "../lib/presentation";
import { useI18n } from "../lib/i18n";
import type { UiLanguage } from "../lib/i18n";

const buttonLabelsZh: Record<string, string> = {
  Up: "上",
  Down: "下",
  Left: "左",
  Right: "右",
  Ok: "确定",
  Back: "返回",
  Home: "主页",
  Menu: "菜单",
  VolumeUp: "音量 +",
  VolumeDown: "音量 -",
  Mute: "静音",
  PlayPause: "播放 / 暂停",
  Power: "电源",
  YouTube: "YouTube",
  Netflix: "Netflix",
  LiveTv: "直播",
  Input: "输入源",
  Voice: "语音键",
  Assistant: "语音助手",
  Custom: "自定义",
};

const buttonLabelsEn: Record<string, string> = {
  Up: "Up",
  Down: "Down",
  Left: "Left",
  Right: "Right",
  Ok: "OK",
  Back: "Back",
  Home: "Home",
  Menu: "Menu",
  VolumeUp: "Volume +",
  VolumeDown: "Volume -",
  Mute: "Mute",
  PlayPause: "Play / Pause",
  Power: "Power",
  YouTube: "YouTube",
  Netflix: "Netflix",
  LiveTv: "Live TV",
  Input: "Input",
  Voice: "Voice key",
  Assistant: "Voice assistant",
  Custom: "Custom",
};

function buttonLabel(button: string, language: UiLanguage) {
  const labels = language === "en" ? buttonLabelsEn : buttonLabelsZh;
  return labels[button] ?? button;
}

function phaseLabel(phase: string, language: UiLanguage) {
  if (language === "en") {
    return { down: "Pressed", up: "Released", repeat: "Repeating" }[phase] ?? phase;
  }
  return { down: "按下", up: "松开", repeat: "持续" }[phase] ?? phase;
}

export function DashboardPage() {
  const { language, t } = useI18n();
  const selectedDeviceId = useAppStore((state) => state.selectedDeviceId);
  const devices = useAppStore((state) => state.devices);
  const connection = useAppStore((state) => state.connection);
  const battery = useAppStore((state) => state.battery);
  const events = useAppStore((state) => state.events);
  const voiceActive = useAppStore((state) => state.voiceActive);
  const voicePhase = useAppStore((state) => state.voicePhase);
  const voiceLevel = useAppStore((state) => state.voiceLevel);
  const voiceLevelHistory = useAppStore((state) => state.voiceLevelHistory);
  const hidTap = useAppStore((state) => state.hidTap);
  const audio = useAppStore((state) => state.audio);
  const settings = useAppStore((state) => state.settings);
  const discovery = useAppStore((state) => state.discovery);
  const lastInput = useAppStore((state) => state.lastInput);
  const pressedButtons = useAppStore((state) => state.pressedButtons);
  const connectionDetail = useAppStore((state) => state.connectionDetail);
  const setActivePage = useAppStore((state) => state.setActivePage);
  const hydrate = useAppStore((state) => state.hydrate);
  const setBackendError = useAppStore((state) => state.setBackendError);
  const device = devices.find((item) => item.id === selectedDeviceId) ?? devices[0];
  const presentation = connectionPresentation(
    connection,
    device?.name,
    connectionDetail?.lastError,
    language,
  );
  const audioReady = audio?.state === "ready";
  const voice = voiceStatus(voiceActive, audioReady, voicePhase, language);
  const liveKey = lastInput
    ? `${buttonLabel(lastInput.button, language)} · ${phaseLabel(lastInput.phase, language)}`
    : t("等待按键", "Waiting for input");
  const liveAction = lastInput?.action ?? "—";

  const reconnect = () => {
    const request = device
      ? connectDevice(device.id)
      : discoverDevices();
    void request
      .then(hydrate)
      .catch((error: unknown) => setBackendError(String(error)));
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow={t("遥控助手 / 实时信号", "Remote Buddy / Live signal")}
        title={presentation.title}
        subtitle={presentation.subtitle}
        actions={
          <>
            <Button variant="ghost" icon={<Zap size={14} />} onClick={reconnect}>
              {device ? t("重新连接", "Reconnect") : t("扫描设备", "Scan devices")}
            </Button>
            <Button
              variant="signal"
              icon={<Radio size={14} />}
              onClick={() => setActivePage("voice")}
            >
              {t("打开语音页", "Open voice")}
            </Button>
          </>
        }
      />

      {!audioReady ? (
        <section
          className="setup-callout"
          aria-label={t("VB-CABLE 安装引导", "VB-CABLE setup guide")}
        >
          <span className="setup-callout-icon">
            <Cable size={18} />
          </span>
          <div className="setup-callout-copy">
            <p className="setup-callout-title">
              {t("未检测到 CABLE Input", "CABLE Input not detected")}
            </p>
            <p className="setup-callout-sub">
              {t(
                "遥控器语音需要 VB-CABLE 虚拟音频驱动。语音页已提供官方下载、安装与路由步骤。",
                "Voice input requires the VB-CABLE virtual audio driver. The Voice page has download, install, and routing steps.",
              )}
            </p>
          </div>
          <Button
            variant="signal"
            size="sm"
            onClick={() => setActivePage("voice")}
          >
            {t("查看安装步骤", "View setup steps")}
          </Button>
        </section>
      ) : null}

      {hidTap?.setupRequired || hidTap?.state === "restartRequired" ? (
        <section
          className="setup-callout"
          aria-label={t("实体按键配置引导", "Button capture setup guide")}
        >
          <span className="setup-callout-icon">
            <Radio size={18} />
          </span>
          <div className="setup-callout-copy">
            <p className="setup-callout-title">
              {hidTap?.state === "restartRequired"
                ? t(
                    "按键增强已写入，等待重新加载蓝牙",
                    "Button enhancement is configured; reload Bluetooth",
                  )
                : t("实体按键载荷尚未启用", "Button reports are not enabled yet")}
            </p>
            <p className="setup-callout-sub">
              {hidTap?.state === "restartRequired"
                ? t(
                    "请关闭并重新打开 Windows 蓝牙，或重启电脑，然后重新启动 Remote Buddy。普通按键随后会实时点亮虚拟遥控器。",
                    "Turn Windows Bluetooth off and on again, or restart the computer, then restart Remote Buddy. Button presses will then light up the virtual remote.",
                  )
                : t(
                    "Windows 目前只允许读取蓝牙事件头，因此普通按键不会点亮虚拟遥控器。请在“诊断”页启用按键增强并重启蓝牙。",
                    "Windows currently exposes only Bluetooth event headers, so button presses cannot light up the virtual remote. Enable button enhancement on Diagnostics and reload Bluetooth.",
                  )}
            </p>
          </div>
          <Button
            variant="signal"
            size="sm"
            onClick={() => setActivePage("diagnostics")}
          >
            {t("前往诊断", "Open diagnostics")}
          </Button>
        </section>
      ) : null}

      <div className="hero-grid">
        <section className="signal-panel">
          <div className="signal-top">
            <div>
              <p className="signal-label">{t("活动桥接", "Active bridge")}</p>
              <p className="signal-device">
                {device?.name ?? t("未选择遥控器", "No remote selected")}
              </p>
              <p className="signal-address">
                {device?.address ??
                  t("请先扫描 Windows 已配对设备", "Scan paired Windows devices first")}
              </p>
            </div>
            <StatusPill tone={presentation.tone} pulse={presentation.pulse}>
              {presentation.badge}
            </StatusPill>
          </div>

          <div className="signal-visual-stage">
            <RemoteIllustration
              className="signal-visual"
              pressedButtons={pressedButtons}
              voiceActive={voiceActive}
            />
          </div>

          <div className="signal-bottom">
            <div className="signal-voice">
              <div className="signal-meter-head">
                <p className="signal-label">
                  {t("实时语音电平", "Live voice level")}
                </p>
                <div className="signal-meter-readout" aria-live="polite">
                  <span>
                    <b>RMS</b> {formatDbfs(voiceLevel.rms)}
                  </span>
                  <span>
                    <b>PEAK</b> {formatDbfs(voiceLevel.peak)}
                  </span>
                </div>
              </div>
              <Waveform
                active={voiceActive}
                levels={voiceLevelHistory}
                compact
              />
            </div>
            <div>
              <p className="signal-label">{t("最近按键", "Last key")}</p>
              <p
                className="signal-device signal-live-key"
                key={lastInput?.id ?? "idle"}
                data-pressed={lastInput?.phase === "down"}
                style={{ fontSize: 17 }}
              >
                {liveKey} <span style={{ color: "#7B8792" }}>/</span>{" "}
                {liveAction}
              </p>
            </div>
          </div>
        </section>

        <section className="telemetry-panel">
          <div className="panel-heading-inline">
            <p className="panel-label">{t("实时状态", "Live telemetry")}</p>
            <StatusPill tone={presentation.tone} pulse={presentation.pulse}>
              {presentation.badge}
            </StatusPill>
          </div>
          <div className="metric-list">
            <div className="metric-row">
              <span className="metric-name">{t("电量", "Battery")}</span>
              <span className="metric-value">
                {battery >= 0 ? battery : "—"}
                {battery >= 0 ? <span className="metric-unit">%</span> : null}
              </span>
            </div>
            <div className="metric-row">
              <span className="metric-name">{t("蓝牙链路", "Bluetooth link")}</span>
              <span className="metric-value metric-value-text">
                {connection === "ready"
                  ? t("已连接", "Connected")
                  : connection === "connecting"
                    ? t("连接中", "Connecting")
                    : connection === "degraded"
                      ? t("重试中", "Retrying")
                      : t("未连接", "Disconnected")}
              </span>
            </div>
            <div className="metric-row">
              <span className="metric-name">{t("语音链路", "Voice link")}</span>
              <span className="metric-value metric-value-text">
                {voiceActive
                  ? t("传输中", "Streaming")
                  : audioReady
                    ? t("待命", "Standby")
                    : t("待检查", "Needs check")}
              </span>
            </div>
            <div className="metric-row">
              <span className="metric-name">{t("按键采集", "Button capture")}</span>
              <span className="metric-value metric-value-text">
                {hidTap?.state === "ready"
                  ? hidTap.source
                  : hidTap?.state === "degraded"
                    ? t("等待绑定", "Waiting for binding")
                    : hidTap?.state === "restartRequired"
                      ? t("需重载蓝牙", "Reload Bluetooth")
                      : t("未就绪", "Not ready")}
              </span>
            </div>
          </div>
          <p className="meter-note">
            {discovery?.detail ??
              t(
                "尚未扫描 Windows 蓝牙设备列表。连接参数由 Windows 蓝牙栈管理。",
                "The Windows Bluetooth device list has not been scanned yet. Connection details are managed by Windows.",
              )}
          </p>
        </section>
      </div>

      <div className="lower-grid">
        <section className="panel">
          <div className="panel-head">
            <div>
              <p className="panel-label">{t("最近捕获", "Recently captured")}</p>
              <h2 className="panel-title" style={{ marginTop: 6 }}>
                {t("按键事件", "Button events")}
              </h2>
            </div>
            <Button
              variant="ghost"
              size="sm"
              icon={<ArrowUpRight size={13} />}
              onClick={() => setActivePage("mapping")}
            >
              {t("查看映射", "View mapping")}
            </Button>
          </div>
          <div className="event-list">
            {events.length === 0 ? (
              <div className="empty-note">
                {t(
                  "按下遥控器按键后，这里会显示实时事件。",
                  "Press a remote button to see live events here.",
                )}
              </div>
            ) : (
              events.slice(0, 5).map((event) => (
                <div className="event-row" key={event.id}>
                  <span className="event-time">{event.time}</span>
                  <div>
                    <p className="event-name">
                      {eventButtonLabel(event.button, language)}
                    </p>
                    <p className="event-action" style={{ margin: "2px 0 0" }}>
                      {event.source}
                    </p>
                  </div>
                  <span className="event-action">
                    {localizedEventAction(event.action, language)}
                  </span>
                </div>
              ))
            )}
          </div>
        </section>

        <section className="panel">
          <div className="panel-head">
            <div>
              <p className="panel-label">{t("语音链路", "Voice route")}</p>
              <h2 className="panel-title" style={{ marginTop: 6 }}>
                {t("语音信号链", "Voice signal chain")}
              </h2>
            </div>
            <StatusPill tone={voice.tone} pulse={voiceActive}>
              {voice.label}
            </StatusPill>
          </div>
          <div className="route-line">
            <div className="route-node">
              <span className="route-index">01</span>
              <div className="route-copy">
                <p className="route-title">{t("遥控器麦克风", "Remote microphone")}</p>
                <p className="route-sub">ATVV · 16 kHz · IMA ADPCM</p>
              </div>
              <BatteryMedium
                size={15}
                color={
                  connection === "ready"
                    ? "var(--signal-strong)"
                    : "var(--muted)"
                }
              />
            </div>
            <div className="route-node">
              <span className="route-index">02</span>
              <div className="route-copy">
                <p className="route-title">CABLE Input</p>
                <p className="route-sub">
                  {audio?.detail ??
                    t("尚未检查 VB-CABLE 输出端点", "VB-CABLE output has not been checked")}
                </p>
              </div>
              <Cable
                size={15}
                color={audioReady ? "var(--signal-strong)" : "var(--amber)"}
              />
            </div>
            <div className="route-node">
              <span className="route-index">03</span>
              <div className="route-copy">
                <p className="route-title">
                  {inputToolLabel(settings?.inputTool, language)}
                </p>
                <p className="route-sub">
                  {settings
                    ? `${triggerModeLabel(settings.triggerMode, language)} · ${shortcutLabel(settings.shortcut, language)}`
                    : t("等待配置", "Waiting for configuration")}
                </p>
              </div>
              <Zap size={15} color="var(--amber)" />
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}

function eventButtonLabel(value: string, language: UiLanguage) {
  const [rawButton, rawGesture] = value.split(" · ");
  const button = buttonLabel(rawButton, language);
  if (!rawGesture) return button;
  const gesture =
    language === "en"
      ? ({
          单击: "Click",
          双击: "Double click",
          长按: "Long press",
          持续: "Repeat",
        }[rawGesture] ?? rawGesture)
      : rawGesture;
  return `${button} · ${gesture}`;
}

function localizedEventAction(value: string, language: UiLanguage) {
  if (language !== "en") return value;
  return (
    {
      无动作: "No action",
      连续重复: "Continuous repeat",
    }[value] ?? value
  );
}
