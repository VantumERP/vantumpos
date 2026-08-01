//! The command layer over the ZoR čl. 55 st. 6 daily register (SW-14).
//!
//! Three rules shape every function here:
//!
//! 1. **Append-only.** There is not one `UPDATE` against `work_time_entries` in
//!    this file, and migration v17 has neither an UPDATE nor a DELETE trigger
//!    path. A correction is a new row with `verzija = predecessor.verzija + 1`
//!    carrying who/when/why; the live row for a day is `MAX(verzija)`, never
//!    `supersedes_id IS NULL`. ZEOR čl. 46 st. 1 puts accuracy responsibility on
//!    the record-keeper, and a register whose past silently changes cannot
//!    discharge it.
//! 2. **The gate is inside the domain function.** `require_admin` is the first
//!    statement of every admin function, not of the `#[tauri::command]` wrapper,
//!    so a future in-process caller cannot route around it. `my_hours` is the
//!    single exception and is session-gated to the caller's own rows — ZoR
//!    čl. 83 st. 1 and ZZPL čl. 26, discharged in one read-only view.
//! 3. **`now` is a parameter.** No `datetime('now')`, no wall clock inside
//!    anything a test must pin. Minutes are the unit throughout; there is no
//!    floating point in this file.
//!
//! A cap breach never blocks a write — it asks for a čl. 53 st. 1 ground and
//! records the day. §4 req. 7 makes the caps the larger fine (čl. 274 st. 1
//! tač. 3), and a register that refuses to describe a day that actually happened
//! hides that exposure instead of surfacing it. A čl. 87–91 protection block is
//! the opposite: the statute bans the work itself, so the row is refused.

use std::fs;
use std::path::Path;

use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::cash_deposit::parse_iso_date;
use crate::clock::utc_now;
use crate::commands::reports::{csv_line, ExportedFile};
use crate::state::AppState;
use crate::worktime::{
    assess_caps, check_protection, CapAssessment, DayHours, EmployeeProtection,
    PRERASPODELA_WEEKLY_CAP_MINUTES,
};

/// §4 req. 22, verbatim. The one sentence every rendering of this register
/// carries, so that no surface can imply a propisani obrazac exists — ZoR
/// čl. 55 st. 6 delegates nothing and no ministerial act was ever made.
pub const EVIDENCIJA_ZAGLAVLJE: &str =
    "Evidencija prekovremenog rada — ZoR čl. 55 st. 6. Zakon ne propisuje obrazac.";

/// The advisory tag §4 req. 4 mandates for `nocni_minuta` and
/// `rad_na_praznik_minuta`. No Serbian provision requires either, so they must
/// never appear under a statutory heading.
const ADVISORY_TAG: &str = "izračunato radi provere usklađenosti";

/// The ten `kategorija_odsustva` values migration v17's `CHECK` accepts.
///
/// This is a closed enum, not a category → bucket map: the bucket column is
/// **derived** from the category (`{kategorija}_minuta`) in [`book_absence`], and
/// `migration_v17_derives_every_absence_bucket_from_its_category` holds the two
/// vocabularies together. The list exists only so an unknown category fails as a
/// clean validation error here instead of as a raw `CHECK` violation from SQLite,
/// and so no operator string is ever interpolated into SQL unvalidated.
const KATEGORIJE_ODSUSTVA: [&str; 10] = [
    "godisnji_odmor",
    "praznik_odmor",
    "odsustvo_uz_naknadu",
    "strucno_osposobljavanje",
    "sprecenost_poslodavac",
    "sprecenost_rfzo",
    "porodiljsko",
    "neplaceno_odsustvo",
    "naknada_drugi_poslodavci",
    "obustava_rada_strajk",
];

/// The ZoR čl. 53 st. 1 grounds on which overtime may be **ordered**, mirroring
/// v17's `CHECK`. Recording one does **not** make a čl. 53 st. 2/st. 3 breach
/// lawful — it records why the employer says the day happened.
///
/// The same closed list is what the čl. 57 st. 5 leg in [`assess_caps_for_employee`]
/// demands, because v17 is frozen and carries no second vocabulary. It is a
/// stretch on the face of it — čl. 53 st. 1 speaks of ordering overtime, and
/// čl. 58 says preraspodela is not overtime — but the alternative is a
/// sixty-hour ceiling that asks for nothing at all.
const CAP_OVERRIDE_RAZLOZI: [&str; 4] = [
    "visa_sila",
    "iznenadno_povecanje_obima_posla",
    "neplanirani_posao_u_roku",
    "drugo",
];

/// Correction reasons, mirroring v17's `CHECK`. A closed enum because **no**
/// unconstrained TEXT may exist on a table that also carries an absence
/// category and the two sprečenost buckets (§4 req. 3, §5 item 4).
const KOREKCIJA_RAZLOZI: [&str; 5] = [
    "greska_u_unosu",
    "ispravka_sati",
    "ispravka_kategorije",
    "naknadno_dostavljen_dokument",
    "drugo",
];

/// Declares the minute columns of `work_time_entries` **once**, and derives from
/// that single list the struct, the SQL column list, the row reader, the export
/// header and the by-name field lookup the absence-bucket derivation needs.
///
/// One declaration is the point: a category books into the column its own name
/// derives (`obustava_rada_strajk` → `obustava_rada_strajk_minuta`), and a
/// hand-maintained second list would be exactly the place where one wrong line
/// posts ZEOR čl. 24 tač. 1 d) hours into the g) bucket with nothing downstream
/// able to notice.
macro_rules! work_time_minutes {
    ($($field:ident => $label:literal),+ $(,)?) => {
        /// One day's hours, in the ZEOR čl. 24 tač. 1 buckets. Integer minutes.
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct WorkTimeMinutes {
            $(pub $field: i64,)+
        }

        impl WorkTimeMinutes {
            /// The columns, in field order. Used for both the SELECT and the
            /// INSERT, so the two can never drift.
            const COLUMNS: &'static [&'static str] = &[$(stringify!($field),)+];

            /// Human labels, in the same order, each naming its ZEOR letter.
            const LABELS: &'static [&'static str] = &[$($label,)+];

            fn from_row(row: &Row<'_>) -> rusqlite::Result<Self> {
                Ok(Self { $($field: row.get(stringify!($field))?,)+ })
            }

            fn values(&self) -> Vec<i64> {
                vec![$(self.$field,)+]
            }

            /// The slot named `column`, or `None` when no such column exists.
            fn field_mut(&mut self, column: &str) -> Option<&mut i64> {
                match column {
                    $(stringify!($field) => Some(&mut self.$field),)+
                    _ => None,
                }
            }

            fn accumulate(&mut self, other: &Self) {
                $(self.$field += other.$field;)+
            }
        }
    };
}

work_time_minutes! {
    moguci_minuta => "a) Mogući časovi (min)",
    ukupno_ostvareni_minuta => "b) Ukupno ostvareni časovi (min)",
    efektivno_izvrseni_minuta => "b) – efektivno izvršeni časovi (min)",
    casovi_cekanja_i_zastoja_minuta => "b) – časovi čekanja, zastoja i prekida (min)",
    obustava_rada_strajk_minuta => "b) – časovi obustave rada zbog štrajka (min)",
    ukupno_neizvrseni_minuta => "v) Ukupno neizvršeni časovi (min)",
    godisnji_odmor_minuta => "g) Časovi godišnjeg odmora (min)",
    praznik_odmor_minuta => "g) Časovi odmora za dane državnih praznika (min)",
    odsustvo_uz_naknadu_minuta => "g) Časovi odsustva uz naknadu zarade (min)",
    strucno_osposobljavanje_minuta => "g) Časovi stručnog osposobljavanja i usavršavanja (min)",
    sprecenost_poslodavac_minuta => "g) Časovi privremene sprečenosti — sredstva poslodavca (min)",
    naknada_drugi_poslodavci_minuta => "d) Časovi naknade na teret drugih poslodavaca (min)",
    sprecenost_rfzo_minuta => "đ) Časovi privremene sprečenosti — sredstva RFZO (min)",
    porodiljsko_minuta => "đ) Časovi porodiljskog i skraćenog radnog vremena roditelja (min)",
    neplaceno_odsustvo_minuta => "e) Časovi neplaćenog odsustva (min)",
    prekovremeni_minuta => "ž) Časovi prekovremenog rada (min)",
    nocni_minuta => "Noćni časovi (min) — izračunato radi provere usklađenosti",
    rad_na_praznik_minuta => "Časovi rada na praznik (min) — izračunato radi provere usklađenosti",
}

/// One stored version of one day. The whole chain is returned to the UI, not
/// just the live row — §4 req. 6 wants the superseded original rendered struck
/// through rather than hidden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkTimeEntryView {
    pub id: i64,
    pub user_id: i64,
    pub dan: String,
    pub verzija: i64,
    /// `true` when a later `verzija` exists for this day — render struck through.
    pub zamenjen: bool,
    pub supersedes_id: Option<i64>,
    pub kategorija_odsustva: Option<String>,
    pub cap_override_razlog: Option<String>,
    pub korekcija_razlog: Option<String>,
    pub unio_user_id: Option<i64>,
    pub unio_ime: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub minuti: WorkTimeMinutes,
}

/// One employee's month, every version of every day plus the live totals.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkTimeMonth {
    pub user_id: i64,
    pub zaposleni: String,
    pub godina: i64,
    pub mesec: i64,
    pub zatvoren: bool,
    pub closed_at: Option<String>,
    pub entries: Vec<WorkTimeEntryView>,
    /// Live rows only — `MAX(verzija)` per day.
    pub ukupno: WorkTimeMinutes,
    /// [`EVIDENCIJA_ZAGLAVLJE`], carried so no surface retypes it.
    pub napomena: String,
    /// [`ADVISORY_TAG`], carried for the same reason (§4 req. 4).
    pub advisory_napomena: String,
}

/// The Class A classification a period close freezes (§4 req. 18, req. 19).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeriodClassification {
    pub user_id: i64,
    pub godina: i64,
    pub mesec: i64,
    pub dana_sa_unosom: usize,
    pub minuti: WorkTimeMinutes,
    pub izvedeno_u: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClosedPeriod {
    pub user_id: i64,
    pub godina: i64,
    pub mesec: i64,
    pub closed_at: String,
    pub closed_by: i64,
    pub klasifikacija: PeriodClassification,
}

