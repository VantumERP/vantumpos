import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { RegisterScreen } from "@/app/register/RegisterScreen";
import type { PosServices } from "@/services/ports";
import type {
  AmlAssessment,
  CompleteSaleRequest,
  CompletedSale,
  PravnaForma,
  ProductSearchQuery,
  ProductSummary,
  SaleDraftRequest,
  SalePreview,
} from "@/services/types";

const product: ProductSummary = {
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
  taxRateBasisPoints: 2000,
  minimumStockMilli: 5000,
  currentStockMilli: 3000,
  allowNegativeStock: false,
  active: true,
  perishable: false,
  perishableJustification: null,
  externalSource: null,
};

function createRegisterServices(
  completeSale = vi.fn(createCompletedSale),
): PosServices {
  return {
    settings: {
      getHealth: async () => ({
        backend: "local",
        appVersion: "test",
        databasePath: "mock://vantumpos.sqlite3",
        migrated: true,
      }),
    },
    catalog: {
      listProducts: async () => ({ items: [product] }),
      searchProducts: async ({ search }: ProductSearchQuery) => ({
        items: [product].filter((item) =>
          [item.name, item.sku, item.barcode ?? ""].some((value) =>
            value.toLowerCase().includes((search ?? "").toLowerCase()),
          ),
        ),
      }),
      getProduct: async () => product,
      createProduct: async () => product,
      updateProduct: async () => product,
      setProductActive: async () => product,
      lookupProductByBarcode: async () => null,
      listCategories: async () => [
        { id: 1, name: "Mlecni proizvodi", active: true },
      ],
      saveCategory: async () => ({
        id: 1,
        name: "Mlecni proizvodi",
        active: true,
      }),
    },
    sales: {
      createSalePreview: async (request: SaleDraftRequest) =>
        createSalePreview(request),
      completeSale,
    },
  } as unknown as PosServices;
}

