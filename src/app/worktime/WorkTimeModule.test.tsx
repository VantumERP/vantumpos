import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import {
  WorkTimeModule,
  capNoticeFor,
  todayIso,
  validateDan,
} from "./WorkTimeModule";
import { navigationItems } from "@/app/navigation";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type {
  UserAccount,
  WorkTimeCapAssessment,
  WorkTimeEntryView,
  WorkTimeMinutes,
  WorkTimeMonth,
} from "@/services/types";

const admin: UserAccount = {
  id: 1,
  username: "admin",
  displayName: "Administrator",
  role: "admin",
  active: true,
  createdAt: "2026-06-18T10:00:00Z",
  updatedAt: "2026-06-18T10:00:00Z",
  lastLoginAt: "2026-06-18T10:00:00Z",
};

const kasir: UserAccount = {
  ...admin,
  id: 2,
  username: "marko",
  displayName: "Marko Marković",
  role: "cashier",
  lastLoginAt: null,
};

function mockServices(): PosServices {
  return createMockServices();
}

const nulaMinuta: WorkTimeMinutes = {
  moguciMinuta: 0,
  ukupnoOstvareniMinuta: 0,
  efektivnoIzvrseniMinuta: 0,
  casoviCekanjaIZastojaMinuta: 0,
  obustavaRadaStrajkMinuta: 0,
  ukupnoNeizvrseniMinuta: 0,
  godisnjiOdmorMinuta: 0,
  praznikOdmorMinuta: 0,
  odsustvoUzNaknaduMinuta: 0,
  strucnoOsposobljavanjeMinuta: 0,
  sprecenostPoslodavacMinuta: 0,
  naknadaDrugiPoslodavciMinuta: 0,
  sprecenostRfzoMinuta: 0,
  porodiljskoMinuta: 0,
  neplacenoOdsustvoMinuta: 0,
  prekovremeniMinuta: 0,
  nocniMinuta: 0,
  radNaPraznikMinuta: 0,
};

function entry(overrides: Partial<WorkTimeEntryView>): WorkTimeEntryView {
  return {
    id: 1,
    userId: 2,
    dan: "2026-06-01",
    verzija: 1,
    zamenjen: false,
    supersedesId: null,
    kategorijaOdsustva: null,
    capOverrideRazlog: null,
    korekcijaRazlog: null,
    unioUserId: 1,
    unioIme: "Administrator",
    createdAt: "2026-06-01T18:00:00Z",
    updatedAt: "2026-06-01T18:00:00Z",
    minuti: { ...nulaMinuta },
    ...overrides,
  };
}

function month(entries: WorkTimeEntryView[], overrides: Partial<WorkTimeMonth> = {}): WorkTimeMonth {
  const ukupno = { ...nulaMinuta };
  for (const row of entries.filter((candidate) => !candidate.zamenjen)) {
    for (const key of Object.keys(ukupno) as (keyof WorkTimeMinutes)[]) {
      ukupno[key] += row.minuti[key];
    }
  }

  return {
    userId: 2,
    zaposleni: "Marko Marković",
    godina: 2026,
    mesec: 6,
    zatvoren: false,
    closedAt: null,
    entries,
    ukupno,
    napomena:
      "Evidencija prekovremenog rada — ZoR čl. 55 st. 6. Zakon ne propisuje obrazac.",
    advisoryNapomena: "izračunato radi provere usklađenosti",
    ...overrides,
  };
}

/** A month fixture served whatever period the module asks for. */
function servicesWithMonth(value: WorkTimeMonth): PosServices {
  const services = mockServices();
  vi.spyOn(services.worktime, "listMonth").mockImplementation(
    async (userId, godina, mesec) => ({ ...value, userId, godina, mesec }),
  );
  return services;
}

/**
 * The day an entry form defaults to — the module never back-dates for you.
 *
 * Local calendar components, never `toISOString()`: between midnight and 02:00
 * CEST the UTC date is still yesterday, and a register whose whole legal point
 * is per-calendar-day granularity must not default to the wrong day.
 */
function today(): string {
  const now = new Date();
  const month = `${now.getMonth() + 1}`.padStart(2, "0");
  const day = `${now.getDate()}`.padStart(2, "0");

  return `${now.getFullYear()}-${month}-${day}`;
}

/** The calendar month before the current one, as `{ godina, mesec }`. */
function previousPeriod(): { godina: number; mesec: number } {
  const now = new Date();
  const mesec = now.getMonth() + 1;

  return mesec === 1
    ? { godina: now.getFullYear() - 1, mesec: 12 }
    : { godina: now.getFullYear(), mesec: mesec - 1 };
}

