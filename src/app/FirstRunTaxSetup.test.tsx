import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { FirstRunTaxSetup } from "./FirstRunTaxSetup";
import type { SettingsService } from "@/services/ports";
import type { TaxRate } from "@/services/types";

function buildSettings(
  listResult: TaxRate[],
  seedTaxRates = vi.fn(async () => listResult),
): SettingsService {
  return {
    getHealth: vi.fn(),
    getCompanySettings: vi.fn(),
    updateCompanySettings: vi.fn(),
    listTaxRates: vi.fn(async () => listResult),
    saveTaxRate: vi.fn(),
    seedTaxRates,
    getReceiptSettings: vi.fn(),
    updateReceiptSettings: vi.fn(),
  } as unknown as SettingsService;
}

describe("FirstRunTaxSetup", () => {
  it("prompts an admin with no tax rates and seeds VAT on Da", async () => {
    const user = userEvent.setup();
    const seedTaxRates = vi.fn(async () => []);
    render(<FirstRunTaxSetup settings={buildSettings([], seedTaxRates)} role="admin" />);

    await user.click(await screen.findByRole("button", { name: "Da" }));
    expect(seedTaxRates).toHaveBeenCalledWith(true);
  });

  it("seeds the zero rate on Ne", async () => {
    const user = userEvent.setup();
    const seedTaxRates = vi.fn(async () => []);
    render(<FirstRunTaxSetup settings={buildSettings([], seedTaxRates)} role="admin" />);

    await user.click(await screen.findByRole("button", { name: "Ne" }));
    expect(seedTaxRates).toHaveBeenCalledWith(false);
  });

  it("renders nothing when rates already exist", async () => {
    render(
      <FirstRunTaxSetup
        settings={buildSettings([
          { id: 1, name: "PDV 20%", rateBasisPoints: 2000, active: true },
        ])}
        role="admin"
      />,
    );
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.queryByText("Da li ste u PDV sistemu?")).toBeNull();
  });

  it("renders nothing for a cashier", async () => {
    const settings = buildSettings([]);
    render(<FirstRunTaxSetup settings={settings} role="cashier" />);
    await new Promise((resolve) => setTimeout(resolve, 0));
    expect(screen.queryByText("Da li ste u PDV sistemu?")).toBeNull();
    expect(settings.listTaxRates).not.toHaveBeenCalled();
  });
});
