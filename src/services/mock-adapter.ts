import type { PosServices } from "./ports";
import type {
  AuthSession,
  BackupJob,
  BackupSettings,
  CampaignInput,
  CampaignItemView,
  CampaignView,
  CategorySummary,
  CompanySettings,
  CreateBackupRequest,
  ImportJob,
  InventoryAdjustmentRequest,
  KepClosure,
  KepClosureView,
  KepEntryView,
  PrethodnaCenaDto,
  ProductLedgerMovement,
  ProductListQuery,
  ProductLookupSuggestion,
  ProductSummary,
  ReceiptDetail,
  ReceiptSettings,
  ReklamacijaInput,
  ReklamacijaView,
  RestoreBackupRequest,
  SaleDraftRequest,
  SalePreview,
  ShiftSummary,
  ShopProfile,
  StockListItem,
  TaxRate,
  UserAccount,
  TaxRateSummary,
} from "./types";

const now = "2026-06-18T10:00:00Z";

export function createMockServices(): PosServices {
  let categories: CategorySummary[] = [
    { id: 1, name: "Mlecni proizvodi", active: true },
    { id: 2, name: "Kafa", active: true },
  ];
  let taxRates: TaxRate[] = [
    { id: 1, name: "PDV 20", rateBasisPoints: 2000, active: true },
    { id: 2, name: "PDV 10", rateBasisPoints: 1000, active: true },
  ];
  const products: ProductSummary[] = [
    {
      id: 1,
      name: "Mleko 1 l",
      sku: "MLEKO-1L",
      barcode: "8600000000010",
      categoryId: 1,
      categoryName: "Mlecni proizvodi",
      unitOfMeasure: "kom",
      salePriceMinor: 15999,
      purchasePriceMinor: 12000,
      taxRateId: 1,
      taxRateName: "PDV 20",
      taxRateBasisPoints: 2000,
      minimumStockMilli: 4000,
      currentStockMilli: 3000,
      allowNegativeStock: false,
      active: true,
      perishable: false,
      perishableJustification: null,
      externalSource: null,
    },
    {
      id: 2,
      name: "Kafa 200 g",
      sku: "KAFA-200",
      barcode: "8600000000027",
      categoryId: 2,
      categoryName: "Kafa",
      unitOfMeasure: "kom",
      salePriceMinor: 50000,
      purchasePriceMinor: 32000,
      taxRateId: 1,
      taxRateName: "PDV 20",
      taxRateBasisPoints: 2000,
      minimumStockMilli: 1000,
      currentStockMilli: 3000,
      allowNegativeStock: false,
      active: true,
      perishable: false,
      perishableJustification: null,
      externalSource: null,
    },
  ];
  const ledgerMovements = new Map<number, ProductLedgerMovement[]>();
  const campaigns: CampaignView[] = [];
  const reklamacije: ReklamacijaView[] = [];
  // A single seeded receipt zaduženje (50 kom x 156,00 retail incl PDV).
  const kepEntries: KepEntryView[] = [
    {
      redniBroj: 1,
      datum: "04.07",
      opis: "Prijem robe",
      zaduzenjeMinor: 780000,
      razduzenjeMinor: null,
      kind: "receipt",
    },
  ];
  const kepClosures: KepClosureView[] = [];
  const receipt = createReceiptDetail();
  let users: UserAccount[] = [
    {
      id: 1,
      username: "admin",
      displayName: "Administrator",
      role: "admin",
      active: true,
      createdAt: now,
      updatedAt: now,
      lastLoginAt: now,
    },
    {
      id: 2,
      username: "marko",
      displayName: "Marko Markovic",
      role: "cashier",
      active: true,
      createdAt: now,
      updatedAt: now,
      lastLoginAt: null,
    },
  ];
  let currentShift: ShiftSummary | null = {
    id: 1,
    userId: 1,
    cashierName: "Administrator",
    openedAt: now,
    closedAt: null,
    openingCashMinor: 500000,
    expectedCashMinor: 500000,
    countedCashMinor: null,
    cashSalesMinor: 0,
    cardSalesMinor: 0,
    paidInMinor: 0,
    paidOutMinor: 0,
    differenceMinor: null,
    status: "open",
    openingNote: "Jutarnja smena",
    closingNote: null,
  };
  let session: AuthSession | null = {
    user: users[0],
    currentShift,
  };
  let importJobs: ImportJob[] = [
    {
      id: 1,
      importType: "products" as const,
      fileName: "prethodni-products.csv",
      status: "completed" as const,
      totalRows: 2,
      errorRows: 0,
      createdAt: now,
      completedAt: now,
    },
  ];
  let companySettings: CompanySettings = {
    shopName: "Vantum Market",
    address: "Bulevar 1, Beograd",
    pib: "",
    registrationNumber: "87654321",
    phone: "+381 11 123 456",
    logoPath: null,
    currency: "RSD",
  };
  let receiptSettings: ReceiptSettings = {
    prefix: "VP-",
    nextSequenceNumber: 1,
    resetPolicy: "none",
  };
  let salesSettings = { allowOverselling: false };
  // Every answer starts UNSET. A fresh install must ask, never assume — see
  // SW11-SW15-VERIFIED-RULES §3 req 31 / §5 Q-8.
  let shopProfile: ShopProfile = {
    pravnaForma: null,
    pdvObveznik: null,
    distanceSelling: null,
    lpfrInPremises: null,
    esirElements: [],
  };
  let backupSettings: BackupSettings = {
    backupFolder: "mock://backups",
    automaticBackupEnabled: true,
  };
  let backupJobs: BackupJob[] = [];

  function findProduct(productId: number) {
    const product = products.find((item) => item.id === productId);

    if (!product) {
      throw { code: "not_found", message: "Artikal nije pronađen." };
    }

    return product;
  }

  function applyStock(
    request: InventoryAdjustmentRequest,
    movementType: "receive" | "correction" | "write_off",
  ) {
    const product = findProduct(request.productId);
    const previousQuantityMilli = product.currentStockMilli;
    const delta =
      movementType === "write_off" ? -request.quantityMilli : request.quantityMilli;
    const newQuantityMilli = previousQuantityMilli + delta;

    if (newQuantityMilli < 0 && !product.allowNegativeStock) {
      throw { code: "insufficient_stock", message: "Nema dovoljno zaliha." };
    }

    product.currentStockMilli = newQuantityMilli;
    const movement = {
      id: Date.now(),
      movementType,
      quantityMilli: delta,
      resultingQuantityMilli: newQuantityMilli,
      reason: request.reason ?? null,
      referenceType: request.referenceType ?? null,
      referenceId: request.referenceId ?? null,
      createdAt: now,
    };
    ledgerMovements.set(product.id, [
      movement,
      ...(ledgerMovements.get(product.id) ?? []),
    ]);

    return {
      productId: product.id,
      movementId: movement.id,
      movementType,
      quantityMilli: request.quantityMilli,
      previousQuantityMilli,
      newQuantityMilli,
      createdAt: now,
    };
  }

  const services: PosServices & { initialSession: AuthSession | null } = {
    initialSession: session,
    settings: {
      async getHealth() {
        return {
          backend: "local",
          appVersion: "test",
          databasePath: "mock://vantumpos.sqlite3",
          migrated: true,
        };
      },
      async getCompanySettings() {
        return companySettings;
      },
      async updateCompanySettings(request) {
        if (request.pib && !/^\d{9}$/.test(request.pib)) {
          throw { code: "validation_error", message: "PIB mora imati 9 cifara." };
        }
        if (request.currency !== "RSD") {
          throw {
            code: "validation_error",
            message: "Valuta za lokalni MVP mora biti RSD.",
          };
        }

        companySettings = { ...request };
        return companySettings;
      },
      async listTaxRates() {
        return taxRates;
      },
      async seedTaxRates(inVatSystem: boolean) {
        if (taxRates.length === 0) {
          taxRates = inVatSystem
            ? [
                { id: 1, name: "PDV 20%", rateBasisPoints: 2000, active: true },
                { id: 2, name: "PDV 10%", rateBasisPoints: 1000, active: true },
              ]
            : [{ id: 1, name: "Bez PDV-a", rateBasisPoints: 0, active: true }];
        }
        return taxRates;
      },
      async saveTaxRate(request) {
        if (!request.name.trim()) {
          throw { code: "validation_error", message: "Naziv PDV stope je obavezan." };
        }

        const saved = {
          id: request.id ?? Math.max(0, ...taxRates.map((item) => item.id)) + 1,
          name: request.name,
          rateBasisPoints: request.rateBasisPoints,
          active: request.active,
        };
        taxRates = request.id
          ? taxRates.map((item) => (item.id === request.id ? saved : item))
          : [...taxRates, saved];

        return saved;
      },
      async getReceiptSettings() {
        return receiptSettings;
      },
      async updateReceiptSettings(request) {
        if (session?.user.role !== "admin") {
          throw {
            code: "forbidden",
            message: "Samo administrator može da izvrši ovu akciju.",
          };
        }

        receiptSettings = { ...request, resetPolicy: "none" };
        return receiptSettings;
      },
      async getSalesSettings() {
        return salesSettings;
      },
      async updateSalesSettings(request) {
        if (session?.user.role !== "admin") {
          throw {
            code: "forbidden",
            message: "Samo administrator može da izvrši ovu akciju.",
          };
        }

        salesSettings = { ...request };
        return salesSettings;
      },
      async getShopProfile() {
        return shopProfile;
      },
      async updateShopProfile(request) {
        if (session?.user.role !== "admin") {
          throw {
            code: "forbidden",
            message: "Samo administrator može da izvrši ovu akciju.",
          };
        }

        for (const element of request.esirElements) {
          if (!element.naziv.trim() || !element.verzija.trim() || !element.ib.trim()) {
            throw {
              code: "validation_error",
              message: "Naziv, verzija i IB elementa su obavezni.",
            };
          }
        }

        shopProfile = {
          ...request,
          esirElements: request.esirElements.map((element) => ({
            ...element,
            naziv: element.naziv.trim(),
            verzija: element.verzija.trim(),
            ib: element.ib.trim(),
          })),
        };
        return shopProfile;
      },
    },
    backup: {
      async getBackupStatus() {
        const lastSuccessfulBackup =
          backupJobs.find(
            (job) =>
              job.status === "completed" &&
              (job.backupType === "manual" || job.backupType === "automatic"),
          ) ?? null;
        const lastFailedBackup =
          backupJobs.find((job) => job.status === "failed") ?? null;

        return {
          ...backupSettings,
          stale: !lastSuccessfulBackup,
          encryptionConfigured: false,
          lastSuccessfulBackup,
          lastFailedBackup,
        };
      },
      async updateBackupSettings(request) {
        backupSettings = request;
        return backupSettings;
      },
      async createBackup(request: CreateBackupRequest) {
        if (request.backupFolder) {
          backupSettings = {
            ...backupSettings,
            backupFolder: request.backupFolder,
          };
        }

        const job = createBackupJob(
          "manual",
          `${backupSettings.backupFolder}/vantumpos-manual-${backupJobs.length + 1}.sqlite3`,
        );
        backupJobs = [job, ...backupJobs];
        return job;
      },
      async restoreBackup(request: RestoreBackupRequest) {
        if (request.confirmationText !== "VRATI PODATKE") {
          throw {
            code: "validation_error",
            message: "Potvrdite restore unosom teksta VRATI PODATKE.",
          };
        }

        const job = createBackupJob("restore", request.path);
        backupJobs = [job, ...backupJobs];
        return job;
      },
      async listBackupJobs() {
        return backupJobs;
      },
      async setBackupPassphrase() {},
      async resetTradingData(confirmationText: string) {
        if (confirmationText !== "OBRISI PODATKE") {
          throw {
            code: "validation_error",
            message: "Potvrdite brisanje unosom teksta OBRISI PODATKE.",
          };
        }
      },
    },
    auth: {
      async getSession() {
        return session;
      },
      async login(request) {
        const user = users.find(
          (item) => item.username === request.username && item.active,
        );

        if (!user || request.credential !== "1234") {
          throw {
            code: "invalid_credentials",
            message: "Korisničko ime ili lozinka nisu ispravni.",
          };
        }

        session = {
          user: { ...user, lastLoginAt: now },
          currentShift: null,
        };

        return session;
      },
      async logout() {
        session = null;
      },
    },
    users: {
      async listUsers() {
        return users;
      },
      async createUser(request) {
        const user = {
          id: Math.max(0, ...users.map((item) => item.id)) + 1,
          username: request.username,
          displayName: request.displayName,
          role: request.role,
          active: request.active,
          createdAt: now,
          updatedAt: now,
          lastLoginAt: null,
        };
        users = [...users, user];
        return user;
      },
      async updateUser(id, request) {
        const current = users.find((user) => user.id === id);

        if (!current) {
          throw { code: "not_found", message: "Korisnik nije pronađen." };
        }

        const updated = {
          ...current,
          username: request.username,
          displayName: request.displayName,
          role: request.role,
          active: request.active,
          updatedAt: now,
        };
        users = users.map((user) => (user.id === id ? updated : user));
        return updated;
      },
      async deactivateUser(id) {
        users = users.map((user) =>
          user.id === id ? { ...user, active: false, updatedAt: now } : user,
        );
      },
    },
    shifts: {
      async getCurrentShift() {
        return session?.currentShift ?? null;
      },
      async openShift(request) {
        if (!session) {
          throw { code: "unauthorized", message: "Prijavite se za rad." };
        }

        const openedShift: ShiftSummary = {
          id: 2,
          userId: session.user.id,
          cashierName: session.user.displayName,
          openedAt: now,
          closedAt: null,
          openingCashMinor: request.openingCashMinor,
          expectedCashMinor: request.openingCashMinor,
          countedCashMinor: null,
          cashSalesMinor: 0,
          cardSalesMinor: 0,
          paidInMinor: 0,
          paidOutMinor: 0,
          differenceMinor: null,
          status: "open",
          openingNote: request.note ?? null,
          closingNote: null,
        };
        currentShift = openedShift;
        session = { ...session, currentShift };
        return openedShift;
      },
      async closeShift(request) {
        if (!session?.currentShift || session.currentShift.id !== request.shiftId) {
          throw { code: "not_found", message: "Smena nije pronađena." };
        }

        const closed = {
          ...session.currentShift,
          closedAt: now,
          countedCashMinor: request.countedCashMinor,
          differenceMinor: request.countedCashMinor - session.currentShift.expectedCashMinor,
          status: "closed" as const,
          closingNote: request.note ?? null,
        };
        currentShift = null;
        session = { ...session, currentShift: null };
        return closed;
      },
      async adminCloseShift(request) {
        if (session?.currentShift && session.currentShift.id === request.shiftId) {
          const closed = {
            ...session.currentShift,
            closedAt: now,
            countedCashMinor: request.countedCashMinor,
            differenceMinor:
              request.countedCashMinor - session.currentShift.expectedCashMinor,
            status: "closed" as const,
            closingNote: request.note ?? null,
          };
          currentShift = null;
          session = { ...session, currentShift: null };
          return closed;
        }

        return {
          id: request.shiftId,
          userId: 2,
          cashierName: "Marko Markovic",
          openedAt: now,
          closedAt: now,
          openingCashMinor: 0,
          expectedCashMinor: request.countedCashMinor,
          countedCashMinor: request.countedCashMinor,
          cashSalesMinor: 0,
          cardSalesMinor: 0,
          paidInMinor: 0,
          paidOutMinor: 0,
          differenceMinor: 0,
          status: "closed" as const,
          openingNote: null,
          closingNote: request.note ?? null,
        };
      },
      async shiftCashMovement(request) {
        if (!session?.currentShift) {
          throw { code: "shift_required", message: "Smena nije otvorena." };
        }

        if (request.amountMinor <= 0) {
          throw {
            code: "validation_error",
            message: "Iznos mora biti veći od nule.",
          };
        }

        const delta =
          request.direction === "pay_in"
            ? request.amountMinor
            : -request.amountMinor;
        const updated: ShiftSummary = {
          ...session.currentShift,
          expectedCashMinor: session.currentShift.expectedCashMinor + delta,
          paidInMinor:
            session.currentShift.paidInMinor +
            (request.direction === "pay_in" ? request.amountMinor : 0),
          paidOutMinor:
            session.currentShift.paidOutMinor +
            (request.direction === "pay_out" ? request.amountMinor : 0),
        };
        currentShift = updated;
        session = { ...session, currentShift: updated };
        return updated;
      },
    },
    catalog: {
      async listProducts(query) {
        const items = filterProducts(products, query);
        return {
          items,
          categories,
          taxRates,
          total: items.length,
        };
      },
      async searchProducts(query) {
        return {
          items: filterProducts(products, {
            search: query.search,
            active: query.active ?? true,
          }).slice(0, query.limit ?? 20),
          categories,
          taxRates,
          total: products.length,
        };
      },
      async getProduct(id) {
        return products.find((item) => item.id === id) ?? null;
      },
      async createProduct(request) {
        rejectDuplicateProduct(products, request.sku, request.barcode);
        const taxRate = findTaxRate(taxRates, request.taxRateId);
        const category = request.categoryId
          ? categories.find((item) => item.id === request.categoryId)
          : null;
        const product = {
          id: products.length + 1,
          ...request,
          categoryName: category?.name ?? null,
          taxRateName: taxRate.name,
          taxRateBasisPoints: taxRate.rateBasisPoints,
          currentStockMilli: 0,
          externalSource: request.externalSource ?? null,
        };
        products.push(product);
        return product;
      },
      async updateProduct(id, request) {
        const index = products.findIndex((item) => item.id === id);
        rejectDuplicateProduct(products, request.sku, request.barcode, id);
        const taxRate = findTaxRate(taxRates, request.taxRateId);
        const category = request.categoryId
          ? categories.find((item) => item.id === request.categoryId)
          : null;
        const product = {
          id,
          ...request,
          categoryName: category?.name ?? null,
          taxRateName: taxRate.name,
          taxRateBasisPoints: taxRate.rateBasisPoints,
          currentStockMilli: products[index]?.currentStockMilli ?? 0,
          externalSource: request.externalSource ?? null,
        };
        products[index] = product;
        return product;
      },
      async setProductActive(id, active) {
        const product = findProduct(id);
        product.active = active;
        return product;
      },
      async lookupProductByBarcode(barcode) {
        return mockBarcodeLookup(barcode);
      },
      async listCategories() {
        return categories;
      },
      async saveCategory(request) {
        const saved = {
          id: request.id ?? Math.max(0, ...categories.map((item) => item.id)) + 1,
          name: request.name,
          active: request.active ?? true,
        };

        categories = request.id
          ? categories.map((category) =>
              category.id === request.id ? saved : category,
            )
          : [...categories, saved];

        return saved;
      },
      async getPrethodnaCena() {
        return {
          status: "computed",
          priceMinor: 499000,
          windowDays: 30,
          windowFrom: "2026-06-20T00:00:00Z",
          windowTo: "2026-07-20T00:00:00Z",
          truncated: false,
          reason: null,
          ageDays: null,
        } satisfies PrethodnaCenaDto;
      },
    },
    sales: {
      async createSalePreview(request) {
        return createSalePreview(products, request);
      },
      async completeSale(request) {
        const preview = createSalePreview(products, request);
        const cashReceivedMinor =
          request.payments.find((payment) => payment.method === "cash")
            ?.amountMinor ?? 0;

        return {
          id: 1,
          localReceiptNumber: "VP-000001",
          createdAt: now,
          cashierName: "Administrator",
          fiscalStatus: "not_fiscalized",
          ...preview,
          payments: request.payments.map((payment) =>
            payment.method === "cash"
              ? { ...payment, amountMinor: preview.totalMinor }
              : payment,
          ),
          cashReceivedMinor,
          changeDueMinor: Math.max(cashReceivedMinor - preview.totalMinor, 0),
        };
      },
    },
    inventory: {
      async listStock(query) {
        const items = products.map(toStockItem);
        const filtered = items.filter((item) => {
          const matchesSearch =
            !query.search ||
            [item.productName, item.sku, item.barcode ?? ""].some((value) =>
              value.toLowerCase().includes(query.search!.toLowerCase()),
            );
          const matchesCategory =
            query.categoryId == null || item.categoryId === query.categoryId;
          const matchesState =
            query.stockState === "low"
              ? item.lowStock
              : query.stockState === "zero"
                ? item.currentQuantityMilli === 0
                : query.stockState === "negative"
                  ? item.currentQuantityMilli < 0
                  : true;

          return matchesSearch && matchesCategory && matchesState;
        });

        return { items: filtered };
      },
      async receiveStock(request) {
        return applyStock(request, "receive");
      },
      async correctStock(request) {
        return applyStock(request, "correction");
      },
      async writeOffStock(request) {
        return applyStock(request, "write_off");
      },
      async getProductLedger(productId) {
        const product = findProduct(productId);
        return {
          productId,
          product: toStockItem(product),
          movements: ledgerMovements.get(productId) ?? [],
        };
      },
    },
    receipts: {
      async searchReceipts(query) {
        const matchesProduct =
          !query.product ||
          receipt.items.some((item) =>
            item.productName.toLowerCase().includes(query.product!.toLowerCase()),
          );

        return {
          receipts: matchesProduct ? [receipt] : [],
          total: matchesProduct ? 1 : 0,
        };
      },
      async getReceipt() {
        return receipt;
      },
      async voidReceipt() {
        receipt.status = "voided";
        receipt.canVoid = false;
        receipt.canReturn = false;
        receipt.linkedDocuments = [
          {
            id: 2,
            documentType: "void",
            receiptNumber: "STO-R-2026-0001-1",
            status: "voided",
            createdAt: now,
            totalMinor: receipt.totalMinor,
          },
        ];
        receipt.linkedDocumentCount = receipt.linkedDocuments.length;
        return receipt;
      },
      async returnItems(request) {
        receipt.status = "refunded";
        for (const returned of request.items) {
          const item = receipt.items.find(
            (receiptItem) => receiptItem.id === returned.saleItemId,
          );
          if (item) {
            item.returnedQuantityMilli = returned.quantityMilli;
          }
        }
        receipt.linkedDocuments = [
          {
            id: 3,
            documentType: "return",
            receiptNumber: "POV-R-2026-0001-1",
            status: "refunded",
            createdAt: now,
            totalMinor: 50000,
          },
        ];
        receipt.linkedDocumentCount = receipt.linkedDocuments.length;
        receipt.canVoid = false;
        return receipt;
      },
      async setEsirNumber(_receiptId, esirReceiptNumber) {
        const trimmed = esirReceiptNumber.trim();
        receipt.esirReceiptNumber = trimmed.length > 0 ? trimmed : null;
        return receipt;
      },
    },
    imports: {
      async readImportHeaders(request) {
        const parsed = parseMockCsv(request.csvText);

        return {
          importType: request.importType,
          fileName: request.fileName,
          delimiter: parsed.delimiter,
          headers: parsed.headers,
          totalRows: parsed.rows.length,
        };
      },
      async validateImport(request) {
        const parsed = parseMockCsv(request.csvText);
        const priceHeader = request.mapping.sale_price;
        const priceIndex = parsed.headers.indexOf(priceHeader);
        const rows = parsed.rows
          .map((row, index) => {
            const price = priceIndex >= 0 ? row[priceIndex] : "";

            if (/neispravno|nije/i.test(price)) {
              return {
                rowNumber: index + 2,
                status: "error" as const,
                action: "skip" as const,
                message: "Cena nije ispravna.",
                values: rowToValues(parsed.headers, row),
              };
            }

            return {
              rowNumber: index + 2,
              status: "valid" as const,
              action: "create" as const,
              message: "",
              values: rowToValues(parsed.headers, row),
            };
          });
        const errorCount = rows.filter((row) => row.status === "error").length;

        return {
          importType: request.importType,
          fileName: request.fileName,
          totalRows: parsed.rows.length,
          validCount: rows.length - errorCount,
          warningCount: 0,
          errorCount,
          summary: { create: rows.length - errorCount, update: 0, skip: errorCount },
          rows,
        };
      },
      async commitImport(request) {
        const parsed = parseMockCsv(request.csvText);
        const job = {
          id: Math.max(0, ...importJobs.map((item) => item.id)) + 1,
          importType: request.importType,
          fileName: request.fileName,
          status: "completed" as const,
          totalRows: parsed.rows.length,
          errorRows: 0,
          createdAt: now,
          completedAt: now,
        };
        importJobs = [job, ...importJobs];

        return job;
      },
      async listImportJobs() {
        return importJobs;
      },
      async getImportJob(id) {
        const job = importJobs.find((item) => item.id === id);

        if (!job) {
          return null;
        }

        return {
          ...job,
          rows: [
            {
              rowNumber: 2,
              status: "imported" as const,
              message: null,
              values: { Naziv: "Hleb", Cena: "120,00" },
            },
          ],
        };
      },
    },
    reports: {
      async getDailyTurnover() {
        return {
          summary: {
            totalMinor: 12000,
            cashMinor: 8000,
            cardMinor: 4000,
            bankTransferMinor: 0,
            receiptCount: 2,
            averageReceiptMinor: 6000,
          },
          rows: [
            {
              day: "2026-06-17",
              receiptCount: 2,
              cashMinor: 8000,
              cardMinor: 4000,
              bankTransferMinor: 0,
              totalMinor: 12000,
              refundsOrVoidsMinor: -3000,
              refundsOrVoidsCount: 1,
            },
          ],
        };
      },
      async getShiftTurnover() {
        return {
          rows: [
            {
              shiftId: 1,
              openedAt: "2026-06-17T07:30:00Z",
              closedAt: null,
              cashierName: "Mira Kasir",
              receiptCount: 2,
              cashMinor: 8000,
              cardMinor: 4000,
              totalMinor: 12000,
            },
          ],
        };
      },
      async getCashierTurnover() {
        return {
          rows: [
            {
              cashierId: 1,
              cashierName: "Mira Kasir",
              receiptCount: 2,
              totalMinor: 12000,
            },
          ],
        };
      },
      async getPaymentMethodTurnover() {
        return {
          rows: [
            { paymentMethod: "cash" as const, receiptCount: 2, totalMinor: 8000 },
            { paymentMethod: "card" as const, receiptCount: 1, totalMinor: 4000 },
          ],
        };
      },
      async getProductSales() {
        return {
          rows: [
            {
              productId: 1,
              productName: "Kafa 200g",
              productSku: "KAF-200",
              quantityMilli: 2000,
              revenueMinor: 12000,
              discountMinor: 200,
              estimatedMarginMinor: 11160,
            },
          ],
        };
      },
      async getCategorySales() {
        return {
          rows: [
            {
              categoryId: 1,
              categoryName: "Pica",
              quantityMilli: 3000,
              revenueMinor: 12000,
              discountMinor: 200,
              estimatedMarginMinor: 10990,
            },
          ],
        };
      },
      async getLowStock() {
        return {
          rows: [
            {
              productId: 1,
              productName: "Kafa 200g",
              productSku: "KAF-200",
              currentStockMilli: 3000,
              minimumStockMilli: 5000,
              differenceMilli: -2000,
              lastMovementAt: "2026-06-17T12:00:00Z",
            },
          ],
        };
      },
      async listShifts() {
        return [
          {
            id: 1,
            openedAt: "2026-06-17T07:30:00Z",
            closedAt: null,
            cashierName: "Mira Kasir",
          },
        ];
      },
      async exportReportCsv(request) {
        return {
          fileName: `${request.reportType}-2026-06-17-2026-06-17.csv`,
          path: `mock://exports/${request.reportType}-2026-06-17-2026-06-17.csv`,
          mimeType: "text/csv" as const,
          rowCount: 1,
        };
      },
    },
    campaigns: {
      async listCampaigns() {
        return campaigns.map((campaign) => ({
          id: campaign.id,
          campaignType: campaign.campaignType,
          status: campaign.status,
          startsOn: campaign.startsOn,
          endsOn: campaign.endsOn,
          marketingLabel: campaign.marketingLabel,
          itemCount: campaign.items.length,
          overdue: campaign.overdue,
        }));
      },
      async getCampaign(id) {
        return findCampaign(campaigns, id);
      },
      // Mock fidelity only: the real report is computed in `crate::campaigns`
      // against the price log. An empty report here is the absence of a
      // verdict, not a finding that the campaign is lawful.
      async validateCampaign() {
        return { hard: [], warnings: [], anchors: [] };
      },
      async createCampaign(input) {
        const campaign = mockCampaignView(
          Math.max(0, ...campaigns.map((item) => item.id)) + 1,
          input,
          products,
        );
        campaigns.push(campaign);
        return campaign;
      },
      async updateCampaign(id, input) {
        const existing = findCampaign(campaigns, id);
        if (existing.status !== "draft") {
          throw new Error("Samo nacrt kampanje može da se izmeni.");
        }
        const updated = mockCampaignView(id, input, products);
        campaigns[campaigns.indexOf(existing)] = updated;
        return updated;
      },
      async activateCampaign(id) {
        const campaign = findCampaign(campaigns, id);
        campaign.status = "active";
        campaign.activatedAt = now;
        return campaign;
      },
      async adjustItemPrice(campaignId, productId, newPriceMinor) {
        const campaign = findCampaign(campaigns, campaignId);
        const item = campaign.items.find(
          (candidate) => candidate.productId === productId,
        );
        if (!item) {
          throw new Error("Artikal nije u kampanji.");
        }
        item.campaignPriceMinor = newPriceMinor;
        return campaign;
      },
      async endCampaign(id, overrides) {
        const campaign = findCampaign(campaigns, id);
        for (const override of overrides) {
          const product = products.find(
            (candidate) => candidate.id === override.productId,
          );
          if (product) {
            product.salePriceMinor = override.returnPriceMinor;
          }
        }
        campaign.status = "ended";
        campaign.endedAt = now;
        campaign.overdue = false;
        return campaign;
      },
      async cancelCampaign(id) {
        const campaign = findCampaign(campaigns, id);
        campaign.status = "cancelled";
        return campaign;
      },
      async exportEvidence(campaignId) {
        return {
          fileName: `dokaz-cene-kampanja-${campaignId}.html`,
          path: `mock://exports/dokaz-cene-kampanja-${campaignId}.html`,
          mimeType: "text/html" as const,
          rowCount: 0,
        };
      },
      async exportLabels(campaignId) {
        return {
          fileName: `etikete-kampanja-${campaignId}.html`,
          path: `mock://exports/etikete-kampanja-${campaignId}.html`,
          mimeType: "text/html" as const,
          rowCount: 0,
        };
      },
      async correctionReport() {
        return { rows: [] };
      },
      async exportCorrectionReport() {
        return {
          fileName: "ispravke-etiketa.html",
          path: "mock://exports/ispravke-etiketa.html",
          mimeType: "text/html" as const,
          rowCount: 0,
        };
      },
    },
    // Mock fidelity only: the real deadline engine lives in `crate::reklamacije`
    // and derives every date from the event log per-read. This stub mirrors the
    // lifecycle — create assigns id + register number and freezes the regime;
    // each transition appends an event and re-derives a plausible DeadlineState.
    reklamacije: {
      async list() {
        return reklamacije.map((view) => ({
          id: view.id,
          registerNumber: view.registerNumber,
          regime: view.regime,
          status: view.status,
          podnosilacImePrezime: view.podnosilacImePrezime,
          filedAt: view.filedAt,
          answerDue: view.deadlines.answerDue,
          resolutionDue: view.deadlines.resolutionDue,
          answerOverdue: view.deadlines.answerOverdue,
          resolutionOverdue: view.deadlines.resolutionOverdue,
          purgeEligible: view.purgeEligible,
        }));
      },
      async get(id) {
        return findReklamacija(reklamacije, id);
      },
      async create(input) {
        const view = mockReklamacijaView(
          Math.max(0, ...reklamacije.map((item) => item.id)) + 1,
          Math.max(0, ...reklamacije.map((item) => item.registerNumber)) + 1,
          input,
        );
        reklamacije.push(view);
        return view;
      },
      async logAnswer(id, input) {
        const view = findReklamacija(reklamacije, id);
        view.events.push({
          eventType: "answer_given",
          eventDate: input.eventDate,
          detailJson: null,
          consumerConsent: false,
        });
        view.status = "answered";
        recomputeMockDeadlines(view);
        return view;
      },
      async consumerReceived(id, eventDate) {
        const view = findReklamacija(reklamacije, id);
        view.events.push({
          eventType: "consumer_received_answer",
          eventDate,
          detailJson: null,
          consumerConsent: false,
        });
        view.status = "awaiting_consumer";
        recomputeMockDeadlines(view);
        return view;
      },
      async consumerResponded(id, eventDate) {
        const view = findReklamacija(reklamacije, id);
        view.events.push({
          eventType: "consumer_responded",
          eventDate,
          detailJson: null,
          consumerConsent: false,
        });
        view.status = "answered";
        recomputeMockDeadlines(view);
        return view;
      },
      async grantExtension(id, newDeadline, consumerConsent, reason, eventDate) {
        const view = findReklamacija(reklamacije, id);
        if (view.deadlines.oneExtensionUsed) {
          throw {
            code: "validation_error",
            message: "Rok je već jednom produžen.",
          };
        }
        view.events.push({
          eventType: "extension_granted",
          eventDate,
          detailJson: JSON.stringify({ newDeadline, reason }),
          consumerConsent,
        });
        recomputeMockDeadlines(view);
        return view;
      },
      async resolve(id, nacin, eventDate) {
        const view = findReklamacija(reklamacije, id);
        view.events.push({
          eventType: "resolved",
          eventDate,
          detailJson: JSON.stringify({ nacin }),
          consumerConsent: false,
        });
        view.status = "resolved";
        recomputeMockDeadlines(view);
        return view;
      },
      async exportPotvrda(id) {
        const view = findReklamacija(reklamacije, id);
        return {
          fileName: `potvrda-reklamacija-${view.registerNumber}.html`,
          path: `mock://exports/potvrda-reklamacija-${view.registerNumber}.html`,
          mimeType: "text/html" as const,
          rowCount: 1,
        };
      },
      async exportNotice() {
        return {
          fileName: "obavestenje-reklamacije.html",
          path: "mock://exports/obavestenje-reklamacije.html",
          mimeType: "text/html" as const,
          rowCount: 0,
        };
      },
    },
    // Mock fidelity only: the real ledger is derived per-read in `crate::kep`
    // from the append-only `kep_entries`. This stub keeps an in-memory list,
    // recomputes the saldo on read, and appends a razduženje per posted day.
    kep: {
      async ledger(bookYear) {
        const openingSaldoMinor = 0;
        const saldoMinor =
          openingSaldoMinor +
          kepEntries.reduce(
            (sum, entry) =>
              sum + (entry.zaduzenjeMinor ?? 0) - (entry.razduzenjeMinor ?? 0),
            0,
          );
        return { bookYear, entries: [...kepEntries], openingSaldoMinor, saldoMinor };
      },
      async postDailySales(date, overrideAmountMinor) {
        const entry: KepEntryView = {
          redniBroj: kepEntries.length + 1,
          datum: `${date.slice(8, 10)}.${date.slice(5, 7)}`,
          opis: `Dnevni promet ${date}`,
          zaduzenjeMinor: null,
          razduzenjeMinor: overrideAmountMinor ?? 234000,
          kind: "daily_sales",
        };
        kepEntries.push(entry);
        return entry;
      },
      async status() {
        return { overdueSalesDays: [], unbookedReceiptCount: 0 };
      },
      // Mock fidelity only: the real kalkulacija/nivelacija/storno surface lives
      // in `crate::kep_kalkulacija` / `crate::kep_storno`; these stubs keep the
      // KEP module's UI tests deterministic without touching the ledger.
      async listKalkulacije() {
        return [];
      },
      async exportKalkulacija(id) {
        return {
          fileName: `kalkulacija-${id}.html`,
          path: `mock://exports/kalkulacija-${id}.html`,
          mimeType: "text/html" as const,
          rowCount: 1,
        };
      },
      async nivelacija() {},
      async postAdjustment() {},
      async correctEntry() {},
      async closePreview(bookYear) {
        const already = kepClosures.some((c) => c.bookYear === bookYear);
        const saldoMinor = kepEntries.reduce(
          (sum, entry) =>
            sum + (entry.zaduzenjeMinor ?? 0) - (entry.razduzenjeMinor ?? 0),
          0,
        );
        return {
          krajnjiSaldoMinor: saldoMinor,
          entryCount: kepEntries.length,
          alreadyClosed: already,
        };
      },
      async closeYear(bookYear, confirmation) {
        if (confirmation !== "ZAKLJUČI KNJIGU") {
          throw new Error("Potvrda nije ispravna.");
        }
        if (kepClosures.some((c) => c.bookYear === bookYear)) {
          throw new Error("Godina je već zaključena.");
        }
        const saldoMinor = kepEntries.reduce(
          (sum, entry) =>
            sum + (entry.zaduzenjeMinor ?? 0) - (entry.razduzenjeMinor ?? 0),
          0,
        );
        const closure: KepClosure = {
          bookYear,
          krajnjiSaldoMinor: saldoMinor,
          entryCount: kepEntries.length,
          closedAt: "2027-01-05T09:00:00Z",
          closedBy: 1,
        };
        kepClosures.push({
          bookYear,
          krajnjiSaldoMinor: saldoMinor,
          entryCount: kepEntries.length,
          closedAt: closure.closedAt,
          purgeEligible: false,
          retentionUntil: `${bookYear + 5}-12-31`,
        });
        return closure;
      },
      async listClosures() {
        return [...kepClosures];
      },
      async exportClose(bookYear) {
        return {
          fileName: `kep-zakljucenje-${bookYear}.html`,
          path: `mock://exports/kep-zakljucenje-${bookYear}.html`,
          mimeType: "text/html" as const,
          rowCount: 1,
        };
      },
      async exportBook(bookYear) {
        return {
          fileName: `kep-knjiga-${bookYear}.html`,
          path: `mock://exports/kep-knjiga-${bookYear}.html`,
          mimeType: "text/html" as const,
          rowCount: kepEntries.length,
        };
      },
    },
    print: {
      async openForPrint() {},
      async openExternalUrl() {},
    },
  };

  return services;
}