/// One day as the operator enters it.
///
/// The totals of ZEOR čl. 24 tač. 1 b) and v) are **not** here: they are derived
/// from the buckets in [`derive_totals`], so a stored row can never claim a
/// total its own indents do not add up to.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEntryRequest {
    pub user_id: i64,
    pub dan: String,
    /// The period the operator had on screen, and the one `dan` must fall inside
    /// — see [`guard_day_is_in_period`]. Deliberately **not** `#[serde(default)]`:
    /// the field exists to carry an assertion the write is checked against, and a
    /// default would let the caller it guards against drop it silently.
    pub godina: i64,
    pub mesec: i64,
    #[serde(default)]
    pub moguci_minuta: i64,
    #[serde(default)]
    pub efektivno_izvrseni_minuta: i64,
    #[serde(default)]
    pub casovi_cekanja_i_zastoja_minuta: i64,
    #[serde(default)]
    pub prekovremeni_minuta: i64,
    #[serde(default)]
    pub nocni_minuta: i64,
    #[serde(default)]
    pub rad_na_praznik_minuta: i64,
    /// One of [`KATEGORIJE_ODSUSTVA`]. There is no free-text sibling and never
    /// will be — §5 item 4.
    #[serde(default)]
    pub kategorija_odsustva: Option<String>,
    /// Booked into the bucket derived from `kategorija_odsustva`.
    #[serde(default)]
    pub odsustvo_minuta: i64,
    /// One of [`CAP_OVERRIDE_RAZLOZI`]; required once a čl. 53 cap is exceeded.
    #[serde(default)]
    pub cap_override_razlog: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectEntryRequest {
    #[serde(flatten)]
    pub entry: SaveEntryRequest,
    /// One of [`KOREKCIJA_RAZLOZI`].
    pub korekcija_razlog: String,
}

/// The two ZoR notices this register surfaces, already tier-resolved.
///
/// They travel together because they are the two halves of the same exposure:
/// not keeping the register at all (čl. 276 st. 1 tač. 1a) and keeping one that
/// records a breach of the čl. 53 caps (čl. 274 st. 1 tač. 3, the larger fine).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkTimeNotices {
    pub record_missing: crate::legal::LegalNotice,
    pub caps_exceeded: crate::legal::LegalNotice,
}

/// What a write returns: the stored row, the čl. 53 assessment it was measured
/// against, and any non-blocking čl. 87–91 findings the operator must see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedEntry {
    pub entry: WorkTimeEntryView,
    pub caps: CapAssessment,
    pub protections: Vec<crate::worktime::ProtectionBlock>,
}

#[tauri::command]
pub fn worktime_list_month(
    state: State<'_, AppState>,
    user_id: i64,
    godina: i64,
    mesec: i64,
) -> Result<WorkTimeMonth, CommandError> {
    list_month(state.inner(), user_id, godina, mesec).map_err(Into::into)
}

#[tauri::command]
pub fn worktime_save_entry(
    state: State<'_, AppState>,
    request: SaveEntryRequest,
) -> Result<SavedEntry, CommandError> {
    save_entry(state.inner(), request, &utc_now()?).map_err(Into::into)
}

#[tauri::command]
pub fn worktime_correct_entry(
    state: State<'_, AppState>,
    request: CorrectEntryRequest,
) -> Result<SavedEntry, CommandError> {
    correct_entry(state.inner(), request, &utc_now()?).map_err(Into::into)
}

#[tauri::command]
pub fn worktime_close_period(
    state: State<'_, AppState>,
    user_id: i64,
    godina: i64,
    mesec: i64,
) -> Result<ClosedPeriod, CommandError> {
    close_period(state.inner(), user_id, godina, mesec, &utc_now()?).map_err(Into::into)
}

#[tauri::command]
pub fn worktime_export_csv(
    state: State<'_, AppState>,
    user_id: i64,
    godina: i64,
    mesec: i64,
) -> Result<ExportedFile, CommandError> {
    export_month_csv(state.inner(), user_id, godina, mesec).map_err(Into::into)
}

/// The two ZoR notices the register surfaces, resolved against the stored
/// legal form.
///
/// Read-only and not admin-gated, mirroring `settings_lpfr_notice`: it states a
/// duty and its tier, and holds no employee data of any kind.
#[tauri::command]
pub fn worktime_notices(state: State<'_, AppState>) -> Result<WorkTimeNotices, CommandError> {
    notices(state.inner()).map_err(Into::into)
}

/// The employee's own read-only view. Session-gated, own rows only.
#[tauri::command]
pub fn worktime_my_hours(
    state: State<'_, AppState>,
    godina: i64,
    mesec: i64,
) -> Result<WorkTimeMonth, CommandError> {
    my_hours(state.inner(), godina, mesec).map_err(Into::into)
}

pub fn list_month(
    state: &AppState,
    user_id: i64,
    godina: i64,
    mesec: i64,
) -> Result<WorkTimeMonth, AppError> {
    crate::commands::auth::require_admin(state)?;
    load_month(state, user_id, godina, mesec)
}

/// Resolves both ZoR notices against the shop's stored legal form.
///
/// The register's surfaces render this and never compose a figure of their own —
/// `legal.rs` is the only place a fine amount is decided, and its enumerated
/// `all_notices` guard is what keeps a ZEOR amount out of every one of them.
pub fn notices(state: &AppState) -> Result<WorkTimeNotices, AppError> {
    let profile = crate::commands::settings::load_shop_profile(state)?;

    Ok(WorkTimeNotices {
        record_missing: crate::legal::overtime_record_missing(&profile),
        caps_exceeded: crate::legal::overtime_caps_exceeded(&profile),
    })
}

/// ZoR čl. 83 st. 1 and ZZPL čl. 26 in one read-only surface. The employee is
/// resolved from the server-side session, never from a client-supplied id, so
/// there is no parameter here that could be pointed at a colleague.
pub fn my_hours(state: &AppState, godina: i64, mesec: i64) -> Result<WorkTimeMonth, AppError> {
    let user_id = crate::commands::auth::require_session(state)?;
    load_month(state, user_id, godina, mesec)
}

/// Records one day as `verzija 1`.
///
/// The order of the checks is deliberate: the day must have happened at all
/// ([`guard_day_has_happened`] — a day still in the future describes no work, so
/// no later check has anything real to run against) and must be the day the write
/// was made for ([`guard_day_is_in_period`]), then the closed-period freeze (a
/// closed month is not writable on any ground), then the čl. 87–91 guards (the
/// statute bans the work, so nothing can authorise the row), then the čl. 53 caps
/// (which ask for a reason rather than refusing). The two day guards run in the
/// order the Datum field applies them, so the operator meets the same sentence
/// whichever side refuses first.
pub fn save_entry(
    state: &AppState,
    request: SaveEntryRequest,
    now: &str,
) -> Result<SavedEntry, AppError> {
    let acting = crate::commands::auth::require_admin(state)?;
    write_entry(state, request, None, acting.id, now)
}

/// Appends a correcting version of a day. The predecessor is never touched.
pub fn correct_entry(
    state: &AppState,
    request: CorrectEntryRequest,
    now: &str,
) -> Result<SavedEntry, AppError> {
    let acting = crate::commands::auth::require_admin(state)?;
    let razlog = request.korekcija_razlog;
    if !KOREKCIJA_RAZLOZI.contains(&razlog.as_str()) {
        return Err(AppError::validation(
            "Nepoznat razlog ispravke.",
            serde_json::json!({ "korekcijaRazlog": razlog }),
        ));
    }
    write_entry(state, request.entry, Some(razlog), acting.id, now)
}

/// Freezes the month's Class A classification and marks the period closed.
///
/// Irreversible by construction: there is no reopen command, and the guard is
/// the stored `status`, so a second close fails whatever the caller intends.
/// §4 req. 19 — without a real state transition there is no defensible moment at
/// which raw data stops being necessary.
///
/// Because it is irreversible, the month must have **ended** — see
/// [`guard_period_has_ended`].
pub fn close_period(
    state: &AppState,
    user_id: i64,
    godina: i64,
    mesec: i64,
    now: &str,
) -> Result<ClosedPeriod, AppError> {
    let acting = crate::commands::auth::require_admin(state)?;
    validate_month(godina, mesec)?;
    guard_period_has_ended(godina, mesec, now)?;

    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;

    ensure_employee_exists(&tx, user_id)?;
    if period_is_closed(&tx, user_id, godina, mesec)? {
        return Err(period_closed_error(godina, mesec));
    }

    let (first, last) = month_bounds(godina, mesec);
    let entries = load_entries(
        &tx,
        "WHERE e.user_id = ?1 AND e.dan >= ?2 AND e.dan <= ?3",
        params![user_id, first, last],
    )?;
    let live: Vec<&WorkTimeEntryView> = entries.iter().filter(|entry| !entry.zamenjen).collect();
    let mut minuti = WorkTimeMinutes::default();
    for entry in &live {
        minuti.accumulate(&entry.minuti);
    }

    let klasifikacija = PeriodClassification {
        user_id,
        godina,
        mesec,
        dana_sa_unosom: live.len(),
        minuti,
        izvedeno_u: now.to_string(),
    };
    let klasifikacija_json = serde_json::to_string(&klasifikacija).map_err(|source| {
        AppError::InvalidState(format!("Klasifikacija nije mogla biti sačuvana: {source}"))
    })?;

    let changed = tx.execute(
        "INSERT INTO work_time_periods
             (user_id, godina, mesec, status, closed_at, closed_by, klasifikacija_json, created_at, updated_at)
         VALUES (?1, ?2, ?3, 'closed', ?4, ?5, ?6, ?4, ?4)
         ON CONFLICT(user_id, godina, mesec) DO UPDATE SET
             status = 'closed', closed_at = ?4, closed_by = ?5,
             klasifikacija_json = ?6, updated_at = ?4
         WHERE work_time_periods.status = 'open'",
        params![user_id, godina, mesec, now, acting.id, klasifikacija_json],
    )?;
    if changed == 0 {
        return Err(period_closed_error(godina, mesec));
    }
    tx.commit()?;

    Ok(ClosedPeriod {
        user_id,
        godina,
        mesec,
        closed_at: now.to_string(),
        closed_by: acting.id,
        klasifikacija,
    })
}

/// Writes the month to `exports/` beside the database — offline, from the till,
/// which §4 req. 21 makes the real constraint (ZIN čl. 20 st. 7 / čl. 21 tač. 4).
/// Live rows only: an inspector reads what the register currently says, and the
/// superseded versions stay in the app where they can be shown struck through.
pub fn export_month_csv(
    state: &AppState,
    user_id: i64,
    godina: i64,
    mesec: i64,
) -> Result<ExportedFile, AppError> {
    let month = list_month(state, user_id, godina, mesec)?;
    let live: Vec<&WorkTimeEntryView> = month.entries.iter().filter(|e| !e.zamenjen).collect();
    let csv = month_to_csv(&month, &live);

    let export_dir = state.db().path().parent().map_or_else(
        || Path::new(".").join("exports"),
        |parent| parent.join("exports"),
    );
    fs::create_dir_all(&export_dir)?;

    let file_name = format!("evidencija-radnog-vremena-{user_id}-{godina}-{mesec:02}.csv");
    let path = export_dir.join(&file_name);
    fs::write(&path, csv)?;

    Ok(ExportedFile {
        file_name,
        path: path.display().to_string(),
        mime_type: "text/csv",
        row_count: live.len(),
    })
}

