import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { CountSheet, countSheetRezim } from "./CountSheet";
import type {
  PopisLineView,
  PopisSessionView,
  PopisStatus,
} from "@/services/types";

/**
 * A counted stavka. `knjigovodstvenaKolicinaMilli` and `razlikaMilli` are what
 * the real backend withholds during Phase A — the fixtures below hand them to
 * the component ANYWAY in the blind states, because that is the only way to
 * tell a sheet that decides from `knjigovodstvoDostupno` from one that decides
 * from „does this row happen to carry a number“.
 */
function linija(overrides: Partial<PopisLineView> = {}): PopisLineView {
  return {
    id: 1,
    listaVrsta: "roba",
    sifra: "KOS-1",
    naziv: "Košulja bela",
    vrsta: "roba",
    jedinicaMere: "kom",
    stvarnaKolicinaMilli: 7000,
    bliziOpis: null,
    knjigovodstvenaKolicinaMilli: null,
    razlikaMilli: null,
    cenaMinor: 249900,
    ...overrides,
  };
}

function popis(
  status: PopisStatus,
  overrides: Partial<PopisSessionView> = {},
): PopisSessionView {
  const fazaAPotpisana =
    status !== "draft" && status !== "counting";

  return {
    id: 1,
    vrsta: "godisnji",
    prodajnoMesto: "Butik Centar",
    datumPopisa: "2026-12-31",
    periodFrom: null,
    periodTo: null,
    status,
    planRadaJson: null,
    odlukaRef: null,
    perpetualOdlukaRef: null,
    uskladjivanjePotvrdjenoAt: "2026-12-30T09:00:00Z",
    postedAt: status === "posted" ? "2027-01-05T09:00:00Z" : null,
    fazaAPotpisana,
    fazaBPotpisana: status === "computed_signed" || status === "posted",
    knjigovodstvoDostupno: fazaAPotpisana,
    komisija: [],
    potpisi: [],
    linije: [linija()],
    liste: [],
    konsignacijaRok: null,
    upozorenja: [],
    ...overrides,
  };
}

describe("countSheetRezim — what the liste admit, state by state", () => {
  /**
   * The three closed states are closed for three different reasons and the
   * reason is what the operator can act on: after the čl. 8 st. 5 potpis the
   * obračun is the next step, after the čl. 9 st. 3 one it is the knjiženje,
   * and after the knjiženje there is no next step at all. One „zaključano“ for
   * all three would send a shop looking for a way back in that does not exist.
   */
  it("names a different reason for each closed state", () => {
    expect(countSheetRezim(popis("draft")).kind).toBe("otvoren");
    expect(countSheetRezim(popis("counting")).kind).toBe("otvoren");
    expect(countSheetRezim(popis("computed")).kind).toBe("obracun");

    const zatvoreni = (["counted_signed", "computed_signed", "posted"] as const)
      .map((status) => countSheetRezim(popis(status)))
      .map((rezim) => {
        expect(rezim.kind).toBe("zatvoren");
        return rezim.kind === "zatvoren" ? rezim.razlog : "";
      });

    expect(new Set(zatvoreni).size).toBe(3);
    expect(zatvoreni[0]).toMatch(/čl\. 8 st\. 5/);
    expect(zatvoreni[1]).toMatch(/čl\. 9 st\. 3/);
    expect(zatvoreni[2]).toMatch(/čl\. 14 st\. 3/);
  });
});