function findCampaign(campaigns: CampaignView[], id: number): CampaignView {
  const campaign = campaigns.find((candidate) => candidate.id === id);
  if (!campaign) {
    throw new Error("Kampanja nije pronađena.");
  }

  return campaign;
}

function mockCampaignView(
  id: number,
  input: CampaignInput,
  products: ProductSummary[],
): CampaignView {
  const items: CampaignItemView[] = input.items.map((item) => {
    const product = products.find(
      (candidate) => candidate.id === item.productId,
    );

    return {
      productId: item.productId,
      productName: product?.name ?? `Artikal ${item.productId}`,
      sku: product?.sku ?? "",
      campaignPriceMinor: item.campaignPriceMinor,
      prethodnaCenaMinor:
        item.manualPrethodnaMinor ?? product?.salePriceMinor ?? null,
      anchorStatus: item.manualPrethodnaMinor == null ? "computed" : "manual",
      anchorWindowDays: item.manualPrethodnaMinor == null ? 30 : null,
      anchorTruncated: false,
      anchorReason: null,
      anchorJustification: item.anchorJustification ?? null,
      futureRegularPriceMinor: item.futureRegularPriceMinor ?? null,
      preCampaignPriceMinor: product?.salePriceMinor ?? null,
    };
  });

  return {
    id,
    campaignType: input.campaignType,
    status: "draft",
    startsOn: input.startsOn,
    endsOn: input.endsOn ?? null,
    displayMode: input.displayMode,
    headlinePercent: input.headlinePercent ?? null,
    rasprodajaGround: input.rasprodajaGround ?? null,
    specialConditions: input.specialConditions ?? null,
    reducedUtilityReason: input.reducedUtilityReason ?? null,
    marketingLabel: input.marketingLabel ?? null,
    seasonAttested: input.seasonAttested,
    separationAttested: input.separationAttested,
    activatedAt: null,
    endedAt: null,
    overdue: false,
    items,
    warnings: [],
  };
}

