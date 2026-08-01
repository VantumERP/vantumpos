//! Working-time rules — ZoR čl. 53 caps and the čl. 87–91 protection guards.
//!
//! Everything here is pure and takes the day as a parameter: breaching a cap is
//! the čl. 274 st. 1 tač. 3 prekršaj, a heavier exposure than the čl. 276 one for
//! the missing register itself, so the caps must be testable exactly at the
//! boundary. No figure appears here — every statutory amount in this application
//! lives in `legal.rs` and is named by article everywhere else.
//!
//! The guards and the command layer that consume this land in Tasks 4 and 5, so
//! `dead_code` is allowed here until that wiring arrives — mirroring the other
//! domain modules.

#![allow(dead_code)]

use std::cmp::Ordering;

use time::{Date, Month};

use crate::cash_deposit::parse_iso_date;

/// ZoR čl. 53 st. 2 — „Prekovremeni rad ne može da traje duže od osam časova
/// nedeljno.“ Eight hours, in minutes.
pub const WEEKLY_OVERTIME_CAP_MINUTES: i64 = 8 * 60;

/// ZoR čl. 53 st. 3 — „Zaposleni ne može da radi duže od 12 časova dnevno
/// uključujući i prekovremeni rad.“ Twelve hours, in minutes.
pub const DAILY_TOTAL_CAP_MINUTES: i64 = 12 * 60;

/// ZoR čl. 57 st. 5 — „U slučaju preraspodele radnog vremena, radno vreme ne može
/// da traje duže od 60 časova nedeljno.“ Sixty hours, in minutes.
///
/// This is the ceiling that has to replace čl. 53 st. 3 for an employee in
/// preraspodela, and the caller applies it: čl. 58 keeps preraspodela out of
/// prekovremeni rad, so such an employee records no overtime and the čl. 53 st. 2
/// weekly leg never fires for them either. Drop the daily leg without putting this
/// one in its place and the register accepts a 91-hour week in silence.
pub const PRERASPODELA_WEEKLY_CAP_MINUTES: i64 = 60 * 60;

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
    /// Every worked minute of `day`'s calendar week — efektivno **and**
    /// prekovremeni — which is the figure čl. 57 st. 5 is drawn on. Reported
    /// always, because it is arithmetic over the same week the other two legs
    /// already walk; whether the 60 h ceiling binds is the caller's branch.
    pub weekly_total_minutes: i64,
    pub weekly_cap_exceeded: bool,
    /// The čl. 53 st. 3 twelve-hour figure. A fact about the day, never erased:
    /// see precondition 2 on [`assess_caps`].
    pub daily_cap_exceeded: bool,
    /// čl. 57 st. 5. [`assess_caps`] always leaves this `false` — it cannot see
    /// whether the employee works in preraspodela — and the caller sets it.
    pub preraspodela_weekly_cap_exceeded: bool,
    pub requires_override: bool,
}

