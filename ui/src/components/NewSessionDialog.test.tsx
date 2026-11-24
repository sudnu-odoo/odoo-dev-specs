import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import NewSessionDialog from "./NewSessionDialog";

const meta = (id: string, updated: number) => ({
  id,
  title: id,
  provider: "claude",
  created_at: 0,
  updated_at: updated,
  started: true,
  conversation_id: null,
  database: null,
  hosting: null,
  version: null,
});
const clients = [
  { name: "Acme", sessions: [meta("a", 100)] },
  { name: "Globex", sessions: [meta("g", 500), meta("g2", 50)] },
  { name: "Initech", sessions: [] },
];

describe("NewSessionDialog", () => {
  it("lists the clients, most recently used first, and starts a session for the chosen one", async () => {
    const onStart = vi.fn(async () => {});
    render(<NewSessionDialog clients={clients} onStart={onStart} onCancel={vi.fn()} />);
    const names = screen.getAllByRole("option").map((o) => o.textContent);
    expect(names[0]).toMatch(/Globex/);
    expect(names[1]).toMatch(/Acme/);
    expect(screen.getByRole("option", { name: /Globex/ })).toHaveAttribute("aria-selected", "true");
    await userEvent.click(screen.getByRole("option", { name: /Acme/ }));
    await userEvent.click(screen.getByRole("button", { name: "Start" }));
    expect(onStart).toHaveBeenCalledWith("Acme", false);
  });

  it("adds a new client and starts its first session", async () => {
    const onStart = vi.fn(async () => {});
    render(<NewSessionDialog clients={clients} onStart={onStart} onCancel={vi.fn()} />);
    await userEvent.click(screen.getByRole("option", { name: /New client/ }));
    await userEvent.type(screen.getByRole("textbox", { name: "Client name" }), "Umbrella{Enter}");
    expect(onStart).toHaveBeenCalledWith("Umbrella", true);
  });

  it("shows why it could not start, and cancels with Escape", async () => {
    const onCancel = vi.fn();
    const onStart = vi.fn(async () => Promise.reject('A project named "Acme" already exists.'));
    render(<NewSessionDialog clients={clients} onStart={onStart} onCancel={onCancel} />);
    await userEvent.click(screen.getByRole("option", { name: /New client/ }));
    await userEvent.type(screen.getByRole("textbox", { name: "Client name" }), "Acme{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("already exists");
    await userEvent.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalled();
  });

  it("starts with a new client when there is none yet", () => {
    render(<NewSessionDialog clients={[]} onStart={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByRole("textbox", { name: "Client name" })).toHaveFocus();
  });
});
