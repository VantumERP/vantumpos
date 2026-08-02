import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { Cl47RegisterPanel } from "./Cl47RegisterPanel";
import { createMockServices } from "@/services/mock-adapter";
import type { PosServices } from "@/services/ports";
import type { ProcessingActivity } from "@/services/types";

function activity(overrides: Partial<ProcessingActivity> = {}): ProcessingActivity {
  return {
    id: 1,
    kljuc: "evidencija_zaposlenih",
    rukovalacNaziv: "Butik Primer",
    rukovalacKontakt: "PIB 100000000",
    svrhaObrade: "Vođenje evidencije o zaposlenim licima.",
    vrstaLica: "Zaposleni i radno angažovana lica kod rukovaoca.",
    vrstaPodataka: "Identitet, matični broj, radno mesto.",
    vrstaPrimalaca: "Nadležni državni organi kada zakon to nalaže.",
    prenosUDrugeDrzave: "Nije utvrđen prenos u druge države.",
    mereZastitePrenosa: null,
    rokCuvanja: "Podaci se čuvaju trajno. Rok se ne podešava.",
    retentionRecordClass: "personnel",
    opisMeraZastite: "Evidencija je u zasebnoj tabeli.",
    updatedAt: "2026-08-01T09:00:00Z",
    ...overrides,
  };
}

function services(activities: ProcessingActivity[] = [activity()]): PosServices {
  const posServices = createMockServices();
  vi.spyOn(posServices.privacy, "listProcessingActivities").mockResolvedValue(
    activities,
  );
  return posServices;
}

describe("Cl47RegisterPanel", () => {
  /** Čl. 47 st. 1 t. 6 — the period per category is part of the register. */
  it("shows the rok čuvanja beside every radnja obrade", async () => {
    render(<Cl47RegisterPanel services={services()} />);

    expect(
      await screen.findByText(/vođenje evidencije o zaposlenim licima/i),
    ).toBeInTheDocument();
    expect(
      screen.getByText(/podaci se čuvaju trajno\. rok se ne podešava\./i),
    ).toBeInTheDocument();
  });

  /**
   * §5 item 6: st. 7's *„čuvaju se trajno“* governs THIS register and nothing
   * else. Copying it onto the audit or breach log would put the product in
   * permanent breach of storage limitation.
   */
  it("attaches the st. 7 trajno only to this register", async () => {
    render(<Cl47RegisterPanel services={services()} />);

    expect(await screen.findByText(/čl\. 47 st\. 7/i)).toBeInTheDocument();
    expect(
      screen.queryByText(/evidencija pristupa.*trajno/i),
    ).not.toBeInTheDocument();
    expect(
      screen.queryByText(/evidencija povreda.*trajno/i),
    ).not.toBeInTheDocument();
  });

  it("regenerates the register from the app's own configuration", async () => {
    const posServices = services();
    const generate = vi
      .spyOn(posServices.privacy, "generateProcessingActivities")
      .mockResolvedValue([
        activity({ id: 2, svrhaObrade: "Vođenje evidencije o radnom vremenu." }),
      ]);
    const user = userEvent.setup();

    render(<Cl47RegisterPanel services={posServices} />);
    await user.click(await screen.findByRole("button", { name: /osveži registar/i }));

    await waitFor(() => {
      expect(generate).toHaveBeenCalled();
    });
    expect(
      await screen.findByText(/vođenje evidencije o radnom vremenu/i),
    ).toBeInTheDocument();
  });

  it("exports the register and opens it for printing", async () => {
    const posServices = services();
    const exportRegister = vi
      .spyOn(posServices.privacy, "exportProcessingActivities")
      .mockResolvedValue({
        fileName: "evidencija-radnji-obrade.html",
        path: "mock://exports/evidencija-radnji-obrade.html",
        mimeType: "text/html",
        rowCount: 1,
      });
    const openForPrint = vi.spyOn(posServices.print, "openForPrint");
    const user = userEvent.setup();

    render(<Cl47RegisterPanel services={posServices} />);
    await user.click(await screen.findByRole("button", { name: /izvezi registar/i }));

    await waitFor(() => {
      expect(exportRegister).toHaveBeenCalled();
    });
    await waitFor(() => {
      expect(openForPrint).toHaveBeenCalledWith(
        "mock://exports/evidencija-radnji-obrade.html",
      );
    });
  });

  it("surfaces a failed read instead of an empty register", async () => {
    const posServices = createMockServices();
    vi.spyOn(posServices.privacy, "listProcessingActivities").mockRejectedValue({
      code: "forbidden",
      message: "Samo administrator može da čita registar radnji obrade.",
    });

    render(<Cl47RegisterPanel services={posServices} />);

    expect(
      await screen.findByText(/samo administrator može da čita registar radnji obrade/i),
    ).toBeInTheDocument();
  });
});
