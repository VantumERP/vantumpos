export type BackendKind = "local";

export interface CommandError {
  code: string;
  message: string;
  details?: unknown;
}

export type CommandErrorShape = CommandError;

export interface AppHealth {
  backend: BackendKind;
  appVersion: string;
  databasePath: string;
  migrated: boolean;
}

export interface CompanySettings {
  shopName: string;
  address: string;
  pib: string;
  registrationNumber: string;
  phone: string;
  logoPath: string | null;
  currency?: "RSD";
}

export type CompanySettingsRequest = CompanySettings;

export interface TaxRate {
  id: number;
  name: string;
  rateBasisPoints: number;
  active: boolean;
}

export type TaxRateSummary = TaxRate;

export interface SaveTaxRateRequest {
  id: number | null;
  name: string;
  rateBasisPoints: number;
  active: boolean;
}

export interface ReceiptSettings {
  prefix: string;
  nextSequenceNumber: number;
  resetPolicy: "none";
}

export interface ReceiptSettingsRequest {
  prefix: string;
  nextSequenceNumber: number;
}

export interface SalesSettings {
  allowOverselling: boolean;
}

export interface SalesSettingsRequest {
  allowOverselling: boolean;
}

export type PravnaForma = "preduzetnik" | "pravno_lice";
export type EsirTip = "ESIR" | "LPFR";

export interface EsirElement {
  naziv: string;
  verzija: string;
  ib: string;
  tip: EsirTip;
  checkedOn: string | null;
}

export interface ShopProfile {
  pravnaForma: PravnaForma | null;
  pdvObveznik: boolean | null;
  /** `null` = nije odgovoreno. Never coalesce it to `false` — see §3 req 31 / §5 Q-8. */
  distanceSelling: boolean | null;
  lpfrInPremises: boolean | null;
  esirElements: EsirElement[];
}

export interface LegalNotice {
  summary: string;
  penalty: string | null;
  citation: string;
  isLegalDuty: boolean;
}

export type RateSource = "nbs" | "manual";

export interface EurRate {
  /** Para per 1 EUR (117,2345 RSD/EUR -> 11723), floored — never rounded up. */
  rateMinor: number;
  /** `YYYY-MM-DD`. A date other than today's makes the check stale. */
  rateDate: string;
  source: RateSource;
}

/**
 * The verdict of `sales_assess_cash_payment` (AML čl. 46 st. 1). Advisory:
 * `breached` never rejects the sale, it only demands an acknowledgement, and
 * `rateUnavailable` degrades the check instead of blocking the till.
 */
export interface AmlAssessment {
  cashMinor: number;
  thresholdMinor: number;
  /**
   * A conservative stand-in cap (10.000 EUR at a 100,00 RSD/EUR floor) computed
   * in `src-tauri/src/aml.rs` — never in the frontend, because the figure is a
   * legal decision. Advisory surfaces use it to decide whether an *unavailable*
   * check is worth mentioning at all; nothing may use it to assert a breach.
   */
  fallbackThresholdMinor: number;
  breached: boolean;
  nearThreshold: boolean;
  rateUnavailable: boolean;
  rate: EurRate | null;
  notice: LegalNotice;
}

/**
 * One trading date's cash and how much of it has reached the bank
 * (`crate::cash_deposit::DepositBucket`).
 *
 * The per-trading-date roll-up is an implementation convention, not a statutory
 * category — čl. 3 st. 1 runs from receipt of the cash — and every rendering
 * must say so.
 */
export interface DepositBucket {
  tradingDate: string;
  subjectMinor: number;
  depositedMinor: number;
  outstandingMinor: number;
  /** `null` when the deadline could not be computed. Never read as „u roku". */
  dueOn: string | null;
  isOverdue: boolean;
}

/**
 * „Izveštaj o nedeponovanom gotovom novcu" (`cash_deposit_report`). Advisory by
 * construction: it carries no blocking flag, because the seven-working-day duty
 * is fiscal hygiene supervised by Poreska uprava, not a condition of a valid
 * sale.
 */
export interface CashDepositReport {
  /** The presek date the deadlines were assessed against, `YYYY-MM-DD`. */
  asOf: string;
  buckets: DepositBucket[];
  outstandingMinor: number;
  /** Of the outstanding total, the part whose deadline has already passed. */
  overdueMinor: number;
  /** Cash the Pravilnik 77/2011 čl. 5 st. 2 carve-out kept out of the base. */
  excludedFloatMinor: number;
  saturdayIsWorking: boolean;
  calendarHorizonYear: number;
  /** A deadline falls past the seeded holiday table for that year. */
  beyondSeededCalendar: boolean;
  notice: LegalNotice;
  /** The honesty labels that must travel with every rendering of the numbers. */
  footer: string;
}

