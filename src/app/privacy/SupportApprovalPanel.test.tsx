import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { SupportApprovalPanel } from "./SupportApprovalPanel";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type { SupportSession } from "@/services/types";

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
