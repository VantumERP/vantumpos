import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { KepModule } from "./KepModule";
import { navigationItems } from "@/app/navigation";
import { Toaster } from "@/components/ui/sonner";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type {
  KalkulacijaSummary,
  KepEntryView,
  KepLedger,
  KepStatus,
  ProductSummary,
} from "@/services/types";

// Memo §2 worked ledger: a receipt zaduženje 7.800,00 (50 × 156,00 retail incl.
// PDV — NOT the nabavna) and a daily razduženje 2.340,00, deriving saldo
// 5.460,00. The datum column is dan.mesec only (kolona 2 booking date).
const receiptEntry: KepEntryView = {
  redniBroj: 1,
  datum: "04.07",
  opis: "Prijem robe",
  zaduzenjeMinor: 780000,
  razduzenjeMinor: null,
  kind: "receipt",
};

const dailyEntry: KepEntryView = {
  redniBroj: 2,
  datum: "05.07",
  opis: "Dnevni promet 2026-07-05",
  zaduzenjeMinor: null,
  razduzenjeMinor: 234000,
  kind: "daily_sales",
};

const ledger: KepLedger = {
  bookYear: 2026,
  entries: [receiptEntry, dailyEntry],
  openingSaldoMinor: 0,
  saldoMinor: 546000,
};

const emptyStatus: KepStatus = {
  overdueSalesDays: [],
  unbookedReceiptCount: 0,
};

function servicesWith(
  ledgerValue: KepLedger,
  statusValue: KepStatus = emptyStatus,
): PosServices {
  const services = createMockServices();
  vi.spyOn(services.kep, "ledger").mockResolvedValue(ledgerValue);
  vi.spyOn(services.kep, "status").mockResolvedValue(statusValue);
  return services;
}

describe("navigation", () => {
  it("exposes KEP as an admin-only item after reklamacije", () => {
    const item = navigationItems.find((candidate) => candidate.id === "kep");

    expect(item).toMatchObject({ label: "KEP", adminOnly: true });

    const reklamacijeIndex = navigationItems.findIndex(
      (c) => c.id === "reklamacije",
    );
    const kepIndex = navigationItems.findIndex((c) => c.id === "kep");
    expect(kepIndex).toBe(reklamacijeIndex + 1);
  });
});

describe("KepModule ledger", () => {
  it("renders the five Serbian column headers", async () => {
    render(<KepModule services={servicesWith(ledger)} />);

    for (const header of [
      "Red. br.",
      "Datum",
      "Opis",
      "Zaduženje",
      "Razduženje",
    ]) {
      expect(await screen.findByText(header)).toBeInTheDocument();
    }
  });

  it("renders the ledger rows and the derived saldo", async () => {
    render(<KepModule services={servicesWith(ledger)} />);

    // Receipt row: zaduženje at retail-with-PDV, razduženje empty.
    expect(await screen.findByText("Prijem robe")).toBeInTheDocument();
    expect(screen.getByText("04.07")).toBeInTheDocument();
    expect(screen.getByText("7.800,00 RSD")).toBeInTheDocument();

    // Daily row: razduženje posted from the day's sales.
    expect(screen.getByText("Dnevni promet 2026-07-05")).toBeInTheDocument();
    expect(screen.getByText("2.340,00 RSD")).toBeInTheDocument();

    // The derived running saldo (Σzaduženje − Σrazduženje).
    expect(screen.getByText("5.460,00 RSD")).toBeInTheDocument();
  });
});

