import { useEffect, useMemo, useState } from "react";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { useDatabases } from "./app/useDatabases";
import { useSpecsApp } from "./app/useSpecsApp";
import DatabaseMenu from "./components/DatabaseMenu";
import DatabasesPage from "./components/DatabasesPage";
import ChatPanel from "./components/ChatPanel";
import SessionDiagram from "./components/SessionDiagram";
import SettingsPanel from "./components/SettingsPanel";
import Sidebar from "./components/Sidebar";
import { api } from "./lib/api";

export default function App() {
  const app = useSpecsApp();
  const dbs = useDatabases();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [view, setView] = useState<"session" | "databases">("session");

  // Which database each session uses, from the session list (refreshed after each
  // answer, when the assistant may have picked one).
  const sessions = useMemo(
    () => app.projects.flatMap((p) => p.sessions.map((s) => ({ project: p.name, ...s }))),
    [app.projects],
  );
  const usage = useMemo(() => {
    const count: Record<string, number> = {};
    for (const s of sessions) if (s.database) count[s.database] = (count[s.database] ?? 0) + 1;
    return count;
  }, [sessions]);
  const sessionDatabase =
    sessions.find((s) => s.project === app.selected?.project && s.id === app.selected?.sessionId)?.database ??
    null;
  const reloadDatabases = dbs.reload;
  useEffect(() => {
    reloadDatabases();
  }, [app.projects, reloadDatabases]);
  // On first run, warn when a required tool is missing so the user fixes it.
  const [setupIncomplete, setSetupIncomplete] = useState(false);

  useEffect(() => {
    api.checkEnvironment().then(
      (report) => setSetupIncomplete(report.checks.some((c) => c.required && c.status === "missing")),
      () => {},
    );
  }, []);

  const chat = (
    <ChatPanel
      items={app.chat.items}
      running={app.chat.running}
      hasSession={!!app.session}
      onSend={app.send}
      onStop={app.stop}
      models={app.models}
      selectedModel={app.selectedModel}
      onModelChange={app.setSelectedModel}
      effort={app.effort}
      onEffortChange={app.setEffort}
      onLogin={app.login}
      sessionProvider={app.sessionProvider}
      usage={app.chat.usage}
      plan={app.chat.plan}
      composerExtras={
        <DatabaseMenu
          databases={dbs.databases}
          selected={sessionDatabase}
          disabled={app.chat.running}
          onChange={app.setSessionDatabase}
          onManage={() => setView("databases")}
        />
      }
    />
  );

  return (
    <div className="app">
      <Sidebar
        projects={app.projects}
        selected={app.selected}
        onSelect={(ref) => {
          setView("session");
          app.open(ref);
        }}
        databasesOpen={view === "databases"}
        onOpenDatabases={() => setView("databases")}
        onCreateProject={app.createProject}
        onCreateSession={(project) => {
          setView("session");
          app.createSession(project);
        }}
        onDeleteSession={app.deleteSession}
        onRenameSession={app.renameSession}
        onOpenSettings={() => setSettingsOpen(true)}
      />
      <div className="main">
        {app.error && (
          <div role="alert" className="banner-error">
            {app.error}
            <button className="icon-btn" aria-label="Dismiss" onClick={app.dismissError}>
              ×
            </button>
          </div>
        )}
        {setupIncomplete && !settingsOpen && (
          <div role="alert" className="banner-warning">
            The setup is incomplete, so some things will not work.
            <button className="btn btn-small" onClick={() => setSettingsOpen(true)}>
              Open settings
            </button>
          </div>
        )}
        {view === "databases" ? (
          <DatabasesPage dbs={dbs} usage={usage} />
        ) : app.diagramDetached ? (
          <div className="main-panels detached">
            <div className="detached-bar">
              The diagram is open in its own window.
              <button className="btn btn-small" onClick={app.dockDiagram}>
                Bring diagram back
              </button>
            </div>
            {chat}
          </div>
        ) : (
          // autoSaveId keeps the chat/diagram split the user dragged to.
          <PanelGroup direction="horizontal" autoSaveId="main-split" className="main-panels">
            <Panel defaultSize={38} minSize={25}>
              {chat}
            </Panel>
            <PanelResizeHandle className="resize-handle" />
            <Panel defaultSize={62} minSize={30}>
              {app.selected && app.session ? (
                <SessionDiagram
                  sessionRef={app.selected}
                  title={app.session.meta.title}
                  actions={
                    <button
                      className="btn"
                      title="Edit the diagram in a separate window"
                      onClick={app.detachDiagram}
                    >
                      Open in new window
                    </button>
                  }
                />
              ) : (
                <div className="artifact-placeholder">The diagram of the open session appears here.</div>
              )}
            </Panel>
          </PanelGroup>
        )}
      </div>
      {settingsOpen && (
        <SettingsPanel
          onClose={() => {
            setSettingsOpen(false);
            // A provider may have been logged in: refresh the model picker.
            app.reloadModels();
            // The user may have just fixed something; re-check.
            api.checkEnvironment().then(
              (report) => setSetupIncomplete(report.checks.some((c) => c.required && c.status === "missing")),
              () => {},
            );
          }}
        />
      )}
    </div>
  );
}
