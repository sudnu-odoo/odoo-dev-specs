import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../lib/api";
import type { VersionInfo } from "../lib/types";

export interface OdooVersionsState {
  /** The versions on this computer. */
  downloaded: VersionInfo[];
  downloadsFolder: string;
  /** True until the versions on this computer are known (quick). */
  loading: boolean;
  /** The versions Odoo publishes on GitHub, newest first; empty until known. */
  available: string[];
  latestSaas: string | null;
  /** True while GitHub's list is being fetched (can take a few seconds). */
  publishedLoading: boolean;
  /** Why GitHub's list could not be fetched (not connected, offline). */
  publishedError: string | null;
  reload: () => Promise<void>;
}

/**
 * The versions on this computer, at once, and the ones Odoo publishes, which come
 * from GitHub and may take a while: each has its own loading state. Refreshed on focus.
 */
export function useOdooVersions(): OdooVersionsState {
  const [local, setLocal] = useState({ downloaded: [] as VersionInfo[], downloadsFolder: "" });
  const [loading, setLoading] = useState(true);
  const [published, setPublished] = useState({
    available: [] as string[],
    latestSaas: null as string | null,
  });
  const [publishedLoading, setPublishedLoading] = useState(true);
  const [publishedError, setPublishedError] = useState<string | null>(null);
  const asking = useRef(false);

  const reloadLocal = useCallback(async () => {
    try {
      setLocal(await api.odooVersions());
    } catch {
      // Keep what we have.
    } finally {
      setLoading(false);
    }
  }, []);

  const reloadPublished = useCallback(async () => {
    if (asking.current) return;
    asking.current = true;
    setPublishedLoading(true);
    try {
      setPublished(await api.odooPublishedVersions());
      setPublishedError(null);
    } catch (e) {
      setPublishedError(String(e));
    } finally {
      asking.current = false;
      setPublishedLoading(false);
    }
  }, []);

  // What changed is usually here (a download, a folder); GitHub's list follows.
  const reload = useCallback(async () => {
    await reloadLocal();
    reloadPublished();
  }, [reloadLocal, reloadPublished]);

  useEffect(() => {
    reloadLocal();
    reloadPublished();
    // Coming back from the GitHub sign-in makes the list available.
    const again = () => {
      reloadLocal();
      reloadPublished();
    };
    window.addEventListener("focus", again);
    return () => window.removeEventListener("focus", again);
  }, [reloadLocal, reloadPublished]);

  return {
    ...local,
    loading,
    ...published,
    publishedLoading,
    publishedError,
    reload,
  };
}
