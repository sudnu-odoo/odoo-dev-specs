import { isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

/** Ask the user for one file of the given kinds; null when they cancel (or outside the app). */
export async function pickFile(
  title: string,
  kinds: { name: string; extensions: string[] }[],
): Promise<string | null> {
  if (!isTauri()) return null;
  const chosen = await open({ directory: false, multiple: false, title, filters: kinds });
  return typeof chosen === "string" ? chosen : null;
}