const nulaCaps: WorkTimeCapAssessment = {
  weeklyOvertimeMinutes: 0,
  dailyTotalMinutes: 0,
  weeklyTotalMinutes: 0,
  weeklyCapExceeded: false,
  dailyCapExceeded: false,
  preraspodelaWeeklyCapExceeded: false,
  requiresOverride: false,
};

/**
 * Waits until an employee is selected and their month has loaded.
 *
 * The module clears the form whenever the selected employee or period changes,
 * so touching the form before the first employee lands would have that reset
 * wipe whatever the test just typed. The loading badge is the signal: it clears
 * only once `listMonth` has answered, which cannot happen before an employee is
 * selected. The employee `<select>` itself is no signal at all — a native select
 * whose value is not among its options renders the first one.
 */
async function awaitLoadedMonth(): Promise<void> {
  await waitFor(() => {
    expect(screen.queryByText(/učitavanje evidencije/i)).not.toBeInTheDocument();
  });
}

async function setMinutes(
  user: ReturnType<typeof userEvent.setup>,
  label: RegExp,
  value: string,
) {
  const field = screen.getByLabelText(label);
  await user.clear(field);
  await user.type(field, value);
}

describe("navigation", () => {
  it("exposes Radno vreme as an admin-only item", () => {
    const item = navigationItems.find((candidate) => candidate.id === "worktime");

    expect(item).toMatchObject({ label: "Radno vreme", adminOnly: true });
  });
});

describe("WorkTimeModule statutory copy", () => {
  it("labels advisory columns as computed, never as statutory fields", async () => {
    render(<WorkTimeModule services={mockServices()} currentUser={admin} />);

    expect(
      await screen.findByText(/izračunato radi provere usklađenosti/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/noćni časovi.*zakonom propisano/i),
    ).not.toBeInTheDocument();
  });

  it("carries the čl. 55 st. 6 sentence and never claims a propisani obrazac", async () => {
    render(<WorkTimeModule services={mockServices()} currentUser={admin} />);

    expect(await screen.findByText(/zakon ne propisuje obrazac/i)).toBeInTheDocument();
    expect(screen.queryByText(/propisani obrazac/i)).not.toBeInTheDocument();
    expect(
      screen.queryByText(/propisan[au] evidencij[au] o zaradama/i),
    ).not.toBeInTheDocument();
  });

  it("shows no ZEOR figure anywhere", async () => {
    render(<WorkTimeModule services={mockServices()} currentUser={admin} />);

    await screen.findByText(/zakon ne propisuje obrazac/i);
    expect(screen.queryByText(/500\.000 do 1\.000\.000/)).not.toBeInTheDocument();
    expect(screen.queryByText(/ZEOR/)).not.toBeInTheDocument();
  });

  it("never threatens a zaštitna mera for the missing register", async () => {
    render(<WorkTimeModule services={mockServices()} currentUser={admin} />);

    await screen.findByText(/zakon ne propisuje obrazac/i);
    expect(
      screen.queryByText(/zabrana obavljanja delatnosti/i),
    ).not.toBeInTheDocument();
  });
});

