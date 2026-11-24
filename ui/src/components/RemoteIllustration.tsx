import type { KeyboardEvent, ReactNode } from "react";
import { useI18n } from "../lib/i18n";

interface RemoteIllustrationProps {
  className?: string;
  pressedButtons?: string[];
  voiceActive?: boolean;
  selectedButton?: string | null;
  onButtonSelect?: (button: string) => void;
  disabledButtons?: string[];
}

const CONTROL_X_LEFT = 80;
const CONTROL_X_RIGHT = 172;

function remoteButtonLabel(
  button: string,
  t: (zh: string, en: string) => string,
) {
  const labels: Record<string, [string, string]> = {
    Up: ["向上", "Up"],
    Down: ["向下", "Down"],
    Left: ["向左", "Left"],
    Right: ["向右", "Right"],
    Ok: ["确定", "OK"],
    Back: ["返回", "Back"],
    Voice: ["语音助手", "Voice assistant"],
    Home: ["主页", "Home"],
    Mute: ["静音", "Mute"],
    Power: ["电源", "Power"],
    Input: ["输入源", "Input"],
    VolumeUp: ["音量加", "Volume up"],
    VolumeDown: ["音量减", "Volume down"],
  };
  const label = labels[button];
  return label ? t(label[0], label[1]) : button;
}