describe("CountSheet — the PoP čl. 8 st. 5 blind count (req. 29)", () => {
  it("renders no book-quantity and no difference column during counting", () => {
    render(<CountSheet session={popis("counting")} />);

    expect(screen.queryByText(/knjigovodstven/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/razlika/i)).not.toBeInTheDocument();
  });

  it("renders neither of them in draft either — the count has not begun", () => {
    render(<CountSheet session={popis("draft")} />);

    expect(screen.queryByText(/knjigovodstven/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/razlika/i)).not.toBeInTheDocument();
  });

  /**
   * The mutation this exists for: a sheet that showed the column „when a row
   * has a book figure“ rather than „when čl. 8 st. 5 has released it“. The
   * backend never sends such a row — that is the whole of req. 29's query-layer
   * enforcement — but the sheet must not be the place where the property is
   * only accidentally true.
   */
  it("shows no book figure during counting even if the payload carries one", () => {
    render(
      <CountSheet
        session={popis("counting", {
          linije: [
            linija({ knjigovodstvenaKolicinaMilli: 9000, razlikaMilli: -2000 }),
          ],
        })}
      />,
    );

    expect(screen.queryByText(/knjigovodstven/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/razlika/i)).not.toBeInTheDocument();
    // 9 kom is the book stanje and 2 kom the manjak; the counted 7 kom stays.
    expect(screen.queryByText("9 kom")).not.toBeInTheDocument();
    expect(screen.queryByText("-2 kom")).not.toBeInTheDocument();
    expect(screen.getByText("7 kom")).toBeInTheDocument();
  });

  it("offers no book-quantity input while the count is blind", () => {
    render(<CountSheet session={popis("counting")} onSaveLine={vi.fn()} />);

    expect(
      screen.queryByLabelText(/knjigovodstvena količina/i),
    ).not.toBeInTheDocument();
    // The counted state is still writable — this is Phase A, not a lock.
    expect(screen.getByLabelText(/stvarna količina/i)).toBeEnabled();
  });

  it("reveals them only after the phase A signature", () => {
    render(
      <CountSheet
        session={popis("computed", {
          linije: [
            linija({ knjigovodstvenaKolicinaMilli: 9000, razlikaMilli: -2000 }),
          ],
        })}
      />,
    );

    expect(screen.getByText(/knjigovodstven/i)).toBeInTheDocument();
    expect(screen.getByText(/razlika/i)).toBeInTheDocument();
    expect(screen.getByText("9 kom")).toBeInTheDocument();
    expect(screen.getByText("-2 kom")).toBeInTheDocument();
  });

  /**
   * The two-limb predicate, not the status limb. `status` is a claim any UPDATE
   * can make and a session can be born in `counted_signed` with no potpis
   * behind it — which is why the status limb is module-private backend-side.
   */
  it("stays blind in counted_signed when no phase A signature stands behind it", () => {
    render(
      <CountSheet
        session={popis("counted_signed", {
          fazaAPotpisana: false,
          knjigovodstvoDostupno: false,
          linije: [linija({ knjigovodstvenaKolicinaMilli: 9000 })],
        })}
      />,
    );

    expect(screen.queryByText(/knjigovodstven/i)).not.toBeInTheDocument();
    expect(screen.queryByText("9 kom")).not.toBeInTheDocument();
  });
});

