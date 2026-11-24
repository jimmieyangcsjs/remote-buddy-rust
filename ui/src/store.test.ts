import { beforeEach, describe, expect, it } from "vitest";
import { useAppStore } from "./store";
import type { InputEvent, VoiceLevelEvent, VoiceStateEvent } from "./lib/types";

function input(phase: InputEvent["phase"]): InputEvent {
  return {
    id: `input-${phase}`,
    time: "12:00:00.000",
    button: "Ok",
    phase,
    action: "Enter",
    source: "BthPort ETW",
  };
}

describe("live input state", () => {
  beforeEach(() => {
    useAppStore.setState({ lastInput: null, pressedButtons: [] });
  });

  it("highlights a button immediately on down and clears it on up", () => {
    useAppStore.getState().applyInput(input("down"));
    expect(useAppStore.getState().pressedButtons).toEqual(["Ok"]);
    expect(useAppStore.getState().lastInput?.phase).toBe("down");

    useAppStore.getState().applyInput(input("up"));
    expect(useAppStore.getState().pressedButtons).toEqual([]);
    expect(useAppStore.getState().lastInput?.phase).toBe("up");
  });

  it("keeps repeat events active without duplicating the button", () => {
    useAppStore.getState().applyInput(input("down"));
    useAppStore.getState().applyInput(input("repeat"));
    expect(useAppStore.getState().pressedButtons).toEqual(["Ok"]);
  });
});

describe("live voice level state", () => {
  beforeEach(() => {
    useAppStore.setState({
      voiceActive: false,
      voiceLevel: { rms: 0, peak: 0 },
      voiceLevelHistory: [],
    });
  });

  it("keeps a bounded history of measured RMS values", () => {
    for (let index = 0; index < 60; index += 1) {
      const event: VoiceLevelEvent = {
        rms: index / 100,
        peak: index / 90,
        sequence: index,
      };
      useAppStore.getState().applyVoiceLevel(event);
    }

    const state = useAppStore.getState();
    expect(state.voiceLevelHistory).toHaveLength(48);
    expect(state.voiceLevelHistory[0]).toBeCloseTo(0.12);
    expect(state.voiceLevelHistory.at(-1)).toBeCloseTo(0.59);
    expect(state.voiceLevel).toEqual({ rms: 0.59, peak: 59 / 90 });
  });

  it("clears the meter as soon as voice input stops", () => {
    useAppStore.getState().applyVoiceLevel({
      rms: 0.4,
      peak: 0.8,
      sequence: 3,
    });
    const stopped: VoiceStateEvent = {
      active: false,
      handsFree: false,
      phase: "idle",
      error: null,
    };

    useAppStore.getState().applyVoice(stopped);

    expect(useAppStore.getState().voiceLevel).toEqual({ rms: 0, peak: 0 });
    expect(useAppStore.getState().voiceLevelHistory).toEqual([]);
  });
});
