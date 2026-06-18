import { describe, expect, it, vi } from "vitest";

import { createLocalServices } from "./local-adapter";

describe("reports service adapter", () => {
  it("maps reports service methods to stable Tauri command names", async () => {
    const invoke = vi.fn().mockImplementation((command: string) => {
      switch (command) {
        case "reports_daily_turnover":
          return Promise.resolve({ summary: {}, rows: [] });
        case "reports_shift_turnover":
          return Promise.resolve({ rows: [] });
        case "reports_cashier_turnover":
          return Promise.resolve({ rows: [] });
        case "reports_payment_methods":
          return Promise.resolve({ rows: [] });
        case "reports_product_sales":
          return Promise.resolve({ rows: [] });
        case "reports_category_sales":
          return Promise.resolve({ rows: [] });
        case "reports_low_stock":
          return Promise.resolve({ rows: [] });
        case "reports_export_csv":
          return Promise.resolve({
            fileName: "dnevni-promet-2026-06-17-2026-06-17.csv",
            path: "C:/exports/dnevni-promet-2026-06-17-2026-06-17.csv",
            mimeType: "text/csv",
            rowCount: 1,
          });
        default:
          throw new Error(`unexpected command ${command}`);
      }
    });
    const services = createLocalServices(invoke);
    const dateQuery = { from: "2026-06-17", to: "2026-06-17" };
    const productQuery = {
      ...dateQuery,
      categoryId: null,
      productId: null,
    };

    await services.reports.getDailyTurnover(dateQuery);
    await services.reports.getShiftTurnover(dateQuery);
    await services.reports.getCashierTurnover(dateQuery);
    await services.reports.getPaymentMethodTurnover(dateQuery);
    await services.reports.getProductSales(productQuery);
    await services.reports.getCategorySales(dateQuery);
    await services.reports.getLowStock();
    await services.reports.exportReportCsv({
      reportType: "dailyTurnover",
      query: productQuery,
    });

    expect(invoke).toHaveBeenNthCalledWith(1, "reports_daily_turnover", {
      query: dateQuery,
    });
    expect(invoke).toHaveBeenNthCalledWith(2, "reports_shift_turnover", {
      query: dateQuery,
    });
    expect(invoke).toHaveBeenNthCalledWith(3, "reports_cashier_turnover", {
      query: dateQuery,
    });
    expect(invoke).toHaveBeenNthCalledWith(4, "reports_payment_methods", {
      query: dateQuery,
    });
    expect(invoke).toHaveBeenNthCalledWith(5, "reports_product_sales", {
      query: productQuery,
    });
    expect(invoke).toHaveBeenNthCalledWith(6, "reports_category_sales", {
      query: dateQuery,
    });
    expect(invoke).toHaveBeenNthCalledWith(7, "reports_low_stock");
    expect(invoke).toHaveBeenNthCalledWith(8, "reports_export_csv", {
      request: { reportType: "dailyTurnover", query: productQuery },
    });
  });
});
