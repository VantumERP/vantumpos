//! Command layer over dobavljači i primljene isprave (ZoT čl. 29 st. 1).
//!
//! Thin by design, like `commands::kep`: every rule — the open `vrsta` clause,
//! the identity snapshot, the datum shape, the one-way possession assertion —
//! lives in `crate::dobavljaci`. These wrappers resolve the acting user from the
//! server-side session (never a client payload), open the connection, and stamp
//! `now` from `utc_now()`.
//!
//! Reads are session-gated rather than admin-gated: a cashier taking in a pallet
//! needs to pick the isprava it arrived with, and `inventory_receive` is itself
//! only `require_session`. Writing a supplier master record is admin-gated, since
//! a mistyped PIB there propagates to every future delivery.

use tauri::State;

use crate::app_error::CommandError;
use crate::dobavljaci::{CreateIspravaRequest, Dobavljac, PrimljenaIsprava, SaveDobavljacRequest};
use crate::state::AppState;

#[tauri::command]
pub fn dobavljaci_list(state: State<'_, AppState>) -> Result<Vec<Dobavljac>, CommandError> {
    super::auth::require_session(state.inner())?;
    let connection = state.db().open()?;
    crate::dobavljaci::list_dobavljaci(&connection).map_err(Into::into)
}

#[tauri::command]
pub fn dobavljac_save(
    state: State<'_, AppState>,
    request: SaveDobavljacRequest,
) -> Result<i64, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open()?;
    let now = crate::clock::utc_now()?;
    crate::dobavljaci::save_dobavljac(&connection, request, &now).map_err(Into::into)
}

/// The most recent isprave, newest document date first — the picker's list.
#[tauri::command]
pub fn isprave_list(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<PrimljenaIsprava>, CommandError> {
    super::auth::require_session(state.inner())?;
    let connection = state.db().open()?;
    crate::dobavljaci::list_isprave(&connection, limit.unwrap_or(100)).map_err(Into::into)
}

#[tauri::command]
pub fn isprava_create(
    state: State<'_, AppState>,
    request: CreateIspravaRequest,
) -> Result<i64, CommandError> {
    let acting_user_id = super::auth::require_session(state.inner())?;
    let connection = state.db().open()?;
    let now = crate::clock::utc_now()?;
    crate::dobavljaci::create_isprava(&connection, request, acting_user_id, &now)
        .map_err(Into::into)
}

/// Records the operator's assertion that the paper isprava is held. Note what
/// this does **not** do: it stores no document and proves no possession — it
/// stores that somebody said so, and who, and when.
#[tauri::command]
pub fn isprava_confirm_possession(
    state: State<'_, AppState>,
    isprava_id: i64,
) -> Result<(), CommandError> {
    let acting_user_id = super::auth::require_session(state.inner())?;
    let connection = state.db().open()?;
    let now = crate::clock::utc_now()?;
    crate::dobavljaci::confirm_possession(&connection, isprava_id, acting_user_id, &now)
        .map_err(Into::into)
}
