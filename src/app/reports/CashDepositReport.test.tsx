import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import type { CashDepositReport as CashDepositReportDto } from "@/services/types";

import { CashDepositReport } from "./CashDepositReport";

// Mirrors the Rust `report_footer` constants in src-tauri/src/cash_deposit.rs.
// Kept verbatim on purpose: the footer is the module's honesty surface, so a
// silent drift between the two is worth failing a test over.
const FOOTER =
  "Zbir po danu prometa je konvencija ove aplikacije, a ne zakonska kategorija: rok teče od " +
  "prijema gotovine, a ni Zakon 68/2015 ni Pravilnik 77/2011 ne poznaju dnevni izveštaj. " +
  "Iz osnovice je izuzeta samo ona gotovina podignuta sa tekućeg računa radnje za koju je " +
  "zabeleženo da je isplaćena u skladu sa Pravilnikom 77/2011 čl. 2 st. 2 (uz originalnu " +
  "dokumentaciju podnetu banci na uvid i overu) ili čl. 2 st. 3 (dnevni limit od 150.000 " +
  "dinara bez dokumentacije). Podizanja bez te potvrde ostaju u osnovici i imaju rok za polog. " +
  "Izuzeće je olakšica na nivou podzakonskog akta — sam zakon („po bilo kom osnovu“) i kazna " +
  "iz čl. 7 ne sadrže nijedan izuzetak. " +
  "Izveštaj je informativan: nadzor vrši Poreska uprava, a rok ne blokira prodaju, " +
  "zatvaranje smene ni fiskalizaciju.";

const notice: CashDepositReportDto["notice"] = {
  summary:
    "Dinare primljene u gotovom po bilo kom osnovu treba uplatiti na tekući račun u roku od sedam radnih dana.",
  penalty: "Prekršaj: novčana kazna od 10.000 do 500.000 dinara (čl. 7 st. 3).",
  citation:
    "Zakon o obavljanju plaćanja pravnih lica, preduzetnika i fizičkih lica koja ne obavljaju delatnost (Sl. glasnik RS, br. 68/2015), čl. 3 st. 1. Nadzor: Poreska uprava (čl. 6).",
  isLegalDuty: true,
};

const overdueReport: CashDepositReportDto = {
  asOf: "2026-08-12",
  buckets: [
    {
      tradingDate: "2026-07-30",
      subjectMinor: 250_000,
      depositedMinor: 100_000,
      outstandingMinor: 150_000,
      dueOn: "2026-08-08",
      isOverdue: true,
    },
    {
      tradingDate: "2026-08-11",
      subjectMinor: 80_000,
      depositedMinor: 0,
      outstandingMinor: 80_000,
      dueOn: "2026-08-20",
      isOverdue: false,
    },
    {
      tradingDate: "2026-08-12",
      subjectMinor: 40_000,
      depositedMinor: 40_000,
      outstandingMinor: 0,
      dueOn: "2026-08-21",
      isOverdue: false,
    },
  ],
  outstandingMinor: 230_000,
  overdueMinor: 150_000,
  excludedFloatMinor: 50_000,
  saturdayIsWorking: true,
  calendarHorizonYear: 2027,
  beyondSeededCalendar: false,
  notice,
  footer: FOOTER,
};

describe("CashDepositReport", () => {
  it("shows the deadline as a date and never blocks anything", async () => {
    render(<CashDepositReport report={overdueReport} />);

    expect(screen.getByText("08.08.2026.")).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: /blokiraj/i })).not.toBeInTheDocument();
  });

  it("labels the float exclusion as bylaw-level relief", () => {
    render(<CashDepositReport report={overdueReport} />);
    expect(screen.getByText(/Pravilnik 77\/2011/)).toBeInTheDocument();
  });

  it("labels a till ceiling as internal policy, not a legal duty", () => {
    render(<CashDepositReport report={overdueReport} />);
    expect(screen.queryByText(/zakonski maksimum/i)).not.toBeInTheDocument();
  });

  it("says the per-trading-date roll-up is this app's convention, not the law's", () => {
    render(<CashDepositReport report={overdueReport} />);

    expect(
      screen.getByText(/konvencija ove aplikacije, a ne zakonska kategorija/),
    ).toBeInTheDocument();
  });

  it("separates a late bucket from one still inside its rok", () => {
    render(<CashDepositReport report={overdueReport} />);

    expect(screen.getByText("Kasni")).toBeInTheDocument();
    expect(screen.getByText("U roku")).toBeInTheDocument();
    expect(screen.getByText("Položeno")).toBeInTheDocument();
  });

  it("never reports a bucket without a computed deadline as being in roku", () => {
    render(
      <CashDepositReport
        report={{
          ...overdueReport,
          buckets: [
            {
              tradingDate: "2026-07-30",
              subjectMinor: 250_000,
              depositedMinor: 0,
              outstandingMinor: 250_000,
              dueOn: null,
              isOverdue: false,
            },
          ],
        }}
      />
    );

    expect(screen.getByText("Rok nije izračunat")).toBeInTheDocument();
    expect(screen.queryByText("U roku")).not.toBeInTheDocument();
  });

  it("shows what the float carve-out kept out of the base", () => {
    render(<CashDepositReport report={overdueReport} />);

    expect(screen.getByText("500,00 RSD")).toBeInTheDocument();
  });

  it("withholds the penalty figure until the legal form is known", () => {
    render(
      <CashDepositReport
        report={{
          ...overdueReport,
          notice: { ...notice, penalty: null },
        }}
      />
    );

    expect(
      screen.getByText(/unesite pravnu formu u podešavanja → profil/i),
    ).toBeInTheDocument();
    expect(screen.queryByText(/novčana kazna/i)).not.toBeInTheDocument();
  });

  it("says a deadline past the seeded calendar was counted without that year's holidays", () => {
    render(
      <CashDepositReport
        report={{ ...overdueReport, beyondSeededCalendar: true }}
      />
    );

    expect(screen.getByText(/2027/)).toBeInTheDocument();
    expect(screen.getByText(/dopunite listu neradnih dana/i)).toBeInTheDocument();
  });

  it("stays a clean report, not a clean bill of health, when nothing is open", () => {
    render(
      <CashDepositReport
        report={{
          ...overdueReport,
          buckets: [],
          outstandingMinor: 0,
          overdueMinor: 0,
        }}
      />
    );

    expect(screen.getByText(/nema nepoloženog gotovog novca/i)).toBeInTheDocument();
  });
});
