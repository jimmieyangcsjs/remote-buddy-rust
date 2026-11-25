import { useEffect, useState } from "react";
import {
  ExternalLink,
  RefreshCw,
  Settings2,
  ShieldAlert,
  ShieldCheck,
} from "lucide-react";
import { Button } from "../components/Button";
import { BrandMark } from "../components/BrandMark";
import { PageHeader } from "../components/PageHeader";
import { StatusPill } from "../components/StatusPill";
import { Toggle } from "../components/Toggle";
import { ShortcutInput } from "../components/ShortcutInput";
import { useAppStore } from "../store";
import type { SettingsView } from "../lib/types";
import {
  openExternalUrl,
  restartAsAdministrator,
  saveSettings,
} from "../lib/ipc";
import { useI18n } from "../lib/i18n";

export function SettingsPage() {
  const { t } = useI18n();
  const stored = useAppStore((state) => state.settings);
  const version = useAppStore((state) => state.version);
  const configPath = useAppStore((state) => state.configPath);
  const hydrate = useAppStore((state) => state.hydrate);
  const setBackendError = useAppStore((state) => state.setBackendError);
  const setLanguage = useAppStore((state) => state.setLanguage);
  const showToast = useAppStore((state) => state.showToast);
  const [draft, setDraft] = useState<SettingsView | null>(stored);
  const [saving, setSaving] = useState(false);
  const [restarting, setRestarting] = useState(false);
  const dirty =
    !!draft && !!stored && JSON.stringify(draft) !== JSON.stringify(stored);

  useEffect(() => {
    setDraft(stored);
  }, [stored]);

  const update = <K extends keyof SettingsView>(
    key: K,
    value: SettingsView[K],
  ) => {
    setDraft((current) => (current ? { ...current, [key]: value } : current));
    if (key === "language") setLanguage(String(value));
  };

  const save = () => {
    if (!draft) return;
    setSaving(true);
    void saveSettings(draft)
      .then((result) => {
        hydrate(result.snapshot);
        showToast(
          result.warning
            ? result.warning
            : t("设置已保存。", "Settings saved."),
          result.warning ? "error" : "success",
        );
      })
      .catch((error: unknown) => setBackendError(String(error)))
      .finally(() => setSaving(false));
  };

  const openLink = (url: string) => {
    void openExternalUrl(url).catch((error: unknown) =>
      setBackendError(String(error)),
    );
  };

  const restartElevated = () => {
    setRestarting(true);
    void restartAsAdministrator()
      .then(() =>
        showToast(
          t(
            "正在请求管理员权限，请在 UAC 中选择“是”。",
            "Requesting administrator access. Choose Yes in the UAC prompt.",
          ),
          "info",
        ),
      )
      .catch((error: unknown) => {
        setBackendError(String(error));
        showToast(t("管理员权限请求已取消或失败。", "The elevation request was cancelled or failed."), "error");
        setRestarting(false);
      });
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow={t("偏好设置", "Preferences")}
        title={t("按你的方式运行", "Run it your way")}
        subtitle={t(
          "设置保存在本机配置目录。升级时会先备份旧配置，再自动迁移到当前版本。",
          "Settings stay on this device. Upgrades back up the old configuration and migrate it automatically.",
        )}
        actions={
          <Button
            variant="signal"
            icon={saving ? <RefreshCw size={14} /> : <Settings2 size={14} />}
            onClick={save}
            disabled={!draft || saving || !dirty}
          >
            {saving
              ? t("保存中", "Saving")
              : dirty
                ? t("保存设置", "Save settings")
                : t("已保存", "Saved")}
          </Button>
        }
      />

      <div className="settings-grid">
        <section className="panel">
          <div className="panel-head">
            <div>
              <p className="panel-label">{t("运行", "Runtime")}</p>
              <h2 className="panel-title" style={{ marginTop: 6 }}>
                {t("启动与连接", "Startup and connection")}
              </h2>
            </div>
            <StatusPill tone="neutral">{t("本机", "LOCAL")}</StatusPill>
          </div>
          <div className="toggle-row" style={{ borderTop: 0 }}>
            <div className="toggle-copy">
              <p className="toggle-title">{t("开机自动启动", "Launch at sign-in")}</p>
              <p className="toggle-sub">
                {t(
                  "登录后自动启动。与“管理员权限启动”同时开启时，会创建一个最高权限的计划任务。",
                  "Start automatically after sign-in. With administrator startup enabled, Remote Buddy creates a highest-privilege scheduled task.",
                )}
              </p>
            </div>
            <Toggle
              checked={draft?.launchAtStartup ?? false}
              onChange={(value) => update("launchAtStartup", value)}
              label={t("开机自动启动", "Launch at sign-in")}
            />
          </div>
          <div className="toggle-row">
            <div className="toggle-copy">
              <p className="toggle-title">{t("静默启动", "Start silently")}</p>
              <p className="toggle-sub">
                {t(
                  "开机启动时直接驻留系统托盘，不显示主窗口；从桌面手动打开时仍正常显示界面。",
                  "At sign-in, start in the system tray without showing the main window. Manual launches still show the interface.",
                )}
              </p>
            </div>
            <Toggle
              checked={draft?.silentStart ?? false}
              onChange={(value) => update("silentStart", value)}
              label={t("静默启动", "Start silently")}
            />
          </div>
          <div className="toggle-row">
            <div className="toggle-copy">
              <p className="toggle-title">{t("关闭到托盘", "Close to tray")}</p>
              <p className="toggle-sub">
                {t(
                  "关闭窗口不退出进程，保持设备连接与 GATT 保活。",
                  "Closing the window keeps the process, device connection, and GATT keepalive running.",
                )}
              </p>
            </div>
            <Toggle
              checked={draft?.minimizeToTray ?? false}
              onChange={(value) => update("minimizeToTray", value)}
              label={t("关闭到托盘", "Close to tray")}
            />
          </div>
          <div className="toggle-row">
            <div className="toggle-copy">
              <p className="toggle-title">{t("连接保活", "Connection keepalive")}</p>
              <p className="toggle-sub">
                {t(
                  "连接建立后由 Windows 维持；不再后台反复强制重连，避免与蓝牙鼠标争用适配器。",
                  "Windows keeps the link alive after connect. The background service no longer forces repeated reconnects that can contend with a Bluetooth mouse.",
                )}
              </p>
            </div>
            <StatusPill tone="signal">{t("保活", "KEEP")}</StatusPill>
          </div>
          <div className="toggle-row">
            <div className="toggle-copy">
              <p className="toggle-title">{t("管理员权限", "Administrator access")}</p>
              <p className="toggle-sub">
                {t(
                  "切换高权限应用（如 Argus 等）并恢复其窗口时更可靠。",
                  "More reliable when switching to elevated applications such as Argus and restoring their windows.",
                )}
              </p>
            </div>
            <StatusPill tone={stored?.runningAsAdmin ? "signal" : "amber"}>
              {stored?.runningAsAdmin
                ? t("已提权", "ELEVATED")
                : t("普通权限", "STANDARD")}
            </StatusPill>
          </div>
          <div className="toggle-row">
            <div className="toggle-copy">
              <p className="toggle-title">
                {t("管理员权限启动", "Start with administrator access")}
              </p>
              <p className="toggle-sub">
                {t(
                  "手动启动时请求 UAC；开机启动则通过计划任务直接以最高权限运行，不会反复弹出 UAC。",
                  "Manual launches request UAC. Sign-in startup uses a scheduled task with highest privileges, so it does not prompt on every logon.",
                )}
              </p>
            </div>
            <Toggle
              checked={draft?.runAsAdmin ?? true}
              onChange={(value) => update("runAsAdmin", value)}
              label={t("管理员权限启动", "Start with administrator access")}
            />
          </div>
          {draft?.launchAtStartup &&
          draft.runAsAdmin &&
          !stored?.runningAsAdmin ? (
            <div className="startup-warning" role="status">
              {t(
                "当前不是管理员权限，暂时无法创建管理员开机启动任务。请先点击下方按钮重启，然后再次保存设置。",
                "Remote Buddy is not elevated, so the administrator startup task cannot be created yet. Restart with the button below, then save again.",
              )}
            </div>
          ) : null}
          <div className="field" style={{ marginTop: 14 }}>
            <Button
              variant={stored?.runningAsAdmin ? "ghost" : "signal"}
              icon={
                stored?.runningAsAdmin ? (
                  <ShieldCheck size={14} />
                ) : (
                  <ShieldAlert size={14} />
                )
              }
              onClick={restartElevated}
              disabled={stored?.runningAsAdmin || restarting}
            >
              {stored?.runningAsAdmin
                ? t("当前已在管理员权限下运行", "Running as administrator")
                : restarting
                  ? t("正在请求权限", "Requesting access")
                  : t("立即以管理员权限重新启动", "Restart as administrator")}
            </Button>
            <span className="shortcut-hint">
              {t(
                "重新启动会保留配置和当前设备，仅更换运行权限。",
                "Restarting keeps your configuration and current device; only the privilege level changes.",
              )}
            </span>
          </div>
          <div className="field" style={{ marginTop: 15 }}>
            <span className="field-label">{t("界面语言", "Language")}</span>
            <select
              className="select"
              value={draft?.language === "en-US" ? "en-US" : "zh-CN"}
              onChange={(event) => update("language", event.target.value)}
            >
              <option value="zh-CN">{t("简体中文", "Simplified Chinese")}</option>
              <option value="en-US">English</option>
            </select>
            <span className="shortcut-hint">
              {t(
                "默认使用简体中文；选择 English 后点击保存即可长期使用。",
                "Simplified Chinese is the default. Choose English and save to keep it.",
              )}
            </span>
          </div>
        </section>

        <section className="panel">
          <div className="panel-head">
            <div>
              <p className="panel-label">{t("输入法", "Input tool")}</p>
              <h2 className="panel-title" style={{ marginTop: 6 }}>
                {t("语音触发", "Voice trigger")}
              </h2>
            </div>
            <StatusPill tone="signal">{t("本机", "LOCAL")}</StatusPill>
          </div>
          <div className="control-stack">
            <label className="field">
              <span className="field-label">{t("输入法", "Input tool")}</span>
              <select
                className="select"
                value={draft?.inputTool ?? ""}
                onChange={(event) => update("inputTool", event.target.value)}
              >
                {!draft?.inputTool ? (
                  <option value="">{t("输入法未加载", "Input tool not loaded")}</option>
                ) : null}
                <option value="WeChatInput">{t("微信输入法", "WeChat Input")}</option>
                <option value="SogouInput">{t("搜狗输入法", "Sogou Input")}</option>
                <option value="Custom">{t("自定义", "Custom")}</option>
              </select>
            </label>
            <label className="field">
              <span className="field-label">{t("触发方式", "Trigger mode")}</span>
              <select
                className="select"
                value={draft?.triggerMode ?? "HoldToTalk"}
                onChange={(event) => update("triggerMode", event.target.value)}
              >
                <option value="HoldToTalk">{t("按住说话", "Hold to talk")}</option>
                <option value="ClickToToggle">{t("点击切换", "Click to toggle")}</option>
                <option value="SmartToggle">{t("智能免提", "Smart hands-free")}</option>
              </select>
            </label>
            <label className="field">
              <span className="field-label">{t("快捷键", "Shortcut")}</span>
              <ShortcutInput
                value={draft?.shortcut ?? ""}
                placeholder={t("点击后按下组合键", "Click, then press keys")}
                onChange={(value) => update("shortcut", value)}
              />
              <span className="shortcut-hint">
                {t(
                  "点击输入框后直接按下组合键自动识别；Esc 取消。",
                  "Click the field and press a shortcut to capture it. Press Esc to cancel.",
                )}
              </span>
            </label>
            <label className="field">
              <span className="field-label">{t("CABLE 输出端点", "CABLE output endpoint")}</span>
              <input
                className="select"
                value={draft?.audioOutput ?? ""}
                onChange={(event) => update("audioOutput", event.target.value)}
              />
            </label>
            <label className="field">
              <span className="field-label">{t("CABLE 录音端点", "CABLE recording endpoint")}</span>
              <input
                className="select"
                value={draft?.audioInput ?? ""}
                onChange={(event) => update("audioInput", event.target.value)}
              />
            </label>
          </div>
          <div className="empty-note" style={{ marginTop: 14 }}>
            {t("配置文件：", "Config file: ")}
            {configPath || t("尚未加载", "Not loaded")}
          </div>
        </section>
      </div>

      <section className="about-card" style={{ marginTop: 18 }}>
        <div className="about-mark">
          <BrandMark size={30} />
        </div>
        <h2 className="about-title">Remote Buddy</h2>
        <p className="about-meta">v{version} · RUST / TAURI 2</p>
        <p style={{ color: "#AAB4BC", fontSize: 11, lineHeight: 1.7 }}>
          {t(
            "Windows 10/11 的本地遥控器桥接。没有账号、没有遥测、没有云端音频。",
            "A local remote-control bridge for Windows 10/11. No account, no telemetry, no cloud audio.",
          )}
        </p>
        <div className="about-links">
          <button
            type="button"
            className="about-link"
            onClick={() =>
              openLink("https://github.com/jimmieyangcsjs/remote-buddy-rust")
            }
          >
            {t("项目主页", "Project home")} <ExternalLink size={12} />
          </button>
          <button
            type="button"
            className="about-link"
            onClick={() =>
              openLink(
                "https://github.com/jimmieyangcsjs/remote-buddy-rust/issues",
              )
            }
          >
            {t("问题反馈", "Report an issue")} <ExternalLink size={12} />
          </button>
          <button
            type="button"
            className="about-link"
            onClick={() => openLink("https://vb-audio.com/Cable/")}
          >
            {t("VB-CABLE 官方下载", "Download VB-CABLE")} <ExternalLink size={12} />
          </button>
          <span className="about-link" style={{ paddingInline: 0 }}>
            {t("配置仅保存在本机", "Configuration stays on this device")}
          </span>
        </div>
      </section>
    </div>
  );
}
