//! Admin-gated command layer over the KEP (evidencija prometa) value ledger.
//!
//! Thin by design: every posting rule — redni-broj allocation, the retail-with-
//! PDV receipt zaduženje, the daily razduženje idempotency, the derived saldo —
//! lives in `crate::kep`. These wrappers do three things and nothing else:
//! establish the acting user from the server-side session (never a client
//! payload), open the connection, and stamp `now`/`today` from `utc_now()`.
//!
//! Every command is `require_admin`; a cashier can neither read the ledger nor
//! post a trading day, a nivelacija, a storno or a correction. The ledger is
//! append-only (PEP čl. 14): there is no edit or delete command — a correction
//! is a reversing storno (`kep_correct_entry`), never an in-place edit.
//!
//! The 9b adjustment commands keep the same thin shape: they resolve the acting
//! admin, open the connection, run the domain function in a transaction, and
//! commit. Every posting rule — the cause→{kolona, sign, kind} map, the backward
//! kalkulacija, the nivelacija revaluation — lives in `crate::kep_storno` /
//! `crate::kep_kalkulacija`, never here.

use serde::Serialize;
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::commands::reports::ExportedFile;
use crate::kep::{KepEntryView, KepLedger, KepStatus};
use crate::kep_kalkulacija::KalkulacijaSummary;
use crate::kep_storno::{BasisDoc, StornoCause};
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

/// Lists a book year's kalkulacije (newest first) for the KEP module's list.
#[tauri::command]
pub fn kep_list_kalkulacije(
    state: State<'_, AppState>,
    book_year: i64,
) -> Result<Vec<KalkulacijaSummary>, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    crate::kep_kalkulacija::list_kalkulacije(&connection, book_year).map_err(Into::into)
}

/// Renders one kalkulacija to a self-contained HTML isprava and writes it into
/// `exports/` (reusing the campaigns export helper), returning the descriptor
/// the frontend opens for print (SW-8). A missing id is `not_found`.
#[tauri::command]
pub fn kep_export_kalkulacija(
    state: State<'_, AppState>,
    id: i64,
) -> Result<ExportedFile, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let view = crate::kep_kalkulacija::load_kalkulacija(&connection, id)?;
    let html = crate::kep_kalkulacija::render_kalkulacija_html(&view);
    let file_name = format!("kalkulacija-{}.html", view.redni_broj);
    super::campaigns::write_export(state.inner(), &file_name, &html, 1).map_err(Into::into)
}

/// Nivelacija — the one adjustment that changes the product price. Updates the
/// catalog price, records the offered-price change, and posts the KEP kolona-4 Δ
/// in one transaction (all in `crate::kep_storno::post_nivelacija`).
#[tauri::command]
pub fn kep_nivelacija(
    state: State<'_, AppState>,
    product_id: i64,
    new_sale_price_minor: i64,
    basis: BasisDoc,
) -> Result<(), CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    let tx = connection.transaction().map_err(AppError::from)?;
    let prices_moved = crate::kep_storno::post_nivelacija(
        &tx,
        product_id,
        new_sale_price_minor,
        &basis,
        acting.id,
        &now,
    )?;
    tx.commit().map_err(AppError::from)?;
    // SW-12 req. 11 — ZZP čl. 6 st. 3: a nivelacija revalues the catalog price,
    // so the published cenovnik must follow it. After the commit, never before
    // it (čl. 6 st. 4), and a publish failure never fails a nivelacija whose
    // price and KEP Δ are already durable.
    if prices_moved {
        super::cenovnik::republish_after_price_move(&connection, &now);
    }
    Ok(())
}

/// Posts a value-only storno for a non-nivelacija cause. The string maps to a
/// `StornoCause` whose {kolona, sign, kind} the hard map dictates; the nivelacija
/// / PDV-rate causes are rejected here (they change price — use `kep_nivelacija`)
/// and an unknown cause is a validation error.
#[tauri::command]
pub fn kep_post_adjustment(
    state: State<'_, AppState>,
    cause: String,
    product_id: i64,
    quantity_milli: i64,
    basis: BasisDoc,
) -> Result<(), CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let storno_cause = parse_adjustment_cause(&cause)?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    let tx = connection.transaction().map_err(AppError::from)?;
    crate::kep_storno::post_value_storno(
        &tx,
        storno_cause,
        product_id,
        quantity_milli,
        &basis,
        acting.id,
        &now,
    )?;
    tx.commit().map_err(AppError::from)?;
    Ok(())
}