export interface NonWorkingDay {
  /** `YYYY-MM-DD`. */
  day: string;
  label: string;
}

/**
 * The calendar the seven-working-day deadline is counted against. „Radni dan"
 * is statutorily undefined, so `saturdayIsWorking` is a configurable
 * assumption — defaulted to `true` because counting Saturdays yields the
 * earlier, conservative deadline.
 */
export interface CashDepositCalendar {
  saturdayIsWorking: boolean;
  days: NonWorkingDay[];
  /** Last year the shipped holiday table covers. */
  horizonYear: number;
}

export interface BackupSettings {
  backupFolder: string;
  automaticBackupEnabled: boolean;
}

export interface CreateBackupRequest {
  backupFolder?: string | null;
  backupType?: "manual" | "automatic" | null;
}

export interface RestoreBackupRequest {
  path: string;
  confirmationText: string;
  passphrase?: string | null;
}

export interface BackupJob {
  id: number;
  backupType: "manual" | "automatic" | "restore" | "pre_restore";
  path: string;
  status: "completed" | "failed";
  errorMessage: string | null;
  fileSizeBytes: number | null;
  createdAt: string;
  completedAt: string | null;
}

export interface BackupStatus extends BackupSettings {
  stale: boolean;
  encryptionConfigured: boolean;
  lastSuccessfulBackup: BackupJob | null;
  lastFailedBackup: BackupJob | null;
}

export type PaymentMethod = "cash" | "card" | "bank_transfer";

export type UserRole = "admin" | "cashier";

export interface UserAccount {
  id: number;
  username: string;
  displayName: string;
  role: UserRole;
  active: boolean;
  createdAt: string;
  updatedAt: string;
  lastLoginAt: string | null;
}

export type UserSummary = UserAccount;

export interface LoginRequest {
  username: string;
  credential: string;
}

export interface ShiftSummary {
  id: number;
  userId: number;
  cashierName: string;
  openedAt: string;
  closedAt: string | null;
  openingCashMinor: number;
  expectedCashMinor: number;
  countedCashMinor: number | null;
  cashSalesMinor: number;
  cardSalesMinor: number;
  paidInMinor: number;
  paidOutMinor: number;
  differenceMinor: number | null;
  status: "open" | "closed";
  openingNote: string | null;
  closingNote: string | null;
}

export interface ShiftListItem {
  id: number;
  openedAt: string;
  closedAt: string | null;
  cashierName: string;
}

export interface AuthSession {
  user: UserAccount;
  currentShift: ShiftSummary | null;
}

export type AppSession = AuthSession;

export interface OpenShiftRequest {
  openingCashMinor: number;
  note?: string | null;
}

export interface CloseShiftRequest {
  shiftId: number;
  countedCashMinor: number;
  note?: string | null;
}

/**
 * `bank_withdrawal` is a podizanje sa računa — money leaves the bank and enters
 * the drawer, so it counts like a pay_in. `bank_deposit` is a polog: money
 * leaves the drawer for the shop's own račun, so it counts like a pay_out and
 * draws down the oldest open deposit bucket.
 */
export type CashMovementDirection =
  | "pay_in"
  | "pay_out"
  | "bank_deposit"
  | "bank_withdrawal";

export interface CashMovementRequest {
  direction: CashMovementDirection;
  amountMinor: number;
  reason?: string | null;
  /**
   * Broj izvoda / uplatnice for the two bank directions — the evidence trail
   * per polog. Optional: a shop that records the polog before the bank
   * confirms it must not be blocked.
   */
  bankReference?: string | null;
}

export interface SaveUserRequest {
  username: string;
  displayName: string;
  role: UserRole;
  active: boolean;
  pin?: string | null;
  password?: string | null;
}

export interface CategorySummary {
  id: number;
  name: string;
  active: boolean;
}

export type Category = CategorySummary;

export interface ProductExternalSource {
  provider: "open_food_facts" | string;
  label: string;
  barcode: string;
  fetchedAt: string;
  acceptedFields: string[];
}

export interface ProductLookupFields {
  name?: string;
  brand?: string;
  imageUrl?: string;
  packageSize?: string;
  categoryName?: string;
}

export interface ProductLookupSuggestion {
  barcode: string;
  source: ProductExternalSource;
  fields: ProductLookupFields;
}

