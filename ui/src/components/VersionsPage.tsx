import { useCallback, useEffect, useState } from "react";
import { useOdooVersions } from "../app/useOdooVersions";
import { api } from "../lib/api";
import { confirmAction } from "../lib/confirm";
import { pickFolder } from "../lib/pickFolder";
import { sizeText } from "../lib/odooApps";
import type { Edition, SetupTask, VersionInfo } from "../lib/types";
import { versionLabel } from "./HostingMenu";

const edition = (v: VersionInfo) =>
  [
    v.community.state === "ready" ? "Community" : null,
    v.enterprise.state === "ready" ? "Enterprise" : null,
    v.community.state === "ready" && v.community.linked ? "your own folder" : null,
  ]
    .filter(Boolean)
    .join(" + ");

/** Where a version's files are: its folder, or each edition's when they differ. */
function whereText(v: VersionInfo): string {
  const c = v.communityPath;
  const e = v.enterprisePath;
  const parent = (p: string) => p.replace(/\/[^/]+\/?$/, "");
  if (c && e && parent(c) === parent(e)) return parent(c);
  return [c, e].filter(Boolean).join(" · ");
}

function updatedText(ms: number): string {
  if (!ms) return "never updated";
  const days = Math.floor((Date.now() - ms) / 86_400_000);
  return days <= 0 ? "updated today" : days === 1 ? "updated yesterday" : `updated ${days} days ago`;
}

/** One edition's download, as its row shows it. */
type Step = { state: "starting" | SetupTask["state"]; progress: number | null; message: string };
/** A download started here: each edition's step; `listed` once the list was reloaded. */
type Download = { community?: Step; enterprise?: Step; listed?: boolean };

const STARTING: Step = { state: "starting", progress: null, message: "" };
const failed = (e: unknown): Step => ({ state: "failed", progress: null, message: String(e) });
const isOver = (s?: Step) => !!s && s.state !== "starting" && s.state !== "running";

function stepText(which: Edition, step: Step): string {
  const name = which === "community" ? "Community" : "Enterprise";
  switch (step.state) {
    case "starting":
      return `${name}: starting…`;
    case "running":
      return step.progress === null
        ? `${name}: ${step.message.replace(/…$/, "").toLowerCase()}…`
        : `${name}: downloading… ${step.progress}%`;
    case "done":
      return `${name}: ready`;
    case "noAccess":
      return `${name}: no access (needs a GitHub account with Odoo Enterprise access)`;
    default:
      return `${name}: failed (${step.message})`;
  }
}

/** A download's steps in one line, e.g. "Community: ready · Enterprise: downloading… 40%". */
function downloadText(d: Download): string {
  return (["community", "enterprise"] as const)
    .filter((which) => d[which])
    .map((which) => stepText(which, d[which]!))
    .join(" · ");
}

/** Still something to wait for (shows a spinner). */
const inProgress = (d?: Download) => !!d && [d.community, d.enterprise].some((s) => s && !isOver(s));

/**
 * The Odoo versions on this computer, in Settings: each with where its files are, its
 * size and last update, Update now and Remove. Another version can be downloaded
 * (Community, then Enterprise, each with its progress) or taken from a folder.
 */
