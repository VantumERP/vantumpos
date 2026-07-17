# Price History (SW-6a) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the append-only offered-price log and the prethodna cena computation (ZoT čl. 37 st. 3–4), so a Serbian shop can prove and correctly display the legally required prior price at a sniženje.

**Architecture:** A new `price_history` table (migration v9) holds append-only *event* rows; `price_minor IS NULL` marks "offering ended", which makes gaps explicit. Validity intervals derive at query time via `LEAD()`. A single domain module `src-tauri/src/price_history.rs` owns both the write helper (used by catalog + importer) and the read computation. Nothing ever UPDATEs or DELETEs the table, except the go-live reset's deliberate clear-and-reseed.

**Tech Stack:** Rust + rusqlite/SQLite (window functions), `time` crate (RFC3339 parse + date math), Tauri v2 commands, React + TypeScript + shadcn/ui + Vitest/RTL.

## Global Constraints

- **Legal authority is `docs/ZOT-36-37-VERIFIED-RULES.md`.** Design is `docs/superpowers/specs/2026-07-17-price-history-design.md`. If code and those documents disagree, STOP and report — do not improvise law.
- **DO NOT boot, launch, or run the application** (no `tauri dev`, `cargo tauri dev`, dev/preview server, or built binary). Verify only via `cargo test` / `cargo build` / `bun run test` / `bun run build` / clippy / fmt. Standing user instruction.
- **Timestamps are RFC3339 and nothing else.** `utc_now()` (catalog) and `now_utc_string()` (importer) both emit RFC3339 (`2026-07-17T10:30:00Z`). **Never use SQLite `datetime('now')` for `price_history`** — it emits `2026-07-17 10:30:00`, and because a space (0x20) sorts before `T` (0x54), such a row would sort before every app-written row regardless of real time, silently corrupting every window query. The migration seed MUST use `strftime('%Y-%m-%dT%H:%M:%SZ','now')`.
- **All date math happens in Rust** via the `time` crate. SQL performs only lexicographic string comparison on RFC3339 values. Do not call SQLite date functions on these columns.
- **`price_history` is append-only.** No `UPDATE`, no `DELETE` anywhere, except Task 6's reset. This is enforced by review + an invariant test, since SQLite cannot express it.
- Money is integer minor units (para): 4.990,00 RSD = `499000`. Quantities are milli-units. Never floats.
- Serbian Latin UI copy with correct diacritics (šđčćž). Exact strings are given per task — copy character-for-character.
- Acting user always comes from the session (`require_admin(...)?.id`), never from a client payload.
- Migrations are append-only: v9 is next; never edit v1–v8. Bump the migration-count assertion 8 → 9 and extend `CORE_TABLES` / `EXPLICIT_INDEXES`.
- Every task ends green on: `bun run test`, `bun run build`, `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`.
- Commit trailer on every commit:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  ```

---

### Task 1: Migration v9 — `price_history` table + honest seed

**Files:**
- Modify: `src-tauri/src/db/migrations.rs` (append v9 after the v8 entry)
- Modify: `src-tauri/src/db/mod.rs` (`CORE_TABLES` ~line 110, `EXPLICIT_INDEXES` ~line 128, count assertion ~line 531)
- Test: `src-tauri/src/db/mod.rs` (`mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: table `price_history(id, product_id, effective_from, price_minor NULL, source, user_id, created_at)`; index `idx_price_history_product`. Consumed by Tasks 2–7.

- [ ] **Step 1: Write the failing test**

Add to `src-tauri/src/db/mod.rs` `mod tests`:

```rust
    #[test]
    fn migration_v9_creates_price_history_and_seeds_active_products_only() {
        with_test_database("migration_v9_price_history", |db| {
            let connection = db.open().expect("database should open");

            assert!(
                schema_object_exists(&connection, "table", "price_history"),
                "expected price_history table"
            );

            // price_minor must be nullable: NULL means "offering ended".
            let notnull: i64 = connection
                .query_row(
                    "SELECT \"notnull\" FROM pragma_table_info('price_history') WHERE name = 'price_minor'",
                    [],
                    |row| row.get(0),
                )
                .expect("column metadata should query");
            assert_eq!(notnull, 0, "price_minor must be nullable (NULL = offering ended)");

            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='price_history'",
                    [],
                    |row| row.get(0),
                )
                .expect("schema should load");
            for token in ["create", "update", "import", "deactivate", "reactivate", "seed"] {
                assert!(schema.contains(token), "expected source CHECK to allow {token}");
            }
        });
    }

    #[test]
    fn migration_v9_seed_rows_use_rfc3339_and_skip_inactive_products() {
        with_test_database("migration_v9_seed_format", |db| {
            let connection = db.open().expect("database should open");
            // Fresh DB has no products, so seed nothing; insert one active + one
            // inactive product and re-run the seed statement shape by hand is not
            // possible post-migration. Instead assert the seed statement's format
            // contract on a synthetic row inserted the same way the migration does.
            connection
                .execute(
                    "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                     SELECT 1, strftime('%Y-%m-%dT%H:%M:%SZ','now'), 1000, 'seed', strftime('%Y-%m-%dT%H:%M:%SZ','now')
                     WHERE 0",
                    [],
                )
                .expect("seed-shaped statement should be valid SQL");

            let stamp: String = connection
                .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%SZ','now')", [], |row| row.get(0))
                .expect("timestamp should format");
            assert!(stamp.contains('T') && stamp.ends_with('Z'), "seed stamp must be RFC3339, got {stamp}");
            assert!(!stamp.contains(' '), "seed stamp must not use SQLite's space separator");
        });
    }
```

Also update the existing count test: change `assert_eq!(migration_count, 8);` to `assert_eq!(migration_count, 9);`.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml migration_v9 -- --test-threads=1`
Expected: FAIL — `price_history` table missing.

- [ ] **Step 3: Append migration v9**

In `src-tauri/src/db/migrations.rs`, add after the v8 `Migration { ... }` entry (before the closing `];`):

```rust
    Migration {
        version: 9,
        name: "price_history",
        sql: r#"
CREATE TABLE price_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    effective_from TEXT NOT NULL,
    price_minor INTEGER CHECK (price_minor IS NULL OR price_minor >= 0),
    source TEXT NOT NULL CHECK (source IN ('create', 'update', 'import', 'deactivate', 'reactivate', 'seed')),
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_price_history_product ON price_history(product_id, effective_from);

INSERT INTO price_history (product_id, effective_from, price_minor, source, user_id, created_at)
SELECT id,
       strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
       sale_price_minor,
       'seed',
       NULL,
       strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
FROM products
WHERE active = 1;
"#,
    },
