# VantumPOS Local POS MVP Design

Date: 2026-06-17
Status: Approved for planning

## Purpose

VantumPOS MVP is a local desktop POS application for Serbian retail shops. The first version targets one shop, one cash register, and one local SQLite database. It is intended for general retail stores first, not cafes or restaurants.

The MVP must be stable as a standalone local application before adding real fiscalization, cloud sync, Medusa.js connectivity, or multi-register operation.

Public competitor positioning was reviewed at:

- https://bkcsoft.rs/
- https://bkcsoft.rs/fiskalizacija/
- https://bkcsoft.rs/cenovnik/

The competitor emphasis around ESIR/fiscalization, stock, barcodes, reports, SEF/SEO, and support informs the product direction, but this MVP deliberately focuses on the local operational core first.

## Approved Scope

In scope:

- Local desktop app with Tauri, React, shadcn/ui, and SQLite.
- One company/shop, one register, one local database.
- Serbian Latin UI for the MVP.
- Login with basic users and roles.
- Cashier shifts with opening and closing flows.
- Product catalog with categories, SKU/code, barcode, VAT rate, purchase price, sale price, and minimum stock.
- Inventory operations: receiving goods, correction, write-off, stock list, and product ledger.
- Sales flow with discounts, cash/card/mixed payments, local receipts, returns, and void/refund flow.
- Owner reports for shifts, cashiers, payment methods, products, categories, margin estimate, low stock, and product ledger.
- CSV/XLSX import wizard with column mapping, validation, dry run, duplicate detection, and import history.
- Company/shop settings, VAT rates, receipt format, local receipt numbering, backup folder, and local backup/restore.
- Manual and automatic local backups.
- Domain adapter architecture prepared for later fiscalization and Medusa.js integration.

Out of scope for the MVP:

- Real ESIR/LPFR/V-PFR fiscalization.
- Medusa.js connection.
- Cloud backup or sync.
- Multiple stores, warehouses, or registers.
- Cafe/restaurant workflows.
- ESC/POS printer commands, cash drawer, scales, or direct hardware drivers.
- Supplier accounting, purchase calculations, debt tracking, advances, reservations, and full BI analytics.
- Product variants such as size/color for boutiques.

## Architecture

The application has three layers.

### React UI

React owns screens, layout, local form state, and user interaction. It must not directly know SQLite schemas, Rust structs, or future Medusa details.

The UI talks to domain services:

- `authService`
- `usersService`
- `shiftService`
- `catalogService`
- `inventoryService`
- `salesService`
- `reportsService`
- `importService`
- `settingsService`
- `backupService`

### TypeScript Domain Service Layer

The domain service layer is the contract between UI and backend. It exposes stable TypeScript types and use-case methods.

The first adapter is `localAdapter`, which calls Tauri commands through `invoke`. Later adapters can implement the same service contracts:

- `medusaAdapter` for Medusa.js-backed catalog/order flows.
- `fiscalAdapter` for real fiscalization.

Frontend screens should depend on service interfaces, not on backend implementation details.

### Rust/Tauri Local Backend

Rust is the local source of truth. It owns:

- SQLite connection management.
- Database migrations.
- Atomic business transactions.
- Backend validation.
- Sales completion.
- Inventory ledger writes.
- Import parsing and validation.
- Backup and restore.
- Structured error codes.
- Local logging.

State-changing operations must be completed in backend transactions. A sale must never be considered complete only because React state changed.

## Fiscal Adapter Boundary

The MVP includes a fiscal adapter boundary but no real fiscalization.

The initial `localFiscalAdapter` only:

- assigns or accepts the local receipt number from the local sales flow,
- returns fiscal status `not_fiscalized`,
- stores enough metadata so later fiscal integration can attach official fiscal identifiers without redesigning the sales domain.

Receipts generated in the MVP are local confirmations, not fiscal receipts.

## Main Modules And Screens

### Login And Shifts

Cashiers sign in with password or PIN. Admin users can manage settings, users, reports, import, and catalog/inventory screens.

If a cashier has no open shift, the app shows an open-shift screen before allowing sale completion. Shift close shows expected totals by payment method and fields for counted cash.

### Register

The register is the home screen for cashiers.

Core UI:

