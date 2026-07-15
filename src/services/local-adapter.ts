import { invoke as tauriInvoke } from "@tauri-apps/api/core";

import type { PosServices } from "./ports";
import type {
  AppHealth,
  AuthSession,
  BackupJob,
  BackupSettings,
  BackupStatus,
  CashierTurnoverReport,
  CategorySummary,
  CategorySalesReport,
  CompanySettings,
  CompletedSale,
  DailyTurnoverReport,
  ExportedFile,
  ImportHeaders,
  ImportJob,
  ImportJobDetail,
  ImportValidationResult,
  InventoryAdjustmentResult,
  LowStockReport,
  PaymentMethodReport,
  ProductLedger,
  ProductListResult,
  ProductLookupSuggestion,
  ProductSalesReport,
  ProductSummary,
  ReceiptDetail,
  ReceiptSettings,
  SalesSettings,
  ReceiptSearchResult,
  SalePreview,
  ShiftListItem,
  ShiftTurnoverReport,
  ShiftSummary,
  StockListResult,
  TaxRate,
  UserAccount,
} from "./types";

export type InvokeFn = <T>(
  command: string,
  args?: Record<string, unknown>,
) => Promise<T>;

export function createLocalServices(invoke: InvokeFn = tauriInvoke): PosServices {
  return {
    settings: {
      getHealth: () => invoke<AppHealth>("get_app_health"),
      getCompanySettings: () =>
        invoke<CompanySettings>("settings_get_company"),
      updateCompanySettings: (request) =>
        invoke<CompanySettings>("settings_update_company", { request }),
      listTaxRates: () => invoke<TaxRate[]>("settings_list_tax_rates"),
      saveTaxRate: (request) =>
        invoke<TaxRate>("settings_save_tax_rate", { request }),
      seedTaxRates: (inVatSystem) =>
        invoke<TaxRate[]>("settings_seed_tax_rates", { inVatSystem }),
      getReceiptSettings: () =>
        invoke<ReceiptSettings>("settings_get_receipt"),
      updateReceiptSettings: (request) =>
        invoke<ReceiptSettings>("settings_update_receipt", { request }),
      getSalesSettings: () => invoke<SalesSettings>("settings_get_sales"),
      updateSalesSettings: (request) =>
        invoke<SalesSettings>("settings_update_sales", { request }),
    },
    backup: {
      getBackupStatus: () => invoke<BackupStatus>("backup_get_status"),
      updateBackupSettings: (request) =>
        invoke<BackupSettings>("backup_update_settings", { request }),
      createBackup: (request) =>
        invoke<BackupJob>("backup_create", { request }),
      restoreBackup: (request) =>
        invoke<BackupJob>("backup_restore", { request }),
      listBackupJobs: () => invoke<BackupJob[]>("backup_list_jobs"),
    },
    auth: {
      getSession: () => invoke<AuthSession | null>("auth_get_session"),
      login: (request) => invoke<AuthSession>("auth_login", { request }),
      logout: () => invoke<void>("auth_logout"),
    },
    users: {
      listUsers: () => invoke<UserAccount[]>("users_list"),
      createUser: (request) => invoke<UserAccount>("users_create", { request }),
      updateUser: (id, request) =>
        invoke<UserAccount>("users_update", { id, request }),
      deactivateUser: (id) => invoke<void>("users_deactivate", { id }),
    },
    shifts: {
      getCurrentShift: () => invoke<ShiftSummary | null>("shift_get_current"),
      openShift: (request) => invoke<ShiftSummary>("shift_open", { request }),
      closeShift: (request) => invoke<ShiftSummary>("shift_close", { request }),
    },
    catalog: {
      listProducts: (query) =>
        invoke<ProductListResult>("catalog_list_products", { query }),
      searchProducts: (query) =>
        invoke<ProductListResult>("catalog_search_products", { query }),
      getProduct: (id) => invoke<ProductSummary | null>("catalog_get_product", { id }),
      createProduct: (request) =>
        invoke<ProductSummary>("catalog_create_product", { request }),
      updateProduct: (id, request) =>
        invoke<ProductSummary>("catalog_update_product", { id, request }),
      setProductActive: (id, active) =>
        invoke<ProductSummary>("catalog_set_product_active", { id, active }),
      lookupProductByBarcode: (barcode) =>
        invoke<ProductLookupSuggestion | null>("catalog_lookup_product_by_barcode", {
          barcode,
        }),
      listCategories: () => invoke<CategorySummary[]>("catalog_list_categories"),
      saveCategory: (request) =>
        invoke<CategorySummary>("catalog_save_category", { request }),
    },
    sales: {
      createSalePreview: (request) =>
        invoke<SalePreview>("sales_preview", { request }),
      completeSale: (request) =>
        invoke<CompletedSale>("sales_complete", { request }),
    },
    inventory: {
      listStock: (query) => invoke<StockListResult>("inventory_list_stock", { query }),
      receiveStock: (request) =>
        invoke<InventoryAdjustmentResult>("inventory_receive", { request }),
      correctStock: (request) =>
        invoke<InventoryAdjustmentResult>("inventory_correct", { request }),
      writeOffStock: (request) =>
        invoke<InventoryAdjustmentResult>("inventory_write_off", { request }),
      getProductLedger: (productId) =>
        invoke<ProductLedger>("inventory_get_product_ledger", { productId }),
    },
    receipts: {
      searchReceipts: (query) => invoke<ReceiptSearchResult>("receipts_search", { query }),
      getReceipt: (id) => invoke<ReceiptDetail | null>("receipts_get", { id }),
      voidReceipt: (request) => invoke<ReceiptDetail>("receipts_void", { request }),
      returnItems: (request) =>
        invoke<ReceiptDetail>("receipts_return_items", { request }),
    },
    imports: {
      readImportHeaders: (request) =>
        invoke<ImportHeaders>("import_read_headers", { request }),
      validateImport: (request) =>
        invoke<ImportValidationResult>("import_validate", { request }),
      commitImport: (request) => invoke<ImportJob>("import_commit", { request }),
      listImportJobs: () => invoke<ImportJob[]>("import_list_jobs"),
      getImportJob: (id) => invoke<ImportJobDetail | null>("import_get_job", { id }),
    },
    reports: {
      getDailyTurnover: (query) =>
        invoke<DailyTurnoverReport>("reports_daily_turnover", { query }),
      getShiftTurnover: (query) =>
        invoke<ShiftTurnoverReport>("reports_shift_turnover", { query }),
      getCashierTurnover: (query) =>
        invoke<CashierTurnoverReport>("reports_cashier_turnover", { query }),
      getPaymentMethodTurnover: (query) =>
        invoke<PaymentMethodReport>("reports_payment_methods", { query }),
      getProductSales: (query) =>
        invoke<ProductSalesReport>("reports_product_sales", { query }),
      getCategorySales: (query) =>
        invoke<CategorySalesReport>("reports_category_sales", { query }),
      getLowStock: () => invoke<LowStockReport>("reports_low_stock"),
      listShifts: () => invoke<ShiftListItem[]>("reports_list_shifts"),
      exportReportCsv: (request) =>
        invoke<ExportedFile>("reports_export_csv", { request }),
    },
  };
}
