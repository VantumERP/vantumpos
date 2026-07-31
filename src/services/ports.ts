import type {
  AmlAssessment,
  AppHealth,
  AuthSession,
  BackupJob,
  BackupSettings,
  BackupStatus,
  CampaignInput,
  CampaignSummary,
  CampaignValidationReport,
  CampaignView,
  CashDepositCalendar,
  CashDepositReport,
  CashierTurnoverReport,
  CorrectionReport,
  CashMovementRequest,
  CategorySummary,
  CategorySalesReport,
  CloseShiftRequest,
  CommitImportRequest,
  CompleteSaleRequest,
  CompletedSale,
  CompanySettings,
  CompanySettingsRequest,
  CreateBackupRequest,
  DailyTurnoverReport,
  DeclarationGapRow,
  EndCampaignOverride,
  EurRateStatus,
  ExportedFile,
  ExportReportRequest,
  ImportHeaders,
  ImportJob,
  ImportJobDetail,
  KepClosePreview,
  KepClosure,
  KepClosureView,
  KepEntryView,
  KepLedger,
  KepStatus,
  InventoryAdjustmentRequest,
  InventoryAdjustmentResult,
  LowStockReport,
  LoginRequest,
  OpenShiftRequest,
  PaymentMethodReport,
  PrethodnaCenaDto,
  ProductLedger,
  ProductListQuery,
  ProductListResult,
  ProductLookupSuggestion,
  ProductSalesQuery,
  ProductSalesReport,
  ProductSearchQuery,
  ProductSummary,
  AnswerInput,
  BasisDoc,
  KalkulacijaSummary,
  StornoCauseId,
  ReklamacijaInput,
  ReklamacijaSummary,
  ReklamacijaView,
  ReportDateQuery,
  ReadImportHeadersRequest,
  ReceiptDetail,
  ReceiptSettings,
  ReceiptSettingsRequest,
  SalesSettings,
  SalesSettingsRequest,
  ReceiptSearchQuery,
  ReceiptSearchResult,
  RestoreBackupRequest,
  ReturnItemsRequest,
  SaleDraftRequest,
  SalePreview,
  SaveCategoryRequest,
  SaveProductRequest,
  SaveTaxRateRequest,
  SaveUserRequest,
  ShiftListItem,
  ShiftTurnoverReport,
  ShiftSummary,
  ShopProfile,
  StockListQuery,
  StockListResult,
  UserAccount,
  ValidateImportRequest,
  ImportValidationResult,
  VoidReceiptRequest,
  TaxRate,
} from "./types";

export interface SettingsService {
  getHealth(): Promise<AppHealth>;
  getCompanySettings(): Promise<CompanySettings>;
  updateCompanySettings(request: CompanySettingsRequest): Promise<CompanySettings>;
  listTaxRates(): Promise<TaxRate[]>;
  saveTaxRate(request: SaveTaxRateRequest): Promise<TaxRate>;
  seedTaxRates(inVatSystem: boolean): Promise<TaxRate[]>;
  getReceiptSettings(): Promise<ReceiptSettings>;
  updateReceiptSettings(request: ReceiptSettingsRequest): Promise<ReceiptSettings>;
  getSalesSettings(): Promise<SalesSettings>;
  updateSalesSettings(request: SalesSettingsRequest): Promise<SalesSettings>;
  getShopProfile(): Promise<ShopProfile>;
  updateShopProfile(request: ShopProfile): Promise<ShopProfile>;
  /**
   * The cached NBS middle rate the AML čl. 46 st. 1 dinar threshold is derived
   * from. Read-only and never admin-gated: the till needs to be able to say
   * *why* the check could not run.
   */
  getEurRate(): Promise<EurRateStatus>;
  /**
   * Refetches from the NBS. Admin-gated backend-side, and **degrading**: a
   * failed fetch resolves with the cached rate plus `isStale` instead of
   * rejecting, because an unreachable NBS must never read as an error the
   * operator has to clear before selling.
   */
  refreshEurRate(): Promise<EurRateStatus>;
  /**
   * The manual fallback for the days the NBS is unreachable. `rateMinor` is
   * para per 1 EUR; `rateDate` is `YYYY-MM-DD`. Admin-gated, band-checked and
   * date-checked backend-side — this value sets the dinar threshold the AML
   * warning is computed from, so a mistyped digit is the dangerous case.
   */
  setManualEurRate(rateMinor: number, rateDate: string): Promise<EurRateStatus>;
  /**
   * The calendar the seven-working-day deposit deadline is counted against.
   * Admin-gated backend-side, and seeded with the Serbian state holidays on
   * first read. Every method returns the whole calendar so the panel can never
   * drift from what was stored.
   */
  getCashDepositCalendar(): Promise<CashDepositCalendar>;
  setSaturdayIsWorking(counts: boolean): Promise<CashDepositCalendar>;
  saveNonWorkingDay(day: string, label: string): Promise<CashDepositCalendar>;
  deleteNonWorkingDay(day: string): Promise<CashDepositCalendar>;
}

