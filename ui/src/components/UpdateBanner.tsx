import { useState } from "react";
import { useAppUpdate } from "../app/useAppUpdate";

/**
 * "Version 0.2.0 is ready": checks GitHub once at start; Update now installs it
 * (the password window) and the app restarts. Quiet when there is nothing new.
 */
export default function UpdateBanner() {
  const { release, changes, installing, error, install, postpone } = useAppUpdate();
  const [notes, setNotes] = useState(false);
  if (!release) return null;

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
        <button className="btn btn-small" onClick={postpone} disabled={installing}>
          Later
        </button>
        <button className="btn btn-small btn-primary" disabled={installing} onClick={install}>
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
