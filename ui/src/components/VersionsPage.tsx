import { useState } from "react";
import { useOdooVersions } from "../app/useOdooVersions";
import { api } from "../lib/api";
import { confirmAction } from "../lib/confirm";
import { pickFolder } from "../lib/pickFolder";
import { sizeText } from "../lib/odooApps";
import type { VersionInfo } from "../lib/types";
import { versionLabel } from "./HostingMenu";

const edition = (v: VersionInfo) =>
  [
    v.community.state === "ready" ? "Community" : null,
    v.enterprise.state === "ready" ? "Enterprise" : null,
    v.community.state === "ready" && v.community.linked ? "your own folder" : null,
  ]
    .filter(Boolean)
    .join(" + ");

function updatedText(ms: number): string {
  if (!ms) return "never updated";
  const days = Math.floor((Date.now() - ms) / 86_400_000);
  return days <= 0 ? "updated today" : days === 1 ? "updated yesterday" : `updated ${days} days ago`;
}

/**
 * The Odoo versions on this computer, in Settings: each with its size and last update,
 * Update now and Remove; another version can be added from the ones Odoo publishes.
 */
export default function VersionsPage() {
  const versions = useOdooVersions();
  const [busy, setBusy] = useState<Record<string, string>>({});
  const [adding, setAdding] = useState("");
  const [error, setError] = useState<string | null>(null);

  async function act(version: string, what: string, action: () => Promise<void>) {
    setBusy((b) => ({ ...b, [version]: what }));
    try {
      await action();
      setError(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy((b) => {
        const next = { ...b };
        delete next[version];
        return next;
      });
      await versions.reload();
    }
  }

  /** Use folders the person already has (e.g. odev worktrees) for a version. */
  async function chooseFolders(version: string) {
    const label = versionLabel(version);
    const community = await pickFolder(`Choose your Odoo ${label} folder (Community)`);
    if (!community) return;
    await act(version, "Checking the folder…", async () => {
      await api.useOdooFolder(version, "community", community);
      const enterprise = await pickFolder(`Choose your Odoo ${label} Enterprise folder (or Cancel)`);
      if (enterprise) await api.useOdooFolder(version, "enterprise", enterprise);
    });
  }

  const here = new Set(versions.downloaded.map((v) => v.version));
  const addable = versions.available.filter((v) => !here.has(v));

  return (
    <>
      <h2>Odoo versions</h2>
      <p className="settings-lead">
        Downloaded when a session needs them and updated every week by themselves.
      </p>
      {error && (
        <div role="alert" className="banner-error">
          {error}
        </div>
      )}
      <div className="settings-card">
        {versions.downloaded.map((v) => (
          <div
            key={v.version}
            className="settings-row"
            role="group"
            aria-label={`Odoo ${versionLabel(v.version)}`}
          >
            <div className="settings-row-text">
              <span>Odoo {versionLabel(v.version)}</span>
              <small>
                {busy[v.version] ?? `${edition(v)} · ${sizeText(v.sizeBytes)} · ${updatedText(v.updated)}`}
              </small>
            </div>
            <button
              className="btn btn-small"
              disabled={!!busy[v.version] || (v.community.state === "ready" && v.community.linked)}
              onClick={() => act(v.version, "Updating…", () => api.updateOdooVersion(v.version))}
            >
              Update now
            </button>
            {!(v.community.state === "ready" && v.community.linked) && (
              <button
                className="btn btn-small"
                disabled={!!busy[v.version]}
                onClick={() => chooseFolders(v.version)}
              >
                Use a folder…
              </button>
            )}
            <button
              className="btn btn-small"
              disabled={!!busy[v.version]}
              onClick={async () => {
                const ok = await confirmAction(
                  `Remove Odoo ${versionLabel(v.version)} from this computer? Its databases stay, and it downloads again when a session needs it.`,
                  "Remove version",
                );
                if (ok) await act(v.version, "Removing…", () => api.removeOdooVersion(v.version));
              }}
            >
              Remove
            </button>
          </div>
        ))}
        {versions.downloaded.length === 0 && (
          <div className="settings-row settings-muted">No version yet.</div>
        )}
      </div>
      {addable.length > 0 && (
        <>
          <h3>Add a version</h3>
          <div className="settings-card">
            <div className="settings-row">
              <div className="settings-row-text">
                <span>Download now</span>
                <small>
                  Download it, or use a folder you already have (e.g. from odev). Otherwise it downloads the
                  first time a session uses it.
                </small>
              </div>
              <select aria-label="Version to add" value={adding} onChange={(e) => setAdding(e.target.value)}>
                <option value="">Choose…</option>
                {addable.map((v) => (
                  <option key={v} value={v}>
                    {versionLabel(v)}
                  </option>
                ))}
              </select>
              <button
                className="btn btn-small"
                disabled={!adding || !!busy[adding]}
                onClick={() =>
                  act(adding, "Downloading…", async () => {
                    await api.downloadOdoo(adding, "community");
                    setAdding("");
                  })
                }
              >
                Download
              </button>
              <button
                className="btn btn-small"
                disabled={!adding || !!busy[adding]}
                onClick={async () => {
                  const version = adding;
                  setAdding("");
                  await chooseFolders(version);
                }}
              >
                Use a folder…
              </button>
            </div>
          </div>
        </>
      )}
    </>
  );
}
