//! SW-13 — the three-class employee split, and the one class a purge may touch.
//!
//! **The split is physical, not a convention** (req. 19). One terminated
//! employee leaves three kinds of record behind and they answer to three
//! different laws:
//!
//! | Class | Where it lives | Lifecycle |
//! |---|---|---|
//! | **A — Personnel** | `personnel_records`, the 25 tačke of ZEOR čl. 5 | `trajno` (ZEOR čl. 7 st. 2). No purge, no reset and no deactivation may reduce it |
//! | **B — Ledger attribution** | the surrogate `users.id` already on `sales`, `kep_entries` and the rest | kept with the accounting document it sits in (ZoRač čl. 8 st. 4) |
//! | **C — Credentials & access** | `users.pin_hash` / `users.password_hash`, and `audit_events` | the only class a purge may touch |
//!
//! No `ime`, `prezime` or `matični broj` is ever denormalised onto a transaction
//! row (req. 20): a name copied into a sale row would be simultaneously an
//! accounting record that cannot be deleted and a personal-data record subject to
//! minimisation, which is an unresolvable conflict of our own making. The
//! surrogate id plus this lookup avoids it, and pseudonimizacija is expressly
//! blessed by ZZPL čl. 42 st. 1 t. 1.
//!
//! **Why the purge lives here and not in `retention.rs`.** It takes a
//! [`PurgeableClass`], an enum with exactly two variants, neither of which is
//! class A — so reaching the personnel record is a type error rather than
//! something a reviewer has to catch. `retention.rs` knows about every class,
//! including the `trajno` ones, and a purge written against `RecordClass` would
//! be one `match` arm away from the ZEOR register.
//!
//! **The purge is time-driven, never request-driven** (req. 23). ZZPL čl. 5
//! st. 1 t. 5 and čl. 42 st. 2 are proactive rukovalac duties and čl. 30 is the
//! request-driven layer on top, so [`purge_expired_classes`] is deliberately
//! **not** a `#[tauri::command]`: it is called from the app's launch and timer
//! path with the wall clock the outermost boundary read. Its audit line records
//! *which class* was discarded and when, and never a discarded value.

use rusqlite::types::Value;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::audit::{AuditAction, AuditDraft, AuditObjectType, AuditReason};
use crate::cash_deposit::parse_iso_date;
use crate::clock::utc_now;
use crate::commands::audit::{append_audit_event, PURGE_ANCHOR_KEY};
use crate::retention::{
    assert_never_purge_intact, expiry_cutoff, is_purgeable, load_policy, never_purge_row_counts,
    RecordClass, ACCESS_LOG_RETENTION_YEARS,
};
use crate::state::AppState;

use super::auth::require_admin;

/// Longer than any ZEOR čl. 5 tačka needs. The columns are free text because the
/// statute's items are descriptions („zanimanje“, „osposobljenost“), but a
/// register is not a notes field.
const MAX_POLJE_ZNAKOVA: usize = 200;

/// The ZEOR čl. 5 evidencija o zaposlenim licima for one employee — class A.
///
/// Two absences are load-bearing:
///
/// 1. **No credential and no session data.** Those are class C and live on
///    `users`, where a purge can reach them; putting them here would tie a
///    discardable secret to a record kept `trajno`.
/// 2. **No diagnosis, no doznaka, no roster of family members.** Čl. 5 t. 20 and
///    t. 21 ask for a *count* of insured family members and a *number of days* of
///    temporary incapacity. Health data is a posebna vrsta podataka under ZZPL
///    čl. 17 and has no home on a till-adjacent machine.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonnelRecord {
    /// t. 1 — the only required tačka. Čl. 7 st. 1 opens the record on the first
    /// day of work and it is filled in as the rest of the data arrives.
    pub prezime_ime: String,
    /// t. 2 — JMBG. Never denormalised onto a transaction row (req. 20) and
    /// never carried into `audit_events`, whose exclusion list bars it (req. 4).
    pub maticni_broj: Option<String>,
    /// t. 3 — `muski` or `zenski`.
    pub pol: Option<String>,
    /// t. 4, first half.
    pub datum_rodjenja: Option<String>,
    /// t. 4, second half.
    pub mesto_rodjenja: Option<String>,
    /// t. 5.
    pub prebivaliste_i_adresa_stana: Option<String>,
    /// t. 6.
    pub mesto_rada: Option<String>,
    /// t. 7.
    pub naziv_i_adresa_poslodavca: Option<String>,
    /// t. 8.
    pub delatnost_poslodavca: Option<String>,
    /// t. 9.
    pub zanimanje: Option<String>,
    /// t. 10.
    pub vrsta_i_stepen_strucne_spreme: Option<String>,
    /// t. 11.
    pub osposobljenost: Option<String>,
    /// t. 12.
    pub naziv_radnog_mesta: Option<String>,
    /// t. 13 — ZEOR says „radno vreme u časovima“; the store is integer minutes
    /// like every other duration in this app, and the hours are a render-time
    /// division.
    pub radno_vreme_minuta_nedeljno: Option<i64>,
    /// t. 14 — `neodredjeno` or `odredjeno`.
    pub trajanje_zaposlenja: Option<String>,
    /// t. 15.
    pub vrsta_radnog_odnosa: Option<String>,
    /// t. 16.
    pub osnov_upucivanja_u_inostranstvo: Option<String>,
    /// t. 17.
    pub naziv_poslodavca_u_dopunskom_radu: Option<String>,
    /// t. 18.
    pub zainteresovanost_za_promenu_posla: Option<bool>,
    /// t. 19.
    pub invalid_rada: Option<bool>,
    /// t. 20 — a count, never a roster.
    pub osigurani_clanovi_porodice: Option<i64>,
    /// t. 21 — days, never a diagnosis.
    pub privremena_nesposobnost_dana: Option<i64>,
    /// t. 22.
    pub placeno_odsustvo_dana: Option<i64>,
    /// t. 23.
    pub datum_zasnivanja: Option<String>,
    /// t. 24 — čl. 7 st. 1 closes the record on this day; it never deletes it.
    pub datum_prestanka: Option<String>,
    /// t. 25.
    pub razlog_prestanka: Option<String>,
}

/// What one run of [`purge_expired_classes`] discarded.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeReport {
    /// Accounts whose PIN and password hashes were removed.
    pub credentials_cleared: usize,
    /// Rows removed from the čl. 48 evidencija pristupa.
    pub access_log_rows_removed: usize,
}

/// The classes a purge is allowed to name.
///
/// There is no `Personnel` variant and there never may be: class A is `trajno`
/// under ZEOR čl. 7 st. 2, and req. 19 says the purge must be *structurally*
/// incapable of touching it. Class B is not here either — ledger attribution is
/// retained with the accounting document it sits in and ZoRač čl. 8 st. 4 makes
/// a posted entry non-deletable, so there is nothing for a purge to do with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PurgeableClass {
    /// The PIN and password hashes of accounts whose employment has ended.
    Credentials,
    /// The čl. 48 evidencija pristupa, past [`ACCESS_LOG_RETENTION_YEARS`].
    AccessLog,
}

impl PurgeableClass {
    pub const ALL: [Self; 2] = [Self::Credentials, Self::AccessLog];