// The cutover the real engine reads from `crate::reklamacije::CUTOVER_DATE`.
// The regime is frozen here at create and never recomputed.
const REKLAMACIJA_CUTOVER = "2026-08-01T00:00:00Z";

function findReklamacija(
  reklamacije: ReklamacijaView[],
  id: number,
): ReklamacijaView {
  const view = reklamacije.find((candidate) => candidate.id === id);
  if (!view) {
    throw { code: "not_found", message: "Reklamacija nije pronađena." };
  }

  return view;
}

function mockReklamacijaView(
  id: number,
  registerNumber: number,
  input: ReklamacijaInput,
): ReklamacijaView {
  const view: ReklamacijaView = {
    id,
    registerNumber,
    regime:
      new Date(input.filedAt) < new Date(REKLAMACIJA_CUTOVER) ? "old" : "new",
    status: "open",
    filedAt: input.filedAt,
    podnosilacImePrezime: input.podnosilacImePrezime,
    kontakt: input.kontakt,
    podaciORobi: input.podaciORobi,
    opisNesaobraznosti: input.opisNesaobraznosti,
    zahtev: input.zahtev,
    robaKind: input.robaKind,
    datumIzdavanjaPotvrde: now,
    createdBy: 1,
    createdAt: now,
    updatedAt: now,
    events: [],
    deadlines: {
      answerDue: now,
      resolutionDue: null,
      clock: "running",
      consumerWindowDue: null,
      answerOverdue: false,
      resolutionOverdue: false,
      oneExtensionUsed: false,
    },
    purgeEligible: false,
  };
  recomputeMockDeadlines(view);
  return view;
}

