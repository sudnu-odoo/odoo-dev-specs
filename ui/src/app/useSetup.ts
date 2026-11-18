import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { Edition, SetupStatus, SetupTask } from "../lib/types";

export const SETUP_VERSION = "19.0";

const key = (edition: Edition, version = SETUP_VERSION) => `${edition}:${version}`;

/**
 * The first-run setup: what is missing, and "Set up everything", which installs the
 * tools (one password prompt) and then starts the downloads in the background.
 */
export function useSetup(version = SETUP_VERSION) {
  const [status, setStatus] = useState<SetupStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      setStatus(await api.setupStatus(version));
    } catch (e) {
      setError(String(e));
    }
  }, [version]);

  useEffect(() => {
    refresh();
    const unlisten = api.onSetupProgress(({ key: k, task }) => {
      setStatus((s) => (s ? { ...s, tasks: { ...s.tasks, [k]: task } } : s));
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

  const task = (k: string): SetupTask | undefined => status?.tasks[k];
  const running = (k: string) => task(k)?.state === "running";

  const download = useCallback(
    async (edition: Edition) => {
      try {
        await api.downloadOdoo(version, edition);
      } catch (e) {
        setError(String(e));
      }
    },
    [version],
  );

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

  /** Everything that can be done without the user, in order. */
  const setUpEverything = useCallback(async () => {
    if (!status) return;
    if (!status.system.ready && !(await installTools())) return;
    if (status.community.state === "missing" && !running(key("community", version)))
      await download("community");
    if (
      status.enterprise.state === "missing" &&
      status.github.signedIn &&
      !running(key("enterprise", version))
    )
      await download("enterprise");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [status, installTools, download, version]);

  const chooseFolder = useCallback(
    async (edition: Edition, path: string) => {
      try {
        await api.useOdooFolder(version, edition, path);
        setError(null);
      } catch (e) {
        setError(String(e));
      }
      await refresh();
    },
    [refresh, version],
  );

  const community = task(key("community", version));
  const enterprise = task(key("enterprise", version));
  const communityReady = status?.community.state === "ready" && community?.state !== "running";
  /** What a person needs before writing a spec (Enterprise is optional). */
  const complete = !!status && status.system.ready && communityReady && status.aiReady;
  /** A download going on, for the small progress ring in the sidebar. */
  const downloading = [community, enterprise].find((t) => t?.state === "running") ?? null;

  return {
    version,
    status,
    error,
    dismissError: () => setError(null),
    refresh,
    complete,
    downloading,
    community,
    enterprise,
    tools: task("tools"),
    setUpEverything,
    installTools,
    download,
    chooseFolder,
    githubSignIn: () => api.githubSignIn().catch((e) => setError(String(e))),
  };
}

export type Setup = ReturnType<typeof useSetup>;
