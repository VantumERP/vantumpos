use serde::Serialize;
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::state::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppHealth {
    pub backend: &'static str,
    pub app_version: String,
    pub database_path: String,
    pub migrated: bool,
}

#[tauri::command]
pub fn get_app_health(state: State<'_, AppState>) -> Result<AppHealth, CommandError> {
    build_health_response(state.inner(), env!("CARGO_PKG_VERSION")).map_err(Into::into)
}

pub fn build_health_response(
    state: &AppState,
    app_version: impl Into<String>,
) -> Result<AppHealth, AppError> {
    let conn = state.db().open()?;
    let migration_count: i64 =
        conn.query_row("SELECT COUNT(*) FROM _migrations", [], |row| row.get(0))?;

    Ok(AppHealth {
        backend: "local",
        app_version: app_version.into(),
        database_path: state.db().path().display().to_string(),
        migrated: migration_count > 0,
    })
}

#[cfg(test)]
mod tests {
    use crate::commands::health::build_health_response;
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

    #[test]
    fn health_response_reports_database_path_and_migration_state() {
        let db_path =
            test_database_path("health_response_reports_database_path_and_migration_state");
        let db = Db::new(db_path.clone()).expect("database should initialize");
        let state = AppState::new(db);

        let response = build_health_response(&state, "0.1.0").expect("health should build");

        assert_eq!(response.backend, "local");
        assert_eq!(response.app_version, "0.1.0");
        assert!(response.database_path.ends_with(".sqlite3"));
        assert!(response.migrated);

        let _ = std::fs::remove_file(db_path);
    }
}