describe("KepModule daily posting", () => {
  it("posts the selected day with no override amount", async () => {
    const user = userEvent.setup();
    const services = servicesWith(ledger);
    const postSpy = vi
      .spyOn(services.kep, "postDailySales")
      .mockResolvedValue(dailyEntry);

    render(<KepModule services={services} />);

    // Wait for the initial ledger load before interacting.
    await screen.findByText("Prijem robe");

    fireEvent.change(screen.getByLabelText("Datum prometa"), {
      target: { value: "2026-07-05" },
    });

    await user.click(
      screen.getByRole("button", { name: "Proknjiži dnevni promet" }),
    );

    await waitFor(() =>
      expect(postSpy).toHaveBeenCalledWith("2026-07-05T00:00:00Z", null),
    );
  });

  it("passes a manual override amount in minor units", async () => {
    const user = userEvent.setup();
    const services = servicesWith(ledger);
    const postSpy = vi
      .spyOn(services.kep, "postDailySales")
      .mockResolvedValue(dailyEntry);

    render(<KepModule services={services} />);

    await screen.findByText("Prijem robe");

    fireEvent.change(screen.getByLabelText("Datum prometa"), {
      target: { value: "2026-07-05" },
    });
    await user.type(screen.getByLabelText("Ručni iznos"), "2500");

    await user.click(
      screen.getByRole("button", { name: "Proknjiži dnevni promet" }),
    );

    await waitFor(() =>
      expect(postSpy).toHaveBeenCalledWith("2026-07-05T00:00:00Z", 250000),
    );
  });
});

// Memo §3/§4.5 worked example product: retail sa PDV 156,00/jm (salePriceMinor
// 15600), on-hand 35 kom (35_000 milli), 20% PDV. Otpis of 35 kom books
// −5.460,00 in kolona 4; a nivelacija 156→176 books +700,00.
const testProduct: ProductSummary = {
  id: 7,
  name: "Test artikal",
  sku: "SKU-7",
  barcode: null,
  categoryId: null,
  categoryName: null,
  unitOfMeasure: "kom",
  salePriceMinor: 15600,
  purchasePriceMinor: 10000,
  taxRateId: 1,
  taxRateBasisPoints: 2000,
  minimumStockMilli: 0,
  currentStockMilli: 35000,
  allowNegativeStock: false,
  active: true,
  perishable: false,
  externalSource: null,
};

function spyProductSearch(services: PosServices) {
  return vi.spyOn(services.catalog, "searchProducts").mockResolvedValue({
    items: [testProduct],
    categories: [],
    taxRates: [],
    total: 1,
  });
}

