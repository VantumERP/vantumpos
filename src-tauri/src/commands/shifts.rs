use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::clock::utc_now;
use crate::state::AppState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShiftSummary {
    pub id: i64,
    pub user_id: i64,
    pub cashier_name: String,
    pub opened_at: String,
    pub closed_at: Option<String>,
    pub opening_cash_minor: i64,
    pub expected_cash_minor: i64,
    pub counted_cash_minor: Option<i64>,
    pub cash_sales_minor: i64,
    pub card_sales_minor: i64,
    pub paid_in_minor: i64,
    pub paid_out_minor: i64,
    pub difference_minor: Option<i64>,
    pub status: String,
    pub opening_note: Option<String>,
    pub closing_note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenShiftRequest {
    pub opening_cash_minor: i64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloseShiftRequest {
    pub shift_id: i64,
    pub counted_cash_minor: i64,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CashMovementRequest {
    /// One of [`CASH_MOVEMENT_DIRECTIONS`].
    pub direction: String,
    pub amount_minor: i64,
    pub reason: Option<String>,
    /// Broj izvoda / uplatnice for the two bank directions. Optional — a shop
    /// that records the polog before the bank confirms it must not be blocked.
    #[serde(default)]
    pub bank_reference: Option<String>,
}

/// The four movement types `cash_movements.movement_type` accepts (migration
/// v15). `bank_withdrawal` is a podizanje sa računa: money leaves the bank and
/// enters the drawer, so it counts like a pay_in. `bank_deposit` is a polog:
/// money leaves the drawer, so it counts like a pay_out.
const CASH_MOVEMENT_DIRECTIONS: [&str; 4] =
    ["pay_in", "pay_out", "bank_deposit", "bank_withdrawal"];

#[tauri::command]
pub fn shift_get_current(state: State<'_, AppState>) -> Result<Option<ShiftSummary>, CommandError> {
    let Some(user_id) = state
        .inner()
        .session_user_id()
        .map_err(CommandError::from)?
    else {
        return Ok(None);
    };

    current_shift_for_user(state.inner(), user_id)
}

#[tauri::command]
pub fn shift_open(
    state: State<'_, AppState>,
    request: OpenShiftRequest,
) -> Result<ShiftSummary, CommandError> {
    let user_id = current_user_id(state.inner())?;
    open_shift_for_user(state.inner(), user_id, request)
}

#[tauri::command]
pub fn shift_close(
    state: State<'_, AppState>,
    request: CloseShiftRequest,
) -> Result<ShiftSummary, CommandError> {
    let user_id = current_user_id(state.inner())?;
    close_shift_for_user(state.inner(), user_id, request)
}

#[tauri::command]
pub fn shift_admin_close(
    state: State<'_, AppState>,
    request: CloseShiftRequest,
) -> Result<ShiftSummary, CommandError> {
    super::auth::require_admin(state.inner()).map_err(CommandError::from)?;
    admin_close_shift(state.inner(), request)
}

#[tauri::command]
pub fn shift_cash_movement(
    state: State<'_, AppState>,
    request: CashMovementRequest,
) -> Result<ShiftSummary, CommandError> {
    let user_id = super::auth::require_session(state.inner()).map_err(CommandError::from)?;
    record_cash_movement(state.inner(), user_id, request)
}

pub fn record_cash_movement(
    state: &AppState,
    user_id: i64,
    request: CashMovementRequest,
) -> Result<ShiftSummary, CommandError> {
    if !CASH_MOVEMENT_DIRECTIONS.contains(&request.direction.as_str()) {
        return Err(CommandError::new(
            "validation_error",
            "Nepoznat tip transakcije.",
        ));
    }
    if request.amount_minor <= 0 {
        return Err(CommandError::new(
            "validation_error",
            "Iznos mora biti veći od nule.",
        ));
    }
    let summary = current_shift_for_user(state, user_id)?
        .ok_or_else(|| CommandError::new("shift_required", "Smena nije otvorena."))?;
    let now = utc_now().map_err(CommandError::from)?;
    let note = normalized_note(request.reason);
    let bank_reference = normalized_note(request.bank_reference);
    state
        .db()
        .open()
        .map_err(CommandError::from)?
        .execute(
            "INSERT INTO cash_movements (shift_id, movement_type, amount_minor, reason, bank_reference, user_id, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                summary.id,
                request.direction,
                request.amount_minor,
                note,
                bank_reference,
                user_id,
                now
            ],
        )
        .map_err(AppError::from)
        .map_err(CommandError::from)?;
    current_shift_for_user(state, user_id)?
        .ok_or_else(|| CommandError::new("shift_required", "Smena nije otvorena."))
}

pub fn admin_close_shift(
    state: &AppState,
    request: CloseShiftRequest,
) -> Result<ShiftSummary, CommandError> {
    let summary = shift_by_id(state, request.shift_id)?
        .ok_or_else(|| CommandError::new("not_found", "Smena nije pronađena."))?;
    if summary.status != "open" {
        return Err(CommandError::new(
            "not_found",
            "Otvorena smena nije pronađena.",
        ));
    }
    let now = utc_now().map_err(CommandError::from)?;
    let note = normalized_note(request.note);
    let conn = state.db().open().map_err(CommandError::from)?;
    let changed = conn
        .execute(
            "UPDATE shifts SET closed_at = ?1, expected_cash_minor = ?2, counted_cash_minor = ?3,
             status = 'closed', closing_note = ?4, updated_at = ?1
         WHERE id = ?5 AND status = 'open'",
            params![
                now,
                summary.expected_cash_minor,
                request.counted_cash_minor,
                note,
                request.shift_id
            ],
        )
        .map_err(AppError::from)
        .map_err(CommandError::from)?;

    if changed == 0 {
        return Err(CommandError::new(
            "not_found",
            "Otvorena smena nije pronađena.",
        ));
    }

    shift_by_id(state, request.shift_id)?
        .ok_or_else(|| CommandError::new("not_found", "Smena nije pronađena."))
}

pub fn current_shift_for_user(
    state: &AppState,
    user_id: i64,
) -> Result<Option<ShiftSummary>, CommandError> {
    load_shift_summary(
        state,
        "WHERE sh.user_id = ?1 AND sh.status = 'open'",
        params![user_id],
    )
    .map_err(Into::into)
}

pub fn open_shift_for_user(
    state: &AppState,
    user_id: i64,
    request: OpenShiftRequest,
) -> Result<ShiftSummary, CommandError> {
    if request.opening_cash_minor < 0 {
        return Err(CommandError::new(
            "validation_error",
            "Početni novac ne može biti negativan.",
        ));
    }

    if current_shift_for_user(state, user_id)?.is_some() {
        return Err(CommandError::new(
            "validation_error",
            "Korisnik već ima otvorenu smenu.",
        ));
    }

    ensure_active_user(state, user_id)?;

    let now = utc_now().map_err(CommandError::from)?;
    let note = normalized_note(request.note);
    let conn = state.db().open().map_err(CommandError::from)?;
    conn.execute(
        "INSERT INTO shifts (
            user_id,
            opened_at,
            opening_cash_minor,
            expected_cash_minor,
            status,
            opening_note,
            created_at,
            updated_at
         )
         VALUES (?1, ?2, ?3, ?3, 'open', ?4, ?2, ?2)",
        params![user_id, now, request.opening_cash_minor, note],
    )
    .map_err(AppError::from)
    .map_err(CommandError::from)?;
    let shift_id = conn.last_insert_rowid();

    shift_by_id(state, shift_id)?
        .ok_or_else(|| CommandError::new("not_found", "Smena nije pronađena posle otvaranja."))
}

pub fn close_shift_for_user(
    state: &AppState,
    user_id: i64,
    request: CloseShiftRequest,
) -> Result<ShiftSummary, CommandError> {
    if request.counted_cash_minor < 0 {
        return Err(CommandError::new(
            "validation_error",
            "Prebrojana gotovina ne može biti negativna.",
        ));
    }

    let shift = shift_by_id(state, request.shift_id)?
        .ok_or_else(|| CommandError::new("not_found", "Smena nije pronađena."))?;

    if shift.user_id != user_id || shift.status != "open" {
        return Err(CommandError::new(
            "not_found",
            "Otvorena smena nije pronađena.",
        ));
    }

    let expected_cash_minor =
        shift.opening_cash_minor + shift.cash_sales_minor + shift.paid_in_minor
            - shift.paid_out_minor;
    let now = utc_now().map_err(CommandError::from)?;
    let note = normalized_note(request.note);
    let conn = state.db().open().map_err(CommandError::from)?;
    let changed = conn
        .execute(
            "UPDATE shifts
             SET closed_at = ?1,
                 expected_cash_minor = ?2,
                 counted_cash_minor = ?3,
                 status = 'closed',
                 closing_note = ?4,
                 updated_at = ?1
             WHERE id = ?5 AND user_id = ?6 AND status = 'open'",
            params![
                now,
                expected_cash_minor,
                request.counted_cash_minor,
                note,
                request.shift_id,
                user_id
            ],
        )
        .map_err(AppError::from)
        .map_err(CommandError::from)?;

    if changed == 0 {
        return Err(CommandError::new(
            "not_found",
            "Otvorena smena nije pronađena.",
        ));
    }

    shift_by_id(state, request.shift_id)?
        .ok_or_else(|| CommandError::new("not_found", "Smena nije pronađena."))
}

fn current_user_id(state: &AppState) -> Result<i64, CommandError> {
    state
        .session_user_id()
        .map_err(CommandError::from)?
        .ok_or_else(|| CommandError::new("unauthorized", "Prijavite se za rad."))
}

fn ensure_active_user(state: &AppState, user_id: i64) -> Result<(), CommandError> {
    let conn = state.db().open().map_err(CommandError::from)?;
    let active: Option<i64> = conn
        .query_row(
            "SELECT active FROM users WHERE id = ?1",
            params![user_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(AppError::from)
        .map_err(CommandError::from)?;

    if active == Some(1) {
        return Ok(());
    }

    Err(CommandError::new("unauthorized", "Prijavite se za rad."))
}

fn shift_by_id(state: &AppState, shift_id: i64) -> Result<Option<ShiftSummary>, CommandError> {
    load_shift_summary(state, "WHERE sh.id = ?1", params![shift_id]).map_err(Into::into)
}

fn load_shift_summary<P>(
    state: &AppState,
    predicate: &str,
    params: P,
) -> Result<Option<ShiftSummary>, AppError>
where
    P: rusqlite::Params,
{
    let conn = state.db().open()?;
    let sql = format!(
        r#"
SELECT
    sh.id,
    sh.user_id,
    u.display_name,
    sh.opened_at,
    sh.closed_at,
    sh.opening_cash_minor,
    sh.expected_cash_minor,
    sh.counted_cash_minor,
    sh.status,
    sh.opening_note,
    sh.closing_note,
    COALESCE(SUM(CASE WHEN sp.payment_method = 'cash' THEN sp.amount_minor ELSE 0 END), 0),
    COALESCE(SUM(CASE WHEN sp.payment_method = 'card' THEN sp.amount_minor ELSE 0 END), 0),
    -- A bank_withdrawal (podizanje sa računa) fills the drawer, so it lands in
    -- paid_in; a bank_deposit (polog) empties it, so it lands in paid_out.
    COALESCE((SELECT SUM(amount_minor) FROM cash_movements
              WHERE shift_id = sh.id AND movement_type IN ('pay_in', 'bank_withdrawal')), 0),
    COALESCE((SELECT SUM(amount_minor) FROM cash_movements
              WHERE shift_id = sh.id AND movement_type IN ('pay_out', 'bank_deposit')), 0)
FROM shifts sh
JOIN users u ON u.id = sh.user_id
LEFT JOIN sales s ON s.shift_id = sh.id
LEFT JOIN sale_payments sp ON sp.sale_id = s.id
{predicate}
GROUP BY sh.id
ORDER BY sh.opened_at DESC
LIMIT 1
"#
    );

    conn.query_row(&sql, params, shift_summary_from_row)
        .optional()
        .map_err(Into::into)
}

fn shift_summary_from_row(row: &Row<'_>) -> rusqlite::Result<ShiftSummary> {
    let opening_cash_minor: i64 = row.get(5)?;
    let stored_expected_cash_minor: i64 = row.get(6)?;
    let counted_cash_minor: Option<i64> = row.get(7)?;
    let status: String = row.get(8)?;
    let cash_sales_minor: i64 = row.get(11)?;
    let paid_in_minor: i64 = row.get(13)?;
    let paid_out_minor: i64 = row.get(14)?;
    let expected_cash_minor = if status == "open" {
        opening_cash_minor + cash_sales_minor + paid_in_minor - paid_out_minor
    } else {
        stored_expected_cash_minor
    };
    let difference_minor = counted_cash_minor.map(|counted| counted - expected_cash_minor);

    Ok(ShiftSummary {
        id: row.get(0)?,
        user_id: row.get(1)?,
        cashier_name: row.get(2)?,
        opened_at: row.get(3)?,
        closed_at: row.get(4)?,
        opening_cash_minor,
        expected_cash_minor,
        counted_cash_minor,
        cash_sales_minor,
        card_sales_minor: row.get(12)?,
        paid_in_minor,
        paid_out_minor,
        difference_minor,
        status,
        opening_note: row.get(9)?,
        closing_note: row.get(10)?,
    })
}

fn normalized_note(value: Option<String>) -> Option<String> {
    value.and_then(|note| {
        let trimmed = note.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use tauri::Manager;

    use super::{
        admin_close_shift, close_shift_for_user, current_shift_for_user, open_shift_for_user,
        record_cash_movement, shift_admin_close, CashMovementRequest, CloseShiftRequest,
        OpenShiftRequest,
    };
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

    fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            test(&state);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    fn seed_cashier(state: &AppState) -> i64 {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, active, created_at, updated_at)
                 VALUES ('kasir', 'Kasir', 'cashier', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        connection.last_insert_rowid()
    }

    fn sign_in_admin(state: &AppState) -> i64 {
        let admin_id: i64 = state
            .db()
            .open()
            .expect("database should open")
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .expect("bootstrap admin should exist");
        state
            .set_session_user_id(admin_id)
            .expect("admin session should set");
        admin_id
    }

    fn sign_in_cashier(state: &AppState, cashier_id: i64) {
        state
            .set_session_user_id(cashier_id)
            .expect("cashier session should set");
    }

    fn seed_completed_sale(
        state: &AppState,
        shift_id: i64,
        cashier_id: i64,
        cash_minor: i64,
        card_minor: i64,
    ) {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO sales (
                    local_receipt_number, shift_id, cashier_id, status,
                    subtotal_minor, tax_minor, total_minor, created_at, updated_at
                 )
                 VALUES ('VP-000001', ?1, ?2, 'completed', 20000, 0, 20000,
                         '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![shift_id, cashier_id],
            )
            .expect("sale should insert");
        let sale_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, 'cash', ?2, '2026-01-01T00:00:00Z')",
                params![sale_id, cash_minor],
            )
            .expect("cash payment should insert");
        connection
            .execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, 'card', ?2, '2026-01-01T00:00:00Z')",
                params![sale_id, card_minor],
            )
            .expect("card payment should insert");
    }

    /// Seeds one completed sale settled entirely by `payment_method`.
    fn seed_sale_with_payment(
        state: &AppState,
        shift_id: i64,
        cashier_id: i64,
        payment_method: &str,
        amount_minor: i64,
    ) {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO sales (
                    local_receipt_number, shift_id, cashier_id, status, fiscal_status,
                    subtotal_minor, discount_minor, tax_minor, total_minor,
                    created_at, updated_at
                 )
                 VALUES (?1, ?2, ?3, 'completed', 'not_fiscalized', ?4, 0, 0, ?4,
                         '2026-07-31T09:00:00Z', '2026-07-31T09:00:00Z')",
                params![
                    format!("VP-{payment_method}"),
                    shift_id,
                    cashier_id,
                    amount_minor
                ],
            )
            .expect("sale should insert");
        let sale_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, ?2, ?3, '2026-07-31T09:00:00Z')",
                params![sale_id, payment_method, amount_minor],
            )
            .expect("payment should insert");
    }

    /// The lawful alternative to a capped cash payment (čl. 46 st. 1) settles in
    /// the bank, never in the drawer — so the till still expects only the cash
    /// line, and the closing count is not asked to find 500.000 para that never
    /// arrived.
    #[test]
    fn bank_transfer_tender_is_not_expected_in_the_till() {
        with_state("bank_transfer_not_in_till", |state| {
            let cashier_id = seed_cashier(state);
            let opened = open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 5_000,
                    note: None,
                },
            )
            .expect("shift should open");

            seed_sale_with_payment(state, opened.id, cashier_id, "cash", 30_000);
            seed_sale_with_payment(state, opened.id, cashier_id, "bank_transfer", 500_000);

            let summary = current_shift_for_user(state, cashier_id)
                .expect("summary loads")
                .expect("shift is open");

            assert_eq!(
                summary.expected_cash_minor,
                summary.opening_cash_minor + 30_000,
                "a bank transfer never enters the drawer"
            );
            assert_eq!(summary.cash_sales_minor, 30_000);
            assert_eq!(summary.card_sales_minor, 0);
        });
    }

    #[test]
    fn close_shift_for_user_derives_expected_cash() {
        with_state("close_shift_derives_expected_cash", |state| {
            let cashier_id = seed_cashier(state);

            let opened = open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 5000,
                    note: None,
                },
            )
            .expect("shift should open");
            assert_eq!(opened.status, "open");

            seed_completed_sale(state, opened.id, cashier_id, 12000, 8000);

            let closed = close_shift_for_user(
                state,
                cashier_id,
                CloseShiftRequest {
                    shift_id: opened.id,
                    counted_cash_minor: 17000,
                    note: None,
                },
            )
            .expect("shift should close");

            assert_eq!(closed.status, "closed");
            assert_eq!(closed.cash_sales_minor, 12000);
            assert_eq!(closed.card_sales_minor, 8000);
            assert_eq!(closed.expected_cash_minor, 17000);
            assert_eq!(closed.counted_cash_minor, Some(17000));
            assert_eq!(closed.difference_minor, Some(0));
        });
    }

    #[test]
    fn open_shift_for_user_rejects_second_open_shift() {
        with_state("open_second_shift_fails", |state| {
            let cashier_id = seed_cashier(state);

            open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 0,
                    note: None,
                },
            )
            .expect("first shift should open");

            let error = open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 0,
                    note: None,
                },
            )
            .expect_err("second shift should fail");

            assert_eq!(error.code, "validation_error");
            assert_eq!(error.message, "Korisnik već ima otvorenu smenu.");
        });
    }

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

    #[test]
    fn admin_close_shift_closes_any_open_shift() {
        with_state("admin_close_shift_closes_any_open_shift", |state| {
            let cashier_id = seed_cashier(state);
            let opened = open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 5000,
                    note: None,
                },
            )
            .expect("cashier's shift should open");

            sign_in_admin(state);

            let closed = admin_close_shift(
                state,
                CloseShiftRequest {
                    shift_id: opened.id,
                    counted_cash_minor: 5000,
                    note: None,
                },
            )
            .expect("admin should force-close another user's open shift");

            assert_eq!(closed.id, opened.id);
            assert_eq!(closed.user_id, cashier_id);
            assert_eq!(closed.status, "closed");
            assert_eq!(closed.counted_cash_minor, Some(5000));
        });
    }

    #[test]
    fn admin_close_shift_rejects_already_closed_shift() {
        with_state("admin_close_shift_rejects_already_closed", |state| {
            let cashier_id = seed_cashier(state);
            let opened = open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 5000,
                    note: None,
                },
            )
            .expect("cashier's shift should open");

            close_shift_for_user(
                state,
                cashier_id,
                CloseShiftRequest {
                    shift_id: opened.id,
                    counted_cash_minor: 5000,
                    note: None,
                },
            )
            .expect("cashier's shift should close");

            sign_in_admin(state);

            let error = admin_close_shift(
                state,
                CloseShiftRequest {
                    shift_id: opened.id,
                    counted_cash_minor: 5000,
                    note: None,
                },
            )
            .expect_err("admin should not force-close an already-closed shift");

            assert_eq!(error.code, "not_found");
        });
    }

    #[test]
    fn shift_admin_close_rejected_for_cashier() {
        let path = test_database_path("shift_admin_close_rejected_for_cashier");

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            let cashier_id = seed_cashier(&state);
            let opened = open_shift_for_user(
                &state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 5000,
                    note: None,
                },
            )
            .expect("cashier's shift should open");

            sign_in_cashier(&state, cashier_id);

            let app = tauri::test::mock_builder()
                .manage(state)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");

            let error = shift_admin_close(
                app.state::<AppState>(),
                CloseShiftRequest {
                    shift_id: opened.id,
                    counted_cash_minor: 5000,
                    note: None,
                },
            )
            .expect_err("cashier should not force-close a shift");

            assert_eq!(error.code, "forbidden");
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    #[test]
    fn record_cash_movement_inserts_pay_in_and_pay_out_rows() {
        with_state("record_cash_movement_inserts_rows", |state| {
            let cashier_id = seed_cashier(state);
            let opened = open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 0,
                    note: None,
                },
            )
            .expect("shift should open");

            record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "pay_in".to_string(),
                    amount_minor: 5000,
                    reason: Some("Sitan novac".to_string()),
                    bank_reference: None,
                },
            )
            .expect("pay_in should record");

            record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "pay_out".to_string(),
                    amount_minor: 2000,
                    reason: None,
                    bank_reference: None,
                },
            )
            .expect("pay_out should record");

            let connection = state.db().open().expect("database should open");
            let (count, total_pay_in, total_pay_out): (i64, i64, i64) = connection
                .query_row(
                    "SELECT
                        COUNT(*),
                        COALESCE(SUM(CASE WHEN movement_type = 'pay_in' THEN amount_minor ELSE 0 END), 0),
                        COALESCE(SUM(CASE WHEN movement_type = 'pay_out' THEN amount_minor ELSE 0 END), 0)
                     FROM cash_movements WHERE shift_id = ?1",
                    params![opened.id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .expect("cash movements should query");

            assert_eq!(count, 2);
            assert_eq!(total_pay_in, 5000);
            assert_eq!(total_pay_out, 2000);
        });
    }

    #[test]
    fn record_cash_movement_requires_open_shift() {
        with_state("record_cash_movement_requires_open_shift", |state| {
            let cashier_id = seed_cashier(state);

            let error = record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "pay_in".to_string(),
                    amount_minor: 5000,
                    reason: None,
                    bank_reference: None,
                },
            )
            .expect_err("cash movement without an open shift should fail");

            assert_eq!(error.code, "shift_required");
        });
    }

    #[test]
    fn record_cash_movement_rejects_non_positive_amount() {
        with_state(
            "record_cash_movement_rejects_non_positive_amount",
            |state| {
                let cashier_id = seed_cashier(state);
                open_shift_for_user(
                    state,
                    cashier_id,
                    OpenShiftRequest {
                        opening_cash_minor: 0,
                        note: None,
                    },
                )
                .expect("shift should open");

                let error = record_cash_movement(
                    state,
                    cashier_id,
                    CashMovementRequest {
                        direction: "pay_in".to_string(),
                        amount_minor: 0,
                        reason: None,
                        bank_reference: None,
                    },
                )
                .expect_err("non-positive amount should fail");

                assert_eq!(error.code, "validation_error");
            },
        );
    }

    #[test]
    fn record_cash_movement_rejects_unknown_direction() {
        with_state("record_cash_movement_rejects_unknown_direction", |state| {
            let cashier_id = seed_cashier(state);
            open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 0,
                    note: None,
                },
            )
            .expect("shift should open");

            let error = record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "refund".to_string(),
                    amount_minor: 1000,
                    reason: None,
                    bank_reference: None,
                },
            )
            .expect_err("unknown direction should fail");

            assert_eq!(error.code, "validation_error");
        });
    }

    #[test]
    fn shift_summary_folds_cash_movements_into_expected_cash() {
        with_state("shift_summary_folds_cash_movements", |state| {
            let cashier_id = seed_cashier(state);
            let opened = open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 5000,
                    note: None,
                },
            )
            .expect("shift should open");

            seed_completed_sale(state, opened.id, cashier_id, 1000, 200);

            record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "pay_in".to_string(),
                    amount_minor: 3000,
                    reason: Some("Sitan novac".to_string()),
                    bank_reference: None,
                },
            )
            .expect("pay_in should record");

            record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "pay_out".to_string(),
                    amount_minor: 1000,
                    reason: Some("Isplata dobavljaču".to_string()),
                    bank_reference: None,
                },
            )
            .expect("pay_out should record");

            let summary = current_shift_for_user(state, cashier_id)
                .expect("summary should query")
                .expect("open shift summary should exist");

            assert_eq!(summary.paid_in_minor, 3000);
            assert_eq!(summary.paid_out_minor, 1000);
            assert_eq!(summary.expected_cash_minor, 5000 + 1000 + 3000 - 1000);
        });
    }

    /// A polog taken to the bank mid-shift leaves the drawer; a podizanje sa
    /// računa (kusur) enters it. If the summary ignored either one, a shop that
    /// deposits during the shift would show a phantom manjak at close — a false
    /// positive in the anti-theft check.
    #[test]
    fn bank_movements_change_expected_cash_in_the_right_direction() {
        with_state("expected_cash_bank_movements", |state| {
            let cashier_id = seed_cashier(state);
            open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 100_000,
                    note: None,
                },
            )
            .expect("shift should open");

            record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "bank_withdrawal".to_string(),
                    amount_minor: 20_000,
                    reason: Some("Kusur".to_string()),
                    bank_reference: Some("izvod-7".to_string()),
                },
            )
            .expect("withdrawal should record");

            record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "bank_deposit".to_string(),
                    amount_minor: 50_000,
                    reason: Some("Polog pazara".to_string()),
                    bank_reference: Some("uplatnica-3".to_string()),
                },
            )
            .expect("deposit should record");

            let summary = current_shift_for_user(state, cashier_id)
                .expect("summary should query")
                .expect("open shift summary should exist");

            assert_eq!(
                summary.expected_cash_minor,
                100_000 + 20_000 - 50_000,
                "podizanje sa računa puni kasu, polog je prazni"
            );
        });
    }

    #[test]
    fn close_shift_for_user_includes_cash_movements_in_expected_cash() {
        with_state("close_shift_includes_cash_movements", |state| {
            let cashier_id = seed_cashier(state);
            let opened = open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 5000,
                    note: None,
                },
            )
            .expect("shift should open");

            seed_completed_sale(state, opened.id, cashier_id, 1000, 200);

            record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "pay_in".to_string(),
                    amount_minor: 3000,
                    reason: None,
                    bank_reference: None,
                },
            )
            .expect("pay_in should record");

            record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "pay_out".to_string(),
                    amount_minor: 1000,
                    reason: None,
                    bank_reference: None,
                },
            )
            .expect("pay_out should record");

            let expected_cash_minor = 5000 + 1000 + 3000 - 1000;

            let closed = close_shift_for_user(
                state,
                cashier_id,
                CloseShiftRequest {
                    shift_id: opened.id,
                    counted_cash_minor: expected_cash_minor,
                    note: None,
                },
            )
            .expect("shift should close");

            assert_eq!(closed.expected_cash_minor, expected_cash_minor);
            assert_eq!(closed.paid_in_minor, 3000);
            assert_eq!(closed.paid_out_minor, 1000);
            assert_eq!(closed.difference_minor, Some(0));
        });
    }

    #[test]
    fn admin_close_shift_includes_cash_movements_in_expected_cash() {
        with_state("admin_close_includes_cash_movements", |state| {
            let cashier_id = seed_cashier(state);
            let opened = open_shift_for_user(
                state,
                cashier_id,
                OpenShiftRequest {
                    opening_cash_minor: 5000,
                    note: None,
                },
            )
            .expect("shift should open");

            seed_completed_sale(state, opened.id, cashier_id, 1000, 200);

            record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "pay_in".to_string(),
                    amount_minor: 3000,
                    reason: None,
                    bank_reference: None,
                },
            )
            .expect("pay_in should record");

            record_cash_movement(
                state,
                cashier_id,
                CashMovementRequest {
                    direction: "pay_out".to_string(),
                    amount_minor: 1000,
                    reason: None,
                    bank_reference: None,
                },
            )
            .expect("pay_out should record");

            let expected_cash_minor = 5000 + 1000 + 3000 - 1000;

            sign_in_admin(state);

            let closed = admin_close_shift(
                state,
                CloseShiftRequest {
                    shift_id: opened.id,
                    counted_cash_minor: expected_cash_minor,
                    note: None,
                },
            )
            .expect("admin should force-close with movement-inclusive expected cash");

            assert_eq!(closed.expected_cash_minor, expected_cash_minor);
            assert_eq!(closed.difference_minor, Some(0));
        });
    }
}
