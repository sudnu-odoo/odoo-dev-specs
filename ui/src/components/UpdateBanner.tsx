import { useEffect, useState } from "react";
import { api } from "../lib/api";
import type { AppRelease } from "../lib/types";

/**
 * "Version 0.2.0 is ready": checks GitHub once at start; Update now installs it
 * (the password window) and the app restarts. Quiet when there is nothing new.
 */
export default function UpdateBanner() {
  const [release, setRelease] = useState<AppRelease | null>(null);
  const [notes, setNotes] = useState(false);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [later, setLater] = useState(false);

  useEffect(() => {
    // No network or GitHub down: simply no banner.
    api.checkUpdate().then(setRelease, () => {});
  }, []);

  if (!release || later) return null;
  const changes = release.notes
    .split("\n")
    .map((l) => l.replace(/^[-*]\s*/, "").trim())
    .filter(Boolean);

  return (
    <div role="status" className="update-banner">
      <div className="update-row">
        <span className="update-dot" aria-hidden="true" />
        <span className="update-text">
          {error ?? (installing ? "Installing the update…" : `Version ${release.version} is ready.`)}
        </span>
        {changes.length > 0 && (
          <button className="link-btn" aria-expanded={notes} onClick={() => setNotes((n) => !n)}>
            What's new
          </button>
        )}
        <span className="composer-spacer" />
        <button className="btn btn-small" onClick={() => setLater(true)} disabled={installing}>
          Later
        </button>
        <button
          className="btn btn-small btn-primary"
          disabled={installing}
          onClick={async () => {
            setInstalling(true);
            setError(null);
            try {
              await api.installUpdate(release);
            } catch (e) {
              setError(String(e));
              setInstalling(false);
            }
          }}
        >
          {error ? "Try again" : "Update now"}
        </button>
      </div>
      {notes && (
        <ul className="update-notes">
          {changes.map((c, i) => (
            <li key={i}>{c}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
