mod aml;
mod app_error;
mod audit;
mod campaign_evidence;
mod campaigns;
mod cash_deposit;
/// The ZZPL čl. 47 evidencija radnji obrade, generated from configuration.
mod cl47;
mod clock;
mod commands;
mod db;
/// Guards over the compliance prose in `docs/`. Test-only — the documents are
/// embedded in the test binary and never in the shipped app.
#[cfg(test)]
mod docs_guard;
mod importer;
mod kep;
mod kep_close;
mod kep_kalkulacija;
mod kep_storno;
mod legal;
mod nbs_rate;
mod price_history;
mod reklamacije;
mod reklamacije_docs;
mod retention;
mod security;
mod state;
mod text;
mod worktime;

use db::Db;
use state::{resolve_database_path, AppState};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!("panic: {info}");
        default_hook(info);
    }));

    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(window) = app.get_webview_window("main") {
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_log::Builder::new()
                .target(tauri_plugin_log::Target::new(
                    tauri_plugin_log::TargetKind::LogDir { file_name: None },
                ))
                .build(),
        )
        .setup(|app| {
            let db_path = resolve_database_path(app.handle())?;
            let db = Db::new(db_path)?;
            app.manage(AppState::new(db));

            let state_for_launch = app.state::<AppState>().inner().clone();

            // The retention classes must exist before anything can consult them,
            // and unlike the backup below this is not best-effort: a database
            // that cannot record what may never be purged is a database whose
            // next purge path has no policy to read. The wall clock is read here,
            // at the outermost boundary, and passed in — `retention.rs` itself
            // never reads one.
            retention::seed_retention_policies(&state_for_launch, &clock::utc_now()?)?;

            // Req. 28: the čl. 47 evidencija radnji obrade is generated, and it
            // is generated HERE rather than behind a button, because a register
            // that exists only once an administrator remembers to press
            // something is the missing register this requirement exists to
            // prevent — it is the cheapest inspection finding for a
            // three-employee shop and the one issuable on the spot. It runs
            // after the retention seed because every rok it prints is read out
            // of that table. Best-effort, like the purge and the backup below:
            // a shop that cannot rewrite a derived document must still be able
            // to open its till, and the previous generation stays on file.
            let _ = cl47::generate(&state_for_launch, &clock::utc_now()?);

            // SW-13 req. 23: the čl. 5 st. 1 tač. 5 purge is a proactive
            // rukovalac duty, so it is time-driven and runs here rather than
            // behind a command a person has to remember to press. Best-effort
            // for the same reason the backup below is: a shop that cannot tidy
            // its expired credentials must still be able to open its till, and
            // the failure direction is toward KEEPING records, never toward
            // losing them. Class A is unreachable from it by construction —
            // `PurgeableClass` has no variant for the ZEOR register.
            let _ =
                commands::personnel::purge_expired_classes(&state_for_launch, &clock::utc_now()?);

            // Best-effort automatic backup: a failure here is already
            // recorded as a failed backup_job and must never prevent the
            // app from opening.
            let _ = commands::backup::auto_backup_if_due(&state_for_launch);

            // Periodic re-check on a plain OS thread. `tokio` is only a
            // transitive dependency (pulled in by `tauri` itself) and is not
            // declared directly in Cargo.toml, so `tokio::time::sleep` is not
            // reachable from this crate without adding a new dependency;
            // `std::thread` + `std::thread::sleep` needs nothing async and
            // suits an infrequent (every 6h), fire-and-forget check like
            // this one.
            let state_for_timer = state_for_launch.clone();
            std::thread::spawn(move || loop {
                std::thread::sleep(commands::backup::AUTO_BACKUP_INTERVAL);
                let _ = commands::backup::auto_backup_if_due(&state_for_timer);
                // A till that stays open for weeks would otherwise only purge on
                // the next restart, which is not „time-driven“ in any sense a
                // čl. 5 st. 1 tač. 5 review would accept.
                if let Ok(now) = clock::utc_now() {
                    let _ = commands::personnel::purge_expired_classes(&state_for_timer, &now);
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::health::get_app_health,
            commands::auth::auth_get_session,
            commands::auth::auth_login,
            commands::auth::auth_logout,
            commands::users::users_list,
            commands::users::users_create,
            commands::users::users_update,
            commands::users::users_deactivate,
            commands::users::users_employee_profile,
            commands::shifts::shift_get_current,
            commands::shifts::shift_open,
            commands::shifts::shift_close,
            commands::shifts::shift_admin_close,
            commands::shifts::shift_cash_movement,
            commands::reports::reports_daily_turnover,
            commands::reports::reports_shift_turnover,
            commands::reports::reports_cashier_turnover,
            commands::reports::reports_payment_methods,
            commands::reports::reports_product_sales,
            commands::reports::reports_category_sales,
            commands::reports::reports_low_stock,
            commands::reports::reports_list_shifts,
            commands::reports::reports_export_csv,
            commands::inventory::inventory_list_stock,
            commands::inventory::inventory_get_product_ledger,
            commands::inventory::inventory_receive,
            commands::inventory::inventory_correct,
            commands::inventory::inventory_write_off,
            commands::inventory::inventory_mark_declaration_checked,
            commands::receipts::receipts_search,
            commands::receipts::receipts_get,
            commands::receipts::receipts_void,
            commands::receipts::receipts_return_items,
            commands::receipts::receipts_set_esir_number,
            commands::imports::import_read_headers,
            commands::imports::import_validate,
            commands::imports::import_commit,
            commands::imports::import_list_jobs,
            commands::imports::import_get_job,
            commands::settings::settings_get_company,
            commands::settings::settings_update_company,
            commands::settings::settings_list_tax_rates,
            commands::settings::settings_save_tax_rate,
            commands::settings::settings_seed_tax_rates,
            commands::settings::settings_get_receipt,
            commands::settings::settings_update_receipt,
            commands::settings::settings_get_sales,
            commands::settings::settings_update_sales,
            commands::settings::settings_get_shop_profile,
            commands::settings::settings_update_shop_profile,
            commands::settings::settings_lpfr_notice,
            commands::settings::settings_get_eur_rate,
            commands::settings::settings_refresh_eur_rate,
            commands::settings::settings_set_manual_eur_rate,
            commands::catalog::catalog_list_products,
            commands::catalog::catalog_search_products,
            commands::catalog::catalog_get_product,
            commands::catalog::catalog_create_product,
            commands::catalog::catalog_update_product,
            commands::catalog::catalog_set_product_active,
            commands::catalog::catalog_declaration_gaps,
            commands::catalog::catalog_lookup_product_by_barcode,
            commands::catalog::catalog_list_categories,
            commands::catalog::catalog_save_category,
            commands::catalog::catalog_prethodna_cena,
            commands::campaigns::campaigns_list,
            commands::campaigns::campaigns_get,
            commands::campaigns::campaigns_validate,
            commands::campaigns::campaigns_create,
            commands::campaigns::campaigns_update,
            commands::campaigns::campaigns_activate,
            commands::campaigns::campaigns_adjust_item_price,
            commands::campaigns::campaigns_end,
            commands::campaigns::campaigns_cancel,
            commands::campaigns::campaigns_export_evidence,
            commands::campaigns::campaigns_export_labels,
            commands::campaigns::campaigns_correction_report,
            commands::campaigns::campaigns_export_correction_report,
            commands::cash_deposit::cash_deposit_report,
            commands::cash_deposit::cash_deposit_export_csv,
            commands::cash_deposit::cash_deposit_calendar,
            commands::cash_deposit::cash_deposit_set_saturday_is_working,
            commands::cash_deposit::cash_deposit_save_non_working_day,
            commands::cash_deposit::cash_deposit_delete_non_working_day,
            commands::reklamacije::reklamacija_list,
            commands::reklamacije::reklamacija_get,
            commands::reklamacije::reklamacija_create,
            commands::reklamacije::reklamacija_log_answer,
            commands::reklamacije::reklamacija_consumer_received,
            commands::reklamacije::reklamacija_consumer_responded,
            commands::reklamacije::reklamacija_grant_extension,
            commands::reklamacije::reklamacija_resolve,
            commands::reklamacije::reklamacija_export_potvrda,
            commands::reklamacije::reklamacija_export_notice,
            commands::kep::kep_ledger,
            commands::kep::kep_post_daily_sales,
            commands::kep::kep_status,
            commands::kep::kep_list_kalkulacije,
            commands::kep::kep_export_kalkulacija,
            commands::kep::kep_nivelacija,
            commands::kep::kep_post_adjustment,
            commands::kep::kep_correct_entry,
            commands::kep::kep_close_preview,
            commands::kep::kep_close_year,
            commands::kep::kep_list_closures,
            commands::kep::kep_export_close,
            commands::kep::kep_export_book,
            commands::sales::sales_preview,
            commands::sales::sales_complete,
            commands::sales::sales_assess_cash_payment,
            commands::backup::backup_get_status,
            commands::backup::backup_update_settings,
            commands::backup::backup_create,
            commands::backup::backup_restore,
            commands::backup::backup_reset_trading_data,
            commands::backup::backup_list_jobs,
            commands::backup::backup_set_passphrase,
            commands::worktime::worktime_list_month,
            commands::worktime::worktime_save_entry,
            commands::worktime::worktime_correct_entry,
            commands::worktime::worktime_close_period,
            commands::worktime::worktime_export_csv,
            commands::worktime::worktime_my_hours,
            commands::worktime::worktime_notices,
            commands::audit::support_grant_access,
            commands::audit::support_request_access,
            commands::audit::support_end_session,
            commands::audit::support_active_session,
            // Req. 7: a read path and nothing else. There is deliberately no
            // command here that edits or removes a logged row — an
            // owner-editable audit log proves nothing, and proving something is
            // the entire reason it exists.
            commands::audit::audit_search,
            commands::audit::audit_export_csv,
            // SW-13 class A. Req. 24: there is deliberately no delete command
            // here, and the time-driven purge is not a command at all — it is
            // called from the launch and timer path above, because čl. 5 st. 1
            // tač. 5 is a proactive duty and not a request the webview makes.
            commands::personnel::personnel_get,
            commands::personnel::personnel_save,
            // Req. 6 + req. 22: the rok is a setting, and this is the pair that
            // makes it one — a reader and a mover, both admin-gated. There is
            // deliberately no shortening verb and no verb that reaches a trajno
            // class: `commands::retention::AdjustableClass` has no variant for
            // one, so the ZEOR čl. 5 evidencija and the čl. 47 register are out
            // of reach by type rather than by review.
            commands::retention::retention_list_policies,
            commands::retention::retention_extend_policy,
            // SW-17. Req. 43: there is no notifiability gate in front of the
            // write — čl. 52 st. 6 covers „svaku povredu“ and the risk question
            // is answered on a record that already exists. And there is no
            // delete command: the record is what st. 7 makes the vehicle for
            // proving compliance with the whole article.
            commands::breaches::breaches_list,
            commands::breaches::breaches_record,
            commands::breaches::breaches_update,
            commands::breaches::breaches_notice,
            commands::breaches::breaches_export_obrazac,
            // Req. 28. Three verbs and no delete: the register is generated from
            // configuration, so removing a radnja means removing the processing,
            // and čl. 47 st. 7 keeps the record trajno either way.
            cl47::cl47_list,
            cl47::cl47_generate,
            cl47::cl47_export
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
