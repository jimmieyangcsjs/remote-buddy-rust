import type {
  ActionConfig,
  ConnectionState,
  MediaControl,
} from "./types";
import type { UiLanguage } from "./i18n";

const inputToolLabels: Record<string, string> = {
  WeChatInput: "微信输入法",
  SogouInput: "搜狗输入法",
  Custom: "自定义输入法",
};

const triggerModeLabels: Record<string, string> = {
  HoldToTalk: "按住说话",
  ClickToToggle: "点击切换",
  SmartToggle: "智能免提",
};

const buttonLabelsZh: Record<string, string> = {
  Up: "向上",
  Down: "向下",
  Left: "向左",
  Right: "向右",
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
  LiveTv: "直播电视",
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
  LiveTv: "Live TV",
  Input: "Input",
  Voice: "Voice key",
  Assistant: "Voice assistant",
  Custom: "Custom",
};

const mediaLabelsZh: Record<MediaControl, string> = {
  volumeUp: "音量 +",
  volumeDown: "音量 -",
  mute: "静音",
  playPause: "播放 / 暂停",
};

const mediaLabelsEn: Record<MediaControl, string> = {
  volumeUp: "Volume up",
  volumeDown: "Volume down",
  mute: "Mute",
  playPause: "Play / pause",
};

export function inputToolLabel(
  value: string | null | undefined,
  language: UiLanguage = "zh",
): string {
  const tool = value?.trim();
  if (!tool) return language === "en" ? "Input tool not loaded" : "输入法未加载";
  if (language === "en") {
    return (
      {
        WeChatInput: "WeChat Input",
        SogouInput: "Sogou Input",
        Custom: "Custom input tool",
      }[tool] ?? tool
    );
  }
  return inputToolLabels[tool] ?? tool;
}

export function triggerModeLabel(
  value: string | null | undefined,
  language: UiLanguage = "zh",
): string {
  const mode = value?.trim();
  if (!mode) return language === "en" ? "Trigger mode not set" : "触发方式未设置";
  if (language === "en") {
    return (
      {
        HoldToTalk: "Hold to talk",
        ClickToToggle: "Click to toggle",
        SmartToggle: "Smart hands-free",
      }[mode] ?? mode
    );
  }
  return triggerModeLabels[mode] ?? mode;
}

export function shortcutLabel(
  value: string | null | undefined,
  language: UiLanguage = "zh",
): string {
  const shortcut = value?.trim();
  return shortcut || (language === "en" ? "Shortcut not set" : "快捷键未设置");
}

export function buttonLabel(
  value: string | null | undefined,
  language: UiLanguage = "zh",
): string {
  const button = value?.trim() ?? "";
  if (!button) return language === "en" ? "No key selected" : "未选择按键";
  const labels = language === "en" ? buttonLabelsEn : buttonLabelsZh;
  return labels[button] ?? button;
}

export function gestureLabel(
  value: "click" | "doubleClick" | "longPress",
  language: UiLanguage = "zh",
): string {
  if (language === "en") {
    return {
      click: "Single click",
      doubleClick: "Double click",
      longPress: "Long press",
    }[value];
  }
  return { click: "单击", doubleClick: "双击", longPress: "长按" }[value];
}

export function actionTypeLabel(
  value: ActionConfig["type"],
  language: UiLanguage = "zh",
): string {
  if (language === "en") {
    return {
      none: "No action",
      shortcut: "Shortcut",
      text: "Text",
      media: "Media key",
      launchApp: "Switch app",
      continuousRepeat: "Continuous repeat",
    }[value];
  }
  return {
    none: "无动作",
    shortcut: "快捷键",
    text: "文本",
    media: "媒体键",
    launchApp: "切换应用",
    continuousRepeat: "连续重复",
  }[value];
}

export function mediaLabel(
  value: MediaControl,
  language: UiLanguage = "zh",
): string {
  return (language === "en" ? mediaLabelsEn : mediaLabelsZh)[value];
}

