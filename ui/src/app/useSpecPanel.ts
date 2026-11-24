import { useCallback, useEffect, useState } from "react";

const KEY = "specPanelOpen";

function remembered(): boolean {
  try {
    return localStorage.getItem(KEY) === "true";
  } catch {
    return false;
  }
}

/**
 * The spec (diagram) side panel: hidden at first so the chat has the room; shown with
 * the Spec button or Ctrl+B (Cmd+B on a Mac), and remembered on this computer.
 */
export function useSpecPanel() {
  const [open, setOpenState] = useState(remembered);
  useEffect(() => {
    try {
      localStorage.setItem(KEY, String(open));
    } catch {
      // Not remembered this time; nothing else depends on it.
    }
  }, [open]);
  const setOpen = useCallback((value: boolean) => setOpenState(value), []);
  const toggle = useCallback(() => setOpenState((o) => !o), []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === "b") {
        e.preventDefault();
        toggle();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [toggle]);

  return { open, setOpen, toggle };
}
