import { useEffect, useState } from "react";
import { api } from "../lib/api";
import type { EnvCheck, EnvReport, Settings } from "../lib/types";

export interface SettingsPanelProps {
  onClose: () => void;
}

const STATUS_ICON = { ok: "✓", warning: "!", missing: "✗" } as const;

export default function SettingsPanel({ onClose }: SettingsPanelProps) {
  const [report, setReport] = useState<EnvReport | null>(null);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [saved, setSaved] = useState(true);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.checkEnvironment().then(setReport, (e) => setError(String(e)));
    api.getSettings().then(setSettings, (e) => setError(String(e)));
  }, []);

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
              <span>Preferred assistant</span>
              <select
                value={settings.provider}
                onChange={(e) => edit({ provider: e.target.value as Settings["provider"] })}
              >
                <option value="claude">Claude Code</option>
              </select>
            </label>
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
