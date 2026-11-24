import { useState, type ReactNode } from "react";
import type { Setup } from "../app/useSetup";
import { pickFolder } from "../lib/pickFolder";
import type { ProviderStatus, Requirement, VersionInfo } from "../lib/types";
import { versionLabel } from "./HostingMenu";

export interface SetupScreenProps {
  setup: Setup;
  /** AI services, to offer their logins. */
  providers: ProviderStatus[];
  onLogin: (provider: string) => void;
  /** Close the setup and go to the app. */
  onDone: () => void;
}

type RowState = "done" | "working" | "todo" | "attention";

/** "17.0, 18.0, 19.0 and 8 more" */
function versionList(versions: string[]): string {
  const shown = versions.slice(0, 4).map(versionLabel).join(", ");
  return versions.length > 4 ? `${shown} and ${versions.length - 4} more` : shown;
}

/**
 * One card with the three things the app needs: the tools, Odoo's source (found on
 * the computer, never downloaded here) and an AI assistant. "Set up everything" does
 * all it can by itself (one password prompt); only what needs the person stays.
 */
export default function SetupScreen({ setup, providers, onLogin, onDone }: SetupScreenProps) {
  const s = setup.status;
  if (!s) {
    return (
      <div className="setup" role="main">
        <div className="setup-card setup-loading">
          <span className="spinner" aria-hidden="true" /> Checking this computer…
        </div>
      </div>
    );
  }
  const missing = s.system.requirements.filter((r) => !r.ok);
  const tools = setup.tools;
  const busy = tools?.state === "running";
  const nothingToDo = s.system.ready && s.found.length === 0;
  const choose = async () => {
    const path = await pickFolder("Choose your Odoo folder (e.g. an odev worktree)");
    if (path) await setup.chooseFolder(path);
  };

  return (
    <div className="setup" role="main" aria-labelledby="setup-title">
      <div className="setup-card">
        <h1 id="setup-title">Get ready to write specs</h1>
        <p className="setup-lead">
          {setup.complete
            ? "Everything is ready."
            : "One click installs what is missing. You type your password once."}
        </p>
        {setup.error && (
          <div role="alert" className="banner-error setup-error">
            {setup.error}
            <button className="icon-btn" aria-label="Dismiss" onClick={setup.dismissError}>
              ×
            </button>
          </div>
        )}

        <ol className="setup-rows">
          <Row
            label="Tools"
            state={
              busy ? "working" : s.system.ready ? "done" : tools?.state === "failed" ? "attention" : "todo"
            }
            detail={
              busy
                ? (tools?.message ?? "Installing…")
                : s.system.ready
                  ? `${listText(s.system.requirements.map((r) => r.label))} are installed.`
                  : tools?.state === "failed"
                    ? tools.message
                    : `Missing: ${missing.map((r) => r.label).join(", ")}.`
            }
            progress={busy ? null : undefined}
            details={<ToolDetails requirements={s.system.requirements} />}
          />
          <SourceRow
            versions={s.versions}
            sources={s.sources}
            found={s.found.map((f) => f.version)}
            onChoose={choose}
            onUseFound={setup.useFound}
          />
          <AiRow ready={s.aiReady} providers={providers} onLogin={onLogin} />
        </ol>

        <div className="setup-actions">
          {setup.complete ? (
            <button className="btn btn-primary setup-main" onClick={onDone}>
              Start writing specs
            </button>
          ) : (
            <button
              className="btn btn-primary setup-main"
              disabled={busy || nothingToDo}
              onClick={setup.setUpEverything}
            >
              {busy ? "Setting up…" : "Set up everything"}
            </button>
          )}
        </div>

        {!setup.complete && (
          <div className="setup-footer">
            <span />
            <button className="link-btn" onClick={onDone}>
              Skip for now
            </button>
          </div>
        )}
      </div>
    </div>
  );
}

function SourceRow({
  versions,
  sources,
  found,
  onChoose,
  onUseFound,
}: {
  versions: string[];
  sources: VersionInfo[];
  found: string[];
  onChoose: () => void;
  onUseFound: () => void;
}) {
  const choose = (
    <button className="btn btn-small" onClick={onChoose}>
      Choose a folder…
    </button>
  );
  const details = <SourceDetails sources={sources} found={found} />;
  // One version in use is enough; other folders found are only extras.
  if (versions.length > 0)
    return (
      <Row
        label="Odoo source"
        state="done"
        detail={
          <>
            Using Odoo {versionList(versions)} from your folders.
            {found.length > 0 && <> Also found: {versionList(found)} (not used yet).</>}
          </>
        }
        action={
          found.length > 0 ? (
            <button className="btn btn-small" onClick={onUseFound}>
              Use them too
            </button>
          ) : (
            choose
          )
        }
        details={details}
      />
    );
  if (found.length > 0)
    return (
      <Row
        label="Odoo source"
        state="todo"
        detail={`Found on this computer: Odoo ${versionList(found)}. Used where it is, nothing is downloaded.`}
        action={choose}
        details={details}
      />
    );
  return (
    <Row
      label="Odoo source"
      state="attention"
      detail="No Odoo source found on this computer. Choose the folder where it is (e.g. an odev worktree)."
      action={choose}
    />
  );
}

