import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { api } from "../lib/api";
import SearchPalette from "./SearchPalette";

vi.mock("../lib/api", () => ({ api: { searchSessions: vi.fn() } }));
const mocked = vi.mocked(api);

const meta = (id: string, title: string, hosting: "online" | "sh" | null, version: string | null) => ({
  id,
  title,
  provider: "claude",
  created_at: 0,
  updated_at: 0,
  started: true,
  conversation_id: null,
  database: null,
  hosting,
  version,
});

beforeEach(() => {
  vi.clearAllMocks();
  mocked.searchSessions.mockImplementation(async (q: string) =>
    q.includes("expiration")
      ? [
          {
            project: "Acme",
            session: meta("s1", "Stock report", "online", "saas-19.2"),
            snippets: ["…the lot's expiration_date is already printed…"],
          },
          { project: "Globex", session: meta("s3", "Lots on pickings", "sh", "18.0"), snippets: [] },
        ]
      : [{ project: "Acme", session: meta("s2", "Revise quotation", null, null), snippets: [] }],
  );
});

describe("SearchPalette", () => {
  it("shows recent sessions at first, then searches what was said, grouped by client", async () => {
    render(<SearchPalette onOpen={vi.fn()} onClose={vi.fn()} />);
    expect(await screen.findByRole("option", { name: /Revise quotation/ })).toBeInTheDocument();
    await userEvent.type(screen.getByRole("combobox", { name: "Search sessions" }), "expiration");
    const hit = await screen.findByRole("option", { name: /Stock report/ });
    expect(hit).toHaveTextContent("Online · 19.2");
    expect(hit).toHaveTextContent("expiration_date is already printed");
    expect(screen.getByRole("group", { name: "Acme" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: "Globex" })).toBeInTheDocument();
  });

  it("moves with the arrows and opens with Enter or a click", async () => {
    const onOpen = vi.fn();
    render(<SearchPalette onOpen={onOpen} onClose={vi.fn()} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Search sessions" }), "expiration");
    await screen.findByRole("option", { name: /Lots on pickings/ });
    await userEvent.keyboard("{ArrowDown}{Enter}");
    expect(onOpen).toHaveBeenCalledWith({ project: "Globex", sessionId: "s3" });
    await userEvent.click(screen.getByRole("option", { name: /Stock report/ }));
    expect(onOpen).toHaveBeenCalledWith({ project: "Acme", sessionId: "s1" });
  });

  it("closes on Escape and says when nothing matches", async () => {
    mocked.searchSessions.mockResolvedValue([]);
    const onClose = vi.fn();
    render(<SearchPalette onOpen={vi.fn()} onClose={onClose} />);
    await userEvent.type(screen.getByRole("combobox", { name: "Search sessions" }), "zzz");
    expect(await screen.findByText(/No session mentions/)).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });
});
