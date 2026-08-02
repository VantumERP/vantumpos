import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { CenovnikPanel } from "./CenovnikPanel";
import { Toaster } from "@/components/ui/sonner";
import type { CenovnikService } from "@/services/ports";
import type {
  CenovnikPublishTarget,
  CenovnikSnapshot,
  CenovnikSnapshotDetail,
  LegalNotice,
} from "@/services/types";

/**
 * Verbatim from `src-tauri/src/legal.rs::cenovnik_not_published`, the
 * preduzetnik arm. The panel must render whatever the backend hands it — the
 * fixed 100.000 of ZZP čl. 210 st. 3, halved on payment within eight days — and
 * the pravno-lice sum of st. 1 must never reach this shop. Copied here for the
 * same reason `RegisterScreen.test.tsx` copies the AML tier: the assertion is
 * about what the panel does with the string, and the string itself is authored
 * in `legal.rs` and nowhere else.
 */
const PREDUZETNIK_PENALTY =
  "Prekršaj: novčana kazna u fiksnom iznosu od 100.000 dinara (čl. 210 st. 3). " +
  "Plaćanjem polovine — 50.000 dinara — u roku od osam dana od prijema " +
  "prekršajnog naloga prihvata se odgovornost za prekršaj i oslobađa plaćanja " +
  "druge polovine (Zakon o prekršajima, čl. 173 st. 1).";

const SUMMARY =
  "Trgovac je dužan da na svojoj internet stranici, posebno za svaki prodajni " +
  "objekat, objavi cenovnik u digitalnom obliku pogodnom za automatsku obradu " +
  "i da ga ažurira u realnom vremenu. U cenovniku se, kao i na prodajnom " +
  "mestu, ističu prodajna i jedinična cena. Zakon nigde ne propisuje obavezu " +
  "trgovca da ima internet stranicu, pa za trgovca koji je nema nije " +
  "razjašnjeno da li je dužan da je izradi.";

const CITATION =
  "Zakon o zaštiti potrošača (Sl. glasnik RS, br. 35/2026), čl. 6 st. 1–3; " +
  "prekršaj: čl. 210 st. 1 tač. 1. Zastarelost: čl. 213, dve godine od izvršenja.";

const OLDER_BODY =
  "﻿sifra;barkod;naziv;jedinica_mere;prodajna_cena;jedinicna_cena;" +
  "jedinica_za_jedinicnu_cenu;datum_azuriranja\r\n" +
  'MLEKO-1L;"8600000000010";Mleko 1 l;kom;154.99;154.99;l;17-06-2026\r\n';

const CURRENT_BODY =
  "﻿sifra;barkod;naziv;jedinica_mere;prodajna_cena;jedinicna_cena;" +
  "jedinica_za_jedinicnu_cenu;datum_azuriranja\r\n" +
  'MLEKO-1L;"8600000000010";Mleko 1 l;kom;159.99;159.99;l;18-06-2026\r\n';

function snapshot(overrides: Partial<CenovnikSnapshot> = {}): CenovnikSnapshot {
  return {
    id: 1,
    prodajnoMesto: "Bulevar 1, Beograd",
    generatedAt: "2026-06-17T08:30:00Z",
    rowCount: 1,
    contentHash: "a1b2c3d4e5f60718293a4b5c6d7e8f90",
    publishedAt: null,
    publishedTarget: null,
    current: false,
    ...overrides,
  };
}

const older = snapshot();
const current = snapshot({
  id: 2,
  generatedAt: "2026-06-18T09:45:00Z",
  contentHash: "ff00ee11dd22cc33bb44aa5566778899",
  current: true,
});

interface DoubleOptions {
  target?: CenovnikPublishTarget;
  snapshots?: CenovnikSnapshot[];
  penalty?: string | null;
  setPublishTarget?: CenovnikService["setPublishTarget"];
}

