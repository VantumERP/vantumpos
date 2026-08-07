import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { IzvestajPanel } from "./IzvestajPanel";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type {
  IzvestajView,
  IzvestajZbir,
  PopisSessionView,
  PopisStatus,
} from "@/services/types";

function session(
  status: PopisStatus,
  overrides: Partial<PopisSessionView> = {},
): PopisSessionView {
  const potpisana = status !== "draft" && status !== "counting";

  return {
    id: 1,
    vrsta: "godisnji",
    prodajnoMesto: "Butik Centar",
    datumPopisa: "2026-12-31",
    periodFrom: null,
    periodTo: null,
    status,
    planRadaJson: null,
    planRadaOdobrio: null,
    planRadaOdobrenoAt: null,
    odlukaRef: null,
    odlukaDonetaAt: null,
    perpetualOdlukaRef: null,
    uskladjivanjePotvrdjenoAt: "2026-12-30T09:00:00Z",
    postedAt: null,
    fazaAPotpisana: potpisana,
    fazaBPotpisana: false,
    knjigovodstvoDostupno: potpisana,
    komisija: [],
    potpisi: [],
    linije: [],
    liste: [],
    konsignacijaRok: null,
    upozorenja: [],
    ...overrides,
  };
}

const prazanZbir: IzvestajZbir = {
  brojStavki: 0,
  stavkeBezCene: 0,
  stavkeBezKnjigovodstvenogStanja: 0,
  stavkeSaViskom: 0,
  stavkeSaManjkom: 0,
  vrednostPoPopisuMinor: 0,
  vrednostPoKnjigamaMinor: 0,
  vrednosnaRazlikaMinor: 0,
  potpuno: true,
};

const izvestaj: IzvestajView = {
  sessionId: 1,
  vrsta: "godisnji",
  status: "computed",
  obveznik: "Butik Primer pr Novi Pazar",
  pib: "100000001",
  maticniBroj: "60000001",
  prodajnoMesto: "Butik Centar",
  datumPopisa: "2026-12-31",
  periodFrom: null,
  periodTo: null,
  komisija: [],
  potpisi: [],
  elementi: [
    {
      element: "stvarno_stanje",
      naziv: "stvarno stanje utvrđeno popisom",
      pravniOsnov: "PoP čl. 13 st. 1",
      uputstvo:
        "Iz popisnih listi. Izveštaj iskazuje vrednost prebrojanog stanja i broj stavki.",
      tekst: null,
    },
    {
      element: "knjigovodstveno_stanje",
      naziv: "knjigovodstveno stanje",
      pravniOsnov: "PoP čl. 13 st. 1",
      uputstvo: "Iz knjiga, tek posle potpisa stvarnog stanja.",
      tekst: null,
    },
    {
      element: "razlike",
      naziv: "razlike između stvarnog i knjigovodstvenog stanja",
      pravniOsnov: "PoP čl. 13 st. 1",
      uputstvo: "Iz obračuna.",
      tekst: null,
    },
    {
      element: "uzroci_neslaganja",
      naziv: "uzroci neslaganja stvarnog i knjigovodstvenog stanja",
      pravniOsnov: "PoP čl. 13 st. 1",
      uputstvo: "Navedite zbog čega se stanja razlikuju.",
      tekst: "Kalo i lom.",
    },
    {
      element: "predlozi_za_likvidaciju_razlika",
      naziv: "predlozi za likvidaciju utvrđenih razlika",
      pravniOsnov: "PoP čl. 13 st. 1",
      uputstvo: "Obuhvata prebijanje manjkova i viškova.",
      tekst: "Manjak na teret radnje.",
    },
    {
      element: "nacin_knjizenja",
      naziv: "način knjiženja razlika",
      pravniOsnov: "PoP čl. 13 st. 1",
      uputstvo: "Navedite kako se razlike knjiže.",
      tekst: "Kroz KEP i glavnu knjigu.",
    },
    {
      element: "primedbe_lica_koja_rukuju_vrednostima",
      naziv: "primedbe i objašnjenja lica koja rukuju vrednostima",
      pravniOsnov: "PoP čl. 13 st. 1",
      uputstvo: "Unesite primedbe lica koja rukuju vrednostima.",
      tekst: "Nema primedbi.",
    },
    {
      element: "ostale_primedbe_i_predlozi",
      naziv: "ostale primedbe i predlozi",
      pravniOsnov: "PoP čl. 13 st. 1",
      uputstvo: "Ostale primedbe i predlozi.",
      tekst: "Nema.",
    },
  ],
  liste: [
    {
      vrsta: "roba",
      naziv: "roba u objektu",
      pravniOsnov: "PoP čl. 9 st. 1 t. 1",
      zbir: {
        ...prazanZbir,
        brojStavki: 2,
        stavkeSaManjkom: 1,
        vrednostPoPopisuMinor: 1_749_300,
        vrednostPoKnjigamaMinor: 1_999_200,
        vrednosnaRazlikaMinor: -249_900,
      },
    },
  ],
  ukupno: {
    ...prazanZbir,
    brojStavki: 2,
    stavkeSaManjkom: 1,
    vrednostPoPopisuMinor: 1_749_300,
    vrednostPoKnjigamaMinor: 1_999_200,
    vrednosnaRazlikaMinor: -249_900,
  },
  rok: "30.01.2027",
  rokPravniOsnov: "PoP čl. 13 st. 2",
  odlukaOUsvajanju: {
    rok: "30.01.2027",
    pravniOsnov: "PoP čl. 14 st. 2",
    donosilac: "preduzetnik lično",
    napomena:
      "Aplikacija ne evidentira odluku o usvajanju izveštaja — donesite je i čuvajte uz izveštaj.",
  },
  upozorenja: [
    "Izveštaj o popisu se ne čuva u aplikaciji, a program ga ne štampa i ne izvozi — štampani primerak sastavite sami i čuvajte ga uz popisne liste.",
  ],
};

