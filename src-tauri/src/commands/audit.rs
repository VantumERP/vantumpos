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
//!
//! The second half of the module is the log's own surface: [`record_audit`], the
//! one door every feature of the app writes through; [`search`], the read path;
//! and [`export_csv`], the čl. 48 st. 4 izvod for the Poverenik. There is no
//! third verb — req. 7 puts tamper-evidence above completeness, so nothing here
//! edits or removes a logged row, and v18's trigger refuses the edit even if
//! something tried.

use std::fs;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

use crate::app_error::{AppError, CommandError};
use crate::audit::{
    chain_hash, reject_forbidden_content, verify_chain_anchored, AuditAction, AuditDraft,
    AuditObjectType, AuditReason, AuditRecipient, AuditRow, ChainVerdict, GENESIS_PREV_HASH,
};
use crate::cash_deposit::parse_iso_date;
use crate::clock::utc_now;
use crate::commands::reports::{csv_line, ExportedFile};
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

#[tauri::command]
pub fn audit_search(
    state: State<'_, AppState>,
    query: AuditQuery,
) -> Result<AuditSearchResult, CommandError> {
    search(state.inner(), &query).map_err(Into::into)
}

#[tauri::command]
pub fn audit_export_csv(
    state: State<'_, AppState>,
    query: AuditQuery,
) -> Result<ExportedFile, CommandError> {
    export_csv(state.inner(), &query, &utc_now()?).map_err(Into::into)
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

// ---------------------------------------------------------------------------
// The general write path
// ---------------------------------------------------------------------------

/// One fact a caller asks to be recorded.
///
/// It is [`AuditDraft`] minus the two fields a caller may not choose: the
/// instant, which comes from the `now` the outermost boundary read, and the
/// actor, which comes from the server-side session. A caller that could pick
/// `at` could backdate a line, and one that could pick the actor could attribute
/// an access to a colleague — which is precisely the fact the log exists to fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuditEntry {
    pub action: AuditAction,
    pub object_type: AuditObjectType,
    /// An opaque internal id, whitelisted by the write boundary (req. 4).
    pub object_id: String,
    pub reason_code: Option<AuditReason>,
    pub recipient: Option<AuditRecipient>,
    pub support_session_id: Option<i64>,
}

/// The app's general door into `audit_events` (req. 3), for every caller that
/// does not already own a transaction.
///
/// A caller that DOES own one — a domain write whose log line must commit or
/// roll back with the fact it describes — calls [`append_audit_event`] with its
/// own `Connection` instead. Both go through the same write boundary; there is
/// no third way in.
///
/// The actor is the session's user, read with [`AppState::session_user_id`]
/// rather than `require_session`: the čl. 5 st. 1 t. 5 purge is time-driven, not
/// request-driven (req. 23), so its line has no human actor and must still be
/// writable. An absent actor is honest; a borrowed one is not.
///
/// [`AppState::session_user_id`]: crate::state::AppState::session_user_id
pub(crate) fn record_audit(
    state: &AppState,
    entry: AuditEntry,
    now: &str,
) -> Result<i64, AppError> {
    let actor_user_id = state.session_user_id()?;

    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;
    let id = append_audit_event(
        &tx,
        &AuditDraft {
            at: now.to_string(),
            actor_user_id,
            action: entry.action,
            object_type: entry.object_type,
            object_id: entry.object_id,
            reason_code: entry.reason_code,
            recipient: entry.recipient,
            support_session_id: entry.support_session_id,
        },
    )?;
    tx.commit()?;
    Ok(id)
}

// ---------------------------------------------------------------------------
// The read path and the čl. 48 st. 4 izvod
// ---------------------------------------------------------------------------

/// Req. 8's two axes — a date range and an actor — and nothing else.
///
/// The days are `gggg-MM-dd` and both bounds are inclusive. Every row this table
/// holds was stamped from [`utc_now`], so the day is the first ten characters of
/// `at` and the comparison is exact; anything that is not a bare day is refused
/// rather than silently mis-filtered.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub actor_user_id: Option<i64>,
}

/// One logged row as the operator reads it: the stored code beside the Serbian
/// prose behind it.
///
/// `actor_name` is resolved from `users` at read time and is NOT stored on the
/// row — the exclusion list (req. 4) governs `audit_events`, and čl. 48 st. 2's
/// *identitet lica* is answered by the id it does store. The document a person
/// reads needs the name; the table must never hold it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub id: i64,
    pub at: String,
    pub actor_user_id: Option<i64>,
    pub actor_name: Option<String>,
    pub action: String,
    pub action_label: String,
    pub object_type: String,
    pub object_type_label: String,
    pub object_id: String,
    pub reason_code: Option<String>,
    pub reason_label: Option<String>,
    pub recipient: Option<String>,
    pub recipient_label: Option<String>,
    pub support_session_id: Option<i64>,
    pub prev_hash: String,
    pub hash: String,
}

/// What the hash chain says about the WHOLE log — never about the filtered
/// slice, because a row removed outside the range is still a row removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChainStatus {
    pub verdict: ChainVerdict,
    pub intact: bool,
    /// How many rows the walk covered — the surviving log, not the filter.
    pub checked_rows: usize,
    /// The verdict as a sentence, so the panel and the izvod cannot disagree.
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditSearchResult {
    pub events: Vec<AuditEvent>,
    pub chain: ChainStatus,
}

/// Reads the log. Admin-gated: čl. 48 st. 4 puts the evidencija in the
/// rukovalac's hands, and the whole shop's access history is not a kasir's to
/// read.
///
/// Reading is deliberately NOT logged. A čl. 48 st. 2 line about an uvid into
/// the log would itself be an uvid into the log, and the panel that refreshes
/// would write the shop a log of nothing else. The disclosure that leaves the
/// till — [`export_csv`] — is the fact worth recording, and it is.
pub fn search(state: &AppState, query: &AuditQuery) -> Result<AuditSearchResult, AppError> {
    crate::commands::auth::require_admin(state)?;
    let (from, to) = validated_range(query)?;

    let connection = state.db().open()?;
    let chain = chain_status(&connection)?;
    let events = read_events(
        &connection,
        from.as_deref(),
        to.as_deref(),
        query.actor_user_id,
    )?;

    Ok(AuditSearchResult { events, chain })
}