export interface ProductSummary {
  id: number;
  name: string;
  sku: string;
  barcode: string | null;
  categoryId: number | null;
  categoryName: string | null;
  unitOfMeasure: string;
  salePriceMinor: number;
  purchasePriceMinor: number;
  taxRateId: number;
  taxRateName?: string;
  taxRateBasisPoints: number;
  minimumStockMilli: number;
  currentStockMilli: number;
  allowNegativeStock: boolean;
  active: boolean;
  perishable: boolean;
  perishableJustification?: string | null;
  manufacturerName?: string | null;
  importerName?: string | null;
  countryOfOrigin?: string | null;
  officialGoodsCode?: string | null;
  barcodeKind?: ProductBarcodeKind | null;
  externalSource: ProductExternalSource | null;
}

export interface ProductListQuery {
  search?: string;
  categoryId?: number | null;
  taxRateId?: number;
  active?: boolean;
  lowStock?: boolean;
  missingBarcode?: boolean;
}

export interface ProductSearchQuery {
  search?: string;
  active?: boolean;
  limit?: number;
}

export interface ProductListResult {
  items: ProductSummary[];
  categories: CategorySummary[];
  taxRates: TaxRateSummary[];
  total: number;
}

export interface SaveProductRequest {
  name: string;
  sku: string;
  barcode: string | null;
  categoryId: number | null;
  unitOfMeasure: string;
  salePriceMinor: number;
  purchasePriceMinor: number;
  taxRateId: number;
  minimumStockMilli: number;
  allowNegativeStock: boolean;
  active: boolean;
  /** Lako kvarljiva roba. Suppresses the čl. 37 st. 3 computation: the price
   *  log of a perishable records end-of-life markdowns, not the „najniža cena"
   *  a shopper compares against, so campaigns demand a manual anchor instead. */
  perishable: boolean;
  perishableJustification?: string | null;
  /** Deklaracija, ZoT čl. 34. In-store the marking duty is the proizvođač's
   *  (st. 2), so these are an aid. Selling at distance moves the duty onto the
   *  trgovac (st. 5), and the Rust side then refuses a save without
   *  `manufacturerName` and `countryOfOrigin`. */
  manufacturerName?: string | null;
  importerName?: string | null;
  countryOfOrigin?: string | null;
  officialGoodsCode?: string | null;
  barcodeKind?: ProductBarcodeKind | null;
  externalSource?: ProductExternalSource | null;
}

/** „none" is an answered state — the article carries no barcode — not an unset
 *  one. An unset kind is `null`. */
export type ProductBarcodeKind = "gtin" | "internal" | "none";

export type ProductFormData = SaveProductRequest;

export interface SaveCategoryRequest {
  id?: number;
  name: string;
  active?: boolean;
}

export interface PrethodnaCenaDto {
  status: "computed" | "incomputable";
  priceMinor: number | null;
  windowDays: number | null;
  windowFrom: string | null;
  windowTo: string | null;
  truncated: boolean;
  reason: "too_new_in_assortment" | "not_offered_in_window" | "no_history" | null;
  ageDays: number | null;
}

/** Mirrors `crate::campaigns` — every field name here is the serde camelCase
 *  rendering of the Rust struct it carries. Nothing in this contract asserts
 *  that a campaign is lawful: `hard` being empty means only that no
 *  mechanically checkable rule was broken (čl. 38 st. 4 stays open). */
export type CampaignType =
  | "rasprodaja"
  | "sezonsko_snizenje"
  | "akcijska_prodaja"
  | "promotivna_prodaja";
export type CampaignStatus = "draft" | "active" | "ended" | "cancelled";
export type CampaignDisplayMode = "two_prices" | "percentage";
export type RasprodajaGround =
  | "prestanak_poslovanja"
  | "prestanak_u_objektu"
  | "prestanak_prodaje_robe";
export type CampaignAnchorStatus = "computed" | "manual" | "none";

export interface CampaignItemInput {
  productId: number;
  campaignPriceMinor: number;
  manualPrethodnaMinor?: number | null;
  anchorJustification?: string | null;
  futureRegularPriceMinor?: number | null;
}

export interface CampaignInput {
  campaignType: CampaignType;
  /**
   * RFC3339 instant, e.g. `2026-07-01T00:00:00Z`. The statute counts calendar
   * days, so the time component is irrelevant — but the format is not: the
   * backend parses this with `parse_rfc3339` and a bare `YYYY-MM-DD` comes back
   * as hard violation h14a.
   */
  startsOn: string;
  /**
   * RFC3339 instant, same format requirement as `startsOn` (h14c on failure).
   * `null` only for rasprodaja — „dok traju zalihe".
   */
  endsOn?: string | null;
  displayMode: CampaignDisplayMode;
  headlinePercent?: number | null;
  rasprodajaGround?: RasprodajaGround | null;
  specialConditions?: string | null;
  reducedUtilityReason?: string | null;
  marketingLabel?: string | null;
  seasonAttested: boolean;
  separationAttested: boolean;
  items: CampaignItemInput[];
}

