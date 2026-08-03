/**
 * The Serbian month names, in the ekavica the rest of the app is written in —
 * „avgust“ and not „august“, „jun“/„jul“ and not „juni“/„juli“.
 *
 * Deliberately a second copy of `MESECI` in `crate::commands::worktime`. The
 * backend has to be able to name a period it refuses a write for without being
 * handed the name by the caller it is refusing, and this side has to be able to
 * fill a `<select>` without asking the backend. Both copies are pinned string by
 * string — here in `period.test.ts`, there in `the_period_is_named_in_serbian` —
 * because nothing else can notice them drifting apart.
 */
export const MESECI = [
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
] as const;

/**
 * A period as the operator names it — „avgust 2026“, the words they picked it by.
 *
 * Carries no trailing dot: in Serbian the year is an ordinal and takes one, but
 * where it lands depends on the sentence, so every call site writes it. Mirrors
 * `naziv_perioda` in `crate::commands::worktime`, including the numeric
 * `MM/GGGG` fallback for a month outside 1–12 — unreachable from a picker that
 * offers twelve options, and preferable to „undefined 2026“ if it ever is not.
 */
export function nazivPerioda(godina: number, mesec: number): string {
  const naziv = MESECI[mesec - 1];

  return naziv
    ? `${naziv} ${godina}`
    : `${String(mesec).padStart(2, "0")}/${godina}`;
}