export interface BackupService {
  getBackupStatus(): Promise<BackupStatus>;
  updateBackupSettings(request: BackupSettings): Promise<BackupSettings>;
  createBackup(request: CreateBackupRequest): Promise<BackupJob>;
  restoreBackup(request: RestoreBackupRequest): Promise<BackupJob>;
  listBackupJobs(): Promise<BackupJob[]>;
  setBackupPassphrase(passphrase: string): Promise<void>;
  resetTradingData(confirmationText: string): Promise<void>;
}

export interface AuthService {
  getSession(): Promise<AuthSession | null>;
  login(request: LoginRequest): Promise<AuthSession>;
  logout(): Promise<void>;
}

export interface UsersService {
  listUsers(): Promise<UserAccount[]>;
  createUser(request: SaveUserRequest): Promise<UserAccount>;
  updateUser(id: number, request: SaveUserRequest): Promise<UserAccount>;
  deactivateUser(id: number): Promise<void>;
}

export interface ShiftService {
  getCurrentShift(): Promise<ShiftSummary | null>;
  openShift(request: OpenShiftRequest): Promise<ShiftSummary>;
  closeShift(request: CloseShiftRequest): Promise<ShiftSummary>;
  adminCloseShift(request: CloseShiftRequest): Promise<ShiftSummary>;
  shiftCashMovement(request: CashMovementRequest): Promise<ShiftSummary>;
}

export interface CatalogService {
  listProducts(query: ProductListQuery): Promise<ProductListResult>;
  searchProducts(query: ProductSearchQuery): Promise<ProductListResult>;
  getProduct(id: number): Promise<ProductSummary | null>;
  createProduct(request: SaveProductRequest): Promise<ProductSummary>;
  updateProduct(id: number, request: SaveProductRequest): Promise<ProductSummary>;
  setProductActive(id: number, active: boolean): Promise<ProductSummary>;
  lookupProductByBarcode(barcode: string): Promise<ProductLookupSuggestion | null>;
  listCategories(): Promise<CategorySummary[]>;
  saveCategory(request: SaveCategoryRequest): Promise<CategorySummary>;
  getPrethodnaCena(productId: number, campaignStart: string): Promise<PrethodnaCenaDto>;
  /**
   * Active articles whose ZoT čl. 34 deklaracija evidence is incomplete.
   * Read-only and admin-gated; every penalty figure rides per row, never on
   * the report as a whole (§3 req 26).
   */
  declarationGaps(): Promise<DeclarationGapRow[]>;
}

export interface SalesService {
  createSalePreview(request: SaleDraftRequest): Promise<SalePreview>;
  completeSale(request: CompleteSaleRequest): Promise<CompletedSale>;
  /**
   * AML čl. 46 st. 1 verdict for the **cash line** of the tender, never the
   * invoice total. Advisory only: the till warns and asks for a reason, it
   * never refuses the sale.
   */
  assessCashPayment(cashMinor: number): Promise<AmlAssessment>;
}

