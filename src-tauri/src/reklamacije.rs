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
}
