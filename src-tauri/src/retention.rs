//! Retention classes — the single shared table SW11-SW15 §3 req. 42 mandates.
//!
//! Two classes, two clocks, one table (SW-14 §4d, req. 18–20). Independent
//! notions of *"isteklo"* are what guarantee that one path deletes what another
//! is obliged to keep, so SW-3 (go-live reset), SW-13 (ZZPL purge) and this
//! module all read the same `retention_policies` rows.
//!
//! **Class A — `worktime_classification`.** The derived, period-closed hour
//! classification per employee per month. `trajno`, ZEOR čl. 7 st. 2 and čl. 25
//! st. 3, because čl. 24 tač. 1 ž) rides the overtime hours inside the wage
//! record. `never_purge = 1`, and no date ever reaches it.
//!
//! **Class B — `worktime_draft`.** Working versions of an entry and any advisory
//! clock data. Bounded: purge-eligible only once the period is closed and the
//! classification derived, per ZZPL čl. 5 st. 1 tač. 5. v17 stores no punch
//! stream at all — the register is the čl. 55 st. 6 daily record itself — so
//! this class is declared and presently binds nothing; it exists so that the day
//! something raw *is* stored, it has a class waiting rather than inheriting
//! Class A's `trajno` by accident.
//!
//! **`worktime_overtime_log`.** The standalone čl. 55 st. 6 overtime register
//! considered apart from the wage record it feeds. No statute sets a period;
//! the defensive floor is three years and moves only forward. Like Class B it
//! takes two clocks — the shared row, and the recorded day's own three years,
//! because that floor binds each row rather than the class.
//!
//! **`personnel`** (SW-13 class A). The ZEOR čl. 5 evidencija o zaposlenim
//! licima. `trajno` on čl. 7 st. 2, `never_purge = 1`, and req. 26 gives the
//! category **no configurable period at all** — `extend_retain_until` refuses
//! it, and `commands::personnel::PurgeableClass` has no variant that names it.
//!
//! **`credentials`** (SW-13 class C). The PIN and password hashes. Discarded at
//! the termination itself and not after a window (req. 21), so the class row is
//! here to let a legal hold stop the sweep — not to time it.
//!
//! **`access_log`** (SW-13 class C). The čl. 48 evidencija pristupa, on
//! [`ACCESS_LOG_RETENTION_YEARS`] and never `trajno` (req. 6, 22). Two clocks
//! again: the class row and the row's own day, the second applied through
//! [`expiry_cutoff`] where the cut is made.
//!
//! **The direction of the trade-off, once.** Under-retention outranks
//! over-retention for this shop: the ZEOR čl. 50 st. 1 tač. 3 offence is *"ako
//! ne čuva trajno"*. Every decision here therefore fails safe toward KEEPING —
//! an unreadable date, a missing floor and an absent policy row all refuse the
//! purge rather than allow it.
//!
//! The purge job that reads these rows is `commands::personnel`; this module
//! ships the table, the classes, the date arithmetic and the transaction fence,
//! and some of it (the worktime gates) still has no non-test caller.

#![allow(dead_code)]

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use time::{Date, Month};

use crate::app_error::AppError;
use crate::cash_deposit::parse_iso_date;
use crate::state::AppState;

/// `[LEGAL-INFERRED]` §4d / req. 20. The standalone overtime register has **no**
/// statutory period. The defensive floor is `max(ZP čl. 84 apsolutna zastarelost
/// 2 g, ZoR čl. 196 3 g)` = three years, and it moves only upward.
///
/// The circulating „2 godine“ is a Serbian blog error traced to a single site and
/// „6 godina“ is Croatian law; neither may ever be surfaced, and neither is a
/// figure this constant may be lowered to.
pub const STANDALONE_OVERTIME_LOG_FLOOR_YEARS: i32 = 3;

/// SW-13 req. 22 + SW-10 req. 6. How long a line in the access evidencija is
/// kept before the time-driven purge may discard it.
///
/// **Two requirements meet on this number and the higher one wins.** Req. 22
/// calls a one-year default defensible on ZoP čl. 84 st. 1 (relativna
/// zastarelost) and names three years (ZoR čl. 196) as the defensible outer
/// bound. Req. 6 is stricter: the floor must sit above the whole ZoP čl. 84
/// window, whose *apsolutna* leg is two years — a log discarded at one year
/// cannot serve the čl. 48 st. 3 purpose *ocena zakonitosti obrade* during the
/// second year of a prekršaj proceeding that is still running.
///
/// Two years, therefore, and not three: ZZPL čl. 5 st. 1 t. 5 makes the shortest
/// defensible period the right *default*, and the shop can push the class floor
/// forward (never back) if it needs longer. **Never `trajno`** — čl. 47 st. 7
/// governs the register of processing activities, and copying it onto this log
/// would put the product in permanent breach of storage limitation (req. 6).
pub const ACCESS_LOG_RETENTION_YEARS: i32 = 2;

/// The tables no purge, reset, restore or backup-prune path may ever reduce
/// (§4d: *"the trajno classes must be structurally unreachable"*).
///
/// **Where that is actually enforced, and where it is not.**
/// [`never_purge_row_counts`] + [`assert_never_purge_intact`] enforce it wherever
/// the destructive step is a transaction that can be rolled back: today that is
/// `commands::backup::reset_trading_data`, and tomorrow SW-13's purge job.
/// `restore_backup` is outside their reach — it replaces the database file, so
/// the register comes back as the snapshot holds it, and the pre-restore safety
/// copy is the protection on that path (the reasoning, including why a
/// count-based refusal there would trap the shop, is on
/// `commands::backup::never_purge_counts`). No backup-prune path exists in this
/// crate at all; when one is built it is a transaction-shaped path and it must
/// carry the fence.
///
/// `work_time_periods` holds the frozen Class A classification and
/// `work_time_entries` is the čl. 55 st. 6 register it is derived from — an
/// inspector reads the second, and ZEOR čl. 24 tač. 1 ž) carries its overtime
/// hours into the wage record that is `trajno`. `retention_policies` is on the
/// list for a different reason: a wipe that took the classes with it would leave
/// the next purge path with no policy at all, which is exactly the state req. 42
/// exists to prevent.
///
/// **The granularity is deliberate.** [`never_purge_row_counts`] compares whole
/// tables, so the superseded `verzija > 1` links of the v17 correction chain are
/// fenced too — even though §4d files edit trails beyond defence needs under
/// ZZPL čl. 5 st. 1 tač. 5 as bounded-and-purged. That is the §4d trade-off taken
/// on purpose (under-retention outranks over-retention), not an oversight. When
/// SW-13 comes to discard superseded versions it must **narrow what is counted**
/// — to the live `MAX(verzija)` rows, or to the trajno subset — and never
/// weaken or remove the fence to get there.
///
/// `personnel_records` joins the list with SW-13 (req. 19, 24): the ZEOR čl. 5
/// evidencija is class A, it is kept `trajno` under čl. 7 st. 2, and no purge,
/// reset or deactivation may reduce it. v18's `BEFORE DELETE` trigger already
/// refuses a row-level delete; this entry is the transaction-level half, so a
/// future edit that finds a way around the trigger still aborts the whole
/// transaction instead of committing a shortened register.
///
/// `processing_activities` joins it with the čl. 47 evidencija radnji obrade
/// (req. 28). St. 7 says the record is kept `trajno` in as many words, and
/// unlike `personnel_records` this table carries no `BEFORE DELETE` trigger —
/// it is regenerated by [`crate::cl47`] on every launch, and a trigger would
/// refuse the upsert that keeps it current. The fence is therefore the only
/// structural protection it has.
pub const NEVER_PURGE_TABLES: &[&str] = &[
    "work_time_entries",
    "work_time_periods",
    "retention_policies",
    "personnel_records",
    "processing_activities",
];