/// The one write path. `korekcija` is `Some` for an ispravka and `None` for an
/// original; everything else is identical, which is what keeps a correction from
/// being able to do anything an original cannot.
fn write_entry(
    state: &AppState,
    request: SaveEntryRequest,
    korekcija: Option<String>,
    acting_id: i64,
    now: &str,
) -> Result<SavedEntry, AppError> {
    let dan = request.dan.clone();
    let Some(datum) = parse_iso_date(&dan) else {
        return Err(AppError::validation(
            "Datum mora biti u obliku gggg-MM-dd.",
            serde_json::json!({ "dan": dan }),
        ));
    };
    guard_day_has_happened(&dan, datum, now)?;
    let godina = i64::from(datum.year());
    let mesec = i64::from(u8::from(datum.month()));
    validate_month(request.godina, request.mesec)?;
    guard_day_is_in_period(&dan, godina, mesec, request.godina, request.mesec)?;

    let minuti = build_minutes(&request)?;
    validate_cap_override(&request)?;

    // One transaction over the closed-period check, the live-row lookup and the
    // INSERT — the codebase norm for every multi-statement statutory write. A save
    // that read an open month while a `close_period` was mid-flight would otherwise
    // land a row in a month that is closed by the time it commits: unwritable-to
    // ever after (`period_closed` refuses the correction) and absent from the
    // frozen Class A `klasifikacija_json`, so the permanent classification would
    // silently disagree with the live rows it is supposed to summarise.
    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;

    ensure_employee_exists(&tx, request.user_id)?;
    if period_is_closed(&tx, request.user_id, godina, mesec)? {
        return Err(period_closed_error(godina, mesec));
    }

    let live = live_entry(&tx, request.user_id, &dan)?;
    let (verzija, supersedes_id) = match (&korekcija, &live) {
        (Some(_), Some(previous)) => (previous.verzija + 1, Some(previous.id)),
        (Some(_), None) => {
            return Err(AppError::not_found(
                "Za ovaj dan ne postoji unos koji bi se ispravio.",
            ))
        }
        (None, Some(_)) => {
            return Err(AppError::business(
                "entry_exists",
                "Za ovaj dan već postoji unos. Izmena se evidentira kao ispravka.",
            ))
        }
        (None, None) => (1, None),
    };

    let protection = load_protection(&tx, request.user_id)?;
    let day_hours = DayHours {
        dan: dan.clone(),
        efektivno_minuta: minuti.efektivno_izvrseni_minuta,
        prekovremeni_minuta: minuti.prekovremeni_minuta,
    };

    let protections = check_protection(&protection, &dan, &day_hours);
    if let Some(block) = protections.iter().find(|block| block.blocking) {
        return Err(AppError::business_with_details(
            "protection_block",
            block.poruka.clone(),
            serde_json::json!({ "protections": protections }),
        ));
    }

    let week = load_week(&tx, request.user_id, &dan)?;
    let caps = assess_caps_for_employee(&dan, &day_hours, &week, &protection);
    if caps.requires_override && request.cap_override_razlog.is_none() {
        let poruka = if caps.preraspodela_weekly_cap_exceeded {
            "Prekoračen je limit radnog vremena u preraspodeli — 60 časova nedeljno \
             (ZoR čl. 57 st. 5). Dan se može evidentirati, ali morate izabrati razlog \
             prekoračenja."
        } else {
            "Prekoračen je zakonski limit iz ZoR čl. 53. Dan se može evidentirati, \
             ali morate izabrati razlog prekoračenja."
        };
        return Err(AppError::business_with_details(
            "cap_override_required",
            poruka,
            serde_json::json!({ "caps": caps }),
        ));
    }

    let id = insert_entry(
        &tx,
        &request,
        &minuti,
        verzija,
        supersedes_id,
        korekcija.as_deref(),
        acting_id,
        now,
    )?;
    tx.commit()?;

    let entry = load_entries(&connection, "WHERE e.id = ?1", params![id])?
        .pop()
        .ok_or_else(|| AppError::not_found("Unos nije pronađen."))?;

    Ok(SavedEntry {
        entry,
        caps,
        protections,
    })
}

/// Applies the čl. 53 caps with the čl. 57/čl. 58 branch §4 req. 9 demands.
///
/// `assess_caps` states the čl. 53 st. 3 rule set alone and documents that the
/// caller must branch the override gate: preraspodela is not prekovremeni rad
/// (čl. 58) and čl. 57 has no daily leg at all, so gating on the 12 h figure there
/// reports a lawful day as a breach. **The branch swaps one ceiling for another,
/// it never deletes one.** čl. 58 also means such an employee records
/// `prekovremeni_minuta = 0`, so the čl. 53 st. 2 weekly leg cannot fire for them
/// either — remove the daily leg with nothing in its place and they have no
/// total-hours ceiling of any kind, and seven thirteen-hour days save in silence.
/// čl. 57 st. 5 — „radno vreme ne može da traje duže od 60 časova nedeljno“ — is
/// what takes over, measured across the day's Monday-anchored calendar week.
/// Breaching preraspodela is čl. 274 st. 1 tač. 4, the bigger fine.
///
/// The weekly overtime leg is *not* branched: čl. 53 st. 2 still governs any
/// overtime the operator actually records, whatever the profile says.
///
/// **`daily_cap_exceeded` is deliberately left standing.** v17 carries a single
/// `radi_u_preraspodeli` flag and it cannot tell čl. 57 preraspodela — no daily
/// leg — from the čl. 56 st. 3 monthly-average scheme, where čl. 56 st. 4
/// *expressly* restates 12 časova dnevno and 48 časova nedeljno. Because the flag
/// conflates the two, erasing the twelve-hour figure would hide a cap that may
/// well apply; it stays visible as a non-blocking finding and only the override
/// gate is branched. The 48 h leg cannot be applied at all without knowing which
/// scheme the employee is in, and v17 is frozen — it needs a column that says so.
fn assess_caps_for_employee(
    dan: &str,
    entry: &DayHours,
    week: &[DayHours],
    protection: &EmployeeProtection,
) -> CapAssessment {
    let mut caps = assess_caps(dan, entry, week);
    if protection.radi_u_preraspodeli {
        caps.preraspodela_weekly_cap_exceeded =
            caps.weekly_total_minutes > PRERASPODELA_WEEKLY_CAP_MINUTES;
        caps.requires_override = caps.weekly_cap_exceeded || caps.preraspodela_weekly_cap_exceeded;
    }
    caps
}

/// Turns the request into stored minutes: validate, book the absence into the
/// bucket its category derives, then derive the two ZEOR totals.
fn build_minutes(request: &SaveEntryRequest) -> Result<WorkTimeMinutes, AppError> {
    for (name, value) in [
        ("moguciMinuta", request.moguci_minuta),
        ("efektivnoIzvrseniMinuta", request.efektivno_izvrseni_minuta),
        (
            "casoviCekanjaIZastojaMinuta",
            request.casovi_cekanja_i_zastoja_minuta,
        ),
        ("prekovremeniMinuta", request.prekovremeni_minuta),
        ("nocniMinuta", request.nocni_minuta),
        ("radNaPraznikMinuta", request.rad_na_praznik_minuta),
        ("odsustvoMinuta", request.odsustvo_minuta),
    ] {
        if value < 0 {
            return Err(AppError::validation(
                "Broj minuta ne može biti negativan.",
                serde_json::json!({ "polje": name, "vrednost": value }),
            ));
        }
    }

    let mut minuti = WorkTimeMinutes {
        moguci_minuta: request.moguci_minuta,
        efektivno_izvrseni_minuta: request.efektivno_izvrseni_minuta,
        casovi_cekanja_i_zastoja_minuta: request.casovi_cekanja_i_zastoja_minuta,
        prekovremeni_minuta: request.prekovremeni_minuta,
        nocni_minuta: request.nocni_minuta,
        rad_na_praznik_minuta: request.rad_na_praznik_minuta,
        ..WorkTimeMinutes::default()
    };

    if let Some(kategorija) = request.kategorija_odsustva.as_deref() {
        book_absence(&mut minuti, kategorija, request.odsustvo_minuta)?;
    } else if request.odsustvo_minuta != 0 {
        return Err(AppError::validation(
            "Časovi odsustva se ne mogu evidentirati bez kategorije odsustva.",
            serde_json::json!({ "odsustvoMinuta": request.odsustvo_minuta }),
        ));
    }

    derive_totals(&mut minuti);
    Ok(minuti)
}

/// Books `minuta` into the bucket **derived** from `kategorija`.
///
/// Every `kategorija_odsustva` value is exactly its minute column minus the
/// `_minuta` suffix, and migration v17's
/// `migration_v17_derives_every_absence_bucket_from_its_category` parses the live
/// `CHECK` to keep it that way. So the column is computed here, never looked up.
fn book_absence(
    minuti: &mut WorkTimeMinutes,
    kategorija: &str,
    minuta: i64,
) -> Result<(), AppError> {
    if !KATEGORIJE_ODSUSTVA.contains(&kategorija) {
        return Err(AppError::validation(
            "Nepoznata kategorija odsustva.",
            serde_json::json!({ "kategorijaOdsustva": kategorija }),
        ));
    }
    let column = format!("{kategorija}_minuta");
    let slot = minuti.field_mut(&column).ok_or_else(|| {
        AppError::InvalidState(format!(
            "Kategorija odsustva „{kategorija}“ nema odgovarajuću kolonu „{column}“."
        ))
    })?;
    *slot = minuta;
    Ok(())
}

/// Derives ZEOR čl. 24 tač. 1 b) and v) from the buckets they enumerate.
///
/// b) reads *"ukupno ostvareni časovi …, od toga:"* followed by three indents —
/// efektivno izvršeni, čekanja/zastoji/prekidi, obustava rada zbog štrajka — so
/// the total **is** the sum of its own indents. Prekovremeni rad is ž), *"časovi
/// rada dužeg od punog radnog vremena"*, a separate letter, and is deliberately
/// not inside b). v) is the total of the non-worked buckets: the g) group, d),
/// the đ) group and e).
fn derive_totals(minuti: &mut WorkTimeMinutes) {
    minuti.ukupno_ostvareni_minuta = minuti.efektivno_izvrseni_minuta
        + minuti.casovi_cekanja_i_zastoja_minuta
        + minuti.obustava_rada_strajk_minuta;
    minuti.ukupno_neizvrseni_minuta = minuti.godisnji_odmor_minuta
        + minuti.praznik_odmor_minuta
        + minuti.odsustvo_uz_naknadu_minuta
        + minuti.strucno_osposobljavanje_minuta
        + minuti.sprecenost_poslodavac_minuta
        + minuti.naknada_drugi_poslodavci_minuta
        + minuti.sprecenost_rfzo_minuta
        + minuti.porodiljsko_minuta
        + minuti.neplaceno_odsustvo_minuta;
}

