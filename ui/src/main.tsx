import React from "react";
import ReactDOM from "react-dom/client";
import App from "./App";
import DiagramWindow from "./DiagramWindow";
import type { DiagramTarget } from "./lib/types";
import "./styles.css";

declare global {
  interface Window {
    /** Set by the Rust side when this page is the separate diagram window. */
    __DIAGRAM_SESSION__?: DiagramTarget;
  }
}

const diagram = window.__DIAGRAM_SESSION__;

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>{diagram ? <DiagramWindow initial={diagram} /> : <App />}</React.StrictMode>,
);