/// A record class as the shared table stores it.
///
/// The variants are the `record_class` values, not a free vocabulary: [`key`]
/// is the stored string and [`from_key`] is its only inverse, so a row written
/// by one feature is read by every other as the same class.
///
/// [`key`]: RecordClass::key
/// [`from_key`]: RecordClass::from_key
// The three working-time variants share a prefix because a bare
// `RecordClass::Classification` would say nothing about which record it
// classifies; this is the shared table SW11-SW15 §3 req. 42 mandates and every
// feature adds its classes to it. SW-13's three follow below.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordClass {
    /// Class A — the derived, period-closed monthly classification. `trajno`.
    WorktimeClassification,
    /// The standalone čl. 55 st. 6 overtime register, three-year floor.
    WorktimeOvertimeLog,
    /// Class B — entry drafts and advisory clock data, bounded by the close.
    WorktimeDraft,
    /// SW-13 class A — the ZEOR čl. 5 evidencija o zaposlenim licima. `trajno`
    /// under čl. 7 st. 2, and req. 26 gives it **no configurable period at all**:
    /// `retain_until` stays `NULL` and [`extend_retain_until`] refuses it.
    Personnel,
    /// SW-13 class C, the credential half — the PIN and password hashes.
    /// Discarded **at termination**, not after a window (req. 21).
    Credentials,
    /// SW-13 class C, the access half — the čl. 48 evidencija pristupa.
    /// [`ACCESS_LOG_RETENTION_YEARS`], never `trajno` (req. 6, 22). This app
    /// keeps no separate login history: `users.last_login_at` is one current
    /// value overwritten on each sign-in, not a stream a purge could shorten.
    AccessLog,
    /// The čl. 47 evidencija radnji obrade (req. 28). **The one class in the
    /// ZZPL trio where `trajno` is the right answer**, because st. 7 says so of
    /// this record and of no other — which is exactly why the audit log and the
    /// breach log must not borrow it.
    ProcessingRegister,
}

impl RecordClass {
    pub const ALL: [Self; 7] = [
        Self::WorktimeClassification,
        Self::WorktimeOvertimeLog,
        Self::WorktimeDraft,
        Self::Personnel,
        Self::Credentials,
        Self::AccessLog,
        Self::ProcessingRegister,
    ];

