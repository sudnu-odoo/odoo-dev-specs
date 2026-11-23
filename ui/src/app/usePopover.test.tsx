import { act, fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { usePopover } from "./usePopover";

function Menu() {
  const menu = usePopover<HTMLDivElement>();
  return (
    <div>
      <div ref={menu.root}>
        <button onClick={menu.toggle}>Toggle</button>
        {menu.open && <p>Inside</p>}
      </div>
      <p>Outside</p>
    </div>
  );
}

describe("usePopover", () => {
  it("toggles, and closes on a click outside or Escape, not on a click inside", () => {
    render(<Menu />);
    const toggle = screen.getByRole("button", { name: "Toggle" });
    act(() => toggle.click());
    expect(screen.getByText("Inside")).toBeInTheDocument();
    fireEvent.mouseDown(screen.getByText("Inside"));
    expect(screen.getByText("Inside")).toBeInTheDocument();
    fireEvent.mouseDown(screen.getByText("Outside"));
    expect(screen.queryByText("Inside")).not.toBeInTheDocument();
    act(() => toggle.click());
    fireEvent.keyDown(document, { key: "Escape" });
    expect(screen.queryByText("Inside")).not.toBeInTheDocument();
    act(() => toggle.click());
    act(() => toggle.click());
    expect(screen.queryByText("Inside")).not.toBeInTheDocument();
  });
});