// A deliberately simplified stand-in for `compute_deadlines`: the 8-day answer
// clock never pauses; the resolution clock pauses on `consumer_received_answer`
// and restarts to a fresh span on `consumer_responded`; a granted extension
// takes the consented date; `resolved` stops the clock. UI-fidelity only.
function recomputeMockDeadlines(view: ReklamacijaView): void {
  const span = view.robaKind === "opsta" ? 15 : 30;
  const answerDue = addDays(view.filedAt, 8);
  const received = view.events.find(
    (event) => event.eventType === "consumer_received_answer",
  );
  const responded = view.events.find(
    (event) => event.eventType === "consumer_responded",
  );
  const extension = view.events.find(
    (event) => event.eventType === "extension_granted",
  );
  const resolved = view.events.some((event) => event.eventType === "resolved");
  const answered = view.events.some(
    (event) => event.eventType === "answer_given",
  );

  let clock = "running";
  let resolutionDue: string | null = addDays(view.filedAt, span);
  let consumerWindowDue: string | null = null;

  if (received) {
    consumerWindowDue = addDays(received.eventDate, 3);
  }
  if (received && !responded) {
    clock = dateGt(now, consumerWindowDue as string) ? "impasse" : "paused";
    resolutionDue = null;
  } else if (received && responded) {
    // Match the Rust engine: OLD restarts to a fresh span from the response;
    // NEW resumes, preserving the base + elapsed suspension.
    resolutionDue =
      view.regime === "old"
        ? addDays(responded.eventDate, span)
        : addDays(
            addDays(view.filedAt, span),
            daysBetween(received.eventDate, responded.eventDate),
          );
  }
  if (extension?.detailJson && resolutionDue != null) {
    // Tighter-date tiebreak, as the engine does.
    const extDeadline = JSON.parse(extension.detailJson).newDeadline as string;
    resolutionDue = dateGt(resolutionDue, extDeadline) ? extDeadline : resolutionDue;
  }
  if (resolved) {
    clock = "resolved";
    resolutionDue = null;
    consumerWindowDue = null;
  }

  view.deadlines = {
    answerDue,
    resolutionDue,
    clock,
    consumerWindowDue,
    answerOverdue: !resolved && !answered && dateGt(now, answerDue),
    resolutionOverdue: resolutionDue != null && dateGt(now, resolutionDue),
    oneExtensionUsed: extension != null,
  };
}

