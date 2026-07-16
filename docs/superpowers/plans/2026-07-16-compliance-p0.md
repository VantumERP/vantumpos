# Compliance P0 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the five P0 compliance software changes (non-fiscal banner, encrypted backups, retention-guarded reset/restore, anti-evasion invariants, ESIR receipt-number nudge) plus six lawyer-ready Serbian legal-document drafts, closing every vendor-side criminal/prekršaj exposure identified in `docs/SERBIAN-LAW-COMPLIANCE.md`.

**Architecture:** Additive throughout. One new SQLite migration (v8) adds an append-only `compliance_log` table and a nullable `sales.esir_receipt_number` column. A new `backup_crypto` Rust module provides envelope encryption (argon2id-wrapped random data key, XChaCha20-Poly1305 file body); `perform_backup`/`restore_backup` gain an encryption branch that is inert until an admin sets a passphrase. Frontend changes are localized to the register post-sale dialog, the receipts detail sheet, and the settings backup panel. Legal drafts are Markdown docs under `docs/compliance/`.

**Tech Stack:** Rust + rusqlite/SQLite + Tauri v2 commands; React + TypeScript + shadcn/ui + Vitest/RTL; `argon2` (already a dep) + new `chacha20poly1305` and `getrandom` crates.

## Global Constraints

- **DO NOT boot, launch, or run the application** (no `tauri dev`, `cargo tauri dev`, dev/preview server, or built binary). Verify only via `cargo test` / `cargo build` / `bun run test` / `bun run build` / clippy / fmt. Standing user instruction.
- Money is integer minor units (para); quantities are milli-units. Never floats.
- Migrations are **append-only**: v8 is the next version; never edit v1–v7. After adding v8, bump the migration-count assertion in `src-tauri/src/db/mod.rs` from `7` to `8` and add any new table to `CORE_TABLES`.
- Serbian Latin UI copy with correct diacritics (šđčćž). Exact mandated strings are given verbatim per task — copy them character-for-character.
- **No output may resemble a fiscal receipt:** never add a QR code, a "FISKALNI RAČUN" heading, or a PIB/PFR-number/brojač block to any sale surface.
- All acting-user attribution comes from the session (`require_session` / `require_admin`), never from a client-supplied id.
- Every task ends green on: `bun run test`, `bun run build`, `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`.
- Commit trailer on every commit:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  ```

---

### Task 1: SW-1 — "OVO NIJE FISKALNI RAČUN" banner

**Files:**
- Modify: `src/app/register/RegisterScreen.tsx` (post-sale `completedSale` Dialog, ~line 627–668)
- Modify: `src/app/ReceiptsScreen.tsx` (detail sheet, ~line 623)
- Test: `src/app/register/RegisterScreen.test.tsx`
- Test: `src/app/ReceiptsScreen.test.tsx`

**Interfaces:**
- Consumes: nothing new.
- Produces: nothing consumed by later tasks (purely presentational).

Legal driver: PVFR čl. 2 st. 8–10; posture defense for ZF čl. 15 st. 1 tač. 5.

- [ ] **Step 1: Write the failing test (register dialog banner)**

Add to `src/app/register/RegisterScreen.test.tsx` (find the test that completes a sale and asserts the dialog shows the receipt number; add a sibling assertion, or add a new test after it). The banner must render once the post-sale dialog is open:

```tsx
it("shows the non-fiscal banner in the completed-sale dialog", async () => {
  renderRegister();
  await completeASale(); // reuse the existing helper the file already uses to reach the completed dialog
  const banners = await screen.findAllByText("OVO NIJE FISKALNI RAČUN");
  expect(banners.length).toBeGreaterThanOrEqual(2); // top and bottom
});
```

If the file has no reusable "complete a sale" helper, model the new test on the existing test that opens `completedSale` and asserts `Račun` — copy its setup verbatim, then assert the banner.

- [ ] **Step 2: Run it to verify it fails**

Run: `bun run test -- RegisterScreen`
Expected: FAIL — banner text not found.

- [ ] **Step 3: Implement the banner in the register dialog**

In `src/app/register/RegisterScreen.tsx`, inside the post-sale `<DialogContent>`, replace the description line and wrap the body with top+bottom banners. Change:

```tsx
              <DialogHeader>
                <DialogTitle>Račun {completedSale.localReceiptNumber}</DialogTitle>
                <DialogDescription>Lokalni račun</DialogDescription>
              </DialogHeader>
```

to:

```tsx
              <DialogHeader>
                <DialogTitle>Račun {completedSale.localReceiptNumber}</DialogTitle>
                <DialogDescription>
                  Interni pregled prodaje — nije fiskalni račun
                </DialogDescription>
              </DialogHeader>
              <NonFiscalBanner />
```

Then, immediately before the closing `</>` of the `completedSale && (...)` block (after the totals `</div>`), add a second banner:

```tsx
              <NonFiscalBanner />
```

Add this component at the bottom of the file (module scope, not exported):

```tsx
function NonFiscalBanner() {
  // Legal: PVFR čl. 2 st. 8–10 — any sale-itemizing surface must be
  // unmistakably NON-fiscal. Never add a QR code, "FISKALNI RAČUN" heading,
  // or PIB/PFR/brojač block to this or any future receipt export.
  return (
    <div
      role="note"
      className="rounded-md border-2 border-destructive bg-destructive/10 px-3 py-2 text-center text-2xl font-bold uppercase tracking-wide text-destructive"
    >
      OVO NIJE FISKALNI RAČUN
    </div>
  );
}
```

(The line items render at `text-sm`/base; `text-2xl font-bold` satisfies the ≥2× requirement.)

- [ ] **Step 4: Run the register test to verify it passes**

Run: `bun run test -- RegisterScreen`
Expected: PASS.

- [ ] **Step 5: Write the failing test (receipts detail banner)**

Add to `src/app/ReceiptsScreen.test.tsx`, modeled on the existing test that opens a receipt's detail sheet and asserts `Račun {number}` (search the file for `Detalji za` or `detail.receiptNumber`). After the detail is shown:

```tsx
it("shows the non-fiscal banner in the receipt detail", async () => {
  // ...reuse the existing setup that selects a receipt and opens its detail sheet...
  expect(await screen.findByText("OVO NIJE FISKALNI RAČUN")).toBeInTheDocument();
});
```

- [ ] **Step 6: Run it to verify it fails**

Run: `bun run test -- ReceiptsScreen`
Expected: FAIL.

- [ ] **Step 7: Implement the banner in the receipts detail**

In `src/app/ReceiptsScreen.tsx`, the detail region begins around line 623 with `<h2 className="text-base font-semibold">Račun {detail.receiptNumber}</h2>`. Immediately above that `<h2>` (as the first child of the detail container), insert:

```tsx
        <div
          role="note"
          className="rounded-md border-2 border-destructive bg-destructive/10 px-3 py-2 text-center text-xl font-bold uppercase tracking-wide text-destructive"
        >
          OVO NIJE FISKALNI RAČUN
        </div>
```

(Detail sheet is an internal working screen — top banner only per the design.)

- [ ] **Step 8: Run both frontend suites**

Run: `bun run test -- RegisterScreen ReceiptsScreen`
Expected: PASS.

- [ ] **Step 9: Full gates + commit**

Run: `bun run test && bun run build`
Expected: PASS.

```bash
git add src/app/register/RegisterScreen.tsx src/app/register/RegisterScreen.test.tsx src/app/ReceiptsScreen.tsx src/app/ReceiptsScreen.test.tsx
git commit -m "feat(compliance): non-fiscal banner on sale surfaces (SW-1)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: SW-3/SW-5 foundation — migration v8 (`compliance_log` + `sales.esir_receipt_number`)

**Files:**
- Modify: `src-tauri/src/db/migrations.rs` (append v8 to the `MIGRATIONS` array, after v7 at ~line 289)
- Modify: `src-tauri/src/db/mod.rs` (`CORE_TABLES` ~line 110; migration-count assertion ~line 496)
- Test: `src-tauri/src/db/mod.rs` (add tests in the existing `mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: table `compliance_log(id INTEGER PK, event_type TEXT, detail_json TEXT, user_id INTEGER NULL, created_at TEXT)` with `event_type` constrained to `('trading_data_reset','backup_restored')`; nullable column `sales.esir_receipt_number TEXT`. Consumed by Tasks 3, 4, 5.

- [ ] **Step 1: Write the failing test**

In `src-tauri/src/db/mod.rs` `mod tests`, add:

```rust
    #[test]
    fn migration_v8_adds_compliance_log_and_esir_column() {
        with_test_database("migration_v8_compliance_and_esir", |db| {
            let connection = db.open().expect("database should open");

            assert!(
                schema_object_exists(&connection, "table", "compliance_log"),
                "expected compliance_log table"
            );

            let esir_exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('sales') WHERE name = 'esir_receipt_number'",
                    [],
                    |row| row.get(0),
                )
                .expect("column metadata should query");
            assert_eq!(esir_exists, 1, "expected sales.esir_receipt_number");

            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='compliance_log'",
                    [],
                    |row| row.get(0),
                )
                .expect("compliance_log schema should load");
            assert!(
                schema.contains("trading_data_reset") && schema.contains("backup_restored"),
                "expected event_type CHECK, schema was: {schema}"
            );
        });
    }
