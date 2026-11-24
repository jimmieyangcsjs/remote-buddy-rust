import { describe, expect, it } from "vitest";
import {
  formatDbfs,
  rmsToBarHeight,
  rmsToDbfs,
} from "./audio-level";

describe("audio level presentation", () => {
  it("converts normalized RMS to dBFS", () => {
    expect(rmsToDbfs(0)).toBe(-Infinity);
    expect(rmsToDbfs(0.5)).toBeCloseTo(-6.0206, 3);
    expect(rmsToDbfs(1)).toBeCloseTo(0, 6);
  });

  it("formats silence and live values for the meter", () => {
    expect(formatDbfs(0)).toBe("−∞ dBFS");
    expect(formatDbfs(0.5)).toBe("-6.0 dBFS");
    expect(formatDbfs(1.5)).toBe("0.0 dBFS");
  });

  it("maps the -60..0 dBFS range to visible bar heights", () => {
    expect(rmsToBarHeight(0, true)).toBe(8);
    expect(rmsToBarHeight(0.5, false)).toBe(8);
    expect(rmsToBarHeight(0.000_001, true)).toBe(8);
    expect(rmsToBarHeight(10 ** (-30 / 20), true)).toBeCloseTo(60, 5);
    expect(rmsToBarHeight(1, true)).toBe(112);
    expect(rmsToBarHeight(10 ** (-30 / 20), true, 34)).toBeCloseTo(21, 5);
    expect(rmsToBarHeight(1, true, 34)).toBe(34);
  });
});
