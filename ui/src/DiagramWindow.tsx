import { useEffect, useState } from "react";
import SessionDiagram from "./components/SessionDiagram";
import { api } from "./lib/api";
import type { DiagramTarget } from "./lib/types";

/** Page of the separate diagram window; follows the session open in the main window. */
export default function DiagramWindow({ initial }: { initial: DiagramTarget }) {
  const [target, setTarget] = useState(initial);

  useEffect(() => {
    const unlisten = api.onDiagramSession(setTarget);
    return () => {
      unlisten.then((stop) => stop());
    };
  }, []);

  return (
    <div className="diagram-window">
      <SessionDiagram
        sessionRef={{ project: target.project, sessionId: target.sessionId }}
        title={target.title}
        actions={
          <button
            className="btn"
            title="Show the diagram beside the chat again"
            onClick={() => api.dockDiagramWindow()}
          >
            Back to main window
          </button>
        }
      />
    </div>
  );
}
