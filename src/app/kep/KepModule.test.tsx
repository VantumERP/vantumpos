import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { KepModule } from "./KepModule";
import { navigationItems } from "@/app/navigation";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type { KepEntryView, KepLedger, KepStatus } from "@/services/types";

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