```

Also update the existing migration-count test `migrate_is_idempotent_and_records_initial_migration_once`: change `assert_eq!(migration_count, 7);` to `assert_eq!(migration_count, 8);`.

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml migration_v8 -- --test-threads=1`
Expected: FAIL — `compliance_log` table missing.

- [ ] **Step 3: Append migration v8**

In `src-tauri/src/db/migrations.rs`, add after the v7 `Migration { ... }` (before the closing `];` at ~line 290):

```rust
    Migration {
        version: 8,
        name: "compliance_log_and_esir_receipt_number",
        sql: r#"
CREATE TABLE compliance_log (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    event_type TEXT NOT NULL CHECK (event_type IN ('trading_data_reset', 'backup_restored')),
    detail_json TEXT,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_compliance_log_created_at ON compliance_log(created_at);

ALTER TABLE sales ADD COLUMN esir_receipt_number TEXT;
"#,
    },
```

- [ ] **Step 4: Register the new table + index in the schema-presence lists**

In `src-tauri/src/db/mod.rs`, add `"compliance_log",` to the `CORE_TABLES` array (after `"cash_movements",`) and `"idx_compliance_log_created_at",` to the `EXPLICIT_INDEXES` array (after `"idx_cash_movements_shift",`).

- [ ] **Step 5: Run the DB tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib db:: -- --test-threads=1`
Expected: PASS (including `migration_v8_...`, the updated count test, and the `CORE_TABLES`/`EXPLICIT_INDEXES` existence tests).

- [ ] **Step 6: Full gates + commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src-tauri/src/db/migrations.rs src-tauri/src/db/mod.rs
git commit -m "feat(compliance): migration v8 — compliance_log + sales.esir_receipt_number (SW-3/SW-5)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: SW-3 — retention guards (compliance_log writes + reset/restore + dialog copy)

**Files:**
- Modify: `src-tauri/src/commands/backup.rs` (`reset_trading_data`, `restore_backup`, add `insert_compliance_event`)
- Modify: `src/app/settings/SettingsScreen.tsx` (reset confirmation dialog copy)
- Test: `src-tauri/src/commands/backup.rs` (`mod tests`)
- Test: `src/app/settings/SettingsScreen.test.tsx`

**Interfaces:**
- Consumes: `compliance_log` table (Task 2); `require_session`/`require_admin` from `super::auth`; existing `create_backup`.
- Produces: `fn insert_compliance_event(conn: &rusqlite::Connection, event_type: &str, detail_json: &str, user_id: Option<i64>) -> rusqlite::Result<()>` (module-private, tx-aware). `reset_trading_data` writes a `trading_data_reset` row inside the wipe transaction; `restore_backup` writes a `backup_restored` row after migrate. Consumed by Task 4.

Legal driver: ZoRač čl. 28; ZPDV čl. 47 + ZPPPA čl. 114ž (10-year horizon); ZPPPA čl. 175b; ZAG čl. 16/65.

- [ ] **Step 1: Write the failing test (reset writes tombstone that survives the wipe)**

In `src-tauri/src/commands/backup.rs` `mod tests`, add:

```rust
    #[test]
    fn reset_trading_data_writes_compliance_tombstone_that_survives_wipe() {
        with_state("reset_writes_compliance_tombstone", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-reset-tombstone");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");
            seed_trading_data(state);

            reset_trading_data(state, "OBRISI PODATKE").expect("reset should succeed");

            assert_eq!(count(state, "sales"), 0);
            assert_eq!(count(state, "compliance_log"), 1);

            let conn = state.db().open().expect("database should open");
            let event_type: String = conn
                .query_row(
                    "SELECT event_type FROM compliance_log ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .expect("compliance event should exist");
            assert_eq!(event_type, "trading_data_reset");
        });
    }
```

- [ ] **Step 2: Run it to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml reset_trading_data_writes_compliance_tombstone -- --test-threads=1`
Expected: FAIL — `compliance_log` count is 0.

- [ ] **Step 3: Implement `insert_compliance_event` and write the reset tombstone in-transaction**

In `src-tauri/src/commands/backup.rs`, add a helper near `insert_backup_job`:

```rust
/// Appends an audit row to the never-deleted `compliance_log`. Takes a live
/// connection/transaction so callers can write the event inside the same
/// atomic unit as the operation it records (e.g. the reset wipe).
fn insert_compliance_event(
    conn: &Connection,
    event_type: &str,
    detail_json: &str,
    user_id: Option<i64>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO compliance_log (event_type, detail_json, user_id, created_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        params![event_type, detail_json, user_id],
    )?;
    Ok(())
}
```

Add `Connection` to the `rusqlite` import at the top: change `use rusqlite::{params, DatabaseName, OptionalExtension, Row};` to `use rusqlite::{params, Connection, DatabaseName, OptionalExtension, Row};`.

In `reset_trading_data`, capture the acting admin's id from `require_admin` (reuse its return — no second lookup). Change the first line of the function body from `super::auth::require_admin(state)?;` to:

```rust
    let acting = super::auth::require_admin(state)?;
```

Then, inside the existing transaction (after the `UPDATE settings ... receipt_numbering` execute and before `tx.commit()?;`), add the tombstone write:

```rust
    let detail = serde_json::json!({
        "note": "Go-live reset (SW-3).",
        "retention": "10y (ZoRač čl. 28; ZPDV čl. 47)",
    })
    .to_string();
    insert_compliance_event(&tx, "trading_data_reset", &detail, Some(acting.id))?;
```

(`tx` is a `Transaction`, which derefs to `Connection`, so `&tx` satisfies `&Connection`. `require_admin` returns `UserAccount`, which has an `id: i64` field.)

- [ ] **Step 4: Run the reset test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml reset_trading_data -- --test-threads=1`
Expected: PASS (both the new tombstone test and the existing reset tests).

- [ ] **Step 5: Write the failing test (restore stamps the restored DB)**

Add to `mod tests`:

```rust
    #[test]
    fn restore_backup_writes_compliance_event() {
        with_state("restore_writes_compliance_event", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-restore-compliance");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");

            // Make a real backup file to restore from.
            let job = create_backup(
                state,
                CreateBackupRequest {
                    backup_folder: Some(folder.display().to_string()),
                    backup_type: None,
                },
            )
            .expect("backup should succeed");

            restore_backup(
                state,
                RestoreBackupRequest {
                    path: job.path.clone(),
                    confirmation_text: "VRATI PODATKE".to_string(),
                },
            )
            .expect("restore should succeed");

            assert_eq!(count(state, "compliance_log"), 1);
            let conn = state.db().open().expect("database should open");
            let event_type: String = conn
                .query_row(
                    "SELECT event_type FROM compliance_log ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .expect("compliance event should exist");
            assert_eq!(event_type, "backup_restored");
        });
    }
```

- [ ] **Step 6: Run it to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml restore_backup_writes_compliance_event -- --test-threads=1`
Expected: FAIL — count is 0.

- [ ] **Step 7: Implement the restore stamp**

In `restore_backup`, reuse `require_admin`'s return for the acting id. Change the first line of the function body from `super::auth::require_admin(state)?;` to:

```rust
    let acting = super::auth::require_admin(state)?;
```

Then, after `state.db().migrate()?;` and before the final `insert_backup_job(...)`, write the event:

```rust
    {
        let conn = state.db().open()?;
        let detail = serde_json::json!({ "source_path": source_path.display().to_string() }).to_string();
        insert_compliance_event(&conn, "backup_restored", &detail, Some(acting.id))?;
    }
```

- [ ] **Step 8: Run the restore test to verify it passes**

Run: `cargo test --manifest-path src-tauri/Cargo.toml restore_backup -- --test-threads=1`
Expected: PASS.

- [ ] **Step 9: Write the failing test (reset dialog retention copy)**

