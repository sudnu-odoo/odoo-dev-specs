import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { chatReducer, emptyChat } from "../chat/chatState";
import { DEFAULT_TITLE, titleFromMessage } from "../chat/title";
import { api } from "../lib/api";
import { confirmAction } from "../lib/confirm";
import type { ProjectInfo, SessionData, SessionRef } from "../lib/types";

const sameRef = (a: SessionRef | null, b: SessionRef | null) =>
  !!a && !!b && a.project === b.project && a.sessionId === b.sessionId;

/** App-level state and actions: projects, the open session, and its chat. */
export function useSpecsApp() {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [selected, setSelected] = useState<SessionRef | null>(null);
  const [session, setSession] = useState<SessionData | null>(null);
  const [chat, dispatch] = useReducer(chatReducer, emptyChat);
  const [error, setError] = useState<string | null>(null);
  // Read inside the event listener, which is registered only once.
  const selectedRef = useRef<SessionRef | null>(null);
  selectedRef.current = selected;

  const refresh = useCallback(async () => {
    try {
      setProjects(await api.listProjects());
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    refresh();
    const unlisten = api.onChatEvent(({ sessionId, event }) => {
      if (sessionId !== selectedRef.current?.sessionId) return;
      dispatch({ type: "event", event });
      if (event.kind === "finished") refresh();
    });
    return () => {
      unlisten.then((stop) => stop());
    };
  }, [refresh]);

  const open = useCallback(async (ref: SessionRef) => {
    setSelected(ref);
    try {
      const data = await api.loadSession(ref);
      setSession(data);
      dispatch({ type: "loaded", transcript: data.transcript });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const close = useCallback(() => {
    setSelected(null);
    setSession(null);
    dispatch({ type: "loaded", transcript: [] });
  }, []);

  const createProject = useCallback(
    async (name: string) => {
      await api.createProject(name);
      await refresh();
    },
    [refresh],
  );

  const createSession = useCallback(
    async (project: string) => {
      try {
        const meta = await api.createSession(project, DEFAULT_TITLE);
        await refresh();
        await open({ project, sessionId: meta.id });
      } catch (e) {
        setError(String(e));
      }
    },
    [open, refresh],
  );

  const deleteSession = useCallback(
    async (ref: SessionRef) => {
      const title = projects
        .find((p) => p.name === ref.project)
        ?.sessions.find((s) => s.id === ref.sessionId)?.title;
      const ok = await confirmAction(
        `Delete "${title}" and its diagram? This cannot be undone.`,
        "Delete session",
      );
      if (!ok) return;
      try {
        await api.deleteSession(ref);
        if (sameRef(ref, selectedRef.current)) close();
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [close, projects, refresh],
  );

  const send = useCallback(
    async (text: string) => {
      if (!selected || !session) return;
      dispatch({ type: "sent", text });
      const isFirstMessage = !chat.items.some((i) => i.role === "user");
      const title = titleFromMessage(text);
      if (isFirstMessage && session.meta.title === DEFAULT_TITLE && title) {
        api.renameSession(selected, title).then(refresh, () => {});
      }
      try {
        await api.sendMessage(selected, text);
      } catch (e) {
        dispatch({ type: "sendFailed", message: String(e) });
      }
    },
    [chat.items, refresh, selected, session],
  );

  const stop = useCallback(() => {
    if (selected) api.cancelTurn(selected.sessionId);
  }, [selected]);

  const saveScene = useCallback(
    (scene: string) => {
      if (selected) api.saveScene(selected, scene).catch((e) => setError(String(e)));
    },
    [selected],
  );

  return {
    projects,
    selected,
    session,
    chat,
    error,
    dismissError: () => setError(null),
    open,
    createProject,
    createSession,
    deleteSession,
    send,
    stop,
    saveScene,
  };
}
