import type { ReactNode } from "react";
import { useSessionScene } from "../app/useSessionScene";
import type { SessionRef } from "../lib/types";
import ExcalidrawPanel from "./ExcalidrawPanel";

export interface SessionDiagramProps {
  sessionRef: SessionRef;
  title: string;
  /** Extra toolbar buttons (e.g. open in a new window). */
  actions?: ReactNode;
}

/**
 * The diagram of one session. The canvas only ever starts from the scene of the
 * session it shows.
 */
export default function SessionDiagram({ sessionRef, title, actions }: SessionDiagramProps) {
  const diagram = useSessionScene(sessionRef);
  if (diagram.loadError) {
    return (
      <div role="alert" className="artifact-placeholder">
        {diagram.loadError}
      </div>
    );
  }
  if (diagram.scene === null) return <div className="artifact-placeholder">Loading diagram…</div>;
  return (
    <div className="session-diagram">
      {diagram.saveError && (
        <div role="alert" className="banner-error">
          {diagram.saveError}
        </div>
      )}
      <ExcalidrawPanel
        key={diagram.key}
        name={title}
        scene={diagram.scene}
        incoming={diagram.incoming}
        holdSaves={diagram.assistantBusy}
        actions={actions}
        onSceneChange={diagram.save}
      />
    </div>
  );
}
