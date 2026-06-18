# Shared Foundation Spec

Status: Required context for all module chats

## Current Baseline

The current app has:

- React 19, Vite, TypeScript, Tailwind v4, shadcn/ui `base-mira`.
- Tauri v2 with Rust 2021.
- SQLite through `rusqlite` and an initial migration.
- A local backend health command.
- TypeScript service ports with local and mock adapters.
- A shadcn sidebar shell with navigation entries.

The current UI is only a shell. Follow-up work must replace generic module content with real screens.

## Shared Architecture Rules

React screens depend on TypeScript service interfaces. They must not call Tauri `invoke` directly from screen components.

Use this layering:

- `src/app/...`: page-level module components and shell composition.
- `src/services/ports.ts`: service interfaces.
- `src/services/types.ts`: shared frontend DTOs.
- `src/services/local-adapter.ts`: maps service methods to Tauri commands.
- `src/services/mock-adapter.ts`: deterministic test/dev data.
- `src-tauri/src/commands/...`: Tauri command handlers.
- `src-tauri/src/db/...`: SQLite wrapper, migrations, and query helpers.

Use stable command names in snake case, grouped by module:

- `auth_login`
- `shift_open`
- `catalog_search_products`
- `inventory_receive`
- `sales_complete`
- `receipts_search`
- `reports_daily_turnover`
- `import_validate`
- `settings_update_company`
- `backup_create`

## Data And Units

Money is stored as integer minor units. Use `formatRsd` and `parseRsdInput` on the frontend and integer math on the backend. Never use floating point for money.

Quantities are stored as milli-units. Example: `1 kom` is `1000`, `0.25 kg` is `250`. UI can display normal decimals, but backend stores integer milli values.

Dates should be stored as ISO-like text in SQLite. Use backend-created timestamps for persisted records.

## SQLite Rules

All schema changes after the current migration must be new migrations. Do not edit the initial migration after it has landed on `master`.

State-changing commands must use one transaction per use case. The transaction boundary belongs in Rust, not React.

Important existing tables:

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

If a module needs fields not present in the initial schema, add a new migration and a regression test that proves old databases migrate forward.

## Error Contract

Every command error should serialize:

```ts
interface CommandError {
  code: string
  message: string
  details?: unknown
}
```

Use stable error codes so the UI can branch without string parsing. Examples:

- `validation_error`
- `not_found`
- `shift_required`
- `insufficient_stock`
- `duplicate_sku`
- `duplicate_barcode`
- `payment_mismatch`
- `import_row_errors`
- `backup_failed`

Messages shown to operators must be Serbian Latin and operational. Technical cause can go in logs or details.

## UI Rules

The product is an operational desktop app, not a landing page.

Use shadcn components that are already installed:

- `Sidebar` for shell navigation.
- `Table` for lists.
- `FieldGroup`, `Field`, `Input`, `InputGroup`, `Select`, `Switch`, `Checkbox`, `Textarea` for forms.
- `Dialog`, `Sheet`, and `AlertDialog` for overlays and destructive confirmations.
- `Tabs` for module subviews.
- `Command` for product search or command palette style selection.
- `Chart` for reports where a chart is useful.
- `Badge`, `Alert`, `Empty`, `Skeleton`, `Spinner`, `Progress`, and `sonner` for status and feedback.

Each navigation item must render a distinct module screen. It is acceptable to use static fixture data for the first frontend-only pass only if the spec explicitly says so, but the final module acceptance must use service methods.

Avoid:

- generic placeholder cards,
- nested cards,
- marketing hero sections,
- in-app explanatory text about implementation internals,
- raw color utilities when semantic tokens or variants exist,
- direct database or Tauri concepts in visible operator copy.

## Shared Shell Requirements

The shell should eventually show:

- current shop name,
- active user,
- role,
- shift status,
- local database status,
- backup warning if backup is stale or failed.

The sidebar can stay, but clicking a module must update the main panel to that module's real UI. The footer should be backed by shift/auth state, not hard-coded `Admin`.

## Testing Rules

Required gates for module work:

- `bun run test`
- `bun run build`
- `cd src-tauri; cargo test -- --test-threads=1`
- `cd src-tauri; cargo clippy --all-targets --all-features --locked -- -D warnings`
- `cd src-tauri; cargo fmt --check`
- `git diff --check`

Frontend tests should use the mock service adapter and assert user-visible behavior. Backend tests should use temp SQLite databases and verify persisted rows, constraints, and transaction rollback for failure paths.

