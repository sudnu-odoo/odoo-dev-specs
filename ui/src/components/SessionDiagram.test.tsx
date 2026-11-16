import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import SessionDiagram from "./SessionDiagram";
import { api } from "../lib/api";
import type { ExcalidrawPanelProps } from "./ExcalidrawPanel";
import type { ChatEventPayload } from "../lib/types";

vi.mock("../lib/api", () => ({ api: { loadScene: vi.fn(), saveScene: vi.fn(), onChatEvent: vi.fn() } }));
let panel: ExcalidrawPanelProps | null = null;
vi.mock("./ExcalidrawPanel", () => ({
  default: (props: ExcalidrawPanelProps) => {
    panel = props;
    return (
      <div data-testid="canvas">
        {props.name}:{props.scene}
        {props.actions}
      </div>
    );
  },
}));

const mocked = vi.mocked(api);
const ref = (sessionId: string) => ({ project: "Acme", sessionId });

let chatEvent: (payload: ChatEventPayload) => void;

beforeEach(() => {
  vi.clearAllMocks();
  panel = null;
  mocked.saveScene.mockResolvedValue(undefined);
  mocked.onChatEvent.mockImplementation(async (handler) => {
    chatEvent = handler;
    return () => {};
  });
});

describe("SessionDiagram", () => {
  it("loads the session's diagram before showing the canvas", async () => {
    let resolve!: (scene: string) => void;
    mocked.loadScene.mockReturnValue(new Promise((r) => (resolve = r)));
    render(<SessionDiagram sessionRef={ref("s1")} title="Spec" actions={<button>Extra</button>} />);
    expect(screen.getByText(/loading diagram/i)).toBeInTheDocument();
    expect(screen.queryByTestId("canvas")).not.toBeInTheDocument();

    await act(async () => resolve("scene-1"));
    expect(screen.getByTestId("canvas")).toHaveTextContent("Spec:scene-1");
    expect(screen.getByRole("button", { name: "Extra" })).toBeInTheDocument();
    expect(mocked.loadScene).toHaveBeenCalledWith(ref("s1"));
  });

  it("never shows one session's diagram under another session", async () => {
    mocked.loadScene.mockResolvedValueOnce("scene-1");
    const { rerender } = render(<SessionDiagram sessionRef={ref("s1")} title="One" />);
    await screen.findByText("One:scene-1");

    let resolve!: (scene: string) => void;
    mocked.loadScene.mockReturnValueOnce(new Promise((r) => (resolve = r)));
    rerender(<SessionDiagram sessionRef={ref("s2")} title="Two" />);
    expect(screen.queryByText(/scene-1/)).not.toBeInTheDocument();

    await act(async () => resolve("scene-2"));
    expect(screen.getByTestId("canvas")).toHaveTextContent("Two:scene-2");
  });

  it("saves edits to the session they belong to", async () => {
    mocked.loadScene.mockResolvedValue("scene-1");
    render(<SessionDiagram sessionRef={ref("s1")} title="One" />);
    await screen.findByTestId("canvas");
    panel!.onSceneChange("edited");
    expect(mocked.saveScene).toHaveBeenCalledWith(ref("s1"), "edited");
  });

  it("reports a save that failed", async () => {
    mocked.loadScene.mockResolvedValue("scene-1");
    mocked.saveScene.mockRejectedValue("Disk full");
    render(<SessionDiagram sessionRef={ref("s1")} title="One" />);
    await screen.findByTestId("canvas");
    await act(async () => panel!.onSceneChange("edited"));
    expect(screen.getByRole("alert")).toHaveTextContent("Could not save the diagram: Disk full");
  });

  it("explains when the diagram cannot be loaded", async () => {
    mocked.loadScene.mockRejectedValue("This session no longer exists.");
    render(<SessionDiagram sessionRef={ref("s1")} title="One" />);
    expect(await screen.findByRole("alert")).toHaveTextContent("no longer exists");
  });

  it("reloads the diagram when the assistant's tools change it", async () => {
    mocked.loadScene.mockResolvedValueOnce("scene-1");
    render(<SessionDiagram sessionRef={ref("s1")} title="One" />);
    await screen.findByTestId("canvas");
    expect(panel!.incoming).toBeUndefined();

    mocked.loadScene.mockResolvedValueOnce("scene-2");
    await act(async () =>
      chatEvent({ sessionId: "s1", event: { kind: "tool_finished", id: "t", is_error: false } }),
    );
    expect(panel!.incoming).toEqual({ scene: "scene-2", revision: 1 });

    mocked.loadScene.mockResolvedValueOnce("scene-3");
    await act(async () =>
      chatEvent({ sessionId: "s1", event: { kind: "finished", is_error: false, message: null } }),
    );
    expect(panel!.incoming).toEqual({ scene: "scene-3", revision: 2 });
    expect(screen.getByTestId("canvas")).toHaveTextContent("One:scene-1"); // no remount
  });

  it("ignores other sessions and streamed text", async () => {
    mocked.loadScene.mockResolvedValue("scene-1");
    render(<SessionDiagram sessionRef={ref("s1")} title="One" />);
    await screen.findByTestId("canvas");
    await act(async () => {
      chatEvent({ sessionId: "s2", event: { kind: "tool_finished", id: "t", is_error: false } });
      chatEvent({ sessionId: "s1", event: { kind: "text_delta", text: "hi" } });
    });
    expect(mocked.loadScene).toHaveBeenCalledTimes(1);
  });

  it("holds saves while the assistant works, until its final changes are merged", async () => {
    mocked.loadScene.mockResolvedValue("scene-1");
    render(<SessionDiagram sessionRef={ref("s1")} title="One" />);
    await screen.findByTestId("canvas");
    expect(panel!.holdSaves).toBe(false);

    await act(async () => chatEvent({ sessionId: "s1", event: { kind: "text_delta", text: "Let me" } }));
    expect(panel!.holdSaves).toBe(true);
    await act(async () =>
      chatEvent({ sessionId: "s1", event: { kind: "tool_finished", id: "t", is_error: false } }),
    );
    expect(panel!.holdSaves).toBe(true);

    mocked.loadScene.mockResolvedValueOnce("final");
    await act(async () =>
      chatEvent({ sessionId: "s1", event: { kind: "finished", is_error: false, message: null } }),
    );
    expect(panel!.incoming?.scene).toBe("final");
    expect(panel!.holdSaves).toBe(false);
  });
});
