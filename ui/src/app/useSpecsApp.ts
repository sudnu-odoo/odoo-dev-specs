import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { chatReducer, emptyChat } from "../chat/chatState";
import { DEFAULT_TITLE, titleFromMessage } from "../chat/title";
import { api } from "../lib/api";
import { confirmAction } from "../lib/confirm";
import type { Model, ProjectInfo, SessionData, SessionRef } from "../lib/types";

const sameRef = (a: SessionRef | null, b: SessionRef | null) =>
  !!a && !!b && a.project === b.project && a.sessionId === b.sessionId;

/** App-level state and actions: projects, the open session, and its chat. */
export function useSpecsApp() {
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [selected, setSelected] = useState<SessionRef | null>(null);
  const [session, setSession] = useState<SessionData | null>(null);
  const [chat, dispatch] = useReducer(chatReducer, emptyChat);
  const [error, setError] = useState<string | null>(null);
  // True while the diagram is shown in its own window instead of beside the chat.
  const [diagramDetached, setDiagramDetached] = useState(false);
  // Models of the active provider, and the one chosen for the next message.
  const [models, setModels] = useState<Model[]>([]);
  const [selectedModel, setSelectedModel] = useState("");

  const reloadModels = useCallback(async () => {
    try {
      const list = await api.listModels();
      setModels(list);
      // Keep the current choice if it still exists; otherwise fall back to the default.
      setSelectedModel((current) => {
        // Keep the choice while it is still selectable; otherwise the first unlocked model.
        const usable = list.filter((m) => !m.locked);
        return usable.some((m) => m.id === current) ? current : (usable[0]?.id ?? "");
      });
    } catch (e) {
      setError(String(e));
    }
  }, []);
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
    reloadModels();
    const unlisten = api.onChatEvent(({ sessionId, event }) => {
      if (sessionId !== selectedRef.current?.sessionId) return;
      dispatch({ type: "event", event });
      if (event.kind === "finished") refresh();
    });
    const unlistenDocked = api.onDiagramDocked(() => setDiagramDetached(false));
    return () => {
      unlisten.then((stop) => stop());
      unlistenDocked.then((stop) => stop());
    };
  }, [refresh, reloadModels]);

  // The separate diagram window follows the open session and its title.
  const openTitle = session?.meta.title;
  useEffect(() => {
    if (diagramDetached && selected && openTitle) {
      api.openDiagramWindow(selected, openTitle).catch((e) => setError(String(e)));
    }
  }, [diagramDetached, selected, openTitle]);

  const open = useCallback(async (ref: SessionRef) => {
    setSelected(ref);
    // Never pair the new selection with the previous session's data while loading.
    setSession(null);
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

  const renameSession = useCallback(
    async (ref: SessionRef, title: string) => {
      try {
        const meta = await api.renameSession(ref, title);
        if (sameRef(ref, selectedRef.current)) setSession((s) => (s ? { ...s, meta } : s));
        await refresh();
      } catch (e) {
        setError(String(e));
      }
    },
    [refresh],
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
        await api.sendMessage(selected, text, selectedModel || null);
      } catch (e) {
        dispatch({ type: "sendFailed", message: String(e) });
      }
    },
    [chat.items, refresh, selected, selectedModel, session],
  );

  const stop = useCallback(() => {
    if (selected) api.cancelTurn(selected.sessionId);
  }, [selected]);

  const detachDiagram = useCallback(() => setDiagramDetached(true), []);
  const dockDiagram = useCallback(() => {
    // The window's closing event switches the layout back (also when closed directly).
    api.dockDiagramWindow().catch((e) => setError(String(e)));
  }, []);

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
    renameSession,
    send,
    stop,
    diagramDetached,
    detachDiagram,
    dockDiagram,
    models,
    selectedModel,
    setSelectedModel,
    reloadModels,
  };
}
