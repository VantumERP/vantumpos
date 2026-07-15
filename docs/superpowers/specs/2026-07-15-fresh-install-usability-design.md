# Fresh-Install Usability — Design

**Date:** 2026-07-15
**Status:** Approved design, ready for implementation plan
**Scope tier:** T0-B gaps 0.4 (no seeded VAT rate) and 0.5 (Serbian text unsearchable / wrong item rings up) in `docs/COMPETITIVE-GAP-ANALYSIS.md`.
**Sequenced:** Part A (VAT seed) first — it is the chronological gate (no product can be created without a tax rate) — then Part B (search).

## Problem

Two defects make a fresh install unusable and unsafe on day one:

1. **No seeded VAT rate (0.4).** The only production seed is the admin user (`db/mod.rs:49` `seed_initial_admin`); `products.tax_rate_id` is `NOT NULL REFERENCES tax_rates(id)` (`migrations.rs:73`). On a fresh DB **no product can be created** and every CSV import row fails, with no guidance. It looks fine in dev only because `mock-adapter.ts` pre-seeds rates.
2. **Serbian text unsearchable + wrong item rings up (0.5).** `list_products_for_connection` builds a Unicode-lowercased pattern (`catalog.rs:598`) but compares it against SQLite's **ASCII-only** `LOWER()` (`catalog.rs:633-635`). `lower('KOŠULJA')` = `'koŠulja'`, so an ALL-CAPS imported name is unfindable, and anything with š/č/ć/ž/đ typed lowercase never matches. The register's only path to the cart then blindly adds `result.items[0]` (`RegisterScreen.tsx:161`), so an ambiguous query rings up the **wrong product at the wrong price**.

## Decisions (locked with the founder)

| Decision | Choice |
|---|---|
| VAT seed UX | **One-question first-run prompt** — *„Da li ste u PDV sistemu?"* seeds 20%+10% (Da) or a single 0% rate (Ne). |
| Search folding | **Shared Rust `fold_text()` registered as a SQL `fold()` scalar function** on every connection. No migration, no normalized column. |
| Register safety | Exact barcode/SKU match → add; single fuzzy match → add; multiple → **disambiguation picker**; none → not-found. Reuses the folded search; no new backend command. |
| Folding scope | Serbian **Latin** case + diacritics only (š→s, ž→z, č→c, ć→c, đ→dj). Cyrillic transliteration out of scope. |
| Modal placement | A dedicated `FirstRunTaxSetup` component rendered in AppShell's authenticated branch (AppShell is already 1281 lines — do not grow it). |

## Part A — VAT/PDV first-run seed

### Backend
- New command `settings_seed_tax_rates(inVatSystem: bool) -> Vec<TaxRate>`, `require_admin`-gated (consistent with other settings commands).
  - Idempotent: if `tax_rates` is non-empty, insert nothing and return the current rates.
  - If empty and `inVatSystem`: insert `PDV 20%` (2000 bp) and `PDV 10%` (1000 bp).
  - If empty and not `inVatSystem`: insert `Bez PDV-a` (0 bp).
  - Returns the resulting rate list.
- Reuse the existing insert path/shape from `save_tax_rate` (`settings.rs`).

### Frontend
- New component `src/app/FirstRunTaxSetup.tsx`:
  - Props: `settings` service + `role`.
  - On mount, only when `role === "admin"`: call `listTaxRates()`. If the list is empty, open a **blocking** `AlertDialog` (no dismiss-without-answer) with the single question *„Da li ste u PDV sistemu?"* and two actions: **Da** and **Ne**.
  - On an answer: call `seedTaxRates(answer === "Da")`, then close. On error, show the message inline and keep the dialog open.
  - Renders `null` when not admin, still loading, or rates already exist.
- Render `<FirstRunTaxSetup settings={…} role={session.user.role} />` inside AppShell's authenticated branch (after `const session = sessionState.session;`).
- Service wiring: add `seedTaxRates(inVatSystem: boolean): Promise<TaxRate[]>` to the settings port (`ports.ts`), the local adapter (`local-adapter.ts`, command `settings_seed_tax_rates`), and the mock adapter.

> The default active screen is `register`, not `catalog`, so by the time the admin navigates to Catalog to add a product the rates already exist — no cross-component refresh needed.

## Part B — Serbian search correctness + register safety

