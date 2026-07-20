# KEP Year-End Close (SW-9c) — Design

**Date:** 2026-07-21
**Status:** Approved (closed-year gate; irreversible close)
**Legal authority:** `docs/KEP-VERIFIED-RULES.md` §5 (year-end freeze, tamper, print, retention). This spec encodes that memo.
**Builds on:** SW-9a (`kep_entries`, `list_ledger` saldo, `book_year_of`, the `opening` kind), SW-9b (kalkulacija + storno posting fns), SW-8 print primitive, SW-6c HTML renderer, company settings.

## Scope

The final piece of the KEP: year-end **zaključivanje** with a čl. 18 data-layer lock, the krajnji-saldo **carry-forward**, the signable electronic-close print and the full-book **paginated** print (DONOS / SVEGA ZA PRENOS page mechanics), and the **5-year retention** clock.

## Confirmed decisions

| Decision | Choice |
|---|---|
| čl. 18 lock | **Closed-year gate** — a closures table + a gate on every posting path. Combined with the existing append-only ledger, a closed year cannot be altered by the application (prevention). |
| Close reversibility | **Irreversible.** No re-open command; a typed confirmation (`ZAKLJUČI KNJIGU`) guards accident; go-live reset is the pre-production escape hatch. |
| Carry-forward | **Computed, not a stored row.** The opening balance of year Y is read from the prior year's closure (`kep_closures[Y−1].krajnji_saldo`), not inserted as a late `opening` entry — which would land with a wrong redni broj, since Y is already trading by the time Y−1 is closed. |
| Pre-close popis | **Light.** A close-preview shows the book saldo; the shop reconciles a physical count and books differences via 9b's popis-višak/manjak *before* closing. A full per-article popis session is SW-16 (separate P2). |

## 1. Schema — migration v14

Count assertion 13 → 14. `kep_closures` joins `CORE_TABLES`; its index joins `EXPLICIT_INDEXES`.

```sql
CREATE TABLE kep_closures (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_year INTEGER NOT NULL UNIQUE,
    krajnji_saldo_minor INTEGER NOT NULL,
    entry_count INTEGER NOT NULL,
    closed_at TEXT NOT NULL,                -- RFC3339
    closed_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_kep_closures_year ON kep_closures(book_year);
```

Go-live reset (`reset_trading_data`) also `DELETE FROM kep_closures` (alongside the existing `kep_entries` wipe).

## 2. Carry-forward — `list_ledger` starts from the prior closure

`list_ledger(conn, book_year)` (9a) currently starts `saldo = 0`. Change it to start from the **carry-in**: `carry_in = SELECT krajnji_saldo_minor FROM kep_closures WHERE book_year = book_year − 1`, else 0. The returned `KepLedger` gains `opening_saldo_minor` (the carry-in) so the UI and print can show the DONOS. Each year's ledger is then self-consistent: opening carry-in + this year's entries. (The `opening` kind stays available for a one-time initial-stock entry at first go-live; it is not used for the year-to-year carry.)

## 3. Close + the čl. 18 gate (`src-tauri/src/kep_close.rs`)

```rust
pub const CLOSE_CONFIRMATION: &str = "ZAKLJUČI KNJIGU";
pub fn is_year_closed(conn: &Connection, book_year: i64) -> Result<bool, AppError>;
pub fn ensure_year_open(conn: &Connection, book_year: i64) -> Result<(), AppError>;   // rejects if closed
pub fn close_year(conn: &mut Connection, book_year: i64, confirmation: &str, acting: i64, now: &str) -> Result<KepClosure, AppError>;
```

