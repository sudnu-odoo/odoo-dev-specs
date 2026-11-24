import { useEffect, useRef } from "react";

/** Run `action` on Ctrl+<key> (Cmd+<key> on a Mac), anywhere in the window. */
export function useShortcut(key: string, action: () => void) {
  const latest = useRef(action);
  useEffect(() => {
    latest.current = action;
  });
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey && e.key.toLowerCase() === key) {
        e.preventDefault();
        latest.current();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [key]);
}
