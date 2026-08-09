import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";

import { MyHoursPanel, MyHoursScreen } from "./MyHoursPanel";
import { navigationItems } from "@/app/navigation";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type {
  WorkTimeEntryView,
  WorkTimeMinutes,
  WorkTimeMonth,
} from "@/services/types";

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

function month(
  entries: WorkTimeEntryView[],
  overrides: Partial<WorkTimeMonth> = {},
): WorkTimeMonth {
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
    // `my_hours` masks nothing — the viewer is the data subject (§4 req. 25's
    // carve-out) — so the read's own flag is false here whatever nalog is live.
    razlogOdsustvaSkriven: false,
    entries,
    ukupno,
    napomena:
      "Evidencija prekovremenog rada — ZoR čl. 55 st. 6. Zakon ne propisuje obrazac.",
    advisoryNapomena: "izračunato radi provere usklađenosti",
    ...overrides,
  };
}

/** A single ordinary worked day: 7 h 30 min effective, 1 h overtime. */
const fixture = month([
  entry({
    id: 1,
    dan: "2026-06-01",
    minuti: {
      ...nulaMinuta,
      moguciMinuta: 480,
      ukupnoOstvareniMinuta: 510,
      efektivnoIzvrseniMinuta: 450,
      prekovremeniMinuta: 60,
      nocniMinuta: 120,
      radNaPraznikMinuta: 90,
    },
  }),
]);

describe("navigation", () => {
  it("exposes Moji sati to every role, not only to the admin", () => {
    const item = navigationItems.find(
      (candidate) => candidate.id === "moji-sati",
    );

    expect(item).toMatchObject({ label: "Moji sati" });
    expect(item && "adminOnly" in item ? item.adminOnly : undefined).toBeFalsy();
  });
});

