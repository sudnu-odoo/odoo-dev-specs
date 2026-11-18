import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { ChatItem } from "../chat/chatState";
import ToolSteps, { toolLabel } from "./ToolSteps";

type Tool = Extract<ChatItem, { role: "tool" }>;
const tool = (name: string, status: Tool["status"] = "ok"): Tool => ({ role: "tool", name, status });

describe("ToolSteps", () => {
  it("names the assistant's steps in plain words", () => {
    expect(toolLabel("mcp__specs__diagram_add_text")).toBe("Adding text");
    expect(toolLabel("mcp__specs__odoo_screenshots")).toBe("Taking screenshots of Odoo");
    expect(toolLabel("Read")).toBe("Reading the Odoo source");
    expect(toolLabel("mcp__other__do_thing")).toBe("Do thing");
  });

  it("shows one line: the latest step and how many there were", () => {
    render(
      <ToolSteps
        steps={[
          tool("mcp__specs__odoo_screenshots"),
          tool("mcp__specs__diagram_add_text"),
          tool("mcp__specs__diagram_add_image"),
        ]}
      />,
    );
    const line = screen.getByRole("button", { name: /steps/ });
    expect(line).toHaveTextContent("Placing a screenshot");
    expect(line).toHaveTextContent("3 steps");
    expect(screen.queryByText("Adding text")).not.toBeInTheDocument();
  });

  it("shows the step that is running, with a spinner", () => {
    render(<ToolSteps steps={[tool("Read"), tool("Grep", "running")]} />);
    const current = screen.getByText("Searching the Odoo source");
    expect(current.closest("[data-status]")).toHaveAttribute("data-status", "running");
  });

  it("opens the full list on click", async () => {
    render(
      <ToolSteps
        steps={[tool("Read"), tool("mcp__specs__diagram_add_text"), tool("mcp__specs__diagram_summary")]}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: /steps/ }));
    const list = screen.getByRole("list", { name: "Steps" });
    expect(list).toHaveTextContent(/Reading the Odoo source.*Adding text.*Checking the layout/);
  });

  it("keeps a failed step visible in red", () => {
    render(
      <ToolSteps steps={[tool("mcp__specs__diagram_move", "error"), tool("mcp__specs__diagram_add_text")]} />,
    );
    const failed = screen.getByText("Moving something");
    expect(failed.closest("[data-status]")).toHaveAttribute("data-status", "error");
    expect(screen.getByRole("button", { name: /steps/ })).toHaveTextContent("Adding text");
  });
});
