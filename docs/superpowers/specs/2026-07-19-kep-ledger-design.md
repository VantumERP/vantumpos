# KEP Value Ledger + Auto-Postings (SW-9a) — Design

**Date:** 2026-07-19
**Status:** Approved (first of the KEP sub-cycles; 9a/9b/9c decomposition confirmed)
**Legal authority:** `docs/KEP-VERIFIED-RULES.md` (adversarially verified 19.07.2026; 36/41 load-bearing rules survived). That memo is the authority for every column, basis, and rule below.
**Builds on:** the existing `inventory_receive`/`apply_inventory_adjustment` transaction, the `sales` ledger (daily total), the append-only + admin-gating patterns, and (later) the SW-8 print primitive.

## Scope

**SW-9a only:** the append-only 5-column KEP value ledger and its automatic postings.
- Zaduženje (kolona 4) auto-posted from goods receipts at **retail value with PDV**.
- Razduženje (kolona 5) posted per trading day from VantumPOS's daily sales total, **with a manual override**.
- Monotonic redni broj per book-year; derived running saldo; dan.mesec display; opis composition; T+1 overdue-posting warning; sell-after-entry status.

**Deferred to later sub-cycles (out of scope here):**
- **SW-9b:** the 14-element kalkulacija generator (the formal isprava that justifies each zaduženje), the typed cause→column **storno engine** (nivelacija, supplier/customer returns, popis višak/manjak, reversing-storno error correction), basis-document capture.
- **SW-9c:** year-end zaključivanje (pre-close popis reconciliation, the čl. 18 data-layer immutability lock, krajnji-saldo carry-forward, signable electronic-close print, full-book/date-range print with DONOS/SVEGA page mechanics, the 5-year retention clock).

## Confirmed decisions

| Decision | Choice |
|---|---|
| Decomposition | 9a ledger+postings → 9b kalkulacija+storno → 9c year-end+print. This cycle = 9a. |
| Razduženje source | **Auto from VantumPOS's daily sales total**, with a **per-day manual override** for when the certified ESIR's dnevni izveštaj differs (the fiscal figure legally governs; the override records it was set manually). |
| Staging boundary | 9a auto-posts a **provisional** zaduženje at qty × retail. The formal 14-element kalkulacija isprava (čl. 29) arrives in 9b. The ledger is usable now; fully čl.-29-compliant after 9b. |
| `kind` enum | The **full anticipated set** (9a + 9b + 9c kinds) goes in the v12 CHECK now, so 9b/9c never rebuild the table. |
| Optionality | Non-optional. KEP is unconditional for a retail preduzetnik — no toggle, no paušal/wholesale gate (memo §6). |

## 1. Schema — migration v12

Count assertion 11 → 12. `kep_entries` joins `CORE_TABLES`; indexes join `EXPLICIT_INDEXES`.

```sql
CREATE TABLE kep_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_year INTEGER NOT NULL,
    redni_broj INTEGER NOT NULL,                 -- monotonic, gap-free, per book_year
    entry_date TEXT NOT NULL,                    -- RFC3339 booking timestamp (kolona 2 shows dan.mesec)
    document_date TEXT,                          -- RFC3339 date of the source isprava (kolona 3); NOT the booking date
    opis TEXT NOT NULL,                          -- kolona 3: document naziv/broj/datum (+ dobavljač on a nabavka)
    kolona TEXT NOT NULL CHECK (kolona IN ('zaduzenje', 'razduzenje')),
    amount_minor INTEGER NOT NULL,               -- signed; a crveni storno (9b) is negative
    kind TEXT NOT NULL CHECK (kind IN (
        'opening', 'receipt', 'daily_sales',                          -- 9a
        'nivelacija_up', 'nivelacija_down_storno',                    -- 9b
        'supplier_return_storno', 'customer_return_storno',           -- 9b
        'popis_visak', 'popis_manjak', 'error_storno', 'error_correction',  -- 9b
        'close_carry'                                                  -- 9c
    )),
    entry_source TEXT NOT NULL DEFAULT 'auto' CHECK (entry_source IN ('auto', 'manual')),
    reference_type TEXT,                          -- e.g. 'inventory_movement', 'sales_day'
    reference_id INTEGER,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    UNIQUE (book_year, redni_broj)
);
CREATE INDEX idx_kep_entries_book ON kep_entries(book_year, redni_broj);
CREATE INDEX idx_kep_entries_date ON kep_entries(entry_date);
```

Append-only is a codebase invariant (čl. 14): nothing UPDATEs or DELETEs a row except the go-live reset's wipe. `redni_broj` allocated `MAX(redni_broj)+1 WHERE book_year=?` inside each posting transaction. `entry_date` (kolona 2) and `document_date` (kolona 3) are **distinct dates**, never conflated.

## 2. Domain module — `src-tauri/src/kep.rs`

Pure-ish domain (opens its own connection or takes `&Transaction` for the receipt hook). Owns redni-broj allocation, posting, and the derived views.

