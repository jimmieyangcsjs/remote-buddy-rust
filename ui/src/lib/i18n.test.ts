import { describe, expect, it } from "vitest";
import { resolveUiLanguage } from "./i18n";

describe("resolveUiLanguage", () => {
  it("defaults to Chinese instead of the operating-system locale", () => {
    expect(resolveUiLanguage("system")).toBe("zh");
    expect(resolveUiLanguage(undefined)).toBe("zh");
  });

  it("uses English only when explicitly selected", () => {
    expect(resolveUiLanguage("en-US")).toBe("en");
    expect(resolveUiLanguage("zh-CN")).toBe("zh");
  });
});