describe("KepModule adjustments", () => {
  it("shows the derived kolona and crveni storno read-only for otpis, then posts via postAdjustment", async () => {
    const user = userEvent.setup();
    const services = servicesWith(ledger);
    spyProductSearch(services);
    const postSpy = vi
      .spyOn(services.kep, "postAdjustment")
      .mockResolvedValue(undefined);

    render(
      <>
        <KepModule services={services} />
        <Toaster />
      </>,
    );

    await screen.findByText("Prijem robe");

    fireEvent.change(screen.getByLabelText("Vrsta izmene"), {
      target: { value: "otpis" },
    });

    // The cause fixes the kolona and sign; the shop sees why it books there.
    const explanation = screen.getByText(/Knjiži se u kolonu 4/);
    expect(explanation).toHaveTextContent("kolonu 4");
    expect(explanation).toHaveTextContent("crveni storno");

    await user.click(
      await screen.findByRole("button", { name: "Izaberi Test artikal" }),
    );

    fireEvent.change(screen.getByLabelText("Količina"), {
      target: { value: "35" },
    });
    fireEvent.change(screen.getByLabelText("Naziv dokumenta"), {
      target: { value: "Odluka o otpisu" },
    });
    fireEvent.change(screen.getByLabelText("Broj dokumenta"), {
      target: { value: "12" },
    });
    fireEvent.change(screen.getByLabelText("Datum dokumenta"), {
      target: { value: "2026-07-20" },
    });

    await user.click(screen.getByRole("button", { name: "Proknjiži izmenu" }));

    await waitFor(() =>
      expect(postSpy).toHaveBeenCalledWith("otpis", 7, 35000, {
        naziv: "Odluka o otpisu",
        broj: "12",
        datum: "2026-07-20",
      }),
    );
  });

  it("shows the new-price field for a nivelacija and posts via nivelacija", async () => {
    const user = userEvent.setup();
    const services = servicesWith(ledger);
    spyProductSearch(services);
    const nivSpy = vi
      .spyOn(services.kep, "nivelacija")
      .mockResolvedValue(undefined);
    const postSpy = vi
      .spyOn(services.kep, "postAdjustment")
      .mockResolvedValue(undefined);

    render(
      <>
        <KepModule services={services} />
        <Toaster />
      </>,
    );

    await screen.findByText("Prijem robe");

    fireEvent.change(screen.getByLabelText("Vrsta izmene"), {
      target: { value: "nivelacija_up" },
    });

    await user.click(
      await screen.findByRole("button", { name: "Izaberi Test artikal" }),
    );

    // Nivelacija changes the price — it takes a new price, not a quantity.
    expect(screen.queryByLabelText("Količina")).not.toBeInTheDocument();

    fireEvent.change(screen.getByLabelText("Nova prodajna cena"), {
      target: { value: "176" },
    });
    fireEvent.change(screen.getByLabelText("Naziv dokumenta"), {
      target: { value: "Nivelacioni zapisnik" },
    });
    fireEvent.change(screen.getByLabelText("Broj dokumenta"), {
      target: { value: "3" },
    });
    fireEvent.change(screen.getByLabelText("Datum dokumenta"), {
      target: { value: "2026-07-20" },
    });

    await user.click(screen.getByRole("button", { name: "Proknjiži izmenu" }));

    await waitFor(() =>
      expect(nivSpy).toHaveBeenCalledWith(7, 17600, {
        naziv: "Nivelacioni zapisnik",
        broj: "3",
        datum: "2026-07-20",
      }),
    );
    expect(postSpy).not.toHaveBeenCalled();
  });

  it("corrects a ledger entry via the Ispravi stavku dialog", async () => {
    const user = userEvent.setup();
    const services = servicesWith(ledger);
    const correctSpy = vi
      .spyOn(services.kep, "correctEntry")
      .mockResolvedValue(undefined);

    render(
      <>
        <KepModule services={services} />
        <Toaster />
      </>,
    );

    await screen.findByText("Prijem robe");

    await user.click(
      screen.getAllByRole("button", { name: /Ispravi stavku/ })[0],
    );

    fireEvent.change(await screen.findByLabelText("Ispravan iznos"), {
      target: { value: "7800" },
    });
    fireEvent.change(screen.getByLabelText("Naziv dokumenta"), {
      target: { value: "Ispravka knjiženja" },
    });
    fireEvent.change(screen.getByLabelText("Broj dokumenta"), {
      target: { value: "1" },
    });
    fireEvent.change(screen.getByLabelText("Datum dokumenta"), {
      target: { value: "2026-07-20" },
    });

    await user.click(screen.getByRole("button", { name: "Sačuvaj ispravku" }));

    await waitFor(() =>
      expect(correctSpy).toHaveBeenCalledWith(1, 2026, 780000, {
        naziv: "Ispravka knjiženja",
        broj: "1",
        datum: "2026-07-20",
      }),
    );
  });
});

describe("KepModule kalkulacije", () => {
  const kalkulacija: KalkulacijaSummary = {
    id: 5,
    redniBroj: 1,
    bookYear: 2026,
    trgovackiNaziv: "Test artikal",
    kolicinaMilli: 50000,
    razlikaUCeniMinor: 150000,
    prodajnaVrednostSaPdvMinor: 780000,
    createdAt: "2026-07-04T10:00:00Z",
  };

  it("prints a kalkulacija by exporting then opening it", async () => {
    const user = userEvent.setup();
    const services = servicesWith(ledger);
    vi.spyOn(services.kep, "listKalkulacije").mockResolvedValue([kalkulacija]);
    const exportSpy = vi
      .spyOn(services.kep, "exportKalkulacija")
      .mockResolvedValue({
        fileName: "kalkulacija-1.html",
        path: "C:/exports/kalkulacija-1.html",
        mimeType: "text/html",
        rowCount: 1,
      });
    const openForPrint = vi
      .spyOn(services.print, "openForPrint")
      .mockResolvedValue(undefined);

    render(
      <>
        <KepModule services={services} />
        <Toaster />
      </>,
    );

    await user.click(
      await screen.findByRole("button", { name: /Štampaj kalkulaciju/ }),
    );

    expect(exportSpy).toHaveBeenCalledWith(5);
    await waitFor(() =>
      expect(openForPrint).toHaveBeenCalledWith(
        "C:/exports/kalkulacija-1.html",
      ),
    );
  });
});

