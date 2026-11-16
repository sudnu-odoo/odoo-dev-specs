import { isTauri } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";

/** Open a link in the user's browser (the app's webview cannot open new tabs). */
export async function openExternal(url: string): Promise<void> {
  if (!/^https?:\/\//i.test(url)) return;
  if (isTauri()) await openUrl(url);
  else window.open(url, "_blank", "noreferrer");
}
