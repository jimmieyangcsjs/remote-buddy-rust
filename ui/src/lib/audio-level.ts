const METER_FLOOR_DBFS = -60;
const MIN_BAR_HEIGHT = 8;
const MAX_BAR_HEIGHT = 112;

export const VOICE_WAVEFORM_BARS = 48;

export function rmsToDbfs(rms: number) {
  if (!Number.isFinite(rms) || rms <= 0) return -Infinity;
  return 20 * Math.log10(Math.min(rms, 1));
}

export function formatDbfs(rms: number) {
  const dbfs = rmsToDbfs(rms);
  if (!Number.isFinite(dbfs)) return "−∞ dBFS";
  const display = Math.abs(dbfs) < 0.05 ? 0 : dbfs;
  return `${display.toFixed(1)} dBFS`;
}

export function rmsToBarHeight(
  rms: number,
  active: boolean,
  maxHeight = MAX_BAR_HEIGHT,
) {
  if (!active) return MIN_BAR_HEIGHT;
  const dbfs = rmsToDbfs(rms);
  if (!Number.isFinite(dbfs)) return MIN_BAR_HEIGHT;
  const normalized = Math.min(
    1,
    Math.max(0, (dbfs - METER_FLOOR_DBFS) / -METER_FLOOR_DBFS),
  );
  return (
    MIN_BAR_HEIGHT +
    normalized * Math.max(0, maxHeight - MIN_BAR_HEIGHT)
  );
}
