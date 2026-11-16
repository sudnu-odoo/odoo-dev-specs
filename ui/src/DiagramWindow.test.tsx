import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import DiagramWindow from "./DiagramWindow";
import { api } from "./lib/api";
import type { DiagramTarget } from "./lib/types";

vi.mock("./lib/api", () => ({
  api: { loadScene: vi.fn(), saveScene: vi.fn(), dockDiagramWindow: vi.fn(), onDiagramSession: vi.fn() },
}));
vi.mock("./components/ExcalidrawPanel", () => ({
  default: ({ name, scene, actions }: { name: string; scene: string; actions?: React.ReactNode }) => (
    <div>
      <div data-testid="diagram">
        {name}|{scene}
      </div>
      {actions}
    </div>
  ),
}));

const mocked = vi.mocked(api);
let follow: (target: DiagramTarget) => void;

beforeEach(() => {
  vi.clearAllMocks();
  mocked.loadScene.mockImplementation(async (ref) => `scene-of-${ref.sessionId}`);
  mocked.dockDiagramWindow.mockResolvedValue(undefined);
  mocked.onDiagramSession.mockImplementation(async (handler) => {
    follow = handler;
    return () => {};
  });
});

describe("DiagramWindow", () => {
  it("shows the session it was opened for", async () => {
    render(<DiagramWindow initial={{ project: "Acme", sessionId: "s1", title: "Delivery" }} />);
    expect(await screen.findByTestId("diagram")).toHaveTextContent("Delivery|scene-of-s1");
  });

  it("follows the session selected in the main window", async () => {
    render(<DiagramWindow initial={{ project: "Acme", sessionId: "s1", title: "Delivery" }} />);
    await screen.findByText("Delivery|scene-of-s1");
    act(() => follow({ project: "Acme", sessionId: "s2", title: "Invoices" }));
    expect(await screen.findByTestId("diagram")).toHaveTextContent("Invoices|scene-of-s2");
  });

  it("can send the diagram back to the main window", async () => {
    render(<DiagramWindow initial={{ project: "Acme", sessionId: "s1", title: "Delivery" }} />);
    await userEvent.click(await screen.findByRole("button", { name: "Back to main window" }));
    expect(mocked.dockDiagramWindow).toHaveBeenCalled();
  });
});