```rust
pub fn book_year_of(rfc3339: &str) -> Result<i64, AppError>;             // calendar year of a timestamp
pub fn post_receipt_zaduzenje(tx: &Transaction, product_id: i64, quantity_milli: i64, sale_price_minor: i64, opis: &str, document_date: Option<&str>, reference_id: Option<i64>, acting: i64, now: &str) -> Result<(), AppError>;
pub fn post_daily_sales(conn: &mut Connection, date: &str, override_amount_minor: Option<i64>, acting: i64, now: &str) -> Result<KepEntryView, AppError>;
pub fn list_ledger(conn: &Connection, book_year: i64) -> Result<KepLedger, AppError>;   // entries + derived saldo
pub fn kep_status(conn: &Connection, today: &str) -> Result<KepStatus, AppError>;        // overdue days + unposted lots
```

- **Zaduženje value:** `amount_minor = quantity_milli * sale_price_minor / 1000` (retail incl. PDV — the product's `sale_price_minor`, **never** nabavna/purchase price). Integer division; whole-unit quantities are exact.
- **`KepLedger`** carries the rows (each with `kolona`, `amount_minor`, dan.mesec-formatted date) and the derived `saldo_minor = Σzaduženje − Σrazduženje` (memo §2.4).
- **`post_daily_sales`** computes the day's total from `sales` (`SELECT COALESCE(SUM(total_minor),0) FROM sales WHERE document_type='sale' AND status='completed' AND date(created_at)=date(?)`), or uses `override_amount_minor` when given (`entry_source='manual'`). Idempotent: a second post for the same date → `AppError::business("already_posted", …)`. `opis = "Dnevni promet {date}"`.

## 3. Zaduženje hook — `apply_inventory_adjustment`

In `apply_inventory_adjustment`, only for `InventoryMovementType::Receive`, **inside the existing transaction** (after `write_stock_movement`), read the product's `sale_price_minor` and call `kep::post_receipt_zaduzenje(&tx, product_id, quantity_milli, sale_price_minor, opis, document_date, reference_id, acting, created_at)`. Composing `opis` from the receipt's `reference_type`/`reference_id`/reason. Because it's in the same transaction, goods can never exist un-booked (čl. 11 st. 2) and a rollback removes both. Corrections/write-offs post nothing here (those are 9b storno kinds).

## 4. Razduženje command + status

`kep_post_daily_sales(date, override_amount_minor)` (admin) → `post_daily_sales`. `kep_status` returns: days with completed sales but no `daily_sales` entry where `today > date + 1` (the **T+1 overdue-posting warning**, memo §2.5), and any received lot whose zaduženje is missing (defensive — should be none, since the hook is atomic).

## 5. Commands (all `require_admin`, acting id from session, `now`/`today` = `utc_now()`)

`kep_ledger(book_year)` → `KepLedger` · `kep_post_daily_sales(date, override_amount_minor)` → the posted entry · `kep_status()` → `KepStatus`. Register in `lib.rs`. (Full-book print + export are 9c.)

## 6. Frontend

`KepService` on `PosServices`. A new admin-only „KEP" nav tab: the ledger rendered as the **5-column table** (Red. br. / Datum [dan.mesec] / Opis / Zaduženje / Razduženje) with the running **saldo**, a book-year selector, a „Proknjiži dnevni promet" action (date + optional override amount) calling `postDailySales`, and the `kep_status` warnings (overdue days, any unposted lot). Read-only otherwise (no edit/delete — čl. 14). Page mechanics (DONOS/SVEGA) and printing are 9c.

## 7. Reset

Go-live reset clears `kep_entries` (practice data), alongside the other trading tables and price_history (it already deletes `campaigns` etc.). Add `DELETE FROM kep_entries` to `reset_trading_data`.

## 8. Testing

The memo's worked ledger example verbatim: opening 10.000 → a receipt posting **7.800** (qty 50 × retail 156, incl. PDV — **not** the 100 nabavna) → a daily promet of **2.340** → derived **saldo 15.460**. Plus:
- zaduženje uses retail `sale_price_minor`, never `purchase_price_minor` (a dedicated assertion — this is the memo's flagged false-assurance bug).
- receipt + zaduženje are one atomic transaction (a forced failure rolls back both; no orphan entry, no orphan stock).
- `redni_broj` monotonic and gap-free per book_year; a 2027 entry restarts the sequence for its year.
- `post_daily_sales` idempotency (second post → `already_posted`); the override path sets `entry_source='manual'` and uses the given amount.
- the T+1 warning fires for an unposted sales day and not for today's.
- append-only invariant: no code path updates/deletes a `kep_entries` row (except reset).
- reset clears the ledger.
- Correction/WriteOff movements post **no** KEP entry (those are 9b).

## Non-goals
- The 14-element kalkulacija document + storno engine + error correction (→ 9b).
- Year-end freeze/lock, carry-forward, printing, page mechanics, retention clock (→ 9c).
- Multi-store `prodajno_mesto` scoping (single store; `book_year` is the only partition now).
- Wholesale (velikoprodaja) price basis.

## Acceptance criteria
- Every goods receipt atomically posts a retail-with-PDV zaduženje; a completed trading day posts a razduženje (auto or overridden); the running saldo derives to the dinar per the worked example.
- The ledger is append-only; redni broj is monotonic per year; dates show dan.mesec.
- The T+1 overdue-posting warning surfaces unbooked days; nothing is user-editable.
- KEP is always-on for the shop; reset clears practice entries.
- All gates green: `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `git diff --check`.
