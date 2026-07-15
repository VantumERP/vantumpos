import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { ReceiptsScreen } from "./ReceiptsScreen";
import { createMockServices } from "@/services/mock-adapter";
import type { ReceiptsService } from "@/services/ports";
import type {
  ReceiptDetail,
  ReceiptSearchResult,
  ReceiptSummary,
} from "@/services/types";

describe("ReceiptsScreen", () => {
  it("threads the session user id into the void call", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const voidReceipt = vi.spyOn(services.receipts, "voidReceipt");

    render(<ReceiptsScreen receipts={services.receipts} userId={9} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );
    await user.click(await screen.findByRole("button", { name: "Storniraj račun" }));
    await user.type(screen.getByLabelText("Razlog"), "Greska kasira");
    await user.click(screen.getByRole("button", { name: "Potvrdi storniranje" }));

    await waitFor(() =>
      expect(voidReceipt).toHaveBeenCalledWith({
        receiptId: 1,
        userId: 9,
        reason: "Greska kasira",
      }),
    );
  });

  it("threads the session user id into the return call", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const returnItems = vi.spyOn(services.receipts, "returnItems");

    render(<ReceiptsScreen receipts={services.receipts} userId={9} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );
    await user.click(await screen.findByRole("button", { name: "Povrat artikala" }));

    const quantity = await screen.findByLabelText("Količina za Kafa 200 g");
    await user.clear(quantity);
    await user.type(quantity, "1");
    await user.type(screen.getByLabelText("Razlog povrata"), "Ostecen artikal");
    await user.click(screen.getByRole("button", { name: "Sačuvaj povrat" }));

    await waitFor(() =>
      expect(returnItems).toHaveBeenCalledWith(
        expect.objectContaining({
          receiptId: 1,
          userId: 9,
          items: [{ saleItemId: 1, quantityMilli: 1000 }],
        }),
      ),
    );
  });

  it("defaults the refund tender to cash and forwards it", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const returnItems = vi.spyOn(services.receipts, "returnItems");

    render(<ReceiptsScreen receipts={services.receipts} userId={9} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );
    await user.click(await screen.findByRole("button", { name: "Povrat artikala" }));

    expect(await screen.findByLabelText("Način povrata")).toHaveValue("cash");

    const quantity = await screen.findByLabelText("Količina za Kafa 200 g");
    await user.clear(quantity);
    await user.type(quantity, "1");
    await user.type(screen.getByLabelText("Razlog povrata"), "Ostecen artikal");
    await user.click(screen.getByRole("button", { name: "Sačuvaj povrat" }));

    await waitFor(() =>
      expect(returnItems).toHaveBeenCalledWith(
        expect.objectContaining({ refundTender: "cash" }),
      ),
    );
  });

  it("blocks the return and skips the service when no quantity is entered", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const returnItems = vi.spyOn(services.receipts, "returnItems");

    render(<ReceiptsScreen receipts={services.receipts} userId={9} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );
    await user.click(await screen.findByRole("button", { name: "Povrat artikala" }));
    await user.type(screen.getByLabelText("Razlog povrata"), "Ostecen artikal");
    await user.click(screen.getByRole("button", { name: "Sačuvaj povrat" }));

    expect(
      await screen.findByText("Unesite količinu za bar jedan artikal."),
    ).toBeInTheDocument();
    expect(returnItems).not.toHaveBeenCalled();
  });
});

const summary: ReceiptSummary = {
  id: 1,
  receiptNumber: "R-2026-0001",
  createdAt: "2026-06-18T10:00:00Z",
  cashierName: "Mina Kasir",
  shiftId: 1,
  status: "voided",
  fiscalStatus: "not_fiscalized",
  documentType: "sale",
  paymentMethods: ["cash"],
  totalMinor: 100000,
  linkedDocumentCount: 1,
};

const detail: ReceiptDetail = {
  ...summary,
  subtotalMinor: 100000,
  discountMinor: 0,
  taxMinor: 16667,
  voidReason: "Pogresna stavka",
  returnReason: null,
  items: [
    {
      id: 1,
      productId: 2,
      productName: "Kafa 200 g",
      productSku: "KAFA-200",
      productBarcode: "8600000000027",
      quantityMilli: 2000,
      unitPriceMinor: 50000,
      discountMinor: 0,
      taxRateBasisPoints: 2000,
      taxMinor: 16667,
      totalMinor: 100000,
      returnedQuantityMilli: 0,
    },
  ],
  payments: [
    { id: 1, paymentMethod: "cash", amountMinor: 100000, createdAt: "2026-06-18T10:00:00Z" },
  ],
  linkedDocuments: [],
  canVoid: false,
  canReturn: false,
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

function buildReceiptsService(
  overrides: Partial<ReceiptsService> = {},
): ReceiptsService {
  return {
    searchReceipts: vi.fn(async () => ({ receipts: [summary], total: 1 })),
    getReceipt: vi.fn(async () => detail),
    voidReceipt: vi.fn(async () => detail),
    returnItems: vi.fn(async () => detail),
    ...overrides,
  };
}

describe("ReceiptsScreen states", () => {
  it("shows a loading indicator while receipts load", async () => {
    const pending = deferred<ReceiptSearchResult>();
    render(
      <ReceiptsScreen
        receipts={buildReceiptsService({ searchReceipts: () => pending.promise })}
        userId={1}
      />,
    );

    expect(await screen.findByText("Učitavanje računa...")).toBeInTheDocument();

    pending.resolve({ receipts: [summary], total: 1 });
    expect(await screen.findByText("R-2026-0001")).toBeInTheDocument();
  });

  it("shows an empty state when no receipts match", async () => {
    render(
      <ReceiptsScreen
        receipts={buildReceiptsService({
          searchReceipts: async () => ({ receipts: [], total: 0 }),
        })}
        userId={1}
      />,
    );

    expect(
      await screen.findByText("Nema računa za izabrane filtere"),
    ).toBeInTheDocument();
  });

  it("surfaces an error when the receipt list fails to load", async () => {
    const searchReceipts = vi
      .fn()
      .mockRejectedValue({ code: "database_error", message: "Baza nije dostupna." });
    render(
      <ReceiptsScreen
        receipts={buildReceiptsService({ searchReceipts })}
        userId={1}
      />,
    );

    const alert = await screen.findByRole("alert");
    expect(alert).toHaveTextContent("Baza nije dostupna.");
  });

  it("surfaces an error when opening receipt detail fails", async () => {
    const user = userEvent.setup();
    const getReceipt = vi
      .fn()
      .mockRejectedValue({ code: "database_error", message: "Detalji nisu dostupni." });
    render(
      <ReceiptsScreen
        receipts={buildReceiptsService({ getReceipt })}
        userId={1}
      />,
    );

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );

    expect(await screen.findByText("Detalji nisu dostupni.")).toBeInTheDocument();
  });

  it("renders the persisted void reason in the detail panel", async () => {
    const user = userEvent.setup();
    render(<ReceiptsScreen receipts={buildReceiptsService()} userId={1} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );

    expect(await screen.findByText("Razlog storniranja")).toBeInTheDocument();
    expect(screen.getByText("Pogresna stavka")).toBeInTheDocument();
  });
});
