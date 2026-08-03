/**
 * The two helpers every Privatnost panel needs, and nothing else.
 *
 * They live outside the panels so that four surfaces which all render statutory
 * copy cannot drift into four different renderings of the same instant or four
 * different ways of swallowing the same backend refusal.
 */

/**
 * A backend refusal, verbatim. Every ZZPL command answers with prose the
 * operator can act on — „samo administrator“, „nalog je već izdat“, „vreme
 * saznanja je nepromenljivo“ — and replacing it with a generic sentence would
 * hide the one fact the panel exists to surface.
 */
export function errorMessage(error: unknown, fallback: string): string {
  if (
    typeof error === "object" &&
    error !== null &&
    "message" in error &&
    typeof (error as { message: unknown }).message === "string"
  ) {
    return (error as { message: string }).message;
  }

  return fallback;
}

/**
 * An RFC3339 instant as a Serbian operator reads it.
 *
 * Every instant this module renders was stamped in UTC by the backend, so the
 * rendering is a display concern only — nothing in the app decides anything
 * from the formatted string.
 */
export function formatInstant(value: string): string {
  const parsed = new Date(value);

  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  return new Intl.DateTimeFormat("sr-Latn-RS", {
    dateStyle: "short",
    timeStyle: "short",
  }).format(parsed);
}
