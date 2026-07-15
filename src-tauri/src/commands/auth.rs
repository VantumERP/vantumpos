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

/// The single authoritative session gate, shared by every command that needs
/// the acting user. The acting user is derived from the server-side session,
/// never from a client-supplied payload. A session pointing at a user that no
/// longer exists or is inactive is treated as unauthenticated and cleared.
pub(crate) fn require_session(state: &AppState) -> Result<i64, AppError> {
    let user_id = state
        .session_user_id()?
        .ok_or_else(|| AppError::business("unauthorized", "Niste prijavljeni."))?;
    if active_user_by_id(state, user_id)?.is_none() {
        state.clear_session()?;
        return Err(AppError::business("unauthorized", "Niste prijavljeni."));
    }
    Ok(user_id)
}

/// The single authoritative admin gate, shared by every admin-only command
/// (Settings, Users, and later Reports). The acting user is derived from the
/// server-side session, never from a client-supplied role.
pub(crate) fn require_admin(state: &AppState) -> Result<UserAccount, AppError> {
    let user_id = require_session(state)?;
    let user = active_user_by_id(state, user_id)?
        .ok_or_else(|| AppError::business("unauthorized", "Niste prijavljeni."))?;

    if user.role != "admin" {
        return Err(AppError::business(
            "forbidden",
            "Samo administrator može da izvrši ovu akciju.",
        ));
    }

    Ok(user)
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
        "Korisničko ime ili lozinka nisu ispravni.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};
    use crate::security::hash_credential;

    fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            test(&state);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    fn seed_admin_session(state: &AppState) -> i64 {
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
        admin_id
    }

    fn seed_cashier_session(state: &AppState) -> i64 {
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
        cashier_id
    }

    #[test]
    fn require_admin_returns_signed_in_admin() {
        with_state("require_admin_returns_signed_in_admin", |state| {
            let admin_id = seed_admin_session(state);

            let user = require_admin(state).expect("admin should pass the gate");

            assert_eq!(user.id, admin_id);
            assert_eq!(user.role, "admin");
        });
    }

    #[test]
    fn require_admin_rejects_cashier_with_forbidden() {
        with_state("require_admin_rejects_cashier_with_forbidden", |state| {
            seed_cashier_session(state);

            let error = require_admin(state).expect_err("cashier should be denied");

            assert_eq!(error.code(), "forbidden");
        });
    }

    #[test]
    fn require_admin_rejects_without_session_with_unauthorized() {
        with_state(
            "require_admin_rejects_without_session_with_unauthorized",
            |state| {
                let error = require_admin(state).expect_err("anonymous caller should be denied");

                assert_eq!(error.code(), "unauthorized");
            },
        );
    }

    #[test]
    fn require_admin_clears_stale_session_and_rejects() {
        with_state("require_admin_clears_stale_session_and_rejects", |state| {
            // Session points at a user id that does not exist (or is inactive).
            state
                .set_session_user_id(9999)
                .expect("stale session should set");

            let error = require_admin(state).expect_err("stale session should be denied");

            assert_eq!(error.code(), "unauthorized");
            assert_eq!(
                state
                    .session_user_id()
                    .expect("session should remain readable"),
                None,
                "the stale session must be cleared",
            );
        });
    }

    #[test]
    fn require_session_returns_signed_in_user_id() {
        with_state("require_session_returns_signed_in_user_id", |state| {
            let cashier_id = seed_cashier_session(state);

            let user_id = require_session(state).expect("signed-in session should resolve");

            assert_eq!(user_id, cashier_id);
        });
    }

    #[test]
    fn require_session_rejects_without_session_with_unauthorized() {
        with_state(
            "require_session_rejects_without_session_with_unauthorized",
            |state| {
                let error = require_session(state).expect_err("anonymous caller should be denied");

                assert_eq!(error.code(), "unauthorized");
            },
        );
    }

    #[test]
    fn login_user_succeeds_with_valid_credentials() {
        with_state("login_succeeds", |state| {
            let session = login_user(
                state,
                LoginRequest {
                    username: "admin".to_string(),
                    credential: "1234".to_string(),
                },
            )
            .expect("seeded admin should log in");

            assert_eq!(session.user.username, "admin");
            assert_eq!(session.user.role, "admin");
            assert!(session.user.active);
            assert!(session.current_shift.is_none());
            assert_eq!(
                state.session_user_id().expect("session id should read"),
                Some(session.user.id)
            );
        });
    }

    #[test]
    fn login_user_rejects_invalid_credentials() {
        with_state("login_invalid_credentials", |state| {
            let error = login_user(
                state,
                LoginRequest {
                    username: "admin".to_string(),
                    credential: "0000".to_string(),
                },
            )
            .expect_err("wrong pin should fail");

            assert_eq!(error.code, "invalid_credentials");
            assert!(state
                .session_user_id()
                .expect("session id should read")
                .is_none());
        });
    }

    #[test]
    fn login_user_rejects_deactivated_user() {
        with_state("login_deactivated", |state| {
            let pin_hash = hash_credential("4321").expect("pin hash should compute");
            let connection = state.db().open().expect("database should open");
            connection
                .execute(
                    "INSERT INTO users (
                        username, display_name, role, pin_hash, active, created_at, updated_at
                     )
                     VALUES ('kasir', 'Kasir', 'cashier', ?1, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                    params![pin_hash],
                )
                .expect("deactivated cashier should insert");

            let error = login_user(
                state,
                LoginRequest {
                    username: "kasir".to_string(),
                    credential: "4321".to_string(),
                },
            )
            .expect_err("deactivated user should not log in");

            assert_eq!(error.code, "invalid_credentials");
            assert!(state
                .session_user_id()
                .expect("session id should read")
                .is_none());
        });
    }
}
