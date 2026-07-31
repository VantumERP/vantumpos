import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ReklamacijeModule } from "./ReklamacijeModule";
import { navigationItems } from "@/app/navigation";
import { Toaster } from "@/components/ui/sonner";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type {
  LegalNotice,
  ReklamacijaSummary,
  ReklamacijaView,
} from "@/services/types";

// `legal.rs::reklamacija_breach` verbatim, preduzetnik tier. The two regimes
// carry different figures (~4× apart) and different citations; the module must
// render whichever one the backend sent and derive neither.
const preduzetnikNewNotice: LegalNotice = {
  summary:
    "Nepostupanje po reklamaciji potrošača u propisanim rokovima je prekršaj.",
  penalty:
    "Prekršaj: novčana kazna u fiksnom iznosu od 100.000 dinara " +
    "(čl. 210 st. 3). Zaštitne mere nisu propisane.",
  citation:
    "Zakon o zaštiti potrošača (Sl. glasnik RS, br. 35/2026), čl. 63; " +
    "prekršajne odredbe čl. 210 st. 1 tač. 24.",
  isLegalDuty: true,
};

// What an UNSET `pravnaForma` produces: no figure at all, never a plausible one.
const unsetFormaNotice: LegalNotice = {
  ...preduzetnikNewNotice,
  penalty: null,
};

// Memo worked example (NEW/tehnička): filed 15.08. → answer 23.08., resolution
// 14.09. Nothing is overdue yet.
const otvorena: ReklamacijaSummary = {
  id: 1,
  registerNumber: 1,
  regime: "new",
  status: "open",
  podnosilacImePrezime: "Petar Petrović",
  filedAt: "2026-08-15T00:00:00Z",
  answerDue: "2026-08-23T00:00:00Z",
  resolutionDue: "2026-09-14T00:00:00Z",
  answerOverdue: false,
  resolutionOverdue: false,
  purgeEligible: false,
};

// Both clocks blown: the deadline engine flagged this row overdue on both fronts.
const prekoracena: ReklamacijaSummary = {
  id: 2,
  registerNumber: 2,
  regime: "old",
  status: "answered",
  podnosilacImePrezime: "Marija Marić",
  filedAt: "2026-06-01T00:00:00Z",
  answerDue: "2026-06-09T00:00:00Z",
  resolutionDue: "2026-06-16T00:00:00Z",
  answerOverdue: true,
  resolutionOverdue: true,
  purgeEligible: false,
};

const createdView: ReklamacijaView = {
  id: 3,
  registerNumber: 3,
  regime: "new",
  status: "open",
  filedAt: "2026-08-15T00:00:00Z",
  podnosilacImePrezime: "Petar Petrović",
  kontakt: "060/123-456",
  podaciORobi: "Veš mašina Beko",
  opisNesaobraznosti: "Ne centrifugira",
  zahtev: "Popravka",
  robaKind: "tehnicka",
  datumIzdavanjaPotvrde: "2026-08-15T10:00:00Z",
  createdBy: 1,
  createdAt: "2026-08-15T10:00:00Z",
  updatedAt: "2026-08-15T10:00:00Z",
  events: [],
  deadlines: {
    answerDue: "2026-08-23T00:00:00Z",
    resolutionDue: "2026-09-14T00:00:00Z",
    clock: "running",
    consumerWindowDue: null,
    answerOverdue: false,
    resolutionOverdue: false,
    oneExtensionUsed: false,
  },
  purgeEligible: false,
  notice: preduzetnikNewNotice,
};

function servicesWith(rows: ReklamacijaSummary[]): PosServices {
  const services = createMockServices();
  vi.spyOn(services.reklamacije, "list").mockResolvedValue(rows);
  return services;
}

// A detail view for a still-open NEW-regime complaint: the answer form must
// expose the pre-filled, mandatory express-warning fields (čl. 63 st. 10).
const newRegimeOpen: ReklamacijaView = {
  id: 10,
  registerNumber: 10,
  regime: "new",
  status: "open",
  filedAt: "2026-08-15T00:00:00Z",
  podnosilacImePrezime: "Petar Petrović",
  kontakt: "060/123-456",
  podaciORobi: "Veš mašina Beko",
  opisNesaobraznosti: "Ne centrifugira",
  zahtev: "Popravka",
  robaKind: "tehnicka",
  datumIzdavanjaPotvrde: "2026-08-15T10:00:00Z",
  createdBy: 1,
  createdAt: "2026-08-15T10:00:00Z",
  updatedAt: "2026-08-15T10:00:00Z",
  events: [],
  deadlines: {
    answerDue: "2026-08-23T00:00:00Z",
    resolutionDue: "2026-09-14T00:00:00Z",
    clock: "running",
    consumerWindowDue: null,
    answerOverdue: false,
    resolutionOverdue: false,
    oneExtensionUsed: false,
  },
  purgeEligible: false,
  notice: preduzetnikNewNotice,
};

