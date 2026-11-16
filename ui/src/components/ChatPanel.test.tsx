import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import ChatPanel, { type ChatPanelProps } from "./ChatPanel";

function setup(overrides: Partial<ChatPanelProps> = {}) {
  const props: ChatPanelProps = {
    items: [],
    running: false,
    hasSession: true,
    onSend: vi.fn(),
    onStop: vi.fn(),
    ...overrides,
  };
  render(<ChatPanel {...props} />);
  return props;
}

const input = () => screen.getByRole("textbox", { name: "Message" });

describe("ChatPanel", () => {
  it("sends the trimmed message on Enter and clears the box", async () => {
    const props = setup();
    await userEvent.type(input(), "  Add a field  {Enter}");
    expect(props.onSend).toHaveBeenCalledWith("Add a field");
    expect(input()).toHaveValue("");
  });

  it("adds a new line on Shift+Enter instead of sending", async () => {
    const props = setup();
    await userEvent.type(input(), "line 1{Shift>}{Enter}{/Shift}line 2");
    expect(props.onSend).not.toHaveBeenCalled();
    expect(input()).toHaveValue("line 1\nline 2");
  });

  it("does not send blank messages", async () => {
    const props = setup();
    await userEvent.type(input(), "   {Enter}");
    await userEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(props.onSend).not.toHaveBeenCalled();
  });

  it("offers Stop instead of Send while the assistant answers", async () => {
    const props = setup({ running: true });
    expect(screen.queryByRole("button", { name: "Send" })).not.toBeInTheDocument();
    await userEvent.type(input(), "next{Enter}");
    expect(props.onSend).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Stop" }));
    expect(props.onStop).toHaveBeenCalled();
  });

  it("shows every kind of message", () => {
    setup({
      items: [
        { role: "user", text: "Add a delivery window" },
        { role: "tool", name: "Read", status: "ok" },
        { role: "tool", name: "Grep", status: "running" },
        { role: "assistant", text: "Here is the spec." },
        { role: "error", message: "Not logged in" },
      ],
    });
    expect(screen.getByText("Add a delivery window")).toBeInTheDocument();
    expect(screen.getByText("Here is the spec.")).toBeInTheDocument();
    expect(screen.getByText(/Read/)).toBeInTheDocument();
    expect(screen.getByText(/Grep/)).toHaveAttribute("data-status", "running");
    expect(screen.getByRole("alert")).toHaveTextContent("Not logged in");
  });

  it("is disabled until a session is open", () => {
    setup({ hasSession: false });
    expect(input()).toBeDisabled();
    expect(screen.getByText(/open or create a session/i)).toBeInTheDocument();
  });
});
