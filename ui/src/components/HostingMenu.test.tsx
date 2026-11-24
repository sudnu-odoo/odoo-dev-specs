import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import HostingMenu, { versionLabel } from "./HostingMenu";

/** The versions whose source is on this computer, in no particular order. */
const HERE = ["19.0", "saas-19.2", "18.0", "saas-18.4"];
const chip = () => screen.getByRole("button", { name: /^Hosting/ });

function setup(
  hosting: "online" | "sh" | null,
  version: string | null,
  versions: string[] = HERE,
  loading = false,
) {
  const onChange = vi.fn();
  render(
    <HostingMenu
      hosting={hosting}
      version={version}
      versions={versions}
      defaultVersion="19.0"
      loading={loading}
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

  it("Odoo Online starts on the newest version here", async () => {
    const onChange = setup(null, null);
    await userEvent.click(chip());
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Odoo Online" }));
    expect(onChange).toHaveBeenCalledWith("online", "saas-19.2");
  });

  it("Odoo Online can be any version here, SaaS or stable, newest first", async () => {
    const onChange = setup("online", "saas-19.2");
    await userEvent.click(chip());
    const versions = screen.getByRole("group", { name: "Odoo version" });
    expect(versions).toHaveTextContent(/19\.2.*latest.*19\.0.*18\.4.*18\.0/);
    await userEvent.click(screen.getByRole("menuitemradio", { name: "19.0" }));
    expect(onChange).toHaveBeenCalledWith("online", "19.0");
  });

  it("Odoo.sh offers only the stable versions here", async () => {
    const onChange = setup("sh", "19.0");
    await userEvent.click(chip());
    const versions = screen.getByRole("group", { name: "Odoo version" });
    expect(versions).toHaveTextContent(/19\.0.*18\.0/);
    expect(versions).not.toHaveTextContent("19.2");
    await userEvent.click(screen.getByRole("menuitemradio", { name: "18.0" }));
    expect(onChange).toHaveBeenCalledWith("sh", "18.0");
  });

  it("moving the client to Odoo.sh keeps a stable version", async () => {
    const onChange = setup("online", "saas-19.2");
    await userEvent.click(chip());
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Odoo.sh" }));
    expect(onChange).toHaveBeenCalledWith("sh", "19.0");
  });

  it("offers only what is on this computer, never versions without their source", async () => {
    setup("online", "20.0", ["20.0", "19.0"]);
    await userEvent.click(chip());
    const versions = screen.getByRole("group", { name: "Odoo version" });
    expect(versions).toHaveTextContent(/20\.0.*19\.0/);
    expect(screen.getAllByRole("menuitemradio")).toHaveLength(4);
  });

  it("Odoo Online starts on 20.0 when it is newer than every SaaS version here", async () => {
    const onChange = setup(null, null, ["saas-19.2", "20.0", "19.0"]);
    await userEvent.click(chip());
    await userEvent.click(screen.getByRole("menuitemradio", { name: "Odoo Online" }));
    expect(onChange).toHaveBeenCalledWith("online", "20.0");
  });

  it("says when no Odoo source is here yet", async () => {
    setup("sh", null, []);
    await userEvent.click(chip());
    expect(screen.getByRole("status")).toHaveTextContent("No Odoo source on this computer yet");
  });

  it("says the versions are loading instead of guessing them", async () => {
    setup("sh", "19.0", [], true);
    await userEvent.click(chip());
    expect(screen.getByRole("status")).toHaveTextContent("Loading versions…");
    expect(screen.queryByRole("menuitemradio", { name: "17.0" })).not.toBeInTheDocument();
  });
});
