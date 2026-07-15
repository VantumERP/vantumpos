# Trustworthy Shifts & Anti-Theft — Design

**Date:** 2026-07-15
**Status:** Approved design, ready for implementation plan
**Scope:** T1 gaps 1.2 (shift attribution), 1.3 (cash in/out), 1.4 (authorization on mutations), 1.7 (backup that runs) from `docs/COMPETITIVE-GAP-ANALYSIS.md`.

## Locked decisions

| Decision | Choice |
|---|---|
| Scope | **All three subsystems now**: auth/attribution, cash in/out, backup scheduler. |
| Admin gate | **Back-office only** — admin for price/catalog mutations, imports, and stock **correct + write-off**. Sales, returns, voids, and stock **receiving** stay open to any signed-in cashier. |
| Acting user | Always derived from the **session**, never the client payload. |
| Backup trigger | **Both** — on-launch-if-stale + a periodic timer while running; plus record **failed** backups. |

## Current reality (verified)
- A session exists: `AppState.session_user_id` (`Mutex<Option<i64>>`) with `session_user_id()`/`set_session_user_id()`. `require_admin(state)` (auth.rs:156) reads it, clears a stale session, and errors `unauthorized`/`forbidden`. `current_user_id` (shifts.rs:191) is session-derived.
- `require_admin` is applied in settings/backup/users/reports but **zero** times in catalog/inventory/sales/receipts/imports/shifts.
- Acting user comes from the **client**: `VoidReceiptRequest.user_id` (receipts.rs:119), `ReturnItemsRequest.user_id` (:134), `InventoryAdjustmentRequest.user_id: Option<i64>` (inventory.rs:64).
- Shift UI: `OpenShiftPanel`/`CloseShiftPanel` in `AppShell.tsx` (~640-820). `close_shift_for_user` requires `shift.user_id == acting`.
- Backup: `create_backup` (require_admin) → online `.backup()` → `insert_backup_job(..., "completed", ...)`. A failed `.backup()` returns `Err` and writes **no** job row, so the "last failed backup" UI is unreachable. `is_backup_stale` uses a 24h threshold. `automatic_backup_enabled` is persisted but never read.

## Part A — Auth & attribution (no migration)

### `require_session`
Add `pub(crate) fn require_session(state: &AppState) -> Result<i64, AppError>` in auth.rs: read `session_user_id()`; `None` → `unauthorized`; load the user; missing/inactive → clear session + `unauthorized`; return the id. Refactor `require_admin` to call it then check role (keeps one source of truth for stale-session handling).

### Acting user from the session
Drop the client `user_id` field from `VoidReceiptRequest`, `ReturnItemsRequest` (receipts.rs) and `InventoryAdjustmentRequest` (inventory.rs). The internal fns gain an `acting_user_id: i64` parameter; the `#[tauri::command]` wrappers resolve it via `require_session(state.inner())` and pass it in. The void/return counter-document `cashier_id` and the `inventory_movements.user_id` then record the **session** user. Update the TS request types to drop `userId`, and update `local-adapter`/`mock`/tests.

### `require_admin` on back-office mutations
Add `super::auth::require_admin(state.inner())?;` at the top of these command wrappers:
- catalog: `catalog_create_product`, `catalog_update_product`, `catalog_set_product_active`, `catalog_save_category`.
- imports: `import_validate`, `import_commit`.
- inventory: `inventory_correct`, `inventory_write_off`.

Left open to any signed-in cashier: `inventory_receive`, all sales, returns, voids, catalog reads, import reads.

### Admin force-close of a stale shift
New `#[tauri::command] shift_admin_close(state, request: { shiftId, countedCashMinor })` → `require_admin`, then `admin_close_shift(state, shift_id, counted)` closing **any** open shift (mirror `close_shift_for_user` without the `user_id == acting` predicate; error `not_found` if the shift is not open). Register in lib.rs; add a small admin-only "Zatvori tuđu smenu" control in the shift panel + service wiring.

## Part B — Cash in/out (migration v7 + UI)

