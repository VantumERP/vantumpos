import { render, screen, waitFor, within } from "@testing-library/react";
import type { UserEvent } from "@testing-library/user-event";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { AppShell } from "./app/AppShell";
import { createMockServices } from "./services/mock-adapter";
import type { PosServices } from "./services/ports";
import type { AppSession } from "./services/types";

const readyHealth = {
  backend: "local" as const,
  appVersion: "test",
  databasePath: "mock://vantumpos.sqlite3",
  migrated: true,
};

const readyCompany = {
  shopName: "Vantum Market",
  address: "Bulevar 1, Beograd",
  pib: "",
  registrationNumber: "87654321",
  phone: "+381 11 123 456",
  logoPath: null,
  currency: "RSD" as const,
};

function buildAuthServices(overrides: Record<string, unknown> = {}) {
  return {
    settings: {
      getHealth: vi.fn().mockResolvedValue(readyHealth),
      getCompanySettings: vi.fn().mockResolvedValue(readyCompany),
      listTaxRates: vi
        .fn()
        .mockResolvedValue([
          { id: 1, name: "PDV 20%", rateBasisPoints: 2000, active: true },
        ]),
      seedTaxRates: vi.fn().mockResolvedValue([]),
    },
    auth: {
      getSession: vi.fn().mockResolvedValue(null),
      login: vi.fn(),
      logout: vi.fn().mockResolvedValue(undefined),
    },
    users: {
      listUsers: vi.fn().mockResolvedValue([]),
      createUser: vi.fn(),
      updateUser: vi.fn(),
      deactivateUser: vi.fn(),
    },
    shifts: {
      getCurrentShift: vi.fn().mockResolvedValue(null),
      openShift: vi.fn(),
      closeShift: vi.fn(),
    },
    ...overrides,
  } as unknown as PosServices;
}

const cashierSession: AppSession = {
  user: {
    id: 2,
    username: "marko",
    displayName: "Marko Markovic",
    role: "cashier",
    active: true,
    createdAt: "2026-06-18T08:00:00Z",
    updatedAt: "2026-06-18T08:00:00Z",
    lastLoginAt: "2026-06-18T09:00:00Z",
  },
  currentShift: null,
};

const adminSession: AppSession = {
  user: {
    id: 1,
    username: "admin",
    displayName: "Administrator",
    role: "admin",
    active: true,
    createdAt: "2026-06-18T08:00:00Z",
    updatedAt: "2026-06-18T08:00:00Z",
    lastLoginAt: "2026-06-18T09:00:00Z",
  },
  currentShift: {
    id: 10,
    userId: 1,
    cashierName: "Administrator",
    openedAt: "2026-06-18T09:00:00Z",
    closedAt: null,
    openingCashMinor: 500000,
    expectedCashMinor: 500000,
    countedCashMinor: null,
    cashSalesMinor: 0,
    cardSalesMinor: 0,
    paidInMinor: 0,
    paidOutMinor: 0,
    differenceMinor: null,
    status: "open",
    openingNote: "Jutarnja smena",
    closingNote: null,
  },
};

