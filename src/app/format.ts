export function formatQuantity(milliUnits: number, unit = "kom"): string {
  const absolute = Math.abs(milliUnits);
  const sign = milliUnits < 0 ? "-" : "";
  const whole = Math.floor(absolute / 1000);
  const fraction = absolute % 1000;

  if (fraction === 0) {
    return `${sign}${whole} ${unit}`;
  }

  const decimal = fraction.toString().padStart(3, "0").replace(/0+$/, "");
  return `${sign}${whole},${decimal} ${unit}`;
}

export function parseQuantityInput(value: string): number {
  const normalized = value.trim().replace(",", ".");

  if (!/^\d+(\.\d{1,3})?$/.test(normalized)) {
    throw new Error("Količina nije ispravna.");
  }

  const [whole, fraction = ""] = normalized.split(".");
  return Number(whole) * 1000 + Number(fraction.padEnd(3, "0"));
}
