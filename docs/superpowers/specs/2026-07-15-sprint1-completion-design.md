# Sprint 1 Completion — Design

**Date:** 2026-07-15
**Status:** Approved design, ready for implementation plan
**Scope:** The remaining Sprint 1 ("app works at all") items from `docs/COMPETITIVE-GAP-ANALYSIS.md` — gaps 0.6, 0.7, 1.8, 1.9, 1.10, 1.11, 1.12. (VAT seed 0.4 and Serbian search 0.5 already merged.)
**Basis:** Per-item scoping in the `sprint1-remainder-scoping` workflow (exact file:line maps).

## Locked decisions

| # | Decision | Choice |
|---|---|---|
| App identity (one-way door) | identifier + productName | **`com.actaer.vantumpos`**, productName **"VantumPOS"** |
| WebView2 delivery | install mode | **offlineInstaller** (~+150MB, truly offline) |
| Diacritics sweep | scope | **Full sweep now**, executed LAST as one atomic commit |
| Out-of-stock override | mechanism | **Both**: shop-level `allowOverselling` setting (default **false**) + one-click cashier confirm at the till |
| Oversell approval | who | **Cashier one-click** (no admin/PIN gate; revisit when per-sale roles exist) |
| Version display | where | **Sidebar footer** (`Verzija x.y.z`, visible to cashiers) |
| Importer encoding | strategy | **Auto-detect** (UTF-8-fatal → Windows-1250 fallback) **+ a selector** on the preview to override |
| Percent discount | input | **Suffix parse** — cashier types `20%` in the discount field (amount if no `%`) |
| Go-live reset | behavior | **Full wipe** (zero balances) + **auto-backup first** + **typed confirmation** |

## Item A — Installer / window / single-instance / WAL (gap 0.7)

Not unit-testable (bundle/config); verify by `cargo build` + config inspection.
- `src-tauri/tauri.conf.json`: set `identifier` = `com.actaer.vantumpos`, `productName` = `VantumPOS`; add `bundle.windows.webviewInstallMode = { type: "offlineInstaller" }`; set the main window `label:"main"` (keep — capabilities pin `["main"]`), `minWidth: 1024`, `minHeight: 720`, `width: 1280`, `height: 800`, `title: "VantumPOS"`.
- `src-tauri/Cargo.toml` + `src-tauri/src/lib.rs`: add `tauri-plugin-single-instance` and register it **first** in the builder (before other plugins), focusing the existing `"main"` window on second launch.
- `src-tauri/src/db/mod.rs` `open()`: add `PRAGMA journal_mode=WAL;` to the `execute_batch`. (Test DB names are nanosecond-unique, so `-wal`/`-shm` sidecar residue is harmless.)

## Item B — Logging / panic hook / version (gap 1.8)

- `src-tauri/Cargo.toml` + `src-tauri/src/lib.rs`: add `tauri-plugin-log` writing to the OS app-log dir; register it before `.setup`. Install `std::panic::set_hook` at the very top of `run()` (chain the default hook so stderr still fires).
- Frontend: new `src/app/AppVersion.tsx` — reads `settings.getHealth()` and renders `Verzija {appVersion}`; render it in the AppShell sidebar footer near the user block. Unit-testable (component renders the version from a mocked health).
- Update `src/App.test.tsx` if any footer-copy assertion is affected.

## Item C — Importer encoding (gap 1.9)

- New `src/lib/encoding.ts`: `decodeCsv(bytes: ArrayBuffer, encoding?: "utf-8" | "windows-1250"): { text, encoding }` — if no encoding given, try `TextDecoder("utf-8", { fatal: true })`, on throw fall back to `TextDecoder("windows-1250")`; return the text and the encoding used. Unit-tested in `src/lib/encoding.test.ts` with cp1250 bytes for "Košulja".
- `src/app/import/ImportWizard.tsx`: read the file as `ArrayBuffer` (keep the bytes in state), decode via `decodeCsv`, and add a small encoding `<NativeSelect>` (Automatski / UTF-8 / Windows-1250) on the preview/mapping card that re-decodes from the retained bytes. The Rust importer and the `csvText: string` IPC contract are unchanged.