fn validate_cap_override(request: &SaveEntryRequest) -> Result<(), AppError> {
    let Some(razlog) = request.cap_override_razlog.as_deref() else {
        return Ok(());
    };
    if !CAP_OVERRIDE_RAZLOZI.contains(&razlog) {
        return Err(AppError::validation(
            "Nepoznat razlog prekoračenja limita.",
            serde_json::json!({ "capOverrideRazlog": razlog }),
        ));
    }
    // v17 enforces the same rule: a čl. 53 cap override is only meaningful on a
    // worked day, so the command layer must not hand the DB a row it will reject.
    if request.kategorija_odsustva.is_some() {
        return Err(AppError::validation(
            "Razlog prekoračenja limita se ne evidentira na danu odsustva.",
            serde_json::json!({ "capOverrideRazlog": razlog }),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_entry(
    connection: &Connection,
    request: &SaveEntryRequest,
    minuti: &WorkTimeMinutes,
    verzija: i64,
    supersedes_id: Option<i64>,
    korekcija_razlog: Option<&str>,
    acting_id: i64,
    now: &str,
) -> Result<i64, AppError> {
    let mut columns: Vec<&str> = vec![
        "user_id",
        "dan",
        "verzija",
        "kategorija_odsustva",
        "cap_override_razlog",
        "supersedes_id",
        "korekcija_razlog",
        "unio_user_id",
        "created_at",
        "updated_at",
    ];
    columns.extend_from_slice(WorkTimeMinutes::COLUMNS);

    let mut values: Vec<Value> = vec![
        Value::Integer(request.user_id),
        Value::Text(request.dan.clone()),
        Value::Integer(verzija),
        optional_text(request.kategorija_odsustva.as_deref()),
        optional_text(request.cap_override_razlog.as_deref()),
        supersedes_id.map_or(Value::Null, Value::Integer),
        optional_text(korekcija_razlog),
        Value::Integer(acting_id),
        Value::Text(now.to_string()),
        Value::Text(now.to_string()),
    ];
    values.extend(minuti.values().into_iter().map(Value::Integer));

    let placeholders = (1..=columns.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "INSERT INTO work_time_entries ({}) VALUES ({placeholders})",
        columns.join(", ")
    );
    connection.execute(&sql, params_from_iter(values))?;
    Ok(connection.last_insert_rowid())
}

fn optional_text(value: Option<&str>) -> Value {
    value.map_or(Value::Null, |text| Value::Text(text.to_string()))
}

fn load_month(
    state: &AppState,
    user_id: i64,
    godina: i64,
    mesec: i64,
) -> Result<WorkTimeMonth, AppError> {
    validate_month(godina, mesec)?;
    let connection = state.db().open()?;
    let zaposleni = employee_name(&connection, user_id)?;
    let (first, last) = month_bounds(godina, mesec);
    let entries = load_entries(
        &connection,
        "WHERE e.user_id = ?1 AND e.dan >= ?2 AND e.dan <= ?3",
        params![user_id, first, last],
    )?;

    let mut ukupno = WorkTimeMinutes::default();
    for entry in entries.iter().filter(|entry| !entry.zamenjen) {
        ukupno.accumulate(&entry.minuti);
    }

    let period: Option<(String, Option<String>)> = connection
        .query_row(
            "SELECT status, closed_at FROM work_time_periods
             WHERE user_id = ?1 AND godina = ?2 AND mesec = ?3",
            params![user_id, godina, mesec],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (zatvoren, closed_at) = match period {
        Some((status, closed_at)) => (status == "closed", closed_at),
        None => (false, None),
    };

    Ok(WorkTimeMonth {
        user_id,
        zaposleni,
        godina,
        mesec,
        zatvoren,
        closed_at,
        entries,
        ukupno,
        napomena: EVIDENCIJA_ZAGLAVLJE.to_string(),
        advisory_napomena: ADVISORY_TAG.to_string(),
    })
}

/// Reads entries under `predicate`, marking every row that a later `verzija`
/// supersedes. `zamenjen` is computed from `MAX(verzija)` per `(user_id, dan)`,
/// never from `supersedes_id IS NULL` — v17's index is total precisely so that a
/// forked chain cannot leave two rows live for one day.
fn load_entries(
    connection: &Connection,
    predicate: &str,
    params: impl rusqlite::Params,
) -> Result<Vec<WorkTimeEntryView>, AppError> {
    let minute_columns = WorkTimeMinutes::COLUMNS
        .iter()
        .map(|column| format!("e.{column} AS {column}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "SELECT e.id AS id, e.user_id AS user_id, e.dan AS dan, e.verzija AS verzija,
                e.supersedes_id AS supersedes_id, e.kategorija_odsustva AS kategorija_odsustva,
                e.cap_override_razlog AS cap_override_razlog,
                e.korekcija_razlog AS korekcija_razlog, e.unio_user_id AS unio_user_id,
                u.display_name AS unio_ime, e.created_at AS created_at,
                e.updated_at AS updated_at,
                (SELECT MAX(x.verzija) FROM work_time_entries x
                  WHERE x.user_id = e.user_id AND x.dan = e.dan) AS max_verzija,
                {minute_columns}
           FROM work_time_entries e
           LEFT JOIN users u ON u.id = e.unio_user_id
           {predicate}
          ORDER BY e.dan, e.verzija"
    );

    let mut statement = connection.prepare(&sql)?;
    let rows = statement.query_map(params, |row| {
        let verzija: i64 = row.get("verzija")?;
        let max_verzija: i64 = row.get("max_verzija")?;
        Ok(WorkTimeEntryView {
            id: row.get("id")?,
            user_id: row.get("user_id")?,
            dan: row.get("dan")?,
            verzija,
            zamenjen: verzija < max_verzija,
            supersedes_id: row.get("supersedes_id")?,
            kategorija_odsustva: row.get("kategorija_odsustva")?,
            cap_override_razlog: row.get("cap_override_razlog")?,
            korekcija_razlog: row.get("korekcija_razlog")?,
            unio_user_id: row.get("unio_user_id")?,
            unio_ime: row.get("unio_ime")?,
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
            minuti: WorkTimeMinutes::from_row(row)?,
        })
    })?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// The live row for one day, or `None`.
fn live_entry(
    connection: &Connection,
    user_id: i64,
    dan: &str,
) -> Result<Option<WorkTimeEntryView>, AppError> {
    let mut entries = load_entries(
        connection,
        "WHERE e.user_id = ?1 AND e.dan = ?2 AND e.verzija =
             (SELECT MAX(x.verzija) FROM work_time_entries x
               WHERE x.user_id = e.user_id AND x.dan = e.dan)",
        params![user_id, dan],
    )?;
    Ok(entries.pop())
}

/// The employee's live rows within seven days either side of `dan`.
///
/// Live rows only — `assess_caps` documents this as the caller's obligation: v17
/// keeps every correction as its own row, so a plain range query also returns
/// superseded versions whose `prekovremeni_minuta` would sum straight into the
/// weekly bucket and manufacture a čl. 53 st. 2 breach out of lawful hours. The
/// window is deliberately wider than a week; `in_same_iso_week` does the filtering.
fn load_week(connection: &Connection, user_id: i64, dan: &str) -> Result<Vec<DayHours>, AppError> {
    let Some(date) = parse_iso_date(dan) else {
        return Ok(Vec::new());
    };
    let Ok(first) = time::Date::from_julian_day(date.to_julian_day() - 7) else {
        return Ok(Vec::new());
    };
    let Ok(last) = time::Date::from_julian_day(date.to_julian_day() + 7) else {
        return Ok(Vec::new());
    };

    let mut statement = connection.prepare(
        "SELECT e.dan, e.efektivno_izvrseni_minuta, e.prekovremeni_minuta
           FROM work_time_entries e
          WHERE e.user_id = ?1 AND e.dan >= ?2 AND e.dan <= ?3
            AND e.verzija = (SELECT MAX(x.verzija) FROM work_time_entries x
                              WHERE x.user_id = e.user_id AND x.dan = e.dan)",
    )?;
    let rows = statement.query_map(params![user_id, iso_date(first), iso_date(last)], |row| {
        Ok(DayHours {
            dan: row.get(0)?,
            efektivno_minuta: row.get(1)?,
            prekovremeni_minuta: row.get(2)?,
        })
    })?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// The čl. 87–91 facts, mirrored from the v17 `users` columns.
///
/// `saglasnost_preraspodela_od` (čl. 57 st. 4) is deliberately not read here:
/// `EmployeeProtection` has no field for it, and the two written consents are
/// legally distinct and not interchangeable.
fn load_protection(connection: &Connection, user_id: i64) -> Result<EmployeeProtection, AppError> {
    connection
        .query_row(
            "SELECT datum_rodjenja, datum_rodjenja_najmladjeg_deteta, samohrani_roditelj,
                    dete_tezak_invalid, trudnoca_ili_dojenje, saglasnost_prekovremeni_od,
                    radi_u_preraspodeli
               FROM users WHERE id = ?1",
            params![user_id],
            |row| {
                Ok(EmployeeProtection {
                    datum_rodjenja: row.get(0)?,
                    datum_rodjenja_najmladjeg_deteta: row.get(1)?,
                    samohrani_roditelj: row.get::<_, Option<i64>>(2)?.map(|flag| flag != 0),
                    dete_tezak_invalid: row.get::<_, Option<i64>>(3)?.map(|flag| flag != 0),
                    trudnoca_ili_dojenje: row.get::<_, Option<i64>>(4)?.map(|flag| flag != 0),
                    saglasnost_prekovremeni_od: row.get(5)?,
                    radi_u_preraspodeli: row.get::<_, i64>(6)? != 0,
                })
            },
        )
        .map_err(Into::into)
}

fn employee_name(connection: &Connection, user_id: i64) -> Result<String, AppError> {
    connection
        .query_row(
            "SELECT display_name FROM users WHERE id = ?1",
            params![user_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("Zaposleni nije pronađen."))
}

fn ensure_employee_exists(connection: &Connection, user_id: i64) -> Result<(), AppError> {
    employee_name(connection, user_id).map(|_| ())
}

fn period_is_closed(
    connection: &Connection,
    user_id: i64,
    godina: i64,
    mesec: i64,
) -> Result<bool, AppError> {
    let status: Option<String> = connection
        .query_row(
            "SELECT status FROM work_time_periods
              WHERE user_id = ?1 AND godina = ?2 AND mesec = ?3",
            params![user_id, godina, mesec],
            |row| row.get(0),
        )
        .optional()?;
    Ok(status.as_deref() == Some("closed"))
}

/// „Dnevnu“ in čl. 55 st. 6 is what makes a reconstructed month look like a
/// fabrication, so a closed month is closed for writes on every ground.
fn period_closed_error(godina: i64, mesec: i64) -> AppError {
    AppError::business_with_details(
        "period_closed",
        format!(
            "Period {mesec:02}/{godina} je zaključen i više se ne može menjati. \
             Zaključenje je konačno."
        ),
        serde_json::json!({ "godina": godina, "mesec": mesec }),
    )
}

fn validate_month(godina: i64, mesec: i64) -> Result<(), AppError> {
    if !(1..=12).contains(&mesec) {
        return Err(AppError::validation(
            "Mesec mora biti između 1 i 12.",
            serde_json::json!({ "mesec": mesec }),
        ));
    }
    if !(1900..=9999).contains(&godina) {
        return Err(AppError::validation(
            "Godina nije ispravna.",
            serde_json::json!({ "godina": godina }),
        ));
    }
    Ok(())
}

/// Refuses to close a month that has not ended yet.
///
/// The close is irreversible — there is no reopen command, by design — and a
/// closed month refuses every write with `period_closed`. Closing August on the
/// first of August would therefore make the remaining thirty days permanently
/// unrecordable and manufacture exactly the „ne vodi dnevnu evidenciju“ exposure
/// (ZoR čl. 276 st. 1, uvodna rečenica, u vezi sa tač. 1a) that this module exists
/// to remove, with no in-app remedy. §4 req. 19 phrases the transition as
/// irreversible *without an audited unlock*; this build has the irreversibility
/// and not the unlock, so the guard sits at the entry instead.
///
/// The month's own last civil day decides, not a fixed 31st: February 2026 ends on
/// the 28th. An unreadable `now` refuses the close — a transition that cannot be
/// undone is not one to take on a guess.
fn guard_period_has_ended(godina: i64, mesec: i64, now: &str) -> Result<(), AppError> {
    let ended = match (month_last_date(godina, mesec), civil_date(now)) {
        (Some(last), Some(danas)) => last < danas,
        _ => false,
    };
    if ended {
        return Ok(());
    }
    Err(AppError::validation(
        format!(
            "Period {mesec:02}/{godina} još nije završen, pa se ne može zaključiti. \
             Zaključenje je konačno i posle njega se ni jedan dan tog meseca više ne \
             može evidentirati. Zaključite ga najranije prvog dana narednog meseca."
        ),
        serde_json::json!({ "godina": godina, "mesec": mesec }),
    ))
}

/// Refuses to record a day that has not happened yet.
///
/// `close_period` has [`guard_period_has_ended`]; the write path needs the same
/// guard one day at a time. A row carrying hours nobody has worked is a forecast,
/// and a forecast is not the „dnevna evidencija“ ZoR čl. 55 st. 6 asks for — it is
/// the thing that makes the register look invented, which is the exposure the
/// module exists to remove. §4 req. 6 („Never back-date“) and §4 req. 17
/// (contemporaneous write, late entries marked rather than moved) both describe a
/// record whose rows are days that actually happened; forward-dating is the same
/// defect facing the other way, and ZEOR čl. 46 st. 1 puts the accuracy of the
/// entry on the record-keeper either way.
///
/// It is the append-only design that makes this a refusal rather than a warning:
/// there is no delete, so a mistyped 2028 lands a permanent row that can only be
/// superseded — a correction log entry about a day that never existed.
///
/// The day **in progress** stays writable. čl. 55 st. 6 wants the record kept
/// daily and §4 req. 17 wants it contemporaneous, so the boundary is the calendar
/// day of `now`, not the day before it.
///
/// The decision reads the `now` already threaded through the write path rather
/// than a wall clock of its own, so a caller can state the day. An unreadable
/// `now` refuses: the row is permanent and would carry that same unreadable stamp
/// as its `created_at`, so there is nothing to fall back on.
fn guard_day_has_happened(dan: &str, datum: time::Date, now: &str) -> Result<(), AppError> {
    let Some(danas) = civil_date(now) else {
        return Err(AppError::validation(
            "Sistemski sat nije čitljiv, pa se ne može utvrditi da li je dan protekao. \
             Unos nije sačuvan.",
            serde_json::json!({ "dan": dan, "sada": now }),
        ));
    };
    if datum > danas {
        return Err(AppError::validation(
            "Ne može se evidentirati dan koji još nije protekao. \
             Evidentira se dan koji se dogodio.",
            serde_json::json!({ "dan": dan, "danas": danas.to_string() }),
        ));
    }
    Ok(())
}

/// Refuses a day that does not belong to the period the write was made for.
///
/// The day alone cannot catch this: `2026-09-01` is a perfectly good date, and
/// every other check passes it. What is wrong is the *mismatch* — the operator
/// had avgust on screen and typed a septembar day — and the request has to state
/// the period before the mismatch is visible at all. That is what `godina` and
/// `mesec` on [`SaveEntryRequest`] are for; they are an assertion about the write,
/// never the source of the row's own month, which stays derived from `dan`.
///
/// It matters because the row is not merely misfiled. `list_month` filters by
/// period, so the operator is told „Dan je evidentiran“ about a row that is not on
/// the screen they are looking at and will not be found by looking harder. In an
/// append-only log it cannot be withdrawn, only superseded — from a month they
/// have to know to open. It also slips the closed-period freeze they *would* have
/// hit: the check runs on the day's own month, so a stray day walks into a
/// neighbouring month that may be open when the one on screen is shut.
///
/// The frontend refuses the same thing at the Datum field (`validateDan`), which
/// is where an operator should meet it. This is the guard for everything that is
/// not that field.
fn guard_day_is_in_period(
    dan: &str,
    godina: i64,
    mesec: i64,
    trazena_godina: i64,
    trazeni_mesec: i64,
) -> Result<(), AppError> {
    if godina == trazena_godina && mesec == trazeni_mesec {
        return Ok(());
    }
    Err(AppError::validation(
        format!("Datum mora pripadati izabranom periodu — {trazeni_mesec:02}/{trazena_godina}."),
        serde_json::json!({
            "dan": dan,
            "godina": trazena_godina,
            "mesec": trazeni_mesec,
        }),
    ))
}

/// The civil date of an RFC3339 timestamp — its first ten characters.
fn civil_date(now: &str) -> Option<time::Date> {
    parse_iso_date(now.get(..10)?)
}

/// The last civil day of a month, via the first day of the next one.
fn month_last_date(godina: i64, mesec: i64) -> Option<time::Date> {
    let (godina, mesec) = if mesec == 12 {
        (godina.checked_add(1)?, 1)
    } else {
        (godina, mesec.checked_add(1)?)
    };
    let first_of_next = time::Date::from_calendar_date(
        i32::try_from(godina).ok()?,
        time::Month::try_from(u8::try_from(mesec).ok()?).ok()?,
        1,
    )
    .ok()?;
    time::Date::from_julian_day(first_of_next.to_julian_day() - 1).ok()
}

/// The first and last civil dates of a month, as ISO strings. The 31st is a safe
/// upper bound for every month because `dan` is a fixed-width ISO date and sorts
/// lexicographically as a calendar date.
fn month_bounds(godina: i64, mesec: i64) -> (String, String) {
    (
        format!("{godina:04}-{mesec:02}-01"),
        format!("{godina:04}-{mesec:02}-31"),
    )
}

fn iso_date(date: time::Date) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        date.year(),
        u8::from(date.month()),
        date.day()
    )
}

/// Renders the month as CSV, columns 1:1 onto ZEOR čl. 24 tač. 1 (§4 req. 21).
///
/// The header carries the req. 22 sentence verbatim. Nothing here is called a
/// propisani obrazac, and nothing claims to be a propisana evidencija o zaradama
/// — čl. 55 st. 6 prescribes no form, and the ZEOR wage record is the
/// accountant's, on prescribed methodology this file does not touch.
fn month_to_csv(month: &WorkTimeMonth, live: &[&WorkTimeEntryView]) -> String {
    let mut lines = vec![
        csv_line(&[EVIDENCIJA_ZAGLAVLJE]),
        csv_line(&["Zaposleni", month.zaposleni.as_str()]),
        csv_line(&["Period", &format!("{:02}/{}", month.mesec, month.godina)]),
        csv_line(&[
            "Status perioda",
            if month.zatvoren {
                "Zaključen"
            } else {
                "Otvoren"
            },
        ]),
        csv_line(&["Jedinica", "Minuti"]),
        String::new(),
    ];

    let mut header = vec!["Datum".to_string(), "Kategorija odsustva".to_string()];
    header.extend(
        WorkTimeMinutes::LABELS
            .iter()
            .map(|label| label.to_string()),
    );
    lines.push(csv_line(
        &header.iter().map(String::as_str).collect::<Vec<_>>(),
    ));

    for entry in live {
        let mut fields = vec![
            entry.dan.clone(),
            entry.kategorija_odsustva.clone().unwrap_or_default(),
        ];
        fields.extend(entry.minuti.values().iter().map(i64::to_string));
        lines.push(csv_line(
            &fields.iter().map(String::as_str).collect::<Vec<_>>(),
        ));
    }

    let mut totals = vec!["Ukupno".to_string(), String::new()];
    totals.extend(month.ukupno.values().iter().map(i64::to_string));
    lines.push(csv_line(
        &totals.iter().map(String::as_str).collect::<Vec<_>>(),
    ));

    lines.push(String::new());
    lines.push(csv_line(&["Napomena", EVIDENCIJA_ZAGLAVLJE]));
    lines.push(csv_line(&[
        "Napomena",
        &format!(
            "Noćni časovi i časovi rada na praznik su {ADVISORY_TAG}, \
             a ne zakonom propisano polje."
        ),
    ]));
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use tauri::Manager;

    use super::{
        close_period, correct_entry, export_month_csv, list_month, my_hours, notices,
        parse_iso_date, save_entry, worktime_save_entry, CorrectEntryRequest, SaveEntryRequest,
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

    fn seed_employee(state: &AppState, username: &str, display_name: &str) -> i64 {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, active, created_at, updated_at)
                 VALUES (?1, ?2, 'cashier', 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![username, display_name],
            )
            .expect("employee should insert");
        connection.last_insert_rowid()
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

    fn sign_in(state: &AppState, user_id: i64) {
        state
            .set_session_user_id(user_id)
            .expect("session should set");
    }

    fn set_datum_rodjenja(state: &AppState, user_id: i64, datum: &str) {
        state
            .db()
            .open()
            .expect("database should open")
            .execute(
                "UPDATE users SET datum_rodjenja = ?1 WHERE id = ?2",
                params![datum, user_id],
            )
            .expect("datum rođenja should update");
    }

    fn set_preraspodela(state: &AppState, user_id: i64) {
        state
            .db()
            .open()
            .expect("database should open")
            .execute(
                "UPDATE users SET radi_u_preraspodeli = 1 WHERE id = ?1",
                params![user_id],
            )
            .expect("preraspodela flag should update");
    }

    /// One ordinary worked day, stated for the period that day belongs to — the
    /// normal case. A test that wants a mismatch overwrites the two fields.
    /// Minutes, never floating point.
    fn radni_dan(user_id: i64, dan: &str, efektivno: i64, prekovremeni: i64) -> SaveEntryRequest {
        let datum = parse_iso_date(dan).expect("a test day should be an ISO date");
        SaveEntryRequest {
            user_id,
            dan: dan.to_string(),
            godina: i64::from(datum.year()),
            mesec: i64::from(u8::from(datum.month())),
            moguci_minuta: 480,
            efektivno_izvrseni_minuta: efektivno,
            casovi_cekanja_i_zastoja_minuta: 0,
            prekovremeni_minuta: prekovremeni,
            nocni_minuta: 0,
            rad_na_praznik_minuta: 0,
            kategorija_odsustva: None,
            odsustvo_minuta: 0,
            cap_override_razlog: None,
        }
    }

    #[test]
    fn saving_an_entry_records_the_operator_and_the_day() {
        with_state("worktime_saving_records_operator_and_day", |state| {
            let admin_id = sign_in_admin(state);
            let radnik = seed_employee(state, "radnik1", "Radnik Jedan");

            let saved = save_entry(
                state,
                radni_dan(radnik, "2026-08-03", 480, 0),
                "2026-08-03T18:00:00Z",
            )
            .expect("the day should record");

            assert_eq!(saved.entry.user_id, radnik);
            assert_eq!(saved.entry.dan, "2026-08-03");
            assert_eq!(saved.entry.verzija, 1);
            assert_eq!(saved.entry.supersedes_id, None);
            assert!(!saved.entry.zamenjen);
            // Who wrote it and when — ZEOR čl. 46 st. 1 puts accuracy on the
            // record-keeper, so the row must name one.
            assert_eq!(saved.entry.unio_user_id, Some(admin_id));
            assert_eq!(saved.entry.created_at, "2026-08-03T18:00:00Z");
            assert_eq!(saved.entry.updated_at, "2026-08-03T18:00:00Z");
            assert_eq!(saved.entry.minuti.efektivno_izvrseni_minuta, 480);
            // ZEOR čl. 24 tač. 1 b) is derived from its own three indents.
            assert_eq!(saved.entry.minuti.ukupno_ostvareni_minuta, 480);
            assert_eq!(saved.entry.minuti.ukupno_neizvrseni_minuta, 0);

            let error = save_entry(
                state,
                radni_dan(radnik, "2026-08-03", 60, 0),
                "2026-08-03T19:00:00Z",
            )
            .expect_err("a second original for the same day must be refused");
            assert_eq!(error.code(), "entry_exists");
        });
    }

    #[test]
    fn a_correction_supersedes_rather_than_mutates() {
        with_state("worktime_correction_supersedes", |state| {
            let admin_id = sign_in_admin(state);
            let radnik = seed_employee(state, "radnik2", "Radnik Dva");

            let original = save_entry(
                state,
                radni_dan(radnik, "2026-08-03", 480, 0),
                "2026-08-03T18:00:00Z",
            )
            .expect("the original should record");

            let ispravka = correct_entry(
                state,
                CorrectEntryRequest {
                    entry: radni_dan(radnik, "2026-08-03", 420, 0),
                    korekcija_razlog: "greska_u_unosu".to_string(),
                },
                "2026-08-04T09:00:00Z",
            )
            .expect("the correction should record");

            assert_eq!(ispravka.entry.verzija, 2);
            assert_eq!(ispravka.entry.supersedes_id, Some(original.entry.id));
            assert_eq!(
                ispravka.entry.korekcija_razlog.as_deref(),
                Some("greska_u_unosu")
            );
            assert_eq!(ispravka.entry.unio_user_id, Some(admin_id));
            assert_eq!(ispravka.entry.created_at, "2026-08-04T09:00:00Z");

            let month = list_month(state, radnik, 2026, 8).expect("the month should list");
            assert_eq!(month.entries.len(), 2, "append-only keeps both versions");

            let survivor = month
                .entries
                .iter()
                .find(|entry| entry.id == original.entry.id)
                .expect("the original row must survive");
            assert_eq!(
                survivor.minuti.efektivno_izvrseni_minuta, 480,
                "the original row must not be mutated"
            );
            assert!(
                survivor.zamenjen,
                "the original is struck through, never deleted"
            );

            let live = month
                .entries
                .iter()
                .find(|entry| entry.id == ispravka.entry.id)
                .expect("the correction must be listed");
            assert!(!live.zamenjen, "MAX(verzija) is the live row");

            assert_eq!(
                month.ukupno.efektivno_izvrseni_minuta, 420,
                "only the live row counts toward the month"
            );
        });
    }

    #[test]
    fn an_entry_cannot_be_back_dated_into_a_closed_period() {
        with_state("worktime_no_back_dating_into_closed", |state| {
            sign_in_admin(state);
            let radnik = seed_employee(state, "radnik3", "Radnik Tri");

            save_entry(
                state,
                radni_dan(radnik, "2026-08-03", 480, 0),
                "2026-08-03T18:00:00Z",
            )
            .expect("the day should record while the month is open");

            close_period(state, radnik, 2026, 8, "2026-09-01T08:00:00Z")
                .expect("the month should close");

            let error = save_entry(
                state,
                radni_dan(radnik, "2026-08-05", 480, 0),
                "2026-09-02T08:00:00Z",
            )
            .expect_err("a day cannot be added to a closed month");
            assert_eq!(error.code(), "period_closed");

            let error = correct_entry(
                state,
                CorrectEntryRequest {
                    entry: radni_dan(radnik, "2026-08-03", 60, 0),
                    korekcija_razlog: "ispravka_sati".to_string(),
                },
                "2026-09-02T08:00:00Z",
            )
            .expect_err("a closed month cannot be corrected either");
            assert_eq!(error.code(), "period_closed");

            // An adjacent open month is untouched — the freeze is per period.
            save_entry(
                state,
                radni_dan(radnik, "2026-09-01", 480, 0),
                "2026-09-01T18:00:00Z",
            )
            .expect("an open month still accepts entries");
        });
    }

    #[test]
    fn closing_a_period_freezes_the_classification_and_is_irreversible() {
        with_state("worktime_close_freezes_classification", |state| {
            let admin_id = sign_in_admin(state);
            let radnik = seed_employee(state, "radnik4", "Radnik Četiri");

            save_entry(
                state,
                radni_dan(radnik, "2026-08-03", 480, 0),
                "2026-08-03T18:00:00Z",
            )
            .expect("worked day should record");

            let mut odmor = radni_dan(radnik, "2026-08-04", 0, 0);
            odmor.kategorija_odsustva = Some("godisnji_odmor".to_string());
            odmor.odsustvo_minuta = 480;
            save_entry(state, odmor, "2026-08-04T18:00:00Z").expect("absence day should record");

            let closed = close_period(state, radnik, 2026, 8, "2026-09-01T08:00:00Z")
                .expect("the month should close");

            assert_eq!(closed.closed_at, "2026-09-01T08:00:00Z");
            assert_eq!(closed.closed_by, admin_id);
            assert_eq!(closed.klasifikacija.dana_sa_unosom, 2);
            assert_eq!(closed.klasifikacija.minuti.efektivno_izvrseni_minuta, 480);
            assert_eq!(closed.klasifikacija.minuti.godisnji_odmor_minuta, 480);
            assert_eq!(closed.klasifikacija.minuti.ukupno_neizvrseni_minuta, 480);

            // Frozen means written down, not recomputed on read.
            let stored: String = state
                .db()
                .open()
                .expect("database should open")
                .query_row(
                    "SELECT klasifikacija_json FROM work_time_periods
                     WHERE user_id = ?1 AND godina = 2026 AND mesec = 8",
                    params![radnik],
                    |row| row.get(0),
                )
                .expect("the closed period should carry a frozen classification");
            let frozen: super::PeriodClassification =
                serde_json::from_str(&stored).expect("the frozen classification should parse");
            assert_eq!(frozen, closed.klasifikacija);

            let error = close_period(state, radnik, 2026, 8, "2026-09-02T08:00:00Z")
                .expect_err("a closed period must not be closed twice");
            assert_eq!(error.code(), "period_closed");

            let month = list_month(state, radnik, 2026, 8).expect("the month should list");
            assert!(month.zatvoren);
            assert_eq!(month.closed_at.as_deref(), Some("2026-09-01T08:00:00Z"));
        });
    }

    #[test]
    fn my_hours_returns_only_the_session_users_own_rows() {
        with_state("worktime_my_hours_own_rows_only", |state| {
            sign_in_admin(state);
            let prvi = seed_employee(state, "radnik5a", "Radnik Pet A");
            let drugi = seed_employee(state, "radnik5b", "Radnik Pet B");

            save_entry(
                state,
                radni_dan(prvi, "2026-08-03", 480, 0),
                "2026-08-03T18:00:00Z",
            )
            .expect("first employee's day should record");
            save_entry(
                state,
                radni_dan(drugi, "2026-08-03", 300, 0),
                "2026-08-03T18:00:00Z",
            )
            .expect("second employee's day should record");

            sign_in(state, prvi);
            let mine = my_hours(state, 2026, 8).expect("an employee may read their own hours");

            assert_eq!(mine.user_id, prvi);
            assert_eq!(mine.entries.len(), 1);
            assert!(mine.entries.iter().all(|entry| entry.user_id == prvi));
            assert_eq!(mine.ukupno.efektivno_izvrseni_minuta, 480);
            // ZoR čl. 55 st. 6 — the export and the panel say the law prescribes
            // no obrazac, and never call this a propisani obrazac.
            assert!(mine.napomena.contains("Zakon ne propisuje obrazac"));
        });
    }

    #[test]
    fn a_cashier_cannot_reach_the_admin_worktime_commands() {
        let path = test_database_path("worktime_cashier_cannot_reach_admin_commands");

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            sign_in_admin(&state);
            let radnik = seed_employee(&state, "radnik6", "Radnik Šest");
            save_entry(
                &state,
                radni_dan(radnik, "2026-08-03", 480, 0),
                "2026-08-03T18:00:00Z",
            )
            .expect("admin should record the day");
            sign_in(&state, radnik);

            let app = tauri::test::mock_builder()
                .manage(state)
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");
            let managed = app.state::<AppState>();
            let state = managed.inner();

            assert_eq!(
                list_month(state, radnik, 2026, 8)
                    .expect_err("a cashier may not read the register")
                    .code(),
                "forbidden"
            );
            assert_eq!(
                save_entry(
                    state,
                    radni_dan(radnik, "2026-08-06", 480, 0),
                    "2026-08-06T18:00:00Z"
                )
                .expect_err("a cashier may not write the register")
                .code(),
                "forbidden"
            );
            assert_eq!(
                correct_entry(
                    state,
                    CorrectEntryRequest {
                        entry: radni_dan(radnik, "2026-08-03", 60, 0),
                        korekcija_razlog: "greska_u_unosu".to_string(),
                    },
                    "2026-08-06T18:00:00Z"
                )
                .expect_err("a cashier may not correct the register")
                .code(),
                "forbidden"
            );
            assert_eq!(
                close_period(state, radnik, 2026, 8, "2026-09-01T08:00:00Z")
                    .expect_err("a cashier may not close a period")
                    .code(),
                "forbidden"
            );
            assert_eq!(
                export_month_csv(state, radnik, 2026, 8)
                    .expect_err("a cashier may not export the register")
                    .code(),
                "forbidden"
            );

            // The gate lives inside the domain function, so the command wrapper
            // cannot route around it either.
            let error = worktime_save_entry(
                app.state::<AppState>(),
                radni_dan(radnik, "2026-08-07", 480, 0),
            )
            .expect_err("the command wrapper is gated too");
            assert_eq!(error.code, "forbidden");

            // The employee's own read-only view stays open to them.
            my_hours(state, 2026, 8).expect("a cashier may read their own hours");
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    #[test]
    fn a_cap_breach_asks_for_an_override_reason_and_then_records_the_day() {
        with_state("worktime_cap_breach_asks_for_override", |state| {
            sign_in_admin(state);
            let radnik = seed_employee(state, "radnik7", "Radnik Sedam");

            // Monday: exactly eight hours of overtime and exactly twelve hours
            // total. ZoR čl. 53 st. 2 and st. 3 are the limits, not breaches.
            let na_granici = save_entry(
                state,
                radni_dan(radnik, "2026-08-03", 240, 480),
                "2026-08-03T20:00:00Z",
            )
            .expect("the boundary day needs no override");
            assert!(!na_granici.caps.weekly_cap_exceeded);
            assert!(!na_granici.caps.daily_cap_exceeded);
            assert!(!na_granici.caps.requires_override);

            // Tuesday, same calendar week: one more minute of overtime crosses
            // čl. 53 st. 2.
            let error = save_entry(
                state,
                radni_dan(radnik, "2026-08-04", 240, 60),
                "2026-08-04T20:00:00Z",
            )
            .expect_err("a cap breach must ask for a reason");
            assert_eq!(error.code(), "cap_override_required");

            let mut sa_razlogom = radni_dan(radnik, "2026-08-04", 240, 60);
            sa_razlogom.cap_override_razlog = Some("iznenadno_povecanje_obima_posla".to_string());
            let saved = save_entry(state, sa_razlogom, "2026-08-04T20:00:00Z")
                .expect("an override reason records the day rather than hiding it");

            assert!(saved.caps.weekly_cap_exceeded);
            assert_eq!(saved.caps.weekly_overtime_minutes, 540);
            assert_eq!(
                saved.entry.cap_override_razlog.as_deref(),
                Some("iznenadno_povecanje_obima_posla")
            );

            let error = save_entry(
                state,
                {
                    let mut request = radni_dan(radnik, "2026-08-05", 240, 60);
                    request.cap_override_razlog = Some("nesto_izmisljeno".to_string());
                    request
                },
                "2026-08-05T20:00:00Z",
            )
            .expect_err("the override reason is a closed enum");
            assert_eq!(error.code(), "validation_error");
        });
    }

    #[test]
    fn a_blocking_protection_refuses_the_write() {
        with_state("worktime_blocking_protection_refuses", |state| {
            sign_in_admin(state);
            let maloletnik = seed_employee(state, "radnik8", "Radnik Osam");
            // Seventeen on the day being recorded.
            set_datum_rodjenja(state, maloletnik, "2009-01-15");

            let error = save_entry(
                state,
                radni_dan(maloletnik, "2026-08-03", 240, 60),
                "2026-08-03T20:00:00Z",
            )
            .expect_err("ZoR čl. 88 st. 1 bans overtime of an employee under 18");
            assert_eq!(error.code(), "protection_block");

            // The plain day is still recordable — the ban is on the overtime,
            // and a register that refuses lawful days hides exposure.
            let saved = save_entry(
                state,
                radni_dan(maloletnik, "2026-08-03", 240, 0),
                "2026-08-03T20:00:00Z",
            )
            .expect("a plain day of a minor records normally");
            assert!(saved.protections.is_empty());

            let error = correct_entry(
                state,
                CorrectEntryRequest {
                    entry: radni_dan(maloletnik, "2026-08-03", 240, 60),
                    korekcija_razlog: "ispravka_sati".to_string(),
                },
                "2026-08-04T09:00:00Z",
            )
            .expect_err("a correction cannot route around the guard");
            assert_eq!(error.code(), "protection_block");
        });
    }

    #[test]
    fn absence_minutes_land_in_the_bucket_derived_from_the_category() {
        with_state("worktime_absence_bucket_derived", |state| {
            sign_in_admin(state);
            let radnik = seed_employee(state, "radnik9", "Radnik Devet");

            let mut rfzo = radni_dan(radnik, "2026-08-03", 0, 0);
            rfzo.kategorija_odsustva = Some("sprecenost_rfzo".to_string());
            rfzo.odsustvo_minuta = 480;
            let saved = save_entry(state, rfzo, "2026-08-03T18:00:00Z")
                .expect("the RFZO-funded absence should record");
            assert_eq!(saved.entry.minuti.sprecenost_rfzo_minuta, 480);
            assert_eq!(saved.entry.minuti.sprecenost_poslodavac_minuta, 0);
            // ZEOR čl. 24 tač. 1 v) — the total of the non-worked buckets.
            assert_eq!(saved.entry.minuti.ukupno_neizvrseni_minuta, 480);
            assert_eq!(saved.entry.minuti.ukupno_ostvareni_minuta, 0);

            // Štrajk is the third indent of b), not a v) bucket.
            let mut strajk = radni_dan(radnik, "2026-08-04", 0, 0);
            strajk.kategorija_odsustva = Some("obustava_rada_strajk".to_string());
            strajk.odsustvo_minuta = 300;
            let saved = save_entry(state, strajk, "2026-08-04T18:00:00Z")
                .expect("the strike hours should record");
            assert_eq!(saved.entry.minuti.obustava_rada_strajk_minuta, 300);
            assert_eq!(saved.entry.minuti.ukupno_ostvareni_minuta, 300);
            assert_eq!(saved.entry.minuti.ukupno_neizvrseni_minuta, 0);

            let mut izmisljeno = radni_dan(radnik, "2026-08-05", 0, 0);
            izmisljeno.kategorija_odsustva = Some("bolovanje_zbog_gripa".to_string());
            izmisljeno.odsustvo_minuta = 480;
            let error = save_entry(state, izmisljeno, "2026-08-05T18:00:00Z")
                .expect_err("the absence category is a closed enum");
            assert_eq!(error.code(), "validation_error");

            // A cap override belongs to a worked day only — v17 enforces it and
            // the command layer must not hand the DB a row it will reject.
            let mut odmor = radni_dan(radnik, "2026-08-06", 0, 0);
            odmor.kategorija_odsustva = Some("godisnji_odmor".to_string());
            odmor.odsustvo_minuta = 480;
            odmor.cap_override_razlog = Some("visa_sila".to_string());
            let error = save_entry(state, odmor, "2026-08-06T18:00:00Z")
                .expect_err("an absence day carries no cap override");
            assert_eq!(error.code(), "validation_error");
        });
    }

    /// The čl. 53 st. 3 daily gate is branched off for preraspodela, but the
    /// twelve-hour *figure* is not deleted: v17 has a single
    /// `radi_u_preraspodeli` flag and it cannot tell čl. 57 preraspodela — which
    /// has no daily leg — from the čl. 56 st. 3 monthly-average scheme, where
    /// čl. 56 st. 4 expressly restates 12 časova dnevno. Erasing the finding on a
    /// flag that conflates the two would hide a cap that may well apply.
    #[test]
    fn a_preraspodela_day_reports_the_daily_figure_without_gating_on_it() {
        with_state("worktime_preraspodela_daily_cap_branch", |state| {
            sign_in_admin(state);
            let redovni = seed_employee(state, "radnik10a", "Radnik Deset A");
            let u_preraspodeli = seed_employee(state, "radnik10b", "Radnik Deset B");
            set_preraspodela(state, u_preraspodeli);

            // Thirteen hours on one day. ZoR čl. 53 st. 3 catches it …
            let error = save_entry(
                state,
                radni_dan(redovni, "2026-08-03", 780, 0),
                "2026-08-03T22:00:00Z",
            )
            .expect_err("thirteen hours breaches čl. 53 st. 3");
            assert_eq!(error.code(), "cap_override_required");

            // … but čl. 58 keeps preraspodela out of it, and čl. 57 has no daily
            // leg at all, so the same day is not gated on the twelve hours.
            let saved = save_entry(
                state,
                radni_dan(u_preraspodeli, "2026-08-03", 780, 0),
                "2026-08-03T22:00:00Z",
            )
            .expect("a preraspodela day is not gated by čl. 53 st. 3");
            assert!(!saved.caps.requires_override);
            assert_eq!(saved.entry.cap_override_razlog, None);

            // The čl. 56 st. 4 case is not silently lost: the figure stays on the
            // assessment as a non-blocking finding.
            assert_eq!(saved.caps.daily_total_minutes, 780);
            assert!(
                saved.caps.daily_cap_exceeded,
                "the twelve-hour figure must stay visible — čl. 56 st. 4 restates \
                 it expressly and the flag cannot tell the two schemes apart"
            );
        });
    }

    /// čl. 58 keeps preraspodela out of the overtime derivation, so such an
    /// employee records `prekovremeni_minuta = 0` and the čl. 53 st. 2 weekly leg
    /// can never fire for them. Suppress the čl. 53 st. 3 daily gate as well and
    /// they are left with no total-hours ceiling of any kind — seven thirteen-hour
    /// days, 91 hours in one calendar week, would all save with nothing recorded
    /// on the row. čl. 57 st. 5 is the ceiling that replaces the daily one:
    /// „radno vreme ne može da traje duže od 60 časova nedeljno“, and breaching
    /// preraspodela is the čl. 274 st. 1 tač. 4 exposure, the bigger fine.
    #[test]
    fn a_preraspodela_week_over_sixty_hours_still_demands_a_ground() {
        with_state("worktime_preraspodela_weekly_ceiling", |state| {
            sign_in_admin(state);
            let radnik = seed_employee(state, "radnik12", "Radnik Dvanaest");
            set_preraspodela(state, radnik);

            // Monday to Thursday, thirteen hours a day: 52 h, inside čl. 57 st. 5.
            for dan in ["2026-08-03", "2026-08-04", "2026-08-05", "2026-08-06"] {
                let saved = save_entry(
                    state,
                    radni_dan(radnik, dan, 780, 0),
                    &format!("{dan}T22:00:00Z"),
                )
                .expect("a preraspodela week under 60 h asks for nothing");
                assert!(
                    !saved.caps.requires_override,
                    "{dan} is inside the čl. 57 st. 5 ceiling"
                );
            }

            // Friday makes 65 h in the same calendar week.
            let error = save_entry(
                state,
                radni_dan(radnik, "2026-08-07", 780, 0),
                "2026-08-07T22:00:00Z",
            )
            .expect_err("čl. 57 st. 5 caps preraspodela at 60 časova nedeljno");
            assert_eq!(error.code(), "cap_override_required");

            // The day still has to be recordable — refusing it would hide the
            // exposure instead of surfacing it.
            let mut sa_razlogom = radni_dan(radnik, "2026-08-07", 780, 0);
            sa_razlogom.cap_override_razlog = Some("visa_sila".to_string());
            let saved = save_entry(state, sa_razlogom, "2026-08-07T22:00:00Z")
                .expect("a stated ground records the day rather than hiding it");
            assert_eq!(
                saved.caps.weekly_overtime_minutes, 0,
                "čl. 58 — preraspodela hours are not prekovremeni rad"
            );
            assert!(
                !saved.caps.weekly_cap_exceeded,
                "the čl. 53 st. 2 leg cannot be what caught this week"
            );
            assert_eq!(saved.caps.weekly_total_minutes, 3900, "65 h in minutes");
            assert!(
                saved.caps.preraspodela_weekly_cap_exceeded,
                "čl. 57 st. 5 is the leg that has to catch a 65-hour week"
            );
            assert_eq!(
                saved.entry.cap_override_razlog.as_deref(),
                Some("visa_sila")
            );

            // The next calendar week starts from zero — „nedeljno“ is the
            // calendar week, not a sliding window.
            let saved = save_entry(
                state,
                radni_dan(radnik, "2026-08-10", 780, 0),
                "2026-08-10T22:00:00Z",
            )
            .expect("a fresh calendar week carries nothing in");
            assert!(!saved.caps.requires_override);
        });
    }

    #[test]
    fn a_period_cannot_be_closed_before_it_has_ended() {
        with_state("worktime_close_only_after_the_month_ends", |state| {
            sign_in_admin(state);
            let radnik = seed_employee(state, "radnik13", "Radnik Trinaest");

            save_entry(
                state,
                radni_dan(radnik, "2026-08-03", 480, 0),
                "2026-08-03T18:00:00Z",
            )
            .expect("the day should record");

            // Closing is irreversible and there is no reopen command, so closing a
            // month that is still running would make the rest of it permanently
            // unrecordable — the čl. 276 st. 1 tač. 1a exposure this module exists
            // to remove, manufactured by one mis-click.
            let error = close_period(state, radnik, 2026, 8, "2026-08-15T08:00:00Z")
                .expect_err("a running month cannot be closed");
            assert_eq!(error.code(), "validation_error");

            // Not on its last day either — 31 August is still a day of August.
            let error = close_period(state, radnik, 2026, 8, "2026-08-31T23:59:59Z")
                .expect_err("the last day of the month is still inside it");
            assert_eq!(error.code(), "validation_error");

            // A month that has not happened at all is the sharper case.
            let error = close_period(state, radnik, 2026, 12, "2026-08-15T08:00:00Z")
                .expect_err("a future month cannot be closed");
            assert_eq!(error.code(), "validation_error");

            // The register keeps taking the rest of the month …
            save_entry(
                state,
                radni_dan(radnik, "2026-08-20", 480, 0),
                "2026-08-20T18:00:00Z",
            )
            .expect("the month is still open");

            // … and the close works from the first day after the month ends.
            close_period(state, radnik, 2026, 8, "2026-09-01T00:05:00Z")
                .expect("the month after its end closes normally");

            // The month's real length decides, not a fixed 31st: February ends on
            // the 28th in 2026 and is closable on 1 March.
            let error = close_period(state, radnik, 2026, 2, "2026-02-28T23:00:00Z")
                .expect_err("28 February is still inside February");
            assert_eq!(error.code(), "validation_error");
            close_period(state, radnik, 2026, 2, "2026-03-01T09:00:00Z")
                .expect("February closes once March starts");
        });
    }

    #[test]
    fn a_day_that_has_not_happened_cannot_be_recorded() {
        with_state("worktime_no_future_day", |state| {
            sign_in_admin(state);
            let radnik = seed_employee(state, "radnik14", "Radnik Četrnaest");

            // Hours nobody has worked yet are a forecast, not a „dnevna evidencija“
            // within ZoR čl. 55 st. 6 — and the log is append-only, so the row can
            // only ever be superseded, never withdrawn.
            let error = save_entry(
                state,
                radni_dan(radnik, "2028-03-04", 480, 0),
                "2026-08-03T18:00:00Z",
            )
            .expect_err("a day two years out cannot be recorded");
            assert_eq!(error.code(), "validation_error");
            // Same sentence the Datum field gives, so the two guards cannot drift.
            assert_eq!(
                error.to_string(),
                "Ne može se evidentirati dan koji još nije protekao. \
                 Evidentira se dan koji se dogodio."
            );

            // Tomorrow is the case that actually happens — a mistyped day.
            let error = save_entry(
                state,
                radni_dan(radnik, "2026-08-04", 480, 0),
                "2026-08-03T23:59:59Z",
            )
            .expect_err("tomorrow has not happened either");
            assert_eq!(error.code(), "validation_error");

            // The day in progress stays recordable: §4 req. 17 asks for a
            // contemporaneous write, so the guard stops at the calendar day of
            // `now` and not one day short of it.
            save_entry(
                state,
                radni_dan(radnik, "2026-08-03", 480, 0),
                "2026-08-03T08:00:00Z",
            )
            .expect("the current day records normally");

            // The guard sits ahead of the live-row lookup, so a correction pointed
            // at a future day is told why rather than „nema unosa za taj dan“.
            let error = correct_entry(
                state,
                CorrectEntryRequest {
                    entry: radni_dan(radnik, "2028-03-04", 60, 0),
                    korekcija_razlog: "ispravka_sati".to_string(),
                },
                "2026-08-03T18:00:00Z",
            )
            .expect_err("a correction cannot reach a future day either");
            assert_eq!(error.code(), "validation_error");

            // An unreadable clock refuses the write instead of guessing — the row
            // is permanent and its own `created_at` would carry the same bad stamp.
            let error = save_entry(state, radni_dan(radnik, "2026-08-05", 480, 0), "")
                .expect_err("a write cannot be dated from an unreadable clock");
            assert_eq!(error.code(), "validation_error");
        });
    }

    #[test]
    fn a_day_outside_the_stated_period_is_refused() {
        with_state("worktime_day_outside_period", |state| {
            sign_in_admin(state);
            let radnik = seed_employee(state, "radnik15", "Radnik Petnaest");

            // The operator had avgust on screen and typed a septembar day. Nothing
            // in the day itself is wrong, so every other check passes it: the row
            // lands in a month nobody was looking at and vanishes from the grid,
            // because `list_month` filters by period. In an append-only log that
            // row cannot be taken back — only superseded, from a screen the
            // operator has to know to open.
            let mut mimo_perioda = radni_dan(radnik, "2026-09-01", 480, 0);
            mimo_perioda.godina = 2026;
            mimo_perioda.mesec = 8;
            let error = save_entry(state, mimo_perioda, "2026-10-01T08:00:00Z")
                .expect_err("a day outside the stated period cannot be recorded");
            assert_eq!(error.code(), "validation_error");
            assert_eq!(
                error.to_string(),
                "Datum mora pripadati izabranom periodu — 08/2026."
            );

            // A correction is the same write path and gets the same refusal, ahead
            // of the live-row lookup that would otherwise report „nema unosa“.
            let mut ispravka = radni_dan(radnik, "2026-09-01", 60, 0);
            ispravka.godina = 2026;
            ispravka.mesec = 8;
            let error = correct_entry(
                state,
                CorrectEntryRequest {
                    entry: ispravka,
                    korekcija_razlog: "ispravka_sati".to_string(),
                },
                "2026-10-01T08:00:00Z",
            )
            .expect_err("a correction cannot reach outside the stated period either");
            assert_eq!(error.code(), "validation_error");

            // The last day of the month is inside it — an off-by-one here would
            // refuse a day the register must be able to describe.
            save_entry(
                state,
                radni_dan(radnik, "2026-08-31", 480, 0),
                "2026-10-01T08:00:00Z",
            )
            .expect("the last day of the stated month records normally");

            // A stated period that is not a month says so, rather than reporting a
            // mismatch against „13/2026“.
            let mut nepostojeci = radni_dan(radnik, "2026-08-30", 480, 0);
            nepostojeci.mesec = 13;
            let error = save_entry(state, nepostojeci, "2026-10-01T08:00:00Z")
                .expect_err("month 13 is not a period");
            assert_eq!(error.code(), "validation_error");
            assert_eq!(error.to_string(), "Mesec mora biti između 1 i 12.");
        });
    }

    #[test]
    fn the_export_never_calls_itself_an_obrazac() {
        with_state("worktime_export_names_no_obrazac", |state| {
            sign_in_admin(state);
            let radnik = seed_employee(state, "radnik11", "Radnik Jedanaest");

            save_entry(
                state,
                radni_dan(radnik, "2026-08-03", 480, 0),
                "2026-08-03T18:00:00Z",
            )
            .expect("the day should record");
            correct_entry(
                state,
                CorrectEntryRequest {
                    entry: radni_dan(radnik, "2026-08-03", 420, 0),
                    korekcija_razlog: "ispravka_sati".to_string(),
                },
                "2026-08-04T09:00:00Z",
            )
            .expect("the correction should record");

            let exported =
                export_month_csv(state, radnik, 2026, 8).expect("the month should export");
            let csv =
                std::fs::read_to_string(&exported.path).expect("the export should be on disk");
            std::fs::remove_file(&exported.path).expect("the export should be removable");

            assert_eq!(exported.row_count, 1, "only live rows are exported");
            assert!(csv.contains("Evidencija prekovremenog rada — ZoR čl. 55 st. 6."));
            assert!(csv.contains("Zakon ne propisuje obrazac."));
            assert!(
                !csv.to_lowercase().contains("propisani obrazac"),
                "the export must never claim to be a prescribed form"
            );
            assert!(
                !csv.contains("evidencija o zaradama"),
                "the export is the ZoR record, never the ZEOR wage record"
            );
            // Advisory columns are labelled, never presented as statutory fields.
            assert!(csv.contains("izračunato radi provere usklađenosti"));
            assert!(csv.contains("420"), "the live version is the exported one");
        });
    }

    /// The register's UI never composes a fine figure; it renders what this
    /// returns. So the tier has to be resolved from the **stored** legal form,
    /// and an unset form has to yield no figure at all rather than a plausible
    /// one — and no ZEOR amount may ever appear, under any profile.
    #[test]
    fn notices_resolve_the_stored_tier_and_stay_silent_while_it_is_unset() {
        with_state("worktime_notices_tier", |state| {
            sign_in_admin(state);

            let unset = notices(state).expect("notices should resolve");
            assert!(
                unset.record_missing.penalty.is_none() && unset.caps_exceeded.penalty.is_none(),
                "no legal form is stored yet, so no figure may be quoted"
            );
            assert!(unset.record_missing.citation.contains("čl. 55 st. 6"));
            assert!(unset.caps_exceeded.citation.contains("čl. 53"));

            crate::commands::settings::save_shop_profile(
                state,
                crate::commands::settings::ShopProfileRequest {
                    pravna_forma: Some(crate::commands::settings::PravnaForma::Preduzetnik),
                    pdv_obveznik: Some(false),
                    distance_selling: Some(false),
                    lpfr_in_premises: Some(false),
                    lpfr_carve_out_internet_only: Some(false),
                    lpfr_carve_out_own_used_assets: Some(false),
                    esir_elements: Vec::new(),
                },
            )
            .expect("admin should save the profile");

            let resolved = notices(state).expect("notices should resolve");
            let missing = resolved
                .record_missing
                .penalty
                .expect("the stored tier is known");
            let caps = resolved
                .caps_exceeded
                .penalty
                .expect("the stored tier is known");

            assert!(
                missing.contains("50.000 do 150.000"),
                "the preduzetnik row is ZoR čl. 276 st. 1: {missing}"
            );
            assert!(
                !missing.contains("odgovorno lice"),
                "čl. 276 st. 2 does not reach a preduzetnik: {missing}"
            );
            assert!(
                caps.contains("200.000 do 400.000"),
                "the caps carry the larger fine: {caps}"
            );

            // The ZEOR tiers are unresolved; silence beats a wrong number.
            for rendered in [missing, caps] {
                for forbidden in ["500.000 do 1.000.000", "300.000 do 500.000"] {
                    assert!(
                        !rendered.contains(forbidden),
                        "no ZEOR figure may be reachable here: {rendered}"
                    );
                }
            }
        });
    }
}