    /// The `record_class` value stored in `retention_policies`.
    pub fn key(self) -> &'static str {
        match self {
            Self::WorktimeClassification => "worktime_classification",
            Self::WorktimeOvertimeLog => "worktime_overtime_log",
            Self::WorktimeDraft => "worktime_draft",
            Self::Personnel => "personnel",
            Self::Credentials => "credentials",
            Self::AccessLog => "access_log",
            Self::ProcessingRegister => "processing_register",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|class| class.key() == key)
    }

    /// ZEOR čl. 7 st. 2 / čl. 25 st. 3 — `trajno`. A class that answers `true`
    /// here is unreachable by every purge, whatever date anything else stores.
    ///
    /// [`Self::Personnel`] answers `true` on the same article: čl. 7 st. 2 is
    /// the retention rule for the evidencija o zaposlenim licima itself, and
    /// req. 26 forbids a configurable period for the category outright.
    ///
    /// [`Self::ProcessingRegister`] answers `true` on a different statute —
    /// ZZPL čl. 47 st. 7, which is the only ZZPL provision in this trio that
    /// prescribes `trajno`, and prescribes it for the register alone.
    pub fn never_purge(self) -> bool {
        matches!(
            self,
            Self::WorktimeClassification | Self::Personnel | Self::ProcessingRegister
        )
    }

    /// The floor the class is seeded with.
    ///
    /// Class A gets `NULL`: `trajno` is the absence of an end date, and
    /// `never_purge` is what carries it. Class B gets the seeding day rather
    /// than `NULL` — it has no calendar floor of its own (the period close is
    /// its gate), and giving it an explicit past date keeps `NULL` meaning one
    /// single thing everywhere in this table: *nema roka, ne briši*.
    ///
    /// The overtime log's floor is three years from the **seeding day** — the day
    /// the shop first launched, which is no record's date. It is the class-wide
    /// earliest-possible day and is necessary but never sufficient: req. 20's
    /// floor is a per-record obligation, and [`overtime_log_purge_eligible`] is
    /// the gate that applies `retention_floor(record_day, …)` to the row's own
    /// `dan`. SW-13 must call that, not this stored value alone.
    /// SW-13's three follow the same two shapes. `Personnel` gets `NULL` for the
    /// same reason Class A does, and req. 26 additionally bars any date from ever
    /// being written there. `Credentials` gets the seeding day, because the thing
    /// that discards a credential is the *termination event* and not a calendar
    /// window (req. 21) — the class row exists so a legal hold can still stop the
    /// sweep, not to time it. `AccessLog` gets the seeding day plus
    /// [`ACCESS_LOG_RETENTION_YEARS`], and — exactly like the overtime log — that
    /// class-wide floor is necessary but never sufficient: the per-row gate is
    /// [`expiry_cutoff`] applied to each row's own day by
    /// `commands::personnel::expire_access_log`, which is where the cut is made.
    fn seed_retain_until(self, now: &str) -> Option<String> {
        match self {
            Self::WorktimeClassification | Self::Personnel | Self::ProcessingRegister => None,
            Self::WorktimeOvertimeLog => retention_floor(now, STANDALONE_OVERTIME_LOG_FLOOR_YEARS),
            Self::WorktimeDraft | Self::Credentials => parse_iso_date(date_only(now)).map(iso_date),
            Self::AccessLog => retention_floor(now, ACCESS_LOG_RETENTION_YEARS),
        }
    }

    /// Operator-facing note stored beside the row. No fine figure appears here
    /// or anywhere outside `legal.rs`, and no ZEOR figure appears at all.
    ///
    /// `pub(crate)` so `docs_guard` can read these the way it reads the
    /// documents: they are the same kind of claim, and one of them promised a
    /// deletion-immunity the restore path never had.
    pub(crate) fn napomena(self) -> &'static str {
        match self {
            Self::WorktimeClassification => {
                "Izvedena mesečna klasifikacija časova. Čuva se trajno (ZEOR čl. 7 st. 2 i čl. 25 st. 3). Aplikacija je izuzima iz brisanja i iz resetovanja podataka. Vraćanje iz rezervne kopije vraća celu bazu na stanje iz te kopije, pa i ovu evidenciju — zaštita na tom putu je rezervna kopija zatečenog stanja koju aplikacija napravi pre vraćanja, uz zapis o tome koliko je zapisa vraćanje pomerilo. Automatsko čišćenje starih rezervnih kopija ne postoji."
            }
            Self::WorktimeOvertimeLog => {
                "Samostalna evidencija prekovremenog rada (ZoR čl. 55 st. 6). Zakon ne propisuje rok čuvanja; primenjuje se odbrambeni minimum od tri godine, koji se pomera samo unapred."
            }
            Self::WorktimeDraft => {
                "Radne verzije unosa i pomoćni podaci o vremenu. Brišu se tek pošto je period zatvoren i klasifikacija izvedena (ZZPL čl. 5 st. 1 tač. 5)."
            }
            Self::Personnel => {
                "Evidencija o zaposlenim licima (ZEOR čl. 5). Čuva se trajno (ZEOR čl. 7 st. 2) i rok se ne podešava. Aplikacija je izuzima iz automatskog čišćenja, iz deaktivacije naloga i iz resetovanja podataka. Vraćanje iz rezervne kopije vraća celu bazu na stanje iz te kopije, pa i ovu evidenciju — zaštita na tom putu je rezervna kopija zatečenog stanja koju aplikacija napravi pre vraćanja. Automatsko čišćenje starih rezervnih kopija ne postoji."
            }
            Self::Credentials => {
                "PIN i lozinka (samo heš vrednosti). Uklanjaju se danom prestanka radnog odnosa, a ne po isteku roka (ZZPL čl. 5 st. 1 tač. 5 i čl. 42 st. 2). Automatsko čišćenje uklanja heš sa svakog deaktiviranog naloga; sam nalog i evidencija o zaposlenom ostaju."
            }
            Self::AccessLog => {
                "Evidencija pristupa podacima o ličnosti, koju rukovalac vodi kao sopstvenu meru: ZZPL čl. 48 obavezuje nadležni organ koji podatke obrađuje u posebne svrhe, pa je ovde uzor za sadržaj, a ne osnov obaveze. Aplikacija ne vodi zasebnu istoriju prijavljivanja — beleži se samo poslednja prijava na nalogu, koja se prepisuje pri svakoj sledećoj. Zakon ne propisuje rok; primenjuje se podrazumevani rok od dve godine, koji nadživljava ceo rok zastarelosti iz ZoP čl. 84. Odbranjiva gornja granica je tri godine (ZoR čl. 196). Rok se pomera samo unapred. Ova evidencija se ne čuva trajno."
            }
            Self::ProcessingRegister => {
                "Evidencija o radnjama obrade (ZZPL čl. 47 st. 1). Čuva se trajno (čl. 47 st. 7) i rok se ne podešava; na zahtev se stavlja na uvid Povereniku (čl. 47 st. 8). Aplikacija je izuzima iz automatskog čišćenja i iz resetovanja podataka. Vraćanje iz rezervne kopije vraća celu bazu na stanje iz te kopije, pa i ovu evidenciju — zaštita na tom putu je rezervna kopija zatečenog stanja koju aplikacija napravi pre vraćanja. Automatsko čišćenje starih rezervnih kopija ne postoji. Evidencija se iznova generiše iz podešavanja programa i iz tabele rokova čuvanja, pa ne može da navede rok koji program ne primenjuje."
            }
        }
    }
}

/// One stored policy row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPolicy {
    pub record_class: RecordClass,
    /// The earliest `gggg-MM-dd` on which the class may be discarded — the same
    /// sense `kep_close::retention_floor` already uses. `None` means no floor is
    /// recorded, and nothing may be discarded.
    pub retain_until: Option<String>,
    pub legal_hold: bool,
    pub never_purge: bool,
}

/// Writes the classes this feature owns, once. Idempotent, and it never
/// overwrites a stored floor — retention moves only forward, so a second run
/// after the shop has already extended a class must not pull it back to the
/// seeded value.
///
/// `never_purge` is the one **decision** the seed repairs, and only upward: a
/// row that somehow reached the database with `never_purge = 0` on a `trajno`
/// class is a row that would let a purge through.
///
/// [`RecordClass::napomena`] is rewritten unconditionally, which is a different
/// kind of repair: the note is derived text this crate authors and nothing else
/// ever writes, and it is the sentence a person reads when they ask why a record
/// is still there. SW-14 shipped one that promised a deletion-immunity the
/// restore path never had, and a correction that only lands in new databases
/// leaves the false claim standing in every shop already running. The seed runs
/// at each launch and after each restore, so it is the propagation path.
/// `updated_at` moves only when one of those two actually changed — the stamp
/// records when the row moved, not when the app last started.
pub fn seed_retention_policies(state: &AppState, now: &str) -> Result<(), AppError> {
    let connection = state.db().open()?;

    for class in RecordClass::ALL {
        connection.execute(
            "INSERT INTO retention_policies
                 (record_class, retain_until, legal_hold, never_purge, napomena, created_at, updated_at)
             VALUES (?1, ?2, 0, ?3, ?4, ?5, ?5)
             ON CONFLICT(record_class) DO UPDATE SET
                 never_purge = MAX(retention_policies.never_purge, excluded.never_purge),
                 napomena = excluded.napomena,
                 updated_at = CASE
                     WHEN retention_policies.never_purge < excluded.never_purge
                       OR retention_policies.napomena IS NOT excluded.napomena
                     THEN excluded.updated_at
                     ELSE retention_policies.updated_at
                 END",
            params![
                class.key(),
                class.seed_retain_until(now),
                i64::from(class.never_purge()),
                class.napomena(),
                now
            ],
        )?;
    }

    Ok(())
}