export interface InventoryService {
  listStock(query: StockListQuery): Promise<StockListResult>;
  receiveStock(request: InventoryAdjustmentRequest): Promise<InventoryAdjustmentResult>;
  correctStock(request: InventoryAdjustmentRequest): Promise<InventoryAdjustmentResult>;
  writeOffStock(request: InventoryAdjustmentRequest): Promise<InventoryAdjustmentResult>;
  getProductLedger(productId: number): Promise<ProductLedger>;
  /**
   * Stamps *who looked and when* at the goods' physical deklaracija — the
   * ZoT čl. 69a tač. 4 mitigation record. It is a mitigating circumstance in
   * sentencing, not a defence, and it asserts nothing about the catalog fields.
   */
  markDeclarationChecked(productId: number): Promise<void>;
}

export interface ReceiptsService {
  searchReceipts(query: ReceiptSearchQuery): Promise<ReceiptSearchResult>;
  getReceipt(id: number): Promise<ReceiptDetail | null>;
  voidReceipt(request: VoidReceiptRequest): Promise<ReceiptDetail>;
  returnItems(request: ReturnItemsRequest): Promise<ReceiptDetail>;
  setEsirNumber(receiptId: number, esirReceiptNumber: string): Promise<ReceiptDetail>;
}

export interface ImportService {
  readImportHeaders(request: ReadImportHeadersRequest): Promise<ImportHeaders>;
  validateImport(request: ValidateImportRequest): Promise<ImportValidationResult>;
  commitImport(request: CommitImportRequest): Promise<ImportJob>;
  listImportJobs(): Promise<ImportJob[]>;
  getImportJob(id: number): Promise<ImportJobDetail | null>;
}

export interface CampaignsService {
  listCampaigns(): Promise<CampaignSummary[]>;
  getCampaign(id: number): Promise<CampaignView>;
  /** Dry run for the wizard's live feedback. Persists nothing. */
  validateCampaign(input: CampaignInput): Promise<CampaignValidationReport>;
  createCampaign(input: CampaignInput): Promise<CampaignView>;
  /** Draft-only: an activated campaign's anchor never moves. */
  updateCampaign(id: number, input: CampaignInput): Promise<CampaignView>;
  activateCampaign(id: number): Promise<CampaignView>;
  adjustItemPrice(
    campaignId: number,
    productId: number,
    newPriceMinor: number,
  ): Promise<CampaignView>;
  endCampaign(
    id: number,
    overrides: EndCampaignOverride[],
  ): Promise<CampaignView>;
  cancelCampaign(id: number): Promise<CampaignView>;
  /** Self-contained walk-away price-evidence document (čl. 48). */
  exportEvidence(campaignId: number): Promise<ExportedFile>;
  /** Shelf-label sheet branching on type/display-mode (čl. 37). */
  exportLabels(campaignId: number): Promise<ExportedFile>;
  /** Active-campaign label data with attention flags — on-screen. */
  correctionReport(): Promise<CorrectionReport>;
  /** The same correction report rendered to a self-contained HTML export. */
  exportCorrectionReport(): Promise<ExportedFile>;
}

/**
 * The admin-gated reklamacije register. Deadlines are DERIVED per-read by the
 * backend, never stored, and the regime is frozen at `create`; the ten methods
 * map 1:1 to the `reklamacija_*` command names.
 */
export interface ReklamacijeService {
  list(): Promise<ReklamacijaSummary[]>;
  get(id: number): Promise<ReklamacijaView>;
  create(input: ReklamacijaInput): Promise<ReklamacijaView>;
  logAnswer(id: number, input: AnswerInput): Promise<ReklamacijaView>;
  consumerReceived(id: number, eventDate: string): Promise<ReklamacijaView>;
  consumerResponded(id: number, eventDate: string): Promise<ReklamacijaView>;
  grantExtension(
    id: number,
    newDeadline: string,
    consumerConsent: boolean,
    reason: string,
    eventDate: string,
  ): Promise<ReklamacijaView>;
  resolve(id: number, nacin: string, eventDate: string): Promise<ReklamacijaView>;
  /** Self-contained potvrda o prijemu reklamacije (čl. 55 st. 7 / čl. 63 st. 7). */
  exportPotvrda(id: number): Promise<ExportedFile>;
  /** The statutory prodajno-mesto display notice (čl. 55 st. 4 / čl. 63 st. 4). */
  exportNotice(): Promise<ExportedFile>;
}