In `src/app/settings/SettingsScreen.test.tsx`, find the test that opens the reset/go-live dialog (search for `OBRISI PODATKE` or the reset button label). Add an assertion that the retention warning is shown when the dialog is open:

```tsx
it("shows the 10-year retention warning in the reset dialog", async () => {
  // ...reuse the existing setup that navigates to the backup tab and opens the reset dialog...
  expect(
    await screen.findByText(/do 10 godina/i),
  ).toBeInTheDocument();
});
```

- [ ] **Step 10: Run it to verify it fails**

Run: `bun run test -- SettingsScreen`
Expected: FAIL — retention text not present.

- [ ] **Step 11: Implement the reset-dialog retention copy**

In `src/app/settings/SettingsScreen.tsx`, locate the reset confirmation dialog (the one gated by the `OBRISI PODATKE` confirmation). Add this warning paragraph inside the dialog body, above the confirmation input:

```tsx
          <p className="text-sm text-muted-foreground">
            Zakon zahteva čuvanje evidencija do 10 godina (ZoRač čl. 28; ZPDV
            čl. 47). Pre brisanja se obavezno pravi rezervna kopija — čuvajte je
            trajno. Pravna lica ne smeju uništavati dokumentarni materijal bez
            pismenog odobrenja arhiva.
          </p>
```

- [ ] **Step 12: Run the settings suite to verify it passes**

Run: `bun run test -- SettingsScreen`
Expected: PASS.

- [ ] **Step 13: Full gates + commit**

Run: `bun run test && bun run build && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src-tauri/src/commands/backup.rs src/app/settings/SettingsScreen.tsx src/app/settings/SettingsScreen.test.tsx
git commit -m "feat(compliance): retention-guarded reset/restore with compliance_log tombstones (SW-3)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: SW-4 — anti-evasion invariant tests + posture note reference

**Files:**
- Test: `src-tauri/src/commands/receipts.rs` (`mod tests`) — ledger-append-only invariants
- Test: `src-tauri/src/commands/backup.rs` (`mod tests`) — reset requires backup+tombstone
- (No production code changes expected; if a genuine gap is found, STOP and report it — do not silently "fix" money code.)

**Interfaces:**
- Consumes: existing `void_receipt`/`return_items` (receipts.rs) and `reset_trading_data` (Task 3).
- Produces: executable invariant tests referenced by name from the posture memo (Task 9).

Legal driver: ZPPPA čl. 175a/175b.

- [ ] **Step 1: Write the invariant test — void never removes or alters the original sale**

In `src-tauri/src/commands/receipts.rs` `mod tests`, add (adapt helper names to the ones already used in that test module for seeding a completed sale and signing in — search the module for an existing `void_` test and reuse its setup verbatim):

```rust
    #[test]
    fn void_is_append_only_and_preserves_original_monetary_row() {
        // Ledger append-only invariant (ZPPPA čl. 175b): a void adds a
        // counter-document and never deletes or monetarily mutates the original.
        with_receipts_fixture(|state, db, sale_id| {
            let before_rows = count_sales(db);
            let before = load_sale_money(db, sale_id);

            void_receipt(
                db,
                VoidReceiptRequest { receipt_id: sale_id, reason: "Greška".into() },
                acting_user(state),
            )
            .expect("void should succeed");

            let after_rows = count_sales(db);
            let after = load_sale_money(db, sale_id);

            assert!(after_rows > before_rows, "void must ADD a counter-document");
            assert_eq!(before, after, "original sale monetary columns must be unchanged");
        });
    }
```

If the test module lacks `with_receipts_fixture`/`count_sales`/`load_sale_money`/`acting_user` helpers, add small local helpers at the top of the `mod tests` block:

```rust
    fn count_sales(db: &Db) -> i64 {
        db.open()
            .expect("db open")
            .query_row("SELECT COUNT(*) FROM sales", [], |r| r.get(0))
            .expect("count sales")
    }

    fn load_sale_money(db: &Db, sale_id: i64) -> (i64, i64, i64, i64) {
        db.open()
            .expect("db open")
            .query_row(
                "SELECT subtotal_minor, discount_minor, tax_minor, total_minor FROM sales WHERE id = ?1",
                params![sale_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("load sale money")
    }
```

For the fixture and `acting_user`, reuse the exact seeding + sign-in pattern from the nearest existing void/return test in the same module (copy it inline into the new test rather than inventing a new abstraction if that is simpler).

- [ ] **Step 2: Run it to verify it passes (invariant already holds)**

Run: `cargo test --manifest-path src-tauri/Cargo.toml void_is_append_only -- --test-threads=1`
Expected: PASS. (This is a characterization test — it documents an existing guarantee. If it FAILS, the ledger is mutable — STOP and report; do not edit money logic to force a pass.)

- [ ] **Step 3: Write the invariant test — return is append-only**

Add an analogous test `return_is_append_only_and_preserves_original` that calls `return_items` for a partial return and asserts `after_rows > before_rows` and the original sale's four monetary columns are unchanged. Reuse the existing return test's setup for the item ids and quantities.

- [ ] **Step 4: Run it**

Run: `cargo test --manifest-path src-tauri/Cargo.toml return_is_append_only -- --test-threads=1`
Expected: PASS.

- [ ] **Step 5: Write the invariant test — reset is gated + always backs up + always tombstones**

In `src-tauri/src/commands/backup.rs` `mod tests`, add:

```rust
    #[test]
    fn reset_requires_backup_and_tombstone_together() {
        with_state("reset_requires_backup_and_tombstone", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-reset-invariant");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");
            seed_trading_data(state);

            let backups_before = count(state, "backup_jobs");
            reset_trading_data(state, "OBRISI PODATKE").expect("reset should succeed");

            assert!(
                count(state, "backup_jobs") > backups_before,
                "reset must take a safety backup first"
            );
            assert_eq!(count(state, "compliance_log"), 1, "reset must leave a tombstone");
        });
    }
```

- [ ] **Step 6: Run it**

Run: `cargo test --manifest-path src-tauri/Cargo.toml reset_requires_backup_and_tombstone -- --test-threads=1`
Expected: PASS.

- [ ] **Step 7: Full gates + commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src-tauri/src/commands/receipts.rs src-tauri/src/commands/backup.rs
git commit -m "test(compliance): anti-evasion ledger invariants (SW-4)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: SW-5 — ESIR receipt-number command, exposure, and register nudge

**Files:**
- Modify: `src-tauri/src/commands/receipts.rs` (`ReceiptDetail` struct + `ReceiptHeader` + header SELECT(s) + new `set_esir_number`/`receipts_set_esir_number`)
- Modify: `src-tauri/src/lib.rs` (register `receipts_set_esir_number`)
- Modify: `src/services/types.ts` (`ReceiptDetail.esirReceiptNumber`), `src/services/ports.ts` (`ReceiptsService.setEsirNumber`), `src/services/local-adapter.ts`, `src/services/mock-adapter.ts`
- Modify: `src/app/register/RegisterScreen.tsx` (post-sale nudge + input), `src/app/ReceiptsScreen.tsx` (display)
- Test: `src-tauri/src/commands/receipts.rs`, `src/app/register/RegisterScreen.test.tsx`

**Interfaces:**
- Consumes: `sales.esir_receipt_number` (Task 2); `require_session`; `sales.completeSale` returning `CompletedSale { id }`.
- Produces: command `receipts_set_esir_number(receipt_id: i64, esir_receipt_number: String) -> ReceiptDetail`; frontend `receipts.setEsirNumber(receiptId: number, esirReceiptNumber: string): Promise<ReceiptDetail>`; `ReceiptDetail.esirReceiptNumber?: string | null`.

Legal driver: ZF čl. 4 st. 2 posture; ZZP26 čl. 63 st. 5; ZEF čl. 3.

- [ ] **Step 1: Write the failing backend test**

In `src-tauri/src/commands/receipts.rs` `mod tests`, add (reuse the existing completed-sale fixture in the module):

```rust
    #[test]
    fn set_esir_number_stores_and_clears_on_a_sale() {
        with_receipts_fixture(|state, db, sale_id| {
            let _ = acting_user(state); // ensure a session exists
            let detail = set_esir_number(db, sale_id, "  ФБ123-1  ".to_string(), acting_user(state))
                .expect("set should succeed");
            assert_eq!(detail.esir_receipt_number.as_deref(), Some("ФБ123-1"));

            let cleared = set_esir_number(db, sale_id, "   ".to_string(), acting_user(state))
                .expect("clear should succeed");
            assert_eq!(cleared.esir_receipt_number, None);
        });
    }

    #[test]
    fn set_esir_number_rejects_non_sale_document() {
        with_receipts_fixture(|state, db, sale_id| {
            // Void the sale, then try to stamp the VOID document (not a 'sale').
            void_receipt(db, VoidReceiptRequest { receipt_id: sale_id, reason: "x".into() }, acting_user(state))
                .expect("void");
            let void_id: i64 = db.open().unwrap()
                .query_row(
                    "SELECT id FROM sales WHERE original_sale_id = ?1 AND document_type = 'void'",
                    params![sale_id],
                    |r| r.get(0),
                )
                .expect("void doc id");
            let err = set_esir_number(db, void_id, "X".into(), acting_user(state))
                .expect_err("must reject non-sale");
            assert_eq!(err.code(), "validation_error");
        });
    }