// The OLD regime is NOT gated on the express warning — rejecting an old-regime
// answer for lacking it would be wrong — so those fields must be absent.
const oldRegimeOpen: ReklamacijaView = {
  ...newRegimeOpen,
  id: 11,
  registerNumber: 11,
  regime: "old",
  filedAt: "2026-06-01T00:00:00Z",
  podnosilacImePrezime: "Marija Marić",
};

// Both clocks blown — the only state in which the advisory penalty context is
// shown at all.
const overdue: ReklamacijaView = {
  ...newRegimeOpen,
  id: 13,
  registerNumber: 13,
  deadlines: {
    ...newRegimeOpen.deadlines,
    answerOverdue: true,
    resolutionOverdue: true,
  },
};

// An extension was already granted: čl. 55/63 st. 11 permits exactly one.
const extensionUsed: ReklamacijaView = {
  ...newRegimeOpen,
  id: 12,
  registerNumber: 12,
  status: "answered",
  events: [
    {
      eventType: "extension_granted",
      eventDate: "2026-09-20T00:00:00Z",
      detailJson: null,
      consumerConsent: true,
    },
  ],
  deadlines: { ...newRegimeOpen.deadlines, oneExtensionUsed: true },
};

function summaryOf(view: ReklamacijaView): ReklamacijaSummary {
  return {
    id: view.id,
    registerNumber: view.registerNumber,
    regime: view.regime,
    status: view.status,
    podnosilacImePrezime: view.podnosilacImePrezime,
    filedAt: view.filedAt,
    answerDue: view.deadlines.answerDue,
    resolutionDue: view.deadlines.resolutionDue,
    answerOverdue: view.deadlines.answerOverdue,
    resolutionOverdue: view.deadlines.resolutionOverdue,
    purgeEligible: view.purgeEligible,
  };
}

function servicesWithView(view: ReklamacijaView): PosServices {
  const services = createMockServices();
  vi.spyOn(services.reklamacije, "list").mockResolvedValue([summaryOf(view)]);
  vi.spyOn(services.reklamacije, "get").mockResolvedValue(view);
  return services;
}

describe("navigation", () => {
  it("exposes reklamacije as an admin-only item after receipts", () => {
    const item = navigationItems.find(
      (candidate) => candidate.id === "reklamacije",
    );

    expect(item).toMatchObject({ label: "Reklamacije", adminOnly: true });

    const receiptsIndex = navigationItems.findIndex((c) => c.id === "receipts");
    const reklamacijeIndex = navigationItems.findIndex(
      (c) => c.id === "reklamacije",
    );
    expect(reklamacijeIndex).toBe(receiptsIndex + 1);
  });
});

describe("ReklamacijeModule list", () => {
  it("renders register numbers, filers and Serbian status labels", async () => {
    render(<ReklamacijeModule services={servicesWith([otvorena, prekoracena])} />);

    expect(await screen.findByText("Petar Petrović")).toBeInTheDocument();
    expect(screen.getByText("Marija Marić")).toBeInTheDocument();

    expect(screen.getByText("Otvorena")).toBeInTheDocument();
    expect(screen.getByText("Odgovoreno")).toBeInTheDocument();
  });

  it("emphasises the answer and resolution dates only on an overdue row", async () => {
    render(<ReklamacijeModule services={servicesWith([otvorena, prekoracena])} />);

    // Overdue row: both dates carry the overdue marker.
    const overdueAnswer = await screen.findByText("09.06.2026.");
    const overdueResolution = screen.getByText("16.06.2026.");
    expect(overdueAnswer).toHaveAttribute("data-overdue", "true");
    expect(overdueResolution).toHaveAttribute("data-overdue", "true");

    // Healthy row: the same cells render without the marker.
    expect(screen.getByText("23.08.2026.")).not.toHaveAttribute("data-overdue");
    expect(screen.getByText("14.09.2026.")).not.toHaveAttribute("data-overdue");
  });
});

