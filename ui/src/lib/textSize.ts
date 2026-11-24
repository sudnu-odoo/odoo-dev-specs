import { useEffect, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/**
 * How big everything is shown (the whole window is zoomed, like in a browser, so
 * the diagram stays exact). A per-computer display preference, set in Settings or
 * with Ctrl +, Ctrl − and Ctrl 0.
 */
export const TEXT_SIZES = [0.8, 0.9, 1, 1.1, 1.25, 1.4, 1.6] as const;

const KEY = "textSize";
const CHANGED = "text-size-changed";

export function getTextSize(): number {
  try {
    const v = Number(localStorage.getItem(KEY));
    return (TEXT_SIZES as readonly number[]).includes(v) ? v : 1;
  } catch {
    return 1;
  }
}

export function setTextSize(size: number) {
  try {
    localStorage.setItem(KEY, String(size));
  } catch {
    // Storage unavailable: the choice lasts until the window closes.
  }
  window.dispatchEvent(new CustomEvent(CHANGED));
}

/** The next size up (`1`) or down (`-1`), staying within the list. */
export function stepTextSize(size: number, direction: 1 | -1): number {
  const at = TEXT_SIZES.indexOf(size as (typeof TEXT_SIZES)[number]);
  const next = (at < 0 ? TEXT_SIZES.indexOf(1) : at) + direction;
  return TEXT_SIZES[Math.max(0, Math.min(TEXT_SIZES.length - 1, next))];
}

/** The chosen size; follows changes made in Settings, the shortcuts and other windows. */
export function useTextSize(): number {
  const [size, setSize] = useState(getTextSize);
  useEffect(() => {
    const sync = () => setSize(getTextSize());
    window.addEventListener(CHANGED, sync);
    window.addEventListener("storage", sync);
    return () => {
      window.removeEventListener(CHANGED, sync);
      window.removeEventListener("storage", sync);
    };
  }, []);
  return size;
}

/** Apply the size to this window. */
export function useAppTextSize() {
  const size = useTextSize();
  useEffect(() => {
    try {
      getCurrentWebview()
        .setZoom(size)
        .catch(() => {});
    } catch {
      // Not inside the app (a plain browser): nothing to zoom.
    }
  }, [size]);
}