describe("CountSheet — the write rules the backend actually has", () => {
  it("appends a stavka with no book quantity during counting", async () => {
    const user = userEvent.setup();
    const onSaveLine = vi.fn().mockResolvedValue(undefined);
    render(
      <CountSheet
        session={popis("counting", { linije: [] })}
        onSaveLine={onSaveLine}
      />,
    );

    await user.type(screen.getByLabelText(/^naziv/i), "Suknja crna");
    await user.type(screen.getByLabelText(/^šifra/i), "SUK-2");
    await user.clear(screen.getByLabelText(/stvarna količina/i));
    await user.type(screen.getByLabelText(/stvarna količina/i), "3");
    await user.click(screen.getByRole("button", { name: /sačuvaj stavku/i }));

    await waitFor(() => expect(onSaveLine).toHaveBeenCalledTimes(1));
    expect(onSaveLine).toHaveBeenCalledWith(null, {
      listaVrsta: "roba",
      sifra: "SUK-2",
      naziv: "Suknja crna",
      vrsta: null,
      jedinicaMere: null,
      stvarnaKolicinaMilli: 3000,
      bliziOpis: null,
      knjigovodstvenaKolicinaMilli: null,
      cenaMinor: null,
    });
  });

  /**
   * Čl. 11 st. 1: on the gotovina lista the „cena“ is the apoen, part of what
   * the commission counted — not the čl. 9 st. 1 t. 5 obračunska cena. The two
   * are the same column and only the wording tells the shop which it is filling
   * in.
   */
  it("calls the price column an apoen on the gotovina lista and a cena elsewhere", () => {
    render(
      <CountSheet
        session={popis("counting", {
          linije: [
            linija(),
            linija({
              id: 2,
              listaVrsta: "gotovina",
              sifra: null,
              naziv: "Novčanica 1.000",
              jedinicaMere: "kom",
              stvarnaKolicinaMilli: 5000,
              cenaMinor: 100000,
            }),
          ],
        })}
      />,
    );

    expect(screen.getByText("Apoen")).toBeInTheDocument();
    expect(screen.getByText("Cena")).toBeInTheDocument();
  });

  /**
   * Req. 30. Between the čl. 8 st. 5 potpis and the obračun the liste are shut:
   * an „Izmeni“ button here would be an affordance the very next call refuses.
   */
  it("offers no edit affordance while the counted state is signed and unread", () => {
    render(
      <CountSheet session={popis("counted_signed")} onSaveLine={vi.fn()} />,
    );

    expect(
      screen.queryByRole("button", { name: /sačuvaj stavku/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /izmeni/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByText(/stvarno stanje je potpisano/i),
    ).toBeInTheDocument();
  });

  /**
   * In `computed` the obračun may still be filled in — the čl. 11 st. 1 and
   * čl. 12 st. 2 liste have no perpetual record behind them — but only over
   * stavke the commission actually counted. A „nova stavka“ form here would
   * offer a line nobody counted.
   */
  it("edits an existing stavka in the obračun but adds no new one", async () => {
    const user = userEvent.setup();
    render(
      <CountSheet
        session={popis("computed", {
          linije: [linija({ knjigovodstvenaKolicinaMilli: 9000 })],
        })}
        onSaveLine={vi.fn()}
      />,
    );

    // No form at all until a counted stavka is picked: an empty „nova stavka“
    // form here would offer a line nobody counted, and the backend refuses it.
    expect(
      screen.queryByRole("button", { name: /sačuvaj stavku/i }),
    ).not.toBeInTheDocument();
    expect(screen.getByText(/nova stavka se ne dodaje/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /izmeni/i }));

    expect(
      screen.getByRole("button", { name: /sačuvaj stavku/i }),
    ).toBeInTheDocument();

    expect(screen.getByLabelText(/knjigovodstvena količina/i)).toBeEnabled();
    // Čl. 8 st. 4 and čl. 9 st. 1 t. 1 — what the commission signed does not move.
    expect(screen.getByLabelText(/stvarna količina/i)).toBeDisabled();
    expect(screen.getByLabelText(/^naziv/i)).toBeDisabled();
  });
});

describe("CountSheet — the req. 41 posting lock", () => {
  it("offers no edit affordance on a posted popis", () => {
    render(<CountSheet session={popis("posted")} onSaveLine={vi.fn()} />);

    expect(
      screen.queryByRole("button", { name: /sačuvaj stavku/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /izmeni/i }),
    ).not.toBeInTheDocument();
    expect(screen.queryByRole("textbox")).not.toBeInTheDocument();
  });

  /**
   * ZoRač čl. 8 st. 4 — a correction is a new document. A sheet that merely
   * greyed the buttons out would leave the operator hunting for the way back
   * in; this says there is none and what to do instead.
   */
  it("says a correction goes through a new popis", () => {
    render(<CountSheet session={popis("posted")} onSaveLine={vi.fn()} />);

    expect(screen.getByText(/novim popisom/i)).toBeInTheDocument();
  });
});
