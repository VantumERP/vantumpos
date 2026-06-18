import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { Toaster } from "@/components/ui/sonner";
import type { ReportsService } from "@/services/ports";

import { ReportsScreen } from "./ReportsScreen";

function buildReportsService(): ReportsService {
  return {
    getDailyTurnover: vi.fn().mockResolvedValue({
      summary: {
        totalMinor: 12000,
        cashMinor: 8000,
        cardMinor: 4000,
        receiptCount: 2,
        averageReceiptMinor: 6000,
      },
      rows: [
        {
          day: "2026-06-17",
          receiptCount: 2,
          cashMinor: 8000,
          cardMinor: 4000,
          totalMinor: 12000,
          refundsOrVoidsMinor: -3000,
          refundsOrVoidsCount: 1,
        },
      ],
    }),
    getShiftTurnover: vi.fn().mockResolvedValue({
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
    }),
    getCashierTurnover: vi.fn().mockResolvedValue({
      rows: [
        {
          cashierId: 1,
          cashierName: "Mira Kasir",
          receiptCount: 2,
          totalMinor: 12000,
        },
      ],
    }),
    getPaymentMethodTurnover: vi.fn().mockResolvedValue({
      rows: [
        { paymentMethod: "cash", receiptCount: 2, totalMinor: 8000 },
        { paymentMethod: "card", receiptCount: 1, totalMinor: 4000 },
      ],
    }),
    getProductSales: vi.fn().mockResolvedValue({
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
    }),
    getCategorySales: vi.fn().mockResolvedValue({
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
    }),
    getLowStock: vi.fn().mockResolvedValue({
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
    }),
    exportReportCsv: vi.fn().mockResolvedValue({
      fileName: "dnevni-promet-2026-06-17-2026-06-17.csv",
      path: "C:/exports/dnevni-promet-2026-06-17-2026-06-17.csv",
      mimeType: "text/csv",
      rowCount: 1,
    }),
  };
}

function renderReports(service = buildReportsService()) {
  render(
    <>
      <ReportsScreen
        reports={service}
        initialQuery={{ from: "2026-06-17", to: "2026-06-17" }}
      />
      <Toaster />
    </>,
  );
  return service;
}

describe("ReportsScreen", () => {
  it("renders turnover, product sales, category sales, and low stock reports", async () => {
    const user = userEvent.setup();
    renderReports();

    expect(
      await screen.findByRole("heading", { name: "Dnevni promet" }),
    ).toBeInTheDocument();
    expect(screen.getAllByText("120,00 RSD").length).toBeGreaterThan(0);
    expect(screen.getByRole("cell", { name: "2026-06-17" })).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Artikli" }));
    expect(await screen.findByRole("cell", { name: "Kafa 200g" }))
      .toBeInTheDocument();
    expect(screen.getByRole("cell", { name: "Pica" })).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Lager" }));
    const lowStockRow = await screen.findByRole("row", { name: /Kafa 200g/i });
    expect(within(lowStockRow).getByText("-2 kom")).toBeInTheDocument();
  });

  it("applies date filters through the reports service", async () => {
    const user = userEvent.setup();
    const reports = renderReports();

    await screen.findByRole("heading", { name: "Dnevni promet" });
    await user.clear(screen.getByLabelText("Od datuma"));
    await user.type(screen.getByLabelText("Od datuma"), "2026-06-16");
    await user.click(screen.getByRole("button", { name: "Primeni filtere" }));

    expect(reports.getDailyTurnover).toHaveBeenLastCalledWith({
      from: "2026-06-16",
      to: "2026-06-17",
    });
    expect(reports.getProductSales).toHaveBeenLastCalledWith({
      from: "2026-06-16",
      to: "2026-06-17",
      categoryId: null,
      productId: null,
    });
  });

  it("exports the active report as CSV and shows the exported path", async () => {
    const user = userEvent.setup();
    const reports = renderReports();

    await screen.findByRole("heading", { name: "Dnevni promet" });
    await user.click(screen.getByRole("tab", { name: "Izvoz" }));
    await user.click(screen.getByRole("button", { name: "Izvezi dnevni promet" }));

    expect(reports.exportReportCsv).toHaveBeenCalledWith({
      reportType: "dailyTurnover",
      query: {
        from: "2026-06-17",
        to: "2026-06-17",
        categoryId: null,
        productId: null,
      },
    });
    expect(await screen.findByText(/CSV izvezen/)).toBeInTheDocument();
    expect(
      screen.getByText("C:/exports/dnevni-promet-2026-06-17-2026-06-17.csv"),
    ).toBeInTheDocument();
  });
});