```

Use the same fixture/`acting_user` helpers as Task 4 (copy the seeding pattern from the nearest existing test if no shared fixture exists).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml set_esir_number -- --test-threads=1`
Expected: FAIL — `set_esir_number` undefined; `ReceiptDetail.esir_receipt_number` missing.

- [ ] **Step 3: Add the field to the structs and header load**

In `src-tauri/src/commands/receipts.rs`:
- Add to `ReceiptDetail` (after `pub return_reason: Option<String>,`): `pub esir_receipt_number: Option<String>,`
- Add to `ReceiptHeader` (after `return_reason: Option<String>,`): `esir_receipt_number: Option<String>,`
- In every SELECT that populates a `ReceiptHeader` (grep the file for `s.document_type,` inside a `SELECT` used by `load_receipt_detail` / the header loader — there are two header queries), add `s.esir_receipt_number,` and map it in the corresponding `row.get(N)?` sequence. Add the field to the `ReceiptDetail { ... }` construction in `load_receipt_detail` (`esir_receipt_number: header.esir_receipt_number,`).

- [ ] **Step 4: Implement `set_esir_number` + the command wrapper**

Add near `void_receipt`:

```rust
pub fn set_esir_number(
    db: &Db,
    receipt_id: i64,
    esir_receipt_number: String,
    _acting_user_id: i64,
) -> Result<ReceiptDetail, AppError> {
    let connection = db.open()?;

    let document_type: Option<String> = connection
        .query_row(
            "SELECT document_type FROM sales WHERE id = ?1",
            params![receipt_id],
            |row| row.get(0),
        )
        .optional()?;

    let document_type = document_type.ok_or_else(|| AppError::not_found("Račun nije pronađen."))?;
    if document_type != "sale" {
        return Err(AppError::validation(
            "Broj fiskalnog računa se može upisati samo na prodajni dokument.",
            serde_json::json!({ "field": "esirReceiptNumber" }),
        ));
    }

    let trimmed = esir_receipt_number.trim();
    let value: Option<&str> = if trimmed.is_empty() { None } else { Some(trimmed) };

    connection.execute(
        "UPDATE sales SET esir_receipt_number = ?1, updated_at = datetime('now') WHERE id = ?2",
        params![value, receipt_id],
    )?;

    load_receipt_detail(&connection, receipt_id)
}

#[tauri::command]
pub fn receipts_set_esir_number(
    state: State<'_, AppState>,
    receipt_id: i64,
    esir_receipt_number: String,
) -> Result<ReceiptDetail, CommandError> {
    let acting_user_id = super::auth::require_session(state.inner())?;
    set_esir_number(state.db(), receipt_id, esir_receipt_number, acting_user_id).map_err(Into::into)
}
```

Register it in `src-tauri/src/lib.rs` `generate_handler!` (after `commands::receipts::receipts_return_items,`): `commands::receipts::receipts_set_esir_number,`.

- [ ] **Step 5: Run backend tests to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml set_esir_number -- --test-threads=1`
Expected: PASS.

- [ ] **Step 6: Wire the frontend contract**

- `src/services/types.ts`: in `ReceiptDetail`, add `esirReceiptNumber?: string | null;`.
- `src/services/ports.ts`: in `ReceiptsService`, add `setEsirNumber(receiptId: number, esirReceiptNumber: string): Promise<ReceiptDetail>;`.
- `src/services/local-adapter.ts`: in the `receipts` object, add:
  ```ts
      setEsirNumber: (receiptId, esirReceiptNumber) =>
        invoke<ReceiptDetail>("receipts_set_esir_number", { receiptId, esirReceiptNumber }),
  ```
- `src/services/mock-adapter.ts`: in the `receipts` mock, add an async `setEsirNumber` that returns a `ReceiptDetail`-shaped object with `esirReceiptNumber` set (mirror the shape the other receipts mocks return; if they throw "not implemented", implement a minimal stored-value stub so the register test can drive it).

- [ ] **Step 7: Write the failing register test (nudge + save)**

In `src/app/register/RegisterScreen.test.tsx`, after completing a sale, assert the nudge renders and entering a number calls the service:

```tsx
it("nudges ESIR issuance and saves the entered fiscal number", async () => {
  const setEsirNumber = vi.fn().mockResolvedValue({});
  renderRegister({ receipts: { ...baseReceipts, setEsirNumber } });
  await completeASale();
  expect(screen.getByText(/Izdajte fiskalni račun na ESIR-u/i)).toBeInTheDocument();
  await userEvent.type(screen.getByLabelText(/Broj fiskalnog računa/i), "ФБ12-1");
  await userEvent.click(screen.getByRole("button", { name: /Sačuvaj broj/i }));
  expect(setEsirNumber).toHaveBeenCalledWith(expect.any(Number), "ФБ12-1");
});
```

Adapt the render/service-injection pattern to how this test file already provides mock services.

- [ ] **Step 8: Run to verify it fails**

Run: `bun run test -- RegisterScreen`
Expected: FAIL — nudge/input absent.

- [ ] **Step 9: Implement the register nudge + input**

In the post-sale dialog body (below the totals, above the bottom `NonFiscalBanner`), add a nudge and optional input bound to local state (`const [esirNumber, setEsirNumberValue] = useState("")`, reset when `completedSale` changes). On save, call `services.receipts.setEsirNumber(completedSale.id, esirNumber)`. Keep it non-blocking — no validation gate on dialog close:

```tsx
                <div className="rounded-md bg-muted p-3 text-sm">
                  <p className="font-medium">Izdajte fiskalni račun na ESIR-u</p>
                  <p className="text-muted-foreground">
                    Ovaj interni račun ne zamenjuje fiskalni račun.
                  </p>
                  <div className="mt-2 flex items-end gap-2">
                    <div className="flex flex-col gap-1">
                      <label htmlFor="esir-number" className="text-xs">
                        Broj fiskalnog računa (ESIR)
                      </label>
                      <Input
                        id="esir-number"
                        value={esirNumber}
                        onChange={(e) => setEsirNumberValue(e.target.value)}
                      />
                    </div>
                    <Button
                      type="button"
                      variant="secondary"
                      onClick={() => {
                        void services.receipts.setEsirNumber(completedSale.id, esirNumber);
                      }}
                    >
                      Sačuvaj broj
                    </Button>
                  </div>
                </div>
```

Ensure `Input` is imported in the file (it uses shadcn inputs elsewhere; add the import if missing).

- [ ] **Step 10: Display the stored number in the receipts detail**

In `src/app/ReceiptsScreen.tsx`, in the detail header block, render the ESIR number when present (near the badges ~line 631):

```tsx
          {detail.esirReceiptNumber ? (
            <Badge variant="outline">ESIR: {detail.esirReceiptNumber}</Badge>
          ) : null}
```

- [ ] **Step 11: Run frontend + backend suites**

Run: `bun run test -- RegisterScreen ReceiptsScreen && cargo test --manifest-path src-tauri/Cargo.toml receipts -- --test-threads=1`
Expected: PASS.

- [ ] **Step 12: Full gates + commit**

Run: `bun run test && bun run build && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src-tauri/src/commands/receipts.rs src-tauri/src/lib.rs src/services/types.ts src/services/ports.ts src/services/local-adapter.ts src/services/mock-adapter.ts src/app/register/RegisterScreen.tsx src/app/register/RegisterScreen.test.tsx src/app/ReceiptsScreen.tsx
git commit -m "feat(compliance): ESIR receipt-number capture + issuance nudge (SW-5)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: SW-2 (core) — `backup_crypto` envelope-encryption module

