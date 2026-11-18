import { useCallback, useEffect, useState } from "react";
import { api } from "../lib/api";
import { confirmAction } from "../lib/confirm";
import { openExternal } from "../lib/openExternal";
import type { DatabaseForm, DatabaseInfo, NewDatabase } from "../lib/types";

/** What is happening to a database right now, shown on its row. */
export type DatabaseBusy = "starting" | "stopping" | "copying" | "resetting" | "deleting";

/** The local Odoo databases and the actions on them. */
export function useDatabases() {
  const [databases, setDatabases] = useState<DatabaseInfo[]>([]);
  const [busy, setBusy] = useState<Record<string, DatabaseBusy>>({});
  const [creating, setCreating] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      setDatabases(await api.listDatabases());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  /** Run an action on one database, marking it busy and reloading the list after. */
  const act = useCallback(
    async <T>(name: string, what: DatabaseBusy, action: () => Promise<T>): Promise<T | undefined> => {
      setBusy((b) => ({ ...b, [name]: what }));
      try {
        const out = await action();
        setError(null);
        return out;
      } catch (e) {
        setError(String(e));
        return undefined;
      } finally {
        setBusy((b) => {
          const next = { ...b };
          delete next[name];
          return next;
        });
        await reload();
      }
    },
    [reload],
  );

  const create = useCallback(
    async (database: NewDatabase) => {
      setCreating(true);
      try {
        const made = await api.createDatabase(database);
        setError(null);
        return made;
      } catch (e) {
        setError(String(e));
        return undefined;
      } finally {
        setCreating(false);
        await reload();
      }
    },
    [reload],
  );

  const open = useCallback(
    (db: DatabaseInfo) =>
      act(db.name, "starting", async () => {
        await openExternal(await api.startDatabase(db.name));
      }),
    [act],
  );

  const stop = useCallback(
    (db: DatabaseInfo) => act(db.name, "stopping", () => api.stopDatabase(db.name)),
    [act],
  );

  const rename = useCallback(
    async (db: DatabaseInfo, label: string) => {
      try {
        await api.renameDatabase(db.name, label);
        setError(null);
      } catch (e) {
        setError(String(e));
      }
      await reload();
    },
    [reload],
  );

  const duplicate = useCallback(
    (db: DatabaseInfo, label: string) => act(db.name, "copying", () => api.duplicateDatabase(db.name, label)),
    [act],
  );

  const reset = useCallback(
    async (db: DatabaseInfo) => {
      const ok = await confirmAction(
        `Reset "${db.label}" to a fresh database? Everything you changed in it is lost.`,
        "Reset database",
      );
      if (ok) await act(db.name, "resetting", () => api.resetDatabase(db.name));
    },
    [act],
  );

  const remove = useCallback(
    async (db: DatabaseInfo, usedBy: number) => {
      const sessions = usedBy
        ? ` ${usedBy} session${usedBy > 1 ? "s use" : " uses"} it and will get a fresh database instead.`
        : "";
      const ok = await confirmAction(
        `Delete "${db.label}"? This cannot be undone.${sessions}`,
        "Delete database",
      );
      if (ok) await act(db.name, "deleting", () => api.deleteDatabase(db.name));
    },
    [act],
  );

  /** What the form suggests for a version; a plain default when it cannot be asked. */
  const formFor = useCallback(
    (version: string): Promise<DatabaseForm> =>
      api.databaseForm(version).catch(() => ({ suggestedLabel: "", enterpriseAvailable: false })),
    [],
  );

  return {
    databases,
    formFor,
    busy,
    creating,
    error,
    dismissError: () => setError(null),
    reload,
    create,
    open,
    stop,
    rename,
    duplicate,
    reset,
    remove,
  };
}

export type Databases = ReturnType<typeof useDatabases>;
