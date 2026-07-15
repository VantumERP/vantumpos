# Returns & Voids Money-Correctness Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make returns and voids record the money they move as a signed `sale_payments` ledger so the shift drawer and every report are computed from that ledger and can no longer disagree.

**Architecture:** The original sale is never mutated by a return/void for money purposes (its display-only `status` flip is kept but no money query reads it). Each return/void counter-document carries *negative* `sale_payments` rows (void mirrors the original tenders; return refunds one tender, default cash). Every money reader is rewritten to sum the signed ledger and to branch on the immutable `document_type`, never on `status`. A return/void requires an open shift and attaches to it.

**Tech Stack:** Tauri v2, Rust, rusqlite/SQLite, React + TypeScript, shadcn/ui, bun.

**Design doc:** `docs/superpowers/specs/2026-07-15-returns-voids-money-correctness-design.md`

## Global Constraints

- Branch: `feat/returns-voids-money-correctness` (already checked out).
- Every backend money value is an integer minor unit (para); quantities are milli-units. Never emit floats.
- Operator-facing error messages are Serbian; reuse existing structured `AppError`/`CommandError` codes.
- No fiscalization, cloud, or hardware dependencies introduced.
- All commits end with the trailer: `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Verification gates that must pass at the end of every task (run from repo root unless noted):
  - `bun run test`
  - `bun run build`
  - `cd src-tauri && cargo test -- --test-threads=1`
  - `cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings`
  - `cd src-tauri && cargo fmt --check`
  - `git diff --check`
- Error-code facts (from `src-tauri/src/app_error.rs`): `AppError::business("shift_required", …)` → `CommandError.code == "shift_required"`; `AppError::validation(…)` → `CommandError.code == "validation_error"`.

---

### Task 1: Migration v6 — signed `sale_payments.amount_minor`

**Files:**
- Modify: `src-tauri/src/db/migrations.rs` (append a `Migration` after the `version: 5` entry)
- Test: `src-tauri/src/db/mod.rs` (add one test to the existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: a migrated schema where `sale_payments.amount_minor` allows any non-zero integer (`CHECK (amount_minor <> 0)`), replacing `CHECK (amount_minor >= 0)`. All later tasks depend on this.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `src-tauri/src/db/mod.rs`:

```rust
    #[test]
    fn migration_v6_allows_signed_sale_payment_amounts() {
        let path = test_database_path("migration_v6_signed_sale_payments");
        {
            let db = Db::new(&path).expect("database should initialize");
            let connection = db.open().expect("database should open");
            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = 'sale_payments'",
                    [],
                    |row| row.get(0),
                )
                .expect("sale_payments schema should load");
            assert!(
                schema.contains("amount_minor") && schema.contains("<> 0"),
                "expected signed amount_minor, schema was: {schema}"
            );
            assert!(
                !schema.contains(">= 0"),
                "expected no non-negative constraint, schema was: {schema}"
            );
        }
        std::fs::remove_file(&path).expect("test database should be removed");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test db::tests::migration_v6_allows_signed_sale_payment_amounts -- --test-threads=1`
Expected: FAIL — schema still contains `>= 0`.

- [ ] **Step 3: Add the migration**

In `src-tauri/src/db/migrations.rs`, add this element to the `MIGRATIONS` array immediately after the `version: 5` migration:

```rust
    Migration {
        version: 6,
        name: "signed_sale_payments",
        sql: r#"
CREATE TABLE sale_payments_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    sale_id INTEGER NOT NULL REFERENCES sales(id) ON DELETE CASCADE,
    payment_method TEXT NOT NULL CHECK (payment_method IN ('cash', 'card')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor <> 0),
    created_at TEXT NOT NULL
);

INSERT INTO sale_payments_next (id, sale_id, payment_method, amount_minor, created_at)
SELECT id, sale_id, payment_method, amount_minor, created_at FROM sale_payments;

