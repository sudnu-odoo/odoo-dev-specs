import { useEffect, useState, type ReactNode } from "react";
import { api } from "../lib/api";
import type { SessionRef } from "../lib/types";
import ExcalidrawPanel, { type IncomingScene } from "./ExcalidrawPanel";

export interface SessionDiagramProps {
  sessionRef: SessionRef;
  title: string;
  /** Extra toolbar buttons (e.g. open in a new window). */
  actions?: ReactNode;
}

type Loaded = { key: string; scene: string } | { key: string; error: string };

/**
 * The diagram of one session: loads its scene from disk and saves edits back.
 * The canvas only ever starts from the scene of the session it shows.
 */
export default function SessionDiagram({ sessionRef, title, actions }: SessionDiagramProps) {
  const { project, sessionId } = sessionRef;
  const key = `${project}/${sessionId}`;
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [incoming, setIncoming] = useState<IncomingScene | undefined>(undefined);

  useEffect(() => {
    let current = true; // ignore a late answer for a session we already left
    api.loadScene({ project, sessionId }).then(
      (scene) => current && setLoaded({ key, scene }),
      (e) => current && setLoaded({ key, error: String(e) }),
    );
    return () => {
      current = false;
    };
  }, [key, project, sessionId]);

  // The assistant edits the diagram file through its tools: pull each change in.
  useEffect(() => {
    setIncoming(undefined);
    let current = true;
    const unlisten = api.onChatEvent(({ sessionId: from, event }) => {
      if (from !== sessionId || (event.kind !== "tool_finished" && event.kind !== "finished")) return;
      api.loadScene({ project, sessionId }).then(
        (scene) => current && setIncoming((prev) => ({ scene, revision: (prev?.revision ?? 0) + 1 })),
        () => {}, // the next event or a session switch reloads anyway
      );
    });
    return () => {
      current = false;
      unlisten.then((stop) => stop());
    };
  }, [project, sessionId]);

  if (!loaded || loaded.key !== key) {
    return <div className="artifact-placeholder">Loading diagram…</div>;
  }
  if ("error" in loaded) {
    return (
      <div role="alert" className="artifact-placeholder">
        {loaded.error}
      </div>
    );
  }
  const save = (scene: string) =>
    api.saveScene({ project, sessionId }, scene).then(
      () => setSaveError(null),
      (e) => setSaveError(`Could not save the diagram: ${e}`),
    );
  return (
    <div className="session-diagram">
      {saveError && (
        <div role="alert" className="banner-error">
          {saveError}
        </div>
      )}
      <ExcalidrawPanel
        key={key}
        name={title}
        scene={loaded.scene}
        incoming={incoming}
        actions={actions}
        onSceneChange={save}
      />
    </div>
  );
}