/// Loads one class. A missing row is an error, never a default: a purge that
/// cannot find its policy must stop, not invent one.
pub fn load_policy(conn: &Connection, class: RecordClass) -> Result<RetentionPolicy, AppError> {
    conn.query_row(
        "SELECT retain_until, legal_hold, never_purge
         FROM retention_policies
         WHERE record_class = ?1",
        params![class.key()],
        |row| {
            Ok(RetentionPolicy {
                record_class: class,
                retain_until: row.get(0)?,
                legal_hold: row.get::<_, i64>(1)? != 0,
                never_purge: row.get::<_, i64>(2)? != 0,
            })
        },
    )
    .optional()?
    .ok_or_else(|| {
        AppError::not_found(format!(
            "Klasa čuvanja „{}“ nije upisana u tabelu rokova.",
            class.key()
        ))
    })
}

/// May this class be discarded on `today`?
///
/// Three gates, and each one alone is enough to refuse:
/// `never_purge` (ZEOR `trajno`), `legal_hold`, and a floor the day has not
/// reached. A missing floor refuses too — see the module note on the direction
/// of the trade-off. Dates compare lexicographically, which is exact on
/// `gggg-MM-dd`, and `today` may be a full RFC3339 stamp.
///
/// This answers for the **class**, never for a row, and it is necessary but not
/// sufficient wherever the obligation is anchored to the record itself:
/// [`draft_purge_eligible`] adds the period close for Class B, and
/// [`overtime_log_purge_eligible`] adds the record's own three-year floor for the
/// overtime register. A purge job must call one of those — this alone would
/// release every row in a class the moment the class row's date passed.
pub fn is_purgeable(policy: &RetentionPolicy, today: &str) -> bool {
    if policy.never_purge || policy.legal_hold {
        return false;
    }

    match policy.retain_until.as_deref() {
        Some(floor) => date_only(today) >= floor,
        None => false,
    }
}

/// Moves a class's floor forward. An earlier date is rejected, never clamped:
/// something asking to shorten a retention period is a defect, and a silent
/// `max()` would hide it until an inspector asked for the records.
///
/// A `trajno` class is refused outright — its `NULL` is not a floor waiting to
/// be lengthened, it is the absence of an end, and every concrete date is
/// shorter than that.
pub fn extend_retain_until(
    conn: &Connection,
    class: RecordClass,
    retain_until: &str,
    now: &str,
) -> Result<RetentionPolicy, AppError> {
    let Some(candidate) = parse_iso_date(retain_until).map(iso_date) else {
        return Err(AppError::validation(
            "Rok čuvanja mora biti u obliku gggg-MM-dd.",
            serde_json::json!({ "recordClass": class.key(), "retainUntil": retain_until }),
        ));
    };

    let current = load_policy(conn, class)?;
    let Some(stored) = current.retain_until.clone() else {
        return Err(AppError::validation(
            format!(
                "Klasa „{}“ se čuva bez roka; upisivanje datuma bi skratilo čuvanje.",
                class.key()
            ),
            serde_json::json!({ "recordClass": class.key(), "retainUntil": candidate }),
        ));
    };

    if candidate < stored {
        return Err(AppError::validation(
            format!(
                "Rok čuvanja se pomera samo unapred: „{stored}“ se ne skraćuje na „{candidate}“."
            ),
            serde_json::json!({
                "recordClass": class.key(),
                "retainUntil": candidate,
                "storedRetainUntil": stored,
            }),
        ));
    }

    conn.execute(
        "UPDATE retention_policies SET retain_until = ?2, updated_at = ?3
         WHERE record_class = ?1",
        params![class.key(), candidate, now],
    )?;

    load_policy(conn, class)
}

/// Class B for one employee-month: bounded by **two** clocks, and both must
/// agree (§4d, req. 19).
///
/// The shared policy row is one — a legal hold or an unreached floor stops the
/// purge like it stops any other. The period close is the other, and it is the
/// one that matters: without a real state transition there is no defensible
/// moment at which raw data stopped being necessary, and ZZPL čl. 5 st. 1 tač. 5
/// has nothing to anchor to. A month with no period row at all has derived
/// nothing yet, so it is not eligible either.
pub fn draft_purge_eligible(
    conn: &Connection,
    user_id: i64,
    godina: i64,
    mesec: i64,
    today: &str,
) -> Result<bool, AppError> {
    let policy = load_policy(conn, RecordClass::WorktimeDraft)?;
    if !is_purgeable(&policy, today) {
        return Ok(false);
    }

    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM work_time_periods
             WHERE user_id = ?1 AND godina = ?2 AND mesec = ?3",
            params![user_id, godina, mesec],
            |row| row.get(0),
        )
        .optional()?;

    Ok(status.as_deref() == Some("closed"))
}

/// The standalone čl. 55 st. 6 overtime register for **one recorded day**:
/// two clocks again, and both must agree (§4d, req. 20).
///
/// The shared policy row is one — a legal hold or an unreached class floor stops
/// this purge like it stops any other. The record's own `dan` is the second, and
/// it is the one that binds in the long run: req. 20's three years are a
/// *per-record* obligation, so a day entered yesterday is not discardable merely
/// because the class row was seeded three years ago. Without this gate the class
/// floor would answer `true` for the whole class forever from its anniversary
/// onward, which inverts the one axis §4d says must never be inverted. The
/// repo's own precedent is `kep_close::purge_eligible`, which anchors to the
/// book's `closed_at` rather than to any global clock.
///
/// An unreadable `record_day` yields no floor, and no floor means keep.
pub fn overtime_log_purge_eligible(
    conn: &Connection,
    record_day: &str,
    today: &str,
) -> Result<bool, AppError> {
    let policy = load_policy(conn, RecordClass::WorktimeOvertimeLog)?;
    if !is_purgeable(&policy, today) {
        return Ok(false);
    }

    let Some(floor) = retention_floor(record_day, STANDALONE_OVERTIME_LOG_FLOOR_YEARS) else {
        return Ok(false);
    };

    Ok(date_only(today) >= floor.as_str())
}

/// Row counts of [`NEVER_PURGE_TABLES`], taken **before** a destructive
/// operation and handed back to [`assert_never_purge_intact`] before it commits.
///
/// This pair is what makes „structurally unreachable“ structural rather than a
/// comment: a future edit that adds a `DELETE` against one of those tables
/// aborts the whole transaction instead of committing it.
///
/// It is a **transaction** fence and only that. A path that replaces the
/// database file wholesale leaves it nothing to compare — see `restore_backup`,
/// which records the counts on both sides instead of asserting over them.
pub fn never_purge_row_counts(conn: &Connection) -> Result<Vec<(&'static str, i64)>, AppError> {
    NEVER_PURGE_TABLES
        .iter()
        .map(|table| {
            let count: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })?;
            Ok((*table, count))
        })
        .collect()
}