/// The čl. 48 st. 4 izvod (req. 8): a clean, human-readable file by date range
/// and by actor, written to disk beside the database.
///
/// **It renders offline, from the till.** Everything the reader needs is in the
/// file — the čl. 49 osnov for handing it over, the čl. 48 st. 4 uzor it is
/// modelled on, the čl. 48 st. 3 purpose lock, the period, the filter, the
/// row count, the chain verdict and both hashes per row, so the chain can be
/// recomputed by whoever receives it. Nothing is fetched.
///
/// The disclosure is logged AFTER the file exists, because an export that failed
/// disclosed nothing; the consequence is that the izvod shows the log as it stood
/// the moment it was rendered, which is what a copy of a register is.
pub fn export_csv(
    state: &AppState,
    query: &AuditQuery,
    now: &str,
) -> Result<ExportedFile, AppError> {
    // The admin gate and the range validation both live in `search`.
    let result = search(state, query)?;
    let (from, to) = validated_range(query)?;

    let actor = match query.actor_user_id {
        None => None,
        Some(user_id) => {
            let connection = state.db().open()?;
            let name: Option<String> = connection
                .query_row(
                    "SELECT display_name FROM users WHERE id = ?1",
                    params![user_id],
                    |row| row.get(0),
                )
                .optional()?;
            Some((user_id, name))
        }
    };

    let csv = izvod_to_csv(
        &result,
        &period_label(from.as_deref(), to.as_deref()),
        &actor_filter_label(actor.as_ref()),
        now,
    );

    let export_dir = state.db().path().parent().map_or_else(
        || Path::new(".").join("exports"),
        |parent| parent.join("exports"),
    );
    fs::create_dir_all(&export_dir)?;

    let file_name = izvod_file_name(from.as_deref(), to.as_deref(), query.actor_user_id);
    let path = export_dir.join(&file_name);
    fs::write(&path, csv)?;

    // Čl. 48 st. 2 in one line: podaci disclosed, to a class of primalac, for a
    // razlog. The object id is how far the disclosure reached — the newest row
    // inside it — because the log itself has no id and the extent is the fact
    // worth being able to check later.
    record_audit(
        state,
        AuditEntry {
            action: AuditAction::Otkrivanje,
            object_type: AuditObjectType::AuditLog,
            object_id: result.events.last().map_or(0, |event| event.id).to_string(),
            reason_code: Some(AuditReason::Inspekcija),
            recipient: Some(AuditRecipient::Poverenik),
            support_session_id: None,
        },
        now,
    )?;

    Ok(ExportedFile {
        file_name,
        path: path.display().to_string(),
        mime_type: "text/csv",
        row_count: result.events.len(),
    })
}

const EVENT_COLUMNS: &str = "e.id, e.at, e.actor_user_id, u.display_name, e.action, e.object_type,
     e.object_id, e.reason_code, e.recipient, e.support_session_id, e.prev_hash, e.hash";

/// A LEFT JOIN, not a JOIN: the purge line (req. 23) has no actor at all, and a
/// row whose actor was later deactivated must still be readable.
fn read_events(
    conn: &Connection,
    from: Option<&str>,
    to: Option<&str>,
    actor_user_id: Option<i64>,
) -> Result<Vec<AuditEvent>, AppError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {EVENT_COLUMNS}
           FROM audit_events e
           LEFT JOIN users u ON u.id = e.actor_user_id
          WHERE (?1 IS NULL OR substr(e.at, 1, 10) >= ?1)
            AND (?2 IS NULL OR substr(e.at, 1, 10) <= ?2)
            AND (?3 IS NULL OR e.actor_user_id = ?3)
          ORDER BY e.id"
    ))?;
    let events = statement
        .query_map(params![from, to, actor_user_id], read_event)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(events)
}

fn read_event(row: &Row<'_>) -> rusqlite::Result<AuditEvent> {
    let action: String = row.get(4)?;
    let object_type: String = row.get(5)?;
    let reason_code: Option<String> = row.get(7)?;
    let recipient: Option<String> = row.get(8)?;

    Ok(AuditEvent {
        id: row.get(0)?,
        at: row.get(1)?,
        actor_user_id: row.get(2)?,
        actor_name: row.get(3)?,
        action_label: action_label(&action),
        action,
        object_type_label: object_type_label(&object_type),
        object_type,
        object_id: row.get(6)?,
        reason_label: reason_code.as_deref().map(reason_label),
        reason_code,
        recipient_label: recipient.as_deref().map(recipient_label),
        recipient,
        support_session_id: row.get(9)?,
        prev_hash: row.get(10)?,
        hash: row.get(11)?,
    })
}

const CHAIN_COLUMNS: &str = "id, at, actor_user_id, action, object_type, object_id, reason_code,
     recipient, support_session_id, prev_hash, hash";

/// Where the chain is walked from once a retention purge has removed the head.
///
/// **The contract SW-13's purge owes this reader:** when expiry removes rows it
/// writes this settings key with the `hash` and the `id` of the LAST row it
/// removed. Req. 6 forbids „trajno“ on this log and v18 leaves DELETE open
/// precisely so expiry can act, but expiry removes the OLDEST rows — the head,
/// where the genesis anchor sits — so without the recorded pair the first
/// legitimate purge reports a break indistinguishable from tampering.
///
/// Absent, the anchor is genesis: nothing has been purged yet.
///
/// `pub(crate)` because the purge that writes it lives in
/// [`crate::commands::personnel`]: the key is a contract between two modules,
/// and a contract spelled out twice is one that drifts silently.
pub(crate) const PURGE_ANCHOR_KEY: &str = "audit_chain_anchor";

fn purge_anchor(conn: &Connection) -> Result<(String, i64), AppError> {
    let stored = conn
        .query_row(
            "SELECT json_extract(value_json, '$.hash'), json_extract(value_json, '$.id')
               FROM settings WHERE key = ?1",
            params![PURGE_ANCHOR_KEY],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                ))
            },
        )
        .optional()?;

    Ok(match stored {
        Some((Some(hash), Some(id))) => (hash, id),
        _ => (GENESIS_PREV_HASH.to_string(), 0),
    })
}