function servicesWith(): PosServices {
  const services = createMockServices();
  vi.spyOn(services.popis, "getPodesavanja").mockResolvedValue({
    rokPredajeFi: "2027-03-31",
  });
  return services;
}

async function popuniNarativ(user: ReturnType<typeof userEvent.setup>) {
  await user.type(screen.getByLabelText(/uzroci neslaganja/i), "Kalo i lom.");
  await user.type(
    screen.getByLabelText(/predlozi za likvidaciju/i),
    "Manjak na teret radnje.",
  );
  await user.type(
    screen.getByLabelText(/način knjiženja/i),
    "Kroz KEP i glavnu knjigu.",
  );
  await user.type(
    screen.getByLabelText(/primedbe i objašnjenja lica/i),
    "Nema primedbi.",
  );
  await user.type(screen.getByLabelText(/ostale primedbe/i), "Nema.");
}

describe("IzvestajPanel — the čl. 8 st. 5 gate", () => {
  /**
   * The izveštaj carries the knjigovodstveno stanje and the razlike, so
   * composing one during the count hands the commission through a *document*
   * exactly what the blind count keeps back. The backend refuses it; a button
   * here would be an affordance the very next call rejects.
   */
  it("offers no composition while the count is blind and says why", async () => {
    const services = servicesWith();
    const spy = vi.spyOn(services.popis, "izvestaj");

    render(
      <IzvestajPanel
        services={services}
        session={session("counting")}
        prijavljene={[]}
      />,
    );

    expect(
      screen.queryByRole("button", { name: /sastavi izveštaj/i }),
    ).not.toBeInTheDocument();
    expect(await screen.findByText(/čl\. 8 st\. 5/i)).toBeInTheDocument();
    expect(spy).not.toHaveBeenCalled();
  });
});

