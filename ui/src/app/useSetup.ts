import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import { confirmAction } from "../lib/confirm";
import { pickFolder } from "../lib/pickFolder";
import type { Edition, GithubStatus, SetupStatus } from "../lib/types";

/**
 * The first-run setup: what the computer is missing, and "Set up everything", which
 * installs the tools (one password prompt) and uses the Odoo source already on the
 * computer (e.g. odev's worktrees). Nothing is downloaded.
 */
export function useSetup() {
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [github, setGithub] = useState<GithubStatus | null>(null);

  // GitHub is asked apart: it can take a few seconds, the rest answers at once.
  const refresh = useCallback(async () => {
    api.githubStatus().then(setGithub, () => {});
    try {
      setStatus(await api.setupStatus());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  /** Run an action, show its error, then look at the computer again. */
  const act = useCallback(
    async (action: () => Promise<unknown>) => {
      try {
        await action();
        setError(null);
      } catch (e) {
        setError(String(e));
      }
      await refresh();
    },
    [refresh],
  );

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

  /** Reinstall PostgreSQL's programs when installing did not cure it (databases kept). */
  const repairPostgres = useCallback(async () => {
    const ok = await confirmAction(
      "Repair reinstalls PostgreSQL's programs and sets it up again. Your databases are kept.",
      "Repair PostgreSQL",
    );
    if (ok) await act(() => api.repairPostgres());
  }, [act]);

  /** Pick the folder again for a version whose folder was moved. */
  const chooseAgain = useCallback(
    async (version: string, edition: Edition) => {
      const which = edition === "community" ? "" : " Enterprise";
      const path = await pickFolder(`Choose your Odoo ${version}${which} folder`);
      if (path) await act(() => api.useOdooFolder(version, edition, path));
    },
    [act],
  );

  /** Copy a report of every check, for whoever helps; false when it could not be copied. */
  const copyReport = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(await api.setupReport());
      return true;
    } catch (e) {
      setError(String(e));
      return false;
    }
  }, []);

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
    github,
    connectGithub: () => act(() => api.githubConnect()),
    repairPostgres,
    chooseAgain,
    forgetSource: (version: string, edition: Edition) => act(() => api.forgetSource(version, edition)),
    prepare: (version: string) => act(() => api.prepareOdoo(version)),
    copyReport,
  };
}

export type Setup = ReturnType<typeof useSetup>;
