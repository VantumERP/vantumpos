# Trustworthy Shifts & Anti-Theft Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make shifts and money trustworthy — the acting user always comes from the session, sensitive mutations are admin-gated, cash in/out is recorded, and automatic backups actually run.

**Architecture:** Three phases executed in order — (A) auth & attribution across the command layer, (B) a `cash_movements` table + shift reconciliation, (C) a shared `perform_backup` with real triggers. No cross-phase file races; each task ends green.

**Tech Stack:** Tauri v2, Rust, rusqlite/SQLite, React + TS, shadcn/ui, bun.

**Design doc:** `docs/superpowers/specs/2026-07-15-trustworthy-shifts-anti-theft-design.md`

## Global Constraints

- Branch: `feat/trustworthy-shifts` (already checked out).
- Money in integer minor units, quantities in milli-units; never floats. Operator strings in Serbian (with diacritics).
- `AppError` → `CommandError` via `From`; codes: `unauthorized` (no session), `forbidden` (not admin), `shift_required` (no open shift). `AppError::code()` is a `#[cfg(test)]` helper.
- Acting user ALWAYS from `require_session`/`require_admin`, never the client payload.
- All commits end with: `Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>`.
- Gates (per task; full set before the final commit): `bun run test` · `bun run build` · `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` · `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings` · `cargo fmt --manifest-path src-tauri/Cargo.toml --check` · `git diff --check`.
- Order is fixed: A1→A2→A3→A4→A5→B1→B2→B3→C1→C2.

---

### Task A1: `require_session` helper

**Files:** Modify `src-tauri/src/commands/auth.rs` (add `require_session`, refactor `require_admin`); test in the same file's `#[cfg(test)] mod tests`.

**Interfaces:**
- Produces: `pub(crate) fn require_session(state: &AppState) -> Result<i64, AppError>` — returns the signed-in user id; `unauthorized` if no session; clears a stale session. Consumed by A2/A3/A5/B1.

- [ ] **Step 1: Write the failing test** — add to auth.rs tests (reuse the harness the existing `require_admin_*` tests use — `with_state`/sign-in helpers):
```rust
    #[test]
    fn require_session_returns_signed_in_user_id() {
        with_state("require_session_ok", |state| {
            let uid = seed_cashier(state); // existing helper that inserts a cashier + sets session
            assert_eq!(super::require_session(state).expect("session"), uid);
        });
    }

    #[test]
    fn require_session_rejects_without_session() {
        with_state("require_session_none", |state| {
            let error = super::require_session(state).expect_err("no session");
            assert_eq!(error.code(), "unauthorized");
        });
    }
```
(If the auth test module names its helpers differently, use those — mirror the existing `require_admin_returns_signed_in_admin` setup.)

- [ ] **Step 2: Run — expect FAIL** (`require_session` missing): `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::auth::tests::require_session -- --test-threads=1`

- [ ] **Step 3: Implement** — in auth.rs, add and refactor:
```rust
pub(crate) fn require_session(state: &AppState) -> Result<i64, AppError> {
    let user_id = state
        .session_user_id()?
        .ok_or_else(|| AppError::business("unauthorized", "Niste prijavljeni."))?;
    if active_user_by_id(state, user_id)?.is_none() {
        state.clear_session()?;
        return Err(AppError::business("unauthorized", "Niste prijavljeni."));
    }
    Ok(user_id)
}
```
and change `require_admin`'s top to reuse it:
```rust
pub(crate) fn require_admin(state: &AppState) -> Result<UserAccount, AppError> {
    let user_id = require_session(state)?;
    let user = active_user_by_id(state, user_id)?
        .ok_or_else(|| AppError::business("unauthorized", "Niste prijavljeni."))?;
    if user.role != "admin" {
        return Err(AppError::business(
            "forbidden",
            "Samo administrator može da izvrši ovu akciju.",
        ));
    }
    Ok(user)
}
```

- [ ] **Step 4: Run — expect PASS** (new tests + existing `require_admin_*`): `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::auth:: -- --test-threads=1`

