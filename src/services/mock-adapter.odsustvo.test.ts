import { describe, expect, it } from "vitest";

import { createMockServices } from "./mock-adapter";
import type { SaveWorkTimeEntryRequest } from "./types";

/**
 * SW-14 req. 28 in the double, because a double that shows what the backend
 * withholds proves nothing.
 *
 * The mock cannot copy the backend's structural half — `load_entries_without_reason`
 * is a second SQL statement that never names `kategorija_odsustva`, and an
 * in-memory array has no query to leave the column out of. What it can and must
 * copy is the observable contract: while a čl. 46 nalog is live and unrevealed
 * the category is absent and the read says so, the absence and its minutes stand,
 * the unmask is per-nalog, and unmasking without a nalog is refused with the
 * backend's own code.
 */
function bolovanje(dan: string): SaveWorkTimeEntryRequest {
  return {
    userId: 1,
    dan,
    godina: Number(dan.slice(0, 4)),
    mesec: Number(dan.slice(5, 7)),
    moguciMinuta: 480,
    efektivnoIzvrseniMinuta: 0,
    casoviCekanjaIZastojaMinuta: 0,
    prekovremeniMinuta: 0,
    nocniMinuta: 0,
    radNaPraznikMinuta: 0,
    kategorijaOdsustva: "sprecenost_rfzo",
    odsustvoMinuta: 480,
    capOverrideRazlog: null,
  };
}

describe("mock req. 28 — the absence reason under a support nalog", () => {
  it("withholds the reason while a nalog is live and keeps the absence", async () => {
    const services = createMockServices();
    await services.worktime.saveEntry(bolovanje("2026-06-02"));

    const otvoreno = await services.worktime.listMonth(1, 2026, 6);
    expect(otvoreno.razlogOdsustvaSkriven).toBe(false);
    expect(otvoreno.entries[0].kategorijaOdsustva).toBe("sprecenost_rfzo");

    await services.privacy.grantSupportAccess("Pregled greške na štampi", 60);

    const maskirano = await services.worktime.listMonth(1, 2026, 6);
    expect(maskirano.razlogOdsustvaSkriven).toBe(true);
    expect(maskirano.entries[0].kategorijaOdsustva).toBeNull();
    // Truthful, not blank: the day is still an absence of 480 minutes.
    expect(maskirano.entries[0].minuti.ukupnoNeizvrseniMinuta).toBe(480);
    expect(maskirano.ukupno.ukupnoNeizvrseniMinuta).toBe(480);
  });

  it("carries the reason again once the shop unmasks that nalog", async () => {
    const services = createMockServices();
    await services.worktime.saveEntry(bolovanje("2026-06-03"));
    await services.privacy.grantSupportAccess("Pregled greške na štampi", 60);

    const nalog = await services.privacy.revealAbsenceReason();
    expect(nalog.odsustvoOtkrivenoAt).not.toBeNull();

    const otkriveno = await services.worktime.listMonth(1, 2026, 6);
    expect(otkriveno.razlogOdsustvaSkriven).toBe(false);
    expect(otkriveno.entries[0].kategorijaOdsustva).toBe("sprecenost_rfzo");
  });

  it("keeps one disclosure per nalog rather than re-stamping", async () => {
    const services = createMockServices();
    await services.privacy.grantSupportAccess("Pregled greške na štampi", 60);

    const prvi = await services.privacy.revealAbsenceReason();
    const drugi = await services.privacy.revealAbsenceReason();

    expect(drugi.odsustvoOtkrivenoAt).toBe(prvi.odsustvoOtkrivenoAt);
  });

  it("refuses an unmask with no live nalog, with the backend's own code", async () => {
    const services = createMockServices();

    await expect(services.privacy.revealAbsenceReason()).rejects.toMatchObject({
      code: "support_bez_naloga_za_otkrivanje",
    });

    // …and the refusal is distinct from the one `enterSupportSession` gives, so
    // the surface can say the right sentence.
    await expect(services.privacy.enterSupportSession()).rejects.toMatchObject({
      code: "support_bez_naloga",
    });
  });

  it("leaves „Moji sati“ unmasked, because the viewer is the data subject", async () => {
    const services = createMockServices();
    await services.worktime.saveEntry(bolovanje("2026-06-04"));
    await services.privacy.grantSupportAccess("Pregled greške na štampi", 60);

    const moji = await services.worktime.myHours(2026, 6);
    expect(moji.razlogOdsustvaSkriven).toBe(false);
    expect(moji.entries[0].kategorijaOdsustva).toBe("sprecenost_rfzo");
  });

  it("exposes one absence-reason verb and no way back", () => {
    const services = createMockServices();

    // A `remaskAbsenceReason` on this surface would be a claim the backend
    // cannot honour: the stamp is a stamp, and an operator who read the category
    // does not unread it.
    expect(
      Object.keys(services.privacy)
        .filter((name) => /absence|mask|skri/i.test(name))
        .sort(),
    ).toEqual(["revealAbsenceReason"]);
  });
});
