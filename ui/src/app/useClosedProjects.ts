import { useEffect, useState } from "react";

const CLOSED_KEY = "sidebar.closedProjects";

/** The projects whose sessions are hidden, remembered on this computer. */
export function useClosedProjects() {
  const [names, setNames] = useState<Set<string>>(() => {
    try {
      return new Set(JSON.parse(localStorage.getItem(CLOSED_KEY) ?? "[]") as string[]);
    } catch {
      return new Set();
    }
  });
  useEffect(() => {
    try {
      localStorage.setItem(CLOSED_KEY, JSON.stringify([...names]));
    } catch {
      // Not remembered this time; nothing else depends on it.
    }
  }, [names]);
  return {
    has: (name: string) => names.has(name),
    toggle: (name: string) =>
      setNames((n) => {
        const next = new Set(n);
        if (next.has(name)) next.delete(name);
        else next.add(name);
        return next;
      }),
    open: (name: string) =>
      setNames((n) => {
        const next = new Set(n);
        next.delete(name);
        return next;
      }),
  };
}
