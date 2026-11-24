import { useCallback, useEffect, useState } from "react";
import {
  AppWindow,
  Check,
  Clock3,
  Keyboard,
  Mic,
  MousePointer2,
  MousePointerClick,
  RefreshCw,
  RotateCcw,
  Save,
  ShieldCheck,
} from "lucide-react";
import { Button } from "../components/Button";
import { PageHeader } from "../components/PageHeader";
import { RemoteIllustration } from "../components/RemoteIllustration";
import { StatusPill } from "../components/StatusPill";
import { ShortcutInput } from "../components/ShortcutInput";
import { useAppStore } from "../store";
import type {
  ActionConfig,
  LongPressMode,
  MediaControl,
  RawMapping,
  RunningApplication,
  SettingsView,
} from "../lib/types";
import {
  isTauriRuntime,
  listRunningApplications,
  restoreDefaultMappings,
  saveMapping,
  saveSettings,
} from "../lib/ipc";
import { useI18n } from "../lib/i18n";
import {
  actionTypeLabel,
  buttonLabel,
  gestureLabel,
  mediaLabel,
  summarizeAction,
} from "../lib/presentation";

type ActionKey = "click" | "doubleClick" | "longPress";
type ActionType = ActionConfig["type"];

const gestureKeys: ActionKey[] = ["click", "doubleClick", "longPress"];
const actionTypes: ActionType[] = [
  "none",
  "shortcut",
  "text",
  "media",
  "launchApp",
  "continuousRepeat",
];

const gestureIcons = {
  click: MousePointerClick,
  doubleClick: MousePointer2,
  longPress: Clock3,
};

