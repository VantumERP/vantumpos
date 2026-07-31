import type { DeclarationGapReason } from "@/services/types";

/**
 * ZoT čl. 34 st. 1 identity fields, keyed by the camelCase name the Rust side
 * sends in `missingFields`. One map, because the goods-receipt warning and the
 * „Artikli bez podataka deklaracije" report name the same blank columns — two
 * copies would let the two surfaces disagree about what is missing.
 *
 * An unknown key is returned as-is rather than dropped: a field the backend
 * learns to report before the UI learns its label must still reach the
 * operator.
 */
const DECLARATION_FIELD_LABELS: Record<string, string> = {
  manufacturerName: "Poslovno ime proizvođača",
  importerName: "Poslovno ime uvoznika",
  countryOfOrigin: "Zemlja proizvodnje",
  officialGoodsCode: "Šifra iz jedinstvenog šifarnika",
};

export function declarationFieldLabel(field: string): string {
  return DECLARATION_FIELD_LABELS[field] ?? field;
}

export function declarationFieldList(fields: string[]): string {
  return fields.map(declarationFieldLabel).join(", ");
}

/**
 * The reason an article is on the gaps list. Deliberately free of any figure —
 * §4 item 11 leaves the tier for a bare barcode defect unresolved, so only the
 * per-row `notice` may ever carry one.
 */
const DECLARATION_GAP_REASON_LABELS: Record<DeclarationGapReason, string> = {
  missingIdentityData: "Nedostaju podaci sa deklaracije",
  barcodeUnclassified: "Barkod nije klasifikovan",
  gtinCheckDigitInvalid: "GTIN ima neispravnu kontrolnu cifru",
};

export function declarationGapReasonLabel(reason: DeclarationGapReason): string {
  return DECLARATION_GAP_REASON_LABELS[reason] ?? reason;
}