describe("AppShell", () => {
  it("starts on the login screen when there is no session", async () => {
    render(<AppShell services={buildAuthServices()} />);

    expect(await screen.findByRole("heading", { name: "Prijava u kasu" }))
      .toBeInTheDocument();
    expect(screen.getByLabelText("Korisničko ime")).toBeInTheDocument();
    expect(screen.getByLabelText("PIN ili lozinka")).toBeInTheDocument();
    expect(screen.queryByText("Admin")).not.toBeInTheDocument();
  });

  it("routes a cashier without an open shift to the open shift screen", async () => {
    const login = vi.fn().mockResolvedValue(cashierSession);
    const services = buildAuthServices({
      auth: {
        getSession: vi.fn().mockResolvedValue(null),
        login,
        logout: vi.fn().mockResolvedValue(undefined),
      },
    });
    const user = userEvent.setup();

    render(<AppShell services={services} />);

    await user.type(await screen.findByLabelText("Korisničko ime"), "marko");
    await user.type(screen.getByLabelText("PIN ili lozinka"), "1234");
    await user.click(screen.getByRole("button", { name: "Prijavi se" }));

    expect(login).toHaveBeenCalledWith({
      username: "marko",
      credential: "1234",
    });
    expect(await screen.findByRole("heading", { name: "Otvori smenu" }))
      .toBeInTheDocument();
  });

  it("shows real signed-in user and shift state in the shell", async () => {
    const services = buildAuthServices({
      auth: {
        getSession: vi.fn().mockResolvedValue(adminSession),
        login: vi.fn(),
        logout: vi.fn().mockResolvedValue(undefined),
      },
    });

    render(<AppShell services={services} />);

    expect(await screen.findByText("Administrator")).toBeInTheDocument();
    expect(screen.getByText("Admin")).toBeInTheDocument();
    expect(screen.getByText("Smena otvorena")).toBeInTheDocument();
    expect(screen.queryByText("Smena nije otvorena")).not.toBeInTheDocument();
  });

  it("hides the Podesavanja nav item from cashiers", async () => {
    const services = buildAuthServices({
      auth: {
        getSession: vi.fn().mockResolvedValue(cashierSession),
        login: vi.fn(),
        logout: vi.fn().mockResolvedValue(undefined),
      },
    });

    render(<AppShell services={services} />);

    expect(await screen.findByRole("button", { name: "Kasa" })).toBeInTheDocument();
    expect(screen.getByText("@marko")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Podešavanja" }),
    ).not.toBeInTheDocument();
  });

  it("shows the Podesavanja nav item for admins", async () => {
    const services = buildAuthServices({
      auth: {
        getSession: vi.fn().mockResolvedValue(adminSession),
        login: vi.fn(),
        logout: vi.fn().mockResolvedValue(undefined),
      },
    });

    render(<AppShell services={services} />);

    expect(
      await screen.findByRole("button", { name: "Podešavanja" }),
    ).toBeInTheDocument();
  });

  it("shows Serbian login errors without leaking technical messages", async () => {
    const services = buildAuthServices({
      auth: {
        getSession: vi.fn().mockResolvedValue(null),
        login: vi.fn().mockRejectedValue({
          code: "invalid_credentials",
          message: "Korisničko ime ili lozinka nisu ispravni.",
        }),
        logout: vi.fn().mockResolvedValue(undefined),
      },
    });
    const user = userEvent.setup();

    render(<AppShell services={services} />);

    await user.type(await screen.findByLabelText("Korisničko ime"), "marko");
    await user.type(screen.getByLabelText("PIN ili lozinka"), "pogresno");
    await user.click(screen.getByRole("button", { name: "Prijavi se" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Korisničko ime ili lozinka nisu ispravni.",
    );
    expect(screen.queryByText(/Error:/i)).not.toBeInTheDocument();
  });

  it("rejects invalid opening cash before calling the shift service", async () => {
    const openShift = vi.fn();
    const services = buildAuthServices({
      auth: {
        getSession: vi.fn().mockResolvedValue(cashierSession),
        login: vi.fn(),
        logout: vi.fn().mockResolvedValue(undefined),
      },
      shifts: {
        getCurrentShift: vi.fn().mockResolvedValue(null),
        openShift,
        closeShift: vi.fn(),
      },
    });
    const user = userEvent.setup();

    render(<AppShell services={services} />);

    await user.type(await screen.findByLabelText("Početni novac"), "abc");
    await user.click(screen.getByRole("button", { name: "Otvori smenu" }));

    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Iznos nije ispravan.",
    );
    expect(openShift).not.toHaveBeenCalled();
  });

  it("renders the POS navigation and backend status", async () => {
    render(<AppShell services={createMockServices()} />);

    expect(await screen.findByRole("heading", { name: "Kasa" }))
      .toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Kasa" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Artikli" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Lager" })).toBeInTheDocument();
    expect(await screen.findByText("Lokalna baza spremna")).toBeInTheDocument();
    expect(screen.getByRole("status")).toHaveTextContent(
      "Lokalna baza spremna",
    );
  });

  it("shows the company shop name as the shell brand", async () => {
    render(<AppShell services={createMockServices()} />);

    expect(await screen.findByText("Vantum Market")).toBeInTheDocument();
    expect(screen.queryByText("VantumPOS")).not.toBeInTheDocument();
  });

  it("keeps default branding when company settings cannot be read", async () => {
    const getCompanySettings = vi.fn().mockRejectedValue({
      code: "database_error",
      message: "Baza nije dostupna.",
    });
    const services = buildAuthServices({
      auth: {
        getSession: vi.fn().mockResolvedValue(adminSession),
        login: vi.fn(),
        logout: vi.fn().mockResolvedValue(undefined),
      },
      settings: {
        getHealth: vi.fn().mockResolvedValue(readyHealth),
        getCompanySettings,
        listTaxRates: vi
          .fn()
          .mockResolvedValue([
            { id: 1, name: "PDV 20%", rateBasisPoints: 2000, active: true },
          ]),
        seedTaxRates: vi.fn().mockResolvedValue([]),
      },
    });

    render(<AppShell services={services} />);

    expect(await screen.findByText("Administrator")).toBeInTheDocument();
    await waitFor(() => expect(getCompanySettings).toHaveBeenCalled());
    expect(screen.getByText("VantumPOS")).toBeInTheDocument();
  });

  it("renders a stable fallback when backend health fails", async () => {
    const services = createMockServices();
    services.settings.getHealth = () => Promise.reject(new Error("boom"));

    render(<AppShell services={services} />);

    expect(await screen.findByText("Backend nije dostupan.")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Backend nije dostupan." }),
    ).not.toBeInTheDocument();
    expect(screen.queryByText("boom")).not.toBeInTheDocument();
  });

  it("keeps visible POS shell copy operator-facing", async () => {
    render(<AppShell services={createMockServices()} />);

    expect(await screen.findByText("Lokalna baza spremna")).toBeInTheDocument();

    for (const internalTerm of [
      /SQLite backend/i,
      /Adapter arhitektura/i,
      /domain service/i,
      /MVP/i,
      /service adapter/i,
    ]) {
      expect(screen.queryByText(internalTerm)).not.toBeInTheDocument();
    }
  });

  it("records a polog on the shift screen with its broj uplatnice", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const shiftCashMovement = vi.spyOn(services.shifts, "shiftCashMovement");
    render(<AppShell services={services} />);

    await screen.findByRole("heading", { name: "Kasa" });
    await user.selectOptions(
      screen.getByLabelText("Vrsta transakcije"),
      "bank_deposit",
    );
    await user.type(screen.getByLabelText("Iznos"), "1500");
    await user.type(
      screen.getByLabelText("Broj izvoda / uplatnice"),
      "uplatnica-7",
    );
    await user.click(screen.getByRole("button", { name: "Polog na račun" }));

    await waitFor(() =>
      expect(shiftCashMovement).toHaveBeenCalledWith({
        direction: "bank_deposit",
        amountMinor: 150000,
        reason: null,
        bankReference: "uplatnica-7",
      }),
    );
  });

  it("offers a podizanje sa računa and never calls it a pazar", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await screen.findByRole("heading", { name: "Kasa" });
    await user.selectOptions(
      screen.getByLabelText("Vrsta transakcije"),
      "bank_withdrawal",
    );

    expect(
      screen.getByRole("button", { name: "Podizanje sa računa" }),
    ).toBeInTheDocument();
    // The bylaw carve-out is conditional, so the form must not promise the
    // money is out of the deposit base — it says what to check instead.
    expect(screen.getByText(/Pravilnik\w* 77\/2011/)).toBeInTheDocument();
  });

  it("keeps the broj uplatnice field out of a plain cash movement", async () => {
    render(<AppShell services={createMockServices()} />);

    await screen.findByRole("heading", { name: "Kasa" });

    expect(
      screen.queryByLabelText("Broj izvoda / uplatnice"),
    ).not.toBeInTheDocument();
  });

  it("opens Podesavanja with company, VAT, receipt, users, and backup tabs", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Podešavanja" }));

    expect(await screen.findByRole("tab", { name: "Radnja" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "PDV" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Računi" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Korisnici" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Backup" })).toBeInTheDocument();
    expect(await screen.findByLabelText("Naziv radnje")).toBeInTheDocument();

    await user.click(await screen.findByRole("tab", { name: "PDV" }));
    expect(
      await screen.findByRole("heading", { name: "PDV stope" }, { timeout: 5000 }),
    ).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "PDV 20" })).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Računi" }));
    expect(screen.getByLabelText("Prefiks računa")).toBeInTheDocument();

    await user.click(await screen.findByRole("tab", { name: "Backup" }));
    expect(await screen.findByRole("heading", { name: "Status backupa" }))
      .toBeInTheDocument();
  });

  it("saves company settings and shows a success toast", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Podešavanja" }));
    await user.clear(await screen.findByLabelText("Naziv radnje"));
    await user.type(screen.getByLabelText("Naziv radnje"), "Vantum Market");
    await user.type(screen.getByLabelText("PIB"), "123456789");
    await user.click(screen.getByRole("button", { name: "Sačuvaj radnju" }));

    expect(await screen.findByText("Podešavanja radnje su sačuvana."))
      .toBeInTheDocument();
  });

  it("renders VAT validation errors without closing the dialog", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Podešavanja" }));
    await user.click(screen.getByRole("tab", { name: "PDV" }));
    await user.click(screen.getByRole("button", { name: "Nova PDV stopa" }));
    await user.click(screen.getByRole("button", { name: "Sačuvaj PDV stopu" }));

    expect(await screen.findByText("Naziv PDV stope je obavezan."))
      .toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "PDV stopa" }))
      .toBeInTheDocument();
  });

  it("renders stale backup status and guarded restore confirmation", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Podešavanja" }));
    await user.click(screen.getByRole("tab", { name: "Backup" }));

    expect(await screen.findByText("Backup nije napravljen")).toBeInTheDocument();

    await user.type(screen.getByLabelText("Putanja backup fajla"), "D:/backup.sqlite3");
    await user.click(screen.getByRole("button", { name: "Vrati backup" }));

    expect(screen.getByRole("heading", { name: "Potvrdite restore" }))
      .toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Potvrdi restore" }))
      .toBeDisabled();

    await user.type(screen.getByLabelText("Potvrda"), "VRATI PODATKE");
    await user.click(screen.getByRole("button", { name: "Potvrdi restore" }));

    expect(await screen.findByText("Restore je završen.")).toBeInTheDocument();
  });

  it("opens the Artikli module with product filters and product rows", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));

    expect(
      screen.getByRole("heading", { name: "Artikli" }),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole("searchbox", { name: "Pretraga artikala" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Novi artikal" }),
    ).toBeInTheDocument();
    expect(
      screen.getByRole("columnheader", { name: "SKU/šifra" }),
    ).toBeInTheDocument();
    expect(await screen.findByRole("cell", { name: "MLEKO-1L" }))
      .toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "159,99 RSD" }))
      .toBeInTheDocument();
  });

  it("validates required product form fields before saving", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    await user.click(screen.getByRole("button", { name: "Novi artikal" }));
    await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

    expect(screen.getByText("Naziv je obavezan.")).toBeInTheDocument();
    expect(screen.getByText("SKU/šifra je obavezna.")).toBeInTheDocument();
  });

  it("opens product entry on the barcode scanner field first", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    await user.click(screen.getByRole("button", { name: "Novi artikal" }));

    const scannerField = await screen.findByLabelText("Barcode / SKU");

    await waitFor(() => expect(scannerField).toHaveFocus());
    expect(scannerField).toHaveValue("");
    expect(screen.getByLabelText("Naziv")).toBeInTheDocument();
  });

  it("shows duplicate backend errors next to the matching product field", async () => {
    const user = userEvent.setup();
    const duplicateServices = createMockServices();
    duplicateServices.catalog.createProduct = () =>
      Promise.reject({
        code: "duplicate_sku",
        message: "SKU/šifra već postoji.",
      });

    render(<AppShell services={duplicateServices} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    await user.click(screen.getByRole("button", { name: "Novi artikal" }));
    await user.type(screen.getByLabelText("Naziv"), "Mleko 1 l");
    await user.type(screen.getByLabelText("SKU/šifra"), "MLEKO-1L");
    await user.type(screen.getByLabelText("Prodajna cena sa PDV"), "159,99");
    await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

    const skuField = screen
      .getByLabelText("SKU/šifra")
      .closest('[data-slot="field"]');

    expect(skuField).not.toBeNull();
    expect(
      within(skuField as HTMLElement).getByText("SKU/šifra već postoji."),
    ).toBeInTheDocument();
  });

  it("applies an online barcode lookup suggestion and keeps its source visible", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const createProduct = vi.spyOn(services.catalog, "createProduct");

    render(<AppShell services={services} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    await user.click(screen.getByRole("button", { name: "Novi artikal" }));
    await user.type(screen.getByLabelText("Barcode / SKU"), "4008400320328");
    await user.click(
      screen.getByRole("button", { name: "Pronađi podatke po barcode-u" }),
    );

    expect(await screen.findByText("Predlog sa Open Food Facts"))
      .toBeInTheDocument();
    expect(screen.getByText("Kinder Bueno")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Primeni javne podatke" }));

    expect(screen.getByLabelText("Naziv")).toHaveValue("Kinder Bueno 43 g");
    expect(screen.getByLabelText("Jedinica mere")).toHaveValue("kom");
    expect(screen.getByText("Open Food Facts")).toBeInTheDocument();

    await user.type(screen.getByLabelText("SKU/šifra"), "KINDER-BUENO");
    await user.type(screen.getByLabelText("Prodajna cena sa PDV"), "129,99");
    await user.click(screen.getByRole("button", { name: "Sačuvaj artikal" }));

    const row = await screen.findByRole("row", { name: /Kinder Bueno/i });
    expect(within(row).getByText("Open Food Facts")).toBeInTheDocument();
    expect(createProduct.mock.calls[0][0]).toMatchObject({
      name: "Kinder Bueno 43 g",
      unitOfMeasure: "kom",
      externalSource: {
        acceptedFields: ["name", "brand", "packageSize"],
      },
    });
  });

  it("supports scanner enter lookup and keyboard save in quick product entry", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    await user.click(screen.getByRole("button", { name: "Novi artikal" }));
    await user.type(await screen.findByLabelText("Barcode / SKU"), "4008400320328");
    await user.keyboard("{Enter}");

    expect(await screen.findByText("Predlog sa Open Food Facts"))
      .toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Primeni javne podatke" }));
    await user.type(screen.getByLabelText("SKU/šifra"), "KINDER-FAST");
    await user.type(screen.getByLabelText("Prodajna cena sa PDV"), "129,99");
    await user.keyboard("{Control>}{Enter}{/Control}");

    expect(await screen.findByRole("row", { name: /Kinder Bueno/i }))
      .toBeInTheDocument();
  });

  it("saves multiple products from barcode-first bulk entry", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const createProduct = vi.spyOn(services.catalog, "createProduct");

    render(<AppShell services={services} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    await user.click(screen.getByRole("button", { name: "Novi artikal" }));
    await user.click(await screen.findByRole("tab", { name: "Bulk unos" }));

    await user.type(screen.getByLabelText("Barcode / SKU red 1"), "4008400320328");
    await user.click(
      screen.getByRole("button", { name: "Pronađi javne podatke za red 1" }),
    );

    expect(await screen.findByLabelText("Naziv red 1")).toHaveValue(
      "Kinder Bueno 43 g",
    );
    expect(screen.getByText("Open Food Facts")).toBeInTheDocument();

    await user.type(screen.getByLabelText("SKU/šifra red 1"), "KINDER-BULK");
    await user.type(screen.getByLabelText("Cena red 1"), "129,99");
    await user.type(screen.getByLabelText("Barcode / SKU red 2"), "8600000000011");
    await user.type(screen.getByLabelText("Naziv red 2"), "Hleb beli");
    await user.type(screen.getByLabelText("SKU/šifra red 2"), "HLEB-BELI");
    await user.type(screen.getByLabelText("Cena red 2"), "89,99");
    await user.keyboard("{Control>}{Enter}{/Control}");

    expect(await screen.findByRole("row", { name: /Kinder Bueno/i }))
      .toBeInTheDocument();
    expect(await screen.findByRole("row", { name: /Hleb beli/i }))
      .toBeInTheDocument();
    expect(createProduct).toHaveBeenCalledTimes(2);
    expect(createProduct.mock.calls[0][0]).toMatchObject({
      barcode: "4008400320328",
      name: "Kinder Bueno 43 g",
      sku: "KINDER-BULK",
      externalSource: {
        acceptedFields: ["name", "brand", "packageSize"],
      },
    });
    expect(createProduct.mock.calls[1][0]).toMatchObject({
      barcode: "8600000000011",
      name: "Hleb beli",
      sku: "HLEB-BELI",
    });
  });

  it("opens and focuses the next bulk row after scanning the last row barcode", async () => {
    const user = userEvent.setup();

    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    await user.click(screen.getByRole("button", { name: "Novi artikal" }));
    await user.click(await screen.findByRole("tab", { name: "Bulk unos" }));

    await user.type(screen.getByLabelText("Barcode / SKU red 1"), "4008400320328");
    await user.keyboard("{Enter}");

    const nextBarcodeInput = await screen.findByLabelText("Barcode / SKU red 2");

    await waitFor(() => expect(nextBarcodeInput).toHaveFocus());
    expect(await screen.findByLabelText("Naziv red 1")).toHaveValue(
      "Kinder Bueno 43 g",
    );
  });

  it("deactivates an active product from the row action", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    await user.click(
      await screen.findByRole("button", { name: "Deaktiviraj Mleko 1 l" }),
    );

    const row = screen.getByRole("row", { name: /Mleko 1 l/i });

    expect(within(row).getByText("Neaktivan")).toBeInTheDocument();
    expect(
      within(row).getByRole("button", { name: "Aktiviraj Mleko 1 l" }),
    ).toBeInTheDocument();
  });

  it("opens the Lager module with stock list, receive flow, and ledger", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Lager" }));

    expect(
      await screen.findByRole("heading", { name: "Stanje lagera" }),
    ).toBeInTheDocument();
    expect(screen.queryByText("Radni modul")).not.toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "Mleko 1 l" })).toBeInTheDocument();
    expect(screen.getByText("Nizak lager")).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Prijem robe za Mleko 1 l" }),
    );
    await user.clear(screen.getByLabelText("Količina"));
    await user.type(screen.getByLabelText("Količina"), "2");
    await user.type(screen.getByLabelText("Razlog"), "Prijem robe");
    await user.click(screen.getByRole("button", { name: "Sačuvaj prijem" }));

    // ZoT čl. 34: „Mleko 1 l" carries no proizvođač or zemlja proizvodnje in
    // the demo catalogue, so the committed receipt raises the advisory warning
    // before the dialog closes. The movement is already in the ledger.
    expect(
      await screen.findByText("Prijem je upisan — nedostaju podaci deklaracije"),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Zatvori" }));

    expect(await screen.findByRole("cell", { name: "5 kom" })).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Ledger za Mleko 1 l" }),
    );
    expect(
      await screen.findByRole("heading", { name: "Kartica artikla" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Prijem robe")).toBeInTheDocument();
  });

  it("shows Serbian inventory errors from the service layer", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    services.inventory.writeOffStock = () =>
      Promise.reject({
        code: "insufficient_stock",
        message: "Nema dovoljno zaliha.",
      });

    render(<AppShell services={services} />);

    await user.click(await screen.findByRole("button", { name: "Lager" }));
    await user.click(
      await screen.findByRole("button", { name: "Otpis za Mleko 1 l" }),
    );
    await user.clear(screen.getByLabelText("Količina"));
    await user.type(screen.getByLabelText("Količina"), "99");
    await user.type(screen.getByLabelText("Razlog"), "Lom");
    await user.click(screen.getByRole("button", { name: "Sačuvaj otpis" }));

    expect(await screen.findByText("Nema dovoljno zaliha.")).toBeInTheDocument();
  });

  it("opens Racuni with receipt search filters and detail data", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Računi" }));

    expect(
      await screen.findByRole("heading", { name: "Pretraga računa" }),
    ).toBeInTheDocument();
    expect(screen.getByLabelText("Od datuma")).toBeInTheDocument();
    expect(screen.getByLabelText("Do datuma")).toBeInTheDocument();
    expect(screen.getByLabelText("Broj računa")).toBeInTheDocument();
    expect(screen.getByLabelText("Kasir")).toBeInTheDocument();
    expect(screen.getByLabelText("Smena")).toBeInTheDocument();
    expect(screen.getByLabelText("Način plaćanja")).toBeInTheDocument();
    expect(screen.getByLabelText("Artikal")).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "R-2026-0001" })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Detalji za R-2026-0001" }));

    expect(
      await screen.findByRole("heading", { name: "Račun R-2026-0001" }),
    ).toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "Kafa 200 g" })).toBeInTheDocument();
    expect(screen.getAllByText("Gotovina").length).toBeGreaterThan(0);
    expect(screen.getAllByText("Nije fiskalizovan").length).toBeGreaterThan(0);
  });

  it("requires a reason before full void", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Računi" }));
    await user.click(await screen.findByRole("button", { name: "Detalji za R-2026-0001" }));
    await user.click(screen.getByRole("button", { name: "Storniraj račun" }));
    await user.click(screen.getByRole("button", { name: "Potvrdi storniranje" }));

    expect(await screen.findByText("Unesite razlog storniranja.")).toBeInTheDocument();
  });

  it("applies a partial return and shows the linked return document", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Računi" }));
    await user.click(await screen.findByRole("button", { name: "Detalji za R-2026-0001" }));
    await user.click(screen.getByRole("button", { name: "Povrat artikala" }));
    await user.clear(screen.getByLabelText("Količina za Kafa 200 g"));
    await user.type(screen.getByLabelText("Količina za Kafa 200 g"), "1");
    await user.type(screen.getByLabelText("Razlog povrata"), "Kupac vratio jedan komad");
    await user.click(screen.getByRole("button", { name: "Sačuvaj povrat" }));

    expect(await screen.findByText("POV-R-2026-0001-1")).toBeInTheDocument();
    expect(screen.getByText("Vraćao 1 kom")).toBeInTheDocument();
  });

  it("validates imported product rows before enabling commit", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await openImportModule(user);

    expect(
      screen.getByRole("heading", { name: "Import podataka" }),
    ).toBeInTheDocument();
    expect(screen.getByText("prethodni-products.csv")).toBeInTheDocument();

    const file = new File(
      ["Naziv;Cena;PDV;Sifra\nHleb;neispravno;20;SKU-1\n"],
      "artikli.csv",
      { type: "text/csv" },
    );

    await user.upload(screen.getByLabelText("CSV fajl"), file);

    expect(await screen.findByText("Mapiranje kolona")).toBeInTheDocument();
    expect(screen.getByText("Naziv -> Naziv")).toBeInTheDocument();
    expect(screen.getByText("Prodajna cena -> Cena")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Validiraj CSV" }));

    expect(await screen.findByText("Red 2")).toBeInTheDocument();
    expect(screen.getByText("Cena nije ispravna.")).toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: "Upiši import" }),
    ).toBeDisabled();
  });

  it("commits a valid product CSV after dry run confirmation", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await openImportModule(user);

    const file = new File(
      ["Naziv;Cena;PDV;Sifra;Barcode;Zaliha\nHleb;120,00;20;SKU-1;8600001;3\n"],
      "artikli.csv",
      { type: "text/csv" },
    );

    await user.upload(screen.getByLabelText("CSV fajl"), file);
    await user.click(await screen.findByRole("button", { name: "Validiraj CSV" }));

    expect(await screen.findByText("Dry run: 1 za kreiranje")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Upiši import" }));
    await user.click(screen.getByRole("button", { name: "Potvrdi upis" }));

    expect(await screen.findByText("Import završen")).toBeInTheDocument();
    expect(screen.getAllByText("artikli.csv")[0]).toBeInTheDocument();
  });

  it("opens the import job detail drill-down from history", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await openImportModule(user);

    await user.click(
      await screen.findByRole("button", {
        name: "Detalji importa prethodni-products.csv",
      }),
    );

    const dialog = await screen.findByRole("dialog", {
      name: "Detalji importa: prethodni-products.csv",
    });
    expect(within(dialog).getByText("Red 2")).toBeInTheDocument();
    expect(within(dialog).getByText("Upisano")).toBeInTheDocument();
  });

  it("deep-links the catalog Lager action to the product ledger", async () => {
    const user = userEvent.setup();
    render(<AppShell services={createMockServices()} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    expect(
      await screen.findByRole("cell", { name: "MLEKO-1L" }),
    ).toBeInTheDocument();

    await user.click(
      screen.getByRole("button", { name: "Lager za Mleko 1 l" }),
    );

    // The ledger Sheet auto-opens as a modal, so the inventory screen behind it
    // is inert; assert its heading with { hidden: true } to confirm navigation.
    expect(
      await screen.findByRole("heading", {
        name: "Stanje lagera",
        hidden: true,
      }),
    ).toBeInTheDocument();

    const ledger = await screen.findByRole("dialog", {
      name: "Kartica artikla",
    });
    expect(within(ledger).getByText("Mleko 1 l")).toBeInTheDocument();
    expect(within(ledger).getByText(/MLEKO-1L/)).toBeInTheDocument();
  });

  it("surfaces an error when the deep-linked ledger fails to load", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    services.inventory.getProductLedger = () =>
      Promise.reject({ code: "not_found", message: "Artikal nije pronađen." });

    render(<AppShell services={services} />);

    await user.click(await screen.findByRole("button", { name: "Artikli" }));
    await user.click(
      await screen.findByRole("button", { name: "Lager za Mleko 1 l" }),
    );

    // The ledger Sheet auto-opens as a modal, so the inventory screen behind it
    // is inert; assert its heading with { hidden: true } to confirm navigation.
    expect(
      await screen.findByRole("heading", {
        name: "Stanje lagera",
        hidden: true,
      }),
    ).toBeInTheDocument();
    expect(
      await screen.findByText("Artikal nije pronađen."),
    ).toBeInTheDocument();
  });
});

async function openImportModule(user: UserEvent) {
  await waitFor(() => {
    expect(
      screen.queryByRole("button", { name: "Import" }) ??
        screen.queryByLabelText("Korisničko ime"),
    ).toBeTruthy();
  });

  const username = screen.queryByLabelText("Korisničko ime");

  if (username) {
    await user.type(username, "admin");
    await user.type(screen.getByLabelText("PIN ili lozinka"), "1234");
    await user.click(screen.getByRole("button", { name: "Prijavi se" }));
  }

  await user.click(await screen.findByRole("button", { name: "Import" }));
}
