import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import ChatPanel, { type ChatPanelProps } from "./ChatPanel";

const MODELS = [
  { id: "sonnet", label: "Sonnet (balanced)" },
  { id: "opus", label: "Opus (most capable)" },
];

function setup(overrides: Partial<ChatPanelProps> = {}) {
  const props: ChatPanelProps = {
    items: [],
    running: false,
    hasSession: true,
    onSend: vi.fn(),
    onStop: vi.fn(),
    models: MODELS,
    selectedModel: "sonnet",
    onModelChange: vi.fn(),
    usage: { inputTokens: 0, outputTokens: 0 },
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

describe("ChatPanel formatting", () => {
  it("renders assistant markdown instead of raw symbols", () => {
    const { container } = render(
      <ChatPanel
        items={[
          {
            role: "assistant",
            text: "**Bold** and `code`\n\n- first\n- second\n\n| A | B |\n|---|---|\n| 1 | 2 |",
          },
        ]}
        running={false}
        hasSession
        onSend={vi.fn()}
        onStop={vi.fn()}
      />,
    );
    expect(screen.getByText("Bold").tagName).toBe("STRONG");
    expect(screen.getByText("code").tagName).toBe("CODE");
    expect(screen.getAllByRole("listitem")).toHaveLength(2);
    expect(screen.getByRole("table")).toBeInTheDocument();
    expect(container.textContent).not.toContain("**");
  });

  it("never renders raw HTML from the assistant", () => {
    const { container } = render(
      <ChatPanel
        items={[{ role: "assistant", text: 'Hi <img src=x onerror="alert(1)"> <b>bold</b>' }]}
        running={false}
        hasSession
        onSend={vi.fn()}
        onStop={vi.fn()}
      />,
    );
    expect(container.querySelector("img, b")).toBeNull();
  });

  it("opens links outside the app", () => {
    render(
      <ChatPanel
        items={[{ role: "assistant", text: "[docs](https://www.odoo.com/documentation)" }]}
        running={false}
        hasSession
        onSend={vi.fn()}
        onStop={vi.fn()}
      />,
    );
    expect(screen.getByRole("link", { name: "docs" })).toHaveAttribute("target", "_blank");
  });

  it("keeps the user's own text as typed", () => {
    render(
      <ChatPanel
        items={[{ role: "user", text: "**not bold**" }]}
        running={false}
        hasSession
        onSend={vi.fn()}
        onStop={vi.fn()}
      />,
    );
    expect(screen.getByText("**not bold**")).toBeInTheDocument();
  });
});

describe("ChatPanel progress", () => {
  const props = { hasSession: true, onSend: vi.fn(), onStop: vi.fn() };

  it("shows a thinking indicator until the answer starts", () => {
    const { rerender } = render(<ChatPanel {...props} running items={[{ role: "user", text: "hi" }]} />);
    expect(screen.getByRole("status")).toHaveTextContent("Thinking");
    rerender(
      <ChatPanel
        {...props}
        running
        items={[
          { role: "user", text: "hi" },
          { role: "assistant", text: "Hel" },
        ]}
      />,
    );
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
    rerender(<ChatPanel {...props} running={false} items={[{ role: "user", text: "hi" }]} />);
    expect(screen.queryByRole("status")).not.toBeInTheDocument();
  });

  it("shows the stop button as busy while answering", () => {
    render(<ChatPanel {...props} running items={[]} />);
    expect(screen.getByRole("button", { name: "Stop" })).toHaveAttribute("aria-busy", "true");
  });
});

describe("ChatPanel model picker and usage", () => {
  it("shows the model picker and changes model", async () => {
    const props = setup();
    const picker = screen.getByRole("combobox", { name: "Model" });
    expect(picker).toHaveValue("sonnet");
    await userEvent.selectOptions(picker, "opus");
    expect(props.onModelChange).toHaveBeenCalledWith("opus");
  });

  it("shows token usage once there is any", () => {
    setup({ usage: { inputTokens: 1234, outputTokens: 567 } });
    const usage = screen.getByText(/tokens/i);
    expect(usage).toHaveTextContent("1,234");
    expect(usage).toHaveTextContent("567");
  });

  it("hides the usage line before any tokens are used", () => {
    setup({ usage: { inputTokens: 0, outputTokens: 0 } });
    expect(screen.queryByText(/tokens/i)).not.toBeInTheDocument();
  });

  it("disables the model picker while the assistant is answering", () => {
    setup({ running: true });
    expect(screen.getByRole("combobox", { name: "Model" })).toBeDisabled();
  });
});
