//! Seven-working-day deposit duty — the calendar half.
//!
//! *Zakon o obavljanju plaćanja pravnih lica, preduzetnika i fizičkih lica koja
//! ne obavljaju delatnost*, "Sl. glasnik RS", br. 68/2015, čl. 3 st. 1: dinars
//! received in cash on any basis go onto the shop's own tekući račun **u roku od
//! sedam radnih dana**. Kazne: čl. 7 st. 1 tač. 2) i st. 3. Never "ZOP".
//!
//! Two honesty constraints this module encodes, both from
//! `docs/SW11-SW15-VERIFIED-RULES.md` §2 Q2 and §5 Q-5:
//!
//! 1. **"Radni dan" is statutorily undefined.** Neither the Zakon nor Pravilnik
//!    77/2011 defines it, and the Zakon o platnim uslugama uses the phrase once,
//!    undefined. So whether Saturday counts is a *setting*, defaulting to `true`
//!    — counting Saturdays yields the earlier, and therefore conservative,
//!    deadline. A wrong assumption the other way produces late alarms.
//! 2. **The holiday table is `[PRUDENTIAL]`, not statutory.** It is seeded for
//!    2026 and 2027 only and is admin-editable, because the list is annual and a
//!    wrong future holiday (which pushes a deadline *later*) is worse than an
//!    absent one.
//!
//! **The clock is a parameter.** The arithmetic here is pure; nothing in it
//! reads the system clock, and `seed_default_non_working_days` takes the RFC3339
//! stamp it writes.
//!
//! Consumed by the bucket builder and the aging report (Tasks 17 and 19), so
//! `dead_code` is allowed until that wiring lands — mirroring the other domain
//! modules.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::params;
use serde::Serialize;
use time::{Date, Month, Weekday};

use crate::app_error::AppError;
use crate::commands::settings::{load_json_setting, save_json_setting};
use crate::state::AppState;

/// Whether Saturday counts as a "radni dan" for čl. 3 st. 1. Default `true`.
pub const SATURDAY_IS_WORKING_DAY_KEY: &str = "saturday_is_working_day";

/// The statutory deposit window, in radni dani (čl. 3 st. 1).
pub const DEPOSIT_WINDOW_WORKING_DAYS: i64 = 7;

/// Serbian state holidays for 2026 and 2027, per *Zakon o državnim i drugim
/// praznicima u Republici Srbiji* ("Sl. glasnik RS", br. 43/2001, 101/2007,
/// 92/2011): the fixed dates plus that year's whole Vaskršnji span, which the
/// act declares non-working "počev od Velikog petka zaključno sa drugim danom
/// Vaskrsa" — so Veliki petak, Velika subota and Vaskrsni ponedeljak are all
/// seeded (Vaskrs itself is a Sunday, already non-working by weekday). The span
/// moves with the Orthodox (Julian) Pascha: 12.04.2026 and 02.05.2027. Velika
/// subota especially must be listed, because Saturdays count as radni dani by
/// default and would otherwise swallow a state holiday.
///
/// 2027 needs no separate Velika subota row: it falls on 01.05.2027, already
/// present as Praznik rada.
///
/// Deliberately **not** seeded past 2027, and deliberately without čl. 3a's
/// "holiday falls on a Sunday, so the next working day is also non-working"
/// roll-over: both would add days the operator did not verify, and every added
/// non-working day pushes the deadline *later* — the unsafe direction. The table
/// is admin-editable so the shop can correct and extend it.
const DEFAULT_NON_WORKING_DAYS: &[(&str, &str)] = &[
    ("2026-01-01", "Nova godina"),
    ("2026-01-02", "Nova godina"),
    ("2026-01-07", "Božić"),
    ("2026-02-15", "Dan državnosti Srbije"),
    ("2026-02-16", "Dan državnosti Srbije"),
    ("2026-04-10", "Veliki petak"),
    ("2026-04-11", "Velika subota"),
    ("2026-04-13", "Vaskrsni ponedeljak"),
    ("2026-05-01", "Praznik rada"),
    ("2026-05-02", "Praznik rada"),
    ("2026-11-11", "Dan primirja u Prvom svetskom ratu"),
    ("2027-01-01", "Nova godina"),
    ("2027-01-02", "Nova godina"),
    ("2027-01-07", "Božić"),
    ("2027-02-15", "Dan državnosti Srbije"),
    ("2027-02-16", "Dan državnosti Srbije"),
    ("2027-04-30", "Veliki petak"),
    ("2027-05-01", "Praznik rada"),
    ("2027-05-02", "Praznik rada"),
    ("2027-05-03", "Vaskrsni ponedeljak"),
    ("2027-11-11", "Dan primirja u Prvom svetskom ratu"),
];

