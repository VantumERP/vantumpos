# KEP Year-End Close (SW-9c) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add year-end *zaključivanje* to the KEP — an irreversible, typed-confirmed close that freezes the year at the data layer (čl. 18 prevention), carries the krajnji saldo forward as the next year's computed opening, prints the signable close + the paginated full book, and starts the 5-year retention clock.

**Architecture:** A new `kep_closures` table (migration v14) records one row per closed year. `list_ledger` starts each year's running saldo from the prior year's closure (the *carry-in*), never from a stored `opening` row. A new `kep_close` module holds the close mechanics and the `ensure_year_open` gate, which every posting path calls first — combined with the append-only ledger, a closed year becomes immutable at the application layer. Two self-contained HTML renderers produce the signable close and the paginated full book. Admin-gated commands and a KEP-module UI expose it.

**Tech Stack:** Rust + rusqlite/SQLite (Tauri v2 backend); React + TypeScript + shadcn/ui + vitest (frontend). Money in integer minor units (para); RFC3339 timestamps passed in as `now` (never `datetime('now')` in ledger code); Serbian Latin diacritics.

## Global Constraints

- **Legal source of truth:** `docs/KEP-VERIFIED-RULES.md` §5 (year-end freeze, tamper, print, retention). This plan encodes that memo and `docs/superpowers/specs/2026-07-21-kep-yearend-design.md`. Do not invent rules.
- **Do NOT boot the app.** No `tauri dev`, dev/preview server, or built binary. Verify only via the gate commands below.
- **Money:** integer minor units (para). **Quantities:** milli-units. Never format money with floats.
- **Time:** ledger/closure code takes `now: &str` (RFC3339) as a parameter; commands stamp it from `crate::clock::utc_now()`. Never `datetime('now')` in `kep*`/closure code.
- **Append-only:** never `UPDATE`/`DELETE` a posted `kep_entries` row. The close inserts **no** `kep_entries` row.
- **Serde:** all backend structs crossing to the frontend use `#[serde(rename_all = "camelCase")]`.
- **Serbian diacritics** in all user-facing copy (č, ć, š, ž, đ). The typed close confirmation is exactly `ZAKLJUČI KNJIGU`.
- **Gates (all must pass, run from repo root):**
  - `bun run test`
  - `bun run build`
  - `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
  - `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`
  - `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
  - `git diff --check`
- **Commit trailer** (matches this branch's convention): every commit ends with
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  ```

---

### Task 1: Schema — migration v14 `kep_closures` + go-live reset wipe

**Files:**
- Modify: `src-tauri/src/db/migrations.rs` (append a `Migration` after the v13 entry, ~line 495)
- Modify: `src-tauri/src/db/mod.rs` (`CORE_TABLES` ~line 132, `EXPLICIT_INDEXES` ~line 159, count assertion ~line 786; add a v14 migration test near `migration_v13_adds_kalkulacije_and_cause` ~line 792)
- Modify: `src-tauri/src/commands/backup.rs` (`reset_trading_data`, after the `DELETE FROM kep_entries` at ~line 436; add a test in the file's `tests` module)

**Interfaces:**
- Consumes: the existing `Migration { version, name, sql }` struct and `MIGRATIONS` array.
- Produces: table `kep_closures(id, book_year UNIQUE, krajnji_saldo_minor, entry_count, closed_at, closed_by, created_at)` and index `idx_kep_closures_year`; go-live reset clears it.

- [ ] **Step 1: Write the failing migration test**

Add to the `tests` module in `src-tauri/src/db/mod.rs` (next to `migration_v13_adds_kalkulacije_and_cause`):

```rust
    #[test]
    fn migration_v14_adds_kep_closures() {
        with_test_database("migration_v14_kep_closures", |db| {
            let connection = db.open().expect("database should open");
            for column in [
                "id",
                "book_year",
                "krajnji_saldo_minor",
                "entry_count",
                "closed_at",
                "closed_by",
                "created_at",
            ] {
                let exists: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('kep_closures') WHERE name = ?1",
                        [column],
                        |row| row.get(0),
                    )
                    .expect("pragma should query");
                assert_eq!(exists, 1, "kep_closures.{column}");
            }
            let index: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                     WHERE type = 'index' AND name = 'idx_kep_closures_year'",
                    [],
                    |row| row.get(0),
                )
                .expect("index query");
            assert_eq!(index, 1, "idx_kep_closures_year exists");
        });
    }
