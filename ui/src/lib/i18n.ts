import { useAppStore } from "../store";

export type UiLanguage = "zh" | "en";

export function resolveUiLanguage(setting?: string | null): UiLanguage {
  if (setting === "en-US") return "en";
  return "zh";
}

export function useUiLanguage(): UiLanguage {
  const setting = useAppStore((state) => state.language);
  return resolveUiLanguage(setting);
}

export function useI18n() {
  const language = useUiLanguage();
  return {
    language,
    t: (zh: string, en: string) => (language === "en" ? en : zh),
  };
}
