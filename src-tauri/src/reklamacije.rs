//! Reklamacije (consumer complaints) — the regime-versioned deadline engine.
//!
//! Legal authority: `docs/ZZP-REKLAMACIJE-VERIFIED-RULES.md`. Design:
//! `docs/superpowers/specs/2026-07-19-reklamacije-design.md`.
//!
//! This is a DEADLINE ENGINE shown to shops. Deadlines are DERIVED, never
//! stored: `compute_deadlines` recomputes them from the event log on every read.
//! `today` is passed INTO the pure functions — never `datetime('now')` inside
//! them — so the same events always produce the same answer for a given day.
//! A date that grants MORE time than the law allows is the harm, so when a
//! consented extension and the statutory clock disagree, the tighter date wins.

// The engine's public API is consumed by the commands and renderers added in
// later SW-7 tasks; keep the staged API green here.
#![allow(dead_code)]

use crate::app_error::AppError;
use rusqlite::{params, Connection, OptionalExtension};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

pub const CUTOVER_DATE: &str = "2026-08-01"; // ZZP 35/2026 application. 1-vs-2-Aug UNRESOLVED; earlier = safe. Counsel-flag.
pub const REGIME_OLD: &str = "old";
pub const REGIME_NEW: &str = "new";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineEvent {
    pub event_type: String,
    pub event_date: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineState {
    pub answer_due: String,
    pub resolution_due: Option<String>, // None while paused / at impasse / resolved
    pub clock: String,                  // "running" | "paused" | "impasse" | "resolved"
    pub consumer_window_due: Option<String>,
    pub answer_overdue: bool,
    pub resolution_overdue: bool,
    pub one_extension_used: bool,
}

fn parse_rfc3339(value: &str, field: &str) -> Result<OffsetDateTime, AppError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|source| {
        AppError::validation(
            format!("Datum nije ispravan: {source}"),
            serde_json::json!({ "field": field }),
        )
    })
}

/// Adds calendar days to an RFC3339 instant. Inputs are validated by the caller
/// before any helper runs, so parsing here cannot fail in practice.
fn add_days(rfc3339: &str, days: i64) -> String {
    let dt = OffsetDateTime::parse(rfc3339, &Rfc3339).expect("caller pre-validated rfc3339");
    (dt + Duration::days(days))
        .format(&Rfc3339)
        .expect("a valid datetime always formats")
}

/// Whole calendar days between `a` and `b` (`a - b`), compared on `.date()`.
fn days_between(a: &str, b: &str) -> i64 {
    let da = OffsetDateTime::parse(a, &Rfc3339)
        .expect("caller pre-validated rfc3339")
        .date();
    let db = OffsetDateTime::parse(b, &Rfc3339)
        .expect("caller pre-validated rfc3339")
        .date();
    (da - db).whole_days()
}

/// Returns the earlier of two instants (its original string), by `.date()`.
fn min_date(a: &str, b: &str) -> String {
    let da = OffsetDateTime::parse(a, &Rfc3339)
        .expect("caller pre-validated rfc3339")
        .date();
    let db = OffsetDateTime::parse(b, &Rfc3339)
        .expect("caller pre-validated rfc3339")
        .date();
    if db < da {
        b.to_string()
    } else {
        a.to_string()
    }
}

/// True iff `date(a) > date(b)` — calendar-day strict comparison, never rolled
/// over a weekend or holiday (that could only extend a date, never the harm we
/// guard against).
fn date_gt(a: &str, b: &str) -> bool {
    let da = OffsetDateTime::parse(a, &Rfc3339)
        .expect("caller pre-validated rfc3339")
        .date();
    let db = OffsetDateTime::parse(b, &Rfc3339)
        .expect("caller pre-validated rfc3339")
        .date();
    da > db
}

fn earliest_event<'a>(events: &'a [DeadlineEvent], event_type: &str) -> Option<&'a DeadlineEvent> {
    events
        .iter()
        .filter(|e| e.event_type == event_type)
        .min_by(|a, b| a.event_date.cmp(&b.event_date))
}

fn latest_event<'a>(events: &'a [DeadlineEvent], event_type: &str) -> Option<&'a DeadlineEvent> {
    events
        .iter()
        .filter(|e| e.event_type == event_type)
        .max_by(|a, b| a.event_date.cmp(&b.event_date))
}

/// The regime is chosen once, at filing, from the filing date vs `CUTOVER_DATE`,
/// and NEVER recomputed afterwards. Boundary: 2026-07-31 → old, 2026-08-01 → new.
pub fn regime_for(filed_at: &str) -> Result<&'static str, AppError> {
    let filed = parse_rfc3339(filed_at, "filedAt")?.date().to_string();
    if filed.as_str() < CUTOVER_DATE {
        Ok(REGIME_OLD)
    } else {
        Ok(REGIME_NEW)
    }
}

