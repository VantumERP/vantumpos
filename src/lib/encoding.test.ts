import { describe, expect, it } from "vitest";

import { decodeCsv } from "./encoding";

// "Košulja" in Windows-1250: š = 0x9A. K o š u l j a
const cp1250Kosulja = new Uint8Array([0x4b, 0x6f, 0x9a, 0x75, 0x6c, 0x6a, 0x61]).buffer;

describe("decodeCsv", () => {
  it("auto-detects Windows-1250 when the bytes are not valid UTF-8", () => {
    const { text, encoding } = decodeCsv(cp1250Kosulja);
    expect(text).toBe("Košulja");
    expect(encoding).toBe("windows-1250");
  });

  it("keeps valid UTF-8 as UTF-8", () => {
    const utf8 = new TextEncoder().encode("Košulja").buffer;
    const { text, encoding } = decodeCsv(utf8);
    expect(text).toBe("Košulja");
    expect(encoding).toBe("utf-8");
  });

  it("honors an explicit encoding override", () => {
    const { text } = decodeCsv(cp1250Kosulja, "windows-1250");
    expect(text).toBe("Košulja");
  });
});
