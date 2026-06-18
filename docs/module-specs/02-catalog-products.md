# Module Spec: Catalog And Products

## Goal

Implement a real Artikli module for local product management. Cashiers and admins need searchable products with SKU/code, barcode, category, VAT, price, stock settings, and active status.

## Scope

In scope:

- Product list with filters.
- Product create/edit/deactivate.
- Category create/edit.
- VAT rate selection from local settings.
- Duplicate SKU and barcode validation.
- Low stock and missing barcode filters.
- Product search endpoint shared with Kasa.

Out of scope:

- Product variants by size/color.
- Supplier accounting.
- Multi-store pricing.
- Image management.

## Screens

### Product List

Use `InputGroup` for search. Use `Table` for product rows.

Filters:

- category,
- active/inactive,
- low stock,
- missing barcode,
- VAT rate.

Columns:

- name,
- SKU/code,
- barcode,
- category,
- price with VAT,
- VAT,
- current stock,
- minimum stock,
- active,
- actions.

Row actions:

- edit,
- deactivate/reactivate,
- view ledger link to Lager.

### Product Form

Use `Sheet` for product editing on desktop.

Fields:

- name,
- SKU/code,
- barcode,
- category,
- unit of measure,
- sale price with VAT,
- purchase price without VAT,
- VAT rate,
- minimum stock,
- allow negative stock,
- active.

Use `FieldGroup` and `Field`; money inputs use existing money helpers.

### Categories

Can be a tab inside Artikli. Use table plus inline dialog/sheet form.

## Service Contract

```ts
interface CatalogService {
  listProducts(query: ProductListQuery): Promise<ProductListResult>
  searchProducts(query: ProductSearchQuery): Promise<ProductSearchResult[]>
  getProduct(id: number): Promise<ProductDetail>
  createProduct(request: SaveProductRequest): Promise<ProductDetail>
  updateProduct(id: number, request: SaveProductRequest): Promise<ProductDetail>
  setProductActive(id: number, active: boolean): Promise<ProductDetail>
  listCategories(): Promise<CategorySummary[]>
  saveCategory(request: SaveCategoryRequest): Promise<CategorySummary>
}
```

## Backend Commands

- `catalog_list_products`
- `catalog_search_products`
- `catalog_get_product`
- `catalog_create_product`
- `catalog_update_product`
- `catalog_set_product_active`
- `catalog_list_categories`
- `catalog_save_category`

## SQLite Notes

Existing tables:

- `products`
- `categories`
- `tax_rates`
- `inventory_balances`

Add migration only if implementation needs:

- normalized SKU search column,
- product notes,
- category active status changes beyond current schema.

Product list should join current balance from `inventory_balances`.

## Business Rules

- Product name is required.
- SKU/code is required and unique.
- Barcode is optional but unique when present.
- Sale price and purchase price cannot be negative.
- VAT rate must reference an active tax rate.
- Deactivation hides product from Kasa search but keeps receipt history intact.
- Product updates do not change historical `sale_items` snapshots.

## Tests

Backend:

- create product succeeds,
- duplicate SKU fails with `duplicate_sku`,
- duplicate barcode fails with `duplicate_barcode`,
- inactive product does not appear in sale search by default,
- product list includes current stock.

Frontend:

- product list renders table and filters,
- product form validates required fields,
- duplicate backend error is shown next to the right field,
- inactive toggle updates row state.

## Acceptance Criteria

- Clicking Artikli shows a real catalog screen, not a placeholder.
- User can create and edit a product.
- Kasa can later reuse `searchProducts`.
- Existing tests and build pass.

## Prompt For Separate Chat

```text
Implementiraj Catalog/Artikli module for VantumPOS.

Read:
- docs/module-specs/00-shared-foundation.md
- docs/module-specs/02-catalog-products.md
- docs/superpowers/specs/2026-06-17-vantumpos-local-pos-design.md

Build a real Artikli screen with product table, filters, product sheet form, category support, local Tauri commands, Rust SQLite tests, TS service contracts, mock adapter data, and frontend tests. Do not implement register sale completion in this chat.
```

