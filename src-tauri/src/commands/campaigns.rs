//! Admin-gated command layer over the campaign domain.
//!
//! Thin by design: every rule, every anchor and every price write lives in
//! `crate::campaigns`. These wrappers do three things and nothing else —
//! establish the acting user from the server-side session (never a client
//! payload), open the connection, and stamp `now` in RFC3339.
//!
//! Nothing here affirms that a promotion is lawful. `campaigns_validate`
//! returns an empty `hard` list when no mechanically checkable rule was
//! broken; čl. 38 st. 4 is an open standard that no command can settle.

use tauri::State;

use crate::app_error::CommandError;
use crate::campaigns::{
    CampaignInput, CampaignSummary, CampaignView, EndOverride, ValidationReport,
};
use crate::state::AppState;

#[tauri::command]
pub fn campaigns_list(state: State<'_, AppState>) -> Result<Vec<CampaignSummary>, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::campaigns::list_campaigns(&connection, &now).map_err(Into::into)
}

#[tauri::command]
pub fn campaigns_get(state: State<'_, AppState>, id: i64) -> Result<CampaignView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::campaigns::get_campaign(&connection, id, &now).map_err(Into::into)
}

/// DRY RUN for the wizard's live feedback. Persists nothing — it opens no
/// transaction and takes a read-only connection precisely so a validation
/// pass can never leave a campaign, an anchor or a price row behind.
#[tauri::command]
pub fn campaigns_validate(
    state: State<'_, AppState>,
    input: CampaignInput,
) -> Result<ValidationReport, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::campaigns::validation_report(&connection, &input, None, &now).map_err(Into::into)
}

#[tauri::command]
pub fn campaigns_create(
    state: State<'_, AppState>,
    input: CampaignInput,
) -> Result<CampaignView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::campaigns::create_campaign(&mut connection, &input, acting.id, &now).map_err(Into::into)
}

#[tauri::command]
pub fn campaigns_update(
    state: State<'_, AppState>,
    id: i64,
    input: CampaignInput,
) -> Result<CampaignView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::campaigns::update_campaign(&mut connection, id, &input, &now).map_err(Into::into)
}

#[tauri::command]
pub fn campaigns_activate(
    state: State<'_, AppState>,
    id: i64,
) -> Result<CampaignView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::campaigns::activate_campaign(&mut connection, id, acting.id, &now).map_err(Into::into)
}

#[tauri::command]
pub fn campaigns_adjust_item_price(
    state: State<'_, AppState>,
    campaign_id: i64,
    product_id: i64,
    new_price_minor: i64,
) -> Result<CampaignView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::campaigns::adjust_item_price(
        &mut connection,
        campaign_id,
        product_id,
        new_price_minor,
        acting.id,
        &now,
    )
    .map_err(Into::into)
}

#[tauri::command]
pub fn campaigns_end(
    state: State<'_, AppState>,
    id: i64,
    overrides: Vec<EndOverride>,
) -> Result<CampaignView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::campaigns::end_campaign(&mut connection, id, &overrides, acting.id, &now)
        .map_err(Into::into)
}

