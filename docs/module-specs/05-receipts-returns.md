# Module Spec: Receipts, Voids, And Returns

## Goal

Implement Racuni as searchable receipt history with detail view, local receipt preview, full void, and selected item return. Original receipts must never be deleted.

## Dependencies

Best implemented after Register/Sales.

## Scope

In scope:

- Receipt list/search.
- Receipt detail.
- Payment and VAT breakdown.
- Linked original/refund documents.
- Full sale void.
- Selected item return.
- Stock reversal through inventory movements.

Out of scope:

- Fiscal receipt cancellation.
- SEF/e-invoice.
- Customer accounts.

## Screens

### Receipt Search

Use `Table`.

Filters:

- date range,
- local receipt number,
- cashier,
- shift,
- payment method,
- product.

Columns:

- receipt number,
- date/time,
- cashier,
- status,
- fiscal status,
- payment methods,
- total,
- actions.

### Receipt Detail

Use a detail panel or route-like main view:

- header with receipt number, date, status,
- items table,
- payments table,
- totals,
- original/linked receipt references,
- action buttons.

### Void/Return Flow

Use `AlertDialog` for full void and `Dialog` or `Sheet` for item return.

Required fields:

- reason,
- selected items and quantities for partial return.

## Service Contract

```ts
interface ReceiptsService {
  searchReceipts(query: ReceiptSearchQuery): Promise<ReceiptSearchResult>
  getReceipt(id: number): Promise<ReceiptDetail>
  voidReceipt(request: VoidReceiptRequest): Promise<ReceiptDetail>
  returnItems(request: ReturnItemsRequest): Promise<ReceiptDetail>
}
```

## Backend Commands

- `receipts_search`
- `receipts_get`
- `receipts_void`
- `receipts_return_items`

## SQLite Notes

Existing tables:

- `sales`
- `sale_items`
- `sale_payments`
- `inventory_movements`
- `inventory_balances`

Voids/returns create new `sales` rows linked by `original_sale_id`. Do not mutate original sale totals. You may update original sale status to `voided` or `refunded` only as a status marker, while keeping original rows.

Add migration if needed for:

- return reason,
- void reason,
- item-level return linkage,
- sale type (`sale`, `void`, `return`) if status alone is insufficient.

## Business Rules

- Completed receipt can be voided once.
- Partial return cannot exceed originally sold quantity minus already returned quantity.
- Return/void writes inventory movements.
- Return/void updates inventory balances.
- Original receipt remains readable.
- Returned document has negative item quantities or explicit return type. Pick one model and document it in code/tests.

## Tests

Backend:

- receipt search returns completed sale,
- receipt detail includes items and payments,
- full void creates linked document and restores stock,
- duplicate void fails,
- partial return rejects excessive quantity,
- failed return rolls back inventory changes.

Frontend:

- receipt search filters render,
- detail view shows totals and linked status,
- void confirmation requires reason,
- partial return updates displayed linked document.

## Acceptance Criteria

- Racuni is a real history and operations screen.
- Original sale is never deleted.
- Voids/returns are auditable and stock-correct.

## Prompt For Separate Chat

```text
Implementiraj Receipts/Racuni module for VantumPOS.

Read:
- docs/module-specs/00-shared-foundation.md
- docs/module-specs/05-receipts-returns.md
- docs/superpowers/specs/2026-06-17-vantumpos-local-pos-design.md

Build receipt search, receipt detail, full void, partial return, linked document persistence, inventory reversal, service contracts, Tauri commands, Rust tests, frontend tests, and mock adapter data. Do not implement real fiscal cancellation.
```