DROP TABLE sale_payments;
ALTER TABLE sale_payments_next RENAME TO sale_payments;
CREATE INDEX idx_sale_payments_sale ON sale_payments(sale_id);
"#,
    },
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test db:: -- --test-threads=1`
Expected: PASS (the new test plus all existing db tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/migrations.rs src-tauri/src/db/mod.rs
git commit -m "feat(db): allow signed sale_payments amounts (migration v6)

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Void records mirrored negative payments, requires an open shift

**Files:**
- Modify: `src-tauri/src/commands/receipts.rs` (`void_receipt`; add helper `current_open_shift_id`)
- Test: `src-tauri/src/commands/receipts.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `fn current_open_shift_id(connection: &Connection) -> Result<i64, AppError>` — the most-recent open shift, or `AppError::business("shift_required", "Smena nije otvorena.")`. Consumed by Task 3.
- After a void: the void counter-document's `shift_id` is the current open shift, and it has one negative `sale_payments` row per original tender (the exact negatives of the original's payments).

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` in `src-tauri/src/commands/receipts.rs`:

```rust
    #[test]
    fn void_records_negative_cash_payment_mirroring_original() {
        with_receipt_database("void_records_negative_cash_payment", |db, seeded| {
            void_receipt(
                db,
                VoidReceiptRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Greska na racunu".to_string(),
                },
            )
            .expect("void should succeed");

            let connection = db.open().expect("database should open");
            let (void_sale_id, void_shift_id): (i64, i64) = connection
                .query_row(
                    "SELECT id, shift_id FROM sales
                     WHERE original_sale_id = ?1 AND document_type = 'void'",
                    params![seeded.sale_id],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("void document should exist");
            let refund: i64 = connection
                .query_row(
                    "SELECT amount_minor FROM sale_payments
                     WHERE sale_id = ?1 AND payment_method = 'cash'",
                    params![void_sale_id],
                    |row| row.get(0),
                )
                .expect("mirrored cash refund should exist");
            assert_eq!(refund, -100_000);

            let open_shift: i64 = connection
                .query_row(
                    "SELECT id FROM shifts WHERE status = 'open'
                     ORDER BY opened_at DESC, id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .expect("open shift should exist");
            assert_eq!(void_shift_id, open_shift);
        });
    }

    #[test]
    fn void_mirrors_split_cash_and_card_tenders() {
        with_receipt_database("void_mirrors_split_tenders", |db, seeded| {
            let connection = db.open().expect("database should open");
            connection
                .execute(
                    "INSERT INTO sales (
                        local_receipt_number, shift_id, cashier_id, status, fiscal_status,
                        subtotal_minor, discount_minor, tax_minor, total_minor, created_at, updated_at)
                     SELECT 'R-2026-0002', shift_id, cashier_id, 'completed', 'not_fiscalized',
                        100000, 0, 16667, 100000, '2026-06-18T09:30:00Z', '2026-06-18T09:30:00Z'
                     FROM sales WHERE id = ?1",
                    params![seeded.sale_id],
                )
                .expect("second sale should insert");
            let sale2 = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                     VALUES (?1, 'cash', 60000, '2026-06-18T09:30:00Z')",
                    params![sale2],
                )
                .expect("cash payment should insert");
            connection
                .execute(
                    "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                     VALUES (?1, 'card', 40000, '2026-06-18T09:30:00Z')",
                    params![sale2],
                )
                .expect("card payment should insert");

            void_receipt(
                db,
                VoidReceiptRequest {
                    receipt_id: sale2,
                    user_id: seeded.user_id,
                    reason: "Greska".to_string(),
                },
            )
            .expect("void should succeed");

            let void_id: i64 = connection
                .query_row(
                    "SELECT id FROM sales WHERE original_sale_id = ?1 AND document_type = 'void'",
                    params![sale2],
                    |row| row.get(0),
                )
                .expect("void document should exist");
            let cash: i64 = connection
                .query_row(
                    "SELECT amount_minor FROM sale_payments WHERE sale_id = ?1 AND payment_method = 'cash'",
                    params![void_id],
                    |row| row.get(0),
                )
                .expect("cash refund should exist");
            let card: i64 = connection
                .query_row(
                    "SELECT amount_minor FROM sale_payments WHERE sale_id = ?1 AND payment_method = 'card'",
                    params![void_id],
                    |row| row.get(0),
                )
                .expect("card refund should exist");
            assert_eq!(cash, -60_000);
            assert_eq!(card, -40_000);
        });
    }

    #[test]
    fn void_requires_open_shift() {
        with_receipt_database("void_requires_open_shift", |db, seeded| {
            db.open()
                .expect("database should open")
                .execute("UPDATE shifts SET status = 'closed'", [])
                .expect("shift should close");

            let error = void_receipt(
                db,
                VoidReceiptRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Greska".to_string(),
                },
            )
            .expect_err("void without an open shift should fail");
            assert_eq!(error.code, "shift_required");

            let voids: i64 = db
                .open()
                .expect("database should open")
                .query_row(
                    "SELECT COUNT(*) FROM sales WHERE document_type = 'void'",
                    [],
                    |row| row.get(0),
                )
                .expect("count should query");
            assert_eq!(voids, 0);
        });
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test commands::receipts::tests::void_ -- --test-threads=1`
Expected: FAIL — no mirrored payment rows; `void_requires_open_shift` fails because void currently succeeds with a closed shift.

- [ ] **Step 3: Add the `current_open_shift_id` helper**

In `src-tauri/src/commands/receipts.rs`, add this private function (place it just above `fn ensure_voidable`):

```rust
fn current_open_shift_id(connection: &Connection) -> Result<i64, AppError> {
    connection
        .query_row(
            "SELECT id FROM shifts
             WHERE status = 'open'
             ORDER BY opened_at DESC, id DESC
             LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::business("shift_required", "Smena nije otvorena."))
}
```

- [ ] **Step 4: Resolve the open shift and stamp it on the void document**

In `void_receipt`, immediately after `ensure_voidable(&tx, &header)?;` add:

```rust
        let shift_id = current_open_shift_id(&tx)?;