describe("WorkTimeModule cap warnings", () => {
  it("offers an override with a reason when a cap is exceeded, never a dead end", async () => {
    const user = userEvent.setup();
    const services = mockServices();
    const saveSpy = vi.spyOn(services.worktime, "saveEntry");

    render(<WorkTimeModule services={services} currentUser={admin} />);
    await screen.findByText(/zakon ne propisuje obrazac/i);

    // 8 h + 5 h overtime = 13 h — over the twelve-hour daily total.
    await setMinutes(user, /efektivno izvršeni/i, "480");
    await setMinutes(user, /prekovremeni/i, "300");
    await user.click(screen.getByRole("button", { name: /sačuvaj dan/i }));

    // The warning names čl. 53 and the day is NOT refused — it asks for a ground.
    expect(
      await screen.findByText(/Prekoračen limit radnog vremena/i),
    ).toBeInTheDocument();
    expect(screen.getByText(/ZoR čl\. 53/)).toBeInTheDocument();

    const razlog = await screen.findByLabelText(/razlog prekoračenja/i);
    await user.selectOptions(razlog, "iznenadno_povecanje_obima_posla");
    await user.click(screen.getByRole("button", { name: /sačuvaj dan/i }));

    await waitFor(() => {
      expect(saveSpy).toHaveBeenLastCalledWith(
        expect.objectContaining({
          dan: today(),
          efektivnoIzvrseniMinuta: 480,
          prekovremeniMinuta: 300,
          capOverrideRazlog: "iznenadno_povecanje_obima_posla",
        }),
      );
    });
    // The day reaches the register, carrying the ground it was recorded on.
    expect(
      await screen.findByText(/Prekoračenje: Iznenadno povećanje obima posla/),
    ).toBeInTheDocument();
  });

  it("surfaces the čl. 53 exposure resolved by the backend, with no figure of its own", async () => {
    const services = mockServices();
    vi.spyOn(services.worktime, "notices").mockResolvedValue({
      recordMissing: {
        summary:
          "Poslodavac je dužan da vodi dnevnu evidenciju o prekovremenom radu zaposlenih.",
        penalty:
          "Prekršaj: novčana kazna od 50.000 do 150.000 dinara (čl. 276 st. 1 u vezi sa tač. 1a).",
        citation: "Zakon o radu, čl. 55 st. 6. Nadzor: inspektor rada.",
        isLegalDuty: true,
      },
      capsExceeded: {
        summary:
          "Prekovremeni rad ne može trajati duže od osam časova nedeljno, niti ukupno radno vreme sa prekovremenim duže od 12 časova dnevno.",
        penalty:
          "Prekršaj: novčana kazna od 200.000 do 400.000 dinara (čl. 274 st. 1 tač. 3 u vezi sa st. 2).",
        citation: "Zakon o radu, čl. 53 st. 2 i st. 3. Nadzor: inspektor rada.",
        isLegalDuty: true,
      },
      preraspodelaCapsExceeded: {
        summary:
          "U slučaju preraspodele radnog vremena, radno vreme ne može da traje duže od 60 časova nedeljno.",
        penalty:
          "Prekršaj: novčana kazna od 200.000 do 400.000 dinara (čl. 274 st. 1 tač. 4 u vezi sa st. 2).",
        citation: "Zakon o radu, čl. 57 st. 5. Nadzor: inspektor rada.",
        isLegalDuty: true,
      },
    });

    render(<WorkTimeModule services={services} currentUser={admin} />);

    expect(
      await screen.findByText(/200\.000 do 400\.000 dinara/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/50\.000 do 150\.000 dinara/),
    ).toBeInTheDocument();
  });

  /**
   * A preraspodela breach is refused on čl. 57 st. 5, so the notice beside it
   * must be the čl. 57 one.
   *
   * čl. 58 says preraspodela is not prekovremeni rad: the čl. 53 summary — eight
   * hours of overtime a week, twelve hours a day — states rules that do not bind
   * this employee, and its citation is čl. 274 st. 1 **tač. 3** when čl. 57 and
   * čl. 60 are **tač. 4**. The amount is the same either way, so nothing here is
   * a wrong figure; it is a wrong article handed to an inspector.
   */
  it("quotes čl. 57 st. 5 and tač. 4 when the preraspodela ceiling is the one breached", async () => {
    const user = userEvent.setup();
    const services = mockServices();
    vi.spyOn(services.worktime, "notices").mockResolvedValue({
      recordMissing: {
        summary:
          "Poslodavac je dužan da vodi dnevnu evidenciju o prekovremenom radu zaposlenih.",
        penalty:
          "Prekršaj: novčana kazna od 50.000 do 150.000 dinara (čl. 276 st. 1 u vezi sa tač. 1a).",
        citation: "Zakon o radu, čl. 55 st. 6. Nadzor: inspektor rada.",
        isLegalDuty: true,
      },
      capsExceeded: {
        summary:
          "Prekovremeni rad ne može trajati duže od osam časova nedeljno, niti ukupno radno vreme sa prekovremenim duže od 12 časova dnevno.",
        penalty:
          "Prekršaj: novčana kazna od 200.000 do 400.000 dinara (čl. 274 st. 1 tač. 3 u vezi sa st. 2).",
        citation: "Zakon o radu, čl. 53 st. 2 i st. 3. Nadzor: inspektor rada.",
        isLegalDuty: true,
      },
      preraspodelaCapsExceeded: {
        summary:
          "U slučaju preraspodele radnog vremena, radno vreme ne može da traje duže od 60 časova nedeljno.",
        penalty:
          "Prekršaj: novčana kazna od 200.000 do 400.000 dinara (čl. 274 st. 1 tač. 4 u vezi sa st. 2).",
        citation: "Zakon o radu, čl. 57 st. 5. Nadzor: inspektor rada.",
        isLegalDuty: true,
      },
    });
    vi.spyOn(services.worktime, "saveEntry").mockRejectedValue({
      code: "cap_override_required",
      message:
        "Prekoračen je limit radnog vremena u preraspodeli — 60 časova nedeljno (ZoR čl. 57 st. 5). Dan se može evidentirati, ali morate izabrati razlog prekoračenja.",
      details: {
        caps: {
          ...nulaCaps,
          dailyTotalMinutes: 780,
          weeklyTotalMinutes: 3660,
          preraspodelaWeeklyCapExceeded: true,
          requiresOverride: true,
        },
      },
    });

    render(<WorkTimeModule services={services} currentUser={admin} />);
    await screen.findByText(/zakon ne propisuje obrazac/i);

    await setMinutes(user, /efektivno izvršeni/i, "780");
    await user.click(screen.getByRole("button", { name: /sačuvaj dan/i }));

    const alert = (
      await screen.findByText(/Prekoračen limit radnog vremena/i)
    ).closest<HTMLElement>('[data-slot="alert"]');
    expect(alert).not.toBeNull();

    expect(within(alert!).getByText(/ZoR čl\. 57 st\. 5/)).toBeInTheDocument();
    expect(within(alert!).getByText(/60 časova nedeljno\./)).toBeInTheDocument();
    expect(within(alert!).getByText(/tač\. 4/)).toBeInTheDocument();
    // čl. 58 makes the čl. 53 rule set inapplicable to this employee, and
    // tač. 3 is the wrong offence.
    expect(
      within(alert!).queryByText(/osam časova nedeljno/),
    ).not.toBeInTheDocument();
    expect(within(alert!).queryByText(/tač\. 3/)).not.toBeInTheDocument();
    expect(
      within(alert!).queryByText(/čl\. 53 st\. 2 i st\. 3/),
    ).not.toBeInTheDocument();
  });

  /**
   * A thirteen-hour preraspodela day inside a forty-hour week saves without
   * requiring an override — and, today, without saying anything at all.
   *
   * `assess_caps_for_employee` deliberately leaves `dailyCapExceeded` standing
   * on the SUCCESS path, because the single `radi_u_preraspodeli` flag cannot
   * tell čl. 57 preraspodela (no daily leg) from the čl. 56 st. 3 scheme, where
   * čl. 56 st. 4 expressly restates twelve hours a day. Dropping `saved.caps`
   * here is the only place that finding can be lost — and it is a finding on the
   * čl. 274 side, the larger fine.
   */
  it("surfaces the non-blocking daily-cap finding a successful preraspodela save carries", async () => {
    const user = userEvent.setup();
    const services = mockServices();
    vi.spyOn(services.worktime, "saveEntry").mockResolvedValue({
      entry: entry({ dan: today() }),
      caps: {
        ...nulaCaps,
        dailyTotalMinutes: 780,
        weeklyTotalMinutes: 2400,
        dailyCapExceeded: true,
        requiresOverride: false,
      },
      protections: [],
    });

    render(<WorkTimeModule services={services} currentUser={admin} />);
    await screen.findByText(/zakon ne propisuje obrazac/i);

    await setMinutes(user, /efektivno izvršeni/i, "780");
    await user.click(screen.getByRole("button", { name: /sačuvaj dan/i }));

    const alert = (
      await screen.findByText(/Dnevno radno vreme prelazi 12 časova/i)
    ).closest<HTMLElement>('[data-slot="alert"]');
    expect(alert).not.toBeNull();

    // Both ceilings are named, because the stored profile cannot say which
    // scheme the employee is in — and the operator is told that is the open
    // question rather than handed one answer.
    expect(within(alert!).getByText(/čl\. 53 st\. 3/)).toBeInTheDocument();
    expect(within(alert!).getByText(/čl\. 56 st\. 4/)).toBeInTheDocument();
    expect(within(alert!).getByText(/12 časova dnevno/)).toBeInTheDocument();
    // Non-blocking: the day was recorded, and nothing here may say otherwise.
    expect(screen.queryByText(/Dan nije evidentiran/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/Unos nije dozvoljen/i)).not.toBeInTheDocument();
  });
});

