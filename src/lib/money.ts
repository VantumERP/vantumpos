const RSD_GROUP_SEPARATOR = ".";
const RSD_DECIMAL_SEPARATOR = ",";

export function formatRsd(minorUnits: number): string {
  assertInteger(minorUnits, "minorUnits");

  const sign = minorUnits < 0 ? "-" : "";
  const absolute = Math.abs(minorUnits);
  const dinars = Math.floor(absolute / 100);
  const paras = absolute % 100;

  return `${sign}${formatDinars(dinars)}${RSD_DECIMAL_SEPARATOR}${paras
    .toString()
    .padStart(2, "0")} RSD`;
}

export function parseRsdInput(input: string): number {
  const trimmed = input.trim();

  if (!trimmed) {
    throw new Error("Unesite iznos.");
  }

  const normalized = normalizeMoneyInput(trimmed);

  if (!/^-?\d+(\.\d{1,2})?$/.test(normalized)) {
    throw new Error("Iznos nije ispravan.");
  }

  const [dinarsPart, parasPart = ""] = normalized.split(".");
  const sign = dinarsPart.startsWith("-") ? -1 : 1;
  const dinars = Math.abs(Number.parseInt(dinarsPart, 10));
  const paras = Number.parseInt(parasPart.padEnd(2, "0"), 10) || 0;

  return sign * (dinars * 100 + paras);
}

function normalizeMoneyInput(input: string): string {
  const compact = input.replace(/\s/g, "");
  const hasComma = compact.includes(",");
  const hasDot = compact.includes(".");

  if (hasComma) {
    return compact.replace(/\./g, "").replace(",", ".");
  }

  if (hasDot) {
    const parts = compact.split(".");
    const lastPart = parts[parts.length - 1];

    if (lastPart.length <= 2 && parts.length > 1) {
      return `${parts.slice(0, -1).join("")}.${lastPart}`;
    }

    return compact.replace(/\./g, "");
  }

  return compact;
}

function formatDinars(value: number): string {
  return value.toString().replace(/\B(?=(\d{3})+(?!\d))/g, RSD_GROUP_SEPARATOR);
}

function assertInteger(value: number, name: string): void {
  if (!Number.isInteger(value)) {
    throw new Error(`${name} mora biti ceo broj.`);
  }
}
