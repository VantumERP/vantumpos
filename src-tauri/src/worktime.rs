//! Working-time rules — ZoR čl. 53 caps and the čl. 87–91 protection guards.
//!
//! Everything here is pure and takes the day as a parameter: the caps are what
//! carry the larger fine (čl. 274 st. 1 tač. 3, preduzetnik 200.000–400.000,
//! against 50.000–150.000 for the missing register), so they must be testable
//! exactly at the boundary.
//!
//! The guards and the command layer that consume this land in Tasks 4 and 5, so
//! `dead_code` is allowed here until that wiring arrives — mirroring the other
//! domain modules.

#![allow(dead_code)]

use time::Date;

use crate::cash_deposit::parse_iso_date;

/// ZoR čl. 53 st. 2 — „Prekovremeni rad ne može da traje duže od osam časova
/// nedeljno.“ Eight hours, in minutes.
pub const WEEKLY_OVERTIME_CAP_MINUTES: i64 = 8 * 60;

/// ZoR čl. 53 st. 3 — „Zaposleni ne može da radi duže od 12 časova dnevno
/// uključujući i prekovremeni rad.“ Twelve hours, in minutes.
pub const DAILY_TOTAL_CAP_MINUTES: i64 = 12 * 60;

/// One calendar day of one employee's hours, reduced to the two figures the
/// čl. 53 caps are drawn on. Minutes, never floating point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayHours {
    pub dan: String,
    pub efektivno_minuta: i64,
    pub prekovremeni_minuta: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapAssessment {
    pub weekly_overtime_minutes: i64,
    pub daily_total_minutes: i64,
    pub weekly_cap_exceeded: bool,
    pub daily_cap_exceeded: bool,
    pub requires_override: bool,
}

/// Assesses one day's entry against both čl. 53 caps.
///
/// `week` is the employee's other days around `day`; rows outside `day`'s
/// calendar week, and the stored row for `day` itself, are dropped so that
/// `entry` — the version being assessed — is the only contribution for its own
/// date. Both caps are strict `>`: eight hours of overtime and twelve hours of
/// total work are the limits, not breaches of them.
///
/// An exceeded cap never blocks the write. Requirement §4 req. 7 makes this the
/// bigger fine, and a record that refuses to describe a day that actually
/// happened hides the čl. 274 st. 1 tač. 3 exposure instead of surfacing it —
/// so the assessment asks for an override reason and lets the day be recorded.
pub fn assess_caps(day: &str, entry: &DayHours, week: &[DayHours]) -> CapAssessment {
    let same_week: i64 = week
        .iter()
        .filter(|d| d.dan != day && in_same_iso_week(&d.dan, day))
        .map(|d| d.prekovremeni_minuta)
        .sum();

    let weekly_overtime_minutes = same_week + entry.prekovremeni_minuta;
    let daily_total_minutes = entry.efektivno_minuta + entry.prekovremeni_minuta;
    let weekly_cap_exceeded = weekly_overtime_minutes > WEEKLY_OVERTIME_CAP_MINUTES;
    let daily_cap_exceeded = daily_total_minutes > DAILY_TOTAL_CAP_MINUTES;

    CapAssessment {
        weekly_overtime_minutes,
        daily_total_minutes,
        weekly_cap_exceeded,
        daily_cap_exceeded,
        requires_override: weekly_cap_exceeded || daily_cap_exceeded,
    }
}

/// True when both ISO days fall in the same Monday-anchored calendar week.
///
/// „Nedeljno“ in čl. 53 st. 2 is the calendar week, so the counter resets on
/// Monday rather than sliding over the last seven days.
///
/// A day that does not parse counts as *inside* the week. Both directions are
/// wrong, but only one is dangerous: dropping an unreadable day under-counts
/// the weekly overtime and can report a čl. 53 st. 2 breach as lawful, while
/// keeping it merely over-counts and asks for an override that turns out to be
/// unnecessary. Over-reporting is the safe direction.
pub fn in_same_iso_week(a: &str, b: &str) -> bool {
    match (monday_of_week(a), monday_of_week(b)) {
        (Some(left), Some(right)) => left == right,
        _ => true,
    }
}

/// The Monday on or before `day`, or `None` if `day` is not a civil date.
fn monday_of_week(day: &str) -> Option<Date> {
    let date = parse_iso_date(day)?;
    let offset = i32::from(date.weekday().number_days_from_monday());
    Date::from_julian_day(date.to_julian_day() - offset).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(effective: i64, overtime: i64) -> DayHours {
        DayHours {
            dan: "2026-08-03".to_string(),
            efektivno_minuta: effective,
            prekovremeni_minuta: overtime,
        }
    }

    #[test]
    fn weekly_overtime_cap_is_eight_hours() {
        let week = vec![day(480, 120), day(480, 120), day(480, 120)]; // 6 h so far
        let a = assess_caps("2026-08-06", &day(480, 120), &week); // +2 h = 8 h exactly
        assert_eq!(a.weekly_overtime_minutes, 480);
        assert!(
            !a.weekly_cap_exceeded,
            "exactly 8 h is the cap, not over it"
        );

        let a = assess_caps("2026-08-06", &day(480, 121), &week);
        assert!(
            a.weekly_cap_exceeded,
            "8 h + 1 minute is over ZoR čl. 53 st. 2"
        );
    }

    #[test]
    fn daily_total_cap_is_twelve_hours_including_overtime() {
        let a = assess_caps("2026-08-03", &day(480, 240), &[]); // 8 + 4 = 12 h
        assert_eq!(a.daily_total_minutes, 720);
        assert!(!a.daily_cap_exceeded, "exactly 12 h is the cap");

        let a = assess_caps("2026-08-03", &day(480, 241), &[]);
        assert!(
            a.daily_cap_exceeded,
            "12 h + 1 minute is over ZoR čl. 53 st. 3"
        );
    }

    /// The record must be able to describe an unlawful day — refusing to record
    /// reality would hide the čl. 274 exposure instead of surfacing it.
    #[test]
    fn exceeding_a_cap_requires_an_override_but_is_never_impossible() {
        let a = assess_caps("2026-08-03", &day(480, 300), &[]);
        assert!(a.daily_cap_exceeded);
        assert!(
            a.requires_override,
            "the operator must say why, not be blocked"
        );
    }

    #[test]
    fn the_week_is_a_calendar_week_starting_monday() {
        // 2026-08-03 is a Monday; 2026-08-02 is the Sunday before and must not count.
        let prior_sunday = DayHours {
            dan: "2026-08-02".to_string(),
            efektivno_minuta: 480,
            prekovremeni_minuta: 480,
        };
        let a = assess_caps("2026-08-03", &day(480, 60), &[prior_sunday]);
        assert_eq!(
            a.weekly_overtime_minutes, 60,
            "last week's overtime does not carry in"
        );
    }

    /// The `dan` GLOB in v17 fixes the shape of a stored day, not its validity —
    /// `2026-13-45` passes it. An unreadable day must therefore still be counted,
    /// because the alternative silently reports a čl. 53 st. 2 breach as lawful.
    #[test]
    fn an_unreadable_day_is_counted_rather_than_silently_dropped() {
        let broken = DayHours {
            dan: "2026-13-45".to_string(),
            efektivno_minuta: 480,
            prekovremeni_minuta: 480,
        };
        let a = assess_caps("2026-08-03", &day(480, 60), &[broken]);
        assert_eq!(a.weekly_overtime_minutes, 540);
        assert!(
            a.weekly_cap_exceeded,
            "over-counting warns; dropping the row would hide the breach"
        );
    }
}
