import { describe, expect, it } from "vitest";
import {
  actionTypeLabel,
  buttonLabel,
  connectionPresentation,
  gestureLabel,
  inputToolLabel,
  shortcutLabel,
  summarizeAction,
  triggerModeLabel,
  voiceStatus,
  webViewRuntime,
} from "./presentation";

describe("connectionPresentation", () => {
  it("never claims a disconnected bridge is ready", () => {
    const copy = connectionPresentation("disconnected", "Chromecast Remote");
    expect(copy.title).toBe("遥控器尚未连接");
    expect(copy.badge).toBe("未连接");
    expect(copy.subtitle).not.toContain("已连接");
  });

  it("reports degraded connections with the actual backend error", () => {
    const copy = connectionPresentation(
      "degraded",
      "Chromecast Remote",
      "ATVV service not found",
    );
    expect(copy.title).toBe("连接需要处理");
    expect(copy.subtitle).toBe("ATVV service not found");
    expect(copy.sidebar).toBe("重试");
  });

  it("does not treat an ATVV connection as proof that every input path works", () => {
    const copy = connectionPresentation("ready", "Chromecast Remote");
    expect(copy.title).toBe("信号已就绪");
    expect(copy.subtitle).toContain("以各指标为准");
  });
});

describe("voiceStatus", () => {
  it("requires the audio endpoint to be ready before showing standby", () => {
    expect(voiceStatus(false, false, "idle")).toEqual({
      label: "音频待检查",
      tone: "amber",
    });
    expect(voiceStatus(false, true, "idle")).toEqual({
      label: "待命",
      tone: "neutral",
    });
  });

  it("can present the interface in English", () => {
    expect(voiceStatus(false, true, "idle", "en").label).toBe("STANDBY");
    expect(inputToolLabel("WeChatInput", "en")).toBe("WeChat Input");
    expect(connectionPresentation("ready", "Remote", null, "en").badge).toBe(
      "CONNECTED",
    );
  });
});

describe("settings presentation", () => {
  it("shows the Chinese input-tool name instead of its internal enum", () => {
    expect(inputToolLabel("WeChatInput")).toBe("微信输入法");
    expect(inputToolLabel("SogouInput")).toBe("搜狗输入法");
    expect(inputToolLabel("")).toBe("输入法未加载");
  });

  it("shows the saved shortcut and trigger mode rather than defaults", () => {
    expect(shortcutLabel("Ctrl+Shift+F8")).toBe("Ctrl+Shift+F8");
    expect(shortcutLabel("  ")).toBe("快捷键未设置");
    expect(triggerModeLabel("ClickToToggle")).toBe("点击切换");
  });
});

describe("mapping presentation", () => {
  it("localizes remote keys, gestures, and action summaries", () => {
    expect(buttonLabel("VolumeUp", "en")).toBe("Volume +");
    expect(gestureLabel("longPress", "en")).toBe("Long press");
    expect(actionTypeLabel("continuousRepeat", "en")).toBe(
      "Continuous repeat",
    );
    expect(
      summarizeAction({ type: "media", control: "mute" }, "en"),
    ).toBe("Mute");
    expect(actionTypeLabel("launchApp")).toBe("切换应用");
    expect(
      summarizeAction(
        {
          type: "launchApp",
          path: "C:\\Program Files\\Tencent\\Weixin\\Weixin.exe",
          name: "微信",
        },
        "zh",
      ),
    ).toBe("微信");
  });

  it("falls back to the executable name for app-switch summaries", () => {
    expect(
      summarizeAction({
        type: "launchApp",
        path: "C:\\Windows\\System32\\notepad.exe",
        name: "",
      }),
    ).toBe("notepad");
  });
});

describe("webViewRuntime", () => {
  it("derives the Chromium version from the real user agent", () => {
    expect(
      webViewRuntime(
        "Mozilla/5.0 Chrome/158.0.0.0 Safari/537.36 Edg/158.0.4370.33",
      ),
    ).toEqual({
      detail: "Microsoft Edge WebView2 · Chromium 158",
      status: "ACTIVE",
      tone: "signal",
    });
  });
});