- **`close_year`** — reject unless `confirmation == CLOSE_CONFIRMATION` (`validation`); reject if already closed (`invalid_state`, „Godina je već zaključena."); in one transaction compute `krajnji_saldo = list_ledger(book_year).saldo_minor` (which already includes the carry-in), count the year's entries, insert the `kep_closures` row. **No entry is inserted** (carry-forward is computed, §2). Irreversible — there is no `reopen`.
- **`ensure_year_open`** — the gate. Rejects a closed year with `AppError::business("year_closed", "Knjiga za {book_year}. godinu je zaključena i ne može se menjati.")`.

**Wire the gate into every posting path** (each computes its target `book_year` and calls `ensure_year_open` first, inside its transaction): `post_receipt_zaduzenje` + `create_kalkulacija` (the receipt hook), `post_daily_sales`, `post_value_storno`, `post_nivelacija`, `correct_entry`. Combined with the ledger being append-only (no UPDATE/DELETE of a posted row anywhere), a closed year is fully immutable at the application layer — the čl. 18 prevention.

## 4. Documents (via SW-8)

Both are self-contained HTML (SW-6c renderer idioms: `<!doctype html>`, inline `<style>`, `escape_html`, `format_rsd_minor`, no `<script>`, no QR/PIB-as-fiscal-block; non-fiscal footer). Header block from company settings: trgovac (shop_name), PIB, prodajni objekat/mesto (address), „KNJIGA EVIDENCIJE PROMETA ZA {year}. GODINU". Footer „M.P. ____ ODGOVORNO LICE ____".

- **Signable close** (`render_close_html`, čl. 17 st. 4): „Zaključenje knjige za {year}" — the opening carry-in (početno stanje), the year's zaduženje/razduženje totals, the **krajnji saldo**, and a signature line. This is the document the preduzetnik prints and signs.
- **Full-book paginated print** (`render_book_html`, §5.5): all the year's entries in the 5-column table, **grouped into pages of N rows** (e.g. 30), each page opening with a **DONOS** (running carry-in) row and closing with a **SVEGA ZA PRENOS** (running carry-out) row, numbered pages (`Strana {k}`), per-page column subtotals. CSS `page-break-after` between page groups so a browser print paginates cleanly. The first page's DONOS = the year's opening carry-in (§2).

## 5. Retention (§5.6)

`purge_eligible(closure, today) = 5 years from the LATER of closed_at / 31 Dec of book_year ≤ today` (a floor). Surfaced on the closures list; **nothing auto-deletes**.

## 6. Commands (`commands/kep.rs`, all `require_admin`, acting from session, now from `utc_now()`)

`kep_close_preview(book_year) -> { krajnji_saldo_minor, entry_count, already_closed }`; `kep_close_year(book_year, confirmation) -> KepClosure`; `kep_list_closures() -> Vec<KepClosureView>` (with `purge_eligible`); `kep_export_close(book_year) -> ExportedFile`; `kep_export_book(book_year) -> ExportedFile` (both write HTML to `exports/`, reusing the campaigns write helper). Register in `lib.rs`.

## 7. UI

The KEP module gains: a **„Zaključi godinu"** action (a dialog showing the krajnji saldo that will carry forward + the typed `ZAKLJUČI KNJIGU` confirmation); **„Štampaj zaključenje"** + **„Štampaj celu knjigu"** print buttons (SW-8 export-then-open); a **closed-year badge** on the ledger view with all posting/adjustment actions **hidden/disabled** for a closed year (mirroring the gate); and a **closures list** with the `purge_eligible` retention indicator. The ledger view shows the opening carry-in (Početno stanje) row.

## 8. Testing

- **Carry-forward:** close 2026 with saldo 15.460 → `list_ledger(2027).opening_saldo_minor == 1546000`, and 2027's running saldo starts there.
- **The gate:** after closing 2026, `post_receipt_zaduzenje`/`post_daily_sales`/`post_value_storno`/`post_nivelacija`/`correct_entry` targeting 2026 all reject with `year_closed`; the same into 2027 succeed.
- **Close:** wrong confirmation → `validation_error`; double close → `invalid_state`; the closure records the exact `list_ledger` saldo + entry count; no `kep_entries` row is inserted by the close.
- **Reset:** go-live reset clears `kep_closures`.
- **Retention:** `purge_eligible` flips at `31 Dec of book_year + 5y` (and never earlier than `closed_at + 5y`).
- **Print:** the full-book HTML paginates (DONOS + SVEGA ZA PRENOS rows, „Strana 1", per-page subtotals) and the close HTML carries the krajnji saldo + signature line; both non-fiscal, escaped.

## Non-goals
- Full per-article popis session (SW-16).
- Re-opening a closed year (irreversible by design).
- Campaign→KEP auto-nivelacija; PDV-rate revaluation (prior flagged follow-ups).

## Acceptance criteria
- A year can be closed (irreversibly, typed-confirmed); its krajnji saldo carries forward as the next year's computed opening.
- After close, no posting path can add to, change, or remove any entry in that year — čl. 18 prevention.
- The signable close and the paginated full-book print render with the statutory page mechanics.
- Retention is a 5-year floor eligibility flag; nothing auto-deletes; reset clears closures.
- All gates green: `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `git diff --check`.
