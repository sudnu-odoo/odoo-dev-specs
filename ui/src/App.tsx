import { useEffect, useMemo, useState } from "react";
import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { useDatabases } from "./app/useDatabases";
import { useOdooVersions } from "./app/useOdooVersions";
import { useSetup } from "./app/useSetup";
import { useSpecsApp } from "./app/useSpecsApp";
import NewDatabaseDialog from "./components/NewDatabaseDialog";
import SetupScreen from "./components/SetupScreen";
import UpdateBanner from "./components/UpdateBanner";
import type { ProviderStatus, SessionRef } from "./lib/types";

const sameSession = (a: SessionRef | null, b: SessionRef | null) =>
  !!a && !!b && a.project === b.project && a.sessionId === b.sessionId;
import DatabaseMenu from "./components/DatabaseMenu";
import HostingMenu from "./components/HostingMenu";
import DatabasesPage from "./components/DatabasesPage";
import ChatPanel from "./components/ChatPanel";
import SessionDiagram from "./components/SessionDiagram";
import SettingsPanel from "./components/SettingsPanel";
import Sidebar from "./components/Sidebar";
import { api } from "./lib/api";

export default function App() {
  const app = useSpecsApp();
  const dbs = useDatabases();
  const versions = useOdooVersions();
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [view, setView] = useState<"session" | "databases">("session");
  // "New database…" from a session's chip: the form, then the session it is for.
  const [newDatabaseFor, setNewDatabaseFor] = useState<SessionRef | null>(null);
  const [creatingFor, setCreatingFor] = useState<SessionRef | null>(null);

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
  const openSession = sessions.find(
    (s) => s.project === app.selected?.project && s.id === app.selected?.sessionId,
  );
  const sessionHosting = openSession?.hosting ?? null;
  const sessionVersion = openSession?.version ?? null;
  const sessionDatabase =
    sessions.find((s) => s.project === app.selected?.project && s.id === app.selected?.sessionId)?.database ??
    null;
  const reloadDatabases = dbs.reload;
  useEffect(() => {
    reloadDatabases();
  }, [app.projects, reloadDatabases]);
  // First run: the setup shows by itself until the computer is ready.
  const setup = useSetup();
  const [setupOpen, setSetupOpen] = useState<boolean | null>(null);
  const [providers, setProviders] = useState<ProviderStatus[]>([]);
  useEffect(() => {
    if (setupOpen === null && setup.status) setSetupOpen(!setup.complete);
  }, [setup.status, setup.complete, setupOpen]);
  useEffect(() => {
    if (!setupOpen) return;
    const load = () => api.providerStatus().then(setProviders, () => {});
    load();
    window.addEventListener("focus", load);
    return () => window.removeEventListener("focus", load);
  }, [setupOpen]);
  const download = setup.downloading
    ? {
        label: setup.community?.state === "running" ? "Downloading Odoo" : "Downloading Enterprise",
        percent: setup.downloading.progress,
      }
    : null;

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
      // Claude's limits mean nothing for Antigravity, which publishes no usage.
      showUsage={
        (app.sessionProvider ?? app.models.find((m) => m.id === app.selectedModel)?.provider) !==
        "Antigravity"
      }
      composerExtras={
        <>
          <HostingMenu
            hosting={sessionHosting}
            version={sessionVersion}
            available={versions.available}
            latestSaas={versions.latestSaas}
            defaultVersion="19.0"
            disabled={app.chat.running}
            onChange={app.setSessionTarget}
          />
          <DatabaseMenu
            // Only databases of the session's version can serve its screenshots.
            databases={
              sessionVersion ? dbs.databases.filter((d) => d.version === sessionVersion) : dbs.databases
            }
            selected={sessionDatabase}
            disabled={app.chat.running}
            onChange={(name) => app.setSessionDatabase(name)}
            onManage={() => setView("databases")}
            onCreate={() => setNewDatabaseFor(app.selected)}
            creating={!!creatingFor && sameSession(creatingFor, app.selected)}
            onOpen={() => {
              const db = dbs.databases.find((d) => d.name === sessionDatabase);
              if (db) dbs.open(db);
            }}
            opening={!!sessionDatabase && dbs.busy[sessionDatabase] === "starting"}
          />
        </>
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
        download={download}
        onOpenSetup={() => setSetupOpen(true)}
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
        <UpdateBanner />
        {app.error && (
          <div role="alert" className="banner-error">
            {app.error}
            <button className="icon-btn" aria-label="Dismiss" onClick={app.dismissError}>
              ×
            </button>
          </div>
        )}
        {setupOpen === false && setup.status && !setup.complete && !setup.downloading && (
          <div role="alert" className="banner-warning">
            The setup is not finished, so some things will not work.
            <button className="btn btn-small" onClick={() => setSetupOpen(true)}>
              Open setup
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
      {newDatabaseFor && (
        <NewDatabaseDialog
          defaultVersion="19.0"
          formFor={dbs.formFor}
          onCancel={() => setNewDatabaseFor(null)}
          onCreate={async (db) => {
            const session = newDatabaseFor;
            setNewDatabaseFor(null);
            setCreatingFor(session);
            const made = await dbs.create(db);
            setCreatingFor(null);
            // Made for this session, even if another one is open by now.
            if (made) await app.setSessionDatabase(made.name, session);
          }}
        />
      )}
      {setupOpen && (
        <SetupScreen
          setup={setup}
          providers={providers}
          onLogin={app.login}
          onDone={() => {
            setSetupOpen(false);
            app.reloadModels();
          }}
        />
      )}
      {settingsOpen && (
        <SettingsPanel
          onClose={() => {
            setSettingsOpen(false);
            // A provider may have been logged in: refresh the model picker.
            app.reloadModels();
            setup.refresh();
          }}
          onOpenSetup={() => {
            setSettingsOpen(false);
            setSetupOpen(true);
          }}
        />
      )}
    </div>
  );
}
