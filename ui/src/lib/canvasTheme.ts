import { useEffect, useState } from "react";

/** The app's theme (the diagram canvas included), chosen in Settings. A per-computer display preference. */
export type CanvasTheme = "light" | "dark" | "system";

const KEY = "canvasTheme";
const CHANGED = "canvas-theme-changed";

export function getCanvasTheme(): CanvasTheme {
  try {
    const v = localStorage.getItem(KEY);
    return v === "dark" || v === "system" ? v : "light";
  } catch {
    return "light";
  }
}

export function setCanvasTheme(theme: CanvasTheme) {
  try {
    localStorage.setItem(KEY, theme);
  } catch {
    // Storage unavailable: the choice lasts until the window closes.
  }
  window.dispatchEvent(new CustomEvent(CHANGED, { detail: theme }));
}

const systemDark = () => window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false;

/** The chosen theme; follows changes made in Settings, other windows and the system. */
export function useCanvasTheme(): CanvasTheme {
  const [theme, setTheme] = useState(getCanvasTheme);
  useEffect(() => {
    const sync = () => setTheme(getCanvasTheme());
    window.addEventListener(CHANGED, sync);
    // The separate diagram window hears about it through storage.
    window.addEventListener("storage", sync);
    return () => {
      window.removeEventListener(CHANGED, sync);
      window.removeEventListener("storage", sync);
    };
  }, []);
  return theme;
}

/** "light" or "dark" for Excalidraw, resolving "system". */
export function useResolvedCanvasTheme(): "light" | "dark" {
  const theme = useCanvasTheme();
  const [dark, setDark] = useState(systemDark);
  useEffect(() => {
    const query = window.matchMedia?.("(prefers-color-scheme: dark)");
    if (!query) return;
    const update = () => setDark(query.matches);
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);
  return theme === "system" ? (dark ? "dark" : "light") : theme;
}

/** Apply the theme to the whole window (its colours follow `data-theme`). */
export function useAppTheme() {
  const theme = useResolvedCanvasTheme();
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);
}