function addDays(iso: string, days: number): string {
  const date = new Date(iso);
  date.setUTCDate(date.getUTCDate() + days);
  return date.toISOString().replace(/\.\d{3}Z$/, "Z");
}

function dateGt(a: string, b: string): boolean {
  return new Date(a).getTime() > new Date(b).getTime();
}

function daysBetween(from: string, to: string): number {
  const ms = new Date(to).getTime() - new Date(from).getTime();
  return Math.round(ms / 86_400_000);
}

function mockBarcodeLookup(barcode: string): ProductLookupSuggestion | null {
  if (barcode !== "4008400320328") {
    return null;
  }

  return {
    barcode,
    source: {
      provider: "open_food_facts",
      label: "Open Food Facts",
      barcode,
      fetchedAt: now,
      acceptedFields: ["name", "brand", "packageSize"],
    },
    fields: {
      name: "Kinder Bueno",
      brand: "Kinder",
      packageSize: "43 g",
      imageUrl:
        "https://images.openfoodfacts.org/images/products/400/840/032/0328/front_en.3.400.jpg",
      categoryName: "Chocolate bars",
    },
  };
}

function createBackupJob(
  backupType: BackupJob["backupType"],
  path: string,
): BackupJob {
  return {
    id: Date.now(),
    backupType,
    path,
    status: "completed",
    errorMessage: null,
    fileSizeBytes: 131072,
    createdAt: now,
    completedAt: now,
  };
}

