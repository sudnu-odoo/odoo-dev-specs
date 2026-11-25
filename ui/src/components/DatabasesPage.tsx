import RowMenu from "./RowMenu";
import { useState } from "react";
import type { Databases, DatabaseBusy } from "../app/useDatabases";
import { appsText, sizeText, usedText } from "../lib/odooApps";
import type { DatabaseInfo } from "../lib/types";
import NewDatabaseDialog, { Dialog } from "./NewDatabaseDialog";

export interface DatabasesPageProps {
  dbs: Databases;
  /** Database name -> how many sessions use it. */
  usage: Record<string, number>;
  defaultVersion?: string;
  /** The Odoo versions on this computer. */
  versions?: string[];
}

const BUSY_TEXT: Record<DatabaseBusy, string> = {
  starting: "Starting…",
  stopping: "Stopping…",
  copying: "Copying…",
  resetting: "Resetting…",
  deleting: "Deleting…",
};

/**
 * The local Odoo databases, most recently used first (also the ones the assistant
 * made for screenshots: they are databases like any other). Each can be opened in the browser, stopped, renamed, copied,
 * reset or deleted; new ones are made fresh with the chosen apps.
 */
export default function DatabasesPage({ dbs, usage, defaultVersion = "19.0", versions }: DatabasesPageProps) {
  const [creatingForm, setCreatingForm] = useState(false);
  const [asking, setAsking] = useState<{ kind: "rename" | "duplicate"; db: DatabaseInfo } | null>(null);
  const all = [...dbs.databases].sort((a, b) => b.lastUsed - a.lastUsed);

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
      {all.length === 0 ? (
        <p className="settings-muted databases-empty">
          None yet. Create one to set Odoo up like your client, then pick it in a session.
        </p>
      ) : (
        renderList(all)
      )}

      {creatingForm && (
        <NewDatabaseDialog
          defaultVersion={defaultVersion}
          versions={versions}
          formFor={dbs.formFor}
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
          { text: "Rename", run: onRename },
          { text: "Duplicate", run: onDuplicate },
          { text: "Reset to fresh", run: onReset },
          { text: "Delete", run: onDelete, danger: true },
        ]}
      />
    </li>
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
    <Dialog title={title} onClose={onCancel}>
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
