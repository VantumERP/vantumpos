mod app_error;
mod campaigns;
mod clock;
mod commands;
mod db;
mod importer;
mod price_history;
mod security;
mod state;
mod text;

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

            // Best-effort automatic backup: a failure here is already
            // recorded as a failed backup_job and must never prevent the
            // app from opening.
            let state_for_launch = app.state::<AppState>().inner().clone();
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
            commands::catalog::catalog_list_products,
            commands::catalog::catalog_search_products,
            commands::catalog::catalog_get_product,
            commands::catalog::catalog_create_product,
            commands::catalog::catalog_update_product,
            commands::catalog::catalog_set_product_active,
            commands::catalog::catalog_lookup_product_by_barcode,
            commands::catalog::catalog_list_categories,
            commands::catalog::catalog_save_category,
            commands::catalog::catalog_prethodna_cena,
            commands::sales::sales_preview,
            commands::sales::sales_complete,
            commands::backup::backup_get_status,
            commands::backup::backup_update_settings,
            commands::backup::backup_create,
            commands::backup::backup_restore,
            commands::backup::backup_reset_trading_data,
            commands::backup::backup_list_jobs,
            commands::backup::backup_set_passphrase
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