- [ ] **Step 5: Commit**
```bash
git add src-tauri/src/commands/auth.rs
git commit -m "feat(auth): require_session helper for session-derived acting user

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task A2: Receipts — acting user from the session

**Files:** Modify `src-tauri/src/commands/receipts.rs` (drop `user_id` from `VoidReceiptRequest`/`ReturnItemsRequest`; add `acting_user_id: i64` param to `void_receipt`/`return_items`; wrappers use `require_session`); update receipts tests; `src/services/types.ts`, `src/services/local-adapter.test.ts` (drop `userId`).

**Interfaces:**
- Consumes: `require_session` (A1).
- Produces: `void_receipt(db, request, acting_user_id)`, `return_items(db, request, acting_user_id)`.

- [ ] **Step 1: Update Rust** — remove `pub user_id: i64` from `VoidReceiptRequest` (receipts.rs:117-121) and `ReturnItemsRequest` (132-139). Add `acting_user_id: i64` as the last param of `void_receipt` and `return_items`; inside each, replace every `request.user_id` with `acting_user_id`. Change the wrappers:
```rust
#[tauri::command]
pub fn receipts_void(
    state: State<'_, AppState>,
    request: VoidReceiptRequest,
) -> Result<ReceiptDetail, CommandError> {
    let acting_user_id = super::auth::require_session(state.inner())?;
    void_receipt(state.db(), request, acting_user_id)
}

#[tauri::command]
pub fn receipts_return_items(
    state: State<'_, AppState>,
    request: ReturnItemsRequest,
) -> Result<ReceiptDetail, CommandError> {
    let acting_user_id = super::auth::require_session(state.inner())?;
    return_items(state.db(), request, acting_user_id)
}
```
Also drop `validate_user_id(request.user_id)?` calls inside `void_receipt`/`return_items` (the id now comes from a validated session).

- [ ] **Step 2: Fix the Rust tests** — every `void_receipt(db, VoidReceiptRequest { receipt_id, user_id: X, reason })` becomes `void_receipt(db, VoidReceiptRequest { receipt_id, reason }, X)`; likewise `return_items(...)` (move the `user_id`/`refund_tender` struct fields, dropping `user_id`, and pass the id as the new arg). Add one attribution test:
```rust
    #[test]
    fn void_attributes_the_session_user_not_the_client() {
        with_receipt_database("void_attributes_session_user", |db, seeded| {
            void_receipt(
                db,
                VoidReceiptRequest { receipt_id: seeded.sale_id, reason: "Greška".to_string() },
                seeded.user_id,
            )
            .expect("void");
            let connection = db.open().expect("db");
            let cashier: i64 = connection
                .query_row(
                    "SELECT cashier_id FROM sales WHERE original_sale_id = ?1 AND document_type = 'void'",
                    params![seeded.sale_id],
                    |row| row.get(0),
                )
                .expect("void doc");
            assert_eq!(cashier, seeded.user_id);
        });
    }
