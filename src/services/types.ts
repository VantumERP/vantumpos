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

export type PaymentMethod = "cash" | "card";

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

export interface CashMovementRequest {
  direction: "pay_in" | "pay_out";
  amountMinor: number;
  reason?: string | null;
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
  externalSource?: ProductExternalSource | null;
}

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

export interface InventoryAdjustmentResult {
  productId: number;
  movementId: number;
  movementType: InventoryMovementType;
  quantityMilli: number;
  previousQuantityMilli: number;
  newQuantityMilli: number;
  createdAt: string;
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
  refundTender?: "cash" | "card";
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
  receiptCount: number;
  averageReceiptMinor: number;
}

export interface DailyTurnoverRow {
  day: string;
  receiptCount: number;
  cashMinor: number;
  cardMinor: number;
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
  mimeType: "text/csv";
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
