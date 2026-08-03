import { describe, expect, it } from "vitest";

import { MESECI, nazivPerioda } from "./period";

describe("nazivPerioda", () => {
  it("names every month the way an operator picked it", () => {
    // Spelled out rather than derived from MESECI: this is the copy that has to
    // notice if the table itself is edited, and a test that reads the table it is
    // checking would agree with any change made to it.
    expect(nazivPerioda(2026, 1)).toBe("januar 2026");
    expect(nazivPerioda(2026, 2)).toBe("februar 2026");
    expect(nazivPerioda(2026, 3)).toBe("mart 2026");
    expect(nazivPerioda(2026, 4)).toBe("april 2026");
    expect(nazivPerioda(2026, 5)).toBe("maj 2026");
    expect(nazivPerioda(2026, 6)).toBe("jun 2026");
    expect(nazivPerioda(2026, 7)).toBe("jul 2026");
    expect(nazivPerioda(2026, 8)).toBe("avgust 2026");
    expect(nazivPerioda(2026, 9)).toBe("septembar 2026");
    expect(nazivPerioda(2026, 10)).toBe("oktobar 2026");
    expect(nazivPerioda(2026, 11)).toBe("novembar 2026");
    expect(nazivPerioda(2026, 12)).toBe("decembar 2026");
  });

  it("matches the Rust table it is a second copy of", () => {
    // `MESECI` in `crate::commands::worktime` is the other copy, pinned there by
    // `the_period_is_named_in_serbian`. Nothing can check the two against each
    // other across the language boundary, so both are pinned to the same twelve
    // strings and the pair of tests is the only thing standing between them and
    // a silent divergence in what the operator is told.
    expect(MESECI).toEqual([
      "januar",
      "februar",
      "mart",
      "april",
      "maj",
      "jun",
      "jul",
      "avgust",
      "septembar",
      "oktobar",
      "novembar",
      "decembar",
    ]);
  });

  it("falls back to the numeric form outside 1–12", () => {
    // Unreachable from the period pickers, which offer twelve options; it is here
    // so a bad period renders as something an operator can act on rather than
    // „undefined 2026“.
    expect(nazivPerioda(2026, 13)).toBe("13/2026");
    expect(nazivPerioda(2026, 0)).toBe("00/2026");
  });
});
