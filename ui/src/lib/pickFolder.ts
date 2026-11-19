import { isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

/** Ask the user for a folder; null when they cancel (or outside the app). */
export async function pickFolder(title: string): Promise<string | null> {
  if (!isTauri()) return null;
  const chosen = await open({ directory: true, multiple: false, title });
  return typeof chosen === "string" ? chosen : null;
}
