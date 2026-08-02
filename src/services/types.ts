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
  /**
   * The two ZF čl. 6 st. 4 carve-outs (§3 req 32): retail conducted
   * exclusively over the internet, and retail of the shop's own used movable
   * assets. `true` is the LENIENT branch — it excuses the per-premises L-PFR
   * floor — so `null` must never be read as either answer. An unanswered
   * carve-out leaves the čl. 6 st. 4 duty standing.
   */
  lpfrCarveOutInternetOnly: boolean | null;
  lpfrCarveOutOwnUsedAssets: boolean | null;
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
 * `commands::settings::EurRateStatus` — the cached rate plus the staleness
 * verdict for the day it was judged against.
 *
 * An absent `rate` is `isStale: true`, never "fine": the AML čl. 46 st. 1
 * threshold is derived from the rate, so silence reads as „nepoznato", not as
 * „kurs nije potreban". `checkedFor` travels with the verdict so the surface
 * can name *which* day the rate was judged against instead of implying „sada".
 */
export interface EurRateStatus {
  rate: EurRate | null;
  isStale: boolean;
  /** `YYYY-MM-DD` — the day `isStale` was judged against. */
  checkedFor: string;
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
  /**
   * Cash the Pravilnik 77/2011 čl. 5 st. 2 carve-out kept out of the base —
   * only those podizanja marked as paid out per čl. 2 st. 2 or st. 3. Any other
   * withdrawal stays in the base and carries its own rok.
   */
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
  /**
   * `bank_withdrawal` only: the operator's declaration that this payout was made
   * per Pravilnik 77/2011 čl. 2 st. 2 (uz originalnu dokumentaciju podnetu banci
   * na uvid i overu) or čl. 2 st. 3 (dnevni limit od 150.000 dinara) — the sole
   * condition on which čl. 5 st. 2 keeps the money out of the čl. 3 st. 1
   * deposit base.
   *
   * `null` means nothing was declared and is the default. Only `true` excludes;
   * `null` and `false` alike leave the podizanje in the base, because a wrongly
   * excluded amount can show a false „izmireno" state.
   */
  documentedPerPravilnik?: boolean | null;
}

/**
 * The ZoR čl. 87–91 protection inputs plus the two ZEOR čl. 44 st. 2 codes, as
 * the Users screen edits them.
 *
 * Two absences are deliberate and must stay that way:
 *
 * 1. `saglasnostPreraspodelaOd` (ZoR čl. 57 st. 4) is **not** here. It is a
 *    legally distinct written consent from the čl. 91 one, and neither may be
 *    read or written for the other's purpose.
 * 2. No free text. `trudnocaIliDojenje` is a flag and an „od“ date and nothing
 *    else — the nalaz nadležnog zdravstvenog organa that čl. 90 conditions the
 *    prohibition on is never entered, attached or described here.
 *
 * Neither consent date collects a consent; each records that a written one
 * exists and from when.
 */
export interface EmployeeProfile {
  datumRodjenja: string | null;
  datumRodjenjaNajmladjegDeteta: string | null;
  samohraniRoditelj: boolean | null;
  deteTezakInvalid: boolean | null;
  trudnocaIliDojenje: boolean | null;
  trudnocaIliDojenjeOd: string | null;
  radiUPreraspodeli: boolean;
  ugovorenoRadnoVremeMinutaNedeljno: number | null;
  zanimanjeSifra: string | null;
  kvalifikacijaSifra: string | null;
  saglasnostPrekovremeniOd: string | null;
}

