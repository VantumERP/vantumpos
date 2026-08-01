//! The remote-support nalog — the `[LEGAL]` half of SW-10 — and the append
//! primitive that writes its lines into `audit_events`.
//!
//! **ZZPL čl. 46 is the operative article here, and it is the penalised one**
//! (čl. 95 st. 1 t. 23): an obrađivač, and „drugo lice … ovlašćeno za pristup“,
//! which reaches the individual support engineer, may not process the
//! rukovalac's data without the rukovalac's nalog. A row in `support_sessions`
//! IS that nalog. The audit log beside it is the PRUDENTIAL half — čl. 50
//! appears nowhere in čl. 95, so nothing here may be presented to the operator
//! as a logging duty.
//!
//! The lifecycle is four steps and each one is one of the four commands:
//!
//! 1. [`grant_access`] — the vlasnik issues the nalog with an explicit scope and
//!    an explicit expiry (req. 2). Admin-gated: a kasir cannot authorise a third
//!    party to touch the shop's data.
//! 2. [`request_access`] — the support side enters. It is gated by the **nalog**,
//!    not by a role, because that is precisely what čl. 46 makes the condition,
//!    and because the obrađivač has no account on this till. Without a live
//!    nalog it is refused outright.
//! 3. [`end_session`] — the vlasnik closes it. A nalog that was entered gets
//!    `ended_at`; one that never was gets `revoked_at`, because v18's
//!    `CHECK (ended_at IS NULL OR started_at IS NOT NULL)` will not let a session
//!    end that never began.
//! 4. [`active_session`] — what, if anything, is authorised right now.
//!
//! `now` is always a parameter. Expiry is decided by comparing parsed instants,
//! never by string ordering and never by `datetime('now')`.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::app_error::{AppError, CommandError};
use crate::audit::{
    chain_hash, reject_forbidden_content, AuditAction, AuditDraft, AuditObjectType, AuditReason,
    AuditRecipient, GENESIS_PREV_HASH,
};
use crate::clock::utc_now;
use crate::state::AppState;

/// The longest nalog this app will issue. Req. 2 asks for a **per-session**
/// approval; a nalog measured in weeks is an open-ended one with a date printed
/// on it, and the whole evidential point of the record is that it was not.
const MAX_TRAJANJE_MINUTA: i64 = 24 * 60;

/// The scope is free text on purpose — a nalog says what the vlasnik authorised,
/// in the vlasnik's own words. The bound only keeps it a sentence.
const MAX_OBIM_ZNAKOVA: usize = 500;

/// One `support_sessions` row: the čl. 46 nalog and its lifecycle stamps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportSession {
    pub id: i64,
    pub granted_by: i64,
    /// Read back for the operator, so the nalog names a person rather than an
    /// id. It never travels into `audit_events`, whose exclusion list (req. 4)
    /// governs that table and not this one.
    pub granted_by_name: String,
    pub granted_at: String,
    pub scope: String,
    pub expires_at: String,
    pub started_at: Option<String>,
    pub ended_at: Option<String>,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantAccessRequest {
    /// What the vlasnik is authorising, in the vlasnik's own words.
    pub scope: String,
    /// Integer minutes, like every other duration in this app.
    pub duration_minutes: i64,
}

#[tauri::command]
pub fn support_grant_access(
    state: State<'_, AppState>,
    request: GrantAccessRequest,
) -> Result<SupportSession, CommandError> {
    grant_access(state.inner(), request, &utc_now()?).map_err(Into::into)
}

#[tauri::command]
pub fn support_request_access(state: State<'_, AppState>) -> Result<SupportSession, CommandError> {
    request_access(state.inner(), &utc_now()?).map_err(Into::into)
}

#[tauri::command]
pub fn support_end_session(state: State<'_, AppState>) -> Result<SupportSession, CommandError> {
    end_session(state.inner(), &utc_now()?).map_err(Into::into)
}

#[tauri::command]
pub fn support_active_session(
    state: State<'_, AppState>,
) -> Result<Option<SupportSession>, CommandError> {
    active_session(state.inner(), &utc_now()?).map_err(Into::into)
}