```

Then in the `INSERT INTO sales (...) VALUES ('voided'...)` for the void document, change the shift parameter from `header.shift_id` to `shift_id`. (It is the first `?`-bound value after `local_receipt_number` — currently `header.shift_id,` in the `params![]`.)

- [ ] **Step 5: Write the mirrored negative payments**

In `void_receipt`, immediately after the `for item in items { … }` loop closes and before the `UPDATE sales SET status = 'voided' …` statement, insert:

```rust
        let original_payments: Vec<(String, i64)> = {
            let mut statement = tx.prepare(
                "SELECT payment_method, amount_minor FROM sale_payments WHERE sale_id = ?1",
            )?;
            let mapped = statement.query_map(params![request.receipt_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })?;
            mapped.collect::<rusqlite::Result<Vec<_>>>().map_err(AppError::from)?
        };
        for (method, amount) in original_payments {
            if amount == 0 {
                continue;
            }
            tx.execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![linked_sale_id, method, -amount, now],
            )
            .map_err(AppError::from)?;
        }
```

- [ ] **Step 6: Run tests to verify they pass**

Run: `cd src-tauri && cargo test commands::receipts:: -- --test-threads=1`
Expected: PASS (new void tests plus all existing receipts tests, which only assert on the original sale).

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/receipts.rs
git commit -m "feat(receipts): void mirrors original tenders as negative payments on the open shift

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 3: Return records a signed refund payment (tender default cash), requires an open shift

**Files:**
- Modify: `src-tauri/src/commands/receipts.rs` (`ReturnItemsRequest` struct; `return_items`)
- Test: `src-tauri/src/commands/receipts.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: `current_open_shift_id` (Task 2), `validate_payment_method` (existing).
- Produces: `ReturnItemsRequest` gains `refund_tender: Option<String>` (`refundTender` in camelCase; `None` → `"cash"`). After a return, the return counter-document's `shift_id` is the current open shift and it carries one negative `sale_payments` row for the returned total in the refund tender.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` in `src-tauri/src/commands/receipts.rs`:

```rust
    #[test]
    fn return_records_negative_cash_refund_by_default() {
        with_receipt_database("return_default_cash_refund", |db, seeded| {
            return_items(
                db,
                ReturnItemsRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Ostecen artikal".to_string(),
                    items: vec![ReturnItemRequest {
                        sale_item_id: seeded.sale_item_id,
                        quantity_milli: 1000,
                    }],
                    refund_tender: None,
                },
            )
            .expect("return should succeed");

            let connection = db.open().expect("database should open");
            let refund: i64 = connection
                .query_row(
                    "SELECT sp.amount_minor FROM sale_payments sp
                     JOIN sales s ON s.id = sp.sale_id
                     WHERE s.original_sale_id = ?1 AND s.document_type = 'return'
                       AND sp.payment_method = 'cash'",
                    params![seeded.sale_id],
                    |row| row.get(0),
                )
                .expect("cash refund should exist");
            assert_eq!(refund, -50_000);
        });
    }

    #[test]
    fn return_refund_tender_card_records_negative_card_refund() {
        with_receipt_database("return_card_refund", |db, seeded| {
            return_items(
                db,
                ReturnItemsRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Zamena velicine".to_string(),
                    items: vec![ReturnItemRequest {
                        sale_item_id: seeded.sale_item_id,
                        quantity_milli: 1000,
                    }],
                    refund_tender: Some("card".to_string()),
                },
            )
            .expect("return should succeed");

            let connection = db.open().expect("database should open");
            let refund: i64 = connection
                .query_row(
                    "SELECT sp.amount_minor FROM sale_payments sp
                     JOIN sales s ON s.id = sp.sale_id
                     WHERE s.original_sale_id = ?1 AND s.document_type = 'return'
                       AND sp.payment_method = 'card'",
                    params![seeded.sale_id],
                    |row| row.get(0),
                )
                .expect("card refund should exist");
            assert_eq!(refund, -50_000);
        });
    }

    #[test]
    fn return_requires_open_shift() {
        with_receipt_database("return_requires_open_shift", |db, seeded| {
            db.open()
                .expect("database should open")
                .execute("UPDATE shifts SET status = 'closed'", [])
                .expect("shift should close");

            let error = return_items(
                db,
                ReturnItemsRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Ostecen".to_string(),
                    items: vec![ReturnItemRequest {
                        sale_item_id: seeded.sale_item_id,
                        quantity_milli: 1000,
                    }],
                    refund_tender: None,
                },
            )
            .expect_err("return without an open shift should fail");
            assert_eq!(error.code, "shift_required");
        });
    }

    #[test]
    fn return_rejects_invalid_refund_tender() {
        with_receipt_database("return_invalid_tender", |db, seeded| {
            let error = return_items(
                db,
                ReturnItemsRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Ostecen".to_string(),
                    items: vec![ReturnItemRequest {
                        sale_item_id: seeded.sale_item_id,
                        quantity_milli: 1000,
                    }],
                    refund_tender: Some("bitcoin".to_string()),
                },
            )
            .expect_err("invalid refund tender should fail");
            assert_eq!(error.code, "validation_error");
        });
    }

    #[test]
    fn void_is_blocked_after_a_partial_return() {
        with_receipt_database("void_blocked_after_return", |db, seeded| {
            return_items(
                db,
                ReturnItemsRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Delimican povrat".to_string(),
                    items: vec![ReturnItemRequest {
                        sale_item_id: seeded.sale_item_id,
                        quantity_milli: 1000,
                    }],
                    refund_tender: None,
                },
            )
            .expect("partial return should succeed");

            let error = void_receipt(
                db,
                VoidReceiptRequest {
                    receipt_id: seeded.sale_id,
                    user_id: seeded.user_id,
                    reason: "Greska".to_string(),
                },
            )
            .expect_err("void after a return must stay blocked");
            assert_eq!(error.code, "invalid_receipt_state");
        });
    }
```

