import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { DatabaseInfo } from "../lib/types";
import DatabaseMenu from "./DatabaseMenu";

const base = {
  version: "19.0",
  demoData: true,
  enterprise: false,
  createdAt: 0,
  lastUsed: 0,
  sizeBytes: 0,
  running: false,
};
const DBS: DatabaseInfo[] = [
  { ...base, name: "specs_db_sales", label: "Sales demo", modules: ["sale_management"], auto: false },
  { ...base, name: "specs_19_0_stock_demo", label: "Auto 19.0: stock", modules: ["stock"], auto: true },
];

function setup(selected: string | null) {
  const props = { onChange: vi.fn(), onManage: vi.fn() };
  render(<DatabaseMenu databases={DBS} selected={selected} {...props} />);
  return props;
}
const chip = () => screen.getByRole("button", { name: /^Database/ });

describe("DatabaseMenu", () => {
  it("shows Automatic until a database is chosen, then its name", () => {
    setup(null);
    expect(chip()).toHaveTextContent("Automatic");
  });

  it("shows the chosen database and offers every database, the user's first", async () => {
    const props = setup("specs_19_0_stock_demo");
    expect(chip()).toHaveTextContent("Auto 19.0: stock");
    await userEvent.click(chip());
    const groups = screen.getAllByRole("group").map((g) => g.getAttribute("aria-label"));
    expect(groups).toEqual([null, "Your databases", "Made by the assistant"]);
    expect(screen.getByRole("menuitemradio", { name: "Auto 19.0: stock" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Sales demo" }));
    expect(props.onChange).toHaveBeenCalledWith("specs_db_sales");
  });

  it("goes back to Automatic and opens the Databases page", async () => {
    const props = setup("specs_db_sales");
    await userEvent.click(chip());
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Automatic" }));
    expect(props.onChange).toHaveBeenCalledWith(null);
    await userEvent.click(chip());
    await userEvent.click(screen.getByRole("menuitem", { name: "Manage databases…" }));
    expect(props.onManage).toHaveBeenCalled();
  });

  it("says when the chosen database was deleted", () => {
    setup("specs_db_gone");
    expect(chip()).toHaveTextContent("Deleted database");
  });

  it("makes a new database for the session and opens the session's one in the browser", async () => {
    const props = { onChange: vi.fn(), onManage: vi.fn(), onCreate: vi.fn(), onOpen: vi.fn() };
    render(<DatabaseMenu databases={DBS} selected="specs_db_sales" {...props} />);
    await userEvent.click(screen.getByRole("button", { name: "Open Sales demo in the browser" }));
    expect(props.onOpen).toHaveBeenCalled();
    await userEvent.click(chip());
    await userEvent.click(screen.getByRole("menuitem", { name: "New database…" }));
    expect(props.onCreate).toHaveBeenCalled();
  });

  it("has no Open button in Automatic, and shows a database being made", () => {
    const { rerender } = render(
      <DatabaseMenu databases={DBS} selected={null} onChange={vi.fn()} onManage={vi.fn()} onOpen={vi.fn()} />,
    );
    expect(screen.queryByRole("button", { name: /^Open/ })).not.toBeInTheDocument();
    rerender(
      <DatabaseMenu
        databases={DBS}
        selected={null}
        onChange={vi.fn()}
        onManage={vi.fn()}
        onOpen={vi.fn()}
        creating
      />,
    );
    expect(chip()).toHaveTextContent("Creating database…");
    expect(chip()).toBeDisabled();
  });
});
