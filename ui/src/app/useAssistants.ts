import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { ProviderStatus } from "../lib/types";

/**
 * Whether each AI assistant is installed and logged in. Logging in happens in a
 * terminal window, so it is checked again when the person comes back to the app.
 * `enabled`: only while something shows it.
 */
export function useAssistants(enabled = true) {
  const [assistants, setAssistants] = useState<ProviderStatus[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const check = useCallback(() => {
    api.providerStatus().then(
      (list) => {
        setAssistants(list);
        setError(null);
      },
      (e) => setError(String(e)),
    );
  }, []);
  useEffect(() => {
    if (!enabled) return;
    check();
    window.addEventListener("focus", check);
    return () => window.removeEventListener("focus", check);
  }, [enabled, check]);

  const login = useCallback(async (provider: string) => {
    try {
      await api.loginProvider(provider);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  return { assistants, error, login };
}
