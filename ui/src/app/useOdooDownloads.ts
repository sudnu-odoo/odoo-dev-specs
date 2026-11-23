import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { Edition, SetupTask } from "../lib/types";

/** One edition's download: "starting" until the app reports on it. */
export type DownloadStep = {
  state: "starting" | SetupTask["state"];
  progress: number | null;
  message: string;
};
/** A version being downloaded: each edition's step; `listed` once the list was refreshed. */
export type Download = Partial<Record<Edition, DownloadStep>> & { listed?: boolean };

const STARTING: DownloadStep = { state: "starting", progress: null, message: "" };
const failed = (e: unknown): DownloadStep => ({ state: "failed", progress: null, message: String(e) });
const isOver = (s?: DownloadStep) => !!s && s.state !== "starting" && s.state !== "running";

/**
 * Downloads started from the app: Community first (Odoo runs from it), then
 * Enterprise, each with the progress the app reports; then `reload` once, so the
 * list shows the version.
 */
export function useOdooDownloads(reload: () => Promise<void> | void) {
  const [downloads, setDownloads] = useState<Record<string, Download>>({});

  const change = useCallback((version: string, part: Partial<Download>) => {
    setDownloads((d) => ({ ...d, [version]: { ...d[version], ...part } }));
  }, []);

  // Progress sent by the app as it goes, keyed "community:18.0".
  useEffect(() => {
    const stop = api.onSetupProgress(({ key, task }) => {
      const [which, version] = key.split(":");
      if ((which === "community" || which === "enterprise") && version) change(version, { [which]: task });
    });
    return () => {
      stop.then((unlisten) => unlisten());
    };
  }, [change]);

  useEffect(() => {
    for (const [version, d] of Object.entries(downloads)) {
      if (d.community?.state === "done" && !d.enterprise) {
        change(version, { enterprise: STARTING });
        api.downloadOdoo(version, "enterprise").catch((e) => change(version, { enterprise: failed(e) }));
      }
      const over = isOver(d.community) && (d.community?.state !== "done" || isOver(d.enterprise));
      if (over && !d.listed) {
        change(version, { listed: true });
        reload();
      }
    }
  }, [downloads, change, reload]);

  const download = useCallback(
    (version: string) => {
      change(version, { community: STARTING, enterprise: undefined, listed: false });
      api.downloadOdoo(version, "community").catch((e) => change(version, { community: failed(e) }));
    },
    [change],
  );

  /** Still something to wait for. */
  const inProgress = (version: string) => {
    const d = downloads[version];
    return !!d && [d.community, d.enterprise].some((s) => s && !isOver(s));
  };

  return { downloads, download, inProgress };
}
