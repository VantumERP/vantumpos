import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

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
    expect(draft?.saznanjeAt).toBe("2026-08-01T09:00:00Z");
    expect(draft?.opis).toBe("Nestao je papirni spisak zaposlenih.");
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
