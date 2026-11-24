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

const many = Array.from({ length: 40 }, (_, i) => ({ name: `Client ${i}`, sessions: [meta(`c${i}`, i)] }));

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

  it("finds a client by typing, and Enter starts it", async () => {
    const onStart = vi.fn(async () => {});
    render(<NewSessionDialog clients={many} onStart={onStart} onCancel={vi.fn()} />);
    const search = screen.getByRole("combobox", { name: "Client" });
    expect(search).toHaveFocus();
    await userEvent.type(search, "client 7");
    const options = screen.getAllByRole("option");
    expect(options[0]).toHaveTextContent("Client 7");
    expect(options[0]).toHaveAttribute("aria-selected", "true");
    await userEvent.keyboard("{Enter}");
    expect(onStart).toHaveBeenCalledWith("Client 7", false);
  });

  it("moves through the list with the arrow keys", async () => {
    const onStart = vi.fn(async () => {});
    render(<NewSessionDialog clients={clients} onStart={onStart} onCancel={vi.fn()} />);
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowUp}{ArrowDown}{Enter}");
    expect(onStart).toHaveBeenCalledWith("Initech", false);
  });

  it("offers to create the client typed when none has that name", async () => {
    const onStart = vi.fn(async () => {});
    render(<NewSessionDialog clients={clients} onStart={onStart} onCancel={vi.fn()} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Client" }), "Umbrella");
    expect(screen.getByRole("option", { name: /Create client "Umbrella"/ })).toHaveAttribute(
      "aria-selected",
      "true",
    );
    await userEvent.keyboard("{Enter}");
    expect(onStart).toHaveBeenCalledWith("Umbrella", true);
  });

  it("does not offer to create a client that already exists", async () => {
    render(<NewSessionDialog clients={clients} onStart={vi.fn()} onCancel={vi.fn()} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Client" }), "acme");
    expect(screen.queryByRole("option", { name: /Create client/ })).not.toBeInTheDocument();
  });

  it("puts the current session's client first", () => {
    render(<NewSessionDialog clients={clients} current="Initech" onStart={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getAllByRole("option")[0]).toHaveTextContent("Initech");
  });

  it("shows why it could not start, and cancels with Escape", async () => {
    const onCancel = vi.fn();
    const onStart = vi.fn(async () => Promise.reject("Could not create it."));
    render(<NewSessionDialog clients={clients} onStart={onStart} onCancel={onCancel} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Client" }), "Umbrella{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("Could not create it.");
    await userEvent.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalled();
  });

  it("asks for a name when there is no client yet", () => {
    render(<NewSessionDialog clients={[]} onStart={vi.fn()} onCancel={vi.fn()} />);
    expect(screen.getByRole("combobox", { name: "Client" })).toHaveFocus();
    expect(screen.getByText(/Type the client's name/)).toBeInTheDocument();
  });
});