```

- [ ] **Step 3: Update the frontend contract** — in `src/services/types.ts`, drop `userId` from `VoidReceiptRequest` and `ReturnItemsRequest`. In `src/services/local-adapter.test.ts`, drop `userId` from the `receipts_void` / `receipts_return_items` request assertions and the calls. In `src/app/ReceiptsScreen.tsx`, drop `userId` from the `voidReceipt`/`returnItems` argument objects (and remove the now-unused `userId` prop threading if present). In `src/app/register`/callers, none send these. `ReceiptsScreen.test.tsx` — drop `userId` from its `toHaveBeenCalledWith` expectations.

- [ ] **Step 4: Run + commit**
Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::receipts:: -- --test-threads=1` and `bun run test src/app/ReceiptsScreen.test.tsx src/services/local-adapter.test.ts` and `bun run build`. Expected: PASS.
```bash
git add src-tauri/src/commands/receipts.rs src/services/types.ts src/services/local-adapter.test.ts src/app/ReceiptsScreen.tsx src/app/ReceiptsScreen.test.tsx
git commit -m "feat(receipts): attribute voids/returns to the session user, not the client

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task A3: Inventory — session user + admin on correct/write-off

**Files:** Modify `src-tauri/src/commands/inventory.rs` (drop `user_id` from `InventoryAdjustmentRequest`; add `acting_user_id: i64` to `apply_inventory_adjustment`; wrappers resolve session + gate correct/write-off with admin); update inventory tests; `src/services/types.ts` (drop `userId` from the adjustment request).

- [ ] **Step 1: Rust** — remove `pub user_id: Option<i64>` from `InventoryAdjustmentRequest` (inventory.rs:64). Add `acting_user_id: i64` as a param to `apply_inventory_adjustment` (407); change the `StockMovementWrite { … user_id: request.user_id … }` (442) to `user_id: Some(acting_user_id)`. Change the three wrappers to resolve/pass the session user, gating correct + write-off with admin:
```rust
#[tauri::command]
pub fn inventory_receive(state: State<'_, AppState>, request: InventoryAdjustmentRequest) -> Result<InventoryAdjustmentResult, CommandError> {
    let acting_user_id = super::auth::require_session(state.inner())?;
    let mut connection = state.db().open()?;
    let created_at = now_utc_string()?;
    apply_inventory_adjustment(&mut connection, InventoryMovementType::Receive, request, acting_user_id, &created_at).map_err(Into::into)
}

#[tauri::command]
pub fn inventory_correct(state: State<'_, AppState>, request: InventoryAdjustmentRequest) -> Result<InventoryAdjustmentResult, CommandError> {
    super::auth::require_admin(state.inner())?;
    let acting_user_id = super::auth::require_session(state.inner())?;
    let mut connection = state.db().open()?;
    let created_at = now_utc_string()?;
    apply_inventory_adjustment(&mut connection, InventoryMovementType::Correction, request, acting_user_id, &created_at).map_err(Into::into)
}

#[tauri::command]
pub fn inventory_write_off(state: State<'_, AppState>, request: InventoryAdjustmentRequest) -> Result<InventoryAdjustmentResult, CommandError> {
    super::auth::require_admin(state.inner())?;
    let acting_user_id = super::auth::require_session(state.inner())?;
    let mut connection = state.db().open()?;
    let created_at = now_utc_string()?;
    apply_inventory_adjustment(&mut connection, InventoryMovementType::WriteOff, request, acting_user_id, &created_at).map_err(Into::into)
}
```

- [ ] **Step 2: Fix inventory tests** — every `InventoryAdjustmentRequest { … user_id: Some(X) … }` drops the `user_id` field, and every `apply_inventory_adjustment(&mut conn, type, req, &created_at)` becomes `apply_inventory_adjustment(&mut conn, type, req, X, &created_at)` (pass the seeded user id). Where a test drove the command wrapper (`inventory_correct`/`inventory_write_off`) as a cashier, add an assertion that it now returns `forbidden`, and switch the happy-path adjustment tests to `apply_inventory_adjustment` directly (or sign in an admin).

- [ ] **Step 3: Frontend** — in `src/services/types.ts`, drop `userId` from the inventory adjustment request type; in `src/app/inventory/InventoryScreen.tsx`, drop `userId` from the receive/correct/write-off request objects. Update `InventoryScreen.test.tsx` if it asserts `userId`.

- [ ] **Step 4: Run + commit**
Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::inventory:: -- --test-threads=1` and `bun run test src/app/inventory/InventoryScreen.test.tsx` and `bun run build`.
```bash
git add src-tauri/src/commands/inventory.rs src/services/types.ts src/app/inventory/InventoryScreen.tsx src/app/inventory/InventoryScreen.test.tsx
git commit -m "feat(inventory): session-attributed adjustments; admin-gate correct/write-off

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task A4: Admin-gate catalog + imports

**Files:** Modify `src-tauri/src/commands/catalog.rs` (wrappers `catalog_create_product`, `catalog_update_product`, `catalog_set_product_active`, `catalog_save_category`) and `src-tauri/src/commands/imports.rs` (`import_validate`, `import_commit`); tests in both.

- [ ] **Step 1: Write failing tests** — add a cashier-rejected test per gated command using each file's test harness. Pattern (catalog, adapt to imports):
```rust
    #[test]
    fn catalog_create_product_rejected_for_cashier() {
        // build state + sign in a cashier, then call the command wrapper via a mock app
        // (mirror settings.rs `reports_command_rejected_for_cashier` / tax_rate_save_rejected_for_cashier);
        // assert error.code == "forbidden".
    }