describe("ReklamacijeModule intake", () => {
  it("defaults to the safe 15-day track and today's filing date", async () => {
    const user = userEvent.setup();
    render(<ReklamacijeModule services={servicesWith([])} />);

    await user.click(
      await screen.findByRole("button", { name: "Nova reklamacija" }),
    );

    // Over-flagging (calling a plain good 30-day) grants unearned time — the
    // default MUST be the shorter, safe track.
    expect(screen.getByLabelText("Vrsta robe")).toHaveValue("opsta");

    const today = new Date().toISOString().slice(0, 10);
    expect(screen.getByLabelText("Datum podnošenja")).toHaveValue(today);
  });

  it("calls create with the shaped intake input", async () => {
    const user = userEvent.setup();
    const services = servicesWith([]);
    const createSpy = vi
      .spyOn(services.reklamacije, "create")
      .mockResolvedValue(createdView);

    render(<ReklamacijeModule services={services} />);

    await user.click(
      await screen.findByRole("button", { name: "Nova reklamacija" }),
    );

    await user.type(
      screen.getByLabelText("Ime i prezime podnosioca"),
      "Petar Petrović",
    );
    await user.type(screen.getByLabelText("Kontakt"), "060/123-456");
    await user.type(screen.getByLabelText("Podaci o robi"), "Veš mašina Beko");
    await user.type(
      screen.getByLabelText("Opis nesaobraznosti"),
      "Ne centrifugira",
    );
    await user.type(screen.getByLabelText("Zahtev potrošača"), "Popravka");
    await user.selectOptions(screen.getByLabelText("Vrsta robe"), "tehnicka");
    fireEvent.change(screen.getByLabelText("Datum podnošenja"), {
      target: { value: "2026-08-15" },
    });

    await user.click(
      screen.getByRole("button", { name: "Evidentiraj reklamaciju" }),
    );

    await waitFor(() =>
      expect(createSpy).toHaveBeenCalledWith({
        podnosilacImePrezime: "Petar Petrović",
        kontakt: "060/123-456",
        podaciORobi: "Veš mašina Beko",
        opisNesaobraznosti: "Ne centrifugira",
        zahtev: "Popravka",
        robaKind: "tehnicka",
        filedAt: "2026-08-15T00:00:00Z",
      }),
    );
  });
});

