import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import VersionsPage from "./VersionsPage";
import { useOdooVersions } from "../app/useOdooVersions";
import { newestFirst } from "../lib/versions";
import { setCanvasTheme, useCanvasTheme, type CanvasTheme } from "../lib/canvasTheme";
import type { EnvCheck, EnvReport, ProviderStatus, Settings } from "../lib/types";

export interface SettingsPanelProps {
  onClose: () => void;
  /** Open the first-run setup again (check the computer, downloads, logins). */
  onOpenSetup?: () => void;
}

const VERSIONS = ["19.0", "18.0", "17.0"];

type Page = "general" | "versions" | "ai";

/**
 * Settings laid out like Antigravity's: a few pages on the left; on the right, titled
 * cards of rows, each a name, one grey line and one control. Changes save at once.
 */
export default function SettingsPanel({ onClose, onOpenSetup }: SettingsPanelProps) {
  const [page, setPage] = useState<Page>("general");
  const canvasTheme = useCanvasTheme();
  const odoo = useOdooVersions();
  const [report, setReport] = useState<EnvReport | null>(null);
  const [assistants, setAssistants] = useState<ProviderStatus[] | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.checkEnvironment().then(setReport, (e) => setError(String(e)));
    api.getSettings().then(setSettings, (e) => setError(String(e)));
  }, []);

  const checkAssistants = useCallback(() => {
    api.providerStatus().then(setAssistants, (e) => setError(String(e)));
  }, []);
  useEffect(() => {
    checkAssistants();
    // Logging in happens in a terminal window: re-check when the user comes back.
    window.addEventListener("focus", checkAssistants);
    return () => window.removeEventListener("focus", checkAssistants);
  }, [checkAssistants]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  async function login(provider: string) {
    try {
      await api.loginProvider(provider);
    } catch (e) {
      setError(String(e));
    }
  }

  async function edit(change: Partial<Settings>) {
    if (!settings) return;
    const next = { ...settings, ...change };
    setSettings(next);
    try {
      await api.saveSettings(next);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  // Only what needs fixing is shown; a healthy setup stays out of the way.
  const problems = report?.checks.filter((c) => c.status !== "ok" && c.key !== "ai") ?? [];
  // The versions on this computer (the usual ones until it has any).
  const local = newestFirst(odoo.downloaded.map((v) => v.version));
  const offered = local.length ? local : VERSIONS;
  const versions =
    settings && !offered.includes(settings.defaultVersion) ? [settings.defaultVersion, ...offered] : offered;

  return (
    <div
      className="settings-overlay"
      role="dialog"
      aria-label="Settings"
      onMouseDown={(e) => e.target === e.currentTarget && onClose()}
    >
      <div className="settings-panel">
        <nav className="settings-nav" aria-label="Settings pages">
          <div className="settings-nav-title">Settings</div>
          <button aria-current={page === "general" ? "page" : undefined} onClick={() => setPage("general")}>
            General
          </button>
          <button aria-current={page === "versions" ? "page" : undefined} onClick={() => setPage("versions")}>
            Odoo versions
          </button>
          <button aria-current={page === "ai" ? "page" : undefined} onClick={() => setPage("ai")}>
            AI services
          </button>
        </nav>
        <div className="settings-body">
          <button className="icon-btn settings-close" aria-label="Close settings" onClick={onClose}>
            ×
          </button>
          {error && (
            <div role="alert" className="banner-error">
              {error}
            </div>
          )}

          {page === "general" && (
            <>
              <h2>General</h2>
              <p className="settings-lead">The local Odoo used for screenshots.</p>
              {settings && (
                <div className="settings-card">
                  <Row label="Odoo version" hint="For new specs; the versions on this computer">
                    <select
                      aria-label="Odoo version"
                      value={settings.defaultVersion}
                      onChange={(e) => edit({ defaultVersion: e.target.value })}
                    >
                      {versions.map((v) => (
                        <option key={v}>{v}</option>
                      ))}
                    </select>
                  </Row>
                  <Row label="Demo data" hint="Screenshots show example records">
                    <div className="segmented" role="radiogroup" aria-label="Demo data">
                      {[true, false].map((on) => (
                        <button
                          key={String(on)}
                          role="radio"
                          aria-checked={settings.demoData === on}
                          onClick={() => edit({ demoData: on })}
                        >
                          {on ? "On" : "Off"}
                        </button>
                      ))}
                    </div>
                  </Row>
                </div>
              )}
              <h3>Appearance</h3>
              <div className="settings-card">
                <Row label="Theme" hint="The app and the diagram. Screenshots always keep their real colours">
                  <div className="segmented" role="radiogroup" aria-label="Theme">
                    {(["light", "dark", "system"] as CanvasTheme[]).map((t) => (
                      <button
                        key={t}
                        role="radio"
                        aria-checked={canvasTheme === t}
                        onClick={() => setCanvasTheme(t)}
                      >
                        {t[0].toUpperCase() + t.slice(1)}
                      </button>
                    ))}
                  </div>
                </Row>
              </div>
              {onOpenSetup && (
                <>
                  <h3>Setup</h3>
                  <div className="settings-card">
                    <Row label="Check this computer" hint="Tools, Odoo source and AI logins">
                      <button className="btn btn-small" onClick={onOpenSetup}>
                        Open setup
                      </button>
                    </Row>
                  </div>
                </>
              )}
              {problems.length > 0 && (
                <>
                  <h3>Needs attention</h3>
                  <div className="settings-card">
                    {problems.map((c) => (
                      <ProblemRow key={c.key} check={c} />
                    ))}
                  </div>
                </>
              )}
            </>
          )}

          {page === "versions" && <VersionsPage />}

          {page === "ai" && (
            <>
              <h2>AI services</h2>
              <p className="settings-lead">Pick the model in the chat; log in here or there.</p>
              <div className="settings-card">
                {assistants?.map((a) => (
                  <AssistantRow key={a.provider} status={a} onLogin={() => login(a.provider)} />
                ))}
                {!assistants && <div className="settings-row settings-muted">Checking…</div>}
              </div>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

function Row({ label, hint, children }: { label: string; hint: string; children: React.ReactNode }) {
  return (
    <div className="settings-row">
      <div className="settings-row-text">
        <span>{label}</span>
        <small>{hint}</small>
      </div>
      {children}
    </div>
  );
}

function ProblemRow({ check }: { check: EnvCheck }) {
  return (
    <div className="settings-row" data-status={check.status}>
      <div className="settings-row-text">
        <span>{check.label}</span>
        <small>{check.hint ?? check.detail}</small>
      </div>
      <span className="status-pill" data-status={check.status}>
        {check.status === "missing" ? "Missing" : "Check"}
      </span>
    </div>
  );
}

function AssistantRow({ status, onLogin }: { status: ProviderStatus; onLogin: () => void }) {
  const hint = status.provider === "claude" ? "Claude Code CLI" : "Antigravity CLI (agy)";
  return (
    <div className="settings-row" role="group" aria-label={status.name}>
      <div className="settings-row-text">
        <span>{status.name}</span>
        <small>{hint}</small>
      </div>
      {!status.installed ? (
        <span className="status-pill">Not installed</span>
      ) : status.loggedIn ? (
        <span className="status-pill" data-status="ok">
          Connected
        </span>
      ) : (
        <button className="btn btn-small" aria-label={`Log in to ${status.name}`} onClick={onLogin}>
          Log in
        </button>
      )}
    </div>
  );
}
