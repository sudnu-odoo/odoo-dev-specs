import { useEffect, useState, type FormEvent, type ReactNode } from "react";
import { ODOO_APPS, ODOO_VERSIONS } from "../lib/odooApps";
import type { DatabaseForm, NewDatabase } from "../lib/types";

/** A small centred dialog (new database, rename, duplicate). */
export function Dialog({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="settings-overlay" role="dialog" aria-label={title}>
      <div className="dialog">
        <h2>{title}</h2>
        {children}
      </div>
    </div>
  );
}

/** The "New database" form: suggested name, version, edition, apps, demo data. */
export default function NewDatabaseDialog({
  defaultVersion,
  versions: local = [],
  formFor,
  onCancel,
  onCreate,
}: {
  defaultVersion: string;
  /** The versions on this computer; the list falls back to the usual ones when empty. */
  versions?: string[];
  formFor: (version: string) => Promise<DatabaseForm>;
  onCancel: () => void;
  onCreate: (db: NewDatabase) => void;
}) {
  const choices = local.length ? local : ODOO_VERSIONS;
  const [label, setLabel] = useState("");
  // Until the user types a name, the next free one is suggested ("db-19-3").
  const [named, setNamed] = useState(false);
  const [version, setVersion] = useState(choices.includes(defaultVersion) ? defaultVersion : choices[0]);
  const [modules, setModules] = useState<string[]>([]);
  const [demoData, setDemoData] = useState(true);
  const [form, setForm] = useState<DatabaseForm | null>(null);
  const [enterprise, setEnterprise] = useState(false);

  useEffect(() => {
    let current = true;
    formFor(version).then((f) => {
      if (!current) return;
      setForm(f);
      // Enterprise when the computer has it, like the assistant's own databases.
      setEnterprise(f.enterpriseAvailable);
      if (!named) setLabel(f.suggestedLabel);
    });
    return () => {
      current = false;
    };
    // `named` is read once per version change on purpose.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [formFor, version]);

  function submit(e: FormEvent) {
    e.preventDefault();
    if (label.trim()) onCreate({ label: label.trim(), version, modules, demoData, enterprise });
  }
  const toggle = (m: string) => setModules((ms) => (ms.includes(m) ? ms.filter((x) => x !== m) : [...ms, m]));
  const enterpriseOff = form !== null && !form.enterpriseAvailable;

  return (
    <Dialog title="New database">
      <form className="dialog-form" onSubmit={submit} onKeyDown={(e) => e.key === "Escape" && onCancel()}>
        <label className="dialog-field">
          <span>Name</span>
          <input
            autoFocus
            value={label}
            placeholder="e.g. Sales demo"
            onFocus={(e) => e.target.select()}
            onChange={(e) => {
              setNamed(true);
              setLabel(e.target.value);
            }}
          />
        </label>
        <label className="dialog-field">
          <span>Odoo version</span>
          <select value={version} onChange={(e) => setVersion(e.target.value)}>
            {choices.map((v) => (
              <option key={v}>{v}</option>
            ))}
          </select>
        </label>
        <div className="dialog-field">
          <div className="dialog-field-row">
            <span>Edition</span>
            <div className="segmented" role="radiogroup" aria-label="Edition">
              <button
                type="button"
                role="radio"
                aria-checked={!enterprise}
                onClick={() => setEnterprise(false)}
              >
                Community
              </button>
              <button
                type="button"
                role="radio"
                aria-checked={enterprise}
                disabled={enterpriseOff}
                title={
                  enterpriseOff ? `No enterprise source for Odoo ${version} on this computer` : undefined
                }
                onClick={() => setEnterprise(true)}
              >
                Enterprise
              </button>
            </div>
          </div>
          {enterpriseOff && (
            <small className="dialog-hint">
              Enterprise is not set up for Odoo {version} on this computer.
            </small>
          )}
        </div>
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