#[tauri::command]
pub fn campaigns_cancel(state: State<'_, AppState>, id: i64) -> Result<CampaignView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::campaigns::cancel_campaign(&mut connection, id, &now).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::campaigns::CampaignItemInput;
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;
    use rusqlite::params;
    use tauri::Manager;

    /// The commands take a Tauri `State`, so a headless mock app is needed to
    /// obtain a real managed state — the pattern established in
    /// `catalog.rs`/`reports.rs`/`inventory.rs`.
    fn with_app(test_name: &str, test: impl FnOnce(&tauri::App<tauri::test::MockRuntime>)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            seed_catalog(&db);
            let app = tauri::test::mock_builder()
                .manage(AppState::new(db))
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");
            test(&app);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    /// One non-perishable product with a clean offered-price history, so the
    /// čl. 37 st. 3 anchor computes without a manual figure.
    fn seed_catalog(db: &Db) {
        let connection = db.open().expect("database should open");
        connection
            .execute_batch(
                "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                 VALUES (1, 'PDV 20', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                 INSERT INTO products (id, name, sku, sale_price_minor, purchase_price_minor,
                                       tax_rate_id, minimum_stock_milli, active, created_at, updated_at)
                 VALUES (1, 'Jakna Z L', 'JAKNA-Z-L', 1290000, 0, 1, 0, 1,
                         '2026-05-15T00:00:00Z', '2026-05-15T00:00:00Z');
                 INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-05-15T00:00:00Z', 1290000, 'create', '2026-05-15T00:00:00Z');",
            )
            .expect("catalog should seed");
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

    fn campaign_count(state: &AppState) -> i64 {
        state
            .db()
            .open()
            .expect("database should open")
            .query_row("SELECT COUNT(*) FROM campaigns", params![], |row| {
                row.get(0)
            })
            .expect("campaign count should read")
    }

    /// Akcijska prodaja, 05.07–20.07.2026 (16 days, inside the čl. 37 st. 10
    /// cap), two-price display, marked down from the 12.900 anchor to 9.900.
    fn valid_input() -> CampaignInput {
        CampaignInput {
            campaign_type: crate::campaigns::TYPE_AKCIJSKA.to_string(),
            starts_on: "2026-07-05T00:00:00Z".to_string(),
            ends_on: Some("2026-07-20T00:00:00Z".to_string()),
            display_mode: "two_prices".to_string(),
            headline_percent: None,
            rasprodaja_ground: None,
            special_conditions: None,
            reduced_utility_reason: None,
            marketing_label: None,
            season_attested: false,
            separation_attested: false,
            items: vec![CampaignItemInput {
                product_id: 1,
                campaign_price_minor: 990000,
                manual_prethodna_minor: None,
                anchor_justification: None,
                future_regular_price_minor: None,
            }],
        }
    }

    #[test]
    fn campaigns_create_rejected_for_cashier() {
        with_app("campaigns_create_rejected_for_cashier", |app| {
            sign_in_cashier(app.state::<AppState>().inner());

            let error = campaigns_create(app.state::<AppState>(), valid_input())
                .expect_err("cashier should not create campaigns");

            assert_eq!(error.code, "forbidden");
            assert_eq!(
                campaign_count(app.state::<AppState>().inner()),
                0,
                "a rejected create must persist nothing"
            );
        });
    }

    #[test]
    fn campaigns_activate_rejected_for_cashier() {
        with_app("campaigns_activate_rejected_for_cashier", |app| {
            sign_in_admin(app.state::<AppState>().inner());
            let created = campaigns_create(app.state::<AppState>(), valid_input())
                .expect("admin should create the draft");

            sign_in_cashier(app.state::<AppState>().inner());
            let error = campaigns_activate(app.state::<AppState>(), created.id)
                .expect_err("cashier should not activate campaigns");

            assert_eq!(error.code, "forbidden");

            // The gate must bite BEFORE any price write: the till price is
            // still the regular one and the campaign is still a draft.
            let till: i64 = app
                .state::<AppState>()
                .db()
                .open()
                .expect("database should open")
                .query_row(
                    "SELECT sale_price_minor FROM products WHERE id = 1",
                    params![],
                    |row| row.get(0),
                )
                .expect("till price should read");
            assert_eq!(till, 1290000);
        });
    }

    #[test]
    fn campaigns_validate_reports_without_persisting() {
        with_app("campaigns_validate_reports_without_persisting", |app| {
            sign_in_admin(app.state::<AppState>().inner());

            let report = campaigns_validate(app.state::<AppState>(), valid_input())
                .expect("validation should run");

            // An empty `hard` means "no rule we can mechanically check was
            // broken" — never "this promotion is legal" (čl. 38 st. 4).
            assert!(
                report.hard.is_empty(),
                "a well-formed akcijska should break no hard rule: {:?}",
                report.hard
            );
            assert_eq!(report.anchors.len(), 1);
            assert_eq!(report.anchors[0].anchor_status, "computed");
            assert_eq!(report.anchors[0].prethodna_cena_minor, Some(1290000));

            assert_eq!(
                campaign_count(app.state::<AppState>().inner()),
                0,
                "a dry run must persist nothing"
            );
        });
    }
}
