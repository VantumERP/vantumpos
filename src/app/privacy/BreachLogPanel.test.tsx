import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterAll, beforeAll, describe, expect, it, vi } from "vitest";

import { BreachLogPanel } from "./BreachLogPanel";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type { Breach, LegalNotice } from "@/services/types";

const notice: LegalNotice = {
  summary:
    "Rukovalac je dužan da o povredi podataka o ličnosti koja može da proizvede rizik po prava " +
    "i slobode fizičkih lica obavesti Poverenika najkasnije u roku od 72 časa od saznanja.",
  penalty:
    "Prekršaj: novčana kazna od 20.000 do 500.000 dinara (čl. 95 st. 1 tač. 24 u vezi sa st. 4).",
  citation: "Zakon o zaštiti podataka o ličnosti, čl. 52 st. 1 i st. 2.",
  isLegalDuty: true,
};

function breach(overrides: Partial<Breach> = {}): Breach {
  return {
    id: 1,
    saznanjeAt: "2026-08-01T09:00:00Z",
    occurredAt: null,
    discoveredAt: null,
    obradjivacSaznanjeAt: null,
    rukovalacObavestenAt: null,
    opis: "Nestao je papirni spisak zaposlenih iz kancelarije.",
    posledice: "Podaci o troje zaposlenih su mogli da budu pročitani.",
    mere: "Brava je zamenjena, spisak se više ne štampa.",
    brojLica: 3,
    kategorijePodataka: null,
    riskOutcome: null,
    notifyDecision: null,
    notifyObrazlozenje: null,
    poverenikNotifiedAt: null,
    delayReason: null,
    licaObavestena: null,
    licaObavestenaAt: null,
    cl53Izuzetak: null,
    cl53IzuzetakObrazlozenje: null,
    createdAt: "2026-08-01T09:05:00Z",
    updatedAt: "2026-08-01T09:05:00Z",
    rokObavestavanjaIsticeAt: "2026-08-04T09:00:00Z",
    notifiable: null,
    delayReasonRequired: false,
    obavestavanjeLicaObavezno: false,
    ...overrides,
  };
}

function services(breaches: Breach[] = [breach()]): PosServices {
  const posServices = createMockServices();
  vi.spyOn(posServices.privacy, "listBreaches").mockResolvedValue(breaches);
  vi.spyOn(posServices.privacy, "breachNotice").mockResolvedValue(notice);
  return posServices;
}

