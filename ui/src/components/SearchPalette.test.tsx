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
          { project: "Acme", session: meta("s9", "Expiration in the title", null, null), snippets: [] },
          {
            project: "Globex",
            session: meta("s3", "Lots on pickings", "sh", "18.0"),
            snippets: ["…expiration…"],
          },
        ]
      : [{ project: "Acme", session: meta("s2", "Revise quotation", null, null), snippets: [] }],
  );
});

const projects = [
  {
    name: "Acme",
    sessions: [meta("s1", "Stock report", "online", "saas-19.2"), meta("s2", "Revise quotation", null, null)],
  },
  { name: "Globex", sessions: [meta("s3", "Lots on pickings", "sh", "18.0")] },
  { name: "Expiration Labs", sessions: [] },
];

function setup() {
  const props = { projects, onOpen: vi.fn(), onOpenProject: vi.fn(), onClose: vi.fn() };
  render(<SearchPalette {...props} />);
  return props;
}

describe("SearchPalette", () => {
  it("shows recent sessions at first, with their project", async () => {
    setup();
    const recent = await screen.findByRole("option", { name: /Revise quotation/ });
    expect(recent).toHaveTextContent("Acme");
  });

  it("finds projects, sessions by name, and sessions by what was said, each in its own section", async () => {
    setup();
    await userEvent.type(screen.getByRole("combobox", { name: "Search" }), "expiration");
    const projectsGroup = await screen.findByRole("group", { name: "Projects" });
    expect(projectsGroup).toHaveTextContent("Expiration Labs");
    const hit = await screen.findByRole("option", { name: /Stock report/ });
    const said = screen.getByRole("group", { name: "In conversations" });
    expect(said).toContainElement(hit);
    expect(hit).toHaveTextContent("Acme");
    expect(hit).toHaveTextContent("Online · 19.2");
    expect(hit).toHaveTextContent("expiration_date is already printed");
    expect(screen.getByRole("group", { name: "Sessions" })).toHaveTextContent("Expiration in the title");
  });

  it("moves with the arrows through every section, and opens a project or a session", async () => {
    const props = setup();
    await userEvent.type(screen.getByRole("combobox", { name: "Search" }), "expiration");
    await screen.findByRole("option", { name: /Stock report/ });
    await userEvent.keyboard("{Enter}");
    expect(props.onOpenProject).toHaveBeenCalledWith("Expiration Labs");
    await userEvent.keyboard("{ArrowDown}{ArrowDown}{Enter}");
    expect(props.onOpen).toHaveBeenCalledWith({ project: "Acme", sessionId: "s1" });
    await userEvent.click(screen.getByRole("option", { name: /Lots on pickings/ }));
    expect(props.onOpen).toHaveBeenCalledWith({ project: "Globex", sessionId: "s3" });
  });

  it("closes on Escape and says when nothing matches", async () => {
    mocked.searchSessions.mockResolvedValue([]);
    const props = setup();
    await userEvent.type(screen.getByRole("combobox", { name: "Search" }), "zzz");
    expect(await screen.findByText(/Nothing matches/)).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(props.onClose).toHaveBeenCalled();
  });
});