function cenovnikDouble({
  target = { kind: "notConfigured" },
  snapshots = [current, older],
  penalty = PREDUZETNIK_PENALTY,
  setPublishTarget,
}: DoubleOptions = {}): { cenovnik: CenovnikService } {
  let stored = target;

  return {
    cenovnik: {
      async listSnapshots() {
        return snapshots;
      },
      async getSnapshot(snapshotId): Promise<CenovnikSnapshotDetail | null> {
        const found = snapshots.find((row) => row.id === snapshotId);

        return found
          ? { snapshot: found, body: found.current ? CURRENT_BODY : OLDER_BODY }
          : null;
      },
      async getPublishTarget() {
        return stored;
      },
      setPublishTarget:
        setPublishTarget ??
        (async (next) => {
          stored = next;
          return stored;
        }),
      async getNotice(): Promise<LegalNotice> {
        return {
          summary: SUMMARY,
          penalty,
          citation: CITATION,
          isLegalDuty: true,
        };
      },
    },
  };
}

function renderPanel(props: { cenovnik: CenovnikService }) {
  render(
    <>
      <CenovnikPanel {...props} />
      <Toaster />
    </>,
  );
}

const notConfigured = cenovnikDouble();
const preduzetnik = cenovnikDouble();

describe("CenovnikPanel", () => {
  it("says plainly that no publishing target is configured, without asserting breach", async () => {
    renderPanel(notConfigured);

    expect(
      await screen.findByText(/nije podešeno mesto objave/i),
    ).toBeInTheDocument();
    expect(screen.queryByText(/u prekršaju|kršite/i)).not.toBeInTheDocument();
  });

  it("shows the preduzetnik figure and never the pravno-lice one", async () => {
    renderPanel(preduzetnik);

    expect(await screen.findByText(/100\.000/)).toBeInTheDocument();
    expect(screen.queryByText(/200\.000/)).not.toBeInTheDocument();
  });

  it("does not claim exposure since 1 May 2026", async () => {
    renderPanel(cenovnikDouble());

    await screen.findByText(/nije podešeno mesto objave/i);
    expect(screen.queryByText(/od 1\. maja 2026.*kazn/i)).not.toBeInTheDocument();
  });

  it("lets the operator browse prior snapshots by date", async () => {
    const user = userEvent.setup();
    renderPanel(cenovnikDouble());

    // Newest first, and only the newest is the file čl. 6 st. 4 binds the shop
    // to — an archived one must never be presented as the one in force.
    const rows = await screen.findAllByRole("row");
    const [, newest, prior] = rows;
    expect(within(newest).getByText(/važeći/i)).toBeInTheDocument();
    expect(within(prior).queryByText(/važeći/i)).not.toBeInTheDocument();

    // Each publication is identified by WHEN it was made — that is the axis
    // čl. 6 st. 5 asks the trader to enable a comparison along.
    expect(within(newest).getByText(/\d{1,2}\.\s?\d{1,2}\.\s?2026/)).toBeInTheDocument();
    expect(within(prior).getByText(/\d{1,2}\.\s?\d{1,2}\.\s?2026/)).toBeInTheDocument();

    await user.click(within(prior).getByRole("button", { name: /prikaži/i }));

    // The prior file, byte for byte as it was published — 154,99 is the price
    // that file carries and 159,99 is the one the current file carries.
    const file = await screen.findByLabelText(/sadržaj objavljenog cenovnika/i);
    expect(file).toHaveTextContent("154.99");
    expect(file).not.toHaveTextContent("159.99");
  });

  it("points at Podešavanja → Profil instead of guessing a tier when the legal form is unset", async () => {
    renderPanel(cenovnikDouble({ penalty: null }));

    expect(await screen.findByText(/Podešavanja → Profil/)).toBeInTheDocument();
    expect(screen.queryByText(/100\.000/)).not.toBeInTheDocument();
    expect(screen.queryByText(/200\.000/)).not.toBeInTheDocument();
  });

  it("does not present a local folder as publication on the internet", async () => {
    renderPanel(
      cenovnikDouble({
        target: { kind: "localFolder", folder: "/Users/ana/sajt/cenovnik" },
      }),
    );

    expect(await screen.findByText("/Users/ana/sajt/cenovnik")).toBeInTheDocument();
    // The folder is where the file lands; whether the shop's site serves it is
    // outside this program, and saying otherwise would promise behaviour the
    // code does not implement.
    expect(
      screen.getByText(/samo ako je to folder koji vaš sajt zaista objavljuje/i),
    ).toBeInTheDocument();
  });

  it("saves a folder as the publishing target and shows the refusal verbatim", async () => {
    const user = userEvent.setup();
    const setPublishTarget = vi.fn(async (next: CenovnikPublishTarget) => next);
    renderPanel(cenovnikDouble({ setPublishTarget }));

    const field = await screen.findByLabelText(/folder za objavu/i);
    await user.type(field, "/Users/ana/sajt/cenovnik");
    await user.click(screen.getByRole("button", { name: /sačuvaj mesto objave/i }));

    expect(setPublishTarget).toHaveBeenCalledWith({
      kind: "localFolder",
      folder: "/Users/ana/sajt/cenovnik",
    });
    expect(
      await screen.findByText("/Users/ana/sajt/cenovnik"),
    ).toBeInTheDocument();
  });

  it("shows a backend refusal verbatim rather than a generic sentence", async () => {
    const user = userEvent.setup();
    const setPublishTarget = vi.fn(async () => {
      throw {
        code: "validation_error",
        message:
          "Putanja do foldera mora biti puna putanja, na primer „/Users/ana/cenovnik“.",
      };
    });
    renderPanel(
      cenovnikDouble({
        setPublishTarget: setPublishTarget as unknown as CenovnikService["setPublishTarget"],
      }),
    );

    const field = await screen.findByLabelText(/folder za objavu/i);
    await user.type(field, "cenovnik");
    await user.click(screen.getByRole("button", { name: /sačuvaj mesto objave/i }));

    expect(
      await screen.findByText(/mora biti puna putanja/i),
    ).toBeInTheDocument();
  });

  it("offers no publish-now button, because publication follows the price write", async () => {
    renderPanel(cenovnikDouble());

    await screen.findByText(/nije podešeno mesto objave/i);
    // Čl. 6 st. 3 wants the file to match the outlet's current prices „u
    // realnom vremenu“, so publication rides on the write that moved a price. A
    // button here would be a second answer to „when did the shop last publish“.
    expect(
      screen.queryByRole("button", { name: /objavi (sada|cenovnik)/i }),
    ).not.toBeInTheDocument();
    expect(
      screen.getByText(/posle svake izmene cene/i),
    ).toBeInTheDocument();
  });

  it("says the archive is empty rather than showing an empty table", async () => {
    renderPanel(cenovnikDouble({ snapshots: [] }));

    expect(
      await screen.findByText(/nijedan cenovnik još nije napravljen/i),
    ).toBeInTheDocument();
  });

  it("says a snapshot was archived but not published when no target accepted it", async () => {
    renderPanel(cenovnikDouble());

    const rows = await screen.findAllByRole("row");
    expect(within(rows[1]).getByText(/nije objavljen/i)).toBeInTheDocument();
  });

  it("surfaces a failed load instead of rendering an empty archive", async () => {
    const { cenovnik } = cenovnikDouble();
    const failing: CenovnikService = {
      ...cenovnik,
      listSnapshots: async () => {
        throw { code: "internal", message: "Arhiva cenovnika nije pročitana." };
      },
    };
    renderPanel({ cenovnik: failing });

    await waitFor(() =>
      expect(
        screen.getByText(/Arhiva cenovnika nije pročitana\./),
      ).toBeInTheDocument(),
    );
  });
});
