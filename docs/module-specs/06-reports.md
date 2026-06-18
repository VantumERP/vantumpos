# Module Spec: Reports

## Goal

Implement Izvestaji for owner/admin visibility into turnover, shifts, payments, products, categories, margin estimate, low stock, and product ledger exports.

## Dependencies

Best implemented after:

- Register/Sales,
- Inventory,
- Receipts.

## Scope

In scope:

- Daily turnover.
- Turnover by shift.
- Turnover by cashier.
- Turnover by payment method.
- Sales by product.
- Sales by category.
- Margin/profit estimate.
- Low stock report.
- Product ledger report.
- CSV export.

Out of scope:

- PDF export.
- BI dashboards.
- Cloud analytics.
- Multi-store comparison.

## Screens

Use `Tabs` for report groups:

- Promet,
- Artikli,
- Lager,
- Izvoz.

Use `Chart` only where it helps comparison. Use `Table` as the primary report surface because this is an operational desktop tool.

Common filters:

- date range,
- shift,
- cashier,
- category,
- product.

### Daily Turnover

Cards:

- total turnover,
- cash,
- card,
- receipt count,
- average receipt.

Table:

- day,
- receipt count,
- cash,
- card,
- total,
- refunds/voids.

### Product/Category Sales

Columns:

- product/category,
- quantity,
- revenue,
- discount,
- estimated margin.

### Low Stock

Columns:

- product,
- current stock,
- minimum stock,
- difference,
- last movement.

## Service Contract

```ts
interface ReportsService {
  getDailyTurnover(query: ReportDateQuery): Promise<DailyTurnoverReport>
  getShiftTurnover(query: ReportDateQuery): Promise<ShiftTurnoverReport>
  getCashierTurnover(query: ReportDateQuery): Promise<CashierTurnoverReport>
  getPaymentMethodTurnover(query: ReportDateQuery): Promise<PaymentMethodReport>
  getProductSales(query: ProductSalesQuery): Promise<ProductSalesReport>
  getCategorySales(query: ReportDateQuery): Promise<CategorySalesReport>
  getLowStock(): Promise<LowStockReport>
  exportReportCsv(request: ExportReportRequest): Promise<ExportedFile>
}
```

## Backend Commands

- `reports_daily_turnover`
- `reports_shift_turnover`
- `reports_cashier_turnover`
- `reports_payment_methods`
- `reports_product_sales`
- `reports_category_sales`
- `reports_low_stock`
- `reports_export_csv`

## SQLite Notes

Reports should query existing tables. Avoid denormalized summary tables until there is a measured performance problem.

Use sale item snapshots for historical product/category reporting where product changes would otherwise distort history. If category snapshot is missing from sale item, either join current category with clear limitation or add category snapshot in a future sales migration.

## Business Rules

- Voids/returns must be reflected consistently. Decide whether reports show net sales, gross sales, or both.
- Margin is an estimate from purchase price snapshot/current purchase price depending on data available. Label it as estimate in UI.
- Reports should not mutate data.
- CSV export uses Serbian-friendly headers.

## Tests

Backend:

- daily turnover groups completed sales,
- payment method report handles mixed payments,
- voids/returns reduce net totals,
- low stock query returns products below minimum,
- CSV export contains expected headers and rows.

Frontend:

- report tabs render distinct content,
- date filters call service with expected query,
- charts/tables render empty and loaded states,
- export action shows success/error toast.

## Acceptance Criteria

- Izvestaji is a real admin screen.
- Owner can answer: daily total, cash/card total, best products, low stock.
- CSV export works for at least daily turnover and product sales.

## Prompt For Separate Chat

```text
Implementiraj Reports/Izvestaji module for VantumPOS.

Read:
- docs/module-specs/00-shared-foundation.md
- docs/module-specs/06-reports.md
- docs/superpowers/specs/2026-06-17-vantumpos-local-pos-design.md

Build report tabs, backend report queries, CSV export, TS service contracts, mock data, frontend tests, and Rust tests. Use shadcn Table and Chart where appropriate. Do not add PDF export or cloud analytics.
```

