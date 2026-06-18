# Module Spec: Register And Sales

## Goal

Implement the main Kasa workflow: scan/search products, build a receipt, apply discounts, take cash/card/mixed payment, complete sale locally, update stock, and show receipt preview.

## Dependencies

Best implemented after:

- Auth and shifts,
- Catalog products,
- Inventory stock helpers.

## Scope

In scope:

- Barcode/SKU/name input.
- Product search.
- Cart item table.
- Quantity changes.
- Item discount and receipt discount.
- VAT breakdown.
- Cash/card/mixed payment.
- Change calculation.
- Complete sale backend transaction.
- Receipt preview after completion.

Out of scope:

- Real fiscalization.
- Printer driver commands.
- Cash drawer.
- Restaurant tables/orders.

## Screen Layout

Kasa should be dense and cashier-first.

Recommended layout:

- top barcode/search `InputGroup`, focused by default,
- left or center receipt item table,
- right payment/totals panel,
- bottom action row for clear cart, hold later if needed, complete sale.

Receipt item row:

- product name,
- SKU/barcode,
- quantity stepper/input,
- unit price,
- discount,
- VAT,
- line total,
- remove action.

Payment panel:

- subtotal,
- discount,
- VAT total,
- total,
- cash received,
- card amount,
- change,
- complete sale.

Use `AlertDialog` for clearing cart and destructive actions.

## Service Contract

```ts
interface SalesService {
  createSalePreview(request: SaleDraftRequest): Promise<SalePreview>
  completeSale(request: CompleteSaleRequest): Promise<CompletedSale>
}
```

Catalog service provides product search. Shift service provides current shift.

## Backend Commands

- `sales_preview`
- `sales_complete`

Optional helper:

- `sales_get_next_receipt_number_preview`

## SQLite Notes

Existing tables:

- `sales`
- `sale_items`
- `sale_payments`
- `inventory_movements`
- `inventory_balances`
- `products`
- `shifts`
- `users`

Consider migration for:

- receipt number sequence setting,
- cash change metadata,
- sale-level note,
- payment method expansion if needed.

## Transaction Flow

`sales_complete` must perform all steps in one transaction:

1. Validate authenticated/current user.
2. Validate open shift.
3. Load products by id.
4. Reject inactive products.
5. Validate stock for products that cannot go negative.
6. Recalculate prices, discounts, VAT, and totals from backend product data.
7. Validate payment total.
8. Assign local receipt number.
9. Set fiscal status `not_fiscalized`.
10. Insert sale.
11. Insert sale items with product snapshots.
12. Insert sale payments.
13. Insert inventory movements.
14. Update inventory balances.
15. Return completed sale DTO.

Frontend totals are previews. Backend totals are authoritative.

## Business Rules

- Sale cannot complete without open shift.
- Empty cart cannot complete.
- Quantity cannot be zero.
- Discount cannot make line or sale total negative.
- Cash overpayment is allowed for change calculation.
- Stored payment amount should not inflate revenue beyond sale total.
- Card overpayment is not allowed.
- Mixed payment must exactly cover sale total, except cash change handling.
- Receipt item snapshots remain unchanged if product later changes.

## Tests

Backend:

- complete cash sale inserts sale, items, payments, movement, and balance update,
- sale without shift fails,
- insufficient stock fails and rolls back all inserts,
- backend recalculates totals and ignores tampered frontend totals,
- mixed payment succeeds when totals match,
- payment mismatch fails.

Frontend:

- scanner/search adds item to cart,
- quantity and discount update preview,
- cash payment shows change,
- complete sale shows receipt preview,
- backend error preserves cart and shows message.

## Acceptance Criteria

- Kasa is a real cashier screen.
- A local sale can be completed end-to-end.
- Stock decreases.
- Receipt appears in Receipts module data later.
- No fiscalization claims appear in UI.

## Prompt For Separate Chat

```text
Implementiraj Register/Kasa and local sale completion for VantumPOS.

Read:
- docs/module-specs/00-shared-foundation.md
- docs/module-specs/04-register-sales.md
- docs/superpowers/specs/2026-06-17-vantumpos-local-pos-design.md

Build a real cashier screen, sale preview, complete sale command, transaction-safe Rust backend, stock updates, receipt preview, TS service contracts, mock adapter support, and tests. Do not add real fiscalization or printer drivers.
```