/// Walks the whole surviving log and reports what the chain says.
///
/// `highest_ever_id` is `sqlite_sequence.seq` — the highest id ever ISSUED,
/// which a DELETE does not lower — and it is what makes a deletion at the TAIL
/// visible: the survivors of that deletion reconcile perfectly among themselves.
fn chain_status(conn: &Connection) -> Result<ChainStatus, AppError> {
    let (anchor_prev, anchor_id) = purge_anchor(conn)?;
    let highest_ever_id: i64 = conn.query_row(
        "SELECT COALESCE((SELECT seq FROM sqlite_sequence WHERE name = 'audit_events'), 0)",
        [],
        |row| row.get(0),
    )?;

    let mut statement = conn.prepare(&format!(
        "SELECT {CHAIN_COLUMNS} FROM audit_events ORDER BY id"
    ))?;
    let mut cursor = statement.query([])?;
    let mut rows: Vec<AuditRow> = Vec::new();
    let mut unreadable: Option<usize> = None;
    while let Some(row) = cursor.next()? {
        match chain_row(row)? {
            Some(parsed) => rows.push(parsed),
            // A code outside the closed vocabularies cannot have been written by
            // this app, so the row is not one the chain can vouch for.
            None => {
                unreadable = Some(rows.len());
                break;
            }
        }
    }

    let verdict = unreadable.map_or_else(
        || verify_chain_anchored(&anchor_prev, anchor_id, &rows, highest_ever_id),
        ChainVerdict::BrokenAt,
    );

    Ok(ChainStatus {
        intact: verdict == ChainVerdict::Intact,
        label: chain_label(verdict),
        verdict,
        checked_rows: rows.len(),
    })
}

fn chain_row(row: &Row<'_>) -> rusqlite::Result<Option<AuditRow>> {
    let action: String = row.get(3)?;
    let object_type: String = row.get(4)?;
    let stored_reason: Option<String> = row.get(6)?;
    let stored_recipient: Option<String> = row.get(7)?;

    let (Some(action), Some(object_type)) = (
        AuditAction::from_code(&action),
        AuditObjectType::from_code(&object_type),
    ) else {
        return Ok(None);
    };
    let reason_code = match stored_reason.as_deref().map(AuditReason::from_code) {
        None => None,
        Some(Some(reason)) => Some(reason),
        Some(None) => return Ok(None),
    };
    let recipient = match stored_recipient.as_deref().map(AuditRecipient::from_code) {
        None => None,
        Some(Some(recipient)) => Some(recipient),
        Some(None) => return Ok(None),
    };

    Ok(Some(AuditRow {
        id: row.get(0)?,
        at: row.get(1)?,
        actor_user_id: row.get(2)?,
        action,
        object_type,
        object_id: row.get(5)?,
        reason_code,
        recipient,
        support_session_id: row.get(8)?,
        prev_hash: row.get(9)?,
        hash: row.get(10)?,
    }))
}

fn chain_label(verdict: ChainVerdict) -> String {
    match verdict {
        ChainVerdict::Intact => "Potvrđena — lanac otisaka je neprekinut.".to_string(),
        ChainVerdict::BrokenAt(index) => format!(
            "Narušena — prvi neusklađen zapis je {}. po redu u evidenciji.",
            index + 1
        ),
        ChainVerdict::Truncated => {
            "Narušena — najnoviji zapisi su uklonjeni iz evidencije.".to_string()
        }
    }
}

fn validated_range(query: &AuditQuery) -> Result<(Option<String>, Option<String>), AppError> {
    let from = validated_day(query.from.as_deref(), "from")?;
    let to = validated_day(query.to.as_deref(), "to")?;
    if let (Some(from), Some(to)) = (&from, &to) {
        if from > to {
            return Err(AppError::validation(
                "Početni datum ne može da bude posle završnog.",
                serde_json::json!({ "field": "from" }),
            ));
        }
    }
    Ok((from, to))
}

/// A bare `gggg-MM-dd` or nothing. An RFC3339 instant is refused rather than
/// truncated: a bound the caller believes is a moment and the query treats as a
/// day is a filter that silently discloses more, or less, than was asked for.
fn validated_day(value: Option<&str>, field: &str) -> Result<Option<String>, AppError> {
    let Some(day) = value.map(str::trim).filter(|day| !day.is_empty()) else {
        return Ok(None);
    };
    if parse_iso_date(day).is_none() {
        return Err(AppError::validation(
            "Datum mora da bude u obliku gggg-MM-dd, na primer 2026-08-01.",
            serde_json::json!({ "field": field }),
        ));
    }
    Ok(Some(day.to_string()))
}

const IZVOD_NASLOV: &str = "IZVOD IZ EVIDENCIJE PRISTUPA PODACIMA O LIČNOSTI";

/// The basis on which this shop actually hands the document over: čl. 49's
/// general duty to cooperate with the Poverenik, which reaches a private
/// rukovalac. Req. 8 names it beside čl. 48 st. 4 precisely because the two are
/// not the same thing, and this is the line the reader meets first.
const IZVOD_OSNOV: &str = "ZZPL čl. 49 — opšta dužnost rukovaoca da sarađuje sa Poverenikom \
     u vršenju njegovih ovlašćenja; po tom osnovu se ovaj izvod predaje na zahtev Poverenika.";

/// The MODEL for the document's shape, never its basis. Čl. 48 st. 4 addresses
/// a nadležni organ koji obrađuje podatke u posebne svrhe (§3 V1), so printing
/// it as „pravni osnov“ would assert a duty this rukovalac does not have.
const IZVOD_UZOR: &str = "ZZPL čl. 48 st. 4 — evidencija se stavlja na uvid Povereniku, na \
     njegov zahtev. Ta odredba obavezuje nadležni organ, pa je ovde uzor, a ne osnov.";