describe("cap notice selection", () => {
  const notices = {
    recordMissing: {
      summary: "Poslodavac je dužan da vodi dnevnu evidenciju.",
      penalty: null,
      citation: "Zakon o radu, čl. 55 st. 6.",
      isLegalDuty: true,
    },
    capsExceeded: {
      summary: "Prekovremeni rad ne može trajati duže od osam časova nedeljno.",
      penalty: null,
      citation: "Zakon o radu, čl. 53 st. 2 i st. 3.",
      isLegalDuty: true,
    },
    preraspodelaCapsExceeded: {
      summary:
        "U slučaju preraspodele radnog vremena, radno vreme ne može da traje duže od 60 časova nedeljno.",
      penalty: null,
      citation: "Zakon o radu, čl. 57 st. 5.",
      isLegalDuty: true,
    },
  };

  it("picks the čl. 57 notice only when the preraspodela ceiling is the breached one", () => {
    expect(
      capNoticeFor(notices, { ...nulaCaps, preraspodelaWeeklyCapExceeded: true }),
    ).toBe(notices.preraspodelaCapsExceeded);
    expect(
      capNoticeFor(notices, { ...nulaCaps, dailyCapExceeded: true }),
    ).toBe(notices.capsExceeded);
  });

  /**
   * An error without a `details.caps` payload must still name a ceiling, and
   * čl. 53 is the one that binds every employee not in preraspodela. Falling
   * back the other way would quote čl. 57 st. 5 — a provision that binds nobody
   * outside preraspodela — at an ordinary twelve-hour breach.
   */
  it("falls back to the čl. 53 notice when no assessment came back", () => {
    expect(capNoticeFor(notices, null)).toBe(notices.capsExceeded);
    expect(capNoticeFor(null, null)).toBeNull();
  });
});

