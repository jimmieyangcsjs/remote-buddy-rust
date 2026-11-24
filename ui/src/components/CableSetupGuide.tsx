import { Cable, Download, ExternalLink, RefreshCw } from "lucide-react";
import { Button } from "./Button";
import { StatusPill } from "./StatusPill";
import { openExternalUrl } from "../lib/ipc";
import type { AudioStatusView } from "../lib/types";
import { useAppStore } from "../store";
import { useI18n } from "../lib/i18n";

const CABLE_DOWNLOAD_URL = "https://vb-audio.com/Cable/";

interface CableSetupGuideProps {
  audio?: AudioStatusView | null;
  busy?: boolean;
  onRecheck?: () => void;
  className?: string;
}

export function CableSetupGuide({
  audio,
  busy = false,
  onRecheck,
  className = "",
}: CableSetupGuideProps) {
  const { t } = useI18n();
  const setBackendError = useAppStore((state) => state.setBackendError);
  const ready = audio?.state === "ready";
  const outputReady = audio?.outputReady ?? false;
  const inputReady = audio?.inputReady ?? false;

  const openDownload = () => {
    void openExternalUrl(CABLE_DOWNLOAD_URL).catch((error: unknown) =>
      setBackendError(String(error)),
    );
  };

  return (
    <section
      className={`panel install-guide ${className}`.trim()}
      data-ready={ready}
      aria-labelledby="cable-guide-title"
    >
      <div className="panel-head">
        <div className="install-guide-heading">
          <span className="install-guide-icon">
            <Cable size={18} />
          </span>
          <div>
            <p className="panel-label">{t("虚拟音频线", "Virtual audio cable")}</p>
            <h2
              className="panel-title"
              id="cable-guide-title"
              style={{ marginTop: 6 }}
            >
              {ready
                ? t("VB-CABLE 已就绪，可以开始路由", "VB-CABLE is ready for routing")
                : t("缺少 CABLE Input：先安装 VB-CABLE", "CABLE Input missing: install VB-CABLE first")}
            </h2>
          </div>
        </div>
        <StatusPill tone={ready ? "signal" : "amber"}>
          {ready ? t("已就绪", "READY") : t("需要安装", "SETUP REQUIRED")}
        </StatusPill>
      </div>

      {ready ? (
        <>
          <p className="guide-note">
            {t(
              "已检测到 CABLE Input 播放端点和 CABLE Output 录音端点。应用把遥控器语音写入前者，输入法从后者读取。保存端点后点击“测试输出”确认路由。",
              "CABLE Input and CABLE Output are both available. Remote audio is written to the playback endpoint and read by the input tool from the recording endpoint. Save the endpoints, then use Test output to confirm routing.",
            )}
          </p>
          <div
            className="guide-endpoints"
            aria-label={t("CABLE 端点状态", "CABLE endpoint status")}
          >
            <span data-ready={outputReady}>
              <strong>CABLE Input</strong>
              {t("播放端点", "Playback endpoint")} ·{" "}
              {outputReady ? t("已就绪", "Ready") : t("缺失", "Missing")}
            </span>
            <span data-ready={inputReady}>
              <strong>CABLE Output</strong>
              {t("录音端点", "Recording endpoint")} ·{" "}
              {inputReady ? t("已就绪", "Ready") : t("缺失", "Missing")}
            </span>
          </div>
        </>
      ) : (
        <>
          <p className="guide-note" style={{ marginBottom: 10 }}>
            {t(
              "Remote Buddy 不捆绑第三方驱动。请从 VB-Audio 官方页面下载并按下面顺序安装；没有该驱动时语音键无法送入输入法。",
              "Remote Buddy does not bundle third-party drivers. Download VB-CABLE from VB-Audio and follow these steps; without it, the voice key cannot reach an input tool.",
            )}
          </p>
          <ol className="setup-steps">
            <li>
              {t("打开", "Open the")}{" "}
              <button
                type="button"
                className="text-link"
                onClick={openDownload}
              >
                {t("VB-Audio 官方下载页", "official VB-Audio download page")}
                <ExternalLink size={12} />
              </button>
              {t(
                "，下载 VB-CABLE 压缩包并完整解压。",
                ", download the VB-CABLE archive, and extract it completely.",
              )}
            </li>
            <li>
              {t(
                "右键 VBCABLE_Setup_x64.exe，选择“以管理员身份运行”，点击 Install Driver。",
                "Right-click VBCABLE_Setup_x64.exe, choose Run as administrator, and select Install Driver.",
              )}
            </li>
            <li>
              {t(
                "按安装器提示重启 Windows，让 CABLE 音频端点完成注册。",
                "Restart Windows when prompted so the CABLE endpoints are registered.",
              )}
            </li>
            <li>
              {t(
                "重启后打开“声音设置 → 所有声音设备”，确认同时出现 CABLE Input（播放）和 CABLE Output（录音）。",
                "After restarting, open Sound settings → All sound devices and confirm both CABLE Input (playback) and CABLE Output (recording) are listed.",
              )}
            </li>
            <li>
              {t(
                "在“语音”页把输出选择为 CABLE Input。不要把它设为 Windows 默认播放设备，否则其他应用的日常声音也会进入虚拟麦克风。",
                "On the Voice page, select CABLE Input as the output. Do not make it the Windows default playback device, or ordinary system audio will also enter the virtual microphone.",
              )}
            </li>
            <li>
              {t(
                "微信输入法 → 语音输入设置：开启“快捷键全局生效”，选择“按住说话”，把麦克风设为 CABLE Output (VB-Audio Virtual Cable)。",
                "WeChat Input → Voice input settings: enable the global shortcut, choose Hold to talk, and set the microphone to CABLE Output (VB-Audio Virtual Cable).",
              )}
            </li>
            <li>
              {t(
                "回到本页点击“重新检测”，确认播放和录音两个端点都显示已就绪。",
                "Return here and click Check again until both endpoints show Ready.",
              )}
            </li>
          </ol>
          <div
            className="guide-endpoints"
            aria-label={t("CABLE 端点状态", "CABLE endpoint status")}
          >
            <span data-ready={outputReady}>
              <strong>CABLE Input</strong>
              {t("播放端点", "Playback endpoint")} ·{" "}
              {outputReady ? t("已就绪", "Ready") : t("待安装", "Setup required")}
            </span>
            <span data-ready={inputReady}>
              <strong>CABLE Output</strong>
              {t("录音端点", "Recording endpoint")} ·{" "}
              {inputReady ? t("已就绪", "Ready") : t("待安装", "Setup required")}
            </span>
          </div>
        </>
      )}

      <div className="guide-actions">
        <Button
          variant={ready ? "ghost" : "signal"}
          icon={ready ? <ExternalLink size={14} /> : <Download size={14} />}
          onClick={openDownload}
        >
          {ready
            ? t("打开 VB-CABLE 官网", "Open VB-CABLE website")
            : t("从官网下载 VB-CABLE", "Download VB-CABLE")}
        </Button>
        {onRecheck ? (
          <Button
            variant="ghost"
            icon={<RefreshCw size={14} />}
            onClick={onRecheck}
            disabled={busy}
          >
            {busy ? t("检测中", "Checking") : t("重新检测", "Check again")}
          </Button>
        ) : null}
        <span className="guide-note">
          {t("当前状态：", "Current status: ")}
          {audio?.detail ?? t("尚未检查音频端点", "Audio endpoints not checked")}
        </span>
      </div>
    </section>
  );
}
