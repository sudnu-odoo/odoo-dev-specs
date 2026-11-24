import { useState } from "react";
import type { ChatItem } from "../chat/chatState";

type Tool = Extract<ChatItem, { role: "tool" }>;

/** What the assistant is doing, in plain words. */
const LABELS: Record<string, string> = {
  Read: "Reading the Odoo source",
  Grep: "Searching the Odoo source",
  Glob: "Looking for files",
  odoo_screenshots: "Taking screenshots of Odoo",
  spec_build: "Drawing the spec",
  diagram_summary: "Checking the layout",
  diagram_add_text: "Adding text",
  diagram_add_image: "Placing a screenshot",
  diagram_add_arrow: "Drawing an arrow",
  diagram_add_pointer: "Pointing at a field",
  diagram_add_highlight: "Outlining a part of the screen",
  diagram_add_box: "Adding a box",
  diagram_add_frame: "Adding a frame",
  diagram_set_text: "Editing text",
  diagram_move: "Moving something",
  diagram_delete: "Removing something",
};

export function toolLabel(name: string): string {
  const short = name.replace(/^mcp__[^_]+(?:_[^_]+)*?__/, "");
  if (LABELS[short]) return LABELS[short];
  const words = short.replace(/_/g, " ").trim();
  return words.charAt(0).toUpperCase() + words.slice(1);
}

const ICON = { running: "", ok: "✓", error: "✗" } as const;

function StepIcon({ status }: { status: Tool["status"] }) {
  return status === "running" ? (
    <span className="spinner spinner-small" aria-hidden="true" />
  ) : (
    <span className="tool-icon" aria-hidden="true">
      {ICON[status]}
    </span>
  );
}

/**
 * A run of the assistant's steps as one line: the latest step slides in over the
 * previous one, with a count; a click lists them all. Failed steps stay visible.
 */
export default function ToolSteps({ steps }: { steps: Tool[] }) {
  const [open, setOpen] = useState(false);
  const last = steps[steps.length - 1];
  const failed = steps.filter((s, i) => s.status === "error" && i !== steps.length - 1);
  const label = toolLabel(last.name);
  return (
    <div className="tool-steps">
      {failed.map((s, i) => (
        <div key={i} className="msg-tool" data-status="error">
          <StepIcon status="error" />
          <span>{toolLabel(s.name)}</span>
        </div>
      ))}
      <button
        type="button"
        className="msg-tool tool-line"
        data-status={last.status}
        aria-expanded={open}
        aria-label={`${label}, ${steps.length} ${steps.length === 1 ? "step" : "steps"}`}
        onClick={() => setOpen((o) => !o)}
      >
        <StepIcon status={last.status} />
        <span className="tool-ticker">
          {/* A new key per step replays the slide-up. */}
          <span key={steps.length} className="tool-ticker-text">
            {label}
          </span>
        </span>
        {steps.length > 1 && <span className="tool-count">· {steps.length} steps</span>}
      </button>
      {open && (
        <ul className="tool-list" aria-label="Steps">
          {steps.map((s, i) => (
            <li key={i} className="msg-tool" data-status={s.status}>
              <StepIcon status={s.status} />
              <span>{toolLabel(s.name)}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
