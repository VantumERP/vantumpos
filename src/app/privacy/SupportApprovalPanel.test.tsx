import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SupportApprovalPanel } from "./SupportApprovalPanel";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type { SupportSession, UserAccount } from "@/services/types";

const vlasnik: UserAccount = {
  id: 1,
  username: "admin",
  displayName: "Administrator",
  role: "admin",
  active: true,
  createdAt: "2026-08-01T08:00:00Z",
  updatedAt: "2026-08-01T08:00:00Z",
  lastLoginAt: "2026-08-01T08:00:00Z",
};

const kasirka: UserAccount = {
  ...vlasnik,
  id: 7,
  username: "jelena",
  displayName: "Jelena Đurić",
  role: "cashier",
};

function session(overrides: Partial<SupportSession> = {}): SupportSession {
  return {
    id: 1,
    grantedBy: 1,
    grantedByName: "Administrator",
    grantedAt: "2026-08-01T09:00:00Z",
    scope: "Pregled greške na štampi fiskalnog isečka",
    expiresAt: "2026-08-01T10:00:00Z",
    startedAt: null,
    endedAt: null,
    revokedAt: null,
    odsustvoOtkrivenoAt: null,
    ...overrides,
  };
}

function services(active: SupportSession | null = null): PosServices {
  const posServices = createMockServices();
  vi.spyOn(posServices.privacy, "activeSupportSession").mockResolvedValue(active);
  return posServices;
}

describe("SupportApprovalPanel", () => {
  /**
   * Čl. 46 is the operative and the penalised article here. The nalog is the
   * legal duty; the log beside it is the prudential control — and the copy must
   * never swap them round.
   */
  it("names čl. 46 as the duty and never presents the log as one", async () => {
    render(<SupportApprovalPanel services={services()} />);

    expect(await screen.findByText(/čl\. 46/i)).toBeInTheDocument();
    expect(
      screen.queryByText(/zakon zahteva.*evidenci/i),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/čl\. 50/i)).not.toBeInTheDocument();
  });

  it("says plainly that nothing is authorised when no nalog is live", async () => {
    render(<SupportApprovalPanel services={services()} />);

    expect(
      await screen.findByText(/nema izdatog naloga za pristup tehničke podrške/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("button", { name: /okončaj nalog/i }),
    ).not.toBeInTheDocument();
  });

  /** Req. 2 — an explicit obim and an explicit trajanje, both recorded. */
  it("issues a nalog with the obim and the trajanje the vlasnik typed", async () => {
    const posServices = services();
    const grant = vi
      .spyOn(posServices.privacy, "grantSupportAccess")
      .mockResolvedValue(session());
    const user = userEvent.setup();

    render(<SupportApprovalPanel services={posServices} />);
    await screen.findByLabelText(/obim pristupa/i);

    await user.type(
      screen.getByLabelText(/obim pristupa/i),
      "Pregled greške na štampi fiskalnog isečka",
    );
    await user.clear(screen.getByLabelText(/trajanje/i));
    await user.type(screen.getByLabelText(/trajanje/i), "60");
    await user.click(screen.getByRole("button", { name: /izdaj nalog/i }));

    await waitFor(() => {
      expect(grant).toHaveBeenCalledWith(
        "Pregled greške na štampi fiskalnog isečka",
        60,
      );
    });
  });

  it("refuses to send a nalog without an obim", async () => {
    const posServices = services();
    const grant = vi.spyOn(posServices.privacy, "grantSupportAccess");
    const user = userEvent.setup();

    render(<SupportApprovalPanel services={posServices} />);
    await screen.findByLabelText(/obim pristupa/i);

    await user.click(screen.getByRole("button", { name: /izdaj nalog/i }));

    expect(
      await screen.findByText(/upišite šta se tehničkoj podršci odobrava/i),
    ).toBeInTheDocument();
    expect(grant).not.toHaveBeenCalled();
  });

  /** The four facts that ARE the nalog: who, when, what, until when. */
  it("shows who signed the live nalog, when, for what and until when", async () => {
    render(<SupportApprovalPanel services={services(session())} />);

    expect(await screen.findByText("Administrator")).toBeInTheDocument();
    expect(
      screen.getByText("Pregled greške na štampi fiskalnog isečka"),
    ).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /okončaj nalog/i })).toBeEnabled();
  });

  it("ends the live nalog on the vlasnik's word", async () => {
    const posServices = services(session());
    const end = vi
      .spyOn(posServices.privacy, "endSupportSession")
      .mockResolvedValue(session({ revokedAt: "2026-08-01T09:30:00Z" }));
    const user = userEvent.setup();

    render(<SupportApprovalPanel services={posServices} />);
    await user.click(await screen.findByRole("button", { name: /okončaj nalog/i }));

    await waitFor(() => {
      expect(end).toHaveBeenCalled();
    });
    expect(
      await screen.findByText(/nema izdatog naloga za pristup tehničke podrške/i),
    ).toBeInTheDocument();
  });

  it("surfaces a refused grant instead of pretending a nalog was issued", async () => {
    const posServices = services();
    vi.spyOn(posServices.privacy, "grantSupportAccess").mockRejectedValue({
      code: "forbidden",
      message: "Samo administrator može da izda nalog.",
    });
    const user = userEvent.setup();

    render(<SupportApprovalPanel services={posServices} />);
    await user.type(await screen.findByLabelText(/obim pristupa/i), "Provera štampe");
    await user.click(screen.getByRole("button", { name: /izdaj nalog/i }));

    expect(
      await screen.findByText(/samo administrator može da izda nalog/i),
    ).toBeInTheDocument();
  });
});