export function RemoteIllustration({
  className = "",
  pressedButtons = [],
  voiceActive = false,
  selectedButton = null,
  onButtonSelect,
  disabledButtons = [],
}: RemoteIllustrationProps) {
  const { t } = useI18n();
  const pressed = new Set(
    pressedButtons.map((button) => button.toLowerCase()),
  );
  if (voiceActive) pressed.add("voice");
  const disabled = new Set(
    disabledButtons.map((button) => button.toLowerCase()),
  );
  const active = (...buttons: string[]) =>
    buttons.some((button) => pressed.has(button.toLowerCase()));
  const selectable = (button: string) =>
    disabled.has(button.toLowerCase()) ? undefined : onButtonSelect;

  return (
    <svg
      className={`remote-illustration ${className}`.trim()}
      viewBox="0 0 252 760"
      fill="none"
      role="group"
      aria-labelledby="remote-illustration-title remote-illustration-desc"
      preserveAspectRatio="xMidYMid meet"
    >
      <title id="remote-illustration-title">
        {t(
          "Chromecast with Google TV G9N9N 遥控器",
          "Chromecast with Google TV G9N9N remote",
        )}
      </title>
      <desc id="remote-illustration-desc">
        {t(
          "白色遥控器，顶部是大尺寸圆形方向盘，中央为确认键。下方依次是返回与黑色语音键、主页与静音、YouTube 与 Netflix、电源与输入源。右侧是音量加减拨键。",
          "White remote with a large circular directional pad and an OK button. Below are Back and Voice, Home and Mute, YouTube and Netflix, Power and Input, with volume controls on the right edge.",
        )}
      </desc>
      <defs>
        <linearGradient id="remote-body" x1="72" y1="20" x2="204" y2="742">
          <stop stopColor="#FFFFFF" />
          <stop offset="0.44" stopColor="#FCFCFA" />
          <stop offset="0.82" stopColor="#F4F3EF" />
          <stop offset="1" stopColor="#E8E7E2" />
        </linearGradient>
        <linearGradient id="remote-rim" x1="32" y1="86" x2="226" y2="676">
          <stop stopColor="#FFFFFF" />
          <stop offset="0.35" stopColor="#E4E3DE" />
          <stop offset="0.68" stopColor="#FAFAF7" />
          <stop offset="1" stopColor="#D2D1CC" />
        </linearGradient>
        <radialGradient
          id="remote-dpad"
          cx="0"
          cy="0"
          r="1"
          gradientTransform="translate(88 104) rotate(51) scale(136)"
        >
          <stop stopColor="#FFFFFF" />
          <stop offset="0.68" stopColor="#FCFCFA" />
          <stop offset="1" stopColor="#EFEEE9" />
        </radialGradient>
        <linearGradient id="remote-key" x1="58" y1="280" x2="196" y2="584">
          <stop stopColor="#FFFFFF" />
          <stop offset="0.72" stopColor="#FCFCFA" />
          <stop offset="1" stopColor="#F0EFEB" />
        </linearGradient>
        <linearGradient id="remote-volume" x1="235" y1="280" x2="235" y2="388">
          <stop stopColor="#F7F6F3" />
          <stop offset="0.5" stopColor="#EAE9E5" />
          <stop offset="1" stopColor="#D8D7D2" />
        </linearGradient>
        <radialGradient
          id="remote-aura"
          cx="0"
          cy="0"
          r="1"
          gradientTransform="translate(126 356) rotate(90) scale(365 92)"
        >
          <stop stopColor="#5B8CFF" stopOpacity="0.2" />
          <stop offset="1" stopColor="#5B8CFF" stopOpacity="0" />
        </radialGradient>
      </defs>

      <ellipse cx="126" cy="370" rx="105" ry="366" fill="url(#remote-aura)" />
      <ellipse cx="126" cy="735" rx="78" ry="16" fill="#020506" opacity="0.42" />
      <g>
        <path
          d="M126 10c55 0 102 37 102 104v532c0 68-44 106-102 106S24 714 24 646V114C24 47 71 10 126 10Z"
          fill="url(#remote-body)"
          stroke="url(#remote-rim)"
          strokeWidth="1.25"
        />
        <path
          d="M126 18c49 0 92 33 92 94v322"
          stroke="#FFFFFF"
          strokeOpacity="0.9"
          strokeWidth="2"
          strokeLinecap="round"
        />
        <path
          d="M39 594v48c0 41 24 72 61 84"
          stroke="#FFFFFF"
          strokeOpacity="0.5"
          strokeWidth="1.5"
          strokeLinecap="round"
        />

        <circle
          className="remote-pad-decor"
          cx="126"
          cy="151"
          r="100"
          fill="url(#remote-dpad)"
        />
        <circle
          className="remote-pad-decor"
          cx="126"
          cy="151"
          r="99.25"
          stroke="#CBCAC5"
          strokeWidth="0.9"
        />
        <circle
          className="remote-pad-decor"
          cx="126"
          cy="151"
          r="79.5"
          stroke="#E2E1DC"
          strokeWidth="0.55"
          strokeOpacity="0.72"
        />

        <RemoteKey
          button="Up"
          active={active("Up")}
          selectedButton={selectedButton}
          onSelect={selectable("Up")}
        >
          <path
            className="remote-direction-zone"
            d="M126 151 66 91a85 85 0 0 1 120 0Z"
          />
          <path
            className="remote-direction-hint"
            d="m119 111 7-7 7 7"
          />
        </RemoteKey>
        <RemoteKey
          button="Down"
          active={active("Down")}
          selectedButton={selectedButton}
          onSelect={selectable("Down")}
        >
          <path
            className="remote-direction-zone"
            d="M126 151 66 211a85 85 0 0 0 120 0Z"
          />
          <path
            className="remote-direction-hint"
            d="m119 191 7 7 7-7"
          />
        </RemoteKey>
        <RemoteKey
          button="Left"
          active={active("Left")}
          selectedButton={selectedButton}
          onSelect={selectable("Left")}
        >
          <path
            className="remote-direction-zone"
            d="M126 151 66 91a85 85 0 0 0 0 120Z"
          />
          <path
            className="remote-direction-hint"
            d="m101 144-7 7 7 7"
          />
        </RemoteKey>
        <RemoteKey
          button="Right"
          active={active("Right")}
          selectedButton={selectedButton}
          onSelect={selectable("Right")}
        >
          <path
            className="remote-direction-zone"
            d="M126 151 186 91a85 85 0 0 1 0 120Z"
          />
          <path
            className="remote-direction-hint"
            d="m151 144 7 7-7 7"
          />
        </RemoteKey>
        <RemoteKey
          button="Ok"
          active={active("Ok")}
          selectedButton={selectedButton}
          onSelect={selectable("Ok")}
        >
          <circle
            className="remote-button-base remote-ok-base"
            cx="126"
            cy="151"
            r="37"
            fill="#FCFCFA"
            stroke="#C8C7C2"
            strokeWidth="0.9"
          />
        </RemoteKey>

        <RemoteControlButton
          button="Back"
          cx={CONTROL_X_LEFT}
          cy={310}
          active={active("Back")}
          selectedButton={selectedButton}
          onSelect={selectable("Back")}
        >
          <path
            className="remote-key-icon"
            d="M89 310h-18m7-7-7 7 7 7"
          />
        </RemoteControlButton>
        <RemoteControlButton
          button="Voice"
          cx={CONTROL_X_RIGHT}
          cy={310}
          active={active("Voice", "Assistant")}
          selectedButton={selectedButton}
          onSelect={selectable("Voice")}
          dark
        >
          <circle
            className="remote-key-icon-fill"
            cx="166"
            cy="305"
            r="3.3"
            fill="#FFFFFF"
          />
          <circle
            className="remote-key-icon-fill"
            cx="177"
            cy="304"
            r="2.9"
            fill="#D8DDE1"
          />
          <circle
            className="remote-key-icon-fill"
            cx="170"
            cy="313"
            r="2.8"
            fill="#F1F3F4"
          />
          <circle
            className="remote-key-icon-fill"
            cx="179"
            cy="312"
            r="2.3"
            fill="#BAC1C7"
          />
        </RemoteControlButton>

        <RemoteControlButton
          button="Home"
          cx={CONTROL_X_LEFT}
          cy={389}
          active={active("Home")}
          selectedButton={selectedButton}
          onSelect={selectable("Home")}
        >
          <path
            className="remote-key-icon"
            d="m70 391 10-9 10 9v9H70v-9Z"
          />
          <path
            className="remote-key-icon"
            d="M77 400v-5h6v5"
            strokeWidth="1.3"
          />
        </RemoteControlButton>
        <RemoteControlButton
          button="Mute"
          cx={CONTROL_X_RIGHT}
          cy={389}
          active={active("Mute")}
          selectedButton={selectedButton}
          onSelect={selectable("Mute")}
        >
          <path
            className="remote-key-icon"
            d="M164 386h5l6-5v17l-6-5h-5v-7Z"
          />
          <path
            className="remote-key-icon"
            d="m177 386 7 7m0-7-7 7"
            strokeWidth="1.45"
          />
        </RemoteControlButton>

        <RemoteControlButton
          button="YouTube"
          cx={CONTROL_X_LEFT}
          cy={468}
          active={active("YouTube")}
          selectedButton={selectedButton}
          onSelect={selectable("YouTube")}
        >
          <text
            className="remote-key-wordmark"
            x={CONTROL_X_LEFT}
            y="471.5"
            fontSize="8.7"
          >
            YouTube
          </text>
        </RemoteControlButton>
        <RemoteControlButton
          button="Netflix"
          cx={CONTROL_X_RIGHT}
          cy={468}
          active={active("Netflix")}
          selectedButton={selectedButton}
          onSelect={selectable("Netflix")}
        >
          <text
            className="remote-key-wordmark"
            x={CONTROL_X_RIGHT}
            y="471.5"
            fontSize="8.2"
          >
            NETFLIX
          </text>
        </RemoteControlButton>

        <RemoteControlButton
          button="Power"
          cx={CONTROL_X_LEFT}
          cy={541}
          radius={25.5}
          active={active("Power")}
          selectedButton={selectedButton}
          onSelect={selectable("Power")}
        >
          <path
            className="remote-key-icon"
            d="M80 532v9m-6-3.6a8.4 8.4 0 1 0 12 0"
            strokeWidth="1.5"
          />
        </RemoteControlButton>
        <path
          className="remote-decor"
          d="M112 541h16"
          stroke="#898781"
          strokeWidth="1.7"
          strokeLinecap="round"
        />
        <RemoteControlButton
          button="Input"
          cx={CONTROL_X_RIGHT}
          cy={541}
          radius={25.5}
          active={active("Input")}
          selectedButton={selectedButton}
          onSelect={selectable("Input")}
        >
          <rect
            className="remote-key-icon"
            x="164.5"
            y="534.5"
            width="15"
            height="12.5"
            rx="1.6"
            strokeWidth="1.45"
          />
          <path
            className="remote-key-icon"
            d="M168 540.8h8m-3-3 3 3-3 3"
            strokeWidth="1.3"
          />
        </RemoteControlButton>

        <RemoteKey
          button="VolumeUp"
          active={active("VolumeUp")}
          selectedButton={selectedButton}
          onSelect={selectable("VolumeUp")}
        >
          <g className="remote-key-motion">
            <path
              className="remote-button-base remote-volume-base"
              d="M238 282c5 0 8 3 8 8v39c0 5-3 8-8 8s-8-3-8-8v-39c0-5 3-8 8-8Z"
              fill="url(#remote-volume)"
              stroke="#C3C2BC"
              strokeWidth="0.8"
            />
            <path
              className="remote-key-icon"
              d="M231.5 309h13m-6.5-6.5v13"
              strokeWidth="1.2"
            />
          </g>
        </RemoteKey>
        <RemoteKey
          button="VolumeDown"
          active={active("VolumeDown")}
          selectedButton={selectedButton}
          onSelect={selectable("VolumeDown")}
        >
          <g className="remote-key-motion">
            <path
              className="remote-button-base remote-volume-base"
              d="M238 325c5 0 8 3 8 8v39c0 5-3 8-8 8s-8-3-8-8v-39c0-5 3-8 8-8Z"
              fill="url(#remote-volume)"
              stroke="#C3C2BC"
              strokeWidth="0.8"
            />
            <path
              className="remote-key-icon"
              d="M231.5 352h13"
              strokeWidth="1.2"
            />
          </g>
        </RemoteKey>
      </g>
    </svg>
  );
}

