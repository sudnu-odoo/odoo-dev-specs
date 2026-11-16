import { isTauri } from "@tauri-apps/api/core";
import { ask } from "@tauri-apps/plugin-dialog";

/** Yes/no question with a native dialog in the app, the browser's confirm otherwise. */
export async function confirmAction(message: string, title: string): Promise<boolean> {
  if (!isTauri()) return window.confirm(message);
  return ask(message, { title, kind: "warning" });
}