/// Assesses one day's entry against both čl. 53 caps.
///
/// `day` is the sole authority: it anchors the calendar week and is the key the
/// rows in `week` are de-duplicated against. `entry.dan` is never read — the
/// field is carried for the caller's convenience only, so an `entry` whose `dan`
/// disagrees with `day` is silently assessed as `day`, and the stored row for the
/// entry's real date would then be counted alongside it. Pass both from the same
/// source.
///
/// `week` is the employee's other days around `day`; rows outside `day`'s
/// calendar week, and the stored row for `day` itself, are dropped so that
/// `entry` — the version being assessed — is the only contribution for its own
/// date. Both caps are strict `>`: eight hours of overtime and twelve hours of
/// total work are the limits, not breaches of them.
///
/// Two preconditions belong to the caller; neither is checkable from here:
///
/// 1. **`week` must carry live rows only — `MAX(verzija)` per `dan`.** v17 is
///    append-only and keeps every correction of a day as its own row, so a plain
///    `SELECT … WHERE dan BETWEEN …` also returns superseded versions, whose
///    `prekovremeni_minuta` then sum straight into `weekly_overtime_minutes` and
///    manufacture a čl. 53 st. 2 breach out of lawful hours. The `dan != day`
///    filter protects the assessed date only — never the other six.
/// 2. **`daily_cap_exceeded` is the čl. 53 st. 3 rule set alone, and the caller
///    must branch the *override gate* off `users.radi_u_preraspodeli`.** čl. 58
///    says preraspodela is not prekovremeni rad, and čl. 57 caps it at 60 časova
///    nedeljno (st. 5) with no daily leg at all — the 12 h/48 h pair belongs to
///    the separate čl. 56 st. 3 monthly-average scheme (čl. 56 st. 4). Gating on
///    this daily cap unconditionally reports a lawful čl. 57 day as a breach.
///    **But the branch must swap one ceiling for another, never delete one:**
///    čl. 58 means such an employee records `prekovremeni_minuta = 0`, so the
///    weekly overtime leg cannot fire for them either, and an employee with the
///    daily leg removed and nothing in its place has no total-hours ceiling at
///    all. [`PRERASPODELA_WEEKLY_CAP_MINUTES`] over [`CapAssessment::weekly_total_minutes`]
///    is the čl. 57 st. 5 leg the caller owes. §4 req. 9 makes these separate rule
///    sets, not variants of one cap.
///
/// An exceeded cap never blocks the write. §4 req. 7 makes this the bigger fine,
/// and a record that refuses to describe a day that actually happened hides the
/// čl. 274 st. 1 tač. 3 exposure instead of surfacing it — so the assessment asks
/// for an override reason and lets the day be recorded.
pub fn assess_caps(day: &str, entry: &DayHours, week: &[DayHours]) -> CapAssessment {
    let same_week = || {
        week.iter()
            .filter(|d| d.dan != day && in_same_iso_week(&d.dan, day))
    };
    let overtime_before: i64 = same_week().map(|d| d.prekovremeni_minuta).sum();
    let total_before: i64 = same_week()
        .map(|d| d.efektivno_minuta + d.prekovremeni_minuta)
        .sum();

    let daily_total_minutes = entry.efektivno_minuta + entry.prekovremeni_minuta;
    let weekly_overtime_minutes = overtime_before + entry.prekovremeni_minuta;
    let weekly_total_minutes = total_before + daily_total_minutes;
    let weekly_cap_exceeded = weekly_overtime_minutes > WEEKLY_OVERTIME_CAP_MINUTES;
    let daily_cap_exceeded = daily_total_minutes > DAILY_TOTAL_CAP_MINUTES;

    CapAssessment {
        weekly_overtime_minutes,
        daily_total_minutes,
        weekly_total_minutes,
        weekly_cap_exceeded,
        daily_cap_exceeded,
        // čl. 57 st. 5 binds only in preraspodela, which this function cannot
        // see. Precondition 2 puts the leg on the caller.
        preraspodela_weekly_cap_exceeded: false,
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

/// ZoR čl. 87 — a zaposleni mlađi od 18 godina may not work longer than eight
/// hours a day. Minutes, never floating point.
pub const MINOR_DAILY_CAP_MINUTES: i64 = 8 * 60;

/// ZoR čl. 88 st. 1 — the prohibition runs to „mlađi od 18 godina života“.
const PUNOLETSTVO_GODINA: i32 = 18;

/// ZoR čl. 91 st. 1 — „jedan od roditelja sa detetom do tri godine života“.
const SAGLASNOST_DETE_GODINA: i32 = 3;

/// ZoR čl. 91 st. 2 — „samohrani roditelj koji ima dete do sedam godina života“.
/// SEVEN. The widely-repeated fourteen belongs to no provision of ZoR and would
/// silently widen the gate instead of guarding it.
const SAGLASNOST_SAMOHRANI_DETE_GODINA: i32 = 7;

/// The čl. 87–91 age and status facts about one employee, mirrored from the v17
/// `users` columns.
///
/// Every field is what the employer has *asserted*, never evidence: `trudnoca_ili_dojenje`
/// is a flag set from a nalaz nadležnog zdravstvenog organa and the nalaz itself is
/// never stored, and `saglasnost_prekovremeni_od` records that a written consent
/// exists and from when — it does not collect one. Neither is a ZZPL pristanak.
///
/// `saglasnost_prekovremeni_od` is the čl. 91 consent and nothing else. The čl. 57
/// st. 4 preraspodela conversion has its own column, `saglasnost_preraspodela_od`,
/// which is deliberately absent from this struct so it cannot be read here by
/// accident — the two consents are legally distinct and not interchangeable.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EmployeeProtection {
    pub datum_rodjenja: Option<String>,
    pub datum_rodjenja_najmladjeg_deteta: Option<String>,
    pub samohrani_roditelj: Option<bool>,
    pub dete_tezak_invalid: Option<bool>,
    pub trudnoca_ili_dojenje: Option<bool>,
    pub saglasnost_prekovremeni_od: Option<String>,
    pub radi_u_preraspodeli: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ProtectionKind {
    /// čl. 88 st. 1 — overtime of an employee under 18 is prohibited outright.
    MaloletanPrekovremeni,
    /// čl. 88 st. 1 — preraspodela radnog vremena of an employee under 18 is
    /// prohibited outright, as its own leg of the same sentence.
    MaloletanPreraspodela,
    /// čl. 87 — an employee under 18 is capped at eight hours a day.
    MaloletanDnevniLimit,
    /// čl. 91 — a protected parent works overtime only on their written consent.
    SaglasnostRoditelja,
    /// čl. 90 — pregnancy or nursing, conditional on a health authority's finding.
    TrudnocaNocniIPrekovremeni,
    /// Not a statutory duty — a profile date the operator entered is not a civil
    /// date, so the guard it feeds could not be evaluated for this day.
    NeispravanDatumUProfilu,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProtectionBlock {
    pub kind: ProtectionKind,
    /// `true` where the statute states the prohibition itself, `false` where it
    /// makes the prohibition conditional on a finding this application does not
    /// hold and must not simulate.
    pub blocking: bool,
    pub poruka: String,
}

/// Applies the ZoR čl. 87–91 protection guards to one day of one employee.
///
/// `day` is the sole authority for every age computation — never a wall clock.
/// A guard that reads `now()` answers a different question on every run and
/// silently changes what a stored day means, which is exactly what an append-only
/// register must not do. As in `assess_caps`, `entry.dan` is not read; pass `day`
/// and `entry` from the same source.
///
/// The čl. 91 and čl. 90 guards are drawn on the day carrying overtime. Both
/// provisions read „prekovremeno, odnosno noću“, and the night leg is not visible
/// in `DayHours` — night hours are a čl. 62 computation over clock times, tagged
/// `izračunato radi provere usklađenosti`, and belong wherever those are modelled.
/// A plain day with no overtime raises nothing here.
///
/// The čl. 88 st. 1 preraspodela leg is the exception to that: it bans the
/// *arrangement*, not a day's minutes, so it fires on a plain day with no
/// overtime. čl. 58 keeps preraspodela out of the overtime derivation, which
/// means nothing else in this module would ever notice the prohibition.
///
/// Two limits of this function, both deliberate:
///
/// 1. **The čl. 87 weekly leg (35 časova nedeljno) is not checked.** It needs the
///    employee's week, which this signature does not carry; `assess_caps` is where
///    a week is already in hand.
/// 2. **An absent `datum_rodjenja` raises nothing.** v17 adds the column nullable
///    and does not backfill it, so treating "unknown" as "minor" would block every
///    employee nobody has filled in yet — and a register that refuses to describe
///    days that actually happened hides exposure instead of surfacing it. Filling
///    the profile is what turns the guard on. A date that is *present but not a
///    civil date* is the opposite case and is reported: the operator believes that
///    profile is filled in, and v17's CHECKs are GLOB shape tests that let
///    `2009-13-45` through.
pub fn check_protection(
    p: &EmployeeProtection,
    day: &str,
    entry: &DayHours,
) -> Vec<ProtectionBlock> {
    let mut blocks = Vec::new();
    let ima_prekovremeni = entry.prekovremeni_minuta > 0;

    if is_younger_than(p.datum_rodjenja.as_deref(), day, PUNOLETSTVO_GODINA) {
        if ima_prekovremeni {
            blocks.push(ProtectionBlock {
                kind: ProtectionKind::MaloletanPrekovremeni,
                blocking: true,
                poruka: "Zabranjen je prekovremeni rad zaposlenog mlađeg od 18 godina života \
                         (ZoR čl. 88 st. 1). Prekovremeni časovi za ovaj dan ne mogu se unositi."
                    .to_string(),
            });
        }
        if p.radi_u_preraspodeli {
            blocks.push(ProtectionBlock {
                kind: ProtectionKind::MaloletanPreraspodela,
                blocking: true,
                poruka: "Zabranjena je preraspodela radnog vremena zaposlenog mlađeg od 18 \
                         godina života (ZoR čl. 88 st. 1). Isključite preraspodelu radnog \
                         vremena u profilu ovog zaposlenog."
                    .to_string(),
            });
        }
        if entry.efektivno_minuta + entry.prekovremeni_minuta > MINOR_DAILY_CAP_MINUTES {
            blocks.push(ProtectionBlock {
                kind: ProtectionKind::MaloletanDnevniLimit,
                blocking: true,
                poruka: "Zaposleni mlađi od 18 godina života ne može da radi duže od osam \
                         časova dnevno (ZoR čl. 87)."
                    .to_string(),
            });
        }
    }

    let treba_saglasnost = ima_prekovremeni && parental_consent_required(p, day);
    if treba_saglasnost && !consent_covers(p.saglasnost_prekovremeni_od.as_deref(), day) {
        blocks.push(ProtectionBlock {
            kind: ProtectionKind::SaglasnostRoditelja,
            blocking: true,
            poruka: "Ovaj zaposleni može da radi prekovremeno samo uz svoju pisanu saglasnost \
                     (ZoR čl. 91). Za ovaj dan nije evidentirana važeća saglasnost — upišite \
                     datum pisane saglasnosti koji nije posle ovog dana."
                .to_string(),
        });
    }

    if ima_prekovremeni && p.trudnoca_ili_dojenje == Some(true) {
        blocks.push(ProtectionBlock {
            kind: ProtectionKind::TrudnocaNocniIPrekovremeni,
            blocking: false,
            poruka: "Zaposlena za vreme trudnoće i zaposlena koja doji dete ne može da radi \
                     prekovremeno i noću ako bi takav rad bio štetan za njeno zdravlje i \
                     zdravlje deteta, na osnovu nalaza nadležnog zdravstvenog organa \
                     (ZoR čl. 90). Ocenu daje nadležni zdravstveni organ, ne aplikacija."
                .to_string(),
        });
    }

    if nije_datum(p.datum_rodjenja.as_deref()) {
        blocks.push(ProtectionBlock {
            kind: ProtectionKind::NeispravanDatumUProfilu,
            blocking: false,
            poruka: "Datum rođenja zaposlenog u profilu nije ispravan datum, pa zaštite za \
                     zaposlene mlađe od 18 godina (ZoR čl. 87 i čl. 88) za ovaj dan nisu \
                     proverene. Ispravite datum rođenja u profilu zaposlenog."
                .to_string(),
        });
    }

    // Only when the unreadable value actually cost a guard: the težak-invalid leg
    // of čl. 91 st. 2 reads no date at all, so the gate can fire without it.
    if ima_prekovremeni
        && !treba_saglasnost
        && nije_datum(p.datum_rodjenja_najmladjeg_deteta.as_deref())
    {
        blocks.push(ProtectionBlock {
            kind: ProtectionKind::NeispravanDatumUProfilu,
            blocking: false,
            poruka: "Datum rođenja najmlađeg deteta u profilu nije ispravan datum, pa provera \
                     pisane saglasnosti za prekovremeni rad (ZoR čl. 91) za ovaj dan nije \
                     izvedena. Ispravite datum u profilu zaposlenog."
                .to_string(),
        });
    }

    blocks
}

/// A value the operator entered that is not a civil date.
///
/// `None` is not this: an empty column is a profile nobody has filled in, which
/// v17 leaves as the default. `Some` that does not parse is a value somebody
/// believes they entered, and every age computation here silently ignores it —
/// v17's CHECKs are GLOB shape tests, so `2009-13-45` reaches this module.
fn nije_datum(value: Option<&str>) -> bool {
    matches!(value, Some(s) if parse_iso_date(s).is_none())
}

/// Whether a stored čl. 91 written consent covers work done on `day`.
///
/// The column is an „od“ date, so it discharges the guard only from that date
/// on. Anything else — absent, unreadable, or later than `day` — keeps the
/// guard: čl. 91 is a consent given before the overtime, and a register whose
/// contemporaneity is the point (čl. 55 st. 6 „dnevnu“) must not let a document
/// dated afterwards authorise hours already worked. An unreadable `day` keeps it
/// too, the same over-report-is-safe direction `in_same_iso_week` takes.
fn consent_covers(saglasnost_od: Option<&str>, day: &str) -> bool {
    match (saglasnost_od.and_then(parse_iso_date), parse_iso_date(day)) {
        (Some(od), Some(day)) => od <= day,
        _ => false,
    }
}

/// čl. 58 — „Preraspodela radnog vremena ne smatra se prekovremenim radom.“
///
/// The naive `hours > 8 ⇒ prekovremeni` derivation is legally wrong for an
/// employee in preraspodela and would inflate the čl. 55 st. 6 register, which is
/// the penalised one. This is the hard branch §4 req. 9 demands, not a display
/// toggle: it says whether overtime may be *derived* at all, and never whether an
/// operator may record overtime they know happened.
pub fn derives_overtime_automatically(p: &EmployeeProtection) -> bool {
    !p.radi_u_preraspodeli
}

/// Whether čl. 91 puts this day's overtime behind a written consent.
///
/// Both legs of st. 2, not just the age one: the statute reads „Samohrani
/// roditelj koji ima dete do sedam godina života **ili dete koje je težak
/// invalid**“, and the disability leg carries no age bound at all. An age-only
/// guard drops it the day the child turns seven.
fn parental_consent_required(p: &EmployeeProtection, day: &str) -> bool {
    let samohrani = p.samohrani_roditelj == Some(true);
    if samohrani && p.dete_tezak_invalid == Some(true) {
        return true;
    }
    let prag = if samohrani {
        SAGLASNOST_SAMOHRANI_DETE_GODINA
    } else {
        SAGLASNOST_DETE_GODINA
    };
    child_within_years(p.datum_rodjenja_najmladjeg_deteta.as_deref(), day, prag)
}

/// True when `birth` is strictly less than `years` old on `day`.
///
/// čl. 88 st. 1 says „mlađi od 18 godina života“, so the eighteenth birthday
/// itself is already outside the prohibition.
fn is_younger_than(birth: Option<&str>, day: &str, years: i32) -> bool {
    compare_day_to_birthday(birth, day, years) == Some(Ordering::Less)
}

/// True when a child born on `birth` is still „do `years` godina života“ on `day`.
///
/// The anniversary itself counts as inside. „Do“ is not „mlađi od“, and of the two
/// readings only this one errs toward keeping the guard: one extra day of asking
/// for a consent that was already on file costs a prompt, while one day short
/// drops the čl. 91 gate on the very day the threshold is reached.
fn child_within_years(birth: Option<&str>, day: &str, years: i32) -> bool {
    matches!(
        compare_day_to_birthday(birth, day, years),
        Some(Ordering::Less | Ordering::Equal)
    )
}

/// Orders `day` against the `years`-th anniversary of `birth`.
///
/// `None` when either date is absent or unreadable — no age is guessed. The two
/// cases are not equivalent to the caller, though, and `check_protection`
/// separates them: an absent date is an unfilled profile and stays silent, an
/// unreadable one is reported through `nije_datum`.
fn compare_day_to_birthday(birth: Option<&str>, day: &str, years: i32) -> Option<Ordering> {
    let birth = parse_iso_date(birth?)?;
    let day = parse_iso_date(day)?;
    Some(day.cmp(&anniversary(birth, years)?))
}

/// The `years`-th anniversary of `birth`.
///
/// A 29 February birth has no anniversary in a common year; it falls to 1 March,
/// which keeps the protection through the whole of 28 February. The other choice
/// would end a minor's or a child's protection a day early, and that is the one
/// direction that costs something.
fn anniversary(birth: Date, years: i32) -> Option<Date> {
    let godina = birth.year().checked_add(years)?;
    Date::from_calendar_date(godina, birth.month(), birth.day())
        .or_else(|_| Date::from_calendar_date(godina, Month::March, 1))
        .ok()
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

    /// An employee with nothing asserted about them except their own date of
    /// birth — every čl. 90/91 flag left unset, exactly as v17 leaves the
    /// columns for an employee nobody has filled in yet.
    fn protection_born(datum_rodjenja: &str) -> EmployeeProtection {
        EmployeeProtection {
            datum_rodjenja: Some(datum_rodjenja.to_string()),
            datum_rodjenja_najmladjeg_deteta: None,
            samohrani_roditelj: None,
            dete_tezak_invalid: None,
            trudnoca_ili_dojenje: None,
            saglasnost_prekovremeni_od: None,
            radi_u_preraspodeli: false,
        }
    }

    /// An adult parent whose youngest child was born on `dete`.
    fn protection_child_born(dete: &str) -> EmployeeProtection {
        let mut p = protection_born("1990-04-11");
        p.datum_rodjenja_najmladjeg_deteta = Some(dete.to_string());
        p
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

    /// The weekly leg carries the same čl. 274 st. 1 tač. 3 exposure as the daily
    /// one, so a week-only breach must ask for a reason on its own. Without this
    /// the `weekly_cap_exceeded ||` half of `requires_override` is unguarded and a
    /// čl. 53 st. 2 breach can silently stop demanding an override.
    #[test]
    fn a_weekly_only_breach_still_requires_an_override() {
        let week = vec![day(480, 120), day(480, 120), day(480, 120)]; // 6 h so far
        let a = assess_caps("2026-08-06", &day(300, 121), &week); // +2 h 1 min = 8 h 1 min
        assert_eq!(a.weekly_overtime_minutes, 481);
        assert!(a.weekly_cap_exceeded, "8 h + 1 minute is over čl. 53 st. 2");
        assert_eq!(
            a.daily_total_minutes, 421,
            "the day itself is nowhere near the 12 h cap"
        );
        assert!(
            !a.daily_cap_exceeded,
            "this must be a weekly-only breach or it does not test the weekly leg"
        );
        assert!(
            a.requires_override,
            "a čl. 53 st. 2 weekly breach demands a reason even on a short day"
        );
    }

    /// v17 keeps every `verzija` of a day as its own row and never UPDATEs, so a
    /// week window can carry the superseded version of the day being corrected.
    /// `entry` is the version under assessment; counting the stored one as well
    /// would double the day's overtime and manufacture a false čl. 53 st. 2 breach.
    #[test]
    fn the_stored_version_of_the_assessed_day_is_not_counted_twice() {
        let stored = day(480, 480); // same `dan` as the assessed day
        let a = assess_caps("2026-08-03", &day(480, 60), &[stored]);
        assert_eq!(
            a.weekly_overtime_minutes, 60,
            "only the version being assessed contributes for its own date"
        );
        assert!(
            !a.weekly_cap_exceeded,
            "double-counting the superseded row would fake a breach on lawful hours"
        );
    }

    /// The čl. 57 st. 5 ceiling is drawn on the week's whole working time, not on
    /// its overtime: čl. 58 means an employee in preraspodela records
    /// `prekovremeni_minuta = 0`, so a figure built out of overtime alone stays at
    /// zero however long the week runs. Sixty hours is the limit, not a breach of
    /// it — the same strict `>` the čl. 53 legs use.
    #[test]
    fn the_weekly_total_is_the_whole_week_and_sixty_hours_is_the_limit() {
        let week = vec![day(720, 0), day(720, 0), day(720, 0), day(720, 0)]; // 48 h
        let a = assess_caps("2026-08-07", &day(720, 0), &week);
        assert_eq!(
            a.weekly_overtime_minutes, 0,
            "an overtime-only figure would never notice this week"
        );
        assert_eq!(a.weekly_total_minutes, PRERASPODELA_WEEKLY_CAP_MINUTES);
        assert_eq!(
            PRERASPODELA_WEEKLY_CAP_MINUTES, 3600,
            "ZoR čl. 57 st. 5 — 60 časova nedeljno, in minutes"
        );
        assert!(
            a.weekly_total_minutes <= PRERASPODELA_WEEKLY_CAP_MINUTES,
            "exactly 60 h is the ceiling, not over it"
        );

        let a = assess_caps("2026-08-07", &day(721, 0), &week);
        assert!(
            a.weekly_total_minutes > PRERASPODELA_WEEKLY_CAP_MINUTES,
            "60 h + 1 minute is over ZoR čl. 57 st. 5"
        );
        assert!(
            !a.preraspodela_weekly_cap_exceeded,
            "this function cannot see the preraspodela flag — the leg is the caller's"
        );

        // Last week's hours do not carry in: 2026-08-03 is a Monday, so the
        // Sunday before it belongs to the previous „nedelja“.
        let prior_sunday = DayHours {
            dan: "2026-08-02".to_string(),
            efektivno_minuta: 720,
            prekovremeni_minuta: 0,
        };
        let a = assess_caps("2026-08-03", &day(600, 0), &[prior_sunday]);
        assert_eq!(a.weekly_total_minutes, 600);
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

    #[test]
    fn an_employee_under_eighteen_cannot_be_given_overtime_at_all() {
        // čl. 88 st. 1 — an unconditional prohibition, not a warning.
        let p = protection_born("2009-09-01");
        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        let block = blocks
            .iter()
            .find(|b| b.kind == ProtectionKind::MaloletanPrekovremeni)
            .expect("čl. 88 st. 1 fires on any overtime minute");
        assert!(block.blocking, "this one blocks, it does not warn");
    }

    #[test]
    fn an_employee_under_eighteen_is_capped_at_eight_hours_a_day() {
        let p = protection_born("2009-09-01");
        let blocks = check_protection(&p, "2026-08-03", &day(540, 0));
        assert!(blocks
            .iter()
            .any(|b| b.kind == ProtectionKind::MaloletanDnevniLimit));
    }

    /// „Mlađi od 18 godina života“ is strictly younger, so the eighteenth
    /// birthday itself is already outside čl. 87 and čl. 88. Off by one here
    /// either blocks a lawful adult day or lets a minor's overtime through.
    #[test]
    fn the_minor_guards_stop_on_the_eighteenth_birthday() {
        let p = protection_born("2008-08-03");
        let on_the_birthday = check_protection(&p, "2026-08-03", &day(540, 60));
        assert!(
            on_the_birthday.is_empty(),
            "an employee who turned 18 today is no longer „mlađi od 18 godina“"
        );

        let day_before = check_protection(&p, "2026-08-02", &day(540, 60));
        assert!(
            day_before
                .iter()
                .any(|b| b.kind == ProtectionKind::MaloletanPrekovremeni),
            "the day before the eighteenth birthday is still inside čl. 88 st. 1"
        );
    }

    /// čl. 88 st. 1 bans two things in one sentence — „prekovremeni rad **i
    /// preraspodela radnog vremena** zaposlenog koji je mlađi od 18 godina
    /// života“. The preraspodela leg is a ban on the *arrangement*, so it fires
    /// on a plain eight-hour day with no overtime minute anywhere in sight.
    #[test]
    fn a_minor_cannot_be_put_in_preraspodela() {
        let mut p = protection_born("2009-09-01");
        p.radi_u_preraspodeli = true;

        let blocks = check_protection(&p, "2026-08-03", &day(480, 0));
        let block = blocks
            .iter()
            .find(|b| b.kind == ProtectionKind::MaloletanPreraspodela)
            .expect("čl. 88 st. 1 bans the arrangement, not just the day's overtime");
        assert!(block.blocking, "the statute states the prohibition itself");
    }

    /// The ban is on being under 18, not on the flag. An adult in preraspodela is
    /// the ordinary čl. 57 case and must raise nothing.
    #[test]
    fn an_adult_may_work_in_preraspodela() {
        let mut p = protection_born("1990-04-11");
        p.radi_u_preraspodeli = true;
        assert!(check_protection(&p, "2026-08-03", &day(480, 0)).is_empty());
    }

    /// čl. 91 is a consent given *before* the overtime. `saglasnost_prekovremeni_od`
    /// is an „od“ date and v17 only GLOB-checks its shape, so a consent dated after
    /// the day is storable — and reading the column as a mere presence flag would
    /// let it discharge a guard for hours worked seven months earlier.
    #[test]
    fn a_consent_dated_after_the_day_does_not_discharge_cl_91() {
        let mut p = protection_child_born("2020-06-01");
        p.samohrani_roditelj = Some(true);
        p.saglasnost_prekovremeni_od = Some("2027-03-01".to_string());

        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(
            blocks
                .iter()
                .any(|b| b.kind == ProtectionKind::SaglasnostRoditelja),
            "a consent signed after the fact authorises nothing"
        );
    }

    /// The same GLOB that lets `2026-13-45` into `dan` lets it into this column.
    /// A value that is not a date cannot evidence a consent, and dropping the
    /// guard on it is the one direction that costs something.
    #[test]
    fn an_unreadable_consent_date_does_not_discharge_cl_91() {
        let mut p = protection_child_born("2020-06-01");
        p.samohrani_roditelj = Some(true);
        p.saglasnost_prekovremeni_od = Some("2026-13-45".to_string());

        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(
            blocks
                .iter()
                .any(|b| b.kind == ProtectionKind::SaglasnostRoditelja),
            "a non-date must not discharge čl. 91"
        );
    }

    /// A consent dated the day itself covers it: „samo uz svoju pisanu saglasnost“
    /// is satisfied by a consent that exists on that day. Only a later date is a
    /// reconstruction.
    #[test]
    fn a_consent_dated_the_day_itself_discharges_cl_91() {
        let mut p = protection_child_born("2020-06-01");
        p.samohrani_roditelj = Some(true);
        p.saglasnost_prekovremeni_od = Some("2026-08-03".to_string());

        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(!blocks
            .iter()
            .any(|b| b.kind == ProtectionKind::SaglasnostRoditelja));
    }

    /// v17's CHECK on `datum_rodjenja` is a GLOB shape test, so `2009-13-45` is
    /// storable and looks like a date to the operator. Silently treating it as an
    /// unknown date of birth drops the čl. 87 and čl. 88 guards on a profile
    /// somebody believes they filled in — the exact inverse of
    /// `an_unreadable_day_is_counted_rather_than_silently_dropped`.
    #[test]
    fn an_unreadable_date_of_birth_is_reported_rather_than_silently_dropped() {
        let p = protection_born("2009-13-45");
        let blocks = check_protection(&p, "2026-08-03", &day(540, 60));
        let block = blocks
            .iter()
            .find(|b| b.kind == ProtectionKind::NeispravanDatumUProfilu)
            .expect("an unreadable date of birth must be surfaced, not swallowed");
        assert!(
            !block.blocking,
            "the day happened — refusing to record it would hide exposure, \
             so this reports the unusable profile instead"
        );
    }

    /// The same defect on the čl. 91 leg: an unreadable child's date of birth
    /// drops the consent gate instead of raising it.
    #[test]
    fn an_unreadable_child_date_of_birth_is_reported_rather_than_silently_dropped() {
        let mut p = protection_child_born("2020-13-45");
        p.samohrani_roditelj = Some(true);

        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(
            blocks
                .iter()
                .any(|b| b.kind == ProtectionKind::NeispravanDatumUProfilu),
            "the čl. 91 gate vanished on a value the operator believes is a date"
        );
    }

    /// Nothing was lost when the unreadable date was never going to be consulted:
    /// the težak-invalid leg of čl. 91 st. 2 has no age bound, so the gate fired
    /// anyway and a warning here would be noise.
    #[test]
    fn an_unreadable_child_date_is_not_reported_when_the_gate_fired_anyway() {
        let mut p = protection_child_born("2020-13-45");
        p.samohrani_roditelj = Some(true);
        p.dete_tezak_invalid = Some(true);

        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(blocks
            .iter()
            .any(|b| b.kind == ProtectionKind::SaglasnostRoditelja));
        assert!(!blocks
            .iter()
            .any(|b| b.kind == ProtectionKind::NeispravanDatumUProfilu));
    }

    /// v17 leaves `datum_rodjenja` NULL and does not backfill it. Treating an
    /// unknown date of birth as a minor would block every employee nobody has
    /// filled in yet, so the guard stays silent and Task 9's profile screen is
    /// where the gap is closed.
    #[test]
    fn an_unknown_date_of_birth_raises_no_minor_guard() {
        let mut p = protection_born("2009-09-01");
        p.datum_rodjenja = None;
        assert!(check_protection(&p, "2026-08-03", &day(540, 60)).is_empty());
    }

    /// The threshold is SEVEN for a samohrani roditelj (čl. 91 st. 2). The
    /// commonly-quoted fourteen is wrong and would silently drop the guard.
    #[test]
    fn a_single_parent_of_a_child_under_seven_needs_recorded_consent() {
        let mut p = protection_child_born("2020-06-01"); // 6 years old on the test day
        p.samohrani_roditelj = Some(true);
        p.saglasnost_prekovremeni_od = None;

        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        let block = blocks
            .iter()
            .find(|b| b.kind == ProtectionKind::SaglasnostRoditelja)
            .expect("a samohrani roditelj of a six-year-old is inside čl. 91 st. 2");
        assert!(
            block.blocking,
            "čl. 91 requires the written consent before the overtime, not after"
        );

        p.saglasnost_prekovremeni_od = Some("2026-01-15".to_string());
        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(!blocks
            .iter()
            .any(|b| b.kind == ProtectionKind::SaglasnostRoditelja));
    }

    #[test]
    fn a_single_parent_of_a_child_over_seven_needs_no_consent() {
        let mut p = protection_child_born("2018-06-01"); // 8 years old
        p.samohrani_roditelj = Some(true);
        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(!blocks
            .iter()
            .any(|b| b.kind == ProtectionKind::SaglasnostRoditelja));
    }

    /// čl. 91 st. 2 is „dete do sedam godina života ILI dete koje je težak invalid“.
    /// The disability leg has NO age limit — an age-only guard drops it silently.
    #[test]
    fn a_single_parent_of_a_disabled_child_needs_consent_at_any_age() {
        let mut p = protection_child_born("2010-06-01"); // 16 years old
        p.samohrani_roditelj = Some(true);
        p.dete_tezak_invalid = Some(true);
        p.saglasnost_prekovremeni_od = None;

        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(
            blocks
                .iter()
                .any(|b| b.kind == ProtectionKind::SaglasnostRoditelja),
            "the težak-invalid leg of čl. 91 st. 2 is not bounded by the seven-year threshold"
        );
    }

    #[test]
    fn a_non_single_parent_threshold_is_three_not_seven() {
        let mut p = protection_child_born("2022-06-01"); // 4 years old
        p.samohrani_roditelj = Some(false);
        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(
            !blocks
                .iter()
                .any(|b| b.kind == ProtectionKind::SaglasnostRoditelja),
            "čl. 91 st. 1 covers a child up to three"
        );
    }

    /// „Dete do tri godine života“ is read through the third birthday itself:
    /// a day of consent nobody needed costs a prompt, a day too few drops the
    /// čl. 91 guard on the exact day the statute is most obviously engaged.
    #[test]
    fn the_child_threshold_includes_the_birthday_itself() {
        let mut p = protection_child_born("2023-08-03"); // turns three on the test day
        p.samohrani_roditelj = Some(false);
        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        assert!(
            blocks
                .iter()
                .any(|b| b.kind == ProtectionKind::SaglasnostRoditelja),
            "the third birthday is still „dete do tri godine života“"
        );

        let blocks = check_protection(&p, "2026-08-04", &day(480, 60));
        assert!(
            !blocks
                .iter()
                .any(|b| b.kind == ProtectionKind::SaglasnostRoditelja),
            "the day after ends čl. 91 st. 1"
        );
    }

    /// The consent gate is drawn on overtime. A plain eight-hour day of a
    /// protected parent is nobody's business and must raise nothing.
    #[test]
    fn a_day_without_overtime_raises_no_consent_requirement() {
        let mut p = protection_child_born("2024-06-01");
        p.samohrani_roditelj = Some(true);
        assert!(check_protection(&p, "2026-08-03", &day(480, 0)).is_empty());
    }

    /// čl. 90 is NOT an unconditional block — it fires on a health authority's
    /// finding. We store the flag; we never store the finding.
    #[test]
    fn pregnancy_warns_rather_than_blocks() {
        let mut p = protection_born("1995-01-01");
        p.trudnoca_ili_dojenje = Some(true);
        let blocks = check_protection(&p, "2026-08-03", &day(480, 60));
        let block = blocks
            .iter()
            .find(|b| b.kind == ProtectionKind::TrudnocaNocniIPrekovremeni)
            .expect("a warning is raised");
        assert!(
            !block.blocking,
            "čl. 90 depends on a nalaz — the app must not decide it"
        );
    }

    /// Every statutory amount in this application lives in `legal.rs` and is
    /// named by article everywhere else. The profiles below are fixtures chosen
    /// to raise every kind between them, not plausible employees.
    ///
    /// A kind that no fixture raises has its poruka outside this guard entirely.
    /// Two things hold the list total: the `match` below is exhaustive, so a new
    /// variant stops this test compiling until it is enumerated, and the asserted
    /// block count then has to be edited — which is where the missing fixture
    /// gets noticed.
    #[test]
    fn no_protection_message_carries_a_fine_figure() {
        // Under 18, in preraspodela, pregnant, samohrani roditelj of a toddler,
        // twelve hours worked: every blocking kind and the čl. 90 warning.
        let mut svi = protection_born("2009-09-01");
        svi.trudnoca_ili_dojenje = Some(true);
        svi.samohrani_roditelj = Some(true);
        svi.datum_rodjenja_najmladjeg_deteta = Some("2024-01-01".to_string());
        svi.radi_u_preraspodeli = true;

        // The two unusable-profile messages, one per date the guards read.
        let neispravan_rodjenja = protection_born("2009-13-45");
        let mut neispravan_deteta = protection_child_born("2020-13-45");
        neispravan_deteta.samohrani_roditelj = Some(true);

        let mut blocks = check_protection(&svi, "2026-08-03", &day(540, 60));
        assert_eq!(
            blocks.len(),
            5,
            "the fixture must raise every statutory kind or it guards nothing: {blocks:?}"
        );
        blocks.extend(check_protection(
            &neispravan_rodjenja,
            "2026-08-03",
            &day(540, 60),
        ));
        blocks.extend(check_protection(
            &neispravan_deteta,
            "2026-08-03",
            &day(480, 60),
        ));
        assert_eq!(
            blocks.len(),
            7,
            "5 statutory + one unusable-date message per date read: {blocks:?}"
        );

        let ocekivane = [
            ProtectionKind::MaloletanPrekovremeni,
            ProtectionKind::MaloletanPreraspodela,
            ProtectionKind::MaloletanDnevniLimit,
            ProtectionKind::SaglasnostRoditelja,
            ProtectionKind::TrudnocaNocniIPrekovremeni,
            ProtectionKind::NeispravanDatumUProfilu,
        ];
        for vrsta in ocekivane {
            assert!(
                blocks.iter().any(|b| b.kind == vrsta),
                "{vrsta:?} raises a poruka no fixture here ever sees"
            );
        }

        for block in &blocks {
            // Exhaustive on purpose: a new ProtectionKind stops this compiling
            // until it is added to `ocekivane` and raised by a fixture above.
            match block.kind {
                ProtectionKind::MaloletanPrekovremeni
                | ProtectionKind::MaloletanPreraspodela
                | ProtectionKind::MaloletanDnevniLimit
                | ProtectionKind::SaglasnostRoditelja
                | ProtectionKind::TrudnocaNocniIPrekovremeni
                | ProtectionKind::NeispravanDatumUProfilu => {}
            }
            let poruka = block.poruka.to_lowercase();
            assert!(
                !poruka.contains("dinara") && !poruka.contains("kazn") && !poruka.contains(".000"),
                "a fine figure escaped legal.rs: {}",
                block.poruka
            );
        }
    }

    #[test]
    fn preraspodela_suppresses_automatic_overtime_derivation() {
        let mut p = protection_born("1995-01-01");
        p.radi_u_preraspodeli = true;
        assert!(
            !derives_overtime_automatically(&p),
            "čl. 58 hours are not overtime"
        );
        p.radi_u_preraspodeli = false;
        assert!(derives_overtime_automatically(&p));
    }
}
