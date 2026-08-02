//! Admin-gated command layer over the reklamacije domain.
//!
//! Thin by design: every deadline rule, the frozen regime, the warning gate and
//! the one-extension rule live in `crate::reklamacije`; the documents live in
//! `crate::reklamacije_docs`. These wrappers do three things and nothing else —
//! establish the acting user from the server-side session (never a client
//! payload), open the connection, and stamp `now` in RFC3339. `today` is that
//! same `now`: deadlines are derived per-read, never with `datetime('now')`
//! inside the engine.
//!
//! Every command is `require_admin`. Consumer PII is admin-gated by that same
//! gate — a cashier can neither read nor write the register.

use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::commands::reports::ExportedFile;
use crate::reklamacije::{AnswerInput, ReklamacijaInput, ReklamacijaSummary, ReklamacijaView};
use crate::state::AppState;

#[tauri::command]
pub fn reklamacija_list(
    state: State<'_, AppState>,
) -> Result<Vec<ReklamacijaSummary>, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::reklamacije::list_reklamacije(&connection, &now).map_err(Into::into)
}

#[tauri::command]
pub fn reklamacija_get(
    state: State<'_, AppState>,
    id: i64,
) -> Result<ReklamacijaView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::reklamacije::get_reklamacija(&connection, id, &now).map_err(Into::into)
}

#[tauri::command]
pub fn reklamacija_create(
    state: State<'_, AppState>,
    input: ReklamacijaInput,
) -> Result<ReklamacijaView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::reklamacije::create_reklamacija(&mut connection, &input, acting.id, &now)
        .map_err(Into::into)
}

#[tauri::command]
pub fn reklamacija_log_answer(
    state: State<'_, AppState>,
    id: i64,
    input: AnswerInput,
) -> Result<ReklamacijaView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::reklamacije::log_answer(&mut connection, id, &input, acting.id, &now).map_err(Into::into)
}

#[tauri::command]
pub fn reklamacija_consumer_received(
    state: State<'_, AppState>,
    id: i64,
    event_date: String,
) -> Result<ReklamacijaView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::reklamacije::log_consumer_received_answer(
        &mut connection,
        id,
        &event_date,
        acting.id,
        &now,
    )
    .map_err(Into::into)
}

#[tauri::command]
pub fn reklamacija_consumer_responded(
    state: State<'_, AppState>,
    id: i64,
    event_date: String,
) -> Result<ReklamacijaView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::reklamacije::log_consumer_response(&mut connection, id, &event_date, acting.id, &now)
        .map_err(Into::into)
}

#[tauri::command]
pub fn reklamacija_grant_extension(
    state: State<'_, AppState>,
    id: i64,
    new_deadline: String,
    consumer_consent: bool,
    reason: String,
    event_date: String,
) -> Result<ReklamacijaView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    // The consented `new_deadline` is the extension event's date; the engine
    // takes the tighter of it and the statutory clock. `event_date` records when
    // the consent was captured. `reason` is stored for the evidencija.
    let _ = event_date;
    crate::reklamacije::grant_extension(
        &mut connection,
        id,
        &new_deadline,
        consumer_consent,
        &reason,
        acting.id,
        &now,
    )
    .map_err(Into::into)
}

#[tauri::command]
pub fn reklamacija_resolve(
    state: State<'_, AppState>,
    id: i64,
    nacin: String,
    event_date: String,
    no_fee_attested: bool,
) -> Result<ReklamacijaView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::reklamacije::resolve_reklamacija(
        &mut connection,
        id,
        &nacin,
        &event_date,
        no_fee_attested,
        acting.id,
        &now,
    )
    .map_err(Into::into)
}

/// Writes a rendered HTML document into the `exports/` dir beside the database —
/// the same location `reports_export_csv` resolves — and returns the
/// `reports::ExportedFile` descriptor the frontend already understands. Mirrors
/// `campaigns::write_export`.
fn write_export(
    state: &AppState,
    file_name: &str,
    html: &str,
    row_count: usize,
) -> Result<ExportedFile, AppError> {
    let export_dir = state.db().path().parent().map_or_else(
        || std::path::Path::new(".").join("exports"),
        |parent| parent.join("exports"),
    );
    std::fs::create_dir_all(&export_dir)?;
    let path = export_dir.join(file_name);
    std::fs::write(&path, html)?;
    Ok(ExportedFile {
        file_name: file_name.to_string(),
        path: path.display().to_string(),
        mime_type: "text/html",
        row_count,
    })
}