/// Fails if any never-purge table lost rows since [`never_purge_row_counts`].
pub fn assert_never_purge_intact(
    conn: &Connection,
    before: &[(&'static str, i64)],
) -> Result<(), AppError> {
    for (table, before_count) in before {
        let after: i64 = conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })?;
        if after < *before_count {
            return Err(AppError::InvalidState(format!(
                "Zapisi koji se čuvaju trajno ne smeju biti obrisani: tabela {table} je pala sa {before_count} na {after} redova."
            )));
        }
    }

    Ok(())
}

/// `2026-08-01T08:00:00Z` → `2026-08-01`. A bare date passes through unchanged.
fn date_only(stamp: &str) -> &str {
    stamp.split('T').next().unwrap_or(stamp)
}

fn iso_date(date: Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// `day` shifted forward by `years`, as `gggg-MM-dd`. `None` when `day` is not a
/// readable calendar day — the caller then has no floor, and no floor means keep.
///
/// A 29 February has no anniversary in a common year and falls to 1 March, the
/// later day. That is the only direction a retention floor may ever move.
pub fn retention_floor(day: &str, years: i32) -> Option<String> {
    let date = parse_iso_date(date_only(day))?;
    let godina = date.year().checked_add(years)?;

    Date::from_calendar_date(godina, date.month(), date.day())
        .or_else(|_| Date::from_calendar_date(godina, Month::March, 1))
        .ok()
        .map(iso_date)
}

/// `day` shifted **backwards** by `years`, as `gggg-MM-dd` — the oldest day a
/// bounded class may still keep. A record whose own day is strictly earlier than
/// the cutoff has outlived its period.
///
/// This is [`retention_floor`] read from the other end, and the leap-day
/// fallback therefore goes the other way: a 29 February cutoff falls back to
/// **28 February**, the EARLIER day, so the boundary moves toward keeping one
/// more day rather than discarding one day early. That is the same direction the
/// module note fixes for every decision here.
///
/// `None` when `day` is not a readable calendar day — the caller then has no
/// cutoff, and no cutoff means keep.
pub fn expiry_cutoff(day: &str, years: i32) -> Option<String> {
    let date = parse_iso_date(date_only(day))?;
    let godina = date.year().checked_sub(years)?;

    Date::from_calendar_date(godina, date.month(), date.day())
        .or_else(|_| Date::from_calendar_date(godina, Month::February, 28))
        .ok()
        .map(iso_date)
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::{
        draft_purge_eligible, expiry_cutoff, extend_retain_until, is_purgeable, load_policy,
        overtime_log_purge_eligible, retention_floor, seed_retention_policies, RecordClass,
        RetentionPolicy, ACCESS_LOG_RETENTION_YEARS, STANDALONE_OVERTIME_LOG_FLOOR_YEARS,
    };
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

    fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            test(&state);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    #[test]
    fn the_closed_classification_is_never_purgeable() {
        with_state("retention_classification_never_purgeable", |state| {
            seed_retention_policies(state, "2026-08-01T08:00:00Z").expect("classes should seed");
            let connection = state.db().open().expect("database should open");

            let policy = load_policy(&connection, RecordClass::WorktimeClassification)
                .expect("Class A must be seeded");
            assert!(
                policy.never_purge,
                "ZEOR čl. 25 st. 3 — the derived classification is trajno"
            );
            assert_eq!(policy.retain_until, None, "trajno has no end date");
            assert!(!is_purgeable(&policy, "2026-08-01"));
            assert!(
                !is_purgeable(&policy, "9999-12-31"),
                "no calendar date ever reaches a trajno class"
            );

            // never_purge outranks a stored floor however it got there — a support
            // session, a restored older database, or a future purge job writing its
            // own notion of „isteklo“ into the shared table.
            connection
                .execute(
                    "UPDATE retention_policies SET retain_until = '2000-01-01'
                     WHERE record_class = ?1",
                    params![RecordClass::WorktimeClassification.key()],
                )
                .expect("a past floor should store");

            let tampered = load_policy(&connection, RecordClass::WorktimeClassification)
                .expect("Class A must still load");
            assert_eq!(tampered.retain_until.as_deref(), Some("2000-01-01"));
            assert!(
                !is_purgeable(&tampered, "2026-08-01"),
                "never_purge wins over any date"
            );
        });
    }

    /// §4d states the direction of the trade-off once, so the purge design
    /// „cannot get it backwards“: *an unreadable date, a missing floor and an
    /// absent policy row all refuse the purge rather than allow it.*
    ///
    /// No seeded class reaches the missing-floor branch on its own — Class A
    /// short-circuits on `never_purge`, and Class B and the overtime log both
    /// carry a concrete date — so the fail-safe is asserted directly on a
    /// constructed row. Flipping `None => false` to `None => true` in
    /// [`is_purgeable`] must fail here.
    #[test]
    fn a_class_with_no_recorded_floor_refuses_the_purge() {
        let no_floor = RetentionPolicy {
            record_class: RecordClass::WorktimeDraft,
            retain_until: None,
            legal_hold: false,
            never_purge: false,
        };

        assert!(
            !is_purgeable(&no_floor, "9999-12-31"),
            "§4d — a missing floor means keep, whatever day it is"
        );
        assert!(
            !is_purgeable(&no_floor, "2026-08-01"),
            "§4d — a missing floor means keep, whatever day it is"
        );
    }

    #[test]
    fn retain_until_only_ever_moves_forward() {
        with_state("retention_retain_until_moves_forward", |state| {
            seed_retention_policies(state, "2026-08-01T08:00:00Z").expect("classes should seed");
            let connection = state.db().open().expect("database should open");

            let seeded = load_policy(&connection, RecordClass::WorktimeOvertimeLog)
                .expect("the overtime log class must be seeded");
            assert_eq!(
                seeded.retain_until.as_deref(),
                Some("2029-08-01"),
                "the defensive floor is three years from the day it was recorded"
            );

            let extended = extend_retain_until(
                &connection,
                RecordClass::WorktimeOvertimeLog,
                "2030-01-31",
                "2026-09-01T08:00:00Z",
            )
            .expect("a later floor should store");
            assert_eq!(extended.retain_until.as_deref(), Some("2030-01-31"));

            let error = extend_retain_until(
                &connection,
                RecordClass::WorktimeOvertimeLog,
                "2029-12-31",
                "2026-09-02T08:00:00Z",
            )
            .expect_err("an earlier floor must be rejected");
            assert_eq!(error.code(), "validation_error");

            let unchanged = load_policy(&connection, RecordClass::WorktimeOvertimeLog)
                .expect("the class must still load");
            assert_eq!(
                unchanged.retain_until.as_deref(),
                Some("2030-01-31"),
                "a rejected shortening must leave the stored floor alone"
            );

            let malformed = extend_retain_until(
                &connection,
                RecordClass::WorktimeOvertimeLog,
                "31.01.2031.",
                "2026-09-03T08:00:00Z",
            )
            .expect_err("a floor that is not gggg-MM-dd must be rejected");
            assert_eq!(malformed.code(), "validation_error");

            // trajno is not a floor that can be shortened to a date.
            let trajno = extend_retain_until(
                &connection,
                RecordClass::WorktimeClassification,
                "2099-12-31",
                "2026-09-04T08:00:00Z",
            )
            .expect_err("a trajno class has no rok to move");
            assert_eq!(trajno.code(), "validation_error");
        });
    }

    #[test]
    fn a_draft_is_purge_eligible_only_once_its_period_is_closed() {
        with_state("retention_draft_waits_for_the_close", |state| {
            seed_retention_policies(state, "2026-08-01T08:00:00Z").expect("classes should seed");
            let connection = state.db().open().expect("database should open");

            let policy = load_policy(&connection, RecordClass::WorktimeDraft)
                .expect("Class B must be seeded");
            assert!(!policy.never_purge, "Class B is bounded, not trajno");
            assert!(
                is_purgeable(&policy, "2026-09-01"),
                "Class B carries no calendar floor of its own — the close is the gate"
            );

            connection
                .execute(
                    "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                     VALUES (800, 'radnik8', 'Radnik Osam', 'cashier', 1,
                             '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z')",
                    [],
                )
                .expect("employee should insert");

            assert!(
                !draft_purge_eligible(&connection, 800, 2026, 8, "2026-09-01")
                    .expect("eligibility should query"),
                "no period row at all means nothing has been derived yet"
            );

            connection
                .execute(
                    "INSERT INTO work_time_periods (user_id, godina, mesec, status, created_at, updated_at)
                     VALUES (800, 2026, 8, 'open', '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z')",
                    [],
                )
                .expect("open period should insert");
            assert!(
                !draft_purge_eligible(&connection, 800, 2026, 8, "2026-09-01")
                    .expect("eligibility should query"),
                "ZZPL čl. 5 st. 1 tač. 5 has no anchor until the period closes"
            );

            connection
                .execute(
                    "UPDATE work_time_periods SET status = 'closed', closed_at = '2026-09-01T08:00:00Z'
                     WHERE user_id = 800 AND godina = 2026 AND mesec = 8",
                    [],
                )
                .expect("period should close");
            assert!(
                draft_purge_eligible(&connection, 800, 2026, 8, "2026-09-01")
                    .expect("eligibility should query"),
                "a closed period is the moment raw data stops being necessary"
            );

            // Both clocks, and both must say yes: a legal hold on the shared table
            // stops the purge even after the close.
            connection
                .execute(
                    "UPDATE retention_policies SET legal_hold = 1 WHERE record_class = ?1",
                    params![RecordClass::WorktimeDraft.key()],
                )
                .expect("legal hold should store");
            assert!(
                !draft_purge_eligible(&connection, 800, 2026, 8, "2026-09-01")
                    .expect("eligibility should query"),
                "a legal hold outranks the close"
            );
        });
    }

    /// Req. 20's three years are a **per-record** obligation, so the class row
    /// alone must never release a day that is younger than its own floor. The
    /// stored class floor is three years from the day the shop first launched —
    /// an earliest-possible date for the class, not a date for any record — and
    /// the record's own `dan` is the second clock, exactly as
    /// `kep_close::purge_eligible` anchors to the book's `closed_at`.
    #[test]
    fn the_overtime_log_needs_both_the_class_floor_and_the_records_own_day() {
        with_state("retention_overtime_log_two_clocks", |state| {
            seed_retention_policies(state, "2026-08-01T08:00:00Z").expect("classes should seed");
            let connection = state.db().open().expect("database should open");

            // The class floor is 2029-08-01. A day recorded on 2029-07-31 is one
            // day old when that anniversary lands, and must survive it.
            assert!(
                !overtime_log_purge_eligible(&connection, "2029-07-31", "2029-08-01")
                    .expect("eligibility should query"),
                "req. 20 anchors the three years to the record, not to the launch day"
            );
            assert!(
                !overtime_log_purge_eligible(&connection, "2029-07-31", "2032-07-30")
                    .expect("eligibility should query"),
                "the day before the record's own floor is still too early"
            );
            assert!(
                overtime_log_purge_eligible(&connection, "2029-07-31", "2032-07-31")
                    .expect("eligibility should query"),
                "both clocks have run out"
            );

            // The class clock still binds on its own: an old record may not leave
            // before the class floor either.
            assert!(
                !overtime_log_purge_eligible(&connection, "2026-08-01", "2029-07-31")
                    .expect("eligibility should query"),
                "the class floor has not been reached"
            );
            assert!(
                overtime_log_purge_eligible(&connection, "2026-08-01", "2029-08-01")
                    .expect("eligibility should query"),
                "here the two clocks land on the same day"
            );

            // An unreadable day yields no floor, and no floor means keep (§4d).
            assert!(
                !overtime_log_purge_eligible(&connection, "31.07.2029.", "9999-12-31")
                    .expect("eligibility should query"),
                "§4d — a date that cannot be read refuses the purge"
            );

            // The shared row is the other clock, and it outranks the record's age.
            connection
                .execute(
                    "UPDATE retention_policies SET legal_hold = 1 WHERE record_class = ?1",
                    params![RecordClass::WorktimeOvertimeLog.key()],
                )
                .expect("legal hold should store");
            assert!(
                !overtime_log_purge_eligible(&connection, "2029-07-31", "9999-12-31")
                    .expect("eligibility should query"),
                "a legal hold outranks both clocks"
            );
        });
    }

    #[test]
    fn the_standalone_overtime_log_floor_is_three_years() {
        assert_eq!(STANDALONE_OVERTIME_LOG_FLOOR_YEARS, 3);
        assert_eq!(
            retention_floor("2026-08-01", STANDALONE_OVERTIME_LOG_FLOOR_YEARS).as_deref(),
            Some("2029-08-01")
        );
        assert_eq!(
            retention_floor("2026-08-01T18:00:00Z", STANDALONE_OVERTIME_LOG_FLOOR_YEARS).as_deref(),
            Some("2029-08-01"),
            "an RFC3339 stamp reduces to its calendar day"
        );
        // 29 February has no anniversary in a common year; it falls to 1 March,
        // the later day — the only direction a retention floor may ever move.
        assert_eq!(
            retention_floor("2028-02-29", STANDALONE_OVERTIME_LOG_FLOOR_YEARS).as_deref(),
            Some("2031-03-01")
        );
        assert_eq!(retention_floor("2026-8-1", 3), None);
        assert_eq!(retention_floor("", 3), None);
    }

    /// SW-13 req. 22 / SW-10 req. 6. The access log's period, and the direction
    /// its boundary moves — which is the opposite of a floor's, because it is the
    /// same axis read from the other end.
    #[test]
    fn the_access_log_cutoff_is_two_years_and_leans_toward_keeping() {
        assert_eq!(ACCESS_LOG_RETENTION_YEARS, 2);
        assert_eq!(
            expiry_cutoff("2029-08-01", ACCESS_LOG_RETENTION_YEARS).as_deref(),
            Some("2027-08-01"),
            "a line stamped before this day has outlived the period"
        );
        assert_eq!(
            expiry_cutoff("2029-08-01T03:00:00Z", ACCESS_LOG_RETENTION_YEARS).as_deref(),
            Some("2027-08-01"),
            "an RFC3339 stamp reduces to its calendar day"
        );
        // 29 February has no counterpart two common years back. It falls to 28
        // February — the EARLIER day, which keeps one more day of the log rather
        // than discarding one day early.
        assert_eq!(
            expiry_cutoff("2028-02-29", ACCESS_LOG_RETENTION_YEARS).as_deref(),
            Some("2026-02-28")
        );
        assert_eq!(expiry_cutoff("2029-8-1", 2), None);
        assert_eq!(expiry_cutoff("", 2), None);
    }

    /// §2a / req. 1. Čl. 48 binds only a „nadležni organ koji obrađuje podatke u
    /// posebne svrhe“; no Serbian provision obliges a private retail rukovalac to
    /// record who accessed personal data, and the shop must never be told
    /// otherwise. These notes are the sentence a person reads when they ask why a
    /// line is still there, so a bare parenthetical „(ZZPL čl. 48)“ reads as the
    /// osnov of a duty. `commands::audit::IZVOD_NAPOMENA` already states the
    /// honest framing on the čl. 48 st. 4 izvod — the two must not disagree.
    #[test]
    fn a_note_may_cite_cl_48_only_as_the_uzor_it_is() {
        for class in RecordClass::ALL {
            let note = class.napomena();
            if !note.contains("čl. 48") {
                continue;
            }

            assert!(
                note.contains("sopstven"),
                "{}: the note must say the rukovalac keeps this evidencija as its own measure \
                 before it cites čl. 48 — {note}",
                class.key()
            );
            assert!(
                note.contains("nadležni organ"),
                "{}: …and name who čl. 48 actually binds — {note}",
                class.key()
            );
            assert!(
                note.contains("posebne svrhe"),
                "{}: …and the purposes that confine it — {note}",
                class.key()
            );
        }

        // Req. 6 and req. 22 live in the same string, and a reword must not lose
        // them: no statutory period, and this class is never trajno.
        let access_log = RecordClass::AccessLog.napomena();
        assert!(
            access_log.contains("Zakon ne propisuje rok"),
            "the access log has no statutory period and the note has to say so: {access_log}"
        );
        assert!(
            access_log.contains("Ova evidencija se ne čuva trajno"),
            "and req. 6 forbids trajno for this class: {access_log}"
        );
    }

    #[test]
    fn every_record_class_round_trips_through_its_stored_key() {
        for class in RecordClass::ALL {
            assert_eq!(
                RecordClass::from_key(class.key()),
                Some(class),
                "{} must round-trip",
                class.key()
            );
        }
        assert_eq!(RecordClass::from_key("nesto_izmisljeno"), None);
    }

    /// The fence the go-live reset carries is only worth having if it fires.
    /// `reset_trading_data` deletes nothing from these tables today, so this is
    /// the test that keeps that guard from being decorative: it reproduces the
    /// snapshot/assert pair around a deletion and demands the failure.
    #[test]
    fn the_never_purge_fence_refuses_a_transaction_that_dropped_a_trajno_row() {
        with_state("retention_never_purge_fence", |state| {
            seed_retention_policies(state, "2026-08-01T08:00:00Z").expect("classes should seed");
            let mut connection = state.db().open().expect("database should open");
            connection
                .execute_batch(
                    "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                         VALUES (801, 'radnik9', 'Radnik Devet', 'cashier', 1,
                                 '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z');
                     INSERT INTO work_time_entries (user_id, dan, efektivno_izvrseni_minuta, created_at, updated_at)
                         VALUES (801, '2026-08-03', 480, '2026-08-03T18:00:00Z', '2026-08-03T18:00:00Z');",
                )
                .expect("a worked day should seed");

            let transaction = connection.transaction().expect("transaction should open");
            let before =
                super::never_purge_row_counts(&transaction).expect("counts should snapshot");
            assert!(
                super::assert_never_purge_intact(&transaction, &before).is_ok(),
                "an untouched transaction must pass the fence"
            );

            transaction
                .execute("DELETE FROM work_time_entries", [])
                .expect("the deletion itself is legal SQL — the fence is what stops it");
            let error = super::assert_never_purge_intact(&transaction, &before)
                .expect_err("a dropped row must abort the whole operation");
            assert_eq!(error.code(), "invalid_state");
            assert!(
                error.to_string().contains("work_time_entries"),
                "the failure must name the table it caught: {error}"
            );

            drop(transaction);
            let survivors: i64 = connection
                .query_row("SELECT COUNT(*) FROM work_time_entries", [], |row| {
                    row.get(0)
                })
                .expect("count should query");
            assert_eq!(survivors, 1, "the rolled-back transaction kept the row");
        });
    }

    /// The same fence, on the table the test above cannot see.
    ///
    /// SW-13 req. 19/24 gives the ZEOR register two layers: v18's
    /// `trg_personnel_records_trajno` refuses a row-level delete, and membership
    /// in [`NEVER_PURGE_TABLES`] aborts any transaction that ever gets past it.
    /// The fence test above hardcodes `work_time_entries`, so the second layer
    /// was carried by nothing — deleting the entry from the list left the whole
    /// suite green. Dropping the trigger inside the transaction stages exactly
    /// the „future edit that finds a way around it“ the module claims to survive;
    /// SQLite rolls the DDL back with everything else.
    #[test]
    fn the_never_purge_fence_covers_the_personnel_register_too() {
        assert!(
            super::NEVER_PURGE_TABLES.contains(&"personnel_records"),
            "the ZEOR čl. 5 evidencija is trajno under čl. 7 st. 2: without this entry the \
             transaction-level half of req. 19 does not exist"
        );

        with_state("retention_never_purge_fence_personnel", |state| {
            let mut connection = state.db().open().expect("database should open");
            connection
                .execute_batch(
                    "INSERT INTO users (id, username, display_name, role, active, created_at, updated_at)
                         VALUES (802, 'radnica2', 'Radnica Dva', 'cashier', 1,
                                 '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z');
                     INSERT INTO personnel_records (user_id, prezime_ime, created_at, updated_at)
                         VALUES (802, 'Radnica Dva', '2026-08-01T08:00:00Z', '2026-08-01T08:00:00Z');",
                )
                .expect("an employee record should seed");

            let transaction = connection.transaction().expect("transaction should open");
            let before =
                super::never_purge_row_counts(&transaction).expect("counts should snapshot");
            assert!(
                before.contains(&("personnel_records", 1)),
                "the snapshot the purge and the go-live reset both take must count the \
                 register: {before:?}"
            );

            // Layer one: the row-level trigger refuses outright.
            let error = transaction
                .execute("DELETE FROM personnel_records WHERE user_id = 802", [])
                .expect_err("the trigger must refuse a row-level delete");
            assert!(
                error.to_string().contains("trajno"),
                "the refusal must say why: {error}"
            );

            // Layer two: with the trigger gone the deletion is legal SQL again,
            // and the fence is the only thing left standing.
            transaction
                .execute_batch(
                    "DROP TRIGGER trg_personnel_records_trajno;
                     DELETE FROM personnel_records;",
                )
                .expect("without the trigger the deletion itself is legal SQL");
            let error = super::assert_never_purge_intact(&transaction, &before)
                .expect_err("a dropped register row must abort the whole operation");
            assert_eq!(error.code(), "invalid_state");
            assert!(
                error.to_string().contains("personnel_records"),
                "the failure must name the table it caught: {error}"
            );

            drop(transaction);
            let survivors: i64 = connection
                .query_row("SELECT COUNT(*) FROM personnel_records", [], |row| {
                    row.get(0)
                })
                .expect("count should query");
            assert_eq!(survivors, 1, "the rolled-back transaction kept the record");
            let triggers: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master
                      WHERE type = 'trigger' AND name = 'trg_personnel_records_trajno'",
                    [],
                    |row| row.get(0),
                )
                .expect("count should query");
            assert_eq!(triggers, 1, "and put the trigger back with it");
        });
    }

    /// The note is the operator-facing half of the class — the sentence a person
    /// reads when they ask why a record is still there — and SW-14 shipped one
    /// that promised a deletion-immunity the restore path never had. Correcting
    /// the literal in code has to reach a database seeded *before* the
    /// correction, or the false claim outlives its fix in the one place it
    /// actually lives. `seed_retention_policies` runs at every launch and again
    /// after every restore, and nothing else ever writes `napomena`, so it is
    /// the propagation path. The floor and the `never_purge` flag keep their own
    /// rules — this repairs derived text, never a retention decision.
    #[test]
    fn re_seeding_carries_a_corrected_note_into_a_row_seeded_before_it() {
        with_state("retention_seed_repairs_the_note", |state| {
            seed_retention_policies(state, "2026-08-01T08:00:00Z").expect("classes should seed");
            let connection = state.db().open().expect("database should open");

            // The superseded note, exactly as an SW-14 database carries it.
            connection
                .execute(
                    "UPDATE retention_policies
                     SET napomena = 'Nijedno brisanje, resetovanje, vraćanje iz rezervne kopije ni čišćenje rezervnih kopija ne sme da je dodirne.',
                         updated_at = '2026-08-01T08:00:00Z'
                     WHERE record_class = ?1",
                    params![RecordClass::WorktimeClassification.key()],
                )
                .expect("the superseded note should store");

            seed_retention_policies(state, "2026-09-01T08:00:00Z").expect("re-seed should run");

            let (napomena, updated_at): (String, String) = connection
                .query_row(
                    "SELECT napomena, updated_at FROM retention_policies WHERE record_class = ?1",
                    params![RecordClass::WorktimeClassification.key()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("Class A should load");
            assert_eq!(
                napomena,
                RecordClass::WorktimeClassification.napomena(),
                "a corrected note must reach a row that was seeded before the correction"
            );
            assert_eq!(
                updated_at, "2026-09-01T08:00:00Z",
                "rewriting the note is a change to the row and moves its updated_at"
            );

            // …and a seed that changes nothing still changes nothing: the stamp
            // records when the row last moved, not when the app last started.
            seed_retention_policies(state, "2027-01-01T08:00:00Z").expect("re-seed should run");
            let unchanged: String = connection
                .query_row(
                    "SELECT updated_at FROM retention_policies WHERE record_class = ?1",
                    params![RecordClass::WorktimeClassification.key()],
                    |row| row.get(0),
                )
                .expect("Class A should load");
            assert_eq!(
                unchanged, "2026-09-01T08:00:00Z",
                "an idempotent seed must not churn updated_at"
            );
        });
    }

    #[test]
    fn seeding_twice_never_pulls_a_stored_floor_back() {
        with_state("retention_seed_is_idempotent", |state| {
            seed_retention_policies(state, "2026-08-01T08:00:00Z").expect("classes should seed");
            let connection = state.db().open().expect("database should open");

            extend_retain_until(
                &connection,
                RecordClass::WorktimeOvertimeLog,
                "2040-01-01",
                "2026-09-01T08:00:00Z",
            )
            .expect("the shop should be able to keep records longer");

            // A `never_purge = 0` row on a trajno class is what a purge would
            // walk straight through; re-seeding must repair it upward.
            connection
                .execute(
                    "UPDATE retention_policies SET never_purge = 0 WHERE record_class = ?1",
                    params![RecordClass::WorktimeClassification.key()],
                )
                .expect("the flag should clear");

            seed_retention_policies(state, "2027-01-01T08:00:00Z").expect("re-seed should run");

            let count: i64 = connection
                .query_row("SELECT COUNT(*) FROM retention_policies", [], |row| {
                    row.get(0)
                })
                .expect("count should query");
            assert_eq!(
                count,
                RecordClass::ALL.len() as i64,
                "seeding is idempotent, never duplicating a class"
            );

            let log = load_policy(&connection, RecordClass::WorktimeOvertimeLog)
                .expect("the class must load");
            assert_eq!(
                log.retain_until.as_deref(),
                Some("2040-01-01"),
                "a re-seed must not pull an extended floor back to its default"
            );

            let classification = load_policy(&connection, RecordClass::WorktimeClassification)
                .expect("Class A must load");
            assert!(
                classification.never_purge,
                "the trajno flag is repaired upward on every seed"
            );
        });
    }
}