describe("WorkTimeModule protection findings", () => {
  /**
   * čl. 90 is a WARNING, not a block: the statute conditions the prohibition on
   * a nalaz nadležnog zdravstvenog organa, so the backend lets the write through
   * and hands the finding back on the SUCCESS path. If the module discards it,
   * the advisory §4 req. 13 mandates is never seen by anyone.
   */
  it("surfaces a non-blocking čl. 90 finding carried by a successful save", async () => {
    const user = userEvent.setup();
    const services = mockServices();
    vi.spyOn(services.worktime, "saveEntry").mockResolvedValue({
      entry: entry({ dan: today() }),
      caps: { ...nulaCaps },
      protections: [
        {
          kind: "trudnocaNocniIPrekovremeni",
          blocking: false,
          poruka:
            "Zaposlena za vreme trudnoće i zaposlena koja doji dete ne može da radi prekovremeno i noću ako bi takav rad bio štetan za njeno zdravlje i zdravlje deteta (ZoR čl. 90).",
        },
      ],
    });

    render(<WorkTimeModule services={services} currentUser={admin} />);
    await screen.findByText(/zakon ne propisuje obrazac/i);

    await setMinutes(user, /efektivno izvršeni/i, "480");
    await user.click(screen.getByRole("button", { name: /sačuvaj dan/i }));

    expect(await screen.findByText(/ZoR čl\. 90/)).toBeInTheDocument();
    expect(
      screen.getByText(/napomena o zaštiti zaposlenog/i),
    ).toBeInTheDocument();
  });

  /**
   * `NeispravanDatumUProfilu` means the under-18 (čl. 87/88) and čl. 91 consent
   * guards COULD NOT RUN for this day. Swallowing it degrades the register to
   * unguarded without telling anyone.
   */
  it("surfaces the unreadable-profile-date finding a successful save carries", async () => {
    const user = userEvent.setup();
    const services = mockServices();
    vi.spyOn(services.worktime, "saveEntry").mockResolvedValue({
      entry: entry({ dan: today() }),
      caps: { ...nulaCaps },
      protections: [
        {
          kind: "neispravanDatumUProfilu",
          blocking: false,
          poruka:
            "Datum rođenja zaposlenog u profilu nije ispravan datum, pa zaštite za zaposlene mlađe od 18 godina (ZoR čl. 87 i čl. 88) za ovaj dan nisu proverene.",
        },
      ],
    });

    render(<WorkTimeModule services={services} currentUser={admin} />);
    await screen.findByText(/zakon ne propisuje obrazac/i);

    await setMinutes(user, /efektivno izvršeni/i, "480");
    await user.click(screen.getByRole("button", { name: /sačuvaj dan/i }));

    expect(
      await screen.findByText(/nije ispravan datum/i),
    ).toBeInTheDocument();
  });

  /**
   * The čl. 87 weekly leg is the opposite of the two above: the statute states
   * the prohibition itself, so the backend REFUSES the row and the finding comes
   * back on the ERROR path, inside `details.protections`. That branch had no
   * test at all, and a block the operator never sees is a refusal they cannot
   * act on — they would be told only „Dan nije evidentiran“ about a day that
   * looks lawful on its own, since eight hours breaches no daily leg.
   *
   * The fixture is the exact payload `write_entry` serializes, variant name
   * included: `maloletanNedeljniLimit` is a value this module never enumerates,
   * which is the point — it renders every `ProtectionKind` off `blocking` and
   * `poruka`, so a new variant surfaces without a branch being added for it.
   */
  it("surfaces the blocking čl. 87 weekly finding a refused save carries", async () => {
    const user = userEvent.setup();
    const services = mockServices();
    const poruka =
      "Zaposleni mlađi od 18 godina života ne može da radi duže od 35 časova nedeljno (ZoR čl. 87). Za dane ove kalendarske nedelje u kojima je zaposleni mlađi od 18 godina već je evidentirano 35 č 00 min, a sa ovim danom bilo bi 43 č 00 min.";
    vi.spyOn(services.worktime, "saveEntry").mockRejectedValue({
      code: "protection_block",
      message: poruka,
      details: {
        protections: [
          { kind: "maloletanNedeljniLimit", blocking: true, poruka },
        ],
      },
    });

    render(<WorkTimeModule services={services} currentUser={admin} />);
    await screen.findByText(/zakon ne propisuje obrazac/i);

    await setMinutes(user, /efektivno izvršeni/i, "480");
    await user.click(screen.getByRole("button", { name: /sačuvaj dan/i }));

    const alert = (
      await screen.findByText(/Unos nije dozvoljen/i)
    ).closest<HTMLElement>('[data-slot="alert"]');
    expect(alert).not.toBeNull();
    expect(within(alert!).getByText(/35 časova nedeljno/)).toBeInTheDocument();
    expect(within(alert!).getByText(/ZoR čl\. 87/)).toBeInTheDocument();
    // The figures the refusal names: what the week already holds, and what this
    // day would have made it. The register holds neither 43 h nor this day.
    expect(within(alert!).getByText(/već je evidentirano 35 č 00 min/)).toBeInTheDocument();

    // A prohibition must not be dressed as a note — that is the title the čl. 90
    // advisory carries, and it is the wrong one here.
    expect(
      screen.queryByText(/napomena o zaštiti zaposlenog/i),
    ).not.toBeInTheDocument();
    // And the operator is told the day did not go in.
    expect(screen.getByText(/Dan nije evidentiran/i)).toBeInTheDocument();
  });
});

