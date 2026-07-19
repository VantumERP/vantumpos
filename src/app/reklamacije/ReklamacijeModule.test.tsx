import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ReklamacijeModule } from "./ReklamacijeModule";
import { navigationItems } from "@/app/navigation";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type { ReklamacijaSummary, ReklamacijaView } from "@/services/types";

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
};

function servicesWith(rows: ReklamacijaSummary[]): PosServices {
  const services = createMockServices();
  vi.spyOn(services.reklamacije, "list").mockResolvedValue(rows);
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