/// The vlasnik issues the čl. 46 nalog: an explicit obim and an explicit expiry,
/// both recorded, plus who signed it and when (req. 2).
///
/// Admin-gated inside the domain function, not in the `#[tauri::command]`
/// wrapper, so no in-process caller can route around it.
pub fn grant_access(
    state: &AppState,
    request: GrantAccessRequest,
    now: &str,
) -> Result<SupportSession, AppError> {
    let acting = crate::commands::auth::require_admin(state)?;

    let scope = request.scope.trim();
    if scope.is_empty() {
        return Err(AppError::validation(
            "Nalog mora da navede obim pristupa koji se odobrava (ZZPL čl. 46).",
            serde_json::json!({ "field": "scope" }),
        ));
    }
    if scope.chars().count() > MAX_OBIM_ZNAKOVA {
        return Err(AppError::validation(
            format!("Obim pristupa može da ima najviše {MAX_OBIM_ZNAKOVA} znakova."),
            serde_json::json!({ "field": "scope" }),
        ));
    }
    if request.duration_minutes <= 0 || request.duration_minutes > MAX_TRAJANJE_MINUTA {
        return Err(AppError::validation(
            format!(
                "Trajanje naloga mora da bude između 1 i {MAX_TRAJANJE_MINUTA} minuta — \
                 nalog bez roka nije nalog za jednu sesiju (ZZPL čl. 46)."
            ),
            serde_json::json!({ "field": "durationMinutes" }),
        ));
    }

    let expires_at = plus_minutes(now, request.duration_minutes)?;

    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;

    if let Some(live) = newest_open_session(&tx)? {
        if is_active(&live, now)? {
            return Err(AppError::business(
                "support_nalog_vec_izdat",
                "Nalog za pristup tehničke podrške je već izdat i još važi. \
                 Prvo okončajte postojeći nalog.",
            ));
        }
    }

    tx.execute(
        "INSERT INTO support_sessions
             (granted_by, granted_at, scope, expires_at, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?2, ?2)",
        params![acting.id, now, scope, expires_at],
    )?;
    let session_id = tx.last_insert_rowid();

    append_audit_event(
        &tx,
        &AuditDraft {
            at: now.to_string(),
            actor_user_id: Some(acting.id),
            action: AuditAction::Unos,
            object_type: AuditObjectType::SupportSession,
            object_id: session_id.to_string(),
            // The nalog itself is not an access to anyone's data, so čl. 48
            // st. 2's razlog has nothing to answer here.
            reason_code: None,
            recipient: None,
            support_session_id: Some(session_id),
        },
    )?;

    let session = load_session(&tx, session_id)?;
    tx.commit()?;
    Ok(session)
}

/// The support side enters. Čl. 46 makes the **nalog**, not a role, the thing
/// that authorises this — and the obrađivač holds no account on this till — so
/// the gate here is the live nalog and nothing else.
///
/// The entry is logged as an `otkrivanje` to the class „obrađivač tehničke
/// podrške“: the shop's data is being made available to a third party, which is
/// what čl. 48 st. 2's *identitet primaoca* asks about. `started_at` is stamped
/// once; re-entering under the same nalog is the same session, and v18 keeps one
/// column, not a history.
pub fn request_access(state: &AppState, now: &str) -> Result<SupportSession, AppError> {
    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;

    let live = match newest_open_session(&tx)? {
        Some(session) if is_active(&session, now)? => session,
        _ => {
            return Err(AppError::business(
                "support_bez_naloga",
                "Pristup tehničke podrške nije odobren. Vlasnik mora prvo da izda nalog \
                 sa obimom i rokom (ZZPL čl. 46).",
            ))
        }
    };

    if live.started_at.is_some() {
        return Ok(live);
    }

    tx.execute(
        "UPDATE support_sessions SET started_at = ?1, updated_at = ?1 WHERE id = ?2",
        params![now, live.id],
    )?;

    append_audit_event(
        &tx,
        &AuditDraft {
            at: now.to_string(),
            // The support engineer has no user id on this till. Borrowing the
            // vlasnik's would record an access the vlasnik did not make; the
            // session link carries the nalog, its obim and who signed it.
            actor_user_id: None,
            action: AuditAction::Otkrivanje,
            object_type: AuditObjectType::SupportSession,
            object_id: live.id.to_string(),
            reason_code: Some(AuditReason::TehnickaPodrska),
            recipient: Some(AuditRecipient::ObradjivacTehnickePodrske),
            support_session_id: Some(live.id),
        },
    )?;

    let session = load_session(&tx, live.id)?;
    tx.commit()?;
    Ok(session)
}

