import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import type { OdooVersions } from "../lib/types";

const EMPTY: OdooVersions = { available: [], latestSaas: null, downloaded: [] };

/** The versions Odoo publishes and the ones on this computer; refreshed on focus. */
export function useOdooVersions() {
  const [versions, setVersions] = useState<OdooVersions>(EMPTY);
  const reload = useCallback(async () => {
    try {
      setVersions(await api.odooVersions());
    } catch {
      // Not connected yet or offline: keep what we have.
    }
  }, []);
  useEffect(() => {
    reload();
    // Coming back from the GitHub sign-in makes the list available.
    window.addEventListener("focus", reload);
    return () => window.removeEventListener("focus", reload);
  }, [reload]);
  return { ...versions, reload };
}
