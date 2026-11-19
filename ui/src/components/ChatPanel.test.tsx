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
    // The steps fold into one line showing the latest.
    expect(screen.getByRole("button", { name: /2 steps/ })).toHaveTextContent("Searching the Odoo source");
    expect(screen.getByRole("button", { name: /2 steps/ })).toHaveAttribute("data-status", "running");
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
  const modelButton = () => screen.getByRole("button", { name: /^Model/ });

  it("shows the chosen model on a chip and changes it from a menu", async () => {
    const props = setup();
    expect(modelButton()).toHaveTextContent("Sonnet");
    expect(modelButton()).not.toHaveTextContent("balanced");
    await userEvent.click(modelButton());
    const opus = screen.getByRole("menuitemradio", { name: "Opus (most capable)" });
    expect(screen.getByRole("menuitemradio", { name: /Sonnet \(balanced\)/ })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await userEvent.click(opus);
    expect(props.onModelChange).toHaveBeenCalledWith("opus");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  it("closes the model menu with Escape", async () => {
    setup();
    await userEvent.click(modelButton());
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });

  const TWO_SERVICES = [
    { id: "sonnet", label: "Sonnet (balanced)", provider: "Claude" },
    { id: "opus", label: "Opus (most capable)", provider: "Claude" },
    { id: "gemini-3.8-flash-low", label: "Gemini 3.8 Flash (Low)", provider: "Antigravity" },
  ];

  it("first picks the AI service, then lists only that service's models", async () => {
    const props = setup({ models: TWO_SERVICES, effort: "low", onEffortChange: vi.fn() });
    await userEvent.click(modelButton());
    const services = screen.getByRole("group", { name: "AI service" });
    expect(services).toHaveTextContent(/Claude.*Antigravity/);
    expect(screen.getByRole("group", { name: "Claude models" })).toHaveTextContent(/Sonnet.*Opus/);
    expect(screen.queryByText(/Gemini/)).not.toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Effort" })).toBeInTheDocument();

    await userEvent.click(screen.getByRole("menuitemradio", { name: "Antigravity" }));
    expect(screen.getByRole("group", { name: "Antigravity models" })).toHaveTextContent("Gemini 3.8 Flash");
    expect(screen.queryByRole("menuitemradio", { name: /Sonnet/ })).not.toBeInTheDocument();
    // Effort belongs to the chosen Claude model, not to another service's list.
    expect(screen.queryByRole("group", { name: "Effort" })).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("menuitemradio", { name: /Gemini/ }));
    expect(props.onModelChange).toHaveBeenCalledWith("gemini-3.8-flash-low");
  });

  it("a model with its level in its name has no effort, and the chip shows that level", async () => {
    setup({
      models: TWO_SERVICES,
      selectedModel: "gemini-3.8-flash-low",
      effort: "high",
      onEffortChange: vi.fn(),
    });
    expect(modelButton()).toHaveTextContent(/^Gemini 3.8 FlashLow$/);
    await userEvent.click(modelButton());
    expect(screen.getByRole("group", { name: "Antigravity models" })).toBeInTheDocument();
    expect(screen.queryByRole("group", { name: "Effort" })).not.toBeInTheDocument();
  });

  it("picks an effort level in one row of the same menu", async () => {
    const props = setup({ effort: "", onEffortChange: vi.fn() });
    await userEvent.click(modelButton());
    const effort = screen.getByRole("group", { name: "Effort" });
    expect(effort).toHaveTextContent(/Default.*Low.*Medium.*High.*Xhigh.*Max/);
    await userEvent.click(screen.getByRole("menuitemradio", { name: "High" }));
    expect(props.onEffortChange).toHaveBeenCalledWith("high");
  });

  it("shows the chosen effort on the chip", () => {
    setup({ effort: "high", onEffortChange: vi.fn() });
    expect(modelButton()).toHaveTextContent(/Sonnet.*High/);
  });

  it("locks a service that is not logged in and offers to log in", async () => {
    const props = setup({
      models: [
        { id: "sonnet", label: "Sonnet", provider: "Claude" },
        { id: "gem", label: "Gemini Pro", provider: "Antigravity", locked: true },
      ],
      onLogin: vi.fn(),
    });
    await userEvent.click(modelButton());
    const agy = screen.getByRole("menuitemradio", { name: /Antigravity/ });
    expect(agy).toHaveTextContent("🔒");
    expect(agy).toHaveAttribute("aria-disabled", "true");
    await userEvent.click(agy);
    expect(screen.queryByText("Gemini Pro")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Log in to Antigravity" }));
    expect(props.onLogin).toHaveBeenCalledWith("antigravity");
  });

  it("in a session tied to one service, the other service is unavailable", async () => {
    setup({
      models: [
        { id: "sonnet", label: "Sonnet", provider: "Claude" },
        { id: "gem", label: "Gemini Pro", provider: "Antigravity", locked: true },
      ],
      sessionProvider: "Claude",
      onLogin: vi.fn(),
    });
    await userEvent.click(modelButton());
    const agy = screen.getByRole("menuitemradio", { name: /Antigravity/ });
    expect(agy).toHaveTextContent("new session");
    expect(agy).toHaveAttribute("aria-disabled", "true");
    // Not a login matter here: no Log in button.
    expect(screen.queryByRole("button", { name: /Log in/ })).not.toBeInTheDocument();
  });

  it("disables the model chip while the assistant is answering", () => {
    setup({ running: true });
    expect(modelButton()).toBeDisabled();
  });

  it("shows token usage under the composer until plan limits are known", () => {
    setup({ usage: { inputTokens: 1234, outputTokens: 567 } });
    const usage = screen.getByText(/tokens/i);
    expect(usage).toHaveTextContent("1,234");
    expect(usage).toHaveTextContent("567");
  });

  it("hides the usage line before any tokens are used", () => {
    setup({ usage: { inputTokens: 0, outputTokens: 0 } });
    expect(screen.queryByText(/tokens/i)).not.toBeInTheDocument();
  });

  it("shows a small plan-limit meter that opens the usage details", async () => {
    const now = Date.now() / 1000;
    setup({
      usage: { inputTokens: 1234, outputTokens: 567 },
      plan: {
        five_hour: { utilization: 0.21, resets_at: now + 2 * 3600 + 14 * 60 },
        seven_day: { utilization: 0.71, resets_at: now + 3 * 86400 },
      },
    });
    const meter = screen.getByRole("meter", { name: /current session/i });
    expect(meter).toHaveAttribute("aria-valuenow", "21");
    expect(screen.getByText(/21% of session limit/i)).toBeInTheDocument();
    expect(screen.queryByText(/tokens/i)).not.toBeInTheDocument();

    await userEvent.click(screen.getByRole("button", { name: /usage/i }));
    const details = screen.getByRole("dialog", { name: "Usage" });
    expect(details).toHaveTextContent(/Current session/);
    expect(details).toHaveTextContent(/resets in 2h 1[34]m/);
    expect(details).toHaveTextContent(/Weekly/);
    expect(details).toHaveTextContent("71%");
  });

  it("shows Thinking… only when no running step already shows the work", () => {
    const { rerender } = render(
      <ChatPanel
        items={[{ role: "user", text: "Hi" }]}
        running
        hasSession
        onSend={vi.fn()}
        onStop={vi.fn()}
      />,
    );
    expect(screen.getByRole("status")).toHaveTextContent("Thinking");
    rerender(
      <ChatPanel
        items={[
          { role: "user", text: "Hi" },
          { role: "tool", name: "Read", status: "running" },
        ]}
        running
        hasSession
        onSend={vi.fn()}
        onStop={vi.fn()}
      />,
    );
    expect(screen.queryByText("Thinking…")).not.toBeInTheDocument();
  });
});
