import { describe, expect, it } from "vitest";

import { createMockServices } from "./mock-adapter";
import type { PosServices } from "./ports";
import type { OpenPopisRequest } from "./types";

function otvoriZahtev(): OpenPopisRequest {
  return {
    vrsta: "godisnji",
    prodajnoMesto: "Butik Centar",
    datumPopisa: "2026-12-31",
    periodFrom: null,
    periodTo: null,
    planRadaJson: null,
    odlukaRef: null,
    perpetualOdlukaRef: null,
    uskladjivanjePotvrdjeno: true,
    komisija: [
      { ime: "Amina Hodžić", uloga: "predsednik", rukujeImovinom: false },
    ],
  };
}

/** Opens a popis and walks it to `computed` through the double's own arrows. */
async function doObracuna(services: PosServices): Promise<number> {
  const session = await services.popis.open(otvoriZahtev());
  await services.popis.saveLine(session.id, null, {
    listaVrsta: "roba",
    sifra: "KOS-1",
    naziv: "Košulja bela",
    vrsta: "roba",
    jedinicaMere: "kom",
    stvarnaKolicinaMilli: 7000,
    bliziOpis: null,
    knjigovodstvenaKolicinaMilli: null,
    cenaMinor: 249900,
  });
  await services.popis.startCount(session.id);
  await services.popis.signPhaseA(session.id, ["Amina Hodžić"]);
  await services.popis.compute(session.id);
  return session.id;
}

/**
 * The popis export and approval verbs, as the double has to model them.
 *
 * A test double that always succeeds proves nothing about the screen driving
 * it: every refusal these UI tests are supposed to surface would arrive as a
 * success, and the module would pass its tests while sending the backend a
 * request the backend refuses by name. So the three refusals the backend
 * actually has are reproduced here — the čl. 8 st. 5 phase guard, the blank
 * approver and the čl. 14 st. 3 posting lock — and each one is asserted from
 * both sides, because a double that refuses everything is no better.
 */
describe("mock popis — the čl. 8 st. 5 print phase", () => {
  it("refuses the čl. 9 st. 3 sheet before the potpis, like the backend does", async () => {
    const services = createMockServices();
    const session = await services.popis.open(otvoriZahtev());
    await services.popis.startCount(session.id);

    await expect(services.popis.exportLista(session.id, "b")).rejects.toThrow(
      /čl\. 8 st\. 5/,
    );

    const faza = await services.popis.exportLista(session.id, null);
    expect(faza.fileName).toContain("faza-a");
  });

  it("derives the čl. 9 st. 3 sheet once the potpis is on the counted state", async () => {
    const services = createMockServices();
    const id = await doObracuna(services);

    const derivirano = await services.popis.exportLista(id, null);
    expect(derivirano.fileName).toContain("faza-b");

    // Čl. 2 st. 6 — the signed counted state stays printable afterwards, and an
    // explicit „a“ can only ever narrow what the sheet carries.
    const ponovo = await services.popis.exportLista(id, "a");
    expect(ponovo.fileName).toContain("faza-a");
  });
});

describe("mock popis — the čl. 8 st. 2 approval", () => {
  it("refuses a blank approver in the backend's own words", async () => {
    const services = createMockServices();
    const session = await services.popis.open(otvoriZahtev());

    await expect(services.popis.odobriPlan(session.id, "   ")).rejects.toThrow(
      /Odobrenje bez imena se ne evidentira/,
    );

    const odobren = await services.popis.odobriPlan(session.id, "Amina Hodžić");
    expect(odobren.planRadaOdobrio).toBe("Amina Hodžić");
    // Half an approval is not an approval — v21 pairs the two columns in a
    // CHECK and the double writes them together or not at all.
    expect(odobren.planRadaOdobrenoAt).not.toBeNull();
  });

  it("refuses to approve the plan of a proknjižen popis", async () => {
    const services = createMockServices();
    const id = await doObracuna(services);
    await services.popis.signPhaseB(id, ["Amina Hodžić"]);
    await services.popis.post(id);

    await expect(
      services.popis.odobriPlan(id, "Amina Hodžić"),
    ).rejects.toThrow(/proknjižen/);

    // The documents stay printable — a reprint changes nothing about a posted
    // popis, and čl. 14 st. 3 locks the record and not the paper.
    await expect(services.popis.exportOdluka(id)).resolves.toBeTruthy();
    await expect(services.popis.exportPlanRada(id)).resolves.toBeTruthy();
  });
});