/// The vlasnik closes the nalog before its expiry.
///
/// A nalog that was entered is *ended*; one that never was is *revoked*, because
/// v18's `CHECK (ended_at IS NULL OR started_at IS NOT NULL)` refuses to let a
/// session end that never began — and „withdrawn before anyone used it“ is a
/// materially different fact from „the support call finished“.
pub fn end_session(state: &AppState, now: &str) -> Result<SupportSession, AppError> {
    let acting = crate::commands::auth::require_admin(state)?;

    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;

    let Some(open) = newest_open_session(&tx)? else {
        return Err(AppError::business(
            "support_nema_sesije",
            "Nema otvorenog naloga za pristup tehničke podrške.",
        ));
    };

    if open.started_at.is_some() {
        tx.execute(
            "UPDATE support_sessions SET ended_at = ?1, updated_at = ?1 WHERE id = ?2",
            params![now, open.id],
        )?;
    } else {
        tx.execute(
            "UPDATE support_sessions
                SET revoked_at = ?1, revoked_by = ?2, updated_at = ?1
              WHERE id = ?3",
            params![now, acting.id, open.id],
        )?;
    }

    append_audit_event(
        &tx,
        &AuditDraft {
            at: now.to_string(),
            actor_user_id: Some(acting.id),
            action: AuditAction::Menjanje,
            object_type: AuditObjectType::SupportSession,
            object_id: open.id.to_string(),
            reason_code: None,
            recipient: None,
            support_session_id: Some(open.id),
        },
    )?;

    let session = load_session(&tx, open.id)?;
    tx.commit()?;
    Ok(session)
}

/// What is authorised right now — the one question the nalog exists to answer.
pub fn active_session(state: &AppState, now: &str) -> Result<Option<SupportSession>, AppError> {
    let connection = state.db().open()?;
    match newest_open_session(&connection)? {
        Some(session) if is_active(&session, now)? => Ok(Some(session)),
        _ => Ok(None),
    }
}

/// Appends one row to `audit_events`, chained onto the last.
///
/// The write boundary runs first: [`reject_forbidden_content`] is the čl. 5 st. 1
/// t. 3 exclusion list in code, and no path may reach this table around it.
///
/// The row's `id` is part of its digest, so it is chosen **before** the hash is
/// computed and inserted explicitly rather than left to AUTOINCREMENT. The high
/// water mark is the greater of the surviving `MAX(id)` and `sqlite_sequence.seq`
/// — the latter is the highest id ever issued and a DELETE does not lower it, so
/// a retention purge cannot make the next row reuse an id the chain already
/// covers. SQLite maintains `seq` for an explicit rowid too, provided it is the
/// largest so far, which it always is here.
///
/// Takes the caller's `Connection` so the log line and the fact it describes
/// commit or roll back together.
pub(crate) fn append_audit_event(conn: &Connection, draft: &AuditDraft) -> Result<i64, AppError> {
    reject_forbidden_content(draft)?;

    let prev_hash: String = conn
        .query_row(
            "SELECT hash FROM audit_events ORDER BY id DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .optional()?
        .unwrap_or_else(|| GENESIS_PREV_HASH.to_string());

    let next_id: i64 = conn.query_row(
        "SELECT MAX(
             COALESCE((SELECT MAX(id) FROM audit_events), 0),
             COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'audit_events'), 0)
         ) + 1",
        [],
        |row| row.get(0),
    )?;

    let hash = chain_hash(&prev_hash, next_id, draft);

    conn.execute(
        "INSERT INTO audit_events
             (id, at, actor_user_id, action, object_type, object_id, reason_code,
              recipient, support_session_id, prev_hash, hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
        params![
            next_id,
            draft.at,
            draft.actor_user_id,
            draft.action.as_code(),
            draft.object_type.as_code(),
            draft.object_id,
            draft.reason_code.map(AuditReason::as_code),
            draft.recipient.map(AuditRecipient::as_code),
            draft.support_session_id,
            prev_hash,
            hash,
        ],
    )?;

    Ok(next_id)
}

const SESSION_COLUMNS: &str = "s.id, s.granted_by, u.display_name, s.granted_at, s.scope,
     s.expires_at, s.started_at, s.ended_at, s.revoked_at";

fn read_session(row: &Row<'_>) -> rusqlite::Result<SupportSession> {
    Ok(SupportSession {
        id: row.get(0)?,
        granted_by: row.get(1)?,
        granted_by_name: row.get(2)?,
        granted_at: row.get(3)?,
        scope: row.get(4)?,
        expires_at: row.get(5)?,
        started_at: row.get(6)?,
        ended_at: row.get(7)?,
        revoked_at: row.get(8)?,
    })
}

