import { useCallback, useEffect, useRef, useState } from "react";

/**
 * A menu or card that opens over the page: open state, and closing on a click
 * outside its root element or on Escape. Put `root` on the element holding both the
 * button and the popover, so a click on either does not close it.
 */
export function usePopover<T extends HTMLElement = HTMLDivElement>() {
  const [open, setOpen] = useState(false);
  const root = useRef<T>(null);
  useEffect(() => {
    if (!open) return;
    const outside = (e: MouseEvent) => !root.current?.contains(e.target as Node) && setOpen(false);
    const escape = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", outside);
    document.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("mousedown", outside);
      document.removeEventListener("keydown", escape);
    };
  }, [open]);
  const toggle = useCallback(() => setOpen((o) => !o), []);
  const close = useCallback(() => setOpen(false), []);
  return { open, setOpen, toggle, close, root };
}