function parseMockCsv(csvText: string) {
  const [headerLine = "", ...dataLines] = csvText
    .trim()
    .split(/\r?\n/)
    .filter(Boolean);
  const delimiter = headerLine.includes(";") ? ";" : ",";
  const headers = headerLine.split(delimiter).map((value) => value.trim());
  const rows = dataLines.map((line) =>
    line.split(delimiter).map((value) => value.trim()),
  );

  return { delimiter, headers, rows };
}

function rowToValues(headers: string[], row: string[]) {
  return Object.fromEntries(
    headers.map((header, index) => [header, row[index] ?? ""]),
  );
}

function filterProducts(
  products: ProductSummary[],
  query: ProductListQuery = {},
) {
  const normalized = query.search?.trim().toLowerCase() ?? "";

  return products.filter((product) => {
    const matchesSearch =
      !normalized ||
      [product.name, product.sku, product.barcode ?? ""].some((value) =>
        value.toLowerCase().includes(normalized),
      );
    const matchesCategory =
      query.categoryId === undefined || product.categoryId === query.categoryId;
    const matchesActive =
      query.active === undefined || product.active === query.active;
    const matchesLowStock =
      !query.lowStock ||
      product.currentStockMilli <= product.minimumStockMilli;
    const matchesMissingBarcode = !query.missingBarcode || !product.barcode;
    const matchesTaxRate =
      query.taxRateId === undefined || product.taxRateId === query.taxRateId;

    return (
      matchesSearch &&
      matchesCategory &&
      matchesActive &&
      matchesLowStock &&
      matchesMissingBarcode &&
      matchesTaxRate
    );
  });
}