**Files:**
- Modify: `src-tauri/Cargo.toml` (add `chacha20poly1305`, `getrandom`)
- Create: `src-tauri/src/commands/backup_crypto.rs`
- Modify: `src-tauri/src/commands/mod.rs` (declare `pub mod backup_crypto;` — or `mod backup_crypto;` if only used within `commands`)
- Test: `src-tauri/src/commands/backup_crypto.rs` (`mod tests`)

**Interfaces:**
- Consumes: `argon2` (already a dep); `AppError`.
- Produces:
  - `pub struct BackupKeyMaterial { pub salt: Vec<u8>, pub wrap_nonce: Vec<u8>, pub wrapped_data_key: Vec<u8>, pub data_key: [u8; 32] }`
  - `pub fn derive_new_key_material(passphrase: &str) -> Result<BackupKeyMaterial, AppError>` — random data key + argon2id-wrapped copy.
  - `pub fn unwrap_data_key(passphrase: &str, salt: &[u8], wrap_nonce: &[u8], wrapped_data_key: &[u8]) -> Result<[u8; 32], AppError>`
  - `pub fn encrypt_snapshot(plaintext: &[u8], km: &BackupKeyMaterial) -> Result<Vec<u8>, AppError>` — produces `VPBK1` file bytes.
  - `pub fn is_encrypted(file_bytes: &[u8]) -> bool` — checks the `VPBK1` magic.
  - `pub fn decrypt_snapshot(file_bytes: &[u8], local_data_key: Option<&[u8; 32]>, passphrase: Option<&str>) -> Result<Vec<u8>, AppError>`
  - Consumed by Task 7.

Legal driver: ZZPL čl. 50 st. 2 tač. 1; čl. 53 st. 3 tač. 1.

- [ ] **Step 1: Add crates**

In `src-tauri/Cargo.toml` `[dependencies]`, add:

```toml
chacha20poly1305 = "0.10"
getrandom = "0.2"
```

- [ ] **Step 2: Write the failing tests**

Create `src-tauri/src/commands/backup_crypto.rs` with a test module first (implementation stubs added next step). Tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_with_local_key() {
        let km = derive_new_key_material("tajna-lozinka").expect("key material");
        let plaintext = b"SQLite format 3\0 ... pretend db bytes".to_vec();
        let file = encrypt_snapshot(&plaintext, &km).expect("encrypt");
        assert!(is_encrypted(&file));
        let out = decrypt_snapshot(&file, Some(&km.data_key), None).expect("decrypt");
        assert_eq!(out, plaintext);
    }

    #[test]
    fn round_trip_with_passphrase_on_fresh_machine() {
        let km = derive_new_key_material("tajna-lozinka").expect("key material");
        let plaintext = b"db-bytes".to_vec();
        let file = encrypt_snapshot(&plaintext, &km).expect("encrypt");
        // No local key (fresh machine): decrypt via passphrase only.
        let out = decrypt_snapshot(&file, None, Some("tajna-lozinka")).expect("decrypt");
        assert_eq!(out, plaintext);
    }

    #[test]
    fn wrong_passphrase_is_rejected() {
        let km = derive_new_key_material("tajna-lozinka").expect("key material");
        let file = encrypt_snapshot(b"db-bytes", &km).expect("encrypt");
        let err = decrypt_snapshot(&file, None, Some("pogrešna")).expect_err("must reject");
        assert_eq!(err.code(), "validation_error");
    }

    #[test]
    fn plaintext_is_not_detected_as_encrypted() {
        assert!(!is_encrypted(b"SQLite format 3\0"));
    }
}
```

- [ ] **Step 3: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml backup_crypto -- --test-threads=1`
Expected: FAIL — module functions undefined.

- [ ] **Step 4: Implement the module**

Write the implementation above the test module in `src-tauri/src/commands/backup_crypto.rs`:

```rust
//! Envelope encryption for backup files (SW-2, ZZPL čl. 50 kriptozaštita).
//!
//! A random 32-byte *data key* encrypts the SQLite snapshot with
//! XChaCha20-Poly1305. The data key is itself wrapped with a key derived from
//! the admin passphrase via argon2id. Both the wrapped key and the argon2
//! salt travel inside every backup file, so a fresh machine + the passphrase
//! can always recover — while the live machine keeps the plain data key so
//! scheduled backups encrypt unattended.

use argon2::Argon2;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{Key, KeyInit, XChaCha20Poly1305, XNonce};

use crate::app_error::AppError;

const MAGIC: &[u8; 5] = b"VPBK1";
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const WRAPPED_KEY_LEN: usize = 48; // 32-byte key + 16-byte AEAD tag
const HEADER_LEN: usize = MAGIC.len() + SALT_LEN + NONCE_LEN + WRAPPED_KEY_LEN + NONCE_LEN;

pub struct BackupKeyMaterial {
    pub salt: Vec<u8>,
    pub wrap_nonce: Vec<u8>,
    pub wrapped_data_key: Vec<u8>,
    pub data_key: [u8; 32],
}

fn random_bytes(len: usize) -> Result<Vec<u8>, AppError> {
    let mut buf = vec![0u8; len];
    getrandom::getrandom(&mut buf)
        .map_err(|error| AppError::InvalidState(format!("Nasumični podaci nisu dostupni: {error}")))?;
    Ok(buf)
}

fn wrapping_key(passphrase: &str, salt: &[u8]) -> Result<[u8; 32], AppError> {
    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|error| AppError::InvalidState(format!("Izvođenje ključa nije uspelo: {error}")))?;
    Ok(key)
}

fn cipher(key: &[u8; 32]) -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new(Key::from_slice(key))
}

pub fn derive_new_key_material(passphrase: &str) -> Result<BackupKeyMaterial, AppError> {
    let salt = random_bytes(SALT_LEN)?;
    let wrap_nonce = random_bytes(NONCE_LEN)?;
    let mut data_key = [0u8; 32];
    data_key.copy_from_slice(&random_bytes(32)?);

    let wrapping = wrapping_key(passphrase, &salt)?;
    let wrapped_data_key = cipher(&wrapping)
        .encrypt(XNonce::from_slice(&wrap_nonce), data_key.as_ref())
        .map_err(|_| AppError::InvalidState("Uvijanje ključa nije uspelo.".to_string()))?;

    Ok(BackupKeyMaterial { salt, wrap_nonce, wrapped_data_key, data_key })
}

pub fn unwrap_data_key(
    passphrase: &str,
    salt: &[u8],
    wrap_nonce: &[u8],
    wrapped_data_key: &[u8],
) -> Result<[u8; 32], AppError> {
    let wrapping = wrapping_key(passphrase, salt)?;
    let key_bytes = cipher(&wrapping)
        .decrypt(XNonce::from_slice(wrap_nonce), wrapped_data_key)
        .map_err(|_| {
            AppError::validation(
                "Lozinka za šifrovanje nije ispravna.",
                serde_json::json!({ "field": "passphrase" }),
            )
        })?;
    let mut data_key = [0u8; 32];
    if key_bytes.len() != 32 {
        return Err(AppError::InvalidState("Ključ šifrovanja je oštećen.".to_string()));
    }
    data_key.copy_from_slice(&key_bytes);
    Ok(data_key)
}

pub fn is_encrypted(file_bytes: &[u8]) -> bool {
    file_bytes.len() >= MAGIC.len() && &file_bytes[..MAGIC.len()] == MAGIC
}

pub fn encrypt_snapshot(plaintext: &[u8], km: &BackupKeyMaterial) -> Result<Vec<u8>, AppError> {
    let data_nonce = random_bytes(NONCE_LEN)?;
    let ciphertext = cipher(&km.data_key)
        .encrypt(XNonce::from_slice(&data_nonce), plaintext)
        .map_err(|_| AppError::InvalidState("Šifrovanje rezervne kopije nije uspelo.".to_string()))?;

    if km.salt.len() != SALT_LEN
        || km.wrap_nonce.len() != NONCE_LEN
        || km.wrapped_data_key.len() != WRAPPED_KEY_LEN
    {
        return Err(AppError::InvalidState("Materijal ključa je neispravan.".to_string()));
    }

    let mut out = Vec::with_capacity(HEADER_LEN + ciphertext.len());
    out.extend_from_slice(MAGIC);
    out.extend_from_slice(&km.salt);
    out.extend_from_slice(&km.wrap_nonce);
    out.extend_from_slice(&km.wrapped_data_key);
    out.extend_from_slice(&data_nonce);
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

pub fn decrypt_snapshot(
    file_bytes: &[u8],
    local_data_key: Option<&[u8; 32]>,
    passphrase: Option<&str>,
) -> Result<Vec<u8>, AppError> {
    if !is_encrypted(file_bytes) || file_bytes.len() < HEADER_LEN {
        return Err(AppError::InvalidState("Fajl nije šifrovana rezervna kopija.".to_string()));
    }

    let mut offset = MAGIC.len();
    let salt = &file_bytes[offset..offset + SALT_LEN];
    offset += SALT_LEN;
    let wrap_nonce = &file_bytes[offset..offset + NONCE_LEN];
    offset += NONCE_LEN;
    let wrapped_data_key = &file_bytes[offset..offset + WRAPPED_KEY_LEN];
    offset += WRAPPED_KEY_LEN;
    let data_nonce = &file_bytes[offset..offset + NONCE_LEN];
    offset += NONCE_LEN;
    let ciphertext = &file_bytes[offset..];

    // Prefer the local plain data key (same-machine restore); fall back to the
    // passphrase (fresh-machine disaster recovery).
    if let Some(key) = local_data_key {
        if let Ok(plaintext) = cipher(key).decrypt(XNonce::from_slice(data_nonce), ciphertext) {
            return Ok(plaintext);
        }
    }

    let passphrase = passphrase.ok_or_else(|| {
        AppError::validation(
            "Potrebna je lozinka za dešifrovanje rezervne kopije.",
            serde_json::json!({ "field": "passphrase" }),
        )
    })?;

    let data_key = unwrap_data_key(passphrase, salt, wrap_nonce, wrapped_data_key)?;
    cipher(&data_key)
        .decrypt(XNonce::from_slice(data_nonce), ciphertext)
        .map_err(|_| {
            AppError::validation(
                "Lozinka za šifrovanje nije ispravna.",
                serde_json::json!({ "field": "passphrase" }),
            )
        })
}
```