> `void_is_blocked_after_a_partial_return` is a regression guard, not a red-first test: it verifies that adding refund payments and the open-shift requirement did not unblock void-after-return (the remainder is reversed via the return flow instead — a design Non-goal). It should pass as soon as Task 3 compiles.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test commands::receipts::tests::return_ -- --test-threads=1`
Expected: FAIL — `refund_tender` field does not exist yet (compile error). That is the expected red state. (After Step 4 the four refund tests turn green and the `void_is_blocked_after_a_partial_return` guard passes.)

- [ ] **Step 3: Add the `refund_tender` field**

In `src-tauri/src/commands/receipts.rs`, change the `ReturnItemsRequest` struct to:

```rust
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnItemsRequest {
    pub receipt_id: i64,
    pub user_id: i64,
    pub reason: String,
    pub items: Vec<ReturnItemRequest>,
    #[serde(default)]
    pub refund_tender: Option<String>,
}
```

- [ ] **Step 4: Validate the tender, resolve the shift, write the refund**

In `return_items`:

(a) Near the top, after `let requested = normalize_return_items(&request.items)?;`, add:

```rust
    let refund_tender = request.refund_tender.as_deref().unwrap_or("cash").to_string();
    validate_payment_method(&refund_tender)?;
```

(b) After `ensure_returnable(&tx, &header)?;`, add:

```rust
        let shift_id = current_open_shift_id(&tx)?;
```

(c) In the `INSERT INTO sales (...) VALUES (…'return'…)` for the return document, change the shift parameter from `header.shift_id` to `shift_id`.

(d) After the `for (item, quantity_milli) in return_items { … }` loop closes and before the `UPDATE sales SET status = 'refunded' …` statement, insert:

```rust
        if total_minor != 0 {
            tx.execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                params![linked_sale_id, refund_tender, -total_minor, now],
            )
            .map_err(AppError::from)?;
        }
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test commands::receipts:: -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/receipts.rs
git commit -m "feat(receipts): returns record a signed refund payment (default cash) on the open shift

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 4: Rewrite report queries to sum the signed ledger, not `status`

**Files:**
- Modify: `src-tauri/src/commands/reports.rs` (six query functions + the `seed_reports_data` test fixture + affected assertions + one CSV-string assertion)
- Test: `src-tauri/src/commands/reports.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: counter-documents with signed payments (Tasks 2–3) and `document_type` on `sales`.
- Produces: all report money is `SUM(sp.amount_minor)` (signed) for payment figures and `document_type`-signed for header/line-item figures. No report query references `sales.status`.

**Why the fixture changes:** `seed_reports_data` currently models the voided sale (id 3) as a *status-flipped original* with a positive cash payment and `document_type = 'sale'` — the old model. Under the ledger model a void is a completed original **plus** a void counter-document with a negative payment. Migrating the fixture also corrects a latent under-count (net Sok quantity becomes 2, which is the true amount sold).

- [ ] **Step 1: Migrate the `seed_reports_data` fixture**

In `src-tauri/src/commands/reports.rs`, in the test `seed_reports_data`, change sale id 3's row in the sales-seeding array from `voided` to `completed` (leave every other field the same). That is, replace:

```rust
            (
                3,
                "R-003",
                "voided",
                3_000,
                0,
                500,
                3_000,
                "2026-06-17T11:00:00Z",
                3_000,
                0,
            ),
```

with:

```rust
            (
                3,
                "R-003",
                "completed",
                3_000,
                0,
                500,
                3_000,
                "2026-06-17T11:00:00Z",
                3_000,
                0,
            ),
```

Then, immediately after the sales-seeding `for` loop that inserts sales 1–3 and their payments (right after its closing `}`), add the void counter-document for sale 3:

```rust
        connection
            .execute(
                "INSERT INTO sales (
                    id, local_receipt_number, shift_id, cashier_id, status, fiscal_status,
                    document_type, original_sale_id, subtotal_minor, discount_minor, tax_minor,
                    total_minor, created_at, updated_at)
                 VALUES (4, 'STO-003', 1, 2, 'voided', 'not_fiscalized', 'void', 3, 3000, 0, 500,
                    3000, '2026-06-17T11:05:00Z', '2026-06-17T11:05:00Z')",
                [],
            )
            .expect("void document should insert");
        connection
            .execute(
                "INSERT INTO sale_items (
                    sale_id, product_id, product_name, product_sku, quantity_milli,
                    unit_price_minor, discount_minor, tax_rate_basis_points, tax_minor, total_minor)
                 VALUES (4, 2, 'Sok 1l', 'SOK-1L', -1000, 3000, 0, 2000, 500, 3000)",
                [],
            )
            .expect("void item should insert");
        connection
            .execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (4, 'cash', -3000, '2026-06-17T11:05:00Z')",
                [],
            )
            .expect("void refund payment should insert");