```
Mirror the exact mock-app + `sign_in_cashier` setup already used by `settings.rs`'s `tax_rate_save_rejected_for_cashier` test (build `tauri::test::mock_builder().manage(state)`, `app.state::<AppState>()`, call the `#[tauri::command]` fn, assert `error.code == "forbidden"`). Add one such test for `catalog_create_product`, `catalog_update_product`, `catalog_save_category`, and `import_commit`.

- [ ] **Step 2: Run — expect FAIL** (commands not gated yet).

- [ ] **Step 3: Implement** — add `super::auth::require_admin(state.inner())?;` as the first line of each of: `catalog_create_product`, `catalog_update_product`, `catalog_set_product_active`, `catalog_save_category`, `import_validate`, `import_commit`. (Leave `catalog_list_products`, `catalog_search_products`, `catalog_get_product`, `catalog_list_categories`, `catalog_lookup_product_by_barcode`, `import_read_headers`, `import_list_jobs`, `import_get_job` open.)

- [ ] **Step 4: Run + commit** — existing catalog/imports tests that call the *internal* fns (`create_product`, etc.) directly stay green; only tests calling the `#[tauri::command]` wrappers need a signed-in admin. Fix any that now fail by signing in an admin in their setup.
Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::catalog:: commands::imports:: -- --test-threads=1`.
```bash
git add src-tauri/src/commands/catalog.rs src-tauri/src/commands/imports.rs
git commit -m "feat(auth): admin-gate catalog mutations and imports

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task A5: Admin force-close of a stale shift

**Files:** Modify `src-tauri/src/commands/shifts.rs` (add `shift_admin_close` command + `admin_close_shift` internal), `src-tauri/src/lib.rs` (register); `src/services/ports.ts`, `local-adapter.ts`, `mock-adapter.ts`, `local-adapter.test.ts`; a small admin control in `AppShell.tsx`.

- [ ] **Step 1: Failing Rust test** — in shifts.rs tests: seed cashier A with an open shift; sign in admin; `admin_close_shift(state, shiftA_id, counted)` closes it (status 'closed'); a cashier caller gets `forbidden`.

- [ ] **Step 2: Implement** — add:
```rust
#[tauri::command]
pub fn shift_admin_close(
    state: State<'_, AppState>,
    request: CloseShiftRequest,
) -> Result<ShiftSummary, CommandError> {
    super::auth::require_admin(state.inner()).map_err(CommandError::from)?;
    admin_close_shift(state.inner(), request)
}

pub fn admin_close_shift(state: &AppState, request: CloseShiftRequest) -> Result<ShiftSummary, CommandError> {
    let summary = shift_by_id(state, request.shift_id)?
        .ok_or_else(|| CommandError::new("not_found", "Smena nije pronađena."))?;
    if summary.status != "open" {
        return Err(CommandError::new("not_found", "Otvorena smena nije pronađena."));
    }
    let now = utc_now().map_err(CommandError::from)?;
    let note = normalized_note(request.note);
    let conn = state.db().open().map_err(CommandError::from)?;
    conn.execute(
        "UPDATE shifts SET closed_at = ?1, expected_cash_minor = ?2, counted_cash_minor = ?3,
             status = 'closed', closing_note = ?4, updated_at = ?1
         WHERE id = ?5 AND status = 'open'",
        params![now, summary.expected_cash_minor, request.counted_cash_minor, note, request.shift_id],
    ).map_err(AppError::from).map_err(CommandError::from)?;
    shift_by_id(state, request.shift_id)?
        .ok_or_else(|| CommandError::new("not_found", "Smena nije pronađena."))
}
```
Register `commands::shifts::shift_admin_close` in lib.rs.

