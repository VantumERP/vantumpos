import { describe, expect, it, vi } from "vitest";

import { createLocalServices } from "./local-adapter";
import { createMockServices } from "./mock-adapter";
import type {
  AnswerInput,
  BasisDoc,
  BreachDraft,
  CampaignInput,
  ReklamacijaInput,
  ShopProfile,
} from "./types";

/**
 * The three ZZPL čl. 52 st. 6 elements plus the immutable saznanje anchor, and
 * every optional column left unanswered — the shape `breaches_record` is called
 * with when a povreda is logged before anyone has assessed its risk (req. 43).
 */
const draft: BreachDraft = {
  saznanjeAt: "2026-08-01T09:00:00Z",
  occurredAt: null,
  discoveredAt: null,
  obradjivacSaznanjeAt: null,
  rukovalacObavestenAt: null,
  opis: "Nestao je papirni spisak zaposlenih iz kancelarije.",
  posledice: "Podaci o troje zaposlenih su mogli da budu pročitani.",
  mere: "Brava je zamenjena, spisak se više ne štampa.",
  brojLica: 3,
  kategorijePodataka: null,
  riskOutcome: null,
  notifyDecision: null,
  notifyObrazlozenje: null,
  poverenikNotifiedAt: null,
  delayReason: null,
  licaObavestena: null,
  licaObavestenaAt: null,
  cl53Izuzetak: null,
  cl53IzuzetakObrazlozenje: null,
};

vi.mock("@tauri-apps/plugin-opener", () => ({
  openPath: vi.fn().mockResolvedValue(undefined),
  openUrl: vi.fn().mockResolvedValue(undefined),
}));

