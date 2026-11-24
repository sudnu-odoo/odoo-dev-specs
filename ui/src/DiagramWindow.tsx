import { useAppTheme } from "./lib/canvasTheme";
import { useAppTextSize } from "./lib/textSize";
import SessionDiagram from "./components/SessionDiagram";
import { useDiagramWindow } from "./app/useDiagramWindow";
import type { DiagramTarget } from "./lib/types";

/** Page of the separate diagram window; follows the session open in the main window. */
export default function DiagramWindow({ initial }: { initial: DiagramTarget }) {
  useAppTheme();
  useAppTextSize();
  const { target, dock } = useDiagramWindow(initial);

  return (
    <div className="diagram-window">
      <SessionDiagram
        sessionRef={{ project: target.project, sessionId: target.sessionId }}
        title={target.title}
        actions={
          <button className="btn" title="Show the diagram beside the chat again" onClick={dock}>
            Back to main window
          </button>
        }
      />
    </div>
  );
}