```

Also bump the existing count assertion in the same file:

```rust
                assert_eq!(migration_count, 14);
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml migration_v14_adds_kep_closures -- --test-threads=1`
Expected: FAIL — `kep_closures.id` assertion (table does not exist yet). The count assertion also fails (still 13 applied).

- [ ] **Step 3: Add the v14 migration**

In `src-tauri/src/db/migrations.rs`, append this entry to `MIGRATIONS` immediately after the v13 `kep_kalkulacije` entry (before the closing `];`):

```rust
    Migration {
        version: 14,
        name: "kep_closures",
        sql: r#"
CREATE TABLE kep_closures (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    book_year INTEGER NOT NULL UNIQUE,
    krajnji_saldo_minor INTEGER NOT NULL,
    entry_count INTEGER NOT NULL,
    closed_at TEXT NOT NULL,
    closed_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_kep_closures_year ON kep_closures(book_year);
"#,
    },
```

In `src-tauri/src/db/mod.rs`, add `"kep_closures"` to the end of `CORE_TABLES` and `"idx_kep_closures_year"` to the end of `EXPLICIT_INDEXES`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
Expected: PASS — `migration_v14_adds_kep_closures`, the count assertion (now 14), and the `CORE_TABLES`/`EXPLICIT_INDEXES` existence loops.

- [ ] **Step 5: Write the failing reset test**

In `src-tauri/src/commands/backup.rs`, add to the `tests` module (follow the file's existing test-harness idiom — reuse whatever `with_app`/state helper the other `reset_trading_data` tests use; seed one closure directly, then assert it is gone). Concretely:

```rust
    #[test]
    fn reset_trading_data_clears_kep_closures() {
        with_reset_app("reset_clears_kep_closures", |state| {
            state
                .db()
                .open()
                .expect("db")
                .execute(
                    "INSERT INTO kep_closures
                        (book_year, krajnji_saldo_minor, entry_count, closed_at, closed_by, created_at)
                     VALUES (2026, 546000, 2, '2027-01-05T09:00:00Z', 1, '2027-01-05T09:00:00Z')",
                    [],
                )
                .expect("seed closure");

            reset_trading_data_inner(state).expect("reset should run");

            let remaining: i64 = state
                .db()
                .open()
                .expect("db")
                .query_row("SELECT COUNT(*) FROM kep_closures", [], |row| row.get(0))
                .expect("count");
            assert_eq!(remaining, 0, "go-live reset clears closures");
        });
    }
```

> If `reset_trading_data`'s testable core is named differently (e.g. the command wraps an inner fn or takes `State`), mirror the existing sibling reset tests in this file exactly — same harness fn, same invocation shape. The assertion (closures table empty after reset) is the fixed contract; adapt only the plumbing to match the file.

- [ ] **Step 6: Run the reset test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml reset_trading_data_clears_kep_closures -- --test-threads=1`
Expected: FAIL — `assert_eq!(remaining, 0)` fails (the seeded closure survives; reset does not touch the table yet).

- [ ] **Step 7: Wire the reset wipe**

In `src-tauri/src/commands/backup.rs`, immediately after the existing `tx.execute("DELETE FROM kep_entries", [])?;` line, add:

```rust
    // Closures gate the ledger (čl. 18). The go-live reset is the one sanctioned
    // escape from an irreversible close: practice years must not stay frozen.
    tx.execute("DELETE FROM kep_closures", [])?;
```

- [ ] **Step 8: Run the reset test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml reset_trading_data_clears_kep_closures -- --test-threads=1`
Expected: PASS.

- [ ] **Step 9: Commit**

```bash
git add src-tauri/src/db/migrations.rs src-tauri/src/db/mod.rs src-tauri/src/commands/backup.rs
git commit -m "$(cat <<'EOF'
feat(kep): migration v14 kep_closures + go-live reset wipe (SW-9c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: Carry-forward — `list_ledger` starts from the prior closure

**Files:**
- Modify: `src-tauri/src/kep.rs` (`KepLedger` struct ~line 126, `list_ledger` ~line 135; add a test in the file's `tests` module)
- Modify: `src/services/types.ts` (`KepLedger` interface ~line 1012)
- Modify: `src/services/mock-adapter.ts` (the `kep.ledger` stub ~line 1234)

**Interfaces:**
- Consumes: `kep_closures.krajnji_saldo_minor` (Task 1).
- Produces: `KepLedger { book_year, entries, opening_saldo_minor, saldo_minor }` — Rust adds `pub opening_saldo_minor: i64`; TS adds `openingSaldoMinor: number`. `list_ledger(conn, book_year)`'s running saldo starts from `kep_closures[book_year − 1].krajnji_saldo_minor` (else 0).

- [ ] **Step 1: Write the failing carry-forward test**

Add to the `tests` module in `src-tauri/src/kep.rs` (reuse the file's existing DB-open test helper; if the module opens a fresh `Db` per test as the sibling tests do, follow that):

```rust
    #[test]
    fn list_ledger_carries_forward_prior_closure() {
        with_kep_db("list_ledger_carry_forward", |conn| {
            // 2026 closed at krajnji saldo 15.460,00.
            conn.execute(
                "INSERT INTO kep_closures
                    (book_year, krajnji_saldo_minor, entry_count, closed_at, closed_by, created_at)
                 VALUES (2026, 1546000, 3, '2027-01-05T09:00:00Z', 1, '2027-01-05T09:00:00Z')",
                [],
            )
            .expect("seed 2026 closure");

            // 2027 opens empty; its opening saldo is the 2026 carry-in.
            let empty = list_ledger(conn, 2027).expect("ledger 2027");
            assert_eq!(empty.opening_saldo_minor, 1546000);
            assert_eq!(empty.saldo_minor, 1546000, "no entries yet → saldo == carry-in");

            // A 2027 receipt zaduženje of 1.000,00 lifts the running saldo above the carry-in.
            conn.execute(
                "INSERT INTO kep_entries
                    (book_year, redni_broj, entry_date, opis, kolona, amount_minor, kind, entry_source, user_id, created_at)
                 VALUES (2027, 1, '2027-01-10T00:00:00Z', 'Prijem robe', 'zaduzenje', 100000, 'receipt', 'auto', 1, '2027-01-10T00:00:00Z')",
                [],
            )
            .expect("seed 2027 entry");

            let ledger = list_ledger(conn, 2027).expect("ledger 2027");
            assert_eq!(ledger.opening_saldo_minor, 1546000);
            assert_eq!(ledger.saldo_minor, 1646000, "carry-in 1546000 + 100000");

            // A year with no prior closure opens at zero.
            let fresh = list_ledger(conn, 2026).expect("ledger 2026");
            assert_eq!(fresh.opening_saldo_minor, 0);
        });
    }
```

> Use the same `with_kep_db`/fixture helper the neighbouring `kep.rs` tests use to obtain a migrated `&Connection`. If they instead build a `Db` and call `.open()`, mirror that exactly.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml list_ledger_carries_forward_prior_closure -- --test-threads=1`
Expected: FAIL to compile — `KepLedger` has no field `opening_saldo_minor`.

- [ ] **Step 3: Add the field and the carry-in**

In `src-tauri/src/kep.rs`, add the field to `KepLedger`:

```rust
pub struct KepLedger {
    pub book_year: i64,
    pub entries: Vec<KepEntryView>,
    pub opening_saldo_minor: i64,
    pub saldo_minor: i64,
}
```

Change `list_ledger` to read the carry-in and seed the running saldo. Replace the `let mut saldo_minor: i64 = 0;` line (and the final `Ok(KepLedger { ... })`) so the body reads:

```rust
pub fn list_ledger(conn: &Connection, book_year: i64) -> Result<KepLedger, AppError> {
    // Carry-in: the prior year's krajnji saldo becomes this year's opening
    // (§2). Computed, not stored as an `opening` entry — a late opening row
    // would take a wrong redni broj once the new year is already trading.
    let opening_saldo_minor: i64 = conn
        .query_row(
            "SELECT krajnji_saldo_minor FROM kep_closures WHERE book_year = ?1",
            params![book_year - 1],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT redni_broj, entry_date, opis, kolona, amount_minor, kind
         FROM kep_entries
         WHERE book_year = ?1
         ORDER BY redni_broj",
    )?;
    let rows = stmt.query_map(params![book_year], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;

    let mut entries = Vec::new();
    let mut saldo_minor: i64 = opening_saldo_minor;
    for row in rows {
        let (redni_broj, entry_date, opis, kolona, amount_minor, kind) = row?;
        let datum = dan_mesec(&entry_date)?;
        let (zaduzenje_minor, razduzenje_minor) = if kolona == "zaduzenje" {
            saldo_minor += amount_minor;
            (Some(amount_minor), None)
        } else {
            saldo_minor -= amount_minor;
            (None, Some(amount_minor))
        };
        entries.push(KepEntryView {
            redni_broj,
            datum,
            opis,
            zaduzenje_minor,
            razduzenje_minor,
            kind,
        });
    }

    Ok(KepLedger {
        book_year,
        entries,
        opening_saldo_minor,
        saldo_minor,
    })
}
```

(`OptionalExtension` and `params` are already imported in `kep.rs`.)

- [ ] **Step 4: Run the Rust tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
Expected: PASS — the new carry-forward test and the existing `admin_posts_a_day_and_reads_saldo` (which seeds no closure, so `opening_saldo_minor == 0` and `saldo_minor == 546000` still holds).

- [ ] **Step 5: Update the TS type and mock (failing frontend build first)**

In `src/services/types.ts`, add the field to `KepLedger`:

```typescript
export interface KepLedger {
  bookYear: number;
  entries: KepEntryView[];
  openingSaldoMinor: number;
  saldoMinor: number;
}
```

In `src/services/mock-adapter.ts`, update the `kep.ledger` stub to include the opening (the mock has no closures, so 0, plus any closure the mock tracks in a later task — for now 0):

```typescript
      async ledger(bookYear) {
        const openingSaldoMinor = 0;
        const saldoMinor =
          openingSaldoMinor +
          kepEntries.reduce(
            (sum, entry) =>
              sum + (entry.zaduzenjeMinor ?? 0) - (entry.razduzenjeMinor ?? 0),
            0,
          );
        return { bookYear, entries: [...kepEntries], openingSaldoMinor, saldoMinor };
      },
```

- [ ] **Step 6: Run the frontend tests + build**

Run: `bun run test && bun run build`
Expected: PASS. If a KEP test constructs a `KepLedger` literal, add `openingSaldoMinor: 0` to it.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/kep.rs src/services/types.ts src/services/mock-adapter.ts
git commit -m "$(cat <<'EOF'
feat(kep): list_ledger carries prior-year closure forward as opening saldo (SW-9c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: `kep_close` module — close mechanics + the čl. 18 gate

**Files:**
- Create: `src-tauri/src/kep_close.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod kep_close;` in the module block, after `mod kep_storno;` ~line 10)

**Interfaces:**
- Consumes: `crate::kep::list_ledger` (Task 2), `crate::app_error::AppError`.
- Produces:
  - `pub const CLOSE_CONFIRMATION: &str = "ZAKLJUČI KNJIGU";`
  - `pub struct KepClosure { book_year: i64, krajnji_saldo_minor: i64, entry_count: i64, closed_at: String, closed_by: Option<i64> }` (serde camelCase)
  - `pub fn is_year_closed(conn: &Connection, book_year: i64) -> Result<bool, AppError>`
  - `pub fn ensure_year_open(conn: &Connection, book_year: i64) -> Result<(), AppError>` (rejects a closed year with `AppError::business("year_closed", …)`)
  - `pub fn close_year(conn: &mut Connection, book_year: i64, confirmation: &str, acting: i64, now: &str) -> Result<KepClosure, AppError>`

- [ ] **Step 1: Create the module with its tests (failing)**

Create `src-tauri/src/kep_close.rs`:

```rust
//! Year-end zaključivanje (SW-9c) — the čl. 18 data-layer lock.
//!
//! A `kep_closures` row freezes a book year. `ensure_year_open` is the gate
//! every posting path calls first; combined with the append-only ledger
//! (no UPDATE/DELETE of a posted row anywhere), a closed year cannot be
//! altered by the application — the čl. 18 *prevention* (not detection).
//!
//! The close is irreversible: there is no `reopen`. A typed confirmation
//! (`ZAKLJUČI KNJIGU`) guards against accident; the go-live reset
//! (`reset_trading_data`) is the one sanctioned escape, for practice years.
//!
//! The carry-forward is COMPUTED, not stored: `crate::kep::list_ledger` reads
//! the prior year's `krajnji_saldo_minor` as the opening. The close therefore
//! inserts NO `kep_entries` row.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::app_error::AppError;

/// The exact phrase the operator must type to close a year.
pub const CLOSE_CONFIRMATION: &str = "ZAKLJUČI KNJIGU";

/// A recorded year-end close (mirrors a `kep_closures` row for the frontend).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KepClosure {
    pub book_year: i64,
    pub krajnji_saldo_minor: i64,
    pub entry_count: i64,
    pub closed_at: String,
    pub closed_by: Option<i64>,
}

/// True when `book_year` has a closure row.
pub fn is_year_closed(conn: &Connection, book_year: i64) -> Result<bool, AppError> {
    let closed: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM kep_closures WHERE book_year = ?1)",
        params![book_year],
        |row| row.get(0),
    )?;
    Ok(closed)
}

/// The gate. Rejects a write into a closed year (čl. 18 prevention).
pub fn ensure_year_open(conn: &Connection, book_year: i64) -> Result<(), AppError> {
    if is_year_closed(conn, book_year)? {
        return Err(AppError::business(
            "year_closed",
            format!("Knjiga za {book_year}. godinu je zaključena i ne može se menjati."),
        ));
    }
    Ok(())
}

/// Closes `book_year` irreversibly. Validates the typed confirmation, rejects a
/// double-close, computes the krajnji saldo from `list_ledger` (which already
/// includes the carry-in), records the closure, and inserts NO ledger entry.
pub fn close_year(
    conn: &mut Connection,
    book_year: i64,
    confirmation: &str,
    acting: i64,
    now: &str,
) -> Result<KepClosure, AppError> {
    if confirmation != CLOSE_CONFIRMATION {
        return Err(AppError::validation(
            "Potvrda nije ispravna. Ukucajte tačno: ZAKLJUČI KNJIGU.",
            serde_json::json!({ "field": "confirmation" }),
        ));
    }

    let tx = conn.transaction()?;
    if is_year_closed(&tx, book_year)? {
        return Err(AppError::business(
            "invalid_state",
            "Godina je već zaključena.",
        ));
    }

    let ledger = crate::kep::list_ledger(&tx, book_year)?;
    let krajnji_saldo_minor = ledger.saldo_minor;
    let entry_count = ledger.entries.len() as i64;

    tx.execute(
        "INSERT INTO kep_closures (
            book_year, krajnji_saldo_minor, entry_count, closed_at, closed_by, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?4)",
        params![book_year, krajnji_saldo_minor, entry_count, now, acting],
    )?;
    tx.commit()?;

    Ok(KepClosure {
        book_year,
        krajnji_saldo_minor,
        entry_count,
        closed_at: now.to_string(),
        closed_by: Some(acting),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};

    /// Opens a migrated connection over a throwaway database.
    fn with_conn(test_name: &str, test: impl FnOnce(&mut Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("database should initialize");
            let mut conn = db.open().expect("database should open");
            test(&mut conn);
        }
        std::fs::remove_file(&path).expect("test database should be removed");
    }

    /// Seeds a receipt zaduženje so the closed saldo is a real figure.
    fn seed_zaduzenje(conn: &Connection, book_year: i64, redni_broj: i64, amount_minor: i64) {
        conn.execute(
            "INSERT INTO kep_entries
                (book_year, redni_broj, entry_date, opis, kolona, amount_minor, kind, entry_source, user_id, created_at)
             VALUES (?1, ?2, '2026-06-01T00:00:00Z', 'Prijem robe', 'zaduzenje', ?3, 'receipt', 'auto', 1, '2026-06-01T00:00:00Z')",
            params![book_year, redni_broj, amount_minor],
        )
        .expect("seed zaduženje");
    }

    #[test]
    fn close_year_records_saldo_and_count() {
        with_conn("close_year_records", |conn| {
            seed_zaduzenje(conn, 2026, 1, 780000);
            seed_zaduzenje(conn, 2026, 2, 120000);

            let closure = close_year(conn, 2026, CLOSE_CONFIRMATION, 1, "2027-01-05T09:00:00Z")
                .expect("close should succeed");

            assert_eq!(closure.krajnji_saldo_minor, 900000, "780000 + 120000");
            assert_eq!(closure.entry_count, 2);
            assert!(is_year_closed(conn, 2026).expect("closed check"));

            // The close inserts NO kep_entries row.
            let entry_count: i64 = conn
                .query_row("SELECT COUNT(*) FROM kep_entries WHERE book_year = 2026", [], |r| {
                    r.get(0)
                })
                .expect("count");
            assert_eq!(entry_count, 2, "no entry inserted by the close");
        });
    }

    #[test]
    fn close_year_rejects_wrong_confirmation() {
        with_conn("close_year_wrong_confirmation", |conn| {
            let err = close_year(conn, 2026, "zakljuci", 1, "2027-01-05T09:00:00Z")
                .expect_err("wrong confirmation must reject");
            assert_eq!(err.code(), "validation_error");
            assert!(!is_year_closed(conn, 2026).expect("still open"));
        });
    }

    #[test]
    fn close_year_rejects_double_close() {
        with_conn("close_year_double", |conn| {
            close_year(conn, 2026, CLOSE_CONFIRMATION, 1, "2027-01-05T09:00:00Z")
                .expect("first close");
            let err = close_year(conn, 2026, CLOSE_CONFIRMATION, 1, "2027-01-06T09:00:00Z")
                .expect_err("second close must reject");
            assert_eq!(err.code(), "invalid_state");
        });
    }

    #[test]
    fn ensure_year_open_gates_a_closed_year() {
        with_conn("ensure_year_open_gate", |conn| {
            ensure_year_open(conn, 2026).expect("open year passes");
            close_year(conn, 2026, CLOSE_CONFIRMATION, 1, "2027-01-05T09:00:00Z")
                .expect("close");
            let err = ensure_year_open(conn, 2026).expect_err("closed year rejects");
            assert_eq!(err.code(), "year_closed");
            ensure_year_open(conn, 2027).expect("a later open year still passes");
        });
    }
}
```

> **Error-code accessor:** the tests call `err.code()`. Confirm how the other `kep*` tests read an `AppError`'s code (the command tests compare `error.code` on a `CommandError`; the domain tests may `matches!` on the `AppError` variant). If `AppError` exposes no `code()` method, replace each `assert_eq!(err.code(), "…")` with the codebase's actual idiom — e.g. `assert!(matches!(err, AppError::Business { code: "year_closed", .. }))` and `AppError::Validation { .. }` / `AppError::Business { code: "invalid_state", .. }`. Match the sibling modules; do not invent an accessor.

- [ ] **Step 2: Register the module**

In `src-tauri/src/lib.rs`, add after `mod kep_storno;`:

```rust
mod kep_close;
```

- [ ] **Step 3: Run the tests to verify they fail, then pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml kep_close -- --test-threads=1`
Expected: PASS (the module is written to satisfy its own tests). If it fails to compile on `err.code()`, apply the accessor note above, then re-run.

- [ ] **Step 4: Lint + format**

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
Expected: clean.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/kep_close.rs src-tauri/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(kep): close_year + ensure_year_open gate (cl. 18 lock) (SW-9c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: Wire `ensure_year_open` into every posting path

**Files:**
- Modify: `src-tauri/src/kep.rs` (`post_receipt_zaduzenje` ~line 90, `post_daily_sales` ~line 189, `correct_entry` ~line 312)
- Modify: `src-tauri/src/kep_kalkulacija.rs` (`create_kalkulacija` ~line 62, right after `let book_year = book_year_of(now)?;` ~line 109)
- Modify: `src-tauri/src/kep_storno.rs` (`post_value_storno` ~line 168, `post_nivelacija` ~line 252 — after each derives its `book_year`)
- Test: add gate-rejection tests to `src-tauri/src/kep.rs` tests

**Interfaces:**
- Consumes: `crate::kep_close::ensure_year_open` (Task 3), `crate::kep::book_year_of`.
- Produces: every posting path rejects a write into a closed year with `year_closed` before it inserts.

- [ ] **Step 1: Write the failing gate tests**

Add to the `tests` module in `src-tauri/src/kep.rs` (use the same fixture helper as Task 2's test):

```rust
    #[test]
    fn posting_paths_reject_a_closed_year() {
        with_kep_db("posting_paths_gated", |conn| {
            // Close 2026.
            crate::kep_close::close_year(conn, 2026, crate::kep_close::CLOSE_CONFIRMATION, 1, "2026-12-31T23:59:00Z")
                .expect("close 2026");

            // A receipt zaduženje into 2026 (now inside 2026) is rejected.
            let tx = conn.transaction().expect("tx");
            let err = post_receipt_zaduzenje(
                &tx, 1, 1000, 15600, "Prijem robe", None, "manual", None, 1, "2026-12-31T23:59:30Z",
            )
            .expect_err("receipt into closed year rejects");
            assert_eq!(err.code(), "year_closed");
            drop(tx);

            // A daily-sales post into 2026 is rejected.
            let err = post_daily_sales(conn, "2026-12-30", Some(50000), 1, "2026-12-31T23:59:30Z")
                .expect_err("daily sales into closed year rejects");
            assert_eq!(err.code(), "year_closed");
        });
    }
```

> If `kep.rs` tests read the error via `matches!` rather than `.code()`, use that idiom (see Task 3's accessor note). `correct_entry`, `post_value_storno`, `post_nivelacija`, and `create_kalkulacija` gate identically; they are exercised by the command-layer test in Task 7. This test covers the two paths that live in `kep.rs`.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml posting_paths_reject_a_closed_year -- --test-threads=1`
Expected: FAIL — both calls currently succeed (no gate), so `expect_err` panics.

- [ ] **Step 3: Add the gate to each posting path**

`post_receipt_zaduzenje` (`kep.rs`) — after `let book_year = book_year_of(now)?;`:

```rust
    let book_year = book_year_of(now)?;
    crate::kep_close::ensure_year_open(tx, book_year)?;
```

`post_daily_sales` (`kep.rs`) — after `let book_year = book_year_of(now)?;` (before `let tx = conn.transaction()?;`):

```rust
    let book_year = book_year_of(now)?;
    crate::kep_close::ensure_year_open(conn, book_year)?;
```

`correct_entry` (`kep.rs`) — a correction books forward under the current year AND touches a target row that must not live in a frozen year; gate both. After `let correction_book_year = book_year_of(now)?;`:

```rust
    let correction_book_year = book_year_of(now)?;
    // Neither the frozen target year nor a frozen current year may be written.
    crate::kep_close::ensure_year_open(tx, book_year)?;
    crate::kep_close::ensure_year_open(tx, correction_book_year)?;
```

`create_kalkulacija` (`kep_kalkulacija.rs`) — after `let book_year = book_year_of(now)?;`:

```rust
    let book_year = book_year_of(now)?;
    crate::kep_close::ensure_year_open(tx, book_year)?;
```

`post_value_storno` (`kep_storno.rs`) — after the fn derives its `book_year` (locate `let book_year = book_year_of(now)?;`; if the fn currently derives `book_year` only implicitly via `next_redni_broj`, add `let book_year = crate::kep::book_year_of(now)?;` immediately before the first insert and gate on it):

```rust
    let book_year = crate::kep::book_year_of(now)?;
    crate::kep_close::ensure_year_open(tx, book_year)?;
```

`post_nivelacija` (`kep_storno.rs`) — same treatment, right before the KEP Δ entry is posted:

```rust
    let book_year = crate::kep::book_year_of(now)?;
    crate::kep_close::ensure_year_open(tx, book_year)?;
```

> The `&Transaction` passed as `tx` coerces to `&Connection` for `ensure_year_open` via deref. Place each gate BEFORE the path's first `INSERT`/`UPDATE` so a rejection leaves nothing written.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
Expected: PASS — the new gate test, and all pre-existing `kep*` tests (their `now` values fall in years with no closure, so the gate is a no-op).

- [ ] **Step 5: Lint + format, then commit**

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
Expected: clean.

```bash
git add src-tauri/src/kep.rs src-tauri/src/kep_kalkulacija.rs src-tauri/src/kep_storno.rs
git commit -m "$(cat <<'EOF'
feat(kep): gate every posting path on ensure_year_open (SW-9c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: Documents — signable close HTML + paginated full-book HTML

**Files:**
- Modify: `src-tauri/src/kep_close.rs` (add `render_close_html`, `render_book_html`, and private helpers; add tests)

**Interfaces:**
- Consumes: `crate::commands::settings::CompanySettings`, `crate::kep::{KepLedger, KepEntryView}`, `KepClosure`.
- Produces:
  - `pub struct KepCloseView { company: CompanySettings, closure: KepClosure, opening_saldo_minor: i64, zaduzenje_total_minor: i64, razduzenje_total_minor: i64 }`
  - `pub fn render_close_html(view: &KepCloseView) -> String`
  - `pub fn render_book_html(company: &CompanySettings, ledger: &KepLedger, rows_per_page: usize) -> String`
  - `pub const BOOK_ROWS_PER_PAGE: usize = 30;`

- [ ] **Step 1: Write the failing document tests**

Add to the `tests` module in `src-tauri/src/kep_close.rs`:

```rust
    use crate::commands::settings::CompanySettings;
    use crate::kep::{KepEntryView, KepLedger};

    fn company() -> CompanySettings {
        CompanySettings {
            shop_name: "STR Delta".to_string(),
            address: "Kralja Petra 1, Novi Sad".to_string(),
            pib: "123456789".to_string(),
        }
    }

    #[test]
    fn close_html_carries_krajnji_saldo_and_signature() {
        let view = KepCloseView {
            company: company(),
            closure: KepClosure {
                book_year: 2026,
                krajnji_saldo_minor: 900000,
                entry_count: 2,
                closed_at: "2027-01-05T09:00:00Z".to_string(),
                closed_by: Some(1),
            },
            opening_saldo_minor: 0,
            zaduzenje_total_minor: 900000,
            razduzenje_total_minor: 0,
        };
        let html = render_close_html(&view);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("Zaključenje knjige za 2026"));
        assert!(html.contains("9.000,00"), "krajnji saldo 900000 minor");
        assert!(html.contains("ODGOVORNO LICE"), "signature line");
        assert!(html.contains("Nije fiskalni dokument"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn book_html_paginates_with_donos_and_svega() {
        // 65 entries at rows_per_page = 30 → 3 pages (30 / 30 / 5).
        let mut entries = Vec::new();
        for i in 1..=65 {
            entries.push(KepEntryView {
                redni_broj: i,
                datum: "01.06".to_string(),
                opis: format!("Stavka {i}"),
                zaduzenje_minor: Some(10000),
                razduzenje_minor: None,
                kind: "receipt".to_string(),
            });
        }
        let ledger = KepLedger {
            book_year: 2026,
            entries,
            opening_saldo_minor: 50000,
            saldo_minor: 700000,
        };
        let html = render_book_html(&company(), &ledger, 30);
        assert!(html.contains("Strana 1"));
        assert!(html.contains("Strana 3"));
        assert!(!html.contains("Strana 4"), "65 rows @ 30 → exactly 3 pages");
        assert!(html.contains("DONOS"), "each page after the first carries a DONOS");
        assert!(html.contains("SVEGA ZA PRENOS"), "each page but the last carries a carry-out");
        assert!(html.contains("page-break-after"));
        assert!(html.contains("500,00"), "first DONOS shows the 50000 opening carry-in");
        assert!(!html.contains("<script"));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml kep_close -- --test-threads=1`
Expected: FAIL to compile — `KepCloseView`, `render_close_html`, `render_book_html` do not exist.

- [ ] **Step 3: Implement the renderers**

Add to `src-tauri/src/kep_close.rs` (above the `#[cfg(test)]` module). Import the settings type at the top of the file: `use crate::commands::settings::CompanySettings;` and `use crate::kep::KepLedger;`.

```rust
/// Default rows per printed book page (§5.5 page mechanics).
pub const BOOK_ROWS_PER_PAGE: usize = 30;

/// Everything the signable close document shows (čl. 17 st. 4).
pub struct KepCloseView {
    pub company: CompanySettings,
    pub closure: KepClosure,
    pub opening_saldo_minor: i64,
    pub zaduzenje_total_minor: i64,
    pub razduzenje_total_minor: i64,
}

const DOC_STYLE: &str = "\
@page { margin: 1cm }\n\
body { font-family: sans-serif; color: #111; margin: 1cm; }\n\
h1 { font-size: 1.3rem; }\n\
.meta { color: #444; margin: 0.1rem 0; }\n\
table { border-collapse: collapse; width: 100%; margin: 0.75rem 0; }\n\
th, td { border: 1px solid #999; padding: 0.25rem 0.5rem; text-align: left; }\n\
td.amount, th.amount { text-align: right; white-space: nowrap; }\n\
tr.donos td, tr.svega td { font-weight: bold; background: #f0f0f0; }\n\
.page { page-break-after: always; }\n\
.page:last-child { page-break-after: auto; }\n\
.sign { margin-top: 3rem; display: flex; justify-content: space-between; }\n\
footer { margin-top: 2rem; color: #666; font-size: 0.85rem; }\n";

fn doc_head(title: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"sr-Latn\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>{}</title>\n<style>\n{}</style>\n</head>\n<body>\n",
        escape_html(title),
        DOC_STYLE
    )
}

fn company_header(company: &CompanySettings, book_year: i64) -> String {
    format!(
        "<p class=\"meta\">Trgovac: {}</p>\n\
         <p class=\"meta\">PIB: {}</p>\n\
         <p class=\"meta\">Prodajni objekat: {}</p>\n\
         <p class=\"meta\">KNJIGA EVIDENCIJE PROMETA ZA {}. GODINU</p>\n",
        escape_html(&company.shop_name),
        escape_html(&company.pib),
        escape_html(&company.address),
        book_year
    )
}

/// The signable electronic-close document (čl. 17 st. 4): opening carry-in, the
/// year's zaduženje/razduženje totals, the krajnji saldo, and a signature line.
pub fn render_close_html(view: &KepCloseView) -> String {
    let mut html = doc_head("Zaključenje knjige evidencije prometa");
    html.push_str(&format!(
        "<h1>Zaključenje knjige za {}</h1>\n",
        view.closure.book_year
    ));
    html.push_str(&company_header(&view.company, view.closure.book_year));

    html.push_str("<table>\n<tbody>\n");
    for (label, minor) in [
        ("Početno stanje (donos)", view.opening_saldo_minor),
        ("Ukupno zaduženje (kolona 4)", view.zaduzenje_total_minor),
        ("Ukupno razduženje (kolona 5)", view.razduzenje_total_minor),
        ("KRAJNJI SALDO", view.closure.krajnji_saldo_minor),
    ] {
        html.push_str(&format!(
            "<tr><th>{label}</th><td class=\"amount\">{} RSD</td></tr>\n",
            format_rsd_minor(minor)
        ));
    }
    html.push_str(&format!(
        "<tr><th>Broj stavki</th><td class=\"amount\">{}</td></tr>\n",
        view.closure.entry_count
    ));
    html.push_str(&format!(
        "<tr><th>Datum zaključenja</th><td>{}</td></tr>\n",
        escape_html(date_only(&view.closure.closed_at))
    ));
    html.push_str("</tbody>\n</table>\n");

    html.push_str(
        "<div class=\"sign\"><span>M.P. ______________</span>\
         <span>ODGOVORNO LICE ______________</span></div>\n",
    );
    html.push_str("<footer>Interni dokument. Nije fiskalni dokument.</footer>\n</body>\n</html>\n");
    html
}

/// The full-book paginated print (§5.5): the 5-column table split into pages of
/// `rows_per_page`, each page after the first opening with a DONOS (running
/// carry-in) row and each page but the last closing with a SVEGA ZA PRENOS
/// (running carry-out) row. Pages are numbered `Strana k`.
pub fn render_book_html(
    company: &CompanySettings,
    ledger: &KepLedger,
    rows_per_page: usize,
) -> String {
    let per_page = rows_per_page.max(1);
    let mut html = doc_head("Knjiga evidencije prometa");
    html.push_str(&company_header(company, ledger.book_year));

    let chunks: Vec<&[KepEntryView]> = ledger.entries.chunks(per_page).collect();
    let page_total = chunks.len().max(1);
    // Running saldo carried across pages, seeded from the opening carry-in.
    let mut running = ledger.opening_saldo_minor;

    for (page_index, chunk) in chunks.iter().enumerate() {
        let page_number = page_index + 1;
        html.push_str("<div class=\"page\">\n");
        html.push_str(&format!(
            "<p class=\"meta\">Strana {page_number} / {page_total}</p>\n"
        ));
        html.push_str(
            "<table>\n<thead>\n<tr>\
             <th>RB</th><th>Datum</th><th>Opis</th>\
             <th class=\"amount\">Zaduženje (4)</th>\
             <th class=\"amount\">Razduženje (5)</th>\
             <th class=\"amount\">Saldo</th></tr>\n</thead>\n<tbody>\n",
        );

        // DONOS — the running carry-in for this page (every page carries it; the
        // first page's DONOS is the year's opening carry-in).
        html.push_str(&format!(
            "<tr class=\"donos\"><td></td><td></td><td>DONOS</td>\
             <td class=\"amount\"></td><td class=\"amount\"></td>\
             <td class=\"amount\">{} RSD</td></tr>\n",
            format_rsd_minor(running)
        ));

        let mut page_zaduzenje = 0i64;
        let mut page_razduzenje = 0i64;
        for entry in chunk.iter() {
            let zad = entry.zaduzenje_minor.unwrap_or(0);
            let raz = entry.razduzenje_minor.unwrap_or(0);
            running += zad - raz;
            page_zaduzenje += zad;
            page_razduzenje += raz;
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td>\
                 <td class=\"amount\">{}</td><td class=\"amount\">{}</td>\
                 <td class=\"amount\">{} RSD</td></tr>\n",
                entry.redni_broj,
                escape_html(&entry.datum),
                escape_html(&entry.opis),
                entry
                    .zaduzenje_minor
                    .map(format_rsd_minor)
                    .unwrap_or_default(),
                entry
                    .razduzenje_minor
                    .map(format_rsd_minor)
                    .unwrap_or_default(),
                format_rsd_minor(running)
            ));
        }

        // Per-page subtotal, then SVEGA ZA PRENOS on every page but the last.
        html.push_str(&format!(
            "<tr class=\"svega\"><td></td><td></td><td>UKUPNO STRANA</td>\
             <td class=\"amount\">{}</td><td class=\"amount\">{}</td>\
             <td class=\"amount\"></td></tr>\n",
            format_rsd_minor(page_zaduzenje),
            format_rsd_minor(page_razduzenje)
        ));
        if page_number < page_total {
            html.push_str(&format!(
                "<tr class=\"svega\"><td></td><td></td><td>SVEGA ZA PRENOS</td>\
                 <td class=\"amount\"></td><td class=\"amount\"></td>\
                 <td class=\"amount\">{} RSD</td></tr>\n",
                format_rsd_minor(running)
            ));
        } else {
            html.push_str(&format!(
                "<tr class=\"svega\"><td></td><td></td><td>KRAJNJI SALDO</td>\
                 <td class=\"amount\"></td><td class=\"amount\"></td>\
                 <td class=\"amount\">{} RSD</td></tr>\n",
                format_rsd_minor(running)
            ));
        }

        html.push_str("</tbody>\n</table>\n</div>\n");
    }

    html.push_str("<footer>Interni dokument. Nije fiskalni dokument.</footer>\n</body>\n</html>\n");
    html
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Formats integer minor units as grouped RSD: `900000 → "9.000,00"`.
fn format_rsd_minor(minor: i64) -> String {
    let negative = minor < 0;
    let abs = minor.unsigned_abs();
    let dinars = abs / 100;
    let para = abs % 100;
    let digits = dinars.to_string();
    let bytes = digits.as_bytes();
    let len = bytes.len();
    let mut grouped = String::with_capacity(len + len / 3);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(*byte as char);
    }
    let sign = if negative { "-" } else { "" };
    format!("{sign}{grouped},{para:02}")
}

/// `2027-01-05T09:00:00Z` → `2027-01-05` for display.
fn date_only(rfc3339: &str) -> &str {
    rfc3339.split('T').next().unwrap_or(rfc3339)
}
```

> `KepEntryView` must be in scope — add it to the `use crate::kep::…` line: `use crate::kep::{KepEntryView, KepLedger};`. If clippy flags `format_rsd_minor`/`escape_html` as duplicated across modules, that duplication is intentional (each KEP doc module keeps its own copy, matching `kep_kalkulacija.rs`); silence only if clippy actually errors, using the same approach the sibling modules use.

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml kep_close -- --test-threads=1`
Expected: PASS — both document tests plus the Task 3 close tests.

- [ ] **Step 5: Lint + format, then commit**

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
Expected: clean.

```bash
git add src-tauri/src/kep_close.rs
git commit -m "$(cat <<'EOF'
feat(kep): signable close + paginated full-book HTML (DONOS/SVEGA) (SW-9c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: Retention + closures list

**Files:**
- Modify: `src-tauri/src/kep_close.rs` (add `KepClosureView`, `purge_eligible`, `list_closures`; add tests)

**Interfaces:**
- Consumes: `kep_closures` rows, `KepClosure`.
- Produces:
  - `pub fn purge_eligible(book_year: i64, closed_at: &str, today: &str) -> bool` — true when `today` is on/after the LATER of `closed_at + 5y` and `31 Dec of book_year + 5y`.
  - `pub struct KepClosureView { book_year, krajnji_saldo_minor, entry_count, closed_at, purge_eligible }` (serde camelCase)
  - `pub fn list_closures(conn: &Connection, today: &str) -> Result<Vec<KepClosureView>, AppError>` (newest first)

- [ ] **Step 1: Write the failing retention tests**

Add to the `tests` module in `src-tauri/src/kep_close.rs`:

```rust
    #[test]
    fn purge_eligible_uses_the_later_floor() {
        // 2026 book year, closed early 2027-01-05.
        // 31 Dec 2026 + 5y = 2031-12-31; closed_at + 5y = 2032-01-05 (the later).
        assert!(!purge_eligible(2026, "2027-01-05T09:00:00Z", "2031-12-31T00:00:00Z"));
        assert!(!purge_eligible(2026, "2027-01-05T09:00:00Z", "2032-01-04T00:00:00Z"));
        assert!(purge_eligible(2026, "2027-01-05T09:00:00Z", "2032-01-05T00:00:00Z"));

        // A close done ON 31 Dec 2026 → both floors 2031-12-31; eligible from then.
        assert!(!purge_eligible(2026, "2026-12-31T23:00:00Z", "2031-12-30T00:00:00Z"));
        assert!(purge_eligible(2026, "2026-12-31T23:00:00Z", "2031-12-31T00:00:00Z"));
    }

    #[test]
    fn list_closures_reports_retention() {
        with_conn("list_closures_retention", |conn| {
            seed_zaduzenje(conn, 2026, 1, 900000);
            close_year(conn, 2026, CLOSE_CONFIRMATION, 1, "2027-01-05T09:00:00Z")
                .expect("close 2026");

            let recent = list_closures(conn, "2028-06-01T00:00:00Z").expect("list");
            assert_eq!(recent.len(), 1);
            assert_eq!(recent[0].book_year, 2026);
            assert_eq!(recent[0].krajnji_saldo_minor, 900000);
            assert!(!recent[0].purge_eligible, "well within 5 years");

            let aged = list_closures(conn, "2032-02-01T00:00:00Z").expect("list");
            assert!(aged[0].purge_eligible, "past the 5-year floor");
        });
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml kep_close -- --test-threads=1`
Expected: FAIL to compile — `purge_eligible`, `KepClosureView`, `list_closures` do not exist.

- [ ] **Step 3: Implement retention + list**

Add to `src-tauri/src/kep_close.rs` (above the tests):

```rust
/// One closure as the frontend list shows it, with the retention flag.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KepClosureView {
    pub book_year: i64,
    pub krajnji_saldo_minor: i64,
    pub entry_count: i64,
    pub closed_at: String,
    pub purge_eligible: bool,
}

/// Retention floor (§5.6): a closed book may be discarded only 5 full years
/// after the LATER of its close date and the end of its book year. Dates are
/// RFC3339; comparison is lexicographic on the `YYYY-MM-DD` prefix, which is
/// correct for ISO dates. `today` on/after the floor ⇒ eligible.
pub fn purge_eligible(book_year: i64, closed_at: &str, today: &str) -> bool {
    let year_end_floor = format!("{}-12-31", book_year + 5);
    let closed_floor = {
        let day = date_only(closed_at);
        // day is YYYY-MM-DD; add 5 to the year component.
        let year: i64 = day.get(0..4).and_then(|y| y.parse().ok()).unwrap_or(book_year + 1);
        format!("{}{}", year + 5, &day[4..])
    };
    let floor = if closed_floor >= year_end_floor {
        closed_floor
    } else {
        year_end_floor
    };
    date_only(today) >= floor.as_str()
}

/// Lists recorded closures newest-first, each with its retention flag.
pub fn list_closures(conn: &Connection, today: &str) -> Result<Vec<KepClosureView>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT book_year, krajnji_saldo_minor, entry_count, closed_at
         FROM kep_closures ORDER BY book_year DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (book_year, krajnji_saldo_minor, entry_count, closed_at) = row?;
        let purge_eligible = purge_eligible(book_year, &closed_at, today);
        out.push(KepClosureView {
            book_year,
            krajnji_saldo_minor,
            entry_count,
            closed_at,
            purge_eligible,
        });
    }
    Ok(out)
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml kep_close -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Lint + format, then commit**

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
Expected: clean.

```bash
git add src-tauri/src/kep_close.rs
git commit -m "$(cat <<'EOF'
feat(kep): 5-year retention floor + closures list (SW-9c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: Commands + registration

**Files:**
- Modify: `src-tauri/src/commands/kep.rs` (add five commands + a `KepClosePreview` struct; extend the `tests` module)
- Modify: `src-tauri/src/lib.rs` (register the five commands in `invoke_handler`, after `commands::kep::kep_correct_entry` ~line 157)

**Interfaces:**
- Consumes: `crate::kep_close::{close_year, is_year_closed, list_closures, render_close_html, render_book_html, KepClosure, KepClosureView, BOOK_ROWS_PER_PAGE, KepCloseView}`, `crate::kep::list_ledger`, `crate::commands::settings::load_company_settings`, `super::campaigns::write_export`, `crate::commands::reports::ExportedFile`.
- Produces the Tauri commands:
  - `kep_close_preview(book_year) -> KepClosePreview { krajnji_saldo_minor, entry_count, already_closed }`
  - `kep_close_year(book_year, confirmation) -> KepClosure`
  - `kep_list_closures() -> Vec<KepClosureView>`
  - `kep_export_close(book_year) -> ExportedFile`
  - `kep_export_book(book_year) -> ExportedFile`

- [ ] **Step 1: Write the failing command tests**

Add to the `tests` module in `src-tauri/src/commands/kep.rs`:

```rust
    #[test]
    fn kep_close_year_rejected_for_cashier() {
        with_app("kep_close_year_rejected_for_cashier", |app| {
            sign_in_cashier(app.state::<AppState>().inner());
            let error = kep_close_year(app.state::<AppState>(), 2026, "ZAKLJUČI KNJIGU".into())
                .expect_err("cashier should not close the year");
            assert_eq!(error.code, "forbidden");
        });
    }

    #[test]
    fn kep_close_preview_and_close_and_gate() {
        with_app("kep_close_preview_and_close", |app| {
            let state = app.state::<AppState>();
            sign_in_admin(state.inner());
            seed_receipt_zaduzenje(state.inner(), 2026, "2026-06-01T00:00:00Z", 780000);

            // Preview reports the saldo and that the year is open.
            let preview = kep_close_preview(app.state::<AppState>(), 2026).expect("preview");
            assert_eq!(preview.krajnji_saldo_minor, 780000);
            assert_eq!(preview.entry_count, 1);
            assert!(!preview.already_closed);

            // Close it.
            let closure = kep_close_year(app.state::<AppState>(), 2026, "ZAKLJUČI KNJIGU".into())
                .expect("admin closes 2026");
            assert_eq!(closure.krajnji_saldo_minor, 780000);

            // Preview now reports it closed; a second close is invalid_state.
            let preview2 = kep_close_preview(app.state::<AppState>(), 2026).expect("preview2");
            assert!(preview2.already_closed);
            let dbl = kep_close_year(app.state::<AppState>(), 2026, "ZAKLJUČI KNJIGU".into())
                .expect_err("double close");
            assert_eq!(dbl.code, "invalid_state");

            // The closures list carries the closure.
            let closures = kep_list_closures(app.state::<AppState>()).expect("list");
            assert_eq!(closures.len(), 1);
            assert_eq!(closures[0].book_year, 2026);

            // Exports write HTML files.
            let close_doc = kep_export_close(app.state::<AppState>(), 2026).expect("export close");
            assert!(close_doc.file_name.starts_with("kep-zakljucenje-2026"));
            let close_html = std::fs::read_to_string(&close_doc.path).expect("file");
            assert!(close_html.contains("Zaključenje knjige za 2026"));
            std::fs::remove_file(&close_doc.path).ok();

            let book_doc = kep_export_book(app.state::<AppState>(), 2026).expect("export book");
            assert!(book_doc.file_name.starts_with("kep-knjiga-2026"));
            std::fs::remove_file(&book_doc.path).ok();
        });
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml kep_close_preview_and_close_and_gate -- --test-threads=1`
Expected: FAIL to compile — the commands do not exist.

- [ ] **Step 3: Implement the commands**

Add to `src-tauri/src/commands/kep.rs` (extend the top `use` block with `use serde::Serialize;` and the `kep_close` imports). Append after `kep_correct_entry`:

```rust
/// The close-preview payload — the saldo that will carry forward and whether the
/// year is already closed. Read-only; computes nothing it does not also show.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KepClosePreview {
    pub krajnji_saldo_minor: i64,
    pub entry_count: i64,
    pub already_closed: bool,
}

/// Previews a year-end close: the krajnji saldo (including carry-in) and entry
/// count that would be recorded, and whether the year is already closed.
#[tauri::command]
pub fn kep_close_preview(
    state: State<'_, AppState>,
    book_year: i64,
) -> Result<KepClosePreview, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let ledger = crate::kep::list_ledger(&connection, book_year)?;
    let already_closed = crate::kep_close::is_year_closed(&connection, book_year)?;
    Ok(KepClosePreview {
        krajnji_saldo_minor: ledger.saldo_minor,
        entry_count: ledger.entries.len() as i64,
        already_closed,
    })
}

/// Closes a book year irreversibly (typed confirmation required). Records the
/// krajnji saldo; inserts no ledger entry (carry-forward is computed).
#[tauri::command]
pub fn kep_close_year(
    state: State<'_, AppState>,
    book_year: i64,
    confirmation: String,
) -> Result<crate::kep_close::KepClosure, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::kep_close::close_year(&mut connection, book_year, &confirmation, acting.id, &now)
        .map_err(Into::into)
}

/// Lists recorded closures (newest first) with the 5-year retention flag.
#[tauri::command]
pub fn kep_list_closures(
    state: State<'_, AppState>,
) -> Result<Vec<crate::kep_close::KepClosureView>, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let today = crate::clock::utc_now()?;
    crate::kep_close::list_closures(&connection, &today).map_err(Into::into)
}

/// Renders the signable electronic-close document and writes it to `exports/`.
#[tauri::command]
pub fn kep_export_close(
    state: State<'_, AppState>,
    book_year: i64,
) -> Result<ExportedFile, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    if !crate::kep_close::is_year_closed(&connection, book_year)? {
        return Err(AppError::business(
            "invalid_state",
            "Godina nije zaključena — nema šta da se štampa.",
        )
        .into());
    }
    let ledger = crate::kep::list_ledger(&connection, book_year)?;
    let company = crate::commands::settings::load_company_settings(state.inner())?;
    let closure = crate::kep_close::list_closures(&connection, &crate::clock::utc_now()?)?
        .into_iter()
        .find(|c| c.book_year == book_year)
        .ok_or_else(|| AppError::not_found("Zaključenje nije pronađeno."))?;
    let zaduzenje_total_minor: i64 = ledger
        .entries
        .iter()
        .filter_map(|e| e.zaduzenje_minor)
        .sum();
    let razduzenje_total_minor: i64 = ledger
        .entries
        .iter()
        .filter_map(|e| e.razduzenje_minor)
        .sum();
    let view = crate::kep_close::KepCloseView {
        company,
        closure: crate::kep_close::KepClosure {
            book_year: closure.book_year,
            krajnji_saldo_minor: closure.krajnji_saldo_minor,
            entry_count: closure.entry_count,
            closed_at: closure.closed_at,
            closed_by: None,
        },
        opening_saldo_minor: ledger.opening_saldo_minor,
        zaduzenje_total_minor,
        razduzenje_total_minor,
    };
    let html = crate::kep_close::render_close_html(&view);
    let file_name = format!("kep-zakljucenje-{book_year}.html");
    super::campaigns::write_export(state.inner(), &file_name, &html, 1).map_err(Into::into)
}

/// Renders the full-book paginated print and writes it to `exports/`.
#[tauri::command]
pub fn kep_export_book(
    state: State<'_, AppState>,
    book_year: i64,
) -> Result<ExportedFile, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let ledger = crate::kep::list_ledger(&connection, book_year)?;
    let company = crate::commands::settings::load_company_settings(state.inner())?;
    let row_count = ledger.entries.len();
    let html = crate::kep_close::render_book_html(
        &company,
        &ledger,
        crate::kep_close::BOOK_ROWS_PER_PAGE,
    );
    let file_name = format!("kep-knjiga-{book_year}.html");
    super::campaigns::write_export(state.inner(), &file_name, &html, row_count).map_err(Into::into)
}
```

> `AppError` is already imported at the top of `commands/kep.rs` (`use crate::app_error::{AppError, CommandError};`). Add `use serde::Serialize;` to the imports for `KepClosePreview`.

- [ ] **Step 4: Register the commands**

In `src-tauri/src/lib.rs`, in the `tauri::generate_handler!` / `invoke_handler` list, after `commands::kep::kep_correct_entry,` add:

```rust
            commands::kep::kep_close_preview,
            commands::kep::kep_close_year,
            commands::kep::kep_list_closures,
            commands::kep::kep_export_close,
            commands::kep::kep_export_book,
```

- [ ] **Step 5: Run to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`
Expected: PASS — the two new command tests plus the whole suite.

- [ ] **Step 6: Lint + format, then commit**

Run: `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml -- --check`
Expected: clean.

```bash
git add src-tauri/src/commands/kep.rs src-tauri/src/lib.rs
git commit -m "$(cat <<'EOF'
feat(kep): close/preview/list/export commands + registration (SW-9c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: Frontend service wiring

**Files:**
- Modify: `src/services/types.ts` (add `KepClosePreview`, `KepClosure`, `KepClosureView` interfaces near `KepStatus` ~line 1018)
- Modify: `src/services/ports.ts` (extend `KepService` ~line 233)
- Modify: `src/services/local-adapter.ts` (extend the `kep` block ~line 252)
- Modify: `src/services/mock-adapter.ts` (extend the `kep` stub ~line 1234)
- Test: `src/services/local-adapter.test.ts` (extend the KEP invoke-mapping test ~line 759)

**Interfaces:**
- Consumes: the Task 7 commands (`kep_close_preview`, `kep_close_year`, `kep_list_closures`, `kep_export_close`, `kep_export_book`) and `ExportedFile`.
- Produces `KepService` gains: `closePreview(bookYear)`, `closeYear(bookYear, confirmation)`, `listClosures()`, `exportClose(bookYear)`, `exportBook(bookYear)`.

- [ ] **Step 1: Write the failing adapter test**

In `src/services/local-adapter.test.ts`, extend the KEP mapping test (the `switch` over command names and the `toHaveBeenNthCalledWith` assertions). Add cases to the mock `invoke` and assertions:

```typescript
        case "kep_close_preview":
          return { krajnjiSaldoMinor: 780000, entryCount: 1, alreadyClosed: false };
        case "kep_close_year":
          return {
            bookYear: 2026,
            krajnjiSaldoMinor: 780000,
            entryCount: 1,
            closedAt: "2027-01-05T09:00:00Z",
            closedBy: 1,
          };
        case "kep_list_closures":
          return [];
        case "kep_export_close":
        case "kep_export_book":
          return {
            fileName: "kep-doc.html",
            path: "exports/kep-doc.html",
            mimeType: "text/html",
            rowCount: 0,
          };
```

And, in the sequence that drives the adapter, call the new methods and assert their command mapping, e.g.:

```typescript
    await services.kep.closePreview(2026);
    await services.kep.closeYear(2026, "ZAKLJUČI KNJIGU");
    await services.kep.listClosures();
    await services.kep.exportClose(2026);
    await services.kep.exportBook(2026);

    expect(invoke).toHaveBeenCalledWith("kep_close_preview", { bookYear: 2026 });
    expect(invoke).toHaveBeenCalledWith("kep_close_year", {
      bookYear: 2026,
      confirmation: "ZAKLJUČI KNJIGU",
    });
    expect(invoke).toHaveBeenCalledWith("kep_list_closures");
    expect(invoke).toHaveBeenCalledWith("kep_export_close", { bookYear: 2026 });
    expect(invoke).toHaveBeenCalledWith("kep_export_book", { bookYear: 2026 });
```

> Match the existing test's structure — if it uses `toHaveBeenNthCalledWith` with explicit ordinals, either append with the correct next ordinals or use `toHaveBeenCalledWith` for the additions (order-independent). Keep the pre-existing assertions intact.

- [ ] **Step 2: Run to verify it fails**

Run: `bun run test src/services/local-adapter.test.ts`
Expected: FAIL — `services.kep.closePreview` is not a function (types/adapter not extended).

- [ ] **Step 3: Add the types**

In `src/services/types.ts`, after `KepStatus`:

```typescript
/** The close-preview payload — mirrors `crate::commands::kep::KepClosePreview`. */
export interface KepClosePreview {
  krajnjiSaldoMinor: number;
  entryCount: number;
  alreadyClosed: boolean;
}

/** A recorded year-end close — mirrors `crate::kep_close::KepClosure`. */
export interface KepClosure {
  bookYear: number;
  krajnjiSaldoMinor: number;
  entryCount: number;
  closedAt: string;
  closedBy: number | null;
}

/** A closure list row with the 5-year retention flag — `crate::kep_close::KepClosureView`. */
export interface KepClosureView {
  bookYear: number;
  krajnjiSaldoMinor: number;
  entryCount: number;
  closedAt: string;
  purgeEligible: boolean;
}
```

- [ ] **Step 4: Extend the port**

In `src/services/ports.ts`, add to the `KepService` interface (import the new types are already re-exported via the shared types module; add them to the existing type import if `ports.ts` imports named types explicitly):

```typescript
  closePreview(bookYear: number): Promise<KepClosePreview>;
  closeYear(bookYear: number, confirmation: string): Promise<KepClosure>;
  listClosures(): Promise<KepClosureView[]>;
  exportClose(bookYear: number): Promise<ExportedFile>;
  exportBook(bookYear: number): Promise<ExportedFile>;
```

- [ ] **Step 5: Extend the local adapter**

In `src/services/local-adapter.ts`, add to the `kep` block (import `KepClosePreview`, `KepClosure`, `KepClosureView` alongside the existing KEP type imports):

```typescript
      closePreview: (bookYear) =>
        invoke<KepClosePreview>("kep_close_preview", { bookYear }),
      closeYear: (bookYear, confirmation) =>
        invoke<KepClosure>("kep_close_year", { bookYear, confirmation }),
      listClosures: () => invoke<KepClosureView[]>("kep_list_closures"),
      exportClose: (bookYear) =>
        invoke<ExportedFile>("kep_export_close", { bookYear }),
      exportBook: (bookYear) =>
        invoke<ExportedFile>("kep_export_book", { bookYear }),
```

- [ ] **Step 6: Extend the mock adapter**

In `src/services/mock-adapter.ts`, add to the `kep` stub an in-memory closures list so the UI's close→disable→list flow is exercised. Near the `kepEntries` declaration, add `const kepClosures: KepClosureView[] = [];`. Then:

```typescript
      async closePreview(bookYear) {
        const already = kepClosures.some((c) => c.bookYear === bookYear);
        const saldoMinor = kepEntries.reduce(
          (sum, entry) =>
            sum + (entry.zaduzenjeMinor ?? 0) - (entry.razduzenjeMinor ?? 0),
          0,
        );
        return {
          krajnjiSaldoMinor: saldoMinor,
          entryCount: kepEntries.length,
          alreadyClosed: already,
        };
      },
      async closeYear(bookYear, confirmation) {
        if (confirmation !== "ZAKLJUČI KNJIGU") {
          throw new Error("Potvrda nije ispravna.");
        }
        if (kepClosures.some((c) => c.bookYear === bookYear)) {
          throw new Error("Godina je već zaključena.");
        }
        const saldoMinor = kepEntries.reduce(
          (sum, entry) =>
            sum + (entry.zaduzenjeMinor ?? 0) - (entry.razduzenjeMinor ?? 0),
          0,
        );
        const closure: KepClosure = {
          bookYear,
          krajnjiSaldoMinor: saldoMinor,
          entryCount: kepEntries.length,
          closedAt: "2027-01-05T09:00:00Z",
          closedBy: 1,
        };
        kepClosures.push({
          bookYear,
          krajnjiSaldoMinor: saldoMinor,
          entryCount: kepEntries.length,
          closedAt: closure.closedAt,
          purgeEligible: false,
        });
        return closure;
      },
      async listClosures() {
        return [...kepClosures];
      },
      async exportClose(bookYear) {
        return {
          fileName: `kep-zakljucenje-${bookYear}.html`,
          path: `mock://exports/kep-zakljucenje-${bookYear}.html`,
          mimeType: "text/html" as const,
          rowCount: 1,
        };
      },
      async exportBook(bookYear) {
        return {
          fileName: `kep-knjiga-${bookYear}.html`,
          path: `mock://exports/kep-knjiga-${bookYear}.html`,
          mimeType: "text/html" as const,
          rowCount: kepEntries.length,
        };
      },
```

Import `KepClosure`, `KepClosureView` at the top of `mock-adapter.ts` alongside the existing KEP type imports.

- [ ] **Step 7: Run tests + build to verify pass**

Run: `bun run test && bun run build`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/services/types.ts src/services/ports.ts src/services/local-adapter.ts src/services/mock-adapter.ts src/services/local-adapter.test.ts
git commit -m "$(cat <<'EOF'
feat(kep): frontend service contract for year-end close (SW-9c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

### Task 9: KEP module UI — close dialog, print buttons, closed-year badge, closures list

**Files:**
- Modify: `src/app/kep/KepModule.tsx`
- Test: `src/app/kep/KepModule.test.tsx`

**Interfaces:**
- Consumes: `services.kep.{closePreview, closeYear, listClosures, exportClose, exportBook}`, `services.print.openForPrint`, `KepClosureView`, `KepClosePreview`, the `KepLedger.openingSaldoMinor` field (Task 2), and the existing module's `services`/state idioms.
- Produces: UI only (no new exported symbols).

- [ ] **Step 1: Write the failing UI tests**

In `src/app/kep/KepModule.test.tsx`, add tests that mirror the module's existing render/act idioms (reuse its `renderModule`/service-mock helper). Cover:

```typescript
  it("shows the opening carry-in row (Početno stanje)", async () => {
    // Arrange a ledger with a non-zero openingSaldoMinor via the mock service,
    // render, and assert the DONOS/opening line appears.
    renderModule();
    expect(await screen.findByText(/Početno stanje/i)).toBeInTheDocument();
  });

  it("closes the year through the typed confirmation dialog", async () => {
    renderModule();
    await userEvent.click(await screen.findByRole("button", { name: /Zaključi godinu/i }));
    const input = await screen.findByLabelText(/Potvrda/i);
    // The confirm button is disabled until the exact phrase is typed.
    const confirm = screen.getByRole("button", { name: /Potvrdi zaključenje/i });
    expect(confirm).toBeDisabled();
    await userEvent.type(input, "ZAKLJUČI KNJIGU");
    expect(confirm).toBeEnabled();
    await userEvent.click(confirm);
    // After closing, the closed-year badge appears and postings are hidden/disabled.
    expect(await screen.findByText(/Zaključena/i)).toBeInTheDocument();
  });

  it("disables posting actions for a closed year", async () => {
    // Render with the mock reporting the year already closed, assert the
    // daily-sales / adjustment actions are absent or disabled.
    renderModuleWithClosedYear();
    expect(screen.queryByRole("button", { name: /Proknjiži dnevni promet/i })).toBeNull();
  });
```

> Adapt the queries to the module's actual Serbian button labels and its test harness. The three fixed contracts: (a) an opening/`Početno stanje` row renders when `openingSaldoMinor > 0`; (b) the close confirm button is disabled until `ZAKLJUČI KNJIGU` is typed exactly and closing shows a *Zaključena* badge; (c) a closed year hides/disables the posting actions. If the harness cannot easily force "already closed", drive it by performing the close in-test (the mock adapter from Task 8 flips `listClosures`/`closePreview`).

- [ ] **Step 2: Run to verify it fails**

Run: `bun run test src/app/kep/KepModule.test.tsx`
Expected: FAIL — the close button / confirmation dialog / badge do not exist yet.

- [ ] **Step 3: Implement the UI**

In `src/app/kep/KepModule.tsx`, following the module's existing patterns (its `services` access, its `useState`/`useEffect` data loads, its shadcn `Dialog`/`Button`/`Input` usage, its Serbian copy):

1. On load (or when the book year changes), call `services.kep.closePreview(bookYear)` and `services.kep.listClosures()`; store `alreadyClosed` and the closures array in state.
2. Render a **closed-year badge** (`Zaključena` — reuse the module's badge component) on the ledger header when `alreadyClosed`. When closed, **hide or `disabled`** every posting/adjustment control (daily-sales post, kalkulacija, nivelacija, adjustment, correction).
3. Render the **opening carry-in** as a leading `Početno stanje` row in the ledger table using `ledger.openingSaldoMinor` (show it whenever non-zero, or always if the module prefers).
4. Add a **„Zaključi godinu"** button (hidden when `alreadyClosed`) that opens a confirmation `Dialog`:
   - The dialog shows the krajnji saldo that will carry forward (`closePreview.krajnjiSaldoMinor`, formatted with the module's existing RSD formatter) and the entry count.
   - A text `Input` labelled `Potvrda` (aria-label/label „Potvrda"); the confirm button („Potvrdi zaključenje") is `disabled` until the input value `=== "ZAKLJUČI KNJIGU"`.
   - On confirm: `await services.kep.closeYear(bookYear, input)`, then refresh preview + closures + ledger and close the dialog. Surface a thrown error via the module's existing error/toast idiom.
5. Add **„Štampaj zaključenje"** and **„Štampaj celu knjigu"** buttons that call `services.kep.exportClose(bookYear)` / `exportBook(bookYear)` then `services.print.openForPrint(result.path)` — mirroring the existing kalkulacija export-then-open handler.
6. Render a **closures list** section: for each `KepClosureView`, show `bookYear`, `krajnjiSaldoMinor` (formatted), `closedAt` (date), and a retention indicator driven by `purgeEligible` (e.g. „Može se arhivirati" when true, otherwise „Čuva se do {book_year + 5}." — nothing auto-deletes; this is informational only).

Use exact, existing helpers from the module (RSD formatter, badge, dialog); do not introduce a new formatting utility.

- [ ] **Step 4: Run the UI tests + build to verify pass**

Run: `bun run test && bun run build`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/app/kep/KepModule.tsx src/app/kep/KepModule.test.tsx
git commit -m "$(cat <<'EOF'
feat(kep): year-end close UI (dialog, print, closed-year badge, closures list) (SW-9c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
EOF
)"
```

---

## Final verification (run after Task 9)

- [ ] **Full gate sweep**

Run, from the repo root:

```bash
bun run test \
&& bun run build \
&& cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 \
&& cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings \
&& cargo fmt --manifest-path src-tauri/Cargo.toml -- --check \
&& git diff --check
```

Expected: all pass, no warnings, no whitespace errors. Do NOT boot the app.

---

## Self-Review (plan vs. spec)

**Spec coverage:**
- §1 Schema v14 → Task 1 (table, indexes, count assertion, reset wipe). ✅
- §2 Carry-forward computed in `list_ledger` + `opening_saldo_minor` → Task 2. ✅
- §3 `kep_close.rs` (`CLOSE_CONFIRMATION`, `is_year_closed`, `ensure_year_open`, `close_year`) + gate wired into all six posting paths → Tasks 3 & 4. ✅
- §4 `render_close_html` (signable) + `render_book_html` (paginated DONOS/SVEGA, `Strana n`, per-page subtotals) → Task 5. ✅
- §5 Retention `purge_eligible` (later-of floor) + closures list → Task 6. ✅
- §6 Commands (`kep_close_preview`, `kep_close_year`, `kep_list_closures`, `kep_export_close`, `kep_export_book`) all `require_admin` + registration → Task 7. ✅
- §7 UI (Zaključi godinu typed-confirm dialog, print buttons, closed-year badge disabling postings, closures list with retention, opening carry-in row) → Task 9; service contract → Task 8. ✅
- §8 Tests: carry-forward (T2), gate rejects postings (T4 + T7), double-close→invalid_state (T3, T7), wrong confirmation→validation (T3), reset clears closures (T1), retention flips (T6), print paginates (T5). ✅

**Placeholder scan:** every code step carries complete code; test-harness adaptation notes point at concrete existing idioms (accessor `code()` vs `matches!`, `with_kep_db` fixture, `reset_trading_data` inner) rather than leaving logic unspecified. The frontend UI task (T9) specifies exact contracts and handler behavior; its JSX is described against the module's existing components because the file is large and pattern-dense — the three testable contracts are fixed.

**Type consistency:** `opening_saldo_minor`/`openingSaldoMinor`, `krajnji_saldo_minor`/`krajnjiSaldoMinor`, `KepClosure`, `KepClosureView`, `KepClosePreview`, `ensure_year_open`, `close_year(&mut Connection, i64, &str, i64, &str)`, `render_close_html(&KepCloseView)`, `render_book_html(&CompanySettings, &KepLedger, usize)`, `purge_eligible(i64, &str, &str)`, `BOOK_ROWS_PER_PAGE`, `CLOSE_CONFIRMATION` are used identically across every task that references them. Command names (`kep_close_preview`, `kep_close_year`, `kep_list_closures`, `kep_export_close`, `kep_export_book`) match between Task 7 (backend), Task 8 (adapter), and the registration list.