Declare the module: in `src-tauri/src/commands/mod.rs` add `pub mod backup_crypto;` (grep the file for the existing `pub mod backup;` line and add alongside).

- [ ] **Step 5: Run the crypto tests to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml backup_crypto -- --test-threads=1`
Expected: PASS (all four).

- [ ] **Step 6: Full gates + commit**

Run: `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS. (Also run `cargo build` once so the new crates compile into the binary target.)

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/commands/backup_crypto.rs src-tauri/src/commands/mod.rs
git commit -m "feat(compliance): backup_crypto envelope-encryption core (SW-2)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: SW-2 (wiring) — encrypt on backup, decrypt on restore, passphrase command, status flag

**Files:**
- Modify: `src-tauri/src/commands/backup.rs` (`perform_backup`, `restore_backup`, `BackupStatus`, `load_backup_status`, new `backup_set_passphrase`/`set_backup_passphrase`, new settings key + struct)
- Modify: `src-tauri/src/commands/settings.rs` (add `pub(crate) const BACKUP_ENCRYPTION_KEY: &str = "backup_encryption";`)
- Modify: `src-tauri/src/lib.rs` (register `backup_set_passphrase`)
- Test: `src-tauri/src/commands/backup.rs` (`mod tests`)

**Interfaces:**
- Consumes: `backup_crypto` (Task 6); `load_json_setting`/`save_json_setting`; `require_admin`.
- Produces: `pub fn set_backup_passphrase(state: &AppState, passphrase: &str) -> Result<(), AppError>`; `#[tauri::command] backup_set_passphrase`; `BackupStatus.encryption_configured: bool`; encrypted `.vpbk` files when a passphrase is set. Consumed by Task 8.

- [ ] **Step 1: Write the failing test (configured passphrase → encrypted .vpbk that round-trips via restore)**

In `src-tauri/src/commands/backup.rs` `mod tests`, add:

```rust
    #[test]
    fn backup_is_encrypted_when_passphrase_set_and_restores_locally() {
        with_state("backup_encrypted_round_trip", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-encrypted-backup");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");

            set_backup_passphrase(state, "tajna-lozinka-123").expect("passphrase should set");

            let job = create_backup(
                state,
                CreateBackupRequest {
                    backup_folder: Some(folder.display().to_string()),
                    backup_type: None,
                },
            )
            .expect("encrypted backup should succeed");

            assert!(job.path.ends_with(".vpbk"), "encrypted backup must use .vpbk: {}", job.path);
            let head = std::fs::read(&job.path).expect("read backup");
            assert_eq!(&head[..5], b"VPBK1");

            // Restore the encrypted file on the same machine (local data key path).
            restore_backup(
                state,
                RestoreBackupRequest {
                    path: job.path.clone(),
                    confirmation_text: "VRATI PODATKE".to_string(),
                    passphrase: None,
                },
            )
            .expect("local restore of encrypted backup should succeed");

            let status = load_backup_status(state).expect("status");
            assert!(status.encryption_configured);
        });
    }

    #[test]
    fn legacy_plaintext_backup_still_restores() {
        with_state("legacy_plaintext_restore", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-legacy-restore");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");

            // No passphrase set → plaintext .sqlite3 (current behavior).
            let job = create_backup(
                state,
                CreateBackupRequest { backup_folder: Some(folder.display().to_string()), backup_type: None },
            )
            .expect("plaintext backup");
            assert!(job.path.ends_with(".sqlite3"));

            restore_backup(
                state,
                RestoreBackupRequest {
                    path: job.path.clone(),
                    confirmation_text: "VRATI PODATKE".to_string(),
                    passphrase: None,
                },
            )
            .expect("legacy restore should succeed");
        });
    }
```

Note: these reference `RestoreBackupRequest { passphrase }` and `set_backup_passphrase` + `status.encryption_configured`, all added below. This also updates the existing restore tests — every existing `RestoreBackupRequest { ... }` literal in the module must gain `passphrase: None,` (see Step 5).

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path src-tauri/Cargo.toml backup_is_encrypted -- --test-threads=1`
Expected: FAIL — compile errors (`set_backup_passphrase`, `passphrase`, `encryption_configured` missing).

- [ ] **Step 3: Add the settings key + encryption-material struct + passphrase command**

In `src-tauri/src/commands/settings.rs`, add near the other keys: `pub(crate) const BACKUP_ENCRYPTION_KEY: &str = "backup_encryption";`.

In `src-tauri/src/commands/backup.rs`:
- Import: `use crate::commands::settings::{load_json_setting, save_json_setting, BACKUP_ENCRYPTION_KEY, BACKUP_SETTINGS_KEY};` (extend the existing import).
- Add near the top (after `use` lines): reference the crypto module `use crate::commands::backup_crypto;`.
- Add a serde struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupEncryption {
    salt: Vec<u8>,
    wrap_nonce: Vec<u8>,
    wrapped_data_key: Vec<u8>,
    data_key: Vec<u8>,
}

fn load_backup_encryption(state: &AppState) -> Result<Option<BackupEncryption>, AppError> {
    load_json_setting::<Option<BackupEncryption>>(state, BACKUP_ENCRYPTION_KEY, None)
}

pub fn set_backup_passphrase(state: &AppState, passphrase: &str) -> Result<(), AppError> {
    super::auth::require_admin(state)?;
    if passphrase.trim().len() < 8 {
        return Err(AppError::validation(
            "Lozinka za šifrovanje mora imati najmanje 8 znakova.",
            serde_json::json!({ "field": "passphrase" }),
        ));
    }
    let km = backup_crypto::derive_new_key_material(passphrase)?;
    let stored = BackupEncryption {
        salt: km.salt,
        wrap_nonce: km.wrap_nonce,
        wrapped_data_key: km.wrapped_data_key,
        data_key: km.data_key.to_vec(),
    };
    save_json_setting(state, BACKUP_ENCRYPTION_KEY, &stored)
}

#[tauri::command]
pub fn backup_set_passphrase(
    state: State<'_, AppState>,
    passphrase: String,
) -> Result<(), CommandError> {
    set_backup_passphrase(state.inner(), &passphrase).map_err(Into::into)
}
```

Register `commands::backup::backup_set_passphrase,` in `src-tauri/src/lib.rs` `generate_handler!` (after `commands::backup::backup_list_jobs`).

- [ ] **Step 4: Encrypt in `perform_backup` when configured**

Rewrite the copy section of `perform_backup`. Replace:

```rust
    let source = state.db().open()?;
    source
        .backup(DatabaseName::Main, &backup_path, None)
        .map_err(|source| {
            let message = format!("Backup nije uspeo: {source}");
            let _ = insert_backup_job(state, backup_type, &backup_path, "failed", Some(&message), None);
            AppError::BackupFailed(message)
        })?;

    let file_size_bytes = checked_file_size(&backup_path)?;
```