function RemoteControlButton({
  button,
  cx,
  cy,
  radius = 29,
  active,
  selectedButton,
  onSelect,
  dark = false,
  children,
}: {
  button: string;
  cx: number;
  cy: number;
  radius?: number;
  active: boolean;
  selectedButton: string | null;
  onSelect?: (button: string) => void;
  dark?: boolean;
  children: ReactNode;
}) {
  return (
    <RemoteKey
      button={button}
      active={active}
      selectedButton={selectedButton}
      onSelect={onSelect}
      dark={dark}
    >
      <g className="remote-key-motion">
        <circle
          className="remote-button-base"
          cx={cx}
          cy={cy}
          r={radius}
          fill={dark ? "#17191C" : "url(#remote-key)"}
          stroke={dark ? "#080A0B" : "#C6C5C0"}
          strokeWidth="0.95"
        />
        {children}
      </g>
    </RemoteKey>
  );
}

function RemoteKey({
  button,
  active,
  selectedButton,
  onSelect,
  dark = false,
  children,
}: {
  button: string;
  active: boolean;
  selectedButton: string | null;
  onSelect?: (button: string) => void;
  dark?: boolean;
  children: ReactNode;
}) {
  const { t } = useI18n();
  const selected = selectedButton?.toLowerCase() === button.toLowerCase();
  const handleKeyDown = (event: KeyboardEvent<SVGGElement>) => {
    if (event.key !== "Enter" && event.key !== " ") return;
    event.preventDefault();
    onSelect?.(button);
  };
  return (
    <g
      className="remote-key"
      data-pressed={active ? "true" : "false"}
      data-selected={selected ? "true" : "false"}
      data-selectable={onSelect ? "true" : "false"}
      data-tone={dark ? "dark" : "light"}
      data-button={button}
      role={onSelect ? "button" : undefined}
      tabIndex={onSelect ? 0 : undefined}
      aria-label={remoteButtonLabel(button, t)}
      aria-pressed={onSelect ? selected : undefined}
      onClick={
        onSelect
          ? (event) => {
              event.stopPropagation();
              onSelect(button);
            }
          : undefined
      }
      onKeyDown={onSelect ? handleKeyDown : undefined}
    >
      {children}
    </g>
  );
}