/// Derives the current deadline state from the event log for a given `today`.
///
/// Pure and total over validated dates: no clock, no I/O, no persistence. See
/// memo §2 for the verbatim math. Both regimes pause the resolution clock
/// between `consumer_received_answer` and `consumer_responded`; on response OLD
/// restarts to a fresh full span while NEW resumes (base + suspension). The
/// 8-day answer clock never pauses. Silence past received + 3 days is impasse.
pub fn compute_deadlines(
    regime: &str,
    filed_at: &str,
    roba_kind: &str,
    events: &[DeadlineEvent],
    today: &str,
) -> Result<DeadlineState, AppError> {
    // Validate every date up front so the pure helpers can parse infallibly.
    parse_rfc3339(filed_at, "filedAt")?;
    parse_rfc3339(today, "today")?;
    for e in events {
        parse_rfc3339(&e.event_date, "eventDate")?;
    }

    let span: i64 = if roba_kind == "tehnicka" || roba_kind == "namestaj" {
        30
    } else {
        15
    };
    let answer_due = add_days(filed_at, 8);
    let base = add_days(filed_at, span);

    let received = earliest_event(events, "consumer_received_answer");
    let responded = earliest_event(events, "consumer_responded");
    let answer_given = events.iter().find(|e| e.event_type == "answer_given");
    let resolved = events.iter().find(|e| e.event_type == "resolved");
    let ext = latest_event(events, "extension_granted");

    let one_extension_used = ext.is_some();

    if resolved.is_some() {
        return Ok(DeadlineState {
            answer_due,
            resolution_due: None,
            clock: "resolved".to_string(),
            consumer_window_due: None,
            answer_overdue: false,
            resolution_overdue: false,
            one_extension_used,
        });
    }

    let consumer_window_due = received.map(|r| add_days(&r.event_date, 3));

    let (clock, clock_due): (String, Option<String>) = match (received, responded) {
        // Awaiting the consumer's response: the resolution clock is suspended.
        (Some(r), None) => {
            if date_gt(today, &add_days(&r.event_date, 3)) {
                ("impasse".to_string(), None)
            } else {
                ("paused".to_string(), None)
            }
        }
        // Consumer responded: OLD restarts to a fresh full span; NEW resumes.
        (Some(r), Some(resp)) => {
            let due = if regime == REGIME_OLD {
                add_days(&resp.event_date, span)
            } else {
                let suspension_days = days_between(&resp.event_date, &r.event_date);
                add_days(&base, suspension_days)
            };
            ("running".to_string(), Some(due))
        }
        // No round-trip yet: the clock runs from the base deadline.
        (None, _) => ("running".to_string(), Some(base.clone())),
    };

    // Extension tiebreak (safe = tighter): a consented extension can only pull
    // the resolution date in, never push it out.
    let resolution_due = match (clock_due, ext) {
        (Some(c), Some(e)) => Some(min_date(&c, &e.event_date)),
        (Some(c), None) => Some(c),
        (None, _) => None,
    };

    let answer_overdue = answer_given.is_none() && date_gt(today, &answer_due);
    let resolution_overdue = resolution_due.as_deref().is_some_and(|d| date_gt(today, d));

    Ok(DeadlineState {
        answer_due,
        resolution_due,
        clock,
        consumer_window_due,
        answer_overdue,
        resolution_overdue,
        one_extension_used,
    })
}

const MSG_REKLAMACIJA_NOT_FOUND: &str = "Reklamacija nije pronađena.";

/// The three `roba_kind` values the schema's CHECK constraint permits.
const ROBA_KINDS: [&str; 3] = ["opsta", "tehnicka", "namestaj"];

/// Retention floor: `filed_at + 2 years` (calendar days). Purge-*eligibility*
/// only — nothing auto-deletes; indefinite retention stays compliant (memo §3).
const RETENTION_DAYS: i64 = 730;

/// Intake payload from the command layer. Consumer PII (`podnosilac_ime_prezime`,
/// `kontakt`) is inline and admin-gated; there is no consent UI — the lawful
/// basis is the shop's legal obligation to keep the evidencija.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReklamacijaInput {
    pub podnosilac_ime_prezime: String,
    pub kontakt: Option<String>,
    pub podaci_o_robi: String,
    pub opis_nesaobraznosti: String,
    pub zahtev: String,
    pub roba_kind: String,
    pub filed_at: String,
}

/// One event-log row, projected for the UI. `consumer_consent` is surfaced as a
/// bool (it only ever carries meaning on `extension_granted`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventView {
    pub event_type: String,
    pub event_date: String,
    pub detail_json: Option<String>,
    pub consumer_consent: bool,
}

/// The full record: every persisted column, the frozen `regime`, the event log,
/// and the freshly `compute_deadlines`-derived `DeadlineState`. `status` is the
/// stored column; `deadlines.clock` is the derived running/paused/impasse view.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReklamacijaView {
    pub id: i64,
    pub register_number: i64,
    pub regime: String,
    pub status: String,
    pub filed_at: String,
    pub podnosilac_ime_prezime: String,
    pub kontakt: Option<String>,
    pub podaci_o_robi: String,
    pub opis_nesaobraznosti: String,
    pub zahtev: String,
    pub roba_kind: String,
    pub datum_izdavanja_potvrde: String,
    pub created_by: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
    pub events: Vec<EventView>,
    pub deadlines: DeadlineState,
    pub purge_eligible: bool,
}

/// Register-list row: the stored `status` plus the derived answer/resolution
/// dates and their overdue flags, so the list can show the deadline engine's
/// verdict without loading each record's full event log into the client.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReklamacijaSummary {
    pub id: i64,
    pub register_number: i64,
    pub regime: String,
    pub status: String,
    pub podnosilac_ime_prezime: String,
    pub filed_at: String,
    pub answer_due: String,
    pub resolution_due: Option<String>,
    pub answer_overdue: bool,
    pub resolution_overdue: bool,
    pub purge_eligible: bool,
}

