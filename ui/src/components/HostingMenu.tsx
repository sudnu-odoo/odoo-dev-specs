import type { Hosting } from "../lib/types";
import { usePopover } from "../app/usePopover";

/** "saas-19.2" -> "19.2"; stable versions stay as they are. */
export const versionLabel = (v: string) => v.replace(/^saas-/, "");

const HOSTINGS: { value: Hosting; title: string; detail: string }[] = [
  { value: "online", title: "Odoo Online", detail: "Latest SaaS version; no custom Python code" },
  { value: "sh", title: "Odoo.sh", detail: "The client's stable version; custom modules are possible" },
];

const FALLBACK_STABLE = ["19.0", "18.0", "17.0"];

export interface HostingMenuProps {
  /** null until chosen: the assistant asks. */
  hosting: Hosting | null;
  version: string | null;
  /** Versions Odoo publishes, newest first (empty until GitHub is connected). */
  available: string[];
  latestSaas: string | null;
  /** The stable version used when nothing else says (Settings). */
  defaultVersion: string;
  disabled?: boolean;
  /** The version list is still being fetched: say so rather than guess. */
  loading?: boolean;
  onChange: (hosting: Hosting, version: string) => void;
}

/**
 * Where the client's Odoo runs and which version, as one chip ("Odoo Online · 19.2"):
 * Odoo Online clients are on the latest SaaS version, Odoo.sh clients on a stable one.
 */
export default function HostingMenu({
  hosting,
  version,
  available,
  latestSaas,
  defaultVersion,
  disabled,
  loading,
  onChange,
}: HostingMenuProps) {
  const { open, setOpen, root } = usePopover();

  const saas = available.filter((v) => v.startsWith("saas-"));
  const stable = available.filter((v) => !v.startsWith("saas-"));
  const stableList = stable.length ? stable : FALLBACK_STABLE;
  const current = HOSTINGS.find((h) => h.value === hosting);
  const label = current ? `${current.title}${version ? ` · ${versionLabel(version)}` : ""}` : "Hosting?";

  /** The version a hosting starts with: keep the current one when it fits. */
  const versionFor = (h: Hosting): string => {
    if (h === "online")
      return version?.startsWith("saas-") ? version : (latestSaas ?? version ?? defaultVersion);
    return version && !version.startsWith("saas-") ? version : (stable[0] ?? defaultVersion);
  };
  const versions = hosting === "online" ? (saas.length ? saas : version ? [version] : []) : stableList;

  const pick = (h: Hosting, v: string) => {
    setOpen(false);
    if (h !== hosting || v !== version) onChange(h, v);
  };

  return (
    <div className="model-menu" ref={root}>
      <button
        type="button"
        className={"model-chip" + (current ? "" : " chip-attention")}
        aria-label={`Hosting: ${current ? label : "not chosen"}`}
        title="Where the client's Odoo runs, and its version"
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        {label}
        <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true">
          <path d="M3 4.5 6 7.5 9 4.5" fill="none" stroke="currentColor" strokeWidth="1.5" />
        </svg>
      </button>
      {open && (
        <div className="menu menu-up db-menu" role="menu" aria-label="Hosting">
          <div role="group" aria-label="Hosting" className="menu-group">
            <div className="menu-head" aria-hidden="true">
              Where the client's Odoo runs
            </div>
            {HOSTINGS.map((h) => (
              <Choice
                key={h.value}
                checked={h.value === hosting}
                title={h.title}
                detail={h.detail}
                name={h.title}
                onPick={() => pick(h.value, versionFor(h.value))}
              />
            ))}
          </div>
          {hosting && loading && available.length === 0 && (
            <div role="status" className="menu-group menu-loading">
              <span className="spinner" aria-hidden="true" /> Loading versions…
            </div>
          )}
          {hosting && versions.length > 0 && !(loading && available.length === 0) && (
            <div role="group" aria-label="Odoo version" className="menu-group">
              <div className="menu-head" aria-hidden="true">
                Odoo version
              </div>
              {versions.map((v, i) => (
                <Choice
                  key={v}
                  checked={v === version}
                  title={versionLabel(v)}
                  detail={hosting === "online" && i === 0 && v === latestSaas ? "latest" : ""}
                  onPick={() => pick(hosting, v)}
                />
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function Choice({
  checked,
  title,
  detail,
  name,
  onPick,
}: {
  checked: boolean;
  title: string;
  detail: string;
  /** Accessible name; by default the title with a short detail ("19.2 (latest)"). */
  name?: string;
  onPick: () => void;
}) {
  return (
    <div
      role="menuitemradio"
      aria-checked={checked}
      aria-label={name ?? (detail ? `${title} (${detail})` : title)}
      tabIndex={0}
      className="menu-item menu-item-2line"
      onClick={onPick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onPick();
        }
      }}
    >
      <span className="menu-item-text">
        <span>{title}</span>
        {detail && <small>{detail}</small>}
      </span>
      {checked && (
        <span className="menu-check" aria-hidden="true">
          ✓
        </span>
      )}
    </div>
  );
}
