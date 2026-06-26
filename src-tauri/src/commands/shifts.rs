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
            "Pocetni novac ne moze biti negativan.",
        ));
    }

    if current_shift_for_user(state, user_id)?.is_some() {
        return Err(CommandError::new(
            "validation_error",
            "Korisnik vec ima otvorenu smenu.",
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
        .ok_or_else(|| CommandError::new("not_found", "Smena nije pronadjena posle otvaranja."))
}

pub fn close_shift_for_user(
    state: &AppState,
    user_id: i64,
    request: CloseShiftRequest,
) -> Result<ShiftSummary, CommandError> {
    if request.counted_cash_minor < 0 {
        return Err(CommandError::new(
            "validation_error",
            "Prebrojana gotovina ne moze biti negativna.",
        ));
    }

    let shift = shift_by_id(state, request.shift_id)?
        .ok_or_else(|| CommandError::new("not_found", "Smena nije pronadjena."))?;

    if shift.user_id != user_id || shift.status != "open" {
        return Err(CommandError::new(
            "not_found",
            "Otvorena smena nije pronadjena.",
        ));
    }

    let expected_cash_minor = shift.opening_cash_minor + shift.cash_sales_minor;
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
            "Otvorena smena nije pronadjena.",
        ));
    }

    shift_by_id(state, request.shift_id)?
        .ok_or_else(|| CommandError::new("not_found", "Smena nije pronadjena."))
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
    COALESCE(SUM(CASE WHEN sp.payment_method = 'card' THEN sp.amount_minor ELSE 0 END), 0)
FROM shifts sh
JOIN users u ON u.id = sh.user_id
LEFT JOIN sales s ON s.shift_id = sh.id AND s.status = 'completed'
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
    let expected_cash_minor = if status == "open" {
        opening_cash_minor + cash_sales_minor
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

    use super::{close_shift_for_user, open_shift_for_user, CloseShiftRequest, OpenShiftRequest};
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
            assert_eq!(error.message, "Korisnik vec ima otvorenu smenu.");
        });
    }
}
