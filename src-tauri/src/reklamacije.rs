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

#[cfg(test)]
mod tests {
    use super::*;

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
}