export function summarizeAction(
  action: ActionConfig,
  language: UiLanguage = "zh",
): string {
  switch (action.type) {
    case "none":
      return actionTypeLabel("none", language);
    case "shortcut":
      return action.keys || shortcutLabel("", language);
    case "text":
      return (
        action.text.trim() ||
        (language === "en" ? "Empty text" : "空文本")
      );
    case "media":
      return mediaLabel(action.control, language);
    case "launchApp": {
      const name =
        action.name.trim() ||
        action.path
          .trim()
          .replace(/^["']|["']$/g, "")
          .split(/[\\/]/)
          .pop()
          ?.replace(/\.exe$/i, "") ||
        "";
      return name || (language === "en" ? "Choose an app" : "选择应用");
    }
    case "continuousRepeat":
      return `${action.intervalMs} ms`;
  }
}

export interface ConnectionPresentation {
  title: string;
  subtitle: string;
  badge: string;
  tone: "signal" | "amber" | "coral" | "neutral";
  pulse: boolean;
  sidebar: string;
  titlebar: string;
}

export function connectionPresentation(
  state: ConnectionState,
  deviceName?: string,
  lastError?: string | null,
  language: UiLanguage = "zh",
): ConnectionPresentation {
  const name = deviceName?.trim() || (language === "en" ? "remote" : "遥控器");
  if (language === "en") {
    switch (state) {
      case "ready":
        return {
          title: "Signal ready",
          subtitle: `${name} is connected. Check each metric for button capture, voice, and audio.`,
          badge: "CONNECTED",
          tone: "signal",
          pulse: true,
          sidebar: "ONLINE",
          titlebar: "bridge online",
        };
      case "connecting":
        return {
          title: "Connecting remote",
          subtitle: `Connecting to ${name} over Windows Bluetooth. Keep the remote nearby and awake.`,
          badge: "CONNECTING",
          tone: "amber",
          pulse: true,
          sidebar: "SYNC",
          titlebar: "connecting",
        };
      case "degraded":
        return {
          title: "Connection needs attention",
          subtitle:
            lastError?.trim() ||
            `${name} is temporarily unavailable. One controlled recovery may be attempted; otherwise reconnect manually.`,
          badge: "ATTENTION",
          tone: "amber",
          pulse: false,
          sidebar: "RETRY",
          titlebar: "connection needs attention",
        };
      case "disconnected":
      default:
        return {
          title: "Remote not connected",
          subtitle:
            `There is no active connection. ${name === "remote" ? "Scan for and select a paired remote first." : `Select ${name} and click Connect.`}`,
          badge: "OFFLINE",
          tone: "coral",
          pulse: false,
          sidebar: "OFFLINE",
          titlebar: "not connected",
        };
    }
  }
  switch (state) {
    case "ready":
      return {
        title: "信号已就绪",
        subtitle: `${name} 已连接；按键采集、语音和音频链路状态以各指标为准。`,
        badge: "已连接",
        tone: "signal",
        pulse: true,
        sidebar: "在线",
        titlebar: "桥接在线",
      };
    case "connecting":
      return {
        title: "正在连接遥控器",
        subtitle: `正在通过 Windows 蓝牙连接 ${name}，请保持遥控器在附近并处于唤醒状态。`,
        badge: "连接中",
        tone: "amber",
        pulse: true,
        sidebar: "同步",
        titlebar: "正在连接",
      };
    case "degraded":
      return {
        title: "连接需要处理",
        subtitle:
          lastError?.trim() ||
          `${name} 暂不可用。程序最多做一次受控恢复；失败后请回到设备页手动重连。`,
        badge: "需处理",
        tone: "amber",
        pulse: false,
        sidebar: "重试",
        titlebar: "连接需处理",
      };
    case "disconnected":
    default:
      return {
        title: "遥控器尚未连接",
        subtitle:
          `当前没有活动连接。${name === "遥控器" ? "请先扫描并选择已配对的遥控器。" : `选择 ${name} 后点击连接。`}`,
        badge: "未连接",
        tone: "coral",
        pulse: false,
        sidebar: "离线",
        titlebar: "未连接",
      };
  }
}

export function voiceStatus(
  active: boolean,
  audioReady: boolean,
  phase: string,
  language: UiLanguage = "zh",
): { label: string; tone: "signal" | "amber" | "neutral" } {
  if (active) {
    if (language === "en") {
      return { label: phase ? phase.toUpperCase() : "STREAMING", tone: "signal" };
    }
    const labels: Record<string, string> = {
      opening: "打开中",
      reopening: "重开麦克风",
      streaming: "传输中",
    };
    return { label: labels[phase] ?? "传输中", tone: "signal" };
  }
  if (!audioReady) {
    return {
      label: language === "en" ? "CHECK AUDIO" : "音频待检查",
      tone: "amber",
    };
  }
  return {
    label: language === "en" ? "STANDBY" : "待命",
    tone: "neutral",
  };
}

export function webViewRuntime(
  userAgent: string,
  language: UiLanguage = "zh",
): {
  detail: string;
  status: string;
  tone: "signal" | "neutral";
} {
  const edge = userAgent.match(/Edg\/([\d.]+)/i)?.[1];
  if (!edge) {
    return {
      detail:
        language === "en"
          ? "WebView2 is hosting the interface; the engine version is unavailable."
          : "WebView2 正在承载界面；浏览器内核版本未暴露。",
      status: "ACTIVE",
      tone: "neutral",
    };
  }
  return {
    detail: `Microsoft Edge WebView2 · Chromium ${edge.split(".")[0]}`,
    status: "ACTIVE",
    tone: "signal",
  };
}
