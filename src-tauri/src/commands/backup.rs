use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, DatabaseName, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::commands::backup_crypto;
use crate::commands::settings::{
    load_json_setting, save_json_setting, BACKUP_ENCRYPTION_KEY, BACKUP_SETTINGS_KEY,
};
use crate::state::AppState;

const RESTORE_CONFIRMATION: &str = "VRATI PODATKE";
const BACKUP_STALE_AFTER_HOURS: i64 = 24;

/// How often the background timer (started in `lib.rs::setup`) re-checks
/// whether an automatic backup is due.
pub const AUTO_BACKUP_INTERVAL: Duration = Duration::from_secs(6 * 3600);

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
    pub encryption_configured: bool,
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
    #[serde(default)]
    pub passphrase: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackupEncryption {
    salt: Vec<u8>,
    wrap_nonce: Vec<u8>,
    wrapped_data_key: Vec<u8>,
    data_key: Vec<u8>,
}

fn load_backup_encryption(state: &AppState) -> Result<Option<BackupEncryption>, AppError> {
    load_json_setting::<Option<BackupEncryption>>(state, BACKUP_ENCRYPTION_KEY, None)
}

pub fn set_backup_passphrase(state: &AppState, passphrase: &str) -> Result<(), AppError> {
    super::auth::require_admin(state)?;
    if passphrase.trim().len() < 8 {
        return Err(AppError::validation(
            "Lozinka za šifrovanje mora imati najmanje 8 znakova.",
            serde_json::json!({ "field": "passphrase" }),
        ));
    }
    let km = backup_crypto::derive_new_key_material(passphrase)?;
    let stored = BackupEncryption {
        salt: km.salt,
        wrap_nonce: km.wrap_nonce,
        wrapped_data_key: km.wrapped_data_key,
        data_key: km.data_key.to_vec(),
    };
    save_json_setting(state, BACKUP_ENCRYPTION_KEY, &stored)
}

#[tauri::command]
pub fn backup_set_passphrase(
    state: State<'_, AppState>,
    passphrase: String,
) -> Result<(), CommandError> {
    set_backup_passphrase(state.inner(), &passphrase).map_err(Into::into)
}

pub fn load_backup_settings(state: &AppState) -> Result<BackupSettings, AppError> {
    load_json_setting(state, BACKUP_SETTINGS_KEY, default_backup_settings(state))
}