describe("IzvestajPanel — the eight čl. 13 st. 1 elements (req. 37)", () => {
  it("renders all eight elements of the composed izveštaj with their article", async () => {
    const user = userEvent.setup();
    const services = servicesWith();
    vi.spyOn(services.popis, "izvestaj").mockResolvedValue(izvestaj);

    render(
      <IzvestajPanel
        services={services}
        session={session("computed")}
        prijavljene={[]}
      />,
    );

    await popuniNarativ(user);
    await user.click(screen.getByRole("button", { name: /sastavi izveštaj/i }));

    for (const element of izvestaj.elementi) {
      expect(await screen.findByText(element.naziv)).toBeInTheDocument();
    }
    expect(screen.getAllByText("PoP čl. 13 st. 1")).toHaveLength(8);
  });

  /**
   * Req. 37 wants a structured template with required fields, and the gate that
   * makes „required“ mean anything is the backend's. The panel neither
   * pre-empts it nor swallows it: the refusal that names the missing element is
   * what the operator reads.
   */
  it("surfaces the backend refusal naming the element left empty", async () => {
    const user = userEvent.setup();
    const services = servicesWith();
    vi.spyOn(services.popis, "izvestaj").mockRejectedValue(
      new Error(
        "Izveštaj o popisu nije potpun — nedostaje: način knjiženja razlika (PoP čl. 13 st. 1).",
      ),
    );

    render(
      <IzvestajPanel
        services={services}
        session={session("computed")}
        prijavljene={[]}
      />,
    );

    await popuniNarativ(user);
    await user.click(screen.getByRole("button", { name: /sastavi izveštaj/i }));

    expect(
      await screen.findByText(/nedostaje: način knjiženja razlika/i),
    ).toBeInTheDocument();
  });
});

describe("IzvestajPanel — the computed rok (req. 38)", () => {
  it("shows the rok the backend computed, never one of its own", async () => {
    const user = userEvent.setup();
    const services = servicesWith();
    vi.spyOn(services.popis, "izvestaj").mockResolvedValue({
      ...izvestaj,
      rok: "31.01.2028",
      odlukaOUsvajanju: { ...izvestaj.odlukaOUsvajanju, rok: "31.01.2028" },
    });

    render(
      <IzvestajPanel
        services={services}
        session={session("computed")}
        prijavljene={[]}
      />,
    );

    await popuniNarativ(user);
    await user.click(screen.getByRole("button", { name: /sastavi izveštaj/i }));

    // The FY2027 leap-year answer. A panel carrying its own table would print
    // 30.01.2028 here and the shop would file a day early believing the app.
    // Twice: čl. 14 st. 2 gives the odluka the rok „iz člana 13. stav 2“.
    expect(await screen.findAllByText("31.01.2028")).toHaveLength(2);
    expect(screen.getByText("PoP čl. 13 st. 2")).toBeInTheDocument();
  });

  /**
   * Čl. 14 st. 2 gives the odluka the rok „iz člana 13. stav 2“ — one date, one
   * milestone. And the app records no such decision, which the copy has to say
   * rather than let a shop assume it was minuted somewhere.
   */
  it("carries the odluka o usvajanju on the same date and says it is not recorded", async () => {
    const user = userEvent.setup();
    const services = servicesWith();
    vi.spyOn(services.popis, "izvestaj").mockResolvedValue(izvestaj);

    render(
      <IzvestajPanel
        services={services}
        session={session("computed")}
        prijavljene={[]}
      />,
    );

    await popuniNarativ(user);
    await user.click(screen.getByRole("button", { name: /sastavi izveštaj/i }));

    expect(await screen.findByText("PoP čl. 14 st. 2")).toBeInTheDocument();
    expect(screen.getAllByText("30.01.2027")).toHaveLength(2);
    expect(
      screen.getByText(/ne evidentira odluku o usvajanju/i),
    ).toBeInTheDocument();
  });

  /**
   * ZoRač čl. 44 st. 1 sets 31 March *„osim ako posebnim zakonom nije drukčije
   * uređeno“*, so the filing deadline is configuration and the annual izveštaj
   * is refused by name without it. A panel that offered no way to set it would
   * leave the refusal unactionable.
   */
  it("lets the shop set the rok za predaju finansijskog izveštaja", async () => {
    const user = userEvent.setup();
    const services = servicesWith();
    const spy = vi
      .spyOn(services.popis, "setPodesavanja")
      .mockResolvedValue({ rokPredajeFi: "2028-03-31" });

    render(
      <IzvestajPanel
        services={services}
        session={session("computed")}
        prijavljene={[]}
      />,
    );

    const polje = await screen.findByLabelText(
      /rok za predaju finansijskog izveštaja/i,
    );
    await waitFor(() => expect(polje).toHaveValue("2027-03-31"));
    await user.clear(polje);
    await user.type(polje, "2028-03-31");
    await user.click(screen.getByRole("button", { name: /sačuvaj rok/i }));

    await waitFor(() =>
      expect(spy).toHaveBeenCalledWith({ rokPredajeFi: "2028-03-31" }),
    );
  });
});