export function MappingPage() {
  const { language, t } = useI18n();
  const devices = useAppStore((state) => state.devices);
  const selectedDeviceId = useAppStore((state) => state.selectedDeviceId);
  const hydrate = useAppStore((state) => state.hydrate);
  const settings = useAppStore((state) => state.settings);
  const lastInput = useAppStore((state) => state.lastInput);
  const pressedButtons = useAppStore((state) => state.pressedButtons);
  const voiceActive = useAppStore((state) => state.voiceActive);
  const setBackendError = useAppStore((state) => state.setBackendError);
  const showToast = useAppStore((state) => state.showToast);
  const device =
    devices.find((item) => item.id === selectedDeviceId) ?? devices[0];
  const mappings = device?.mappings ?? [];
  const [selectedButton, setSelectedButton] = useState("Ok");
  const [draft, setDraft] = useState<RawMapping | null>(null);
  const [voiceDraft, setVoiceDraft] = useState<SettingsView | null>(settings);
  const [saving, setSaving] = useState(false);

  const current = mappings.find(
    (mapping) => mapping.button === selectedButton,
  );
  const selectedLabel =
    selectedButton === "Voice"
      ? t("语音键", "Voice key")
      : buttonLabel(current?.button ?? selectedButton, language);
  const mappingDirty =
    !!draft &&
    !!current &&
    JSON.stringify(draft) !== JSON.stringify(current.raw);
  const voiceDirty =
    !!voiceDraft &&
    !!settings &&
    JSON.stringify(voiceDraft) !== JSON.stringify(settings);
  const dirty = selectedButton === "Voice" ? voiceDirty : mappingDirty;
  const canSave =
    selectedButton === "Voice" ? voiceDirty && !!voiceDraft : mappingDirty;

  useEffect(() => {
    if (mappings.length === 0) {
      setDraft(null);
      return;
    }
    if (
      selectedButton !== "Voice" &&
      !mappings.some((mapping) => mapping.button === selectedButton)
    ) {
      setSelectedButton(mappings[0].button);
    }
  }, [mappings, selectedButton]);

  useEffect(() => {
    setDraft(current ? structuredClone(current.raw) : null);
  }, [current?.button, current?.raw]);

  useEffect(() => {
    setVoiceDraft(settings);
  }, [settings]);

  useEffect(() => {
    if (!lastInput || lastInput.phase !== "down") return;
    if (mappingDirty || voiceDirty) return;
    if (
      lastInput.button === "Voice" ||
      mappings.some((mapping) => mapping.button === lastInput.button)
    ) {
      setSelectedButton(lastInput.button);
    }
  }, [lastInput, mappings, mappingDirty, voiceDirty]);

  const changeActionType = (key: ActionKey, type: ActionType) => {
    setDraft((previous) => {
      if (!previous) return previous;
      const next = { ...previous };
      next[key] = actionForType(type, previous[key]);
      return next;
    });
  };

  const updateShortcut = (key: ActionKey, keys: string) => {
    setDraft((previous) =>
      previous ? { ...previous, [key]: { type: "shortcut", keys } } : previous,
    );
  };

  const updateText = (key: ActionKey, text: string) => {
    setDraft((previous) =>
      previous ? { ...previous, [key]: { type: "text", text } } : previous,
    );
  };

  const updateMedia = (key: ActionKey, control: MediaControl) => {
    setDraft((previous) =>
      previous ? { ...previous, [key]: { type: "media", control } } : previous,
    );
  };

  const updateRepeat = (key: ActionKey, intervalMs: number) => {
    setDraft((previous) =>
      previous
        ? { ...previous, [key]: { type: "continuousRepeat", intervalMs } }
        : previous,
    );
  };

  const updateLaunchApp = (key: ActionKey, path: string, name: string) => {
    setDraft((previous) =>
      previous ? { ...previous, [key]: { type: "launchApp", path, name } } : previous,
    );
  };

  const updateLongPressMode = (mode: LongPressMode) => {
    setDraft((previous) =>
      previous ? { ...previous, longPressMode: mode } : previous,
    );
  };

  const save = () => {
    if (selectedButton === "Voice") {
      if (!voiceDraft) return;
      setSaving(true);
      void saveSettings(voiceDraft)
        .then((snapshot) => {
          hydrate(snapshot);
          showToast(
            t(
              "语音键设置已保存，并会立即用于下一次触发。",
              "Voice key settings saved and will apply to the next trigger.",
            ),
          );
        })
        .catch((error: unknown) => setBackendError(String(error)))
        .finally(() => setSaving(false));
      return;
    }
    if (!device || !draft) return;
    setSaving(true);
    void saveMapping(device.id, draft)
      .then((snapshot) => {
        hydrate(snapshot);
        showToast(
          t("按键映射已保存并立即生效。", "Key mapping saved and applied."),
        );
      })
      .catch((error: unknown) => setBackendError(String(error)))
      .finally(() => setSaving(false));
  };

  const reset = () => {
    if (!device) return;
    const confirmed = window.confirm(
      t(
        "恢复默认会覆盖该遥控器的全部单击、双击和长按映射。确定继续吗？",
        "Restoring defaults will overwrite every click, double-click, and long-press mapping for this remote. Continue?",
      ),
    );
    if (!confirmed) return;
    setSaving(true);
    void restoreDefaultMappings(device.id)
      .then((snapshot) => {
        hydrate(snapshot);
        showToast(
          t("已恢复该设备的默认映射。", "Default mappings restored."),
        );
      })
      .catch((error: unknown) => setBackendError(String(error)))
      .finally(() => setSaving(false));
  };

  const chooseRemoteButton = (button: string) => {
    if (button === selectedButton) return;
    if (
      dirty &&
      !window.confirm(
        t(
          "当前按键有未保存的修改，切换后这些修改会丢失。继续吗？",
          "This key has unsaved changes. Switching keys will discard them. Continue?",
        ),
      )
    ) {
      return;
    }
    setSelectedButton(button);
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow={t("按键映射", "Button mapping")}
        title={t("把遥控器变成你的快捷键", "Make the remote your shortcut pad")}
        subtitle={t(
          `当前编辑：${device?.name ?? "未选择设备"}。按键事件只在本地处理，不会上传。`,
          `Editing ${device?.name ?? "no device"}. Input events stay on this computer and are never uploaded.`,
        )}
        actions={
          <>
            <Button
              variant="ghost"
              icon={<RotateCcw size={14} />}
              onClick={reset}
              disabled={!device || saving || selectedButton === "Voice"}
            >
              {t("恢复默认", "Restore defaults")}
            </Button>
            <Button
              variant="signal"
              icon={saving ? <Save size={14} /> : <Check size={14} />}
              onClick={save}
              disabled={saving || !canSave}
            >
              {saving
                ? t("保存中", "Saving")
                : selectedButton === "Voice"
                  ? t("保存语音设置", "Save voice settings")
                  : t("保存映射", "Save mapping")}
            </Button>
          </>
        }
      />

      <div className="mapping-layout">
        <section className="panel mapping-remote-panel">
          <div className="panel-head">
            <div>
              <p className="panel-label">{t("实体遥控器", "Physical remote")}</p>
              <h2 className="panel-title" style={{ marginTop: 6 }}>
                {t("点击按键开始编辑", "Select a key to edit")}
              </h2>
            </div>
            <StatusPill tone={dirty ? "amber" : "signal"}>
              {dirty ? t("未保存", "UNSAVED") : t("已同步", "SYNCED")}
            </StatusPill>
          </div>
          <div className="mapping-remote-stage">
            <RemoteIllustration
              className="mapping-remote-visual"
              selectedButton={selectedButton}
              pressedButtons={pressedButtons}
              voiceActive={voiceActive}
              onButtonSelect={chooseRemoteButton}
            />
          </div>
          <div
            className="mapping-live-input"
            data-active={lastInput?.phase === "down"}
            aria-live="polite"
          >
            <span className="mapping-live-dot" aria-hidden="true" />
            {lastInput
              ? t(
                  `最近按键：${buttonLabel(lastInput.button, language)} · ${
                    lastInput.phase === "down" ? "按下" : "松开"
                  } · ${lastInput.source}`,
                  `Last key: ${buttonLabel(lastInput.button, language)} · ${
                    lastInput.phase === "down" ? "pressed" : "released"
                  } · ${lastInput.source}`,
                )
              : t(
                  "等待遥控器按键；实体按键按下时，虚拟遥控器会同步亮起。",
                  "Waiting for input. Physical presses light up the matching virtual key.",
                )}
          </div>
          <div className="mapping-picker-copy">
            <span className="mapping-picker-dot" aria-hidden="true" />
            <span>
              {selectedButton === "Voice"
                ? t(
                    "语音键独立配置输入法、触发方式与快捷键，不参与普通手势映射。",
                    "The Voice key has its own input tool, trigger mode, and shortcut. It does not use normal gestures.",
                  )
                : current
                  ? t(
                      `当前：${selectedLabel}。点击其他实体按键即可切换，未保存的修改会先确认。`,
                      `Selected: ${selectedLabel}. Choose another key to switch; unsaved changes are confirmed first.`,
                    )
                  : t(
                      "点击方向键、主页、返回、音量或其他实体按键开始编辑。",
                      "Choose a direction, Home, Back, volume, or another key to start editing.",
                    )}
            </span>
          </div>
        </section>

        <section className="panel mapping-editor-panel">
          <div className="panel-head">
            <div>
              <p className="panel-label">{t("按键动作", "Key actions")}</p>
              <h2 className="panel-title" style={{ marginTop: 6 }}>
                {selectedLabel}
              </h2>
            </div>
            <StatusPill tone="neutral">
              {selectedButton === "Voice" ? "VOICE" : current?.button ?? "NONE"}
            </StatusPill>
          </div>

          <div className="mapping-editor-scroll">
          {selectedButton === "Voice" ? (
            voiceDraft ? (
              <div className="voice-map-editor">
                <div className="voice-map-intro">
                  <span className="voice-map-icon">
                    <Mic size={18} />
                  </span>
                  <div>
                    <p className="voice-map-title">{t("语音输入链路", "Voice input path")}</p>
                    <p className="voice-map-sub">
                      {t(
                        "按下语音键后，应用向输入法发送已配置的快捷键，同时把遥控器音频写入 CABLE Input。",
                        "When the Voice key is pressed, the app sends the configured shortcut to the input tool and writes remote audio to CABLE Input.",
                      )}
                    </p>
                  </div>
                </div>
                <div className="control-stack">
                  <label className="field">
                    <span className="field-label">{t("输入法", "Input tool")}</span>
                    <select
                      className="select"
                      value={voiceDraft.inputTool}
                      onChange={(event) =>
                        setVoiceDraft({
                          ...voiceDraft,
                          inputTool: event.target.value,
                        })
                      }
                    >
                      <option value="WeChatInput">
                        {t("微信输入法", "WeChat Input")}
                      </option>
                      <option value="SogouInput">
                        {t("搜狗输入法", "Sogou Input")}
                      </option>
                      <option value="Custom">{t("自定义", "Custom")}</option>
                    </select>
                  </label>
                  <label className="field">
                    <span className="field-label">{t("触发方式", "Trigger mode")}</span>
                    <select
                      className="select"
                      value={voiceDraft.triggerMode}
                      onChange={(event) =>
                        setVoiceDraft({
                          ...voiceDraft,
                          triggerMode: event.target.value,
                        })
                      }
                    >
                      <option value="HoldToTalk">
                        {t("按住说话", "Hold to talk")}
                      </option>
                      <option value="ClickToToggle">
                        {t("点击切换", "Click to toggle")}
                      </option>
                      <option value="SmartToggle">
                        {t("智能免提", "Smart hands-free")}
                      </option>
                    </select>
                  </label>
                  <label className="field">
                    <span className="field-label">{t("语音快捷键", "Voice shortcut")}</span>
                    <ShortcutInput
                      value={voiceDraft.shortcut}
                      onChange={(shortcut) =>
                        setVoiceDraft({ ...voiceDraft, shortcut })
                      }
                    />
                    <span className="shortcut-hint">
                      {t(
                        "点击输入框后直接按组合键；每次语音触发都会读取最新设置。",
                        "Click the field and press a shortcut. Every voice trigger reads the latest value.",
                      )}
                    </span>
                  </label>
                  <div className="voice-map-route">
                    <span>{t("音频输出", "Audio output")}</span>
                    <strong>{voiceDraft.audioOutput}</strong>
                    <span>{t("输入法读取", "Input tool input")}</span>
                    <strong>{voiceDraft.audioInput}</strong>
                  </div>
                  <Button
                    variant="signal"
                    icon={<Save size={14} />}
                    onClick={save}
                    disabled={saving || !voiceDirty}
                  >
                    {saving
                      ? t("保存中", "Saving")
                      : t("保存语音键设置", "Save voice key settings")}
                  </Button>
                </div>
                <p className="voice-map-note">
                  {t(
                    "音频端点可在“语音”页调整。语音键使用独立的 ATVV 触发通道，不占用普通按键映射。",
                    "Audio endpoints can be changed on the Voice page. The Voice key uses its own ATVV trigger path and does not consume a normal key mapping.",
                  )}
                </p>
              </div>
            ) : (
              <div className="empty-note">
                {t("语音设置尚未加载。", "Voice settings are not loaded yet.")}
              </div>
            )
          ) : mappings.length === 0 ? (
            <div className="empty-note">
              {t("正在载入该设备的按键映射…", "Loading mappings for this device…")}
            </div>
          ) : draft ? (
            <>
              <div className="mapping-action-grid">
                {gestureKeys.map((key) => (
                  <ActionEditor
                    key={key}
                    gesture={key}
                    action={draft[key]}
                    longPressMode={draft.longPressMode}
                    onChangeType={changeActionType}
                    onChangeShortcut={updateShortcut}
                    onChangeText={updateText}
                    onChangeMedia={updateMedia}
                    onChangeRepeat={updateRepeat}
                    onChangeLaunchApp={updateLaunchApp}
                    onChangeLongPressMode={updateLongPressMode}
                  />
                ))}
              </div>
              <div className="mapping-rule-note">
                <ShieldCheck size={15} />
                <span>
                  {t(
                    "长按默认在约 700 ms 后执行一次。快捷键和媒体键可选择“按住保持、松开释放”；方向键和音量键仍可单独设为连续重复。",
                    "By default a long press runs once after about 700 ms. Shortcuts and media keys can instead stay pressed until release; direction and volume keys can still use continuous repeat.",
                  )}
                </span>
              </div>
            </>
          ) : (
            <div className="empty-note">
              {t("选择一个按键后即可编辑。", "Select a key to edit its actions.")}
            </div>
          )}
          </div>
        </section>
      </div>
    </div>
  );
}