/// The last calendar year the default table covers. A deadline computed beyond
/// it is arithmetically sound but calendar-blind, and the aging report says so.
pub const SEEDED_CALENDAR_HORIZON_YEAR: i32 = 2027;

/// Adds `days` working days to `start` (ISO `yyyy-MM-dd`), never counting the
/// start day itself. "Radni dan" is statutorily undefined; Saturday is
/// configurable and defaults to counting, which yields the earlier — and
/// therefore conservative — deadline.
///
/// Returns `None` on a start date that is not strictly `yyyy-MM-dd`, rather than
/// guessing at a locale, and on a walk past the representable calendar.
pub fn add_working_days(
    start: &str,
    days: i64,
    non_working: &BTreeSet<String>,
    saturday_is_working: bool,
) -> Option<String> {
    let mut current = parse_iso_date(start)?;
    let mut remaining = days;
    while remaining > 0 {
        current = current.next_day()?;
        let iso = format_iso_date(current);
        let is_working = match current.weekday() {
            Weekday::Sunday => false,
            Weekday::Saturday => saturday_is_working,
            _ => true,
        } && !non_working.contains(&iso);
        if is_working {
            remaining -= 1;
        }
    }
    Some(format_iso_date(current))
}

/// Strict `yyyy-MM-dd`. Anything else — `31.07.2026`, `2026-7-1`, `+026-01-01` —
/// is a parse failure, not an invitation to infer an order for the fields.
fn parse_iso_date(value: &str) -> Option<Date> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return None;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit())
    {
        return None;
    }

    let year: i32 = value.get(0..4)?.parse().ok()?;
    let month: u8 = value.get(5..7)?.parse().ok()?;
    let day: u8 = value.get(8..10)?.parse().ok()?;

    Date::from_calendar_date(year, Month::try_from(month).ok()?, day).ok()
}

fn format_iso_date(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// One cash receipt as the duty sees it: an amount received on one trading date,
/// and whether čl. 3 st. 1 reaches it at all.
///
/// `subject == false` is the Pravilnik 77/2011 čl. 5 st. 2 float carve-out —
/// dinars paid out of the shop's own tekući račun per čl. 2 st. 2 i 3. **The
/// caller decides**, because the exclusion turns on how the withdrawal was
/// documented, not on the movement type alone (§3 rule 14), and because the
/// relief is bylaw-level: the statute's own "po bilo kom osnovu" and its kazna
/// (čl. 7 st. 1 tač. 2) contain no exclusion.
///
/// `amount_minor` may be negative — a documented payout out of the till reduces
/// that trading date's base.
#[derive(Debug, Clone)]
pub struct CashInflow {
    pub date: String,
    pub amount_minor: i64,
    pub subject: bool,
}

/// One polog onto the shop's own račun kod banke.
#[derive(Debug, Clone)]
pub struct CashDeposit {
    pub date: String,
    pub amount_minor: i64,
}

/// The calendar the per-bucket deadlines are computed against. Held by the
/// caller so the arithmetic stays pure and the clock stays a parameter.
#[derive(Debug, Clone)]
pub struct BucketConfig {
    pub non_working_days: BTreeSet<String>,
    pub saturday_is_working: bool,
}

/// Cash received on one trading date, and how much of it has reached the bank.
///
/// Per-trading-date aggregation is an **implementation convention**, not the
/// statute: čl. 3 st. 1 runs from receipt of the cash, and neither it nor
/// Pravilnik 77/2011 knows anything about a dnevni izveštaj or a Z-report. The
/// UI must say so (§3 rule 11).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositBucket {
    pub trading_date: String,
    pub subject_minor: i64,
    pub deposited_minor: i64,
    pub outstanding_minor: i64,
    /// `None` only when `trading_date` is not strictly `yyyy-MM-dd`; the report
    /// then shows no deadline rather than a guessed one.
    pub due_on: Option<String>,
}

