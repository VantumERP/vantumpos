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
//!
//! Consumed by the gate wiring (Task 4) and the command layer, so `dead_code`
//! is allowed here — mirroring the other KEP domain modules.

#![allow(dead_code)]

use rusqlite::{params, Connection};
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
                .query_row(
                    "SELECT COUNT(*) FROM kep_entries WHERE book_year = 2026",
                    [],
                    |r| r.get(0),
                )
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
            close_year(conn, 2026, CLOSE_CONFIRMATION, 1, "2027-01-05T09:00:00Z").expect("close");
            let err = ensure_year_open(conn, 2026).expect_err("closed year rejects");
            assert_eq!(err.code(), "year_closed");
            ensure_year_open(conn, 2027).expect("a later open year still passes");
        });
    }
}