- [ ] **Step 3: Frontend wiring + control** — `ports.ts` `ShiftService.adminCloseShift(request: CloseShiftRequest): Promise<ShiftSummary>`; `local-adapter.ts` → `invoke("shift_admin_close", { request })`; `mock-adapter.ts` a working stub; `local-adapter.test.ts` maps it. In `AppShell.tsx`, when `session.user.role === "admin"` and a *different* user holds the open shift, show a "Zatvori tuđu smenu" action calling `adminCloseShift`. Keep it minimal.

- [ ] **Step 4: Run + commit** (`cargo test … commands::shifts::`, `bun run test`, `bun run build`).
```bash
git add src-tauri/src/commands/shifts.rs src-tauri/src/lib.rs src/services/ports.ts src/services/local-adapter.ts src/services/mock-adapter.ts src/services/local-adapter.test.ts src/app/AppShell.tsx
git commit -m "feat(shifts): admin force-close of a stale shift

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task B1: cash_movements table + shift_cash_movement command

**Files:** Modify `src-tauri/src/db/migrations.rs` (migration v7), `src-tauri/src/db/mod.rs` (update the migrate-count test 6→7), `src-tauri/src/commands/shifts.rs` (command + internal), `src-tauri/src/lib.rs` (register); tests in shifts.rs.

- [ ] **Step 1: Migration v7** — append to `MIGRATIONS`:
```rust
    Migration {
        version: 7,
        name: "cash_movements",
        sql: r#"
CREATE TABLE cash_movements (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    shift_id INTEGER NOT NULL REFERENCES shifts(id) ON DELETE CASCADE,
    movement_type TEXT NOT NULL CHECK (movement_type IN ('pay_in', 'pay_out')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    reason TEXT,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_cash_movements_shift ON cash_movements(shift_id);
"#,
    },
```
Update the `migrate_is_idempotent…` count assertion in db/mod.rs from `6` to `7`, and add `"cash_movements"` to the `CORE_TABLES` list + `"idx_cash_movements_shift"` to `EXPLICIT_INDEXES` if those lists exist.

- [ ] **Step 2: Failing test** — in shifts.rs tests: open a shift, `shift_cash_movement(pay_in 5000)` and `(pay_out 2000)` succeed and insert rows; a movement with no open shift → `shift_required`; `amount_minor <= 0` → validation error.

- [ ] **Step 3: Implement** — add a request struct + command + internal:
```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashMovementRequest {
    pub direction: String, // "pay_in" | "pay_out"
    pub amount_minor: i64,
    pub reason: Option<String>,
}

#[tauri::command]
pub fn shift_cash_movement(state: State<'_, AppState>, request: CashMovementRequest) -> Result<ShiftSummary, CommandError> {
    let user_id = super::auth::require_session(state.inner()).map_err(CommandError::from)?;
    record_cash_movement(state.inner(), user_id, request)
}

pub fn record_cash_movement(state: &AppState, user_id: i64, request: CashMovementRequest) -> Result<ShiftSummary, CommandError> {
    if request.direction != "pay_in" && request.direction != "pay_out" {
        return Err(CommandError::new("validation_error", "Nepoznat tip transakcije."));
    }
    if request.amount_minor <= 0 {
        return Err(CommandError::new("validation_error", "Iznos mora biti veći od nule."));
    }
    let summary = current_shift_for_user(state, user_id)?
        .ok_or_else(|| CommandError::new("shift_required", "Smena nije otvorena."))?;
    let now = utc_now().map_err(CommandError::from)?;
    let note = normalized_note(request.reason);
    state.db().open().map_err(CommandError::from)?.execute(
        "INSERT INTO cash_movements (shift_id, movement_type, amount_minor, reason, user_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![summary.id, request.direction, request.amount_minor, note, user_id, now],
    ).map_err(AppError::from).map_err(CommandError::from)?;
    current_shift_for_user(state, user_id)?
        .ok_or_else(|| CommandError::new("shift_required", "Smena nije otvorena."))
}
```
Register `commands::shifts::shift_cash_movement` in lib.rs.

- [ ] **Step 4: Run + commit** (`cargo test … commands::shifts:: db::`).
```bash
git add src-tauri/src/db/migrations.rs src-tauri/src/db/mod.rs src-tauri/src/commands/shifts.rs src-tauri/src/lib.rs
git commit -m "feat(shifts): cash_movements table + shift_cash_movement command

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task B2: Fold cash movements into shift reconciliation

**Files:** Modify `src-tauri/src/commands/shifts.rs` (`load_shift_summary` SQL + `ShiftSummary` struct + `shift_summary_from_row` + `close_shift_for_user`/`admin_close_shift` expected-cash); `src/services/types.ts` (`ShiftSummary`); tests.

- [ ] **Step 1: Failing test** — extend `shift_summary_nets_returns_against_cash_sales` (or a new test): after a `pay_in 3000` and `pay_out 1000` on a shift with 1000 cash sales, `expected_cash_minor == opening + 1000 + 3000 - 1000` and `paid_in_minor == 3000`, `paid_out_minor == 1000`.

- [ ] **Step 2: Implement** — add `paid_in_minor: i64` and `paid_out_minor: i64` to `ShiftSummary`. In `load_shift_summary`, add two correlated sums and fold them into the open-shift expected-cash:
  - Add to the SELECT: `COALESCE((SELECT SUM(amount_minor) FROM cash_movements WHERE shift_id = sh.id AND movement_type='pay_in'),0)` and the `pay_out` equivalent.
  - In `shift_summary_from_row`, read them into `paid_in_minor`/`paid_out_minor`; for an open shift compute `expected_cash = opening + cash_sales + paid_in - paid_out`.
  - In `close_shift_for_user` and `admin_close_shift`, change the stored `expected_cash_minor` from `opening + cash_sales` to `opening + cash_sales + paid_in - paid_out` (read from the loaded summary).

- [ ] **Step 3: Frontend** — add `paidInMinor`/`paidOutMinor` to the TS `ShiftSummary`.

- [ ] **Step 4: Run + commit** (`cargo test … commands::shifts::`, `bun run build`).
```bash
git add src-tauri/src/commands/shifts.rs src/services/types.ts
git commit -m "feat(shifts): fold cash in/out into expected-cash reconciliation

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task B3: Cash-in/out UI

**Files:** `src/services/ports.ts`, `local-adapter.ts`, `mock-adapter.ts`, `local-adapter.test.ts`; `src/app/AppShell.tsx` (shift panel controls).

- [ ] **Step 1: Failing adapter test** — `shiftCashMovement({ direction: "pay_out", amountMinor: 5000, reason: "Pazar u banku" })` → `invoke("shift_cash_movement", { request: { … } })`.
- [ ] **Step 2: Wire** — `ports.ts` `ShiftService.shiftCashMovement(request): Promise<ShiftSummary>`; `local-adapter.ts` → `invoke("shift_cash_movement", { request })`; `mock-adapter.ts` returns the current shift with adjusted `expectedCashMinor`.
- [ ] **Step 3: UI** — in the current-shift panel (`CloseShiftPanel` region of `AppShell.tsx`), add "Uplata u kasu" / "Isplata iz kase" controls (amount + reason) calling `shiftCashMovement`, and show `paidInMinor`/`paidOutMinor`. Update `onSessionChange` with the returned summary.
- [ ] **Step 4: Run + commit** (`bun run test`, `bun run build`).
```bash
git add src/services/ports.ts src/services/local-adapter.ts src/services/mock-adapter.ts src/services/local-adapter.test.ts src/app/AppShell.tsx
git commit -m "feat(shifts): cash in/out controls in the shift panel

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task C1: perform_backup + failure recording

**Files:** Modify `src-tauri/src/commands/backup.rs`; tests in the same file.

- [ ] **Step 1: Failing test** — `perform_backup` with a folder that cannot be created/written records a `'failed'` backup_job (query `SELECT status FROM backup_jobs ORDER BY id DESC LIMIT 1` == `'failed'`) and returns `Err`. Use a folder path guaranteed to fail the copy (e.g. point the backup at a path whose parent is a file), or make the source unavailable.

- [ ] **Step 2: Implement** — extract from `create_backup` a no-auth `perform_backup(state, backup_folder: Option<&str>, backup_type: &str) -> Result<BackupJob, AppError>` that resolves the folder, `create_dir_all`, opens the source, and on the `.backup()` `Err` does:
```rust
    .map_err(|source| {
        let message = format!("Backup nije uspeo: {source}");
        let _ = insert_backup_job(state, backup_type, &backup_path, "failed", Some(&message), None);
        AppError::BackupFailed(message)
    })?;
```
then records the `'completed'` job on success. `create_backup` becomes: `super::auth::require_admin(state)?;` then `perform_backup(state, request.backup_folder.as_deref(), &backup_type)`.

- [ ] **Step 3: Run + commit** — existing `create_backup` tests (`manual_backup_creates…`, `create_backup_rejected_for_cashier`) stay green.
Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib commands::backup:: -- --test-threads=1`.
```bash
git add src-tauri/src/commands/backup.rs
git commit -m "feat(backup): perform_backup records failed jobs so failures surface

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task C2: Auto-backup triggers (launch + periodic)

**Files:** Modify `src-tauri/src/commands/backup.rs` (an `auto_backup_if_due` helper + `AUTO_BACKUP_INTERVAL`) and `src-tauri/src/lib.rs` (`setup`).

- [ ] **Step 1: Failing test** — a unit test on the decision helper: with `automatic_backup_enabled=false` → no backup attempted; with enabled + stale → `perform_backup` is invoked and records a job. Implement as `pub fn auto_backup_if_due(state: &AppState) -> Result<(), AppError>` that returns early unless `load_backup_settings(state)?.automatic_backup_enabled && is_backup_stale(state)?`, then calls `perform_backup(state, None, "automatic")` (ignoring the returned job). Test it by enabling auto + no prior backup (stale) and asserting a job row appears; and by disabling and asserting none.

- [ ] **Step 2: Implement + wire** — add `const AUTO_BACKUP_INTERVAL: Duration = Duration::from_secs(6 * 3600);` and `auto_backup_if_due`. In `lib.rs` `setup`, after `app.manage(AppState::new(db))`:
```rust
            let state_for_launch = app.state::<AppState>().inner().clone();
            let _ = commands::backup::auto_backup_if_due(&state_for_launch);
            let state_for_timer = state_for_launch.clone();
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(commands::backup::AUTO_BACKUP_INTERVAL).await;
                    let _ = commands::backup::auto_backup_if_due(&state_for_timer);
                }
            });
```
(Confirm `AppState` is `Clone`; it wraps `Arc`s, so derive/have `Clone`. Expose `AUTO_BACKUP_INTERVAL` as `pub`. The launch call is best-effort — a failure is already recorded as a failed job and must never block startup.)

- [ ] **Step 3: Full verification** — every gate:
```bash
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml --check
git diff --check
```

- [ ] **Step 4: Commit**
```bash
git add src-tauri/src/commands/backup.rs src-tauri/src/lib.rs
git commit -m "feat(backup): run automatic backups on launch and on a periodic timer

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

## Verification summary

After C2: no mutating command trusts a client-supplied acting user; a cashier cannot change prices, import, or write off/correct stock; an admin can force-close a forgotten shift; cash in/out is recorded and reflected in `expected_cash`; and automatic backups run on launch + periodically with failures recorded. Deliberately deferred (spec Non-goals): per-action PIN, richer cash types, off-volume backup enforcement, audit log/exception report.
