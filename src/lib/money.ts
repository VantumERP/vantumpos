const RSD_GROUP_SEPARATOR = ".";
const RSD_DECIMAL_SEPARATOR = ",";
const INVALID_MONEY_MESSAGE = "Iznos nije ispravan.";
const MAX_SAFE_MINOR_UNITS = BigInt(Number.MAX_SAFE_INTEGER);
const PLAIN_INTEGER_INPUT_PATTERN = /^-?\d+$/;
const COMMA_DECIMAL_INPUT_PATTERN = /^-?(?:\d+|\d{1,3}(?:\.\d{3})+),\d{1,2}$/;
const DOT_DECIMAL_INPUT_PATTERN = /^-?\d+\.\d{1,2}$/;
const DOT_GROUPED_INTEGER_INPUT_PATTERN = /^-?\d{1,3}(?:\.\d{3})+$/;

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
    throwInvalidMoneyInput();
  }

  const [dinarsPart, parasPart = ""] = normalized.split(".");
  const isNegative = dinarsPart.startsWith("-");
  const dinarsDigits = isNegative ? dinarsPart.slice(1) : dinarsPart;
  const minorUnits =
    BigInt(dinarsDigits) * 100n + BigInt(parasPart.padEnd(2, "0") || "0");

  if (minorUnits > MAX_SAFE_MINOR_UNITS) {
    throwInvalidMoneyInput();
  }

  return Number(isNegative ? -minorUnits : minorUnits);
}

function normalizeMoneyInput(input: string): string {
  const compact = input.replace(/\s/g, "");

  if (COMMA_DECIMAL_INPUT_PATTERN.test(compact)) {
    return compact.replace(/\./g, "").replace(",", ".");
  }

  if (DOT_DECIMAL_INPUT_PATTERN.test(compact)) {
    return compact;
  }

  if (DOT_GROUPED_INTEGER_INPUT_PATTERN.test(compact)) {
    return compact.replace(/\./g, "");
  }

  if (PLAIN_INTEGER_INPUT_PATTERN.test(compact)) {
    return compact;
  }

  throwInvalidMoneyInput();
}

function formatDinars(value: number): string {
  return value.toString().replace(/\B(?=(\d{3})+(?!\d))/g, RSD_GROUP_SEPARATOR);
}

function assertInteger(value: number, name: string): void {
  if (!Number.isInteger(value)) {
    throw new Error(`${name} mora biti ceo broj.`);
  }
}

function throwInvalidMoneyInput(): never {
  throw new Error(INVALID_MONEY_MESSAGE);
}