/// ZZPL čl. 48 st. 3, verbatim (req. 5). The article names the only purposes for
/// which this evidencija may be used at all, so the izvod carries them.
const SVRHE_CL_48_ST_3: &str = "ocena zakonitosti obrade · interni nadzor · \
     obezbeđivanje integriteta i bezbednosti podataka · pokretanje i vođenje krivičnog postupka";

/// Why the shop keeps this at all — stated accurately, because SW-10 is a
/// prudential control and čl. 48 binds a nadležni organ, not a boutique.
const IZVOD_NAPOMENA: &str = "Rukovalac ovu evidenciju vodi kao sopstvenu meru: ZZPL čl. 48 \
     obavezuje nadležni organ koji podatke obrađuje u posebne svrhe. Evidencija služi \
     odgovornosti za postupanje (ZZPL čl. 5 st. 2) i mogućnosti predočavanja primene načela \
     obrade (ZZPL čl. 41 st. 1).";

const IZVOD_KOLONE: [&str; 11] = [
    "Redni broj",
    "Datum i vreme",
    "Lice koje je izvršilo radnju",
    "Radnja (ZZPL čl. 48 st. 1)",
    "Vrsta objekta",
    "Oznaka objekta",
    "Razlog (ZZPL čl. 48 st. 2)",
    "Primalac (ZZPL čl. 48 st. 2)",
    "Sesija tehničke podrške",
    "Otisak prethodnog zapisa",
    "Otisak zapisa",
];

fn izvod_to_csv(result: &AuditSearchResult, period: &str, actor: &str, now: &str) -> String {
    let mut lines = vec![
        csv_line(&[IZVOD_NASLOV]),
        csv_line(&["Pravni osnov predaje izvoda", IZVOD_OSNOV]),
        csv_line(&["Uzor za izvod", IZVOD_UZOR]),
        csv_line(&[
            "Svrha korišćenja evidencije (ZZPL čl. 48 st. 3)",
            SVRHE_CL_48_ST_3,
        ]),
        csv_line(&["Napomena o osnovu vođenja", IZVOD_NAPOMENA]),
        csv_line(&["Period", period]),
        csv_line(&["Lice", actor]),
        csv_line(&["Izvod napravljen", now]),
        csv_line(&["Broj zapisa u izvodu", &result.events.len().to_string()]),
        csv_line(&["Celovitost lanca", &result.chain.label]),
        csv_line(&[
            "Provereno zapisa u celoj evidenciji",
            &result.chain.checked_rows.to_string(),
        ]),
        String::new(),
        csv_line(&IZVOD_KOLONE),
    ];

    for (index, event) in result.events.iter().enumerate() {
        lines.push(csv_line(&[
            &(index + 1).to_string(),
            &event.at,
            &actor_label(event),
            &event.action_label,
            &event.object_type_label,
            &event.object_id,
            event.reason_label.as_deref().unwrap_or(""),
            event.recipient_label.as_deref().unwrap_or(""),
            &event
                .support_session_id
                .map(|id| id.to_string())
                .unwrap_or_default(),
            &event.prev_hash,
            &event.hash,
        ]));
    }

    lines.join("\n")
}

/// The name behind the id, for the person reading the document.
fn actor_label(event: &AuditEvent) -> String {
    match (event.actor_user_id, event.actor_name.as_deref()) {
        (Some(id), Some(name)) => format!("{name} (ID {id})"),
        (Some(id), None) => format!("ID {id}"),
        // Req. 23: the purge is time-driven, so its line has no human actor.
        _ => "Automatska obrada (bez korisnika)".to_string(),
    }
}

fn actor_filter_label(actor: Option<&(i64, Option<String>)>) -> String {
    match actor {
        None => "Sva lica".to_string(),
        Some((id, Some(name))) => format!("{name} (ID {id})"),
        Some((id, None)) => format!("ID {id}"),
    }
}

fn period_label(from: Option<&str>, to: Option<&str>) -> String {
    match (from, to) {
        (Some(from), Some(to)) => format!("od {from} do {to}"),
        (Some(from), None) => format!("od {from}"),
        (None, Some(to)) => format!("do {to}"),
        (None, None) => "cela evidencija".to_string(),
    }
}

/// Built from the VALIDATED range and an integer id, so nothing a caller typed
/// reaches a path.
fn izvod_file_name(from: Option<&str>, to: Option<&str>, actor_user_id: Option<i64>) -> String {
    let mut file_name = String::from("izvod-evidencija-pristupa");
    if let Some(from) = from {
        file_name.push_str(&format!("-od-{from}"));
    }
    if let Some(to) = to {
        file_name.push_str(&format!("-do-{to}"));
    }
    if let Some(actor_user_id) = actor_user_id {
        file_name.push_str(&format!("-lice-{actor_user_id}"));
    }
    file_name.push_str(".csv");
    file_name
}

/// The stored codes as Serbian prose. A raw `sudski_ili_upravni_postupak` in a
/// document for the Poverenik is not the „clean, human-readable export“ req. 8
/// asks for — and the fallback is the code itself, so an unknown value is shown
/// rather than swallowed.
fn action_label(code: &str) -> String {
    AuditAction::from_code(code).map_or_else(
        || code.to_string(),
        |action| {
            match action {
                AuditAction::Unos => "Unos",
                AuditAction::Menjanje => "Menjanje",
                AuditAction::Uvid => "Uvid",
                AuditAction::Otkrivanje => "Otkrivanje (uključujući i prenos)",
                AuditAction::Uporedjivanje => "Upoređivanje",
                AuditAction::Brisanje => "Brisanje",
            }
            .to_string()
        },
    )
}

fn object_type_label(code: &str) -> String {
    AuditObjectType::from_code(code).map_or_else(
        || code.to_string(),
        |object_type| {
            match object_type {
                AuditObjectType::Sale => "Račun",
                AuditObjectType::Employee => "Nalog zaposlenog",
                AuditObjectType::PersonnelRecord => "Evidencija o zaposlenom",
                AuditObjectType::Credentials => "Pristupni podaci",
                AuditObjectType::SupportSession => "Sesija tehničke podrške",
                AuditObjectType::DataBreach => "Povreda podataka o ličnosti",
                AuditObjectType::ProcessingActivity => "Radnja obrade",
                AuditObjectType::RetentionPolicy => "Rok čuvanja",
                AuditObjectType::AuditLog => "Evidencija pristupa",
                AuditObjectType::Backup => "Rezervna kopija",
                AuditObjectType::PopisSession => "Popis imovine i obaveza",
            }
            .to_string()
        },
    )
}