pub fn save_backup_settings(
    state: &AppState,
    request: BackupSettingsRequest,
) -> Result<BackupSettings, AppError> {
    super::auth::require_admin(state)?;

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
    let encryption_configured = load_backup_encryption(state)?.is_some();

    Ok(BackupStatus {
        backup_folder: settings.backup_folder,
        automatic_backup_enabled: settings.automatic_backup_enabled,
        stale,
        encryption_configured,
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
    super::auth::require_admin(state)?;

    let backup_type = request
        .backup_type
        .as_deref()
        .unwrap_or("manual")
        .trim()
        .to_string();
    validate_backup_type(&backup_type)?;

    perform_backup(state, request.backup_folder.as_deref(), &backup_type)
}

/// Resolves the backup folder, copies the live database into it, and records
/// the resulting job. Deliberately auth-free: callers (the `backup_create`
/// command, restore's pre-restore snapshot, go-live reset's safety backup)
/// are each responsible for their own admin gate before reaching here. On a
/// copy failure a `"failed"` backup_job is recorded before the error is
/// propagated, so the "last failed backup" status has something to surface.
pub fn perform_backup(
    state: &AppState,
    backup_folder: Option<&str>,
    backup_type: &str,
) -> Result<BackupJob, AppError> {
    let mut settings = load_backup_settings(state)?;
    let backup_folder = backup_folder
        .map(str::trim)
        .filter(|folder| !folder.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| settings.backup_folder.clone());

    let backup_path = build_backup_path(&backup_folder, backup_type)?;
    std::fs::create_dir_all(&backup_folder)?;

    let encryption = load_backup_encryption(state)?;

    // Always snapshot to a temp plaintext file first via SQLite's online backup.
    let temp_path = backup_path.with_extension("sqlite3.tmp");
    let source = state.db().open()?;
    source
        .backup(DatabaseName::Main, &temp_path, None)
        .map_err(|source| {
            let message = format!("Backup nije uspeo: {source}");
            let _ = insert_backup_job(
                state,
                backup_type,
                &backup_path,
                "failed",
                Some(&message),
                None,
            );
            AppError::BackupFailed(message)
        })?;

    let final_path = if let Some(enc) = encryption {
        let plaintext = std::fs::read(&temp_path)?;
        let km = backup_crypto::BackupKeyMaterial {
            salt: enc.salt,
            wrap_nonce: enc.wrap_nonce,
            wrapped_data_key: enc.wrapped_data_key,
            data_key: to_key_array(&enc.data_key)?,
        };
        let ciphertext = backup_crypto::encrypt_snapshot(&plaintext, &km)?;
        let encrypted_path = backup_path.with_extension("vpbk");
        std::fs::write(&encrypted_path, &ciphertext)?;
        let _ = std::fs::remove_file(&temp_path);
        encrypted_path
    } else {
        std::fs::rename(&temp_path, &backup_path)?;
        backup_path
    };

    let backup_path = final_path;
    let file_size_bytes = checked_file_size(&backup_path)?;
    settings.backup_folder = backup_folder;
    save_json_setting(state, BACKUP_SETTINGS_KEY, &settings)?;

    insert_backup_job(
        state,
        backup_type,
        &backup_path,
        "completed",
        None,
        Some(file_size_bytes),
    )
}

/// Best-effort automatic backup check, run once at app launch and then on a
/// recurring timer (see `lib.rs::setup`). Deliberately auth-free like
/// `perform_backup` — it runs unattended in the background, not on behalf of
/// a signed-in user. Does nothing unless automatic backups are enabled in
/// settings AND the last successful backup is stale; a failed attempt is
/// still recorded as a `"failed"` backup_job by `perform_backup` itself, so
/// callers are expected to ignore the `Err` here rather than let it block
/// startup.
pub fn auto_backup_if_due(state: &AppState) -> Result<(), AppError> {
    let settings = load_backup_settings(state)?;
    if !settings.automatic_backup_enabled || !is_backup_stale(state)? {
        return Ok(());
    }

    perform_backup(state, None, "automatic")?;
    Ok(())
}

pub fn restore_backup(
    state: &AppState,
    request: RestoreBackupRequest,
) -> Result<BackupJob, AppError> {
    let acting = super::auth::require_admin(state)?;

    if request.confirmation_text.trim() != RESTORE_CONFIRMATION {
        return Err(AppError::validation(
            "Potvrdite restore unosom teksta VRATI PODATKE.",
            serde_json::json!({ "field": "confirmationText" }),
        ));
    }

    let source_path = PathBuf::from(request.path.trim());
    if !source_path.is_file() {
        return Err(AppError::not_found("Backup fajl nije pronađen."));
    }

    let _pre_restore = create_backup(
        state,
        CreateBackupRequest {
            backup_folder: None,
            backup_type: Some("pre_restore".to_string()),
        },
    )?;

    let file_bytes = std::fs::read(&source_path)?;
    if backup_crypto::is_encrypted(&file_bytes) {
        let local_key = load_backup_encryption(state)?
            .map(|enc| to_key_array(&enc.data_key))
            .transpose()?;
        let plaintext = backup_crypto::decrypt_snapshot(
            &file_bytes,
            local_key.as_ref(),
            request.passphrase.as_deref(),
        )?;
        let temp_plain = source_path.with_extension("restore.tmp");
        std::fs::write(&temp_plain, &plaintext)?;
        {
            let mut conn = state.db().open()?;
            conn.restore(
                DatabaseName::Main,
                &temp_plain,
                Option::<fn(rusqlite::backup::Progress)>::None,
            )
            .map_err(|source| AppError::BackupFailed(format!("Restore nije uspeo: {source}")))?;
        }
        let _ = std::fs::remove_file(&temp_plain);
    } else {
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

    {
        let conn = state.db().open()?;
        let detail =
            serde_json::json!({ "source_path": source_path.display().to_string() }).to_string();
        insert_compliance_event(&conn, "backup_restored", &detail, Some(acting.id))?;
    }

    insert_backup_job(
        state,
        "restore",
        &source_path,
        "completed",
        None,
        Some(file_size_bytes),
    )
}

const RESET_CONFIRMATION: &str = "OBRISI PODATKE";

/// Wipes practice/trading data (sales, movements, shifts) and resets the receipt
/// counter so a shop can go live on a clean slate, keeping catalog/users/settings.
/// Takes a safety backup first and requires exact typed confirmation.
pub fn reset_trading_data(state: &AppState, confirmation_text: &str) -> Result<(), AppError> {
    let acting = super::auth::require_admin(state)?;

    if confirmation_text.trim() != RESET_CONFIRMATION {
        return Err(AppError::validation(
            "Potvrdite brisanje unosom teksta OBRISI PODATKE.",
            serde_json::json!({ "field": "confirmationText" }),
        ));
    }

    let _safety_backup = create_backup(
        state,
        CreateBackupRequest {
            backup_folder: None,
            backup_type: Some("automatic".to_string()),
        },
    )?;

    let mut conn = state.db().open()?;
    let tx = conn.transaction()?;
    // sale_items + sale_payments cascade via ON DELETE CASCADE.
    tx.execute("DELETE FROM sales", [])?;
    // inventory_movements has no FK cascade — delete explicitly.
    tx.execute("DELETE FROM inventory_movements", [])?;
    // shifts must go AFTER sales (sales.shift_id -> shifts).
    tx.execute("DELETE FROM shifts", [])?;
    // The KEP (evidencija prometa) ledger is append-only (PEP čl. 14); the
    // go-live reset is its one sanctioned wipe — practice postings were never a
    // real promet and must not carry into the live book.
    tx.execute("DELETE FROM kep_entries", [])?;
    // Closures gate the ledger (čl. 18). The go-live reset is the one sanctioned
    // escape from an irreversible close: practice years must not stay frozen.
    tx.execute("DELETE FROM kep_closures", [])?;
    // The kalkulacija (SW-9b) is the isprava behind a receipt zaduženje and is
    // numbered per book_year like the KEP it feeds — it must restart with it, or
    // the first real isprava lands at N+1 in a book of practice documents.
    tx.execute("DELETE FROM kalkulacije", [])?;
    tx.execute(
        "UPDATE inventory_balances SET quantity_milli = 0, updated_at = datetime('now')",
        [],
    )?;
    // A deklaracija check (SW-15) attests that a physical label was read on
    // goods that were physically present. Practice stock was zeroed a line
    // above, so a surviving stamp would vouch for a label nobody ever saw on
    // the live goods — the check must be re-performed on real receipt.
    // The AML provenance columns (aml_cash_minor / aml_rate_* / aml_ack_reason)
    // live on `sales` and go with the wipe above; no separate clear is needed.
    tx.execute(
        "UPDATE products SET declaration_checked_at = NULL, declaration_checked_by = NULL",
        [],
    )?;
    // Reset the receipt counter, preserving the shop's prefix.
    tx.execute(
        "UPDATE settings
         SET value_json = json_set(value_json, '$.nextSequenceNumber', 1),
             updated_at = datetime('now')
         WHERE key = 'receipt_numbering'",
        [],
    )?;
    // Campaigns hold a MATERIALIZED copy of the prethodna cena in
    // campaign_items.prethodna_cena_minor, so wiping price_history alone would
    // leave a practice-derived anchor intact and drive a real sniženje from
    // prices no consumer was ever offered. A practice rasprodaja left active
    // would also keep blocking goods receipt (čl. 37 st. 7) on real stock.
    // Delete rather than re-snapshot: rewriting an activated campaign's anchor
    // is exactly what čl. 37 st. 5 forbids. campaign_items cascades.
    tx.execute("DELETE FROM campaigns", [])?;
    // Price history is normally append-only and retained ~5 years. The go-live
    // reset is the one exception: practice prices were never offered to a
    // consumer, so leaving them in would let training data drive a real
    // prethodna cena. Clear and re-seed from the surviving catalog.
    tx.execute("DELETE FROM price_history", [])?;
    tx.execute(
        "INSERT INTO price_history (product_id, effective_from, price_minor, source, user_id, created_at)
         SELECT id,
                strftime('%Y-%m-%dT%H:%M:%SZ', 'now'),
                sale_price_minor,
                'seed',
                NULL,
                strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
         FROM products
         WHERE active = 1",
        [],
    )?;
    // Deliberately untouched: the `shop_profile` and `eur_rate` settings keys and
    // the `non_working_days` table. Those are configuration the operator answered
    // once, not practice trading data — wiping them would silently drop the shop
    // back to `pravna_forma = None` and re-seed praznici the admin had deleted.
    let detail = serde_json::json!({
        "note": "Go-live reset (SW-3).",
        // §2 Q4: the general 10-year floor is ZPPPA čl. 114ž (apsolutna
        // zastarelost) plus ZoRač čl. 28 st. 4 (dnevnik i glavna knjiga).
        // ZPDV čl. 47 supplies no general period — cite it only for the
        // čl. 32 objekti i ulaganja limb.
        "retention": "10y (ZoRač čl. 28 st. 4; ZPPPA čl. 114ž)",
    })
    .to_string();
    insert_compliance_event(&tx, "trading_data_reset", &detail, Some(acting.id))?;
    tx.commit()?;
    Ok(())
}

#[tauri::command]
pub fn backup_reset_trading_data(
    state: State<'_, AppState>,
    confirmation_text: String,
) -> Result<(), CommandError> {
    reset_trading_data(state.inner(), &confirmation_text).map_err(Into::into)
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

/// Appends an audit row to the never-deleted `compliance_log`. Takes a live
/// connection/transaction so callers can write the event inside the same
/// atomic unit as the operation it records (e.g. the reset wipe).
fn insert_compliance_event(
    conn: &Connection,
    event_type: &str,
    detail_json: &str,
    user_id: Option<i64>,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO compliance_log (event_type, detail_json, user_id, created_at)
         VALUES (?1, ?2, ?3, datetime('now'))",
        params![event_type, detail_json, user_id],
    )?;
    Ok(())
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

fn to_key_array(bytes: &[u8]) -> Result<[u8; 32], AppError> {
    <[u8; 32]>::try_from(bytes)
        .map_err(|_| AppError::InvalidState("Ključ šifrovanja je oštećen.".to_string()))
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
    use crate::cash_deposit::seed_default_non_working_days;
    use crate::commands::settings::{
        load_shop_profile, save_shop_profile, PravnaForma, ShopProfileRequest,
    };
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

    #[test]
    fn backup_settings_round_trip_drives_status() {
        with_state("backup_settings_round_trip_drives_status", |state| {
            sign_in_admin(state);

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

    fn seed_trading_data(state: &AppState) {
        let conn = state.db().open().expect("database should open");
        conn.execute_batch(
            r#"
INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
    VALUES (1, 'PDV 20', 2000, '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z');
INSERT INTO products (id, name, sku, unit_of_measure, sale_price_minor, purchase_price_minor, tax_rate_id, minimum_stock_milli, active, created_at, updated_at)
    VALUES (1, 'Mleko 1 l', 'MLEKO-1L', 'kom', 12000, 9000, 1, 0, 1, '2026-06-18T10:00:00Z', '2026-06-18T10:00:00Z');
INSERT INTO inventory_balances (product_id, quantity_milli, updated_at) VALUES (1, 3000, '2026-06-18T10:00:00Z');
INSERT INTO shifts (id, user_id, opened_at, opening_cash_minor, expected_cash_minor, status, created_at, updated_at)
    VALUES (1, 1, '2026-06-18T09:00:00Z', 0, 0, 'open', '2026-06-18T09:00:00Z', '2026-06-18T09:00:00Z');
INSERT INTO sales (id, local_receipt_number, shift_id, cashier_id, status, fiscal_status, subtotal_minor, discount_minor, tax_minor, total_minor, created_at, updated_at)
    VALUES (1, 'VP-000004', 1, 1, 'completed', 'not_fiscalized', 12000, 0, 2000, 12000, '2026-06-18T09:15:00Z', '2026-06-18T09:15:00Z');
INSERT INTO sale_items (id, sale_id, product_id, product_name, product_sku, quantity_milli, unit_price_minor, discount_minor, tax_rate_basis_points, tax_minor, total_minor)
    VALUES (1, 1, 1, 'Mleko 1 l', 'MLEKO-1L', 1000, 12000, 0, 2000, 2000, 12000);
INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at) VALUES (1, 'cash', 12000, '2026-06-18T09:15:00Z');
INSERT INTO inventory_movements (product_id, movement_type, quantity_milli, reason, created_at) VALUES (1, 'sale', -1000, 'Prodaja', '2026-06-18T09:15:00Z');
INSERT INTO settings (key, value_json, updated_at)
    VALUES ('receipt_numbering', '{"prefix":"VP-","nextSequenceNumber":5,"resetPolicy":"none"}', '2026-06-18T10:00:00Z');
"#,
        )
        .expect("trading data should seed");
    }

    /// A campaign built while the shop was practising, with an anchor derived
    /// from practice prices and an active rasprodaja blocking goods receipt.
    fn seed_practice_campaign(state: &AppState) {
        let conn = state.db().open().expect("database should open");
        conn.execute_batch(
            r#"
INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode,
                       rasprodaja_ground, separation_attested, activated_at, created_at, updated_at)
    VALUES (1, 'rasprodaja', 'active', '2026-06-20T00:00:00Z', NULL, 'two_prices',
            'prestanak_prodaje_robe', 1, '2026-06-20T08:00:00Z', '2026-06-19T00:00:00Z', '2026-06-20T08:00:00Z');
INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor, prethodna_cena_minor,
                            anchor_status, anchor_window_days, pre_campaign_price_minor)
    VALUES (1, 1, 9000, 12000, 'computed', 30, 12000);
"#,
        )
        .expect("practice campaign should seed");
    }

    #[test]
    fn reset_trading_data_clears_practice_campaigns() {
        with_state("reset_clears_campaigns", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-reset-campaigns");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");
            seed_trading_data(state);
            seed_practice_campaign(state);

            reset_trading_data(state, "OBRISI PODATKE").expect("reset should succeed");

            // A practice campaign carries a materialized anchor computed from
            // prices no consumer was ever offered. Wiping price_history without
            // it would leave that figure driving a real sniženje — the exact
            // harm the reset exists to prevent.
            assert_eq!(
                count(state, "campaigns"),
                0,
                "practice campaigns must not survive go-live"
            );
            assert_eq!(
                count(state, "campaign_items"),
                0,
                "campaign_items must cascade with their campaign"
            );
            // The catalog survives, so the re-seeded price log still has its row.
            assert_eq!(count(state, "products"), 1);
            assert_eq!(count(state, "price_history"), 1);
        });
    }

    fn count(state: &AppState, table: &str) -> i64 {
        state
            .db()
            .open()
            .expect("database should open")
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .expect("count should query")
    }

    #[test]
    fn reset_trading_data_wipes_transactions_keeps_catalog() {
        with_state("reset_trading_data_wipes", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-reset-backup");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");
            seed_trading_data(state);

            reset_trading_data(state, "OBRISI PODATKE").expect("reset should succeed");

            assert_eq!(count(state, "sales"), 0);
            assert_eq!(count(state, "sale_items"), 0);
            assert_eq!(count(state, "sale_payments"), 0);
            assert_eq!(count(state, "inventory_movements"), 0);
            assert_eq!(count(state, "shifts"), 0);

            let conn = state.db().open().expect("database should open");
            let balance: i64 = conn
                .query_row(
                    "SELECT quantity_milli FROM inventory_balances WHERE product_id = 1",
                    [],
                    |row| row.get(0),
                )
                .expect("balance should query");
            assert_eq!(balance, 0);
            let next_seq: i64 = conn
                .query_row(
                    "SELECT json_extract(value_json, '$.nextSequenceNumber') FROM settings WHERE key = 'receipt_numbering'",
                    [],
                    |row| row.get(0),
                )
                .expect("receipt setting should query");
            assert_eq!(next_seq, 1);
            let prefix: String = conn
                .query_row(
                    "SELECT json_extract(value_json, '$.prefix') FROM settings WHERE key = 'receipt_numbering'",
                    [],
                    |row| row.get(0),
                )
                .expect("prefix should query");
            assert_eq!(prefix, "VP-");

            // Catalog and users are preserved.
            assert_eq!(count(state, "products"), 1);
            assert_eq!(count(state, "tax_rates"), 1);
            assert!(count(state, "users") >= 1);
        });
    }

    #[test]
    fn reset_trading_data_clears_and_reseeds_price_history() {
        with_state("reset_clears_price_history", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-reset-price-history");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");
            seed_trading_data(state);

            // Practice price history: two rows for the seeded product.
            let conn = state.db().open().expect("database should open");
            conn.execute(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-06-01T00:00:00Z', 11000, 'update', '2026-06-01T00:00:00Z'),
                        (1, '2026-06-02T00:00:00Z', 12000, 'update', '2026-06-02T00:00:00Z')",
                [],
            )
            .expect("practice history should insert");
            drop(conn);

            reset_trading_data(state, "OBRISI PODATKE").expect("reset should succeed");

            let conn = state.db().open().expect("database should open");
            // Practice rows are gone; exactly one fresh seed row per active product.
            let (count, source): (i64, String) = conn
                .query_row(
                    "SELECT COUNT(*), COALESCE(MIN(source), '') FROM price_history",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("query");
            assert_eq!(
                count, 1,
                "expected exactly one re-seeded row for the active product"
            );
            assert_eq!(source, "seed");

            let effective_from: String = conn
                .query_row("SELECT effective_from FROM price_history", [], |row| {
                    row.get(0)
                })
                .expect("query");
            assert!(
                effective_from.contains('T') && effective_from.ends_with('Z'),
                "re-seed must be RFC3339, got {effective_from}"
            );
        });
    }

    #[test]
    fn reset_trading_data_clears_kep_entries() {
        with_state("reset_clears_kep_entries", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-reset-kep");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");
            seed_trading_data(state);

            // A practice-era ledger entry: a receipt zaduženje the shop booked
            // while training. It must not survive go-live (čl. 14 append-only
            // is suspended only for this sanctioned reset wipe).
            state
                .db()
                .open()
                .expect("database should open")
                .execute(
                    "INSERT INTO kep_entries (
                        book_year, redni_broj, entry_date, opis,
                        kolona, amount_minor, kind, entry_source, user_id, created_at
                     ) VALUES (
                        2026, 1, '2026-06-18T10:00:00Z', 'Prijem robe',
                        'zaduzenje', 780000, 'receipt', 'auto', 1, '2026-06-18T10:00:00Z'
                     )",
                    [],
                )
                .expect("practice kep entry should seed");
            assert_eq!(count(state, "kep_entries"), 1, "seeded a practice entry");

            reset_trading_data(state, "OBRISI PODATKE").expect("reset should succeed");

            assert_eq!(
                count(state, "kep_entries"),
                0,
                "practice KEP entries must not survive go-live"
            );
        });
    }

    #[test]
    fn reset_trading_data_clears_kep_closures() {
        with_state("reset_clears_kep_closures", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-reset-kep-closures");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");
            seed_trading_data(state);

            // A practice-era year-end close. Closures gate the ledger (čl. 18);
            // the go-live reset is the one sanctioned escape from an irreversible
            // close — practice years must not stay frozen into the live book.
            state
                .db()
                .open()
                .expect("database should open")
                .execute(
                    "INSERT INTO kep_closures
                        (book_year, krajnji_saldo_minor, entry_count, closed_at, closed_by, created_at)
                     VALUES (2026, 546000, 2, '2027-01-05T09:00:00Z', 1, '2027-01-05T09:00:00Z')",
                    [],
                )
                .expect("practice closure should seed");
            assert_eq!(count(state, "kep_closures"), 1, "seeded a practice closure");

            reset_trading_data(state, "OBRISI PODATKE").expect("reset should succeed");

            assert_eq!(
                count(state, "kep_closures"),
                0,
                "go-live reset clears closures"
            );
        });
    }

    #[test]
    fn reset_requires_backup_and_tombstone_together() {
        with_state("reset_requires_backup_and_tombstone", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-reset-invariant");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");
            seed_trading_data(state);

            let backups_before = count(state, "backup_jobs");
            reset_trading_data(state, "OBRISI PODATKE").expect("reset should succeed");

            assert!(
                count(state, "backup_jobs") > backups_before,
                "reset must take a safety backup first"
            );
            assert_eq!(
                count(state, "compliance_log"),
                1,
                "reset must leave a tombstone"
            );
        });
    }

    #[test]
    fn reset_trading_data_writes_compliance_tombstone_that_survives_wipe() {
        with_state("reset_writes_compliance_tombstone", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-reset-tombstone");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");
            seed_trading_data(state);

            reset_trading_data(state, "OBRISI PODATKE").expect("reset should succeed");

            assert_eq!(count(state, "sales"), 0);
            assert_eq!(count(state, "compliance_log"), 1);

            let conn = state.db().open().expect("database should open");
            let (event_type, detail_json): (String, String) = conn
                .query_row(
                    "SELECT event_type, detail_json FROM compliance_log ORDER BY id DESC LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("compliance event should exist");
            assert_eq!(event_type, "trading_data_reset");

            // SW11-SW15-VERIFIED-RULES.md §2 Q4: the general 10-year floor comes
            // from ZPPPA čl. 114ž (apsolutna zastarelost) and ZoRač čl. 28 st. 4
            // (dnevnik i glavna knjiga). ZPDV čl. 47 sets no general period at
            // all — it defers to zastarelost, and its own "najmanje deset
            // godina" limb is object-specific to čl. 32 objekti i ulaganja.
            assert!(
                detail_json.contains("ZoRač čl. 28 st. 4")
                    && detail_json.contains("ZPPPA čl. 114ž"),
                "tombstone must cite the statutes that actually supply the 10-year floor: {detail_json}"
            );
            assert!(
                !detail_json.contains("ZPDV"),
                "ZPDV čl. 47 may only be cited for the čl. 32 objekti/ulaganja limb, never for the general retention floor: {detail_json}"
            );
        });
    }

    fn preduzetnik_profile_request() -> ShopProfileRequest {
        ShopProfileRequest {
            pravna_forma: Some(PravnaForma::Preduzetnik),
            pdv_obveznik: Some(false),
            distance_selling: Some(false),
            lpfr_in_premises: Some(true),
            lpfr_carve_out_internet_only: Some(false),
            lpfr_carve_out_own_used_assets: Some(false),
            esir_elements: Vec::new(),
        }
    }

    /// A practice sale carrying the AML provenance `sales_complete` stamps on
    /// the row (SW-15): the cash leg, the NBS rate it was measured against and
    /// where that rate came from.
    fn seed_sale_with_aml_provenance(state: &AppState) {
        state
            .db()
            .open()
            .expect("database should open")
            .execute(
                "UPDATE sales
                 SET aml_cash_minor = 12000,
                     aml_rate_minor = 11720,
                     aml_rate_date = '2026-07-30',
                     aml_rate_source = 'manual'
                 WHERE id = 1",
                [],
            )
            .expect("aml provenance should seed");
    }

    /// A deklaracija check performed while practising (SW-15): it attests a
    /// physical label on practice stock, so it must not vouch for live goods.
    fn seed_declaration_checked_product(state: &AppState) {
        state
            .db()
            .open()
            .expect("database should open")
            .execute(
                "UPDATE products
                 SET declaration_checked_at = '2026-07-30T10:00:00Z',
                     declaration_checked_by = 1
                 WHERE id = 1",
                [],
            )
            .expect("declaration stamp should seed");
    }

    /// The kalkulacija written by a practice goods receipt (SW-9b): the formal
    /// isprava behind a zaduženje, numbered per book_year.
    fn seed_practice_kalkulacija(state: &AppState) {
        state
            .db()
            .open()
            .expect("database should open")
            .execute_batch(
                r#"
INSERT INTO kalkulacije (redni_broj, book_year, product_id,
                         poslovno_ime, prodajno_mesto, pib,
                         trgovacki_naziv, jedinica_mere, kolicina_milli,
                         nabavna_cena_po_jm_minor, vrednost_po_fakturi_minor, razlika_u_ceni_minor,
                         prodajna_vrednost_bez_pdv_minor, pdv_minor, prodajna_vrednost_sa_pdv_minor,
                         prodajna_cena_po_jm_minor, reference_type, reference_id, created_by, created_at)
    VALUES (1, 2026, 1,
            'Probna radnja', 'Prodavnica 1', '100000000',
            'Mleko 1 l', 'kom', 3000,
            9000, 27000, 9000,
            30000, 6000, 36000,
            12000, 'otpremnica', 1, 1, '2026-06-18T10:00:00Z');
"#,
            )
            .expect("practice kalkulacija should seed");
    }

    /// A practice polog: bank_deposit movements are what the undeposited-cash
    /// aging report (SW-11b) draws down against.
    fn seed_practice_bank_deposit(state: &AppState) {
        state
            .db()
            .open()
            .expect("database should open")
            .execute(
                "INSERT INTO cash_movements (shift_id, movement_type, amount_minor, reason, bank_reference, user_id, created_at)
                 VALUES (1, 'bank_deposit', 12000, 'Probni polog', 'REF-1', 1, '2026-06-18T12:00:00Z')",
                [],
            )
            .expect("practice bank deposit should seed");
    }

    #[test]
    fn go_live_reset_keeps_the_profile_and_holidays_but_clears_trading_compliance_state() {
        with_state("reset_preserves_configuration", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-reset-configuration");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");
            save_shop_profile(state, preduzetnik_profile_request()).expect("profile saves");
            seed_default_non_working_days(state, "2026-07-31T00:00:00Z").expect("holidays seed");
            seed_trading_data(state);
            seed_sale_with_aml_provenance(state);
            seed_declaration_checked_product(state);
            seed_practice_kalkulacija(state);
            seed_practice_bank_deposit(state);

            reset_trading_data(state, "OBRISI PODATKE").expect("reset should succeed");

            let profile = load_shop_profile(state).expect("profile loads");
            assert_eq!(
                profile.pravna_forma,
                Some(PravnaForma::Preduzetnik),
                "the shop profile is configuration, not trading data"
            );

            assert!(
                count(state, "non_working_days") > 0,
                "the holiday table is configuration too"
            );

            let conn = state.db().open().expect("db opens");
            let stamps: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM products
                     WHERE declaration_checked_at IS NOT NULL
                        OR declaration_checked_by IS NOT NULL",
                    [],
                    |row| row.get(0),
                )
                .expect("count");
            assert_eq!(
                stamps, 0,
                "declaration check stamps are trading state and must clear"
            );

            let aml: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sales
                     WHERE aml_cash_minor IS NOT NULL
                        OR aml_rate_minor IS NOT NULL
                        OR aml_rate_date IS NOT NULL
                        OR aml_rate_source IS NOT NULL
                        OR aml_ack_reason IS NOT NULL",
                    [],
                    |row| row.get(0),
                )
                .expect("count");
            assert_eq!(
                aml, 0,
                "AML provenance lives on the sale row and goes with the sales wipe"
            );

            // The kalkulacija is the isprava behind a practice zaduženje, and it
            // is numbered per book_year like the KEP it feeds. Leaving it would
            // number the shop's first real isprava N+1 in a book whose entries
            // 1..N document goods that were never received.
            assert_eq!(
                count(state, "kalkulacije"),
                0,
                "practice kalkulacije must not carry into the live book"
            );

            assert_eq!(
                count(state, "cash_movements"),
                0,
                "deposit-aging state is trading data"
            );

            // The catalog itself is configuration and survives.
            assert_eq!(count(state, "products"), 1);
        });
    }

    #[test]
    fn restore_backup_writes_compliance_event() {
        with_state("restore_writes_compliance_event", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-restore-compliance");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");

            // Make a real backup file to restore from.
            let job = create_backup(
                state,
                CreateBackupRequest {
                    backup_folder: Some(folder.display().to_string()),
                    backup_type: None,
                },
            )
            .expect("backup should succeed");

            restore_backup(
                state,
                RestoreBackupRequest {
                    path: job.path.clone(),
                    confirmation_text: "VRATI PODATKE".to_string(),
                    passphrase: None,
                },
            )
            .expect("restore should succeed");

            assert_eq!(count(state, "compliance_log"), 1);
            let conn = state.db().open().expect("database should open");
            let event_type: String = conn
                .query_row(
                    "SELECT event_type FROM compliance_log ORDER BY id DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .expect("compliance event should exist");
            assert_eq!(event_type, "backup_restored");
        });
    }

    #[test]
    fn reset_trading_data_rejects_wrong_confirmation() {
        with_state("reset_trading_data_wrong_confirmation", |state| {
            sign_in_admin(state);
            seed_trading_data(state);

            let error =
                reset_trading_data(state, "obrisi").expect_err("wrong confirmation should fail");
            assert_eq!(error.code(), "validation_error");
            assert_eq!(count(state, "sales"), 1);
        });
    }

    #[test]
    fn manual_backup_creates_sqlite_copy_and_success_job() {
        with_state(
            "manual_backup_creates_sqlite_copy_and_success_job",
            |state| {
                sign_in_admin(state);

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

    // Unix-only: relies on directory write permission bits to force the
    // `.backup()` copy itself to fail (the folder is created successfully,
    // so `create_dir_all` is not the failure point) without needing admin
    // auth, since `perform_backup` is intentionally auth-free.
    #[cfg(unix)]
    #[test]
    fn perform_backup_records_failed_job_when_copy_fails() {
        use std::os::unix::fs::PermissionsExt;

        with_state(
            "perform_backup_records_failed_job_when_copy_fails",
            |state| {
                let folder = test_backup_dir("vantumpos-perform-backup-unwritable");

                // Directory exists and is readable/executable, so `create_dir_all`
                // is a no-op success — but it is not writable, so SQLite's
                // `.backup()` cannot create the destination file inside it.
                std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o500))
                    .expect("read-only permissions should set");

                let result = perform_backup(state, Some(&folder.display().to_string()), "manual");

                // Restore write access so the temp directory can be cleaned up.
                std::fs::set_permissions(&folder, std::fs::Permissions::from_mode(0o700))
                    .expect("permissions should restore");

                let error = result.expect_err("backup copy should fail into a read-only folder");
                assert_eq!(error.code(), "backup_failed");

                let conn = state.db().open().expect("database should open");
                let status: String = conn
                    .query_row(
                        "SELECT status FROM backup_jobs ORDER BY id DESC LIMIT 1",
                        [],
                        |row| row.get(0),
                    )
                    .expect("failed backup job should be recorded");
                assert_eq!(status, "failed");
            },
        );
    }

    #[test]
    fn restore_rejects_missing_file_without_recording_success() {
        with_state(
            "restore_rejects_missing_file_without_recording_success",
            |state| {
                sign_in_admin(state);

                let missing = std::env::temp_dir().join("vantumpos-missing-backup.sqlite3");

                let error = restore_backup(
                    state,
                    RestoreBackupRequest {
                        path: missing.display().to_string(),
                        confirmation_text: "VRATI PODATKE".to_string(),
                        passphrase: None,
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
            sign_in_admin(state);

            let error = restore_backup(
                state,
                RestoreBackupRequest {
                    path: "C:/backup.sqlite3".to_string(),
                    confirmation_text: "vrati".to_string(),
                    passphrase: None,
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

    #[test]
    fn save_backup_settings_rejected_for_cashier() {
        with_state("save_backup_settings_rejected_for_cashier", |state| {
            sign_in_cashier(state);

            let folder = test_backup_dir("vantumpos-backup-settings-forbidden");

            let error = save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: true,
                },
            )
            .expect_err("cashier should not save backup settings");

            assert_eq!(error.code(), "forbidden");
        });
    }

    #[test]
    fn create_backup_rejected_for_cashier() {
        with_state("create_backup_rejected_for_cashier", |state| {
            sign_in_cashier(state);

            let folder = test_backup_dir("vantumpos-create-backup-forbidden");

            let error = create_backup(
                state,
                CreateBackupRequest {
                    backup_folder: Some(folder.display().to_string()),
                    backup_type: None,
                },
            )
            .expect_err("cashier should not create a backup");

            assert_eq!(error.code(), "forbidden");
        });
    }

    #[test]
    fn auto_backup_if_due_skips_when_automatic_backup_disabled() {
        with_state(
            "auto_backup_if_due_skips_when_automatic_backup_disabled",
            |state| {
                sign_in_admin(state);
                let folder = test_backup_dir("vantumpos-auto-backup-disabled");
                save_backup_settings(
                    state,
                    BackupSettingsRequest {
                        backup_folder: folder.display().to_string(),
                        automatic_backup_enabled: false,
                    },
                )
                .expect("backup settings should save");

                auto_backup_if_due(state).expect("disabled auto backup check should not error");

                assert_eq!(count(state, "backup_jobs"), 0);
            },
        );
    }

    #[test]
    fn auto_backup_if_due_runs_backup_when_enabled_and_stale() {
        with_state(
            "auto_backup_if_due_runs_backup_when_enabled_and_stale",
            |state| {
                sign_in_admin(state);
                let folder = test_backup_dir("vantumpos-auto-backup-enabled-stale");
                save_backup_settings(
                    state,
                    BackupSettingsRequest {
                        backup_folder: folder.display().to_string(),
                        automatic_backup_enabled: true,
                    },
                )
                .expect("backup settings should save");

                // No prior backup_jobs rows exist, so the backup is stale by definition.
                auto_backup_if_due(state).expect("due auto backup should succeed");

                assert_eq!(count(state, "backup_jobs"), 1);
                let conn = state.db().open().expect("database should open");
                let (backup_type, status): (String, String) = conn
                    .query_row(
                        "SELECT backup_type, status FROM backup_jobs ORDER BY id DESC LIMIT 1",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .expect("backup job should be recorded");
                assert_eq!(backup_type, "automatic");
                assert_eq!(status, "completed");
            },
        );
    }

    #[test]
    fn backup_is_encrypted_when_passphrase_set_and_restores_locally() {
        with_state("backup_encrypted_round_trip", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-encrypted-backup");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");

            set_backup_passphrase(state, "tajna-lozinka-123").expect("passphrase should set");

            let job = create_backup(
                state,
                CreateBackupRequest {
                    backup_folder: Some(folder.display().to_string()),
                    backup_type: None,
                },
            )
            .expect("encrypted backup should succeed");

            assert!(
                job.path.ends_with(".vpbk"),
                "encrypted backup must use .vpbk: {}",
                job.path
            );
            let head = std::fs::read(&job.path).expect("read backup");
            assert_eq!(&head[..5], b"VPBK1");

            // Restore the encrypted file on the same machine (local data key path).
            restore_backup(
                state,
                RestoreBackupRequest {
                    path: job.path.clone(),
                    confirmation_text: "VRATI PODATKE".to_string(),
                    passphrase: None,
                },
            )
            .expect("local restore of encrypted backup should succeed");

            let status = load_backup_status(state).expect("status");
            assert!(status.encryption_configured);
        });
    }

    #[test]
    fn legacy_plaintext_backup_still_restores() {
        with_state("legacy_plaintext_restore", |state| {
            sign_in_admin(state);
            let folder = test_backup_dir("vantumpos-legacy-restore");
            save_backup_settings(
                state,
                BackupSettingsRequest {
                    backup_folder: folder.display().to_string(),
                    automatic_backup_enabled: false,
                },
            )
            .expect("backup folder should save");

            // No passphrase set → plaintext .sqlite3 (current behavior).
            let job = create_backup(
                state,
                CreateBackupRequest {
                    backup_folder: Some(folder.display().to_string()),
                    backup_type: None,
                },
            )
            .expect("plaintext backup");
            assert!(job.path.ends_with(".sqlite3"));

            restore_backup(
                state,
                RestoreBackupRequest {
                    path: job.path.clone(),
                    confirmation_text: "VRATI PODATKE".to_string(),
                    passphrase: None,
                },
            )
            .expect("legacy restore should succeed");
        });
    }

    #[test]
    fn restore_backup_rejected_for_cashier() {
        with_state("restore_backup_rejected_for_cashier", |state| {
            sign_in_cashier(state);

            let error = restore_backup(
                state,
                RestoreBackupRequest {
                    path: "C:/backup.sqlite3".to_string(),
                    confirmation_text: "VRATI PODATKE".to_string(),
                    passphrase: None,
                },
            )
            .expect_err("cashier should not restore a backup");

            assert_eq!(error.code(), "forbidden");
        });
    }
}
