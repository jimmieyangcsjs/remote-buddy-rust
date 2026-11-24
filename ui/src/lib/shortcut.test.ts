import { describe, expect, it } from "vitest";
import { formatShortcut, normalizeShortcutKey } from "./shortcut";

function keyEvent(
  key: string,
  code = "",
  modifiers: Partial<{
    ctrlKey: boolean;
    altKey: boolean;
    shiftKey: boolean;
    metaKey: boolean;
  }> = {},
) {
  return {
    key,
    code,
    ctrlKey: false,
    altKey: false,
    shiftKey: false,
    metaKey: false,
    ...modifiers,
  };
}

describe("shortcut recording", () => {
  it("records function keys with modifiers in parser order", () => {
    expect(
      formatShortcut(
        keyEvent("F5", "F5", { shiftKey: true, ctrlKey: true }),
      ),
    ).toBe("Ctrl+Shift+F5");
  });

  it("normalizes physical letter, digit, arrow and media keys", () => {
    expect(normalizeShortcutKey(keyEvent("r", "KeyR"))).toBe("R");
    expect(normalizeShortcutKey(keyEvent("!", "Digit1"))).toBe("1");
    expect(normalizeShortcutKey(keyEvent("ArrowUp"))).toBe("Up");
    expect(normalizeShortcutKey(keyEvent("AudioVolumeUp"))).toBe("VolumeUp");
  });

  it("does not commit modifier-only input", () => {
    expect(formatShortcut(keyEvent("Control", "ControlLeft", { ctrlKey: true }))).toBeNull();
  });

  it("rejects keys the Rust shortcut parser cannot represent", () => {
    expect(formatShortcut(keyEvent(";", "Semicolon", { ctrlKey: true }))).toBeNull();
  });
});