export interface CampaignViolation {
  code: string;
  message: string;
  productId: number | null;
}

export interface CampaignItemAnchor {
  productId: number;
  anchorStatus: CampaignAnchorStatus;
  prethodnaCenaMinor: number | null;
  anchorWindowDays: number | null;
  anchorTruncated: boolean;
  anchorReason: string | null;
  anchorJustification: string | null;
}

export interface CampaignValidationReport {
  hard: CampaignViolation[];
  warnings: CampaignViolation[];
  anchors: CampaignItemAnchor[];
}

export interface CampaignItemView {
  productId: number;
  productName: string;
  sku: string;
  campaignPriceMinor: number;
  prethodnaCenaMinor: number | null;
  anchorStatus: CampaignAnchorStatus;
  anchorWindowDays: number | null;
  anchorTruncated: boolean;
  anchorReason: string | null;
  anchorJustification: string | null;
  futureRegularPriceMinor: number | null;
  preCampaignPriceMinor: number | null;
}

export interface CampaignView {
  id: number;
  campaignType: CampaignType;
  status: CampaignStatus;
  startsOn: string;
  endsOn: string | null;
  displayMode: CampaignDisplayMode;
  headlinePercent: number | null;
  rasprodajaGround: RasprodajaGround | null;
  specialConditions: string | null;
  reducedUtilityReason: string | null;
  marketingLabel: string | null;
  seasonAttested: boolean;
  separationAttested: boolean;
  /** RFC3339. Once set the anchor is frozen — no path re-snapshots it. */
  activatedAt: string | null;
  endedAt: string | null;
  overdue: boolean;
  items: CampaignItemView[];
  warnings: CampaignViolation[];
}

export interface CampaignSummary {
  id: number;
  campaignType: CampaignType;
  status: CampaignStatus;
  startsOn: string;
  endsOn: string | null;
  marketingLabel: string | null;
  itemCount: number;
  overdue: boolean;
}

export interface EndCampaignOverride {
  productId: number;
  returnPriceMinor: number;
}

/** Mirrors `crate::campaign_evidence::CorrectionRow` — one active-campaign
 *  label line plus whether its anchor warrants a second look. Field names are
 *  the serde camelCase rendering of that Rust struct. */
export interface CorrectionRow {
  campaignId: number;
  productId: number;
  productName: string;
  sku: string;
  campaignType: CampaignType;
  displayMode: CampaignDisplayMode;
  campaignPriceMinor: number;
  prethodnaCenaMinor: number | null;
  needsAttention: boolean;
  attentionReason: string | null;
}

export interface CorrectionReport {
  rows: CorrectionRow[];
}

/**
 * The three `roba_kind` values the schema's CHECK constraint permits, mirroring
 * `crate::reklamacije::ROBA_KINDS`. The furniture/technical classification is
 * the shop's legal call (memo §2.3) — this is only the persisted tag.
 */
export type RobaKind = "opsta" | "tehnicka" | "namestaj";

/**
 * The frozen regime, persisted at intake from the filing date vs the cutover and
 * NEVER recomputed. Mirrors `crate::reklamacije::REGIME_OLD`/`REGIME_NEW`.
 */
export type ReklamacijaRegime = "old" | "new";

/**
 * The derived deadline verdict — mirrors `crate::reklamacije::DeadlineState`
 * (serde camelCase). Nothing here is stored: `compute_deadlines` re-derives it
 * from the event log on every read, so the same events always produce the same
 * answer for a given day. `clock` is `"running" | "paused" | "impasse" |
 * "resolved"`; `resolutionDue` is `null` while paused / at impasse / resolved.
 */
export interface DeadlineState {
  answerDue: string;
  resolutionDue: string | null;
  clock: string;
  consumerWindowDue: string | null;
  answerOverdue: boolean;
  resolutionOverdue: boolean;
  oneExtensionUsed: boolean;
}

/**
 * One event-log row projected for the UI — mirrors `crate::reklamacije::
 * EventView`. `consumerConsent` only ever carries meaning on an
 * `extension_granted` row.
 */
export interface ReklamacijaEvent {
  eventType: string;
  eventDate: string;
  detailJson: string | null;
  consumerConsent: boolean;
}

/**
 * Intake payload — mirrors `crate::reklamacije::ReklamacijaInput`. Consumer PII
 * (`podnosilacImePrezime`, `kontakt`) is inline and admin-gated; there is no
 * consent UI — the lawful basis is the shop's legal obligation to keep the
 * evidencija. `filedAt` is an RFC3339 instant; its calendar date fixes the
 * regime and starts the clocks.
 */