describe("local service adapter", () => {
  it("calls the Tauri health command through the injected invoker", async () => {
    const invoke = vi.fn().mockResolvedValue({
      backend: "local",
      appVersion: "0.1.0",
      databasePath: "C:/Users/test/AppData/Roaming/vantumpos/vantumpos.sqlite3",
      migrated: true,
    });

    const services = createLocalServices(invoke);
    const health = await services.settings.getHealth();

    expect(invoke).toHaveBeenCalledWith("get_app_health");
    expect(health.backend).toBe("local");
    expect(health.migrated).toBe(true);
  });

  it("maps auth, users, and shift services to stable Tauri command names", async () => {
    const invoke = vi.fn().mockImplementation((command: string) => {
      switch (command) {
        case "auth_get_session":
          return Promise.resolve(null);
        case "auth_login":
          return Promise.resolve({
            user: {
              id: 1,
              username: "admin",
              displayName: "Administrator",
              role: "admin",
              active: true,
              createdAt: "2026-06-18T08:00:00Z",
              updatedAt: "2026-06-18T08:00:00Z",
              lastLoginAt: null,
            },
            currentShift: null,
          });
        case "users_list":
          return Promise.resolve([]);
        case "shift_open":
          return Promise.resolve({
            id: 1,
            userId: 1,
            cashierName: "Administrator",
            openedAt: "2026-06-18T09:00:00Z",
            closedAt: null,
            openingCashMinor: 10000,
            expectedCashMinor: 10000,
            countedCashMinor: null,
            cashSalesMinor: 0,
            cardSalesMinor: 0,
            paidInMinor: 0,
            paidOutMinor: 0,
            differenceMinor: null,
            status: "open",
            openingNote: null,
            closingNote: null,
          });
        case "shift_admin_close":
          return Promise.resolve({
            id: 2,
            userId: 2,
            cashierName: "Marko Markovic",
            openedAt: "2026-06-18T07:00:00Z",
            closedAt: "2026-06-18T09:00:00Z",
            openingCashMinor: 5000,
            expectedCashMinor: 5000,
            countedCashMinor: 5000,
            cashSalesMinor: 0,
            cardSalesMinor: 0,
            paidInMinor: 0,
            paidOutMinor: 0,
            differenceMinor: 0,
            status: "closed",
            openingNote: null,
            closingNote: null,
          });
        default:
          return Promise.resolve(undefined);
      }
    });

    const services = createLocalServices(invoke);

    await services.auth.getSession();
    await services.auth.login({ username: "admin", credential: "1234" });
    await services.users.listUsers();
    await services.shifts.openShift({
      openingCashMinor: 10000,
      note: null,
    });
    await services.shifts.adminCloseShift({
      shiftId: 2,
      countedCashMinor: 5000,
      note: null,
    });

    expect(invoke).toHaveBeenCalledWith("auth_get_session");
    expect(invoke).toHaveBeenCalledWith("auth_login", {
      request: { username: "admin", credential: "1234" },
    });
    expect(invoke).toHaveBeenCalledWith("users_list");
    expect(invoke).toHaveBeenCalledWith("shift_open", {
      request: { openingCashMinor: 10000, note: null },
    });
    expect(invoke).toHaveBeenCalledWith("shift_admin_close", {
      request: { shiftId: 2, countedCashMinor: 5000, note: null },
    });
  });

  it("maps shiftCashMovement to shift_cash_movement", async () => {
    const invoke = vi.fn().mockResolvedValue({
      id: 1,
      userId: 1,
      cashierName: "Administrator",
      openedAt: "2026-06-18T09:00:00Z",
      closedAt: null,
      openingCashMinor: 10000,
      expectedCashMinor: 5000,
      countedCashMinor: null,
      cashSalesMinor: 0,
      cardSalesMinor: 0,
      paidInMinor: 0,
      paidOutMinor: 5000,
      differenceMinor: null,
      status: "open",
      openingNote: null,
      closingNote: null,
    });
    const services = createLocalServices(invoke);

    await services.shifts.shiftCashMovement({
      direction: "pay_out",
      amountMinor: 5000,
      reason: "Pazar u banku",
    });

    expect(invoke).toHaveBeenCalledWith("shift_cash_movement", {
      request: {
        direction: "pay_out",
        amountMinor: 5000,
        reason: "Pazar u banku",
      },
    });
  });

  it("maps settings and backup services to stable Tauri command names", async () => {
    const invoke = vi.fn().mockResolvedValue({});
    const services = createLocalServices(invoke);

    await services.settings.getCompanySettings();
    await services.settings.updateCompanySettings({
      shopName: "Vantum Market",
      address: "Bulevar 1",
      pib: "123456789",
      registrationNumber: "87654321",
      phone: "+381 11 123 456",
      logoPath: null,
      currency: "RSD",
    });
    await services.settings.listTaxRates();
    await services.settings.saveTaxRate({
      id: null,
      name: "PDV 20",
      rateBasisPoints: 2000,
      active: true,
    });
    await services.settings.getReceiptSettings();
    await services.settings.updateReceiptSettings({
      prefix: "VP-",
      nextSequenceNumber: 42,
    });
    await services.backup.getBackupStatus();
    await services.backup.updateBackupSettings({
      backupFolder: "D:/VantumPOS/backups",
      automaticBackupEnabled: true,
    });
    await services.backup.createBackup({
      backupFolder: "D:/VantumPOS/backups",
    });
    await services.backup.restoreBackup({
      path: "D:/VantumPOS/backups/vantumpos.sqlite3",
      confirmationText: "VRATI PODATKE",
    });
    await services.backup.listBackupJobs();

    expect(invoke).toHaveBeenNthCalledWith(1, "settings_get_company");
    expect(invoke).toHaveBeenNthCalledWith(2, "settings_update_company", {
      request: expect.objectContaining({ shopName: "Vantum Market" }),
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "settings_list_tax_rates");
    expect(invoke).toHaveBeenNthCalledWith(4, "settings_save_tax_rate", {
      request: expect.objectContaining({ rateBasisPoints: 2000 }),
    });
    expect(invoke).toHaveBeenNthCalledWith(5, "settings_get_receipt");
    expect(invoke).toHaveBeenNthCalledWith(6, "settings_update_receipt", {
      request: { prefix: "VP-", nextSequenceNumber: 42 },
    });
    expect(invoke).toHaveBeenNthCalledWith(7, "backup_get_status");
    expect(invoke).toHaveBeenNthCalledWith(8, "backup_update_settings", {
      request: {
        backupFolder: "D:/VantumPOS/backups",
        automaticBackupEnabled: true,
      },
    });
    expect(invoke).toHaveBeenNthCalledWith(9, "backup_create", {
      request: { backupFolder: "D:/VantumPOS/backups" },
    });
    expect(invoke).toHaveBeenNthCalledWith(10, "backup_restore", {
      request: {
        path: "D:/VantumPOS/backups/vantumpos.sqlite3",
        confirmationText: "VRATI PODATKE",
      },
    });
    expect(invoke).toHaveBeenNthCalledWith(11, "backup_list_jobs");
  });

  it("maps resetTradingData to backup_reset_trading_data", async () => {
    const invoke = vi.fn().mockResolvedValue(undefined);
    const services = createLocalServices(invoke);

    await services.backup.resetTradingData("OBRISI PODATKE");

    expect(invoke).toHaveBeenCalledWith("backup_reset_trading_data", {
      confirmationText: "OBRISI PODATKE",
    });
  });

  it("maps sales settings to stable Tauri command names", async () => {
    const invoke = vi.fn().mockResolvedValue({ allowOverselling: false });
    const services = createLocalServices(invoke);

    await services.settings.getSalesSettings();
    await services.settings.updateSalesSettings({ allowOverselling: true });

    expect(invoke).toHaveBeenCalledWith("settings_get_sales");
    expect(invoke).toHaveBeenCalledWith("settings_update_sales", {
      request: { allowOverselling: true },
    });
  });

  it("maps the shop profile to stable Tauri command names", async () => {
    const profile: ShopProfile = {
      pravnaForma: null,
      pdvObveznik: null,
      distanceSelling: null,
      lpfrInPremises: null,
      lpfrCarveOutInternetOnly: null,
      lpfrCarveOutOwnUsedAssets: null,
      esirElements: [],
    };
    const invoke = vi.fn().mockResolvedValue(profile);
    const services = createLocalServices(invoke);

    await services.settings.getShopProfile();
    await services.settings.updateShopProfile({
      ...profile,
      pravnaForma: "preduzetnik",
      distanceSelling: true,
    });

    expect(invoke).toHaveBeenCalledWith("settings_get_shop_profile");
    expect(invoke).toHaveBeenCalledWith("settings_update_shop_profile", {
      request: expect.objectContaining({
        pravnaForma: "preduzetnik",
        distanceSelling: true,
      }),
    });
  });

  // The ZF čl. 6 st. 4 figure is resolved in `legal.rs` and travels to the
  // panel over this command — the frontend must never derive one.
  it("maps the L-PFR notice to settings_lpfr_notice", async () => {
    const invoke = vi.fn().mockResolvedValue({
      summary: "…",
      penalty: null,
      citation: "Zakon o fiskalizaciji, čl. 6 st. 4; prekršaj: čl. 15 st. 1 tač. 4.",
      isLegalDuty: true,
    });
    const services = createLocalServices(invoke);

    await services.settings.getLpfrNotice();

    expect(invoke).toHaveBeenCalledWith("settings_lpfr_notice");
  });

  it("maps the EUR rate surface to stable Tauri command names", async () => {
    const status = {
      rate: { rateMinor: 11_723, rateDate: "2026-07-31", source: "nbs" },
      isStale: false,
      checkedFor: "2026-07-31",
    };
    const invoke = vi.fn().mockResolvedValue(status);
    const services = createLocalServices(invoke);

    await services.settings.getEurRate();
    await services.settings.refreshEurRate();
    await services.settings.setManualEurRate(11_723, "2026-07-31");

    expect(invoke).toHaveBeenCalledWith("settings_get_eur_rate");
    expect(invoke).toHaveBeenCalledWith("settings_refresh_eur_rate");
    // Flat args, not `{ request }` — `settings_set_manual_eur_rate` takes
    // `rate_minor` and `rate_date` as two parameters.
    expect(invoke).toHaveBeenCalledWith("settings_set_manual_eur_rate", {
      rateMinor: 11_723,
      rateDate: "2026-07-31",
    });
  });

  it("round-trips the mock EUR rate instead of freezing one constant", async () => {
    const services = createMockServices();

    // A fresh install has no rate at all: the AML check cannot run, and the
    // double has to be able to express that, not only the happy path.
    await services.settings.setManualEurRate(20_000, "2026-06-01");
    const stale = await services.settings.getEurRate();

    expect(stale.rate).toEqual({
      rateMinor: 20_000,
      rateDate: "2026-06-01",
      source: "manual",
    });
    expect(
      stale.isStale,
      "a rate carrying a past date is stale even right after it was entered",
    ).toBe(true);

    // The threshold the till warns on must follow the stored rate, not the
    // seeded demo one: 10.000 EUR x 200,00 RSD/EUR.
    const assessment = await services.sales.assessCashPayment(200_000_000);
    expect(assessment.thresholdMinor).toBe(200_000_000);
    expect(assessment.rateUnavailable).toBe(false);
    expect(assessment.breached).toBe(true);

    const fresh = await services.settings.setManualEurRate(11_723, "2026-06-18");
    expect(fresh.isStale).toBe(false);
    expect(fresh.checkedFor).toBe("2026-06-18");
  });

  it("refuses a mock manual rate outside the typo-guard band", async () => {
    const services = createMockServices();

    // 1.172,30 RSD/EUR — a tenfold typo. Accepting it would multiply the AML
    // čl. 46 st. 1 dinar threshold by ten and let an unlawful cash amount pass.
    await expect(
      services.settings.setManualEurRate(117_230, "2026-06-18"),
    ).rejects.toMatchObject({ code: "validation_error" });
    await expect(
      services.settings.setManualEurRate(11_723, "18.06.2026"),
    ).rejects.toMatchObject({ code: "validation_error" });
  });

  it("seeds the mock shop profile as fully unset", async () => {
    const services = createMockServices();

    const profile = await services.settings.getShopProfile();

    expect(profile.pravnaForma).toBeNull();
    expect(profile.distanceSelling).toBeNull();
    expect(profile.esirElements).toEqual([]);
  });

  it("maps seedTaxRates to settings_seed_tax_rates", async () => {
    const invoke = vi.fn().mockResolvedValue([]);
    const services = createLocalServices(invoke);

    await services.settings.seedTaxRates(true);

    expect(invoke).toHaveBeenCalledWith("settings_seed_tax_rates", {
      inVatSystem: true,
    });
  });

  it("maps catalog service methods to stable Tauri command names", async () => {
    const invoke = vi.fn().mockResolvedValue({});
    const services = createLocalServices(invoke);

    await services.catalog.listProducts({ search: "mleko", active: true });
    await services.catalog.searchProducts({ search: "mleko" });
    await services.catalog.getProduct(7);
    await services.catalog.createProduct({
      name: "Mleko 1 l",
      sku: "MLEKO-1L",
      barcode: "8600000000010",
      categoryId: 1,
      unitOfMeasure: "kom",
      salePriceMinor: 15999,
      purchasePriceMinor: 12000,
      taxRateId: 1,
      minimumStockMilli: 5000,
      allowNegativeStock: false,
      active: true,
      perishable: false,
      perishableJustification: null,
    });
    await services.catalog.updateProduct(7, {
      name: "Mleko 1 l",
      sku: "MLEKO-1L",
      barcode: "8600000000010",
      categoryId: 1,
      unitOfMeasure: "kom",
      salePriceMinor: 16999,
      purchasePriceMinor: 12000,
      taxRateId: 1,
      minimumStockMilli: 5000,
      allowNegativeStock: false,
      active: true,
      perishable: false,
      perishableJustification: null,
    });
    await services.catalog.setProductActive(7, false);
    await services.catalog.listCategories();
    await services.catalog.saveCategory({ id: 1, name: "Mlecni proizvodi" });

    expect(invoke).toHaveBeenNthCalledWith(1, "catalog_list_products", {
      query: { search: "mleko", active: true },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "catalog_search_products", {
      query: { search: "mleko" },
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "catalog_get_product", { id: 7 });
    expect(invoke).toHaveBeenNthCalledWith(4, "catalog_create_product", {
      request: expect.objectContaining({ sku: "MLEKO-1L" }),
    });
    expect(invoke).toHaveBeenNthCalledWith(5, "catalog_update_product", {
      id: 7,
      request: expect.objectContaining({ salePriceMinor: 16999 }),
    });
    expect(invoke).toHaveBeenNthCalledWith(6, "catalog_set_product_active", {
      id: 7,
      active: false,
    });
    expect(invoke).toHaveBeenNthCalledWith(7, "catalog_list_categories");
    expect(invoke).toHaveBeenNthCalledWith(8, "catalog_save_category", {
      request: { id: 1, name: "Mlecni proizvodi" },
    });
  });

  it("maps catalog barcode lookup to the stable Tauri command name", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    const services = createLocalServices(invoke);

    await services.catalog.lookupProductByBarcode("4008400320328");

    expect(invoke).toHaveBeenCalledWith("catalog_lookup_product_by_barcode", {
      barcode: "4008400320328",
    });
  });

  it("maps sales preview and completion to stable Tauri command names", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce({
        items: [],
        subtotalMinor: 12000,
        discountMinor: 0,
        taxMinor: 2000,
        totalMinor: 12000,
      })
      .mockResolvedValueOnce({
        id: 1,
        localReceiptNumber: "VP-000001",
        createdAt: "2026-06-18T10:00:00Z",
        cashierName: "Kasir",
        fiscalStatus: "not_fiscalized",
        items: [],
        payments: [{ method: "cash", amountMinor: 12000 }],
        subtotalMinor: 12000,
        discountMinor: 0,
        taxMinor: 2000,
        totalMinor: 12000,
        cashReceivedMinor: 12000,
        changeDueMinor: 0,
      });
    const services = createLocalServices(invoke);
    const draft = {
      items: [{ productId: 1, quantityMilli: 1000 }],
      receiptDiscount: { type: "amount" as const, amountMinor: 0 },
    };
    const completion = {
      ...draft,
      payments: [{ method: "cash" as const, amountMinor: 12000 }],
    };

    await services.sales.createSalePreview(draft);
    await services.sales.completeSale(completion);

    expect(invoke).toHaveBeenNthCalledWith(1, "sales_preview", {
      request: draft,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "sales_complete", {
      request: completion,
    });
  });

  it("maps inventory service methods to stable Tauri command names", async () => {
    const invoke = vi.fn().mockResolvedValueOnce({ items: [] });
    const services = createLocalServices(invoke);

    await services.inventory.listStock({ search: "mleko", stockState: "low" });

    expect(invoke).toHaveBeenCalledWith("inventory_list_stock", {
      query: { search: "mleko", stockState: "low" },
    });
  });

  it("maps inventory mutations to request-scoped Tauri commands", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce({
        productId: 1,
        movementId: 10,
        movementType: "receive",
        quantityMilli: 2000,
        previousQuantityMilli: 3000,
        newQuantityMilli: 5000,
        createdAt: "2026-06-18T12:00:00Z",
      })
      .mockResolvedValueOnce({
        productId: 1,
        movements: [],
      });
    const services = createLocalServices(invoke);

    await services.inventory.receiveStock({
      productId: 1,
      quantityMilli: 2000,
      reason: "Prijem robe",
    });
    await services.inventory.getProductLedger(1);

    expect(invoke).toHaveBeenNthCalledWith(1, "inventory_receive", {
      request: {
        productId: 1,
        quantityMilli: 2000,
        reason: "Prijem robe",
      },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "inventory_get_product_ledger", {
      productId: 1,
    });
  });

  // ZoT čl. 34: the gaps report is read-only and the check stamp is the čl. 69a
  // mitigation record. Both are admin-gated in Rust, so the frontend must reach
  // the real command names rather than reimplement the rule.
  it("maps the deklaracija surfaces to stable Tauri command names", async () => {
    const invoke = vi.fn().mockResolvedValue([]);
    const services = createLocalServices(invoke);

    await services.catalog.declarationGaps();
    await services.inventory.markDeclarationChecked(7);

    expect(invoke).toHaveBeenNthCalledWith(1, "catalog_declaration_gaps");
    expect(invoke).toHaveBeenNthCalledWith(
      2,
      "inventory_mark_declaration_checked",
      { productId: 7 },
    );
  });

  it("routes receipt use cases through stable Tauri command names", async () => {
    const invoke = vi.fn().mockImplementation((command: string) => {
      if (command === "receipts_search") {
        return Promise.resolve({ receipts: [], total: 0 });
      }

      if (command === "receipts_get") {
        return Promise.resolve(null);
      }

      if (command === "receipts_void") {
        return Promise.resolve(null);
      }

      if (command === "receipts_return_items") {
        return Promise.resolve(null);
      }

      throw new Error(`unexpected command ${command}`);
    });

    const services = createLocalServices(invoke);
    await services.receipts.searchReceipts({ receiptNumber: "R-2026" });
    await services.receipts.getReceipt(7);
    await services.receipts.voidReceipt({
      receiptId: 7,
      reason: "Greska u unosu",
    });
    await services.receipts.returnItems({
      receiptId: 7,
      reason: "Kupac vratio artikal",
      items: [{ saleItemId: 3, quantityMilli: 1000 }],
    });

    expect(invoke).toHaveBeenNthCalledWith(1, "receipts_search", {
      query: { receiptNumber: "R-2026" },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "receipts_get", { id: 7 });
    expect(invoke).toHaveBeenNthCalledWith(3, "receipts_void", {
      request: { receiptId: 7, reason: "Greska u unosu" },
    });
    expect(invoke).toHaveBeenNthCalledWith(4, "receipts_return_items", {
      request: {
        receiptId: 7,
        reason: "Kupac vratio artikal",
        items: [{ saleItemId: 3, quantityMilli: 1000 }],
      },
    });
  });

  it("forwards refundTender through receipts_return_items", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    const services = createLocalServices(invoke);

    await services.receipts.returnItems({
      receiptId: 7,
      reason: "Zamena velicine",
      items: [{ saleItemId: 3, quantityMilli: 1000 }],
      refundTender: "card",
    });

    expect(invoke).toHaveBeenCalledWith("receipts_return_items", {
      request: {
        receiptId: 7,
        reason: "Zamena velicine",
        items: [{ saleItemId: 3, quantityMilli: 1000 }],
        refundTender: "card",
      },
    });
  });

  it("maps import service methods to stable Tauri command names", async () => {
    const invoke = vi
      .fn()
      .mockResolvedValueOnce({
        importType: "products",
        fileName: "artikli.csv",
        delimiter: ";",
        headers: ["Naziv", "Cena", "PDV"],
        totalRows: 1,
      })
      .mockResolvedValueOnce({
        importType: "products",
        fileName: "artikli.csv",
        totalRows: 1,
        validCount: 1,
        warningCount: 0,
        errorCount: 0,
        summary: { create: 1, update: 0, skip: 0 },
        rows: [],
      })
      .mockResolvedValueOnce({
        id: 1,
        importType: "products",
        fileName: "artikli.csv",
        status: "completed",
        totalRows: 1,
        errorRows: 0,
        createdAt: "2026-06-18T10:00:00Z",
        completedAt: "2026-06-18T10:00:01Z",
      })
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce({
        id: 1,
        importType: "products",
        fileName: "artikli.csv",
        status: "completed",
        totalRows: 1,
        errorRows: 0,
        createdAt: "2026-06-18T10:00:00Z",
        completedAt: "2026-06-18T10:00:01Z",
        rows: [],
      });

    const services = createLocalServices(invoke);
    const headerRequest = {
      importType: "products" as const,
      fileName: "artikli.csv",
      csvText: "Naziv;Cena;PDV\nHleb;120,00;20\n",
    };
    const mapping = {
      name: "Naziv",
      sale_price: "Cena",
      vat_rate: "PDV",
      sku: "Sifra",
    };

    await services.imports.readImportHeaders(headerRequest);
    await services.imports.validateImport({ ...headerRequest, mapping });
    await services.imports.commitImport({ ...headerRequest, mapping });
    await services.imports.listImportJobs();
    await services.imports.getImportJob(1);

    expect(invoke).toHaveBeenNthCalledWith(1, "import_read_headers", {
      request: headerRequest,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "import_validate", {
      request: { ...headerRequest, mapping },
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "import_commit", {
      request: { ...headerRequest, mapping },
    });
    expect(invoke).toHaveBeenNthCalledWith(4, "import_list_jobs");
    expect(invoke).toHaveBeenNthCalledWith(5, "import_get_job", { id: 1 });
  });

  it("maps campaign service methods to stable Tauri command names", async () => {
    const invoke = vi.fn().mockImplementation((command: string) => {
      switch (command) {
        case "campaigns_list":
          return Promise.resolve([]);
        case "campaigns_validate":
          return Promise.resolve({ hard: [], warnings: [], anchors: [] });
        default:
          return Promise.resolve(null);
      }
    });
    const services = createLocalServices(invoke);

    const input: CampaignInput = {
      campaignType: "sezonsko_snizenje",
      startsOn: "2026-07-01T00:00:00Z",
      endsOn: "2026-07-31T00:00:00Z",
      displayMode: "two_prices",
      headlinePercent: null,
      rasprodajaGround: null,
      specialConditions: null,
      reducedUtilityReason: null,
      marketingLabel: "Letnje sniženje",
      seasonAttested: true,
      separationAttested: false,
      items: [
        {
          productId: 1,
          campaignPriceMinor: 1290000,
          manualPrethodnaMinor: null,
          anchorJustification: null,
          futureRegularPriceMinor: null,
        },
      ],
    };

    await services.campaigns.listCampaigns();
    await services.campaigns.getCampaign(7);
    await services.campaigns.validateCampaign(input);
    await services.campaigns.createCampaign(input);
    await services.campaigns.updateCampaign(7, input);
    await services.campaigns.activateCampaign(7);
    await services.campaigns.adjustItemPrice(7, 1, 990000);
    await services.campaigns.endCampaign(7, [
      { productId: 1, returnPriceMinor: 1590000 },
    ]);
    await services.campaigns.cancelCampaign(7);

    expect(invoke).toHaveBeenNthCalledWith(1, "campaigns_list");
    expect(invoke).toHaveBeenNthCalledWith(2, "campaigns_get", { id: 7 });
    expect(invoke).toHaveBeenNthCalledWith(3, "campaigns_validate", { input });
    expect(invoke).toHaveBeenNthCalledWith(4, "campaigns_create", { input });
    expect(invoke).toHaveBeenNthCalledWith(5, "campaigns_update", {
      id: 7,
      input,
    });
    expect(invoke).toHaveBeenNthCalledWith(6, "campaigns_activate", { id: 7 });
    expect(invoke).toHaveBeenNthCalledWith(7, "campaigns_adjust_item_price", {
      campaignId: 7,
      productId: 1,
      newPriceMinor: 990000,
    });
    expect(invoke).toHaveBeenNthCalledWith(8, "campaigns_end", {
      id: 7,
      overrides: [{ productId: 1, returnPriceMinor: 1590000 }],
    });
    expect(invoke).toHaveBeenNthCalledWith(9, "campaigns_cancel", { id: 7 });
  });

  it("maps campaign evidence and correction methods to stable Tauri command names", async () => {
    const invoke = vi.fn().mockImplementation((command: string) => {
      if (command === "campaigns_correction_report") {
        return Promise.resolve({ rows: [] });
      }

      return Promise.resolve({
        fileName: "dokaz-cene-kampanja-7.html",
        path: "C:/exports/dokaz-cene-kampanja-7.html",
        mimeType: "text/html",
        rowCount: 1,
      });
    });
    const services = createLocalServices(invoke);

    await services.campaigns.exportEvidence(7);
    await services.campaigns.exportLabels(7);
    await services.campaigns.correctionReport();
    await services.campaigns.exportCorrectionReport();

    expect(invoke).toHaveBeenNthCalledWith(1, "campaigns_export_evidence", {
      campaignId: 7,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "campaigns_export_labels", {
      campaignId: 7,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "campaigns_correction_report");
    expect(invoke).toHaveBeenNthCalledWith(
      4,
      "campaigns_export_correction_report",
    );
  });

  it("maps reklamacije service methods to stable Tauri command names", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    const services = createLocalServices(invoke);

    const input: ReklamacijaInput = {
      podnosilacImePrezime: "Petar Petrović",
      kontakt: "060/123-456",
      podaciORobi: "Frižider Beko",
      opisNesaobraznosti: "Ne hladi",
      zahtev: "Zamena",
      robaKind: "tehnicka",
      filedAt: "2026-06-15T00:00:00Z",
    };
    const answer: AnswerInput = {
      answerText: "Reklamacija prihvaćena.",
      warningDuty: "Dužnost izjašnjenja.",
      warningConsequences: "Posledice.",
      warningZastoj: "Zastoj rokova.",
      eventDate: "2026-06-18T00:00:00Z",
    };

    await services.reklamacije.list();
    await services.reklamacije.get(7);
    await services.reklamacije.create(input);
    await services.reklamacije.logAnswer(7, answer);
    await services.reklamacije.consumerReceived(7, "2026-06-20T00:00:00Z");
    await services.reklamacije.consumerResponded(7, "2026-06-22T00:00:00Z");
    await services.reklamacije.grantExtension(
      7,
      "2026-07-20T00:00:00Z",
      true,
      "Naručen rezervni deo",
      "2026-06-22T00:00:00Z",
    );
    await services.reklamacije.resolve(7, "zamena", "2026-06-25T00:00:00Z");
    await services.reklamacije.exportPotvrda(7);
    await services.reklamacije.exportNotice();

    expect(invoke).toHaveBeenNthCalledWith(1, "reklamacija_list");
    expect(invoke).toHaveBeenNthCalledWith(2, "reklamacija_get", { id: 7 });
    expect(invoke).toHaveBeenNthCalledWith(3, "reklamacija_create", { input });
    expect(invoke).toHaveBeenNthCalledWith(4, "reklamacija_log_answer", {
      id: 7,
      input: answer,
    });
    expect(invoke).toHaveBeenNthCalledWith(5, "reklamacija_consumer_received", {
      id: 7,
      eventDate: "2026-06-20T00:00:00Z",
    });
    expect(invoke).toHaveBeenNthCalledWith(6, "reklamacija_consumer_responded", {
      id: 7,
      eventDate: "2026-06-22T00:00:00Z",
    });
    expect(invoke).toHaveBeenNthCalledWith(7, "reklamacija_grant_extension", {
      id: 7,
      newDeadline: "2026-07-20T00:00:00Z",
      consumerConsent: true,
      reason: "Naručen rezervni deo",
      eventDate: "2026-06-22T00:00:00Z",
    });
    expect(invoke).toHaveBeenNthCalledWith(8, "reklamacija_resolve", {
      id: 7,
      nacin: "zamena",
      eventDate: "2026-06-25T00:00:00Z",
    });
    expect(invoke).toHaveBeenNthCalledWith(9, "reklamacija_export_potvrda", {
      id: 7,
    });
    expect(invoke).toHaveBeenNthCalledWith(10, "reklamacija_export_notice");
  });

  it("maps kep service methods to stable Tauri command names", async () => {
    const invoke = vi.fn().mockImplementation((command: string) => {
      switch (command) {
        case "kep_ledger":
          return Promise.resolve({
            bookYear: 2026,
            entries: [],
            openingSaldoMinor: 0,
            saldoMinor: 0,
          });
        case "kep_status":
          return Promise.resolve({
            overdueSalesDays: [],
            unbookedReceiptCount: 0,
          });
        case "kep_close_preview":
          return Promise.resolve({
            krajnjiSaldoMinor: 780000,
            entryCount: 1,
            alreadyClosed: false,
          });
        case "kep_close_year":
          return Promise.resolve({
            bookYear: 2026,
            krajnjiSaldoMinor: 780000,
            entryCount: 1,
            closedAt: "2027-01-05T09:00:00Z",
            closedBy: 1,
          });
        case "kep_list_closures":
          return Promise.resolve([]);
        case "kep_export_close":
        case "kep_export_book":
          return Promise.resolve({
            fileName: "kep-doc.html",
            path: "exports/kep-doc.html",
            mimeType: "text/html",
            rowCount: 0,
          });
        default:
          return Promise.resolve(null);
      }
    });
    const services = createLocalServices(invoke);

    await services.kep.ledger(2026);
    await services.kep.postDailySales("2026-07-05", 234000);
    await services.kep.status();
    await services.kep.closePreview(2026);
    await services.kep.closeYear(2026, "ZAKLJUČI KNJIGU");
    await services.kep.listClosures();
    await services.kep.exportClose(2026);
    await services.kep.exportBook(2026);

    expect(invoke).toHaveBeenNthCalledWith(1, "kep_ledger", { bookYear: 2026 });
    expect(invoke).toHaveBeenNthCalledWith(2, "kep_post_daily_sales", {
      date: "2026-07-05",
      overrideAmountMinor: 234000,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "kep_status");
    expect(invoke).toHaveBeenCalledWith("kep_close_preview", {
      bookYear: 2026,
    });
    expect(invoke).toHaveBeenCalledWith("kep_close_year", {
      bookYear: 2026,
      confirmation: "ZAKLJUČI KNJIGU",
    });
    expect(invoke).toHaveBeenCalledWith("kep_list_closures");
    expect(invoke).toHaveBeenCalledWith("kep_export_close", { bookYear: 2026 });
    expect(invoke).toHaveBeenCalledWith("kep_export_book", { bookYear: 2026 });
  });

  it("maps kep kalkulacija/nivelacija/storno methods to stable Tauri command names", async () => {
    const invoke = vi.fn().mockImplementation((command: string) => {
      if (command === "kep_list_kalkulacije") {
        return Promise.resolve([]);
      }

      return Promise.resolve(undefined);
    });
    const services = createLocalServices(invoke);
    const basis: BasisDoc = {
      naziv: "Zapisnik o otpisu",
      broj: "7",
      datum: "08.07.2026",
    };

    await services.kep.listKalkulacije(2026);
    await services.kep.exportKalkulacija(5);
    await services.kep.nivelacija(1, 17600, basis);
    await services.kep.postAdjustment("otpis", 1, 35000, basis);
    await services.kep.correctEntry(3, 2026, 780000, basis);

    expect(invoke).toHaveBeenNthCalledWith(1, "kep_list_kalkulacije", {
      bookYear: 2026,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "kep_export_kalkulacija", {
      id: 5,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "kep_nivelacija", {
      productId: 1,
      newSalePriceMinor: 17600,
      basis,
    });
    expect(invoke).toHaveBeenNthCalledWith(4, "kep_post_adjustment", {
      cause: "otpis",
      productId: 1,
      quantityMilli: 35000,
      basis,
    });
    expect(invoke).toHaveBeenNthCalledWith(5, "kep_correct_entry", {
      targetRedniBroj: 3,
      bookYear: 2026,
      correctAmountMinor: 780000,
      basis,
    });
  });

  it("maps the ZZPL privacy surface to stable Tauri command names", async () => {
    const invoke = vi.fn().mockImplementation((command: string) => {
      switch (command) {
        case "audit_search":
          return Promise.resolve({
            events: [],
            chain: {
              verdict: "intact",
              intact: true,
              checkedRows: 0,
              label: "Potvrđena — lanac otisaka je neprekinut.",
            },
          });
        case "breaches_list":
        case "cl47_list":
        case "cl47_generate":
          return Promise.resolve([]);
        case "support_active_session":
          return Promise.resolve(null);
        default:
          return Promise.resolve(null);
      }
    });
    const services = createLocalServices(invoke);

    await services.privacy.grantSupportAccess("Pregled greške na štampi", 60);
    await services.privacy.enterSupportSession();
    await services.privacy.endSupportSession();
    await services.privacy.activeSupportSession();
    await services.privacy.searchAudit({
      from: "2026-08-01",
      to: "2026-08-31",
      actorUserId: 3,
    });
    await services.privacy.exportAuditCsv({
      from: null,
      to: null,
      actorUserId: null,
    });
    await services.privacy.listBreaches();
    await services.privacy.recordBreach(draft);
    await services.privacy.updateBreach(4, draft);
    await services.privacy.breachNotice();
    await services.privacy.exportBreachObrazac(4);
    await services.privacy.listProcessingActivities();
    await services.privacy.generateProcessingActivities();
    await services.privacy.exportProcessingActivities();

    // The nalog travels as one request object, so its obim and its trajanje
    // cannot be sent apart from each other.
    expect(invoke).toHaveBeenNthCalledWith(1, "support_grant_access", {
      request: { scope: "Pregled greške na štampi", durationMinutes: 60 },
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "support_request_access");
    expect(invoke).toHaveBeenNthCalledWith(3, "support_end_session");
    expect(invoke).toHaveBeenNthCalledWith(4, "support_active_session");
    expect(invoke).toHaveBeenCalledWith("audit_search", {
      query: { from: "2026-08-01", to: "2026-08-31", actorUserId: 3 },
    });
    expect(invoke).toHaveBeenCalledWith("audit_export_csv", {
      query: { from: null, to: null, actorUserId: null },
    });
    expect(invoke).toHaveBeenCalledWith("breaches_list");
    expect(invoke).toHaveBeenCalledWith("breaches_record", { draft });
    expect(invoke).toHaveBeenCalledWith("breaches_update", { id: 4, draft });
    expect(invoke).toHaveBeenCalledWith("breaches_notice");
    expect(invoke).toHaveBeenCalledWith("breaches_export_obrazac", { id: 4 });
    expect(invoke).toHaveBeenCalledWith("cl47_list");
    expect(invoke).toHaveBeenCalledWith("cl47_generate");
    expect(invoke).toHaveBeenCalledWith("cl47_export");

    // Req. 7: there is no write verb on the evidencija pristupa to map.
    const commands = invoke.mock.calls.map(([command]) => command as string);
    expect(commands.some((command) => /^audit_(record|update|delete)/.test(command))).toBe(
      false,
    );
  });

  it("opens an exported document for printing through the opener plugin", async () => {
    const { openPath } = await import("@tauri-apps/plugin-opener");
    const services = createLocalServices(vi.fn());
    await services.print.openForPrint("C:/exports/etikete-kampanja-1.html");
    expect(openPath).toHaveBeenCalledWith("C:/exports/etikete-kampanja-1.html");
  });

  it("opens an external URL through the opener plugin", async () => {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    const services = createLocalServices(vi.fn());
    await services.print.openExternalUrl("https://www.purs.gov.rs/");
    expect(openUrl).toHaveBeenCalledWith("https://www.purs.gov.rs/");
  });
});

describe("mock service adapter", () => {
  it("returns deterministic health for UI tests", async () => {
    const services = createMockServices();
    const health = await services.settings.getHealth();

    expect(health).toEqual({
      backend: "local",
      appVersion: "test",
      databasePath: "mock://vantumpos.sqlite3",
      migrated: true,
    });
  });

  it("returns deterministic auth, users, and shift data for UI tests", async () => {
    const services = createMockServices();

    await expect(services.auth.getSession()).resolves.toMatchObject({
      user: { username: "admin", displayName: "Administrator", role: "admin" },
      currentShift: { status: "open" },
    });
    await expect(
      services.auth.login({ username: "admin", credential: "1234" }),
    ).resolves.toMatchObject({
      user: { username: "admin", displayName: "Administrator", role: "admin" },
      currentShift: null,
    });
    await expect(services.users.listUsers()).resolves.toEqual(
      expect.arrayContaining([
        expect.objectContaining({ username: "admin", role: "admin" }),
      ]),
    );
  });

  it("applies cash in/out to expected cash and running totals in mock services", async () => {
    const services = createMockServices();
    const opening = await services.shifts.getCurrentShift();

    const afterPayIn = await services.shifts.shiftCashMovement({
      direction: "pay_in",
      amountMinor: 3000,
      reason: "Sitan novac",
    });
    expect(afterPayIn.paidInMinor).toBe(3000);
    expect(afterPayIn.paidOutMinor).toBe(0);
    expect(afterPayIn.expectedCashMinor).toBe(
      (opening?.expectedCashMinor ?? 0) + 3000,
    );

    const afterPayOut = await services.shifts.shiftCashMovement({
      direction: "pay_out",
      amountMinor: 1000,
      reason: "Pazar u banku",
    });
    expect(afterPayOut.paidInMinor).toBe(3000);
    expect(afterPayOut.paidOutMinor).toBe(1000);
    expect(afterPayOut.expectedCashMinor).toBe(
      (opening?.expectedCashMinor ?? 0) + 3000 - 1000,
    );

    await expect(services.shifts.getCurrentShift()).resolves.toMatchObject({
      paidInMinor: 3000,
      paidOutMinor: 1000,
    });
  });

  it("rejects non-positive cash movement amounts in mock services", async () => {
    const services = createMockServices();

    await expect(
      services.shifts.shiftCashMovement({
        direction: "pay_in",
        amountMinor: 0,
        reason: null,
      }),
    ).rejects.toMatchObject({ code: "validation_error" });
  });

  it("persists settings, tax rates, receipt numbering, and backup jobs in mock services", async () => {
    const services = createMockServices();

    const company = await services.settings.updateCompanySettings({
      shopName: "Vantum Market",
      address: "Bulevar 1",
      pib: "123456789",
      registrationNumber: "87654321",
      phone: "+381 11 123 456",
      logoPath: null,
      currency: "RSD",
    });
    const taxRate = await services.settings.saveTaxRate({
      id: null,
      name: "PDV 20",
      rateBasisPoints: 2000,
      active: true,
    });
    const receipt = await services.settings.updateReceiptSettings({
      prefix: "VP-",
      nextSequenceNumber: 42,
    });
    const backupSettings = await services.backup.updateBackupSettings({
      backupFolder: "D:/VantumPOS/backups",
      automaticBackupEnabled: true,
    });
    const backupJob = await services.backup.createBackup({
      backupFolder: "D:/VantumPOS/backups",
    });

    await expect(services.settings.getCompanySettings()).resolves.toEqual(company);
    expect(taxRate.name).toBe("PDV 20");
    expect(receipt).toMatchObject({ prefix: "VP-", resetPolicy: "none" });
    expect(backupSettings.automaticBackupEnabled).toBe(true);
    expect(backupJob.status).toBe("completed");
    await expect(services.backup.getBackupStatus()).resolves.toMatchObject({
      stale: false,
      lastSuccessfulBackup: expect.objectContaining({ status: "completed" }),
    });
  });

  it("keeps inventory stock and ledger deterministic for UI tests", async () => {
    const services = createMockServices();

    const initial = await services.inventory.listStock({ stockState: "low" });
    const lowStockItem = initial.items.find((item) => item.sku === "MLEKO-1L");
    expect(lowStockItem?.lowStock).toBe(true);

    await services.inventory.receiveStock({
      productId: lowStockItem?.productId ?? 0,
      quantityMilli: 2000,
      reason: "Prijem robe",
    });

    const ledger = await services.inventory.getProductLedger(
      lowStockItem?.productId ?? 0,
    );
    expect(ledger.movements[0]).toMatchObject({
      movementType: "receive",
      quantityMilli: 2000,
      resultingQuantityMilli:
        (lowStockItem?.currentQuantityMilli ?? 0) + 2000,
      reason: "Prijem robe",
    });
  });

  it("returns deterministic receipt history and applies partial returns", async () => {
    const services = createMockServices();

    const search = await services.receipts.searchReceipts({
      product: "Kafa",
    });
    expect(search.receipts.map((receipt) => receipt.receiptNumber)).toEqual([
      "R-2026-0001",
    ]);

    const detail = await services.receipts.returnItems({
      receiptId: search.receipts[0].id,
      reason: "Kupac vratio jedan komad",
      items: [{ saleItemId: 1, quantityMilli: 1000 }],
    });

    expect(detail.status).toBe("refunded");
    expect(detail.items[0].returnedQuantityMilli).toBe(1000);
    expect(detail.linkedDocuments.map((document) => document.documentType)).toEqual([
      "return",
    ]);
  });

  it("carries a campaign draft through activate and end at mock fidelity", async () => {
    const services = createMockServices();

    const report = await services.campaigns.validateCampaign(campaignInput());
    expect(report).toEqual({ hard: [], warnings: [], anchors: [] });

    const created = await services.campaigns.createCampaign(campaignInput());
    expect(created.id).toBeGreaterThan(0);
    expect(created.status).toBe("draft");
    expect(created.activatedAt).toBeNull();
    expect(created.items[0]).toMatchObject({
      productId: 1,
      campaignPriceMinor: 1290000,
    });

    await expect(services.campaigns.listCampaigns()).resolves.toMatchObject([
      { id: created.id, status: "draft", itemCount: 1 },
    ]);
    await expect(services.campaigns.getCampaign(created.id)).resolves.toEqual(
      created,
    );

    const active = await services.campaigns.activateCampaign(created.id);
    expect(active.status).toBe("active");
    expect(active.activatedAt).not.toBeNull();

    const ended = await services.campaigns.endCampaign(created.id, [
      { productId: 1, returnPriceMinor: 1590000 },
    ]);
    expect(ended.status).toBe("ended");
    expect(ended.endedAt).not.toBeNull();
  });

  it("marks a cancelled mock campaign without ending it", async () => {
    const services = createMockServices();

    const created = await services.campaigns.createCampaign(campaignInput());
    const cancelled = await services.campaigns.cancelCampaign(created.id);

    expect(cancelled.status).toBe("cancelled");
    expect(cancelled.activatedAt).toBeNull();
  });

  it("carries a reklamacija through its lifecycle at mock fidelity", async () => {
    const services = createMockServices();

    const created = await services.reklamacije.create(reklamacijaInput());
    expect(created.id).toBeGreaterThan(0);
    expect(created.registerNumber).toBeGreaterThan(0);
    expect(created.status).toBe("open");
    expect(created.regime).toBeTruthy();
    expect(created.purgeEligible).toBe(false);
    // A plausible DeadlineState: the 8-day answer clock is running, not overdue.
    expect(created.deadlines.answerDue).toBeTruthy();
    expect(created.deadlines.resolutionDue).toBeTruthy();
    expect(created.deadlines.clock).toBe("running");
    expect(created.deadlines.answerOverdue).toBe(false);
    expect(created.deadlines.oneExtensionUsed).toBe(false);

    await expect(services.reklamacije.list()).resolves.toMatchObject([
      {
        id: created.id,
        registerNumber: created.registerNumber,
        status: "open",
      },
    ]);
    await expect(services.reklamacije.get(created.id)).resolves.toEqual(created);

    const answered = await services.reklamacije.logAnswer(
      created.id,
      answerInput(),
    );
    expect(answered.status).toBe("answered");

    const received = await services.reklamacije.consumerReceived(
      created.id,
      "2026-06-20T00:00:00Z",
    );
    expect(received.status).toBe("awaiting_consumer");
    expect(received.deadlines.clock).toBe("paused");
    expect(received.deadlines.resolutionDue).toBeNull();
    expect(received.deadlines.consumerWindowDue).toBeTruthy();

    const responded = await services.reklamacije.consumerResponded(
      created.id,
      "2026-06-22T00:00:00Z",
    );
    expect(responded.deadlines.clock).toBe("running");
    expect(responded.deadlines.resolutionDue).toBeTruthy();

    const extended = await services.reklamacije.grantExtension(
      created.id,
      "2026-07-20T00:00:00Z",
      true,
      "Naručen rezervni deo",
      "2026-06-22T00:00:00Z",
    );
    expect(extended.deadlines.oneExtensionUsed).toBe(true);
    expect(extended.deadlines.resolutionDue).toBe("2026-07-20T00:00:00Z");

    const resolved = await services.reklamacije.resolve(
      created.id,
      "zamena",
      "2026-06-25T00:00:00Z",
    );
    expect(resolved.status).toBe("resolved");
    expect(resolved.deadlines.clock).toBe("resolved");
    expect(resolved.deadlines.resolutionDue).toBeNull();
  });

  it("keeps a seeded KEP ledger and appends daily postings at mock fidelity", async () => {
    const services = createMockServices();

    const ledger = await services.kep.ledger(2026);
    expect(ledger.bookYear).toBe(2026);
    expect(ledger.entries.length).toBeGreaterThan(0);
    expect(ledger.saldoMinor).toBe(ledger.entries[0].zaduzenjeMinor);

    const posted = await services.kep.postDailySales("2026-07-05", 234000);
    expect(posted.razduzenjeMinor).toBe(234000);
    expect(posted.zaduzenjeMinor).toBeNull();
    expect(posted.kind).toBe("daily_sales");

    const after = await services.kep.ledger(2026);
    expect(after.entries.length).toBe(ledger.entries.length + 1);
    expect(after.saldoMinor).toBe(ledger.saldoMinor - 234000);

    await expect(services.kep.status()).resolves.toEqual({
      overdueSalesDays: [],
      unbookedReceiptCount: 0,
    });
  });

  it("stubs the kalkulacija/nivelacija/storno surface at mock fidelity", async () => {
    const services = createMockServices();
    const basis: BasisDoc = {
      naziv: "Zapisnik o otpisu",
      broj: "7",
      datum: "08.07.2026",
    };

    await expect(services.kep.listKalkulacije(2026)).resolves.toEqual([]);
    await expect(services.kep.exportKalkulacija(5)).resolves.toMatchObject({
      fileName: "kalkulacija-5.html",
      mimeType: "text/html",
    });
    await expect(
      services.kep.nivelacija(1, 17600, basis),
    ).resolves.toBeUndefined();
    await expect(
      services.kep.postAdjustment("otpis", 1, 35000, basis),
    ).resolves.toBeUndefined();
    await expect(
      services.kep.correctEntry(3, 2026, 780000, basis),
    ).resolves.toBeUndefined();
  });

  it("exports the potvrda and notice at mock fidelity", async () => {
    const services = createMockServices();
    const created = await services.reklamacije.create(reklamacijaInput());

    await expect(
      services.reklamacije.exportPotvrda(created.id),
    ).resolves.toMatchObject({
      fileName: `potvrda-reklamacija-${created.registerNumber}.html`,
      mimeType: "text/html",
    });
    await expect(services.reklamacije.exportNotice()).resolves.toMatchObject({
      fileName: "obavestenje-reklamacije.html",
      mimeType: "text/html",
    });
  });
});

function reklamacijaInput(): ReklamacijaInput {
  return {
    podnosilacImePrezime: "Petar Petrović",
    kontakt: "060/123-456",
    podaciORobi: "Frižider Beko",
    opisNesaobraznosti: "Ne hladi",
    zahtev: "Zamena",
    robaKind: "tehnicka",
    filedAt: "2026-06-15T00:00:00Z",
  };
}

function answerInput(): AnswerInput {
  return {
    answerText: "Reklamacija prihvaćena.",
    warningDuty: "Dužnost izjašnjenja.",
    warningConsequences: "Posledice.",
    warningZastoj: "Zastoj rokova.",
    eventDate: "2026-06-18T00:00:00Z",
  };
}

function campaignInput(): CampaignInput {
  return {
    campaignType: "sezonsko_snizenje",
    startsOn: "2026-07-01T00:00:00Z",
    endsOn: "2026-07-31T00:00:00Z",
    displayMode: "two_prices",
    headlinePercent: null,
    rasprodajaGround: null,
    specialConditions: null,
    reducedUtilityReason: null,
    marketingLabel: "Letnje sniženje",
    seasonAttested: true,
    separationAttested: false,
    items: [
      {
        productId: 1,
        campaignPriceMinor: 1290000,
        manualPrethodnaMinor: null,
        anchorJustification: null,
        futureRegularPriceMinor: null,
      },
    ],
  };
}