interface ActionEditorProps {
  gesture: ActionKey;
  action: ActionConfig;
  longPressMode: LongPressMode;
  onChangeType: (key: ActionKey, type: ActionType) => void;
  onChangeShortcut: (key: ActionKey, keys: string) => void;
  onChangeText: (key: ActionKey, text: string) => void;
  onChangeMedia: (key: ActionKey, control: MediaControl) => void;
  onChangeRepeat: (key: ActionKey, interval: number) => void;
  onChangeLaunchApp: (key: ActionKey, path: string, name: string) => void;
  onChangeLongPressMode: (mode: LongPressMode) => void;
}

function ActionEditor({
  gesture,
  action,
  longPressMode: currentLongPressMode,
  onChangeType,
  onChangeShortcut,
  onChangeText,
  onChangeMedia,
  onChangeRepeat,
  onChangeLaunchApp,
  onChangeLongPressMode,
}: ActionEditorProps) {
  const { language, t } = useI18n();
  const [applications, setApplications] = useState<RunningApplication[]>([]);
  const [applicationListState, setApplicationListState] = useState<
    "idle" | "loading" | "ready" | "error"
  >("idle");
  const [applicationListError, setApplicationListError] = useState("");
  const Icon = gestureIcons[gesture];
  const supportsHold =
    gesture === "longPress" &&
    (action.type === "shortcut" || action.type === "media");

  const refreshApplications = useCallback(() => {
    if (!isTauriRuntime()) {
      setApplications([]);
      setApplicationListState("error");
      setApplicationListError(
        t(
          "运行中的应用列表仅在桌面版可用，可直接填写 EXE 路径。",
          "The running-app list is available in the desktop app. You can still enter an EXE path.",
        ),
      );
      return;
    }
    setApplicationListState("loading");
    setApplicationListError("");
    void listRunningApplications()
      .then((items) => {
        setApplications(items);
        setApplicationListState("ready");
      })
      .catch((error: unknown) => {
        setApplications([]);
        setApplicationListState("error");
        setApplicationListError(String(error));
      });
  }, [t]);

  useEffect(() => {
    if (action.type === "launchApp" && applicationListState === "idle") {
      refreshApplications();
    }
  }, [action.type, applicationListState, refreshApplications]);

  const selectApplication = (path: string) => {
    const selected = applications.find((application) => application.path === path);
    if (selected) {
      onChangeLaunchApp(gesture, selected.path, selected.name);
    }
  };
  return (
    <article className="action-card" data-gesture={gesture}>
      <div className="action-card-head">
        <span className="action-card-icon">
          <Icon size={15} />
        </span>
        <div className="action-card-copy">
          <p className="action-card-title">{gestureLabel(gesture, language)}</p>
          <p className="action-card-summary">
            {summarizeAction(action, language)}
          </p>
        </div>
        <Keyboard size={14} className="action-card-keyboard" />
      </div>
      <div className="control-stack action-card-controls">
        <select
          className="select"
          aria-label={t(
            `${gestureLabel(gesture, language)}动作类型`,
            `${gestureLabel(gesture, "en")} action type`,
          )}
          value={action.type}
          onChange={(event) =>
            onChangeType(gesture, event.target.value as ActionType)
          }
        >
          {actionTypes.map((value) => (
            <option key={value} value={value}>
              {actionTypeLabel(value, language)}
            </option>
          ))}
        </select>
        {action.type === "shortcut" ? (
          <ShortcutInput
            value={action.keys}
            placeholder={t("点击后按下组合键", "Click, then press keys")}
            onChange={(keys) => onChangeShortcut(gesture, keys)}
          />
        ) : null}
        {action.type === "text" ? (
          <input
            className="select"
            value={action.text}
            placeholder={t("要输入的文本", "Text to type")}
            onChange={(event) => onChangeText(gesture, event.target.value)}
          />
        ) : null}
        {action.type === "media" ? (
          <select
            className="select"
            aria-label={t("媒体键", "Media key")}
            value={action.control}
            onChange={(event) =>
              onChangeMedia(gesture, event.target.value as MediaControl)
            }
          >
            {(["volumeUp", "volumeDown", "mute", "playPause"] as MediaControl[]).map(
              (value) => (
                <option key={value} value={value}>
                  {mediaLabel(value, language)}
                </option>
              ),
            )}
          </select>
        ) : null}
        {action.type === "launchApp" ? (
          <div className="application-action">
            <div className="field">
              <div className="application-action-head">
                <span className="field-label">
                  <AppWindow size={13} />
                  {t("运行中的应用", "Running applications")}
                </span>
                <Button
                  type="button"
                  size="sm"
                  variant="ghost"
                  icon={<RefreshCw size={12} />}
                  disabled={applicationListState === "loading"}
                  onClick={refreshApplications}
                >
                  {applicationListState === "loading"
                    ? t("读取中", "Loading")
                    : t("刷新", "Refresh")}
                </Button>
              </div>
              <select
                className="select"
                aria-label={t("选择运行中的应用", "Choose a running application")}
                value={
                  applications.some(
                    (application) => application.path === action.path,
                  )
                    ? action.path
                    : ""
                }
                disabled={applicationListState === "loading"}
                onChange={(event) => selectApplication(event.target.value)}
              >
                <option value="">
                  {t("选择要切换到的应用…", "Choose an app to switch to…")}
                </option>
                {applications.map((application) => (
                  <option key={application.path} value={application.path}>
                    {application.name}
                    {application.isForeground
                      ? t(" · 当前前台", " · foreground")
                      : ""}
                    {" — "}
                    {application.windowTitle}
                  </option>
                ))}
              </select>
              {applicationListState === "error" ? (
                <span className="application-list-note" data-tone="error">
                  {applicationListError}
                </span>
              ) : applicationListState === "ready" && applications.length === 0 ? (
                <span className="application-list-note">
                  {t(
                    "没有找到可见的应用窗口，可直接填写下方 EXE 路径。",
                    "No visible app windows were found. Enter an EXE path below.",
                  )}
                </span>
              ) : (
                <span className="application-list-note">
                  {t(
                    "选择已运行的应用会切到前台；应用未运行时会启动它。",
                    "A running app is brought to the foreground; otherwise it is launched.",
                  )}
                </span>
              )}
            </div>
            <label className="field">
              <span className="field-label">{t("EXE 路径", "EXE path")}</span>
              <input
                className="select"
                value={action.path}
                spellCheck={false}
                placeholder="C:\\Program Files\\Tencent\\Weixin\\Weixin.exe"
                onChange={(event) =>
                  onChangeLaunchApp(gesture, event.target.value, action.name)
                }
              />
            </label>
            <label className="field">
              <span className="field-label">
                {t("显示名称（可选）", "Display name (optional)")}
              </span>
              <input
                className="select"
                value={action.name}
                placeholder={t("例如：微信", "For example: WeChat")}
                onChange={(event) =>
                  onChangeLaunchApp(gesture, action.path, event.target.value)
                }
              />
            </label>
          </div>
        ) : null}
        {action.type === "continuousRepeat" ? (
          <label className="field">
            <span className="field-label">
              {t("重复间隔", "Repeat interval")} · {action.intervalMs} ms
            </span>
            <input
              className="range"
              type="range"
              min="20"
              max="500"
              step="10"
              value={action.intervalMs}
              onChange={(event) =>
                onChangeRepeat(gesture, Number(event.target.value))
              }
            />
          </label>
        ) : null}
        {supportsHold ? (
          <label className="field">
            <span className="field-label">{t("长按行为", "Long-press behavior")}</span>
            <select
              className="select"
              aria-label={t("长按行为", "Long-press behavior")}
              value={currentLongPressMode}
              onChange={(event) =>
                onChangeLongPressMode(event.target.value as LongPressMode)
              }
            >
              <option value="oneShot">
                {t("按住约 700 ms，执行一次", "Run once after about 700 ms")}
              </option>
              <option value="hold">
                {t("按住保持，松开释放", "Hold until the key is released")}
              </option>
            </select>
          </label>
        ) : null}
        <span className="action-card-hint">
          {gesture === "click"
            ? t(
                "松开后触发；最常用的动作放在这里。",
                "Runs after release. Put the most common action here.",
              )
            : gesture === "doubleClick"
              ? t(
                  "350 ms 内按两次触发；启用后单击会等待判定。",
                  "Press twice within 350 ms. When enabled, single click waits for the window.",
                )
              : supportsHold && currentLongPressMode === "hold"
                ? t(
                    "按住约 700 ms 后发送按键按下，直到松开遥控器才发送释放；不会连点。",
                    "After about 700 ms the key goes down and stays down until the remote is released; no repeated clicks.",
                  )
                : t(
                    "按住约 700 ms 触发一次，不再连点。",
                    "Runs once after about 700 ms and does not repeat clicks.",
                  )}
        </span>
      </div>
    </article>
  );
}

function actionForType(type: ActionType, previous: ActionConfig): ActionConfig {
  switch (type) {
    case "none":
      return { type: "none" };
    case "shortcut":
      return {
        type: "shortcut",
        keys: previous.type === "shortcut" ? previous.keys : "Enter",
      };
    case "text":
      return {
        type: "text",
        text: previous.type === "text" ? previous.text : "",
      };
    case "media":
      return {
        type: "media",
        control: previous.type === "media" ? previous.control : "playPause",
      };
    case "launchApp":
      return {
        type: "launchApp",
        path: previous.type === "launchApp" ? previous.path : "",
        name: previous.type === "launchApp" ? previous.name : "",
      };
    case "continuousRepeat":
      return {
        type: "continuousRepeat",
        intervalMs:
          previous.type === "continuousRepeat" ? previous.intervalMs : 80,
      };
  }
}
