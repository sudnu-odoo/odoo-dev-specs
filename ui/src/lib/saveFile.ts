import { invoke, isTauri } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

/**
 * Ask the user where to save, then write the blob.
 * Inside the desktop app this uses the native dialog; in a plain browser
 * (vite preview) it falls back to a regular download.
 * Returns false when the user cancels.
 */
export async function saveFile(
  blob: Blob,
  suggestedName: string,
  filterName: string,
  ext: string,
): Promise<boolean> {
  if (!isTauri()) {
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = suggestedName;
    a.click();
    URL.revokeObjectURL(url);
    return true;
  }
  const path = await save({ defaultPath: suggestedName, filters: [{ name: filterName, extensions: [ext] }] });
  if (!path) return false;
  const bytes = Array.from(new Uint8Array(await blob.arrayBuffer()));
  await invoke("write_file", { path, contents: bytes });
  return true;
}
