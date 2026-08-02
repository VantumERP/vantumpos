import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { openPath, openUrl } from "@tauri-apps/plugin-opener";

import type { PosServices } from "./ports";
import type {
  AmlAssessment,
  AppHealth,
  AuthSession,
  BackupJob,
  BackupSettings,
  BackupStatus,
  CampaignSummary,
  CampaignValidationReport,
  CampaignView,
  CashDepositCalendar,
  CashDepositReport,
  CashierTurnoverReport,
  CategorySummary,
  CategorySalesReport,
  CompanySettings,
  CorrectionReport,
  CompletedSale,
  DailyTurnoverReport,
  DeclarationGapRow,
  ExportedFile,
  ImportHeaders,
  ImportJob,
  ImportJobDetail,
  ImportValidationResult,
  InventoryAdjustmentResult,
  KalkulacijaSummary,
  KepClosePreview,
  KepClosure,
  KepClosureView,
  KepEntryView,
  KepLedger,
  KepStatus,
  LowStockReport,
  PaymentMethodReport,
  PrethodnaCenaDto,
  ProductLedger,
  ProductListResult,
  ProductLookupSuggestion,
  ProductSalesReport,
  ProductSummary,
  ReceiptDetail,
  ReceiptSettings,
  ReklamacijaSummary,
  ReklamacijaView,
  SalesSettings,
  ReceiptSearchResult,
  SalePreview,
  ShiftListItem,
  ShiftTurnoverReport,
  ShiftSummary,
  ShopProfile,
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
      getShopProfile: () => invoke<ShopProfile>("settings_get_shop_profile"),
      updateShopProfile: (request) =>
        invoke<ShopProfile>("settings_update_shop_profile", { request }),
      getCashDepositCalendar: () =>
        invoke<CashDepositCalendar>("cash_deposit_calendar"),
      setSaturdayIsWorking: (counts) =>
        invoke<CashDepositCalendar>("cash_deposit_set_saturday_is_working", {
          counts,
        }),
      saveNonWorkingDay: (day, label) =>
        invoke<CashDepositCalendar>("cash_deposit_save_non_working_day", {
          day,
          label,
        }),
      deleteNonWorkingDay: (day) =>
        invoke<CashDepositCalendar>("cash_deposit_delete_non_working_day", {
          day,
        }),
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
      setBackupPassphrase: (passphrase) =>
        invoke<void>("backup_set_passphrase", { passphrase }),
      resetTradingData: (confirmationText) =>
        invoke<void>("backup_reset_trading_data", { confirmationText }),
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
      adminCloseShift: (request) =>
        invoke<ShiftSummary>("shift_admin_close", { request }),
      shiftCashMovement: (request) =>
        invoke<ShiftSummary>("shift_cash_movement", { request }),
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
      getPrethodnaCena: (productId, campaignStart) =>
        invoke<PrethodnaCenaDto>("catalog_prethodna_cena", { productId, campaignStart }),
      declarationGaps: () =>
        invoke<DeclarationGapRow[]>("catalog_declaration_gaps"),
    },
    sales: {
      createSalePreview: (request) =>
        invoke<SalePreview>("sales_preview", { request }),
      completeSale: (request) =>
        invoke<CompletedSale>("sales_complete", { request }),
      assessCashPayment: (cashMinor) =>
        invoke<AmlAssessment>("sales_assess_cash_payment", { cashMinor }),
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
      markDeclarationChecked: (productId) =>
        invoke<void>("inventory_mark_declaration_checked", { productId }),
    },
    receipts: {
      searchReceipts: (query) => invoke<ReceiptSearchResult>("receipts_search", { query }),
      getReceipt: (id) => invoke<ReceiptDetail | null>("receipts_get", { id }),
      voidReceipt: (request) => invoke<ReceiptDetail>("receipts_void", { request }),
      returnItems: (request) =>
        invoke<ReceiptDetail>("receipts_return_items", { request }),
      setEsirNumber: (receiptId, esirReceiptNumber) =>
        invoke<ReceiptDetail>("receipts_set_esir_number", { receiptId, esirReceiptNumber }),
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
      getCashDepositReport: (asOf) =>
        invoke<CashDepositReport>("cash_deposit_report", { asOf }),
      exportCashDepositCsv: (asOf) =>
        invoke<ExportedFile>("cash_deposit_export_csv", { asOf }),
    },
    campaigns: {
      listCampaigns: () => invoke<CampaignSummary[]>("campaigns_list"),
      getCampaign: (id) => invoke<CampaignView>("campaigns_get", { id }),
      validateCampaign: (input) =>
        invoke<CampaignValidationReport>("campaigns_validate", { input }),
      createCampaign: (input) =>
        invoke<CampaignView>("campaigns_create", { input }),
      updateCampaign: (id, input) =>
        invoke<CampaignView>("campaigns_update", { id, input }),
      activateCampaign: (id) =>
        invoke<CampaignView>("campaigns_activate", { id }),
      adjustItemPrice: (campaignId, productId, newPriceMinor) =>
        invoke<CampaignView>("campaigns_adjust_item_price", {
          campaignId,
          productId,
          newPriceMinor,
        }),
      endCampaign: (id, overrides) =>
        invoke<CampaignView>("campaigns_end", { id, overrides }),
      cancelCampaign: (id) => invoke<CampaignView>("campaigns_cancel", { id }),
      exportEvidence: (campaignId) =>
        invoke<ExportedFile>("campaigns_export_evidence", { campaignId }),
      exportLabels: (campaignId) =>
        invoke<ExportedFile>("campaigns_export_labels", { campaignId }),
      correctionReport: () =>
        invoke<CorrectionReport>("campaigns_correction_report"),
      exportCorrectionReport: () =>
        invoke<ExportedFile>("campaigns_export_correction_report"),
    },
    reklamacije: {
      list: () => invoke<ReklamacijaSummary[]>("reklamacija_list"),
      get: (id) => invoke<ReklamacijaView>("reklamacija_get", { id }),
      create: (input) =>
        invoke<ReklamacijaView>("reklamacija_create", { input }),
      logAnswer: (id, input) =>
        invoke<ReklamacijaView>("reklamacija_log_answer", { id, input }),
      consumerReceived: (id, eventDate) =>
        invoke<ReklamacijaView>("reklamacija_consumer_received", {
          id,
          eventDate,
        }),
      consumerResponded: (id, eventDate) =>
        invoke<ReklamacijaView>("reklamacija_consumer_responded", {
          id,
          eventDate,
        }),
      grantExtension: (id, newDeadline, consumerConsent, reason, eventDate) =>
        invoke<ReklamacijaView>("reklamacija_grant_extension", {
          id,
          newDeadline,
          consumerConsent,
          reason,
          eventDate,
        }),
      resolve: (id, nacin, eventDate, noFeeAttested) =>
        invoke<ReklamacijaView>("reklamacija_resolve", {
          id,
          nacin,
          eventDate,
          noFeeAttested,
        }),
      exportPotvrda: (id) =>
        invoke<ExportedFile>("reklamacija_export_potvrda", { id }),
      exportNotice: () =>
        invoke<ExportedFile>("reklamacija_export_notice"),
    },
    kep: {
      ledger: (bookYear) => invoke<KepLedger>("kep_ledger", { bookYear }),
      postDailySales: (date, overrideAmountMinor) =>
        invoke<KepEntryView>("kep_post_daily_sales", {
          date,
          overrideAmountMinor,
        }),
      status: () => invoke<KepStatus>("kep_status"),
      listKalkulacije: (bookYear) =>
        invoke<KalkulacijaSummary[]>("kep_list_kalkulacije", { bookYear }),
      exportKalkulacija: (id) =>
        invoke<ExportedFile>("kep_export_kalkulacija", { id }),
      nivelacija: (productId, newSalePriceMinor, basis) =>
        invoke<void>("kep_nivelacija", { productId, newSalePriceMinor, basis }),
      postAdjustment: (cause, productId, quantityMilli, basis) =>
        invoke<void>("kep_post_adjustment", {
          cause,
          productId,
          quantityMilli,
          basis,
        }),
      correctEntry: (targetRedniBroj, bookYear, correctAmountMinor, basis) =>
        invoke<void>("kep_correct_entry", {
          targetRedniBroj,
          bookYear,
          correctAmountMinor,
          basis,
        }),
      closePreview: (bookYear) =>
        invoke<KepClosePreview>("kep_close_preview", { bookYear }),
      closeYear: (bookYear, confirmation) =>
        invoke<KepClosure>("kep_close_year", { bookYear, confirmation }),
      listClosures: () => invoke<KepClosureView[]>("kep_list_closures"),
      exportClose: (bookYear) =>
        invoke<ExportedFile>("kep_export_close", { bookYear }),
      exportBook: (bookYear) =>
        invoke<ExportedFile>("kep_export_book", { bookYear }),
    },
    print: {
      openForPrint: (path) => openPath(path),
      openExternalUrl: (url) => openUrl(url),
    },
  };
}
