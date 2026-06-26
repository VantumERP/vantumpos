import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";

import { Toaster } from "@/components/ui/sonner";
import type { ReportsService, UsersService } from "@/services/ports";
import type { UserAccount } from "@/services/types";

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
    listShifts: vi.fn().mockResolvedValue([
      {
        id: 1,
        openedAt: "2026-06-17T07:30:00Z",
        closedAt: null,
        cashierName: "Mira Kasir",
      },
    ]),
  };
}

function buildUsersService(): UsersService {
  return {
    listUsers: vi.fn().mockResolvedValue([adminUser, cashierUser]),
    createUser: vi.fn(),
    updateUser: vi.fn(),
    deactivateUser: vi.fn(),
  };
}

const adminUser: UserAccount = {
  id: 1,
  username: "admin",
  displayName: "Administrator",
  role: "admin",
  active: true,
  createdAt: "2026-06-17T07:00:00Z",
  updatedAt: "2026-06-17T07:00:00Z",
  lastLoginAt: null,
};

const cashierUser: UserAccount = {
  ...adminUser,
  id: 2,
  username: "mira",
  displayName: "Mira Kasir",
  role: "cashier",
};

function buildEmptyReportsService(): ReportsService {
  const service = buildReportsService();
  service.getDailyTurnover = vi.fn().mockResolvedValue({
    summary: {
      totalMinor: 0,
      cashMinor: 0,
      cardMinor: 0,
      receiptCount: 0,
      averageReceiptMinor: 0,
    },
    rows: [],
  });
  service.getShiftTurnover = vi.fn().mockResolvedValue({ rows: [] });
  service.getCashierTurnover = vi.fn().mockResolvedValue({ rows: [] });
  service.getPaymentMethodTurnover = vi.fn().mockResolvedValue({ rows: [] });
  service.getProductSales = vi.fn().mockResolvedValue({ rows: [] });
  service.getCategorySales = vi.fn().mockResolvedValue({ rows: [] });
  service.getLowStock = vi.fn().mockResolvedValue({ rows: [] });
  return service;
}