/// Corrects a posted row via a reversing storno + re-entry (two new rows, current
/// date). The original row is never edited or deleted (`crate::kep::correct_entry`).
#[tauri::command]
pub fn kep_correct_entry(
    state: State<'_, AppState>,
    target_redni_broj: i64,
    book_year: i64,
    correct_amount_minor: i64,
    basis: BasisDoc,
) -> Result<(), CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    let tx = connection.transaction().map_err(AppError::from)?;
    crate::kep::correct_entry(
        &tx,
        target_redni_broj,
        book_year,
        correct_amount_minor,
        &basis,
        acting.id,
        &now,
    )?;
    tx.commit().map_err(AppError::from)?;
    Ok(())
}

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
    let html =
        crate::kep_close::render_book_html(&company, &ledger, crate::kep_close::BOOK_ROWS_PER_PAGE);
    let file_name = format!("kep-knjiga-{book_year}.html");
    super::campaigns::write_export(state.inner(), &file_name, &html, row_count).map_err(Into::into)
}

/// Maps a frontend cause id to a value-storno `StornoCause`. The nivelacija /
/// PDV-rate causes change the product price and must route through
/// `kep_nivelacija`; anything unrecognised is a validation error (never a silent
/// no-op that would leave the saldo unadjusted).
fn parse_adjustment_cause(cause: &str) -> Result<StornoCause, AppError> {
    match cause {
        "supplier_return" => Ok(StornoCause::SupplierReturn),
        "customer_return" => Ok(StornoCause::CustomerReturn),
        "otpis" => Ok(StornoCause::Otpis),
        "manjak_odluka" => Ok(StornoCause::ManjakOdluka),
        "rashod" => Ok(StornoCause::Rashod),
        "popis_visak" => Ok(StornoCause::PopisVisak),
        "popis_manjak" => Ok(StornoCause::PopisManjak),
        "nivelacija_up" | "nivelacija_down" | "pdv_rate_up" | "pdv_rate_down" => {
            Err(AppError::business(
                "invalid_state",
                "Nivelacija menja cenu — koristite nivelaciju.",
            ))
        }
        _ => Err(AppError::validation(
            "Nepoznat uzrok storna.",
            serde_json::json!({ "field": "cause", "value": cause }),
        )),
    }
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

    /// A storno/nivelacija basis isprava for the command tests.
    fn basis() -> crate::kep_storno::BasisDoc {
        crate::kep_storno::BasisDoc {
            naziv: "Zapisnik o otpisu".to_string(),
            broj: "7".to_string(),
            datum: "08.07.2026".to_string(),
        }
    }

    /// Seeds a 20% tax rate and a product at the given retail price so the
    /// value-storno / kalkulacija commands have a real product line to read.
    fn seed_product(state: &AppState, id: i64, name: &str, sale_price_minor: i64) {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT OR IGNORE INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                 VALUES (1, 'PDV 20', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("tax rate should insert");
        connection
            .execute(
                "INSERT INTO products (
                    id, name, sku, unit_of_measure, sale_price_minor, purchase_price_minor,
                    tax_rate_id, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, 'kom', ?4, 10000, 1,
                           '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![id, name, format!("SKU-{id}"), sale_price_minor],
            )
            .expect("product should insert");
    }

    #[test]
    fn kep_post_adjustment_rejected_for_cashier() {
        with_app("kep_post_adjustment_rejected_for_cashier", |app| {
            sign_in_cashier(app.state::<AppState>().inner());

            let error =
                kep_post_adjustment(app.state::<AppState>(), "otpis".into(), 1, 35_000, basis())
                    .expect_err("cashier should not post an adjustment");

            assert_eq!(error.code, "forbidden");
        });
    }

    #[test]
    fn kep_nivelacija_rejected_for_cashier() {
        with_app("kep_nivelacija_rejected_for_cashier", |app| {
            sign_in_cashier(app.state::<AppState>().inner());

            let error = kep_nivelacija(app.state::<AppState>(), 1, 17600, basis())
                .expect_err("cashier should not post a nivelacija");

            assert_eq!(error.code, "forbidden");
        });
    }

    /// Identifies the prodajni objekat so the SW-12 publish path has an archive
    /// lineage to key on; without it nothing is published at all.
    fn seed_outlet(state: &AppState) {
        state
            .db()
            .open()
            .expect("database should open")
            .execute(
                "INSERT INTO settings (key, value_json, updated_at)
                 VALUES ('company', ?1, '2026-01-01T00:00:00Z')",
                params![serde_json::json!({
                    "shopName": "Butik Ana",
                    "address": "Bulevar oslobođenja 1, Novi Sad",
                    "pib": "",
                    "registrationNumber": "",
                    "phone": "",
                    "logoPath": null,
                    "currency": "RSD",
                })
                .to_string()],
            )
            .expect("company settings should seed");
    }

    /// SW-12 req. 11 — ZZP čl. 6 st. 3. A nivelacija is a revaluation of the
    /// catalog price, so the published cenovnik must follow it just as a catalog
    /// edit does; the KEP Δ it books is not a substitute for republishing.
    #[test]
    fn a_nivelacija_republishes_the_cenovnik() {
        with_app("a_nivelacija_republishes_the_cenovnik", |app| {
            let state = app.state::<AppState>();
            sign_in_admin(state.inner());
            seed_product(state.inner(), 1, "Mleko 1l", 15600);
            seed_outlet(state.inner());

            kep_nivelacija(app.state::<AppState>(), 1, 17600, basis())
                .expect("admin should post the nivelacija");

            let body: String = state
                .db()
                .open()
                .expect("database should open")
                .query_row(
                    "SELECT body FROM cenovnik_snapshots ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .expect("the nivelacija must republish the cenovnik");
            assert!(body.contains("176.00"), "{body:?}");
        });
    }

    // Admin happy path (memo §4.5): otpis 35 kom @ 156,00 → −5.460,00 crveni
    // storno in kolona 4.
    #[test]
    fn admin_posts_an_otpis_adjustment_negative_in_kolona_4() {
        with_app("kep_admin_otpis_adjustment", |app| {
            let state = app.state::<AppState>();
            sign_in_admin(state.inner());
            seed_product(state.inner(), 1, "Mleko 1l", 15600);

            kep_post_adjustment(app.state::<AppState>(), "otpis".into(), 1, 35_000, basis())
                .expect("admin should post the otpis");

            let (kolona, amount, kind, cause) = state
                .db()
                .open()
                .expect("database should open")
                .query_row(
                    "SELECT kolona, amount_minor, kind, cause
                     FROM kep_entries ORDER BY id DESC LIMIT 1",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                        ))
                    },
                )
                .expect("a kep row");
            assert_eq!(kolona, "zaduzenje");
            assert_eq!(amount, -546000, "35 × 156,00 crveni storno");
            assert_eq!(kind, "nivelacija_down_storno");
            assert_eq!(cause, "otpis");
        });
    }

    // The nivelacija causes change the product price and must route through
    // `kep_nivelacija`; an unrecognised cause is a validation error.
    #[test]
    fn kep_post_adjustment_rejects_nivelacija_and_unknown_causes() {
        with_app("kep_post_adjustment_rejects_bad_causes", |app| {
            let state = app.state::<AppState>();
            sign_in_admin(state.inner());
            seed_product(state.inner(), 1, "Mleko 1l", 15600);

            let niv = kep_post_adjustment(
                app.state::<AppState>(),
                "nivelacija_down".into(),
                1,
                1_000,
                basis(),
            )
            .expect_err("nivelacija must route through kep_nivelacija");
            assert_eq!(niv.code, "invalid_state");

            let unknown = kep_post_adjustment(
                app.state::<AppState>(),
                "izmišljeno".into(),
                1,
                1_000,
                basis(),
            )
            .expect_err("an unknown cause must be rejected");
            assert_eq!(unknown.code, "validation_error");
        });
    }

    #[test]
    fn kep_export_kalkulacija_writes_file_with_product_name() {
        with_app("kep_export_kalkulacija", |app| {
            let state = app.state::<AppState>();
            sign_in_admin(state.inner());
            seed_product(state.inner(), 1, "Mleko 1l", 15600);

            // Persist a kalkulacija (elements 5–14) to export.
            let id = {
                let mut connection = state.db().open().expect("database should open");
                let tx = connection.transaction().expect("tx");
                let id = crate::kep_kalkulacija::create_kalkulacija(
                    &tx,
                    1,
                    50_000,
                    10_000,
                    Some("kalkulacija"),
                    Some(7),
                    1,
                    "2026-07-04T09:00:00Z",
                )
                .expect("create kalkulacija");
                tx.commit().expect("commit");
                id
            };

            let exported = kep_export_kalkulacija(app.state::<AppState>(), id)
                .expect("admin should export the kalkulacija");

            assert!(exported.file_name.starts_with("kalkulacija-"));
            assert_eq!(exported.mime_type, "text/html");
            let contents =
                std::fs::read_to_string(&exported.path).expect("the export file must exist");
            assert!(
                contents.contains("Mleko 1l"),
                "the kalkulacija document names the product"
            );
            assert!(contents.contains("Kalkulacija cene"));

            std::fs::remove_file(&exported.path).expect("export file should clean up");
        });
    }

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
}
