# Module Spec: Inventory And Ledger

## Goal

Implement a real Lager module that tracks stock list, receiving, corrections, write-offs, and product ledger. Inventory changes must be auditable and transactional.

## Scope

In scope:

- Stock list with filters and low-stock state.
- Receive goods flow.
- Correction flow.
- Write-off flow.
- Product ledger view.
- Inventory movement history.
- Stock balance updates in Rust transactions.

Out of scope:

- Supplier invoices.
- Purchase order workflow.
- Multiple warehouses.
- Barcode label printing.

## Screens

### Stock List

Use `Table` with filters:

- search product,
- category,
- low stock,
- zero stock,
- negative stock.

Columns:

- product,
- SKU/code,
- barcode,
- current quantity,
- minimum stock,
- unit,
- last movement,
- actions.

Actions:

- receive,
- correction,
- write-off,
- ledger.

### Movement Forms

Use `Dialog` or `Sheet`.

Common fields:

- product,
- quantity,
- reason/note.

Receive can optionally include purchase price snapshot, but MVP can leave supplier fields out.

Correction requires a reason.

Write-off requires a reason.

### Product Ledger

Show all movements for one product:

- date,
- movement type,
- quantity delta,
- resulting balance if feasible,
- reference type/id,
- user,
- reason.

## Service Contract

```ts
interface InventoryService {
  listStock(query: StockListQuery): Promise<StockListResult>
  getProductLedger(productId: number): Promise<ProductLedger>
  receiveStock(request: InventoryAdjustmentRequest): Promise<InventoryAdjustmentResult>
  correctStock(request: InventoryAdjustmentRequest): Promise<InventoryAdjustmentResult>
  writeOffStock(request: InventoryAdjustmentRequest): Promise<InventoryAdjustmentResult>
}
```

## Backend Commands

- `inventory_list_stock`
- `inventory_get_product_ledger`
- `inventory_receive`
- `inventory_correct`
- `inventory_write_off`

## SQLite Notes

Existing tables:

- `inventory_balances`
- `inventory_movements`
- `products`
- `users`

Every stock change writes:

- one `inventory_movements` row,
- one balance update in `inventory_balances`.

Use transaction. If movement insert fails, balance must not change.

## Business Rules

- Quantity cannot be zero.
- Receive quantity must be positive.
- Correction can be positive or negative.
- Write-off quantity must reduce stock and should be stored as negative movement.
- If product does not allow negative stock, backend rejects resulting negative balance.
- All movements must include user id when available.
- Product deletion should not erase ledger if product had movements. If current FK behavior is too destructive for audited movements, add a migration or change delete behavior before implementing destructive product operations.

## Tests

Backend:

- receive creates movement and increases balance,
- correction creates movement and sets/increments balance according to chosen model,
- write-off decreases balance,
- insufficient stock fails for non-negative product,
- failed transaction leaves balance unchanged,
- ledger returns movements ordered newest first or oldest first consistently.

Frontend:

- stock list renders low-stock badges,
- receive form updates visible stock,
- write-off failure shows Serbian error,
- ledger opens for selected product.

## Acceptance Criteria

- Clicking Lager shows stock and movement UI.
- Inventory actions persist to SQLite and are visible after refresh.
- Kasa sale completion can later reuse stock validation/update helpers.

## Prompt For Separate Chat

```text
Implementiraj Inventory/Lager module for VantumPOS.

Read:
- docs/module-specs/00-shared-foundation.md
- docs/module-specs/03-inventory.md
- docs/superpowers/specs/2026-06-17-vantumpos-local-pos-design.md

Build stock list, receive/correction/write-off flows, product ledger, service contracts, Tauri commands, Rust transaction helpers, backend tests, mock adapter data, and frontend tests. Do not implement sale completion except reusable inventory helpers if needed.
```