```

- [ ] **Step 2: Update the affected assertions and run to confirm they now fail**

Apply these exact assertion edits in `src-tauri/src/commands/reports.rs` (they encode the corrected, ledger-based numbers):

| Test fn | Old assertion | New assertion |
|---|---|---|
| `query_daily_turnover_groups_completed_sales_and_reduces_voids` | `total_minor, 12_000` | `total_minor, 15_000` |
| same | `cash_minor, 8_000` | `cash_minor, 11_000` |
| same | `receipt_count, 2` | `receipt_count, 3` |
| `query_payment_methods_handles_mixed_payments_and_voids` | `report.rows[0].total_minor, 8_000` | `report.rows[0].total_minor, 11_000` |
| `query_product_sales_returns_net_quantity_revenue_and_margin_estimate` | `report.rows[1].quantity_milli, 1_000` | `report.rows[1].quantity_milli, 2_000` |
| same | `report.rows[1].revenue_minor, 0` | `report.rows[1].revenue_minor, 3_000` |
| `query_daily_turnover_filters_by_shift_and_cashier` | `by_shift.summary.total_minor, 12_000` | `by_shift.summary.total_minor, 15_000` |
| same | `by_shift.summary.cash_minor, 8_000` | `by_shift.summary.cash_minor, 11_000` |
| same | `by_cashier.summary.total_minor, 12_000` | `by_cashier.summary.total_minor, 15_000` |
| `export_report_csv_writes_daily_turnover_with_serbian_headers` | `csv.contains("2026-06-17,2,8000,4000,12000,-3000")` | `csv.contains("2026-06-17,3,11000,4000,15000,-3000")` |

Leave unchanged (these stay correct): `card_minor, 4_000`; `refunds_or_voids_minor, -3_000`; `report.rows[0].revenue_minor, 12_000` and `estimated_margin_minor, 11_160` (Kafa); `report.rows[1].payment_method, "card"` and its `total_minor, 4_000`.

Run: `cd src-tauri && cargo test commands::reports:: -- --test-threads=1`
Expected: FAIL — the queries still key on `status`, so the migrated fixture produces the old numbers and the updated assertions do not match yet.

- [ ] **Step 3: Rewrite `query_daily_turnover`**

In the `query_daily_turnover` SQL, make these replacements:
- `SUM(CASE WHEN s.status = 'completed' THEN 1 ELSE 0 END) AS receipt_count,` → `SUM(CASE WHEN s.document_type = 'sale' THEN 1 ELSE 0 END) AS receipt_count,`
- both occurrences of `CASE WHEN ps.status = 'completed' THEN sp.amount_minor ELSE -sp.amount_minor END` → `sp.amount_minor`
- `SUM(CASE WHEN s.status = 'completed' THEN s.total_minor ELSE -s.total_minor END) AS total_minor,` → `SUM(CASE WHEN s.document_type = 'sale' THEN s.total_minor ELSE -s.total_minor END) AS total_minor,`
- `-SUM(CASE WHEN s.status <> 'completed' THEN s.total_minor ELSE 0 END) AS refunds_or_voids_minor,` → `-SUM(CASE WHEN s.document_type IN ('void', 'return') THEN s.total_minor ELSE 0 END) AS refunds_or_voids_minor,`
- `SUM(CASE WHEN s.status <> 'completed' THEN 1 ELSE 0 END) AS refunds_or_voids_count` → `SUM(CASE WHEN s.document_type IN ('void', 'return') THEN 1 ELSE 0 END) AS refunds_or_voids_count`

- [ ] **Step 4: Rewrite `query_shift_turnover`**

Replacements in its SQL:
- `SUM(CASE WHEN s.status = 'completed' THEN 1 ELSE 0 END) AS receipt_count,` → `SUM(CASE WHEN s.document_type = 'sale' THEN 1 ELSE 0 END) AS receipt_count,`
- the cash block →
```
    COALESCE(SUM(CASE WHEN sp.payment_method = 'cash' THEN sp.amount_minor ELSE 0 END), 0) AS cash_minor,
```
- the card block →
```
    COALESCE(SUM(CASE WHEN sp.payment_method = 'card' THEN sp.amount_minor ELSE 0 END), 0) AS card_minor,
