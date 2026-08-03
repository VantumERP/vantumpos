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
//! The aging report, its CSV export and the admin-editable calendar all live
//! here behind `require_admin` — the gate sits inside each function rather than
//! in `crate::commands::cash_deposit`, so no other caller can go around it.
//! `dead_code` stays allowed for the pure helpers the tests exercise directly,
//! mirroring the other domain modules.

#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, Connection};
use serde::Serialize;
use time::{Date, Month, Weekday};

use crate::app_error::AppError;
use crate::commands::reports::csv_line;
use crate::commands::settings::{load_json_setting, load_shop_profile, save_json_setting};
use crate::legal::{cash_deposit_duty, LegalNotice};
use crate::state::AppState;

/// Whether Saturday counts as a "radni dan" for čl. 3 st. 1. Default `true`.
pub const SATURDAY_IS_WORKING_DAY_KEY: &str = "saturday_is_working_day";

/// The statutory deposit window, in radni dani (čl. 3 st. 1).
pub const DEPOSIT_WINDOW_WORKING_DAYS: i64 = 7;

/// Records that `DEFAULT_NON_WORKING_DAYS` has already been written to this
/// install, so the seed never runs a second time and an admin deletion sticks.
pub const NON_WORKING_DAYS_SEED_VERSION_KEY: &str = "non_working_days_seeded_version";

/// Bump only when `DEFAULT_NON_WORKING_DAYS` gains days — see
/// `ensure_default_non_working_days` for what a bump costs.
pub const NON_WORKING_DAYS_SEED_VERSION: i64 = 1;

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
///
/// `pub(crate)` so that `worktime.rs` can anchor the ZoR čl. 53 st. 2 calendar
/// week on the same civil-date reader this module's deadline walk is proven on,
/// rather than the codebase carrying a second date parser that could disagree
/// with this one about what a day is.
pub(crate) fn parse_iso_date(value: &str) -> Option<Date> {
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
/// dinars paid out of the shop's own tekući račun per čl. 2 st. 2 i 3. The
/// relief is bylaw-level: the statute's own "po bilo kom osnovu" and its kazna
/// (čl. 7 st. 1 tač. 2) contain no exclusion at all.
///
/// **The carve-out turns on the evidence, not on the movement type.** §3 rule 14
/// is explicit that not all bank withdrawals may be excluded, so
/// `load_cash_inflows` sets `subject = false` only for a `bank_withdrawal` the
/// operator affirmatively marked `documented_per_pravilnik = 1`. An unasserted
/// (`NULL`) or denied (`0`) withdrawal stays subject: the exclusion defaults
/// **off**, because a wrongly excluded amount shrinks the čl. 3 st. 1 base and
/// hands the owner the false „izmireno" state §3 rule 10 forbids, while a
/// wrongly included one merely over-reports — the safe direction.
///
/// A caller constructing `CashInflow` values by hand still decides `subject`
/// itself; nothing here forces the loader's rule on it.
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
    /// Whether the deadline has passed on the report's as-of date, with money
    /// still outstanding.
    ///
    /// `build_buckets` cannot decide this — being late is a fact about a date
    /// the pure builder is never given — so it always emits `false` and
    /// `cash_deposit_report` sets it. A bucket obtained any other way must be
    /// read as "not yet assessed", never as "in roku".
    pub is_overdue: bool,
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
            is_overdue: false,
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

/// Seeds the 2026–2027 Serbian state holidays. `INSERT OR IGNORE` keyed on
/// `day`, so a **relabel** survives a re-seed — but a **deletion does not**: the
/// row comes straight back. Nothing outside `ensure_default_non_working_days`
/// may call this on a read path for exactly that reason. Returns how many rows
/// were actually added.
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

/// One admin-editable non-working day. `day` is the primary key, so a re-seed
/// never overwrites an edit.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NonWorkingDay {
    pub day: String,
    pub label: String,
}

/// The calendar the seven-working-day count runs against, as the Settings
/// surface sees it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashDepositCalendar {
    pub saturday_is_working: bool,
    /// Chronological, so the operator reads it the way a calendar reads.
    pub days: Vec<NonWorkingDay>,
    pub horizon_year: i32,
}

