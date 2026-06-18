# Module Spec: Import Wizard

## Goal

Implement Import for migrating clients from other POS systems. This is important for adoption: users should be able to import products, categories, and initial stock with validation before writing anything.

## Scope

In scope:

- CSV import for products, categories, and initial stock.
- XLSX import if dependency choice is approved in the implementation chat.
- Header detection.
- Column mapping.
- Row validation.
- Dry-run summary.
- Duplicate detection.
- Import history.
- Confirmed write transaction.

Out of scope:

- Automatic import from competitor proprietary databases.
- Supplier data.
- Customer data.
- Scheduled imports.

## Flow

1. Select import type.
2. Select file.
3. Read headers.
4. Map required columns.
5. Map optional columns.
6. Validate rows.
7. Show errors/warnings.
8. Run dry-run summary.
9. Confirm import.
10. Show import result and history.

## UI

Use stepper-like layout with `Tabs` or explicit progress sections. Use `Progress` for validation/import progress if async work is visible.

Use `Table` for row validation results.

Use `FieldGroup`, `Field`, and `Select` for column mapping.

Use `Alert` for row-error summary and `AlertDialog` for final import confirmation.

## File Handling

Preferred MVP approach:

- Frontend uses browser file input to read CSV text or XLSX bytes.
- Frontend sends parsed raw content/bytes to Tauri command for backend validation.
- Backend owns validation and database writes.

This avoids broad filesystem permissions for import. If a Tauri file dialog is added, capability permissions must be scoped and documented.

## Service Contract

```ts
interface ImportService {
  readImportHeaders(request: ReadImportHeadersRequest): Promise<ImportHeaders>
  validateImport(request: ValidateImportRequest): Promise<ImportValidationResult>
  commitImport(request: CommitImportRequest): Promise<ImportJobResult>
  listImportJobs(): Promise<ImportJobSummary[]>
  getImportJob(id: number): Promise<ImportJobDetail>
}
```

## Backend Commands

- `import_read_headers`
- `import_validate`
- `import_commit`
- `import_list_jobs`
- `import_get_job`

## SQLite Notes

Existing tables:

- `import_jobs`
- `import_job_rows`
- `products`
- `categories`
- `tax_rates`
- `inventory_balances`
- `inventory_movements`

Import commit should write an import job and row statuses. Product import and initial stock import should happen in a transaction.

## Product Import Rules

Required:

- name,
- sale price,
- VAT rate,
- SKU/code or barcode.

Optional:

- category,
- purchase price,
- minimum stock,
- unit of measure,
- initial stock.

Duplicate matching order:

1. barcode,
2. SKU/code,
3. name as warning only.

Import never deletes existing products.

## Initial Stock Rules

- Product match by barcode or SKU/code.
- Quantity is required.
- Quantity can be positive for initial load.
- Writes inventory movement with type `receive` or `correction`, depending on implementation decision.

## Tests

Backend:

- missing required column fails validation,
- invalid money fails row validation,
- unknown VAT rate fails or warns based on selected policy,
- duplicate barcode maps to update/warning according to policy,
- commit writes products and import job in one transaction,
- failed commit rolls back product writes.

Frontend:

- wizard advances through steps,
- mapping requires required fields,
- row errors render with row numbers,
- commit disabled while errors exist,
- import history renders previous jobs.

## Acceptance Criteria

- Import is a real migration tool, not a file-picker placeholder.
- User can validate a CSV before writing data.
- Commit is transactional.
- Import history is persisted.

## Prompt For Separate Chat

```text
Implementiraj Import wizard for VantumPOS.

Read:
- docs/module-specs/00-shared-foundation.md
- docs/module-specs/07-import.md
- docs/superpowers/specs/2026-06-17-vantumpos-local-pos-design.md

Build CSV product/category/initial-stock import with header mapping, validation, row errors, dry run, commit transaction, import history, service contracts, Tauri commands, Rust tests, frontend tests, and mock adapter data. Discuss XLSX dependency only if needed; do not implement competitor proprietary database import.
```

