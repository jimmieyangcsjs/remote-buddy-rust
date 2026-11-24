import { useRef, useState, type KeyboardEvent } from "react";
import {
  formatShortcut,
  isModifierOnly,
  modifierPreview,
} from "../lib/shortcut";
import { useI18n } from "../lib/i18n";

interface ShortcutInputProps {
  value: string;
  onChange: (value: string) => void;
  id?: string;
  placeholder?: string;
}

export function ShortcutInput({
  value,
  onChange,
  id,
  placeholder,
}: ShortcutInputProps) {
  const { t } = useI18n();
  const inputRef = useRef<HTMLInputElement>(null);
  const [recording, setRecording] = useState(false);
  const [preview, setPreview] = useState<string | null>(null);

  const handleKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    event.preventDefault();
    event.stopPropagation();
    if (event.repeat || event.nativeEvent.isComposing) return;

    if (isModifierOnly(event.nativeEvent)) {
      setPreview(modifierPreview(event.nativeEvent));
      return;
    }
    if (event.key === "Escape" && !event.ctrlKey && !event.altKey && !event.shiftKey && !event.metaKey) {
      setPreview(null);
      setRecording(false);
      inputRef.current?.blur();
      return;
    }

    const shortcut = formatShortcut(event.nativeEvent);
    if (!shortcut) {
      setPreview(t("该按键不支持，请换一个按键", "Unsupported key. Try another."));
      return;
    }
    onChange(shortcut);
    setPreview(null);
    setRecording(false);
    inputRef.current?.blur();
  };

  return (
    <input
      ref={inputRef}
      id={id}
      className="select shortcut-capture"
      data-recording={recording}
      readOnly
      value={
        recording
          ? (preview ?? t("请按下组合键…", "Press a shortcut…"))
          : value
      }
      placeholder={placeholder ?? t("点击后按下组合键", "Click, then press keys")}
      aria-label={t("快捷键录制", "Shortcut recorder")}
      title={t(
        "点击后直接按下组合键，按 Esc 取消",
        "Click, then press a shortcut. Press Esc to cancel.",
      )}
      onFocus={() => {
        setRecording(true);
        setPreview(null);
      }}
      onBlur={() => {
        setRecording(false);
        setPreview(null);
      }}
      onClick={() => inputRef.current?.focus()}
      onKeyDown={handleKeyDown}
      onKeyUp={(event) => {
        if (
          recording &&
          !event.ctrlKey &&
          !event.altKey &&
          !event.shiftKey &&
          !event.metaKey
        ) {
          setPreview(null);
        }
      }}
    />
  );
}
