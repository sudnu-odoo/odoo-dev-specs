import { useState, type ReactNode } from "react";
import type { Setup } from "../app/useSetup";
import { pickFolder } from "../lib/pickFolder";
import type { ProviderStatus, SetupTask } from "../lib/types";

export interface SetupScreenProps {
  setup: Setup;
  /** AI services, to offer their logins. */
  providers: ProviderStatus[];
  onLogin: (provider: string) => void;
  /** Close the setup and go to the app. */
  onDone: () => void;
}

type RowState = "done" | "working" | "todo" | "attention" | "optional";

/**
 * One card with the four things the app needs; "Set up everything" does all it can
 * by itself (one password prompt), and only what truly needs the person stays.
 */
export default function SetupScreen({ setup, providers, onLogin, onDone }: SetupScreenProps) {
  const [folders, setFolders] = useState(false);
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
  const v = s.version.replace(/\.0$/, "");
  const missing = s.system.requirements.filter((r) => !r.ok);
  const tools = setup.tools;
  const busy = tools?.state === "running";
  const nothingToDo =
    s.system.ready &&
    (s.community.state === "ready" || setup.community?.state === "running") &&
    (s.enterprise.state === "ready" ||
      !s.github.signedIn ||
      setup.enterprise?.state === "running" ||
      setup.enterprise?.state === "noAccess");

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
                  ? "PostgreSQL, Python, Git and Chrome are installed."
                  : tools?.state === "failed"
                    ? tools.message
                    : `Missing: ${missing.map((r) => r.label).join(", ")}.`
            }
            progress={busy ? null : undefined}
          />
          <SourceRow
            label={`Odoo ${v} Community`}
            ready={s.community.state === "ready"}
            linked={s.community.state === "ready" && s.community.linked}
            task={setup.community}
            todo="About 1.5 GB, downloads in the background."
            onRetry={() => setup.download("community")}
          />
          <EnterpriseRow setup={setup} v={v} />
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
          {!setup.complete && s.system.ready && s.community.state === "ready" && (
            <p className="setup-note">Log in to an AI service to finish.</p>
          )}
          {!setup.complete && setup.community?.state === "running" && s.aiReady && s.system.ready && (
            <button className="btn setup-later" onClick={onDone}>
              Open the app while Odoo downloads
            </button>
          )}
        </div>

        {!setup.complete && (
          <div className="setup-footer">
            <button className="link-btn" aria-expanded={folders} onClick={() => setFolders((f) => !f)}>
              Use folders I already have
            </button>
            <button className="link-btn" onClick={onDone}>
              Skip for now
            </button>
          </div>
        )}
        {folders && !setup.complete && (
          <div className="setup-folders">
            <FolderChoice
              label={`Odoo ${v} Community`}
              onPick={async () => {
                const path = await pickFolder(`Choose your Odoo ${s.version} folder`);
                if (path) await setup.chooseFolder("community", path);
              }}
            />
            <FolderChoice
              label={`Odoo ${v} Enterprise`}
              onPick={async () => {
                const path = await pickFolder(`Choose your Odoo ${s.version} Enterprise folder`);
                if (path) await setup.chooseFolder("enterprise", path);
              }}
            />
          </div>
        )}
      </div>
    </div>
  );
}

function Row({
  label,
  state,
  detail,
  tag,
  progress,
  action,
}: {
  label: string;
  state: RowState;
  detail: ReactNode;
  tag?: string;
  /** A number shows a bar; null shows a moving bar; undefined shows none. */
  progress?: number | null;
  action?: ReactNode;
}) {
  return (
    <li className="setup-row" data-state={state} aria-label={label}>
      <span className="setup-icon" aria-hidden="true">
        {state === "done" ? "✓" : state === "working" ? <span className="spinner spinner-small" /> : ""}
      </span>
      <div className="setup-row-text">
        <span className="setup-row-title">
          {label}
          {tag && <span className="setup-tag">{tag}</span>}
        </span>
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
    </li>
  );
}

function SourceRow({
  label,
  ready,
  linked,
  task,
  todo,
  onRetry,
}: {
  label: string;
  ready: boolean;
  linked: boolean;
  task?: SetupTask;
  todo: string;
  onRetry: () => void;
}) {
  if (task?.state === "running") {
    const percent = task.progress;
    return (
      <Row
        label={label}
        state="working"
        detail={percent === null ? task.message : `Downloading… ${percent}%`}
        progress={percent}
      />
    );
  }
  if (ready) return <Row label={label} state="done" detail={linked ? "Using your folder." : "Ready."} />;
  if (task?.state === "failed")
    return (
      <Row
        label={label}
        state="attention"
        detail={task.message}
        action={
          <button className="btn btn-small" onClick={onRetry}>
            Try again
          </button>
        }
      />
    );
  return <Row label={label} state="todo" detail={todo} />;
}

function EnterpriseRow({ setup, v }: { setup: Setup; v: string }) {
  const s = setup.status!;
  const label = `Odoo ${v} Enterprise`;
  const task = setup.enterprise;
  if (s.enterprise.state === "ready" || task?.state === "running" || task?.state === "failed")
    return (
      <SourceRow
        label={label}
        ready={s.enterprise.state === "ready"}
        linked={s.enterprise.state === "ready" && s.enterprise.linked}
        task={task}
        todo=""
        onRetry={() => setup.download("enterprise")}
      />
    );
  if (task?.state === "noAccess")
    return (
      <Row
        label={label}
        tag="Optional"
        state="optional"
        detail="Not available yet: your GitHub account has no access. You can add it later."
        action={
          <button className="btn btn-small" onClick={() => setup.download("enterprise")}>
            Try again
          </button>
        }
      />
    );
  if (!s.github.signedIn)
    return (
      <Row
        label={label}
        tag="Optional"
        state="optional"
        detail="Needs your GitHub account. Community works without it."
        action={
          <button className="btn btn-small" onClick={setup.githubSignIn} disabled={!s.github.installed}>
            Sign in to GitHub
          </button>
        }
      />
    );
  return <Row label={label} tag="Optional" state="todo" detail="Downloads with Community." />;
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
  if (ready) return <Row label="AI assistant" state="done" detail="Logged in." />;
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
      action={installed.map((p) => (
        <button key={p.provider} className="btn btn-small" onClick={() => onLogin(p.provider)}>
          Log in to {p.name}
        </button>
      ))}
    />
  );
}

function FolderChoice({ label, onPick }: { label: string; onPick: () => void }) {
  return (
    <div className="setup-folder">
      <span>{label}</span>
      <button className="btn btn-small" onClick={onPick}>
        Choose folder…
      </button>
    </div>
  );
}
