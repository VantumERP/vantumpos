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

  // čl. 37 st. 3's carve-out is from the *definition* of prethodna cena, not
  // from st. 2's duty to display one (ZOT-36-37-VERIFIED-RULES.md §2.6). So the
  // flag opens a manual-entry path with a recorded justification; it never
  // suppresses anything. Default FALSE is the stricter rule (memo §5.5).
  it("reveals a justification field when the perishable flag is checked", async () => {
    const user = userEvent.setup();
    const services = createMockServices();

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );

    expect(screen.queryByLabelText("Obrazloženje")).not.toBeInTheDocument();

    await user.click(
      screen.getByRole("checkbox", {
        name: "Lako kvarljiva roba / kratak rok trajanja",
      }),
    );

    expect(await screen.findByLabelText("Obrazloženje")).toBeInTheDocument();
    expect(
      screen.getByText(
        "Za kvarljivu robu prethodna cena se unosi ručno pri sniženju (čl. 37 st. 3).",
      ),
    ).toBeInTheDocument();
  });

  it("requires a justification before saving a perishable article", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const updateProduct = vi.spyOn(services.catalog, "updateProduct");

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );
    await user.click(
      screen.getByRole("checkbox", {
        name: "Lako kvarljiva roba / kratak rok trajanja",
      }),
    );
    await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

    expect(
      await screen.findByText("Obrazloženje je obavezno za kvarljivu robu."),
    ).toBeInTheDocument();
    expect(updateProduct).not.toHaveBeenCalled();
  });

  it("sends the perishable flag and justification on save", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const updateProduct = vi.spyOn(services.catalog, "updateProduct");

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );
    await user.click(
      screen.getByRole("checkbox", {
        name: "Lako kvarljiva roba / kratak rok trajanja",
      }),
    );
    await user.type(
      await screen.findByLabelText("Obrazloženje"),
      "Svež proizvod, rok trajanja 5 dana.",
    );
    await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

    await waitFor(() => {
      expect(updateProduct).toHaveBeenCalledWith(
        1,
        expect.objectContaining({
          perishable: true,
          perishableJustification: "Svež proizvod, rok trajanja 5 dana.",
        }),
      );
    });
  });

  it("round-trips the perishable flag back into the form after save", async () => {
    const user = userEvent.setup();
    const services = createMockServices();

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );
    await user.click(
      screen.getByRole("checkbox", {
        name: "Lako kvarljiva roba / kratak rok trajanja",
      }),
    );
    await user.type(
      await screen.findByLabelText("Obrazloženje"),
      "Kratak rok trajanja.",
    );
    await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );

    expect(
      await screen.findByRole("checkbox", {
        name: "Lako kvarljiva roba / kratak rok trajanja",
      }),
    ).toBeChecked();
    expect(await screen.findByLabelText("Obrazloženje")).toHaveValue(
      "Kratak rok trajanja.",
    );
  });

  it("sends perishable false when the flag is left unchecked", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const updateProduct = vi.spyOn(services.catalog, "updateProduct");

    render(<CatalogModule services={services} onOpenInventory={() => {}} />);

    await user.click(
      await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
    );
    await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

    await waitFor(() => {
      expect(updateProduct).toHaveBeenCalledWith(
        1,
        expect.objectContaining({
          perishable: false,
          perishableJustification: null,
        }),
      );
    });
  });

  /**
   * SW-12 req. 10 `[LEGAL]`. ZZP čl. 6 st. 2's second sentence pulls st. 1 into
   * the published cenovnik, so the file has to carry a jedinična cena — and it
   * can only carry one if the form captures the measure and the package content
   * the backend divides by. Without this the published file's jedinična cena is
   * empty for every article.
   *
   * A 0,75 l bottle at 279,00 RSD is 372,00 RSD per litar; the division itself
   * is asserted in `src-tauri/src/cenovnik.rs`, and what this test owns is that
   * the pair the division needs actually leaves the form.
   */
  describe("jedinična cena (ZZP čl. 6 st. 1/st. 2)", () => {
    it("sends the measure and the package content on save", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      const updateProduct = vi.spyOn(services.catalog, "updateProduct");

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      const price = await screen.findByLabelText("Prodajna cena sa PDV");
      await user.clear(price);
      await user.type(price, "279");
      await user.type(
        screen.getByLabelText("Jedinica za jediničnu cenu"),
        "l",
      );
      await user.type(
        screen.getByLabelText("Sadržaj pakovanja"),
        "0,75",
      );
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      await waitFor(() => {
        expect(updateProduct).toHaveBeenCalledWith(
          1,
          expect.objectContaining({
            salePriceMinor: 27_900,
            jedinicnaCenaJedinica: "l",
            // The schema-wide milli scale: value × 1000, so 0,75 l is 750. A
            // renderer that assumed any other scale would publish a jedinična
            // cena wrong by three orders of magnitude.
            jedinicnaCenaSadrzajMilli: 750,
          }),
        );
      });
    });

    /**
     * `SaveProductRequest` is a FULL replacement of the row, so a form that
     * reads the pair but does not send it back clears it on the next ordinary
     * edit — and the published file silently loses that article's jedinična
     * cena.
     */
    it("round-trips the pair through an edit that touches neither field", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      const updateProduct = vi.spyOn(services.catalog, "updateProduct");

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      await user.type(
        await screen.findByLabelText("Jedinica za jediničnu cenu"),
        "l",
      );
      await user.type(screen.getByLabelText("Sadržaj pakovanja"), "1");
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      await waitFor(() => expect(updateProduct).toHaveBeenCalledTimes(1));

      // Reopen and save again, touching only the name.
      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      expect(await screen.findByLabelText("Jedinica za jediničnu cenu")).toHaveValue(
        "l",
      );
      expect(screen.getByLabelText("Sadržaj pakovanja")).toHaveValue("1");
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      await waitFor(() => {
        expect(updateProduct).toHaveBeenLastCalledWith(
          1,
          expect.objectContaining({
            jedinicnaCenaJedinica: "l",
            jedinicnaCenaSadrzajMilli: 1000,
          }),
        );
      });
    });

    /**
     * The same round trip at the thousand, where `toLocaleString("sr-RS")`
     * starts grouping: a 1 kg bag entered in grams reads back as „1.000“, and
     * `parseQuantityInput` takes that dot for a decimal point and saves 1000
     * milli — one gram. Nothing downstream catches it, because one gram is a
     * perfectly valid sadržaj, and `CenovnikRow::jedinicna_cena_minor` then
     * publishes a jedinična cena a thousand times too high as fact under čl. 6
     * st. 4. The read-back has to carry no group separator at all.
     */
    it("round-trips a package content of a thousand units without collapsing it", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      const updateProduct = vi.spyOn(services.catalog, "updateProduct");

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      await user.type(
        await screen.findByLabelText("Jedinica za jediničnu cenu"),
        "g",
      );
      await user.type(screen.getByLabelText("Sadržaj pakovanja"), "1000");
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      await waitFor(() =>
        expect(updateProduct).toHaveBeenCalledWith(
          1,
          expect.objectContaining({ jedinicnaCenaSadrzajMilli: 1_000_000 }),
        ),
      );

      // Reopen and save again, touching neither field.
      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      expect(await screen.findByLabelText("Sadržaj pakovanja")).toHaveValue(
        "1000",
      );
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      await waitFor(() => {
        expect(updateProduct).toHaveBeenLastCalledWith(
          1,
          expect.objectContaining({
            jedinicnaCenaJedinica: "g",
            jedinicnaCenaSadrzajMilli: 1_000_000,
          }),
        );
      });
    });

    /** The same defect one grouped value up: a 1,5 l bottle entered in ml. */
    it("round-trips a package content above the thousand without collapsing it", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      const updateProduct = vi.spyOn(services.catalog, "updateProduct");

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      await user.type(
        await screen.findByLabelText("Jedinica za jediničnu cenu"),
        "ml",
      );
      await user.type(screen.getByLabelText("Sadržaj pakovanja"), "1500");
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      await waitFor(() => expect(updateProduct).toHaveBeenCalledTimes(1));

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      expect(await screen.findByLabelText("Sadržaj pakovanja")).toHaveValue(
        "1500",
      );
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      await waitFor(() => {
        expect(updateProduct).toHaveBeenLastCalledWith(
          1,
          expect.objectContaining({ jedinicnaCenaSadrzajMilli: 1_500_000 }),
        );
      });
    });

    /**
     * A sadržaj without its measure divides by nothing, so it can state no
     * jedinična cena at all — the Rust side refuses that pairing and the form
     * says so before the round trip rather than after it.
     */
    it("refuses a package content with no measure to express it in", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      const updateProduct = vi.spyOn(services.catalog, "updateProduct");

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      await user.type(
        await screen.findByLabelText("Sadržaj pakovanja"),
        "0,75",
      );
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      expect(
        await screen.findByText(
          "Uz sadržaj pakovanja izaberite i jedinicu za jediničnu cenu.",
        ),
      ).toBeInTheDocument();
      expect(updateProduct).not.toHaveBeenCalled();
    });

    /**
     * Nothing guesses a unit price from a package size nobody entered: an
     * article priced per piece may legitimately have neither field, and the
     * published cell is then empty — a visible gap, not a fabricated figure.
     */
    it("sends both as null when the operator says nothing", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      const updateProduct = vi.spyOn(services.catalog, "updateProduct");

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      await waitFor(() => {
        expect(updateProduct).toHaveBeenCalledWith(
          1,
          expect.objectContaining({
            jedinicnaCenaJedinica: null,
            jedinicnaCenaSadrzajMilli: null,
          }),
        );
      });
    });

    /**
     * The half-configured pair: „l“ typed into the measure with the package
     * content left blank. `CenovnikRow::jedinicna_cena_minor` then publishes
     * the sale price AS the jedinična cena — 279,00 per litre for a 0,75 l
     * bottle whose true figure is 372,00 — and čl. 6 st. 4 makes the shop
     * answer for that published number. The form must say so beside the input
     * rather than let the file carry it.
     */
    it("refuses a measure that differs from the unit of measure with no package content", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      const updateProduct = vi.spyOn(services.catalog, "updateProduct");

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      // „Mleko 1 l“ is sold po komadu, so „l“ here is a different measure.
      await user.type(
        await screen.findByLabelText("Jedinica za jediničnu cenu"),
        "l",
      );
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      expect(
        await screen.findByText(
          "Jedinica za jediničnu cenu se razlikuje od jedinice mere — unesite sadržaj pakovanja. Prazan sadržaj znači da je jedna prodajna jedinica jednaka jednoj jedinici mere (na primer 1 kom = 1 l).",
        ),
      ).toBeInTheDocument();
      expect(updateProduct).not.toHaveBeenCalled();
    });

    /**
     * The v19 convention stays expressible: goods sold by the kilogram whose
     * jedinična cena is per kilogram need no package content at all, and the
     * refusal above must not reach them.
     */
    it("saves a blank package content when the two measures agree", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      const updateProduct = vi.spyOn(services.catalog, "updateProduct");

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      const unit = await screen.findByLabelText("Jedinica mere");
      await user.clear(unit);
      await user.type(unit, "kg");
      await user.type(
        screen.getByLabelText("Jedinica za jediničnu cenu"),
        "kg",
      );
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      await waitFor(() => {
        expect(updateProduct).toHaveBeenCalledWith(
          1,
          expect.objectContaining({
            unitOfMeasure: "kg",
            jedinicnaCenaJedinica: "kg",
            jedinicnaCenaSadrzajMilli: null,
          }),
        );
      });
    });

    /**
     * The convention that makes a blank sadržaj legitimate — one selling unit
     * IS one unit of the measure — lived only in the v19 comment and in the
     * tests. An operator reading „program sam deli prodajnu cenu“ beside an
     * empty field cannot tell a deliberate blank from a forgotten one.
     */
    it("states the blank-content convention in the field help", async () => {
      const user = userEvent.setup();
      const services = createMockServices();

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );

      expect(
        await screen.findByText(
          /ostavite sadržaj prazan — jedinična cena je tada jednaka prodajnoj ceni/i,
        ),
      ).toBeInTheDocument();
    });
  });

  // ZoT čl. 34 st. 5 — the Rust gate refuses every create/update from a
  // distance-selling shop that carries no proizvođač and no zemlja proizvodnje.
  // Without these fields on the sheet that shop cannot save a single article.
  describe("deklaracija (ZoT čl. 34)", () => {
    function servicesWithDistanceSelling(distanceSelling: boolean | null) {
      const services = createMockServices();
      vi.spyOn(services.settings, "getShopProfile").mockResolvedValue({
        pravnaForma: "preduzetnik",
        pdvObveznik: false,
        distanceSelling,
        lpfrInPremises: true,
        lpfrCarveOutInternetOnly: false,
        lpfrCarveOutOwnUsedAssets: false,
        esirElements: [],
      });
      return services;
    }

    it("lets a distance-selling shop save the declaration data čl. 34 st. 5 demands", async () => {
      const user = userEvent.setup();
      const services = servicesWithDistanceSelling(true);
      const updateProduct = vi.spyOn(services.catalog, "updateProduct");

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      await user.type(
        await screen.findByLabelText("Poslovno ime proizvođača"),
        "Mlekara Šabac d.o.o.",
      );
      await user.type(
        screen.getByLabelText("Zemlja proizvodnje"),
        "Srbija",
      );
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      await waitFor(() => {
        expect(updateProduct).toHaveBeenCalledWith(
          1,
          expect.objectContaining({
            manufacturerName: "Mlekara Šabac d.o.o.",
            countryOfOrigin: "Srbija",
          }),
        );
      });
    });

    it("marks the section obavezno once the shop sells at distance", async () => {
      const user = userEvent.setup();
      const services = servicesWithDistanceSelling(true);

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );

      expect(
        await screen.findByText(
          "Obavezno za prodaju na daljinu (ZoT čl. 34 st. 5).",
        ),
      ).toBeInTheDocument();
    });

    it("labels the section a recommendation for a walk-in shop", async () => {
      const user = userEvent.setup();
      const services = servicesWithDistanceSelling(false);

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );

      expect(await screen.findByText("Preporuka.")).toBeInTheDocument();
      expect(
        screen.queryByText(/obavezno za prodaju na daljinu/i),
      ).not.toBeInTheDocument();
      expect(
        screen.queryByText(/Odgovorite da li radnja prodaje na daljinu/i),
      ).not.toBeInTheDocument();
    });

    // §5 Q-8: silence is not a "no". An unanswered profile must neither claim
    // the duty nor present the shop as a settled walk-in one.
    it("asks for the answer instead of claiming a walk-in shop while it is unanswered", async () => {
      const user = userEvent.setup();
      const services = servicesWithDistanceSelling(null);

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );

      expect(
        await screen.findByText(
          "Odgovorite da li radnja prodaje na daljinu u Podešavanjima.",
        ),
      ).toBeInTheDocument();
      expect(
        screen.queryByText(/obavezno za prodaju na daljinu/i),
      ).not.toBeInTheDocument();
    });

    it("round-trips the declaration fields back into the form", async () => {
      const user = userEvent.setup();
      const services = servicesWithDistanceSelling(true);

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );
      await user.type(
        await screen.findByLabelText("Poslovno ime proizvođača"),
        "Mlekara Šabac d.o.o.",
      );
      await user.type(screen.getByLabelText("Zemlja proizvodnje"), "Srbija");
      await user.type(
        screen.getByLabelText("Poslovno ime uvoznika"),
        "Uvoznik Beograd d.o.o.",
      );
      await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

      await user.click(
        await screen.findByRole("button", { name: "Izmeni Mleko 1 l" }),
      );

      expect(
        await screen.findByLabelText("Poslovno ime proizvođača"),
      ).toHaveValue("Mlekara Šabac d.o.o.");
      expect(screen.getByLabelText("Zemlja proizvodnje")).toHaveValue("Srbija");
      expect(screen.getByLabelText("Poslovno ime uvoznika")).toHaveValue(
        "Uvoznik Beograd d.o.o.",
      );
    });

    // The bulk grid has no declaration columns, so every row it posts would be
    // refused by the čl. 34 st. 5 gate. Offering it to a distance seller is
    // offering a button that cannot succeed.
    it("withdraws bulk entry from a distance seller rather than letting every row fail", async () => {
      const user = userEvent.setup();
      const services = servicesWithDistanceSelling(true);

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);

      await user.click(await screen.findByRole("button", { name: /Novi artikal/ }));

      expect(
        await screen.findByText(
          "Bulk unos je isključen dok radnja prodaje na daljinu — deklaracija se unosi po artiklu.",
        ),
      ).toBeInTheDocument();
      expect(
        screen.queryByRole("tab", { name: "Bulk unos" }),
      ).not.toBeInTheDocument();
    });
  });

  // The read-only čl. 34 gaps register. It blocks nothing: a blank column is a
  // gap in the shop's own records, not proof that the pallet carries no
  // deklaracija.
  describe("izveštaj artikala bez podataka deklaracije", () => {
    it("lists an article with blank identity fields and its advisory beside the notice", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      vi.spyOn(services.catalog, "declarationGaps").mockResolvedValue([
        {
          productId: 1,
          sku: "MLEKO-1L",
          name: "Mleko 1 l",
          barcode: "8600000000010",
          barcodeKind: null,
          missingFields: ["manufacturerName", "countryOfOrigin"],
          barcodeUnclassified: true,
          gtinCheckDigitInvalid: false,
          reasons: ["missingIdentityData", "barcodeUnclassified"],
          advisory: "U sistemu nisu evidentirani podaci sa deklaracije.",
          // `legal::declaration_missing` verbatim, with `penalty: null`: one
          // čl. 34 state (roba bez deklaracije, čl. 68) and no figure — every
          // amount lives in src-tauri/src/legal.rs.
          notice: {
            summary:
              "Prodaja robe bez deklaracije. Deklaraciju obezbeđuje proizvođač, " +
              "odnosno uvoznik, ali za prodaju takve robe odgovara trgovac.",
            penalty: null,
            citation: "Zakon o trgovini, čl. 34 st. 1–2, čl. 68 st. 1 tač. 9.",
            isLegalDuty: true,
          },
        },
      ]);

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);
      await user.click(
        await screen.findByRole("tab", { name: "Deklaracije" }),
      );

      expect(await screen.findByText("Mleko 1 l")).toBeInTheDocument();
      expect(
        screen.getByText("Poslovno ime proizvođača, Zemlja proizvodnje"),
      ).toBeInTheDocument();
      expect(
        screen.getByText("U sistemu nisu evidentirani podaci sa deklaracije."),
      ).toBeInTheDocument();
      expect(
        screen.getByText(
          "Prodaja robe bez deklaracije. Deklaraciju obezbeđuje proizvođač, odnosno uvoznik, ali za prodaju takve robe odgovara trgovac.",
        ),
      ).toBeInTheDocument();
      expect(
        screen.getByText(
          "Unesite pravnu formu u Podešavanja → Profil za pun prikaz.",
        ),
      ).toBeInTheDocument();
    });

    // §4 item 11 / §3 req 26: no figure may sit next to a bare barcode defect —
    // the tier between „roba bez deklaracije" and „neuredna deklaracija" is
    // unresolved, so the backend sends no notice and the row must show none.
    it("attaches no penalty figure to a row whose only defect is the barcode", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      vi.spyOn(services.catalog, "declarationGaps").mockResolvedValue([
        {
          productId: 2,
          sku: "KAFA-200",
          name: "Kafa 200 g",
          barcode: "8600000000027",
          barcodeKind: "gtin",
          missingFields: [],
          barcodeUnclassified: false,
          gtinCheckDigitInvalid: true,
          reasons: ["gtinCheckDigitInvalid"],
          advisory: null,
          notice: null,
        },
      ]);

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);
      await user.click(
        await screen.findByRole("tab", { name: "Deklaracije" }),
      );

      expect(
        await screen.findByText("GTIN ima neispravnu kontrolnu cifru"),
      ).toBeInTheDocument();
      expect(screen.queryByText(/dinara/)).not.toBeInTheDocument();
      expect(screen.queryByText(/kazn/i)).not.toBeInTheDocument();
    });

    it("says so plainly when no active article has a gap", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      vi.spyOn(services.catalog, "declarationGaps").mockResolvedValue([]);

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);
      await user.click(
        await screen.findByRole("tab", { name: "Deklaracije" }),
      );

      expect(
        await screen.findByText(
          "Svi aktivni artikli imaju evidentirane podatke deklaracije.",
        ),
      ).toBeInTheDocument();
    });

    // `catalog_declaration_gaps` is admin-gated but the "Artikli" screen is not,
    // so a cashier can reach this tab. A permission refusal is not a broken
    // report and must not be dressed as one.
    it("tells a cashier the report is an administrator view instead of showing a load failure", async () => {
      const user = userEvent.setup();
      const services = createMockServices();
      await services.auth.login({ username: "marko", credential: "1234" });

      render(<CatalogModule services={services} onOpenInventory={() => {}} />);
      await user.click(
        await screen.findByRole("tab", { name: "Deklaracije" }),
      );

      expect(
        await screen.findByText("Izveštaj je dostupan samo administratoru."),
      ).toBeInTheDocument();
      expect(screen.queryByText("Izveštaj nije učitan")).not.toBeInTheDocument();
      expect(
        screen.queryByText("Samo administrator može da izvrši ovu akciju."),
      ).not.toBeInTheDocument();
    });
  });
});
