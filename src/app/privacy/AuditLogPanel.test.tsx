import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { AuditLogPanel } from "./AuditLogPanel";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type { AuditEvent, AuditSearchResult } from "@/services/types";

function event(overrides: Partial<AuditEvent> = {}): AuditEvent {
  return {
    id: 1,
    at: "2026-08-01T09:00:00Z",
    actorUserId: 1,
    actorName: "Administrator",
    action: "uvid",
    actionLabel: "Uvid",
    objectType: "personnel_record",
    objectTypeLabel: "Evidencija o zaposlenom",
    objectId: "7",
    reasonCode: "interni_nadzor",
    reasonLabel: "Interni nadzor",
    recipient: null,
    recipientLabel: null,
    supportSessionId: null,
    prevHash: "0".repeat(64),
    hash: "1".repeat(64),
    ...overrides,
  };
}

function result(
  events: AuditEvent[],
  overrides: Partial<AuditSearchResult["chain"]> = {},
): AuditSearchResult {
  return {
    events,
    chain: {
      verdict: "intact",
      intact: true,
      checkedRows: events.length,
      label: "Potvrđena — lanac otisaka je neprekinut.",
      ...overrides,
    },
  };
}

const dveRadnje = result([
  event({ id: 1 }),
  event({
    id: 2,
    at: "2026-08-01T10:15:00Z",
    action: "otkrivanje",
    actionLabel: "Otkrivanje (uključujući i prenos)",
    objectType: "audit_log",
    objectTypeLabel: "Evidencija pristupa",
    objectId: "1",
    reasonCode: "inspekcija",
    reasonLabel: "Inspekcijski nadzor",
    recipient: "poverenik",
    recipientLabel:
      "Poverenik za informacije od javnog značaja i zaštitu podataka o ličnosti",
    prevHash: "1".repeat(64),
    hash: "2".repeat(64),
  }),
]);

function services(search: AuditSearchResult = dveRadnje): PosServices {
  const posServices = createMockServices();
  vi.spyOn(posServices.privacy, "searchAudit").mockResolvedValue(search);
  return posServices;
}

describe("AuditLogPanel", () => {
  /**
   * The single most dangerous sentence this app could print. Čl. 50 appears
   * nowhere in čl. 95, so the log is a prudential control — but „no fine“ is
   * not „no consequence“, and the Poverenik's corrective powers are live.
   */
  it("never tells the operator that ZZPL requires an audit log", async () => {
    render(<AuditLogPanel services={services()} />);

    expect(
      await screen.findByText(/nije propisan prekršaj/i),
    ).toBeInTheDocument();
    expect(screen.queryByText(/nema posledice/i)).not.toBeInTheDocument();
    expect(
      screen.queryByText(/zakon zahteva.*evidenci/i),
    ).not.toBeInTheDocument();
    expect(screen.queryByText(/zzpl zahteva/i)).not.toBeInTheDocument();
    expect(screen.queryByText(/zakonska obaveza/i)).not.toBeInTheDocument();
  });

  /** Req. 7: tamper-evidence outranks completeness. */
  it("offers no edit or delete affordance on an audit row", async () => {
    render(<AuditLogPanel services={services()} />);

    const table = await screen.findByRole("table");
    expect(within(table).queryAllByRole("button")).toHaveLength(0);
    expect(within(table).queryAllByRole("textbox")).toHaveLength(0);
    expect(
      screen.queryByRole("button", { name: /obriši|izbriši|ukloni|izmeni|uredi/i }),
    ).not.toBeInTheDocument();
  });

  /**
   * §5 item 3: in a three-employee shop the log identifies each cashier by
   * construction, so a per-cashier roll-up is a new purpose under čl. 5 st. 1
   * t. 2. Each name appears once per row it belongs to, and never in a total.
   */
  it("renders no per-cashier aggregate over the logged rows", async () => {
    render(<AuditLogPanel services={services()} />);

    const table = await screen.findByRole("table");
    expect(within(table).getAllByText("Administrator")).toHaveLength(2);
    expect(screen.queryByText(/učinak|produktivnost|broj radnji po/i)).not.toBeInTheDocument();
  });

  /** Req. 8 — the two axes the izvod is filtered on, and no third one. */
  it("passes the date range and the actor to the backend", async () => {
    const posServices = services();
    const search = vi.spyOn(posServices.privacy, "searchAudit");
    const user = userEvent.setup();

    render(<AuditLogPanel services={posServices} />);
    await screen.findByRole("table");

    await user.type(screen.getByLabelText(/^od$/i), "2026-08-01");
    await user.type(screen.getByLabelText(/^do$/i), "2026-08-31");
    await user.selectOptions(screen.getByLabelText(/lice/i), "1");
    await user.click(screen.getByRole("button", { name: /prikaži/i }));

    await waitFor(() => {
      expect(search).toHaveBeenLastCalledWith({
        from: "2026-08-01",
        to: "2026-08-31",
        actorUserId: 1,
      });
    });
  });

  /** The verdict is the backend's, never one this panel recomputes. */
  it("reports the chain verdict the backend returned", async () => {
    const broken = result([event({ id: 1 })], {
      verdict: { brokenAt: 0 },
      intact: false,
      label: "Narušena — prvi neusklađen zapis je 1. po redu u evidenciji.",
    });

    render(<AuditLogPanel services={services(broken)} />);

    expect(
      await screen.findByText(/narušena — prvi neusklađen zapis je 1/i),
    ).toBeInTheDocument();
  });

  /** Req. 8: the izvod renders offline, from the till, and is then printed. */
  it("exports the čl. 48 st. 4 izvod and opens it for printing", async () => {
    const posServices = services();
    const exportCsv = vi
      .spyOn(posServices.privacy, "exportAuditCsv")
      .mockResolvedValue({
        fileName: "izvod-evidencija-pristupa.csv",
        path: "mock://exports/izvod-evidencija-pristupa.csv",
        mimeType: "text/csv",
        rowCount: 2,
      });
    const openForPrint = vi.spyOn(posServices.print, "openForPrint");
    const user = userEvent.setup();

    render(<AuditLogPanel services={posServices} />);
    await screen.findByRole("table");

    await user.click(screen.getByRole("button", { name: /izvod za poverenika/i }));

    await waitFor(() => {
      expect(exportCsv).toHaveBeenCalledWith({
        from: null,
        to: null,
        actorUserId: null,
      });
    });
    await waitFor(() => {
      expect(openForPrint).toHaveBeenCalledWith(
        "mock://exports/izvod-evidencija-pristupa.csv",
      );
    });
  });

  it("says an empty log is empty instead of rendering an empty table", async () => {
    render(<AuditLogPanel services={services(result([]))} />);

    expect(
      await screen.findByText(/nema zapisa za izabrani period/i),
    ).toBeInTheDocument();
    expect(screen.queryByRole("table")).not.toBeInTheDocument();
  });

  it("surfaces a failed read instead of rendering a blank evidencija", async () => {
    const posServices = createMockServices();
    vi.spyOn(posServices.privacy, "searchAudit").mockRejectedValue({
      code: "forbidden",
      message: "Samo administrator može da čita evidenciju pristupa.",
    });

    render(<AuditLogPanel services={posServices} />);

    expect(
      await screen.findByText(/samo administrator može da čita evidenciju pristupa/i),
    ).toBeInTheDocument();
  });
});
