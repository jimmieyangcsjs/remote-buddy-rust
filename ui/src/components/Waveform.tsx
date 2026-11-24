import {
  formatDbfs,
  rmsToBarHeight,
  VOICE_WAVEFORM_BARS,
} from "../lib/audio-level";
import { useI18n } from "../lib/i18n";

export function Waveform({
  active = true,
  levels = [],
  compact = false,
}: {
  active?: boolean;
  levels?: number[];
  compact?: boolean;
}) {
  const { t } = useI18n();
  const recent = levels.slice(-VOICE_WAVEFORM_BARS);
  const displayed = [
    ...Array<number>(VOICE_WAVEFORM_BARS - recent.length).fill(0),
    ...recent,
  ];
  const currentRms = recent.at(-1) ?? 0;

  return (
    <div
      className={`wave-canvas${compact ? " wave-canvas--compact" : ""}`}
      data-active={active}
      aria-label={t(
        `实时语音电平，RMS ${formatDbfs(currentRms)}`,
        `Live voice level, RMS ${formatDbfs(currentRms)}`,
      )}
    >
      {displayed.map((rms, index) => (
        <span
          key={index}
          style={{
            height: rmsToBarHeight(rms, active, compact ? 34 : 112),
            opacity: active && rms > 0 ? 0.94 : 0.22,
          }}
        />
      ))}
    </div>
  );
}