export interface ReklamacijaInput {
  podnosilacImePrezime: string;
  kontakt: string | null;
  podaciORobi: string;
  opisNesaobraznosti: string;
  zahtev: string;
  robaKind: RobaKind;
  filedAt: string;
}

/**
 * The answer payload — mirrors `crate::reklamacije::AnswerInput`. The three
 * `warning*` fields carry the express warning (memo §4(a)); they are mandatory
 * only under the NEW regime, where `log_answer` is gated on them.
 */
export interface AnswerInput {
  answerText: string;
  warningDuty: string | null;
  warningConsequences: string | null;
  warningZastoj: string | null;
  /** čl. 63 st. 3 attestation. Ignored by the backend under the OLD regime. */
  noFeeAttested: boolean;
  eventDate: string;
}

/**
 * The full record — mirrors `crate::reklamacije::ReklamacijaView`. `status` is
 * the stored column; `deadlines.clock` is the derived running/paused/impasse
 * view. `purgeEligible` is the derived `filedAt + 2y ≤ today` retention flag —
 * an eligibility signal only; nothing auto-deletes (memo §3).
 */
export interface ReklamacijaView {
  id: number;
  registerNumber: number;
  regime: ReklamacijaRegime;
  status: string;
  filedAt: string;
  podnosilacImePrezime: string;
  kontakt: string | null;
  podaciORobi: string;
  opisNesaobraznosti: string;
  zahtev: string;
  robaKind: RobaKind;
  datumIzdavanjaPotvrde: string;
  createdBy: number | null;
  createdAt: string;
  updatedAt: string;
  events: ReklamacijaEvent[];
  deadlines: DeadlineState;
  purgeEligible: boolean;
  /**
   * Advisory prekršaj exposure for this record's own frozen regime, already
   * resolved to the shop's tier by `legal.rs::reklamacija_breach`. No fine
   * figure may live outside that module — render this, never derive one.
   */
  notice: LegalNotice;
  /**
   * čl. 63 st. 3 attestation. Always `false` under the OLD regime, which has no
   * fee ban — that `false` is not a compliance signal.
   */
  noFeeAttested: boolean;
  noFeeAttestedAt: string | null;
  /**
   * The prohibition text, `null` unless this record's frozen regime is NEW.
   * Render on presence; never re-derive the regime in a component.
   */
  noFeeNotice: string | null;
}

/**
 * Register-list row — mirrors `crate::reklamacije::ReklamacijaSummary`. Carries
 * the stored `status` plus the derived answer/resolution dates and overdue
 * flags, so the list shows the deadline engine's verdict without loading each
 * record's full event log.
 */
export interface ReklamacijaSummary {
  id: number;
  registerNumber: number;
  regime: ReklamacijaRegime;
  status: string;
  podnosilacImePrezime: string;
  filedAt: string;
  answerDue: string;
  resolutionDue: string | null;
  answerOverdue: boolean;
  resolutionOverdue: boolean;
  purgeEligible: boolean;
}

export type DiscountDraft =
  | { type: "amount"; amountMinor: number }
  | { type: "percent"; basisPoints: number };

export interface SaleDraftItem {
  productId: number;
  quantityMilli: number;
  discount?: DiscountDraft | null;
}

export interface SaleDraftRequest {
  items: SaleDraftItem[];
  receiptDiscount?: DiscountDraft | null;
}

export interface SalePreviewItem {
  productId: number;
  productName: string;
  productSku: string;
  productBarcode: string | null;
  quantityMilli: number;
  quantityLabel: string;
  unitPriceMinor: number;
  discountMinor: number;
  taxRateBasisPoints: number;
  taxMinor: number;
  totalMinor: number;
}

export interface SalePreview {
  items: SalePreviewItem[];
  subtotalMinor: number;
  discountMinor: number;
  taxMinor: number;
  totalMinor: number;
}

export interface SalePaymentDraft {
  method: PaymentMethod;
  amountMinor: number;
}

export interface CompleteSaleRequest extends SaleDraftRequest {
  payments: SalePaymentDraft[];
  allowStockOverride?: boolean;
  /**
   * What the operator typed when acknowledging the AML cash-cap warning.
   * Recorded, never required — the sale is never rejected on an AML result.
   */
  amlAckReason?: string;
}

export interface CompletedSale extends SalePreview {
  id: number;
  localReceiptNumber: string;
  createdAt: string;
  cashierName: string;
  fiscalStatus: "not_fiscalized" | "fiscalized" | "failed";
  payments: SalePaymentDraft[];
  cashReceivedMinor: number;
  changeDueMinor: number;
}

