import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Databases } from "../app/useDatabases";
import type { DatabaseInfo } from "../lib/types";
import DatabasesPage from "./DatabasesPage";

const db = (over: Partial<DatabaseInfo>): DatabaseInfo => ({
  name: "specs_db_x",
  label: "X",
  version: "19.0",
  modules: [],
  demoData: true,
  enterprise: false,
  auto: false,
  createdAt: 0,
  lastUsed: Date.now(),
  sizeBytes: 420 * 1024 ** 2,
  running: false,
  ...over,
});

const SALES = db({
  name: "specs_db_sales",
  label: "Sales demo",
  modules: ["sale_management", "crm"],
  running: true,
});
const AUTO = db({ name: "specs_19_0_stock_demo", label: "Auto 19.0: stock", modules: ["stock"], auto: true });

function setup(over: Partial<Databases> = {}, usage: Record<string, number> = {}) {
  const dbs = {
    databases: [AUTO, SALES],
    busy: {},
    creating: false,
    error: null,
    dismissError: vi.fn(),
    reload: vi.fn(),
    create: vi.fn(),
    open: vi.fn(),
    stop: vi.fn(),
    rename: vi.fn(),
    duplicate: vi.fn(),
    reset: vi.fn(),
    remove: vi.fn(),
    // Enterprise is set up for 19.0 only; names count per version.
    formFor: vi.fn(async (version: string) => ({
      suggestedLabel: `db-${version.replace(".0", "")}-3`,
      enterpriseAvailable: version === "19.0",
    })),
    ...over,
  } as unknown as Databases;
  render(<DatabasesPage dbs={dbs} usage={usage} />);
  return dbs;
}

