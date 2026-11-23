import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { EnvReport, Settings } from "../lib/types";

/**
 * The person's settings (saved at once on every change) and what the computer
 * still needs, for the Settings panel.
 */
export function useSettings() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [report, setReport] = useState<EnvReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.checkEnvironment().then(setReport, (e) => setError(String(e)));
    api.getSettings().then(setSettings, (e) => setError(String(e)));
  }, []);

  const edit = useCallback(
    async (change: Partial<Settings>) => {
      if (!settings) return;
      const next = { ...settings, ...change };
      setSettings(next);
      try {
        await api.saveSettings(next);
        setError(null);
      } catch (e) {
        setError(String(e));
      }
    },
    [settings],
  );

  // Only what needs fixing is shown; a healthy setup stays out of the way.
  const problems = report?.checks.filter((c) => c.status !== "ok" && c.key !== "ai") ?? [];

  return { settings, report, problems, error, edit };
}
