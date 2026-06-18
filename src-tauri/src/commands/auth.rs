use rusqlite::{params, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::clock::utc_now;
use crate::security::verify_credential;
use crate::state::AppState;

use super::shifts::{current_shift_for_user, ShiftSummary};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserAccount {
    pub id: i64,
    pub username: String,
    pub display_name: String,
    pub role: String,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
    pub last_login_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub username: String,
    pub credential: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub user: UserAccount,
    pub current_shift: Option<ShiftSummary>,
}

struct CredentialUser {
    account: UserAccount,
    pin_hash: Option<String>,
    password_hash: Option<String>,
}

#[tauri::command]
pub fn auth_get_session(state: State<'_, AppState>) -> Result<Option<AuthSession>, CommandError> {
    get_current_session(state.inner())
}

#[tauri::command]
pub fn auth_login(
    state: State<'_, AppState>,
    request: LoginRequest,
) -> Result<AuthSession, CommandError> {
    login_user(state.inner(), request)
}

#[tauri::command]
pub fn auth_logout(state: State<'_, AppState>) -> Result<(), CommandError> {
    state.inner().clear_session().map_err(Into::into)
}

pub fn get_current_session(state: &AppState) -> Result<Option<AuthSession>, CommandError> {
    let Some(user_id) = state.session_user_id().map_err(CommandError::from)? else {
        return Ok(None);
    };

    let Some(user) = active_user_by_id(state, user_id).map_err(CommandError::from)? else {
        state.clear_session().map_err(CommandError::from)?;
        return Ok(None);
    };

    let current_shift = current_shift_for_user(state, user.id)?;

    Ok(Some(AuthSession {
        user,
        current_shift,
    }))
}

pub fn login_user(state: &AppState, request: LoginRequest) -> Result<AuthSession, CommandError> {
    let username = request.username.trim();
    let credential = request.credential.trim();

    if username.is_empty() || credential.is_empty() {
        return Err(invalid_credentials_error());
    }

    let user = credential_user_by_username(state, username)
        .map_err(CommandError::from)?
        .ok_or_else(invalid_credentials_error)?;

    if !user.account.active || !credential_matches(&user, credential) {
        return Err(invalid_credentials_error());
    }

    let now = utc_now().map_err(CommandError::from)?;
    let conn = state.db().open().map_err(CommandError::from)?;
    conn.execute(
        "UPDATE users SET last_login_at = ?1, updated_at = ?1 WHERE id = ?2",
        params![now, user.account.id],
    )
    .map_err(AppError::from)
    .map_err(CommandError::from)?;

    state
        .set_session_user_id(user.account.id)
        .map_err(CommandError::from)?;

    let user = active_user_by_id(state, user.account.id)
        .map_err(CommandError::from)?
        .ok_or_else(invalid_credentials_error)?;
    let current_shift = current_shift_for_user(state, user.id)?;

    Ok(AuthSession {
        user,
        current_shift,
    })
}

pub(crate) fn user_account_from_row(row: &Row<'_>) -> rusqlite::Result<UserAccount> {
    let active: i64 = row.get(4)?;

    Ok(UserAccount {
        id: row.get(0)?,
        username: row.get(1)?,
        display_name: row.get(2)?,
        role: row.get(3)?,
        active: active == 1,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        last_login_at: row.get(7)?,
    })
}

pub(crate) fn active_user_by_id(
    state: &AppState,
    user_id: i64,
) -> Result<Option<UserAccount>, AppError> {
    let conn = state.db().open()?;

    conn.query_row(
        "SELECT id, username, display_name, role, active, created_at, updated_at, last_login_at
         FROM users
         WHERE id = ?1 AND active = 1",
        params![user_id],
        user_account_from_row,
    )
    .optional()
    .map_err(Into::into)
}

fn credential_user_by_username(
    state: &AppState,
    username: &str,
) -> Result<Option<CredentialUser>, AppError> {
    let conn = state.db().open()?;

    conn.query_row(
        "SELECT
            id,
            username,
            display_name,
            role,
            active,
            created_at,
            updated_at,
            last_login_at,
            pin_hash,
            password_hash
         FROM users
         WHERE username = ?1",
        params![username],
        |row| {
            Ok(CredentialUser {
                account: user_account_from_row(row)?,
                pin_hash: row.get(8)?,
                password_hash: row.get(9)?,
            })
        },
    )
    .optional()
    .map_err(Into::into)
}

fn credential_matches(user: &CredentialUser, credential: &str) -> bool {
    [user.pin_hash.as_deref(), user.password_hash.as_deref()]
        .into_iter()
        .flatten()
        .any(|hash| verify_credential(credential, hash))
}

fn invalid_credentials_error() -> CommandError {
    CommandError::new(
        "invalid_credentials",
        "Korisnicko ime ili lozinka nisu ispravni.",
    )
}