/// The newest nalog that has neither ended nor been revoked. Expiry is not part
/// of the query: [`end_session`] must still be able to close an expired nalog
/// that nobody closed, and [`is_active`] decides the clock question separately.
fn newest_open_session(conn: &Connection) -> Result<Option<SupportSession>, AppError> {
    conn.query_row(
        &format!(
            "SELECT {SESSION_COLUMNS}
               FROM support_sessions s
               JOIN users u ON u.id = s.granted_by
              WHERE s.ended_at IS NULL AND s.revoked_at IS NULL
              ORDER BY s.id DESC LIMIT 1"
        ),
        [],
        read_session,
    )
    .optional()
    .map_err(AppError::from)
}

fn load_session(conn: &Connection, id: i64) -> Result<SupportSession, AppError> {
    conn.query_row(
        &format!(
            "SELECT {SESSION_COLUMNS}
               FROM support_sessions s
               JOIN users u ON u.id = s.granted_by
              WHERE s.id = ?1"
        ),
        params![id],
        read_session,
    )
    .map_err(AppError::from)
}

/// Instants are compared parsed, never as strings: RFC3339 renders the same
/// moment with and without a fractional part, and „09:00:00.5Z“ sorts *before*
/// „09:00:00Z“ lexicographically. The expiry is exclusive — at `expires_at` the
/// nalog is spent.
fn is_active(session: &SupportSession, now: &str) -> Result<bool, AppError> {
    let at = parse_rfc3339(now, "now")?;
    let granted = parse_rfc3339(&session.granted_at, "grantedAt")?;
    let expires = parse_rfc3339(&session.expires_at, "expiresAt")?;
    Ok(granted <= at && at < expires)
}

fn parse_rfc3339(value: &str, field: &str) -> Result<OffsetDateTime, AppError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|source| {
        AppError::validation(
            format!("Vreme nije ispravno: {source}"),
            serde_json::json!({ "field": field }),
        )
    })
}

