import type {
  AppHealth,
  AuthSession,
  BackupJob,
  BackupSettings,
  BackupStatus,
  CampaignInput,
  CampaignSummary,
  CampaignValidationReport,
  CampaignView,
  CashierTurnoverReport,
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
  EndCampaignOverride,
  ExportedFile,
  ExportReportRequest,
  ImportHeaders,
  ImportJob,
  ImportJobDetail,
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
}

export interface SalesService {
  createSalePreview(request: SaleDraftRequest): Promise<SalePreview>;
  completeSale(request: CompleteSaleRequest): Promise<CompletedSale>;
}

export interface InventoryService {
  listStock(query: StockListQuery): Promise<StockListResult>;
  receiveStock(request: InventoryAdjustmentRequest): Promise<InventoryAdjustmentResult>;
  correctStock(request: InventoryAdjustmentRequest): Promise<InventoryAdjustmentResult>;
  writeOffStock(request: InventoryAdjustmentRequest): Promise<InventoryAdjustmentResult>;
  getProductLedger(productId: number): Promise<ProductLedger>;
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
}
