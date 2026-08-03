import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { CountSheet, countSheetRezim } from "./CountSheet";
import type {
  ListaPregled,
  PopisLineView,
  PopisSessionView,
  PopisStatus,
} from "@/services/types";

/**
 * All six liste as `read_liste` reports them — **the empty ones included, each
 * with its own pravni osnov**. That is the shape the backend deliberately
 * returns and the shape the count sheet owes the shop: a posebna lista the
 * sheet never shows is a posebna lista the shop never fills.
 */
const SVE_LISTE: ListaPregled[] = [
  {
    vrsta: "roba",
    naziv: "roba u objektu",
    pravniOsnov: "PoP čl. 9 st. 1 t. 1",
    brojStavki: 0,
  },
  {
    vrsta: "ostecena",
    naziv: "oštećena, zastarela i neupotrebljiva roba",
    pravniOsnov: "PoP čl. 10 st. 3",
    brojStavki: 0,
  },
  {
    vrsta: "van_objekta",
    naziv: "roba van objekta (na popravci i kod trećeg lica)",
    pravniOsnov: "PoP čl. 10 st. 4",
    brojStavki: 0,
  },
  {
    vrsta: "gotovina",
    naziv: "gotovina po apoenima",
    pravniOsnov: "PoP čl. 11 st. 1",
    brojStavki: 0,
  },
  {
    vrsta: "potrazivanja",
    naziv: "nedokumentovana potraživanja i obaveze",
    pravniOsnov: "PoP čl. 12 st. 2",
    brojStavki: 0,
  },
  {
    vrsta: "konsignacija",
    naziv: "konsignaciona i druga tuđa roba",
    pravniOsnov: "PoP čl. 2 st. 5",
    brojStavki: 0,
  },
];

