use rusqlite::{params, OptionalExtension};
use serde::Deserialize;
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::clock::utc_now;
use crate::security::hash_credential;
use crate::state::AppState;

use super::auth::{active_user_by_id, user_account_from_row, UserAccount};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveUserRequest {
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub active: bool,
    pub pin: Option<String>,
    pub password: Option<String>,
}

#[tauri::command]
pub fn users_list(state: State<'_, AppState>) -> Result<Vec<UserAccount>, CommandError> {
    require_admin(state.inner())?;
    list_users(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn users_create(
    state: State<'_, AppState>,
    request: SaveUserRequest,
) -> Result<UserAccount, CommandError> {
    require_admin(state.inner())?;
    create_user(state.inner(), request).map_err(Into::into)
}

#[tauri::command]
pub fn users_update(
    state: State<'_, AppState>,
    id: i64,
    request: SaveUserRequest,
) -> Result<UserAccount, CommandError> {
    require_admin(state.inner())?;
    update_user(state.inner(), id, request).map_err(Into::into)
}

#[tauri::command]
pub fn users_deactivate(state: State<'_, AppState>, id: i64) -> Result<(), CommandError> {
    require_admin(state.inner())?;
    deactivate_user(state.inner(), id).map_err(Into::into)
}

pub fn list_users(state: &AppState) -> Result<Vec<UserAccount>, AppError> {
    let conn = state.db().open()?;
    let mut stmt = conn.prepare(
        "SELECT id, username, display_name, role, active, created_at, updated_at, last_login_at
         FROM users
         ORDER BY active DESC, display_name ASC, username ASC",
    )?;
    let rows = stmt.query_map([], user_account_from_row)?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

pub fn create_user(state: &AppState, request: SaveUserRequest) -> Result<UserAccount, AppError> {
    let normalized = normalize_request(request)?;

    if normalized.pin.is_none() && normalized.password.is_none() {
        return Err(AppError::validation(
            "Unesite PIN ili lozinku za novog korisnika.",
            serde_json::json!({ "field": "pin" }),
        ));
    }

    ensure_unique_username(state, &normalized.username, None)?;

    let now = utc_now()?;
    let pin_hash = normalized.pin.as_deref().map(hash_credential).transpose()?;
    let password_hash = normalized
        .password
        .as_deref()
        .map(hash_credential)
        .transpose()?;
    let active = i64::from(normalized.active);
    let mut conn = state.db().open()?;
    let tx = conn.transaction()?;

    tx.execute(
        "INSERT INTO users (
            username,
            display_name,
            role,
            pin_hash,
            password_hash,
            active,
            created_at,
            updated_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
        params![
            normalized.username,
            normalized.display_name,
            normalized.role,
            pin_hash,
            password_hash,
            active,
            now
        ],
    )?;
    let user_id = tx.last_insert_rowid();
    tx.commit()?;

    user_by_id(state, user_id)
}

pub fn update_user(
    state: &AppState,
    user_id: i64,
    request: SaveUserRequest,
) -> Result<UserAccount, AppError> {
    let normalized = normalize_request(request)?;
    ensure_unique_username(state, &normalized.username, Some(user_id))?;

    let existing = user_hashes(state, user_id)?
        .ok_or_else(|| AppError::not_found("Korisnik nije pronadjen."))?;
    let pin_hash = match normalized.pin.as_deref() {
        Some(pin) => Some(hash_credential(pin)?),
        None => existing.pin_hash,
    };
    let password_hash = match normalized.password.as_deref() {
        Some(password) => Some(hash_credential(password)?),
        None => existing.password_hash,
    };

    if pin_hash.is_none() && password_hash.is_none() {
        return Err(AppError::validation(
            "Korisnik mora imati PIN ili lozinku.",
            serde_json::json!({ "field": "pin" }),
        ));
    }

    let now = utc_now()?;
    let active = i64::from(normalized.active);
    let conn = state.db().open()?;
    let changed = conn.execute(
        "UPDATE users
         SET username = ?1,
             display_name = ?2,
             role = ?3,
             active = ?4,
             pin_hash = ?5,
             password_hash = ?6,
             updated_at = ?7
         WHERE id = ?8",
        params![
            normalized.username,
            normalized.display_name,
            normalized.role,
            active,
            pin_hash,
            password_hash,
            now,
            user_id
        ],
    )?;

    if changed == 0 {
        return Err(AppError::not_found("Korisnik nije pronadjen."));
    }

    user_by_id(state, user_id)
}

pub fn deactivate_user(state: &AppState, user_id: i64) -> Result<(), AppError> {
    let now = utc_now()?;
    let conn = state.db().open()?;
    let changed = conn.execute(
        "UPDATE users SET active = 0, updated_at = ?1 WHERE id = ?2",
        params![now, user_id],
    )?;

    if changed == 0 {
        return Err(AppError::not_found("Korisnik nije pronadjen."));
    }

    Ok(())
}

fn require_admin(state: &AppState) -> Result<(), CommandError> {
    let Some(user_id) = state.session_user_id().map_err(CommandError::from)? else {
        return Err(CommandError::new("unauthorized", "Prijavite se za rad."));
    };

    let Some(user) = active_user_by_id(state, user_id).map_err(CommandError::from)? else {
        state.clear_session().map_err(CommandError::from)?;
        return Err(CommandError::new("unauthorized", "Prijavite se za rad."));
    };

    if user.role != "admin" {
        return Err(CommandError::new(
            "unauthorized",
            "Samo administrator moze da uredjuje korisnike.",
        ));
    }

    Ok(())
}

fn user_by_id(state: &AppState, user_id: i64) -> Result<UserAccount, AppError> {
    let conn = state.db().open()?;

    conn.query_row(
        "SELECT id, username, display_name, role, active, created_at, updated_at, last_login_at
         FROM users
         WHERE id = ?1",
        params![user_id],
        user_account_from_row,
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("Korisnik nije pronadjen."))
}

fn ensure_unique_username(
    state: &AppState,
    username: &str,
    current_user_id: Option<i64>,
) -> Result<(), AppError> {
    let conn = state.db().open()?;
    let duplicate_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM users WHERE username = ?1 AND (?2 IS NULL OR id != ?2)",
        params![username, current_user_id],
        |row| row.get(0),
    )?;

    if duplicate_count > 0 {
        return Err(AppError::validation(
            "Korisnicko ime vec postoji.",
            serde_json::json!({ "field": "username" }),
        ));
    }

    Ok(())
}

struct UserHashes {
    pin_hash: Option<String>,
    password_hash: Option<String>,
}

fn user_hashes(state: &AppState, user_id: i64) -> Result<Option<UserHashes>, AppError> {
    let conn = state.db().open()?;

    conn.query_row(
        "SELECT pin_hash, password_hash FROM users WHERE id = ?1",
        params![user_id],
        |row| {
            Ok(UserHashes {
                pin_hash: row.get(0)?,
                password_hash: row.get(1)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn normalize_request(request: SaveUserRequest) -> Result<SaveUserRequest, AppError> {
    let username = request.username.trim().to_string();
    let display_name = request.display_name.trim().to_string();
    let role = request.role.trim().to_string();

    if username.is_empty() {
        return Err(AppError::validation(
            "Korisnicko ime je obavezno.",
            serde_json::json!({ "field": "username" }),
        ));
    }

    if display_name.is_empty() {
        return Err(AppError::validation(
            "Ime za prikaz je obavezno.",
            serde_json::json!({ "field": "displayName" }),
        ));
    }

    if role != "admin" && role != "cashier" {
        return Err(AppError::validation(
            "Uloga nije ispravna.",
            serde_json::json!({ "field": "role" }),
        ));
    }

    Ok(SaveUserRequest {
        username,
        display_name,
        role,
        active: request.active,
        pin: normalized_secret(request.pin),
        password: normalized_secret(request.password),
    })
}

fn normalized_secret(value: Option<String>) -> Option<String> {
    value.and_then(|secret| {
        let trimmed = secret.trim().to_string();
        (!trimmed.is_empty()).then_some(trimmed)
    })
}