/// Reads the calendar for the Settings surface.
///
/// Admin-gated **here**, like the report, so no other caller can reach the
/// figures that decide a statutory deadline by going around the command layer.
pub fn calendar(state: &AppState) -> Result<CashDepositCalendar, AppError> {
    crate::commands::auth::require_admin(state)?;

    let conn = state.db().open()?;
    let mut statement = conn.prepare("SELECT day, label FROM non_working_days ORDER BY day")?;
    let days = statement
        .query_map([], |row| {
            Ok(NonWorkingDay {
                day: row.get(0)?,
                label: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;

    Ok(CashDepositCalendar {
        saturday_is_working: saturday_is_working_day(state)?,
        days,
        horizon_year: SEEDED_CALENDAR_HORIZON_YEAR,
    })
}

/// Seeds the shipped holiday table **once per install**, never on every read.
///
/// The marker is what makes an admin deletion durable. `INSERT OR IGNORE` is
/// keyed on `day`, so a re-seed silently restores any shipped day the shop
/// removed — and restoring a non-working day pushes the čl. 3 st. 1 rok *later*,
/// the unsafe direction this module refuses everywhere else.
///
/// Bumping `NON_WORKING_DAYS_SEED_VERSION` re-runs the whole seed, so it is only
/// for genuinely extending `DEFAULT_NON_WORKING_DAYS` to a year the shop cannot
/// have curated yet; a bump does restore shipped days the shop deleted, so the
/// operator must be told to re-check the list when one happens. The same applies
/// once to an install that predates the marker: it seeds a second time on
/// upgrade, and from then on the deletion is durable.
pub fn ensure_default_non_working_days(state: &AppState, now: &str) -> Result<usize, AppError> {
    let seeded: i64 = load_json_setting(state, NON_WORKING_DAYS_SEED_VERSION_KEY, 0)?;
    if seeded >= NON_WORKING_DAYS_SEED_VERSION {
        return Ok(0);
    }

    let inserted = seed_default_non_working_days(state, now)?;
    save_json_setting(
        state,
        NON_WORKING_DAYS_SEED_VERSION_KEY,
        &NON_WORKING_DAYS_SEED_VERSION,
    )?;
    Ok(inserted)
}

/// The Settings surface, with the shipped holiday table in place.
///
/// Every command that renders the calendar goes through here, so the seed
/// decision is made in one place and can be tested without a Tauri `State`.
pub fn calendar_with_seeded_defaults(
    state: &AppState,
    now: &str,
) -> Result<CashDepositCalendar, AppError> {
    // Gate first: seeding is a write, and an unauthenticated caller must not
    // provoke one.
    crate::commands::auth::require_admin(state)?;
    ensure_default_non_working_days(state, now)?;
    calendar(state)
}

/// The aging report, computed against the same holiday table the Settings
/// calendar shows — never against an empty one just because nobody has opened
/// Podešavanja yet. Both surfaces seed through
/// `ensure_default_non_working_days`, so whichever the operator reaches first
/// establishes the table and neither can later move the other's deadlines.
pub fn report_with_seeded_defaults(
    state: &AppState,
    as_of: &str,
    now: &str,
) -> Result<CashDepositReport, AppError> {
    crate::commands::auth::require_admin(state)?;
    ensure_default_non_working_days(state, now)?;
    cash_deposit_report(state, as_of)
}

/// Sets whether Saturday counts as a radni dan. Never defaulted to `false`:
/// "radni dan" is statutorily undefined (§5 Q-5), and excluding Saturdays moves
/// every deadline later — the unsafe direction.
pub fn set_saturday_is_working(
    state: &AppState,
    counts: bool,
) -> Result<CashDepositCalendar, AppError> {
    crate::commands::auth::require_admin(state)?;
    save_json_setting(state, SATURDAY_IS_WORKING_DAY_KEY, &counts)?;
    calendar(state)
}

/// Adds or relabels one non-working day.
///
/// `day` must be a real `yyyy-MM-dd`: the arithmetic compares it as a string
/// against ISO dates it generates itself, so a rendered "05.08.2026." would sit
/// in the table forever and never match anything. The label is required —
/// an unlabelled day cannot be re-checked by the operator or the knjigovođa.
pub fn save_non_working_day(
    state: &AppState,
    day: &str,
    label: &str,
    now: &str,
) -> Result<CashDepositCalendar, AppError> {
    crate::commands::auth::require_admin(state)?;

    if parse_iso_date(day).is_none() {
        return Err(AppError::validation(
            "Datum neradnog dana mora biti u obliku gggg-MM-dd.",
            serde_json::json!({ "day": day }),
        ));
    }
    let label = label.trim();
    if label.is_empty() {
        return Err(AppError::validation(
            "Unesite naziv neradnog dana.",
            serde_json::json!({ "day": day }),
        ));
    }

    state.db().open()?.execute(
        "INSERT INTO non_working_days (day, label, created_at)
         VALUES (?1, ?2, ?3)
         ON CONFLICT(day) DO UPDATE SET label = excluded.label",
        params![day, label, now],
    )?;

    calendar(state)
}

/// Removes one non-working day. Deleting pulls every deadline that spans it
/// *earlier*, so it is the conservative direction and needs no confirmation
/// beyond the admin gate.
pub fn delete_non_working_day(
    state: &AppState,
    day: &str,
) -> Result<CashDepositCalendar, AppError> {
    crate::commands::auth::require_admin(state)?;

    state
        .db()
        .open()?
        .execute("DELETE FROM non_working_days WHERE day = ?1", params![day])?;

    calendar(state)
}

/// §3 rule 11 — the per-trading-date roll-up is ours, not the law's.
const FOOTER_AGGREGATION_IS_A_CONVENTION: &str =
    "Zbir po danu prometa je konvencija ove aplikacije, a ne zakonska kategorija: rok teče od \
     prijema gotovine, a ni Zakon 68/2015 ni Pravilnik 77/2011 ne poznaju dnevni izveštaj.";

/// §3 rule 14 — the carve-out the app relies on lives one level below the act,
/// **and carries a condition**. Pravilnik čl. 5 st. 2 excludes only dinars paid
/// out per čl. 2 st. 2 (against the original documentation submitted to the bank
/// na uvid i overu) or st. 3 (the 150.000 RSD/day undocumented lane). The app
/// now excludes a withdrawal only where the operator asserted that condition per
/// movement, so this note says which withdrawals were excluded and which were
/// not — the reader must be able to tell the aged amounts from the relieved ones
/// without opening the ledger.
const FOOTER_FLOAT_EXCLUSION_IS_BYLAW_RELIEF: &str =
    "Iz osnovice je izuzeta samo ona gotovina podignuta sa tekućeg računa radnje za koju je \
     zabeleženo da je isplaćena u skladu sa Pravilnikom 77/2011 čl. 2 st. 2 (uz originalnu \
     dokumentaciju podnetu banci na uvid i overu) ili čl. 2 st. 3 (dnevni limit od 150.000 \
     dinara bez dokumentacije). Podizanja bez te potvrde ostaju u osnovici i imaju rok za polog. \
     Izuzeće je olakšica na nivou podzakonskog akta — sam zakon („po bilo kom osnovu“) i kazna \
     iz čl. 7 ne sadrže nijedan izuzetak.";

/// §3 rule 16 — advisory, supervised by Poreska uprava, blocking nothing.
const FOOTER_ADVISORY_ONLY: &str =
    "Izveštaj je informativan: nadzor vrši Poreska uprava, a rok ne blokira prodaju, \
     zatvaranje smene ni fiskalizaciju.";

/// The copy that must travel with every rendering of this report — screen,
/// print and CSV alike — because each sentence corrects something the table of
/// numbers would otherwise imply.
fn report_footer(beyond_seeded_calendar: bool) -> String {
    let mut parts = vec![
        FOOTER_AGGREGATION_IS_A_CONVENTION.to_string(),
        FOOTER_FLOAT_EXCLUSION_IS_BYLAW_RELIEF.to_string(),
        FOOTER_ADVISORY_ONLY.to_string(),
    ];
    if beyond_seeded_calendar {
        parts.push(format!(
            "Rok koji pada posle {SEEDED_CALENDAR_HORIZON_YEAR}. godine izračunat je bez tabele \
             praznika za tu godinu — dopunite listu neradnih dana u Podešavanjima."
        ));
    }
    parts.join(" ")
}

/// The undeposited-cash aging report (§3 rule 17): every open trading-date
/// bucket, its deadline, and the honesty labels that must be rendered with it.
///
/// Advisory by construction — it carries no blocking flag, because čl. 3 st. 1
/// is fiscal hygiene supervised by Poreska uprava, not a condition of a valid
/// sale (§3 rule 16).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CashDepositReport {
    /// The date the deadlines were assessed against, `yyyy-MM-dd`.
    pub as_of: String,
    pub buckets: Vec<DepositBucket>,
    pub outstanding_minor: i64,
    /// Of the outstanding total, the part whose deadline has already passed.
    pub overdue_minor: i64,
    /// Cash the Pravilnik čl. 5 st. 2 carve-out kept out of the base. Shown so
    /// the operator can see what was excluded instead of having to trust that
    /// nothing was.
    pub excluded_float_minor: i64,
    pub saturday_is_working: bool,
    pub calendar_horizon_year: i32,
    /// A deadline falls past the seeded holiday table, so the count for it is
    /// arithmetically sound but calendar-blind.
    pub beyond_seeded_calendar: bool,
    pub notice: LegalNotice,
    pub footer: String,
}

/// Every dinar of cash the shop received **up to and including `as_of`**,
/// aggregated per trading date.
///
/// "Po bilo kom osnovu" (čl. 3 st. 1) is wider than the pazar, so the base is
/// receipt cash **plus** cash paid into the drawer by hand (`pay_in`) — a
/// supplier refund, cash rent, the proceeds of an asset sale (§3 rule 10).
/// Documented payouts (`pay_out`) come back off that date's base, and cash
/// drawn from the shop's own račun (`bank_withdrawal`) is carried with
/// `subject = false` — reported, never aged — **only** where the operator
/// asserted the Pravilnik documentation; see below.
///
/// `as_of` is a real cut-off, not a caption. Cash the shop had not yet received
/// on the presek date is no part of that date's obligation, and a report headed
/// "Presek na dan …" that swept in later takings would be reading the future.
/// The bound must be `yyyy-MM-dd`; the caller validates it (`cash_deposit_report`).
///
/// The receipt arm deliberately does **not** filter on `sales.status`. A void
/// and a return each write a linked document row carrying the *negative*
/// payment, so summing every `sale_payments` row nets the cash the shop
/// actually kept. Filtering to `status = 'completed'` would instead drop the
/// whole original receipt the moment a **partial** return flipped it to
/// `refunded`, erasing cash that never left the drawer — the false "clean"
/// state §3 rule 10 forbids.
///
/// **The `bank_withdrawal` arm is split in two, and the exclusion defaults
/// off.** Pravilnik 77/2011 čl. 5 st. 2 only reaches dinars paid out per čl. 2
/// st. 2 (against the original documentation submitted to the bank na uvid i
/// overu) or st. 3 (the 150.000 RSD/day undocumented lane), and §3 rule 14 says
/// in terms: do not exclude all bank withdrawals. So only
/// `documented_per_pravilnik = 1` — the operator's own assertion, recorded per
/// movement (migration v16) — comes back `subject = 0`. `NULL` (nothing
/// asserted, which is every pre-v16 row and every movement recorded without the
/// box ticked) and `0` come back `subject = 1`.
///
/// The `NULL` branch is written out rather than left to fall through, because
/// `documented_per_pravilnik = 1` and `documented_per_pravilnik <> 1` do **not**
/// partition the rows in SQL: `NULL <> 1` is `NULL`, so a fall-through arm would
/// silently drop every unasserted withdrawal out of the base — the same
/// under-reporting this split exists to end.
///
/// Getting this wrong in the excluding direction compounds: a withdrawal kept
/// out of the base whose cash is later re-deposited still draws down genuine
/// sale buckets, so a withdraw-then-redeposit float cycle under-reports twice.
pub fn load_cash_inflows(
    connection: &Connection,
    as_of: &str,
) -> Result<Vec<CashInflow>, AppError> {
    let mut statement = connection.prepare(
        r#"
SELECT substr(s.created_at, 1, 10) AS day, SUM(sp.amount_minor) AS amount_minor, 1 AS subject
  FROM sales s
  JOIN sale_payments sp ON sp.sale_id = s.id
 WHERE sp.payment_method = 'cash'
   AND substr(s.created_at, 1, 10) <= ?1
 GROUP BY day
UNION ALL
SELECT substr(created_at, 1, 10), SUM(amount_minor), 1
  FROM cash_movements
 WHERE movement_type = 'pay_in'
   AND substr(created_at, 1, 10) <= ?1
 GROUP BY substr(created_at, 1, 10)
UNION ALL
SELECT substr(created_at, 1, 10), -SUM(amount_minor), 1
  FROM cash_movements
 WHERE movement_type = 'pay_out'
   AND substr(created_at, 1, 10) <= ?1
 GROUP BY substr(created_at, 1, 10)
UNION ALL
SELECT substr(created_at, 1, 10), SUM(amount_minor), 0
  FROM cash_movements
 WHERE movement_type = 'bank_withdrawal'
   AND documented_per_pravilnik = 1
   AND substr(created_at, 1, 10) <= ?1
 GROUP BY substr(created_at, 1, 10)
UNION ALL
SELECT substr(created_at, 1, 10), SUM(amount_minor), 1
  FROM cash_movements
 WHERE movement_type = 'bank_withdrawal'
   AND (documented_per_pravilnik IS NULL OR documented_per_pravilnik = 0)
   AND substr(created_at, 1, 10) <= ?1
 GROUP BY substr(created_at, 1, 10)
"#,
    )?;

    let inflows = statement
        .query_map(params![as_of], |row| {
            Ok(CashInflow {
                date: row.get(0)?,
                amount_minor: row.get(1)?,
                subject: row.get::<_, i64>(2)? != 0,
            })
        })?
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;

    Ok(inflows)
}

/// Every polog onto the shop's own račun kod banke made **up to and including
/// `as_of`**, per calendar date.
///
/// The cut-off is what makes the retrospective question answerable. A polog
/// made after the presek did not exist on the presek date, so crediting it
/// would show the shop clean on a day its money was still in the drawer and
/// already past the rok — the false "clean" state §3 rule 10 forbids, in the
/// artefact §3 rule 17 designates for the knjigovođa and for a documentary
/// Poreska uprava check.
pub fn load_cash_deposits(
    connection: &Connection,
    as_of: &str,
) -> Result<Vec<CashDeposit>, AppError> {
    let mut statement = connection.prepare(
        "SELECT substr(created_at, 1, 10) AS day, SUM(amount_minor)
           FROM cash_movements
          WHERE movement_type = 'bank_deposit'
            AND substr(created_at, 1, 10) <= ?1
          GROUP BY day
          ORDER BY day",
    )?;

    let deposits = statement
        .query_map(params![as_of], |row| {
            Ok(CashDeposit {
                date: row.get(0)?,
                amount_minor: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, rusqlite::Error>>()?;

    Ok(deposits)
}

/// Ages every open bucket against `as_of` (`yyyy-MM-dd`).
///
/// `as_of` is the presek: only cash received on or before it, and only pologe
/// made on or before it, enter the report. Running it for a past date therefore
/// answers "was I late then?" rather than "am I late now?".
///
/// Admin-gated here rather than in the command wrapper, so the gate cannot be
/// bypassed by any other caller of the report. A bucket is late only once
/// `as_of` is strictly **past** its `due_on`: the polog is still lawful on the
/// deadline day itself.
pub fn cash_deposit_report(state: &AppState, as_of: &str) -> Result<CashDepositReport, AppError> {
    crate::commands::auth::require_admin(state)?;

    if parse_iso_date(as_of).is_none() {
        return Err(AppError::validation(
            "Datum preseka mora biti u obliku gggg-MM-dd.",
            serde_json::json!({ "asOf": as_of }),
        ));
    }

    let inflows;
    let deposits;
    {
        let connection = state.db().open()?;
        inflows = load_cash_inflows(&connection, as_of)?;
        deposits = load_cash_deposits(&connection, as_of)?;
    }

    let saturday_is_working = saturday_is_working_day(state)?;
    let config = BucketConfig {
        non_working_days: load_non_working_days(state)?,
        saturday_is_working,
    };

    let mut buckets = build_buckets(&inflows, &deposits, &config);
    for bucket in &mut buckets {
        bucket.is_overdue = bucket.outstanding_minor > 0
            && bucket
                .due_on
                .as_deref()
                .is_some_and(|due_on| due_on < as_of);
    }

    let outstanding_minor = buckets.iter().map(|bucket| bucket.outstanding_minor).sum();
    let overdue_minor = buckets
        .iter()
        .filter(|bucket| bucket.is_overdue)
        .map(|bucket| bucket.outstanding_minor)
        .sum();
    let excluded_float_minor = inflows
        .iter()
        .filter(|inflow| !inflow.subject)
        .map(|inflow| inflow.amount_minor)
        .sum();
    let beyond_seeded_calendar = buckets
        .iter()
        .filter_map(|bucket| bucket.due_on.as_deref())
        .filter_map(|due_on| due_on.get(0..4).and_then(|year| year.parse::<i32>().ok()))
        .any(|year| year > SEEDED_CALENDAR_HORIZON_YEAR);

    Ok(CashDepositReport {
        as_of: as_of.to_string(),
        buckets,
        outstanding_minor,
        overdue_minor,
        excluded_float_minor,
        saturday_is_working,
        calendar_horizon_year: SEEDED_CALENDAR_HORIZON_YEAR,
        beyond_seeded_calendar,
        notice: cash_deposit_duty(&load_shop_profile(state)?),
        footer: report_footer(beyond_seeded_calendar),
    })
}

/// The knjigovođa's copy (§3 rule 17). Amounts are in para, matching every
/// other CSV export in the app. The footer and the pravni osnov are appended
/// below a blank line so an export can never separate the labels from the
/// numbers.
pub fn report_to_csv(report: &CashDepositReport) -> String {
    let mut lines = vec![csv_line(&[
        "Datum prometa",
        "Primljeno",
        "Položeno",
        "Ostatak",
        "Rok",
        "Status",
    ])];

    for bucket in &report.buckets {
        // `due_on == None` means the deadline could not be computed at all, so
        // `is_overdue` is `false` for want of a date, not because the money is
        // inside a rok. Saying "U roku" there would be a positive affirmation
        // next to an empty Rok column — false reassurance in the knjigovođa's
        // own copy. The unknown must stay visible as an unknown.
        let status = if bucket.outstanding_minor == 0 {
            "Položeno"
        } else if bucket.due_on.is_none() {
            "Rok nije izračunat"
        } else if bucket.is_overdue {
            "Kasni"
        } else {
            "U roku"
        };
        lines.push(csv_line(&[
            bucket.trading_date.as_str(),
            &bucket.subject_minor.to_string(),
            &bucket.deposited_minor.to_string(),
            &bucket.outstanding_minor.to_string(),
            bucket.due_on.as_deref().unwrap_or(""),
            status,
        ]));
    }

    lines.push(String::new());
    lines.push(csv_line(&["Presek na dan", report.as_of.as_str()]));
    lines.push(csv_line(&["Napomena", report.footer.as_str()]));
    lines.push(csv_line(&["Pravni osnov", report.notice.citation.as_str()]));
    if let Some(penalty) = report.notice.penalty.as_deref() {
        lines.push(csv_line(&["Kazna", penalty]));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    use crate::commands::settings::ShopProfile;
    use crate::db::{test_database_path, Db};

    fn holidays(days: &[&str]) -> BTreeSet<String> {
        days.iter().map(|d| (*d).to_string()).collect()
    }

    fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            test(&state);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    fn admin_id(state: &AppState) -> i64 {
        state
            .db()
            .open()
            .expect("database should open")
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .expect("bootstrap admin should exist")
    }

    fn sign_in_admin(state: &AppState) {
        state
            .set_session_user_id(admin_id(state))
            .expect("admin session should set");
    }

    /// The report reads whole trading dates, so one long-lived shift is enough;
    /// `sales.shift_id` is `NOT NULL` and foreign keys are on.
    fn ensure_shift(connection: &Connection, user_id: i64) -> i64 {
        let existing: Option<i64> = connection
            .query_row("SELECT id FROM shifts ORDER BY id LIMIT 1", [], |row| {
                row.get(0)
            })
            .ok();
        if let Some(id) = existing {
            return id;
        }

        connection
            .execute(
                "INSERT INTO shifts (
                    user_id, opened_at, opening_cash_minor, expected_cash_minor,
                    status, created_at, updated_at
                 )
                 VALUES (?1, '2026-07-01T08:00:00Z', 0, 0, 'open',
                         '2026-07-01T08:00:00Z', '2026-07-01T08:00:00Z')",
                params![user_id],
            )
            .expect("shift should insert");
        connection.last_insert_rowid()
    }

    /// A completed cash sale received on `day` (`yyyy-MM-dd`), at midday so the
    /// stamp is unambiguously inside that trading date. Returns the sale id so
    /// a return can be linked to it.
    fn seed_cash_sale_on(state: &AppState, day: &str, amount_minor: i64) -> i64 {
        let connection = state.db().open().expect("database should open");
        let cashier_id = admin_id(state);
        let shift_id = ensure_shift(&connection, cashier_id);
        let created_at = format!("{day}T12:00:00Z");
        let sequence: i64 = connection
            .query_row("SELECT COUNT(*) FROM sales", [], |row| row.get(0))
            .expect("sales should count");

        connection
            .execute(
                "INSERT INTO sales (
                    local_receipt_number, shift_id, cashier_id, status,
                    subtotal_minor, tax_minor, total_minor, created_at, updated_at
                 )
                 VALUES (?1, ?2, ?3, 'completed', ?4, 0, ?4, ?5, ?5)",
                params![
                    format!("VP-{:06}", sequence + 1),
                    shift_id,
                    cashier_id,
                    amount_minor,
                    created_at
                ],
            )
            .expect("sale should insert");
        let sale_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, 'cash', ?2, ?3)",
                params![sale_id, amount_minor, created_at],
            )
            .expect("cash payment should insert");

        sale_id
    }

    /// A partial cash return on the same trading date, written exactly the way
    /// `receipts::return_items` writes one: a linked `return` document carrying
    /// the negative payment, and the original receipt flipped to `refunded`.
    fn seed_partial_cash_return_on(
        state: &AppState,
        original_sale_id: i64,
        day: &str,
        amount_minor: i64,
    ) {
        let connection = state.db().open().expect("database should open");
        let cashier_id = admin_id(state);
        let shift_id = ensure_shift(&connection, cashier_id);
        let created_at = format!("{day}T15:00:00Z");
        let sequence: i64 = connection
            .query_row("SELECT COUNT(*) FROM sales", [], |row| row.get(0))
            .expect("sales should count");

        connection
            .execute(
                "INSERT INTO sales (
                    local_receipt_number, shift_id, cashier_id, status, document_type,
                    original_sale_id, subtotal_minor, tax_minor, total_minor,
                    created_at, updated_at
                 )
                 VALUES (?1, ?2, ?3, 'refunded', 'return', ?4, ?5, 0, ?5, ?6, ?6)",
                params![
                    format!("POV-{:06}", sequence + 1),
                    shift_id,
                    cashier_id,
                    original_sale_id,
                    amount_minor,
                    created_at
                ],
            )
            .expect("return document should insert");
        let return_sale_id = connection.last_insert_rowid();

        connection
            .execute(
                "INSERT INTO sale_payments (sale_id, payment_method, amount_minor, created_at)
                 VALUES (?1, 'cash', ?2, ?3)",
                params![return_sale_id, -amount_minor, created_at],
            )
            .expect("refund payment should insert");

        connection
            .execute(
                "UPDATE sales SET status = 'refunded', updated_at = ?1 WHERE id = ?2",
                params![created_at, original_sale_id],
            )
            .expect("original receipt should flip to refunded");
    }

    /// A cash movement of any type on `day`. `amount_minor` is CHECK-constrained
    /// positive; the sign of a `pay_out` is applied by the query, not here.
    fn seed_cash_movement_on(state: &AppState, day: &str, kind: &str, amount_minor: i64) {
        let connection = state.db().open().expect("database should open");
        let user_id = admin_id(state);
        let shift_id = ensure_shift(&connection, user_id);
        let created_at = format!("{day}T16:00:00Z");

        connection
            .execute(
                "INSERT INTO cash_movements (
                    shift_id, movement_type, amount_minor, reason, user_id, created_at
                 )
                 VALUES (?1, ?2, ?3, NULL, ?4, ?5)",
                params![shift_id, kind, amount_minor, user_id, created_at],
            )
            .expect("cash movement should insert");
    }

    /// A `bank_withdrawal` on `day` carrying an explicit documentation state:
    /// `None` is „operator asserted nothing", which is the shipped default and
    /// must behave exactly like an explicit `Some(false)`.
    fn seed_bank_withdrawal_on(
        state: &AppState,
        day: &str,
        amount_minor: i64,
        documented: Option<bool>,
    ) {
        let connection = state.db().open().expect("database should open");
        let user_id = admin_id(state);
        let shift_id = ensure_shift(&connection, user_id);
        let created_at = format!("{day}T09:00:00Z");

        connection
            .execute(
                "INSERT INTO cash_movements (
                    shift_id, movement_type, amount_minor, reason,
                    documented_per_pravilnik, user_id, created_at
                 )
                 VALUES (?1, 'bank_withdrawal', ?2, NULL, ?3, ?4, ?5)",
                params![
                    shift_id,
                    amount_minor,
                    documented.map(i64::from),
                    user_id,
                    created_at
                ],
            )
            .expect("bank withdrawal should insert");
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
    /// per čl. 2 st. 2 or st. 3 is not "gotov novac" for this duty. Without the
    /// carve-out the app ages a documented change float as undeposited pazar and
    /// invents violations. `subject` is the loader's decision (see
    /// `load_cash_inflows`); this test only fixes what `build_buckets` does with
    /// it once made.
    #[test]
    fn a_not_subject_inflow_never_enters_the_subject_base() {
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

    #[test]
    fn report_flags_only_buckets_past_their_deadline() {
        with_state("cash_deposit_report", |state| {
            sign_in_admin(state);
            seed_cash_sale_on(state, "2026-07-20", 100_000);
            seed_cash_sale_on(state, "2026-07-30", 40_000);

            let report = cash_deposit_report(state, "2026-08-03").expect("report should run");

            let overdue: Vec<&DepositBucket> =
                report.buckets.iter().filter(|b| b.is_overdue).collect();
            assert_eq!(
                overdue.len(),
                1,
                "only the 20.07 bucket is past +7 radnih dana"
            );
            assert_eq!(overdue[0].trading_date, "2026-07-20");

            assert!(report.notice.is_legal_duty);
            assert!(
                report.footer.contains("Pravilnik"),
                "the float exclusion is bylaw-level relief and must be labelled"
            );
            assert!(
                report.footer.contains("čl. 2"),
                "the carve-out only reaches withdrawals made per Pravilnik čl. 2 st. 2 or st. 3, \
                 so the footer must carry the condition, not just the relief: {}",
                report.footer
            );
        });
    }

    /// `as_of` is a presek, not a caption. Cash the shop had not yet received on
    /// that date cannot be part of that date's obligation, so a trading date
    /// after the presek must not appear at all — a report headed "Presek na dan
    /// 25.07." that carries a 30.07. bucket is reading the future.
    #[test]
    fn the_presek_excludes_cash_received_after_it() {
        with_state("cash_deposit_as_of_future_take", |state| {
            sign_in_admin(state);
            seed_cash_sale_on(state, "2026-07-20", 100_000);
            seed_cash_sale_on(state, "2026-07-30", 40_000);

            let report = cash_deposit_report(state, "2026-07-25").expect("report should run");

            let dates: Vec<&str> = report
                .buckets
                .iter()
                .map(|bucket| bucket.trading_date.as_str())
                .collect();
            assert_eq!(
                dates,
                vec!["2026-07-20"],
                "30.07. is five days after the presek and cannot be in it"
            );
            assert_eq!(
                report.outstanding_minor, 100_000,
                "only cash received by 25.07. is outstanding on 25.07."
            );
        });
    }

    /// The retrospective question — "was I late on that date?" — is the whole
    /// point of the report (§3 rule 17, a documentary Poreska uprava check). A
    /// polog made weeks later must not reach back and discharge the bucket, or
    /// the presek shows the shop clean on a day it was three days past the rok:
    /// the false "clean" state §3 rule 10 forbids.
    #[test]
    fn a_polog_made_after_the_presek_does_not_clear_it() {
        with_state("cash_deposit_as_of_future_polog", |state| {
            sign_in_admin(state);
            seed_cash_sale_on(state, "2026-07-20", 100_000);
            seed_cash_movement_on(state, "2026-09-15", "bank_deposit", 100_000);

            let report = cash_deposit_report(state, "2026-07-31").expect("report should run");

            assert_eq!(report.buckets.len(), 1, "one trading date, one bucket");
            assert_eq!(
                report.buckets[0].deposited_minor, 0,
                "the polog is seven weeks after the presek"
            );
            assert_eq!(report.outstanding_minor, 100_000);
            assert_eq!(
                report.overdue_minor, 100_000,
                "the rok was 28.07., so on 31.07. the money was already late"
            );
            assert!(report.buckets[0].is_overdue);
        });
    }

    /// A partial return flips the **original** receipt to `refunded` while the
    /// linked document carries only the refunded part. Aging the base off
    /// `sales.status = 'completed'` would therefore drop the whole receipt and
    /// report a clean day, while 70.000 dinara sat undeposited in the drawer —
    /// the false "clean" state §3 rule 10 forbids.
    #[test]
    fn a_partial_return_reduces_the_base_instead_of_erasing_it() {
        with_state("cash_deposit_partial_return", |state| {
            sign_in_admin(state);
            let sale_id = seed_cash_sale_on(state, "2026-07-20", 100_000);
            seed_partial_cash_return_on(state, sale_id, "2026-07-20", 30_000);

            let report = cash_deposit_report(state, "2026-08-03").expect("report should run");

            assert_eq!(report.buckets.len(), 1, "one trading date, one bucket");
            assert_eq!(
                report.buckets[0].subject_minor, 70_000,
                "the shop kept 70.000 in cash; only the refunded 30.000 leaves the base"
            );
            assert_eq!(report.outstanding_minor, 70_000);
        });
    }

    /// §3 rule 14 — "do not exclude all bank withdrawals". Pravilnik 77/2011
    /// čl. 5 st. 2 reaches only dinars paid out per čl. 2 st. 2 (against the
    /// original documentation submitted to the bank na uvid i overu) or čl. 2
    /// st. 3 (the 150.000 RSD/day undocumented lane). A withdrawal on which the
    /// operator asserted nothing is **not** shown to be either, so it stays in
    /// the čl. 3 st. 1 base: excluding it would shrink the base and hand the
    /// owner the false „izmireno" state rule 10 forbids.
    #[test]
    fn an_undocumented_bank_withdrawal_stays_in_the_subject_base() {
        with_state("cash_deposit_undocumented_withdrawal", |state| {
            sign_in_admin(state);
            seed_cash_sale_on(state, "2026-07-20", 100_000);
            seed_bank_withdrawal_on(state, "2026-07-20", 50_000, None);

            let report = cash_deposit_report(state, "2026-07-21").expect("report should run");

            assert_eq!(
                report.outstanding_minor, 150_000,
                "an unasserted withdrawal is not shown to be čl. 2 st. 2/st. 3 money"
            );
            assert_eq!(
                report.excluded_float_minor, 0,
                "nothing was excluded, so the report must not claim a carve-out"
            );
        });
    }

    /// An explicit „ne" is the same answer as silence: the bylaw excludes only
    /// what was paid out per čl. 2 st. 2 or st. 3, and this was not.
    #[test]
    fn a_withdrawal_marked_undocumented_stays_in_the_subject_base() {
        with_state("cash_deposit_flagged_undocumented_withdrawal", |state| {
            sign_in_admin(state);
            seed_cash_sale_on(state, "2026-07-20", 100_000);
            seed_bank_withdrawal_on(state, "2026-07-20", 50_000, Some(false));

            let report = cash_deposit_report(state, "2026-07-21").expect("report should run");

            assert_eq!(report.outstanding_minor, 150_000);
            assert_eq!(report.excluded_float_minor, 0);
        });
    }

    /// The other half of rule 14: where the operator *did* assert the čl. 2
    /// st. 2 / st. 3 documentation, the bylaw relief applies and the float
    /// leaves the base — otherwise the app ages the owner's own change money as
    /// undeposited pazar.
    #[test]
    fn a_documented_bank_withdrawal_is_excluded_from_the_subject_base() {
        with_state("cash_deposit_documented_withdrawal", |state| {
            sign_in_admin(state);
            seed_cash_sale_on(state, "2026-07-20", 100_000);
            seed_bank_withdrawal_on(state, "2026-07-20", 50_000, Some(true));

            let report = cash_deposit_report(state, "2026-07-21").expect("report should run");

            assert_eq!(
                report.outstanding_minor, 100_000,
                "only the pazar is subject; the documented float is not"
            );
            assert_eq!(
                report.excluded_float_minor, 50_000,
                "the excluded amount stays visible instead of vanishing"
            );
        });
    }

    /// The compounding case. Excluding a withdrawal from the base while the
    /// re-deposit of that same cash still draws genuine sale buckets down
    /// under-reports **twice**: once when the float never enters the base, and
    /// again when it discharges someone else's bucket on the way back. With the
    /// withdrawal undocumented, the two legs must net to zero.
    #[test]
    fn a_withdraw_then_redeposit_cycle_does_not_under_report_twice() {
        with_state("cash_deposit_float_cycle", |state| {
            sign_in_admin(state);
            seed_cash_sale_on(state, "2026-07-20", 100_000);
            seed_bank_withdrawal_on(state, "2026-07-21", 50_000, None);
            seed_cash_movement_on(state, "2026-07-22", "bank_deposit", 50_000);

            let report = cash_deposit_report(state, "2026-07-23").expect("report should run");

            assert_eq!(
                report.outstanding_minor, 100_000,
                "the polog returned the float, so the whole pazar is still owed"
            );
            assert_eq!(report.excluded_float_minor, 0);
        });
    }

    /// The knjigovođa's copy must carry the labels, not just the numbers: an
    /// export that sheds the "konvencija" and "podzakonski akt" notices would
    /// present this application's aggregation as the statute's own.
    #[test]
    fn the_csv_carries_the_bucket_rows_and_the_honesty_labels() {
        with_state("cash_deposit_csv", |state| {
            sign_in_admin(state);
            seed_cash_sale_on(state, "2026-07-20", 100_000);

            let report = cash_deposit_report(state, "2026-08-03").expect("report should run");
            let csv = report_to_csv(&report);
            let mut lines = csv.lines();

            assert_eq!(
                lines.next(),
                Some("Datum prometa,Primljeno,Položeno,Ostatak,Rok,Status")
            );
            assert_eq!(
                lines.next(),
                Some("2026-07-20,100000,0,100000,2026-07-28,Kasni")
            );
            assert!(
                csv.contains("Pravilnik 77/2011"),
                "the bylaw-level relief must be named in the export: {csv}"
            );
            assert!(
                csv.contains("68/2015"),
                "the pravni osnov must travel with the export: {csv}"
            );
        });
    }

    /// A bucket whose trading date could not be parsed — a hand-edited or
    /// migrated `sales.created_at` — has no computed rok. `is_overdue` is
    /// `false` there because the deadline is *unknown*, not because it is in the
    /// future, so the export must not turn that silence into "U roku": an empty
    /// Rok column beside a positive affirmation is false reassurance in the one
    /// copy the knjigovođa reads.
    #[test]
    fn the_csv_never_says_u_roku_without_a_computed_rok() {
        let report = CashDepositReport {
            as_of: "2026-08-03".to_string(),
            buckets: vec![DepositBucket {
                trading_date: "not-a-date".to_string(),
                subject_minor: 100_000,
                deposited_minor: 0,
                outstanding_minor: 100_000,
                due_on: None,
                is_overdue: false,
            }],
            outstanding_minor: 100_000,
            overdue_minor: 0,
            excluded_float_minor: 0,
            saturday_is_working: true,
            calendar_horizon_year: SEEDED_CALENDAR_HORIZON_YEAR,
            beyond_seeded_calendar: false,
            notice: cash_deposit_duty(&ShopProfile::default()),
            footer: report_footer(false),
        };

        let csv = report_to_csv(&report);
        let row = csv.lines().nth(1).expect("one bucket row");

        assert!(
            !row.ends_with("U roku"),
            "a bucket with no computed rok must not be affirmed as inside one: {row}"
        );
        assert!(
            row.contains("Rok nije izračunat"),
            "the export must say the deadline is unknown: {row}"
        );
    }

    #[test]
    fn the_calendar_reports_the_seeded_days_and_the_conservative_saturday_default() {
        with_state("cash_deposit_calendar_reads", |state| {
            sign_in_admin(state);
            seed_default_non_working_days(state, "2026-07-31T00:00:00Z").expect("seed");

            let calendar = calendar(state).expect("calendar should read");

            assert!(
                calendar.saturday_is_working,
                "counting Saturdays yields the earlier deadline and is the default"
            );
            assert_eq!(calendar.horizon_year, SEEDED_CALENDAR_HORIZON_YEAR);
            assert!(
                calendar
                    .days
                    .iter()
                    .any(|entry| entry.day == "2026-01-07" && entry.label == "Božić"),
                "the seeded table must reach the operator"
            );
            let mut sorted = calendar.days.clone();
            sorted.sort_by(|left, right| left.day.cmp(&right.day));
            assert_eq!(
                calendar.days.iter().map(|d| &d.day).collect::<Vec<_>>(),
                sorted.iter().map(|d| &d.day).collect::<Vec<_>>(),
                "days are listed chronologically"
            );
        });
    }

    /// The table is `[PRUDENTIAL]` and annual, so the shop must be able to
    /// correct it — including removing a day it does not observe, which pulls
    /// the deadline *earlier* rather than later.
    #[test]
    fn an_admin_edits_the_calendar_and_the_edit_changes_the_deadline() {
        with_state("cash_deposit_calendar_edits", |state| {
            sign_in_admin(state);

            let after_add =
                save_non_working_day(state, "2026-08-05", "Slava radnje", "2026-07-31T00:00:00Z")
                    .expect("day should save");
            assert!(after_add
                .days
                .iter()
                .any(|entry| entry.day == "2026-08-05" && entry.label == "Slava radnje"));

            let with_holiday = load_non_working_days(state).expect("days should load");
            assert_eq!(
                add_working_days(
                    "2026-07-31",
                    DEPOSIT_WINDOW_WORKING_DAYS,
                    &with_holiday,
                    true
                ),
                Some("2026-08-10".to_string()),
                "the added non-working day pushes the deadline one day later"
            );

            let after_delete =
                delete_non_working_day(state, "2026-08-05").expect("day should delete");
            assert!(!after_delete
                .days
                .iter()
                .any(|entry| entry.day == "2026-08-05"));
        });
    }

    /// The Settings panel re-reads the calendar on every mount, and that read is
    /// what carries the seed. If the seed ran on each read, deleting a shipped
    /// holiday would be undone the next time the operator opened the tab — and
    /// silently in the *unsafe* direction, because a restored non-working day
    /// pushes the čl. 3 st. 1 rok later, so the report would say "U roku" for
    /// cash the shop's own calendar says is already late.
    #[test]
    fn a_deleted_default_holiday_is_not_resurrected_by_reopening_the_calendar() {
        with_state("cash_deposit_calendar_delete_sticks", |state| {
            sign_in_admin(state);

            let first = calendar_with_seeded_defaults(state, "2026-07-31T00:00:00Z")
                .expect("calendar should read");
            assert!(
                first.days.iter().any(|entry| entry.day == "2026-04-11"),
                "the shipped table must arrive on the first read"
            );

            delete_non_working_day(state, "2026-04-11").expect("day should delete");

            // The mount effect the Settings panel runs on the next visit.
            let reopened = calendar_with_seeded_defaults(state, "2026-08-01T00:00:00Z")
                .expect("calendar should read");
            assert!(
                !reopened.days.iter().any(|entry| entry.day == "2026-04-11"),
                "a deleted non-working day must stay deleted across re-reads"
            );

            // …and the arithmetic must agree, not just the list. Friday 03.04
            // plus seven radni dana lands on 15.04 with Velika subota listed and
            // on 14.04 without it — the earlier, conservative deadline.
            let days = load_non_working_days(state).expect("days should load");
            assert_eq!(
                add_working_days("2026-04-03", DEPOSIT_WINDOW_WORKING_DAYS, &days, true),
                Some("2026-04-14".to_string()),
                "the deleted day must pull the deadline earlier and keep it there"
            );
        });
    }

    /// A fresh install must not compute the report against an empty holiday
    /// table until somebody happens to open Podešavanja — that would move every
    /// deadline later the moment a settings page was viewed.
    #[test]
    fn the_report_sees_the_same_holiday_table_as_the_settings_calendar() {
        with_state("cash_deposit_report_seeds_too", |state| {
            sign_in_admin(state);
            seed_cash_sale_on(state, "2026-04-03", 100_000);

            let report = report_with_seeded_defaults(state, "2026-04-20", "2026-04-20T12:00:00Z")
                .expect("report should build");

            assert_eq!(
                report.buckets[0].due_on.as_deref(),
                Some("2026-04-15"),
                "the report must count against the shipped Vaskršnji span, not an empty table"
            );
            assert!(
                calendar(state)
                    .expect("calendar should read")
                    .days
                    .iter()
                    .any(|entry| entry.day == "2026-04-11"),
                "the report and the Settings calendar must see one table"
            );
        });
    }

    #[test]
    fn the_saturday_assumption_is_settable_and_read_back() {
        with_state("cash_deposit_calendar_saturday", |state| {
            sign_in_admin(state);

            let updated = set_saturday_is_working(state, false).expect("flag should save");

            assert!(!updated.saturday_is_working);
            assert!(
                !calendar(state)
                    .expect("calendar should read")
                    .saturday_is_working
            );
        });
    }

    #[test]
    fn the_calendar_refuses_a_day_that_is_not_an_iso_date_or_carries_no_label() {
        with_state("cash_deposit_calendar_validates", |state| {
            sign_in_admin(state);

            assert!(
                save_non_working_day(state, "05.08.2026.", "Slava", "2026-07-31T00:00:00Z")
                    .is_err(),
                "a rendered date must never be stored as a key the arithmetic reads"
            );
            assert!(
                save_non_working_day(state, "2026-08-05", "   ", "2026-07-31T00:00:00Z").is_err(),
                "an unlabelled day cannot be checked by the operator later"
            );
        });
    }

    /// The calendar decides a statutory deadline, so reading and writing it are
    /// both admin-gated inside these functions — not in the command wrappers,
    /// which any other caller could bypass.
    #[test]
    fn the_calendar_is_closed_to_a_caller_without_an_admin_session() {
        with_state("cash_deposit_calendar_admin_only", |state| {
            assert!(calendar(state).is_err());
            assert!(set_saturday_is_working(state, false).is_err());
            assert!(
                save_non_working_day(state, "2026-08-05", "Slava", "2026-07-31T00:00:00Z").is_err()
            );
            assert!(delete_non_working_day(state, "2026-08-05").is_err());

            // The seeding entry points gate *before* they write, so a caller
            // without an admin session cannot provoke the seed either.
            assert!(calendar_with_seeded_defaults(state, "2026-07-31T00:00:00Z").is_err());
            assert!(
                report_with_seeded_defaults(state, "2026-07-31", "2026-07-31T00:00:00Z").is_err()
            );
            assert!(
                load_non_working_days(state)
                    .expect("days should load")
                    .is_empty(),
                "a refused call must not have written the holiday table"
            );
        });
    }
}
