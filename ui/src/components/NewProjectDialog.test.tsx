import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import NewProjectDialog from "./NewProjectDialog";

describe("NewProjectDialog", () => {
  it("creates a project from its name", async () => {
    const onCreate = vi.fn(async () => {});
    render(<NewProjectDialog existing={["Acme"]} onCreate={onCreate} onCancel={vi.fn()} />);
    const field = screen.getByRole("textbox", { name: "Project name" });
    expect(field).toHaveFocus();
    expect(screen.getByRole("button", { name: "Create" })).toBeDisabled();
    await userEvent.type(field, "  Umbrella {Enter}");
    expect(onCreate).toHaveBeenCalledWith("Umbrella");
  });

  it("refuses a name already used", async () => {
    const onCreate = vi.fn(async () => {});
    render(<NewProjectDialog existing={["Acme"]} onCreate={onCreate} onCancel={vi.fn()} />);
    await userEvent.type(screen.getByRole("textbox", { name: "Project name" }), "acme{Enter}");
    expect(screen.getByRole("alert")).toHaveTextContent('"acme" already exists');
    expect(onCreate).not.toHaveBeenCalled();
  });

  it("shows why it failed, and cancels with Escape", async () => {
    const onCancel = vi.fn();
    render(
      <NewProjectDialog
        existing={[]}
        onCreate={() => Promise.reject("No space left.")}
        onCancel={onCancel}
      />,
    );
    await userEvent.type(screen.getByRole("textbox", { name: "Project name" }), "Umbrella{Enter}");
    expect(await screen.findByRole("alert")).toHaveTextContent("No space left.");
    await userEvent.keyboard("{Escape}");
    expect(onCancel).toHaveBeenCalled();
  });
});
