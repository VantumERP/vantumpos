use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, DatabaseName, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::commands::settings::{load_json_setting, save_json_setting, BACKUP_SETTINGS_KEY};
use crate::state::AppState;

const RESTORE_CONFIRMATION: &str = "VRATI PODATKE";
const BACKUP_STALE_AFTER_HOURS: i64 = 24;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettings {
    #[serde(default)]
    pub backup_folder: String,
    #[serde(default)]
    pub automatic_backup_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettingsRequest {
    pub backup_folder: String,
    pub automatic_backup_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupStatus {
    pub backup_folder: String,
    pub automatic_backup_enabled: bool,
    pub stale: bool,
    pub last_successful_backup: Option<BackupJob>,
    pub last_failed_backup: Option<BackupJob>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupJob {
    pub id: i64,
    pub backup_type: String,
    pub path: String,
    pub status: String,
    pub error_message: Option<String>,
    pub file_size_bytes: Option<i64>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBackupRequest {
    pub backup_folder: Option<String>,
    pub backup_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreBackupRequest {
    pub path: String,
    pub confirmation_text: String,
}

#[tauri::command]
pub fn backup_get_status(state: State<'_, AppState>) -> Result<BackupStatus, CommandError> {
    load_backup_status(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn backup_update_settings(
    state: State<'_, AppState>,
    request: BackupSettingsRequest,
) -> Result<BackupSettings, CommandError> {
    save_backup_settings(state.inner(), request).map_err(Into::into)
}

#[tauri::command]
pub fn backup_create(
    state: State<'_, AppState>,
    request: CreateBackupRequest,
) -> Result<BackupJob, CommandError> {
    create_backup(state.inner(), request).map_err(Into::into)
}

#[tauri::command]
pub fn backup_restore(
    state: State<'_, AppState>,
    request: RestoreBackupRequest,
) -> Result<BackupJob, CommandError> {
    restore_backup(state.inner(), request).map_err(Into::into)
}

#[tauri::command]
pub fn backup_list_jobs(state: State<'_, AppState>) -> Result<Vec<BackupJob>, CommandError> {
    list_backup_jobs(state.inner()).map_err(Into::into)
}

pub fn load_backup_settings(state: &AppState) -> Result<BackupSettings, AppError> {
    load_json_setting(state, BACKUP_SETTINGS_KEY, default_backup_settings(state))
}

pub fn save_backup_settings(
    state: &AppState,
    request: BackupSettingsRequest,
) -> Result<BackupSettings, AppError> {
    if request.backup_folder.trim().is_empty() {
        return Err(AppError::validation(
            "Folder za backup je obavezan.",
            serde_json::json!({ "field": "backupFolder" }),
        ));
    }

    let settings = BackupSettings {
        backup_folder: request.backup_folder.trim().to_string(),
        automatic_backup_enabled: request.automatic_backup_enabled,
    };

    save_json_setting(state, BACKUP_SETTINGS_KEY, &settings)?;
    Ok(settings)
}

pub fn load_backup_status(state: &AppState) -> Result<BackupStatus, AppError> {
    let settings = load_backup_settings(state)?;
    let last_successful_backup = query_latest_backup_job(
        state,
        "status = 'completed' AND backup_type IN ('manual', 'automatic')",
    )?;
    let last_failed_backup = query_latest_backup_job(state, "status = 'failed'")?;
    let stale = is_backup_stale(state)?;

    Ok(BackupStatus {
        backup_folder: settings.backup_folder,
        automatic_backup_enabled: settings.automatic_backup_enabled,
        stale,
        last_successful_backup,
        last_failed_backup,
    })
}

fn is_backup_stale(state: &AppState) -> Result<bool, AppError> {
    let conn = state.db().open()?;
    let threshold_modifier = format!("-{BACKUP_STALE_AFTER_HOURS} hours");
    let has_recent_backup: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM backup_jobs
            WHERE status = 'completed'
              AND backup_type IN ('manual', 'automatic')
              AND created_at >= datetime('now', ?1)
         )",
        params![threshold_modifier],
        |row| row.get(0),
    )?;

    Ok(!has_recent_backup)
}

pub fn create_backup(
    state: &AppState,
    request: CreateBackupRequest,
) -> Result<BackupJob, AppError> {
    let backup_type = request
        .backup_type
        .as_deref()
        .unwrap_or("manual")
        .trim()
        .to_string();
    validate_backup_type(&backup_type)?;

    let mut settings = load_backup_settings(state)?;
    let backup_folder = request
        .backup_folder
        .as_deref()
        .map(str::trim)
        .filter(|folder| !folder.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| settings.backup_folder.clone());

    let backup_path = build_backup_path(&backup_folder, &backup_type)?;
    std::fs::create_dir_all(&backup_folder)?;

    let source = state.db().open()?;
    source
        .backup(DatabaseName::Main, &backup_path, None)
        .map_err(|source| AppError::BackupFailed(format!("Backup nije uspeo: {source}")))?;

    let file_size_bytes = checked_file_size(&backup_path)?;
    settings.backup_folder = backup_folder;
    save_json_setting(state, BACKUP_SETTINGS_KEY, &settings)?;

    insert_backup_job(
        state,
        &backup_type,
        &backup_path,
        "completed",
        None,
        Some(file_size_bytes),
    )
}

pub fn restore_backup(
    state: &AppState,
    request: RestoreBackupRequest,
) -> Result<BackupJob, AppError> {
    if request.confirmation_text.trim() != RESTORE_CONFIRMATION {
        return Err(AppError::validation(
            "Potvrdite restore unosom teksta VRATI PODATKE.",
            serde_json::json!({ "field": "confirmationText" }),
        ));
    }

    let source_path = PathBuf::from(request.path.trim());
    if !source_path.is_file() {
        return Err(AppError::not_found("Backup fajl nije pronadjen."));
    }

    let _pre_restore = create_backup(
        state,
        CreateBackupRequest {
            backup_folder: None,
            backup_type: Some("pre_restore".to_string()),
        },
    )?;

    {
        let mut conn = state.db().open()?;
        conn.restore(
            DatabaseName::Main,
            &source_path,
            Option::<fn(rusqlite::backup::Progress)>::None,
        )
        .map_err(|source| AppError::BackupFailed(format!("Restore nije uspeo: {source}")))?;
    }

    state.db().migrate()?;
    let file_size_bytes = checked_file_size(&source_path)?;

    insert_backup_job(
        state,
        "restore",
        &source_path,
        "completed",
        None,
        Some(file_size_bytes),
    )
}

pub fn list_backup_jobs(state: &AppState) -> Result<Vec<BackupJob>, AppError> {
    let conn = state.db().open()?;
    let mut stmt = conn.prepare(
        "SELECT id, backup_type, path, status, error_message, file_size_bytes, created_at, completed_at
         FROM backup_jobs
         ORDER BY created_at DESC, id DESC",
    )?;
    let rows = stmt.query_map([], backup_job_from_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn query_latest_backup_job(
    state: &AppState,
    where_clause: &str,
) -> Result<Option<BackupJob>, AppError> {
    let conn = state.db().open()?;
    let sql = format!(
        "SELECT id, backup_type, path, status, error_message, file_size_bytes, created_at, completed_at
         FROM backup_jobs
         WHERE {where_clause}
         ORDER BY created_at DESC, id DESC
         LIMIT 1"
    );

    conn.query_row(&sql, [], backup_job_from_row)
        .optional()
        .map_err(Into::into)
}

fn insert_backup_job(
    state: &AppState,
    backup_type: &str,
    path: &Path,
    status: &str,
    error_message: Option<&str>,
    file_size_bytes: Option<i64>,
) -> Result<BackupJob, AppError> {
    let mut conn = state.db().open()?;
    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO backup_jobs (
            backup_type,
            path,
            status,
            error_message,
            file_size_bytes,
            created_at,
            completed_at
         )
         VALUES (
            ?1,
            ?2,
            ?3,
            ?4,
            ?5,
            datetime('now'),
            CASE WHEN ?3 = 'completed' THEN datetime('now') ELSE NULL END
         )",
        params![
            backup_type,
            path.display().to_string(),
            status,
            error_message,
            file_size_bytes
        ],
    )?;
    let job_id = tx.last_insert_rowid();
    let job = tx.query_row(
        "SELECT id, backup_type, path, status, error_message, file_size_bytes, created_at, completed_at
         FROM backup_jobs
         WHERE id = ?1",
        params![job_id],
        backup_job_from_row,
    )?;

    tx.commit()?;
    Ok(job)
}

fn backup_job_from_row(row: &Row<'_>) -> rusqlite::Result<BackupJob> {
    Ok(BackupJob {
        id: row.get(0)?,
        backup_type: row.get(1)?,
        path: row.get(2)?,
        status: row.get(3)?,
        error_message: row.get(4)?,
        file_size_bytes: row.get(5)?,
        created_at: row.get(6)?,
        completed_at: row.get(7)?,
    })
}

fn default_backup_settings(state: &AppState) -> BackupSettings {
    let backup_folder = state
        .db()
        .path()
        .parent()
        .map(|parent| parent.join("backups"))
        .unwrap_or_else(|| PathBuf::from("backups"))
        .display()
        .to_string();

    BackupSettings {
        backup_folder,
        automatic_backup_enabled: false,
    }
}

fn build_backup_path(folder: &str, backup_type: &str) -> Result<PathBuf, AppError> {
    let token = timestamp_token()?;
    Ok(Path::new(folder).join(format!("vantumpos-{backup_type}-{token}.sqlite3")))
}

fn timestamp_token() -> Result<String, AppError> {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| AppError::InvalidState(format!("Vreme sistema nije ispravno: {error}")))?;

    Ok(elapsed.as_millis().to_string())
}

fn checked_file_size(path: &Path) -> Result<i64, AppError> {
    let len = std::fs::metadata(path)?.len();
    i64::try_from(len)
        .map_err(|_| AppError::InvalidState("Backup fajl je prevelik za evidenciju.".to_string()))
}

fn validate_backup_type(backup_type: &str) -> Result<(), AppError> {
    if matches!(backup_type, "manual" | "automatic" | "pre_restore") {
        Ok(())
    } else {
        Err(AppError::validation(
            "Tip backup operacije nije ispravan.",
            serde_json::json!({ "field": "backupType" }),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

    fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            test(&state);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    fn test_backup_dir(test_name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(test_name);
        std::fs::create_dir_all(&path).expect("backup directory should be created");
        path
    }

    #[test]
    fn backup_settings_round_trip_drives_status() {
        with_state("backup_settings_round_trip_drives_status", |state| {
            let folder = test_backup_dir("vantumpos-backup-settings");

            let saved = save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: true,
                },
            )
            .expect("backup settings should save");

            assert!(saved.automatic_backup_enabled);

            let status = load_backup_status(state).expect("backup status should load");

            assert_eq!(status.backup_folder, folder.display().to_string());
            assert!(status.automatic_backup_enabled);
        });
    }

    #[test]
    fn manual_backup_creates_sqlite_copy_and_success_job() {
        with_state(
            "manual_backup_creates_sqlite_copy_and_success_job",
            |state| {
                let folder = test_backup_dir("vantumpos-manual-backup");

                let job = create_backup(
                    state,
                    CreateBackupRequest {
                        backup_folder: Some(folder.display().to_string()),
                        backup_type: None,
                    },
                )
                .expect("manual backup should succeed");

                assert_eq!(job.status, "completed");
                assert!(std::path::Path::new(&job.path).exists());
                assert!(job.file_size_bytes.expect("backup should record size") > 0);
            },
        );
    }

    #[test]
    fn restore_rejects_missing_file_without_recording_success() {
        with_state(
            "restore_rejects_missing_file_without_recording_success",
            |state| {
                let missing = std::env::temp_dir().join("vantumpos-missing-backup.sqlite3");

                let error = restore_backup(
                    state,
                    RestoreBackupRequest {
                        path: missing.display().to_string(),
                        confirmation_text: "VRATI PODATKE".to_string(),
                    },
                )
                .expect_err("missing restore file should be rejected");

                let command_error: crate::app_error::CommandError = error.into();

                assert_eq!(command_error.code, "not_found");
            },
        );
    }

    #[test]
    fn restore_requires_confirmation_text() {
        with_state("restore_requires_confirmation_text", |state| {
            let error = restore_backup(
                state,
                RestoreBackupRequest {
                    path: "C:/backup.sqlite3".to_string(),
                    confirmation_text: "vrati".to_string(),
                },
            )
            .expect_err("restore should require exact confirmation");

            let command_error: crate::app_error::CommandError = error.into();

            assert_eq!(command_error.code, "validation_error");
        });
    }

    #[test]
    fn backup_status_is_stale_when_last_success_is_older_than_threshold() {
        with_state(
            "backup_status_is_stale_when_last_success_is_older_than_threshold",
            |state| {
                let conn = state.db().open().expect("database should open");
                conn.execute(
                    "INSERT INTO backup_jobs (
                        backup_type, path, status, error_message,
                        file_size_bytes, created_at, completed_at
                     )
                     VALUES (
                        'manual', '/tmp/vantumpos-old.sqlite3', 'completed', NULL, 1024,
                        datetime('now', '-48 hours'), datetime('now', '-48 hours')
                     )",
                    [],
                )
                .expect("aged backup job should insert");

                let status = load_backup_status(state).expect("backup status should load");

                assert!(status.last_successful_backup.is_some());
                assert!(status.stale);
            },
        );
    }

    #[test]
    fn backup_status_is_fresh_when_recent_success_exists() {
        with_state(
            "backup_status_is_fresh_when_recent_success_exists",
            |state| {
                let conn = state.db().open().expect("database should open");
                conn.execute(
                    "INSERT INTO backup_jobs (
                        backup_type, path, status, error_message,
                        file_size_bytes, created_at, completed_at
                     )
                     VALUES (
                        'manual', '/tmp/vantumpos-recent.sqlite3', 'completed', NULL, 1024,
                        datetime('now', '-1 hours'), datetime('now', '-1 hours')
                     )",
                    [],
                )
                .expect("recent backup job should insert");

                let status = load_backup_status(state).expect("backup status should load");

                assert!(status.last_successful_backup.is_some());
                assert!(!status.stale);
            },
        );
    }
}