## Item D — Register ergonomics (gap 1.10)

- `src/app/register/RegisterScreen.tsx`: change the discount parser so a trailing `%` produces `{ type: "percent", basisPoints }` (e.g. `"20%"` → 2000 bp) and a bare number stays `{ type: "amount", amountMinor }`; apply to both line and receipt discount inputs; add placeholder hint `20 ili 20%`. The in-file **test preview mock must be taught to compute percent** (it currently only reads `type === "amount"`), or a green test masks a real bug.
- Prefill the cash tender field to the preview total via a `cashTouched` guard (don't clobber a manual edit; reset on sale-complete). Existing tests that type into cash must `clear()` first.
- Backend/contract unchanged (`DiscountRequest::Percent` already honored).

## Item E — Out-of-stock override (gap 0.6)

- `src-tauri/src/commands/settings.rs`: add a `sales` settings key mirroring the receipt-settings shape — `SalesSettings { allowOverselling: bool }` (default false) + `SalesSettingsRequest`, `settings_get_sales` / `settings_update_sales` (update is `require_admin`), `load_sales_settings` / `save_sales_settings`, and `pub(crate) const SALES_SETTINGS_KEY`.
- `src-tauri/src/commands/sales.rs`: `validate_stock` gains an `allow_overselling: bool` param; guard becomes `!allow_negative && !allow_overselling && current < required`. `CompleteSaleRequest` gains `#[serde(default)] allow_stock_override: Option<bool>` (camelCase `allowStockOverride`). In `complete_sale_transaction`, read the shop setting **inside the sale `tx`** (mirror `take_next_receipt_number`'s in-tx settings read — not `load_json_setting`, which opens a fresh connection), compute `overselling = shop_setting || allow_stock_override.unwrap_or(false)`, pass into `validate_stock`. Preview path stays non-blocking (it never calls `validate_stock`).
- `src-tauri/src/lib.rs`: register `settings_get_sales` + `settings_update_sales`.
- Frontend: add `SalesSettings`/`SalesSettingsRequest` types, `getSalesSettings`/`updateSalesSettings` to the port + local adapter + mock. `SettingsScreen.tsx`: a `Switch` (label „Dozvoli prodaju ispod stanja") wired to `updateSalesSettings`. `RegisterScreen.tsx`: in the completeSale catch, when `code === "insufficient_stock"`, open a confirm `AlertDialog` (name the product/shortfall from `error.details`, actions „Odustani" / „Ipak prodaj"); on confirm re-invoke `completeSale({ ...draft, payments, allowStockOverride: true })`.

## Item F — Go-live reset (gap 1.11)

- `src-tauri/src/commands/backup.rs` (or a new `reset` module): admin-gated `reset_trading_data(confirmationText)` requiring exact typed confirmation (mirror `restore_backup`). It first creates a safety backup via the existing `create_backup` (reuse an already-allowed `backup_type` — no new migration), then in **one FK-safe transaction**: `DELETE FROM sales` (cascades sale_items + sale_payments), `DELETE FROM inventory_movements` (no FK cascade — explicit), `DELETE FROM shifts` (**after** sales, since sales.shift_id → shifts), `UPDATE inventory_balances SET quantity_milli = 0`, and reset the `receipt_numbering` setting's `next_sequence_number` to 1. Catalog / users / settings / import history / backup history are **kept**.
- `src-tauri/src/lib.rs`: register the command.
- Frontend: types + port + adapter + mock `resetTradingData`; a `SettingsScreen` control with a typed-confirmation dialog (like restore); after success, re-fetch session/shift (open shift is now gone).

## Item G — Diacritics + locale sweep (gap 1.12) — LAST

- Replace every user-facing Serbian string in `src/` and `src-tauri/` with correct Latin diacritics (šđčćž) — per-string-literal edits, **not** a repo-wide sed (must not touch `text.rs`'s fold map/fixtures, importer CSV header fixtures, or English identifiers/keys).
- Normalize locale helpers to `sr-Latn-RS` consistently (`format.ts`, `money.ts`, `chart.tsx`).
- Update every test assertion that exact-matches a Serbian string (`src/App.test.tsx`, `RegisterScreen.test.tsx`, `ReceiptsScreen.test.tsx`, etc.) **in the same commit**, and align `mock-adapter.ts` messages with backend messages (frontend tests assert against the mock).
- Executed last so it also corrects any ASCII strings introduced by items A–F.

## Sequencing (shared files)

Execute functional items first, diacritics last:
**A (installer) → B (logging) → C (encoding) → D (ergonomics) → E (oversell) → F (reset) → G (diacritics sweep).**
Rationale: `lib.rs` (A,B,E,F), `RegisterScreen.tsx` (D,E,G), `SettingsScreen.tsx` (E,F,G), `types.ts`/adapters (E,F), and `Cargo.toml` (A,B) are shared — sequential avoids conflicts. G touches nearly everything and rewrites test assertions, so it must be the final atomic step.

## Non-goals (explicit)

- Cyrillic UI / transliteration (Latin only — matches `fold_text`).
- Admin/PIN gate on the per-sale oversell (deferred until per-sale roles exist).
- Moving CSV decoding into Rust / `encoding_rs` (TS `TextDecoder` fallback is sufficient and non-invasive).
- A dedicated `pre_reset` backup_type + migration (reuse an existing type).
- Surgical stock-preserving reset, open-price/"razno" line, discount-toggle UI, preview debounce, About dialog — all future.
- Clearing import/backup history on reset (not trading data — kept).

## Test plan (per item)

- **A:** `cargo build` succeeds; `cargo test` db suite still green (WAL is a pragma, not a migration — count stays 6); config inspected. Single-instance/webview not testable on macOS host.
- **B:** `AppVersion.test.tsx` renders `Verzija 0.1.0` from mocked health; App.test.tsx footer assertions still green; `cargo clippy -D warnings` clean.
- **C:** `encoding.test.ts` — cp1250 bytes for "Košulja" decode correctly under auto and explicit `windows-1250`, and valid UTF-8 stays UTF-8.
- **D:** register tests — `20%` on a line yields the percent-discounted preview total; cash field prefills to total and computes change; manual cash edit not clobbered.
- **E:** sales.rs — oversell rejected by default; allowed with `allowStockOverride`; allowed when shop setting on; per-product `allow_negative_stock` still works; existing `complete_sale_transaction_rolls_back_when_stock_is_insufficient` stays green. Frontend — the insufficient-stock dialog appears and retries with the override; `settings_update_sales` rejects a cashier.
- **F:** `reset_trading_data` clears sales/movements/shifts + zeros balances + resets the counter, keeps catalog/users/settings, requires exact confirmation, and writes a safety backup; wrong confirmation text rejects and changes nothing.
- **G:** full `bun run test` + `cargo test` green after the atomic string+assertion update; a smoke check that `Račun`/`Košulja`-type strings still fold and search.

## Acceptance criteria

- The Windows installer works offline with a real product identity; a second launch focuses the running instance; SQLite runs in WAL.
- Panics and runtime logs land in the OS log dir; the app version is visible in the shell.
- A Windows-1250 CSV imports without mojibake, with a manual encoding override available.
- A cashier can enter a `20%` discount and the cash field prefills to the total.
- An out-of-stock sale is a one-click cashier decision (or a shop-wide setting), never a dead-end.
- An admin can wipe practice data on go-live morning, keeping the catalog, with a safety backup and typed confirmation.
- The UI reads in correct Serbian (šđčćž) throughout.
- All gates pass: `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy … -D warnings`, `cargo fmt --check`, `git diff --check`.