- focused input for barcode, SKU/code, or product search,
- product search by name, SKU/code, or barcode,
- receipt item table,
- quantity control,
- item discount,
- receipt-level discount,
- VAT and total breakdown,
- payment panel for cash, card, and mixed payment,
- change display for cash payments,
- complete sale action,
- local receipt preview and print action after completion.

Barcode scanners are treated as keyboard input.

### Products

Product list supports filters for category, active/inactive status, low stock, and missing barcode.

Product form fields:

- name,
- SKU/code,
- barcode,
- category,
- unit of measure,
- sale price with VAT,
- purchase price without VAT,
- VAT rate,
- minimum stock,
- active status.

### Inventory

Inventory is separate from product editing.

Screens:

- stock list,
- receive goods,
- correction,
- write-off,
- product ledger.

Every stock change writes an inventory movement with type, reason, user, timestamp, product, quantity, and document/reference id.

### Receipts

Receipt search supports date, local receipt number, cashier, shift, payment method, and product.

Receipt detail shows:

- local receipt number,
- cashier,
- shift,
- status,
- fiscal status,
- items,
- discounts,
- payments,
- totals,
- linked original receipt for returns/voids.

Return and void operations create linked documents. The original receipt is never deleted.

### Reports

MVP reports:

- daily turnover,
- turnover by shift,
- turnover by cashier,
- turnover by payment method,
- sales by product,
- sales by category,
- margin/profit estimate,
- low stock,
- product ledger.

CSV export is enough for the MVP. PDF export can be added later.

### Import

The import wizard supports CSV and XLSX files for products, categories, and initial stock.

Flow:

1. Select file and import type.
2. Read headers.
3. Map required columns.
4. Map optional columns.
5. Validate rows.
6. Show row errors and warnings.
7. Run dry run summary.
8. Confirm real import.

Required product columns:

- name,
- sale price,
- VAT rate,
- SKU/code or barcode.

Optional columns:

- category,
- purchase price,
- minimum stock,
- unit of measure,
- initial stock.

Duplicate matching order:

1. barcode,
2. SKU/code,
3. name only as a warning.

Import never deletes existing products.

### Settings

Settings include:

- company/shop name,
- address,
- PIB,
- registration number,
- phone,
- logo,
- VAT rates,
- default currency `RSD`,
- receipt format,
- local receipt number prefix/sequence,
- backup folder,
- users and roles,
- restore from backup.

## Data Model

Use SQLite migrations from the start.

Prices and money amounts are stored as integer minor units, meaning Serbian dinar para values. Do not use floating point for money.

Core tables:

- `settings`
- `users`
- `shifts`
- `categories`
- `tax_rates`
- `products`
- `inventory_balances`
- `inventory_movements`
- `sales`
- `sale_items`
- `sale_payments`
- `import_jobs`
- `import_job_rows`
- `backup_jobs`

### Important Table Notes

`products` stores the current product definition. It includes sale price with VAT, purchase price without VAT, VAT rate, minimum stock, and active status.

`inventory_balances` is the current stock projection for fast reads.

`inventory_movements` is the audit ledger and source for product stock history.

`sales` stores local receipt metadata, totals, status, fiscal status, cashier, shift, and links to original sale records for void/return documents.

`sale_items` stores snapshots of product name, SKU/code, barcode, price, VAT rate, discount, quantity, and totals. Old receipts must not change when product data changes later.

`sale_payments` stores one or more payment rows for cash, card, and mixed payments.

## Sales Flow

Preconditions:

- User is authenticated.
- User has an open shift.
- Products in the cart are active.
- Stock is sufficient for products that cannot go negative.

Frontend sends a `completeSale` request with items, discounts, and payments.

Backend completes the sale in one SQLite transaction:

1. Validate shift.
2. Validate products.
3. Validate stock.
4. Recalculate item totals, discounts, VAT, and receipt total.
5. Validate payment totals.
6. Assign local receipt number.
7. Call `localFiscalAdapter`, returning `not_fiscalized`.
8. Insert sale.
9. Insert sale items.
10. Insert sale payments.
11. Insert inventory movements.
12. Update inventory balances.
13. Return completed sale data for receipt preview and print.

Discounts can be fixed amount or percentage at item level and receipt level. Backend rejects discounts that would make totals negative.

Cash, card, and mixed payment are supported. Cash overpayment is allowed only for change calculation. Stored paid amount should match the sale total, while change is returned as receipt metadata.