    /// The shared `retention_policies` row this class reads. Every arm resolves
    /// to a class whose `never_purge` is false — pinned by
    /// `a_personnel_record_has_no_configurable_retention_period`.
    fn record_class(self) -> RecordClass {
        match self {
            Self::Credentials => RecordClass::Credentials,
            Self::AccessLog => RecordClass::AccessLog,
        }
    }
}

#[tauri::command]
pub fn personnel_get(
    state: State<'_, AppState>,
    user_id: i64,
) -> Result<Option<PersonnelRecord>, CommandError> {
    read_personnel(state.inner(), user_id, &utc_now()?).map_err(Into::into)
}

#[tauri::command]
pub fn personnel_save(
    state: State<'_, AppState>,
    user_id: i64,
    record: PersonnelRecord,
) -> Result<PersonnelRecord, CommandError> {
    save_personnel(state.inner(), user_id, record, &utc_now()?).map_err(Into::into)
}

/// Reads one employee's ZEOR čl. 5 register, and records the access itself.
///
/// Admin-gated inside the domain function, not in the `#[tauri::command]`
/// wrapper, so no in-process caller can route around it. The register holds the
/// matični broj and the address; it is the rukovalac's, not a kasir's.
///
/// The čl. 48 st. 2 *uvid* line is written **here**, server-side, with the actor
/// taken from the session and the razlog fixed in code — the frontend never
/// asserts that an access happened, and could not assert a different reason for
/// it. A read that found nothing writes nothing: there was no access to record.
pub fn read_personnel(
    state: &AppState,
    user_id: i64,
    now: &str,
) -> Result<Option<PersonnelRecord>, AppError> {
    let acting = require_admin(state)?;

    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;
    let record = load_record(&tx, user_id)?;
    if record.is_some() {
        append_audit_event(
            &tx,
            &AuditDraft {
                at: now.to_string(),
                actor_user_id: Some(acting.id),
                action: AuditAction::Uvid,
                object_type: AuditObjectType::PersonnelRecord,
                // The surrogate id and nothing else. The record this line is
                // about contains a JMBG; the line about it contains a number.
                object_id: user_id.to_string(),
                // ZEOR čl. 7 st. 1–2 is why this register exists and why it is
                // consulted: the rukovalac is discharging a statutory duty.
                reason_code: Some(AuditReason::ZakonskaObaveza),
                recipient: None,
                support_session_id: None,
            },
        )?;
    }
    tx.commit()?;

    Ok(record)
}

/// Opens or corrects one employee's ZEOR čl. 5 register.
///
/// Čl. 7 st. 1 opens the record on the day work starts and closes it on the day
/// the employment ends; there is no third verb, and this module ships none. A
/// correction is an UPDATE — unlike the audit log, this is a living record whose
/// accuracy is itself a duty — and v18's `BEFORE DELETE` trigger refuses the
/// only operation that would end it.
///
/// The register write and its čl. 48 st. 2 line share one transaction, so a
/// logged change is a change that happened.
pub fn save_personnel(
    state: &AppState,
    user_id: i64,
    record: PersonnelRecord,
    now: &str,
) -> Result<PersonnelRecord, AppError> {
    let acting = require_admin(state)?;
    let normalized = normalize_record(record)?;

    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;

    let account_exists: i64 = tx.query_row(
        "SELECT COUNT(*) FROM users WHERE id = ?1",
        params![user_id],
        |row| row.get(0),
    )?;
    if account_exists == 0 {
        return Err(AppError::not_found("Korisnik nije pronađen."));
    }

    let existed = load_record(&tx, user_id)?.is_some();

    let columns = RECORD_COLUMNS.join(", ");
    let placeholders = (0..RECORD_COLUMNS.len())
        .map(|index| format!("?{}", index + 2))
        .collect::<Vec<_>>()
        .join(", ");
    let stamp = format!("?{}", RECORD_COLUMNS.len() + 2);
    let assignments = RECORD_COLUMNS
        .iter()
        .map(|column| format!("{column} = excluded.{column}"))
        .collect::<Vec<_>>()
        .join(", ");

    let mut values: Vec<Value> = Vec::with_capacity(RECORD_COLUMNS.len() + 2);
    values.push(Value::Integer(user_id));
    values.extend(record_values(&normalized));
    values.push(Value::Text(now.to_string()));

    tx.execute(
        &format!(
            "INSERT INTO personnel_records (user_id, {columns}, created_at, updated_at)
             VALUES (?1, {placeholders}, {stamp}, {stamp})
             ON CONFLICT(user_id) DO UPDATE SET {assignments}, updated_at = excluded.updated_at"
        ),
        rusqlite::params_from_iter(values),
    )?;

    append_audit_event(
        &tx,
        &AuditDraft {
            at: now.to_string(),
            actor_user_id: Some(acting.id),
            action: if existed {
                AuditAction::Menjanje
            } else {
                AuditAction::Unos
            },
            object_type: AuditObjectType::PersonnelRecord,
            object_id: user_id.to_string(),
            reason_code: None,
            recipient: None,
            support_session_id: None,
        },
    )?;

    let saved = load_record(&tx, user_id)?
        .ok_or_else(|| AppError::InvalidState("Evidencija nije upisana.".to_string()))?;
    tx.commit()?;

    Ok(saved)
}

/// Removes an account's PIN and password hashes and records that it did.
///
/// The one primitive both class-C erasure paths share: the termination
/// (req. 21), which is request-driven and has the admin who recorded it as its
/// actor, and the time-driven sweep (req. 23), which has none. `reason` and
/// `actor_user_id` are what separate the two in the evidencija.
///
/// Takes the caller's `Connection`, so the erasure and the line about it commit
/// or roll back together. Answers `false` when there was nothing left to remove,
/// and writes no line in that case — a log of erasures that never happened is
/// not evidence of anything.
pub(crate) fn clear_credentials(
    conn: &Connection,
    user_id: i64,
    actor_user_id: Option<i64>,
    reason: AuditReason,
    now: &str,
) -> Result<bool, AppError> {
    let cleared = conn.execute(
        "UPDATE users SET pin_hash = NULL, password_hash = NULL, updated_at = ?2
         WHERE id = ?1 AND (pin_hash IS NOT NULL OR password_hash IS NOT NULL)",
        params![user_id, now],
    )?;
    if cleared == 0 {
        return Ok(false);
    }

    append_audit_event(
        conn,
        &AuditDraft {
            at: now.to_string(),
            actor_user_id,
            action: AuditAction::Brisanje,
            object_type: AuditObjectType::Credentials,
            // Which account, never which secret (req. 23).
            object_id: user_id.to_string(),
            reason_code: Some(reason),
            recipient: None,
            support_session_id: None,
        },
    )?;

    Ok(true)
}

