import {
  Activity,
  AudioLines,
  Keyboard,
  LayoutDashboard,
  RadioTower,
  Settings,
} from "lucide-react";
import { BrandMark } from "./BrandMark";
import { StatusPill } from "./StatusPill";
import { useAppStore } from "../store";
import type { PageId } from "../lib/types";
import { connectionPresentation } from "../lib/presentation";
import { useI18n } from "../lib/i18n";

const items: {
  id: PageId;
  label: string;
  en: string;
  icon: typeof LayoutDashboard;
  kbd: string;
}[] = [
  { id: "dashboard", label: "总览", en: "Overview", icon: LayoutDashboard, kbd: "1" },
  { id: "devices", label: "设备", en: "Devices", icon: RadioTower, kbd: "2" },
  { id: "mapping", label: "按键映射", en: "Mapping", icon: Keyboard, kbd: "3" },
  { id: "voice", label: "语音", en: "Voice", icon: AudioLines, kbd: "4" },
  { id: "diagnostics", label: "诊断", en: "Diagnostics", icon: Activity, kbd: "5" },
  { id: "settings", label: "设置", en: "Settings", icon: Settings, kbd: "6" },
];

export function Sidebar() {
  const { language, t } = useI18n();
  const activePage = useAppStore((state) => state.activePage);
  const setActivePage = useAppStore((state) => state.setActivePage);
  const devices = useAppStore((state) => state.devices);
  const selectedDeviceId = useAppStore((state) => state.selectedDeviceId);
  const connection = useAppStore((state) => state.connection);
  const connectionDetail = useAppStore((state) => state.connectionDetail);
  const version = useAppStore((state) => state.version);
  const selected = devices.find((device) => device.id === selectedDeviceId) ?? devices[0];
  const presentation = connectionPresentation(
    connection,
    selected?.name,
    connectionDetail?.lastError,
    language,
  );

  return (
    <aside className="sidebar">
      <div className="brand">
        <div className="brand-mark">
          <BrandMark size={22} />
        </div>
        <div className="brand-copy">
          <p className="brand-name">Remote Buddy</p>
          <p className="brand-sub">
            {t("Windows 遥控桥接", "Windows signal bridge")}
          </p>
        </div>
      </div>

      <p className="nav-section-label">{t("工作区", "Workspace")}</p>
      <nav className="nav-list" aria-label={t("主导航", "Main navigation")}>
        {items.map((item) => {
          const Icon = item.icon;
          const label = language === "en" ? item.en : item.label;
          return (
            <button
              key={item.id}
              className="nav-item"
              data-active={activePage === item.id}
              onClick={() => setActivePage(item.id)}
              aria-label={label}
              title={label}
            >
              <Icon className="nav-icon" />
              <span className="nav-label">{label}</span>
              <span className="nav-kbd">{item.kbd}</span>
            </button>
          );
        })}
      </nav>

      <div className="sidebar-spacer" />

      <div className="sidebar-device">
        <div className="sidebar-device-top">
          <p className="sidebar-device-name">
            {selected?.name ?? t("未选择设备", "No device selected")}
          </p>
          <StatusPill tone={presentation.tone} pulse={presentation.pulse}>
            {presentation.sidebar}
          </StatusPill>
        </div>
        <p className="sidebar-device-meta">
          {selected?.address ??
            t("WINDOWS 蓝牙 · 未选择设备", "WINDOWS BLUETOOTH · NO DEVICE")}
        </p>
        <p className="sidebar-device-meta">
          {selected?.model ?? t("扫描已配对设备", "Scan paired devices")}
        </p>
      </div>

      <div className="sidebar-footer">
        <span>v{version}</span>
        <span>RUST / TAURI</span>
      </div>
    </aside>
  );
}