function createSalePreview(
  request: SaleDraftRequest,
  priced: ProductSummary = product,
): SalePreview {
  const items = request.items.map((item) => {
    const lineSubtotalMinor = Math.round(
      (priced.salePriceMinor * item.quantityMilli) / 1000,
    );
    const discountMinor =
      item.discount?.type === "amount"
        ? item.discount.amountMinor
        : item.discount?.type === "percent"
          ? Math.round((lineSubtotalMinor * item.discount.basisPoints) / 10000)
          : 0;
    const totalMinor = lineSubtotalMinor - discountMinor;

    return {
      productId: priced.id,
      productName: priced.name,
      productSku: priced.sku,
      productBarcode: priced.barcode,
      quantityMilli: item.quantityMilli,
      quantityLabel: `${item.quantityMilli / 1000} kom`,
      unitPriceMinor: priced.salePriceMinor,
      discountMinor,
      taxRateBasisPoints: priced.taxRateBasisPoints,
      taxMinor: Math.round((totalMinor * 2000) / 12000),
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
  const totalBeforeReceiptDiscount = items.reduce(
    (sum, item) => sum + item.totalMinor,
    0,
  );
  const receiptDiscountMinor =
    request.receiptDiscount?.type === "amount"
      ? request.receiptDiscount.amountMinor
      : request.receiptDiscount?.type === "percent"
        ? Math.round(
            (totalBeforeReceiptDiscount * request.receiptDiscount.basisPoints) /
              10000,
          )
        : 0;
  const totalMinor = totalBeforeReceiptDiscount - receiptDiscountMinor;

  return {
    items,
    subtotalMinor,
    discountMinor: itemDiscountMinor + receiptDiscountMinor,
    taxMinor: Math.round((totalMinor * 2000) / 12000),
    totalMinor,
  };
}

async function createCompletedSale(
  request: CompleteSaleRequest,
): Promise<CompletedSale> {
  const preview = createSalePreview(request);
  const cashReceivedMinor =
    request.payments.find((payment) => payment.method === "cash")?.amountMinor ??
    0;

  return {
    id: 1,
    localReceiptNumber: "VP-000001",
    createdAt: "2026-06-18T10:00:00Z",
    cashierName: "Kasir",
    fiscalStatus: "not_fiscalized",
    ...preview,
    payments: [{ method: "cash", amountMinor: preview.totalMinor }],
    cashReceivedMinor,
    changeDueMinor: Math.max(cashReceivedMinor - preview.totalMinor, 0),
  };
}

/**
 * The AML fixtures below stand in for the Rust `sales_assess_cash_payment`
 * command. The copy is verbatim from `src-tauri/src/legal.rs::aml_cash_cap` —
 * the till must render whatever the backend hands it, and the operator must
 * see the preduzetnik tier, never "privredni prestup".
 */
const amlSummary =
  "Zabranjeno je primiti gotovinu u iznosu od 10.000 evra ili više u dinarskoj protivvrednosti. Iznos se mora uplatiti na tekući račun.";
const amlPreduzetnikPenalty =
  "Prekršaj: novčana kazna od 100.000 do 300.000 dinara (čl. 120 st. 2), a u srazmeri s vrednošću robe i više (čl. 120 st. 8).";
const amlCitation =
  "Zakon o sprečavanju pranja novca i finansiranja terorizma, čl. 46 st. 1. Nadzor: tržišna inspekcija (čl. 110 st. 6).";

/**
 * The till debounces the assessment call, so the warning lands a `setTimeout`
 * after the last keystroke. Under full vitest parallelism this worker can be
 * starved for seconds at a time, and the default 1s findBy budget is
 * wall-clock, so it expires while the timer is still queued. These budgets are
 * slack for a starved box, not an expectation — the warning renders in well
 * under a second when the machine is idle.
 */
const AML_WAIT_MS = 5_000;
const AML_TEST_TIMEOUT_MS = 20_000;

/** 10.000,00 RSD, so a rate of 100 para/EUR puts one unit exactly on the cap. */
const capPricedProduct: ProductSummary = {
  ...product,
  id: 2,
  name: "Zlatni set",
  sku: "ZLATO-1",
  barcode: "8600000000027",
  salePriceMinor: 1_000_000,
};

function todayIso(): string {
  const now = new Date();
  const month = `${now.getMonth() + 1}`.padStart(2, "0");
  const day = `${now.getDate()}`.padStart(2, "0");

  return `${now.getFullYear()}-${month}-${day}`;
}

interface AmlServiceOptions {
  /** `null` stands for "no rate cached" — the check cannot run. */
  eurRateMinor?: number | null;
  rateDate?: string;
  pravnaForma?: PravnaForma | null;
  completeSale?: Parameters<typeof createRegisterServices>[0];
}

function createAmlServices(options: AmlServiceOptions = {}): PosServices {
  const {
    eurRateMinor = 100,
    rateDate = todayIso(),
    pravnaForma = "preduzetnik",
    completeSale = vi.fn(createCompletedSale),
  } = options;
  const services = createRegisterServices(completeSale);

  services.catalog.searchProducts = async () => ({
    items: [capPricedProduct],
    categories: [],
    taxRates: [],
    total: 1,
  });
  services.sales.createSalePreview = async (request: SaleDraftRequest) =>
    createSalePreview(request, capPricedProduct);
  services.sales.assessCashPayment = async (
    cashMinor: number,
  ): Promise<AmlAssessment> => {
    const notice = {
      summary: amlSummary,
      penalty: pravnaForma === "preduzetnik" ? amlPreduzetnikPenalty : null,
      citation: amlCitation,
      isLegalDuty: true,
    };

    if (eurRateMinor === null) {
      return {
        cashMinor,
        thresholdMinor: 0,
        breached: false,
        nearThreshold: false,
        rateUnavailable: true,
        rate: null,
        notice,
      };
    }

    // Mirrors `aml::assess_cash_payment` exactly: inclusive `>=`, 80% soft line.
    const thresholdMinor = 10_000 * eurRateMinor;
    const softMinor = Math.floor((thresholdMinor * 80) / 100);

    return {
      cashMinor,
      thresholdMinor,
      breached: cashMinor >= thresholdMinor,
      nearThreshold: cashMinor >= softMinor && cashMinor < thresholdMinor,
      rateUnavailable: false,
      rate: { rateMinor: eurRateMinor, rateDate, source: "nbs" },
      notice,
    };
  };

  return services;
}

async function addCapPricedItem(user: ReturnType<typeof userEvent.setup>) {
  await user.type(
    screen.getByRole("searchbox", {
      name: "Skeniraj barkod ili pretraži artikal",
    }),
    "8600000000027{enter}",
  );

  expect(await screen.findByText("Zlatni set")).toBeInTheDocument();
}

async function tenderCash(
  user: ReturnType<typeof userEvent.setup>,
  amount: string,
) {
  const cashField = screen.getByLabelText(/gotovina primljeno/i);
  await user.clear(cashField);
  await user.type(cashField, amount);
}

async function addProductToCart(user: ReturnType<typeof userEvent.setup>) {
  render(<RegisterScreen services={createRegisterServices()} />);

  await user.type(
    screen.getByRole("searchbox", {
      name: "Skeniraj barkod ili pretraži artikal",
    }),
    "8600000000010{enter}",
  );

  expect(await screen.findByText("Mleko 1 l")).toBeInTheDocument();
}

describe("RegisterScreen", () => {
  it("adds a scanned product to the cart and shows sale preview", async () => {
    const user = userEvent.setup();

    await addProductToCart(user);

    expect(screen.getByText("MLEKO-1L")).toBeInTheDocument();
    expect(screen.getAllByText("159,99 RSD").length).toBeGreaterThan(0);
  });

  it("updates preview when quantity and item discount change", async () => {
    const user = userEvent.setup();

    await addProductToCart(user);
    await user.clear(screen.getByLabelText("Količina za Mleko 1 l"));
    await user.type(screen.getByLabelText("Količina za Mleko 1 l"), "2");
    await user.clear(screen.getByLabelText("Popust za Mleko 1 l"));
    await user.type(screen.getByLabelText("Popust za Mleko 1 l"), "10");

    expect((await screen.findAllByText("309,98 RSD")).length).toBeGreaterThan(0);
  });

  it("applies a percent line discount typed as 20%", async () => {
    const user = userEvent.setup();

    await addProductToCart(user);
    await user.clear(screen.getByLabelText("Popust za Mleko 1 l"));
    await user.type(screen.getByLabelText("Popust za Mleko 1 l"), "20%");

    // 15999 - round(15999 * 2000 / 10000 = 3200) = 12799 -> "127,99 RSD"
    expect((await screen.findAllByText("127,99 RSD")).length).toBeGreaterThan(0);
  });

  it("prefills the cash tender field to the preview total", async () => {
    const user = userEvent.setup();

    await addProductToCart(user);

    await waitFor(() =>
      expect(screen.getByLabelText("Gotovina primljeno")).toHaveValue("159.99"),
    );
  });

  it("prefills only the cash still due after a card amount (no phantom change)", async () => {
    const user = userEvent.setup();

    await addProductToCart(user); // total 159.99, cash prefills to "159.99"
    const cashField = screen.getByLabelText("Gotovina primljeno");
    await waitFor(() => expect(cashField).toHaveValue("159.99"));

    // Paying the whole amount by card should drop the prefilled cash to 0,
    // so the change ("Kusur") stays 0 instead of the full total.
    await user.type(screen.getByLabelText("Kartica"), "159.99");

    await waitFor(() => expect(cashField).toHaveValue("0.00"));
  });

  it("shows cash change from the current preview total", async () => {
    const user = userEvent.setup();

    await addProductToCart(user);
    const cashField = screen.getByLabelText("Gotovina primljeno");
    await waitFor(() => expect(cashField).toHaveValue("159.99"));
    await user.clear(cashField);
    await user.type(cashField, "200");

    expect(await screen.findByText("Kusur")).toBeInTheDocument();
    expect(screen.getByText("40,01 RSD")).toBeInTheDocument();
  });

  it("completes the sale and opens local receipt preview", async () => {
    const user = userEvent.setup();

    await addProductToCart(user);
    const cashField = screen.getByLabelText("Gotovina primljeno");
    await waitFor(() => expect(cashField).toHaveValue("159.99"));
    await user.clear(cashField);
    await user.type(cashField, "160");
    await user.click(screen.getByRole("button", { name: "Završi prodaju" }));

    const dialog = await screen.findByRole("dialog", {
      name: "Račun VP-000001",
    });
    expect(
      within(dialog).getByText("Interni pregled prodaje — nije fiskalni račun"),
    ).toBeInTheDocument();
    expect(
      within(dialog).getAllByText("OVO NIJE FISKALNI RAČUN").length,
    ).toBeGreaterThanOrEqual(2);
  });

  it("shows the non-fiscal banner in the completed-sale dialog", async () => {
    const user = userEvent.setup();

    await addProductToCart(user);
    const cashField = screen.getByLabelText("Gotovina primljeno");
    await waitFor(() => expect(cashField).toHaveValue("159.99"));
    await user.clear(cashField);
    await user.type(cashField, "160");
    await user.click(screen.getByRole("button", { name: "Završi prodaju" }));

    await screen.findByRole("dialog", { name: "Račun VP-000001" });
    const banners = await screen.findAllByText("OVO NIJE FISKALNI RAČUN");
    expect(banners.length).toBeGreaterThanOrEqual(2); // top and bottom
  });

  it("keeps the cart intact when backend completion fails", async () => {
    const user = userEvent.setup();
    const completeSale = vi.fn().mockRejectedValue({
      code: "payment_mismatch",
      message: "Plaćanja se ne poklapaju.",
    });

    render(<RegisterScreen services={createRegisterServices(completeSale)} />);
    await user.type(
      screen.getByRole("searchbox", {
        name: "Skeniraj barkod ili pretraži artikal",
      }),
      "Mleko{enter}",
    );
    await user.clear(screen.getByLabelText("Gotovina primljeno"));
    await user.type(screen.getByLabelText("Gotovina primljeno"), "160");
    await user.click(screen.getByRole("button", { name: "Završi prodaju" }));

    expect(await screen.findByText("Plaćanja se ne poklapaju.")).toBeInTheDocument();
    expect(screen.getByText("Mleko 1 l")).toBeInTheDocument();
  });

  it("offers an oversell confirm and retries with allowStockOverride", async () => {
    const user = userEvent.setup();
    const completeSale = vi.fn(createCompletedSale);
    completeSale.mockRejectedValueOnce({
      code: "insufficient_stock",
      message: "Nema dovoljno zaliha.",
    });

    render(<RegisterScreen services={createRegisterServices(completeSale)} />);
    await user.type(
      screen.getByRole("searchbox", {
        name: "Skeniraj barkod ili pretraži artikal",
      }),
      "8600000000010{enter}",
    );
    expect(await screen.findByText("Mleko 1 l")).toBeInTheDocument();
    const cashField = screen.getByLabelText("Gotovina primljeno");
    await waitFor(() => expect(cashField).toHaveValue("159.99"));
    await user.click(screen.getByRole("button", { name: "Završi prodaju" }));

    await user.click(await screen.findByRole("button", { name: "Ipak prodaj" }));

    await waitFor(() =>
      expect(completeSale).toHaveBeenLastCalledWith(
        expect.objectContaining({ allowStockOverride: true }),
      ),
    );
  });

  it("renders the swallowed preview error and keeps the cart", async () => {
    const user = userEvent.setup();
    const services = createRegisterServices();
    services.sales.createSalePreview = vi.fn().mockRejectedValue({
      code: "validation_error",
      message: "Popust ne može biti veći od iznosa.",
    });

    render(<RegisterScreen services={services} />);
    await user.type(
      screen.getByRole("searchbox", {
        name: "Skeniraj barkod ili pretraži artikal",
      }),
      "Mleko{enter}",
    );

    expect(
      await screen.findByText("Popust ne može biti veći od iznosa."),
    ).toBeInTheDocument();
    expect(screen.getByText("Mleko 1 l")).toBeInTheDocument();
  });

  it("does not render a preview error when the preview succeeds", async () => {
    const user = userEvent.setup();

    await addProductToCart(user);

    expect(
      screen.queryByText("Pregled računa nije moguć"),
    ).not.toBeInTheDocument();
  });

  it("shows a picker for multiple matches and adds the chosen product", async () => {
    const user = userEvent.setup();
    const services = createRegisterServices();
    const productA = { ...product, id: 10, name: "Kosulja plava", sku: "KOS-P", barcode: "111" };
    const productB = { ...product, id: 11, name: "Kosulja bela", sku: "KOS-B", barcode: "222" };
    services.catalog.searchProducts = async () => ({
      items: [productA, productB],
      categories: [],
      taxRates: [],
      total: 2,
    });
    render(<RegisterScreen services={services} />);

    const box = screen.getByRole("searchbox");
    await user.type(box, "kosulja{enter}");

    await user.click(await screen.findByRole("button", { name: /Kosulja bela/ }));
    expect(await screen.findByText("Kosulja bela")).toBeInTheDocument();
  });

  it("adds the exact barcode match instead of the first result", async () => {
    const user = userEvent.setup();
    const services = createRegisterServices();
    const productA = { ...product, id: 10, name: "Kosulja plava", sku: "KOS-P", barcode: "1112223334445" };
    const productB = { ...product, id: 11, name: "Kosulja bela", sku: "KOS-B", barcode: "2223334445556" };
    services.catalog.searchProducts = async () => ({
      items: [productA, productB],
      categories: [],
      taxRates: [],
      total: 2,
    });
    render(<RegisterScreen services={services} />);

    const box = screen.getByRole("searchbox");
    await user.type(box, "2223334445556{enter}");

    expect(await screen.findByText("Kosulja bela")).toBeInTheDocument();
    expect(screen.queryByText("Kosulja plava")).toBeNull();
  });

  it("nudges ESIR issuance and saves the entered fiscal number", async () => {
    const user = userEvent.setup();
    const setEsirNumber = vi.fn().mockResolvedValue({});
    const services = createRegisterServices();
    (
      services as unknown as {
        receipts: { setEsirNumber: typeof setEsirNumber };
      }
    ).receipts = { setEsirNumber };

    render(<RegisterScreen services={services} />);
    await user.type(
      screen.getByRole("searchbox", {
        name: "Skeniraj barkod ili pretraži artikal",
      }),
      "8600000000010{enter}",
    );
    expect(await screen.findByText("Mleko 1 l")).toBeInTheDocument();
    const cashField = screen.getByLabelText("Gotovina primljeno");
    await waitFor(() => expect(cashField).toHaveValue("159.99"));
    await user.clear(cashField);
    await user.type(cashField, "160");
    await user.click(screen.getByRole("button", { name: "Završi prodaju" }));

    await screen.findByRole("dialog", { name: "Račun VP-000001" });
    expect(
      screen.getByText(/Izdajte fiskalni račun na ESIR-u/i),
    ).toBeInTheDocument();
    await user.type(
      screen.getByLabelText(/Broj fiskalnog računa/i),
      "ФБ12-1",
    );
    await user.click(screen.getByRole("button", { name: /Sačuvaj broj/i }));
    expect(setEsirNumber).toHaveBeenCalledWith(expect.any(Number), "ФБ12-1");
  });

  it("warns at exactly the cap and requires a reason before completing", async () => {
    const user = userEvent.setup();
    // rate 100 para/EUR -> threshold 1.000.000 para
    const completeSale = vi.fn(createCompletedSale);
    const services = createAmlServices({
      eurRateMinor: 100,
      pravnaForma: "preduzetnik",
      completeSale,
    });

    render(<RegisterScreen services={services} />);
    await addCapPricedItem(user);
    await tenderCash(user, "10000");

    expect(
      await screen.findByText(/10\.000 evra ili više/i, undefined, {
        timeout: AML_WAIT_MS,
      }),
    ).toBeInTheDocument();
    expect(screen.getByText(/100\.000 do 300\.000/)).toBeInTheDocument();
    expect(screen.queryByText(/privredni prestup/i)).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Završi prodaju" }));

    expect(
      screen.getByText(/unesite razlog/i),
      "the soft block asks for a reason, it does not forbid the sale",
    ).toBeInTheDocument();
    expect(completeSale).not.toHaveBeenCalled();
  }, AML_TEST_TIMEOUT_MS);

  it("completes the breached sale once a reason is entered and records it", async () => {
    const user = userEvent.setup();
    const completeSale = vi.fn(createCompletedSale);
    const services = createAmlServices({ eurRateMinor: 100, completeSale });

    render(<RegisterScreen services={services} />);
    await addCapPricedItem(user);
    await tenderCash(user, "10000");
    await screen.findByText(/10\.000 evra ili više/i, undefined, {
      timeout: AML_WAIT_MS,
    });

    await user.type(
      screen.getByLabelText(/razlog prijema gotovine/i),
      "Kupac je odbio prenos na račun.",
    );
    await user.click(screen.getByRole("button", { name: "Završi prodaju" }));

    await waitFor(() =>
      expect(completeSale).toHaveBeenCalledWith(
        expect.objectContaining({
          amlAckReason: "Kupac je odbio prenos na račun.",
        }),
      ),
    );
  }, AML_TEST_TIMEOUT_MS);

  it("shows a staleness warning when the rate is not today's", async () => {
    const user = userEvent.setup();
    const services = createAmlServices({
      eurRateMinor: 100,
      rateDate: "2026-07-01",
    });

    render(<RegisterScreen services={services} />);
    await addCapPricedItem(user);
    await tenderCash(user, "10000");

    expect(
      await screen.findByText(/kurs od 01\.07\.2026/i, undefined, {
        timeout: AML_WAIT_MS,
      }),
    ).toBeInTheDocument();
  }, AML_TEST_TIMEOUT_MS);

  it("says the check could not run when no rate is cached, and still sells", async () => {
    const user = userEvent.setup();
    const completeSale = vi.fn(createCompletedSale);
    const services = createAmlServices({ eurRateMinor: null, completeSale });

    render(<RegisterScreen services={services} />);
    await addCapPricedItem(user);
    await tenderCash(user, "10000");

    expect(
      await screen.findByText(/provera nije mogla da se izvrši/i, undefined, {
        timeout: AML_WAIT_MS,
      }),
    ).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Završi prodaju" }));

    await waitFor(() => expect(completeSale).toHaveBeenCalled());
  }, AML_TEST_TIMEOUT_MS);

  it("offers the bank-transfer tender as the lawful alternative", async () => {
    const services = createAmlServices({ eurRateMinor: 100 });

    render(<RegisterScreen services={services} />);

    expect(
      screen.getByLabelText(/uplata na tekući račun/i),
    ).toBeInTheDocument();
  }, AML_TEST_TIMEOUT_MS);

  it("sends the bank-transfer amount as its own payment line", async () => {
    const user = userEvent.setup();
    const completeSale = vi.fn(createCompletedSale);
    const services = createAmlServices({ eurRateMinor: 100, completeSale });

    render(<RegisterScreen services={services} />);
    await addCapPricedItem(user);
    await user.type(screen.getByLabelText(/uplata na tekući račun/i), "10000");
    await waitFor(() =>
      expect(screen.getByLabelText(/gotovina primljeno/i)).toHaveValue("0.00"),
    );

    await user.click(screen.getByRole("button", { name: "Završi prodaju" }));

    await waitFor(() =>
      expect(completeSale).toHaveBeenCalledWith(
        expect.objectContaining({
          payments: [{ method: "bank_transfer", amountMinor: 1_000_000 }],
        }),
      ),
    );
  }, AML_TEST_TIMEOUT_MS);

  it("reports when nothing matches", async () => {
    const user = userEvent.setup();
    const services = createRegisterServices();
    services.catalog.searchProducts = async () => ({
      items: [],
      categories: [],
      taxRates: [],
      total: 0,
    });
    render(<RegisterScreen services={services} />);

    const box = screen.getByRole("searchbox");
    await user.type(box, "nepostojece{enter}");

    expect(await screen.findByText("Artikal nije pronađen.")).toBeInTheDocument();
  });
});
