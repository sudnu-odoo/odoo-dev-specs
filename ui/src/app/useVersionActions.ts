import { useCallback, useEffect, useState } from "react";
import { versionLabel } from "../components/HostingMenu";
import { api } from "../lib/api";
import { confirmAction } from "../lib/confirm";
import { pickFolder } from "../lib/pickFolder";

/**
 * What can be done to a version on this computer: update it, remove it, or use
 * folders the person already has. `busy` says what is going on for each version
 * ("Updating…"); the list is refreshed after every action.
 */
export function useVersionActions(reload: () => Promise<void>) {
  const [busy, setBusy] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);

  const act = useCallback(
    async (version: string, what: string, action: () => Promise<void>) => {
      setBusy((b) => ({ ...b, [version]: what }));
      try {
        await action();
        setError(null);
      } catch (e) {
        setError(String(e));
      } finally {
        setBusy((b) => {
          const next = { ...b };
          delete next[version];
          return next;
        });
        await reload();
      }
    },
    [reload],
  );

  const update = (version: string) => act(version, "Updating…", () => api.updateOdooVersion(version));

  const remove = async (version: string) => {
    const ok = await confirmAction(
      `Remove Odoo ${versionLabel(version)} from this computer? Its databases stay, and it downloads again when a session needs it.`,
      "Remove version",
    );
    if (ok) await act(version, "Removing…", () => api.removeOdooVersion(version));
  };

  /** Use folders the person already has (e.g. odev worktrees) for a version. */
  const chooseFolders = async (version: string) => {
    const label = versionLabel(version);
    const community = await pickFolder(`Choose your Odoo ${label} folder (Community)`);
    if (!community) return;
    await act(version, "Checking the folder…", async () => {
      await api.useOdooFolder(version, "community", community);
      const enterprise = await pickFolder(`Choose your Odoo ${label} Enterprise folder (or Cancel)`);
      if (enterprise) await api.useOdooFolder(version, "enterprise", enterprise);
    });
  };

  return { busy, error, update, remove, chooseFolders };
}

/** Whether downloading from GitHub is possible; checked again when the person comes back. */
export function useGithub() {
  const [github, setGithub] = useState<{ installed: boolean; connected: boolean } | null>(null);
  useEffect(() => {
    const check = () => api.githubStatus().then(setGithub, () => {});
    check();
    window.addEventListener("focus", check);
    return () => window.removeEventListener("focus", check);
  }, []);
  return { github, connect: () => api.githubConnect() };
}
