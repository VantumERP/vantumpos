import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { RegisterScreen } from "@/app/register/RegisterScreen";
import type { PosServices } from "@/services/ports";
import type {
  CompleteSaleRequest,
  CompletedSale,
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

function createSalePreview(request: SaleDraftRequest): SalePreview {
  const items = request.items.map((item) => {
    const lineSubtotalMinor = Math.round(
      (product.salePriceMinor * item.quantityMilli) / 1000,
    );
    const discountMinor =
      item.discount?.type === "amount" ? item.discount.amountMinor : 0;
    const totalMinor = lineSubtotalMinor - discountMinor;

    return {
      productId: product.id,
      productName: product.name,
      productSku: product.sku,
      productBarcode: product.barcode,
      quantityMilli: item.quantityMilli,
      quantityLabel: `${item.quantityMilli / 1000} kom`,
      unitPriceMinor: product.salePriceMinor,
      discountMinor,
      taxRateBasisPoints: product.taxRateBasisPoints,
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
  const receiptDiscountMinor =
    request.receiptDiscount?.type === "amount"
      ? request.receiptDiscount.amountMinor
      : 0;
  const totalBeforeReceiptDiscount = items.reduce(
    (sum, item) => sum + item.totalMinor,
    0,
  );
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

async function addProductToCart(user: ReturnType<typeof userEvent.setup>) {
  render(<RegisterScreen services={createRegisterServices()} />);

  await user.type(
    screen.getByRole("searchbox", {
      name: "Skeniraj barkod ili pretrazi artikal",
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
    await user.clear(screen.getByLabelText("Kolicina za Mleko 1 l"));
    await user.type(screen.getByLabelText("Kolicina za Mleko 1 l"), "2");
    await user.clear(screen.getByLabelText("Popust za Mleko 1 l"));
    await user.type(screen.getByLabelText("Popust za Mleko 1 l"), "10");

    expect((await screen.findAllByText("309,98 RSD")).length).toBeGreaterThan(0);
  });

  it("shows cash change from the current preview total", async () => {
    const user = userEvent.setup();

    await addProductToCart(user);
    await user.type(screen.getByLabelText("Gotovina primljeno"), "200");

    expect(await screen.findByText("Kusur")).toBeInTheDocument();
    expect(screen.getByText("40,01 RSD")).toBeInTheDocument();
  });

  it("completes the sale and opens local receipt preview", async () => {
    const user = userEvent.setup();

    await addProductToCart(user);
    await user.type(screen.getByLabelText("Gotovina primljeno"), "160");
    await user.click(screen.getByRole("button", { name: "Zavrsi prodaju" }));

    const dialog = await screen.findByRole("dialog", {
      name: "Racun VP-000001",
    });
    expect(within(dialog).getByText("Lokalni racun")).toBeInTheDocument();
    expect(screen.queryByText(/fiskal/i)).not.toBeInTheDocument();
  });

  it("keeps the cart intact when backend completion fails", async () => {
    const user = userEvent.setup();
    const completeSale = vi.fn().mockRejectedValue({
      code: "insufficient_stock",
      message: "Nema dovoljno zaliha.",
    });

    render(<RegisterScreen services={createRegisterServices(completeSale)} />);
    await user.type(
      screen.getByRole("searchbox", {
        name: "Skeniraj barkod ili pretrazi artikal",
      }),
      "Mleko{enter}",
    );
    await user.type(screen.getByLabelText("Gotovina primljeno"), "160");
    await user.click(screen.getByRole("button", { name: "Zavrsi prodaju" }));

    expect(await screen.findByText("Nema dovoljno zaliha.")).toBeInTheDocument();
    expect(screen.getByText("Mleko 1 l")).toBeInTheDocument();
  });
});
