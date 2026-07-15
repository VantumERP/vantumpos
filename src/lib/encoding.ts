export type CsvEncoding = "utf-8" | "windows-1250";

/**
 * Decodes CSV bytes to text. With no explicit encoding, tries UTF-8 (fatal) and
 * falls back to Windows-1250 — the encoding Serbian Excel exports by default.
 */
export function decodeCsv(
  bytes: ArrayBuffer,
  encoding?: CsvEncoding,
): { text: string; encoding: CsvEncoding } {
  if (encoding) {
    return { text: new TextDecoder(encoding).decode(bytes), encoding };
  }
  try {
    const text = new TextDecoder("utf-8", { fatal: true }).decode(bytes);
    return { text, encoding: "utf-8" };
  } catch {
    return {
      text: new TextDecoder("windows-1250").decode(bytes),
      encoding: "windows-1250",
    };
  }
}
