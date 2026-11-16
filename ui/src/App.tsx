import { Panel, PanelGroup, PanelResizeHandle } from "react-resizable-panels";
import Sidebar from "./components/Sidebar";
import ChatPanel from "./components/ChatPanel";
import ExcalidrawPanel from "./components/ExcalidrawPanel";

export default function App() {
  return (
    <div className="app">
      <Sidebar />
      {/* autoSaveId keeps the chat/artifact split the user dragged to. */}
      <PanelGroup direction="horizontal" autoSaveId="main-split" className="main">
        <Panel defaultSize={38} minSize={25}>
          <ChatPanel />
        </Panel>
        <PanelResizeHandle className="resize-handle" />
        <Panel defaultSize={62} minSize={30}>
          <ExcalidrawPanel />
        </Panel>
      </PanelGroup>
    </div>
  );
}
