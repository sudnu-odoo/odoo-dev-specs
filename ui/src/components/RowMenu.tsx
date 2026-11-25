import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Icon, type IconName } from "./Icon";

export interface RowMenuItem {
  text: string;
  icon?: IconName;
  /** Shown in red (deleting). */
  danger?: boolean;
  run: () => void;
}

const MENU_WIDTH = 200;

/**
 * The "⋯" button of a row, and its menu of actions. The menu is drawn over the whole
 * window, next to the button, so sticky titles and scrolling lists never cover it.
 */
export default function RowMenu({
  label,
  disabled = false,
  items,
  className = "",
}: {
  /** What the row is, for "More actions for …". */
  label: string;
  disabled?: boolean;
  items: RowMenuItem[];
  className?: string;
}) {
  const [at, setAt] = useState<{ top: number; left: number } | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);
  const open = at !== null;

  function place() {
    let r = button.current?.getBoundingClientRect();
    // A hidden button has no place: use its row's.
    if (r && !r.width) r = button.current?.closest(".side-row, li")?.getBoundingClientRect() ?? r;
    if (!r) return;
    const left = Math.max(8, Math.min(r.right - MENU_WIDTH, window.innerWidth - MENU_WIDTH - 8));
    setAt({ top: r.bottom + 4, left });
  }

  // Keep it inside the window: open upwards when there is no room below.
  useLayoutEffect(() => {
    const m = menu.current;
    const r = button.current?.getBoundingClientRect();
    if (!m || !r || !at) return;
    const height = m.offsetHeight;
    if (at.top + height > window.innerHeight - 8 && r.top - height - 4 > 8) {
      setAt({ ...at, top: r.top - height - 4 });
    }
  }, [at]);

  useEffect(() => {
    if (!open) return;
    const close = () => setAt(null);
    const outside = (e: MouseEvent) => {
      const t = e.target as Node;
      if (!menu.current?.contains(t) && !button.current?.contains(t)) close();
    };
    const escape = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      close();
      button.current?.focus();
    };
    document.addEventListener("mousedown", outside);
    document.addEventListener("keydown", escape);
    window.addEventListener("resize", close);
    // The list scrolling under it would leave it floating in the wrong place.
    document.addEventListener("scroll", close, true);
    return () => {
      document.removeEventListener("mousedown", outside);
      document.removeEventListener("keydown", escape);
      window.removeEventListener("resize", close);
      document.removeEventListener("scroll", close, true);
    };
  }, [open]);

  return (
    <div className={"row-menu " + className} data-open={open || undefined}>
      <button
        ref={button}
        className="icon-btn"
        aria-label={`More actions for ${label}`}
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => (open ? setAt(null) : place())}
      >
        <Icon name="more" />
      </button>
      {open &&
        createPortal(
          <div
            ref={menu}
            className="menu row-menu-list"
            role="menu"
            aria-label={label}
            style={{ position: "fixed", top: at.top, left: at.left, width: MENU_WIDTH }}
          >
            {items.map((item) => (
              <button
                key={item.text}
                role="menuitem"
                className={"menu-item" + (item.danger ? " menu-item-danger" : "")}
                onClick={() => {
                  setAt(null);
                  item.run();
                }}
              >
                {item.icon && <Icon name={item.icon} />}
                {item.text}
              </button>
            ))}
          </div>,
          document.body,
        )}
    </div>
  );
}