describe("DatabasesPage", () => {
  it("lists the user's databases first, then the assistant's, with their details", () => {
    setup({}, { specs_db_sales: 2 });
    const headings = screen.getAllByRole("heading", { level: 3 }).map((h) => h.textContent);
    expect(headings).toEqual(["Your databases", "Made by the assistant"]);
    const row = screen.getByRole("listitem", { name: "Sales demo" });
    expect(row).toHaveTextContent("19.0 · Sales, CRM · Community · demo data");
    expect(row).toHaveTextContent(/Running · 420 MB · used today · 2 sessions/);
    expect(screen.getByRole("listitem", { name: "Auto 19.0: stock" })).toHaveTextContent("Stopped");
  });

  it("opens a database in the browser and stops a running one", async () => {
    const dbs = setup();
    const row = screen.getByRole("listitem", { name: "Sales demo" });
    await userEvent.click(within(row).getByRole("button", { name: "Open" }));
    expect(dbs.open).toHaveBeenCalledWith(SALES);
    await userEvent.click(within(row).getByRole("button", { name: "Stop" }));
    expect(dbs.stop).toHaveBeenCalledWith(SALES);
    const stopped = screen.getByRole("listitem", { name: "Auto 19.0: stock" });
    expect(within(stopped).queryByRole("button", { name: "Stop" })).not.toBeInTheDocument();
  });

  it("suggests the next numbered name and creates with the chosen edition, apps and demo data", async () => {
    const dbs = setup();
    await userEvent.click(screen.getByRole("button", { name: "New database" }));
    const dialog = screen.getByRole("dialog", { name: "New database" });
    const name = within(dialog).getByLabelText("Name");
    expect(await within(dialog).findByDisplayValue("db-19-3")).toBe(name);
    // Enterprise by default where it is set up.
    expect(within(dialog).getByRole("radio", { name: "Enterprise" })).toHaveAttribute("aria-checked", "true");
    await userEvent.click(within(dialog).getByRole("radio", { name: "Community" }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Inventory" }));
    await userEvent.click(within(dialog).getByRole("button", { name: "Purchase" }));
    await userEvent.click(within(dialog).getAllByRole("radio", { name: "Off" })[0]);
    await userEvent.click(within(dialog).getByRole("button", { name: "Create" }));
    expect(dbs.create).toHaveBeenCalledWith({
      label: "db-19-3",
      version: "19.0",
      modules: ["stock", "purchase"],
      demoData: false,
      enterprise: false,
    });
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("follows the version: its next name, and no Enterprise where it is not set up", async () => {
    setup();
    await userEvent.click(screen.getByRole("button", { name: "New database" }));
    const dialog = screen.getByRole("dialog", { name: "New database" });
    await within(dialog).findByDisplayValue("db-19-3");
    await userEvent.selectOptions(within(dialog).getByLabelText("Odoo version"), "18.0");
    expect(await within(dialog).findByDisplayValue("db-18-3")).toBeInTheDocument();
    const enterprise = within(dialog).getByRole("radio", { name: "Enterprise" });
    expect(enterprise).toBeDisabled();
    expect(within(dialog).getByRole("radio", { name: "Community" })).toHaveAttribute("aria-checked", "true");
    expect(dialog).toHaveTextContent("Enterprise is not set up for Odoo 18.0");
  });

  it("keeps a name the user typed when the version changes", async () => {
    const dbs = setup();
    await userEvent.click(screen.getByRole("button", { name: "New database" }));
    const dialog = screen.getByRole("dialog", { name: "New database" });
    const name = await within(dialog).findByDisplayValue("db-19-3");
    await userEvent.clear(name);
    await userEvent.type(name, "Sales demo");
    await userEvent.selectOptions(within(dialog).getByLabelText("Odoo version"), "17.0");
    await within(dialog).findByText(/not set up for Odoo 17.0/);
    expect(name).toHaveValue("Sales demo");
    await userEvent.click(within(dialog).getByRole("button", { name: "Create" }));
    expect(dbs.create).toHaveBeenCalledWith(
      expect.objectContaining({ label: "Sales demo", version: "17.0" }),
    );
  });

  it("says creating takes a while, and shows what a busy database is doing", () => {
    setup({ creating: true, busy: { specs_db_sales: "copying" } });
    expect(screen.getByRole("status")).toHaveTextContent(/few minutes/);
    const row = screen.getByRole("listitem", { name: "Sales demo" });
    expect(row).toHaveTextContent("Copying…");
    expect(within(row).getByRole("button", { name: "Open" })).toBeDisabled();
  });

  it("renames and duplicates from the row menu", async () => {
    const dbs = setup();
    const row = screen.getByRole("listitem", { name: "Sales demo" });
    await userEvent.click(within(row).getByRole("button", { name: "More actions for Sales demo" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Rename" }));
    const name = within(screen.getByRole("dialog", { name: "Rename database" })).getByLabelText("Name");
    await userEvent.clear(name);
    await userEvent.type(name, "Sales v2{Enter}");
    expect(dbs.rename).toHaveBeenCalledWith(SALES, "Sales v2");

    await userEvent.click(within(row).getByRole("button", { name: "More actions for Sales demo" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Duplicate" }));
    const copy = within(screen.getByRole("dialog", { name: "Duplicate database" })).getByLabelText("Name");
    expect(copy).toHaveValue("Sales demo (copy)");
    await userEvent.click(screen.getByRole("button", { name: "Duplicate" }));
    expect(dbs.duplicate).toHaveBeenCalledWith(SALES, "Sales demo (copy)");
  });

  it("resets and deletes, telling how many sessions use it", async () => {
    const dbs = setup({}, { specs_db_sales: 3 });
    const row = screen.getByRole("listitem", { name: "Sales demo" });
    await userEvent.click(within(row).getByRole("button", { name: /More actions/ }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Reset to fresh" }));
    expect(dbs.reset).toHaveBeenCalledWith(SALES);
    await userEvent.click(within(row).getByRole("button", { name: /More actions/ }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Delete" }));
    expect(dbs.remove).toHaveBeenCalledWith(SALES, 3);
  });

  it("invites to create a first database", () => {
    setup({ databases: [AUTO] });
    expect(screen.getByText(/None yet/)).toBeInTheDocument();
  });
});