### Backend — `fold_text` + `fold()` SQL function
- Enable the rusqlite `functions` feature: `rusqlite = { version = "0.32", features = ["bundled", "backup", "functions"] }`.
- Add `pub fn fold_text(input: &str) -> String` (in `catalog.rs` or a small shared module): Unicode-lowercase, then map Serbian Latin diacritics — š→s, ž→z, č→c, ć→c, đ→dj (uppercase handled by lowercasing first). Output is lowercase ASCII-foldable text.
- In `Db::open()` (`db/mod.rs:33`), after opening the connection, register a deterministic scalar function:
  ```rust
  connection.create_scalar_function(
      "fold", 1,
      FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
      |ctx| Ok(fold_text(&ctx.get::<String>(0)?)),
  )?;
  ```
  (`use rusqlite::functions::FunctionFlags;`)
- In `list_products_for_connection`:
  - Build the pattern with `fold_text`: `format!("%{}%", fold_text(value))`.
  - Change the `WHERE` search clause to `fold(p.name) LIKE ?1 OR fold(p.sku) LIKE ?1 OR fold(COALESCE(p.barcode, '')) LIKE ?1`.
- Both sides are folded identically, so ASCII `LIKE` matches correctly; exact barcode/SKU lookups keep working (folding is idempotent on plain ASCII/digits).

### Frontend — register scan/disambiguation
Replace the `result.items[0]` logic in `handleSearchSubmit` (`RegisterScreen.tsx:147-170`):
1. `const q = query.trim()` (already have `query`).
2. `searchProducts({ search: q, active: true, limit: 20 })`.
3. Resolve:
   - **Exact match:** if any item's `barcode` or `sku` equals `q` case-insensitively → `addProduct(thatItem)` (scanner/SKU fast path; barcodes are UNIQUE so this is unambiguous).
   - **Single result:** exactly one item → `addProduct(items[0])`.
   - **Multiple results:** show a disambiguation picker listing the matches (name, price via `formatRsd(salePriceMinor)`, stock via `formatQuantity(currentStockMilli)`); clicking a row calls `addProduct` and closes the picker.
   - **No results:** set message „Artikal nije pronađen."
4. Clear the search box after a successful add.

The picker is a small list rendered under/near the search (reuse existing primitives — a `Command`/list or a simple bordered list of buttons). Keyboard-first: the first row is focusable so Enter on a single narrowed result is fast.

## Non-goals (explicit)
- **Serbian collation for sort order** (Š/Č sorting after Z) — cosmetic; `ORDER BY p.name` keeps BINARY collation.
- **Cyrillic search / transliteration** — Latin only.
- **Out-of-stock override (0.6)** and **installer/WebView2 (0.7)** — the next chunk.
- **Receipt search folding** (`receipts.rs` `lower(...)`) — separate surface; not touched here.
- No change to the 500-row product list cap or paging (tracked separately).

## Test plan (TDD — write red first)

**Rust (`catalog.rs`, `db/mod.rs`, `settings.rs` test modules):**
1. `fold_text`: `"KOŠULJA"` → `"kosulja"`; `"Đầ"`-style → `đ`→`dj`; `"ŽČĆ"` → `"zcc"`; plain ASCII/digits unchanged.
2. Search finds a product named `"KOŠULJA"` when the query is `"kosulja"`, `"KOSULJA"`, and `"košulja"`.
3. Search still finds a product by exact barcode and by SKU.
4. `seed_tax_rates(true)` on an empty DB creates PDV 20% + 10%; `seed_tax_rates(false)` creates one 0% rate; a second call is a no-op (no duplicates) and returns the existing rates.
5. `settings_seed_tax_rates` rejects a non-admin session (`forbidden`).

**Frontend (`RegisterScreen.test.tsx`, `FirstRunTaxSetup.test.tsx`, `local-adapter.test.ts`):**
6. Typing a query with multiple matches renders the picker; clicking a row adds that product; a single match adds directly; no match shows „Artikal nije pronađen."
7. An exact barcode among multiple fuzzy matches adds the exact product, not `items[0]`.
8. `FirstRunTaxSetup`: with an admin session and empty rates, the dialog shows; clicking **Da** calls `seedTaxRates(true)`; **Ne** calls `seedTaxRates(false)`; with existing rates it renders nothing; with a cashier session it renders nothing.
9. Adapter maps `seedTaxRates` → `settings_seed_tax_rates` with `{ inVatSystem }`.

## Acceptance criteria
- On a fresh DB, an admin is prompted once for PDV status, after which products can be created and CSV import works.
- Searching any case/diacritic variant of a Serbian product name finds it; a scanned barcode resolves to exactly the right product; an ambiguous text query never silently rings up the wrong item.
- `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `git diff --check` all pass.