export type InventoryMovementType =
  | "receive"
  | "correction"
  | "write_off"
  | "sale"
  | "return"
  | "void";

export type StockStateFilter = "all" | "low" | "zero" | "negative";

export interface StockListQuery {
  search?: string;
  categoryId?: number | null;
  stockState?: StockStateFilter;
}

export interface StockListItem {
  productId: number;
  productName: string;
  sku: string;
  barcode: string | null;
  categoryId: number | null;
  categoryName: string | null;
  unitOfMeasure: string;
  currentQuantityMilli: number;
  minimumStockMilli: number;
  lowStock: boolean;
  salePriceMinor: number;
  lastMovementAt: string | null;
}

export interface StockListResult {
  items: StockListItem[];
}

export interface InventoryAdjustmentRequest {
  productId: number;
  quantityMilli: number;
  reason?: string | null;
  purchasePriceMinor?: number | null;
  referenceType?: string | null;
  referenceId?: number | null;
}

/**
 * `commands::inventory::DeclarationWarning`. ZoT čl. 34 st. 2 puts the marking
 * duty on the proizvođač/uvoznik, but čl. 68 st. 1 tač. 9 punishes the trgovac
 * who *sells* goods without a deklaracija — so the goods receipt warns, and
 * only warns. The receipt is already committed by the time this arrives.
 */
export interface DeclarationWarning {
  productId: number;
  productName: string;
  /** camelCase names of the blank fields, so the UI can point at the input. */
  missingFields: string[];
  /** The record-vs-reality qualifier. Render it **with** `notice`, never the
   *  notice alone: a blank column is a gap in the shop's own records, not proof
   *  that the pallet carries no deklaracija. */
  advisory: string;
  notice: LegalNotice;
}

export interface InventoryAdjustmentResult {
  productId: number;
  movementId: number;
  movementType: InventoryMovementType;
  quantityMilli: number;
  previousQuantityMilli: number;
  newQuantityMilli: number;
  createdAt: string;
  /** Advisory only, and always empty for corrections and write-offs. */
  declarationWarnings: DeclarationWarning[];
}

/** `commands::catalog::DeclarationGapReason`. The UI branches on the reason
 *  rather than inferring it from the flags: only `missingIdentityData` may ever
 *  carry a penalty figure (§3 req 26). */
export type DeclarationGapReason =
  | "missingIdentityData"
  | "barcodeUnclassified"
  | "gtinCheckDigitInvalid";

/** One row of `catalog_declaration_gaps`. Read-only; blocks nothing. */
export interface DeclarationGapRow {
  productId: number;
  sku: string;
  name: string;
  barcode: string | null;
  barcodeKind: ProductBarcodeKind | null;
  missingFields: string[];
  /** `null` barcode_kind means „unclassified", never „is a GTIN". */
  barcodeUnclassified: boolean;
  gtinCheckDigitInvalid: boolean;
  /** Never empty — a row with no reason is not a gap and is not returned. */
  reasons: DeclarationGapReason[];
  /** `null` unless `notice` is set; the two are rendered together or not at all. */
  advisory: string | null;
  /** `null` for a bare barcode defect: §4 item 11 attaches no figure to it. */
  notice: LegalNotice | null;
}

export interface InventoryLedgerMovement {
  id: number;
  movementType: InventoryMovementType;
  quantityMilli: number;
  resultingQuantityMilli: number;
  reason: string | null;
  referenceType: string | null;
  referenceId: number | null;
  createdAt: string;
}

export type ProductLedgerMovement = InventoryLedgerMovement;

export interface ProductLedger {
  productId: number;
  product: StockListItem;
  movements: InventoryLedgerMovement[];
}

export type ReceiptStatus = "completed" | "voided" | "refunded";
export type FiscalStatus = "not_fiscalized" | "fiscalized" | "failed";
export type ReceiptDocumentType = "sale" | "void" | "return";

export interface ReceiptSearchQuery {
  dateFrom?: string;
  dateTo?: string;
  from?: string;
  to?: string;
  receiptNumber?: string;
  cashier?: string;
  shiftId?: number;
  paymentMethod?: PaymentMethod;
  product?: string;
}

export interface ReceiptSummary {
  id: number;
  receiptNumber: string;
  createdAt: string;
  cashierName: string;
  shiftId: number;
  status: ReceiptStatus;
  fiscalStatus: FiscalStatus;
  documentType: ReceiptDocumentType;
  paymentMethods: PaymentMethod[];
  totalMinor: number;
  linkedDocumentCount: number;
}

export type ReceiptListItem = ReceiptSummary;

