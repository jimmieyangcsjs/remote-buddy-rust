export interface ShortcutKeyLike {
  key: string;
  code: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}

const namedKeys: Record<string, string> = {
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Enter: "Enter",
  Escape: "Escape",
  Tab: "Tab",
  " ": "Space",
  Spacebar: "Space",
  Backspace: "Backspace",
  Delete: "Delete",
  Insert: "Insert",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  ContextMenu: "Apps",
  AudioVolumeUp: "VolumeUp",
  AudioVolumeDown: "VolumeDown",
  AudioVolumeMute: "Mute",
  MediaPlayPause: "PlayPause",
};

const modifierKeys = new Set(["Control", "Alt", "Shift", "Meta", "OS"]);

export function normalizeShortcutKey(event: ShortcutKeyLike): string | null {
  if (namedKeys[event.key]) return namedKeys[event.key];

  const letter = event.code.match(/^Key([A-Z])$/)?.[1];
  if (letter) return letter;

  const digit = event.code.match(/^(?:Digit|Numpad)([0-9])$/)?.[1];
  if (digit) return digit;

  if (/^F(?:[1-9]|1[0-9]|2[0-4])$/.test(event.key)) {
    return event.key.toUpperCase();
  }
  if (/^[a-zA-Z]$/.test(event.key)) return event.key.toUpperCase();
  if (/^[0-9]$/.test(event.key)) return event.key;
  return null;
}

export function isModifierOnly(event: ShortcutKeyLike): boolean {
  return modifierKeys.has(event.key);
}

export function formatShortcut(event: ShortcutKeyLike): string | null {
  if (isModifierOnly(event)) return null;
  const key = normalizeShortcutKey(event);
  if (!key) return null;

  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Win");
  parts.push(key);
  return parts.join("+");
}

export function modifierPreview(event: ShortcutKeyLike): string | null {
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  if (event.metaKey) parts.push("Win");
  return parts.length > 0 ? `${parts.join(" + ")} + …` : null;
}