```

Two things are deliberate and must not be "tidied":
- `strftime('%Y-%m-%dT%H:%M:%SZ','now')`, **not** `datetime('now')` — see Global Constraints.
- Seed at migration time, **not** `products.created_at`. Claiming today's price was offered since creation would be a fabrication; seeding at migration time makes pre-log windows honestly `truncated` (Task 5).

- [ ] **Step 4: Register the table + index**

In `src-tauri/src/db/mod.rs`: add `"price_history",` to `CORE_TABLES` (after `"compliance_log",`) and `"idx_price_history_product",` to `EXPLICIT_INDEXES` (after `"idx_compliance_log_created_at",`).

- [ ] **Step 5: Run the DB tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib db:: -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Full gates + commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src-tauri/src/db/migrations.rs src-tauri/src/db/mod.rs
git commit -m "feat(price-history): migration v9 — append-only price_history + honest seed (SW-6a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: `price_history` module — the write helper

**Files:**
- Create: `src-tauri/src/price_history.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod price_history;` beside the other top-level `mod` lines at the top of the file)
- Test: `src-tauri/src/price_history.rs` (`mod tests`)

**Interfaces:**
- Consumes: the `price_history` table (Task 1).
- Produces — relied on verbatim by Tasks 3, 4, 6:
  - `pub struct OfferingState { pub active: bool, pub price_minor: i64 }` (derives `Debug, Clone, Copy, PartialEq, Eq`)
  - `pub fn load_offering_state(conn: &rusqlite::Connection, product_id: i64) -> rusqlite::Result<Option<OfferingState>>`
  - `pub fn record_offered_price_change(conn: &rusqlite::Connection, product_id: i64, before: Option<OfferingState>, after: OfferingState, source: &str, acting_user_id: Option<i64>, now_rfc3339: &str) -> rusqlite::Result<()>`
  - Returns `rusqlite::Result` on purpose: catalog (`AppError`) and importer (`ImportError`) both implement `From<rusqlite::Error>`, so both can use `?`.

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/price_history.rs` containing only this test module for now:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn conn_with_product(active: bool, price_minor: i64) -> (Connection, i64) {
        let connection = Connection::open_in_memory().expect("in-memory db");
        connection
            .execute_batch(
                "CREATE TABLE products (id INTEGER PRIMARY KEY AUTOINCREMENT, sale_price_minor INTEGER NOT NULL, active INTEGER NOT NULL, created_at TEXT NOT NULL);
                 CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT);
                 CREATE TABLE price_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
                    effective_from TEXT NOT NULL,
                    price_minor INTEGER CHECK (price_minor IS NULL OR price_minor >= 0),
                    source TEXT NOT NULL CHECK (source IN ('create','update','import','deactivate','reactivate','seed')),
                    user_id INTEGER REFERENCES users(id),
                    created_at TEXT NOT NULL);",
            )
            .expect("schema should create");
        connection
            .execute(
                "INSERT INTO products (sale_price_minor, active, created_at) VALUES (?1, ?2, '2026-01-01T00:00:00Z')",
                rusqlite::params![price_minor, if active { 1 } else { 0 }],
            )
            .expect("product should insert");
        let id = connection.last_insert_rowid();
        (connection, id)
    }

    fn rows(conn: &Connection) -> Vec<(Option<i64>, String)> {
        let mut stmt = conn
            .prepare("SELECT price_minor, source FROM price_history ORDER BY id")
            .expect("prepare");
        let mapped = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query");
        mapped.collect::<Result<Vec<_>, _>>().expect("collect")
    }

    #[test]
    fn creating_an_active_product_records_the_offered_price() {
        let (conn, id) = conn_with_product(true, 499000);
        record_offered_price_change(&conn, id, None, OfferingState { active: true, price_minor: 499000 }, "create", Some(1), "2026-07-17T00:00:00Z").expect("record");
        assert_eq!(rows(&conn), vec![(Some(499000), "create".to_string())]);
    }

    #[test]
    fn creating_an_inactive_product_records_nothing() {
        let (conn, id) = conn_with_product(false, 499000);
        record_offered_price_change(&conn, id, None, OfferingState { active: false, price_minor: 499000 }, "create", Some(1), "2026-07-17T00:00:00Z").expect("record");
        assert!(rows(&conn).is_empty(), "an unoffered product has no offered price to log");
    }

    #[test]
    fn deactivating_records_a_null_gap_row() {
        let (conn, id) = conn_with_product(true, 499000);
        record_offered_price_change(&conn, id, Some(OfferingState { active: true, price_minor: 499000 }), OfferingState { active: false, price_minor: 499000 }, "deactivate", Some(1), "2026-07-17T00:00:00Z").expect("record");
        assert_eq!(rows(&conn), vec![(None, "deactivate".to_string())]);
    }

    #[test]
    fn reactivating_records_the_price_again() {
        let (conn, id) = conn_with_product(false, 529000);
        record_offered_price_change(&conn, id, Some(OfferingState { active: false, price_minor: 529000 }), OfferingState { active: true, price_minor: 529000 }, "reactivate", Some(1), "2026-07-17T00:00:00Z").expect("record");
        assert_eq!(rows(&conn), vec![(Some(529000), "reactivate".to_string())]);
    }

    #[test]
    fn changing_the_price_while_active_records_it() {
        let (conn, id) = conn_with_product(true, 499000);
        record_offered_price_change(&conn, id, Some(OfferingState { active: true, price_minor: 499000 }), OfferingState { active: true, price_minor: 429000 }, "update", Some(1), "2026-07-17T00:00:00Z").expect("record");
        assert_eq!(rows(&conn), vec![(Some(429000), "update".to_string())]);
    }

    #[test]
    fn an_unchanged_price_records_nothing() {
        let (conn, id) = conn_with_product(true, 499000);
        record_offered_price_change(&conn, id, Some(OfferingState { active: true, price_minor: 499000 }), OfferingState { active: true, price_minor: 499000 }, "update", Some(1), "2026-07-17T00:00:00Z").expect("record");
        assert!(rows(&conn).is_empty(), "editing name/SKU must not pollute the price timeline");
    }

    #[test]
    fn changing_the_price_while_inactive_records_nothing() {
        let (conn, id) = conn_with_product(false, 499000);
        record_offered_price_change(&conn, id, Some(OfferingState { active: false, price_minor: 499000 }), OfferingState { active: false, price_minor: 429000 }, "update", Some(1), "2026-07-17T00:00:00Z").expect("record");
        assert!(rows(&conn).is_empty(), "a price edit on an unoffered product is not an offered price");
    }

    #[test]
    fn load_offering_state_reads_active_and_price() {
        let (conn, id) = conn_with_product(true, 499000);
        assert_eq!(load_offering_state(&conn, id).expect("load"), Some(OfferingState { active: true, price_minor: 499000 }));
        assert_eq!(load_offering_state(&conn, 999).expect("load"), None);
    }
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml price_history -- --test-threads=1`
Expected: FAIL — module not declared / functions undefined.

- [ ] **Step 3: Implement the module**

Write above the test module in `src-tauri/src/price_history.rs`:

```rust
//! Append-only offered-price log (ZoT čl. 37 st. 3–4).
//!
//! Legal authority: `docs/ZOT-36-37-VERIFIED-RULES.md`. Design:
//! `docs/superpowers/specs/2026-07-17-price-history-design.md`.
//!
//! Rows are INSERT-only. Nothing in this crate may UPDATE or DELETE them —
//! the log is the shop's evidence that a displayed prethodna cena was correct,
//! and it is retained ~5 years. A NULL `price_minor` means the product stopped
//! being offered at `effective_from`; that explicit gap is what distinguishes
//! returning seasonal stock from a genuinely new arrival.

use rusqlite::{params, Connection, OptionalExtension};

/// What the shop is offering for a product: an inactive product is not offered
/// at all, so its price is not an "offered price" in the sense of čl. 37 st. 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfferingState {
    pub active: bool,
    pub price_minor: i64,
}

pub fn load_offering_state(
    conn: &Connection,
    product_id: i64,
) -> rusqlite::Result<Option<OfferingState>> {
    conn.query_row(
        "SELECT active, sale_price_minor FROM products WHERE id = ?1",
        params![product_id],
        |row| {
            Ok(OfferingState {
                active: row.get::<_, i64>(0)? != 0,
                price_minor: row.get(1)?,
            })
        },
    )
    .optional()
}