/**
 * Req. 28's third limb — *„unless the shop explicitly unmasks for that session“*.
 *
 * The control belongs beside the nalog because it is a property OF the nalog:
 * `support_sessions.odsustvo_otkriveno_at` is a per-nalog stamp, the backend
 * refuses an unmask with no live nalog, and the disclosure dies with the nalog.
 * Putting it on the working-time screen would make it look like a view setting.
 */
describe("SupportApprovalPanel absence-reason unmask", () => {
  /** Any affordance that would claim the disclosure can be taken back. */
  const PONOVO_SAKRIJ = /sakrij|maskiraj|vrati pod|opozovi otkrivanje/i;

  it("offers the unmask to the vlasnik only", async () => {
    const posServices = services(session());
    const reveal = vi.spyOn(posServices.privacy, "revealAbsenceReason");

    const { unmount } = render(
      <SupportApprovalPanel services={posServices} currentUser={kasirka} />,
    );
    await screen.findByText("Administrator");

    expect(
      screen.queryByRole("button", { name: /otkrij razlog odsustva/i }),
    ).not.toBeInTheDocument();
    expect(reveal).not.toHaveBeenCalled();
    unmount();

    // Absent means closed: a gate that opens when the caller forgets to wire the
    // role is not a gate.
    const { unmount: unmountBezUloge } = render(
      <SupportApprovalPanel services={services(session())} />,
    );
    await screen.findByText("Administrator");
    expect(
      screen.queryByRole("button", { name: /otkrij razlog odsustva/i }),
    ).not.toBeInTheDocument();
    unmountBezUloge();

    render(<SupportApprovalPanel services={services(session())} currentUser={vlasnik} />);
    expect(
      await screen.findByRole("button", { name: /otkrij razlog odsustva/i }),
    ).toBeEnabled();
  });

  it("states that the disclosure cannot be taken back and that it is recorded", async () => {
    render(<SupportApprovalPanel services={services(session())} currentUser={vlasnik} />);
    await screen.findByRole("button", { name: /otkrij razlog odsustva/i });

    // Irreversible for this nalog — the operator who has read the category does
    // not unread it, so the control must not be presented as a toggle.
    expect(screen.getByText(/ne može da se povuče/i)).toBeInTheDocument();
    // …and logged, which is the čl. 48 half req. 28 sends to SW-10.
    expect(screen.getByText(/evidenciju pristupa/i)).toBeInTheDocument();
  });

  /**
   * The copy the vlasnik reads while deciding must not overstate the mask.
   *
   * `worktime::my_hours` calls `load_month(.., true)` unconditionally — Task 3's
   * deliberate čl. 26 carve-out — so while a nalog is live an employee opening
   * „Moji sati“ reads their own category out of the database and shows it to
   * whoever is mirroring the screen. A panel that says the datum „se ne čita iz
   * baze“ full stop is telling the vlasnik the obrađivač cannot reach it, which
   * is the one direction it is worst to be wrong in: this is the surface where
   * the čl. 46 decision to widen the processing is actually made. The čl. 23
   * notice already carries the carve-out; the panel is the copy that dropped it.
   */
  it("does not deny the „Moji sati“ carve-out the backend keeps open", async () => {
    render(<SupportApprovalPanel services={services(session())} currentUser={vlasnik} />);
    await screen.findByRole("button", { name: /otkrij razlog odsustva/i });

    const paragraf = screen.getByText(/moji sati/i).textContent ?? "";
    // The sentence, not the phrase. Until 09.08.2026 this test asserted only
    // that „moji sati“ appears somewhere on the panel, which the exact inverse
    // („Pregled „Moji sati“ se takođe skriva dok nalog važi“) satisfies just as
    // well — the guard was blind to what the sentence says about its subject.
    // Sentences are split on a full stop followed by a capital or an opening
    // Serbian quote, so „čl. 26“ does not read as a boundary.
    const recenica = paragraf
      .split(/(?<=\.)\s+(?=[A-ZĐŠČĆŽ„])/)
      .find((deo) => /moji sati/i.test(deo));
    expect(recenica).toBeDefined();

    // The claim itself: a withholding verb applied to „Moji sati“, negated.
    const SAKRIVANJE = /(skriva|prikazuje|prikazuju|čita|čitaju)/i;
    const bezNegacija = (recenica ?? "").replace(
      /ne\s+(skriva|prikazuje|prikazuju|čita|čitaju)/gi,
      "«negirano»",
    );
    expect(recenica).toMatch(/ne\s+(skriva|prikazuje|prikazuju|čita|čitaju)/i);
    // …and nothing left over that withholds it after the negations are struck:
    // `worktime.rs::my_hours` calls `load_month(.., true)` unconditionally, so a
    // panel that said this screen is masked would deny a čl. 26 carve-out the
    // backend deliberately keeps open — on the surface where the vlasnik decides
    // whether to widen the čl. 46 processing.
    expect(bezNegacija).not.toMatch(SAKRIVANJE);
  });

  it("offers no re-mask affordance, because there is no re-mask verb", async () => {
    render(<SupportApprovalPanel services={services(session())} currentUser={vlasnik} />);
    await screen.findByRole("button", { name: /otkrij razlog odsustva/i });

    expect(screen.queryAllByRole("button", { name: PONOVO_SAKRIJ })).toHaveLength(0);
  });

  it("reveals through the port and then stops offering the control", async () => {
    const posServices = services(session());
    const reveal = vi
      .spyOn(posServices.privacy, "revealAbsenceReason")
      .mockResolvedValue(
        session({ odsustvoOtkrivenoAt: "2026-08-01T09:20:00Z" }),
      );
    const user = userEvent.setup();

    render(<SupportApprovalPanel services={posServices} currentUser={vlasnik} />);
    await user.click(
      await screen.findByRole("button", { name: /otkrij razlog odsustva/i }),
    );

    await waitFor(() => {
      expect(reveal).toHaveBeenCalled();
    });
    // One disclosure per nalog: the button is gone rather than repeatable, and
    // the stamp is on screen so the vlasnik can see what was done and when.
    await waitFor(() => {
      expect(
        screen.queryByRole("button", { name: /otkrij razlog odsustva/i }),
      ).not.toBeInTheDocument();
    });
    expect(screen.getByText(/razlog odsustva je otkriven/i)).toBeInTheDocument();
    expect(screen.queryAllByRole("button", { name: PONOVO_SAKRIJ })).toHaveLength(0);
    // …and the panel must stop saying the category is withheld, because for THIS
    // nalog it no longer is: `razlog_odsustva_dostupan` returns true the moment
    // `odsustvo_otkriveno_at` is set, so `list_month` and `export_month_csv`
    // carry `kategorija_odsustva` again. Leaving the withholding paragraph above
    // the „otkriven“ alert makes the panel state the mask and its removal in the
    // same breath, on the one surface where the čl. 46 decision is made.
    expect(screen.queryByText(/ne čita iz baze/i)).not.toBeInTheDocument();
    expect(screen.getByText(/ponovo prikazuje u pregledu i izvozu/i)).toBeInTheDocument();
  });

  it("shows the refusal rather than claiming a disclosure that did not happen", async () => {
    const posServices = services(session());
    vi.spyOn(posServices.privacy, "revealAbsenceReason").mockRejectedValue({
      code: "support_bez_naloga_za_otkrivanje",
      message:
        "Nema važećeg naloga za pristup tehničke podrške, pa nema šta da se otkrije.",
    });
    const user = userEvent.setup();

    render(<SupportApprovalPanel services={posServices} currentUser={vlasnik} />);
    await user.click(
      await screen.findByRole("button", { name: /otkrij razlog odsustva/i }),
    );

    expect(
      await screen.findByText(/nema važećeg naloga za pristup tehničke podrške/i),
    ).toBeInTheDocument();
    expect(
      screen.queryByText(/razlog odsustva je otkriven/i),
    ).not.toBeInTheDocument();
  });

  it("says nothing about a live mask when there is no nalog to unmask", async () => {
    render(<SupportApprovalPanel services={services()} currentUser={vlasnik} />);
    await screen.findByText(/nema izdatog naloga za pristup tehničke podrške/i);

    expect(
      screen.queryByRole("button", { name: /otkrij razlog odsustva/i }),
    ).not.toBeInTheDocument();
  });
});
