import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { CatalogModule } from "./CatalogModule";
import { createMockServices } from "@/services/mock-adapter";

describe("CatalogModule", () => {
  it("shows the prethodna cena advisory when the price is lowered", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const getPrethodnaCena = vi
      .spyOn(services.catalog, "getPrethodnaCena")
      .mockResolvedValue({
        status: "computed",
        priceMinor: 499000,
        windowDays: 30,
        windowFrom: "2026-06-20T00:00:00Z",
        windowTo: "2026-07-20T00:00:00Z",
        truncated: false,
        reason: null,
        ageDays: null,
      });

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );

    const price = await screen.findByLabelText("Prodajna cena sa PDV");
    await user.clear(price);
    await user.type(price, "100");

    expect(
      await screen.findByText("Prethodna cena: 4.990,00 RSD"),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Prethodna cena izračunata prema čl. 37 st. 3. Mora biti istaknuta uz sniženu cenu na prodajnom mestu.",
      ),
    ).toBeInTheDocument();
    expect(getPrethodnaCena).toHaveBeenCalledWith(1, expect.any(String));
  });

  // ZoT čl. 37 was renumbered by 35/2026: st. 3 is the 30-day general rule and
  // st. 4 the shorter-assortment rule. Citing st. 3 for a st. 4 result is the
  // hardcoded-stav bug ZOT-36-37-VERIFIED-RULES.md §5.1/§5.2 warns against.
  it("cites čl. 37 st. 4 and the real window length for a short-assortment result", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    vi.spyOn(services.catalog, "getPrethodnaCena").mockResolvedValue({
      status: "computed",
      priceMinor: 229000,
      windowDays: 22,
      windowFrom: "2026-06-25T00:00:00Z",
      windowTo: "2026-07-17T00:00:00Z",
      truncated: true,
      reason: null,
      ageDays: null,
    });

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );

    const price = await screen.findByLabelText("Prodajna cena sa PDV");
    await user.clear(price);
    await user.type(price, "100");

    expect(
      await screen.findByText(
        "Prethodna cena izračunata prema čl. 37 st. 4. Mora biti istaknuta uz sniženu cenu na prodajnom mestu.",
      ),
    ).toBeInTheDocument();
    expect(
      screen.getByText(
        "Evidencija cena ne pokriva ceo period od 22 dana — proverite podatke.",
      ),
    ).toBeInTheDocument();
  });

  it("shows no advisory once the entered price is at or above the stored one", async () => {
    const user = userEvent.setup();
    const services = createMockServices();

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );

    // Mleko 1 l is stored at 159,99 RSD; 200,00 is a price increase.
    const price = await screen.findByLabelText("Prodajna cena sa PDV");
    await user.clear(price);
    await user.type(price, "200");

    await waitFor(() =>
      expect(screen.queryByText(/^Prethodna cena:/)).not.toBeInTheDocument(),
    );
  });

  it("explains that goods too new in the assortment have no reference period", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    vi.spyOn(services.catalog, "getPrethodnaCena").mockResolvedValue({
      status: "incomputable",
      priceMinor: null,
      windowDays: null,
      windowFrom: null,
      windowTo: null,
      truncated: false,
      reason: "too_new_in_assortment",
      ageDays: 6,
    });

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );

    const price = await screen.findByLabelText("Prodajna cena sa PDV");
    await user.clear(price);
    await user.type(price, "100");

    expect(
      await screen.findByText(
        "Roba je u asortimanu kraće od 15 dana — zakon ne propisuje jasan referentni period. Unesite prethodnu cenu ručno i obrazložite.",
      ),
    ).toBeInTheDocument();
  });

  it("warns when the price log does not cover the whole window", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    vi.spyOn(services.catalog, "getPrethodnaCena").mockResolvedValue({
      status: "computed",
      priceMinor: 429000,
      windowDays: 30,
      windowFrom: "2026-06-20T00:00:00Z",
      windowTo: "2026-07-20T00:00:00Z",
      truncated: true,
      reason: null,
      ageDays: null,
    });

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );

    const price = await screen.findByLabelText("Prodajna cena sa PDV");
    await user.clear(price);
    await user.type(price, "100");

    expect(
      await screen.findByText(
        "Evidencija cena ne pokriva ceo period od 30 dana — proverite podatke.",
      ),
    ).toBeInTheDocument();
  });

  it("reports an article that was not offered during the reference period", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    vi.spyOn(services.catalog, "getPrethodnaCena").mockResolvedValue({
      status: "incomputable",
      priceMinor: null,
      windowDays: null,
      windowFrom: null,
      windowTo: null,
      truncated: false,
      reason: "not_offered_in_window",
      ageDays: null,
    });

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );

    const price = await screen.findByLabelText("Prodajna cena sa PDV");
    await user.clear(price);
    await user.type(price, "100");

    expect(
      await screen.findByText(
        "Artikal nije bio u ponudi tokom referentnog perioda.",
      ),
    ).toBeInTheDocument();
  });

  it("reports a missing price log", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    vi.spyOn(services.catalog, "getPrethodnaCena").mockResolvedValue({
      status: "incomputable",
      priceMinor: null,
      windowDays: null,
      windowFrom: null,
      windowTo: null,
      truncated: false,
      reason: "no_history",
      ageDays: null,
    });

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );

    const price = await screen.findByLabelText("Prodajna cena sa PDV");
    await user.clear(price);
    await user.type(price, "100");

    expect(
      await screen.findByText("Nema evidencije cena za ovaj artikal."),
    ).toBeInTheDocument();
  });
});
