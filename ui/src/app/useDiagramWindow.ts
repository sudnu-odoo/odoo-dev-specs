import { useEffect, useState } from "react";
import { api } from "../lib/api";
import type { DiagramTarget } from "../lib/types";

/** The separate diagram window: follows the session open in the main window. */
export function useDiagramWindow(initial: DiagramTarget) {
  const [target, setTarget] = useState(initial);
  useEffect(() => {
    const unlisten = api.onDiagramSession(setTarget);
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);
  return { target, dock: () => api.dockDiagramWindow() };
}
