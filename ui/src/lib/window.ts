import { invoke } from "@tauri-apps/api/core";
import { isTauriRuntime } from "./ipc";

export async function toggleWindowMaximize(): Promise<boolean | null> {
  if (!isTauriRuntime()) return null;
  return invoke<boolean>("toggle_main_window_maximize");
}