describe("WorkTimeModule day selection", () => {
  it("defaults to the local calendar day, never the UTC one", () => {
    vi.stubEnv("TZ", "Europe/Belgrade");
    vi.useFakeTimers();
    // 00:30 on 1 June in Belgrade is still 31 May in UTC. A `toISOString()`
    // default would open the form on the wrong calendar day, and on the 1st of
    // a month on a day in the previous — possibly already closed — period.
    vi.setSystemTime(new Date("2026-06-01T00:30:00+02:00"));

    try {
      expect(todayIso()).toBe("2026-06-01");
    } finally {
      vi.useRealTimers();
      vi.unstubAllEnvs();
    }
  });

  it("never lets a future day reach the register", async () => {
    const user = userEvent.setup();
    const services = mockServices();
    const saveSpy = vi.spyOn(services.worktime, "saveEntry");
    const now = new Date();

    render(<WorkTimeModule services={services} currentUser={admin} />);
    await screen.findByText(/zakon ne propisuje obrazac/i);
    await awaitLoadedMonth();

    const datum = screen.getByLabelText(/^datum$/i);
    // The picker must not offer a day that has not happened.
    expect(datum).toHaveAttribute("max", today());

    const sutra = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
    fireEvent.change(datum, {
      target: {
        value: `${sutra.getFullYear()}-${`${sutra.getMonth() + 1}`.padStart(2, "0")}-${`${sutra.getDate()}`.padStart(2, "0")}`,
      },
    });
    await setMinutes(user, /efektivno izvršeni/i, "480");
    await user.click(screen.getByRole("button", { name: /sačuvaj dan/i }));

    expect(saveSpy).not.toHaveBeenCalled();
  });

  it("refuses a future day in code, not only in the picker", () => {
    const now = new Date();
    const sutra = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1);
    const sutraIso = `${sutra.getFullYear()}-${`${sutra.getMonth() + 1}`.padStart(2, "0")}-${`${sutra.getDate()}`.padStart(2, "0")}`;

    // Validated against `sutra`'s own period, so only the future rule can fire.
    expect(
      validateDan(sutraIso, sutra.getFullYear(), sutra.getMonth() + 1),
    ).toMatch(/nije protekao/i);
    expect(
      validateDan(today(), now.getFullYear(), now.getMonth() + 1),
    ).toBeNull();
  });

  it("opens on a day inside the period the grid is showing", async () => {
    const user = userEvent.setup();
    const { godina, mesec } = previousPeriod();
    const dvocifreni = `${mesec}`.padStart(2, "0");
    const poslednji = new Date(Date.UTC(godina, mesec, 0)).getUTCDate();

    render(<WorkTimeModule services={mockServices()} currentUser={admin} />);
    await screen.findByText(/zakon ne propisuje obrazac/i);
    await awaitLoadedMonth();

    await user.selectOptions(screen.getByLabelText(/godina/i), String(godina));
    await user.selectOptions(screen.getByLabelText(/mesec/i), String(mesec));

    const datum = screen.getByLabelText(/^datum$/i);
    // The default day belongs to the displayed month, not to today: a row saved
    // outside the period is invisible in the grid, because `listMonth` filters
    // by period.
    expect(datum).toHaveValue(`${godina}-${dvocifreni}-01`);
    expect(datum).toHaveAttribute("min", `${godina}-${dvocifreni}-01`);
    expect(datum).toHaveAttribute("max", `${godina}-${dvocifreni}-${poslednji}`);
  });

  it("tells the backend which period the day was written for", async () => {
    const user = userEvent.setup();
    const services = mockServices();
    const saveSpy = vi.spyOn(services.worktime, "saveEntry");
    const now = new Date();

    render(<WorkTimeModule services={services} currentUser={admin} />);
    await screen.findByText(/zakon ne propisuje obrazac/i);
    await awaitLoadedMonth();

    await setMinutes(user, /efektivno izvršeni/i, "480");
    await user.click(screen.getByRole("button", { name: /sačuvaj dan/i }));

    // The Datum field's own guard protects the operator, not the write: the
    // backend refuses a day that falls outside the period it was written for, and
    // it can only see that mismatch if the request says which period that was.
    await waitFor(() => {
      expect(saveSpy).toHaveBeenLastCalledWith(
        expect.objectContaining({
          dan: today(),
          godina: now.getFullYear(),
          mesec: now.getMonth() + 1,
        }),
      );
    });
  });

  it("refuses a day outside the selected period", () => {
    const { godina, mesec } = previousPeriod();
    const dvocifreni = `${mesec}`.padStart(2, "0");
    const poslednji = new Date(Date.UTC(godina, mesec, 0)).getUTCDate();

    expect(validateDan(`${godina - 1}-${dvocifreni}-01`, godina, mesec)).toMatch(
      /mora pripadati izabranom periodu/i,
    );
    expect(validateDan(`${godina}-${dvocifreni}-01`, godina, mesec)).toBeNull();
    // The last day of the month is inside it — an off-by-one here would refuse a
    // day the register must be able to describe.
    expect(
      validateDan(`${godina}-${dvocifreni}-${poslednji}`, godina, mesec),
    ).toBeNull();
  });
});

