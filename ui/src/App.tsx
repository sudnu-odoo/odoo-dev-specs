import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import { useSpecsApp } from "./app/useSpecsApp";
import ChatPanel from "./components/ChatPanel";
import ExcalidrawPanel from "./components/ExcalidrawPanel";
import Sidebar from "./components/Sidebar";

export default function App() {
  const app = useSpecsApp();
  const sessionKey = app.selected ? `${app.selected.project}/${app.selected.sessionId}` : null;

  return (
    <div className="app">
      <Sidebar
        projects={app.projects}
        selected={app.selected}
        onSelect={app.open}
        onCreateProject={app.createProject}
        onCreateSession={app.createSession}
        onDeleteSession={app.deleteSession}
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
        {/* autoSaveId keeps the chat/artifact split the user dragged to. */}
        <PanelGroup direction="horizontal" autoSaveId="main-split" className="main-panels">
          <Panel defaultSize={38} minSize={25}>
            <ChatPanel
              items={app.chat.items}
              running={app.chat.running}
              hasSession={!!app.session}
              onSend={app.send}
              onStop={app.stop}
            />
          </Panel>
          <PanelResizeHandle className="resize-handle" />
          <Panel defaultSize={62} minSize={30}>
            {app.session && sessionKey ? (
              // Remount per session so the canvas starts from that session's diagram.
              <ExcalidrawPanel
                key={sessionKey}
                name={app.session.meta.title}
                scene={app.session.scene}
                onSceneChange={app.saveScene}
              />
            ) : (
              <div className="artifact-placeholder">The diagram of the open session appears here.</div>
            )}
          </Panel>
        </PanelGroup>
      </div>
    </div>
  );
}