describe("BreachLogPanel", () => {
  /**
   * The panel converts in both directions across a zone boundary — the operator
   * types a wall clock, čl. 52 st. 1 runs from an instant — so the suite pins
   * its own zone instead of inheriting the machine's. Beograd is UTC+2 in
   * August, which is exactly the offset an anchor bug would swallow.
   */
  beforeAll(() => {
    vi.stubEnv("TZ", "Europe/Belgrade");
  });

  afterAll(() => {
    vi.unstubAllEnvs();
  });

  /**
   * Req. 43. Čl. 52 st. 6 documents *„svaku povredu“* with no risk qualifier;
   * the risk gate lives one stav up and governs only the Poverenik. A wizard
   * that asks „is this notifiable?“ first and discards the No answers inverts
   * the statute.
   */
  it("does not gate the breach form on notifiability", async () => {
    const posServices = services([]);
    const record = vi
      .spyOn(posServices.privacy, "recordBreach")
      .mockResolvedValue(breach());
    const user = userEvent.setup();

    render(<BreachLogPanel services={posServices} />);
    await screen.findByLabelText(/činjenice o povredi/i);

    // Nothing asks whether the incident is notifiable before the three st. 6
    // elements can be entered, and no control is disabled pending an answer.
    expect(
      screen.queryByLabelText(/da li povreda podleže obaveštavanju/i),
    ).not.toBeInTheDocument();
    expect(
      screen.getByRole("button", { name: /evidentiraj povredu/i }),
    ).toBeEnabled();

    await user.type(screen.getByLabelText(/saznanje/i), "2026-08-01T09:00");
    await user.type(
      screen.getByLabelText(/činjenice o povredi/i),
      "Nestao je papirni spisak zaposlenih.",
    );
    await user.type(
      screen.getByLabelText(/posledice/i),
      "Podaci o troje zaposlenih su mogli da budu pročitani.",
    );
    await user.type(
      screen.getByLabelText(/preduzete mere/i),
      "Brava je zamenjena.",
    );
    await user.click(screen.getByRole("button", { name: /evidentiraj povredu/i }));

    await waitFor(() => {
      expect(record).toHaveBeenCalled();
    });

    // The record was written with the risk assessment still unanswered.
    const draft = record.mock.calls[0]?.[0];
    expect(draft?.riskOutcome ?? null).toBeNull();
    expect(draft?.notifyDecision ?? null).toBeNull();
    // 09:00 in Beograd is 07:00Z in August. Stamping the wall clock „09:00Z“
    // would move the anchor two hours later than the moment the operator meant,
    // and every čl. 52 st. 1 countdown off it would run that much late.
    expect(draft?.saznanjeAt).toBe("2026-08-01T07:00:00Z");
    expect(draft?.opis).toBe("Nestao je papirni spisak zaposlenih.");
  });

  /**
   * Čl. 52 st. 1 / Pravilnik 40/2019 čl. 3. The instant the panel sends and the
   * instant it renders back are the same moment, so the register card and the
   * printed obrazac cannot contradict the wall clock the operator typed.
   */
  it("renders the saznanje back at the wall clock the operator typed", async () => {
    const posServices = services([]);
    vi.spyOn(posServices.privacy, "recordBreach").mockImplementation(
      async (draft) => breach({ saznanjeAt: draft.saznanjeAt }),
    );
    const user = userEvent.setup();

    render(<BreachLogPanel services={posServices} />);
    await screen.findByLabelText(/činjenice o povredi/i);

    await user.type(screen.getByLabelText(/saznanje/i), "2026-08-01T09:00");
    await user.type(
      screen.getByLabelText(/činjenice o povredi/i),
      "Nestao je papirni spisak zaposlenih.",
    );
    await user.type(
      screen.getByLabelText(/posledice/i),
      "Podaci su mogli da budu pročitani.",
    );
    await user.type(screen.getByLabelText(/preduzete mere/i), "Brava je zamenjena.");
    await user.click(screen.getByRole("button", { name: /evidentiraj povredu/i }));

    const card = await screen.findByText(/^Saznanje: /);
    expect(card).toHaveTextContent(/09:00/);
    expect(card).not.toHaveTextContent(/11:00/);
  });

  /**
   * Čl. 52 st. 2 asks why the st. 1 deadline was missed. A shop that DID notify
   * inside the 72 h must be able to say so — otherwise every later save on the
   * record is refused until a delay reason that does not exist is typed onto the
   * very document čl. 52 st. 7 makes the vehicle for proving compliance.
   */
  it("lets a povreda notified inside 72 h save without a delay reason", async () => {
    const posServices = services([
      breach({ id: 7, delayReasonRequired: true, notifiable: true }),
    ]);
    const update = vi
      .spyOn(posServices.privacy, "updateBreach")
      .mockResolvedValue(breach({ id: 7 }));
    const user = userEvent.setup();

    render(<BreachLogPanel services={posServices} />);
    await user.click(await screen.findByRole("button", { name: /otvori zapis/i }));

    await user.type(
      screen.getByLabelText(/obaveštenje dostavljeno povereniku/i),
      "2026-08-02T10:00",
    );
    await user.click(screen.getByRole("button", { name: /sačuvaj zapis/i }));

    await waitFor(() => {
      expect(update).toHaveBeenCalled();
    });
    const draft = update.mock.calls[0]?.[1];
    expect(draft?.poverenikNotifiedAt).toBe("2026-08-02T08:00:00Z");
    expect(draft?.delayReason ?? null).toBeNull();
  });

  /**
   * Čl. 53 st. 3 — a visok-rizik povreda whose subjects were not told must name
   * which of the three exceptions the rukovalac relied on, and say why.
   */
  it("records the čl. 53 st. 3 izuzetak on a visok-rizik povreda", async () => {
    const posServices = services([
      breach({
        id: 8,
        riskOutcome: "visok_rizik",
        obavestavanjeLicaObavezno: true,
        notifiable: true,
      }),
    ]);
    const update = vi
      .spyOn(posServices.privacy, "updateBreach")
      .mockResolvedValue(breach({ id: 8 }));
    const user = userEvent.setup();

    render(<BreachLogPanel services={posServices} />);
    await user.click(await screen.findByRole("button", { name: /otvori zapis/i }));

    await user.selectOptions(
      screen.getByLabelText(/lica na koja se podaci odnose obaveštena/i),
      "ne",
    );
    await user.selectOptions(
      screen.getByLabelText(/izuzetak od obaveštavanja lica/i),
      "primenjene_mere_zastite",
    );
    await user.type(
      screen.getByLabelText(/obrazloženje izuzetka/i),
      "Spisak je bio šifrovan.",
    );
    await user.click(screen.getByRole("button", { name: /sačuvaj zapis/i }));

    await waitFor(() => {
      expect(update).toHaveBeenCalled();
    });
    const draft = update.mock.calls[0]?.[1];
    expect(draft?.licaObavestena).toBe(false);
    expect(draft?.cl53Izuzetak).toBe("primenjene_mere_zastite");
    expect(draft?.cl53IzuzetakObrazlozenje).toBe("Spisak je bio šifrovan.");
  });

  /**
   * Req. 45 / §3 V4. Section 2 (2) *vrsta podataka o ličnosti* and 2 (5) *datum
   * i vreme povrede* are prescribed sub-fields of the Pravilnik 40/2019 obrazac
   * this panel exports, so the record has to be able to hold them.
   */
  it("fills the two obrazac slots the record used to leave blank", async () => {
    const posServices = services();
    const update = vi
      .spyOn(posServices.privacy, "updateBreach")
      .mockResolvedValue(breach());
    const user = userEvent.setup();

    render(<BreachLogPanel services={posServices} />);
    await user.click(await screen.findByRole("button", { name: /otvori zapis/i }));

    await user.type(
      screen.getByLabelText(/vrsta podataka o ličnosti/i),
      "Ime i prezime, radno mesto.",
    );
    await user.type(
      screen.getByLabelText(/datum i vreme povrede/i),
      "2026-07-31T18:30",
    );
    await user.click(screen.getByRole("button", { name: /sačuvaj zapis/i }));

    await waitFor(() => {
      expect(update).toHaveBeenCalled();
    });
    const draft = update.mock.calls[0]?.[1];
    expect(draft?.kategorijePodataka).toBe("Ime i prezime, radno mesto.");
    expect(draft?.occurredAt).toBe("2026-07-31T16:30:00Z");
  });

  /** Čl. 52 st. 1 — the anchor is immutable, so it is never offered for edit. */
  it("keeps saznanje immutable on a recorded povreda", async () => {
    const user = userEvent.setup();
    render(<BreachLogPanel services={services()} />);

    await user.click(await screen.findByRole("button", { name: /otvori zapis/i }));

    const anchor = await screen.findByText(/vreme saznanja je nepromenljivo/i);
    expect(anchor).toBeInTheDocument();
    // The open record's own saznanje field is not writable anywhere.
    expect(
      screen.queryByLabelText(/promeni vreme saznanja/i),
    ).not.toBeInTheDocument();
  });

  /** Čl. 52 st. 6 keeps the record whichever way st. 1 came out. */
  it("shows a not-notifiable povreda in the register all the same", async () => {
    render(
      <BreachLogPanel
        services={services([
          breach({
            id: 4,
            riskOutcome: "bez_rizika",
            notifyDecision: "ne_obavestiti",
            notifyObrazlozenje: "Podaci su bili pseudonimizovani.",
            notifiable: false,
          }),
        ])}
      />,
    );

    expect(await screen.findByText(/nestao je papirni spisak/i)).toBeInTheDocument();
    expect(screen.getByText(/poverenik se ne obaveštava/i)).toBeInTheDocument();
  });

  /** Req. 50: the figure on screen is the notification tier, from the backend. */
  it("renders the čl. 52 exposure exactly as the backend resolved it", async () => {
    render(<BreachLogPanel services={services()} />);

    expect(await screen.findByText(notice.summary)).toBeInTheDocument();
    expect(screen.getByText(notice.penalty as string)).toBeInTheDocument();
    expect(screen.getByText(notice.citation)).toBeInTheDocument();
  });

  /**
   * Req. 45 / §5 item 23: the deliverable is a document the operator files.
   * Nothing here may read as „the app has notified the Poverenik“.
   */
  it("exports the obrazac for printing and never claims it was filed", async () => {
    const posServices = services();
    const exportObrazac = vi
      .spyOn(posServices.privacy, "exportBreachObrazac")
      .mockResolvedValue({
        fileName: "obrazac-povreda-podataka-1.html",
        path: "mock://exports/obrazac-povreda-podataka-1.html",
        mimeType: "text/html",
        rowCount: 1,
      });
    const openForPrint = vi.spyOn(posServices.print, "openForPrint");
    const user = userEvent.setup();

    render(<BreachLogPanel services={posServices} />);
    await user.click(await screen.findByRole("button", { name: /obrazac za poverenika/i }));

    await waitFor(() => {
      expect(exportObrazac).toHaveBeenCalledWith(1);
    });
    await waitFor(() => {
      expect(openForPrint).toHaveBeenCalledWith(
        "mock://exports/obrazac-povreda-podataka-1.html",
      );
    });
    expect(screen.queryByText(/poslato Povereniku|prijava je podneta/i)).not.toBeInTheDocument();
    expect(screen.getByText(/obrazac se podnosi u pisanom obliku/i)).toBeInTheDocument();
  });

  /** Čl. 52 st. 2 — once the 72 h have run, the delay must be explained. */
  it("marks the records that owe a čl. 52 st. 2 obrazloženje", async () => {
    render(
      <BreachLogPanel
        services={services([
          breach({ id: 9, delayReasonRequired: true, notifiable: true }),
        ])}
      />,
    );

    expect(
      await screen.findByText(/rok od 72 časa je istekao/i),
    ).toBeInTheDocument();
  });

  it("says an empty register is empty instead of rendering an empty table", async () => {
    render(<BreachLogPanel services={services([])} />);

    expect(
      await screen.findByText(/nema evidentiranih povreda/i),
    ).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});
