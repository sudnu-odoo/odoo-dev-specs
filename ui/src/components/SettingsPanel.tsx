import { useCallback, useEffect, useState } from "react";
import type { PlanUsage } from "../chat/chatState";
import { api } from "../lib/api";
import type { EnvCheck, EnvReport, ProviderStatus, Settings } from "../lib/types";
import { UsageBar } from "./UsageMeter";

export interface SettingsPanelProps {
  onClose: () => void;
  /** Plan limits, once the assistant has reported them. */
  plan?: PlanUsage;
}

const STATUS_ICON = { ok: "✓", warning: "!", missing: "✗" } as const;

export default function SettingsPanel({ onClose, plan }: SettingsPanelProps) {
  const [report, setReport] = useState<EnvReport | null>(null);
  const [assistants, setAssistants] = useState<ProviderStatus[] | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saved, setSaved] = useState(true);
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

  async function login(provider: string) {
    try {
      await api.loginProvider(provider);
    } catch (e) {
      setError(String(e));
    }
  }

  function edit(change: Partial<Settings>) {
    setSettings((s) => (s ? { ...s, ...change } : s));
    setSaved(false);
  }

  async function save() {
    if (!settings) return;
    try {
      await api.saveSettings(settings);
      setSaved(true);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="settings-overlay" role="dialog" aria-label="Settings">
      <div className="settings-panel">
        <header className="settings-header">
          <h2>Settings</h2>
          <button className="icon-btn" aria-label="Close settings" onClick={onClose}>
            ×
          </button>
        </header>

        {error && (
          <div role="alert" className="banner-error">
            {error}
          </div>
        )}

        <section className="settings-section">
          <h3>AI assistants</h3>
          <ul className="env-list">
            {assistants?.map((a) => (
              <AssistantRow key={a.provider} status={a} onLogin={() => login(a.provider)} />
            ))}
            {!assistants && <li className="env-loading">Checking…</li>}
          </ul>
        </section>

        <section className="settings-section" role="region" aria-labelledby="usage-title">
          <h3 id="usage-title">Usage</h3>
          {plan?.five_hour ? (
            <div className="settings-usage">
              <UsageBar label="Current session" window={plan.five_hour} />
              {plan.seven_day && <UsageBar label="Weekly · all models" window={plan.seven_day} />}
            </div>
          ) : (
            <p className="settings-note">
              Your Claude plan limits appear here, and under the message box, after the first reply in this
              app.
            </p>
          )}
        </section>

        <section className="settings-section">
          <h3>Setup check</h3>
          <ul className="env-list">
            {report?.checks.map((c) => <EnvRow key={c.key} check={c} />)}
            {!report && <li className="env-loading">Checking…</li>}
          </ul>
        </section>

        {settings && (
          <section className="settings-section">
            <h3>Preferences</h3>
            <label className="field">
              <span>Default Odoo version</span>
              <input
                value={settings.defaultVersion}
                onChange={(e) => edit({ defaultVersion: e.target.value })}
              />
            </label>
            <label className="field field-check">
              <input
                type="checkbox"
                checked={settings.demoData}
                onChange={(e) => edit({ demoData: e.target.checked })}
              />
              <span>Load demo data in screenshot databases (realistic records)</span>
            </label>
            <div className="settings-actions">
              <button className="btn btn-primary" onClick={save} disabled={saved}>
                {saved ? "Saved" : "Save"}
              </button>
            </div>
          </section>
        )}
      </div>
    </div>
  );
}

function EnvRow({ check }: { check: EnvCheck }) {
  return (
    <li className="env-row" data-status={check.status}>
      <span className="env-icon" aria-hidden="true">
        {STATUS_ICON[check.status]}
      </span>
      <div className="env-text">
        <span className="env-label">{check.label}</span>
        <span className="env-detail">{check.detail}</span>
        {check.hint && check.status !== "ok" && <span className="env-hint">{check.hint}</span>}
      </div>
    </li>
  );
}

function AssistantRow({ status, onLogin }: { status: ProviderStatus; onLogin: () => void }) {
  const state = !status.installed ? "missing" : status.loggedIn ? "ok" : "warning";
  const text = !status.installed ? "Not installed" : status.loggedIn ? "Logged in" : "Not logged in";
  return (
    <li className="env-row" data-status={state} aria-label={status.name}>
      <span className="env-icon" aria-hidden="true">
        {STATUS_ICON[state]}
      </span>
      <div className="env-text">
        <span className="env-label">{status.name}</span>
        <span className="env-detail">{text}</span>
      </div>
      {status.installed && !status.loggedIn && (
        <button className="btn btn-small" aria-label={`Log in to ${status.name}`} onClick={onLogin}>
          Log in
        </button>
      )}
    </li>
  );
}