/// Builds the deposit buckets, oldest first, and draws every polog down against
/// the oldest bucket it is *eligible* for.
///
/// A polog may only discharge cash the shop had already received when it was
/// made — buckets whose `trading_date` is at or before `deposit.date`. Čl. 3
/// st. 1 runs from receipt, so a deposit cannot satisfy the duty for takings
/// that did not exist yet. Deposits are therefore applied chronologically, and
/// among the eligible buckets the oldest is drawn down first (§3 rule 12); the
/// ascending sort below is load-bearing, not cosmetic.
///
/// Partial deposits are lawful — the duty is not all-or-nothing — so a polog
/// smaller than the bucket leaves a remainder rather than clearing or failing.
/// Amount that no eligible bucket can absorb is **dropped**, not carried
/// forward: outstanding saturates at zero and no later bucket is credited,
/// because either would be credit against future takings that the statute does
/// not grant, and would hand the owner a false "clean" state. Over-reporting an
/// obaveza is the safe direction; a silently pre-paid bucket is not.
///
/// A trading date whose documented payouts consumed the whole take carries no
/// duty and gets no bucket.
pub fn build_buckets(
    inflows: &[CashInflow],
    deposits: &[CashDeposit],
    config: &BucketConfig,
) -> Vec<DepositBucket> {
    let mut subject_by_date: BTreeMap<&str, i64> = BTreeMap::new();
    for inflow in inflows.iter().filter(|inflow| inflow.subject) {
        let total = subject_by_date.entry(inflow.date.as_str()).or_insert(0);
        *total = total.saturating_add(inflow.amount_minor);
    }

    // ISO dates sort lexicographically, so the BTreeMap already hands them back
    // chronologically — that ordering *is* the FIFO queue.
    let mut buckets: Vec<DepositBucket> = subject_by_date
        .into_iter()
        .filter(|(_, subject_minor)| *subject_minor > 0)
        .map(|(trading_date, subject_minor)| DepositBucket {
            trading_date: trading_date.to_string(),
            subject_minor,
            deposited_minor: 0,
            outstanding_minor: subject_minor,
            due_on: add_working_days(
                trading_date,
                DEPOSIT_WINDOW_WORKING_DAYS,
                &config.non_working_days,
                config.saturday_is_working,
            ),
        })
        .collect();

    let mut ordered: Vec<&CashDeposit> = deposits.iter().collect();
    ordered.sort_by(|left, right| left.date.cmp(&right.date));

    for deposit in ordered {
        let mut remaining = deposit.amount_minor.max(0);
        for bucket in buckets.iter_mut() {
            if remaining == 0 {
                break;
            }
            // Both are `yyyy-MM-dd`, so lexicographic order is chronological —
            // the same property the BTreeMap above relies on. `continue`, not
            // `break`: eligibility is a per-bucket test, so the walk must not
            // depend on the buckets happening to be sorted.
            if bucket.trading_date.as_str() > deposit.date.as_str() {
                continue;
            }
            let applied = remaining.min(bucket.outstanding_minor);
            if applied <= 0 {
                continue;
            }
            bucket.deposited_minor = bucket.deposited_minor.saturating_add(applied);
            bucket.outstanding_minor = bucket.outstanding_minor.saturating_sub(applied);
            remaining -= applied;
        }
    }

    buckets
}

/// Loads the non-working-day calendar the deadline arithmetic runs against.
pub fn load_non_working_days(state: &AppState) -> Result<BTreeSet<String>, AppError> {
    let conn = state.db().open()?;
    let mut statement = conn.prepare("SELECT day FROM non_working_days")?;
    let days = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<BTreeSet<String>, rusqlite::Error>>()?;
    Ok(days)
}

/// Seeds the 2026–2027 Serbian state holidays. Idempotent: an admin edit to a
/// seeded day survives a re-seed, because every insert is `INSERT OR IGNORE`.
/// Returns how many rows were actually added.
pub fn seed_default_non_working_days(state: &AppState, now: &str) -> Result<usize, AppError> {
    let mut conn = state.db().open()?;
    let tx = conn.transaction()?;

    let mut inserted = 0usize;
    for (day, label) in DEFAULT_NON_WORKING_DAYS {
        inserted += tx.execute(
            "INSERT OR IGNORE INTO non_working_days (day, label, created_at)
             VALUES (?1, ?2, ?3)",
            params![day, label, now],
        )?;
    }

    tx.commit()?;
    Ok(inserted)
}

/// Defaults to `true`: counting Saturdays produces the earlier deadline, and the
/// statute never defines "radni dan" (§5 Q-5).
pub fn saturday_is_working_day(state: &AppState) -> Result<bool, AppError> {
    load_json_setting(state, SATURDAY_IS_WORKING_DAY_KEY, true)
}