/** The six liste with `brojStavki` set for the ones the fixture filled. */
function liste(broj: Partial<Record<ListaPregled["vrsta"], number>>): ListaPregled[] {
  return SVE_LISTE.map((pregled) => ({
    ...pregled,
    brojStavki: broj[pregled.vrsta] ?? 0,
  }));
}

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
    liste: liste({ roba: 1 }),
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
   * The third meaning of the same column, and the one the sheet used to get
   * wrong in both directions. On the čl. 12 st. 2 lista `cenaMinor` is the
   * **iznos of a nedokumentovano potraživanje ili obaveza** — the commission's
   * own figure, and the only substantive one the lista carries, since a claim
   * has no count. Labelling it „Cena“ under „Obračunska cena (PoP čl. 9 st. 1
   * t. 5)“ told the operator it was a step-5 field, while the blind read
   * withheld it: the field took a value during `counting`, the row came back
   * „—“, the edit form reopened empty, and the čl. 8 st. 5 potpis went over an
   * amount the screen never showed back.
   */
  it("calls the price column an iznos on the čl. 12 st. 2 lista and shows it during the count", async () => {
    const user = userEvent.setup();
    render(
      <CountSheet
        session={popis("counting", {
          linije: [
            linija({
              id: 2,
              listaVrsta: "potrazivanja",
              sifra: null,
              naziv: "Potraživanje bez isprave",
              jedinicaMere: null,
              stvarnaKolicinaMilli: 0,
              cenaMinor: 350000,
            }),
          ],
          liste: liste({ potrazivanja: 1 }),
        })}
        onSaveLine={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    // The counted amount is on the lista while the lista is being written.
    expect(screen.getByText("3.500,00 RSD")).toBeInTheDocument();
    expect(screen.getByText("Iznos")).toBeInTheDocument();

    // And it round-trips into the edit form instead of reopening empty.
    await user.click(screen.getByRole("button", { name: /izmeni/i }));
    expect(screen.getByLabelText(/^iznos$/i)).toHaveValue("3500.00");
    // The field says which figure it is, and it is not the step-5 cena.
    expect(
      screen.getByText(/deo prebrojanog stanja \(PoP čl\. 12 st\. 2\)/),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/Obračunska cena \(PoP čl\. 9 st\. 1 t\. 5\)/),
    ).not.toBeInTheDocument();
  });

  /**
   * The write side of the same rule. What the count showed, the čl. 8 st. 5
   * potpis froze — the backend refuses a moved iznos by name — so the obračun
   * must not offer an input for it, exactly as it does not for the apoen. An
   * enabled field whose save the next call refuses is the affordance this
   * module refuses to render anywhere else.
   */
  it("withholds the iznos input in the obračun, as it does the apoen", async () => {
    const user = userEvent.setup();
    render(
      <CountSheet
        session={popis("computed", {
          linije: [
            linija({
              id: 2,
              listaVrsta: "potrazivanja",
              naziv: "Potraživanje bez isprave",
              stvarnaKolicinaMilli: 0,
              cenaMinor: 350000,
            }),
          ],
          liste: liste({ potrazivanja: 1 }),
        })}
        onSaveLine={vi.fn().mockResolvedValue(undefined)}
      />,
    );

    await user.click(screen.getByRole("button", { name: /izmeni/i }));

    expect(screen.getByLabelText(/^iznos$/i)).toBeDisabled();
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

  /**
   * A negative knjigovodstvena količina is reachable and it must not lock the
   * stavka out of its obračun. `inventory_balances.quantity_milli` carries no
   * non-negative CHECK, the app supports `allow_negative_stock` and
   * `populate_book_quantities` copies the balance verbatim — so the field comes
   * back holding „-3“, and a parser that accepts no minus refuses the save,
   * names a field the operator never touched, and leaves the čl. 9 st. 1 t. 5
   * cena unenterable for that stavka forever. The counted side needs none of
   * this: `stvarna_kolicina_milli >= 0` is a v20 CHECK.
   */
  it("round-trips a negative book quantity instead of refusing the obračun", async () => {
    const user = userEvent.setup();
    const onSaveLine = vi.fn().mockResolvedValue(undefined);
    render(
      <CountSheet
        session={popis("computed", {
          linije: [linija({ knjigovodstvenaKolicinaMilli: -3000 })],
        })}
        onSaveLine={onSaveLine}
      />,
    );

    await user.click(screen.getByRole("button", { name: /izmeni/i }));

    expect(screen.getByLabelText(/knjigovodstvena količina/i)).toHaveValue("-3");

    await user.click(screen.getByRole("button", { name: /sačuvaj stavku/i }));

    await waitFor(() => expect(onSaveLine).toHaveBeenCalledTimes(1));
    expect(onSaveLine.mock.calls[0][1]).toMatchObject({
      knjigovodstvenaKolicinaMilli: -3000,
      cenaMinor: 249900,
    });
    expect(
      screen.queryByText(/knjigovodstvena količina nije ispravna/i),
    ).not.toBeInTheDocument();
  });

  /** A minus is not a licence to accept anything: the digits still have to parse. */
  it("still refuses a book quantity that is not a number", async () => {
    const user = userEvent.setup();
    const onSaveLine = vi.fn().mockResolvedValue(undefined);
    render(
      <CountSheet
        session={popis("computed", {
          linije: [linija({ knjigovodstvenaKolicinaMilli: 9000 })],
        })}
        onSaveLine={onSaveLine}
      />,
    );

    await user.click(screen.getByRole("button", { name: /izmeni/i }));
    await user.clear(screen.getByLabelText(/knjigovodstvena količina/i));
    await user.type(screen.getByLabelText(/knjigovodstvena količina/i), "-");
    await user.click(screen.getByRole("button", { name: /sačuvaj stavku/i }));

    expect(
      await screen.findByText(/knjigovodstvena količina nije ispravna/i),
    ).toBeInTheDocument();
    expect(onSaveLine).not.toHaveBeenCalled();
  });
});

describe("CountSheet — the six liste of req. 36", () => {
  /**
   * `read_liste` returns all six deliberately — the empty ones included, each
   * with its naziv and its pravni osnov — and a sheet that showed only the
   * groups already carrying rows would leave the five posebne liste invisible
   * until somebody guessed they existed. Čl. 10 st. 3, čl. 10 st. 4, čl. 11
   * st. 1, čl. 12 st. 2 and čl. 2 st. 5 each require one where the category is
   * present, and „present“ is a fact about the shop that no ledger holds.
   */
  it("renders all six liste with their article, the empty ones visibly empty", () => {
    render(<CountSheet session={popis("counting")} />);

    for (const pregled of SVE_LISTE) {
      expect(screen.getAllByText(pregled.naziv).length).toBeGreaterThan(0);
      expect(
        screen.getAllByText(pregled.pravniOsnov).length,
      ).toBeGreaterThan(0);
    }

    // Five of the six carry nothing yet, and each says so where it stands.
    expect(screen.getAllByText(/nema nijedne stavke/i)).toHaveLength(5);
  });

  /**
   * Req. 36's declaration belongs to the count phase and not to the izveštaj:
   * after the čl. 8 st. 5 potpis `counted_signed` refuses every line write and
   * `computed` refuses a new stavka by name, so a shop first told „prijavljena
   * kategorija je prazna“ at izveštaj time has no remedy left but a whole new
   * popis. Asked here, the remedy is still one stavka away.
   */
  it("asks which categories exist while the liste still admit a stavka", async () => {
    const user = userEvent.setup();
    const onPrijava = vi.fn();
    render(
      <CountSheet
        session={popis("counting")}
        onSaveLine={vi.fn()}
        prijavljene={[]}
        onPrijava={onPrijava}
      />,
    );

    await user.click(
      screen.getByRole("checkbox", { name: /gotovina po apoenima/i }),
    );

    expect(onPrijava).toHaveBeenCalledWith("gotovina");
  });

  /**
   * The refusal is the backend's, printed verbatim: `provera_listi` calls the
   * very function the izveštaj generator refuses with, so the shop reads one
   * sentence and not two.
   */
  it("prints the backend's readiness refusal without rewording it", () => {
    const poruka =
      "Popisne liste nisu potpune: prijavljeno je da postoji „gotovina po " +
      "apoenima“ (PoP čl. 11 st. 1) — a te liste su prazne.";

    render(
      <CountSheet
        session={popis("counting")}
        onSaveLine={vi.fn()}
        prijavljene={["gotovina"]}
        onPrijava={vi.fn()}
        provera={{
          spremno: false,
          nedostaju: [SVE_LISTE[3]],
          poruka,
        }}
      />,
    );

    expect(screen.getByText(poruka)).toBeInTheDocument();
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
