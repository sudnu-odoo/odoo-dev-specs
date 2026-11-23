import { useCallback, useEffect, useState } from "react";
import type { IncomingScene } from "../components/ExcalidrawPanel";
import { api } from "../lib/api";
import type { SessionRef } from "../lib/types";

type Loaded = { key: string; scene: string } | { key: string; error: string };

/**
 * One session's diagram: its scene from disk, the assistant's edits as they land
 * (`incoming`), and saving the person's edits back. While the assistant works,
 * saves are held (they could overwrite newer tool edits) until its last change is in.
 */
export function useSessionScene({ project, sessionId }: SessionRef) {
  const key = `${project}/${sessionId}`;
  const [loaded, setLoaded] = useState<Loaded | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);
  const [incoming, setIncoming] = useState<IncomingScene | undefined>(undefined);
  const [assistantBusy, setAssistantBusy] = useState(false);

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

  useEffect(() => {
    setIncoming(undefined);
    setAssistantBusy(false);
    let current = true;
    const reload = (done: boolean) =>
      api.loadScene({ project, sessionId }).then(
        (scene) => {
          if (!current) return;
          setIncoming((prev) => ({ scene, revision: (prev?.revision ?? 0) + 1 }));
          if (done) setAssistantBusy(false);
        },
        () => current && done && setAssistantBusy(false),
      );
    const unlisten = api.onChatEvent(({ sessionId: from, event }) => {
      if (from !== sessionId) return;
      if (event.kind === "finished") {
        reload(true);
        return;
      }
      setAssistantBusy(true);
      if (event.kind === "tool_finished") reload(false);
    });
    return () => {
      current = false;
      unlisten.then((stop) => stop());
    };
  }, [project, sessionId]);

  const save = useCallback(
    (scene: string) =>
      api.saveScene({ project, sessionId }, scene).then(
        () => setSaveError(null),
        (e) => setSaveError(`Could not save the diagram: ${e}`),
      ),
    [project, sessionId],
  );

  // Only the scene of the session being shown counts.
  const current = loaded?.key === key ? loaded : null;
  return {
    key,
    scene: current && "scene" in current ? current.scene : null,
    loadError: current && "error" in current ? current.error : null,
    incoming,
    assistantBusy,
    save,
    saveError,
  };
}