describe("IzvestajPanel — what the app does not keep", () => {
  /**
   * The document is composed on demand and stored nowhere. A shop that typed
   * five paragraphs and closed the screen would otherwise lose them without
   * being told, and req. 42's five-year floor would look like it covered a
   * document this application never held.
   */
  it("prints the backend's warnings, including that nothing is stored", async () => {
    const user = userEvent.setup();
    const services = servicesWith();
    vi.spyOn(services.popis, "izvestaj").mockResolvedValue(izvestaj);

    render(
      <IzvestajPanel
        services={services}
        session={session("computed")}
        prijavljene={[]}
      />,
    );

    await popuniNarativ(user);
    await user.click(screen.getByRole("button", { name: /sastavi izveštaj/i }));

    expect(
      await screen.findByText(/se ne čuva u aplikaciji/i),
    ).toBeInTheDocument();
  });

  /**
   * Req. 36 — the declaration is a parameter, not a stored flag, and it is
   * REQUIRED on the wire: an omitted array satisfies the completeness gate
   * vacuously. The panel therefore always sends one, empty or not.
   *
   * It is *taken* on the count sheet, where an empty declared lista can still
   * be filled — after the čl. 8 st. 5 potpis, which this panel needs before it
   * works at all, no stavka can be added to one.
   */
  it("sends the declared categories with the request", async () => {
    const user = userEvent.setup();
    const services = servicesWith();
    const spy = vi
      .spyOn(services.popis, "izvestaj")
      .mockResolvedValue(izvestaj);

    render(
      <IzvestajPanel
        services={services}
        session={session("computed")}
        prijavljene={["gotovina"]}
      />,
    );

    await popuniNarativ(user);
    await user.click(screen.getByRole("button", { name: /sastavi izveštaj/i }));

    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    expect(spy).toHaveBeenCalledWith(1, {
      prijavljeneListe: ["gotovina"],
      narativ: {
        uzrociNeslaganja: "Kalo i lom.",
        predloziZaLikvidacijuRazlika: "Manjak na teret radnje.",
        nacinKnjizenja: "Kroz KEP i glavnu knjigu.",
        primedbeLicaKojaRukujuVrednostima: "Nema primedbi.",
        ostalePrimedbeIPredlozi: "Nema.",
      },
    });
  });

  it("declares nothing as an explicit empty array, never an absent field", async () => {
    const user = userEvent.setup();
    const services = servicesWith();
    const spy = vi
      .spyOn(services.popis, "izvestaj")
      .mockResolvedValue(izvestaj);

    render(
      <IzvestajPanel
        services={services}
        session={session("computed")}
        prijavljene={[]}
      />,
    );

    await popuniNarativ(user);
    await user.click(screen.getByRole("button", { name: /sastavi izveštaj/i }));

    await waitFor(() => expect(spy).toHaveBeenCalledTimes(1));
    const [, request] = spy.mock.calls[0];
    expect(Object.keys(request)).toContain("prijavljeneListe");
    expect(request.prijavljeneListe).toEqual([]);
  });
});

describe("IzvestajPanel — the totals it does and does not report", () => {
  /**
   * Stavke on one lista can be in komadima, metrima and kilogramima at once, so
   * a summed količina across them is a number with no unit. The izveštaj totals
   * money and counts of stavki; the naturalne razlike stay per stavka on the
   * popisne liste, and the panel must send the reader there rather than print a
   * figure the payload does not carry.
   */
  it("reports value and counts, and points at the liste for the quantities", async () => {
    const user = userEvent.setup();
    const services = servicesWith();
    vi.spyOn(services.popis, "izvestaj").mockResolvedValue(izvestaj);

    render(
      <IzvestajPanel
        services={services}
        session={session("computed")}
        prijavljene={[]}
      />,
    );

    await popuniNarativ(user);
    await user.click(screen.getByRole("button", { name: /sastavi izveštaj/i }));

    // Once per lista and once in the Ukupno row — the single seeded lista.
    expect(await screen.findAllByText("17.493,00 RSD")).toHaveLength(2);
    expect(screen.getAllByText("19.992,00 RSD")).toHaveLength(2);
    expect(screen.getAllByText("-2.499,00 RSD")).toHaveLength(2);
    expect(screen.getByText(/uz izveštaj se prilažu popisne liste/i)).toBeInTheDocument();
  });
});