```
- `SUM(CASE WHEN s.status = 'completed' THEN s.total_minor ELSE -s.total_minor END) AS total_minor` → `SUM(CASE WHEN s.document_type = 'sale' THEN s.total_minor ELSE -s.total_minor END) AS total_minor`

- [ ] **Step 5: Rewrite `query_cashier_turnover`**

Replacements:
- `SUM(CASE WHEN s.status = 'completed' THEN 1 ELSE 0 END) AS receipt_count,` → `SUM(CASE WHEN s.document_type = 'sale' THEN 1 ELSE 0 END) AS receipt_count,`
- `SUM(CASE WHEN s.status = 'completed' THEN s.total_minor ELSE -s.total_minor END) AS total_minor` → `SUM(CASE WHEN s.document_type = 'sale' THEN s.total_minor ELSE -s.total_minor END) AS total_minor`

- [ ] **Step 6: Rewrite `query_payment_methods`**

Replacements:
- `COUNT(DISTINCT CASE WHEN s.status = 'completed' THEN s.id END) AS receipt_count,` → `COUNT(DISTINCT CASE WHEN s.document_type = 'sale' THEN s.id END) AS receipt_count,`
- `SUM(CASE WHEN s.status = 'completed' THEN sp.amount_minor ELSE -sp.amount_minor END) AS total_minor` → `SUM(sp.amount_minor) AS total_minor`

- [ ] **Step 7: Rewrite `query_product_sales` and `query_category_sales`**

In BOTH SQL statements, replace every occurrence of `s.status = 'completed'` with `s.document_type = 'sale'`. There are five per statement (quantity, revenue, discount, and the two arms of the margin expression). Do not otherwise change the margin formula.

- [ ] **Step 8: Run tests to verify they pass**

Run: `cd src-tauri && cargo test commands::reports:: -- --test-threads=1`
Expected: PASS (updated assertions plus all filter/cross-cashier tests, which are unaffected).

- [ ] **Step 9: Add an explicit return-document report test**

Add to the reports `#[cfg(test)] mod tests` to prove `document_type = 'return'` is handled (the fixture only exercises `'void'`):

```rust
    #[test]
    fn daily_turnover_nets_a_return_document() {
        let db_path = test_database_path("daily_turnover_nets_a_return_document");
        {
            let db = Db::new(&db_path).expect("database should initialize");
            let connection = db.open().expect("database should open");
            connection
                .execute(
                    "INSERT INTO users (id, username, display_name, role, created_at, updated_at)
                     VALUES (2, 'mira', 'Mira Kasir', 'cashier', '2026-06-17T07:00:00Z', '2026-06-17T07:00:00Z')",
                    [],
                )
                .expect("cashier should insert");
            connection
                .execute(
                    "INSERT INTO shifts (id, user_id, opened_at, opening_cash_minor, expected_cash_minor, status, created_at, updated_at)
                     VALUES (1, 2, '2026-06-17T07:30:00Z', 0, 0, 'open', '2026-06-17T07:30:00Z', '2026-06-17T07:30:00Z')",
                    [],
                )
                .expect("shift should insert");
            connection
                .execute(
                    "INSERT INTO sales (id, local_receipt_number, shift_id, cashier_id, status, fiscal_status, subtotal_minor, discount_minor, tax_minor, total_minor, created_at, updated_at)
                     VALUES (1, 'R-1', 1, 2, 'completed', 'not_fiscalized', 1000, 0, 167, 1000, '2026-06-17T09:00:00Z', '2026-06-17T09:00:00Z')",
                    [],
                )
                .expect("sale should insert");
            connection
                .execute(
                    "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                     VALUES (1, 'cash', 1000, '2026-06-17T09:00:00Z')",
                    [],
                )
                .expect("payment should insert");
            connection
                .execute(
                    "INSERT INTO sales (id, local_receipt_number, shift_id, cashier_id, status, fiscal_status, document_type, original_sale_id, subtotal_minor, discount_minor, tax_minor, total_minor, created_at, updated_at)
                     VALUES (2, 'POV-1', 1, 2, 'refunded', 'not_fiscalized', 'return', 1, 300, 0, 50, 300, '2026-06-17T09:30:00Z', '2026-06-17T09:30:00Z')",
                    [],
                )
                .expect("return document should insert");
            connection
                .execute(
                    "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                     VALUES (2, 'cash', -300, '2026-06-17T09:30:00Z')",
                    [],
                )
                .expect("refund payment should insert");

            let report = super::query_daily_turnover(
                &connection,
                &super::ReportDateQuery {
                    from: "2026-06-17".to_string(),
                    to: "2026-06-17".to_string(),
                    shift_id: None,
                    cashier_id: None,
                },
            )
            .expect("daily turnover should query");
            assert_eq!(report.summary.cash_minor, 700);
            assert_eq!(report.summary.total_minor, 700);
            assert_eq!(report.summary.receipt_count, 1);
            assert_eq!(report.rows[0].refunds_or_voids_minor, 300);
            assert_eq!(report.rows[0].refunds_or_voids_count, 1);
        }
        std::fs::remove_file(&db_path).expect("test database should be removed");
    }
```

- [ ] **Step 10: Run tests and commit**

Run: `cd src-tauri && cargo test commands::reports:: -- --test-threads=1`
Expected: PASS

```bash
git add src-tauri/src/commands/reports.rs
git commit -m "feat(reports): derive money from the signed payment ledger and document_type, not status

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 5: Rewrite the shift summary to include all documents

**Files:**
- Modify: `src-tauri/src/commands/shifts.rs` (`load_shift_summary` SQL)
- Test: `src-tauri/src/commands/shifts.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: signed counter-document payments (Tasks 2–3).
- Produces: `cash_sales_minor`/`card_sales_minor` on a `ShiftSummary` are the signed sums across every document in the shift.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` in `src-tauri/src/commands/shifts.rs`. It uses the existing `with_state` and `seed_cashier` helpers and `rusqlite::params` (already imported at line 302). Add `current_shift_for_user` to the existing `use super::{…}` line (currently `use super::{close_shift_for_user, open_shift_for_user, CloseShiftRequest, OpenShiftRequest};`) so it becomes:

```rust
    use super::{
        close_shift_for_user, current_shift_for_user, open_shift_for_user, CloseShiftRequest,
        OpenShiftRequest,
    };
