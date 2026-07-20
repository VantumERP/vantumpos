//! Admin-gated command layer over the KEP (evidencija prometa) value ledger.
//!
//! Thin by design: every posting rule — redni-broj allocation, the retail-with-
//! PDV receipt zaduženje, the daily razduženje idempotency, the derived saldo —
//! lives in `crate::kep`. These wrappers do three things and nothing else:
//! establish the acting user from the server-side session (never a client
//! payload), open the connection, and stamp `now`/`today` from `utc_now()`.
//!
//! Every command is `require_admin`; a cashier can neither read the ledger nor
//! post a trading day. The ledger is append-only (PEP čl. 14): there is no edit
//! or delete command — corrections are 9b.

use tauri::State;

use crate::app_error::CommandError;
use crate::kep::{KepEntryView, KepLedger, KepStatus};
use crate::state::AppState;

#[tauri::command]
pub fn kep_ledger(state: State<'_, AppState>, book_year: i64) -> Result<KepLedger, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    crate::kep::list_ledger(&connection, book_year).map_err(Into::into)
}

#[tauri::command]
pub fn kep_post_daily_sales(
    state: State<'_, AppState>,
    date: String,
    override_amount_minor: Option<i64>,
) -> Result<KepEntryView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::kep::post_daily_sales(
        &mut connection,
        &date,
        override_amount_minor,
        acting.id,
        &now,
    )
    .map_err(Into::into)
}

#[tauri::command]
pub fn kep_status(state: State<'_, AppState>) -> Result<KepStatus, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let today = crate::clock::utc_now()?;
    crate::kep::kep_status(&connection, &today).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;
    use rusqlite::params;
    use tauri::Manager;

    /// The commands take a Tauri `State`, so a headless mock app is needed to
    /// obtain a real managed state — the pattern established in `reklamacije.rs`.
    /// The bootstrap admin (id 1) seeded by `Db::new` satisfies the `user_id` FK.
    fn with_app(test_name: &str, test: impl FnOnce(&tauri::App<tauri::test::MockRuntime>)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let app = tauri::test::mock_builder()
                .manage(AppState::new(db))
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");
            test(&app);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    fn sign_in_admin(state: &AppState) {
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
    }

    fn sign_in_cashier(state: &AppState) {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, created_at, updated_at)
                 VALUES ('marko', 'Marko Markovic', 'cashier', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        let cashier_id = connection.last_insert_rowid();
        state
            .set_session_user_id(cashier_id)
            .expect("cashier session should set");
    }

    /// Seeds a receipt zaduženje directly into the ledger for `book_year` so the
    /// happy-path saldo is a realistic zaduženje − razduženje, not a lone sale.
    fn seed_receipt_zaduzenje(
        state: &AppState,
        book_year: i64,
        entry_date: &str,
        amount_minor: i64,
    ) {
        state
            .db()
            .open()
            .expect("database should open")
            .execute(
                "INSERT INTO kep_entries (
                    book_year, redni_broj, entry_date, opis,
                    kolona, amount_minor, kind, entry_source, user_id, created_at
                 ) VALUES (?1, 1, ?2, 'Prijem robe', 'zaduzenje', ?3, 'receipt', 'auto', 1, ?2)",
                params![book_year, entry_date, amount_minor],
            )
            .expect("receipt zaduženje should seed");
    }

    #[test]
    fn kep_ledger_rejected_for_cashier() {
        with_app("kep_ledger_rejected_for_cashier", |app| {
            sign_in_cashier(app.state::<AppState>().inner());

            let error = kep_ledger(app.state::<AppState>(), 2026)
                .expect_err("cashier should not read the ledger");

            assert_eq!(error.code, "forbidden");
        });
    }

    #[test]
    fn kep_post_daily_sales_rejected_for_cashier() {
        with_app("kep_post_daily_sales_rejected_for_cashier", |app| {
            sign_in_cashier(app.state::<AppState>().inner());

            let error = kep_post_daily_sales(app.state::<AppState>(), "2026-07-05".into(), None)
                .expect_err("cashier should not post a trading day");

            assert_eq!(error.code, "forbidden");
        });
    }

    #[test]
    fn kep_status_rejected_for_cashier() {
        with_app("kep_status_rejected_for_cashier", |app| {
            sign_in_cashier(app.state::<AppState>().inner());

            let error = kep_status(app.state::<AppState>())
                .expect_err("cashier should not read the KEP status");

            assert_eq!(error.code, "forbidden");
        });
    }

    // Admin happy path (memo §2.7 numbers): a seeded receipt zaduženje of
    // 7.800,00 minus a posted daily razduženje of 2.340,00 leaves saldo 5.460,00.
    #[test]
    fn admin_posts_a_day_and_reads_saldo() {
        with_app("kep_admin_posts_and_reads_saldo", |app| {
            let state = app.state::<AppState>();
            sign_in_admin(state.inner());

            // The daily post books into `utc_now()`'s book year; seed the
            // zaduženje into that same year so both rows share the ledger.
            let now = crate::clock::utc_now().expect("clock");
            let book_year = crate::kep::book_year_of(&now).expect("book year");
            seed_receipt_zaduzenje(state.inner(), book_year, &now, 780000);

            let entry =
                kep_post_daily_sales(app.state::<AppState>(), "2026-07-05".into(), Some(234000))
                    .expect("admin should post the day");
            assert_eq!(entry.razduzenje_minor, Some(234000));
            assert_eq!(entry.kind, "daily_sales");

            let ledger = kep_ledger(app.state::<AppState>(), book_year).expect("admin should read");
            assert_eq!(ledger.entries.len(), 2, "zaduženje + razduženje");
            assert_eq!(ledger.saldo_minor, 546000, "780000 − 234000");
        });
    }
}
