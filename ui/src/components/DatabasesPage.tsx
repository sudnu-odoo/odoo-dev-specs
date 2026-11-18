import { useEffect, useRef, useState, type FormEvent } from "react";
import type { Databases, DatabaseBusy } from "../app/useDatabases";
import { ODOO_APPS, ODOO_VERSIONS, appsText, sizeText, usedText } from "../lib/odooApps";
import type { DatabaseInfo } from "../lib/types";

export interface DatabasesPageProps {
  dbs: Databases;
  /** Database name -> how many sessions use it. */
  usage: Record<string, number>;
  defaultVersion?: string;
}

const BUSY_TEXT: Record<DatabaseBusy, string> = {
  starting: "Starting…",
  stopping: "Stopping…",
  copying: "Copying…",
  resetting: "Resetting…",
  deleting: "Deleting…",
};

/**
 * The local Odoo databases: the user's own on top, those the assistant made for
 * screenshots below. Each can be opened in the browser, stopped, renamed, copied,
 * reset or deleted; new ones are made fresh with the chosen apps.
 */
export default function DatabasesPage({ dbs, usage, defaultVersion = "19.0" }: DatabasesPageProps) {
  const [creatingForm, setCreatingForm] = useState(false);
  const [asking, setAsking] = useState<{ kind: "rename" | "duplicate"; db: DatabaseInfo } | null>(null);
  const mine = dbs.databases.filter((d) => !d.auto);
  const auto = dbs.databases.filter((d) => d.auto);

  return (
    <section className="databases" aria-labelledby="databases-title">
      <header className="databases-header">
        <div>
          <h2 id="databases-title">Databases</h2>
          <p className="settings-lead">
            Local Odoo for screenshots and trying things out. Demo data only, never client data.
          </p>
        </div>
        <button className="btn btn-primary" onClick={() => setCreatingForm(true)} disabled={dbs.creating}>
          New database
        </button>
      </header>

      {dbs.error && (
        <div role="alert" className="banner-error">
          {dbs.error}
          <button className="icon-btn" aria-label="Dismiss" onClick={dbs.dismissError}>
            ×
          </button>
        </div>
      )}
      {dbs.creating && (
        <div role="status" className="databases-progress">
          <span className="spinner" aria-hidden="true" /> Creating the database… The first one of an Odoo
          version takes a few minutes.
        </div>
      )}

      <h3>Your databases</h3>
      {mine.length === 0 ? (
        <p className="settings-muted databases-empty">
          None yet. Create one to set Odoo up like your client, then pick it in a session.
        </p>
      ) : (
        renderList(mine)
      )}

      {auto.length > 0 && (
        <>
          <h3>Made by the assistant</h3>
          {renderList(auto)}
        </>
      )}

      {creatingForm && (
        <NewDatabaseDialog
          defaultVersion={defaultVersion}
          onCancel={() => setCreatingForm(false)}
          onCreate={(db) => {
            setCreatingForm(false);
            dbs.create(db);
          }}
        />
      )}
      {asking && (
        <NameDialog
          title={asking.kind === "rename" ? "Rename database" : "Duplicate database"}
          action={asking.kind === "rename" ? "Rename" : "Duplicate"}
          initial={asking.kind === "rename" ? asking.db.label : `${asking.db.label} (copy)`}
          onCancel={() => setAsking(null)}
          onDone={(label) => {
            const { kind, db } = asking;
            setAsking(null);
            if (kind === "rename") dbs.rename(db, label);
            else dbs.duplicate(db, label);
          }}
        />
      )}
    </section>
  );

  // A plain function, not a component: rows (and their open menus) survive re-renders.
  function renderList(items: DatabaseInfo[]) {
    return (
      <ul className="settings-card databases-list">
        {items.map((db) => (
          <DatabaseRow
            key={db.name}
            db={db}
            usedBy={usage[db.name] ?? 0}
            busy={dbs.busy[db.name]}
            onOpen={() => dbs.open(db)}
            onStop={() => dbs.stop(db)}
            onRename={() => setAsking({ kind: "rename", db })}
            onDuplicate={() => setAsking({ kind: "duplicate", db })}
            onReset={() => dbs.reset(db)}
            onDelete={() => dbs.remove(db, usage[db.name] ?? 0)}
          />
        ))}
      </ul>
    );
  }
}

interface RowProps {
  db: DatabaseInfo;
  usedBy: number;
  busy?: DatabaseBusy;
  onOpen: () => void;
  onStop: () => void;
  onRename: () => void;
  onDuplicate: () => void;
  onReset: () => void;
  onDelete: () => void;
}