```

Then add the test:

```rust
    #[test]
    fn shift_summary_nets_returns_against_cash_sales() {
        with_state("shift_summary_nets_returns", |state| {
            let user_id = seed_cashier(state);
            open_shift_for_user(
                state,
                user_id,
                OpenShiftRequest {
                    opening_cash_minor: 0,
                    note: None,
                },
            )
            .expect("shift should open");

            let conn = state.db().open().expect("database should open");
            let shift_id: i64 = conn
                .query_row(
                    "SELECT id FROM shifts WHERE user_id = ?1 AND status = 'open'",
                    params![user_id],
                    |row| row.get(0),
                )
                .expect("open shift id should load");
            conn.execute(
                "INSERT INTO sales (local_receipt_number, shift_id, cashier_id, status, fiscal_status, subtotal_minor, discount_minor, tax_minor, total_minor, created_at, updated_at)
                 VALUES ('R-1', ?1, ?2, 'completed', 'not_fiscalized', 1000, 0, 167, 1000, '2026-06-18T09:00:00Z', '2026-06-18T09:00:00Z')",
                params![shift_id, user_id],
            )
            .expect("sale should insert");
            let sale_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, 'cash', 1000, '2026-06-18T09:00:00Z')",
                params![sale_id],
            )
            .expect("payment should insert");
            conn.execute(
                "INSERT INTO sales (local_receipt_number, shift_id, cashier_id, status, fiscal_status, document_type, original_sale_id, subtotal_minor, discount_minor, tax_minor, total_minor, created_at, updated_at)
                 VALUES ('POV-1', ?1, ?2, 'refunded', 'not_fiscalized', 'return', ?3, 300, 0, 50, 300, '2026-06-18T09:30:00Z', '2026-06-18T09:30:00Z')",
                params![shift_id, user_id, sale_id],
            )
            .expect("return document should insert");
            let return_id = conn.last_insert_rowid();
            conn.execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, 'cash', -300, '2026-06-18T09:30:00Z')",
                params![return_id],
            )
            .expect("refund payment should insert");

            let summary = current_shift_for_user(state, user_id)
                .expect("summary should query")
                .expect("open shift summary should exist");
            assert_eq!(summary.cash_sales_minor, 700);
            assert_eq!(summary.card_sales_minor, 0);
            assert_eq!(summary.expected_cash_minor, 700);
        });
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd src-tauri && cargo test commands::shifts::tests::shift_summary_nets_returns -- --test-threads=1`
Expected: FAIL — the summary reports `cash_sales_minor == 1000` because the join still filters `s.status = 'completed'` and drops the refund.

- [ ] **Step 3: Drop the status filter from the join**

In `load_shift_summary`, change:

```
LEFT JOIN sales s ON s.shift_id = sh.id AND s.status = 'completed'
```

to:

```
LEFT JOIN sales s ON s.shift_id = sh.id
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd src-tauri && cargo test commands::shifts:: -- --test-threads=1`
Expected: PASS (new test plus all existing shift tests, whose fixtures contain only completed sales and are therefore unchanged by dropping the filter).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/shifts.rs
git commit -m "feat(shifts): sum the full signed payment ledger for shift cash reconciliation

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 6: Frontend — refund tender selector and open-shift error

**Files:**
- Modify: `src/services/types.ts` (`ReturnItemsRequest`)
- Modify: `src/app/ReceiptsScreen.tsx` (return sheet: tender state + `NativeSelect`; pass `refundTender`)
- Test: `src/app/ReceiptsScreen.test.tsx`, `src/services/local-adapter.test.ts`

**Interfaces:**
- Consumes: the backend `refundTender` field (Task 3) and `shift_required` error (Tasks 2–3).
- Produces: the return sheet lets the operator pick Gotovina (default) or Kartica, and passes `refundTender` through `receipts_return_items`.

- [ ] **Step 1: Write the failing adapter test**

Do **not** modify the existing `receipts_return_items` test (it passes no `refundTender`, and an optional field keeps it green). Add this new test to the same `describe` block in `src/services/local-adapter.test.ts`, right after the `"routes receipt use cases through stable Tauri command names"` test:

```ts
  it("forwards refundTender through receipts_return_items", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    const services = createLocalServices(invoke);

    await services.receipts.returnItems({
      receiptId: 7,
      userId: 1,
      reason: "Zamena velicine",
      items: [{ saleItemId: 3, quantityMilli: 1000 }],
      refundTender: "card",
    });

    expect(invoke).toHaveBeenCalledWith("receipts_return_items", {
      request: {
        receiptId: 7,
        userId: 1,
        reason: "Zamena velicine",
        items: [{ saleItemId: 3, quantityMilli: 1000 }],
        refundTender: "card",
      },
    });
  });
