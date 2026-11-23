import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { AppRelease } from "../lib/types";

/**
 * A newer version of the app: checked once at start (quietly nothing without a
 * network); `install` runs the update (the password window), then the app restarts.
 */
export function useAppUpdate() {
  const [release, setRelease] = useState<AppRelease | null>(null);
  const [installing, setInstalling] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [later, setLater] = useState(false);

  useEffect(() => {
    api.checkUpdate().then(setRelease, () => {});
  }, []);

  const install = useCallback(async () => {
    if (!release) return;
    setInstalling(true);
    setError(null);
    try {
      await api.installUpdate(release);
    } catch (e) {
      setError(String(e));
      setInstalling(false);
    }
  }, [release]);

  /** What changed, one line per change. */
  const changes = (release?.notes ?? "")
    .split("\n")
    .map((l) => l.replace(/^[-*]\s*/, "").trim())
    .filter(Boolean);

  return {
    release: later ? null : release,
    changes,
    installing,
    error,
    install,
    postpone: () => setLater(true),
  };
}