export interface ReceiptSearchResult {
  receipts: ReceiptSummary[];
  total: number;
}

export interface ReceiptItem {
  id: number;
  productId?: number | null;
  productName: string;
  productSku: string;
  productBarcode?: string | null;
  quantityMilli: number;
  unitPriceMinor: number;
  discountMinor: number;
  taxRateBasisPoints: number;
  taxMinor: number;
  totalMinor: number;
  returnedQuantityMilli: number;
}

export type ReceiptDetailItem = ReceiptItem;

export interface ReceiptPayment {
  id: number;
  paymentMethod: PaymentMethod;
  amountMinor: number;
  createdAt: string;
}

export interface ReceiptLink {
  id: number;
  receiptNumber: string;
  documentType: ReceiptDocumentType;
  status: ReceiptStatus;
  createdAt: string;
  totalMinor: number;
}

export type ReceiptLinkedDocument = ReceiptLink;

export interface ReceiptDetail extends ReceiptSummary {
  subtotalMinor: number;
  discountMinor: number;
  taxMinor: number;
  originalSaleId?: number | null;
  originalReceiptNumber?: string | null;
  voidReason?: string | null;
  returnReason?: string | null;
  esirReceiptNumber?: string | null;
  items: ReceiptItem[];
  payments: ReceiptPayment[];
  linkedDocuments: ReceiptLink[];
  canVoid: boolean;
  canReturn: boolean;
}

export interface VoidReceiptRequest {
  receiptId: number;
  reason: string;
}

export interface ReturnItemRequest {
  saleItemId: number;
  quantityMilli: number;
}

export interface ReturnItemsRequest {
  receiptId: number;
  reason: string;
  items: ReturnItemRequest[];
  refundTender?: PaymentMethod;
}

export interface ReportDateQuery {
  from: string;
  to: string;
  shiftId?: number | null;
  cashierId?: number | null;
}

export interface ProductSalesQuery extends ReportDateQuery {
  categoryId: number | null;
  productId: number | null;
}

export interface DailyTurnoverSummary {
  totalMinor: number;
  cashMinor: number;
  cardMinor: number;
  bankTransferMinor: number;
  receiptCount: number;
  averageReceiptMinor: number;
}

export interface DailyTurnoverRow {
  day: string;
  receiptCount: number;
  cashMinor: number;
  cardMinor: number;
  bankTransferMinor: number;
  totalMinor: number;
  refundsOrVoidsMinor: number;
  refundsOrVoidsCount: number;
}

export interface DailyTurnoverReport {
  summary: DailyTurnoverSummary;
  rows: DailyTurnoverRow[];
}

export interface ShiftTurnoverRow {
  shiftId: number;
  openedAt: string;
  closedAt: string | null;
  cashierName: string;
  receiptCount: number;
  cashMinor: number;
  cardMinor: number;
  bankTransferMinor: number;
  totalMinor: number;
}

export interface ShiftTurnoverReport {
  rows: ShiftTurnoverRow[];
}

export interface CashierTurnoverRow {
  cashierId: number;
  cashierName: string;
  receiptCount: number;
  totalMinor: number;
}

export interface CashierTurnoverReport {
  rows: CashierTurnoverRow[];
}

export interface PaymentMethodRow {
  paymentMethod: PaymentMethod;
  receiptCount: number;
  totalMinor: number;
}

export interface PaymentMethodReport {
  rows: PaymentMethodRow[];
}

export interface ProductSalesRow {
  productId: number | null;
  productName: string;
  productSku: string;
  quantityMilli: number;
  revenueMinor: number;
  discountMinor: number;
  estimatedMarginMinor: number;
}

export interface ProductSalesReport {
  rows: ProductSalesRow[];
}

export interface CategorySalesRow {
  categoryId: number | null;
  categoryName: string;
  quantityMilli: number;
  revenueMinor: number;
  discountMinor: number;
  estimatedMarginMinor: number;
}

export interface CategorySalesReport {
  rows: CategorySalesRow[];
}

export interface LowStockRow {
  productId: number;
  productName: string;
  productSku: string;
  currentStockMilli: number;
  minimumStockMilli: number;
  differenceMilli: number;
  lastMovementAt: string | null;
}

export interface LowStockReport {
  rows: LowStockRow[];
}

export type ExportReportType =
  | "dailyTurnover"
  | "shiftTurnover"
  | "cashierTurnover"
  | "paymentMethods"
  | "productSales"
  | "categorySales"
  | "lowStock";

export interface ExportReportRequest {
  reportType: ExportReportType;
  query: ProductSalesQuery;
}

