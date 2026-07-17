use tauri::State;

use crate::app_error::CommandError;
use crate::importer::{
    commit_import, get_import_job, list_import_jobs, read_import_headers, validate_import,
    CommitImportRequest, ImportHeaders, ImportJobDetail, ImportJobSummary, ImportValidationResult,
    ReadImportHeadersRequest, ValidateImportRequest,
};
use crate::state::AppState;

#[tauri::command]
pub fn import_read_headers(
    request: ReadImportHeadersRequest,
) -> Result<ImportHeaders, CommandError> {
    read_import_headers(&request).map_err(Into::into)
}

#[tauri::command]
pub fn import_validate(
    state: State<'_, AppState>,
    request: ValidateImportRequest,
) -> Result<ImportValidationResult, CommandError> {
    super::auth::require_admin(state.inner())?;
    validate_import(state.db(), &request).map_err(Into::into)
}

#[tauri::command]
pub fn import_commit(
    state: State<'_, AppState>,
    request: CommitImportRequest,
) -> Result<ImportJobSummary, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    commit_import(state.db(), &request, acting.id).map_err(Into::into)
}

#[tauri::command]
pub fn import_list_jobs(state: State<'_, AppState>) -> Result<Vec<ImportJobSummary>, CommandError> {
    list_import_jobs(state.db()).map_err(Into::into)
}

#[tauri::command]
pub fn import_get_job(
    state: State<'_, AppState>,
    id: i64,
) -> Result<ImportJobDetail, CommandError> {
    get_import_job(state.db(), id).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use tauri::Manager;

    use crate::commands::imports::import_commit;
    use crate::db::{test_database_path, Db};
    use crate::importer::{CommitImportRequest, ImportType};
    use crate::state::AppState;

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

    #[test]
    fn import_commit_rejected_for_cashier() {
        let path = test_database_path("import_commit_rejected_for_cashier");

        {
            let db = Db::new(path.clone()).expect("database should initialize");
            let state = AppState::new(db);
            sign_in_cashier(&state);

            // import_commit takes a Tauri `State`, so a headless mock app is
            // needed to obtain a real managed state, mirroring the admin-gate
            // test pattern used in reports.rs/inventory.rs.
            let app = tauri::test::mock_builder()
                .manage(state)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");

            let error = import_commit(
                app.state::<AppState>(),
                CommitImportRequest {
                    import_type: ImportType::Products,
                    file_name: "artikli.csv".to_string(),
                    csv_text: "Naziv;Cena;PDV;Sifra\nHleb;120,00;20;SKU-1\n".to_string(),
                    mapping: HashMap::new(),
                },
            )
            .expect_err("cashier should not commit imports");

            assert_eq!(error.code, "forbidden");
        }

        std::fs::remove_file(&path).unwrap_or_else(|error| {
            panic!(
                "test database file {} should be removed: {error}",
                path.display()
            )
        });
    }
}