describe("WorkTimeModule legal notices", () => {
  it("never files a non-duty notice under „Zakonska osnova“", async () => {
    const services = mockServices();
    vi.spyOn(services.worktime, "notices").mockResolvedValue({
      recordMissing: {
        summary: "Poslodavac je dužan da vodi dnevnu evidenciju.",
        penalty: null,
        citation: "Zakon o radu, čl. 55 st. 6. Nadzor: inspektor rada.",
        isLegalDuty: true,
      },
      capsExceeded: {
        summary: "Preporučuje se čuvanje evidencije uz platne liste.",
        penalty: null,
        citation: "Praksa inspekcije rada, bez propisane obaveze.",
        isLegalDuty: false,
      },
      preraspodelaCapsExceeded: {
        summary:
          "U slučaju preraspodele radnog vremena, radno vreme ne može da traje duže od 60 časova nedeljno.",
        penalty: null,
        citation: "Zakon o radu, čl. 57 st. 5. Nadzor: inspektor rada.",
        isLegalDuty: true,
      },
    });

    render(<WorkTimeModule services={services} currentUser={admin} />);

    const preporuka = (
      await screen.findByText(/Preporučuje se čuvanje evidencije/)
    ).closest("div");
    expect(preporuka).not.toBeNull();
    expect(within(preporuka!).getByText(/preporuka/i)).toBeInTheDocument();
    expect(
      within(preporuka!).queryByText(/zakonska osnova/i),
    ).not.toBeInTheDocument();

    const obaveza = (
      await screen.findByText(/Poslodavac je dužan da vodi dnevnu evidenciju/)
    ).closest("div");
    expect(within(obaveza!).getByText(/zakonska osnova/i)).toBeInTheDocument();
  });

  it("states the advisory columns in grammatical Serbian", async () => {
    render(<WorkTimeModule services={mockServices()} currentUser={admin} />);

    expect(
      await screen.findByText(
        /Ove dve kolone nose oznaku „izračunato radi provere usklađenosti“\./,
      ),
    ).toBeInTheDocument();
  });
});