describe("MyHoursPanel", () => {
  it("is read-only", async () => {
    render(<MyHoursPanel hours={fixture} />);

    expect(
      screen.queryByRole("button", { name: /sačuvaj|izmeni/i }),
    ).not.toBeInTheDocument();
    // Nothing at all is writable here: no control of any kind, not merely no
    // control that happens to be named „Sačuvaj“.
    expect(screen.queryAllByRole("button")).toHaveLength(0);
    expect(screen.queryAllByRole("textbox")).toHaveLength(0);
    expect(screen.queryAllByRole("combobox")).toHaveLength(0);
    expect(screen.queryAllByRole("spinbutton")).toHaveLength(0);
  });

  it("states that no obrazac is prescribed", async () => {
    render(<MyHoursPanel hours={fixture} />);

    expect(
      screen.getByText(/zakon ne propisuje obrazac/i),
    ).toBeInTheDocument();
  });

  it("never labels what it shows a propisani obrazac", () => {
    render(<MyHoursPanel hours={fixture} />);

    expect(screen.queryByText(/propisani obrazac/i)).not.toBeInTheDocument();
    expect(
      screen.queryByText(/propisana evidencija o zaradama/i),
    ).not.toBeInTheDocument();
  });

  it("shows no ZEOR figure anywhere", () => {
    render(<MyHoursPanel hours={fixture} />);

    expect(screen.queryByText(/500\.000 do 1\.000\.000/)).not.toBeInTheDocument();
    expect(screen.queryByText(/ZEOR/)).not.toBeInTheDocument();
    expect(screen.queryByText(/dinara/i)).not.toBeInTheDocument();
  });

  it("names the two computed columns as computed, never as statutory fields", () => {
    render(<MyHoursPanel hours={fixture} />);

    expect(
      screen.getByText(/izračunato radi provere usklađenosti/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/noćni časovi.*zakonom propisano/i),
    ).not.toBeInTheDocument();
  });

  it("renders hours as časovi and minuti, never as a decimal hour", () => {
    render(<MyHoursPanel hours={fixture} />);

    expect(screen.getAllByText("7 č 30 min").length).toBeGreaterThan(0);
    expect(screen.queryByText(/7[.,]5/)).not.toBeInTheDocument();
  });

  it("names the day count instead of leaving a bare figure under „Odsustvo“", () => {
    const { unmount } = render(<MyHoursPanel hours={fixture} />);

    // One recorded day. „1 dana“ is both ungrammatical and, sitting under the
    // „Odsustvo“ header, reads as one day of absence — on the one column that
    // carries čl. 17 data.
    expect(screen.getByText("1 dan sa unosom")).toBeInTheDocument();
    expect(screen.queryByText(/^\d+ dana$/)).not.toBeInTheDocument();
    unmount();

    render(
      <MyHoursPanel
        hours={month([
          entry({ id: 1, dan: "2026-06-01" }),
          entry({ id: 2, dan: "2026-06-02" }),
          entry({ id: 3, dan: "2026-06-03" }),
          entry({ id: 4, dan: "2026-06-04" }),
          entry({ id: 5, dan: "2026-06-05" }),
        ])}
      />,
    );

    expect(screen.getByText("5 dana sa unosom")).toBeInTheDocument();
  });

  it("keeps a superseded version visible rather than hiding the correction", () => {
    const corrected = month([
      entry({
        id: 1,
        dan: "2026-06-02",
        verzija: 1,
        zamenjen: true,
        minuti: { ...nulaMinuta, efektivnoIzvrseniMinuta: 480 },
      }),
      entry({
        id: 2,
        dan: "2026-06-02",
        verzija: 2,
        supersedesId: 1,
        korekcijaRazlog: "greska_u_unosu",
        minuti: { ...nulaMinuta, efektivnoIzvrseniMinuta: 420 },
      }),
    ]);

    render(<MyHoursPanel hours={corrected} />);

    // Two versions of the same day, the earlier one still rendered. The
    // superseded figure appears once — its own row — while the live one appears
    // twice, in its row and in the month total: visible, and not double-counted.
    expect(screen.getAllByText("02.06.2026.")).toHaveLength(2);
    expect(screen.getAllByText("8 č 00 min")).toHaveLength(1);
    expect(screen.getAllByText("7 č 00 min")).toHaveLength(2);
    expect(screen.getByText(/greška u unosu/i)).toBeInTheDocument();
  });

  it("states the two rights the view discharges", () => {
    render(<MyHoursPanel hours={fixture} />);

    expect(screen.getByText(/čl\. 83 st\. 1/i)).toBeInTheDocument();
    expect(screen.getByText(/ZZPL čl\. 26/i)).toBeInTheDocument();
  });

  it("shows the employee their own absence category", () => {
    const odsustvo = month([
      entry({
        id: 3,
        dan: "2026-06-03",
        kategorijaOdsustva: "godisnji_odmor",
        minuti: { ...nulaMinuta, moguciMinuta: 480, godisnjiOdmorMinuta: 480 },
      }),
    ]);

    render(<MyHoursPanel hours={odsustvo} />);

    expect(screen.getByText(/godišnji odmor/i)).toBeInTheDocument();
    // Never the raw storage token.
    expect(screen.queryByText("godisnji_odmor")).not.toBeInTheDocument();
  });

  it("marks a closed period", () => {
    render(<MyHoursPanel hours={month([], { zatvoren: true })} />);

    expect(screen.getByText(/period je zaključen/i)).toBeInTheDocument();
  });

  it("says a month is empty instead of rendering an empty table", () => {
    render(<MyHoursPanel hours={month([])} />);

    expect(screen.getByText(/nema unosa za izabrani mesec/i)).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });
});

describe("MyHoursScreen", () => {
  function services(): PosServices {
    return createMockServices();
  }

  it("reads only the caller's own hours — there is no employee parameter", async () => {
    const posServices = services();
    const myHours = vi
      .spyOn(posServices.worktime, "myHours")
      .mockResolvedValue(fixture);
    const listMonth = vi.spyOn(posServices.worktime, "listMonth");

    render(<MyHoursScreen services={posServices} />);

    await waitFor(() => {
      expect(myHours).toHaveBeenCalled();
    });
    // `listMonth` takes a `userId`; reaching for it here would be the one way
    // this surface could be pointed at a colleague.
    expect(listMonth).not.toHaveBeenCalled();
    expect(
      await screen.findByText(/zakon ne propisuje obrazac/i),
    ).toBeInTheDocument();
  });

  it("offers no employee picker", async () => {
    const posServices = services();
    vi.spyOn(posServices.worktime, "myHours").mockResolvedValue(fixture);

    render(<MyHoursScreen services={posServices} />);

    await screen.findByText(/zakon ne propisuje obrazac/i);
    expect(screen.queryByLabelText(/zaposleni/i)).not.toBeInTheDocument();
  });

  it("surfaces a failed read instead of rendering a blank month", async () => {
    const posServices = services();
    vi.spyOn(posServices.worktime, "myHours").mockRejectedValue({
      code: "unauthorized",
      message: "Prijavite se ponovo.",
    });

    render(<MyHoursScreen services={posServices} />);

    expect(await screen.findByText(/prijavite se ponovo/i)).toBeInTheDocument();
  });
});
