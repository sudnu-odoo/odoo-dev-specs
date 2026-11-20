import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import HostingMenu, { versionLabel } from "./HostingMenu";

const AVAILABLE = ["saas-19.2", "saas-19.1", "19.0", "saas-18.4", "18.0", "17.0"];
const chip = () => screen.getByRole("button", { name: /^Hosting/ });

function setup(hosting: "online" | "sh" | null, version: string | null) {
  const onChange = vi.fn();
  render(
    <HostingMenu
      hosting={hosting}
      version={version}
      available={AVAILABLE}
      latestSaas="saas-19.2"
      defaultVersion="19.0"
      onChange={onChange}
    />,
  );
  return onChange;
}

describe("HostingMenu", () => {
  it("shows versions the way people say them", () => {
    expect(versionLabel("saas-19.2")).toBe("19.2");
    expect(versionLabel("19.0")).toBe("19.0");
  });

  it("asks for the hosting until it is chosen", () => {
    setup(null, null);
    expect(chip()).toHaveTextContent("Hosting?");
  });

  it("shows the hosting and version together", () => {
    setup("online", "saas-19.2");
    expect(chip()).toHaveTextContent("Odoo Online · 19.2");
  });

  it("Odoo Online picks the latest SaaS version; older ones stay selectable", async () => {
    const onChange = setup(null, null);
    await userEvent.click(chip());
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Odoo Online" }));
    expect(onChange).toHaveBeenCalledWith("online", "saas-19.2");
  });

  it("lists the SaaS versions for Odoo Online and the stable ones for Odoo.sh", async () => {
    const onChange = setup("online", "saas-19.2");
    await userEvent.click(chip());
    const versions = screen.getByRole("group", { name: "Odoo version" });
    expect(versions).toHaveTextContent(/19\.2.*latest.*19\.1.*18\.4/);
    expect(versions).not.toHaveTextContent("19.0");
    await userEvent.click(screen.getByRole("menuitemradio", { name: /^18\.4/ }));
    expect(onChange).toHaveBeenCalledWith("online", "saas-18.4");
  });

  it("moving the client to Odoo.sh keeps a stable version", async () => {
    const onChange = setup("online", "saas-19.2");
    await userEvent.click(chip());
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Odoo.sh" }));
    expect(onChange).toHaveBeenCalledWith("sh", "19.0");
  });

  it("says the versions are loading instead of guessing them", async () => {
    render(
      <HostingMenu
        hosting="sh"
        version="19.0"
        available={[]}
        latestSaas={null}
        defaultVersion="19.0"
        loading
        onChange={vi.fn()}
      />,
    );
    await userEvent.click(chip());
    expect(screen.getByRole("status")).toHaveTextContent("Loading versions…");
    expect(screen.queryByRole("menuitemradio", { name: "17.0" })).not.toBeInTheDocument();
  });
});
