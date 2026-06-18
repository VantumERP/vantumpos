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
    validate_import(state.db(), &request).map_err(Into::into)
}

#[tauri::command]
pub fn import_commit(
    state: State<'_, AppState>,
    request: CommitImportRequest,
) -> Result<ImportJobSummary, CommandError> {
    commit_import(state.db(), &request).map_err(Into::into)
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
