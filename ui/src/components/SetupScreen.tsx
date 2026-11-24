import { useState, type ReactNode } from "react";
import type { Setup } from "../app/useSetup";
import { pickFolder } from "../lib/pickFolder";
import type {
  BrokenSource,
  Edition,
  GithubStatus,
  ProviderStatus,
  Requirement,
  SetupTask,
  VersionInfo,
} from "../lib/types";
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
  const [copied, setCopied] = useState(false);
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
        <div className="setup-tools">
          <button className="btn btn-small" onClick={setup.refresh}>
            Check again
          </button>
          <button
            className="btn btn-small"
            onClick={async () => {
              if (await setup.copyReport()) {
                setCopied(true);
                setTimeout(() => setCopied(false), 2000);
              }
            }}
          >
            {copied ? "Copied" : "Copy a report for support"}
          </button>
        </div>
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
            details={
              <ToolDetails
                requirements={s.system.requirements}
                // The usual fixes ran and PostgreSQL still fails: offer the repair.
                onRepair={tools?.state === "failed" ? setup.repairPostgres : undefined}
              />
            }
          />
          <SourceRow
            versions={s.versions}
            sources={s.sources}
            broken={s.broken}
            found={s.found.map((f) => f.version)}
            tasks={s.tasks}
            onChoose={choose}
            onUseFound={setup.useFound}
            onChooseAgain={setup.chooseAgain}
            onForget={setup.forgetSource}
            onPrepare={setup.prepare}
          />
          <AiRow ready={s.aiReady} providers={providers} onLogin={onLogin} />
          <GithubRow github={setup.github} onConnect={setup.connectGithub} />
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
  broken,
  found,
  tasks,
  onChoose,
  onUseFound,
  onChooseAgain,
  onForget,
  onPrepare,
}: {
  versions: string[];
  sources: VersionInfo[];
  broken: BrokenSource[];
  found: string[];
  tasks: Record<string, SetupTask>;
  onChoose: () => void;
  onUseFound: () => void;
  onChooseAgain: (version: string, edition: Edition) => void;
  onForget: (version: string, edition: Edition) => void;
  onPrepare: (version: string) => void;
}) {
  const choose = (
    <button className="btn btn-small" onClick={onChoose}>
      Choose a folder…
    </button>
  );
  const details = <SourceDetails sources={sources} found={found} tasks={tasks} onPrepare={onPrepare} />;
  // A folder that was moved needs the person: say which, and offer both ways out.
  if (broken.length > 0)
    return (
      <Row
        label="Odoo source"
        state="attention"
        detail={
          <span className="setup-broken">
            {broken.map((b) => (
              <span key={`${b.version}-${b.edition}`} className="setup-broken-item">
                {versionLabel(b.version)} {b.edition === "community" ? "Community" : "Enterprise"}: its folder
                was moved or deleted ({b.path}).
                <button className="btn btn-small" onClick={() => onChooseAgain(b.version, b.edition)}>
                  Choose again
                </button>
                <button className="btn btn-small" onClick={() => onForget(b.version, b.edition)}>
                  Forget it
                </button>
              </span>
            ))}
          </span>
        }
        details={sources.length ? details : undefined}
      />
    );
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

/** Help text whose `commands` show as code. */
function withCommands(text: string): ReactNode {
  return text.split(/`([^`]+)`/).map((part, i) => (i % 2 ? <code key={i}>{part}</code> : part));
}

/** "A, B and C" */
function listText(items: string[]): string {
  return items.length < 2 ? items.join("") : `${items.slice(0, -1).join(", ")} and ${items.at(-1)}`;
}

function ToolDetails({ requirements, onRepair }: { requirements: Requirement[]; onRepair?: () => void }) {
  return (
    <ul className="setup-details" aria-label="Tools details">
      {requirements.map((r) => (
        <li key={r.key} data-ok={r.ok}>
          <span aria-hidden="true">{r.ok ? "✓" : "✗"}</span>
          <span>{r.label}</span>
          <small>{r.ok ? "Installed" : r.problem}</small>
          {!r.ok && r.help && (
            <p className="setup-help">
              {r.fix === "manual" && <strong>You fix this one: </strong>}
              {withCommands(r.help)}
            </p>
          )}
          {!r.ok && r.key === "postgresql" && r.fix === "automatic" && onRepair && (
            <div className="setup-help">
              <button className="btn btn-small" onClick={onRepair}>
                Repair PostgreSQL
              </button>{" "}
              Reinstalls it and keeps your databases.
            </div>
          )}
        </li>
      ))}
    </ul>
  );
}

function SourceDetails({
  sources,
  found,
  tasks,
  onPrepare,
}: {
  sources: VersionInfo[];
  found: string[];
  tasks: Record<string, SetupTask>;
  onPrepare: (version: string) => void;
}) {
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
          <PythonState
            version={v.version}
            ready={v.pythonReady}
            task={tasks[`python:${v.version}`]}
            onPrepare={onPrepare}
          />
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

/** Whether a version's Python packages are installed, with a way to install them. */
function PythonState({
  version,
  ready,
  task,
  onPrepare,
}: {
  version: string;
  ready: boolean;
  task?: SetupTask;
  onPrepare: (version: string) => void;
}) {
  if (task?.state === "running")
    return (
      <p className="setup-help">
        <span className="spinner spinner-small" aria-hidden="true" /> Installing Python packages…
      </p>
    );
  if (task?.state === "failed")
    return (
      <p className="setup-help">
        Python packages failed: {task.message}{" "}
        <button className="btn btn-small" onClick={() => onPrepare(version)}>
          Try again
        </button>
      </p>
    );
  if (ready) return <p className="setup-help">Python packages ready.</p>;
  return (
    <p className="setup-help">
      Python packages not installed yet (done on first use, or now:{" "}
      <button className="btn btn-small" onClick={() => onPrepare(version)}>
        Prepare
      </button>
      ).
    </p>
  );
}

/** GitHub is only needed to download versions: never blocks the setup. */
function GithubRow({ github, onConnect }: { github: GithubStatus | null; onConnect: () => void }) {
  if (!github)
    return <Row label="GitHub" state="working" detail="Checking GitHub… (optional, for downloads)" />;
  if (github.unreachable)
    return (
      <Row
        label="GitHub"
        state="todo"
        detail="GitHub does not answer: check the internet connection, then check again. Only needed to download versions."
      />
    );
  if (!github.connected)
    return (
      <Row
        label="GitHub"
        state="todo"
        detail="Optional: connect it to download Odoo versions. A window shows a code for github.com/login/device."
        action={
          <button className="btn btn-small" onClick={onConnect} disabled={!github.installed}>
            Connect GitHub
          </button>
        }
      />
    );
  return (
    <Row
      label="GitHub"
      state="done"
      detail={
        github.enterprise
          ? "Connected: Community and Enterprise can be downloaded."
          : "Connected for Community. Your account cannot read Odoo Enterprise: ask for access, or use a folder you have."
      }
    />
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