fn reason_label(code: &str) -> String {
    AuditReason::from_code(code).map_or_else(
        || code.to_string(),
        |reason| {
            match reason {
                AuditReason::Inspekcija => "Inspekcijski nadzor",
                AuditReason::ZahtevLica => "Zahtev lica na koje se podaci odnose",
                AuditReason::InterniNadzor => "Interni nadzor",
                AuditReason::ObradaReklamacije => "Obrada reklamacije",
                AuditReason::ObracunZarade => "Obračun zarade",
                AuditReason::TehnickaPodrska => "Tehnička podrška",
                AuditReason::SudskiIliUpravniPostupak => "Sudski ili upravni postupak",
                AuditReason::BezbednosniIncident => "Bezbednosni incident",
                AuditReason::ZakonskaObaveza => "Zakonska obaveza",
                AuditReason::AutomatskoCiscenje => "Automatsko čišćenje po roku čuvanja",
            }
            .to_string()
        },
    )
}

fn recipient_label(code: &str) -> String {
    AuditRecipient::from_code(code).map_or_else(
        || code.to_string(),
        |recipient| {
            match recipient {
                AuditRecipient::LiceNaKojeSePodaciOdnose => "Lice na koje se podaci odnose",
                AuditRecipient::PoreskaUprava => "Poreska uprava",
                AuditRecipient::Inspekcija => "Inspekcija",
                AuditRecipient::Poverenik => {
                    "Poverenik za informacije od javnog značaja i zaštitu podataka o ličnosti"
                }
                AuditRecipient::SudIliJavniTuzilac => "Sud ili javni tužilac",
                AuditRecipient::Mup => "Ministarstvo unutrašnjih poslova",
                AuditRecipient::Knjigovodja => "Knjigovođa",
                AuditRecipient::ObradjivacTehnickePodrske => "Obrađivač tehničke podrške",
                AuditRecipient::Banka => "Banka",
                AuditRecipient::DrugiOrgan => "Drugi organ",
            }
            .to_string()
        },
    )
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

    // -----------------------------------------------------------------------
    // Task 4 — the general write path, the read path and the čl. 48 st. 4 izvod
    // -----------------------------------------------------------------------

    /// An `uvid` into an employee's personnel record — the commonest line the
    /// app will write, and one that carries a čl. 48 st. 2 razlog.
    fn uvid_entry(object_id: &str) -> AuditEntry {
        AuditEntry {
            action: AuditAction::Uvid,
            object_type: AuditObjectType::PersonnelRecord,
            object_id: object_id.to_string(),
            reason_code: Some(AuditReason::InterniNadzor),
            recipient: None,
            support_session_id: None,
        }
    }

    fn whole_log() -> AuditQuery {
        AuditQuery::default()
    }

    fn read_file_and_remove(exported: &ExportedFile) -> String {
        let csv = std::fs::read_to_string(&exported.path).expect("the izvod should be on disk");
        std::fs::remove_file(&exported.path).expect("the izvod should be removable");
        csv
    }

    /// Every line the app writes goes on the END of the chain, and the chain the
    /// reader walks back is the chain the writer built.
    #[test]
    fn a_recorded_row_chains_onto_the_previous() {
        with_state("a_recorded_row_chains_onto_the_previous", |state| {
            let admin_id = sign_in_admin(state);

            record_audit(state, uvid_entry("3"), "2026-08-01T09:00:00Z")
                .expect("the first line should record");
            record_audit(state, uvid_entry("4"), "2026-08-01T09:05:00Z")
                .expect("the second line should record");

            let rows = audit_rows(state);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0].7, GENESIS_PREV_HASH, "the first row is genesis");
            assert_eq!(rows[1].7, rows[0].8, "the second carries the first's hash");
            assert_eq!(rows[0].5, Some(admin_id));

            let result = search(state, &whole_log()).expect("the log should read back");
            assert_eq!(result.chain.verdict, ChainVerdict::Intact);
            assert!(result.chain.intact);
            assert_eq!(result.events.len(), 2);
            assert_eq!(result.events[0].action, "uvid");
            assert_eq!(result.events[0].object_type, "personnel_record");
            assert_eq!(
                result.events[0].reason_code.as_deref(),
                Some("interni_nadzor")
            );
            assert_eq!(
                result.events[0].actor_name.as_deref(),
                Some("Administrator"),
                "the reader resolves the actor's name for the operator"
            );
        });
    }

    /// Req. 4 is enforced at the write boundary or it is enforced nowhere, and
    /// this is the door every feature of the app will use.
    #[test]
    fn record_audit_refuses_forbidden_content_before_the_row_lands() {
        with_state(
            "record_audit_refuses_forbidden_content_before_the_row_lands",
            |state| {
                sign_in_admin(state);

                for object_id in ["0101990710015", "4111111111111111", "Petar Petrović"] {
                    let error = record_audit(state, uvid_entry(object_id), "2026-08-01T09:00:00Z")
                        .expect_err("the write boundary must refuse this id");
                    assert_eq!(error.code(), "audit_forbidden_content", "for {object_id}");
                }

                // The čl. 48 st. 2 leg of the same gate.
                let error = record_audit(
                    state,
                    AuditEntry {
                        reason_code: None,
                        ..uvid_entry("3")
                    },
                    "2026-08-01T09:00:00Z",
                )
                .expect_err("an uvid without a razlog must be refused");
                assert_eq!(error.code(), "audit_missing_reason");

                assert!(
                    audit_rows(state).is_empty(),
                    "a refused entry must not reach audit_events at all"
                );
            },
        );
    }

    /// The two fields a caller may not choose. A caller that picks `at` can
    /// backdate a line; one that picks the actor can attribute an access to a
    /// colleague — and an attributable access is the whole point of the log.
    #[test]
    fn record_audit_stamps_the_instant_and_the_actor_itself() {
        with_state(
            "record_audit_stamps_the_instant_and_the_actor_itself",
            |state| {
                let cashier_id = sign_in_cashier(state);

                record_audit(state, uvid_entry("7"), "2026-08-01T09:00:00Z")
                    .expect("the line should record");

                let at: String = state
                    .db()
                    .open()
                    .expect("database should open")
                    .query_row("SELECT at FROM audit_events", [], |row| row.get(0))
                    .expect("the row should read back");
                assert_eq!(at, "2026-08-01T09:00:00Z", "the instant is the parameter");
                assert_eq!(audit_rows(state)[0].5, Some(cashier_id));

                // The čl. 5 st. 1 t. 5 purge is time-driven, not request-driven
                // (req. 23), so its line has no human actor at all — and a
                // fabricated actor id would be worse evidence than an absent one.
                state.clear_session().expect("the session should clear");
                record_audit(
                    state,
                    AuditEntry {
                        action: AuditAction::Brisanje,
                        object_type: AuditObjectType::Credentials,
                        object_id: "7".to_string(),
                        reason_code: None,
                        recipient: None,
                        support_session_id: None,
                    },
                    "2026-08-02T03:00:00Z",
                )
                .expect("an automatic line should record without a session");
                assert_eq!(audit_rows(state)[1].5, None);
            },
        );
    }

    /// Req. 7: „no owner-facing edit or delete path“. An owner-editable audit
    /// log proves nothing, and proving something is the entire reason it exists.
    ///
    /// The needles are composed at run time so that this assertion cannot match
    /// its own source text.
    #[test]
    fn no_audit_command_can_edit_or_delete_a_logged_row() {
        const LIB_RS: &str = include_str!("../lib.rs");
        const THIS_MODULE: &str = include_str!("audit.rs");

        let registered: Vec<&str> = LIB_RS
            .lines()
            .filter_map(|line| line.trim().strip_prefix("commands::audit::"))
            .map(|name| name.trim_end_matches(','))
            .collect();
        assert_eq!(
            registered,
            vec![
                "support_grant_access",
                "support_request_access",
                "support_end_session",
                "support_active_session",
                "audit_search",
                "audit_export_csv",
            ],
            "the log exposes a read path and nothing else"
        );

        // The shipped half of the module only. The tests below deliberately
        // remove rows — that is how tampering and the SW-13 expiry are staged —
        // and the claim being pinned is about the code that ships.
        let shipped = THIS_MODULE
            .split_once("\n#[cfg(test)]")
            .map_or(THIS_MODULE, |(code, _tests)| code);
        let update = format!("{} audit_events", "UPDATE");
        let delete = format!("{} FROM audit_events", "DELETE");
        assert!(
            !shipped.contains(&update),
            "no statement in this module may rewrite a logged row"
        );
        assert!(
            !shipped.contains(&delete),
            "and none may remove one either — expiry is SW-13's, under a policy"
        );
    }

    /// And the database refuses the edit even if a statement ever reached it.
    #[test]
    fn the_database_itself_refuses_to_rewrite_a_logged_row() {
        with_state(
            "the_database_itself_refuses_to_rewrite_a_logged_row",
            |state| {
                sign_in_admin(state);
                record_audit(state, uvid_entry("3"), "2026-08-01T09:00:00Z")
                    .expect("the line should record");

                let connection = state.db().open().expect("database should open");
                let statement = format!("{} audit_events SET object_id = '9'", "UPDATE");
                let error = connection
                    .execute(&statement, [])
                    .expect_err("v18's trigger must refuse the rewrite");
                assert!(
                    error.to_string().contains("ne može da se menja"),
                    "the refusal should be the trigger's, got {error}"
                );
            },
        );
    }

    /// Req. 8: „a clean, human-readable export by date range“, modelled on
    /// čl. 48 st. 4.
    #[test]
    fn the_izvod_covers_a_date_range() {
        with_state("the_izvod_covers_a_date_range", |state| {
            sign_in_admin(state);
            for (object_id, at) in [
                ("1", "2026-07-31T23:00:00Z"),
                ("2", "2026-08-01T09:00:00Z"),
                ("3", "2026-08-02T09:00:00Z"),
            ] {
                record_audit(state, uvid_entry(object_id), at).expect("the line should record");
            }

            let exported = export_csv(
                state,
                &AuditQuery {
                    from: Some("2026-08-01".to_string()),
                    to: Some("2026-08-01".to_string()),
                    actor_user_id: None,
                },
                "2026-08-03T10:00:00Z",
            )
            .expect("the izvod should render");
            let csv = read_file_and_remove(&exported);

            assert_eq!(exported.row_count, 1);
            assert!(csv.contains("2026-08-01T09:00:00Z"), "the day asked for");
            assert!(
                !csv.contains("2026-07-31T23:00:00Z"),
                "the day before the range is not disclosed"
            );
            assert!(
                !csv.contains("2026-08-02T09:00:00Z"),
                "nor the day after it"
            );
            assert!(csv.contains("od 2026-08-01 do 2026-08-01"));
        });
    }

    /// Req. 8's second axis: „and by actor“.
    #[test]
    fn the_izvod_covers_an_actor() {
        with_state("the_izvod_covers_an_actor", |state| {
            let cashier_id = sign_in_cashier(state);
            record_audit(state, uvid_entry("11"), "2026-08-01T09:00:00Z")
                .expect("the kasir's line should record");

            let admin_id = sign_in_admin(state);
            record_audit(state, uvid_entry("22"), "2026-08-01T09:30:00Z")
                .expect("the vlasnik's line should record");

            // Read first: the izvod below records its own disclosure line, and
            // that line is the vlasnik's.
            let result = search(
                state,
                &AuditQuery {
                    from: None,
                    to: None,
                    actor_user_id: Some(admin_id),
                },
            )
            .expect("the search should run");
            assert_eq!(result.events.len(), 1);
            assert_eq!(result.events[0].actor_user_id, Some(admin_id));

            let exported = export_csv(
                state,
                &AuditQuery {
                    from: None,
                    to: None,
                    actor_user_id: Some(cashier_id),
                },
                "2026-08-03T10:00:00Z",
            )
            .expect("the izvod should render");
            let csv = read_file_and_remove(&exported);

            assert_eq!(exported.row_count, 1);
            assert!(csv.contains("Kasir Kasirović"), "the actor is named");
            assert!(
                !csv.contains("2026-08-01T09:30:00Z"),
                "another actor's line is not disclosed"
            );
        });
    }

    /// „Must render offline, from the till“ (req. 8): a complete document on the
    /// local disk beside the database, readable with nothing else to hand.
    #[test]
    fn the_izvod_renders_offline_and_reads_as_a_document() {
        with_state(
            "the_izvod_renders_offline_and_reads_as_a_document",
            |state| {
                sign_in_admin(state);
                record_audit(state, uvid_entry("3"), "2026-08-01T09:00:00Z")
                    .expect("the line should record");

                let exported = export_csv(state, &whole_log(), "2026-08-03T10:00:00Z")
                    .expect("the izvod should render");
                let export_dir = state
                    .db()
                    .path()
                    .parent()
                    .expect("the database has a parent directory")
                    .join("exports");
                assert!(
                    exported.path.starts_with(&export_dir.display().to_string()),
                    "the izvod is written beside the database, not fetched"
                );
                assert_eq!(exported.mime_type, "text/csv");
                let csv = read_file_and_remove(&exported);

                assert!(csv.contains("IZVOD IZ EVIDENCIJE PRISTUPA PODACIMA O LIČNOSTI"));
                // Req. 8 names two provisions and they are NOT interchangeable.
                // Čl. 48 st. 4 binds a nadležni organ (§3 V1), so it is the
                // uzor for the document's shape and may never be printed as
                // this shop's pravni osnov; the osnov on which a preduzetnik
                // hands the izvod over is čl. 49's duty to cooperate.
                let osnov_line = csv
                    .lines()
                    .find(|line| line.starts_with("Pravni osnov predaje izvoda,"))
                    .expect("the izvod states the basis on which it is handed over");
                assert!(
                    osnov_line.contains("ZZPL čl. 49")
                        && osnov_line.contains(
                            "sarađuje sa Poverenikom u vršenju njegovih \
                             ovlašćenja"
                        ),
                    "the osnov for handing the izvod over is čl. 49, got {osnov_line}"
                );
                let uzor_line = csv
                    .lines()
                    .find(|line| line.contains("ZZPL čl. 48 st. 4"))
                    .expect("the izvod names the model it was built on");
                assert!(
                    uzor_line.starts_with("Uzor za izvod,")
                        && uzor_line.contains(
                            "obavezuje nadležni organ, pa je ovde uzor, a ne \
                             osnov"
                        ),
                    "čl. 48 st. 4 is the uzor, never the osnov, got {uzor_line}"
                );
                // The čl. 48 st. 3 purpose lock, verbatim (req. 5).
                for svrha in [
                    "ocena zakonitosti obrade",
                    "interni nadzor",
                    "obezbeđivanje integriteta i bezbednosti podataka",
                    "pokretanje i vođenje krivičnog postupka",
                ] {
                    assert!(csv.contains(svrha), "the izvod must state „{svrha}“");
                }
                // Human-readable: the stored codes are rendered as Serbian prose.
                assert!(csv.contains("Uvid"));
                assert!(csv.contains("Evidencija o zaposlenom"));
                assert!(csv.contains("Interni nadzor"));
                assert!(csv.contains("Celovitost lanca"));
                assert!(
                    csv.contains("Administrator"),
                    "the actor is named for the reader"
                );

                // Req. 1: never tell anyone ZZPL requires this log, and never soften
                // an unpenalised article into „nema posledice“.
                assert!(!csv.contains("nema posledice"));
                assert!(!csv.to_lowercase().contains("zakon zahteva"));
                assert!(!csv.to_lowercase().contains("propisana obaveza rukovaoca"));
                // No fine figure lives outside legal.rs, and none belongs here.
                assert!(!csv.contains("RSD") && !csv.to_lowercase().contains("dinar"));
            },
        );
    }

    /// The izvod leaves the till, which is exactly the čl. 48 st. 2 fact worth
    /// recording: data disclosed, to a named class of recipient, for a razlog.
    #[test]
    fn the_izvod_is_recorded_as_a_disclosure_to_the_poverenik() {
        with_state(
            "the_izvod_is_recorded_as_a_disclosure_to_the_poverenik",
            |state| {
                let admin_id = sign_in_admin(state);
                record_audit(state, uvid_entry("3"), "2026-08-01T09:00:00Z")
                    .expect("the line should record");

                let exported = export_csv(state, &whole_log(), "2026-08-03T10:00:00Z")
                    .expect("the izvod should render");
                let csv = read_file_and_remove(&exported);
                assert_eq!(exported.row_count, 1);

                let rows = audit_rows(state);
                assert_eq!(rows.len(), 2, "the izvod itself is the second line");
                let disclosure = &rows[1];
                assert_eq!(disclosure.0, "otkrivanje");
                assert_eq!(disclosure.1, "audit_log");
                assert_eq!(disclosure.2, "1", "how far the disclosure reached");
                assert_eq!(disclosure.3.as_deref(), Some("inspekcija"));
                assert_eq!(disclosure.4.as_deref(), Some("poverenik"));
                assert_eq!(disclosure.5, Some(admin_id));
                assert_eq!(
                    disclosure.7, rows[0].8,
                    "and it chains onto the line it disclosed"
                );

                assert!(
                    !csv.contains("otkrivanje"),
                    "the izvod records the state at the moment it was rendered"
                );
            },
        );
    }

    /// The whole shop's access log in one file is not a cashier's to read, and
    /// čl. 48 st. 4 puts it in the rukovalac's hands.
    #[test]
    fn the_search_and_the_izvod_are_admin_gated() {
        with_state("the_search_and_the_izvod_are_admin_gated", |state| {
            sign_in_admin(state);
            record_audit(state, uvid_entry("3"), "2026-08-01T09:00:00Z")
                .expect("the line should record");

            sign_in_cashier(state);
            assert_eq!(
                search(state, &whole_log())
                    .expect_err("a kasir may not read the log")
                    .code(),
                "forbidden"
            );
            assert_eq!(
                export_csv(state, &whole_log(), "2026-08-03T10:00:00Z")
                    .expect_err("nor export it")
                    .code(),
                "forbidden"
            );
            assert_eq!(
                audit_rows(state).len(),
                1,
                "a refused izvod discloses nothing and logs no disclosure"
            );
        });
    }

    #[test]
    fn a_malformed_date_range_is_refused() {
        with_state("a_malformed_date_range_is_refused", |state| {
            sign_in_admin(state);

            for (from, to) in [
                (Some("01.08.2026"), None),
                (Some("2026-08-01T09:00:00Z"), None),
                (None, Some("2026-13-01")),
                (Some("2026-08-02"), Some("2026-08-01")),
            ] {
                let query = AuditQuery {
                    from: from.map(str::to_string),
                    to: to.map(str::to_string),
                    actor_user_id: None,
                };
                let error = search(state, &query).expect_err("the range is not readable");
                assert_eq!(error.code(), "validation_error", "for {from:?}–{to:?}");
            }
        });
    }

    /// Tamper-evidence outranks completeness (req. 7): the reader reports what
    /// the chain says, and the operator's izvod carries the verdict.
    #[test]
    fn the_reader_reports_a_broken_chain() {
        with_state("the_reader_reports_a_broken_chain", |state| {
            sign_in_admin(state);
            for object_id in ["1", "2", "3"] {
                record_audit(state, uvid_entry(object_id), "2026-08-01T09:00:00Z")
                    .expect("the line should record");
            }

            let connection = state.db().open().expect("database should open");
            connection
                .execute("DELETE FROM audit_events WHERE id = 2", [])
                .expect("a row can be removed — only the chain notices");

            let result = search(state, &whole_log()).expect("the search should still run");
            assert!(!result.chain.intact);
            assert!(
                matches!(result.chain.verdict, ChainVerdict::BrokenAt(1)),
                "the survivor after the gap still carries the vanished row's hash"
            );

            // And the tail: the survivors reconcile, but the log no longer ends
            // where the sequence says it ended.
            connection
                .execute("DELETE FROM audit_events WHERE id = 3", [])
                .expect("the newest row can be removed too");
            connection
                .execute("DELETE FROM audit_events WHERE id = 1", [])
                .expect("and so can the head");
            let result = search(state, &whole_log()).expect("the search should still run");
            assert!(!result.chain.intact);

            let exported = export_csv(state, &whole_log(), "2026-08-03T10:00:00Z")
                .expect("the izvod renders even on a broken chain");
            let csv = read_file_and_remove(&exported);
            assert!(
                csv.contains("Narušena"),
                "the izvod must not present a broken chain as an intact one"
            );
        });
    }

    /// The contract Task 2 left for the purge (SW-13): when expiry removes rows
    /// it persists the hash and the id of the LAST row it removed, and that pair
    /// is the anchor the chain is walked from. Without it the first legitimate
    /// purge would report a permanent break indistinguishable from tampering.
    #[test]
    fn a_head_purged_log_verifies_once_the_purge_anchor_is_recorded() {
        with_state(
            "a_head_purged_log_verifies_once_the_purge_anchor_is_recorded",
            |state| {
                sign_in_admin(state);
                for object_id in ["1", "2", "3"] {
                    record_audit(state, uvid_entry(object_id), "2026-08-01T09:00:00Z")
                        .expect("the line should record");
                }

                let rows = audit_rows(state);
                let purged_hash = rows[0].8.clone();

                let connection = state.db().open().expect("database should open");
                connection
                    .execute("DELETE FROM audit_events WHERE id = 1", [])
                    .expect("expiry removes the oldest row");
                assert!(
                    !search(state, &whole_log())
                        .expect("the search should run")
                        .chain
                        .intact,
                    "without the anchor a purged head is indistinguishable from tampering"
                );

                connection
                    .execute(
                        "INSERT INTO settings (key, value_json, updated_at)
                         VALUES ('audit_chain_anchor', json_object('hash', ?1, 'id', 1), ?2)",
                        params![purged_hash, "2026-08-03T03:00:00Z"],
                    )
                    .expect("the purge records its anchor");

                let result = search(state, &whole_log()).expect("the search should run");
                assert_eq!(result.chain.verdict, ChainVerdict::Intact);
                assert_eq!(result.events.len(), 2);
            },
        );
    }

    /// The izvod is read by a person, so every stored code has to have Serbian
    /// prose behind it — a raw `sudski_ili_upravni_postupak` in a document for
    /// the Poverenik is not the „clean, human-readable export“ req. 8 asks for.
    #[test]
    fn every_stored_code_renders_as_serbian_prose() {
        let labels: Vec<(String, String)> = AuditAction::ALL
            .into_iter()
            .map(|action| (action.as_code().to_string(), action_label(action.as_code())))
            .chain(AuditObjectType::ALL.into_iter().map(|object_type| {
                (
                    object_type.as_code().to_string(),
                    object_type_label(object_type.as_code()),
                )
            }))
            .chain(
                AuditReason::ALL
                    .into_iter()
                    .map(|reason| (reason.as_code().to_string(), reason_label(reason.as_code()))),
            )
            .chain(AuditRecipient::ALL.into_iter().map(|recipient| {
                (
                    recipient.as_code().to_string(),
                    recipient_label(recipient.as_code()),
                )
            }))
            .collect();

        for (code, label) in &labels {
            assert_ne!(label, code, "„{code}“ is a storage code, not prose");
            assert!(!label.contains('_'), "„{label}“ still reads as a code");
            assert!(
                label.starts_with(|first: char| first.is_uppercase()),
                "„{label}“ should read as a sentence"
            );
        }
        assert_eq!(labels.len(), 37, "every code in every vocabulary");
    }
}
