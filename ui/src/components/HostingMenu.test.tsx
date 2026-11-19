import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import HostingMenu from "./HostingMenu";

const chip = () => screen.getByRole("button", { name: /^Hosting/ });

describe("HostingMenu", () => {
  it("asks for the hosting until it is chosen", () => {
    render(<HostingMenu hosting={null} onChange={vi.fn()} />);
    expect(chip()).toHaveTextContent("Hosting?");
    expect(chip()).toHaveAccessibleName("Hosting: not chosen");
  });

  it("shows and changes where the client's Odoo runs", async () => {
    const onChange = vi.fn();
    render(<HostingMenu hosting="online" onChange={onChange} />);
    expect(chip()).toHaveTextContent("Odoo Online");
    await userEvent.click(chip());
    expect(screen.getByRole("menuitemradio", { name: "Odoo Online" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Odoo.sh" }));
    expect(onChange).toHaveBeenCalledWith("sh");
  });
});
