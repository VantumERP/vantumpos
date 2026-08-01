import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { WorkTimeModule } from "./WorkTimeModule";
import { navigationItems } from "@/app/navigation";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type {
  UserAccount,
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

/** The day an entry form defaults to — the module never back-dates for you. */
function today(): string {
  return new Date().toISOString().slice(0, 10);
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
    });

    render(<WorkTimeModule services={services} currentUser={admin} />);

    expect(
      await screen.findByText(/200\.000 do 400\.000 dinara/),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/50\.000 do 150\.000 dinara/),
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