describe("KepModule year-end close", () => {
  it("shows the opening carry-in row (Početno stanje) when the year carries a balance", async () => {
    // 2026 closed at 15.460,00 → 2027 opens with that carry-in.
    const carried: KepLedger = { ...ledger, openingSaldoMinor: 1546000 };

    render(<KepModule services={servicesWith(carried)} />);

    expect(await screen.findByText(/Početno stanje/i)).toBeInTheDocument();
    // The carry-in shows as the leading donos balance.
    expect(screen.getByText("15.460,00 RSD")).toBeInTheDocument();
  });

  it("closes the year through the typed confirmation dialog", async () => {
    const user = userEvent.setup();
    const services = servicesWith(ledger);

    render(
      <>
        <KepModule services={services} />
        <Toaster />
      </>,
    );

    await screen.findByText("Prijem robe");

    await user.click(screen.getByRole("button", { name: /Zaključi godinu/i }));

    const input = await screen.findByLabelText(/Potvrda/i);
    const confirm = screen.getByRole("button", {
      name: /Potvrdi zaključenje/i,
    });

    // The confirm button stays disabled until the exact phrase is typed.
    expect(confirm).toBeDisabled();
    await user.type(input, "ZAKLJUČI KNJIGU");
    expect(confirm).toBeEnabled();

    await user.click(confirm);

    // After closing, the closed-year badge appears and postings are hidden.
    expect(await screen.findByText("Zaključena")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Proknjiži dnevni promet" }),
    ).toBeNull();
  });

  it("disables posting actions for a closed year", async () => {
    const services = servicesWith(ledger);
    vi.spyOn(services.kep, "closePreview").mockResolvedValue({
      krajnjiSaldoMinor: 546000,
      entryCount: 2,
      alreadyClosed: true,
    });
    vi.spyOn(services.kep, "listClosures").mockResolvedValue([
      {
        bookYear: 2026,
        krajnjiSaldoMinor: 546000,
        entryCount: 2,
        closedAt: "2027-01-05T09:00:00Z",
        purgeEligible: false,
        retentionUntil: "2032-01-05",
      },
    ]);

    render(<KepModule services={services} />);

    // The badge confirms the closed state has loaded before we assert absence.
    expect(await screen.findByText("Zaključena")).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: "Proknjiži dnevni promet" }),
    ).toBeNull();
    expect(
      screen.queryByRole("button", { name: "Zaključi godinu" }),
    ).toBeNull();
  });
});

describe("KepModule status", () => {
  it("surfaces overdue sales days in the warning", async () => {
    const services = servicesWith(ledger, {
      overdueSalesDays: ["2026-07-05"],
      unbookedReceiptCount: 0,
    });

    render(<KepModule services={services} />);

    const warning = await screen.findByText(
      /Nije proknjižen dnevni promet za:/,
    );
    expect(warning).toHaveTextContent("05.07.2026.");
  });

  it("warns about unbooked goods receipts when the defensive count is non-zero", async () => {
    const services = servicesWith(ledger, {
      overdueSalesDays: [],
      unbookedReceiptCount: 3,
    });

    render(<KepModule services={services} />);

    expect(
      await screen.findByText(/Neproknjižen prijem robe/),
    ).toHaveTextContent("3");
  });
});
