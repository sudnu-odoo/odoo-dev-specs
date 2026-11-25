import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { describe, expect, it, vi } from "vitest";
import Modal from "./Modal";

describe("Modal", () => {
  it("is a native dialog, labelled, that closes on Escape and outside clicks", async () => {
    const onClose = vi.fn();
    render(
      <Modal label="New project" onClose={onClose}>
        <input aria-label="Name" />
      </Modal>,
    );
    const dialog = screen.getByRole("dialog", { name: "New project" });
    expect(dialog.tagName).toBe("DIALOG");
    expect(dialog).toHaveAttribute("open");
    dialog.dispatchEvent(new Event("cancel", { cancelable: true }));
    expect(onClose).toHaveBeenCalledTimes(1);
    await userEvent.pointer({ keys: "[MouseLeft]", target: dialog });
    expect(onClose).toHaveBeenCalledTimes(2);
    await userEvent.click(screen.getByRole("textbox", { name: "Name" }));
    expect(onClose).toHaveBeenCalledTimes(2);
  });

  it("gives focus back to what had it before", async () => {
    function Page() {
      const [open, setOpen] = useState(false);
      return (
        <>
          <button onClick={() => setOpen(true)}>Open</button>
          {open && (
            <Modal label="Box" onClose={() => setOpen(false)}>
              <button onClick={() => setOpen(false)}>Done</button>
            </Modal>
          )}
        </>
      );
    }
    render(<Page />);
    await userEvent.click(screen.getByRole("button", { name: "Open" }));
    await userEvent.click(screen.getByRole("button", { name: "Done" }));
    expect(screen.getByRole("button", { name: "Open" })).toHaveFocus();
  });

  it("starts in its field, or on itself when it has none (no focus ring on a button)", () => {
    const { unmount } = render(
      <Modal label="With field" onClose={vi.fn()}>
        <button>First</button>
        <input aria-label="Name" />
      </Modal>,
    );
    expect(screen.getByRole("textbox", { name: "Name" })).toHaveFocus();
    unmount();
    render(
      <Modal label="Buttons only" onClose={vi.fn()}>
        <button>General</button>
      </Modal>,
    );
    expect(screen.getByRole("dialog", { name: "Buttons only" })).toHaveFocus();
  });
});