export interface SaveUserRequest {
  username: string;
  displayName: string;
  role: UserRole;
  active: boolean;
  pin?: string | null;
  password?: string | null;
  /**
   * Omitted means „this save does not carry the employee profile“ and the
   * stored čl. 87–91 inputs stay exactly as they are.
   */
  profile?: EmployeeProfile | null;
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

/**
 * One day's hours in the statutory buckets — mirrors
 * `crate::commands::worktime::WorkTimeMinutes` (serde camelCase).
 *
 * **Integer minutes, never hours and never floating point.** `ukupnoOstvareni`
 * and `ukupnoNeizvrseni` are DERIVED backend-side from the buckets they
 * enumerate; nothing on this side may recompute or override them.
 *
 * `nocniMinuta` and `radNaPraznikMinuta` are advisory — no Serbian provision
 * requires either, so every surface tags them `izračunato radi provere
 * usklađenosti` and never as a statutory field (§4 req. 4).
 */
export interface WorkTimeMinutes {
  moguciMinuta: number;
  ukupnoOstvareniMinuta: number;
  efektivnoIzvrseniMinuta: number;
  casoviCekanjaIZastojaMinuta: number;
  obustavaRadaStrajkMinuta: number;
  ukupnoNeizvrseniMinuta: number;
  godisnjiOdmorMinuta: number;
  praznikOdmorMinuta: number;
  odsustvoUzNaknaduMinuta: number;
  strucnoOsposobljavanjeMinuta: number;
  sprecenostPoslodavacMinuta: number;
  naknadaDrugiPoslodavciMinuta: number;
  sprecenostRfzoMinuta: number;
  porodiljskoMinuta: number;
  neplacenoOdsustvoMinuta: number;
  prekovremeniMinuta: number;
  nocniMinuta: number;
  radNaPraznikMinuta: number;
}

/**
 * The closed absence vocabulary migration v17's `CHECK` accepts, mirroring
 * `crate::commands::worktime::KATEGORIJE_ODSUSTVA`. A closed union because
 * there is no free-text sibling and never will be — a free-text column would
 * eventually be filled with a diagnosis (§5 item 4).
 */
export type AbsenceCategory =
  | "godisnji_odmor"
  | "praznik_odmor"
  | "odsustvo_uz_naknadu"
  | "strucno_osposobljavanje"
  | "sprecenost_poslodavac"
  | "sprecenost_rfzo"
  | "porodiljsko"
  | "neplaceno_odsustvo"
  | "naknada_drugi_poslodavci"
  | "obustava_rada_strajk";

/** The ZoR čl. 53 st. 1 grounds — `crate::commands::worktime::CAP_OVERRIDE_RAZLOZI`. */
export type CapOverrideReason =
  | "visa_sila"
  | "iznenadno_povecanje_obima_posla"
  | "neplanirani_posao_u_roku"
  | "drugo";

/** Correction reasons — `crate::commands::worktime::KOREKCIJA_RAZLOZI`. */
export type CorrectionReason =
  | "greska_u_unosu"
  | "ispravka_sati"
  | "ispravka_kategorije"
  | "naknadno_dostavljen_dokument"
  | "drugo";

/**
 * One stored version of one day — `crate::commands::worktime::WorkTimeEntryView`.
 * The whole chain is returned, not just the live row: `zamenjen` marks a version
 * a later one supersedes, and it is rendered struck through rather than hidden
 * (§4 req. 6).
 */
export interface WorkTimeEntryView {
  id: number;
  userId: number;
  dan: string;
  verzija: number;
  zamenjen: boolean;
  supersedesId: number | null;
  kategorijaOdsustva: string | null;
  capOverrideRazlog: string | null;
  korekcijaRazlog: string | null;
  unioUserId: number | null;
  unioIme: string | null;
  createdAt: string;
  updatedAt: string;
  minuti: WorkTimeMinutes;
}

/** One employee's month — `crate::commands::worktime::WorkTimeMonth`. */
export interface WorkTimeMonth {
  userId: number;
  zaposleni: string;
  godina: number;
  mesec: number;
  zatvoren: boolean;
  closedAt: string | null;
  entries: WorkTimeEntryView[];
  /** Live rows only. */
  ukupno: WorkTimeMinutes;
  /** The čl. 55 st. 6 sentence, carried so no surface retypes it. */
  napomena: string;
  /** The advisory tag for the two computed columns (§4 req. 4). */
  advisoryNapomena: string;
}

/** The frozen Class A classification — `crate::commands::worktime::PeriodClassification`. */
export interface WorkTimePeriodClassification {
  userId: number;
  godina: number;
  mesec: number;
  danaSaUnosom: number;
  minuti: WorkTimeMinutes;
  izvedenoU: string;
}

/** A recorded period close — `crate::commands::worktime::ClosedPeriod`. */
export interface WorkTimeClosedPeriod {
  userId: number;
  godina: number;
  mesec: number;
  closedAt: string;
  closedBy: number;
  klasifikacija: WorkTimePeriodClassification;
}

/**
 * One day as the operator enters it — `crate::commands::worktime::SaveEntryRequest`.
 * The b) and v) totals are absent by design: they are derived backend-side.
 */
export interface SaveWorkTimeEntryRequest {
  userId: number;
  dan: string;
  moguciMinuta: number;
  efektivnoIzvrseniMinuta: number;
  casoviCekanjaIZastojaMinuta: number;
  prekovremeniMinuta: number;
  nocniMinuta: number;
  radNaPraznikMinuta: number;
  kategorijaOdsustva: AbsenceCategory | null;
  odsustvoMinuta: number;
  capOverrideRazlog: CapOverrideReason | null;
}

/** `crate::commands::worktime::CorrectEntryRequest` — the flattened entry plus a reason. */
export interface CorrectWorkTimeEntryRequest extends SaveWorkTimeEntryRequest {
  korekcijaRazlog: CorrectionReason;
}

/**
 * The ZoR čl. 53 assessment a write was measured against — mirrors
 * `crate::worktime::CapAssessment`. Minutes throughout.
 */
export interface WorkTimeCapAssessment {
  weeklyOvertimeMinutes: number;
  dailyTotalMinutes: number;
  weeklyTotalMinutes: number;
  weeklyCapExceeded: boolean;
  dailyCapExceeded: boolean;
  preraspodelaWeeklyCapExceeded: boolean;
  requiresOverride: boolean;
}

/** `crate::worktime::ProtectionKind`, serde camelCase. */
export type WorkTimeProtectionKind =
  | "maloletanPrekovremeni"
  | "maloletanPreraspodela"
  | "maloletanDnevniLimit"
  | "saglasnostRoditelja"
  | "trudnocaNocniIPrekovremeni"
  | "neispravanDatumUProfilu";

/**
 * A čl. 87–91 finding — `crate::worktime::ProtectionBlock`. `blocking` is `true`
 * only where the statute states the prohibition itself; a čl. 90 finding is
 * conditional on a nalaz nadležnog zdravstvenog organa the app never holds.
 */
export interface WorkTimeProtectionBlock {
  kind: WorkTimeProtectionKind;
  blocking: boolean;
  poruka: string;
}

/** What a write returns — `crate::commands::worktime::SavedEntry`. */
export interface SavedWorkTimeEntry {
  entry: WorkTimeEntryView;
  caps: WorkTimeCapAssessment;
  protections: WorkTimeProtectionBlock[];
}

/**
 * The ZoR notices the register surfaces, already resolved against the stored
 * legal form — `crate::commands::worktime::WorkTimeNotices`. Every penalty
 * figure is authored in `legal.rs` and rides here; this side never composes one.
 *
 * `capsExceeded` and `preraspodelaCapsExceeded` are **not** interchangeable.
 * čl. 58 says hours worked in preraspodela are not prekovremeni rad, so the
 * čl. 53 caps do not bind such an employee, and the offence is čl. 274 st. 1
 * tač. 4 rather than tač. 3. The amount is the same under both tačke, so putting
 * the wrong one on screen ships no wrong figure — only a wrong article and a
 * rule that does not apply.
 */
export interface WorkTimeNotices {
  recordMissing: LegalNotice;
  capsExceeded: LegalNotice;
  preraspodelaCapsExceeded: LegalNotice;
}

// ---------------------------------------------------------------------------
// SW-10 / SW-13 / SW-17 — the ZZPL trio
// ---------------------------------------------------------------------------

/**
 * One `support_sessions` row — `crate::commands::audit::SupportSession`.
 *
 * The row **is** the ZZPL čl. 46 nalog: who authorised the obrađivač, when, for
 * what obim and until when. `startedAt` is stamped when the support side
 * actually enters; a nalog closed after entry gets `endedAt`, one withdrawn
 * before anyone used it gets `revokedAt`, and the two are different facts.
 */
export interface SupportSession {
  id: number;
  grantedBy: number;
  grantedByName: string;
  grantedAt: string;
  scope: string;
  expiresAt: string;
  startedAt: string | null;
  endedAt: string | null;
  revokedAt: string | null;
}

/**
 * Req. 8's two axes and no third one — `crate::commands::audit::AuditQuery`.
 * Both days are `gggg-MM-dd` and both bounds are inclusive; anything that is
 * not a bare day is refused backend-side rather than silently mis-filtered.
 */
export interface AuditQuery {
  from: string | null;
  to: string | null;
  actorUserId: number | null;
}

/**
 * One logged row as the operator reads it — `crate::commands::audit::AuditEvent`.
 *
 * `actorName` is resolved from `users` at read time and is **not** stored on the
 * row: the čl. 5 st. 1 t. 3 exclusion list governs `audit_events`, and čl. 48
 * st. 2's *identitet lica* is answered by the id the table does store.
 */
export interface AuditEvent {
  id: number;
  at: string;
  actorUserId: number | null;
  actorName: string | null;
  action: string;
  actionLabel: string;
  objectType: string;
  objectTypeLabel: string;
  objectId: string;
  reasonCode: string | null;
  reasonLabel: string | null;
  recipient: string | null;
  recipientLabel: string | null;
  supportSessionId: number | null;
  prevHash: string;
  hash: string;
}

/**
 * `crate::audit::ChainVerdict`, serde camelCase and externally tagged.
 * `brokenAt` is a zero-based index into the surviving log.
 */
export type ChainVerdict = "intact" | "truncated" | { brokenAt: number };

/**
 * What the hash chain says about the WHOLE log, never about the filtered slice
 * — `crate::commands::audit::ChainStatus`. `label` is authored backend-side so
 * the panel and the izvod can never disagree about the verdict.
 */
export interface ChainStatus {
  verdict: ChainVerdict;
  intact: boolean;
  checkedRows: number;
  label: string;
}

/** `crate::commands::audit::AuditSearchResult`. */
export interface AuditSearchResult {
  events: AuditEvent[];
  chain: ChainStatus;
}

/** `crate::commands::breaches::RiskOutcome`, serde snake_case. */
export type RiskOutcome = "bez_rizika" | "rizik" | "visok_rizik";

/** `crate::commands::breaches::NotifyDecision`, serde snake_case. */
export type NotifyDecision = "obavestiti" | "ne_obavestiti";

/** The three čl. 53 st. 3 exceptions, closed — `crate::commands::breaches::Cl53Izuzetak`. */
export type Cl53Izuzetak =
  | "primenjene_mere_zastite"
  | "naknadne_mere"
  | "nesrazmeran_utrosak_vremena_i_sredstava";

/**
 * What the operator submits for one povreda — `crate::commands::breaches::BreachDraft`.
 *
 * `saznanjeAt` rides on the update path too so a client that round-trips the
 * record cannot silently drop it; the backend compares it against the stored
 * instant and refuses a change rather than ignoring one.
 */
export interface BreachDraft {
  saznanjeAt: string;
  occurredAt: string | null;
  discoveredAt: string | null;
  obradjivacSaznanjeAt: string | null;
  rukovalacObavestenAt: string | null;
  opis: string;
  posledice: string;
  mere: string;
  brojLica: number | null;
  kategorijePodataka: string | null;
  riskOutcome: RiskOutcome | null;
  notifyDecision: NotifyDecision | null;
  notifyObrazlozenje: string | null;
  poverenikNotifiedAt: string | null;
  delayReason: string | null;
  licaObavestena: boolean | null;
  licaObavestenaAt: string | null;
  cl53Izuzetak: Cl53Izuzetak | null;
  cl53IzuzetakObrazlozenje: string | null;
}

/**
 * One stored povreda — `crate::commands::breaches::Breach`: every column, plus
 * the four answers computed from `now` and therefore never stored.
 *
 * `notifiable` is a **derived flag on a row that always exists** (req. 43). It
 * never decided whether the record was written.
 */
export interface Breach extends BreachDraft {
  id: number;
  createdAt: string;
  updatedAt: string;
  /** Saznanje + 72 h — Pravilnik 40/2019 čl. 3. */
  rokObavestavanjaIsticeAt: string;
  notifiable: boolean | null;
  /** Čl. 52 st. 2 — whether a delay justification is owed as of now. */
  delayReasonRequired: boolean;
  /** Čl. 53 st. 1 — whether the affected individuals must be told. */
  obavestavanjeLicaObavezno: boolean;
}

/** `crate::retention::RecordClass`, serde snake_case. */
export type RecordClass =
  | "worktime_classification"
  | "worktime_overtime_log"
  | "worktime_draft"
  | "personnel"
  | "credentials"
  | "access_log"
  | "processing_register";

/**
 * One row of the shared retention table — `crate::commands::retention::
 * RetentionPolicyView`.
 *
 * **`adjustable` is not the negation of `neverPurge`.** It answers whether a
 * registered command can actually move this class's rok
 * (`crate::commands::retention::AdjustableClass`), which is the only sense in
 * which a screen may offer the shop a period to change. The two can disagree
 * only by mistake, and this is the side that has to be true.
 *
 * `retainUntil` is the earliest day on which the class may be discarded, not a
 * day on which anything is discarded. `null` means trajno: the absence of an
 * end, never „no rule“.
 */
export interface RetentionPolicy {
  recordClass: RecordClass;
  naziv: string;
  retainUntil: string | null;
  legalHold: boolean;
  neverPurge: boolean;
  adjustable: boolean;
  napomena: string;
  updatedAt: string;
}

/**
 * One generated radnja obrade — `crate::cl47::ProcessingActivity`.
 *
 * The register is generated from the app's own configured purposes, recipients
 * and retention rows, never typed by the operator: a register that can be
 * edited into agreement with whatever the till happens to do documents nothing.
 */
export interface ProcessingActivity {
  id: number;
  kljuc: string;
  /** St. 1 t. 1. */
  rukovalacNaziv: string;
  rukovalacKontakt: string | null;
  /** St. 1 t. 2. */
  svrhaObrade: string;
  /** St. 1 t. 3. */
  vrstaLica: string;
  vrstaPodataka: string;
  /** St. 1 t. 4. */
  vrstaPrimalaca: string | null;
  /** St. 1 t. 5. */
  prenosUDrugeDrzave: string | null;
  mereZastitePrenosa: string | null;
  /** St. 1 t. 6 — the period, per category. */
  rokCuvanja: string | null;
  retentionRecordClass: RecordClass | null;
  /** St. 1 t. 7. */
  opisMeraZastite: string | null;
  updatedAt: string;
}
