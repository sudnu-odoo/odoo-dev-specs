import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { SetupStatus } from "../lib/types";

/**
 * The first-run setup: what the computer is missing, and "Set up everything", which
 * installs the tools (one password prompt) and uses the Odoo source already on the
 * computer (e.g. odev's worktrees). Nothing is downloaded.
 */
export function useSetup() {
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await api.setupStatus());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    const unlisten = api.onSetupProgress(({ key, task }) => {
      setStatus((s) => (s ? { ...s, tasks: { ...s.tasks, [key]: task } } : s));
      // A step finished: what is ready has changed.
      if (task.state !== "running") refresh();
    });
    // Logins happen in other windows: look again when the user comes back.
    window.addEventListener("focus", refresh);
    return () => {
      window.removeEventListener("focus", refresh);
      unlisten.then((stop) => stop());
    };
  }, [refresh]);

  const installTools = useCallback(async () => {
    setError(null);
    try {
      await api.installTools();
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    } finally {
      await refresh();
    }
  }, [refresh]);

  const applyFound = useCallback(async () => {
    try {
      await api.useFoundSources();
      setError(null);
    } catch (e) {
      setError(String(e));
    }
    await refresh();
  }, [refresh]);

  /** Everything that can be done without the user, in order. */
  const setUpEverything = useCallback(async () => {
    if (!status) return;
    if (!status.system.ready && !(await installTools())) return;
    if (status.found.length) await applyFound();
  }, [status, installTools, applyFound]);

  /** A folder the person picked: an odev worktree folder or an odoo checkout. */
  const chooseFolder = useCallback(
    async (path: string) => {
      try {
        await api.useSourceFolder(path);
        setError(null);
      } catch (e) {
        setError(String(e));
      }
      await refresh();
    },
    [refresh],
  );

  /** What a person needs before writing a spec. */
  const complete = !!status && status.system.ready && status.versions.length > 0 && status.aiReady;
  /** A download from Settings still going on, for the small ring in the sidebar. */
  const downloading =
    Object.values(status?.tasks ?? {}).find((t) => t.state === "running" && t.progress !== null) ?? null;

  return {
    status,
    error,
    dismissError: () => setError(null),
    refresh,
    complete,
    downloading,
    tools: status?.tasks.tools,
    setUpEverything,
    /** Use the Odoo folders found on the computer too. */
    useFound: applyFound,
    installTools,
    chooseFolder,
  };
}

export type Setup = ReturnType<typeof useSetup>;