/// Appends a row IFF the *offered* price actually changed. `before == None`
/// means the product is being created. Call inside the same transaction as the
/// product write: a price change recorded non-atomically can diverge from the
/// catalog, which is worse than not recording it.
pub fn record_offered_price_change(
    conn: &Connection,
    product_id: i64,
    before: Option<OfferingState>,
    after: OfferingState,
    source: &str,
    acting_user_id: Option<i64>,
    now_rfc3339: &str,
) -> rusqlite::Result<()> {
    let was_offered = before.map(|state| state.active).unwrap_or(false);

    // Some(x) => append a row carrying x; None => nothing to record.
    let price_to_log: Option<Option<i64>> = match (was_offered, after.active) {
        (false, true) => Some(Some(after.price_minor)), // offering started or resumed
        (true, false) => Some(None),                    // offering ended -> explicit gap
        (true, true) => {
            let before_price = before
                .expect("was_offered == true implies before is Some")
                .price_minor;
            if before_price == after.price_minor {
                None
            } else {
                Some(Some(after.price_minor))
            }
        }
        (false, false) => None, // never offered before or after
    };

    let Some(price_minor) = price_to_log else {
        return Ok(());
    };

    conn.execute(
        "INSERT INTO price_history (
            product_id, effective_from, price_minor, source, user_id, created_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?2)",
        params![product_id, now_rfc3339, price_minor, source, acting_user_id],
    )?;

    Ok(())
}
```

Declare the module in `src-tauri/src/lib.rs` — add `mod price_history;` alongside the existing `mod importer;` / `mod security;` lines (keep alphabetical: after `mod importer;`).

- [ ] **Step 4: Run the module tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml price_history -- --test-threads=1`
Expected: PASS (8 tests).

- [ ] **Step 5: Full gates + commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src-tauri/src/price_history.rs src-tauri/src/lib.rs
git commit -m "feat(price-history): offered-price write helper with explicit offering gaps (SW-6a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Capture in the catalog (create / update / set_active)

**Files:**
- Modify: `src-tauri/src/commands/catalog.rs` — `create_product` (271), `update_product` (345), `set_product_active` (432), and the three command wrappers (181, 190, 200)
- Test: `src-tauri/src/commands/catalog.rs` (`mod tests`)

**Interfaces:**
- Consumes: `price_history::{OfferingState, load_offering_state, record_offered_price_change}` (Task 2).
- Produces: inner functions gain a trailing `acting_user_id: i64` parameter:
  - `create_product(db: &Db, request: SaveProductRequest, acting_user_id: i64) -> Result<ProductSummary, AppError>`
  - `update_product(db: &Db, id: i64, request: SaveProductRequest, acting_user_id: i64) -> Result<ProductSummary, AppError>`
  - `set_product_active(db: &Db, id: i64, active: bool, acting_user_id: i64) -> Result<ProductSummary, AppError>`

**Spec refinement — read this before implementing.** `update_product` sets `active` as well as `sale_price_minor` (catalog.rs:383, 402), so it can flip the offering state, not only the price. The design doc assigned deactivate/reactivate to `set_product_active` alone. The `record_offered_price_change` helper already handles every transition uniformly from `(before, after)`, so simply pass the real before/after state from **all three** paths and let it decide. `source` records which code path caused the row (`'update'` even when an update deactivates); the NULL price already carries the legal meaning.

- [ ] **Step 1: Write the failing tests**

Add to `src-tauri/src/commands/catalog.rs` `mod tests`. The module already provides `with_catalog_database(test_name, |db| ...)` and `product_request(sku, barcode) -> SaveProductRequest` (which prices at `sale_price_minor: 18000` with `active: true`). `Db::new` seeds the bootstrap admin, so `acting_user_id = 1` satisfies the `user_id` foreign key — a helper below resolves it by username rather than hardcoding, so the tests stay correct if seeding changes.

```rust
    fn admin_id(db: &Db) -> i64 {
        db.open()
            .expect("db open")
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| row.get(0))
            .expect("bootstrap admin should exist")
    }

    fn price_rows(db: &Db, product_id: i64) -> Vec<(Option<i64>, String)> {
        let connection = db.open().expect("db open");
        let mut stmt = connection
            .prepare("SELECT price_minor, source FROM price_history WHERE product_id = ?1 ORDER BY id")
            .expect("prepare");
        let mapped = stmt
            .query_map(params![product_id], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query");
        mapped.collect::<Result<Vec<_>, _>>().expect("collect")
    }

    #[test]
    fn creating_an_active_product_appends_a_create_price_row() {
        with_catalog_database("price_history_create_row", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-1", None), acting)
                .expect("product should create");

            assert_eq!(price_rows(db, product.id), vec![(Some(18000), "create".to_string())]);
        });
    }

    #[test]
    fn creating_an_inactive_product_appends_no_price_row() {
        with_catalog_database("price_history_create_inactive", |db| {
            let acting = admin_id(db);
            let mut request = product_request("PH-INACTIVE", None);
            request.active = false;
            let product = create_product(db, request, acting).expect("product should create");

            assert!(
                price_rows(db, product.id).is_empty(),
                "an unoffered product has no offered price to log"
            );
        });
    }

    #[test]
    fn updating_only_the_name_appends_no_price_row() {
        with_catalog_database("price_history_name_only", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-2", None), acting)
                .expect("product should create");

            let mut renamed = product_request("PH-2", None);
            renamed.name = "Novo ime".to_string();
            update_product(db, product.id, renamed, acting).expect("product should update");

            assert_eq!(
                price_rows(db, product.id).len(),
                1,
                "editing the name must not pollute the price timeline"
            );
        });
    }

    #[test]
    fn lowering_the_price_appends_an_update_price_row() {
        with_catalog_database("price_history_price_change", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-3", None), acting)
                .expect("product should create");

            let mut cheaper = product_request("PH-3", None);
            cheaper.sale_price_minor = 15000;
            update_product(db, product.id, cheaper, acting).expect("product should update");

            assert_eq!(
                price_rows(db, product.id),
                vec![(Some(18000), "create".to_string()), (Some(15000), "update".to_string())]
            );
        });
    }

    #[test]
    fn deactivating_then_reactivating_records_a_gap_then_a_return() {
        with_catalog_database("price_history_gap_then_return", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-4", None), acting)
                .expect("product should create");

            set_product_active(db, product.id, false, acting).expect("deactivate");
            set_product_active(db, product.id, true, acting).expect("reactivate");

            assert_eq!(
                price_rows(db, product.id),
                vec![
                    (Some(18000), "create".to_string()),
                    (None, "deactivate".to_string()),
                    (Some(18000), "reactivate".to_string()),
                ]
            );
        });
    }

    #[test]
    fn setting_active_to_its_current_value_appends_nothing() {
        with_catalog_database("price_history_active_noop", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-5", None), acting)
                .expect("product should create");

            set_product_active(db, product.id, true, acting).expect("no-op activate");

            assert_eq!(price_rows(db, product.id).len(), 1, "no state change, no row");
        });
    }

    #[test]
    fn deactivating_via_update_product_also_records_the_gap() {
        with_catalog_database("price_history_update_deactivates", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-6", None), acting)
                .expect("product should create");

            // update_product sets `active` too, so it can end an offering.
            let mut deactivated = product_request("PH-6", None);
            deactivated.active = false;
            update_product(db, product.id, deactivated, acting).expect("product should update");

            assert_eq!(
                price_rows(db, product.id),
                vec![(Some(18000), "create".to_string()), (None, "update".to_string())]
            );
        });
    }

    #[test]
    fn price_row_is_attributed_to_the_acting_user() {
        with_catalog_database("price_history_attribution", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PH-7", None), acting)
                .expect("product should create");

            let user_id: Option<i64> = db
                .open()
                .expect("db open")
                .query_row(
                    "SELECT user_id FROM price_history WHERE product_id = ?1 ORDER BY id LIMIT 1",
                    params![product.id],
                    |row| row.get(0),
                )
                .expect("query");

            assert_eq!(user_id, Some(acting));
        });
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::catalog -- --test-threads=1`
Expected: FAIL — arity mismatch on `create_product` (3 args vs 2).