/// The time-driven sweep over class C — and over nothing else (req. 19, 23).
///
/// `now` is the RFC3339 instant the outermost boundary read: the class gates
/// compare calendar days and the audit line needs an instant, so one parameter
/// carries both rather than two that could disagree.
///
/// The whole sweep is one transaction with the [`never_purge_row_counts`] /
/// [`assert_never_purge_intact`] fence around it. That pair is what makes „class
/// A is unreachable“ structural rather than a comment: a future edit that finds
/// a way to delete a `personnel_records` row aborts the sweep instead of
/// committing it.
pub fn purge_expired_classes(state: &AppState, now: &str) -> Result<PurgeReport, AppError> {
    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;
    let never_purge_before = never_purge_row_counts(&tx)?;

    let mut report = PurgeReport::default();
    for class in PurgeableClass::ALL {
        let policy = load_policy(&tx, class.record_class())?;
        // A legal hold, a `trajno` flag or a floor the day has not reached each
        // refuse on their own — see `retention::is_purgeable`.
        if !is_purgeable(&policy, now) {
            continue;
        }

        match class {
            PurgeableClass::Credentials => {
                report.credentials_cleared = sweep_credentials(&tx, now)?
            }
            PurgeableClass::AccessLog => {
                report.access_log_rows_removed = expire_access_log(&tx, now)?
            }
        }
    }

    assert_never_purge_intact(&tx, &never_purge_before)?;
    tx.commit()?;

    Ok(report)
}

