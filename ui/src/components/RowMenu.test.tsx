import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import RowMenu from "./RowMenu";

describe("RowMenu", () => {
  it("opens its actions from ⋯, runs one and closes", async () => {
    const pin = vi.fn();
    render(
      <RowMenu
        label="Invoice"
        items={[
          { text: "Pin", icon: "pin", run: pin },
          { text: "Delete", danger: true, run: vi.fn() },
        ]}
      />,
    );
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "More actions for Invoice" }));
    expect(screen.getByRole("menuitem", { name: "Delete" })).toHaveClass("menu-item-danger");
    await userEvent.click(screen.getByRole("menuitem", { name: "Pin" }));
    expect(pin).toHaveBeenCalled();
    expect(screen.queryByRole("menu")).not.toBeInTheDocument();
  });
});
