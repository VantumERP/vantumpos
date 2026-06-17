mod app_error;
mod commands;
mod db;
mod state;

use db::Db;
use state::{resolve_database_path, AppState};
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let db_path = resolve_database_path(&app.handle())?;
            let db = Db::new(db_path)?;
            app.manage(AppState::new(db));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::health::get_app_health])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