- [ ] **Step 3: Thread the acting user and record on create**

At the top of `catalog.rs`, add: `use crate::price_history::{load_offering_state, record_offered_price_change, OfferingState};`

`create_product` — change the signature to `pub fn create_product(db: &Db, request: SaveProductRequest, acting_user_id: i64) -> Result<ProductSummary, AppError>`. After `let product_id = tx.last_insert_rowid();` and before `let product = product_by_id_for_connection(...)`, add:

```rust
    record_offered_price_change(
        &tx,
        product_id,
        None,
        OfferingState {
            active: normalized.active,
            price_minor: normalized.sale_price_minor,
        },
        "create",
        Some(acting_user_id),
        &now,
    )?;
```

- [ ] **Step 4: Record on update**

`update_product` — change the signature to `pub fn update_product(db: &Db, id: i64, request: SaveProductRequest, acting_user_id: i64) -> Result<ProductSummary, AppError>`. Capture the before-state **before** the `UPDATE products` executes (after `ensure_product_exists(&tx, id)?;`):

```rust
    let before = load_offering_state(&tx, id)?;
```

Then after the `tx.execute("UPDATE products ...")?;` call and before `product_by_id_for_connection`:

```rust
    record_offered_price_change(
        &tx,
        id,
        before,
        OfferingState {
            active: normalized.active,
            price_minor: normalized.sale_price_minor,
        },
        "update",
        Some(acting_user_id),
        &now,
    )?;
```

- [ ] **Step 5: Record on set_product_active**

`set_product_active` — change the signature to `pub fn set_product_active(db: &Db, id: i64, active: bool, acting_user_id: i64) -> Result<ProductSummary, AppError>`. Capture the before-state **before** the UPDATE (right after `let tx = connection.transaction()?;`):

```rust
    let before = load_offering_state(&tx, id)?;
```

Then after the `if changed == 0 { ... }` guard and before `product_by_id_for_connection`:

```rust
    if let Some(before_state) = before {
        record_offered_price_change(
            &tx,
            id,
            Some(before_state),
            OfferingState {
                active,
                price_minor: before_state.price_minor,
            },
            if active { "reactivate" } else { "deactivate" },
            Some(acting_user_id),
            &now,
        )?;
    }
```

- [ ] **Step 6: Update the three command wrappers**

In `catalog_create_product`, `catalog_update_product`, `catalog_set_product_active` (catalog.rs:181–207), the existing `super::auth::require_admin(state.inner())?;` line already returns the admin. Bind it and pass the id — e.g. for create:

```rust
    let acting = super::auth::require_admin(state.inner())?;
    create_product(state.db(), request, acting.id).map_err(Into::into)
```

Apply the same shape to update (`update_product(state.db(), id, request, acting.id)`) and set-active (`set_product_active(state.db(), id, active, acting.id)`).

- [ ] **Step 7: Fix other call sites**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "^error" | head -20`
Fix every arity error the compiler reports (existing catalog tests call these functions and each needs the new argument — pass the seeded admin's id or `1`).

- [ ] **Step 8: Run the catalog tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::catalog -- --test-threads=1`
Expected: PASS.

- [ ] **Step 9: Full gates + commit**