fn require_non_empty(value: &str, field: &str) -> Result<(), AppError> {
    if value.trim().is_empty() {
        return Err(AppError::validation(
            "Obavezno polje ne sme biti prazno.",
            serde_json::json!({ "field": field }),
        ));
    }
    Ok(())
}

/// `filed_at + 2 years ≤ today`, compared on the calendar date. An eligibility
/// flag only — the module surfaces it; it never auto-deletes (memo §3).
fn is_purge_eligible(filed_at: &str, today: &str) -> bool {
    !date_gt(&add_days(filed_at, RETENTION_DAYS), today)
}

/// Loads the event log for one record, oldest first, in the UI projection.
fn load_events(conn: &Connection, reklamacija_id: i64) -> Result<Vec<EventView>, AppError> {
    let mut statement = conn.prepare(
        "SELECT event_type, event_date, detail_json, consumer_consent
         FROM reklamacija_events
         WHERE reklamacija_id = ?1
         ORDER BY event_date ASC, id ASC",
    )?;
    let events = statement
        .query_map(params![reklamacija_id], |row| {
            Ok(EventView {
                event_type: row.get(0)?,
                event_date: row.get(1)?,
                detail_json: row.get(2)?,
                consumer_consent: row.get::<_, i64>(3)? == 1,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(events)
}

/// Projects the event log into the pure engine's `DeadlineEvent` shape.
fn deadline_events(events: &[EventView]) -> Vec<DeadlineEvent> {
    events
        .iter()
        .map(|e| DeadlineEvent {
            event_type: e.event_type.clone(),
            event_date: e.event_date.clone(),
        })
        .collect()
}

/// Intake: validate, then in ONE transaction stamp the frozen `regime`, allocate
/// the sequential `register_number` (`MAX+1`, UNIQUE-guarded), and insert the
/// record with `datum_izdavanja_potvrde = now` and `status = 'open'`.
pub fn create_reklamacija(
    conn: &mut Connection,
    input: &ReklamacijaInput,
    acting_user_id: i64,
    now: &str,
) -> Result<ReklamacijaView, AppError> {
    require_non_empty(&input.podnosilac_ime_prezime, "podnosilacImePrezime")?;
    require_non_empty(&input.podaci_o_robi, "podaciORobi")?;
    require_non_empty(&input.opis_nesaobraznosti, "opisNesaobraznosti")?;
    require_non_empty(&input.zahtev, "zahtev")?;
    if !ROBA_KINDS.contains(&input.roba_kind.as_str()) {
        return Err(AppError::validation(
            "Vrsta robe nije prepoznata.",
            serde_json::json!({ "field": "robaKind" }),
        ));
    }
    // Also validates that `filed_at` parses; the regime is frozen here forever.
    let regime = regime_for(&input.filed_at)?;

    let tx = conn.transaction()?;
    // Single-instance desktop: `MAX+1` inside the intake transaction is atomic,
    // and the UNIQUE column is the backstop.
    let register_number: i64 = tx.query_row(
        "SELECT COALESCE(MAX(register_number), 0) + 1 FROM reklamacije",
        [],
        |row| row.get(0),
    )?;
    tx.execute(
        "INSERT INTO reklamacije (
            register_number, regime, status, filed_at, podnosilac_ime_prezime, kontakt,
            podaci_o_robi, opis_nesaobraznosti, zahtev, roba_kind, datum_izdavanja_potvrde,
            created_by, created_at, updated_at
         )
         VALUES (?1, ?2, 'open', ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?12)",
        params![
            register_number,
            regime,
            input.filed_at,
            input.podnosilac_ime_prezime,
            input.kontakt,
            input.podaci_o_robi,
            input.opis_nesaobraznosti,
            input.zahtev,
            input.roba_kind,
            now, // datum_izdavanja_potvrde — stamped at intake
            acting_user_id,
            now,
        ],
    )?;
    let id = tx.last_insert_rowid();
    tx.commit()?;

    get_reklamacija(conn, id, now)
}

/// Loads one record and derives its `DeadlineState` from the event log for
/// `today`. The stored `status` and derived clock travel together.
pub fn get_reklamacija(
    conn: &Connection,
    id: i64,
    today: &str,
) -> Result<ReklamacijaView, AppError> {
    #[allow(clippy::type_complexity)]
    let base: Option<(
        i64,
        i64,
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        Option<i64>,
        String,
        String,
    )> = conn
        .query_row(
            "SELECT id, register_number, regime, status, filed_at, podnosilac_ime_prezime,
                    kontakt, podaci_o_robi, opis_nesaobraznosti, zahtev, roba_kind,
                    datum_izdavanja_potvrde, created_by, created_at, updated_at
             FROM reklamacije
             WHERE id = ?1",
            params![id],
            |row| {
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
                    row.get(9)?,
                    row.get(10)?,
                    row.get(11)?,
                    row.get(12)?,
                    row.get(13)?,
                    row.get(14)?,
                ))
            },
        )
        .optional()?;
    let (
        id,
        register_number,
        regime,
        status,
        filed_at,
        podnosilac_ime_prezime,
        kontakt,
        podaci_o_robi,
        opis_nesaobraznosti,
        zahtev,
        roba_kind,
        datum_izdavanja_potvrde,
        created_by,
        created_at,
        updated_at,
    ) = base.ok_or_else(|| AppError::not_found(MSG_REKLAMACIJA_NOT_FOUND))?;

    let events = load_events(conn, id)?;
    let deadlines = compute_deadlines(
        &regime,
        &filed_at,
        &roba_kind,
        &deadline_events(&events),
        today,
    )?;
    let purge_eligible = is_purge_eligible(&filed_at, today);

    Ok(ReklamacijaView {
        id,
        register_number,
        regime,
        status,
        filed_at,
        podnosilac_ime_prezime,
        kontakt,
        podaci_o_robi,
        opis_nesaobraznosti,
        zahtev,
        roba_kind,
        datum_izdavanja_potvrde,
        created_by,
        created_at,
        updated_at,
        events,
        deadlines,
        purge_eligible,
    })
}

/// The whole register, newest filing first, each row carrying its derived
/// answer/resolution dates and overdue flags for `today`.
pub fn list_reklamacije(
    conn: &Connection,
    today: &str,
) -> Result<Vec<ReklamacijaSummary>, AppError> {
    // Collect the rows fully before iterating: `load_events` prepares its own
    // statement, which cannot overlap an active `query_map` on the same conn.
    let base: Vec<(i64, i64, String, String, String, String, String)> = {
        let mut statement = conn.prepare(
            "SELECT id, register_number, regime, status, podnosilac_ime_prezime, filed_at, roba_kind
             FROM reklamacije
             ORDER BY filed_at DESC, id DESC",
        )?;
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
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };

    let mut summaries = Vec::with_capacity(base.len());
    for (id, register_number, regime, status, podnosilac_ime_prezime, filed_at, roba_kind) in base {
        let events = load_events(conn, id)?;
        let deadlines = compute_deadlines(
            &regime,
            &filed_at,
            &roba_kind,
            &deadline_events(&events),
            today,
        )?;
        summaries.push(ReklamacijaSummary {
            id,
            register_number,
            regime,
            status,
            podnosilac_ime_prezime,
            answer_due: deadlines.answer_due,
            resolution_due: deadlines.resolution_due,
            answer_overdue: deadlines.answer_overdue,
            resolution_overdue: deadlines.resolution_overdue,
            purge_eligible: is_purge_eligible(&filed_at, today),
            filed_at,
        });
    }
    Ok(summaries)
}

// ── Lifecycle events ───────────────────────────────────────────────────────
// Every lifecycle mutation appends one `reklamacija_events` row and updates the
// record's `status`/`updated_at` in a single transaction. The `regime` is read
// from the stored record and NEVER recomputed — deadlines stay derived, the
// warning gate depends only on the frozen regime.

/// Verbatim from the plan (memo §4a, čl. 63 st. 10): the NEW-regime answer is
/// gated on this three-part express warning. Copy character-for-character.
const MSG_NEW_ANSWER_WARNING: &str = "Za novu reklamaciju odgovor mora sadržati izričito obaveštenje potrošaču o obavezi izjašnjenja, posledicama i zastoju rokova (čl. 63 st. 10).";

/// The answer payload. The three `warning_*` fields carry the express warning;
/// they are mandatory only under the NEW regime (see `log_answer`).
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerInput {
    pub answer_text: String,
    pub warning_duty: Option<String>,
    pub warning_consequences: Option<String>,
    pub warning_zastoj: Option<String>,
    pub event_date: String,
}

/// One event row to append. `consumer_consent` only ever carries meaning on an
/// `extension_granted` row.
struct NewEvent<'a> {
    event_type: &'a str,
    event_date: &'a str,
    detail_json: Option<&'a str>,
    consumer_consent: bool,
}

