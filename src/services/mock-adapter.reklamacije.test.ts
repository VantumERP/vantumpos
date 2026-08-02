import { describe, expect, it } from "vitest";

import { createMockServices } from "./mock-adapter";
import type { ReklamacijaInput } from "./types";

function intake(filedAt: string): ReklamacijaInput {
  return {
    podnosilacImePrezime: "Petar Petrović",
    kontakt: null,
    podaciORobi: "Veš mašina Beko",
    opisNesaobraznosti: "Ne centrifugira",
    zahtev: "Popravka",
    robaKind: "tehnicka",
    filedAt,
  };
}

describe("mock reklamacije — čl. 63 st. 3", () => {
  it("refuses an unattested new-regime resolve, like the backend does", async () => {
    const services = createMockServices();
    const view = await services.reklamacije.create(intake("2026-09-01T00:00:00Z"));

    expect(view.noFeeNotice).toContain("utvrđivanje nesaobraznosti");

    // A double that accepts what the backend rejects is worse than no double.
    await expect(
      services.reklamacije.resolve(view.id, "Zamena", "2026-09-04T00:00:00Z", false),
    ).rejects.toMatchObject({ code: "validation_error" });

    const resolved = await services.reklamacije.resolve(
      view.id,
      "Zamena",
      "2026-09-04T00:00:00Z",
      true,
    );
    expect(resolved.status).toBe("resolved");
    expect(resolved.noFeeAttested).toBe(true);
  });

  it("never gates an old-regime record", async () => {
    const services = createMockServices();
    const view = await services.reklamacije.create(intake("2026-06-01T00:00:00Z"));

    expect(view.noFeeNotice).toBeNull();

    const resolved = await services.reklamacije.resolve(
      view.id,
      "Popravka",
      "2026-06-10T00:00:00Z",
      false,
    );
    expect(resolved.status).toBe("resolved");
    expect(resolved.noFeeAttested).toBe(false);
  });
});