/// Integer minutes, like every other duration in this app.
fn plus_minutes(now: &str, minutes: i64) -> Result<String, AppError> {
    let at = parse_rfc3339(now, "now")?;
    (at + Duration::minutes(minutes))
        .format(&Rfc3339)
        .map_err(|source| AppError::InvalidState(format!("Vreme nije dostupno: {source}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};

    fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            test(&state);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    fn sign_in_admin(state: &AppState) -> i64 {
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

    fn sign_in_cashier(state: &AppState) -> i64 {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, active, created_at, updated_at)
                 VALUES ('kasir', 'Kasir Kasirović', 'cashier', 1,
                         '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        let cashier_id = connection.last_insert_rowid();
        state
            .set_session_user_id(cashier_id)
            .expect("cashier session should set");
        cashier_id
    }

    fn grant_request() -> GrantAccessRequest {
        GrantAccessRequest {
            scope: "Pregled greške na štampi fiskalnog isečka".to_string(),
            duration_minutes: 60,
        }
    }

    /// A draft that is well-formed in every respect except the field under test,
    /// so a refusal can only have come from the field under test.
    fn forbidden_draft(object_id: &str) -> AuditDraft {
        AuditDraft {
            at: "2026-08-01T09:00:00Z".to_string(),
            actor_user_id: None,
            action: AuditAction::Unos,
            object_type: AuditObjectType::SupportSession,
            object_id: object_id.to_string(),
            reason_code: None,
            recipient: None,
            support_session_id: None,
        }
    }

    /// Every `audit_events` row, oldest first, as (action, object_type,
    /// object_id, reason, recipient, actor, session, prev_hash, hash).
    #[allow(clippy::type_complexity)]
    fn audit_rows(
        state: &AppState,
    ) -> Vec<(
        String,
        String,
        String,
        Option<String>,
        Option<String>,
        Option<i64>,
        Option<i64>,
        String,
        String,
    )> {
        let connection = state.db().open().expect("database should open");
        let mut statement = connection
            .prepare(
                "SELECT action, object_type, object_id, reason_code, recipient,
                        actor_user_id, support_session_id, prev_hash, hash
                 FROM audit_events ORDER BY id",
            )
            .expect("statement should prepare");
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                    row.get(7)?,
                    row.get(8)?,
                ))
            })
            .expect("query should run")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("rows should read");
        rows
    }

    /// Req. 2: „the owner grants access with an explicit scope and duration“.
    /// All four facts — who, when, what, until when — are the nalog.
    #[test]
    fn a_grant_records_who_when_scope_and_expiry() {
        with_state("a_grant_records_who_when_scope_and_expiry", |state| {
            let admin_id = sign_in_admin(state);

            let session = grant_access(state, grant_request(), "2026-08-01T09:00:00Z")
                .expect("the vlasnik should be able to issue a nalog");

            assert_eq!(session.granted_by, admin_id);
            assert_eq!(session.granted_at, "2026-08-01T09:00:00Z");
            assert_eq!(
                session.scope, "Pregled greške na štampi fiskalnog isečka",
                "the nalog must keep the vlasnik's own words"
            );
            assert_eq!(
                session.expires_at, "2026-08-01T10:00:00Z",
                "60 minutes after the grant"
            );
            assert_eq!(session.started_at, None);
            assert_eq!(session.ended_at, None);
            assert_eq!(session.revoked_at, None);

            assert_eq!(
                active_session(state, "2026-08-01T09:30:00Z").expect("the nalog should read back"),
                Some(session)
            );
        });
    }

    /// Čl. 46 puts the nalog in the rukovalac's hands. A kasir authorising a
    /// third party to touch the shop's data is the exact failure req. 2 exists
    /// to prevent.
    #[test]
    fn a_cashier_cannot_grant_support_access() {
        with_state("a_cashier_cannot_grant_support_access", |state| {
            sign_in_cashier(state);

            let error = grant_access(state, grant_request(), "2026-08-01T09:00:00Z")
                .expect_err("a kasir must not be able to issue a nalog");

            assert_eq!(error.code(), "forbidden");
            assert_eq!(
                active_session(state, "2026-08-01T09:00:00Z").expect("read back"),
                None,
                "a refused grant must leave no nalog behind"
            );
            assert!(
                audit_rows(state).is_empty(),
                "a refused grant must write no audit line either"
            );
        });
    }

    /// An open-ended grant is not a nalog. The duration is the second half of
    /// req. 2's „explicit scope AND duration“.
    #[test]
    fn a_grant_needs_an_explicit_positive_and_bounded_duration() {
        with_state(
            "a_grant_needs_an_explicit_positive_and_bounded_duration",
            |state| {
                sign_in_admin(state);

                for minutes in [0, -30, MAX_TRAJANJE_MINUTA + 1] {
                    let request = GrantAccessRequest {
                        duration_minutes: minutes,
                        ..grant_request()
                    };
                    let error = grant_access(state, request, "2026-08-01T09:00:00Z")
                        .expect_err("{minutes} is not a per-session duration");
                    assert_eq!(error.code(), "validation_error", "for {minutes} minutes");
                }

                let request = GrantAccessRequest {
                    duration_minutes: MAX_TRAJANJE_MINUTA,
                    ..grant_request()
                };
                grant_access(state, request, "2026-08-01T09:00:00Z")
                    .expect("the upper bound itself is allowed");
            },
        );
    }

    /// A nalog that does not say what was authorised authorises nothing.
    #[test]
    fn a_grant_needs_a_scope() {
        with_state("a_grant_needs_a_scope", |state| {
            sign_in_admin(state);

            for scope in ["", "   "] {
                let request = GrantAccessRequest {
                    scope: scope.to_string(),
                    ..grant_request()
                };
                let error = grant_access(state, request, "2026-08-01T09:00:00Z")
                    .expect_err("a nalog without an obim is not a nalog");
                assert_eq!(error.code(), "validation_error");
            }

            let request = GrantAccessRequest {
                scope: "x".repeat(MAX_OBIM_ZNAKOVA + 1),
                ..grant_request()
            };
            let error = grant_access(state, request, "2026-08-01T09:00:00Z")
                .expect_err("an essay is not a nalog either");
            assert_eq!(error.code(), "validation_error");
        });
    }

    /// The clock is what makes the grant per-session rather than standing.
    #[test]
    fn an_expired_grant_is_not_active() {
        with_state("an_expired_grant_is_not_active", |state| {
            sign_in_admin(state);
            grant_access(state, grant_request(), "2026-08-01T09:00:00Z").expect("nalog issues");

            assert!(
                active_session(state, "2026-08-01T09:59:59Z")
                    .expect("read back")
                    .is_some(),
                "still inside the granted window"
            );
            assert_eq!(
                active_session(state, "2026-08-01T10:00:00Z").expect("read back"),
                None,
                "the expiry is exclusive — at expires_at the nalog is spent"
            );
            assert_eq!(
                active_session(state, "2026-08-02T09:00:00Z").expect("read back"),
                None,
            );
        });
    }

    /// Čl. 46: without the rukovalac's nalog the obrađivač may not process at
    /// all. This is the enforcement point, and it is the reason the approval
    /// record is worth more evidentially than the session log.
    #[test]
    fn support_cannot_enter_without_a_live_nalog() {
        with_state("support_cannot_enter_without_a_live_nalog", |state| {
            let error = request_access(state, "2026-08-01T09:00:00Z")
                .expect_err("no nalog means no access");
            assert_eq!(error.code(), "support_bez_naloga");

            sign_in_admin(state);
            grant_access(state, grant_request(), "2026-08-01T09:00:00Z").expect("nalog issues");

            let expired = request_access(state, "2026-08-01T11:00:00Z")
                .expect_err("an expired nalog authorises nothing");
            assert_eq!(expired.code(), "support_bez_naloga");

            assert!(
                audit_rows(state).len() == 1,
                "only the grant should have been logged"
            );
        });
    }

    /// `started_at` is stamped once. Re-entering under the same nalog is the
    /// same session, not a second one — v18 has one column, not a history.
    #[test]
    fn entering_under_a_live_nalog_stamps_started_at_once() {
        with_state(
            "entering_under_a_live_nalog_stamps_started_at_once",
            |state| {
                sign_in_admin(state);
                grant_access(state, grant_request(), "2026-08-01T09:00:00Z").expect("nalog issues");

                let entered = request_access(state, "2026-08-01T09:05:00Z")
                    .expect("a live nalog admits support");
                assert_eq!(entered.started_at.as_deref(), Some("2026-08-01T09:05:00Z"));

                let again = request_access(state, "2026-08-01T09:20:00Z")
                    .expect("re-entry under the same nalog is the same session");
                assert_eq!(again.id, entered.id);
                assert_eq!(
                    again.started_at.as_deref(),
                    Some("2026-08-01T09:05:00Z"),
                    "the first entry is the one the nalog records"
                );

                let disclosures: Vec<_> = audit_rows(state)
                    .into_iter()
                    .filter(|row| row.0 == "otkrivanje")
                    .collect();
                assert_eq!(
                    disclosures.len(),
                    1,
                    "one entry, one čl. 48 st. 2 disclosure line"
                );
            },
        );
    }

    /// The support engineer holds no account on this till, so the čl. 48 st. 2
    /// „identitet lica“ is answered by the session link — which carries the
    /// nalog, its obim and who signed it — and never by borrowing the vlasnik's
    /// id, which would record an access the vlasnik did not make.
    #[test]
    fn the_entry_line_is_a_disclosure_to_the_processor_with_no_borrowed_actor() {
        with_state(
            "the_entry_line_is_a_disclosure_to_the_processor_with_no_borrowed_actor",
            |state| {
                sign_in_admin(state);
                let session = grant_access(state, grant_request(), "2026-08-01T09:00:00Z")
                    .expect("nalog issues");
                request_access(state, "2026-08-01T09:05:00Z").expect("support enters");

                let rows = audit_rows(state);
                let entry = rows
                    .iter()
                    .find(|row| row.0 == "otkrivanje")
                    .expect("the entry must be logged");

                assert_eq!(entry.1, "support_session");
                assert_eq!(entry.2, session.id.to_string());
                assert_eq!(entry.3.as_deref(), Some("tehnicka_podrska"));
                assert_eq!(entry.4.as_deref(), Some("obradjivac_tehnicke_podrske"));
                assert_eq!(entry.5, None, "the vlasnik's id must not be borrowed");
                assert_eq!(entry.6, Some(session.id));
            },
        );
    }

    #[test]
    fn ending_a_started_session_stamps_ended_at() {
        with_state("ending_a_started_session_stamps_ended_at", |state| {
            sign_in_admin(state);
            grant_access(state, grant_request(), "2026-08-01T09:00:00Z").expect("nalog issues");
            request_access(state, "2026-08-01T09:05:00Z").expect("support enters");

            let ended = end_session(state, "2026-08-01T09:40:00Z").expect("the vlasnik ends it");

            assert_eq!(ended.ended_at.as_deref(), Some("2026-08-01T09:40:00Z"));
            assert_eq!(ended.revoked_at, None);
            assert_eq!(
                active_session(state, "2026-08-01T09:41:00Z").expect("read back"),
                None,
                "an ended session is not active even before its expiry"
            );

            let error = end_session(state, "2026-08-01T09:45:00Z")
                .expect_err("there is nothing left to end");
            assert_eq!(error.code(), "support_nema_sesije");
        });
    }

    /// v18's `CHECK (ended_at IS NULL OR started_at IS NOT NULL)` will not let a
    /// session end that never began, so withdrawing an unused nalog has to be
    /// recorded as what it is: a revocation.
    #[test]
    fn ending_a_nalog_that_was_never_entered_revokes_it() {
        with_state(
            "ending_a_nalog_that_was_never_entered_revokes_it",
            |state| {
                let admin_id = sign_in_admin(state);
                grant_access(state, grant_request(), "2026-08-01T09:00:00Z").expect("nalog issues");

                let revoked = end_session(state, "2026-08-01T09:10:00Z")
                    .expect("the vlasnik withdraws the nalog");

                assert_eq!(revoked.revoked_at.as_deref(), Some("2026-08-01T09:10:00Z"));
                assert_eq!(revoked.ended_at, None);
                assert_eq!(revoked.started_at, None);
                assert_eq!(
                    active_session(state, "2026-08-01T09:11:00Z").expect("read back"),
                    None
                );

                let revoked_by: Option<i64> = state
                    .db()
                    .open()
                    .expect("database should open")
                    .query_row(
                        "SELECT revoked_by FROM support_sessions WHERE id = ?1",
                        params![revoked.id],
                        |row| row.get(0),
                    )
                    .expect("the row should read back");
                assert_eq!(revoked_by, Some(admin_id));
            },
        );
    }

    #[test]
    fn a_cashier_cannot_end_the_vlasniks_nalog() {
        with_state("a_cashier_cannot_end_the_vlasniks_nalog", |state| {
            sign_in_admin(state);
            grant_access(state, grant_request(), "2026-08-01T09:00:00Z").expect("nalog issues");

            sign_in_cashier(state);
            let error = end_session(state, "2026-08-01T09:10:00Z")
                .expect_err("the nalog is the rukovalac's instrument");
            assert_eq!(error.code(), "forbidden");
        });
    }

    /// Req. 2 is a PER-SESSION approval. Two live nalozi at once make „what is
    /// authorised right now“ unanswerable, which is the one question the record
    /// exists to answer.
    #[test]
    fn a_second_grant_while_one_is_live_is_refused() {
        with_state("a_second_grant_while_one_is_live_is_refused", |state| {
            sign_in_admin(state);
            grant_access(state, grant_request(), "2026-08-01T09:00:00Z").expect("nalog issues");

            let error = grant_access(state, grant_request(), "2026-08-01T09:10:00Z")
                .expect_err("one nalog at a time");
            assert_eq!(error.code(), "support_nalog_vec_izdat");

            end_session(state, "2026-08-01T09:15:00Z").expect("withdraw the first");
            grant_access(state, grant_request(), "2026-08-01T09:20:00Z")
                .expect("a fresh nalog after the first is closed");

            // An expired nalog blocks nothing either.
            let later = grant_access(state, grant_request(), "2026-08-02T09:00:00Z");
            assert!(later.is_ok(), "an expired nalog is not a live one");
        });
    }

    /// Every step of the nalog's life is a line in the log, and the lines chain.
    #[test]
    fn every_grant_and_end_writes_a_chained_audit_row() {
        with_state("every_grant_and_end_writes_a_chained_audit_row", |state| {
            let admin_id = sign_in_admin(state);

            let session =
                grant_access(state, grant_request(), "2026-08-01T09:00:00Z").expect("nalog issues");
            request_access(state, "2026-08-01T09:05:00Z").expect("support enters");
            end_session(state, "2026-08-01T09:40:00Z").expect("the vlasnik ends it");

            let rows = audit_rows(state);
            let actions: Vec<&str> = rows.iter().map(|row| row.0.as_str()).collect();
            assert_eq!(actions, vec!["unos", "otkrivanje", "menjanje"]);

            for row in &rows {
                assert_eq!(row.1, "support_session");
                assert_eq!(row.2, session.id.to_string());
                assert_eq!(row.6, Some(session.id));
            }
            assert_eq!(rows[0].5, Some(admin_id), "the vlasnik signed the nalog");
            assert_eq!(rows[2].5, Some(admin_id), "and the vlasnik ended it");

            // The chain: genesis, then each row carrying its predecessor's hash.
            assert_eq!(rows[0].7, GENESIS_PREV_HASH);
            assert_eq!(rows[1].7, rows[0].8);
            assert_eq!(rows[2].7, rows[1].8);
            for row in &rows {
                assert_eq!(row.8.len(), 64, "SHA-256 hex");
            }
        });
    }

    /// Req. 4 is enforced at the write boundary or it is enforced nowhere.
    /// [`reject_forbidden_content`] is unit-tested as a pure function in
    /// `audit.rs`; this test pins the fact that the function that actually
    /// touches the table still calls it — and honours the answer.
    ///
    /// The transaction is committed rather than dropped, so a refusal that
    /// leaked a row would be caught as a persisted row and not merely as a
    /// rolled-back one.
    #[test]
    fn the_write_boundary_refuses_forbidden_content_before_the_row_lands() {
        with_state(
            "the_write_boundary_refuses_forbidden_content_before_the_row_lands",
            |state| {
                let mut connection = state.db().open().expect("database should open");
                let tx = connection.transaction().expect("transaction should begin");

                // A JMBG-shaped run and a Luhn-passing PAN — the two shapes req. 4
                // names first. Neither may reach `audit_events` through this door.
                for object_id in ["0101990710015", "4111111111111111"] {
                    let error = append_audit_event(&tx, &forbidden_draft(object_id))
                        .expect_err("the write boundary must refuse this id");
                    assert_eq!(error.code(), "audit_forbidden_content", "for {object_id}");
                }

                // The čl. 48 st. 2 leg of the same gate: an otkrivanje with no
                // class of primalac is not the record the article describes.
                let error = append_audit_event(
                    &tx,
                    &AuditDraft {
                        action: AuditAction::Otkrivanje,
                        reason_code: Some(AuditReason::TehnickaPodrska),
                        recipient: None,
                        ..forbidden_draft("1")
                    },
                )
                .expect_err("an otkrivanje without a primalac must be refused");
                assert_eq!(error.code(), "audit_missing_recipient");

                tx.commit().expect("the transaction should commit");
                drop(connection);

                assert!(
                    audit_rows(state).is_empty(),
                    "a refused draft must not reach audit_events at all"
                );
            },
        );
    }

    /// A refused write must leave nothing behind — neither the domain row nor a
    /// half-written log line. The two share one transaction.
    ///
    /// The failure has to land BETWEEN the two writes, so it is injected at the
    /// only seam `grant_access` itself can trip: the audit line's `object_id` is
    /// the new nalog's id, and the write boundary refuses a run of nine or more
    /// digits. Seeding the table so the next AUTOINCREMENT id is ten digits
    /// makes the nalog's own log line unwritable — the domain INSERT has already
    /// happened when the refusal arrives, which is exactly the window the shared
    /// transaction exists to close.
    #[test]
    fn the_audit_line_and_the_nalog_share_one_transaction() {
        with_state(
            "the_audit_line_and_the_nalog_share_one_transaction",
            |state| {
                let admin_id = sign_in_admin(state);

                // Closed, so it is not a live nalog standing in the way; its only
                // job is to push `sqlite_sequence` up to the last nine-digit id.
                state
                    .db()
                    .open()
                    .expect("database should open")
                    .execute(
                        "INSERT INTO support_sessions
                             (id, granted_by, granted_at, scope, expires_at,
                              revoked_at, revoked_by, created_at, updated_at)
                         VALUES (999999999, ?1, '2026-07-01T09:00:00Z', 'Ranija sesija',
                                 '2026-07-01T10:00:00Z', '2026-07-01T09:30:00Z', ?1,
                                 '2026-07-01T09:00:00Z', '2026-07-01T09:30:00Z')",
                        params![admin_id],
                    )
                    .expect("the seed session should insert");

                let error = grant_access(state, grant_request(), "2026-08-01T09:00:00Z")
                    .expect_err("a nalog whose log line cannot be written must not be issued");
                assert_eq!(error.code(), "audit_forbidden_content");

                let connection = state.db().open().expect("database should open");
                let sessions: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM support_sessions WHERE id <> 999999999",
                        [],
                        |row| row.get(0),
                    )
                    .expect("count should read");
                assert_eq!(
                    sessions, 0,
                    "the nalog must roll back with the log line it could not write"
                );
                assert!(
                    audit_rows(state).is_empty(),
                    "and no half-written log line may survive either"
                );
            },
        );
    }
}