/// Loads the frozen `regime` for one record; doubles as the existence check.
fn load_regime(conn: &Connection, id: i64) -> Result<String, AppError> {
    conn.query_row(
        "SELECT regime FROM reklamacije WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )
    .optional()?
    .ok_or_else(|| AppError::not_found(MSG_REKLAMACIJA_NOT_FOUND))
}

/// True iff at least one event of `event_type` exists for the record.
fn event_exists(conn: &Connection, id: i64, event_type: &str) -> Result<bool, AppError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM reklamacija_events WHERE reklamacija_id = ?1 AND event_type = ?2",
        params![id, event_type],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

/// Appends one event row, carrying the acting user's id and the `now` stamp.
fn insert_event(
    conn: &Connection,
    id: i64,
    event: &NewEvent<'_>,
    acting: i64,
    now: &str,
) -> Result<(), AppError> {
    conn.execute(
        "INSERT INTO reklamacija_events
            (reklamacija_id, event_type, event_date, detail_json, consumer_consent, user_id, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            id,
            event.event_type,
            event.event_date,
            event.detail_json,
            i64::from(event.consumer_consent),
            acting,
            now
        ],
    )?;
    Ok(())
}

/// Sets the stored `status` and bumps `updated_at`.
fn set_status(conn: &Connection, id: i64, status: &str, now: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE reklamacije SET status = ?1, updated_at = ?2 WHERE id = ?3",
        params![status, now, id],
    )?;
    Ok(())
}

/// Bumps `updated_at` without changing the stored `status` (an extension changes
/// the derived resolution date, not the lifecycle status).
fn touch_updated_at(conn: &Connection, id: i64, now: &str) -> Result<(), AppError> {
    conn.execute(
        "UPDATE reklamacije SET updated_at = ?1 WHERE id = ?2",
        params![now, id],
    )?;
    Ok(())
}