### Schema (migration v7)
```sql
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
```
(Bumps the migration count to 7 — update the `migrate_is_idempotent…` count assertion.)

### Command
`#[tauri::command] shift_cash_movement(state, request: { direction: "pay_in"|"pay_out", amountMinor, reason })` → `require_session`, resolve the acting user's current open shift (error `shift_required` if none), validate `amount_minor > 0` and a valid direction, insert a `cash_movements` row with the session `user_id`.

### Reconciliation
`load_shift_summary` folds cash movements in: `expected_cash = opening_cash + cash_sales + Σ(pay_in) − Σ(pay_out)` for an open shift (and the stored value at close uses the same). `ShiftSummary` gains `paid_in_minor` / `paid_out_minor`; the TS type + the shift panel display them. Cash-in/out controls (amount + reason) live in the current-shift panel.

## Part C — Backup that runs (backend + scheduler)

### Factor + failure recording
Extract `perform_backup(state, folder: Option<&str>, backup_type: &str) -> Result<BackupJob, AppError>` from `create_backup` **without** the auth check. On a failed `.backup()`, `insert_backup_job(state, backup_type, path, "failed", Some(error_message), None)` **before** propagating the error, so the failure surfaces in the UI. `create_backup` becomes: `require_admin` → `perform_backup(...)`.

### Triggers (both)
- **On launch:** in `lib.rs` `setup`, after the state is managed, if `automatic_backup_enabled` and `is_backup_stale`, run `perform_backup` once — best-effort (ignore the `Err`; it is already recorded as a failed job) and non-blocking (must never prevent the app opening).
- **Periodic:** `tauri::async_runtime::spawn` a task holding a clone of the state that, every `AUTO_BACKUP_INTERVAL` (const, e.g. 6h), checks `automatic_backup_enabled` and runs `perform_backup`, recording success/failure. Best-effort.

The default folder stays configurable (`app_data/backups`); we cannot pick a user's external drive. The existing staleness + last-failed-backup UI now becomes meaningful because failures are recorded.

## Non-goals (explicit)
- Per-action manager/PIN prompt at the till (deferred until a per-sale role primitive exists).
- Cash-movement types beyond `pay_in`/`pay_out` (safe-drop/pickup fold into these + a reason).
- Enforcing an off-volume backup folder (kept configurable; we only record failures + surface staleness).
- A full audit log / exception report (separate T2 item; needs price history).
- Concurrent multiple open shifts per user (the unique index already prevents this).

## Test plan
**Rust:**
1. `require_session` returns the id for a signed-in user; `unauthorized` with no session; clears a stale session.
2. Each newly-gated command (catalog create/update/set-active/save-category, import validate/commit, inventory correct/write-off) → `forbidden` for a cashier, works for an admin.
3. Void/return/inventory record the **session** user in the counter-document/movement (the request no longer carries a user id).
4. `shift_admin_close` closes another user's open shift (admin); rejects a cashier.
5. `shift_cash_movement`: a `pay_in` raises and a `pay_out` lowers `expected_cash`; rejects with no open shift and with `amount_minor <= 0`.
6. `perform_backup` records a `'failed'` job when the copy fails; `create_backup` still records `'completed'` and rejects a cashier.
7. Migration v7 creates `cash_movements`; the migration-count assertion is updated to 7.

**Frontend:**
8. Request types no longer send `userId`; adapters map `shift_cash_movement` / `shift_admin_close`.
9. Cash-in/out controls call `shiftCashMovement` and the shift panel shows paid-in/out; the admin force-close control calls through.

## Acceptance criteria
- No mutating command trusts a client-supplied acting user; voids/returns/stock adjustments are attributed to the signed-in user.
- A cashier cannot change prices, run an import, or write off/correct stock; an admin can, and can force-close a forgotten shift.
- Cash paid into or out of the drawer during a shift is recorded and reflected in `expected_cash`.
- Automatic backups actually run (on launch when stale + periodically), and failures are recorded and surfaced.
- All gates pass: `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy … -D warnings`, `cargo fmt --check`, `git diff --check`.