function rejectDuplicateProduct(
  products: ProductSummary[],
  sku: string,
  barcode: string | null,
  currentProductId?: number,
) {
  if (
    products.some(
      (product) => product.id !== currentProductId && product.sku === sku,
    )
  ) {
    throw { code: "duplicate_sku", message: "SKU/šifra već postoji." };
  }

  if (
    barcode &&
    products.some(
      (product) =>
        product.id !== currentProductId && product.barcode === barcode,
    )
  ) {
    throw { code: "duplicate_barcode", message: "Barcode već postoji." };
  }
}

function findTaxRate(taxRates: TaxRateSummary[], taxRateId: number) {
  const taxRate = taxRates.find((item) => item.id === taxRateId && item.active);

  if (!taxRate) {
    throw { code: "validation_error", message: "Izaberite važeću PDV stopu." };
  }

  return taxRate;
}

function toStockItem(product: ProductSummary): StockListItem {
  return {
    productId: product.id,
    productName: product.name,
    sku: product.sku,
    barcode: product.barcode,
    categoryId: product.categoryId,
    categoryName: product.categoryName,
    unitOfMeasure: product.unitOfMeasure,
    currentQuantityMilli: product.currentStockMilli,
    minimumStockMilli: product.minimumStockMilli,
    lowStock: product.currentStockMilli <= product.minimumStockMilli,
    salePriceMinor: product.salePriceMinor,
    lastMovementAt: ledgerLatest(product.id),
  };
}