```

- [ ] **Step 2: Run test to verify it fails**

Run: `bun run test src/services/local-adapter.test.ts`
Expected: FAIL — `refundTender` is not on the `ReturnItemsRequest` type (type error) / not forwarded.

- [ ] **Step 3: Add `refundTender` to the type**

In `src/services/types.ts`, change `ReturnItemsRequest` to:

```ts
export interface ReturnItemsRequest {
  receiptId: number;
  userId: number;
  reason: string;
  items: ReturnItemRequest[];
  refundTender?: "cash" | "card";
}
```

(The adapter at `src/services/local-adapter.ts:130-131` already forwards the whole `request` object, so no adapter change is needed.)

- [ ] **Step 4: Run adapter test to verify it passes**

Run: `bun run test src/services/local-adapter.test.ts`
Expected: PASS

- [ ] **Step 5: Write the failing UI test**

Add this test to the first `describe("ReceiptsScreen", …)` block in `src/app/ReceiptsScreen.test.tsx` (it mirrors the existing `"threads the session user id into the return call"` test exactly, adding the tender assertions):

```ts
  it("defaults the refund tender to cash and forwards it", async () => {
    const user = userEvent.setup();
    const services = createMockServices();
    const returnItems = vi.spyOn(services.receipts, "returnItems");

    render(<ReceiptsScreen receipts={services.receipts} userId={9} />);

    await user.click(
      await screen.findByRole("button", { name: "Detalji za R-2026-0001" }),
    );
    await user.click(await screen.findByRole("button", { name: "Povrat artikala" }));

    expect(await screen.findByLabelText("Nacin povrata")).toHaveValue("cash");

    const quantity = await screen.findByLabelText("Kolicina za Kafa 200 g");
    await user.clear(quantity);
    await user.type(quantity, "1");
    await user.type(screen.getByLabelText("Razlog povrata"), "Ostecen artikal");
    await user.click(screen.getByRole("button", { name: "Sacuvaj povrat" }));

    await waitFor(() =>
      expect(returnItems).toHaveBeenCalledWith(
        expect.objectContaining({ refundTender: "cash" }),
      ),
    );
  });
```

- [ ] **Step 6: Run test to verify it fails**

Run: `bun run test src/app/ReceiptsScreen.test.tsx`
Expected: FAIL — no element labelled "Nacin povrata"; `returnItems` called without `refundTender`.

- [ ] **Step 7: Add tender state and the selector**

In `src/app/ReceiptsScreen.tsx`:

(a) Add state near the other return state (`const [returnReason, setReturnReason] = useState("");`):

```tsx
  const [refundTender, setRefundTender] = useState<"cash" | "card">("cash");
```

(b) In `submitReturn`, add `refundTender,` to the `receipts.returnItems({ … })` argument object (alongside `reason`).

(c) In the return sheet JSX, add a tender field immediately before the `Razlog povrata` field (before the `<Field data-invalid={Boolean(returnError)}>` that wraps `return-reason`):

```tsx
              <Field>
                <FieldLabel htmlFor="return-tender">Nacin povrata</FieldLabel>
                <NativeSelect
                  id="return-tender"
                  aria-label="Nacin povrata"
                  value={refundTender}
                  onChange={(event) =>
                    setRefundTender(event.target.value as "cash" | "card")
                  }
                >
                  <NativeSelectOption value="cash">Gotovina</NativeSelectOption>
                  <NativeSelectOption value="card">Kartica</NativeSelectOption>
                </NativeSelect>
              </Field>
```

`NativeSelect` and `NativeSelectOption` are already imported (`ReceiptsScreen.tsx:42`) and used for the payment-method filter at `ReceiptsScreen.tsx:387-402` — mirror that markup. The `aria-label="Nacin povrata"` is what the test queries.

(d) When the return sheet is closed/reset, reset the tender to cash. Find where `setReturnReason("")` and `setReturnQuantities` are reset (the return-sheet open/close handlers) and add `setRefundTender("cash");` beside them.

- [ ] **Step 8: Run the frontend tests to verify they pass**

Run: `bun run test src/app/ReceiptsScreen.test.tsx src/services/local-adapter.test.ts`
Expected: PASS

- [ ] **Step 9: Full verification and commit**

Run all gates:
```bash
bun run test
bun run build
cd src-tauri && cargo test -- --test-threads=1
cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings
cd src-tauri && cargo fmt --check
cd .. && git diff --check
```
Expected: all pass.

```bash
git add src/services/types.ts src/app/ReceiptsScreen.tsx src/app/ReceiptsScreen.test.tsx src/services/local-adapter.test.ts
git commit -m "feat(receipts-ui): refund tender selector on the return sheet

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Verification summary

After Task 6, the end-to-end invariant holds: for any mix of sales, partial/full returns, and voids within a shift, the shift summary cash and the daily turnover are both signed sums of the same `sale_payments` ledger and agree to the para; no money query reads `sales.status`; and a return/void requires and attaches to the current open shift. Deliberately deferred (per the design's Non-goals): proportional void-after-partial-return, the margin/COGS VAT bug, payment-tender expansion, and shift attribution by logged-in user.