/**
 * The admin-gated KEP (evidencija prometa) value ledger. The ledger view and
 * its running saldo are derived per-read by the backend; every method maps 1:1
 * to a `kep_*` command name. The kalkulacija + storno surface (9b) extends the
 * 9a ledger methods: `nivelacija` is the one adjustment that changes the product
 * price, so it has its own method; every other write-down routes through
 * `postAdjustment`, whose cause fixes the kolona and sign backend-side.
 */
export interface KepService {
  ledger(bookYear: number): Promise<KepLedger>;
  postDailySales(
    date: string,
    overrideAmountMinor: number | null,
  ): Promise<KepEntryView>;
  status(): Promise<KepStatus>;
  listKalkulacije(bookYear: number): Promise<KalkulacijaSummary[]>;
  exportKalkulacija(id: number): Promise<ExportedFile>;
  nivelacija(
    productId: number,
    newSalePriceMinor: number,
    basis: BasisDoc,
  ): Promise<void>;
  postAdjustment(
    cause: StornoCauseId,
    productId: number,
    quantityMilli: number,
    basis: BasisDoc,
  ): Promise<void>;
  correctEntry(
    targetRedniBroj: number,
    bookYear: number,
    correctAmountMinor: number,
    basis: BasisDoc,
  ): Promise<void>;
  closePreview(bookYear: number): Promise<KepClosePreview>;
  closeYear(bookYear: number, confirmation: string): Promise<KepClosure>;
  listClosures(): Promise<KepClosureView[]>;
  exportClose(bookYear: number): Promise<ExportedFile>;
  exportBook(bookYear: number): Promise<ExportedFile>;
}

export interface PrintService {
  /** Opens an exported document in the OS default handler for printing. */
  openForPrint(path: string): Promise<void>;
  /**
   * Opens an external URL in the system browser. Needed because a
   * `target="_blank"` anchor does nothing inside the Tauri webview.
   */
  openExternalUrl(url: string): Promise<void>;
}

export interface ReportsService {
  getDailyTurnover(query: ReportDateQuery): Promise<DailyTurnoverReport>;
  getShiftTurnover(query: ReportDateQuery): Promise<ShiftTurnoverReport>;
  getCashierTurnover(query: ReportDateQuery): Promise<CashierTurnoverReport>;
  getPaymentMethodTurnover(query: ReportDateQuery): Promise<PaymentMethodReport>;
  getProductSales(query: ProductSalesQuery): Promise<ProductSalesReport>;
  getCategorySales(query: ReportDateQuery): Promise<CategorySalesReport>;
  getLowStock(): Promise<LowStockReport>;
  listShifts(): Promise<ShiftListItem[]>;
  exportReportCsv(request: ExportReportRequest): Promise<ExportedFile>;
  /**
   * „Izveštaj o nedeponovanom gotovom novcu" as of `asOf` (`YYYY-MM-DD`) —
   * the presek is a real cut-off, so a past date answers „was I late then?".
   * Advisory: nothing in the app may gate a sale, a day-close or a
   * fiscalization on it.
   */
  getCashDepositReport(asOf: string): Promise<CashDepositReport>;
  exportCashDepositCsv(asOf: string): Promise<ExportedFile>;
}

export interface PosServices {
  settings: SettingsService;
  backup: BackupService;
  auth: AuthService;
  users: UsersService;
  shifts: ShiftService;
  catalog: CatalogService;
  sales: SalesService;
  inventory: InventoryService;
  receipts: ReceiptsService;
  imports: ImportService;
  reports: ReportsService;
  campaigns: CampaignsService;
  reklamacije: ReklamacijeService;
  kep: KepService;
  print: PrintService;
}