export default function VersionsPage() {
  const versions = useOdooVersions();
  const [busy, setBusy] = useState<Record<string, string>>({});
  const [adding, setAdding] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [downloads, setDownloads] = useState<Record<string, Download>>({});

  const update = useCallback((version: string, change: Partial<Download>) => {
    setDownloads((d) => ({ ...d, [version]: { ...d[version], ...change } }));
  }, []);

  // Progress of the downloads, sent by the app as they go ("community:18.0").
  useEffect(() => {
    const stop = api.onSetupProgress(({ key, task }) => {
      const [which, version] = key.split(":");
      if ((which === "community" || which === "enterprise") && version) update(version, { [which]: task });
    });
    return () => {
      stop.then((unlisten) => unlisten());
    };
  }, [update]);

  // Community first (Odoo runs from it), then Enterprise; then the list shows the version.
  const { reload } = versions;
  useEffect(() => {
    for (const [version, d] of Object.entries(downloads)) {
      if (d.community?.state === "done" && !d.enterprise) {
        update(version, { enterprise: STARTING });
        api.downloadOdoo(version, "enterprise").catch((e) => update(version, { enterprise: failed(e) }));
      }
      const over = isOver(d.community) && (d.community?.state !== "done" || isOver(d.enterprise));
      if (over && !d.listed) {
        update(version, { listed: true });
        reload();
      }
    }
  }, [downloads, update, reload]);

  function download(version: string) {
    update(version, { community: STARTING, enterprise: undefined, listed: false });
    api.downloadOdoo(version, "community").catch((e) => update(version, { community: failed(e) }));
  }

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

  // Downloading needs GitHub; using your own folders does not.
  const [github, setGithub] = useState<{ installed: boolean; connected: boolean } | null>(null);
  useEffect(() => {
    const check = () => api.githubStatus().then(setGithub, () => {});
    check();
    window.addEventListener("focus", check);
    return () => window.removeEventListener("focus", check);
  }, []);

  const here = new Set(versions.downloaded.map((v) => v.version));
  // Versions being downloaded show in the list right away, before they are here.
  const pending = Object.keys(downloads).filter((v) => !here.has(v));
  const addable = versions.available.filter((v) => !here.has(v) && !downloads[v]);
  const folder = versions.downloadsFolder;

  return (
    <>
      <h2>Odoo versions</h2>
      <p className="settings-lead">
        Downloaded when a session needs them and updated every week by themselves.
        {folder && (
          <>
            {" "}
            Downloads go to <code>{folder}/&lt;version&gt;/odoo</code> (Community) and{" "}
            <code>/enterprise</code> (Enterprise).
          </>
        )}
      </p>
      {error && (
        <div role="alert" className="banner-error">
          {error}
        </div>
      )}
      <div className="settings-card">
        {versions.downloaded.map((v) => {
          const d = downloads[v.version];
          const own = v.community.state === "ready" && v.community.linked;
          return (
            <div
              key={v.version}
              className="settings-row"
              role="group"
              aria-label={`Odoo ${versionLabel(v.version)}`}
              aria-busy={inProgress(d) || !!busy[v.version] || undefined}
            >
              <div className="settings-row-text">
                <span>Odoo {versionLabel(v.version)}</span>
                <small>
                  {busy[v.version] ?? `${edition(v)} · ${sizeText(v.sizeBytes)} · ${updatedText(v.updated)}`}
                </small>
                <small className="settings-path">{whereText(v)}</small>
                {d && <small>{downloadText(d)}</small>}
              </div>
              {(inProgress(d) || busy[v.version]) && <span className="spinner" aria-hidden="true" />}
              <button
                className="btn btn-small"
                disabled={!!busy[v.version] || inProgress(d) || own}
                onClick={() => act(v.version, "Updating…", () => api.updateOdooVersion(v.version))}
              >
                Update now
              </button>
              {!own && (
                <button
                  className="btn btn-small"
                  disabled={!!busy[v.version] || inProgress(d)}
                  onClick={() => chooseFolders(v.version)}
                >
                  Use a folder…
                </button>
              )}
              <button
                className="btn btn-small"
                disabled={!!busy[v.version] || inProgress(d)}
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
          );
        })}
        {pending.map((version) => (
          <div
            key={version}
            className="settings-row"
            role="group"
            aria-label={`Odoo ${versionLabel(version)}`}
            aria-busy={inProgress(downloads[version]) || undefined}
          >
            <div className="settings-row-text">
              <span>Odoo {versionLabel(version)}</span>
              <small>{downloadText(downloads[version])}</small>
              {folder && <small className="settings-path">{`${folder}/${version}`}</small>}
            </div>
            {inProgress(downloads[version]) && <span className="spinner" aria-hidden="true" />}
          </div>
        ))}
        {versions.loading && (
          <div className="settings-row settings-muted" role="status">
            <span className="spinner" aria-hidden="true" /> Looking for the versions on this computer…
          </div>
        )}
        {!versions.loading && versions.downloaded.length === 0 && pending.length === 0 && (
          <div className="settings-row settings-muted">No version yet.</div>
        )}
      </div>
      <h3>Download from GitHub</h3>
      <div className="settings-card">
        <div className="settings-row" role="group" aria-label="GitHub">
          <div className="settings-row-text">
            <span>GitHub</span>
            <small>
              {github?.connected
                ? "Connected: versions can be downloaded and are updated every week."
                : "Optional: only needed to download versions. A window shows a code for github.com/login/device."}
            </small>
          </div>
          {github && !github.connected && (
            <button
              className="btn btn-small"
              disabled={!github.installed}
              onClick={() => api.githubConnect()}
            >
              Connect GitHub
            </button>
          )}
        </div>
      </div>
      <h3>Add a version</h3>
      <div className="settings-card">
        {versions.publishedLoading && versions.available.length === 0 ? (
          <div className="settings-row settings-muted" role="status">
            <span className="spinner" aria-hidden="true" /> Loading the versions Odoo publishes…
          </div>
        ) : versions.publishedError && versions.available.length === 0 ? (
          <div className="settings-row settings-muted">
            Could not get the versions from GitHub: {versions.publishedError} The versions above keep working;
            downloading needs GitHub.
          </div>
        ) : addable.length === 0 ? (
          <div className="settings-row settings-muted">Every version Odoo publishes is here.</div>
        ) : (
          <div className="settings-row">
            <div className="settings-row-text">
              <span>Download now</span>
              <small>
                Community, then Enterprise if your GitHub account has access. Or use a folder you already have
                (e.g. from odev).
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
              disabled={!adding}
              onClick={() => {
                download(adding);
                setAdding("");
              }}
            >
              Download
            </button>
            <button
              className="btn btn-small"
              disabled={!adding}
              onClick={async () => {
                const version = adding;
                setAdding("");
                await chooseFolders(version);
              }}
            >
              Use a folder…
            </button>
          </div>
        )}
      </div>
    </>
  );
}