/** "A, B and C" */
function listText(items: string[]): string {
  return items.length < 2 ? items.join("") : `${items.slice(0, -1).join(", ")} and ${items.at(-1)}`;
}

function ToolDetails({ requirements }: { requirements: Requirement[] }) {
  return (
    <ul className="setup-details" aria-label="Tools details">
      {requirements.map((r) => (
        <li key={r.key} data-ok={r.ok}>
          <span aria-hidden="true">{r.ok ? "✓" : "✗"}</span>
          <span>{r.label}</span>
          <small>{r.ok ? "Installed" : r.problem}</small>
        </li>
      ))}
    </ul>
  );
}

function SourceDetails({ sources, found }: { sources: VersionInfo[]; found: string[] }) {
  const where = (v: VersionInfo) => {
    const c = v.communityPath;
    const e = v.enterprisePath;
    const parent = (p: string) => p.replace(/\/[^/]+\/?$/, "");
    return c && e && parent(c) === parent(e) ? parent(c) : [c, e].filter(Boolean).join(" · ");
  };
  return (
    <ul className="setup-details" aria-label="Odoo source details">
      {sources.map((v) => (
        <li key={v.version} data-ok="true">
          <span aria-hidden="true">✓</span>
          <span>{versionLabel(v.version)}</span>
          <small>{v.enterprise.state === "ready" ? "Community + Enterprise" : "Community"}</small>
          <code>{where(v)}</code>
        </li>
      ))}
      {found.map((v) => (
        <li key={`found-${v}`} data-ok="false">
          <span aria-hidden="true">·</span>
          <span>{versionLabel(v)}</span>
          <small>Found, not used yet</small>
        </li>
      ))}
    </ul>
  );
}

function Row({
  label,
  state,
  detail,
  progress,
  action,
  details,
}: {
  label: string;
  state: RowState;
  detail: ReactNode;
  /** null shows a moving bar; undefined shows none. */
  progress?: number | null;
  action?: ReactNode;
  /** What the step checked, shown when its title is clicked. */
  details?: ReactNode;
}) {
  const [open, setOpen] = useState(false);
  return (
    <li className="setup-row" data-state={state} aria-label={label}>
      <span className="setup-icon" aria-hidden="true">
        {state === "done" ? "✓" : state === "working" ? <span className="spinner spinner-small" /> : ""}
      </span>
      <div className="setup-row-text">
        {details ? (
          <button
            type="button"
            className="setup-row-title setup-row-toggle"
            aria-expanded={open}
            onClick={() => setOpen((o) => !o)}
          >
            {label}
            <svg viewBox="0 0 12 12" width="10" height="10" aria-hidden="true" data-open={open}>
              <path d="M4.5 3 7.5 6 4.5 9" fill="none" stroke="currentColor" strokeWidth="1.5" />
            </svg>
          </button>
        ) : (
          <span className="setup-row-title">{label}</span>
        )}
        <small>{detail}</small>
        {progress !== undefined && (
          <div
            className={"setup-bar" + (progress === null ? " setup-bar-moving" : "")}
            role="progressbar"
            aria-label={`${label} progress`}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={progress ?? undefined}
          >
            <i style={progress === null ? undefined : { width: `${progress}%` }} />
          </div>
        )}
      </div>
      {action && <div className="setup-row-action">{action}</div>}
      {open && <div className="setup-row-more">{details}</div>}
    </li>
  );
}

function AiRow({
  ready,
  providers,
  onLogin,
}: {
  ready: boolean;
  providers: ProviderStatus[];
  onLogin: (provider: string) => void;
}) {
  const details = (
    <ul className="setup-details" aria-label="AI assistant details">
      {providers.map((p) => (
        <li key={p.provider} data-ok={p.installed && p.loggedIn}>
          <span aria-hidden="true">{p.installed && p.loggedIn ? "✓" : "✗"}</span>
          <span>{p.name}</span>
          <small>
            {!p.installed
              ? "Not installed"
              : p.loggedIn
                ? "Installed, logged in"
                : "Installed, not logged in"}
          </small>
        </li>
      ))}
    </ul>
  );
  if (ready) return <Row label="AI assistant" state="done" detail="Logged in." details={details} />;
  const installed = providers.filter((p) => p.installed);
  return (
    <Row
      label="AI assistant"
      state="attention"
      detail={
        installed.length
          ? "Log in once; a window opens for it."
          : "Install Claude Code or Antigravity, then come back."
      }
      details={details}
      action={installed.map((p) => (
        <button key={p.provider} className="btn btn-small" onClick={() => onLogin(p.provider)}>
          Log in to {p.name}
        </button>
      ))}
    />
  );
}
