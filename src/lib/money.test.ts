import { describe, expect, it } from "vitest";

import { formatRsd, parseRsdInput } from "./money";

describe("RSD money helpers", () => {
  it("formats integer minor units without floating point math", () => {
    expect(formatRsd(0)).toBe("0,00 RSD");
    expect(formatRsd(1234)).toBe("12,34 RSD");
    expect(formatRsd(123456)).toBe("1.234,56 RSD");
    expect(formatRsd(-1234)).toBe("-12,34 RSD");
  });

  it("parses Serbian decimal input into minor units", () => {
    expect(parseRsdInput("0")).toBe(0);
    expect(parseRsdInput("12,34")).toBe(1234);
    expect(parseRsdInput("1.234,56")).toBe(123456);
    expect(parseRsdInput("1234.56")).toBe(123456);
  });

  it("rejects invalid money input", () => {
    expect(() => parseRsdInput("")).toThrow("Unesite iznos.");
    expect(() => parseRsdInput("abc")).toThrow("Iznos nije ispravan.");
    expect(() => parseRsdInput("12,345")).toThrow("Iznos nije ispravan.");
  });

  it("rejects malformed dot-separated input", () => {
    expect(() => parseRsdInput("1.2.3")).toThrow("Iznos nije ispravan.");
    expect(() => parseRsdInput("1.23.45")).toThrow("Iznos nije ispravan.");
    expect(() => parseRsdInput("1.234.56")).toThrow("Iznos nije ispravan.");
  });

  it("rejects amounts that cannot be represented safely", () => {
    expect(() => parseRsdInput("9007199254740993")).toThrow(
      "Iznos nije ispravan.",
    );
  });
});