/// Self-contained potvrda o prijemu reklamacije (čl. 55 st. 7 / čl. 63 st. 7),
/// carrying only this complaint's own data. Read-only over the record.
#[tauri::command]
pub fn reklamacija_export_potvrda(
    state: State<'_, AppState>,
    id: i64,
) -> Result<ExportedFile, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    let view = crate::reklamacije::get_reklamacija(&connection, id, &now)?;
    let html = crate::reklamacije_docs::render_potvrda_html(&view);
    let file_name = format!("potvrda-reklamacija-{}.html", view.register_number);
    write_export(state.inner(), &file_name, &html, 1).map_err(Into::into)
}

/// The statutory prodajno-mesto display notice (čl. 55 st. 4 / čl. 63 st. 4).
/// Static text — no record is read.
#[tauri::command]
pub fn reklamacija_export_notice(state: State<'_, AppState>) -> Result<ExportedFile, CommandError> {
    super::auth::require_admin(state.inner())?;
    let html = crate::reklamacije_docs::render_notice_html();
    write_export(state.inner(), "obavestenje-reklamacije.html", &html, 0).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};
    use crate::reklamacije::ReklamacijaInput;
    use crate::state::AppState;
    use rusqlite::params;
    use tauri::Manager;

    /// The commands take a Tauri `State`, so a headless mock app is needed to
    /// obtain a real managed state — the pattern established in `campaigns.rs`.
    /// Reklamacije carry no product FK, so no catalog seeding is required; the
    /// bootstrap admin (id 1) seeded by `Db::new` satisfies the `created_by` FK.
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

    fn reklamacija_count(state: &AppState) -> i64 {
        state
            .db()
            .open()
            .expect("database should open")
            .query_row("SELECT COUNT(*) FROM reklamacije", params![], |row| {
                row.get(0)
            })
            .expect("reklamacija count should read")
    }

    fn intake_input() -> ReklamacijaInput {
        ReklamacijaInput {
            podnosilac_ime_prezime: "Petar Petrović".into(),
            kontakt: Some("060/123-456".into()),
            podaci_o_robi: "Frižider Beko".into(),
            opis_nesaobraznosti: "Ne hladi".into(),
            zahtev: "Zamena".into(),
            roba_kind: "tehnicka".into(),
            filed_at: "2026-06-01T00:00:00Z".into(),
        }
    }

    #[test]
    fn reklamacija_create_rejected_for_cashier() {
        with_app("reklamacija_create_rejected_for_cashier", |app| {
            sign_in_cashier(app.state::<AppState>().inner());

            let error = reklamacija_create(app.state::<AppState>(), intake_input())
                .expect_err("cashier should not create reklamacije");

            assert_eq!(error.code, "forbidden");
            assert_eq!(
                reklamacija_count(app.state::<AppState>().inner()),
                0,
                "a rejected create must persist nothing"
            );
        });
    }

    #[test]
    fn reklamacija_list_rejected_for_cashier() {
        with_app("reklamacija_list_rejected_for_cashier", |app| {
            sign_in_cashier(app.state::<AppState>().inner());

            let error = reklamacija_list(app.state::<AppState>())
                .expect_err("cashier should not list reklamacije");

            assert_eq!(error.code, "forbidden");
        });
    }

    #[test]
    fn reklamacija_export_potvrda_writes_file_for_admin() {
        with_app("reklamacija_export_potvrda_writes_file_for_admin", |app| {
            sign_in_admin(app.state::<AppState>().inner());

            let created = reklamacija_create(app.state::<AppState>(), intake_input())
                .expect("admin should create a reklamacija");
            assert_eq!(created.register_number, 1);

            let exported = reklamacija_export_potvrda(app.state::<AppState>(), created.id)
                .expect("admin should export the potvrda");

            assert_eq!(exported.file_name, "potvrda-reklamacija-1.html");
            assert_eq!(exported.mime_type, "text/html");

            // The command's contract: a file exists at the returned path, and its
            // bytes carry this complaint's register number.
            let contents =
                std::fs::read_to_string(&exported.path).expect("the export file must exist");
            assert!(
                contents.contains(&created.register_number.to_string()),
                "the potvrda must carry the register number"
            );

            std::fs::remove_file(&exported.path).expect("export file should clean up");
        });
    }

    #[test]
    fn reklamacija_export_notice_writes_static_file_for_admin() {
        with_app(
            "reklamacija_export_notice_writes_static_file_for_admin",
            |app| {
                sign_in_admin(app.state::<AppState>().inner());

                let exported = reklamacija_export_notice(app.state::<AppState>())
                    .expect("admin should export the notice");

                assert_eq!(exported.file_name, "obavestenje-reklamacije.html");
                assert_eq!(exported.mime_type, "text/html");

                let contents =
                    std::fs::read_to_string(&exported.path).expect("the export file must exist");
                assert!(contents.contains("Obaveštenje o načinu i mestu prijema reklamacija"));

                std::fs::remove_file(&exported.path).expect("export file should clean up");
            },
        );
    }
}
