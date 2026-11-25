import { useEffect, useState } from "react";
import { AudioLines, Cable, Mic, Play, Save } from "lucide-react";
import { Button } from "../components/Button";
import { CableSetupGuide } from "../components/CableSetupGuide";
import { PageHeader } from "../components/PageHeader";
import { StatusPill } from "../components/StatusPill";
import { Waveform } from "../components/Waveform";
import { useAppStore } from "../store";
import { formatDbfs } from "../lib/audio-level";
import {
  getAppState,
  saveSettings,
  saveVoiceSettings,
  testAudioOutput,
} from "../lib/ipc";
import {
  inputToolLabel,
  shortcutLabel,
  triggerModeLabel,
  voiceStatus,
} from "../lib/presentation";
import { useI18n } from "../lib/i18n";

export function VoicePage() {
  const { language, t } = useI18n();
  const devices = useAppStore((state) => state.devices);
  const selectedDeviceId = useAppStore((state) => state.selectedDeviceId);
  const settings = useAppStore((state) => state.settings);
  const audio = useAppStore((state) => state.audio);
  const voiceActive = useAppStore((state) => state.voiceActive);
  const voiceHandsFree = useAppStore((state) => state.voiceHandsFree);
  const voicePhase = useAppStore((state) => state.voicePhase);
  const voiceError = useAppStore((state) => state.voiceError);
  const voiceLevel = useAppStore((state) => state.voiceLevel);
  const voiceLevelHistory = useAppStore((state) => state.voiceLevelHistory);
  const voiceGain = useAppStore((state) => state.voiceGain);
  const setVoiceGain = useAppStore((state) => state.setVoiceGain);
  const hydrate = useAppStore((state) => state.hydrate);
  const setBackendError = useAppStore((state) => state.setBackendError);
  const showToast = useAppStore((state) => state.showToast);
  const device =
    devices.find((item) => item.id === selectedDeviceId) ?? devices[0];
  const [output, setOutput] = useState("CABLE Input");
  const [input, setInput] = useState("CABLE Output");
  const [busy, setBusy] = useState(false);
  const audioReady = audio?.state === "ready";
  const status = voiceStatus(voiceActive, audioReady, voicePhase, language);
  const endpointsDirty =
    !!settings &&
    (settings.audioOutput !== output || settings.audioInput !== input);

  useEffect(() => {
    if (settings) {
      setOutput(settings.audioOutput);
      setInput(settings.audioInput);
    }
  }, [settings?.audioOutput, settings?.audioInput]);

  const saveDevices = () => {
    if (!settings) return;
    setBusy(true);
    void saveSettings({
      ...settings,
      audioOutput: output,
      audioInput: input,
    })
      .then((result) => {
        hydrate(result.snapshot);
        showToast(
          result.warning
            ? result.warning
            : t(
                "音频端点已保存，将在下一次语音会话生效。",
                "Audio endpoints saved for the next voice session.",
              ),
          result.warning ? "error" : "success",
        );
      })
      .catch((error: unknown) => setBackendError(String(error)))
      .finally(() => setBusy(false));
  };

  const commitGain = () => {
    if (!device) return;
    void saveVoiceSettings(device.id, {
      gain: voiceGain,
      voiceMode: device.voiceMode,
    })
      .then((snapshot) => {
        hydrate(snapshot);
        showToast(t("语音增益已保存。", "Voice gain saved."));
      })
      .catch((error: unknown) => setBackendError(String(error)));
  };

  const runAudioTest = () => {
    setBusy(true);
    void testAudioOutput()
      .then((deviceName) =>
        showToast(
          t(
            `测试音已发送到 ${deviceName}。`,
            `Test tone sent to ${deviceName}.`,
          ),
        ),
      )
      .catch((error: unknown) => setBackendError(String(error)))
      .finally(() => setBusy(false));
  };

  const refreshAudio = () => {
    setBusy(true);
    void getAppState()
      .then((snapshot) => {
        hydrate(snapshot);
        showToast(t("音频端点已重新检查。", "Audio endpoints checked again."));
      })
      .catch((error: unknown) => setBackendError(String(error)))
      .finally(() => setBusy(false));
  };

  return (
    <div className="page">
      <PageHeader
        eyebrow={t("语音链路", "Voice route")}
        title={t("让遥控器成为麦克风", "Turn the remote into a microphone")}
        subtitle={t(
          "ATVV 音频会解码为 16 kHz 单声道，再重采样到所选输出端点。语音内容不会落盘。",
          "ATVV audio is decoded to 16 kHz mono and resampled to the selected output endpoint. Voice content is never written to disk.",
        )}
        actions={
          <Button
            variant="signal"
            icon={<Play size={13} />}
            onClick={runAudioTest}
            disabled={busy}
          >
            {busy ? t("检测中", "Checking") : t("测试输出", "Test output")}
          </Button>
        }
      />

      {voiceError ? (
        <div className="error-banner" role="alert">
          {voiceError}
        </div>
      ) : null}

      {!audioReady ? (
        <CableSetupGuide
          audio={audio}
          busy={busy}
          onRecheck={refreshAudio}
          className="install-guide--lead"
        />
      ) : null}

      <div className="voice-hero">
        <section className="wave-panel">
          <div className="wave-panel-head">
            <div>
              <p className="panel-label">{t("实时麦克风", "Live microphone")}</p>
              <h2 className="panel-title">
                {voiceActive
                  ? voiceHandsFree
                    ? t("免提收音中", "Hands-free capture")
                    : t("正在采集", "Capturing")
                  : t("等待语音键", "Waiting for Voice")}
              </h2>
            </div>
            <StatusPill tone={status.tone} pulse={voiceActive}>
              {status.label}
            </StatusPill>
          </div>
          <Waveform active={voiceActive} levels={voiceLevelHistory} />
          <div className="wave-readout" aria-live="polite">
            <span>
              <b>RMS</b> {formatDbfs(voiceLevel.rms)}
            </span>
            <span>
              <b>PEAK</b> {formatDbfs(voiceLevel.peak)}
            </span>
          </div>
          <div className="wave-panel-meta">
            <span>16 kHz · MONO · ADPCM</span>
            <span>
              {voiceActive
                ? t("正在传输", "streaming")
                : t("预缓冲 120 ms", "preroll 120 ms")}
            </span>
          </div>
        </section>

        <section className="panel">
          <div className="panel-head">
            <div>
              <p className="panel-label">{t("路由设置", "Route settings")}</p>
              <h2 className="panel-title" style={{ marginTop: 6 }}>
                {t("音频端点", "Audio endpoints")}
              </h2>
            </div>
            <Cable size={17} color="var(--signal-strong)" />
          </div>
          <div className="control-stack">
            <label className="field">
              <span className="field-label">
                {t("输出到 CABLE Input", "Output to CABLE Input")}
              </span>
              <select
                className="select"
                value={output}
                onChange={(event) => setOutput(event.target.value)}
              >
                {!audio?.devices.includes(output) ? (
                  <option>{output}</option>
                ) : null}
                {audio?.devices.length ? (
                  audio.devices.map((name) => (
                    <option key={name}>{name}</option>
                  ))
                ) : (
                  <option>{output}</option>
                )}
              </select>
            </label>
            <label className="field">
              <span className="field-label">
                {t("输入法读取 CABLE Output", "Input tool reads CABLE Output")}
              </span>
              <select
                className="select"
                value={input}
                onChange={(event) => setInput(event.target.value)}
              >
                {!audio?.inputDevices.includes(input) ? (
                  <option>{input}</option>
                ) : null}
                {audio?.inputDevices.length ? (
                  audio.inputDevices.map((name) => (
                    <option key={name}>{name}</option>
                  ))
                ) : (
                  <option>{input}</option>
                )}
              </select>
            </label>
            <label className="field">
              <span className="field-label">
                {t("增益", "Gain")} · {voiceGain.toFixed(2)}×
              </span>
              <input
                className="range"
                type="range"
                min="0.5"
                max="8"
                step="0.05"
                value={voiceGain}
                onChange={(event) => setVoiceGain(Number(event.target.value))}
                onPointerUp={commitGain}
                onKeyUp={commitGain}
              />
            </label>
            <div className="route-node" style={{ paddingTop: 8 }}>
              <span className="route-index">
                <Mic size={12} />
              </span>
              <div className="route-copy">
                <p className="route-title">
                  {audioReady
                    ? t("VB-CABLE 已就绪", "VB-CABLE is ready")
                    : t("音频端点需要检查", "Audio endpoints need attention")}
                </p>
                <p className="route-sub">
                  {audio?.detail ?? t("尚未读取音频状态", "Audio status not loaded")}
                </p>
              </div>
              <StatusPill tone={audioReady ? "signal" : "amber"}>
                {audioReady ? t("已就绪", "READY") : t("安装", "INSTALL")}
              </StatusPill>
            </div>
            <div className="route-node">
              <span className="route-index">
                <AudioLines size={12} />
              </span>
              <div className="route-copy">
                <p className="route-title">
                  {inputToolLabel(settings?.inputTool, language)}
                </p>
                <p className="route-sub">
                  {settings
                    ? `${triggerModeLabel(settings.triggerMode, language)} · ${shortcutLabel(settings.shortcut, language)}`
                    : t("输入法配置尚未加载", "Input tool configuration is not loaded")}
                </p>
              </div>
              <StatusPill tone="neutral">{t("已绑定", "BOUND")}</StatusPill>
            </div>
            <Button
              variant="primary"
              icon={<Save size={14} />}
              onClick={saveDevices}
              disabled={!settings || busy || !endpointsDirty}
            >
              {endpointsDirty
                ? t("保存音频端点", "Save audio endpoints")
                : t("端点已保存", "Endpoints saved")}
            </Button>
          </div>
        </section>
      </div>

      {audioReady ? (
        <CableSetupGuide audio={audio} busy={busy} onRecheck={refreshAudio} />
      ) : null}
    </div>
  );
}
