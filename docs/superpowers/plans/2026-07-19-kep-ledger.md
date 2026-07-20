# KEP Value Ledger + Auto-Postings (SW-9a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** An append-only 5-column KEP (evidencija prometa) value ledger whose zaduženje auto-posts from goods receipts at retail-with-PDV and whose razduženje posts per trading day from the sales total (with a manual override), with a derived running saldo.

**Architecture:** Migration v12 adds `kep_entries` (append-only, one row per posting, a `kolona`/`amount_minor` pair). A domain module `kep.rs` owns redni-broj allocation, the receipt zaduženje, the daily razduženje, and the derived-saldo ledger view. The receipt zaduženje is written inside `apply_inventory_adjustment`'s existing transaction, so goods can't exist un-booked. Thin admin-gated commands + a KEP frontend module.

**Tech Stack:** Rust + rusqlite + `time` (RFC3339); Tauri v2 commands; React + TypeScript + shadcn/ui + Vitest/RTL.

## Global Constraints

- **Legal authority is `docs/KEP-VERIFIED-RULES.md`**; design is `docs/superpowers/specs/2026-07-19-kep-ledger-design.md`. If code and those documents disagree, STOP and report — never improvise law.
- **DO NOT boot, launch, or run the application** (no `tauri dev`, dev/preview server, or built binary). Verify only via `cargo test` / `cargo build` / `bun run test` / `bun run build` / clippy / fmt. Standing user instruction.
- **Zaduženje = retail value WITH PDV** = `quantity_milli * product.sale_price_minor / 1000`. **NEVER** `purchase_price_minor`/nabavna — that is the memo's flagged false-assurance bug.
- **Append-only** (PEP čl. 14): no code UPDATEs or DELETEs a `kep_entries` row, except the go-live reset's wipe. Corrections are 9b (out of scope).
- **`redni_broj`** is monotonic, gap-free, **per `book_year`**, allocated `MAX(redni_broj)+1 WHERE book_year=?` inside the posting transaction. `entry_date` (kolona 2, booking) and `document_date` (kolona 3, the isprava's date) are **distinct** and never conflated; kolona 2 displays **dan.mesec only**.
- Money in integer minor units (para): 7.800,00 RSD = `780000`; 156,00 = `15600`. Never floats.
- Serbian Latin copy with correct diacritics (šđčćž) — exact strings from the plan, character-for-character.
- Commands `require_admin`; acting id from the session; `now`/`today` from `crate::clock::utc_now()`.
- Migrations append-only: v12 next; count 11 → 12; `kep_entries` in `CORE_TABLES`; both indexes in `EXPLICIT_INDEXES`.
- KEP is non-optional (no toggle). The full `kind` enum (9a+9b+9c) goes in the v12 CHECK now, so 9b/9c never rebuild the table.
- Every task ends green on: `bun run test`, `bun run build`, `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`.
- Commit trailer on every commit:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  ```

---

### Task 1: Migration v12 — `kep_entries`

**Files:**
- Modify: `src-tauri/src/db/migrations.rs` (append v12 after v11)
- Modify: `src-tauri/src/db/mod.rs` (`CORE_TABLES`, `EXPLICIT_INDEXES`, count assertion ~line 781)
- Test: `src-tauri/src/db/mod.rs` (`mod tests`)

**Interfaces:**
- Produces: table `kep_entries` (columns per spec §1). Consumed by every later task.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn migration_v12_creates_kep_entries() {
        with_test_database("migration_v12_kep", |db| {
            let connection = db.open().expect("database should open");
            assert!(schema_object_exists(&connection, "table", "kep_entries"), "expected kep_entries");
            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='kep_entries'",
                    [],
                    |row| row.get(0),
                )
                .expect("schema");
            for token in ["book_year", "redni_broj", "zaduzenje", "razduzenje", "receipt", "daily_sales", "nivelacija_down_storno", "close_carry"] {
                assert!(schema.contains(token), "kep_entries schema missing {token}");
            }
            assert!(schema.contains("UNIQUE (book_year, redni_broj)"), "expected UNIQUE(book_year, redni_broj)");
        });
    }
```

Update the count test: `assert_eq!(migration_count, 11);` → `assert_eq!(migration_count, 12);`.

- [ ] **Step 2: Run to verify failure** → `cargo test --manifest-path src-tauri/Cargo.toml migration_v12 -- --test-threads=1` → FAIL.

- [ ] **Step 3: Append migration v12** — in `src-tauri/src/db/migrations.rs`, after the v11 entry:

```rust
    Migration {
        version: 12,
        name: "kep_entries",
        sql: r#"
CREATE TABLE kep_entries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_year INTEGER NOT NULL,
    redni_broj INTEGER NOT NULL,
    entry_date TEXT NOT NULL,
    document_date TEXT,
    opis TEXT NOT NULL,
    kolona TEXT NOT NULL CHECK (kolona IN ('zaduzenje', 'razduzenje')),
    amount_minor INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN (
        'opening', 'receipt', 'daily_sales',
        'nivelacija_up', 'nivelacija_down_storno',
        'supplier_return_storno', 'customer_return_storno',
        'popis_visak', 'popis_manjak', 'error_storno', 'error_correction',
        'close_carry'
    )),
    entry_source TEXT NOT NULL DEFAULT 'auto' CHECK (entry_source IN ('auto', 'manual')),
    reference_type TEXT,
    reference_id INTEGER,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    UNIQUE (book_year, redni_broj)
);
CREATE INDEX idx_kep_entries_book ON kep_entries(book_year, redni_broj);
CREATE INDEX idx_kep_entries_date ON kep_entries(entry_date);
"#,
    },
```

- [ ] **Step 4: Register** — `"kep_entries",` in `CORE_TABLES`; `"idx_kep_entries_book",` and `"idx_kep_entries_date",` in `EXPLICIT_INDEXES`.

- [ ] **Step 5: DB tests + full gates** → PASS. **Step 6: Commit**

```bash
git add src-tauri/src/db/migrations.rs src-tauri/src/db/mod.rs
git commit -m "feat(kep): migration v12 — append-only evidencija prometa ledger (SW-9a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: `kep.rs` — book year, redni broj, receipt zaduženje, ledger view

**Files:**
- Create: `src-tauri/src/kep.rs`
- Modify: `src-tauri/src/lib.rs` (`mod kep;`, alphabetical — after `mod importer;`)
- Test: `src-tauri/src/kep.rs` (`mod tests`)

**Interfaces:**
- Consumes: v12 schema; `AppError`; `time`.
- Produces (used by Tasks 3–5):

```rust
pub fn book_year_of(rfc3339: &str) -> Result<i64, AppError>;   // calendar year via `time`
pub fn next_redni_broj(conn: &rusqlite::Connection, book_year: i64) -> Result<i64, AppError>;  // MAX+1 for the year
pub fn post_receipt_zaduzenje(tx: &rusqlite::Transaction<'_>, product_id: i64, quantity_milli: i64, sale_price_minor: i64, opis: &str, document_date: Option<&str>, reference_id: Option<i64>, acting_user_id: i64, now: &str) -> Result<(), AppError>;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KepEntryView {
    pub redni_broj: i64,
    pub datum: String,          // dan.mesec, e.g. "04.07"
    pub opis: String,
    pub zaduzenje_minor: Option<i64>,   // Some when kolona=zaduzenje
    pub razduzenje_minor: Option<i64>,  // Some when kolona=razduzenje
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KepLedger { pub book_year: i64, pub entries: Vec<KepEntryView>, pub saldo_minor: i64 }

pub fn list_ledger(conn: &rusqlite::Connection, book_year: i64) -> Result<KepLedger, AppError>;
```

Semantics:
- `post_receipt_zaduzenje`: `amount = quantity_milli * sale_price_minor / 1000` (integer). Insert `kolona='zaduzenje'`, `kind='receipt'`, `entry_source='auto'`, `reference_type='inventory_movement'`, `book_year = book_year_of(now)`, `redni_broj = next_redni_broj`, `amount_minor = amount`. `entry_date = now`.
- `list_ledger`: rows for the year ordered by `redni_broj`; `datum` = the `dan.mesec` of `entry_date` (format `%d.%m` via `time`); `zaduzenje_minor`/`razduzenje_minor` from `kolona`; `saldo_minor = Σ(kolona=zaduzenje) − Σ(kolona=razduzenje)`.

- [ ] **Step 1: Write failing tests** (use a migrated in-memory or temp DB via the `Db::new`/`test_database_path` pattern; seed a product + a tax rate):

```rust
    // Memo §2.7 worked example: 50 kom at retail 156,00 -> zaduženje 7.800,00
    // (NOT the 100,00 nabavna). Then a daily razduženje 2.340,00 (Task 4 posts
    // that; here just assert the receipt zaduženje value + saldo of a manual seed).
    #[test]
    fn receipt_posts_retail_with_pdv_not_nabavna() {
        with_kep_db("kep_receipt_retail", |conn| {
            seed_product(conn, 1, 15600, 10000); // sale_price 156,00 ; purchase 100,00
            let tx = conn.transaction().expect("tx");
            post_receipt_zaduzenje(&tx, 1, 50_000 /* 50 kom in milli */, 15600, "Kalkulacija br. 12", Some("2026-07-03T00:00:00Z"), Some(7), 1, "2026-07-04T09:00:00Z").expect("post");
            tx.commit().expect("commit");
            let ledger = list_ledger(conn, 2026).expect("ledger");
            assert_eq!(ledger.entries.len(), 1);
            assert_eq!(ledger.entries[0].zaduzenje_minor, Some(780000), "50 x 156,00 retail incl PDV");
            assert_eq!(ledger.entries[0].datum, "04.07");
            assert_eq!(ledger.saldo_minor, 780000);
        });
    }

    #[test]
    fn redni_broj_is_monotonic_per_book_year() {
        with_kep_db("kep_redni_broj", |conn| {
            seed_product(conn, 1, 10000, 5000);
            for (i, now) in ["2026-07-04T09:00:00Z", "2026-07-05T09:00:00Z"].iter().enumerate() {
                let tx = conn.transaction().expect("tx");
                post_receipt_zaduzenje(&tx, 1, 1000, 10000, "x", None, None, 1, now).expect("post");
                tx.commit().expect("commit");
                let _ = i;
            }
            // A 2027 posting restarts the sequence for its year.
            let tx = conn.transaction().expect("tx");
            post_receipt_zaduzenje(&tx, 1, 1000, 10000, "x", None, None, 1, "2027-01-02T09:00:00Z").expect("post");
            tx.commit().expect("commit");
            let rb: Vec<i64> = {
                let mut s = conn.prepare("SELECT redni_broj FROM kep_entries WHERE book_year=2026 ORDER BY redni_broj").unwrap();
                s.query_map([], |r| r.get(0)).unwrap().collect::<Result<_,_>>().unwrap()
            };
            assert_eq!(rb, vec![1, 2]);
            let rb2027: i64 = conn.query_row("SELECT redni_broj FROM kep_entries WHERE book_year=2027", [], |r| r.get(0)).unwrap();
            assert_eq!(rb2027, 1, "2027 restarts");
        });
    }
```

Add `with_kep_db`/`seed_product` helpers to `mod tests` (mirror the campaign/reklamacije fixtures: `Db::new` a temp DB, seed a `tax_rates` row and a `products` row with the given `sale_price_minor`/`purchase_price_minor`).

- [ ] **Step 2: Run to verify failure** → FAIL. **Step 3: Implement** `kep.rs` + `mod kep;`. Use `parse_rfc3339` + `time` `format` for `%d.%m` and `year()`. **Step 4: Run** `cargo test --manifest-path src-tauri/Cargo.toml kep:: -- --test-threads=1` → PASS. **Step 5: Full gates** → PASS. **Step 6: Commit**

```bash
git add src-tauri/src/kep.rs src-tauri/src/lib.rs
git commit -m "feat(kep): ledger domain — redni broj, receipt zaduženje (retail+PDV), derived saldo (SW-9a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Zaduženje hook in `apply_inventory_adjustment`

**Files:**
- Modify: `src-tauri/src/commands/inventory.rs` (`apply_inventory_adjustment`, ~line 414)
- Test: `src-tauri/src/commands/inventory.rs` (`mod tests`)

**Interfaces:**
- Consumes: `kep::post_receipt_zaduzenje` (Task 2).
- Produces: every `Receive` movement atomically posts a KEP zaduženje.

Semantics: in `apply_inventory_adjustment`, only for `InventoryMovementType::Receive`, **inside the existing `tx`** and after `write_stock_movement`, read the product's `sale_price_minor` (`SELECT sale_price_minor FROM products WHERE id=?1`), compose `opis` (e.g. `format!("Prijem robe{}", reference_type-based suffix)` — if `request.reference_type`/`reference_id` present, include them; else a plain „Prijem robe" + the reason if any), and call `kep::post_receipt_zaduzenje(&tx, request.product_id, request.quantity_milli, sale_price_minor, &opis, None, request.reference_id, acting_user_id, created_at)`. Because it shares `tx`, a rollback removes both the stock movement and the KEP entry.

- [ ] **Step 1: Write failing tests** (reuse the inventory test fixtures):

```rust
    #[test]
    fn receiving_stock_posts_a_kep_zaduzenje_at_retail() {
        // ...seed a product with sale_price 15600, purchase 10000; call inventory_receive for 50 kom...
        // assert a kep_entries row exists with amount_minor = 780000 (retail 156 x 50), kolona 'zaduzenje', kind 'receipt'.
    }

    #[test]
    fn a_correction_posts_no_kep_entry() {
        // ...apply a Correction movement...; assert COUNT(kep_entries)=0.
    }

    #[test]
    fn receive_and_kep_entry_are_atomic() {
        // Force a failure after the stock write (e.g. a product row that violates a downstream constraint,
        // or assert via a rolled-back transaction path). Assert neither the movement nor the kep_entry persisted.
        // If a clean forced-failure hook is impractical, assert instead that a receive into a NONEXISTENT product
        // errors and leaves zero kep_entries and zero inventory_movements.
    }
```

- [ ] **Step 2: FAIL → Step 3: implement the hook → Step 4: PASS → Step 5: full gates → Step 6: commit:**

```bash
git add src-tauri/src/commands/inventory.rs
git commit -m "feat(kep): goods receipts post a retail zaduženje atomically (SW-9a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Daily razduženje + status (T+1)

**Files:**
- Modify: `src-tauri/src/kep.rs`
- Test: `src-tauri/src/kep.rs` (`mod tests`)

**Interfaces:**
- Consumes: Task 2; the `sales` table.
- Produces:

```rust
pub fn post_daily_sales(conn: &mut rusqlite::Connection, date: &str, override_amount_minor: Option<i64>, acting_user_id: i64, now: &str) -> Result<KepEntryView, AppError>;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KepStatus { pub overdue_sales_days: Vec<String>, pub unbooked_receipt_count: i64 }
pub fn kep_status(conn: &rusqlite::Connection, today: &str) -> Result<KepStatus, AppError>;
```

Semantics:
- `post_daily_sales`: reject a second post for the same `date` (`AppError::business("already_posted", "Dnevni promet za taj dan je već proknjižen.")`) — check `EXISTS(kind='daily_sales' AND date(entry_date)=date(?) )`... but `entry_date` is the booking time, not the sales day. So store the sales day in `document_date` and check on that: `EXISTS(kind='daily_sales' AND document_date=?date)`. The amount = `override_amount_minor` if `Some` (`entry_source='manual'`), else `SELECT COALESCE(SUM(total_minor),0) FROM sales WHERE document_type='sale' AND status='completed' AND date(created_at)=date(?1)` (`entry_source='auto'`). Insert `kolona='razduzenje'`, `kind='daily_sales'`, `opis = format!("Dnevni promet {date}")`, `document_date = date`, `entry_date = now`, a fresh `redni_broj` for `book_year_of(now)`.
- `kep_status`: `overdue_sales_days` = distinct `date(created_at)` in `sales` (completed, document_type='sale') with **no** matching `daily_sales` entry AND `date < date(today) - 0`... precisely: the day is over (`date < date(today)`) and `today > day + 1` (T+1). Encode as `date(created_at) < date(today, '-1 day')`. `unbooked_receipt_count` = receive movements with no linked kep receipt entry (should be 0; a defensive count — join `inventory_movements` type='receive' to `kep_entries` kind='receipt' by `reference_id`... simplest: count receive movements minus receipt kep entries; if a clean join is impractical, return 0 with a comment, since the atomic hook guarantees it).

- [ ] **Step 1: Failing tests** — post a day's sales (seed 2 completed sales on 2026-07-05 summing 234000) → razduženje 234000; a second post → `already_posted`; an override of 250000 → that amount + `entry_source='manual'`; `kep_status` lists 2026-07-05 as overdue when today is 2026-07-08 and it's unposted, and drops it once posted.
- [ ] **Step 2: FAIL → Step 3: implement → Step 4: PASS → Step 5: full gates → Step 6: commit:**

```bash
git add src-tauri/src/kep.rs
git commit -m "feat(kep): daily razduženje posting with override + T+1 status (SW-9a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Commands + reset + registration

**Files:**
- Create: `src-tauri/src/commands/kep.rs`
- Modify: `src-tauri/src/commands/mod.rs` (`pub mod kep;`), `src-tauri/src/lib.rs` (register), `src-tauri/src/commands/backup.rs` (`reset_trading_data`)
- Test: `src-tauri/src/commands/kep.rs`; `src-tauri/src/commands/backup.rs` (`mod tests`)

**Interfaces:**
- Consumes: the `kep` domain module.
- Produces (all `require_admin`, acting from session, `now`/`today` = `utc_now()`):
  - `kep_ledger(book_year: i64) -> KepLedger`
  - `kep_post_daily_sales(date: String, override_amount_minor: Option<i64>) -> KepEntryView`
  - `kep_status() -> KepStatus`

Reset: add `tx.execute("DELETE FROM kep_entries", [])?;` to `reset_trading_data` (alongside the other trading-table deletes, before the compliance tombstone).

- [ ] **Step 1: Failing tests** — a cashier gets `forbidden` from `kep_ledger` and `kep_post_daily_sales`; an admin happy path posts a day and reads the ledger back with the correct saldo. Backup: extend/mirror the existing reset test to assert `kep_entries` count is 0 after `reset_trading_data` (seed a kep row first).
- [ ] **Step 2: FAIL → Step 3: implement + register + reset → Step 4: PASS → Step 5: full gates → Step 6: commit:**

```bash
git add src-tauri/src/commands/kep.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/src/commands/backup.rs
git commit -m "feat(kep): admin-gated commands + go-live reset clears the ledger (SW-9a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: Frontend service contract

**Files:**
- Modify: `src/services/types.ts`, `src/services/ports.ts`, `src/services/local-adapter.ts`, `src/services/mock-adapter.ts`
- Test: `src/services/local-adapter.test.ts`

**Interfaces:**
- Produces (TS mirrors of the Rust DTOs):

```ts
export interface KepEntryView {
  redniBroj: number; datum: string; opis: string;
  zaduzenjeMinor: number | null; razduzenjeMinor: number | null; kind: string;
}
export interface KepLedger { bookYear: number; entries: KepEntryView[]; saldoMinor: number; }
export interface KepStatus { overdueSalesDays: string[]; unbookedReceiptCount: number; }

export interface KepService {
  ledger(bookYear: number): Promise<KepLedger>;
  postDailySales(date: string, overrideAmountMinor: number | null): Promise<KepEntryView>;
  status(): Promise<KepStatus>;
}
```

`PosServices` gains `kep: KepService`. Local adapter maps 1:1 (`invoke("kep_ledger", { bookYear })`, `invoke("kep_post_daily_sales", { date, overrideAmountMinor })`, `invoke("kep_status")`). Mock adapter: an in-memory stub (ledger with a seeded entry + saldo; postDailySales appends; status returns empty). Every `PosServices` construction gains `kep`.

- [ ] Steps: failing adapter-mapping test → FAIL → implement → PASS → full gates → commit:

```bash
git add src/services
git commit -m "feat(kep): frontend service contract (SW-9a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: KEP module UI

**Files:**
- Create: `src/app/kep/KepModule.tsx`, `src/app/kep/KepModule.test.tsx`
- Modify: `src/app/navigation.ts` (admin-only „KEP" nav item, `BookIcon`), `src/app/AppShell.tsx` (render branch)

**Interfaces:**
- Consumes: `KepService`.
- Produces: `<KepModule services={services} />`; nav id `"kep"`.

Content: nav item after `reklamacije`, `adminOnly: true`. The ledger rendered as a **5-column table** — headers „Red. br." / „Datum" / „Opis" / „Zaduženje" / „Razduženje" — from `ledger(bookYear)`, a **book-year selector** (default the current year), a **saldo** line (`formatRsd(saldoMinor)`), a „Proknjiži dnevni promet" control (a date input + an optional override amount input) calling `postDailySales`, and the `status()` warnings: „Nije proknjižen dnevni promet za: {days}" and (if any) an unbooked-receipt count. Read-only otherwise (no edit/delete controls). Tests (RTL, mock services): the ledger table renders entries + saldo; posting a day calls `postDailySales` with the date; overdue days render in the warning.

- [ ] Steps: failing tests → FAIL → implement → PASS → full gates → commit:

```bash
git add src/app/kep src/app/navigation.ts src/app/AppShell.tsx
git commit -m "feat(kep): KEP ledger module — 5-column table, saldo, daily posting, warnings (SW-9a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Self-Review Notes

- **Spec coverage:** §1 schema → T1; §2 domain (book_year/redni_broj/zaduženje/ledger) → T2, (daily/status) → T4; §3 receipt hook → T3; §4 razduženje+status → T4; §5 commands → T5; §6 frontend → T6–T7; §7 reset → T5; §8 testing → the worked example + retail-not-nabavna in T2/T3, idempotency+override+T1 in T4.
- **Type consistency:** `KepEntryView`/`KepLedger`/`KepStatus` defined once (T2/T4), mirrored camelCase in T6; three command names in T5 = invoke names in T6.
- **Zaduženje basis:** T2 and T3 both assert `sale_price_minor` (retail), never `purchase_price_minor` — the load-bearing anti-bug.
- **Append-only:** no task UPDATEs/DELETEs a `kep_entries` row except T5's reset; the receipt hook shares the inventory transaction (atomicity in T3).
- **`document_date` vs `entry_date`:** the daily-sales idempotency check keys on `document_date` (the sales day), not `entry_date` (the booking time) — called out in T4 so a same-day re-post is correctly rejected while next-day booking still works.
