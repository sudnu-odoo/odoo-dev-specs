import { useCallback, useEffect, useReducer, useRef, useState } from "react";
import { emptyChat } from "../chat/chatState";
import { emptySessionChats, sessionChatsReducer } from "../chat/sessionChats";
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
  // Every session's chat, so one can keep answering while another is open.
  const [chats, dispatch] = useReducer(sessionChatsReducer, emptySessionChats);
  const chat = {
    ...((selected && chats.chats[selected.sessionId]) || emptyChat),
    plan: chats.plan,
  };
  const [error, setError] = useState<string | null>(null);
  // True while the diagram is shown in its own window instead of beside the chat.
  const [diagramDetached, setDiagramDetached] = useState(false);
  // Models of the active provider, and the one chosen for the next message.
  const [models, setModels] = useState<Model[]>([]);
  const [selectedModel, setSelectedModel] = useState("");
  // Reasoning effort for the next message ("" = the CLI's default); remembered across runs.
  const [effort, setEffortState] = useState(() => {
    try {
      return localStorage.getItem("effort") ?? "";
    } catch {
      return "";
    }
  });
  const setEffort = useCallback((value: string) => {
    setEffortState(value);
    try {
      localStorage.setItem("effort", value);
    } catch {
      // Not remembered; still used for this run.
    }
  }, []);

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
  // A session belongs to the provider of its first message ("Claude", "Antigravity").
  const sessionProvider =
    session?.meta.started && session.meta.provider ? displayName(session.meta.provider) : undefined;
  // Keep the picker on a model of that provider when such a session is open.
  useEffect(() => {
    if (!sessionProvider) return;
    setSelectedModel((current) => {
      const own = models.filter((m) => m.provider === sessionProvider && !m.locked);
      return own.some((m) => m.id === current) ? current : (own[0]?.id ?? current);
    });
  }, [sessionProvider, models]);

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
      dispatch({ type: "event", sessionId, event });
      if (event.kind === "finished") refresh();
    });
    const unlistenDocked = api.onDiagramDocked(() => setDiagramDetached(false));
    // Logging in happens in a terminal window: unlock its models when the user comes back.
    window.addEventListener("focus", reloadModels);
    return () => {
      window.removeEventListener("focus", reloadModels);
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
      const [data, running] = await Promise.all([
        api.loadSession(ref),
        // The backend knows which turns are still going, also after a UI reload.
        api.runningSessions().catch(() => [] as string[]),
      ]);
      setSession(data);
      dispatch({
        type: "opened",
        sessionId: ref.sessionId,
        transcript: data.transcript,
        running: running.includes(ref.sessionId),
      });
    } catch (e) {
      setError(String(e));
    }
  }, []);

  const close = useCallback(() => {
    setSelected(null);
    setSession(null);
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
      const sessionId = selected.sessionId;
      dispatch({ type: "chat", sessionId, action: { type: "sent", text } });
      // The first message ties the session to the provider of the chosen model.
      const provider = models.find((m) => m.id === selectedModel)?.provider;
      if (!session.meta.started && provider) {
        setSession((s) =>
          s && sameRef(selected, selectedRef.current)
            ? { ...s, meta: { ...s.meta, started: true, provider: provider.toLowerCase() } }
            : s,
        );
      }
      const isFirstMessage = !chat.items.some((i) => i.role === "user");
      const title = titleFromMessage(text);
      if (isFirstMessage && session.meta.title === DEFAULT_TITLE && title) {
        api.renameSession(selected, title).then(refresh, () => {});
      }
      try {
        await api.sendMessage(selected, text, selectedModel || null, effort || null);
      } catch (e) {
        dispatch({ type: "chat", sessionId, action: { type: "sendFailed", message: String(e) } });
      }
    },
    [chat.items, effort, models, refresh, selected, selectedModel, session],
  );

  const login = useCallback((provider: string) => {
    api.loginProvider(provider).catch((e) => setError(String(e)));
  }, []);

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
    effort,
    setEffort,
    login,
    sessionProvider,
  };
}

/** "antigravity" -> "Antigravity", as providers are named in the model list. */
function displayName(providerId: string) {
  return providerId.charAt(0).toUpperCase() + providerId.slice(1);
}