function renderReports(
  service = buildReportsService(),
  currentUser: UserAccount = adminUser,
  users: UsersService = buildUsersService(),
) {
  render(
    <>
      <ReportsScreen
        reports={service}
        users={users}
        currentUser={currentUser}
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

  it("filters reports by the selected shift", async () => {
    const user = userEvent.setup();
    const reports = renderReports();

    await screen.findByRole("heading", { name: "Dnevni promet" });
    await user.click(screen.getByRole("combobox", { name: "Smena" }));
    await user.click(
      await screen.findByRole("option", {
        name: "#1 - Mira Kasir (2026-06-17)",
      }),
    );

    expect(reports.getShiftTurnover).toHaveBeenLastCalledWith({
      from: "2026-06-17",
      to: "2026-06-17",
      shiftId: 1,
    });
  });

  it("filters reports by the selected cashier", async () => {
    const user = userEvent.setup();
    const reports = renderReports();

    await screen.findByRole("heading", { name: "Dnevni promet" });
    await user.click(screen.getByRole("combobox", { name: "Kasir" }));
    await user.click(await screen.findByRole("option", { name: "Mira Kasir" }));

    expect(reports.getCashierTurnover).toHaveBeenLastCalledWith({
      from: "2026-06-17",
      to: "2026-06-17",
      cashierId: 2,
    });
  });

  it("clears the cashier filter when the all option is chosen", async () => {
    const user = userEvent.setup();
    const reports = renderReports();

    await screen.findByRole("heading", { name: "Dnevni promet" });
    await user.click(screen.getByRole("combobox", { name: "Kasir" }));
    await user.click(await screen.findByRole("option", { name: "Mira Kasir" }));
    expect(reports.getCashierTurnover).toHaveBeenLastCalledWith({
      from: "2026-06-17",
      to: "2026-06-17",
      cashierId: 2,
    });

    await user.click(screen.getByRole("combobox", { name: "Kasir" }));
    await user.click(await screen.findByRole("option", { name: "Svi kasiri" }));

    expect(reports.getCashierTurnover).toHaveBeenLastCalledWith({
      from: "2026-06-17",
      to: "2026-06-17",
      cashierId: null,
    });
  });

  it("clears the shift filter when the all option is chosen", async () => {
    const user = userEvent.setup();
    const reports = renderReports();

    await screen.findByRole("heading", { name: "Dnevni promet" });
    await user.click(screen.getByRole("combobox", { name: "Smena" }));
    await user.click(
      await screen.findByRole("option", {
        name: "#1 - Mira Kasir (2026-06-17)",
      }),
    );
    expect(reports.getShiftTurnover).toHaveBeenLastCalledWith({
      from: "2026-06-17",
      to: "2026-06-17",
      shiftId: 1,
    });

    await user.click(screen.getByRole("combobox", { name: "Smena" }));
    await user.click(await screen.findByRole("option", { name: "Sve smene" }));

    expect(reports.getShiftTurnover).toHaveBeenLastCalledWith({
      from: "2026-06-17",
      to: "2026-06-17",
      shiftId: null,
    });
  });

  it("still renders and filters when the shift and user lists fail to load", async () => {
    const user = userEvent.setup();
    const reports = buildReportsService();
    reports.listShifts = vi.fn().mockRejectedValue({
      code: "database_error",
      message: "Smene nisu dostupne.",
    });
    const users = buildUsersService();
    users.listUsers = vi.fn().mockRejectedValue({
      code: "database_error",
      message: "Korisnici nisu dostupni.",
    });
    renderReports(reports, adminUser, users);

    // The screen must remain usable even though the filter dropdowns stayed empty.
    await screen.findByRole("heading", { name: "Dnevni promet" });
    expect(screen.getByRole("combobox", { name: "Smena" })).toBeInTheDocument();

    await user.clear(screen.getByLabelText("Od datuma"));
    await user.type(screen.getByLabelText("Od datuma"), "2026-06-16");
    await user.click(screen.getByRole("button", { name: "Primeni filtere" }));

    expect(reports.getDailyTurnover).toHaveBeenLastCalledWith({
      from: "2026-06-16",
      to: "2026-06-17",
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

  it("blocks non-admin operators with a role notice and skips loading", () => {
    const reports = renderReports(buildReportsService(), cashierUser);

    expect(
      screen.getByText("Samo administrator moze da vidi izvestaje."),
    ).toBeInTheDocument();
    expect(
      screen.queryByRole("heading", { name: "Dnevni promet" }),
    ).not.toBeInTheDocument();
    expect(reports.getDailyTurnover).not.toHaveBeenCalled();
  });

  it("rejects an inverted date range without calling the service", async () => {
    const user = userEvent.setup();
    const reports = renderReports();

    await screen.findByRole("heading", { name: "Dnevni promet" });
    await user.clear(screen.getByLabelText("Od datuma"));
    await user.type(screen.getByLabelText("Od datuma"), "2026-06-20");
    await user.click(screen.getByRole("button", { name: "Primeni filtere" }));

    expect(
      screen.getByText("Pocetni datum ne sme biti posle krajnjeg datuma."),
    ).toBeInTheDocument();
    expect(reports.getDailyTurnover).toHaveBeenCalledTimes(1);
  });

  it("renders empty states when reports return no rows", async () => {
    const user = userEvent.setup();
    renderReports(buildEmptyReportsService());

    expect(await screen.findByText("Nema prometa")).toBeInTheDocument();

    await user.click(screen.getByRole("tab", { name: "Lager" }));
    expect(
      await screen.findByText("Nema artikala ispod minimuma"),
    ).toBeInTheDocument();
  });

  it("surfaces a backend failure in an alert", async () => {
    const reports = buildReportsService();
    reports.getDailyTurnover = vi.fn().mockRejectedValue({
      code: "database_error",
      message: "Izvestaj nije dostupan.",
    });
    renderReports(reports);

    expect(
      await screen.findByText("Izvestaj nije dostupan."),
    ).toBeInTheDocument();
  });
});