function ledgerLatest(_productId: number): string | null {
  return null;
}

function createSalePreview(
  products: ProductSummary[],
  request: SaleDraftRequest,
): SalePreview {
  const items = request.items.map((item) => {
    const product = products.find((candidate) => candidate.id === item.productId);

    if (!product) {
      throw { code: "not_found", message: "Artikal nije pronađen." };
    }

    const grossMinor = Math.round(
      (product.salePriceMinor * item.quantityMilli) / 1000,
    );
    const discountMinor = discountAmount(grossMinor, item.discount);
    const totalMinor = grossMinor - discountMinor;

    return {
      productId: product.id,
      productName: product.name,
      productSku: product.sku,
      productBarcode: product.barcode,
      quantityMilli: item.quantityMilli,
      quantityLabel: `${item.quantityMilli / 1000} ${product.unitOfMeasure}`,
      unitPriceMinor: product.salePriceMinor,
      discountMinor,
      taxRateBasisPoints: product.taxRateBasisPoints,
      taxMinor: includedTax(totalMinor, product.taxRateBasisPoints),
      totalMinor,
    };
  });
  const subtotalMinor = items.reduce(
    (sum, item) =>
      sum + Math.round((item.unitPriceMinor * item.quantityMilli) / 1000),
    0,
  );
  const itemDiscountMinor = items.reduce(
    (sum, item) => sum + item.discountMinor,
    0,
  );
  const receiptDiscountMinor = discountAmount(
    items.reduce((sum, item) => sum + item.totalMinor, 0),
    request.receiptDiscount,
  );
  const totalMinor =
    items.reduce((sum, item) => sum + item.totalMinor, 0) - receiptDiscountMinor;

  return {
    items,
    subtotalMinor,
    discountMinor: itemDiscountMinor + receiptDiscountMinor,
    taxMinor: includedTax(totalMinor, 2000),
    totalMinor,
  };
}

function discountAmount(
  totalMinor: number,
  discount: SaleDraftRequest["receiptDiscount"],
) {
  if (!discount) {
    return 0;
  }

  if (discount.type === "amount") {
    return discount.amountMinor;
  }

  return Math.round((totalMinor * discount.basisPoints) / 10000);
}

function includedTax(totalMinor: number, basisPoints: number) {
  if (basisPoints === 0) {
    return 0;
  }

  return Math.round((totalMinor * basisPoints) / (10000 + basisPoints));
}

function createReceiptDetail(): ReceiptDetail {
  return {
    id: 1,
    receiptNumber: "R-2026-0001",
    createdAt: now,
    cashierName: "Mina Kasir",
    shiftId: 1,
    status: "completed",
    fiscalStatus: "not_fiscalized",
    documentType: "sale",
    totalMinor: 100000,
    linkedDocumentCount: 0,
    paymentMethods: ["cash"],
    subtotalMinor: 100000,
    discountMinor: 0,
    taxMinor: 16667,
    items: [
      {
        id: 1,
        productId: 2,
        productName: "Kafa 200 g",
        productSku: "KAFA-200",
        productBarcode: "8600000000027",
        quantityMilli: 2000,
        returnedQuantityMilli: 0,
        unitPriceMinor: 50000,
        discountMinor: 0,
        taxRateBasisPoints: 2000,
        taxMinor: 16667,
        totalMinor: 100000,
      },
    ],
    payments: [
      {
        id: 1,
        paymentMethod: "cash",
        amountMinor: 100000,
        createdAt: now,
      },
    ],
    linkedDocuments: [],
    canVoid: true,
    canReturn: true,
  };
}