## Returns And Voids

The original sale is never deleted.

Void/refund creates a linked document that:

- references the original sale,
- records the user and reason,
- reverses relevant quantities,
- writes inventory movements,
- updates inventory balances,
- keeps receipt history intact.

MVP supports full sale void and selected item return for local receipts.

## Backup And Restore

Because the app is local-first, backup is required in the MVP.

Supported flows:

- automatic daily backup to the configured local folder,
- manual backup,
- restore from backup,
- backup job history.

Backups must be consistent. The backend should use the SQLite backup API or an equivalent controlled backend operation instead of copying the database file while it may be open for writes.

Restore must require confirmation and clearly state that current data will be replaced.

## Error Handling And Logging

Backend errors return a structured shape:

- code,
- user-facing message,
- optional details,
- optional field or row references for validation errors.

UI messages should be Serbian, clear, and operational:

- Artikal nije pronadjen.
- Nema dovoljno zaliha.
- Smena nije otvorena.
- Placanja se ne poklapaju sa ukupnim iznosom.
- Backup nije uspeo. Proverite da li je folder dostupan.
- Import ima greske. Ispravite oznacene redove pre upisa.

Technical details go to a local log file for support.

## UI And UX Direction

The UI is a working desktop tool, not a landing page. It should feel calm, dense, and fast.

Primary language is Serbian Latin.

Navigation:

- Kasa
- Artikli
- Lager
- Racuni
- Izvestaji
- Import
- Podesavanja

Header:

- shop name,
- active shift,
- signed-in user,
- backup status,
- local database status if needed.

Use shadcn/ui components and registry blocks before custom markup. For this project, shadcn context is:

- Vite,
- Tailwind v4,
- `base-mira`,
- `lucide`,
- shadcn base primitives,
- alias `@`.

Installed components already include `sidebar`, `table`, `chart`, `field`, `input-group`, `dialog`, `sheet`, `command`, `empty`, `sonner`, and the other core UI primitives.

Blocks and patterns to review before UI implementation:

- `login-02` for login/auth layout inspiration,
- `sidebar-01`, `sidebar-02`, `sidebar-05`, `sidebar-07`, `sidebar-16` for app shell/navigation,
- `dashboard-01` for sidebar, chart, and data table composition.

Implementation rules:

- Use shadcn `Sidebar` for the app shell.
- Use `Table` for dense admin lists.
- Use `FieldGroup` and `Field` for forms.
- Use `InputGroup` for search/barcode inputs with addons.
- Use `Dialog`, `Sheet`, and `AlertDialog` for overlays and confirmations.
- Use `Empty`, `Skeleton`, `Spinner`, and `sonner` for feedback states.
- Use lucide icons in buttons.
- Use semantic colors and variants instead of raw color utility styling.
- Avoid nested cards and marketing-style hero composition.

## Testing Strategy

Backend/Rust tests:

- migrations run on an empty SQLite database,
- `completeSale` inserts sale, items, payments, inventory movements, and updates stock,
- sale completion fails without an open shift,
- sale completion fails with insufficient stock,
- void/refund creates a linked document and restores stock,
- import validation catches missing fields, invalid prices, unknown VAT rates, and duplicates,
- backup creates a consistent copy.

Frontend/TypeScript tests:

- domain services have stable types and a mock adapter for UI development,
- register flow handles product add, discount, payment, completion, and error states,
- import wizard handles mapping and row errors,
- admin tables show loading, empty, and error states.

Manual acceptance checks:

- open shift,
- sell two products,
- pay with cash and card,
- verify receipt,
- verify stock decreased,
- receive goods,
- view product ledger,
- void/refund and verify stock restored,
- import CSV/XLSX sample,
- create backup,
- restore into a test database.

## Implementation Notes

The first implementation plan should start with the backend foundation:

1. SQLite setup and migrations.
2. Rust domain models and transaction helpers.
3. TypeScript service contracts and local adapter.
4. Auth/users/shifts.
5. Catalog and inventory.
6. Register and sale completion.
7. Receipt search and void/refund.
8. Reports.
9. Import wizard.
10. Backup/restore.
11. UI polish and shadcn block-based app shell.

This order keeps high-risk data consistency work ahead of UI breadth.