function DatabaseRow({
  db,
  usedBy,
  busy,
  onOpen,
  onStop,
  onRename,
  onDuplicate,
  onReset,
  onDelete,
}: RowProps) {
  const details = [
    db.version,
    appsText(db.modules),
    db.enterprise ? "Enterprise" : "Community",
    db.demoData ? "demo data" : "no demo data",
  ].join(" · ");
  const status = [
    busy ? BUSY_TEXT[busy] : db.running ? "Running" : "Stopped",
    sizeText(db.sizeBytes),
    usedText(db.lastUsed),
    usedBy ? `${usedBy} session${usedBy > 1 ? "s" : ""}` : null,
  ]
    .filter(Boolean)
    .join(" · ");
  return (
    <li className="settings-row database-row" aria-label={db.label} aria-busy={!!busy || undefined}>
      <span className="database-dot" data-running={db.running} aria-hidden="true" />
      <div className="settings-row-text">
        <span>{db.label}</span>
        <small>{details}</small>
        <small>{status}</small>
      </div>
      <button className="btn btn-small" onClick={onOpen} disabled={!!busy} title="Open in your browser">
        Open
      </button>
      {db.running && (
        <button className="btn btn-small" onClick={onStop} disabled={!!busy}>
          Stop
        </button>
      )}
      <RowMenu
        label={db.label}
        disabled={!!busy}
        items={[
          ["Rename", onRename],
          ["Duplicate", onDuplicate],
          ["Reset to fresh", onReset],
          ["Delete", onDelete],
        ]}
      />
    </li>
  );
}

function RowMenu({
  label,
  disabled,
  items,
}: {
  label: string;
  disabled: boolean;
  items: [string, () => void][];
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => !root.current?.contains(e.target as Node) && setOpen(false);
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);
  return (
    <div className="row-menu" ref={root}>
      <button
        className="icon-btn"
        aria-label={`More actions for ${label}`}
        aria-haspopup="menu"
        aria-expanded={open}
        disabled={disabled}
        onClick={() => setOpen((o) => !o)}
      >
        ⋯
      </button>
      {open && (
        <div className="menu row-menu-list" role="menu">
          {items.map(([text, run]) => (
            <button
              key={text}
              role="menuitem"
              className={"menu-item" + (text === "Delete" ? " menu-item-danger" : "")}
              onClick={() => {
                setOpen(false);
                run();
              }}
            >
              {text}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function Dialog({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <div className="settings-overlay" role="dialog" aria-label={title}>
      <div className="dialog">
        <h2>{title}</h2>
        {children}
      </div>
    </div>
  );
}

function NewDatabaseDialog({
  defaultVersion,
  onCancel,
  onCreate,
}: {
  defaultVersion: string;
  onCancel: () => void;
  onCreate: (db: { label: string; version: string; modules: string[]; demoData: boolean }) => void;
}) {
  const [label, setLabel] = useState("");
  const [version, setVersion] = useState(
    ODOO_VERSIONS.includes(defaultVersion) ? defaultVersion : ODOO_VERSIONS[0],
  );
  const [modules, setModules] = useState<string[]>([]);
  const [demoData, setDemoData] = useState(true);

  function submit(e: FormEvent) {
    e.preventDefault();
    if (label.trim()) onCreate({ label: label.trim(), version, modules, demoData });
  }
  const toggle = (m: string) => setModules((ms) => (ms.includes(m) ? ms.filter((x) => x !== m) : [...ms, m]));

  return (
    <Dialog title="New database">
      <form className="dialog-form" onSubmit={submit} onKeyDown={(e) => e.key === "Escape" && onCancel()}>
        <label className="dialog-field">
          <span>Name</span>
          <input
            autoFocus
            value={label}
            placeholder="e.g. Sales demo"
            onChange={(e) => setLabel(e.target.value)}
          />
        </label>
        <label className="dialog-field">
          <span>Odoo version</span>
          <select value={version} onChange={(e) => setVersion(e.target.value)}>
            {ODOO_VERSIONS.map((v) => (
              <option key={v}>{v}</option>
            ))}
          </select>
        </label>
        <fieldset className="dialog-field">
          <legend>Apps</legend>
          <div className="app-chips">
            {ODOO_APPS.map((a) => (
              <button
                type="button"
                key={a.module}
                className="app-chip"
                aria-pressed={modules.includes(a.module)}
                onClick={() => toggle(a.module)}
              >
                {a.label}
              </button>
            ))}
          </div>
        </fieldset>
        <div className="dialog-field dialog-field-row">
          <span>Demo data</span>
          <div className="segmented" role="radiogroup" aria-label="Demo data">
            {[true, false].map((on) => (
              <button
                type="button"
                key={String(on)}
                role="radio"
                aria-checked={demoData === on}
                onClick={() => setDemoData(on)}
              >
                {on ? "On" : "Off"}
              </button>
            ))}
          </div>
        </div>
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onCancel}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" disabled={!label.trim()}>
            Create
          </button>
        </div>
      </form>
    </Dialog>
  );
}

function NameDialog({
  title,
  action,
  initial,
  onCancel,
  onDone,
}: {
  title: string;
  action: string;
  initial: string;
  onCancel: () => void;
  onDone: (label: string) => void;
}) {
  const [label, setLabel] = useState(initial);
  return (
    <Dialog title={title}>
      <form
        className="dialog-form"
        onSubmit={(e) => {
          e.preventDefault();
          if (label.trim()) onDone(label.trim());
        }}
        onKeyDown={(e) => e.key === "Escape" && onCancel()}
      >
        <label className="dialog-field">
          <span>Name</span>
          <input
            autoFocus
            value={label}
            onFocus={(e) => e.target.select()}
            onChange={(e) => setLabel(e.target.value)}
          />
        </label>
        <div className="dialog-actions">
          <button type="button" className="btn" onClick={onCancel}>
            Cancel
          </button>
          <button type="submit" className="btn btn-primary" disabled={!label.trim()}>
            {action}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