with:

```rust
    let encryption = load_backup_encryption(state)?;

    // Always snapshot to a temp plaintext file first via SQLite's online backup.
    let temp_path = backup_path.with_extension("sqlite3.tmp");
    let source = state.db().open()?;
    source
        .backup(DatabaseName::Main, &temp_path, None)
        .map_err(|source| {
            let message = format!("Backup nije uspeo: {source}");
            let _ = insert_backup_job(state, backup_type, &backup_path, "failed", Some(&message), None);
            AppError::BackupFailed(message)
        })?;

    let final_path = if let Some(enc) = encryption {
        let plaintext = std::fs::read(&temp_path)?;
        let km = backup_crypto::BackupKeyMaterial {
            salt: enc.salt,
            wrap_nonce: enc.wrap_nonce,
            wrapped_data_key: enc.wrapped_data_key,
            data_key: to_key_array(&enc.data_key)?,
        };
        let ciphertext = backup_crypto::encrypt_snapshot(&plaintext, &km)?;
        let encrypted_path = backup_path.with_extension("vpbk");
        std::fs::write(&encrypted_path, &ciphertext)?;
        let _ = std::fs::remove_file(&temp_path);
        encrypted_path
    } else {
        std::fs::rename(&temp_path, &backup_path)?;
        backup_path
    };

    let backup_path = final_path;
    let file_size_bytes = checked_file_size(&backup_path)?;
```

Add a helper near `checked_file_size`:

```rust
fn to_key_array(bytes: &[u8]) -> Result<[u8; 32], AppError> {
    <[u8; 32]>::try_from(bytes)
        .map_err(|_| AppError::InvalidState("Ključ šifrovanja je oštećen.".to_string()))
}
```

(The rest of `perform_backup` — `settings.backup_folder = backup_folder; save_json_setting(...); insert_backup_job(...)` — is unchanged and now records the `final_path`.)

- [ ] **Step 5: Add `passphrase` to `RestoreBackupRequest` and decrypt on restore**

Add the field:

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBackupRequest {
    pub path: String,
    pub confirmation_text: String,
    #[serde(default)]
    pub passphrase: Option<String>,
}
```

Update every existing `RestoreBackupRequest { ... }` literal in `mod tests` to add `passphrase: None,` (there are several: `restore_rejects_missing_file...`, `restore_requires_confirmation_text`, `restore_backup_rejected_for_cashier`).

In `restore_backup`, replace the restore block. After the pre-restore snapshot and before `state.db().migrate()?;`, detect encryption:

```rust
    let file_bytes = std::fs::read(&source_path)?;
    if backup_crypto::is_encrypted(&file_bytes) {
        let local_key = load_backup_encryption(state)?
            .map(|enc| to_key_array(&enc.data_key))
            .transpose()?;
        let plaintext = backup_crypto::decrypt_snapshot(
            &file_bytes,
            local_key.as_ref(),
            request.passphrase.as_deref(),
        )?;
        let temp_plain = source_path.with_extension("restore.tmp");
        std::fs::write(&temp_plain, &plaintext)?;
        {
            let mut conn = state.db().open()?;
            conn.restore(
                DatabaseName::Main,
                &temp_plain,
                Option::<fn(rusqlite::backup::Progress)>::None,
            )
            .map_err(|source| AppError::BackupFailed(format!("Restore nije uspeo: {source}")))?;
        }
        let _ = std::fs::remove_file(&temp_plain);
    } else {
        let mut conn = state.db().open()?;
        conn.restore(
            DatabaseName::Main,
            &source_path,
            Option::<fn(rusqlite::backup::Progress)>::None,
        )
        .map_err(|source| AppError::BackupFailed(format!("Restore nije uspeo: {source}")))?;
    }
```

(Remove the old single restore block that this replaces.)

- [ ] **Step 6: Add `encryption_configured` to `BackupStatus`**

Add the field to the struct and populate it in `load_backup_status`:

```rust
pub struct BackupStatus {
    pub backup_folder: String,
    pub automatic_backup_enabled: bool,
    pub stale: bool,
    pub encryption_configured: bool,
    pub last_successful_backup: Option<BackupJob>,
    pub last_failed_backup: Option<BackupJob>,
}
```

In `load_backup_status`, before the `Ok(BackupStatus { ... })`:

```rust
    let encryption_configured = load_backup_encryption(state)?.is_some();
```

and add `encryption_configured,` to the returned struct.

- [ ] **Step 7: Run the backup tests to verify pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::backup -- --test-threads=1`
Expected: PASS (new encryption tests + all existing backup tests with the `passphrase: None` additions).

- [ ] **Step 8: Full gates + commit**

Run: `bun run test && bun run build && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS. (`bun run` gates should be unaffected — the frontend contract changes in Task 8.)

```bash
git add src-tauri/src/commands/backup.rs src-tauri/src/commands/settings.rs src-tauri/src/lib.rs
git commit -m "feat(compliance): encrypt backups + decrypt on restore + passphrase command (SW-2)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 8: SW-2 (frontend) — passphrase controls, encryption warning, restore passphrase input

**Files:**
- Modify: `src/services/types.ts` (`BackupStatus.encryptionConfigured`; `RestoreBackupRequest.passphrase?`)
- Modify: `src/services/ports.ts` (`BackupService.setBackupPassphrase`)
- Modify: `src/services/local-adapter.ts`, `src/services/mock-adapter.ts`
- Modify: `src/app/settings/SettingsScreen.tsx` (backup panel: warning banner, passphrase controls, restore passphrase input)
- Test: `src/app/settings/SettingsScreen.test.tsx`

**Interfaces:**
- Consumes: `backup_set_passphrase` command + `encryption_configured` flag (Task 7).
- Produces: `BackupService.setBackupPassphrase(passphrase: string): Promise<void>`; UI surfacing.

- [ ] **Step 1: Write the failing test (unencrypted warning + set passphrase)**

In `src/app/settings/SettingsScreen.test.tsx`, on the backup tab with a status where `encryptionConfigured: false`, assert the warning shows; then set a passphrase:

```tsx
it("warns when backups are unencrypted and lets an admin set a passphrase", async () => {
  const setBackupPassphrase = vi.fn().mockResolvedValue(undefined);
  // ...render settings on the backup tab with backupStatus.encryptionConfigured === false
  //    and services.backup.setBackupPassphrase = setBackupPassphrase...
  expect(
    await screen.findByText(/Rezervne kopije nisu šifrovane/i),
  ).toBeInTheDocument();
  await userEvent.type(screen.getByLabelText(/Lozinka za šifrovanje/i), "tajna-lozinka");
  await userEvent.click(screen.getByRole("button", { name: /Postavi lozinku/i }));
  expect(setBackupPassphrase).toHaveBeenCalledWith("tajna-lozinka");
});
```

Adapt to the file's existing settings-render helper and its mock `backupStatus` fixture (add `encryptionConfigured: false` to that fixture).

- [ ] **Step 2: Run to verify it fails**

Run: `bun run test -- SettingsScreen`
Expected: FAIL — warning/controls absent; type errors on the new fields.

- [ ] **Step 3: Extend the frontend contract**

- `src/services/types.ts`: `BackupStatus` → add `encryptionConfigured: boolean;`. `RestoreBackupRequest` → add `passphrase?: string | null;`.
- `src/services/ports.ts`: `BackupService` → add `setBackupPassphrase(passphrase: string): Promise<void>;`.
- `src/services/local-adapter.ts`: in `backup`, add:
  ```ts
      setBackupPassphrase: (passphrase) =>
        invoke<void>("backup_set_passphrase", { passphrase }),
  ```
- `src/services/mock-adapter.ts`: add `async setBackupPassphrase() {}` to the backup mock and add `encryptionConfigured: false` to any mock `BackupStatus` literal it returns.

- [ ] **Step 4: Implement the backup-panel warning + passphrase controls**

In `src/app/settings/SettingsScreen.tsx` backup panel (the `BackupPanel`-style component around line 783+), when `!status.encryptionConfigured` render a prominent warning; always render a passphrase set/change control with the loud lost-passphrase note. Add local state `const [passphrase, setPassphrase] = useState("")` and call a new `onSetPassphrase` prop wired at the parent to `services.backup.setBackupPassphrase`:

```tsx
        {!status.encryptionConfigured ? (
          <div role="alert" className="rounded-md border-2 border-destructive bg-destructive/10 p-3 text-sm text-destructive">
            Rezervne kopije nisu šifrovane — postavite lozinku za šifrovanje.
          </div>
        ) : null}
        <div className="flex flex-col gap-2">
          <label htmlFor="backup-passphrase" className="text-sm font-medium">
            Lozinka za šifrovanje
          </label>
          <Input
            id="backup-passphrase"
            type="password"
            value={passphrase}
            onChange={(event) => setPassphrase(event.target.value)}
          />
          <p className="text-xs text-muted-foreground">
            Ako izgubite lozinku, šifrovane rezervne kopije su NEPOVRATNO
            nečitljive na drugom računaru.
          </p>
          <Button type="button" onClick={() => void onSetPassphrase(passphrase)}>
            Postavi lozinku
          </Button>
        </div>
```

Wire `onSetPassphrase` in the parent (near the other backup handlers ~line 266): 

```tsx
          onSetPassphrase={async (passphrase) => {
            await services.backup.setBackupPassphrase(passphrase);
            const backupStatus = await services.backup.getBackupStatus();
            setState((current) =>
              current.status === "ready" ? { ...current, backupStatus } : current,
            );
          }}
```

Add `onSetPassphrase: (passphrase: string) => Promise<void>;` to the panel's props type.

- [ ] **Step 5: Add the restore passphrase input**

In the restore dialog (the one calling `services.backup.restoreBackup`), add an optional passphrase input and thread it into the request. If the panel already builds a `restoreBackup({ path, confirmationText })` call, extend it to `{ path, confirmationText, passphrase: restorePassphrase || null }` with local state `const [restorePassphrase, setRestorePassphrase] = useState("")` and a field labelled `Lozinka (za šifrovane kopije)`. Keep it optional — empty means the local-key path.

- [ ] **Step 6: Run the settings suite to verify pass**

Run: `bun run test -- SettingsScreen`
Expected: PASS.

- [ ] **Step 7: Full gates + commit**

Run: `bun run test && bun run build && cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1 && cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings && cargo fmt --manifest-path src-tauri/Cargo.toml --check`
Expected: PASS.

```bash
git add src/services/types.ts src/services/ports.ts src/services/local-adapter.ts src/services/mock-adapter.ts src/app/settings/SettingsScreen.tsx src/app/settings/SettingsScreen.test.tsx
git commit -m "feat(compliance): backup passphrase UI + unencrypted warning + restore passphrase (SW-2)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 9: Legal document drafts + anti-evasion posture memo

**Files:**
- Create: `docs/compliance/pitanje-purs.md`
- Create: `docs/compliance/ugovor-o-obradi-nacrt.md`
- Create: `docs/compliance/obavestenje-zaposlenima.md`
- Create: `docs/compliance/evidencija-obrade-cl47.md`
- Create: `docs/compliance/runbook-povreda-podataka.md`
- Create: `docs/compliance/checklist-onboarding-pilota.md`
- Create: `docs/compliance/memo-uskladjenost-fiskalizacije.md`

**Interfaces:**
- Consumes: content from `docs/SERBIAN-LAW-COMPLIANCE.md` (§4 F-1/F-3/F-4/F-5/F-8/F-11; §1a for the memo; §6.1 for the PURS question).
- Produces: seven lawyer-ready Serbian drafts. No code.

Every file starts with this header line verbatim:

```
> NACRT — pre upotrebe obavezna advokatska revizija. Stanje prava: 16.07.2026.
```

- [ ] **Step 1: `pitanje-purs.md`** — a short context paragraph (who Actaer is, what VantumPOS does — non-fiscal back-office), then the verbatim question from `docs/SERBIAN-LAW-COMPLIANCE.md` §6.1 (the `"Da li softver koji nije odobreni element EFU…"` text), then the submission channel (`budiefiskalizovan@gov.rs` / pisano obraćanje PURS). Cite ZF čl. 2 st. 1 tač. 5, čl. 6, and TU 7.4.2 tač. 10 (P17).

- [ ] **Step 2: `ugovor-o-obradi-nacrt.md`** — DPA skeleton per ZZPL čl. 45: st. 3 identifiers (predmet, trajanje, priroda i svrha = "daljinska tehnička podrška i održavanje VantumPOS-a", vrsta podataka = employee identity + credential metadata + shift/sales attribution, vrsta lica = zaposleni prodavnice); the eight st. 4 tač. 1–8 processor duties as headed clauses; st. 5 unlawful-instruction warning; sub-processor annex (st. 2/7 authorization + flow-down + full liability); breach clause fixing čl. 52 st. 3 at ≤24h; audit clause (ties to the future audit log); st. 8 controller-conversion note. A clearly-marked bracketed `[ZA PRAVNIKA: AnyDesk = prenos u drugu državu / pod-obrađivač? — potvrditi]` flag.

- [ ] **Step 3: `obavestenje-zaposlenima.md`** — čl. 23 employee privacy notice: controller identity/contacts; purposes + legal basis (čl. 12 st. 1 tač. 2–3, contract + legal obligation, NOT consent); recipients incl. Actaer during support; retention criteria; rights (access/rectification/erasure/restriction/objection/portability/Poverenik complaint); whether provision is obligatory + consequences; no automated decision-making.

- [ ] **Step 4: `evidencija-obrade-cl47.md`** — two prefilled records: a controller record (7 items, for the shop) and Actaer's processor record (4 items). Explicit note: `Kontakt za zaštitu podataka — NE imenovati DPO (dobrovoljno imenovanje stvara obaveze objave i prijave).`

- [ ] **Step 5: `runbook-povreda-podataka.md`** — one page: Actaer→shop ≤24h; shop→Poverenik ≤72h on the Pravilnik 40/2019 form to `povredapodataka@poverenik.rs`; internal breach documentation duty (čl. 52 st. 6–7); direct notice to data subjects only on high risk (čl. 53); note the encrypted-backup safe-harbor (čl. 53 st. 3 tač. 1) now applies once SW-2 is on.

- [ ] **Step 6: `checklist-onboarding-pilota.md`** — F-8 checklist: verify the shop's ESIR in the Registar (naziv/verzija/IB/nema rešenja o ukidanju); record legal form (preduzetnik/d.o.o.) + PDV status; "ESIR first, VantumPOS second" training incl. provokativna kupovina risk; korporacijska kartica / ZEF čl. 3 briefing; documented refusal of any "preskoči ESIR" request; **set the backup passphrase (SW-2) and record that it is stored safely off-machine.**

- [ ] **Step 7: `memo-uskladjenost-fiskalizacije.md`** — SW-4 posture memo: VantumPOS's non-fiscal purpose; append-only ledger design; counter-document model for voids/returns; logged, admin-gated, backup-first reset; the standing rebuttal to any ZPPPA čl. 175b "služi za izbegavanje evidentiranja prometa" characterization. Reference the invariant tests by name: `void_is_append_only_and_preserves_original_monetary_row`, `return_is_append_only_and_preserves_original`, `reset_requires_backup_and_tombstone_together`. Record the design rule: when backup rotation/pruning is ever built, closed-year backups are excluded from pruning.

- [ ] **Step 8: Commit** (docs only — no test/build gates apply, but run `git diff --check` for whitespace)

Run: `git diff --check`
Expected: no output.

```bash
git add docs/compliance/
git commit -m "docs(compliance): lawyer-ready Serbian legal drafts + anti-evasion posture memo (F-1/F-3/F-4/F-5/F-8/F-11, SW-4)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Self-Review Notes

- **Spec coverage:** SW-1 → Task 1; SW-2 → Tasks 6/7/8; SW-3 → Tasks 2/3; SW-4 → Tasks 4 + memo in 9; SW-5 → Tasks 2/5; templates F-1/F-3/F-4/F-5/F-8/F-11 + memo → Task 9. Migration v8 (shared by SW-3 + SW-5) is isolated in Task 2 so both dependents build on a single reviewed schema change.
- **Type consistency:** Rust `esir_receipt_number: Option<String>` ↔ TS `esirReceiptNumber?: string | null` (serde camelCase); `encryption_configured` ↔ `encryptionConfigured`; `RestoreBackupRequest.passphrase: Option<String>` ↔ `passphrase?: string | null`. Command names: `receipts_set_esir_number`, `backup_set_passphrase`.
- **Ordering/dependencies:** 1 (independent) → 2 (schema) → 3, 4, 5 (depend on 2/3) → 6 (independent crypto) → 7 (depends on 6) → 8 (depends on 7) → 9 (docs; references Task 4 test names). Tasks 1, 6, 9 have no cross-dependencies and could run early.
- **Non-goals honored:** no live-DB encryption, no backup rotation, no ESIR integration, no P1 modules.
