import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SettingsScreen } from "./SettingsScreen";
import { createMockServices } from "@/services/mock-adapter";

describe("SettingsScreen VAT rates", () => {
  it("edits an existing VAT rate and threads its id to saveTaxRate", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const saveTaxRate = vi.spyOn(services.settings, "saveTaxRate");

    render(<SettingsScreen services={services} usersPanel={<div />} />);

    await user.click(await screen.findByRole("tab", { name: "PDV" }));
    await user.click(await screen.findByRole("button", { name: "Uredi PDV 20" }));

    const dialog = await screen.findByRole("dialog", { name: "PDV stopa" });
    const nameInput = within(dialog).getByLabelText("Naziv");
    expect(nameInput).toHaveValue("PDV 20");

    await user.clear(nameInput);
    await user.type(nameInput, "PDV 20 standard");
    await user.click(within(dialog).getByRole("button", { name: "Sacuvaj PDV stopu" }));

    await waitFor(() =>
      expect(saveTaxRate).toHaveBeenCalledWith({
        id: 1,
        name: "PDV 20 standard",
        rateBasisPoints: 2000,
        active: true,
      }),
    );
    expect(await screen.findByText("PDV 20 standard")).toBeInTheDocument();
  });

  it("deactivates a VAT rate instead of deleting it", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const saveTaxRate = vi.spyOn(services.settings, "saveTaxRate");

    render(<SettingsScreen services={services} usersPanel={<div />} />);

    await user.click(await screen.findByRole("tab", { name: "PDV" }));
    await user.click(await screen.findByRole("button", { name: "Deaktiviraj PDV 20" }));

    await waitFor(() =>
      expect(saveTaxRate).toHaveBeenCalledWith({
        id: 1,
        name: "PDV 20",
        rateBasisPoints: 2000,
        active: false,
      }),
    );

    const row = (await screen.findByText("PDV 20")).closest("tr");
    expect(row).not.toBeNull();
    expect(within(row as HTMLElement).getByText("Neaktivna")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Deaktiviraj PDV 20" }),
    ).not.toBeInTheDocument();
  });

  it("blocks saving a VAT rate with an empty name", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const saveTaxRate = vi.spyOn(services.settings, "saveTaxRate");

    render(<SettingsScreen services={services} usersPanel={<div />} />);

    await user.click(await screen.findByRole("tab", { name: "PDV" }));
    await user.click(await screen.findByRole("button", { name: "Uredi PDV 20" }));

    const dialog = await screen.findByRole("dialog", { name: "PDV stopa" });
    await user.clear(within(dialog).getByLabelText("Naziv"));
    await user.click(within(dialog).getByRole("button", { name: "Sacuvaj PDV stopu" }));

    expect(
      await within(dialog).findByText("Naziv PDV stope je obavezan."),
    ).toBeInTheDocument();
    expect(saveTaxRate).not.toHaveBeenCalled();
  });
});