/// Logs the shop's answer. **NEW regime:** rejected unless all three express-
/// warning fields are present and non-empty (čl. 63 st. 10). **OLD regime:** the
/// warning is optional — rejecting an old-regime answer for its absence would be
/// wrong. Stores the answer text + warnings in `detail_json`; status → answered.
pub fn log_answer(
    conn: &mut Connection,
    id: i64,
    input: &AnswerInput,
    acting: i64,
    now: &str,
) -> Result<ReklamacijaView, AppError> {
    require_non_empty(&input.answer_text, "answerText")?;
    parse_rfc3339(&input.event_date, "eventDate")?;

    let tx = conn.transaction()?;
    let regime = load_regime(&tx, id)?;

    if regime == REGIME_NEW {
        let all_present = [
            input.warning_duty.as_deref(),
            input.warning_consequences.as_deref(),
            input.warning_zastoj.as_deref(),
        ]
        .into_iter()
        .all(|w| w.is_some_and(|v| !v.trim().is_empty()));
        if !all_present {
            return Err(AppError::validation(
                MSG_NEW_ANSWER_WARNING,
                serde_json::json!({ "field": "warning" }),
            ));
        }
    }

    let detail = serde_json::json!({
        "answerText": input.answer_text,
        "warningDuty": input.warning_duty,
        "warningConsequences": input.warning_consequences,
        "warningZastoj": input.warning_zastoj,
    })
    .to_string();

    insert_event(
        &tx,
        id,
        &NewEvent {
            event_type: "answer_given",
            event_date: &input.event_date,
            detail_json: Some(&detail),
            consumer_consent: false,
        },
        acting,
        now,
    )?;
    set_status(&tx, id, "answered", now)?;
    tx.commit()?;
    get_reklamacija(conn, id, now)
}

/// Records that the consumer received the answer. Requires a prior `answer_given`;
/// opens the 3-day response window and suspends the resolution clock. Status →
/// awaiting_consumer.
pub fn log_consumer_received_answer(
    conn: &mut Connection,
    id: i64,
    event_date: &str,
    acting: i64,
    now: &str,
) -> Result<ReklamacijaView, AppError> {
    parse_rfc3339(event_date, "eventDate")?;
    let tx = conn.transaction()?;
    load_regime(&tx, id)?;
    if !event_exists(&tx, id, "answer_given")? {
        return Err(AppError::business(
            "invalid_state",
            "Prijem odgovora se može evidentirati tek nakon što je odgovor dat.",
        ));
    }
    insert_event(
        &tx,
        id,
        &NewEvent {
            event_type: "consumer_received_answer",
            event_date,
            detail_json: None,
            consumer_consent: false,
        },
        acting,
        now,
    )?;
    set_status(&tx, id, "awaiting_consumer", now)?;
    tx.commit()?;
    get_reklamacija(conn, id, now)
}

/// Records the consumer's response. Requires a prior `consumer_received_answer`;
/// OLD restarts the resolution clock to a fresh span, NEW resumes it. Status →
/// answered (running again).
pub fn log_consumer_response(
    conn: &mut Connection,
    id: i64,
    event_date: &str,
    acting: i64,
    now: &str,
) -> Result<ReklamacijaView, AppError> {
    parse_rfc3339(event_date, "eventDate")?;
    let tx = conn.transaction()?;
    load_regime(&tx, id)?;
    if !event_exists(&tx, id, "consumer_received_answer")? {
        return Err(AppError::business(
            "invalid_state",
            "Izjašnjenje potrošača se može evidentirati tek nakon evidentiranog prijema odgovora.",
        ));
    }
    insert_event(
        &tx,
        id,
        &NewEvent {
            event_type: "consumer_responded",
            event_date,
            detail_json: None,
            consumer_consent: false,
        },
        acting,
        now,
    )?;
    set_status(&tx, id, "answered", now)?;
    tx.commit()?;
    get_reklamacija(conn, id, now)
}

/// Grants the single permitted extension. Rejected if one already exists, and
/// rejected without the consumer's consent; the consented `new_deadline` is the
/// event's `event_date`, and it can only pull the resolution date in (the
/// engine's tighter-date tiebreak). The lifecycle status is unchanged.
pub fn grant_extension(
    conn: &mut Connection,
    id: i64,
    new_deadline: &str,
    consumer_consent: bool,
    reason: &str,
    acting: i64,
    now: &str,
) -> Result<ReklamacijaView, AppError> {
    parse_rfc3339(new_deadline, "newDeadline")?;
    let tx = conn.transaction()?;
    load_regime(&tx, id)?;
    if event_exists(&tx, id, "extension_granted")? {
        return Err(AppError::business(
            "invalid_state",
            "Produženje roka je moguće samo jednom (čl. 55/63 st. 11).",
        ));
    }
    if !consumer_consent {
        return Err(AppError::validation(
            "Produženje roka zahteva saglasnost potrošača.",
            serde_json::json!({ "field": "consumerConsent" }),
        ));
    }
    let detail = serde_json::json!({ "reason": reason }).to_string();
    insert_event(
        &tx,
        id,
        &NewEvent {
            event_type: "extension_granted",
            event_date: new_deadline,
            detail_json: Some(&detail),
            consumer_consent: true,
        },
        acting,
        now,
    )?;
    touch_updated_at(&tx, id, now)?;
    tx.commit()?;
    get_reklamacija(conn, id, now)
}