pub fn set_saturday_is_working_day(state: &AppState, counts: bool) -> Result<(), AppError> {
    save_json_setting(state, SATURDAY_IS_WORKING_DAY_KEY, &counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn holidays(days: &[&str]) -> BTreeSet<String> {
        days.iter().map(|d| (*d).to_string()).collect()
    }

    fn inflow(date: &str, amount_minor: i64) -> CashInflow {
        CashInflow {
            date: date.to_string(),
            amount_minor,
            subject: true,
        }
    }

    fn deposit(date: &str, amount_minor: i64) -> CashDeposit {
        CashDeposit {
            date: date.to_string(),
            amount_minor,
        }
    }

    /// An empty calendar with Saturdays counting — the shipped default.
    fn config() -> BucketConfig {
        BucketConfig {
            non_working_days: BTreeSet::new(),
            saturday_is_working: true,
        }
    }

    /// The shipped calendar, as the arithmetic sees it after a seed.
    fn seeded_calendar() -> BTreeSet<String> {
        DEFAULT_NON_WORKING_DAYS
            .iter()
            .map(|(day, _)| (*day).to_string())
            .collect()
    }

    #[test]
    fn the_seeded_calendar_covers_the_whole_2026_vaskrsnji_span() {
        // Vaskrs 2026 is Sunday 12.04, so the non-working span runs Veliki petak
        // 10.04, Velika subota 11.04, Vaskrs 12.04, Vaskrsni ponedeljak 13.04.
        // From Thursday 09.04 the first radni dan is therefore Tuesday 14.04 —
        // and it stays Tuesday even with Saturdays counting, which is the case
        // a missing Velika subota would silently break.
        let due = add_working_days("2026-04-09", 1, &seeded_calendar(), true).expect("date");
        assert_eq!(
            due, "2026-04-14",
            "Velika subota is inside the Vaskršnji span, so it cannot be a radni dan"
        );
    }

    #[test]
    fn seven_working_days_skips_sundays_when_saturday_counts() {
        // 2026-07-31 is a Friday. Counting Saturdays, skipping Sundays:
        // Sat 01.08(1) Mon 03.08(2) Tue 04(3) Wed 05(4) Thu 06(5) Fri 07(6) Sat 08(7)
        let due =
            add_working_days("2026-07-31", 7, &holidays(&[]), true).expect("date should compute");
        assert_eq!(due, "2026-08-08");
    }

    #[test]
    fn excluding_saturdays_pushes_the_deadline_later() {
        // Mon 03(1) Tue 04(2) Wed 05(3) Thu 06(4) Fri 07(5) Mon 10(6) Tue 11(7)
        let due =
            add_working_days("2026-07-31", 7, &holidays(&[]), false).expect("date should compute");
        assert_eq!(due, "2026-08-11");
        assert!(
            due.as_str() > "2026-08-08",
            "Saturday-counting is the conservative default"
        );
    }

    #[test]
    fn a_holiday_extends_the_deadline() {
        let without = add_working_days("2026-07-31", 7, &holidays(&[]), true).expect("date");
        let with =
            add_working_days("2026-07-31", 7, &holidays(&["2026-08-03"]), true).expect("date");

        assert_eq!(without, "2026-08-08");
        assert_eq!(
            with, "2026-08-10",
            "Monday is a holiday, so the count rolls on"
        );
    }

    #[test]
    fn the_receipt_day_itself_is_not_counted() {
        let due = add_working_days("2026-08-03", 1, &holidays(&[]), true).expect("date");
        assert_eq!(due, "2026-08-04", "the clock starts the day after receipt");
    }

    #[test]
    fn rejects_a_malformed_start_date_rather_than_guessing() {
        assert!(add_working_days("31.07.2026", 7, &holidays(&[]), true).is_none());
    }

    #[test]
    fn deposits_discharge_the_oldest_bucket_first() {
        let inflows = vec![inflow("2026-08-03", 100_000), inflow("2026-08-04", 50_000)];
        let deposits = vec![deposit("2026-08-05", 120_000)];

        let buckets = build_buckets(&inflows, &deposits, &config());

        assert_eq!(
            buckets[0].outstanding_minor, 0,
            "the older bucket clears first"
        );
        assert_eq!(
            buckets[1].outstanding_minor, 30_000,
            "the remainder lands on the newer one"
        );
    }

    #[test]
    fn a_partial_deposit_is_lawful_and_leaves_a_remainder() {
        let buckets = build_buckets(
            &[inflow("2026-08-03", 100_000)],
            &[deposit("2026-08-04", 40_000)],
            &config(),
        );

        assert_eq!(buckets[0].deposited_minor, 40_000);
        assert_eq!(buckets[0].outstanding_minor, 60_000);
    }

    /// Pravilnik 77/2011 čl. 5 st. 2 — cash withdrawn from the shop's own account
    /// is not "gotov novac" for this duty. Without this the app ages the owner's
    /// change float as undeposited pazar and invents violations.
    #[test]
    fn bank_withdrawal_never_enters_the_subject_base() {
        let buckets = build_buckets(
            &[
                inflow("2026-08-03", 100_000),
                CashInflow {
                    date: "2026-08-03".into(),
                    amount_minor: 500_000,
                    subject: false,
                },
            ],
            &[],
            &config(),
        );

        assert_eq!(buckets[0].subject_minor, 100_000, "the float is excluded");
    }

    #[test]
    fn each_bucket_carries_its_own_seven_working_day_deadline() {
        let buckets = build_buckets(&[inflow("2026-07-31", 10_000)], &[], &config());
        assert_eq!(buckets[0].due_on, Some("2026-08-08".to_string()));
    }

    #[test]
    fn over_depositing_never_produces_a_negative_outstanding() {
        let buckets = build_buckets(
            &[inflow("2026-08-03", 10_000)],
            &[deposit("2026-08-04", 999_000)],
            &config(),
        );
        assert_eq!(buckets[0].outstanding_minor, 0);
    }

    /// Čl. 3 st. 1 runs from *receipt* of the cash: a polog made before the
    /// money existed cannot have deposited it. Without the eligibility guard a
    /// `bank_deposit` row that predates the sales — pre-migration takings, the
    /// owner paying in his own dinars, a deposit recorded ahead of the pazar —
    /// becomes a standing credit that marks every later bucket satisfied, which
    /// is exactly the false "clean" state §3 rule 10 forbids.
    #[test]
    fn a_deposit_cannot_discharge_cash_received_after_it() {
        let buckets = build_buckets(
            &[inflow("2026-08-10", 100_000)],
            &[deposit("2026-08-01", 100_000)],
            &config(),
        );

        assert_eq!(
            buckets[0].deposited_minor, 0,
            "the polog predates the take, so it deposited none of it"
        );
        assert_eq!(
            buckets[0].outstanding_minor, 100_000,
            "the whole take is still outstanding"
        );
    }

    /// The excess is dropped, not carried forward. Over-reporting an obaveza is
    /// the safe direction; a future bucket silently pre-paid is not.
    #[test]
    fn over_depositing_is_not_credited_against_a_later_trading_date() {
        let buckets = build_buckets(
            &[inflow("2026-08-03", 10_000), inflow("2026-08-20", 500_000)],
            &[deposit("2026-08-04", 999_000)],
            &config(),
        );

        assert_eq!(buckets[0].outstanding_minor, 0, "03.08 is covered");
        assert_eq!(
            buckets[1].outstanding_minor, 500_000,
            "the 04.08 excess cannot pay for cash received on 20.08"
        );
        assert_eq!(buckets[1].deposited_minor, 0);
    }

    /// Ineligibility is per polog, not permanent: the bucket an early deposit
    /// could not lawfully reach is still discharged by a later one. Dropping the
    /// early excess must not also drop the duty.
    #[test]
    fn a_later_polog_discharges_a_bucket_the_earlier_one_could_not_reach() {
        let buckets = build_buckets(
            &[inflow("2026-08-03", 10_000), inflow("2026-08-20", 30_000)],
            &[
                deposit("2026-08-04", 100_000),
                deposit("2026-08-21", 30_000),
            ],
            &config(),
        );

        assert_eq!(buckets[0].outstanding_minor, 0, "04.08 covers 03.08");
        assert_eq!(
            buckets[1].deposited_minor, 30_000,
            "21.08 is after the take, so it may discharge it"
        );
        assert_eq!(buckets[1].outstanding_minor, 0);
    }

    /// With eligibility in play the ascending deposit order is load-bearing:
    /// applying 06.08's 150.000 first would clear both buckets, whereas the
    /// lawful chronological pass leaves 03.08's remainder for it.
    #[test]
    fn deposits_are_applied_in_chronological_order() {
        let inflows = vec![inflow("2026-08-03", 100_000), inflow("2026-08-05", 60_000)];
        let deposits = vec![
            deposit("2026-08-06", 150_000),
            deposit("2026-08-04", 40_000),
        ];

        let buckets = build_buckets(&inflows, &deposits, &config());

        assert_eq!(buckets[0].outstanding_minor, 0);
        assert_eq!(buckets[1].outstanding_minor, 0);
        assert_eq!(
            buckets[0].deposited_minor, 100_000,
            "04.08 pays 40.000 toward 03.08; 06.08 finishes it"
        );
        assert_eq!(buckets[1].deposited_minor, 60_000);
    }
}
