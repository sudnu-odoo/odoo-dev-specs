import { usePopover } from "../app/usePopover";
import { Icon, type IconName } from "./Icon";

export interface RowMenuItem {
  text: string;
  icon?: IconName;
  /** Shown in red (deleting). */
  danger?: boolean;
  run: () => void;
}

/** The "⋯" button of a row, and its menu of actions. */
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
  const { open, setOpen, root } = usePopover();
  return (
    <div className={"row-menu " + className} ref={root} data-open={open || undefined}>
      <button
        className="icon-btn"
        aria-label={`More actions for ${label}`}
        title="More actions"
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        <Icon name="more" />
      </button>
      {open && (
        <div className="menu row-menu-list" role="menu">
          {items.map((item) => (
            <button
              key={item.text}
              role="menuitem"
              className={"menu-item" + (item.danger ? " menu-item-danger" : "")}
              onClick={() => {
                setOpen(false);
                item.run();
              }}
            >
              {item.icon && <Icon name={item.icon} />}
              {item.text}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