/// Resolves the record: appends `resolved` with `nacin` in `detail_json` and sets
/// `status = 'resolved'`, which clears the derived clock.
pub fn resolve_reklamacija(
    conn: &mut Connection,
    id: i64,
    nacin: &str,
    event_date: &str,
    acting: i64,
    now: &str,
) -> Result<ReklamacijaView, AppError> {
    require_non_empty(nacin, "nacin")?;
    parse_rfc3339(event_date, "eventDate")?;
    let tx = conn.transaction()?;
    load_regime(&tx, id)?;
    let detail = serde_json::json!({ "nacin": nacin }).to_string();
    insert_event(
        &tx,
        id,
        &NewEvent {
            event_type: "resolved",
            event_date,
            detail_json: Some(&detail),
            consumer_consent: false,
        },
        acting,
        now,
    )?;
    set_status(&tx, id, "resolved", now)?;
    tx.commit()?;
    get_reklamacija(conn, id, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};

    fn ev(t: &str, d: &str) -> DeadlineEvent {
        DeadlineEvent {
            event_type: t.into(),
            event_date: d.into(),
        }
    }

    #[test]
    fn regime_is_selected_at_the_cutover_boundary() {
        assert_eq!(regime_for("2026-07-31T00:00:00Z").unwrap(), REGIME_OLD);
        assert_eq!(regime_for("2026-08-01T00:00:00Z").unwrap(), REGIME_NEW);
    }

    // Memo §2.5 OLD/general/restart.
    #[test]
    fn worked_example_old_general_restart() {
        let events = [
            ev("answer_given", "2026-06-05T00:00:00Z"),
            ev("consumer_received_answer", "2026-06-06T00:00:00Z"),
            ev("consumer_responded", "2026-06-08T00:00:00Z"),
        ];
        let s = compute_deadlines(
            REGIME_OLD,
            "2026-06-01T00:00:00Z",
            "opsta",
            &events,
            "2026-06-10T00:00:00Z",
        )
        .unwrap();
        assert_eq!(&s.answer_due[..10], "2026-06-09");
        assert_eq!(
            s.resolution_due.as_deref().map(|d| &d[..10]),
            Some("2026-06-23"),
            "restart = responded + 15"
        );
        assert_eq!(s.clock, "running");
    }

    // Memo §2.5 NEW/tehnička/suspend.
    #[test]
    fn worked_example_new_tehnicka_suspend() {
        let events = [
            ev("answer_given", "2026-08-20T00:00:00Z"),
            ev("consumer_received_answer", "2026-08-21T00:00:00Z"),
            ev("consumer_responded", "2026-08-25T00:00:00Z"),
        ];
        let s = compute_deadlines(
            REGIME_NEW,
            "2026-08-15T00:00:00Z",
            "tehnicka",
            &events,
            "2026-08-30T00:00:00Z",
        )
        .unwrap();
        assert_eq!(&s.answer_due[..10], "2026-08-23");
        assert_eq!(
            s.resolution_due.as_deref().map(|d| &d[..10]),
            Some("2026-09-18"),
            "base 09-14 + 4-day suspension"
        );
    }

    #[test]
    fn clock_pauses_between_received_and_response() {
        let events = [
            ev("answer_given", "2026-08-20T00:00:00Z"),
            ev("consumer_received_answer", "2026-08-21T00:00:00Z"),
        ];
        let s = compute_deadlines(
            REGIME_NEW,
            "2026-08-15T00:00:00Z",
            "opsta",
            &events,
            "2026-08-22T00:00:00Z",
        )
        .unwrap();
        assert_eq!(s.clock, "paused");
        assert!(s.resolution_due.is_none());
        assert_eq!(
            s.consumer_window_due.as_deref().map(|d| &d[..10]),
            Some("2026-08-24")
        );
    }

    #[test]
    fn silence_past_three_days_is_impasse() {
        let events = [
            ev("answer_given", "2026-08-20T00:00:00Z"),
            ev("consumer_received_answer", "2026-08-21T00:00:00Z"),
        ];
        let s = compute_deadlines(
            REGIME_NEW,
            "2026-08-15T00:00:00Z",
            "opsta",
            &events,
            "2026-08-26T00:00:00Z",
        )
        .unwrap();
        assert_eq!(s.clock, "impasse");
        assert!(s.resolution_due.is_none());
    }

    #[test]
    fn answer_clock_never_suspends_and_flags_overdue() {
        // No answer given; today is past filed+8.
        let s = compute_deadlines(
            REGIME_NEW,
            "2026-08-15T00:00:00Z",
            "opsta",
            &[],
            "2026-08-24T00:00:00Z",
        )
        .unwrap();
        assert!(s.answer_overdue);
        // Answered → not overdue.
        let s2 = compute_deadlines(
            REGIME_NEW,
            "2026-08-15T00:00:00Z",
            "opsta",
            &[ev("answer_given", "2026-08-22T00:00:00Z")],
            "2026-08-24T00:00:00Z",
        )
        .unwrap();
        assert!(!s2.answer_overdue);
    }

    #[test]
    fn extension_takes_the_tighter_date() {
        // Running with base 09-14; an extension to 09-10 (earlier) wins (safe = tighter).
        let events = [ev("extension_granted", "2026-09-10T00:00:00Z")];
        let s = compute_deadlines(
            REGIME_NEW,
            "2026-08-15T00:00:00Z",
            "tehnicka",
            &events,
            "2026-08-16T00:00:00Z",
        )
        .unwrap();
        assert_eq!(
            s.resolution_due.as_deref().map(|d| &d[..10]),
            Some("2026-09-10")
        );
        assert!(s.one_extension_used);
    }

    #[test]
    fn resolved_clears_the_clock() {
        let s = compute_deadlines(
            REGIME_OLD,
            "2026-06-01T00:00:00Z",
            "opsta",
            &[ev("resolved", "2026-06-12T00:00:00Z")],
            "2026-07-01T00:00:00Z",
        )
        .unwrap();
        assert_eq!(s.clock, "resolved");
        assert!(!s.resolution_overdue && !s.answer_overdue);
    }

    /// Db::new seeds the admin user (id 1), which the `created_by` FK needs.
    fn with_reklamacija_db(test_name: &str, test: impl FnOnce(&mut Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("db init");
            let mut connection = db.open().expect("open");
            test(&mut connection);
        }
        std::fs::remove_file(&path).expect("cleanup");
    }

    fn intake_input(filed_at: &str) -> ReklamacijaInput {
        ReklamacijaInput {
            podnosilac_ime_prezime: "Petar Petrović".into(),
            kontakt: Some("060/123-456".into()),
            podaci_o_robi: "Frižider Beko".into(),
            opis_nesaobraznosti: "Ne hladi".into(),
            zahtev: "Zamena".into(),
            roba_kind: "tehnicka".into(),
            filed_at: filed_at.into(),
        }
    }

    #[test]
    fn create_assigns_sequential_register_numbers_and_freezes_regime() {
        with_reklamacija_db("rek_create_sequential", |conn| {
            let first = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            assert_eq!(first.register_number, 1);
            assert_eq!(first.regime, REGIME_OLD, "2026-06-01 filing is pre-cutover");
            assert_eq!(first.status, "open");
            assert_eq!(first.podnosilac_ime_prezime, "Petar Petrović");
            assert_eq!(first.podaci_o_robi, "Frižider Beko");
            assert_eq!(first.kontakt.as_deref(), Some("060/123-456"));
            assert_eq!(first.datum_izdavanja_potvrde, "2026-06-01T08:00:00Z");
            assert_eq!(first.created_by, Some(1));

            let second = create_reklamacija(
                conn,
                &intake_input("2026-09-01T00:00:00Z"),
                1,
                "2026-09-01T08:00:00Z",
            )
            .unwrap();
            assert_eq!(second.register_number, 2, "MAX+1 allocates the next number");
            assert_eq!(
                second.regime, REGIME_NEW,
                "2026-09-01 filing is post-cutover"
            );
        });
    }

    #[test]
    fn get_derives_deadline_state_from_the_event_log() {
        with_reklamacija_db("rek_get_deadlines", |conn| {
            let created = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            let view = get_reklamacija(conn, created.id, "2026-06-05T00:00:00Z").unwrap();
            assert!(view.events.is_empty());
            // tehnička → 30-day base; answer_due = filed + 8.
            assert_eq!(&view.deadlines.answer_due[..10], "2026-06-09");
            assert_eq!(
                view.deadlines.resolution_due.as_deref().map(|d| &d[..10]),
                Some("2026-07-01"),
                "filed + 30 with no round-trip yet"
            );
            assert_eq!(view.deadlines.clock, "running");
        });
    }

    #[test]
    fn list_orders_newest_filing_first_and_carries_derived_dates() {
        with_reklamacija_db("rek_list_order", |conn| {
            create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            create_reklamacija(
                conn,
                &intake_input("2026-06-10T00:00:00Z"),
                1,
                "2026-06-10T08:00:00Z",
            )
            .unwrap();
            let list = list_reklamacije(conn, "2026-06-12T00:00:00Z").unwrap();
            assert_eq!(list.len(), 2);
            assert_eq!(&list[0].filed_at[..10], "2026-06-10", "filed_at DESC");
            assert_eq!(&list[1].filed_at[..10], "2026-06-01");
            assert_eq!(&list[1].answer_due[..10], "2026-06-09");
        });
    }

    #[test]
    fn create_rejects_empty_required_field() {
        with_reklamacija_db("rek_reject_empty", |conn| {
            let mut input = intake_input("2026-06-01T00:00:00Z");
            input.podnosilac_ime_prezime = "   ".into();
            let error = create_reklamacija(conn, &input, 1, "2026-06-01T08:00:00Z").unwrap_err();
            assert_eq!(error.code(), "validation_error");
        });
    }

    #[test]
    fn purge_eligibility_flips_at_the_two_year_floor() {
        with_reklamacija_db("rek_purge_floor", |conn| {
            let created = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            // Day before filed + 730 (2028-05-31) → not yet eligible.
            let before = get_reklamacija(conn, created.id, "2028-05-30T00:00:00Z").unwrap();
            assert!(!before.purge_eligible);
            // On filed + 730 → eligible (filed_at + 2y ≤ today).
            let on = get_reklamacija(conn, created.id, "2028-05-31T00:00:00Z").unwrap();
            assert!(on.purge_eligible);
            let summary = list_reklamacije(conn, "2028-05-31T00:00:00Z").unwrap();
            assert!(summary[0].purge_eligible);
        });
    }

    fn answer(event_date: &str) -> AnswerInput {
        AnswerInput {
            answer_text: "Predlog zamene robe.".into(),
            warning_duty: None,
            warning_consequences: None,
            warning_zastoj: None,
            event_date: event_date.into(),
        }
    }

    #[test]
    fn new_regime_answer_requires_the_express_warning() {
        with_reklamacija_db("rek_new_answer_gate", |conn| {
            let created = create_reklamacija(
                conn,
                &intake_input("2026-09-01T00:00:00Z"),
                1,
                "2026-09-01T08:00:00Z",
            )
            .unwrap();
            assert_eq!(created.regime, REGIME_NEW);

            // Missing warning fields → rejected.
            let bare = answer("2026-09-05T00:00:00Z");
            let error = log_answer(conn, created.id, &bare, 1, "2026-09-05T09:00:00Z").unwrap_err();
            assert_eq!(error.code(), "validation_error");

            // All three present → accepted; status → answered; answer_given logged.
            let full = AnswerInput {
                warning_duty: Some("Dužni ste da se izjasnite o predlogu.".into()),
                warning_consequences: Some("U suprotnom se smatra da ste odustali.".into()),
                warning_zastoj: Some("Rok za rešavanje ne teče do vašeg izjašnjenja.".into()),
                ..answer("2026-09-05T00:00:00Z")
            };
            let view = log_answer(conn, created.id, &full, 1, "2026-09-05T09:00:00Z").unwrap();
            assert_eq!(view.status, "answered");
            assert!(view.events.iter().any(|e| e.event_type == "answer_given"));
        });
    }

    #[test]
    fn old_regime_answer_accepts_without_warning() {
        with_reklamacija_db("rek_old_answer_no_warning", |conn| {
            let created = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            assert_eq!(created.regime, REGIME_OLD);
            let view = log_answer(
                conn,
                created.id,
                &answer("2026-06-05T00:00:00Z"),
                1,
                "2026-06-05T09:00:00Z",
            )
            .unwrap();
            assert_eq!(view.status, "answered");
        });
    }

    #[test]
    fn extension_is_one_only_and_requires_consent() {
        with_reklamacija_db("rek_extension_rules", |conn| {
            let created = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            // Without consent → validation_error.
            let no_consent = grant_extension(
                conn,
                created.id,
                "2026-06-20T00:00:00Z",
                false,
                "razlog",
                1,
                "2026-06-02T09:00:00Z",
            )
            .unwrap_err();
            assert_eq!(no_consent.code(), "validation_error");

            // First consented extension → ok.
            let granted = grant_extension(
                conn,
                created.id,
                "2026-06-20T00:00:00Z",
                true,
                "Dogovoreno sa potrošačem.",
                1,
                "2026-06-02T09:00:00Z",
            )
            .unwrap();
            assert!(granted.deadlines.one_extension_used);

            // Second extension → invalid_state.
            let second = grant_extension(
                conn,
                created.id,
                "2026-06-22T00:00:00Z",
                true,
                "opet",
                1,
                "2026-06-03T09:00:00Z",
            )
            .unwrap_err();
            assert_eq!(second.code(), "invalid_state");
        });
    }

    #[test]
    fn consumer_events_require_their_prior_event() {
        with_reklamacija_db("rek_consumer_order", |conn| {
            let created = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            // received before any answer → invalid_state.
            let e1 = log_consumer_received_answer(
                conn,
                created.id,
                "2026-06-06T00:00:00Z",
                1,
                "2026-06-06T09:00:00Z",
            )
            .unwrap_err();
            assert_eq!(e1.code(), "invalid_state");
            // response before received → invalid_state.
            let e2 = log_consumer_response(
                conn,
                created.id,
                "2026-06-08T00:00:00Z",
                1,
                "2026-06-08T09:00:00Z",
            )
            .unwrap_err();
            assert_eq!(e2.code(), "invalid_state");
        });
    }

    #[test]
    fn old_regime_full_cycle_restarts_to_the_memo_date() {
        with_reklamacija_db("rek_old_full_cycle", |conn| {
            let mut input = intake_input("2026-06-01T00:00:00Z");
            input.roba_kind = "opsta".into(); // 15-day span → memo §2.5 OLD/general/restart
            let created = create_reklamacija(conn, &input, 1, "2026-06-01T08:00:00Z").unwrap();

            let after_answer = log_answer(
                conn,
                created.id,
                &answer("2026-06-05T00:00:00Z"),
                1,
                "2026-06-05T09:00:00Z",
            )
            .unwrap();
            assert_eq!(after_answer.status, "answered");

            let after_received = log_consumer_received_answer(
                conn,
                created.id,
                "2026-06-06T00:00:00Z",
                1,
                "2026-06-06T09:00:00Z",
            )
            .unwrap();
            assert_eq!(after_received.status, "awaiting_consumer");

            let after_responded = log_consumer_response(
                conn,
                created.id,
                "2026-06-08T00:00:00Z",
                1,
                "2026-06-08T09:00:00Z",
            )
            .unwrap();
            assert_eq!(after_responded.status, "answered");

            let view = get_reklamacija(conn, created.id, "2026-06-10T00:00:00Z").unwrap();
            assert_eq!(
                view.deadlines.resolution_due.as_deref().map(|d| &d[..10]),
                Some("2026-06-23"),
                "OLD restart = responded + 15"
            );
            assert_eq!(view.deadlines.clock, "running");
        });
    }

    #[test]
    fn resolve_marks_the_record_resolved() {
        with_reklamacija_db("rek_resolve", |conn| {
            let created = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            let view = resolve_reklamacija(
                conn,
                created.id,
                "Zamena robe",
                "2026-06-05T00:00:00Z",
                1,
                "2026-06-05T09:00:00Z",
            )
            .unwrap();
            assert_eq!(view.status, "resolved");
            assert_eq!(view.deadlines.clock, "resolved");
            assert!(view.events.iter().any(|e| e.event_type == "resolved"));
        });
    }
}