describe("ReklamacijeModule detail", () => {
  it("pre-fills and requires the express-warning fields for a new-regime answer", async () => {
    const user = userEvent.setup();
    render(<ReklamacijeModule services={servicesWithView(newRegimeOpen)} />);

    await user.click(
      await screen.findByRole("button", {
        name: "Detalji za reklamaciju #10",
      }),
    );

    // The regime is shown as read-only context.
    expect(await screen.findByText("Novi režim")).toBeInTheDocument();

    // All three express-warning parts are present, pre-filled and required.
    const duty = await screen.findByLabelText("Obaveza izjašnjenja potrošača");
    const consequences = screen.getByLabelText("Posledice propuštanja roka");
    const zastoj = screen.getByLabelText("Zastoj rokova");

    expect(duty).toBeRequired();
    expect(consequences).toBeRequired();
    expect(zastoj).toBeRequired();

    expect(duty).toHaveDisplayValue(/najkasnije u roku od 3 \(tri\) dana/);
    expect(consequences).toHaveDisplayValue(/smatraće se da niste saglasni/);
    expect(zastoj).toHaveDisplayValue(/zastaje danom Vašeg prijema/);
  });

  it("renders the backend's penalty copy verbatim on an overdue record", async () => {
    const user = userEvent.setup();
    render(<ReklamacijeModule services={servicesWithView(overdue)} />);

    await user.click(
      await screen.findByRole("button", {
        name: "Detalji za reklamaciju #13",
      }),
    );

    // Every word and every figure comes from `legal.rs` — the module resolves
    // neither the regime nor the pravna forma into an amount of its own.
    expect(
      await screen.findByText(preduzetnikNewNotice.summary, { exact: false }),
    ).toBeInTheDocument();
    expect(
      screen.getByText(preduzetnikNewNotice.penalty as string),
    ).toBeInTheDocument();
    expect(screen.getByText(preduzetnikNewNotice.citation)).toBeInTheDocument();
  });

  it("shows no figure at all when the shop's legal form is unanswered", async () => {
    const user = userEvent.setup();
    render(
      <ReklamacijeModule
        services={servicesWithView({ ...overdue, notice: unsetFormaNotice })}
      />,
    );

    await user.click(
      await screen.findByRole("button", {
        name: "Detalji za reklamaciju #13",
      }),
    );

    expect(
      await screen.findByText(/Unesite pravnu formu u Podešavanja → Profil/),
    ).toBeInTheDocument();
    // A plausible-but-untiered amount is the defect this replaces: with no
    // legal form on file there is no correct figure, so none may be shown.
    expect(screen.queryByText(/dinara/)).not.toBeInTheDocument();
  });

  it("omits the express-warning fields for an old-regime answer", async () => {
    const user = userEvent.setup();
    render(<ReklamacijeModule services={servicesWithView(oldRegimeOpen)} />);

    await user.click(
      await screen.findByRole("button", {
        name: "Detalji za reklamaciju #11",
      }),
    );

    expect(await screen.findByText("Stari režim")).toBeInTheDocument();
    // The answer form itself is present…
    expect(screen.getByLabelText("Odgovor na reklamaciju")).toBeInTheDocument();
    // …but the express warning is a NEW-regime duty only.
    expect(
      screen.queryByLabelText("Obaveza izjašnjenja potrošača"),
    ).not.toBeInTheDocument();
  });

  it("logs a new-regime answer with the express-warning fields", async () => {
    const user = userEvent.setup();
    const services = servicesWithView(newRegimeOpen);
    const logAnswer = vi
      .spyOn(services.reklamacije, "logAnswer")
      .mockResolvedValue({ ...newRegimeOpen, status: "answered" });

    render(<ReklamacijeModule services={services} />);

    await user.click(
      await screen.findByRole("button", {
        name: "Detalji za reklamaciju #10",
      }),
    );

    await user.type(
      await screen.findByLabelText("Odgovor na reklamaciju"),
      "Prihvatamo reklamaciju i predlažemo popravku.",
    );
    await user.click(screen.getByRole("button", { name: "Unesi odgovor" }));

    await waitFor(() => expect(logAnswer).toHaveBeenCalledTimes(1));
    const [id, input] = logAnswer.mock.calls[0]!;
    expect(id).toBe(10);
    expect(input.answerText).toBe(
      "Prihvatamo reklamaciju i predlažemo popravku.",
    );
    expect(input.warningDuty).toMatch(/najkasnije u roku od 3 \(tri\) dana/);
    expect(input.warningConsequences).toMatch(/smatraće se da niste saglasni/);
    expect(input.warningZastoj).toMatch(/zastaje danom Vašeg prijema/);
  });

  it("disables Produži rok once one extension has been used", async () => {
    const user = userEvent.setup();
    render(<ReklamacijeModule services={servicesWithView(extensionUsed)} />);

    await user.click(
      await screen.findByRole("button", {
        name: "Detalji za reklamaciju #12",
      }),
    );

    expect(
      await screen.findByRole("button", { name: "Produži rok" }),
    ).toBeDisabled();
  });

  it("prints the potvrda by exporting then opening it", async () => {
    const user = userEvent.setup();
    const services = servicesWithView(newRegimeOpen);
    const exportPotvrda = vi
      .spyOn(services.reklamacije, "exportPotvrda")
      .mockResolvedValue({
        fileName: "potvrda-reklamacija-10.html",
        path: "C:/exports/potvrda-reklamacija-10.html",
        mimeType: "text/html",
        rowCount: 1,
      });
    const openForPrint = vi
      .spyOn(services.print, "openForPrint")
      .mockResolvedValue(undefined);

    render(
      <>
        <ReklamacijeModule services={services} />
        <Toaster />
      </>,
    );

    await user.click(
      await screen.findByRole("button", {
        name: "Detalji za reklamaciju #10",
      }),
    );

    await user.click(
      await screen.findByRole("button", { name: "Štampaj potvrdu" }),
    );

    expect(exportPotvrda).toHaveBeenCalledWith(10);
    await waitFor(() =>
      expect(openForPrint).toHaveBeenCalledWith(
        "C:/exports/potvrda-reklamacija-10.html",
      ),
    );
  });

  it("prints the prodajno-mesto notice by exporting then opening it", async () => {
    const user = userEvent.setup();
    const services = servicesWithView(newRegimeOpen);
    const exportNotice = vi
      .spyOn(services.reklamacije, "exportNotice")
      .mockResolvedValue({
        fileName: "obavestenje-reklamacije.html",
        path: "C:/exports/obavestenje-reklamacije.html",
        mimeType: "text/html",
        rowCount: 0,
      });
    const openForPrint = vi
      .spyOn(services.print, "openForPrint")
      .mockResolvedValue(undefined);

    render(
      <>
        <ReklamacijeModule services={services} />
        <Toaster />
      </>,
    );

    await user.click(
      await screen.findByRole("button", {
        name: "Detalji za reklamaciju #10",
      }),
    );

    await user.click(
      await screen.findByRole("button", { name: "Štampaj obaveštenje" }),
    );

    expect(exportNotice).toHaveBeenCalledTimes(1);
    await waitFor(() =>
      expect(openForPrint).toHaveBeenCalledWith(
        "C:/exports/obavestenje-reklamacije.html",
      ),
    );
  });
});