export interface ExportedFile {
  fileName: string;
  path: string;
  mimeType: "text/csv" | "text/html";
  rowCount: number;
}

export type ImportType = "products" | "categories" | "initial_stock";
export type ImportRowStatus =
  | "valid"
  | "warning"
  | "error"
  | "imported"
  | "skipped";
export type ImportRowAction = "create" | "update" | "skip";
export type ImportJobStatus = "draft" | "validated" | "completed" | "failed";
export type ImportMapping = Record<string, string>;

export interface ReadImportHeadersRequest {
  importType: ImportType;
  fileName: string;
  csvText: string;
}

export interface ImportHeaders {
  importType: ImportType;
  fileName: string;
  delimiter: string;
  headers: string[];
  totalRows: number;
}

export interface ValidateImportRequest extends ReadImportHeadersRequest {
  mapping: ImportMapping;
}

export type CommitImportRequest = ValidateImportRequest;

export interface ImportSummary {
  create: number;
  update: number;
  skip: number;
}

export interface ImportRowPreview {
  rowNumber: number;
  status: ImportRowStatus;
  action?: ImportRowAction;
  message?: string | null;
  raw?: Record<string, string>;
  values?: Record<string, string>;
}

export type ImportRowResult = ImportRowPreview;
export type ImportValidationRow = ImportRowPreview;

export interface ImportValidationResult {
  importType: ImportType;
  fileName: string;
  totalRows: number;
  validCount: number;
  warningCount: number;
  errorCount: number;
  summary: ImportSummary;
  rows: ImportRowPreview[];
}

export interface ImportJobSummary {
  id: number;
  importType: ImportType;
  fileName: string;
  status: ImportJobStatus;
  totalRows: number;
  errorRows: number;
  createdAt: string;
  completedAt?: string | null;
}

export type ImportJob = ImportJobSummary;

export interface ImportJobDetail extends ImportJobSummary {
  rows: ImportRowPreview[];
}

export interface KepEntryView {
  redniBroj: number;
  datum: string;
  opis: string;
  zaduzenjeMinor: number | null;
  razduzenjeMinor: number | null;
  kind: string;
}

export interface KepLedger {
  bookYear: number;
  entries: KepEntryView[];
  openingSaldoMinor: number;
  saldoMinor: number;
}

export interface KepStatus {
  overdueSalesDays: string[];
  unbookedReceiptCount: number;
}

/** The close-preview payload — mirrors `crate::commands::kep::KepClosePreview`. */
export interface KepClosePreview {
  krajnjiSaldoMinor: number;
  entryCount: number;
  alreadyClosed: boolean;
}

/** A recorded year-end close — mirrors `crate::kep_close::KepClosure`. */
export interface KepClosure {
  bookYear: number;
  krajnjiSaldoMinor: number;
  entryCount: number;
  closedAt: string;
  closedBy: number | null;
}

/** A closure list row with the 5-year retention flag — `crate::kep_close::KepClosureView`. */
export interface KepClosureView {
  bookYear: number;
  krajnjiSaldoMinor: number;
  entryCount: number;
  closedAt: string;
  purgeEligible: boolean;
  /** `YYYY-MM-DD` the retention obligation ends (later of closedAt+5y / bookYear-end+5y). */
  retentionUntil: string;
}

/**
 * The frontend cause ids for a value-only storno — the strings
 * `crate::commands::kep::parse_adjustment_cause` accepts. The cause→{kolona,
 * sign, kind} booking is a HARD map decided backend-side, never a user choice.
 * Nivelacija (and the PDV-rate revaluations) change the product price and route
 * through the dedicated `nivelacija` method, so they are excluded here.
 */
export type StornoCauseId =
  | "supplier_return"
  | "customer_return"
  | "otpis"
  | "manjak_odluka"
  | "rashod"
  | "popis_visak"
  | "popis_manjak";

/**
 * A storno/nivelacija basis isprava (naziv/broj/datum) — mirrors
 * `crate::kep_storno::BasisDoc` (serde camelCase). Composed into the KEP `opis`.
 */
export interface BasisDoc {
  naziv: string;
  broj: string;
  datum: string;
}

/**
 * One kalkulacija list row — mirrors `crate::kep_kalkulacija::KalkulacijaSummary`
 * (serde camelCase). `razlikaUCeniMinor` (element 10, the marža) is derived
 * backward from the catalog price and MAY be negative for a loss-leader.
 */
export interface KalkulacijaSummary {
  id: number;
  redniBroj: number;
  bookYear: number;
  trgovackiNaziv: string;
  kolicinaMilli: number;
  razlikaUCeniMinor: number;
  prodajnaVrednostSaPdvMinor: number;
  createdAt: string;
}