describe("WorkTimeModule absence reason gate", () => {
  const godisnji = entry({
    id: 7,
    dan: "2026-06-02",
    kategorijaOdsustva: "godisnji_odmor",
    minuti: {
      ...nulaMinuta,
      godisnjiOdmorMinuta: 480,
      ukupnoNeizvrseniMinuta: 480,
    },
  });

  it("shows the absence reason to the payroll role", async () => {
    render(
      <WorkTimeModule
        services={servicesWithMonth(month([godisnji]))}
        currentUser={admin}
      />,
    );

    const row = await screen.findByRole("row", { name: /02\.06\.2026/ });
    expect(within(row).getByText(/godišnji odmor/i)).toBeInTheDocument();
  });

  it("hides the absence reason from a non-payroll role", async () => {
    render(
      <WorkTimeModule
        services={servicesWithMonth(month([godisnji]))}
        currentUser={kasir}
      />,
    );

    const row = await screen.findByRole("row", { name: /02\.06\.2026/ });
    // Only „odsutan" plus the hour total — never the category.
    expect(within(row).getByText(/odsutan/i)).toBeInTheDocument();
    expect(within(row).queryByText(/godišnji odmor/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/godišnji odmor/i)).not.toBeInTheDocument();
  });

  it("keeps the gate closed when the role is unknown", async () => {
    render(<WorkTimeModule services={servicesWithMonth(month([godisnji]))} />);

    const row = await screen.findByRole("row", { name: /02\.06\.2026/ });
    expect(within(row).getByText(/odsutan/i)).toBeInTheDocument();
    expect(within(row).queryByText(/godišnji odmor/i)).not.toBeInTheDocument();
  });
});

describe("WorkTimeModule correction log", () => {
  it("renders the superseded version struck through beside its replacement", async () => {
    const original = entry({
      id: 10,
      dan: "2026-06-03",
      verzija: 1,
      zamenjen: true,
      minuti: { ...nulaMinuta, efektivnoIzvrseniMinuta: 480 },
    });
    const correction = entry({
      id: 11,
      dan: "2026-06-03",
      verzija: 2,
      supersedesId: 10,
      korekcijaRazlog: "greska_u_unosu",
      minuti: { ...nulaMinuta, efektivnoIzvrseniMinuta: 420 },
    });

    render(
      <WorkTimeModule
        services={servicesWithMonth(month([original, correction]))}
        currentUser={admin}
      />,
    );

    const rows = await screen.findAllByRole("row", { name: /03\.06\.2026/ });
    expect(rows).toHaveLength(2);
    expect(rows[0].className).toContain("line-through");
    expect(rows[1].className).not.toContain("line-through");
    expect(within(rows[1]).getByText(/greška u unosu/i)).toBeInTheDocument();
  });
});

describe("WorkTimeModule totals row", () => {
  it("names the day count instead of leaving a bare figure under „Odsustvo“", async () => {
    render(
      <WorkTimeModule
        services={servicesWithMonth(month([entry({ id: 20, dan: "2026-06-04" })]))}
        currentUser={admin}
      />,
    );

    // A single recorded day. „1 dana“ is ungrammatical, and under the
    // „Odsustvo“ header a bare figure reads as one day of absence.
    expect(await screen.findByText("1 dan sa unosom")).toBeInTheDocument();
    expect(screen.queryByText(/^\d+ dana$/)).not.toBeInTheDocument();
  });
});

describe("WorkTimeModule period close", () => {
  it("closes the period and says the close is final", async () => {
    const user = userEvent.setup();
    const services = servicesWithMonth(month([]));
    const closeSpy = vi
      .spyOn(services.worktime, "closePeriod")
      .mockResolvedValue({
        userId: 2,
        godina: 2026,
        mesec: 6,
        closedAt: "2026-07-01T08:00:00Z",
        closedBy: 1,
        klasifikacija: {
          userId: 2,
          godina: 2026,
          mesec: 6,
          danaSaUnosom: 0,
          minuti: { ...nulaMinuta },
          izvedenoU: "2026-07-01T08:00:00Z",
        },
      });

    render(<WorkTimeModule services={services} currentUser={admin} />);
    await screen.findByText(/zakon ne propisuje obrazac/i);

    await user.click(screen.getByRole("button", { name: /zaključi period/i }));
    expect(await screen.findByText(/zaključenje je konačno/i)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /^zaključi$/i }));

    await waitFor(() => {
      expect(closeSpy).toHaveBeenCalled();
    });
  });

  it("locks the entry form once the period is closed", async () => {
    render(
      <WorkTimeModule
        services={servicesWithMonth(
          month([], { zatvoren: true, closedAt: "2026-07-01T08:00:00Z" }),
        )}
        currentUser={admin}
      />,
    );

    expect(await screen.findByText(/period je zaključen/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /sačuvaj dan/i })).toBeDisabled();
    expect(
      screen.queryByRole("button", { name: /zaključi period/i }),
    ).not.toBeInTheDocument();
  });
});
