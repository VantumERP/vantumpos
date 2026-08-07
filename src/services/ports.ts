import type {
  AmlAssessment,
  AppHealth,
  AuditQuery,
  AuditSearchResult,
  AuthSession,
  BackupJob,
  Breach,
  BreachDraft,
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
  CenovnikPublishTarget,
  CenovnikSnapshot,
  CenovnikSnapshotDetail,
  CloseShiftRequest,
  CommitImportRequest,
  CompleteSaleRequest,
  CompletedSale,
  CompanySettings,
  CompanySettingsRequest,
  CreateBackupRequest,
  DailyTurnoverReport,
  DeclarationGapRow,
  EmployeeProfile,
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
  LegalNotice,
  LowStockReport,
  LoginRequest,
  OpenShiftRequest,
  PaymentMethodReport,
  IzvestajRequest,
  IzvestajView,
  NivelacijaObuhvatId,
  NivelacijaObuhvatView,
  NivelacijaPregledView,
  OpenPopisRequest,
  PopisLineInput,
  PopisLista,
  PopisPodesavanja,
  PopisPrintFaza,
  PopisSessionView,
  PopisSummary,
  ProveraListiView,
  PrethodnaCenaDto,
  PriceDivergence,
  ProductLedger,
  ProductListQuery,
  ProductListResult,
  ProductLookupSuggestion,
  ProductSalesQuery,
  ProductSalesReport,
  ProductSearchQuery,
  ProductSummary,
  ProcessingActivity,
  RecordClass,
  RetentionPolicy,
  SupportSession,
  AnswerInput,
  BasisDoc,
  KalkulacijaSummary,
  NivelacijaObavestenje,
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
  CorrectWorkTimeEntryRequest,
  SaveWorkTimeEntryRequest,
  SavedWorkTimeEntry,
  WorkTimeClosedPeriod,
  WorkTimeMonth,
  WorkTimeNotices,
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
   * The ZF čl. 6 st. 4 duty (≥1 L-PFR per premises) with its penalty already
   * resolved against the **stored** legal form. Read-only and never
   * admin-gated. The frontend never derives the figure — it decides only
   * whether the duty is engaged, from the shop's own answers.
   */
  getLpfrNotice(): Promise<LegalNotice>;
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
  /**
   * One employee at a time, and only when an admin opens that employee's
   * account. `trudnocaIliDojenje` is health data — the user list must not carry
   * it for everyone just to draw a row.
   */
  getEmployeeProfile(id: number): Promise<EmployeeProfile>;
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
  /**
   * Articles in the draft priced above what the outlet's current cenovnik
   * publishes (ZZP čl. 6 st. 4, req. 12). Advisory in exactly the way
   * `assessCashPayment` is: `completeSale` accepts the sale whatever this
   * returns, and an empty answer is the ordinary case. Asked while the cart is
   * being built, so the operator learns about a divergence before the money
   * changes hands rather than from a log afterwards.
   */
  assessPriceIntegrity(request: SaleDraftRequest): Promise<PriceDivergence[]>;
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
  resolve(
    id: number,
    nacin: string,
    eventDate: string,
    noFeeAttested: boolean,
  ): Promise<ReklamacijaView>;
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
  /**
   * Returns the popis obligation the same price change raises (SW-16 req. 33 —
   * ZoRač čl. 21). It is a second, independent duty on one event: the KEP delta
   * does not discharge it. Surface it; do not treat it as a receipt.
   */
  nivelacija(
    productId: number,
    newSalePriceMinor: number,
    basis: BasisDoc,
  ): Promise<NivelacijaObavestenje>;
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

/**
 * The ZoR čl. 55 st. 6 daily working-time register. Every method maps 1:1 to a
 * `worktime_*` command name.
 *
 * Three properties of the backend this interface must not paper over. Writes are
 * **append-only**: `correctEntry` appends a superseding version and the
 * predecessor survives, so `listMonth` returns the whole chain and the caller
 * renders the superseded row struck through rather than dropping it. A ZoR
 * čl. 53 cap breach is **never a refusal** — `saveEntry` rejects with
 * `cap_override_required` and the same call succeeds once `capOverrideRazlog`
 * carries a čl. 53 st. 1 ground, because a register that cannot describe a day
 * that actually happened hides the exposure instead of surfacing it. A čl. 87–91
 * `protection_block` is the opposite and is final. `closePeriod` is
 * **irreversible** and there is no reopen method, by design.
 *
 * Every method except `myHours` is admin-gated backend-side; `myHours` is
 * session-gated to the caller's own rows.
 */
export interface WorkTimeService {
  listMonth(userId: number, godina: number, mesec: number): Promise<WorkTimeMonth>;
  saveEntry(request: SaveWorkTimeEntryRequest): Promise<SavedWorkTimeEntry>;
  correctEntry(request: CorrectWorkTimeEntryRequest): Promise<SavedWorkTimeEntry>;
  closePeriod(
    userId: number,
    godina: number,
    mesec: number,
  ): Promise<WorkTimeClosedPeriod>;
  /** Per-employee month, columns 1:1 onto the statutory buckets. Offline, from the till. */
  exportCsv(userId: number, godina: number, mesec: number): Promise<ExportedFile>;
  /** The employee's own read-only month (ZoR čl. 83 st. 1, ZZPL čl. 26). */
  myHours(godina: number, mesec: number): Promise<WorkTimeMonth>;
  /**
   * The ZoR notices with their penalty already resolved against the stored
   * legal form. Read-only: the frontend decides whether to show a duty, never
   * what it costs.
   */
  notices(): Promise<WorkTimeNotices>;
}

/**
 * The ZZPL trio: the čl. 46 remote-support nalog, the čl. 48 evidencija
 * pristupa, the čl. 52 internal breach record and the čl. 47 register of
 * processing activities. Every method maps 1:1 onto a Tauri command name.
 *
 * Three properties of the backend this interface must not paper over.
 *
 * **The evidencija pristupa has no write verb here and never may have one.**
 * Req. 7 puts tamper-evidence above completeness: nothing edits or removes a
 * logged row, and v18's trigger refuses the edit even if something tried. Every
 * line is written server-side by the feature that performed the radnja, so
 * there is no `recordAudit` on this surface — a frontend that could assert an
 * access happened could assert one that did not.
 *
 * **`recordBreach` is never gated on notifiability** (req. 43). Čl. 52 st. 6
 * documents *„svaku povredu“*; the risk test lives in st. 1 and governs only
 * whether the Poverenik is told. `Breach.notifiable` is a derived flag on a row
 * that always exists.
 *
 * **`exportBreachObrazac` produces a document, not a filing.** Pravilnik
 * 40/2019 čl. 5 is the whole route — in writing, in person or by post — and
 * there is no submission API to build.
 *
 * Every method here is admin-gated backend-side except `breachNotice`, which is
 * read-only and must be able to state the exposure before anything is recorded.
 */
export interface PrivacyService {
  /**
   * Issues the čl. 46 nalog. `durationMinutes` is integer minutes, like every
   * other duration in this app, and is bounded backend-side — a nalog measured
   * in weeks is an open-ended one with a date printed on it.
   */
  grantSupportAccess(scope: string, durationMinutes: number): Promise<SupportSession>;
  /**
   * The support side entering under a live nalog. Gated by the **nalog** and
   * nothing else: čl. 46 makes the nalog the condition, and the obrađivač holds
   * no account on this till.
   */
  enterSupportSession(): Promise<SupportSession>;
  /** The vlasnik closing the nalog — ended if it was entered, revoked if not. */
  endSupportSession(): Promise<SupportSession>;
  activeSupportSession(): Promise<SupportSession | null>;
  /** Req. 8's two axes; the chain verdict covers the whole log, not the slice. */
  searchAudit(query: AuditQuery): Promise<AuditSearchResult>;
  /** The čl. 48 st. 4 izvod, rendered offline from the till. */
  exportAuditCsv(query: AuditQuery): Promise<ExportedFile>;
  listBreaches(): Promise<Breach[]>;
  recordBreach(draft: BreachDraft): Promise<Breach>;
  updateBreach(id: number, draft: BreachDraft): Promise<Breach>;
  /** The čl. 52 exposure with its penalty already tier-resolved. Read-only. */
  breachNotice(): Promise<LegalNotice>;
  /** The Pravilnik 40/2019 obrazac for one record — print, sign and file. */
  exportBreachObrazac(id: number): Promise<ExportedFile>;
  listProcessingActivities(): Promise<ProcessingActivity[]>;
  /** Regenerates the register from the app's own configuration (req. 28). */
  generateProcessingActivities(): Promise<ProcessingActivity[]>;
  exportProcessingActivities(): Promise<ExportedFile>;
}

/**
 * The shared retention table — the rok čuvanja per class of record, and the one
 * verb that may change it (SW-10 req. 6, SW-13 req. 22).
 *
 * Two properties of the backend this interface must not paper over.
 *
 * **There is no verb that shortens a rok, and there never may be one.** The
 * period moves only forward — that sentence is printed on the čl. 23 notice
 * handed to the employee and on the čl. 47 register read by the Poverenik — so
 * `extendPolicy` refuses an earlier date rather than clamping it, and the
 * refusal comes back as prose to show the operator.
 *
 * **A `trajno` class is not reachable from here.** Backend-side the classes this
 * verb accepts are an enum with no variant for the ZEOR čl. 5 evidencija, the
 * frozen monthly classification or the čl. 47 register; `RetentionPolicy
 * .adjustable` is that list, so a screen never offers a control it would only
 * get refused for.
 *
 * Both methods are admin-gated backend-side.
 */
export interface RetentionService {
  listPolicies(): Promise<RetentionPolicy[]>;
  /**
   * Moves one class's rok forward. `retainUntil` is `gggg-MM-dd`; the chosen
   * value reaches the čl. 47 register in the same call, because čl. 47 st. 1
   * t. 6 is what the shop has told the Poverenik it applies.
   */
  extendPolicy(recordClass: RecordClass, retainUntil: string): Promise<RetentionPolicy>;
}

/**
 * The published cenovnik (ZZP čl. 6) — where it goes, and what has been
 * published so far.
 *
 * **Nothing here publishes on demand.** Čl. 6 st. 3 wants the file to match the
 * outlet's current prices *„u realnom vremenu“*, so publication rides on the
 * write that moved a price (req. 11) and this surface only reports. A „objavi
 * sada“ button would be a second source of truth about when the shop last
 * published, and the one thing an operator could then do wrong is believe it.
 *
 * Reads are open; `setPublishTarget` is admin-gated backend-side — where the
 * shop's published prices go is what čl. 6 st. 4 then binds it to.
 */
export interface CenovnikService {
  /** The outlet's archive, newest first. Empty before the first publish. */
  listSnapshots(): Promise<CenovnikSnapshot[]>;
  /** One archived cenovnik with its file, or `null` when no such snapshot exists. */
  getSnapshot(snapshotId: number): Promise<CenovnikSnapshotDetail | null>;
  /**
   * The prodajni objekat the archive is keyed on, or `null` while the shop has
   * named neither an address nor a shop name.
   *
   * **`null` is the state in which nothing is made at all.** `publish_current`
   * returns `Ok(None)` and archives nothing on every price write until the shop
   * identifies itself, and a fresh install sits there until Podešavanja →
   * Radnja is saved. Any surface that says a cenovnik is made on every price
   * change has to read this first, or it is promising a file that never appears.
   */
  getOutlet(): Promise<string | null>;
  getPublishTarget(): Promise<CenovnikPublishTarget>;
  setPublishTarget(target: CenovnikPublishTarget): Promise<CenovnikPublishTarget>;
  /**
   * The ZZP čl. 6 duty with its čl. 210 figure already resolved against the
   * **stored** legal form. The frontend never derives a figure and never picks
   * a tier: an unset legal form answers `penalty: null`.
   */
  getNotice(): Promise<LegalNotice>;
}

/**
 * The admin-gated popis (SW-16, reqs. 29–42). Every method maps 1:1 onto a
 * `popis_*` command name.
 *
 * Four properties of the backend this interface must not paper over.
 *
 * **The blind count is enforced at the query layer, not here.** While
 * `PopisSessionView.knjigovodstvoDostupno` is false the response carries no
 * book quantity from any source — not `popis_lines.knjigovodstvena_kolicina_
 * milli`, not `inventory_balances`, not `inventory_movements` — because PoP
 * čl. 8 st. 5 is about book data reaching the commission, whatever table it is
 * read out of (req. 29). A screen that hid a column would not be compliance,
 * and a screen that computed an „očekivano“ figure from the lager would breach
 * the article with the whole suite green.
 *
 * **There is no `update` and no `delete`.** Once the result is knjižen
 * (čl. 14 st. 3) the popis, its liste, its komisija and any new potpis are
 * closed, and a correction is a NEW popis — ZoRač čl. 8 st. 4, req. 41. The
 * retention purge of req. 42 is not a verb on this surface either.
 *
 * **`izvestaj` composes a document; it stores nothing.** Nothing in the schema
 * records an izveštaj, so what comes back exists only for as long as it is on
 * screen or on paper. Its own `upozorenja` say so.
 *
 * **Nothing here opens a popis on the shop's behalf.** `nivelacijaPregled`
 * reports the ZoRač čl. 21 obligations a price change raised; req. 39 puts the
 * čl. 20 st. 3 reconciliation confirmation before a popis exists at all, so an
 * auto-opened session would assert a reconciliation nobody performed.
 */
export interface PopisService {
  list(): Promise<PopisSummary[]>;
  get(id: number): Promise<PopisSessionView>;
  /** Req. 36 — which declared category still has an empty lista. */
  proveraListi(id: number, prijavljene: PopisLista[]): Promise<ProveraListiView>;
  /** Refused without the ZoRač čl. 20 st. 3 confirmation (req. 39). */
  open(request: OpenPopisRequest): Promise<PopisSessionView>;
  /** `lineId` null appends. A book quantity before the čl. 8 st. 5 potpis is refused, never dropped. */
  saveLine(
    sessionId: number,
    lineId: number | null,
    input: PopisLineInput,
  ): Promise<PopisSessionView>;
  startCount(id: number): Promise<PopisSessionView>;
  /**
   * The čl. 8 st. 5 potpis. It freezes the counted state and only **then**
   * releases the book quantities — the statutory order, and the order the v20
   * write guard enforces.
   */
  signPhaseA(id: number, potpisnici: string[]): Promise<PopisSessionView>;
  compute(id: number): Promise<PopisSessionView>;
  /** The čl. 9 st. 3 potpis on the printed, computed liste. */
  signPhaseB(id: number, potpisnici: string[]): Promise<PopisSessionView>;
  /** Čl. 14 st. 3 — knjiženje. The last permitted write on this popis. */
  post(id: number): Promise<PopisSessionView>;
  getPodesavanja(): Promise<PopisPodesavanja>;
  setPodesavanja(podesavanja: PopisPodesavanja): Promise<PopisPodesavanja>;
  nivelacijaPregled(): Promise<NivelacijaPregledView>;
  /** `obuhvat` null takes the narrowed default — a default, never a limit (req. 33). */
  nivelacijaObuhvat(
    id: number,
    obuhvat: NivelacijaObuhvatId | null,
  ): Promise<NivelacijaObuhvatView>;
  izvestaj(id: number, request: IzvestajRequest): Promise<IzvestajView>;
  /**
   * Reqs. 31/32 — writes one of the two popisna-lista documents into `exports/`
   * and returns the descriptor to hand `PrintService.openForPrint`.
   *
   * **`faza` null is the ordinary call.** The backend derives the phase from
   * the session and refuses `"b"` before the čl. 8 st. 5 potpis by name; a
   * caller that picked the phase would be the one deciding when book quantities
   * reach paper, which is the breach the whole module is built around. `"a"`
   * only ever narrows, so the čl. 2 st. 6 reprint of the signed counted state
   * may ask for it in any state.
   */
  exportLista(id: number, faza: PopisPrintFaza | null): Promise<ExportedFile>;
  /** Req. 35 — the odluka o popisu i obrazovanju komisije, as a document. */
  exportOdluka(id: number): Promise<ExportedFile>;
  /** Req. 35 / PoP čl. 8 st. 1 — the plan rada with the approval as stored. */
  exportPlanRada(id: number): Promise<ExportedFile>;
  /**
   * PoP čl. 8 st. 2 — records that the lice iz čl. 4 st. 2 approved the plan
   * rada. A blank name is refused by name rather than substituted, and a posted
   * popis is refused outright (čl. 14 st. 3).
   */
  odobriPlan(id: number, odobrio: string): Promise<PopisSessionView>;
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
  worktime: WorkTimeService;
  privacy: PrivacyService;
  retention: RetentionService;
  cenovnik: CenovnikService;
  popis: PopisService;
  print: PrintService;
}