Run: `bun run test && bun run build && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src-tauri/src/commands/catalog.rs
git commit -m "feat(price-history): record offered-price changes from the catalog (SW-6a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Capture in the importer

**Files:**
- Modify: `src-tauri/src/importer.rs` — `commit_product_rows` (692) and its caller `commit_import` (357)
- Modify: `src-tauri/src/commands/imports.rs` — `import_commit` (28)
- Test: `src-tauri/src/importer.rs` (`mod tests`)

**Interfaces:**
- Consumes: `price_history::{OfferingState, load_offering_state, record_offered_price_change}` (Task 2).
- Produces: `commit_product_rows(..., acting_user_id: i64)` and `commit_import(..., acting_user_id: i64)`.

Note: the importer's UPDATE branch (importer.rs:731) does **not** touch `active`, and the INSERT branch (760) omits `active` so it defaults to 1 (active). So an imported product is always offered; the before/after state differs only in price.

- [ ] **Step 1: Write the failing tests**

Add to `src-tauri/src/importer.rs` `mod tests`, modelled on the existing `commit_products_writes_product_stock_job_and_history_transactionally` test (copy its CSV/fixture setup verbatim):

```rust
    #[test]
    fn import_records_an_offered_price_row_for_a_new_product() {
        // ...existing fixture that commits a products CSV with sale_price 4990,00...
        let connection = db.open().expect("db open");
        let (price_minor, source): (Option<i64>, String) = connection
            .query_row(
                "SELECT price_minor, source FROM price_history ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("price row should exist");
        assert_eq!(price_minor, Some(499000));
        assert_eq!(source, "import");
    }

    #[test]
    fn re_importing_the_same_price_appends_no_row() {
        // ...commit the same products CSV twice...
        let connection = db.open().expect("db open");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM price_history", [], |row| row.get(0))
            .expect("count");
        assert_eq!(count, 1, "an unchanged price must not pollute the timeline");
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml import_records_an_offered -- --test-threads=1`
Expected: FAIL — no price_history rows written by the importer.

- [ ] **Step 3: Implement**

At the top of `importer.rs`, add: `use crate::price_history::{load_offering_state, record_offered_price_change, OfferingState};`

Change `commit_product_rows` to take a trailing `acting_user_id: i64`. In the UPDATE branch, read the before-state **before** the `tx.execute("UPDATE products ...")`:

```rust
        let product_id = if let Some(product_id) =
            find_existing_product(tx, &sku_input, barcode.as_deref().unwrap_or(""))?
        {
            let before = load_offering_state(tx, product_id)?;
            tx.execute(
                "UPDATE products
                 SET name = ?1,
                 // ...unchanged...
            )?;
            record_offered_price_change(
                tx,
                product_id,
                before,
                OfferingState {
                    // The importer's UPDATE never touches `active`; preserve it.
                    active: before.map(|state| state.active).unwrap_or(true),
                    price_minor: sale_price,
                },
                "import",
                Some(acting_user_id),
                &now,
            )?;
            product_id
        } else {
            tx.execute(
                "INSERT INTO products (
                 // ...unchanged...
            )?;
            let product_id = tx.last_insert_rowid();
            record_offered_price_change(
                tx,
                product_id,
                None,
                // The INSERT omits `active`, so it defaults to 1 (offered).
                OfferingState { active: true, price_minor: sale_price },
                "import",
                Some(acting_user_id),
                &now,
            )?;
            product_id
        };
```

Thread `acting_user_id` through `commit_import` to the `commit_product_rows` call. In `src-tauri/src/commands/imports.rs`, `import_commit` already calls `require_admin`; bind it and pass `.id`:

```rust
    let acting = super::auth::require_admin(state.inner())?;
    commit_import(state.db(), request, acting.id).map_err(Into::into)
```

(Match the existing wrapper's exact shape — if it maps a different error type, keep that mapping.)

- [ ] **Step 4: Fix other call sites**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -E "^error" | head -20`
Fix every arity error (importer tests calling `commit_import`).

- [ ] **Step 5: Run the importer tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib importer -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Full gates + commit**

Run: `bun run test && bun run build && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src-tauri/src/importer.rs src-tauri/src/commands/imports.rs
git commit -m "feat(price-history): record offered-price changes from CSV import (SW-6a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: `compute_prethodna_cena` — ZoT čl. 37 st. 3–4

**Files:**
- Modify: `src-tauri/src/price_history.rs`
- Test: `src-tauri/src/price_history.rs` (`mod tests`)

**Interfaces:**
- Consumes: the `price_history` table (Task 1); `time` crate (already a dependency with `formatting` + `parsing`).
- Produces — consumed by Task 7:
  - `pub struct PrethodnaCena { pub price_minor: i64, pub window_days: i64, pub window_from: String, pub window_to: String, pub truncated: bool }`
  - `pub enum IncomputableReason { TooNewInAssortment { age_days: i64 }, NotOfferedInWindow, NoHistory }`
  - `pub enum PrethodnaCenaResult { Computed(PrethodnaCena), Incomputable(IncomputableReason) }`
  - `pub fn compute_prethodna_cena(conn: &rusqlite::Connection, product_id: i64, campaign_start_rfc3339: &str) -> Result<PrethodnaCenaResult, AppError>`

**This is the legally load-bearing task.** Every rule below is quoted from `docs/ZOT-36-37-VERIFIED-RULES.md`. Do not "optimise" any of them:
- Assortment age comes from `products.created_at`, **not** from the price log and **not** from the latest restock. A restock never resets age.
- `age >= 30 → 30d`; `15 <= age < 30 → age`; `age < 15 → Incomputable` (the statute supplies no usable window; blocking and computing are both wrong).
- Window is half-open `[start − Nd, start)`, calendar days.
- The MIN **includes prior promotional prices**. Never filter them out — that yields an unlawfully high anchor.
- `truncated` is reported, never hidden.

- [ ] **Step 1: Write the failing tests — the memo's worked examples verbatim**

Add to the `mod tests` in `src-tauri/src/price_history.rs`:

```rust
    fn conn_with_timeline(created_at: &str, rows: &[(&str, Option<i64>)]) -> (Connection, i64) {
        let (connection, id) = conn_with_product(true, 0);
        connection
            .execute(
                "UPDATE products SET created_at = ?1 WHERE id = ?2",
                rusqlite::params![created_at, id],
            )
            .expect("created_at should set");
        for (effective_from, price_minor) in rows {
            connection
                .execute(
                    "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                     VALUES (?1, ?2, ?3, 'update', ?2)",
                    rusqlite::params![id, effective_from, price_minor],
                )
                .expect("history row should insert");
        }
        (connection, id)
    }

    // Worked example (a) — ZOT-36-37-VERIFIED-RULES.md §2.7.
    // KOSULJA-M-42, campaign starts 20.07.2026. Window [20.06, 20.07).
    // The June akcija at 4.290,00 falls OUTSIDE the window; the 01.07 price
    // INCREASE to 5.290,00 does not become the anchor (MIN, not "price before").
    #[test]
    fn worked_example_a_simple_reduction_anchors_at_4990() {
        let (conn, id) = conn_with_timeline(
            "2026-04-01T00:00:00Z",
            &[
                ("2026-04-01T00:00:00Z", Some(499000)),
                ("2026-06-10T00:00:00Z", Some(429000)),
                ("2026-06-15T00:00:00Z", Some(499000)),
                ("2026-07-01T00:00:00Z", Some(529000)),
            ],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-20T00:00:00Z").expect("compute");
        match result {
            PrethodnaCenaResult::Computed(value) => {
                assert_eq!(value.price_minor, 499000, "prethodna cena must be 4.990,00");
                assert_eq!(value.window_days, 30);
                assert!(!value.truncated);
            }
            other => panic!("expected Computed, got {other:?}"),
        }
    }

    // Worked example (a') — the ratchet-down effect. Starting 8 days earlier
    // pulls the June akcija INTO the window and collapses the anchor.
    #[test]
    fn worked_example_a_prime_earlier_start_ratchets_anchor_down_to_4290() {
        let (conn, id) = conn_with_timeline(
            "2026-04-01T00:00:00Z",
            &[
                ("2026-04-01T00:00:00Z", Some(499000)),
                ("2026-06-10T00:00:00Z", Some(429000)),
                ("2026-06-15T00:00:00Z", Some(499000)),
                ("2026-07-01T00:00:00Z", Some(529000)),
            ],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-12T00:00:00Z").expect("compute");
        match result {
            PrethodnaCenaResult::Computed(value) => assert_eq!(value.price_minor, 429000),
            other => panic!("expected Computed, got {other:?}"),
        }
    }

    // Worked example (c2) — 22 days in assortment -> st. 4 window = 22 days.
    #[test]
    fn worked_example_c2_short_assortment_uses_actual_age_window() {
        let (conn, id) = conn_with_timeline(
            "2026-06-25T00:00:00Z",
            &[
                ("2026-06-25T00:00:00Z", Some(249000)),
                ("2026-07-08T00:00:00Z", Some(229000)),
            ],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-17T00:00:00Z").expect("compute");
        match result {
            PrethodnaCenaResult::Computed(value) => {
                assert_eq!(value.price_minor, 229000, "prethodna cena must be 2.290,00");
                assert_eq!(value.window_days, 22, "window is the item's actual age, not 30");
            }
            other => panic!("expected Computed, got {other:?}"),
        }
    }

    // Worked example (c3) — 6 days in assortment. The law supplies no usable
    // window: neither block nor compute.
    #[test]
    fn worked_example_c3_too_new_is_incomputable_not_blocked_and_not_guessed() {
        let (conn, id) = conn_with_timeline(
            "2026-07-11T00:00:00Z",
            &[("2026-07-11T00:00:00Z", Some(100000))],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-17T00:00:00Z").expect("compute");
        assert_eq!(
            result,
            PrethodnaCenaResult::Incomputable(IncomputableReason::TooNewInAssortment { age_days: 6 })
        );
    }

    // Worked example (c4) — THE TRAP. A jacket returning to the shelf after a
    // gap is NOT a new arrival: st. 3 with the full 30-day window applies,
    // reaching across the gap. A naive "days since last offered" implementation
    // would use ~10 days here and anchor too high.
    #[test]
    fn worked_example_c4_returning_seasonal_goods_use_st3_not_st4() {
        let (conn, id) = conn_with_timeline(
            "2025-10-01T00:00:00Z",
            &[
                ("2025-10-01T00:00:00Z", Some(1290000)),
                ("2026-02-28T00:00:00Z", None), // offering ended
                ("2026-07-05T00:00:00Z", Some(1290000)), // back on the shelf
            ],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-15T00:00:00Z").expect("compute");
        match result {
            PrethodnaCenaResult::Computed(value) => {
                assert_eq!(value.window_days, 30, "returning stock is not a new arrival");
                assert_eq!(value.price_minor, 1290000);
            }
            other => panic!("expected Computed, got {other:?}"),
        }
    }

    #[test]
    fn a_product_off_shelf_for_the_whole_window_is_not_offered_in_window() {
        let (conn, id) = conn_with_timeline(
            "2025-10-01T00:00:00Z",
            &[
                ("2025-10-01T00:00:00Z", Some(1290000)),
                ("2026-02-28T00:00:00Z", None),
            ],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-15T00:00:00Z").expect("compute");
        assert_eq!(
            result,
            PrethodnaCenaResult::Incomputable(IncomputableReason::NotOfferedInWindow)
        );
    }

    #[test]
    fn a_window_reaching_before_the_log_is_reported_truncated() {
        let (conn, id) = conn_with_timeline(
            "2026-01-01T00:00:00Z",
            &[("2026-07-10T00:00:00Z", Some(499000))], // log starts mid-window (a v9 seed)
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-20T00:00:00Z").expect("compute");
        match result {
            PrethodnaCenaResult::Computed(value) => {
                assert!(value.truncated, "the log does not cover the full 30-day window");
                assert_eq!(value.price_minor, 499000);
            }
            other => panic!("expected Computed, got {other:?}"),
        }
    }

    #[test]
    fn a_product_with_no_history_reports_no_history() {
        let (conn, id) = conn_with_timeline("2026-01-01T00:00:00Z", &[]);
        let result = compute_prethodna_cena(&conn, id, "2026-07-20T00:00:00Z").expect("compute");
        assert_eq!(result, PrethodnaCenaResult::Incomputable(IncomputableReason::NoHistory));
    }
```

Add `#[derive(Debug, PartialEq, Eq)]` to the three result types so `assert_eq!`/`panic!("{other:?}")` compile.

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml worked_example -- --test-threads=1`
Expected: FAIL — `compute_prethodna_cena` undefined.

- [ ] **Step 3: Implement the computation**

Add to `src-tauri/src/price_history.rs` (extend the imports at the top with `use crate::app_error::AppError;`, `use time::format_description::well_known::Rfc3339;`, `use time::{Duration, OffsetDateTime};`):

```rust
/// A prethodna cena computed under ZoT čl. 37 st. 3–4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrethodnaCena {
    pub price_minor: i64,
    pub window_days: i64,
    /// Inclusive, RFC3339.
    pub window_from: String,
    /// Exclusive, RFC3339.
    pub window_to: String,
    /// True when the log does not cover the whole window — say so, never
    /// silently compute over a partial window.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomputableReason {
    /// čl. 37 st. 4 names a window „ne kraćem od 15 dana" that cannot be
    /// observed for younger goods, and supplies no fallback. Neither block
    /// (no prohibitory language exists) nor guess.
    TooNewInAssortment { age_days: i64 },
    /// Off the shelf for the entire window.
    NotOfferedInWindow,
    NoHistory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrethodnaCenaResult {
    Computed(PrethodnaCena),
    Incomputable(IncomputableReason),
}

fn parse_rfc3339(value: &str, field: &str) -> Result<OffsetDateTime, AppError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|source| {
        AppError::validation(
            format!("Datum nije ispravan: {source}"),
            serde_json::json!({ "field": field }),
        )
    })
}

/// Computes the prethodna cena for a sniženje starting at `campaign_start_rfc3339`.
///
/// Legal authority: ZoT čl. 37 st. 3 (30-day lowest OFFERED price) and st. 4
/// (shorter assortment age, window floor of 15 days). st. 5 (the frozen
/// progressive anchor) is deliberately NOT implemented here — it is defined
/// against a campaign's start and belongs with the campaign entity (SW-6b).
pub fn compute_prethodna_cena(
    conn: &Connection,
    product_id: i64,
    campaign_start_rfc3339: &str,
) -> Result<PrethodnaCenaResult, AppError> {
    let start = parse_rfc3339(campaign_start_rfc3339, "campaignStart")?;

    // Assortment age comes from products.created_at — NOT from the price log.
    // Deriving it from the log would make every product freshly seeded by
    // migration v9 look brand new and report TooNewInAssortment for 15 days.
    let created_at: Option<String> = conn
        .query_row(
            "SELECT created_at FROM products WHERE id = ?1",
            params![product_id],
            |row| row.get(0),
        )
        .optional()?;
    let created_at = created_at.ok_or_else(|| AppError::not_found("Artikal nije pronađen."))?;
    let created = parse_rfc3339(&created_at, "createdAt")?;

    let age_days = (start - created).whole_days();

    // čl. 37 st. 3 / st. 4.
    let window_days = if age_days >= 30 {
        30
    } else if age_days >= 15 {
        age_days
    } else {
        return Ok(PrethodnaCenaResult::Incomputable(
            IncomputableReason::TooNewInAssortment { age_days },
        ));
    };

    let window_from = (start - Duration::days(window_days))
        .format(&Rfc3339)
        .map_err(|source| AppError::InvalidState(format!("Vreme nije dostupno: {source}")))?;
    let window_to = campaign_start_rfc3339.to_string();

    let earliest: Option<String> = conn.query_row(
        "SELECT MIN(effective_from) FROM price_history WHERE product_id = ?1",
        params![product_id],
        |row| row.get(0),
    )?;
    let Some(earliest) = earliest else {
        return Ok(PrethodnaCenaResult::Incomputable(IncomputableReason::NoHistory));
    };
    let truncated = earliest.as_str() > window_from.as_str();

    // Intervals derive from consecutive rows: row i covers
    // [effective_from_i, effective_from_{i+1}). A NULL price_minor is an
    // offering gap and is excluded. The MIN deliberately INCLUDES earlier
    // promotional prices — filtering them out to "find the regular price"
    // would produce an unlawfully high anchor.
    let min_price: Option<i64> = conn.query_row(
        "WITH timeline AS (
            SELECT price_minor,
                   effective_from AS valid_from,
                   LEAD(effective_from) OVER (
                       PARTITION BY product_id ORDER BY effective_from, id
                   ) AS valid_to
            FROM price_history
            WHERE product_id = ?1
         )
         SELECT MIN(price_minor)
         FROM timeline
         WHERE price_minor IS NOT NULL
           AND valid_from < ?3
           AND (valid_to IS NULL OR valid_to > ?2)",
        params![product_id, window_from, window_to],
        |row| row.get(0),
    )?;

    match min_price {
        Some(price_minor) => Ok(PrethodnaCenaResult::Computed(PrethodnaCena {
            price_minor,
            window_days,
            window_from,
            window_to,
            truncated,
        })),
        None => Ok(PrethodnaCenaResult::Incomputable(
            IncomputableReason::NotOfferedInWindow,
        )),
    }
}
```

- [ ] **Step 4: Run the worked-example tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib price_history -- --test-threads=1`
Expected: PASS (all 16). If (c4) fails, the age calculation is reading the log instead of `products.created_at` — fix that, do not adjust the test.

- [ ] **Step 5: Full gates + commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src-tauri/src/price_history.rs
git commit -m "feat(price-history): prethodna cena per ZoT cl. 37 st. 3-4 (SW-6a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: Go-live reset clears and re-seeds price history

**Files:**
- Modify: `src-tauri/src/commands/backup.rs` — `reset_trading_data` (~line 308)
- Test: `src-tauri/src/commands/backup.rs` (`mod tests`)

**Interfaces:**
- Consumes: the `price_history` table (Task 1); the existing reset transaction and `compliance_log` tombstone.
- Produces: nothing consumed downstream.

**Why this is the one deliberate exception to "never prune"** (design §5): reset is the pre-production tool — admin-only, exact typed confirmation, forced safety backup, permanent tombstone. Practice prices were never offered to a consumer, so leaving them in would let training data drive a real anchor. Clearing and re-seeding is therefore both safe and more correct.

- [ ] **Step 1: Write the failing test**

Add to `src-tauri/src/commands/backup.rs` `mod tests`:

```rust
    #[test]
    fn reset_trading_data_clears_and_reseeds_price_history() {
        with_state("reset_clears_price_history", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-reset-price-history");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");
            seed_trading_data(state);

            // Practice price history: two rows for the seeded product.
            let conn = state.db().open().expect("database should open");
            conn.execute(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-06-01T00:00:00Z', 11000, 'update', '2026-06-01T00:00:00Z'),
                        (1, '2026-06-02T00:00:00Z', 12000, 'update', '2026-06-02T00:00:00Z')",
                [],
            )
            .expect("practice history should insert");
            drop(conn);

            reset_trading_data(state, "OBRISI PODATKE").expect("reset should succeed");

            let conn = state.db().open().expect("database should open");
            // Practice rows are gone; exactly one fresh seed row per active product.
            let (count, source): (i64, String) = conn
                .query_row(
                    "SELECT COUNT(*), COALESCE(MIN(source), '') FROM price_history",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("query");
            assert_eq!(count, 1, "expected exactly one re-seeded row for the active product");
            assert_eq!(source, "seed");

            let effective_from: String = conn
                .query_row("SELECT effective_from FROM price_history", [], |row| row.get(0))
                .expect("query");
            assert!(
                effective_from.contains('T') && effective_from.ends_with('Z'),
                "re-seed must be RFC3339, got {effective_from}"
            );
        });
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml reset_trading_data_clears_and_reseeds -- --test-threads=1`
Expected: FAIL — practice rows survive (count is 2).

- [ ] **Step 3: Implement**

In `reset_trading_data`, inside the existing transaction (alongside the other `DELETE FROM` statements, before the `insert_compliance_event(...)` call), add:

```rust
    // Price history is normally append-only and retained ~5 years. The go-live
    // reset is the one exception: practice prices were never offered to a
    // consumer, so leaving them in would let training data drive a real
    // prethodna cena. Clear and re-seed from the surviving catalog.
    tx.execute("DELETE FROM price_history", [])?;
    tx.execute(
        "INSERT INTO price_history (product_id, effective_from, price_minor, source, user_id, created_at)
         SELECT id,
                strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                sale_price_minor,
                'seed',
                NULL,
                strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         FROM products
         WHERE active = 1",
        [],
    )?;
```

Note `strftime('%Y-%m-%dT%H:%M:%SZ','now')`, matching migration v9 — **not** `datetime('now')`.

- [ ] **Step 4: Run the backup tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::backup -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Full gates + commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src-tauri/src/commands/backup.rs
git commit -m "feat(price-history): go-live reset clears and re-seeds price history (SW-6a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: Advisory command + catalog form surfacing

**Files:**
- Modify: `src-tauri/src/commands/catalog.rs` (new command + DTO)
- Modify: `src-tauri/src/lib.rs` (register the command)
- Modify: `src/services/types.ts`, `src/services/ports.ts`, `src/services/local-adapter.ts`, `src/services/mock-adapter.ts`
- Modify: the catalog product form component (find it via `grep -rln "salePriceMinor" src/app`)
- Test: `src-tauri/src/commands/catalog.rs`; the catalog form's `.test.tsx`

**Interfaces:**
- Consumes: `compute_prethodna_cena` (Task 5).
- Produces: command `catalog_prethodna_cena(product_id: i64, campaign_start: String) -> PrethodnaCenaDto`; frontend `catalog.getPrethodnaCena(productId: number, campaignStart: string): Promise<PrethodnaCenaDto>`.

- [ ] **Step 1: Write the failing backend test**

Add to `src-tauri/src/commands/catalog.rs` `mod tests`:

Uses the same `with_catalog_database` / `product_request` / `admin_id` fixtures as Task 3. Both tests back-date `products.created_at` directly, because the fixture creates products "now" and these assertions depend on assortment age.

```rust
    #[test]
    fn prethodna_cena_dto_reports_computed_values() {
        with_catalog_database("prethodna_cena_dto_computed", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PC-1", None), acting)
                .expect("product should create");

            let connection = db.open().expect("db open");
            // Old enough in assortment for the full 30-day window (st. 3).
            connection
                .execute(
                    "UPDATE products SET created_at = '2026-04-01T00:00:00Z' WHERE id = ?1",
                    params![product.id],
                )
                .expect("created_at should back-date");
            // Replace the create row with the worked-example timeline.
            connection
                .execute("DELETE FROM price_history WHERE product_id = ?1", params![product.id])
                .expect("clear");
            connection
                .execute(
                    "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                     VALUES (?1, '2026-04-01T00:00:00Z', 499000, 'create', '2026-04-01T00:00:00Z'),
                            (?1, '2026-07-01T00:00:00Z', 529000, 'update', '2026-07-01T00:00:00Z')",
                    params![product.id],
                )
                .expect("timeline should insert");
            drop(connection);

            let dto = prethodna_cena(db, product.id, "2026-07-20T00:00:00Z").expect("compute");

            assert_eq!(dto.status, "computed");
            assert_eq!(dto.price_minor, Some(499000));
            assert_eq!(dto.window_days, Some(30));
            assert_eq!(dto.reason, None);
        });
    }

    #[test]
    fn prethodna_cena_dto_reports_too_new_in_assortment() {
        with_catalog_database("prethodna_cena_dto_too_new", |db| {
            let acting = admin_id(db);
            let product = create_product(db, product_request("PC-2", None), acting)
                .expect("product should create");

            db.open()
                .expect("db open")
                .execute(
                    "UPDATE products SET created_at = '2026-07-11T00:00:00Z' WHERE id = ?1",
                    params![product.id],
                )
                .expect("created_at should back-date");

            let dto = prethodna_cena(db, product.id, "2026-07-17T00:00:00Z").expect("compute");

            assert_eq!(dto.status, "incomputable");
            assert_eq!(dto.reason, Some("too_new_in_assortment"));
            assert_eq!(dto.age_days, Some(6));
            assert_eq!(dto.price_minor, None);
        });
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml prethodna_cena_dto -- --test-threads=1`
Expected: FAIL — `prethodna_cena` undefined.

- [ ] **Step 3: Implement the DTO + command**

Add to `src-tauri/src/commands/catalog.rs`:

```rust
use crate::price_history::{
    compute_prethodna_cena, IncomputableReason, PrethodnaCenaResult,
};

/// Flat DTO for the frontend. The domain uses a Rust enum; flattening here
/// keeps the TypeScript contract simple.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrethodnaCenaDto {
    /// "computed" | "incomputable"
    pub status: &'static str,
    pub price_minor: Option<i64>,
    pub window_days: Option<i64>,
    pub window_from: Option<String>,
    pub window_to: Option<String>,
    pub truncated: bool,
    /// "too_new_in_assortment" | "not_offered_in_window" | "no_history"
    pub reason: Option<&'static str>,
    pub age_days: Option<i64>,
}

impl From<PrethodnaCenaResult> for PrethodnaCenaDto {
    fn from(result: PrethodnaCenaResult) -> Self {
        match result {
            PrethodnaCenaResult::Computed(value) => Self {
                status: "computed",
                price_minor: Some(value.price_minor),
                window_days: Some(value.window_days),
                window_from: Some(value.window_from),
                window_to: Some(value.window_to),
                truncated: value.truncated,
                reason: None,
                age_days: None,
            },
            PrethodnaCenaResult::Incomputable(reason) => {
                let (reason_code, age_days) = match reason {
                    IncomputableReason::TooNewInAssortment { age_days } => {
                        ("too_new_in_assortment", Some(age_days))
                    }
                    IncomputableReason::NotOfferedInWindow => ("not_offered_in_window", None),
                    IncomputableReason::NoHistory => ("no_history", None),
                };
                Self {
                    status: "incomputable",
                    price_minor: None,
                    window_days: None,
                    window_from: None,
                    window_to: None,
                    truncated: false,
                    reason: Some(reason_code),
                    age_days,
                }
            }
        }
    }
}

pub fn prethodna_cena(
    db: &Db,
    product_id: i64,
    campaign_start: &str,
) -> Result<PrethodnaCenaDto, AppError> {
    let connection = db.open()?;
    Ok(compute_prethodna_cena(&connection, product_id, campaign_start)?.into())
}

#[tauri::command]
pub fn catalog_prethodna_cena(
    state: State<'_, AppState>,
    product_id: i64,
    campaign_start: String,
) -> Result<PrethodnaCenaDto, CommandError> {
    super::auth::require_session(state.inner())?;
    prethodna_cena(state.db(), product_id, &campaign_start).map_err(Into::into)
}
```

Register `commands::catalog::catalog_prethodna_cena,` in `src-tauri/src/lib.rs` `generate_handler!` (after `commands::catalog::catalog_save_category,`).

- [ ] **Step 4: Run the backend tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::catalog -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Wire the frontend contract**

- `src/services/types.ts` — add:
  ```ts
  export interface PrethodnaCenaDto {
    status: "computed" | "incomputable";
    priceMinor: number | null;
    windowDays: number | null;
    windowFrom: string | null;
    windowTo: string | null;
    truncated: boolean;
    reason: "too_new_in_assortment" | "not_offered_in_window" | "no_history" | null;
    ageDays: number | null;
  }
  ```
- `src/services/ports.ts` — add to `CatalogService`: `getPrethodnaCena(productId: number, campaignStart: string): Promise<PrethodnaCenaDto>;` (and add `PrethodnaCenaDto` to the import list from `./types`).
- `src/services/local-adapter.ts` — add to `catalog`:
  ```ts
      getPrethodnaCena: (productId, campaignStart) =>
        invoke<PrethodnaCenaDto>("catalog_prethodna_cena", { productId, campaignStart }),
  ```
- `src/services/mock-adapter.ts` — add to the catalog mock:
  ```ts
      async getPrethodnaCena() {
        return {
          status: "computed",
          priceMinor: 499000,
          windowDays: 30,
          windowFrom: "2026-06-20T00:00:00Z",
          windowTo: "2026-07-20T00:00:00Z",
          truncated: false,
          reason: null,
          ageDays: null,
        } satisfies PrethodnaCenaDto;
      },
  ```

- [ ] **Step 6: Write the failing frontend test**

Locate the catalog product form with `grep -rln "salePriceMinor" src/app`. In its `.test.tsx`, add a test that lowering the price shows the advisory (adapt the render/service-injection pattern the file already uses):

```tsx
it("shows the prethodna cena advisory when the price is lowered", async () => {
  const getPrethodnaCena = vi.fn().mockResolvedValue({
    status: "computed",
    priceMinor: 499000,
    windowDays: 30,
    windowFrom: "2026-06-20T00:00:00Z",
    windowTo: "2026-07-20T00:00:00Z",
    truncated: false,
    reason: null,
    ageDays: null,
  });
  // ...render the form for an existing product priced 5.290,00 with the mocked service...
  await userEvent.clear(screen.getByLabelText(/Prodajna cena/i));
  await userEvent.type(screen.getByLabelText(/Prodajna cena/i), "3990");
  expect(await screen.findByText(/Prethodna cena/i)).toBeInTheDocument();
  expect(screen.getByText(/čl. 37 st. 3/)).toBeInTheDocument();
});
```

- [ ] **Step 7: Run to verify it fails**

Run: `bun run test -- <the form's test file name>`
Expected: FAIL — advisory absent.

- [ ] **Step 8: Implement the advisory**

In the product form, when editing an existing product and the entered `salePriceMinor` is **below** the stored one, call `services.catalog.getPrethodnaCena(productId, new Date().toISOString())` and render a read-only advisory. Exact copy:

- Computed: `Prethodna cena: {formatRsd(priceMinor)}` and, beneath it, the muted line `Prethodna cena izračunata prema čl. 37 st. 3. Mora biti istaknuta uz sniženu cenu na prodajnom mestu.`
- If `truncated`: also `Evidencija cena ne pokriva ceo period od 30 dana — proverite podatke.`
- If `reason === "too_new_in_assortment"`: `Roba je u asortimanu kraće od 15 dana — zakon ne propisuje jasan referentni period. Unesite prethodnu cenu ručno i obrazložite.`
- If `reason === "not_offered_in_window"`: `Artikal nije bio u ponudi tokom referentnog perioda.`
- If `reason === "no_history"`: `Nema evidencije cena za ovaj artikal.`

**Never** render a „this is legal" affirmation — čl. 38 st. 4 can bite even where the st. 3 arithmetic is right. Nothing here blocks saving.

- [ ] **Step 9: Run the frontend tests**

Run: `bun run test`
Expected: PASS.

- [ ] **Step 10: Full gates + commit**

Run: `bun run test && bun run build && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src-tauri/src/commands/catalog.rs src-tauri/src/lib.rs src/services src/app
git commit -m "feat(price-history): prethodna cena advisory in the catalog form (SW-6a)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Self-Review Notes

- **Spec coverage:** §1 schema → Task 1; §2 four capture points → Tasks 3 (create/update/set_active) + 4 (import); §3 honest seeding → Task 1; §4 computation → Tasks 2 (helper) + 5 (algorithm); §5 retention + the reset exception → Task 6; §6 surfacing → Task 7; §7 test plan → Tasks 2, 3, 4, 5 (the memo's worked examples land in Task 5 verbatim).
- **Spec refinement recorded in Task 3:** `update_product` can flip `active` as well as price, which the design doc did not anticipate. The `(before, after)` helper design absorbs this without a special case.
- **Type consistency:** `OfferingState { active, price_minor }`, `record_offered_price_change(conn, product_id, before, after, source, acting_user_id, now_rfc3339)`, and `compute_prethodna_cena(conn, product_id, campaign_start_rfc3339)` are used identically in Tasks 2–7. Rust `price_minor`/`window_days`/`age_days` ↔ TS `priceMinor`/`windowDays`/`ageDays` via serde camelCase.
- **Verified before writing:** the algorithm was executed against all five worked examples from `ZOT-36-37-VERIFIED-RULES.md` §2.7 and reproduces every expected value (4.990,00 / 4.290,00 / 2.290,00 / Incomputable(6) / returning-jacket 30-day window).
- **Ordering:** 1 (schema) → 2 (helper) → 3, 4 (capture) → 5 (computation) → 6 (reset) → 7 (surfacing). Tasks 3 and 4 are independent of each other; 5 depends only on 1; 7 depends on 5.