/// Every deactivated account that still carries a credential.
///
/// Req. 21 discards the hash at the termination itself, so in a healthy database
/// this finds nothing. It is the sweep for the accounts that were terminated
/// before that rule existed, and for any path that ever forgets it.
fn sweep_credentials(conn: &Connection, now: &str) -> Result<usize, AppError> {
    let ids: Vec<i64> = {
        let mut statement = conn.prepare(
            "SELECT id FROM users
              WHERE active = 0 AND (pin_hash IS NOT NULL OR password_hash IS NOT NULL)
              ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?
    };

    let mut cleared = 0;
    for user_id in ids {
        if clear_credentials(conn, user_id, None, AuditReason::AutomatskoCiscenje, now)? {
            cleared += 1;
        }
    }

    Ok(cleared)
}

/// Discards the head of the čl. 48 evidencija pristupa, and leaves the chain
/// verifiable behind it.
///
/// **Why a prefix by `id` rather than every row the date test matches.** The
/// surviving rows have to remain a chain, and a chain is contiguous: removing a
/// row from the middle leaves its successor carrying a `prev_hash` that no
/// longer matches anything, which the reader reports — correctly — as tampering.
/// So the cut is „everything before the oldest row still inside the period“. A
/// row whose `at` somehow sits behind a younger id is kept rather than removed,
/// which is the direction `retention.rs` says every decision here must fail.
///
/// **Why the line goes in before the deletion.** Appended first, it chains onto
/// the current tail and survives the cut; appended after a sweep that took every
/// row, it would start a second genesis chain that the anchor recorded below
/// contradicts, and the first legitimate purge would read as a break.
fn expire_access_log(conn: &Connection, now: &str) -> Result<usize, AppError> {
    let total: i64 = conn.query_row("SELECT COUNT(*) FROM audit_events", [], |row| row.get(0))?;
    if total == 0 {
        return Ok(0);
    }

    let Some(cutoff) = expiry_cutoff(now, ACCESS_LOG_RETENTION_YEARS) else {
        return Ok(0);
    };

    let first_kept: Option<i64> = conn.query_row(
        "SELECT MIN(id) FROM audit_events WHERE substr(at, 1, 10) >= ?1",
        params![cutoff],
        |row| row.get(0),
    )?;
    let last_expired_id: i64 = match first_kept {
        Some(id) => id - 1,
        None => conn.query_row("SELECT MAX(id) FROM audit_events", [], |row| row.get(0))?,
    };

    let anchor: Option<(i64, String)> = conn
        .query_row(
            "SELECT id, hash FROM audit_events WHERE id <= ?1 ORDER BY id DESC LIMIT 1",
            params![last_expired_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((anchor_id, anchor_hash)) = anchor else {
        return Ok(0);
    };

    append_audit_event(
        conn,
        &AuditDraft {
            at: now.to_string(),
            // Req. 23: time-driven, so there is no human actor. A borrowed one
            // would record an access the admin on shift never made.
            actor_user_id: None,
            action: AuditAction::Brisanje,
            object_type: AuditObjectType::AuditLog,
            // How far the cut reached — the id of the last row removed, which is
            // also the id half of the anchor below.
            object_id: anchor_id.to_string(),
            reason_code: Some(AuditReason::AutomatskoCiscenje),
            recipient: None,
            support_session_id: None,
        },
    )?;

    let removed = conn.execute(
        "DELETE FROM audit_events WHERE id <= ?1",
        params![last_expired_id],
    )?;

    // The contract `audit::verify_chain_from` and `commands::audit::purge_anchor`
    // are written against: the hash and the id of the LAST row removed. The cut
    // only ever takes the head, so this pair only ever moves forward.
    conn.execute(
        "INSERT INTO settings (key, value_json, updated_at)
         VALUES (?1, json_object('hash', ?2, 'id', ?3), ?4)
         ON CONFLICT(key) DO UPDATE SET
             value_json = excluded.value_json,
             updated_at = excluded.updated_at",
        params![PURGE_ANCHOR_KEY, anchor_hash, anchor_id, now],
    )?;

    Ok(removed)
}

/// Trims and bounds every tačka before it reaches the register.
fn normalize_record(record: PersonnelRecord) -> Result<PersonnelRecord, AppError> {
    let prezime_ime =
        normalized_field(Some(record.prezime_ime), "prezimeIme")?.ok_or_else(|| {
            AppError::validation(
                "Prezime i ime su obavezni za evidenciju o zaposlenim licima (ZEOR čl. 5 tač. 1).",
                serde_json::json!({ "field": "prezimeIme" }),
            )
        })?;

    Ok(PersonnelRecord {
        prezime_ime,
        maticni_broj: normalized_field(record.maticni_broj, "maticniBroj")?,
        pol: normalized_code(record.pol, "pol", &["muski", "zenski"])?,
        datum_rodjenja: normalized_day(record.datum_rodjenja, "datumRodjenja")?,
        mesto_rodjenja: normalized_field(record.mesto_rodjenja, "mestoRodjenja")?,
        prebivaliste_i_adresa_stana: normalized_field(
            record.prebivaliste_i_adresa_stana,
            "prebivalisteIAdresaStana",
        )?,
        mesto_rada: normalized_field(record.mesto_rada, "mestoRada")?,
        naziv_i_adresa_poslodavca: normalized_field(
            record.naziv_i_adresa_poslodavca,
            "nazivIAdresaPoslodavca",
        )?,
        delatnost_poslodavca: normalized_field(record.delatnost_poslodavca, "delatnostPoslodavca")?,
        zanimanje: normalized_field(record.zanimanje, "zanimanje")?,
        vrsta_i_stepen_strucne_spreme: normalized_field(
            record.vrsta_i_stepen_strucne_spreme,
            "vrstaIStepenStrucneSpreme",
        )?,
        osposobljenost: normalized_field(record.osposobljenost, "osposobljenost")?,
        naziv_radnog_mesta: normalized_field(record.naziv_radnog_mesta, "nazivRadnogMesta")?,
        radno_vreme_minuta_nedeljno: normalized_count(
            record.radno_vreme_minuta_nedeljno,
            "radnoVremeMinutaNedeljno",
        )?,
        trajanje_zaposlenja: normalized_code(
            record.trajanje_zaposlenja,
            "trajanjeZaposlenja",
            &["neodredjeno", "odredjeno"],
        )?,
        vrsta_radnog_odnosa: normalized_field(record.vrsta_radnog_odnosa, "vrstaRadnogOdnosa")?,
        osnov_upucivanja_u_inostranstvo: normalized_field(
            record.osnov_upucivanja_u_inostranstvo,
            "osnovUpucivanjaUInostranstvo",
        )?,
        naziv_poslodavca_u_dopunskom_radu: normalized_field(
            record.naziv_poslodavca_u_dopunskom_radu,
            "nazivPoslodavcaUDopunskomRadu",
        )?,
        zainteresovanost_za_promenu_posla: record.zainteresovanost_za_promenu_posla,
        invalid_rada: record.invalid_rada,
        osigurani_clanovi_porodice: normalized_count(
            record.osigurani_clanovi_porodice,
            "osiguraniClanoviPorodice",
        )?,
        privremena_nesposobnost_dana: normalized_count(
            record.privremena_nesposobnost_dana,
            "privremenaNesposobnostDana",
        )?,
        placeno_odsustvo_dana: normalized_count(
            record.placeno_odsustvo_dana,
            "placenoOdsustvoDana",
        )?,
        datum_zasnivanja: normalized_day(record.datum_zasnivanja, "datumZasnivanja")?,
        datum_prestanka: normalized_day(record.datum_prestanka, "datumPrestanka")?,
        razlog_prestanka: normalized_field(record.razlog_prestanka, "razlogPrestanka")?,
    })
}

/// The stored values, in [`RECORD_COLUMNS`] order.
fn record_values(record: &PersonnelRecord) -> Vec<Value> {
    fn text(value: &Option<String>) -> Value {
        value
            .as_ref()
            .map_or(Value::Null, |value| Value::Text(value.clone()))
    }
    fn number(value: Option<i64>) -> Value {
        value.map_or(Value::Null, Value::Integer)
    }
    fn flag(value: Option<bool>) -> Value {
        value.map_or(Value::Null, |flag| Value::Integer(i64::from(flag)))
    }

    vec![
        Value::Text(record.prezime_ime.clone()),
        text(&record.maticni_broj),
        text(&record.pol),
        text(&record.datum_rodjenja),
        text(&record.mesto_rodjenja),
        text(&record.prebivaliste_i_adresa_stana),
        text(&record.mesto_rada),
        text(&record.naziv_i_adresa_poslodavca),
        text(&record.delatnost_poslodavca),
        text(&record.zanimanje),
        text(&record.vrsta_i_stepen_strucne_spreme),
        text(&record.osposobljenost),
        text(&record.naziv_radnog_mesta),
        number(record.radno_vreme_minuta_nedeljno),
        text(&record.trajanje_zaposlenja),
        text(&record.vrsta_radnog_odnosa),
        text(&record.osnov_upucivanja_u_inostranstvo),
        text(&record.naziv_poslodavca_u_dopunskom_radu),
        flag(record.zainteresovanost_za_promenu_posla),
        flag(record.invalid_rada),
        number(record.osigurani_clanovi_porodice),
        number(record.privremena_nesposobnost_dana),
        number(record.placeno_odsustvo_dana),
        text(&record.datum_zasnivanja),
        text(&record.datum_prestanka),
        text(&record.razlog_prestanka),
    ]
}

/// Trims a free-text tačka; an empty one is an absent one.
fn normalized_field(value: Option<String>, field: &str) -> Result<Option<String>, AppError> {
    let Some(value) = value else { return Ok(None) };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > MAX_POLJE_ZNAKOVA {
        return Err(AppError::validation(
            format!("Podatak „{field}“ je predugačak za evidenciju o zaposlenim licima."),
            serde_json::json!({ "field": field }),
        ));
    }
    Ok(Some(trimmed.to_string()))
}

/// A `gggg-MM-dd` day. v18's GLOB admits `2026-13-45`; this does not.
fn normalized_day(value: Option<String>, field: &str) -> Result<Option<String>, AppError> {
    let Some(value) = normalized_field(value, field)? else {
        return Ok(None);
    };
    if parse_iso_date(&value).is_none() {
        return Err(AppError::validation(
            format!("Datum „{field}“ mora biti u obliku gggg-MM-dd."),
            serde_json::json!({ "field": field }),
        ));
    }
    Ok(Some(value))
}

/// One of a closed set, or a refusal that names the set.
fn normalized_code(
    value: Option<String>,
    field: &str,
    allowed: &[&str],
) -> Result<Option<String>, AppError> {
    let Some(value) = normalized_field(value, field)? else {
        return Ok(None);
    };
    if !allowed.contains(&value.as_str()) {
        return Err(AppError::validation(
            format!("Vrednost polja „{field}“ nije dozvoljena."),
            serde_json::json!({ "field": field }),
        ));
    }
    Ok(Some(value))
}

/// A count that cannot be negative — days, family members, minutes.
fn normalized_count(value: Option<i64>, field: &str) -> Result<Option<i64>, AppError> {
    match value {
        Some(count) if count < 0 => Err(AppError::validation(
            format!("Vrednost polja „{field}“ ne može biti negativna."),
            serde_json::json!({ "field": field }),
        )),
        other => Ok(other),
    }
}

fn read_record(row: &Row<'_>) -> rusqlite::Result<PersonnelRecord> {
    Ok(PersonnelRecord {
        prezime_ime: row.get(0)?,
        maticni_broj: row.get(1)?,
        pol: row.get(2)?,
        datum_rodjenja: row.get(3)?,
        mesto_rodjenja: row.get(4)?,
        prebivaliste_i_adresa_stana: row.get(5)?,
        mesto_rada: row.get(6)?,
        naziv_i_adresa_poslodavca: row.get(7)?,
        delatnost_poslodavca: row.get(8)?,
        zanimanje: row.get(9)?,
        vrsta_i_stepen_strucne_spreme: row.get(10)?,
        osposobljenost: row.get(11)?,
        naziv_radnog_mesta: row.get(12)?,
        radno_vreme_minuta_nedeljno: row.get(13)?,
        trajanje_zaposlenja: row.get(14)?,
        vrsta_radnog_odnosa: row.get(15)?,
        osnov_upucivanja_u_inostranstvo: row.get(16)?,
        naziv_poslodavca_u_dopunskom_radu: row.get(17)?,
        zainteresovanost_za_promenu_posla: row.get::<_, Option<i64>>(18)?.map(|flag| flag != 0),
        invalid_rada: row.get::<_, Option<i64>>(19)?.map(|flag| flag != 0),
        osigurani_clanovi_porodice: row.get(20)?,
        privremena_nesposobnost_dana: row.get(21)?,
        placeno_odsustvo_dana: row.get(22)?,
        datum_zasnivanja: row.get(23)?,
        datum_prestanka: row.get(24)?,
        razlog_prestanka: row.get(25)?,
    })
}

/// The 25 tačke as columns, in the one order [`read_record`] and
/// [`record_values`] both follow. Changing this order without changing both is a
/// silent field swap, which is why they sit beside each other.
const RECORD_COLUMNS: [&str; 26] = [
    "prezime_ime",
    "maticni_broj",
    "pol",
    "datum_rodjenja",
    "mesto_rodjenja",
    "prebivaliste_i_adresa_stana",
    "mesto_rada",
    "naziv_i_adresa_poslodavca",
    "delatnost_poslodavca",
    "zanimanje",
    "vrsta_i_stepen_strucne_spreme",
    "osposobljenost",
    "naziv_radnog_mesta",
    "radno_vreme_minuta_nedeljno",
    "trajanje_zaposlenja",
    "vrsta_radnog_odnosa",
    "osnov_upucivanja_u_inostranstvo",
    "naziv_poslodavca_u_dopunskom_radu",
    "zainteresovanost_za_promenu_posla",
    "invalid_rada",
    "osigurani_clanovi_porodice",
    "privremena_nesposobnost_dana",
    "placeno_odsustvo_dana",
    "datum_zasnivanja",
    "datum_prestanka",
    "razlog_prestanka",
];

fn load_record(conn: &Connection, user_id: i64) -> Result<Option<PersonnelRecord>, AppError> {
    conn.query_row(
        &format!(
            "SELECT {} FROM personnel_records WHERE user_id = ?1",
            RECORD_COLUMNS.join(", ")
        ),
        params![user_id],
        read_record,
    )
    .optional()
    .map_err(AppError::from)
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::{
        clear_credentials, purge_expired_classes, read_personnel, save_personnel, AuditReason,
        PersonnelRecord, PurgeableClass,
    };
    use crate::commands::audit::{search, AuditQuery};
    use crate::commands::users::{create_user, deactivate_user, SaveUserRequest};
    use crate::db::{test_database_path, Db};
    use crate::retention::{
        extend_retain_until, is_purgeable, load_policy, seed_retention_policies, RecordClass,
    };
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

    /// An account with both credential hashes present, so a purge has something
    /// to remove and a test can prove the value never reached the log.
    fn seed_employee(state: &AppState, active: bool) -> i64 {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, pin_hash, password_hash,
                                    active, created_at, updated_at)
                 VALUES ('radnica', 'Milica Milićević', 'cashier', 'pin-hes-tajna',
                         'lozinka-hes-tajna', ?1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![i64::from(active)],
            )
            .expect("employee should insert");
        connection.last_insert_rowid()
    }

    fn sign_in_cashier(state: &AppState, user_id: i64) {
        state
            .set_session_user_id(user_id)
            .expect("cashier session should set");
    }

    fn credential_hashes(state: &AppState, user_id: i64) -> (Option<String>, Option<String>) {
        state
            .db()
            .open()
            .expect("database should open")
            .query_row(
                "SELECT pin_hash, password_hash FROM users WHERE id = ?1",
                params![user_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("the account should still exist")
    }

    fn full_record() -> PersonnelRecord {
        PersonnelRecord {
            prezime_ime: "Milićević Milica".to_string(),
            maticni_broj: Some("0101990715023".to_string()),
            pol: Some("zenski".to_string()),
            datum_rodjenja: Some("1990-01-01".to_string()),
            mesto_rodjenja: Some("Kragujevac".to_string()),
            prebivaliste_i_adresa_stana: Some("Kragujevac, Kralja Petra 12".to_string()),
            mesto_rada: Some("Kragujevac, Aerodrom".to_string()),
            naziv_i_adresa_poslodavca: Some("Butik Vantum, Kragujevac".to_string()),
            delatnost_poslodavca: Some("47.71 Trgovina na malo odećom".to_string()),
            zanimanje: Some("Prodavac".to_string()),
            vrsta_i_stepen_strucne_spreme: Some("Srednja stručna sprema".to_string()),
            osposobljenost: Some("Obučena za rad na kasi".to_string()),
            naziv_radnog_mesta: Some("Prodavac u maloprodaji".to_string()),
            radno_vreme_minuta_nedeljno: Some(2400),
            trajanje_zaposlenja: Some("neodredjeno".to_string()),
            vrsta_radnog_odnosa: Some("Radni odnos sa punim radnim vremenom".to_string()),
            osnov_upucivanja_u_inostranstvo: None,
            naziv_poslodavca_u_dopunskom_radu: None,
            zainteresovanost_za_promenu_posla: Some(false),
            invalid_rada: Some(false),
            osigurani_clanovi_porodice: Some(2),
            privremena_nesposobnost_dana: Some(3),
            placeno_odsustvo_dana: Some(5),
            datum_zasnivanja: Some("2026-02-01".to_string()),
            datum_prestanka: None,
            razlog_prestanka: None,
        }
    }

    /// One logged row as these tests read it: action, object type, object id,
    /// razlog, actor.
    type LoggedRow = (String, String, String, Option<String>, Option<i64>);

    /// Every `audit_events` row, oldest first.
    fn audit_rows(state: &AppState) -> Vec<LoggedRow> {
        let connection = state.db().open().expect("database should open");
        let mut statement = connection
            .prepare(
                "SELECT action, object_type, object_id, reason_code, actor_user_id
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
                ))
            })
            .expect("query should run")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("rows should read");
        rows
    }

    /// Every textual column of every logged row, concatenated. What is not in
    /// here never reached the evidencija.
    fn audit_table_text(state: &AppState) -> String {
        let connection = state.db().open().expect("database should open");
        let mut statement = connection
            .prepare(
                "SELECT at || '|' || COALESCE(actor_user_id, '') || '|' || action || '|' ||
                        object_type || '|' || object_id || '|' || COALESCE(reason_code, '') ||
                        '|' || COALESCE(recipient, '') || '|' || prev_hash || '|' || hash
                 FROM audit_events ORDER BY id",
            )
            .expect("statement should prepare");
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query should run")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("rows should read");
        rows.join("\n")
    }

    /// A chained line in the access log, stamped on a chosen day.
    fn seed_audit_line(state: &AppState, at: &str, object_id: &str) {
        let connection = state.db().open().expect("database should open");
        crate::commands::audit::append_audit_event(
            &connection,
            &crate::audit::AuditDraft {
                at: at.to_string(),
                actor_user_id: None,
                action: crate::audit::AuditAction::Unos,
                object_type: crate::audit::AuditObjectType::Sale,
                object_id: object_id.to_string(),
                reason_code: None,
                recipient: None,
                support_session_id: None,
            },
        )
        .expect("the line should append");
    }

    /// Class B: the surrogate `users.id` on an accounting document. ZoRač čl. 8
    /// st. 4 makes a posted entry non-deletable, and no name is copied onto it.
    fn seed_sale(state: &AppState, cashier_id: i64) -> i64 {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO shifts (user_id, opened_at, status, created_at, updated_at)
                 VALUES (?1, '2026-02-01T08:00:00Z', 'closed',
                         '2026-02-01T08:00:00Z', '2026-02-01T20:00:00Z')",
                params![cashier_id],
            )
            .expect("shift should insert");
        let shift_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO sales (local_receipt_number, shift_id, cashier_id, status,
                                    subtotal_minor, tax_minor, total_minor, created_at, updated_at)
                 VALUES ('VP-000001', ?1, ?2, 'completed', 20000, 0, 20000,
                         '2026-02-01T09:00:00Z', '2026-02-01T09:00:00Z')",
                params![shift_id, cashier_id],
            )
            .expect("sale should insert");
        connection.last_insert_rowid()
    }

    fn save_user_request(username: &str) -> SaveUserRequest {
        SaveUserRequest {
            username: username.to_string(),
            display_name: "Milica Milićević".to_string(),
            role: "cashier".to_string(),
            active: true,
            pin: Some("4321".to_string()),
            password: None,
            profile: None,
        }
    }

    /// Req. 19: „only class C may be touched by the purge job“. A and B are not
    /// merely left alone by today's SQL — they are unreachable through the enum
    /// the purge is written against.
    #[test]
    fn the_purge_touches_only_the_credential_class() {
        with_state("the_purge_touches_only_the_credential_class", |state| {
            sign_in_admin(state);
            seed_retention_policies(state, "2026-01-01T08:00:00Z")
                .expect("the classes should seed");
            let employee_id = seed_employee(state, false);
            let sale_id = seed_sale(state, employee_id);
            let saved = save_personnel(state, employee_id, full_record(), "2026-02-01T08:00:00Z")
                .expect("the ZEOR register should open");

            seed_audit_line(state, "2026-08-01T09:00:00Z", "11");
            seed_audit_line(state, "2029-01-05T09:00:00Z", "12");

            let report =
                purge_expired_classes(state, "2029-08-01T03:00:00Z").expect("the purge should run");

            // Class C — gone. Two log rows, not one: the `unos` line the ZEOR
            // save wrote in 2026 has outlived its period exactly like the
            // seeded one, and the evidencija pristupa is class C too.
            assert_eq!(report.credentials_cleared, 1);
            assert_eq!(report.access_log_rows_removed, 2);
            assert_eq!(
                credential_hashes(state, employee_id),
                (None, None),
                "the credential class is the one the purge exists for"
            );

            // Class A — untouched, tačka for tačka.
            let after = read_personnel(state, employee_id, "2029-08-01T04:00:00Z")
                .expect("the register should read")
                .expect("class A survives every purge");
            assert_eq!(after, saved, "ZEOR čl. 7 st. 2: the register is trajno");

            // Class B — the surrogate attribution is still on the document.
            let connection = state.db().open().expect("database should open");
            let cashier_id: i64 = connection
                .query_row(
                    "SELECT cashier_id FROM sales WHERE id = ?1",
                    params![sale_id],
                    |row| row.get(0),
                )
                .expect("the accounting document survives");
            assert_eq!(cashier_id, employee_id, "ZoRač čl. 8 st. 4");

            // The account itself is deactivated, never deleted (req. 24).
            let accounts: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM users WHERE id = ?1",
                    params![employee_id],
                    |row| row.get(0),
                )
                .expect("count should query");
            assert_eq!(accounts, 1);

            // And the access log lost only what had outlived its period.
            let object_ids: Vec<String> = audit_rows(state)
                .into_iter()
                .map(|row| row.2)
                .collect::<Vec<_>>();
            assert!(
                !object_ids.contains(&"11".to_string()),
                "the 2026 line is past ACCESS_LOG_RETENTION_YEARS"
            );
            assert!(
                object_ids.contains(&"12".to_string()),
                "the 2029 line is still inside the period"
            );
        });
    }

    /// Req. 21: „purge the PIN/password hash and salt at termination, not after a
    /// retention window“. The database here has no `retention_policies` rows at
    /// all — nothing timed this, and nothing could have.
    #[test]
    fn the_credential_hash_is_gone_at_termination_not_after_a_window() {
        with_state(
            "the_credential_hash_is_gone_at_termination_not_after_a_window",
            |state| {
                let admin_id = sign_in_admin(state);
                let employee_id = seed_employee(state, true);
                let (pin, password) = credential_hashes(state, employee_id);
                assert!(pin.is_some() && password.is_some(), "seeded with both");

                {
                    let connection = state.db().open().expect("database should open");
                    assert_eq!(
                        load_policy(&connection, RecordClass::Credentials)
                            .expect_err("no window has been written")
                            .code(),
                        "not_found",
                        "there is no retention row, so no window can have elapsed"
                    );
                }

                deactivate_user(state, employee_id).expect("termination should apply");

                assert_eq!(
                    credential_hashes(state, employee_id),
                    (None, None),
                    "the hash goes with the employment, not with a calendar"
                );
                let connection = state.db().open().expect("database should open");
                let active: i64 = connection
                    .query_row(
                        "SELECT active FROM users WHERE id = ?1",
                        params![employee_id],
                        |row| row.get(0),
                    )
                    .expect("the account survives");
                assert_eq!(active, 0, "deactivation, never deletion (req. 24)");

                let rows = audit_rows(state);
                assert_eq!(rows.len(), 1, "the erasure is one čl. 48 st. 2 line");
                assert_eq!(rows[0].0, "brisanje");
                assert_eq!(rows[0].1, "credentials");
                assert_eq!(rows[0].2, employee_id.to_string());
                assert_eq!(
                    rows[0].4,
                    Some(admin_id),
                    "a termination has a human behind it, unlike the sweep"
                );
                let dump = audit_table_text(state);
                assert!(
                    !dump.contains("pin-hes-tajna") && !dump.contains("lozinka-hes-tajna"),
                    "the discarded secret must never appear in the log about it"
                );
            },
        );
    }

    /// Req. 23: the purge „persists an audit line recording *what class* was
    /// purged and when — the audit line must not contain the purged values“.
    #[test]
    fn the_purge_audit_line_records_the_class_but_never_the_purged_value() {
        with_state(
            "the_purge_audit_line_records_the_class_but_never_the_purged_value",
            |state| {
                sign_in_admin(state);
                seed_retention_policies(state, "2026-01-01T08:00:00Z")
                    .expect("the classes should seed");
                let employee_id = seed_employee(state, false);
                seed_audit_line(state, "2026-08-01T09:00:00Z", "11");

                purge_expired_classes(state, "2029-08-01T03:00:00Z").expect("the purge should run");

                let rows = audit_rows(state);
                let credentials = rows
                    .iter()
                    .find(|row| row.1 == "credentials")
                    .expect("the credential class reports its own sweep");
                assert_eq!(credentials.0, "brisanje");
                assert_eq!(
                    credentials.2,
                    employee_id.to_string(),
                    "an internal id, never a name and never a hash"
                );
                assert_eq!(credentials.3.as_deref(), Some("automatsko_ciscenje"));
                assert_eq!(
                    credentials.4, None,
                    "req. 23: the purge is time-driven, so there is no human actor"
                );

                let expiry = rows
                    .iter()
                    .find(|row| row.1 == "audit_log")
                    .expect("the access log reports its own expiry");
                assert_eq!(expiry.0, "brisanje");
                assert_eq!(expiry.3.as_deref(), Some("automatsko_ciscenje"));
                assert_eq!(expiry.4, None);

                let dump = audit_table_text(state);
                for forbidden in [
                    "pin-hes-tajna",
                    "lozinka-hes-tajna",
                    "radnica",
                    "Milica",
                    "Milićević",
                ] {
                    assert!(
                        !dump.contains(forbidden),
                        "„{forbidden}“ is a purged value or the person behind it \
                         and must not be in the line that records the purge"
                    );
                }
            },
        );
    }

    /// Req. 26: „personnel and payroll categories get no configurable period at
    /// all“. Not a default the operator may change — no period at all.
    #[test]
    fn a_personnel_record_has_no_configurable_retention_period() {
        with_state(
            "a_personnel_record_has_no_configurable_retention_period",
            |state| {
                seed_retention_policies(state, "2026-01-01T08:00:00Z")
                    .expect("the classes should seed");
                let connection = state.db().open().expect("database should open");

                let policy = load_policy(&connection, RecordClass::Personnel)
                    .expect("class A must be declared");
                assert_eq!(
                    policy.retain_until, None,
                    "trajno is the absence of an end date, not a distant one"
                );
                assert!(policy.never_purge, "ZEOR čl. 7 st. 2");
                assert!(!is_purgeable(&policy, "2099-12-31"));

                let error = extend_retain_until(
                    &connection,
                    RecordClass::Personnel,
                    "2099-12-31",
                    "2026-01-01T08:00:00Z",
                )
                .expect_err("a period may not be written onto class A");
                assert_eq!(error.code(), "validation_error");

                // And the purge cannot even name it: there is no variant.
                for class in PurgeableClass::ALL {
                    assert_ne!(class.record_class(), RecordClass::Personnel);
                    let policy = load_policy(&connection, class.record_class())
                        .expect("every purgeable class must be declared");
                    assert!(
                        !policy.never_purge,
                        "a purgeable class may never resolve to a trajno row"
                    );
                }
            },
        );
    }

    /// Req. 24: „deactivation must be structurally incapable of deleting the
    /// personnel record“.
    #[test]
    fn deactivating_an_account_never_touches_the_personnel_record() {
        with_state(
            "deactivating_an_account_never_touches_the_personnel_record",
            |state| {
                sign_in_admin(state);
                let employee_id = seed_employee(state, true);
                let saved =
                    save_personnel(state, employee_id, full_record(), "2026-02-01T08:00:00Z")
                        .expect("the ZEOR register should open");

                deactivate_user(state, employee_id).expect("termination should apply");

                let after = read_personnel(state, employee_id, "2026-09-01T08:00:00Z")
                    .expect("the register should read")
                    .expect("class A survives the termination");
                assert_eq!(after, saved, "tačka for tačka, unchanged");

                let connection = state.db().open().expect("database should open");
                let error = connection
                    .execute(
                        "DELETE FROM personnel_records WHERE user_id = ?1",
                        params![employee_id],
                    )
                    .expect_err("no path removes a ZEOR record");
                assert!(
                    error.to_string().contains("trajno"),
                    "v18's trigger refuses the delete: {error}"
                );

                // Nor may deleting the ACCOUNT take the record with it.
                let error = connection
                    .execute("DELETE FROM users WHERE id = ?1", params![employee_id])
                    .expect_err("the FK restricts rather than cascades");
                assert!(
                    error.to_string().to_lowercase().contains("foreign key"),
                    "no ON DELETE CASCADE may point at personnel_records: {error}"
                );
            },
        );
    }

    /// Req. 24: „deactivate-never-reuse, with non-reuse enforced by a uniqueness
    /// constraint that survives deactivation“.
    #[test]
    fn a_username_cannot_be_reused_after_deactivation() {
        with_state("a_username_cannot_be_reused_after_deactivation", |state| {
            sign_in_admin(state);
            let created = create_user(state, save_user_request("milica")).expect("the hire");
            deactivate_user(state, created.id).expect("the termination");

            let error =
                create_user(state, save_user_request("milica")).expect_err("req. 24: never reuse");
            assert_eq!(error.code(), "validation_error");

            let connection = state.db().open().expect("database should open");
            let error = connection
                .execute(
                    "INSERT INTO users (username, display_name, role, pin_hash, active,
                                        created_at, updated_at)
                     VALUES ('milica', 'Druga Osoba', 'cashier', 'hes', 1,
                             '2027-01-01T00:00:00Z', '2027-01-01T00:00:00Z')",
                    [],
                )
                .expect_err("the constraint survives deactivation");
            assert!(
                error.to_string().to_lowercase().contains("unique"),
                "uniqueness is a storage constraint, not a command-level check: {error}"
            );
        });
    }

    /// Čl. 48 st. 2 asks who looked and why. The answer is recorded server-side —
    /// the frontend never asserts that an access happened.
    #[test]
    fn reading_a_personnel_record_writes_its_own_uvid_line() {
        with_state(
            "reading_a_personnel_record_writes_its_own_uvid_line",
            |state| {
                let admin_id = sign_in_admin(state);
                let employee_id = seed_employee(state, true);
                save_personnel(state, employee_id, full_record(), "2026-02-01T08:00:00Z")
                    .expect("the ZEOR register should open");

                read_personnel(state, employee_id, "2026-09-01T08:00:00Z")
                    .expect("the register should read")
                    .expect("the record is there");

                let rows = audit_rows(state);
                let uvid = rows.last().expect("the read is the newest line");
                assert_eq!(uvid.0, "uvid");
                assert_eq!(uvid.1, "personnel_record");
                assert_eq!(uvid.2, employee_id.to_string());
                assert!(
                    uvid.3.is_some(),
                    "čl. 48 st. 2 requires a razlog for an uvid"
                );
                assert_eq!(uvid.4, Some(admin_id));

                let dump = audit_table_text(state);
                assert!(
                    !dump.contains("0101990715023"),
                    "the matični broj inside the record must never reach the log"
                );
            },
        );
    }

    /// The ZEOR register is the rukovalac's, and its 25 tačke include the JMBG.
    #[test]
    fn the_personnel_record_is_admin_gated() {
        with_state("the_personnel_record_is_admin_gated", |state| {
            sign_in_admin(state);
            let employee_id = seed_employee(state, true);
            save_personnel(state, employee_id, full_record(), "2026-02-01T08:00:00Z")
                .expect("the ZEOR register should open");
            let before = audit_rows(state).len();

            sign_in_cashier(state, employee_id);
            assert_eq!(
                read_personnel(state, employee_id, "2026-09-01T08:00:00Z")
                    .expect_err("a kasir may not read the register")
                    .code(),
                "forbidden"
            );
            assert_eq!(
                save_personnel(state, employee_id, full_record(), "2026-09-01T08:00:00Z")
                    .expect_err("nor write it")
                    .code(),
                "forbidden"
            );
            assert_eq!(
                audit_rows(state).len(),
                before,
                "a refused read is not an access and logs none"
            );
        });
    }

    /// Task 2's contract, discharged: the purge persists the hash and the id of
    /// the last row it removed, so a legitimately shortened log still verifies.
    #[test]
    fn the_purge_records_the_anchor_so_the_chain_still_verifies() {
        with_state(
            "the_purge_records_the_anchor_so_the_chain_still_verifies",
            |state| {
                sign_in_admin(state);
                seed_retention_policies(state, "2026-01-01T08:00:00Z")
                    .expect("the classes should seed");
                for object_id in ["11", "12"] {
                    seed_audit_line(state, "2026-08-01T09:00:00Z", object_id);
                }
                seed_audit_line(state, "2029-01-05T09:00:00Z", "13");

                let purged_hash: String = state
                    .db()
                    .open()
                    .expect("database should open")
                    .query_row("SELECT hash FROM audit_events WHERE id = 2", [], |row| {
                        row.get(0)
                    })
                    .expect("the second line should read");

                let report = purge_expired_classes(state, "2029-08-01T03:00:00Z")
                    .expect("the purge should run");
                assert_eq!(report.access_log_rows_removed, 2);

                let connection = state.db().open().expect("database should open");
                let (hash, id): (String, i64) = connection
                    .query_row(
                        "SELECT json_extract(value_json, '$.hash'),
                                json_extract(value_json, '$.id')
                         FROM settings WHERE key = 'audit_chain_anchor'",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .expect("the purge records its anchor");
                assert_eq!(hash, purged_hash, "the hash of the LAST row removed");
                assert_eq!(id, 2);

                let result =
                    search(state, &AuditQuery::default()).expect("the reader should still run");
                assert!(
                    result.chain.intact,
                    "a purged head must not read as tampering: {}",
                    result.chain.label
                );
            },
        );
    }

    /// The half of that contract the test above cannot see.
    ///
    /// `the_purge_records_the_anchor_so_the_chain_still_verifies` only ever cuts
    /// a **prefix** — the youngest line survives, so the purge line chains onto
    /// it whichever side of the `DELETE` it is appended on, and both orderings
    /// produce a byte-identical chain. The ordering only becomes load-bearing
    /// when the cut takes every surviving row: appended first, the purge line
    /// chains onto the tail the anchor records and carries the chain across the
    /// cut; appended after, it starts a second genesis chain that the anchor
    /// contradicts, and the first legitimate whole-log purge reads as tampering.
    #[test]
    fn a_purge_that_takes_the_whole_log_still_leaves_a_verifiable_chain() {
        with_state(
            "a_purge_that_takes_the_whole_log_still_leaves_a_verifiable_chain",
            |state| {
                sign_in_admin(state);
                seed_retention_policies(state, "2026-01-01T08:00:00Z")
                    .expect("the classes should seed");
                // Every line is older than the period, so nothing survives the
                // cut and the anchor is the tail of the log itself.
                for object_id in ["11", "12", "13"] {
                    seed_audit_line(state, "2026-08-01T09:00:00Z", object_id);
                }

                let tail_hash: String = state
                    .db()
                    .open()
                    .expect("database should open")
                    .query_row("SELECT hash FROM audit_events WHERE id = 3", [], |row| {
                        row.get(0)
                    })
                    .expect("the third line should read");

                let report = purge_expired_classes(state, "2029-08-01T03:00:00Z")
                    .expect("the purge should run");
                assert_eq!(
                    report.access_log_rows_removed, 3,
                    "every seeded line had outlived the period"
                );

                let connection = state.db().open().expect("database should open");
                let (hash, id): (String, i64) = connection
                    .query_row(
                        "SELECT json_extract(value_json, '$.hash'),
                                json_extract(value_json, '$.id')
                         FROM settings WHERE key = 'audit_chain_anchor'",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .expect("the purge records its anchor");
                assert_eq!(hash, tail_hash, "the hash of the LAST row removed");
                assert_eq!(id, 3);

                let rows = audit_rows(state);
                assert_eq!(
                    rows.len(),
                    1,
                    "the purge line is the only survivor: {rows:?}"
                );

                let result =
                    search(state, &AuditQuery::default()).expect("the reader should still run");
                assert!(
                    result.chain.intact,
                    "the purge line must chain onto the anchor, not restart the chain \
                     behind it — a whole-log purge is not tampering: {}",
                    result.chain.label
                );
            },
        );
    }

    /// A legal hold stops the sweep like it stops every other purge in this app.
    #[test]
    fn a_legal_hold_stops_the_purge() {
        with_state("a_legal_hold_stops_the_purge", |state| {
            sign_in_admin(state);
            seed_retention_policies(state, "2026-01-01T08:00:00Z")
                .expect("the classes should seed");
            let employee_id = seed_employee(state, false);
            seed_audit_line(state, "2026-08-01T09:00:00Z", "11");

            state
                .db()
                .open()
                .expect("database should open")
                .execute(
                    "UPDATE retention_policies SET legal_hold = 1
                     WHERE record_class IN ('credentials', 'access_log')",
                    [],
                )
                .expect("the hold should apply");

            let report =
                purge_expired_classes(state, "2029-08-01T03:00:00Z").expect("the purge should run");

            assert_eq!(report.credentials_cleared, 0);
            assert_eq!(report.access_log_rows_removed, 0);
            let (pin, password) = credential_hashes(state, employee_id);
            assert!(pin.is_some() && password.is_some());
            assert_eq!(audit_rows(state).len(), 1, "and nothing was logged either");
        });
    }

    /// The one-line primitive both paths share, exercised directly: it removes
    /// nothing twice and writes nothing when there was nothing to remove.
    #[test]
    fn clearing_a_credential_twice_writes_one_line() {
        with_state("clearing_a_credential_twice_writes_one_line", |state| {
            sign_in_admin(state);
            let employee_id = seed_employee(state, false);
            let connection = state.db().open().expect("database should open");

            assert!(clear_credentials(
                &connection,
                employee_id,
                None,
                AuditReason::AutomatskoCiscenje,
                "2029-08-01T03:00:00Z"
            )
            .expect("the first sweep clears"));
            assert!(
                !clear_credentials(
                    &connection,
                    employee_id,
                    None,
                    AuditReason::AutomatskoCiscenje,
                    "2029-08-02T03:00:00Z"
                )
                .expect("the second finds nothing"),
                "an account with no hash left has nothing to purge"
            );
            assert_eq!(audit_rows(state).len(), 1, "and logs no second erasure");
        });
    }

    /// Req. 23 („the purge is not a command“) and req. 24 („no Delete employee
    /// affordance“) are properties of the *surface*, not of any one function, so
    /// only the registration list can hold them. Task 4 pins its own surface the
    /// same way in `no_audit_command_can_edit_or_delete_a_logged_row`.
    ///
    /// The filter keeps bare identifiers only, so `lib.rs`'s two launch-path
    /// **calls** to `purge_expired_classes` — which are the point: it runs on a
    /// timer and not behind a button — do not read as registrations.
    #[test]
    fn the_personnel_surface_is_a_read_and_a_save_and_nothing_else() {
        const LIB_RS: &str = include_str!("../lib.rs");

        let registered: Vec<&str> = LIB_RS
            .lines()
            .filter_map(|line| line.trim().strip_prefix("commands::personnel::"))
            .map(|name| name.trim_end_matches(','))
            .filter(|name| {
                !name.is_empty()
                    && name
                        .chars()
                        .all(|character| character.is_ascii_alphanumeric() || character == '_')
            })
            .collect();

        assert_eq!(
            registered,
            vec!["personnel_get", "personnel_save"],
            "req. 23: the čl. 5 st. 1 tač. 5 purge is a proactive duty and must stay time-driven, \
             never a command someone has to press; req. 24: the ZEOR register is trajno, so there \
             is no delete affordance to expose in the first place"
        );
    }
}
