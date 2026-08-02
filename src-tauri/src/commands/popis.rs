//! Admin-gated command layer over the popis (SW-16, reqs. 29–41).
//!
//! Thin in the same sense the other compliance modules are thin: every *decision*
//! about what may follow what lives in `crate::popis` — the state machine, the
//! čl. 8 st. 5 release predicate and the čl. 13 st. 2 deadline engine are pure and
//! are not re-derived here. What lives here is the part that touches rows: the
//! sessions, the liste, the two potpisi and the knjiženje — plus the one document
//! that touches none, the **izveštaj o popisu** (req. 37), which is composed out of
//! all of them when it is asked for and is not stored anywhere.
//!
//! Three properties are this module's own and none of them is a UI convention:
//!
//! 1. **The čl. 20 st. 3 gate (req. 39).** A popis cannot be opened until the
//!    reconciliation is confirmed, because ZoRač čl. 20 st. 3 legislates the
//!    ordering rather than recommending it.
//! 2. **The blind count (req. 29).** A book quantity is refused at the boundary
//!    while `book_quantities_released` is false, and — the half a hidden column
//!    would miss — the read path in that state does not name the book column or
//!    any inventory table at all, so there is nothing for a response to carry.
//! 3. **The posting lock (req. 41).** Once the result is knjižen the session, its
//!    liste, its komisija and any new potpis are closed; a correction is a NEW
//!    popis (ZoRač čl. 8 st. 4), never an edit.
//!
//! Every command is `require_admin`.

use std::fmt::Write as _;

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::popis::{
    advance, book_quantities_released, ensure_izvestaj_kompletan, izvestaj_due, konsignacija_rok,
    nedostajuce_liste, vrednost_minor, IzvestajElement, IzvestajNarativ, PopisEvent, PopisLista,
    PopisStatus, PopisVrsta,
};
use crate::state::AppState;

/// The three `popis_commission.uloga` values of migration v20. `jedno_lice` is the
/// PoP čl. 6 st. 1 single-person popis, which is its own legal shape and not a
/// commission of one.
const KOMISIJA_ULOGE: [&str; 3] = ["predsednik", "clan", "jedno_lice"];

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KomisijaClanInput {
    pub ime: String,
    /// One of [`KOMISIJA_ULOGE`].
    pub uloga: String,
    /// Req. 40 / PoP čl. 5 st. 1 — recorded per named person so the warning can
    /// say who it is about. It warns; it never blocks (§6 R-5 is unresolved).
    #[serde(default)]
    pub rukuje_imovinom: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenPopisRequest {
    pub vrsta: PopisVrsta,
    pub prodajno_mesto: String,
    pub datum_popisa: String,
    #[serde(default)]
    pub period_from: Option<String>,
    #[serde(default)]
    pub period_to: Option<String>,
    /// PoP čl. 8 st. 1–2 — the plan rada, stored as it was approved.
    #[serde(default)]
    pub plan_rada_json: Option<String>,
    #[serde(default)]
    pub odluka_ref: Option<String>,
    /// Req. 34 / PoP čl. 9 st. 2 — the odluka behind the perpetual-inventory
    /// shortcut. **Not free text:** a non-empty value is refused unless a popis of
    /// the same business year, dated before this one, is already `posted` (see
    /// [`ensure_perpetual_shortcut`], which is the whole of what is checked). The
    /// „usvojen“ limb — the čl. 14 st. 2 odluka o usvajanju — is **not** checked
    /// here: the izveštaj surfaces that odluka as a milestone with its rok (see
    /// [`OdlukaOUsvajanjuView`]) but nothing in this schema records that it was
    /// made, so there is no fact to check. The limb stays open and is stated as
    /// open wherever it shows.
    #[serde(default)]
    pub perpetual_odluka_ref: Option<String>,
    /// Req. 39 / ZoRač čl. 20 st. 3.
    #[serde(default)]
    pub uskladjivanje_potvrdjeno: bool,
    #[serde(default)]
    pub komisija: Vec<KomisijaClanInput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopisLineInput {
    /// The stored value of one [`PopisLista`]. A string rather than the enum so an
    /// unknown lista comes back as a sentence naming it, instead of a serde
    /// rejection of the whole payload.
    pub lista_vrsta: String,
    #[serde(default)]
    pub sifra: Option<String>,
    pub naziv: String,
    #[serde(default)]
    pub vrsta: Option<String>,
    #[serde(default)]
    pub jedinica_mere: Option<String>,
    /// PoP čl. 9 st. 1 t. 1 — the natural count, in milli-units.
    #[serde(default)]
    pub stvarna_kolicina_milli: i64,
    #[serde(default)]
    pub blizi_opis: Option<String>,
    /// PoP čl. 9 st. 1 t. 3. Present on the wire on purpose: the posebne liste of
    /// čl. 11 st. 1 and čl. 12 st. 2 have no perpetual record to populate them
    /// from, so their book side is entered by hand — but only once čl. 8 st. 5
    /// has released it. A payload carrying this field earlier is **refused**, not
    /// dropped, so the caller learns that what it tried was the breach.
    #[serde(default)]
    pub knjigovodstvena_kolicina_milli: Option<i64>,
    /// PoP čl. 9 st. 1 t. 5, integer minor units (para).
    #[serde(default)]
    pub cena_minor: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KomisijaClanView {
    pub id: i64,
    pub ime: String,
    pub uloga: String,
    pub rukuje_imovinom: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopisSignatureView {
    pub id: i64,
    pub faza: String,
    pub potpisnik: String,
    pub potpisano_at: String,
    pub snapshot_hash: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopisLineView {
    pub id: i64,
    pub lista_vrsta: String,
    pub sifra: Option<String>,
    pub naziv: String,
    pub vrsta: Option<String>,
    pub jedinica_mere: Option<String>,
    pub stvarna_kolicina_milli: i64,
    pub blizi_opis: Option<String>,
    /// `None` for as long as čl. 8 st. 5 withholds it — and in that state it is
    /// `None` because nothing was read, not because something read was dropped.
    pub knjigovodstvena_kolicina_milli: Option<i64>,
    /// PoP čl. 9 st. 1 t. 4, the naturalna razlika. Derived, so it cannot exist
    /// before the book quantity does.
    pub razlika_milli: Option<i64>,
    pub cena_minor: Option<i64>,
}

/// One of the six popisne liste of req. 36 with what is on it (and, for five of
/// them, the article that requires it). All six are always reported, empty ones
/// included: a posebna lista the count sheet never shows is a posebna lista the
/// shop never fills.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListaPregled {
    pub vrsta: PopisLista,
    pub naziv: String,
    pub pravni_osnov: String,
    pub broj_stavki: i64,
}

/// The req. 36 readiness report: the declared categories checked against the
/// liste. `spremno` is what Task 6's izveštaj generation rests on — čl. 13 st. 1
/// has the izveštaj report the stvarno stanje of the popis, and a popis that
/// declared a category and wrote none of it down reports a stanje that is not the
/// shop's.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProveraListiView {
    pub spremno: bool,
    pub nedostaju: Vec<ListaPregled>,
    /// The refusal `crate::popis::ensure_liste_kompletne` gives, carried as text so
    /// a screen can show the same sentence the generator will refuse with — one
    /// wording, not two.
    pub poruka: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopisSessionView {
    pub id: i64,
    pub vrsta: PopisVrsta,
    pub prodajno_mesto: String,
    pub datum_popisa: String,
    pub period_from: Option<String>,
    pub period_to: Option<String>,
    pub status: PopisStatus,
    pub plan_rada_json: Option<String>,
    pub odluka_ref: Option<String>,
    pub perpetual_odluka_ref: Option<String>,
    pub uskladjivanje_potvrdjeno_at: Option<String>,
    pub posted_at: Option<String>,
    /// The second limb of čl. 8 st. 5, reported as the fact it is rather than
    /// inferred from `status` by whoever renders this.
    pub faza_a_potpisana: bool,
    pub faza_b_potpisana: bool,
    /// `book_quantities_released(status, faza_a_potpisana)` — what actually
    /// governs whether the Phase B block below carries anything.
    pub knjigovodstvo_dostupno: bool,
    pub komisija: Vec<KomisijaClanView>,
    pub potpisi: Vec<PopisSignatureView>,
    pub linije: Vec<PopisLineView>,
    /// Req. 36 — all six liste with what is on each.
    pub liste: Vec<ListaPregled>,
    /// PoP čl. 2 st. 6 — the day by which a signed copy of the konsignaciona lista
    /// must be in the owner's hands. `None` when the popis carries no tuđa roba,
    /// and also when the count date is such that the rok cannot be computed — in
    /// which case the duty is reported in `upozorenja` instead, because arithmetic
    /// failing is not an obligation ending.
    pub konsignacija_rok: Option<String>,
    /// Req. 40 — warnings, never blocks. Also carries the čl. 2 st. 6 reminder.
    pub upozorenja: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopisSummary {
    pub id: i64,
    pub vrsta: PopisVrsta,
    pub prodajno_mesto: String,
    pub datum_popisa: String,
    pub status: PopisStatus,
    pub posted_at: Option<String>,
    pub broj_linija: i64,
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

/// The stored session row, before the two derived facts — whether the čl. 8 st. 5
/// potpis exists and what that releases — are put beside it.
struct SessionRow {
    id: i64,
    vrsta: PopisVrsta,
    prodajno_mesto: String,
    datum_popisa: String,
    period_from: Option<String>,
    period_to: Option<String>,
    status: PopisStatus,
    plan_rada_json: Option<String>,
    odluka_ref: Option<String>,
    perpetual_odluka_ref: Option<String>,
    uskladjivanje_potvrdjeno_at: Option<String>,
    posted_at: Option<String>,
}

fn read_session(connection: &Connection, id: i64) -> Result<SessionRow, AppError> {
    let row = connection
        .query_row(
            "SELECT id, vrsta, prodajno_mesto, datum_popisa, period_from, period_to, status,
                    plan_rada_json, odluka_ref, perpetual_odluka_ref,
                    uskladjivanje_potvrdjeno_at, posted_at
             FROM popis_sessions
             WHERE id = ?1",
            params![id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("Popis nije pronađen."))?;

    Ok(SessionRow {
        id: row.0,
        vrsta: decode_vrsta(&row.1)?,
        prodajno_mesto: row.2,
        datum_popisa: row.3,
        period_from: row.4,
        period_to: row.5,
        status: decode_status(&row.6)?,
        plan_rada_json: row.7,
        odluka_ref: row.8,
        perpetual_odluka_ref: row.9,
        uskladjivanje_potvrdjeno_at: row.10,
        posted_at: row.11,
    })
}

/// The v20 CHECKs and the `crate::popis` enums are one vocabulary (asserted there
/// against `sqlite_master`), so a value that does not decode is a database this
/// build does not understand — not a row to guess at.
fn decode_status(value: &str) -> Result<PopisStatus, AppError> {
    PopisStatus::from_db_str(value)
        .ok_or_else(|| AppError::InvalidState(format!("Nepoznato stanje popisa „{value}“.")))
}

fn decode_vrsta(value: &str) -> Result<PopisVrsta, AppError> {
    PopisVrsta::from_db_str(value)
        .ok_or_else(|| AppError::InvalidState(format!("Nepoznata vrsta popisa „{value}“.")))
}

fn read_signatures(
    connection: &Connection,
    session_id: i64,
) -> Result<Vec<PopisSignatureView>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, faza, potpisnik, potpisano_at, snapshot_hash
         FROM popis_signatures
         WHERE session_id = ?1
         ORDER BY id",
    )?;
    let rows = statement.query_map(params![session_id], |row| {
        Ok(PopisSignatureView {
            id: row.get(0)?,
            faza: row.get(1)?,
            potpisnik: row.get(2)?,
            potpisano_at: row.get(3)?,
            snapshot_hash: row.get(4)?,
        })
    })?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

fn read_commission(
    connection: &Connection,
    session_id: i64,
) -> Result<Vec<KomisijaClanView>, AppError> {
    let mut statement = connection.prepare(
        "SELECT id, ime, uloga, rukuje_imovinom
         FROM popis_commission
         WHERE session_id = ?1
         ORDER BY id",
    )?;
    let rows = statement.query_map(params![session_id], |row| {
        Ok(KomisijaClanView {
            id: row.get(0)?,
            ime: row.get(1)?,
            uloga: row.get(2)?,
            rukuje_imovinom: row.get::<_, i64>(3)? != 0,
        })
    })?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// The čl. 8 st. 5 read path, in **two statements rather than one and a filter**.
///
/// While the book data is withheld, the query does not name
/// `knjigovodstvena_kolicina_milli` and does not name `inventory_balances` or
/// `inventory_movements` either — so there is no value in the row for a later
/// refactor, a log line or a debug print to spill, and adding an „očekivano“
/// column to the count sheet would have to be done here, in the open, against
/// this comment. Selecting everything and blanking it in Rust would look the same
/// from the outside and be a different thing.
///
/// **The one thing the blind read does carry out of the Phase B block is the
/// apoen** (req. 36 / čl. 11 st. 1). On the gotovina lista `cena_minor` is not the
/// čl. 9 st. 1 t. 5 obračunska cena — it is the denomination the commission itself
/// counted, and withholding it would leave the cash lista unreadable exactly while
/// it is being written. Čl. 8 st. 5 keeps back podatke iz knjigovodstva **o
/// količinama**; it does not keep the commission from its own count. Everywhere
/// else the cena stays withheld, because there it is an obračun figure and the
/// obračun has not happened yet.
fn read_lines(
    connection: &Connection,
    session_id: i64,
    released: bool,
) -> Result<Vec<PopisLineView>, AppError> {
    if !released {
        let mut statement = connection.prepare(
            "SELECT id, lista_vrsta, sifra, naziv, vrsta, jedinica_mere,
                    stvarna_kolicina_milli, blizi_opis,
                    CASE WHEN lista_vrsta = ?2 THEN cena_minor END
             FROM popis_lines
             WHERE session_id = ?1
             ORDER BY id",
        )?;
        let rows = statement.query_map(
            params![session_id, PopisLista::Gotovina.as_db_str()],
            |row| {
                Ok(PopisLineView {
                    id: row.get(0)?,
                    lista_vrsta: row.get(1)?,
                    sifra: row.get(2)?,
                    naziv: row.get(3)?,
                    vrsta: row.get(4)?,
                    jedinica_mere: row.get(5)?,
                    stvarna_kolicina_milli: row.get(6)?,
                    blizi_opis: row.get(7)?,
                    knjigovodstvena_kolicina_milli: None,
                    razlika_milli: None,
                    cena_minor: row.get(8)?,
                })
            },
        )?;

        return rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into);
    }

    let mut statement = connection.prepare(
        "SELECT id, lista_vrsta, sifra, naziv, vrsta, jedinica_mere,
                stvarna_kolicina_milli, blizi_opis, knjigovodstvena_kolicina_milli, cena_minor
         FROM popis_lines
         WHERE session_id = ?1
         ORDER BY id",
    )?;
    let rows = statement.query_map(params![session_id], |row| {
        let stvarna: i64 = row.get(6)?;
        let knjigovodstvena: Option<i64> = row.get(8)?;
        Ok(PopisLineView {
            id: row.get(0)?,
            lista_vrsta: row.get(1)?,
            sifra: row.get(2)?,
            naziv: row.get(3)?,
            vrsta: row.get(4)?,
            jedinica_mere: row.get(5)?,
            stvarna_kolicina_milli: stvarna,
            blizi_opis: row.get(7)?,
            knjigovodstvena_kolicina_milli: knjigovodstvena,
            // Čl. 9 st. 1 t. 4 — višak is positive, manjak negative. Saturating
            // rather than wrapping: a razlika that silently changed sign would be
            // a manjak reported as a višak.
            razlika_milli: knjigovodstvena.map(|book| stvarna.saturating_sub(book)),
            cena_minor: row.get(9)?,
        })
    })?;

    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(Into::into)
}

/// Req. 36 — the six liste with what is on each. Counting rows rather than
/// listing them, and reporting the empty ones too: which liste a popis has *not*
/// touched is the question čl. 10–12 and čl. 2 st. 5 actually ask.
fn read_liste(connection: &Connection, session_id: i64) -> Result<Vec<ListaPregled>, AppError> {
    let mut statement = connection.prepare(
        "SELECT lista_vrsta, COUNT(*)
         FROM popis_lines
         WHERE session_id = ?1
         GROUP BY lista_vrsta",
    )?;
    let rows = statement.query_map(params![session_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
    })?;
    let broj = rows.collect::<rusqlite::Result<Vec<_>>>()?;

    Ok(PopisLista::ALL
        .into_iter()
        .map(|lista| ListaPregled {
            vrsta: lista,
            naziv: lista.naziv().to_string(),
            pravni_osnov: lista.pravni_osnov().to_string(),
            broj_stavki: broj
                .iter()
                .find(|(vrsta, _)| vrsta == lista.as_db_str())
                .map_or(0, |(_, count)| *count),
        })
        .collect())
}

/// The liste this popis actually has stavke on — the „present“ half of req. 36's
/// „required where the category is present“.
fn liste_sa_stavkama(liste: &[ListaPregled]) -> Vec<PopisLista> {
    liste
        .iter()
        .filter(|pregled| pregled.broj_stavki > 0)
        .map(|pregled| pregled.vrsta)
        .collect()
}

/// PoP čl. 2 st. 6 — „дужан је да примерак потписане посебне пописне листе достави
/// … власнику те имовине најкасније у року од десет дана од дана на који је попис
/// извршен“.
///
/// The duty attaches to the popis that recorded tuđa roba, so the reminder is
/// derived from the liste rather than from a switch somebody has to remember to
/// set. Two things the copy must do and one it must not: it carries the rok and
/// it says the delivery is the shop's to make — this module stores a popis, it
/// does not send anything to anybody, and a reminder that read like a scheduled
/// job would promise behaviour that does not exist.
///
/// When the rok cannot be computed from the count date the duty is still
/// reported, with the date it could not be computed from. Dropping the reminder
/// because the arithmetic left the calendar would be the app deciding away an
/// obligation it merely failed to date.
fn konsignacija_podsetnik(datum_popisa: &str) -> (Option<String>, String) {
    match konsignacija_rok(datum_popisa) {
        Ok(rok) => {
            let poruka = format!(
                "Podsetnik: popis obuhvata tuđu (konsignacionu) robu — potpisan primerak posebne \
                 popisne liste dostavlja se vlasniku najkasnije do „{rok}“, u roku od deset dana \
                 od dana popisa (PoP čl. 2 st. 6). Aplikacija tu listu ne dostavlja umesto vas."
            );
            (Some(rok), poruka)
        }
        Err(error) => (
            None,
            format!(
                "Podsetnik: popis obuhvata tuđu (konsignacionu) robu i potpisan primerak posebne \
                 popisne liste dostavlja se vlasniku u roku od deset dana od dana popisa (PoP \
                 čl. 2 st. 6). Rok nije izračunat iz datuma popisa „{datum_popisa}“: {error} \
                 Obaveza time ne prestaje — izračunajte rok ručno. Aplikacija tu listu ne \
                 dostavlja umesto vas."
            ),
        ),
    }
}

fn phase_signed(connection: &Connection, session_id: i64, faza: &str) -> Result<bool, AppError> {
    let signed: bool = connection.query_row(
        "SELECT EXISTS (SELECT 1 FROM popis_signatures WHERE session_id = ?1 AND faza = ?2)",
        params![session_id, faza],
        |row| row.get(0),
    )?;

    Ok(signed)
}

/// Req. 40. PoP čl. 5 st. 1 keeps lica koja rukuju imovinom out of the komisija;
/// whether that reaches the čl. 6 st. 1–2 single-person popis *shodno* is
/// unresolved (§6 R-5). So the copy states the rule, states plainly that the
/// single-person case is unsettled, and states that nothing was stopped — a
/// warning that implied the app had blocked something would be promising
/// behaviour this module does not implement.
fn komisija_upozorenja(komisija: &[KomisijaClanView]) -> Vec<String> {
    komisija
        .iter()
        .filter(|clan| clan.rukuje_imovinom)
        .map(|clan| {
            format!(
                "Upozorenje: „{}“ rukuje imovinom koja se popisuje. PoP čl. 5 st. 1 isključuje \
                 takva lica iz komisije za popis, a da li to važi i kada popis sprovodi jedno \
                 lice (čl. 6 st. 1–2) nije razjašnjeno. Popis nije zaustavljen — proverite \
                 sastav pre potpisivanja listi.",
                clan.ime
            )
        })
        .collect()
}

pub(crate) fn list_sessions(connection: &Connection) -> Result<Vec<PopisSummary>, AppError> {
    let mut statement = connection.prepare(
        "SELECT s.id, s.vrsta, s.prodajno_mesto, s.datum_popisa, s.status, s.posted_at,
                (SELECT COUNT(*) FROM popis_lines l WHERE l.session_id = s.id)
         FROM popis_sessions s
         ORDER BY s.datum_popisa DESC, s.id DESC",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, Option<String>>(5)?,
            row.get::<_, i64>(6)?,
        ))
    })?;

    let mut sessions = Vec::new();
    for row in rows {
        let row = row?;
        sessions.push(PopisSummary {
            id: row.0,
            vrsta: decode_vrsta(&row.1)?,
            prodajno_mesto: row.2,
            datum_popisa: row.3,
            status: decode_status(&row.4)?,
            posted_at: row.5,
            broj_linija: row.6,
        });
    }

    Ok(sessions)
}

/// The query layer of req. 29. Whether the Phase B block is carried is decided by
/// `book_quantities_released` — both limbs — and by nothing else, and the decision
/// is made *before* the lines are read rather than after.
pub(crate) fn load_session(connection: &Connection, id: i64) -> Result<PopisSessionView, AppError> {
    let session = read_session(connection, id)?;
    let potpisi = read_signatures(connection, id)?;
    let faza_a_potpisana = potpisi.iter().any(|potpis| potpis.faza == "a");
    let faza_b_potpisana = potpisi.iter().any(|potpis| potpis.faza == "b");
    let knjigovodstvo_dostupno = book_quantities_released(session.status, faza_a_potpisana);
    let komisija = read_commission(connection, id)?;
    let mut upozorenja = komisija_upozorenja(&komisija);
    let linije = read_lines(connection, id, knjigovodstvo_dostupno)?;
    let liste = read_liste(connection, id)?;

    // Req. 36 / čl. 2 st. 6 — the ten-day duty follows the tuđa roba, so it is
    // read off the liste and not off a flag.
    let mut konsignacija_rok = None;
    if liste_sa_stavkama(&liste).contains(&PopisLista::Konsignacija) {
        let (rok, podsetnik) = konsignacija_podsetnik(&session.datum_popisa);
        konsignacija_rok = rok;
        upozorenja.push(podsetnik);
    }

    Ok(PopisSessionView {
        id: session.id,
        vrsta: session.vrsta,
        prodajno_mesto: session.prodajno_mesto,
        datum_popisa: session.datum_popisa,
        period_from: session.period_from,
        period_to: session.period_to,
        status: session.status,
        plan_rada_json: session.plan_rada_json,
        odluka_ref: session.odluka_ref,
        perpetual_odluka_ref: session.perpetual_odluka_ref,
        uskladjivanje_potvrdjeno_at: session.uskladjivanje_potvrdjeno_at,
        posted_at: session.posted_at,
        faza_a_potpisana,
        faza_b_potpisana,
        knjigovodstvo_dostupno,
        komisija,
        potpisi,
        linije,
        liste,
        konsignacija_rok,
        upozorenja,
    })
}

/// Req. 36 through the query layer: the categories the shop declared present,
/// checked against the liste it actually filled.
///
/// The refusal itself is `crate::popis::ensure_liste_kompletne` — the same
/// function Task 6 calls before it composes the izveštaj — so the sentence a
/// screen shows and the sentence a generator refuses with are one sentence.
pub(crate) fn provera_listi(
    connection: &Connection,
    session_id: i64,
    prijavljene: &[PopisLista],
) -> Result<ProveraListiView, AppError> {
    // A report about a popis that exists: an id nobody opened is not „spremno“.
    read_session(connection, session_id)?;

    let liste = read_liste(connection, session_id)?;
    let sa_stavkama = liste_sa_stavkama(&liste);
    let poruka = crate::popis::ensure_liste_kompletne(prijavljene, &sa_stavkama)
        .err()
        .map(|error| error.to_string());
    let nedostaju_vrste = nedostajuce_liste(prijavljene, &sa_stavkama);
    let nedostaju: Vec<ListaPregled> = liste
        .into_iter()
        .filter(|pregled| nedostaju_vrste.contains(&pregled.vrsta))
        .collect();

    Ok(ProveraListiView {
        spremno: poruka.is_none(),
        nedostaju,
        poruka,
    })
}

// ---------------------------------------------------------------------------
// Writes
// ---------------------------------------------------------------------------

/// Strictly `gggg-MM-dd`, the shape every date column in v20 is GLOB-checked into
/// and the shape `crate::popis::izvestaj_due` computes its roks from.
fn ensure_datum(value: &str, polje: &str) -> Result<(), AppError> {
    if crate::cash_deposit::parse_iso_date(value).is_none() {
        return Err(AppError::business(
            "popis_neispravan_datum",
            format!(
                "Polje „{polje}“ mora da bude datum u obliku gggg-MM-dd, a primljeno je „{value}“."
            ),
        ));
    }

    Ok(())
}

/// Req. 34 / design §4 — the čl. 9 st. 2 shortcut is a **conditional gate, not a
/// default**.
///
/// „Изузетно од става 1. тачка 2) … као стање по попису на датум биланса може
/// уписати њено књиговодствено стање на тај дан, под условом да је у току године
/// извршен попис имовине и да су вишкови и мањкови утврђени тим пописом
/// прокњижени…“ — so the exception rests on a popis that actually happened and
/// whose result was posted. A reference stored without that behind it is a claim
/// about a document that does not exist, and it would let the shop skip step 2)
/// on the strength of it. The exception excuses **step 2) alone**; the natural
/// count of t. 1 is untouched by it, which is why this refuses the reference
/// rather than the count.
///
/// What is checked is exactly: a `popis_sessions` row of the same calendar year,
/// dated before this one, whose status is `posted`. The čl. 14 st. 2 odluka o
/// usvajanju is a separate artefact that **nothing in this schema records** — the
/// izveštaj surfaces it as a milestone with its rok and says so, but there is no
/// stored fact for a gate to read — so this gate does not check it and no string
/// here claims it did.
fn ensure_perpetual_shortcut(connection: &Connection, datum_popisa: &str) -> Result<(), AppError> {
    let postoji: bool = connection.query_row(
        "SELECT EXISTS (SELECT 1
                          FROM popis_sessions
                         WHERE status = 'posted'
                           AND substr(datum_popisa, 1, 4) = substr(?1, 1, 4)
                           AND datum_popisa < ?1)",
        params![datum_popisa],
        |row| row.get(0),
    )?;

    if !postoji {
        return Err(AppError::business(
            "popis_nema_popisa_u_toku_godine",
            format!(
                "Knjigovodstveno stanje se upisuje kao stanje po popisu samo ako je u toku iste \
                 godine, pre datuma „{datum_popisa}“, već izvršen popis čiji su viškovi i manjkovi \
                 proknjiženi (PoP čl. 9 st. 2). Takav popis nije evidentiran — sprovedite popis \
                 brojanjem ili izostavite odluku o stalnoj evidenciji."
            ),
        ));
    }

    Ok(())
}

pub(crate) fn open_popis(
    connection: &mut Connection,
    request: &OpenPopisRequest,
    now: &str,
) -> Result<PopisSessionView, AppError> {
    // Req. 39. ZoRač čl. 20 st. 3 legislates the ORDER — reconcile the books,
    // then take the popis — so this is asked first and the session does not come
    // into existence unconfirmed. Recording it is half the requirement: an
    // inspector asks when it was done, not whether a box was ticked.
    if !request.uskladjivanje_potvrdjeno {
        return Err(AppError::business(
            "popis_uskladjivanje_nije_potvrdjeno",
            "Popis se ne otvara pre usklađivanja knjiga: glavna knjiga sa dnevnikom i pomoćne \
             knjige sa glavnom knjigom (ZoRač čl. 20 st. 3). Potvrdite usklađivanje pa ponovite.",
        ));
    }

    let prodajno_mesto = request.prodajno_mesto.trim();
    if prodajno_mesto.is_empty() {
        return Err(AppError::business(
            "popis_bez_prodajnog_mesta",
            "Popis se vodi po prodajnom mestu — unesite maloprodajni objekat.",
        ));
    }

    ensure_datum(&request.datum_popisa, "datum popisa")?;
    if let Some(from) = request.period_from.as_deref() {
        ensure_datum(from, "početak perioda")?;
    }
    if let Some(to) = request.period_to.as_deref() {
        ensure_datum(to, "kraj perioda")?;
    }
    if let (Some(from), Some(to)) = (request.period_from.as_deref(), request.period_to.as_deref()) {
        if to < from {
            return Err(AppError::business(
                "popis_neispravan_period",
                "Kraj perioda ne može da bude pre početka perioda.",
            ));
        }
    }

    // Req. 34. Only a reference that says something is gated: an empty string is
    // no reference at all, and treating it as one would refuse an open that never
    // claimed the exception.
    let perpetual_odluka_ref = request
        .perpetual_odluka_ref
        .as_deref()
        .map(str::trim)
        .filter(|reference| !reference.is_empty());
    if perpetual_odluka_ref.is_some() {
        ensure_perpetual_shortcut(connection, &request.datum_popisa)?;
    }

    for clan in &request.komisija {
        if clan.ime.trim().is_empty() {
            return Err(AppError::business(
                "popis_bez_imena_clana",
                "Popisivači se evidentiraju poimenično — unesite ime i prezime.",
            ));
        }
        if !KOMISIJA_ULOGE.contains(&clan.uloga.as_str()) {
            return Err(AppError::business(
                "popis_nepoznata_uloga",
                format!("Nepoznata uloga u popisu „{}“.", clan.uloga),
            ));
        }
    }

    let transaction = connection.transaction()?;
    transaction.execute(
        "INSERT INTO popis_sessions (vrsta, prodajno_mesto, datum_popisa, period_from, period_to,
                                     status, plan_rada_json, odluka_ref,
                                     uskladjivanje_potvrdjeno_at, perpetual_odluka_ref,
                                     created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, 'draft', ?6, ?7, ?8, ?9, ?8, ?8)",
        params![
            request.vrsta.as_db_str(),
            prodajno_mesto,
            request.datum_popisa,
            request.period_from,
            request.period_to,
            request.plan_rada_json,
            request.odluka_ref,
            now,
            perpetual_odluka_ref,
        ],
    )?;
    let id = transaction.last_insert_rowid();

    for clan in &request.komisija {
        transaction.execute(
            "INSERT INTO popis_commission (session_id, ime, uloga, rukuje_imovinom, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                id,
                clan.ime.trim(),
                clan.uloga,
                i64::from(clan.rukuje_imovinom),
                now
            ],
        )?;
    }

    let view = load_session(&transaction, id)?;
    transaction.commit()?;

    Ok(view)
}

/// The part of a popisna lista the čl. 8 st. 5 potpis fixes: what čl. 8 st. 4
/// handed the commission before it counted (nomenklaturni broj, naziv, vrsta,
/// jedinica mere), which lista the stavka sits on, and the čl. 9 st. 1 t. 1 count
/// with its bliži opis. Read back so a Phase B write can be compared against what
/// was actually signed rather than trusted.
struct PotpisanaStavka {
    lista_vrsta: String,
    sifra: Option<String>,
    naziv: String,
    vrsta: Option<String>,
    jedinica_mere: Option<String>,
    stvarna_kolicina_milli: i64,
    blizi_opis: Option<String>,
    /// On the gotovina lista this is the **apoen** and therefore part of what was
    /// counted; everywhere else it is the čl. 9 st. 1 t. 5 cena and the obračun's
    /// to fill in. Read back so the two cases can be told apart.
    cena_minor: Option<i64>,
}

impl PotpisanaStavka {
    /// The identity fields a Phase B payload would move, named so the refusal can
    /// say which one it was. Compared exactly as the UPDATE would have written
    /// them — `naziv` trimmed, the rest as given — so the check and the write
    /// cannot disagree about what „unchanged“ means.
    fn pomerena_polja(&self, input: &PopisLineInput) -> Vec<&'static str> {
        let mut polja = Vec::new();
        if self.lista_vrsta != input.lista_vrsta {
            polja.push("vrsta popisne liste");
        }
        if self.sifra.as_deref() != input.sifra.as_deref() {
            polja.push("šifra");
        }
        if self.naziv != input.naziv.trim() {
            polja.push("naziv");
        }
        if self.vrsta.as_deref() != input.vrsta.as_deref() {
            polja.push("vrsta");
        }
        if self.jedinica_mere.as_deref() != input.jedinica_mere.as_deref() {
            polja.push("jedinica mere");
        }
        if self.blizi_opis.as_deref() != input.blizi_opis.as_deref() {
            polja.push("bliži opis");
        }
        polja
    }

    fn lista(&self) -> Option<PopisLista> {
        PopisLista::from_db_str(&self.lista_vrsta)
    }

    /// Whether a Phase B payload moves the apoen of a signed gotovina line. `None`
    /// leaves it alone — the UPDATE coalesces — so only a payload that actually
    /// carries a different apoen is a move.
    fn apoen_pomeren(&self, input: &PopisLineInput) -> bool {
        self.lista() == Some(PopisLista::Gotovina)
            && input.cena_minor.is_some()
            && input.cena_minor != self.cena_minor
    }

    /// What the obračun may still fill in on this stavka, said in the refusal
    /// above. It differs by lista and the difference matters: on the gotovina lista
    /// the „cena“ is the apoen and is frozen with the count, so telling the shop it
    /// may still edit a cena there would be an operator string promising something
    /// the very next call refuses.
    fn obracunska_polja(&self) -> &'static str {
        if self.lista() == Some(PopisLista::Gotovina) {
            "U obračunu se popunjava samo knjigovodstvena količina — „cena“ na listi gotovine je \
             apoen i deo je potpisanog stvarnog stanja"
        } else {
            "U obračunu se popunjavaju samo cena i knjigovodstvena količina"
        }
    }
}

fn read_potpisana_stavka(
    connection: &Connection,
    session_id: i64,
    line_id: i64,
) -> Result<PotpisanaStavka, AppError> {
    connection
        .query_row(
            "SELECT lista_vrsta, sifra, naziv, vrsta, jedinica_mere,
                    stvarna_kolicina_milli, blizi_opis, cena_minor
             FROM popis_lines
             WHERE id = ?1 AND session_id = ?2",
            params![line_id, session_id],
            |row| {
                Ok(PotpisanaStavka {
                    lista_vrsta: row.get(0)?,
                    sifra: row.get(1)?,
                    naziv: row.get(2)?,
                    vrsta: row.get(3)?,
                    jedinica_mere: row.get(4)?,
                    stvarna_kolicina_milli: row.get(5)?,
                    blizi_opis: row.get(6)?,
                    cena_minor: row.get(7)?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("Stavka popisne liste nije pronađena."))
}

/// Milli-units per counted piece — the schema's own quantity unit. A banknote is
/// one piece and never a fraction of one. Taken from `crate::popis` rather than
/// written out again: the same divisor turns a količina into money in the izveštaj.
const MILLI: i64 = crate::popis::MILLI;

/// What each lista must carry to be the document its own article asks for.
///
/// Only the gotovina lista has such a rule today, and it is the one req. 36 spells
/// out: **PoP čl. 11 st. 1 counts cash „по апоенима“.** A single „u kasi ima
/// 45.000 dinara“ line is a figure, not a count, and it is exactly what a
/// denomination breakdown exists to replace — so a cash line names the apoen it
/// counted (`cena_minor`, para) and the number of whole notes or coins found at
/// it. Half a banknote was never counted; a popis is evidence, and admitting the
/// typo would put an amount in the izveštaj that no drawer ever held.
///
/// The other four posebne liste are left with the rules every line has (a naziv,
/// a non-negative count): nothing in čl. 10 st. 3, čl. 10 st. 4 or čl. 12 st. 2
/// prescribes a field, and inventing one here would refuse a lawful count in the
/// name of a duty that does not exist.
///
/// `apoen` is the value that will be ON the stavka after the write, not the value
/// the payload carried — see `efektivni_apoen`.
fn ensure_stavka(
    lista: PopisLista,
    input: &PopisLineInput,
    apoen: Option<i64>,
) -> Result<(), AppError> {
    if lista != PopisLista::Gotovina {
        return Ok(());
    }

    if apoen.unwrap_or(0) <= 0 {
        return Err(AppError::business(
            "popis_gotovina_bez_apoena",
            "Gotovina se popisuje po apoenima (PoP čl. 11 st. 1) — uz svaku stavku upišite apoen \
             i broj novčanica odnosno kovanica tog apoena, a ne jedan ukupan iznos.",
        ));
    }

    if input.stvarna_kolicina_milli % MILLI != 0 {
        return Err(AppError::business(
            "popis_gotovina_deo_apoena",
            format!(
                "Broj novčanica odnosno kovanica jednog apoena mora da bude ceo broj (PoP čl. 11 \
                 st. 1), a prebrojano je „{},{:03}“.",
                input.stvarna_kolicina_milli / MILLI,
                input.stvarna_kolicina_milli % MILLI
            ),
        ));
    }

    Ok(())
}

/// The apoen that will stand on the stavka once the write lands.
///
/// The UPDATE coalesces `cena_minor`, so a payload that omits it does not blank
/// the apoen — it keeps the one already on the lista. Čl. 11 st. 1 asks that the
/// stavka NAME its apoen, and a stavka that already names one satisfies it
/// whether or not the edit repeats the figure; judging the absent field instead
/// would refuse a lawful correction of a bliži opis on a fully denominated cash
/// line. It would also split the meaning of one wire shape: after the potpis,
/// `cena_minor: None` means „leave the apoen alone“ (see `apoen_pomeren`), and it
/// must mean the same thing before it.
///
/// Read from the row rather than trusted from the caller, and only where it can
/// change the answer: a new stavka (no `line_id`) has no stored apoen to inherit,
/// and neither does one being moved onto the gotovina lista for the first time —
/// both still owe the figure.
fn efektivni_apoen(
    connection: &Connection,
    lista: PopisLista,
    session_id: i64,
    line_id: Option<i64>,
    input: &PopisLineInput,
) -> Result<Option<i64>, AppError> {
    let (PopisLista::Gotovina, None, Some(line_id)) = (lista, input.cena_minor, line_id) else {
        return Ok(input.cena_minor);
    };

    Ok(connection
        .query_row(
            "SELECT cena_minor FROM popis_lines WHERE id = ?1 AND session_id = ?2",
            params![line_id, session_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?
        .flatten())
}

pub(crate) fn save_line(
    connection: &Connection,
    session_id: i64,
    line_id: Option<i64>,
    input: &PopisLineInput,
    now: &str,
) -> Result<PopisSessionView, AppError> {
    let session = read_session(connection, session_id)?;
    // Req. 41 first, and with the engine's own wording: on a posted popis „the
    // popis is closed“ is the truer answer than „that field is frozen“.
    crate::popis::posting_lock(session.status)?;

    let Some(lista) = PopisLista::from_db_str(&input.lista_vrsta) else {
        return Err(AppError::business(
            "popis_nepoznata_lista",
            format!(
                "Nepoznata vrsta popisne liste „{}“ (PoP čl. 2 st. 5, čl. 10–12).",
                input.lista_vrsta
            ),
        ));
    };
    if input.naziv.trim().is_empty() {
        return Err(AppError::business(
            "popis_bez_naziva",
            "Svaka stavka popisne liste mora da ima naziv (PoP čl. 8 st. 4).",
        ));
    }
    if input.stvarna_kolicina_milli < 0 {
        return Err(AppError::business(
            "popis_negativna_kolicina",
            "Stvarna količina je rezultat brojanja i ne može da bude negativna.",
        ));
    }

    let released =
        book_quantities_released(session.status, phase_signed(connection, session_id, "a")?);

    // Req. 29, at the boundary. The v20 trigger refuses the same write, but a
    // caller that met only the trigger would read a SQLite abort; here it reads
    // which duty it walked into. Refused, never dropped: silently discarding the
    // field would let a client believe it had stored book data in Phase A.
    if input.knjigovodstvena_kolicina_milli.is_some() && !released {
        return Err(AppError::business(
            "popis_knjigovodstvo_pre_potpisa",
            "Knjigovodstvena količina ne sme da se upiše pre nego što se stvarno stanje unese u \
             popisne liste i pre nego što članovi komisije potpišu te liste (PoP čl. 8 st. 5).",
        ));
    }

    // Req. 30 — the two potpisi freeze two snapshots. The counted state is
    // writable up to the čl. 8 st. 5 potpis and never again: a stvarna količina
    // that could still be edited afterwards would make that signature attest to a
    // state that no longer exists. In `computed` the obračun may still be filled
    // in — the posebne liste of čl. 11 st. 1 and čl. 12 st. 2 have no perpetual
    // record behind them — but only over lines the commission actually counted,
    // and without moving what it counted.
    match session.status {
        PopisStatus::Draft | PopisStatus::Counting => ensure_stavka(
            lista,
            input,
            efektivni_apoen(connection, lista, session_id, line_id, input)?,
        )?,
        PopisStatus::Computed => {
            let Some(line_id) = line_id else {
                return Err(AppError::business(
                    "popis_stvarno_stanje_potpisano",
                    "Nova stavka se ne dodaje posle potpisa stvarnog stanja — nju niko nije \
                     prebrojao (PoP čl. 8 st. 5).",
                ));
            };
            let potpisana = read_potpisana_stavka(connection, session_id, line_id)?;
            if potpisana.stvarna_kolicina_milli != input.stvarna_kolicina_milli {
                return Err(AppError::business(
                    "popis_stvarno_stanje_potpisano",
                    "Stvarna količina je potpisana i više se ne menja (PoP čl. 8 st. 5) — \
                     ispravka se sprovodi novim popisom.",
                ));
            }
            // The potpis fixes a DOCUMENT, not a number. Čl. 8 st. 4 hands the
            // commission the nomenklaturni broj, the naziv, the vrsta and the
            // jedinica mere, and čl. 9 st. 1 t. 1 has it sign the count together
            // with the bliži opis; čl. 8 st. 5 releases the book data BECAUSE
            // those liste were signed. A šifra or a naziv still rewritable in the
            // obračun would leave the potpis attesting to a stavka nobody counted
            // — the same breach as moving the količina, wearing a different hat.
            // Refused rather than ignored, for the reason the book-quantity write
            // is refused: a caller whose fields were silently dropped would
            // believe the change landed.
            let pomerena = potpisana.pomerena_polja(input);
            if !pomerena.is_empty() {
                let polja = pomerena
                    .iter()
                    .map(|polje| format!("„{polje}“"))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(AppError::business(
                    "popis_stvarno_stanje_potpisano",
                    format!(
                        "Potpisana stavka popisne liste se više ne menja (PoP čl. 8 st. 5, čl. 9 \
                         st. 1 t. 1), a razlikuje se: {polja}. {} — ispravka same stavke sprovodi \
                         se novim popisom.",
                        potpisana.obracunska_polja()
                    ),
                ));
            }

            // Req. 36 / čl. 11 st. 1 — on the gotovina lista `cena_minor` is the
            // APOEN, part of what the commission counted and signed, and not a
            // price the obračun fills in under čl. 9 st. 1 t. 5. Left unpinned, a
            // signed „5 × 1.000“ could become „5 × 5.000“ in Phase B with the
            // količina untouched, and the čl. 8 st. 5 potpis would attest to a cash
            // count nobody took.
            if potpisana.apoen_pomeren(input) {
                return Err(AppError::business(
                    "popis_apoen_potpisan",
                    "Na popisnoj listi gotovine „cena“ je apoen — deo prebrojanog stvarnog stanja \
                     (PoP čl. 11 st. 1), a ne obračunska cena. Potpisani apoen se više ne menja \
                     (PoP čl. 8 st. 5) — ispravka se sprovodi novim popisom.",
                ));
            }
        }
        PopisStatus::CountedSigned => {
            return Err(AppError::business(
                "popis_stvarno_stanje_potpisano",
                "Stvarno stanje je potpisano (PoP čl. 8 st. 5) — popisne liste se više ne menjaju.",
            ));
        }
        PopisStatus::ComputedSigned => {
            return Err(AppError::business(
                "popis_liste_potpisane",
                "Obračunate popisne liste su potpisane (PoP čl. 9 st. 3) i više se ne menjaju.",
            ));
        }
        // `posting_lock` above already refused this state.
        PopisStatus::Posted => return Err(AppError::InvalidState("Popis je proknjižen.".into())),
    }

    match line_id {
        None => {
            connection.execute(
                "INSERT INTO popis_lines (session_id, lista_vrsta, sifra, naziv, vrsta,
                                          jedinica_mere, stvarna_kolicina_milli, blizi_opis,
                                          knjigovodstvena_kolicina_milli, cena_minor,
                                          created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
                params![
                    session_id,
                    input.lista_vrsta,
                    input.sifra,
                    input.naziv.trim(),
                    input.vrsta,
                    input.jedinica_mere,
                    input.stvarna_kolicina_milli,
                    input.blizi_opis,
                    input.knjigovodstvena_kolicina_milli,
                    input.cena_minor,
                    now,
                ],
            )?;
        }
        Some(line_id) => {
            // COALESCE, not assignment: a Phase B edit that only sets a cena must
            // not blank the book quantity the čl. 8 st. 5 potpis released.
            let updated = connection.execute(
                "UPDATE popis_lines
                    SET lista_vrsta = ?3, sifra = ?4, naziv = ?5, vrsta = ?6, jedinica_mere = ?7,
                        stvarna_kolicina_milli = ?8, blizi_opis = ?9,
                        knjigovodstvena_kolicina_milli =
                            COALESCE(?10, knjigovodstvena_kolicina_milli),
                        cena_minor = COALESCE(?11, cena_minor), updated_at = ?12
                  WHERE id = ?1 AND session_id = ?2",
                params![
                    line_id,
                    session_id,
                    input.lista_vrsta,
                    input.sifra,
                    input.naziv.trim(),
                    input.vrsta,
                    input.jedinica_mere,
                    input.stvarna_kolicina_milli,
                    input.blizi_opis,
                    input.knjigovodstvena_kolicina_milli,
                    input.cena_minor,
                    now,
                ],
            )?;
            if updated == 0 {
                return Err(AppError::not_found("Stavka popisne liste nije pronađena."));
            }
        }
    }

    load_session(connection, session_id)
}

/// The one place a popis changes state. `crate::popis::advance` decides; this
/// writes what it decided.
fn apply_status(
    connection: &Connection,
    id: i64,
    next: PopisStatus,
    now: &str,
) -> Result<(), AppError> {
    if next == PopisStatus::Posted {
        // The v20 CHECK binds `posted` and `posted_at` together, and this UPDATE
        // is the LAST write the session admits (req. 41) — anything else the
        // knjiženje needed must already be durable when it runs.
        connection.execute(
            "UPDATE popis_sessions SET status = ?2, posted_at = ?3, updated_at = ?3 WHERE id = ?1",
            params![id, next.as_db_str(), now],
        )?;
    } else {
        connection.execute(
            "UPDATE popis_sessions SET status = ?2, updated_at = ?3 WHERE id = ?1",
            params![id, next.as_db_str(), now],
        )?;
    }

    Ok(())
}

fn transition(
    connection: &Connection,
    id: i64,
    event: PopisEvent,
    now: &str,
) -> Result<PopisSessionView, AppError> {
    let session = read_session(connection, id)?;
    let next = advance(session.status, event)?;
    apply_status(connection, id, next, now)?;

    load_session(connection, id)
}

pub(crate) fn start_count(
    connection: &Connection,
    id: i64,
    now: &str,
) -> Result<PopisSessionView, AppError> {
    transition(connection, id, PopisEvent::Count, now)
}

pub(crate) fn compute_differences(
    connection: &Connection,
    id: i64,
    now: &str,
) -> Result<PopisSessionView, AppError> {
    transition(connection, id, PopisEvent::Compute, now)
}

/// Req. 41 / PoP čl. 14 st. 3. The flip is the last write the session admits, so
/// it is also the last thing this function does.
pub(crate) fn post_popis(
    connection: &Connection,
    id: i64,
    now: &str,
) -> Result<PopisSessionView, AppError> {
    transition(connection, id, PopisEvent::Post, now)
}

fn clean_signatories(potpisnici: &[String]) -> Result<Vec<String>, AppError> {
    let cleaned: Vec<String> = potpisnici
        .iter()
        .map(|potpisnik| potpisnik.trim().to_string())
        .filter(|potpisnik| !potpisnik.is_empty())
        .collect();

    if cleaned.is_empty() {
        return Err(AppError::business(
            "popis_bez_potpisnika",
            "Popisne liste potpisuju članovi komisije za popis poimenično — bez potpisnika nema \
             potpisa (PoP čl. 8 st. 5, čl. 9 st. 3).",
        ));
    }

    Ok(cleaned)
}

/// SHA-256 over the state that was signed, length-prefixed per field like the
/// audit chain's — so „7“ + „000“ and „7000“ + „“ cannot hash alike.
///
/// The snapshot is deliberately taken from [`load_session`], which is the same
/// blind read the commission itself gets: in Phase A the hash therefore covers the
/// counted state and nothing else, which is exactly what the čl. 8 st. 5 potpis
/// attests to.
fn snapshot_hash(faza: &str, session: &PopisSessionView) -> String {
    fn absorb(hasher: &mut Sha256, value: &str) {
        hasher.update(value.len().to_string().as_bytes());
        hasher.update(b":");
        hasher.update(value.as_bytes());
    }

    let mut hasher = Sha256::new();
    absorb(&mut hasher, faza);
    absorb(&mut hasher, &session.id.to_string());
    absorb(&mut hasher, &session.datum_popisa);
    absorb(&mut hasher, session.status.as_db_str());
    for line in &session.linije {
        absorb(&mut hasher, &line.id.to_string());
        absorb(&mut hasher, &line.lista_vrsta);
        absorb(&mut hasher, line.sifra.as_deref().unwrap_or(""));
        absorb(&mut hasher, &line.naziv);
        // Čl. 8 st. 4's other two — the same set `PotpisanaStavka` pins, so what
        // the hash attests to and what the obračun may not move are one list.
        absorb(&mut hasher, line.vrsta.as_deref().unwrap_or(""));
        absorb(&mut hasher, line.jedinica_mere.as_deref().unwrap_or(""));
        absorb(&mut hasher, &line.stvarna_kolicina_milli.to_string());
        absorb(&mut hasher, line.blizi_opis.as_deref().unwrap_or(""));
        absorb(
            &mut hasher,
            &line
                .knjigovodstvena_kolicina_milli
                .map(|quantity| quantity.to_string())
                .unwrap_or_default(),
        );
        absorb(
            &mut hasher,
            &line
                .cena_minor
                .map(|cena| cena.to_string())
                .unwrap_or_default(),
        );
    }

    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Populates `knjigovodstvena_kolicina_milli` from the perpetual record, for the
/// lines whose šifra resolves to a catalog article that has one.
///
/// **Called only after the čl. 8 st. 5 potpis and the state flip**, which is both
/// the statutory order and the only order the v20 guard admits. A line whose šifra
/// resolves to nothing keeps its NULL — the gotovina and potraživanja liste have
/// no perpetual record behind them, and inventing a zero for them would report a
/// manjak the shop does not have.
fn populate_book_quantities(
    connection: &Connection,
    session_id: i64,
    now: &str,
) -> Result<(), AppError> {
    connection.execute(
        "UPDATE popis_lines
            SET knjigovodstvena_kolicina_milli = (
                    SELECT b.quantity_milli
                      FROM inventory_balances b
                      JOIN products p ON p.id = b.product_id
                     WHERE p.sku = popis_lines.sifra),
                updated_at = ?2
          WHERE session_id = ?1
            AND sifra IS NOT NULL
            AND EXISTS (SELECT 1
                          FROM inventory_balances b
                          JOIN products p ON p.id = b.product_id
                         WHERE p.sku = popis_lines.sifra)",
        params![session_id, now],
    )?;

    Ok(())
}

/// The two signature events of req. 30, in one function because they differ in
/// exactly one thing: čl. 8 st. 5 is what releases the book data, čl. 9 st. 3 is
/// not.
fn sign_phase(
    connection: &mut Connection,
    id: i64,
    faza: &str,
    event: PopisEvent,
    potpisnici: &[String],
    now: &str,
) -> Result<PopisSessionView, AppError> {
    let session = read_session(connection, id)?;
    let next = advance(session.status, event)?;
    let potpisnici = clean_signatories(potpisnici)?;

    let transaction = connection.transaction()?;

    let snapshot = load_session(&transaction, id)?;
    let hash = snapshot_hash(faza, &snapshot);

    // 1. The potpis, FIRST — and not merely by preference. The v20 guard releases
    //    the book column only once this row exists, so an implementation that
    //    populated the quantities first would be aborted by the engine rather than
    //    quietly reordered. It is also the statutory order: čl. 8 st. 5 releases
    //    the data BECAUSE the liste have been signed.
    for potpisnik in &potpisnici {
        transaction.execute(
            "INSERT INTO popis_signatures (session_id, faza, potpisnik, potpisano_at,
                                           snapshot_hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?4)",
            params![id, faza, potpisnik, now, hash],
        )?;
    }

    // 2. The state — the guard's other limb.
    apply_status(&transaction, id, next, now)?;

    // 3. Only now the book quantities, and only for the čl. 8 st. 5 potpis.
    if faza == "a" {
        populate_book_quantities(&transaction, id, now)?;
    }

    let view = load_session(&transaction, id)?;
    transaction.commit()?;

    Ok(view)
}

pub(crate) fn sign_phase_a(
    connection: &mut Connection,
    id: i64,
    potpisnici: &[String],
    now: &str,
) -> Result<PopisSessionView, AppError> {
    sign_phase(connection, id, "a", PopisEvent::SignA, potpisnici, now)
}

pub(crate) fn sign_phase_b(
    connection: &mut Connection,
    id: i64,
    potpisnici: &[String],
    now: &str,
) -> Result<PopisSessionView, AppError> {
    sign_phase(connection, id, "b", PopisEvent::SignB, potpisnici, now)
}

// ---------------------------------------------------------------------------
// The izveštaj o popisu (req. 37) and its rok (req. 38)
// ---------------------------------------------------------------------------

/// The `settings` key the popis module's one configured value lives under.
pub(crate) const POPIS_SETTINGS_KEY: &str = "popis";

/// What the popis module cannot compute for itself.
///
/// **Why the filing deadline is configuration and not a constant.** PoP čl. 13
/// st. 2 counts the annual izveštaj's rok back from the rok za dostavljanje
/// redovnog godišnjeg finansijskog izveštaja, and ZoRač čl. 44 st. 1 sets that at
/// 31 March *„осим ако посебним законом није друкчије уређено“*. The date is
/// therefore not this crate's to own: a special law, or a change to čl. 44, must
/// be a setting the shop's accountant edits and not a release of the app. Nothing
/// is presumed in its place — an unset deadline refuses the annual izveštaj by
/// name rather than dating it with a guess.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopisPodesavanja {
    /// The rok za dostavljanje redovnog godišnjeg finansijskog izveštaja, as
    /// `gggg-MM-dd`. `None` until the shop sets it.
    #[serde(default)]
    pub rok_predaje_fi: Option<String>,
}

pub(crate) fn load_podesavanja(state: &AppState) -> Result<PopisPodesavanja, AppError> {
    super::settings::load_json_setting(state, POPIS_SETTINGS_KEY, PopisPodesavanja::default())
}

/// Validated where it is written rather than where it is used: this date is handed
/// straight to the deadline engine, and a shape it cannot read would surface as a
/// refused izveštaj weeks later, in front of the person who did not type it.
pub(crate) fn save_podesavanja(
    state: &AppState,
    podesavanja: &PopisPodesavanja,
) -> Result<PopisPodesavanja, AppError> {
    let rok_predaje_fi = podesavanja
        .rok_predaje_fi
        .as_deref()
        .map(str::trim)
        .filter(|rok| !rok.is_empty());
    if let Some(rok) = rok_predaje_fi {
        ensure_datum(
            rok,
            "rok za dostavljanje redovnog godišnjeg finansijskog izveštaja",
        )?;
    }

    let sacuvano = PopisPodesavanja {
        rok_predaje_fi: rok_predaje_fi.map(str::to_string),
    };
    super::settings::save_json_setting(state, POPIS_SETTINGS_KEY, &sacuvano)?;

    Ok(sacuvano)
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IzvestajRequest {
    /// Req. 36 — the categories the shop declares are present, checked against the
    /// liste by `crate::popis::ensure_liste_kompletne` before anything is composed.
    /// A parameter for the reason Task 5 gave: presence is a fact about the shop
    /// that no ledger in this database holds.
    ///
    /// **Required on the wire, exactly as `popis_provera_listi` requires it.** The
    /// gate refuses a *declared* lista that is empty, so a request that simply
    /// omitted the field would satisfy it vacuously and compose an izveštaj with no
    /// req. 36 check behind it at all. An explicit empty array still says „ništa
    /// nije prijavljeno“; an absent field says nothing.
    pub prijavljene_liste: Vec<PopisLista>,
    /// The five čl. 13 st. 1 elements the commission writes.
    pub narativ: IzvestajNarativ,
}

/// One of the eight čl. 13 st. 1 elements as it appears in the izveštaj.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IzvestajElementView {
    pub element: IzvestajElement,
    pub naziv: String,
    pub pravni_osnov: String,
    pub uputstvo: String,
    /// The text written for it, or `None` for the three the popis itself answers —
    /// their content is the figures in `liste` and `ukupno`.
    pub tekst: Option<String>,
}

/// The čl. 9 st. 1 t. 4 and t. 6 figures behind elements one to three, over one
/// lista or over the whole popis.
///
/// **Three counts sit beside the three amounts and they are not decoration.** A
/// stavka with no cena cannot be valued and a stavka with no knjigovodstveno
/// stanje has no razlika; counting either as zero would report a total that is not
/// a total and a manjak the shop does not have. So the unvalued and the unbooked
/// are counted, `potpuno` says whether there were any, and the izveštaj carries a
/// warning naming them.
///
/// No natural total is reported. Stavke on one lista can be in komadima, metrima
/// and kilogramima at once, and a single summed količina across them would be a
/// number with no unit — the naturalna razlika belongs per stavka, on the popisna
/// lista, which is where čl. 9 st. 1 t. 4 puts it.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IzvestajZbir {
    pub broj_stavki: i64,
    pub stavke_bez_cene: i64,
    pub stavke_bez_knjigovodstvenog_stanja: i64,
    pub stavke_sa_viskom: i64,
    pub stavke_sa_manjkom: i64,
    /// Čl. 9 st. 1 t. 6 — the value of what was counted, over the stavke that carry
    /// a cena.
    pub vrednost_po_popisu_minor: i64,
    /// The same, valued at the knjigovodstvena količina — over the stavke that
    /// carry both a cena and a book quantity.
    pub vrednost_po_knjigama_minor: i64,
    /// The vrednosna razlika over exactly those stavke: negative is a manjak.
    pub vrednosna_razlika_minor: i64,
    /// True when every stavka carried both a cena and a knjigovodstveno stanje, so
    /// the three amounts above cover the whole lista.
    pub potpuno: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IzvestajListaPregled {
    pub vrsta: PopisLista,
    pub naziv: String,
    pub pravni_osnov: String,
    pub zbir: IzvestajZbir,
}

/// PoP čl. 14 st. 2 — the odluka o usvajanju izveštaja, surfaced **with** the
/// izveštaj as one milestone (req. 38) because the article gives it the rok „из
/// члана 13. став 2“: it is the same date, not a second deadline.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OdlukaOUsvajanjuView {
    pub rok: String,
    pub pravni_osnov: String,
    /// Čl. 14 st. 2 names the organ upravljanja „односно предузетник“, and for this
    /// shop čl. 4 st. 2 → ZoRač čl. 43 st. 3 makes that the owner personally.
    pub donosilac: String,
    /// What this application does **not** do with the decision.
    pub napomena: String,
}

/// The izveštaj o popisu (req. 37), composed from the popis and the commission's
/// own words.
///
/// **Composed, not stored.** Nothing in this schema records an izveštaj: the
/// document is assembled when it is asked for and handed back, and the copy says
/// so in `upozorenja` rather than letting a shop assume the app is keeping it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IzvestajView {
    pub session_id: i64,
    pub vrsta: PopisVrsta,
    pub status: PopisStatus,
    pub obveznik: String,
    pub pib: String,
    pub maticni_broj: String,
    pub prodajno_mesto: String,
    pub datum_popisa: String,
    pub period_from: Option<String>,
    pub period_to: Option<String>,
    pub komisija: Vec<KomisijaClanView>,
    pub potpisi: Vec<PopisSignatureView>,
    /// All eight čl. 13 st. 1 elements, in the article's order.
    pub elementi: Vec<IzvestajElementView>,
    /// All six liste, the empty ones included.
    pub liste: Vec<IzvestajListaPregled>,
    pub ukupno: IzvestajZbir,
    /// PoP čl. 13 st. 2, computed by `crate::popis::izvestaj_due`.
    pub rok: String,
    pub rok_pravni_osnov: String,
    pub odluka_o_usvajanju: OdlukaOUsvajanjuView,
    pub upozorenja: Vec<String>,
}

fn vrednost(kolicina_milli: i64, cena_minor: i64) -> Result<i64, AppError> {
    vrednost_minor(kolicina_milli, cena_minor).ok_or_else(|| {
        AppError::business(
            "popis_izvestaj_vrednost_prevelika",
            "Vrednost stavke popisne liste je prevelika za obračun — proverite količinu i cenu.",
        )
    })
}

fn saberi(zbir: i64, iznos: i64) -> Result<i64, AppError> {
    zbir.checked_add(iznos).ok_or_else(|| {
        AppError::business(
            "popis_izvestaj_vrednost_prevelika",
            "Zbir vrednosti u izveštaju o popisu je prevelik za obračun.",
        )
    })
}

/// The rollup behind čl. 13 st. 1's first three elements. Every amount is integer
/// para throughout — `crate::popis::vrednost_minor` does the arithmetic and a
/// figure that will not fit is refused rather than wrapped, because a wrapped
/// value is a manjak reported as a višak.
fn zbir_stavki<'a>(
    linije: impl Iterator<Item = &'a PopisLineView>,
) -> Result<IzvestajZbir, AppError> {
    let mut zbir = IzvestajZbir::default();

    for linija in linije {
        zbir.broj_stavki += 1;

        match linija.cena_minor {
            None => zbir.stavke_bez_cene += 1,
            Some(cena) => {
                zbir.vrednost_po_popisu_minor = saberi(
                    zbir.vrednost_po_popisu_minor,
                    vrednost(linija.stvarna_kolicina_milli, cena)?,
                )?;
                if let Some(knjigovodstvena) = linija.knjigovodstvena_kolicina_milli {
                    zbir.vrednost_po_knjigama_minor = saberi(
                        zbir.vrednost_po_knjigama_minor,
                        vrednost(knjigovodstvena, cena)?,
                    )?;
                }
                if let Some(razlika) = linija.razlika_milli {
                    zbir.vrednosna_razlika_minor =
                        saberi(zbir.vrednosna_razlika_minor, vrednost(razlika, cena)?)?;
                }
            }
        }

        match linija.razlika_milli {
            None => zbir.stavke_bez_knjigovodstvenog_stanja += 1,
            Some(razlika) if razlika > 0 => zbir.stavke_sa_viskom += 1,
            Some(razlika) if razlika < 0 => zbir.stavke_sa_manjkom += 1,
            Some(_) => {}
        }
    }

    zbir.potpuno = zbir.stavke_bez_cene == 0 && zbir.stavke_bez_knjigovodstvenog_stanja == 0;

    Ok(zbir)
}

/// Req. 37 — the izveštaj o popisu, a **structured template with the eight čl. 13
/// st. 1 elements**, never free text.
///
/// Four gates, in the order the duties bite:
///
/// 1. **Čl. 8 st. 5 (req. 29).** The izveštaj carries the knjigovodstveno stanje
///    and the razlike, so composing one during the count would put in the
///    commission's hands, through a document, exactly what the blind count keeps
///    back. This is the one refusal here that is a breach to get wrong.
/// 2. **Req. 36.** A category the shop declared present with an empty lista stops
///    the izveštaj, through Task 5's own refusal — čl. 13 st. 1 has this document
///    report the stvarno stanje of the popis, and a popis missing a declared lista
///    reports a stanje that is not the shop's.
/// 3. **Req. 37.** Every prescribed element must be answered.
/// 4. **Req. 38.** The rok is computed by `crate::popis::izvestaj_due` from the
///    configured filing deadline; an annual izveštaj with none configured is
///    refused rather than dated by guesswork.
///
/// Read-only from end to end. The izveštaj is not a row in this database and
/// composing one writes nothing.
pub(crate) fn compose_izvestaj(
    state: &AppState,
    id: i64,
    request: &IzvestajRequest,
) -> Result<IzvestajView, AppError> {
    let connection = state.db().open()?;
    let session = load_session(&connection, id)?;

    if !session.knjigovodstvo_dostupno {
        return Err(AppError::business(
            "popis_izvestaj_pre_potpisa",
            "Izveštaj o popisu sadrži knjigovodstveno stanje i razlike (PoP čl. 13 st. 1), pa se \
             ne sastavlja pre nego što se stvarno stanje unese u popisne liste i pre nego što \
             članovi komisije potpišu te liste (PoP čl. 8 st. 5).",
        ));
    }

    crate::popis::ensure_liste_kompletne(
        &request.prijavljene_liste,
        &liste_sa_stavkama(&session.liste),
    )?;
    ensure_izvestaj_kompletan(&request.narativ)?;

    let podesavanja = load_podesavanja(state)?;
    let rok = izvestaj_due(
        session.vrsta,
        &session.datum_popisa,
        podesavanja.rok_predaje_fi.as_deref(),
    )?;

    let company = super::settings::load_company_settings(state)?;
    let ukupno = zbir_stavki(session.linije.iter())?;
    let mut liste = Vec::with_capacity(session.liste.len());
    for pregled in &session.liste {
        liste.push(IzvestajListaPregled {
            vrsta: pregled.vrsta,
            naziv: pregled.naziv.clone(),
            pravni_osnov: pregled.pravni_osnov.clone(),
            zbir: zbir_stavki(
                session
                    .linije
                    .iter()
                    .filter(|linija| linija.lista_vrsta == pregled.vrsta.as_db_str()),
            )?,
        });
    }

    let mut upozorenja = komisija_upozorenja(&session.komisija);

    // Req. 31 / čl. 9 st. 3 — „уз штампање пописних листа које потписују чланови
    // комисије“. The izveštaj rests on those liste, so an izveštaj drafted before
    // they are signed rests on a document that is not yet evidence. A warning and
    // not a refusal: drafting the izveštaj early is not itself a breach, and this
    // module does not invent duties the bylaw does not impose.
    if !session.faza_b_potpisana {
        upozorenja.push(
            "Obračunate popisne liste još nisu potpisane (PoP čl. 9 st. 3) — izveštaj se \
             sastavlja na osnovu potpisanih listi. Sastavljanje nije zaustavljeno."
                .to_string(),
        );
    }

    if ukupno.stavke_bez_cene > 0 {
        upozorenja.push(format!(
            "Vrednosti u izveštaju nisu potpune: {} stavki nema upisanu cenu (PoP čl. 9 st. 1 \
             t. 5), pa te stavke ulaze u popis, ali ne i u vrednosne zbirove.",
            ukupno.stavke_bez_cene
        ));
    }
    if ukupno.stavke_bez_knjigovodstvenog_stanja > 0 {
        upozorenja.push(format!(
            "Razlike u izveštaju ne obuhvataju sve stavke: {} stavki nema knjigovodstveno stanje \
             (PoP čl. 9 st. 1 t. 3), pa se za njih razlika ne utvrđuje.",
            ukupno.stavke_bez_knjigovodstvenog_stanja
        ));
    }

    // A rok that falls before the popis was taken is arithmetic doing what it was
    // told with a stale setting — the filing deadline of a business year that has
    // already closed. Detectable without reading a clock, which is why it is
    // checked here at all; and a warning rather than a refusal, because a popis
    // taken after its rok is still a popis whose izveštaj has to be written.
    if rok < session.datum_popisa {
        let podeseno = podesavanja
            .rok_predaje_fi
            .as_deref()
            .map(|rok| format!(" Podešeni rok za dostavljanje finansijskog izveštaja je „{rok}“."))
            .unwrap_or_default();
        upozorenja.push(format!(
            "Rok za izveštaj o popisu („{rok}“) pada pre datuma popisa („{}“).{podeseno} \
             Proverite podešavanje — rok je verovatno zastareo.",
            session.datum_popisa
        ));
    }

    if liste_sa_stavkama(&session.liste).contains(&PopisLista::Konsignacija) {
        let (_, podsetnik) = konsignacija_podsetnik(&session.datum_popisa);
        upozorenja.push(podsetnik);
    }

    upozorenja.push(
        "Ovaj izveštaj se sastavlja u trenutku kada se zatraži i ne čuva se u aplikaciji — \
         odštampajte ga i čuvajte uz popisne liste."
            .to_string(),
    );

    Ok(IzvestajView {
        session_id: session.id,
        vrsta: session.vrsta,
        status: session.status,
        obveznik: company.shop_name,
        pib: company.pib,
        maticni_broj: company.registration_number,
        prodajno_mesto: session.prodajno_mesto,
        datum_popisa: session.datum_popisa,
        period_from: session.period_from,
        period_to: session.period_to,
        komisija: session.komisija,
        potpisi: session.potpisi,
        elementi: IzvestajElement::ALL
            .into_iter()
            .map(|element| IzvestajElementView {
                element,
                naziv: element.naziv().to_string(),
                pravni_osnov: element.pravni_osnov().to_string(),
                uputstvo: element.uputstvo().to_string(),
                tekst: request
                    .narativ
                    .tekst(element)
                    .map(|tekst| tekst.trim().to_string()),
            })
            .collect(),
        liste,
        ukupno,
        odluka_o_usvajanju: OdlukaOUsvajanjuView {
            rok: rok.clone(),
            pravni_osnov: "PoP čl. 14 st. 2".to_string(),
            donosilac: "preduzetnik lično (PoP čl. 4 st. 2 u vezi sa ZoRač čl. 43 st. 3)"
                .to_string(),
            napomena: "Aplikacija ne evidentira odluku o usvajanju izveštaja o popisu — donesite \
                       je u istom roku i čuvajte je uz izveštaj."
                .to_string(),
        },
        rok,
        rok_pravni_osnov: "PoP čl. 13 st. 2".to_string(),
        upozorenja,
    })
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn popis_list(state: State<'_, AppState>) -> Result<Vec<PopisSummary>, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    list_sessions(&connection).map_err(Into::into)
}

#[tauri::command]
pub fn popis_get(state: State<'_, AppState>, id: i64) -> Result<PopisSessionView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    load_session(&connection, id).map_err(Into::into)
}

/// Req. 36 — which of the declared categories still has an empty lista.
///
/// The declaration is a parameter and not a stored flag because presence is a
/// fact about the shop that no ledger here holds: nothing in the books says that
/// part of the stock is damaged, that a carton is at a third party's, or that a
/// rail belongs to somebody else on consignment. The person taking the popis
/// answers, and the answer is checked against the liste.
#[tauri::command]
pub fn popis_provera_listi(
    state: State<'_, AppState>,
    id: i64,
    prijavljene: Vec<PopisLista>,
) -> Result<ProveraListiView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    provera_listi(&connection, id, &prijavljene).map_err(Into::into)
}

#[tauri::command]
pub fn popis_open(
    state: State<'_, AppState>,
    request: OpenPopisRequest,
) -> Result<PopisSessionView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    open_popis(&mut connection, &request, &now).map_err(Into::into)
}

#[tauri::command]
pub fn popis_save_line(
    state: State<'_, AppState>,
    session_id: i64,
    line_id: Option<i64>,
    input: PopisLineInput,
) -> Result<PopisSessionView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    save_line(&connection, session_id, line_id, &input, &now).map_err(Into::into)
}

#[tauri::command]
pub fn popis_start_count(
    state: State<'_, AppState>,
    id: i64,
) -> Result<PopisSessionView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    start_count(&connection, id, &now).map_err(Into::into)
}

#[tauri::command]
pub fn popis_sign_phase_a(
    state: State<'_, AppState>,
    id: i64,
    potpisnici: Vec<String>,
) -> Result<PopisSessionView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    sign_phase_a(&mut connection, id, &potpisnici, &now).map_err(Into::into)
}

#[tauri::command]
pub fn popis_compute(
    state: State<'_, AppState>,
    id: i64,
) -> Result<PopisSessionView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    compute_differences(&connection, id, &now).map_err(Into::into)
}

#[tauri::command]
pub fn popis_sign_phase_b(
    state: State<'_, AppState>,
    id: i64,
    potpisnici: Vec<String>,
) -> Result<PopisSessionView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    sign_phase_b(&mut connection, id, &potpisnici, &now).map_err(Into::into)
}

#[tauri::command]
pub fn popis_post(state: State<'_, AppState>, id: i64) -> Result<PopisSessionView, CommandError> {
    super::auth::require_admin(state.inner())?;
    let connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    post_popis(&connection, id, &now).map_err(Into::into)
}

#[tauri::command]
pub fn popis_podesavanja_get(state: State<'_, AppState>) -> Result<PopisPodesavanja, CommandError> {
    super::auth::require_admin(state.inner())?;
    load_podesavanja(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn popis_podesavanja_set(
    state: State<'_, AppState>,
    podesavanja: PopisPodesavanja,
) -> Result<PopisPodesavanja, CommandError> {
    super::auth::require_admin(state.inner())?;
    save_podesavanja(state.inner(), &podesavanja).map_err(Into::into)
}

/// Req. 37. Read-only: the izveštaj is composed from the popis and the narrative
/// the caller supplies, and nothing about the popis changes when it is asked for.
#[tauri::command]
pub fn popis_izvestaj(
    state: State<'_, AppState>,
    id: i64,
    request: IzvestajRequest,
) -> Result<IzvestajView, CommandError> {
    super::auth::require_admin(state.inner())?;
    compose_izvestaj(state.inner(), id, &request).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use tauri::Manager;

    use super::*;
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

    /// The commands take a Tauri `State`, so a headless mock app is needed to
    /// obtain a real managed state — the pattern `reklamacije.rs` established.
    fn with_app(test_name: &str, test: impl FnOnce(&tauri::App<tauri::test::MockRuntime>)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let app = tauri::test::mock_builder()
                .manage(AppState::new(db))
                .build(tauri::test::mock_context(tauri::test::noop_assets()))
                .expect("mock app should build");
            test(&app);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    fn sign_in_admin(state: &AppState) {
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
    }

    fn sign_in_cashier(state: &AppState) {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, created_at, updated_at)
                 VALUES ('kasir', 'Kasir', 'cashier', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        let cashier_id = connection.last_insert_rowid();
        state
            .set_session_user_id(cashier_id)
            .expect("cashier session should set");
    }

    /// One catalog article with a perpetual balance — the „druga strana“ of the
    /// popis and, during Phase A, the data čl. 8 st. 5 keeps away from the
    /// commission. The balance is deliberately a number that appears nowhere else.
    const KNJIGOVODSTVENO_STANJE_MILLI: i64 = 61_237;

    fn seed_article(state: &AppState, sku: &str) {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT OR IGNORE INTO tax_rates (name, rate_basis_points, created_at, updated_at)
                 VALUES ('Opšta', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("tax rate should insert");
        connection
            .execute(
                "INSERT INTO products (name, sku, unit_of_measure, sale_price_minor,
                                       purchase_price_minor, tax_rate_id, created_at, updated_at)
                 VALUES ('Košulja', ?1, 'kom', 249900, 120000,
                         (SELECT id FROM tax_rates ORDER BY id LIMIT 1),
                         '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                params![sku],
            )
            .expect("article should insert");
        let product_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO inventory_balances (product_id, quantity_milli, updated_at)
                 VALUES (?1, ?2, '2026-01-01T00:00:00Z')",
                params![product_id, KNJIGOVODSTVENO_STANJE_MILLI],
            )
            .expect("balance should insert");
    }

    fn open_request() -> OpenPopisRequest {
        OpenPopisRequest {
            vrsta: PopisVrsta::Godisnji,
            prodajno_mesto: "Butik Centar".into(),
            datum_popisa: "2026-12-31".into(),
            period_from: Some("2026-01-01".into()),
            period_to: Some("2026-12-31".into()),
            plan_rada_json: Some(r#"{"zadaci":["brojanje robe"]}"#.into()),
            odluka_ref: Some("Odluka 3/2026".into()),
            perpetual_odluka_ref: None,
            uskladjivanje_potvrdjeno: true,
            komisija: vec![KomisijaClanInput {
                ime: "Miloš Đurđević".into(),
                uloga: "jedno_lice".into(),
                rukuje_imovinom: false,
            }],
        }
    }

    fn line_input(sifra: &str, stvarna_kolicina_milli: i64) -> PopisLineInput {
        PopisLineInput {
            lista_vrsta: "roba".into(),
            sifra: Some(sifra.into()),
            naziv: "Košulja".into(),
            vrsta: Some("Ženska konfekcija".into()),
            jedinica_mere: Some("kom".into()),
            stvarna_kolicina_milli,
            blizi_opis: Some("Polica A2".into()),
            knjigovodstvena_kolicina_milli: None,
            cena_minor: None,
        }
    }

    /// One line for each posebna popisna lista of req. 36, carrying the field that
    /// lista exists to record: what is wrong with the goods (čl. 10 st. 3), where
    /// they are (čl. 10 st. 4), which apoen was counted (čl. 11 st. 1), the iznos
    /// of an undocumented claim (čl. 12 st. 2) and whose the goods are (čl. 2
    /// st. 5).
    fn lista_line(lista: PopisLista) -> PopisLineInput {
        let mut input = PopisLineInput {
            lista_vrsta: lista.as_db_str().into(),
            sifra: None,
            naziv: String::new(),
            vrsta: None,
            jedinica_mere: Some("kom".into()),
            stvarna_kolicina_milli: 0,
            blizi_opis: None,
            knjigovodstvena_kolicina_milli: None,
            cena_minor: None,
        };

        match lista {
            PopisLista::Roba => {
                input.sifra = Some("KOS-1".into());
                input.naziv = "Košulja".into();
                input.stvarna_kolicina_milli = 7_000;
                input.blizi_opis = Some("Polica A2".into());
            }
            PopisLista::Ostecena => {
                input.naziv = "Košulja sa oštećenjem".into();
                input.stvarna_kolicina_milli = 1_000;
                input.blizi_opis = Some("Pocepan rukav — za otpis".into());
            }
            PopisLista::VanObjekta => {
                input.naziv = "Mantil".into();
                input.stvarna_kolicina_milli = 2_000;
                input.blizi_opis = Some("Na popravci kod krojača".into());
            }
            PopisLista::Gotovina => {
                input.naziv = "Novčanica 1.000 RSD".into();
                input.stvarna_kolicina_milli = 5_000;
                input.cena_minor = Some(100_000);
            }
            PopisLista::Potrazivanja => {
                input.naziv = "Potraživanje bez isprave".into();
                input.blizi_opis = Some("Dobavljač „Tekstil promet“".into());
                input.cena_minor = Some(350_000);
            }
            PopisLista::Konsignacija => {
                input.naziv = "Torba".into();
                input.stvarna_kolicina_milli = 3_000;
                input.blizi_opis = Some("Vlasnik: „Moda“ doo".into());
            }
        }

        input
    }

    /// Opens a session, records one counted line for `sifra`, and leaves it in
    /// `counting` — the state every čl. 8 st. 5 assertion is about.
    fn seeded_count(state: &AppState, sifra: &str) -> i64 {
        seeded_count_on(state, sifra, "2026-12-31")
    }

    fn seeded_count_on(state: &AppState, sifra: &str, datum_popisa: &str) -> i64 {
        seed_article(state, sifra);
        let mut request = open_request();
        request.datum_popisa = datum_popisa.into();
        let mut connection = state.db().open().expect("database should open");
        let session = open_popis(&mut connection, &request, "2026-12-31T08:00:00Z")
            .expect("the popis should open");
        start_count(&connection, session.id, "2026-12-31T09:00:00Z")
            .expect("the count should start");
        save_line(
            &connection,
            session.id,
            None,
            &line_input(sifra, 7_000),
            "2026-12-31T09:10:00Z",
        )
        .expect("a counted line should save");
        session.id
    }

    fn session_status(state: &AppState, id: i64) -> String {
        state
            .db()
            .open()
            .expect("database should open")
            .query_row(
                "SELECT status FROM popis_sessions WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .expect("the session should exist")
    }

    fn signature_count(state: &AppState, id: i64) -> i64 {
        state
            .db()
            .open()
            .expect("database should open")
            .query_row(
                "SELECT COUNT(*) FROM popis_signatures WHERE session_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .expect("the count should read")
    }

    fn stored_book_quantity(state: &AppState, id: i64) -> Option<i64> {
        state
            .db()
            .open()
            .expect("database should open")
            .query_row(
                "SELECT knjigovodstvena_kolicina_milli FROM popis_lines
                 WHERE session_id = ?1 ORDER BY id LIMIT 1",
                params![id],
                |row| row.get(0),
            )
            .expect("the line should exist")
    }

    fn session_count(state: &AppState) -> i64 {
        state
            .db()
            .open()
            .expect("database should open")
            .query_row("SELECT COUNT(*) FROM popis_sessions", [], |row| row.get(0))
            .expect("the count should read")
    }

    /// Walks a seeded count all the way to `posted`.
    fn walk_to_posted(state: &AppState, sifra: &str) -> i64 {
        walk_to_posted_on(state, sifra, "2026-12-31")
    }

    fn walk_to_posted_on(state: &AppState, sifra: &str, datum_popisa: &str) -> i64 {
        let id = seeded_count_on(state, sifra, datum_popisa);
        let mut connection = state.db().open().expect("database should open");
        sign_phase_a(
            &mut connection,
            id,
            &["Miloš Đurđević".to_string()],
            "2026-12-31T17:00:00Z",
        )
        .expect("the čl. 8 st. 5 potpis should record");
        compute_differences(&connection, id, "2027-01-02T09:00:00Z")
            .expect("the obračun should open");
        sign_phase_b(
            &mut connection,
            id,
            &["Miloš Đurđević".to_string()],
            "2027-01-03T10:00:00Z",
        )
        .expect("the čl. 9 st. 3 potpis should record");
        post_popis(&connection, id, "2027-01-05T11:00:00Z").expect("the result should post");
        id
    }

    // -----------------------------------------------------------------
    // Req. 39 — the ZoRač čl. 20 st. 3 reconciliation gate
    // -----------------------------------------------------------------

    /// ZoRač čl. 20 st. 3 legislates an ordering: the books are reconciled and
    /// then the popis is taken. So this is a gate, not a checklist item — the
    /// session does not come into existence unconfirmed.
    #[test]
    fn opening_without_the_reconciliation_confirmation_is_refused() {
        with_app("popis_open_without_reconciliation", |app| {
            let state = app.state::<AppState>();
            let mut connection = state.db().open().expect("database should open");

            let mut request = open_request();
            request.uskladjivanje_potvrdjeno = false;
            let error = open_popis(&mut connection, &request, "2026-12-31T08:00:00Z")
                .expect_err("an unreconciled popis must not open");

            assert_eq!(error.code(), "popis_uskladjivanje_nije_potvrdjeno");
            assert!(
                error.to_string().contains("čl. 20 st. 3"),
                "the refusal must name the article it enforces, said: {error}"
            );
            assert_eq!(
                session_count(state.inner()),
                0,
                "a refused open must persist no session"
            );
        });
    }

    #[test]
    fn opening_records_when_the_reconciliation_was_confirmed() {
        with_app("popis_open_records_reconciliation", |app| {
            let state = app.state::<AppState>();
            let mut connection = state.db().open().expect("database should open");

            let session = open_popis(&mut connection, &open_request(), "2026-12-31T08:00:00Z")
                .expect("a reconciled popis should open");

            assert_eq!(
                session.uskladjivanje_potvrdjeno_at.as_deref(),
                Some("2026-12-31T08:00:00Z"),
                "the confirmation is recorded, not merely required"
            );
            assert_eq!(session.status, PopisStatus::Draft);
        });
    }

    // -----------------------------------------------------------------
    // Req. 29 — the čl. 8 st. 5 blind count
    // -----------------------------------------------------------------

    /// The plan's own words: refused **at the boundary**. Not dropped, not hidden
    /// — a caller that tried to put book data in front of the commission is told
    /// that is what it did.
    #[test]
    fn a_phase_a_line_write_carrying_a_book_quantity_is_refused_at_the_boundary() {
        with_app("popis_line_book_quantity_refused", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let connection = state.db().open().expect("database should open");

            let mut input = line_input("KOS-2", 3_000);
            input.knjigovodstvena_kolicina_milli = Some(9_000);
            let error = save_line(&connection, id, None, &input, "2026-12-31T09:20:00Z")
                .expect_err("a book quantity in Phase A must be refused");

            assert_eq!(error.code(), "popis_knjigovodstvo_pre_potpisa");
            assert!(
                error.to_string().contains("čl. 8 st. 5"),
                "the refusal must name the article, said: {error}"
            );

            let lines: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM popis_lines WHERE session_id = ?1",
                    params![id],
                    |row| row.get(0),
                )
                .expect("the count should read");
            assert_eq!(lines, 1, "a refused line write must persist nothing");
        });
    }

    /// The half a refused write does not cover. The v20 trigger protects
    /// `popis_lines.knjigovodstvena_kolicina_milli` and nothing else, so the book
    /// stock in `inventory_balances` is readable by any query at any time — and a
    /// read path that joined it would breach čl. 8 st. 5 with every other test
    /// still green. The assertion is therefore on the whole serialized response:
    /// the perpetual quantity must appear nowhere in it. The second half of the
    /// test is what keeps the first from being vacuous — after the potpis the same
    /// number must be there.
    #[test]
    fn a_phase_a_read_carries_no_book_quantity_from_any_source() {
        with_app("popis_read_blind_in_phase_a", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let mut connection = state.db().open().expect("database should open");

            let counting = load_session(&connection, id).expect("the session should load");
            assert_eq!(counting.status, PopisStatus::Counting);
            assert!(!counting.knjigovodstvo_dostupno);
            assert_eq!(counting.linije.len(), 1);
            assert_eq!(counting.linije[0].knjigovodstvena_kolicina_milli, None);
            assert_eq!(counting.linije[0].razlika_milli, None);

            let blind = serde_json::to_string(&counting).expect("the view should serialize");
            assert!(
                !blind.contains(&KNJIGOVODSTVENO_STANJE_MILLI.to_string()),
                "the perpetual book stock reached the commission during the count: {blind}"
            );

            sign_phase_a(
                &mut connection,
                id,
                &["Miloš Đurđević".to_string()],
                "2026-12-31T17:00:00Z",
            )
            .expect("the čl. 8 st. 5 potpis should record");

            let released = load_session(&connection, id).expect("the session should load");
            let visible = serde_json::to_string(&released).expect("the view should serialize");
            assert!(
                visible.contains(&KNJIGOVODSTVENO_STANJE_MILLI.to_string()),
                "after the potpis the book stock must be reachable, or the test above proves \
                 nothing: {visible}"
            );
        });
    }

    // -----------------------------------------------------------------
    // The čl. 8 st. 5 potpis
    // -----------------------------------------------------------------

    /// Signing A does three writes that must all land or none: the potpis, the
    /// state, and the book quantities read out of the perpetual record. The order
    /// is not decoration — the v20 trigger refuses a book quantity until both the
    /// potpis exists and the session has left Phase A, so an implementation that
    /// wrote the quantities first would abort here rather than pass.
    #[test]
    fn signing_phase_a_takes_the_potpis_and_only_then_releases_the_book_quantities() {
        with_app("popis_sign_phase_a_populates", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let mut connection = state.db().open().expect("database should open");

            let signed = sign_phase_a(
                &mut connection,
                id,
                &["Miloš Đurđević".to_string()],
                "2026-12-31T17:00:00Z",
            )
            .expect("the potpis should record");

            assert_eq!(signed.status, PopisStatus::CountedSigned);
            assert!(signed.faza_a_potpisana);
            assert!(signed.knjigovodstvo_dostupno);
            assert_eq!(signed.potpisi.len(), 1);
            assert_eq!(signed.potpisi[0].faza, "a");
            assert_eq!(signed.potpisi[0].potpisano_at, "2026-12-31T17:00:00Z");
            assert!(
                !signed.potpisi[0].snapshot_hash.is_empty(),
                "a potpis with no snapshot behind it attests to nothing"
            );

            assert_eq!(
                signed.linije[0].knjigovodstvena_kolicina_milli,
                Some(KNJIGOVODSTVENO_STANJE_MILLI),
                "the book quantity comes from the perpetual record"
            );
            assert_eq!(
                signed.linije[0].razlika_milli,
                Some(7_000 - KNJIGOVODSTVENO_STANJE_MILLI),
                "čl. 9 st. 1 t. 4 — the naturalna razlika is counted minus book"
            );
            assert_eq!(
                stored_book_quantity(state.inner(), id),
                Some(KNJIGOVODSTVENO_STANJE_MILLI)
            );
        });
    }

    /// The observable half of „in one transaction“: a refused potpis leaves the
    /// popis exactly as it was — still counting, unsigned, and blind. An empty
    /// `now` fails the v20 `potpisano_at <> ''` CHECK.
    ///
    /// It is only half. Every write in the step is stamped with the same `now` and
    /// guarded by the same `<> ''` CHECK, so the earliest write always fails first
    /// and **no reachable input leaves a durable half to observe** — verified by
    /// mutation: running the three writes on the bare connection leaves this test
    /// green. The other half is asserted structurally by
    /// [`the_signature_step_runs_inside_one_transaction`], which is where that
    /// property actually lives.
    #[test]
    fn a_failed_phase_a_signature_leaves_the_popis_exactly_as_it_was() {
        with_app("popis_sign_phase_a_rolls_back", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let mut connection = state.db().open().expect("database should open");

            sign_phase_a(&mut connection, id, &["Miloš Đurđević".to_string()], "")
                .expect_err("an unwritable stamp must fail the whole step");

            assert_eq!(session_status(state.inner(), id), "counting");
            assert_eq!(signature_count(state.inner(), id), 0);
            assert_eq!(stored_book_quantity(state.inner(), id), None);
        });
    }

    /// The structural half of the plan's „in one transaction“, asserted here
    /// because no reachable input can demonstrate it behaviourally (see the test
    /// above). What matters legally is that the three writes are indivisible: a
    /// popis carrying a čl. 8 st. 5 potpis whose book quantities never landed, or
    /// book quantities behind a potpis that was rolled back, is a document that
    /// says something untrue about when the commission saw what. So the step's own
    /// source is asserted — every write between the `transaction()` and the
    /// `commit()`.
    ///
    /// Verified load-bearing by mutation: running the three writes on the bare
    /// connection instead leaves this the only failing test in the module.
    #[test]
    fn the_signature_step_runs_inside_one_transaction() {
        const SOURCE: &str = include_str!("popis.rs");

        let step = SOURCE
            .split("fn sign_phase(")
            .nth(1)
            .expect("the signature step must exist")
            .split("\npub(crate) fn ")
            .next()
            .expect("the signature step must end somewhere");

        let opened = step
            .find("connection.transaction()")
            .expect("the čl. 8 st. 5 step must open a transaction");
        let committed = step
            .rfind("transaction.commit()")
            .expect("the čl. 8 st. 5 step must commit it");

        for write in [
            "INSERT INTO popis_signatures",
            "apply_status(",
            "populate_book_quantities(",
        ] {
            let at = step
                .find(write)
                .unwrap_or_else(|| panic!("`{write}` should be part of the signature step"));
            assert!(
                at > opened && at < committed,
                "`{write}` runs outside the transaction, so a failure after it would leave a \
                 half-signed popis behind"
            );
        }
    }

    /// One named single-field change, so every assertion below is about exactly
    /// one field and a case that stops failing says which one stopped.
    type IzmenaLinije = (&'static str, fn(&mut PopisLineView));
    type IzmenaUnosa = (&'static str, fn(&mut PopisLineInput));

    fn potpisana_linija() -> PopisLineView {
        PopisLineView {
            id: 1,
            lista_vrsta: "roba".into(),
            sifra: Some("KOS-1".into()),
            naziv: "Košulja".into(),
            vrsta: Some("Ženska konfekcija".into()),
            jedinica_mere: Some("kom".into()),
            stvarna_kolicina_milli: 7_000,
            blizi_opis: Some("Polica A2".into()),
            knjigovodstvena_kolicina_milli: None,
            razlika_milli: None,
            cena_minor: None,
        }
    }

    fn view_of(linija: PopisLineView) -> PopisSessionView {
        PopisSessionView {
            id: 1,
            vrsta: PopisVrsta::Godisnji,
            prodajno_mesto: "Butik Centar".into(),
            datum_popisa: "2026-12-31".into(),
            period_from: None,
            period_to: None,
            status: PopisStatus::Counting,
            plan_rada_json: None,
            odluka_ref: None,
            perpetual_odluka_ref: None,
            uskladjivanje_potvrdjeno_at: None,
            posted_at: None,
            faza_a_potpisana: false,
            faza_b_potpisana: false,
            knjigovodstvo_dostupno: false,
            komisija: Vec::new(),
            potpisi: Vec::new(),
            linije: vec![linija],
            liste: Vec::new(),
            konsignacija_rok: None,
            upozorenja: Vec::new(),
        }
    }

    /// A potpis attests to what it hashed, and `save_line` refuses to move what
    /// the potpis froze — so the two lists have to be the same list. The čl. 8
    /// st. 4 identity (lista, šifra, naziv, vrsta, jedinica mere) and the čl. 9
    /// st. 1 t. 1 count with its bliži opis are pinned by `PotpisanaStavka`, and
    /// each of them is asserted here to change the snapshot. A field pinned but
    /// unhashed would be guarded only by the command layer; a field hashed but
    /// unpinned would let the stored hash quietly stop describing the row.
    #[test]
    fn the_snapshot_hash_covers_every_field_the_potpis_freezes() {
        let osnovni = snapshot_hash("a", &view_of(potpisana_linija()));

        let izmene: [IzmenaLinije; 7] = [
            ("lista_vrsta", |linija| {
                linija.lista_vrsta = "gotovina".into()
            }),
            ("šifra", |linija| linija.sifra = Some("KOS-2".into())),
            ("naziv", |linija| linija.naziv = "Nešto sasvim drugo".into()),
            ("vrsta", |linija| {
                linija.vrsta = Some("Muška konfekcija".into())
            }),
            ("jedinica mere", |linija| {
                linija.jedinica_mere = Some("kg".into())
            }),
            ("stvarna količina", |linija| {
                linija.stvarna_kolicina_milli = 7_001
            }),
            ("bliži opis", |linija| {
                linija.blizi_opis = Some("Drugo mesto".into())
            }),
        ];

        for (polje, izmeni) in izmene {
            let mut linija = potpisana_linija();
            izmeni(&mut linija);
            assert_ne!(
                snapshot_hash("a", &view_of(linija)),
                osnovni,
                "„{polje}“ is frozen by the potpis but the snapshot does not cover it, so the \
                 stored hash would describe a document that changed"
            );
        }
    }

    /// Čl. 8 st. 5 releases the book data when „чланови комисије за попис потпишу
    /// те листе“. Nobody signing is not a signature.
    #[test]
    fn a_signature_with_no_signatory_is_refused() {
        with_app("popis_sign_without_signatory", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let mut connection = state.db().open().expect("database should open");

            let error = sign_phase_a(&mut connection, id, &[], "2026-12-31T17:00:00Z")
                .expect_err("a potpis needs a potpisnik");

            assert_eq!(error.code(), "popis_bez_potpisnika");
            assert_eq!(signature_count(state.inner(), id), 0);
        });
    }

    /// The state machine is consulted, not re-implemented: a draft popis has not
    /// counted anything, so there is no stvarno stanje to sign.
    #[test]
    fn signing_before_the_count_is_refused_by_the_state_machine() {
        with_app("popis_sign_before_counting", |app| {
            let state = app.state::<AppState>();
            let mut connection = state.db().open().expect("database should open");
            let session = open_popis(&mut connection, &open_request(), "2026-12-31T08:00:00Z")
                .expect("the popis should open");

            let error = sign_phase_a(
                &mut connection,
                session.id,
                &["Miloš Đurđević".to_string()],
                "2026-12-31T17:00:00Z",
            )
            .expect_err("a draft popis has nothing to sign");

            assert_eq!(error.code(), "popis_nedozvoljen_prelaz");
        });
    }

    /// Req. 30 — the čl. 8 st. 5 potpis freezes a snapshot. A counted quantity
    /// that could still be edited afterwards would make that signature attest to a
    /// state that no longer exists.
    #[test]
    fn the_counted_state_is_frozen_by_the_phase_a_potpis() {
        with_app("popis_counted_state_frozen", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let mut connection = state.db().open().expect("database should open");
            sign_phase_a(
                &mut connection,
                id,
                &["Miloš Đurđević".to_string()],
                "2026-12-31T17:00:00Z",
            )
            .expect("the potpis should record");

            let error = save_line(
                &connection,
                id,
                None,
                &line_input("KOS-2", 1_000),
                "2026-12-31T18:00:00Z",
            )
            .expect_err("the signed count must not take another line");

            assert_eq!(error.code(), "popis_stvarno_stanje_potpisano");
        });
    }

    /// Req. 30, the other half of the freeze. The čl. 8 st. 5 potpis fixes a
    /// **document**, not a number: čl. 8 st. 4 hands the commission the
    /// nomenklaturni broj, the naziv, the vrsta and the jedinica mere, and čl. 9
    /// st. 1 t. 1 has it sign the count together with its bliži opis. An obračun
    /// that could rewrite any of those while keeping the količina would leave the
    /// potpis attesting to a stavka nobody counted — the same breach as moving the
    /// količina, wearing a different hat. So every field of the signed identity is
    /// pinned, one case per field, and the obračun itself still has to work or the
    /// pin would just be a wall.
    #[test]
    fn a_phase_b_edit_that_moves_the_signed_line_identity_is_refused() {
        with_app("popis_line_identity_frozen", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let mut connection = state.db().open().expect("database should open");
            sign_phase_a(
                &mut connection,
                id,
                &["Miloš Đurđević".to_string()],
                "2026-12-31T17:00:00Z",
            )
            .expect("the čl. 8 st. 5 potpis should record");
            let computed = compute_differences(&connection, id, "2027-01-02T09:00:00Z")
                .expect("the obračun should open");
            let line_id = computed.linije[0].id;

            let podmetanja: [IzmenaUnosa; 6] = [
                ("lista_vrsta", |input| input.lista_vrsta = "gotovina".into()),
                ("šifra", |input| input.sifra = Some("PODMETNUTO-9".into())),
                ("naziv", |input| input.naziv = "Nešto sasvim drugo".into()),
                ("vrsta", |input| {
                    input.vrsta = Some("Muška konfekcija".into())
                }),
                ("jedinica mere", |input| {
                    input.jedinica_mere = Some("kg".into())
                }),
                ("bliži opis", |input| {
                    input.blizi_opis = Some("Drugo mesto".into())
                }),
            ];

            for (polje, podmetni) in podmetanja {
                let mut input = line_input("KOS-1", 7_000);
                podmetni(&mut input);
                let error = save_line(
                    &connection,
                    id,
                    Some(line_id),
                    &input,
                    "2027-01-02T10:00:00Z",
                )
                .err()
                .unwrap_or_else(|| {
                    panic!("moving „{polje}“ after the čl. 8 st. 5 potpis must be refused")
                });

                assert_eq!(
                    error.code(),
                    "popis_stvarno_stanje_potpisano",
                    "„{polje}“ said: {error}"
                );
                assert!(
                    error.to_string().contains("čl. 8 st. 5"),
                    "the refusal must name the article, „{polje}“ said: {error}"
                );
            }

            // Nothing the commission signed moved, and no new stavka appeared.
            let after = load_session(&connection, id).expect("the session should load");
            assert_eq!(after.linije.len(), 1);
            assert_eq!(after.linije[0].lista_vrsta, "roba");
            assert_eq!(after.linije[0].sifra.as_deref(), Some("KOS-1"));
            assert_eq!(after.linije[0].naziv, "Košulja");
            assert_eq!(after.linije[0].vrsta.as_deref(), Some("Ženska konfekcija"));
            assert_eq!(after.linije[0].jedinica_mere.as_deref(), Some("kom"));
            assert_eq!(after.linije[0].blizi_opis.as_deref(), Some("Polica A2"));
            assert_eq!(after.linije[0].stvarna_kolicina_milli, 7_000);

            // And the obračun the freeze exists to allow still goes through: čl. 9
            // st. 1 t. 5 pricing over the line the commission counted.
            let mut obracun = line_input("KOS-1", 7_000);
            obracun.cena_minor = Some(249_900);
            let priced = save_line(
                &connection,
                id,
                Some(line_id),
                &obracun,
                "2027-01-02T10:05:00Z",
            )
            .expect("the obračun must still be able to price the signed stavka");
            assert_eq!(priced.linije[0].cena_minor, Some(249_900));
            assert_eq!(
                priced.linije[0].knjigovodstvena_kolicina_milli,
                Some(KNJIGOVODSTVENO_STANJE_MILLI),
                "the pricing edit must not blank what the potpis released"
            );
        });
    }

    // -----------------------------------------------------------------
    // Req. 36 — the posebne popisne liste
    // -----------------------------------------------------------------

    /// Req. 36 — the five posebne liste are required wherever their category is
    /// present, so each of them has to be a list this module can actually take
    /// lines onto, not a value the CHECK constraint tolerates. The count per lista
    /// is asserted through the session view because that is what a count sheet
    /// renders: six liste, each with what is on it.
    #[test]
    fn each_popisna_lista_takes_its_own_lines() {
        with_app("popis_six_liste", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let connection = state.db().open().expect("database should open");

            // Before anything is written onto them, all five posebne liste are
            // already reported as the empty liste they are: a lista the count sheet
            // never shows is a lista the shop never fills, and req. 36 is about the
            // ones that are missing.
            let prazne = load_session(&connection, id).expect("the session should load");
            assert_eq!(prazne.liste.len(), 6);
            for pregled in &prazne.liste {
                assert_eq!(
                    pregled.broj_stavki,
                    i64::from(pregled.vrsta == PopisLista::Roba),
                    "„{}“ before any posebna lista is written",
                    pregled.naziv
                );
            }

            for lista in PopisLista::ALL {
                if lista == PopisLista::Roba {
                    continue; // already counted by `seeded_count`
                }
                save_line(
                    &connection,
                    id,
                    None,
                    &lista_line(lista),
                    "2026-12-31T09:20:00Z",
                )
                .unwrap_or_else(|error| {
                    panic!(
                        "the {} lista must take its own lines: {error}",
                        lista.naziv()
                    )
                });
            }

            let session = load_session(&connection, id).expect("the session should load");
            assert_eq!(session.linije.len(), 6);
            assert_eq!(
                session.liste.len(),
                6,
                "all six liste are reported, including the empty ones — a lista nobody can see \
                 is a lista nobody fills"
            );

            for pregled in &session.liste {
                assert_eq!(
                    pregled.broj_stavki, 1,
                    "„{}“ should carry exactly the one line saved onto it",
                    pregled.naziv
                );
                assert_eq!(pregled.naziv, pregled.vrsta.naziv());
                assert!(
                    pregled.pravni_osnov.contains("čl."),
                    "„{}“ must carry the article that requires it, carries „{}“",
                    pregled.naziv,
                    pregled.pravni_osnov
                );
            }
        });
    }

    /// PoP čl. 11 st. 1 — gotovina is counted **po apoenima**. A single „u kasi
    /// ima 45.000 dinara“ line is exactly what that provision refuses: it is a
    /// figure, not a count. So a cash line carries the apoen it counted, and it
    /// counts whole notes and coins — half a novčanica is a typo, and a popis is
    /// evidence.
    #[test]
    fn a_cash_line_that_is_not_denominated_by_apoen_is_refused() {
        with_app("popis_gotovina_apoen", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let connection = state.db().open().expect("database should open");

            let mut bez_apoena = lista_line(PopisLista::Gotovina);
            bez_apoena.cena_minor = None;
            let error = save_line(&connection, id, None, &bez_apoena, "2026-12-31T09:20:00Z")
                .expect_err("a lump sum is not a count po apoenima");
            assert_eq!(error.code(), "popis_gotovina_bez_apoena");
            assert!(
                error.to_string().contains("čl. 11 st. 1"),
                "the refusal must name the article, said: {error}"
            );

            let mut nula = lista_line(PopisLista::Gotovina);
            nula.cena_minor = Some(0);
            assert_eq!(
                save_line(&connection, id, None, &nula, "2026-12-31T09:21:00Z")
                    .expect_err("an apoen of zero is no apoen")
                    .code(),
                "popis_gotovina_bez_apoena"
            );

            let mut pola = lista_line(PopisLista::Gotovina);
            pola.stvarna_kolicina_milli = 5_500;
            let error = save_line(&connection, id, None, &pola, "2026-12-31T09:22:00Z")
                .expect_err("half a banknote was never counted");
            assert_eq!(error.code(), "popis_gotovina_deo_apoena");

            // The positive control: a properly denominated line saves, or the three
            // refusals above would be a wall rather than a rule.
            let session = save_line(
                &connection,
                id,
                None,
                &lista_line(PopisLista::Gotovina),
                "2026-12-31T09:23:00Z",
            )
            .expect("5 × 1.000 RSD is a count po apoenima");
            let gotovina = session
                .linije
                .iter()
                .find(|linija| linija.lista_vrsta == "gotovina")
                .expect("the cash line should be there");
            assert_eq!(gotovina.cena_minor, Some(100_000));
            assert_eq!(gotovina.stvarna_kolicina_milli, 5_000);
        });
    }

    /// Čl. 11 st. 1 asks that a cash line NAME its apoen — it does not ask the
    /// commission to retype it every time the bliži opis changes. An UPDATE that
    /// omits „cena“ coalesces, so the apoen already on the lista is what survives
    /// the write and is what the rule must be judged against. Judged against the
    /// absent field instead, the very same payload would mean „you forgot the
    /// apoen“ before the potpis and „leave the apoen alone“ after it, and a
    /// lawful correction of where the money was found would be refused on a rule
    /// that is already satisfied.
    #[test]
    fn an_apoen_already_on_the_lista_is_not_demanded_again_by_an_edit() {
        with_app("popis_gotovina_apoen_edit", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let connection = state.db().open().expect("database should open");

            let session = save_line(
                &connection,
                id,
                None,
                &lista_line(PopisLista::Gotovina),
                "2026-12-31T09:20:00Z",
            )
            .expect("the cash line should save");
            let gotovina_id = session
                .linije
                .iter()
                .find(|linija| linija.lista_vrsta == "gotovina")
                .expect("the cash line should be there")
                .id;

            let mut opis = lista_line(PopisLista::Gotovina);
            opis.cena_minor = None;
            opis.blizi_opis = Some("Fioka kase — druga smena".into());
            save_line(
                &connection,
                id,
                Some(gotovina_id),
                &opis,
                "2026-12-31T09:25:00Z",
            )
            .expect("an edit that leaves the apoen alone must go through in counting too");

            let after = load_session(&connection, id).expect("the session should load");
            let gotovina = after
                .linije
                .iter()
                .find(|linija| linija.lista_vrsta == "gotovina")
                .expect("the cash line should be there");
            assert_eq!(
                gotovina.cena_minor,
                Some(100_000),
                "the apoen on the lista is what the COALESCE keeps"
            );
            assert_eq!(
                gotovina.blizi_opis.as_deref(),
                Some("Fioka kase — druga smena"),
                "the edit the commission actually made must land"
            );

            // The rule is judged against what SURVIVES the write, not against the
            // line's history: moving a line onto the gotovina lista where no apoen
            // was ever recorded still leaves a lump sum, and is still refused.
            let roba_id = after
                .linije
                .iter()
                .find(|linija| linija.lista_vrsta == "roba")
                .expect("the seeded roba line should be there")
                .id;
            let mut preseljeno = lista_line(PopisLista::Gotovina);
            preseljeno.cena_minor = None;
            assert_eq!(
                save_line(
                    &connection,
                    id,
                    Some(roba_id),
                    &preseljeno,
                    "2026-12-31T09:30:00Z"
                )
                .expect_err("a line that never named an apoen is not denominated by one")
                .code(),
                "popis_gotovina_bez_apoena"
            );
        });
    }

    /// The blind read withholds the whole Phase B block, and the čl. 9 st. 1 t. 5
    /// cena with it — but on the gotovina lista that column is the **apoen**, part
    /// of what the commission itself counted (čl. 11 st. 1). Čl. 8 st. 5 keeps
    /// podatke iz knjigovodstva o količinama away from the commission; it does not
    /// keep the commission from its own count, and a cash lista that cannot show
    /// „5 × 1.000“ while it is being written is unreadable exactly when it matters.
    #[test]
    fn the_blind_read_shows_the_apoen_and_still_withholds_the_obracun_cena() {
        with_app("popis_blind_apoen", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let connection = state.db().open().expect("database should open");

            let session = load_session(&connection, id).expect("the session should load");
            let roba_id = session.linije[0].id;
            let mut sa_cenom = lista_line(PopisLista::Roba);
            sa_cenom.cena_minor = Some(249_900);
            save_line(
                &connection,
                id,
                Some(roba_id),
                &sa_cenom,
                "2026-12-31T09:15:00Z",
            )
            .expect("a cena may be recorded on a roba line");
            save_line(
                &connection,
                id,
                None,
                &lista_line(PopisLista::Gotovina),
                "2026-12-31T09:20:00Z",
            )
            .expect("the cash line should save");

            let counting = load_session(&connection, id).expect("the session should load");
            assert!(!counting.knjigovodstvo_dostupno, "still Phase A");
            let roba = &counting.linije[0];
            let gotovina = &counting.linije[1];
            assert_eq!(roba.lista_vrsta, "roba");
            assert_eq!(
                roba.cena_minor, None,
                "the obračun cena stays withheld until the čl. 8 st. 5 potpis"
            );
            assert_eq!(
                gotovina.cena_minor,
                Some(100_000),
                "the apoen is part of the count and must be readable during it"
            );
        });
    }

    /// On the gotovina lista `cena_minor` is the **apoen** — part of what the
    /// commission counted and signed under čl. 8 st. 5, not a price the obračun
    /// fills in under čl. 9 st. 1 t. 5. Left unpinned, a signed „5 × 1.000“ could
    /// become „5 × 5.000“ in Phase B and the potpis would attest to a cash count
    /// nobody took.
    #[test]
    fn the_signed_apoen_does_not_move_in_the_obracun() {
        with_app("popis_gotovina_apoen_frozen", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let mut connection = state.db().open().expect("database should open");
            save_line(
                &connection,
                id,
                None,
                &lista_line(PopisLista::Gotovina),
                "2026-12-31T09:20:00Z",
            )
            .expect("the cash line should save");
            sign_phase_a(
                &mut connection,
                id,
                &["Miloš Đurđević".to_string()],
                "2026-12-31T17:00:00Z",
            )
            .expect("the čl. 8 st. 5 potpis should record");
            let computed = compute_differences(&connection, id, "2027-01-02T09:00:00Z")
                .expect("the obračun should open");
            let line_id = computed
                .linije
                .iter()
                .find(|linija| linija.lista_vrsta == "gotovina")
                .expect("the cash line should be there")
                .id;

            let mut podmetnut = lista_line(PopisLista::Gotovina);
            podmetnut.cena_minor = Some(500_000);
            let error = save_line(
                &connection,
                id,
                Some(line_id),
                &podmetnut,
                "2027-01-02T10:00:00Z",
            )
            .expect_err("the signed apoen must not move");

            assert_eq!(error.code(), "popis_apoen_potpisan");
            assert!(
                error.to_string().contains("čl. 11 st. 1"),
                "the refusal must say why „cena“ is not editable here, said: {error}"
            );

            // Leaving the apoen alone is still a legal obračun edit.
            let mut netaknuto = lista_line(PopisLista::Gotovina);
            netaknuto.cena_minor = None;
            save_line(
                &connection,
                id,
                Some(line_id),
                &netaknuto,
                "2027-01-02T10:05:00Z",
            )
            .expect("an obračun edit that does not touch the apoen must go through");
            let after = load_session(&connection, id).expect("the session should load");
            assert_eq!(
                after
                    .linije
                    .iter()
                    .find(|linija| linija.lista_vrsta == "gotovina")
                    .expect("the cash line should be there")
                    .cena_minor,
                Some(100_000),
                "the apoen the commission signed is what stays on the lista"
            );
        });
    }

    /// The freeze refusal tells the shop what the obračun may still fill in, and
    /// on the gotovina lista that is a different answer: „cena“ there is the apoen
    /// and is frozen with the count. One wording for both would tell a shop it may
    /// still edit a cena that the very next call refuses — an operator string
    /// promising behaviour this module does not implement.
    #[test]
    fn the_freeze_refusal_says_what_is_still_editable_on_the_lista_it_is_about() {
        with_app("popis_freeze_refusal_wording", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let mut connection = state.db().open().expect("database should open");
            save_line(
                &connection,
                id,
                None,
                &lista_line(PopisLista::Gotovina),
                "2026-12-31T09:20:00Z",
            )
            .expect("the cash line should save");
            sign_phase_a(
                &mut connection,
                id,
                &["Miloš Đurđević".to_string()],
                "2026-12-31T17:00:00Z",
            )
            .expect("the čl. 8 st. 5 potpis should record");
            let computed = compute_differences(&connection, id, "2027-01-02T09:00:00Z")
                .expect("the obračun should open");

            for (lista, mora, ne_sme) in [
                (
                    PopisLista::Roba,
                    "popunjavaju samo cena i knjigovodstvena količina",
                    "apoen",
                ),
                (
                    PopisLista::Gotovina,
                    "apoen i deo je potpisanog stvarnog stanja",
                    "popunjavaju samo cena",
                ),
            ] {
                let line_id = computed
                    .linije
                    .iter()
                    .find(|linija| linija.lista_vrsta == lista.as_db_str())
                    .expect("the line should be there")
                    .id;
                let mut podmetnut = lista_line(lista);
                podmetnut.naziv = "Nešto sasvim drugo".into();

                let error = save_line(
                    &connection,
                    id,
                    Some(line_id),
                    &podmetnut,
                    "2027-01-02T10:00:00Z",
                )
                .expect_err("a signed naziv must not move");
                let poruka = error.to_string();

                assert!(
                    poruka.contains(mora),
                    "the {} refusal must say „{mora}“, said: {poruka}",
                    lista.naziv()
                );
                assert!(
                    !poruka.contains(ne_sme),
                    "the {} refusal must not say „{ne_sme}“, said: {poruka}",
                    lista.naziv()
                );
            }
        });
    }

    /// PoP čl. 2 st. 6 — a signed copy of the posebna lista for tuđa roba reaches
    /// its owner within ten days of the count. The rok is computed from the count
    /// date and surfaced with the popis, because a rok nobody is shown is not a
    /// reminder; and the copy says plainly that the app does not do the delivering,
    /// because a reminder that implied otherwise would promise behaviour this
    /// module does not implement.
    #[test]
    fn a_konsignacija_lista_carries_the_ten_day_reminder() {
        with_app("popis_konsignacija_reminder", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let connection = state.db().open().expect("database should open");

            let session = save_line(
                &connection,
                id,
                None,
                &lista_line(PopisLista::Konsignacija),
                "2026-12-31T09:20:00Z",
            )
            .expect("the consignment line should save");

            assert_eq!(
                session.konsignacija_rok.as_deref(),
                Some("2027-01-10"),
                "ten days from the 31 December count"
            );
            let podsetnik = session
                .upozorenja
                .iter()
                .find(|poruka| poruka.contains("čl. 2 st. 6"))
                .unwrap_or_else(|| {
                    panic!(
                        "the čl. 2 st. 6 duty must be surfaced, warnings were: {:?}",
                        session.upozorenja
                    )
                });
            assert!(
                podsetnik.contains("2027-01-10"),
                "the reminder must carry the rok itself, said: {podsetnik}"
            );
            assert!(
                podsetnik.contains("ne dostavlja"),
                "the reminder must say the app does not deliver the lista, said: {podsetnik}"
            );
        });
    }

    /// The other side of the same rule: a popis with no tuđa roba on it owes
    /// nobody a copy, and a reminder that fired anyway would be the noise that
    /// teaches a shop to click past the ones that matter.
    #[test]
    fn a_popis_without_tudja_roba_carries_no_konsignacija_reminder() {
        with_app("popis_no_konsignacija_reminder", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let connection = state.db().open().expect("database should open");

            let session = load_session(&connection, id).expect("the session should load");

            assert_eq!(session.konsignacija_rok, None);
            assert!(
                session
                    .upozorenja
                    .iter()
                    .all(|poruka| !poruka.contains("čl. 2 st. 6")),
                "warnings were: {:?}",
                session.upozorenja
            );
        });
    }

    /// The degrade path, and it degrades in the safe direction. A count date whose
    /// ten-day rok leaves the calendar has no rok this module may invent — but the
    /// čl. 2 st. 6 duty does not go away because arithmetic did, so the session
    /// still loads and the shop is told the duty stands and the date is theirs to
    /// work out. Silently dropping the reminder would be the app deciding an
    /// obligation away.
    #[test]
    fn a_count_date_whose_rok_leaves_the_calendar_still_reports_the_duty() {
        with_app("popis_konsignacija_rok_overflow", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count_on(state.inner(), "KOS-1", "9999-12-31");
            let connection = state.db().open().expect("database should open");

            let session = save_line(
                &connection,
                id,
                None,
                &lista_line(PopisLista::Konsignacija),
                "9999-12-31T09:20:00Z",
            )
            .expect("the consignment line should save");

            assert_eq!(session.konsignacija_rok, None, "no rok may be invented");
            let podsetnik = session
                .upozorenja
                .iter()
                .find(|poruka| poruka.contains("čl. 2 st. 6"))
                .unwrap_or_else(|| {
                    panic!(
                        "the duty must be reported even when its rok cannot be computed, \
                         warnings were: {:?}",
                        session.upozorenja
                    )
                });
            assert!(
                podsetnik.contains("9999-12-31"),
                "the reminder must name the count date it could not compute from, said: \
                 {podsetnik}"
            );
        });
    }

    /// Req. 36 through the command layer: the declaration the shop makes about
    /// which categories exist is checked against the liste, and the report names
    /// what is missing and the article behind it. `spremno` is the flag Task 6's
    /// izveštaj generation rests on.
    #[test]
    fn the_lista_report_names_the_declared_liste_that_are_empty() {
        with_app("popis_provera_listi", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let prijavljene = vec![
                PopisLista::Roba,
                PopisLista::Gotovina,
                PopisLista::Konsignacija,
            ];

            sign_in_cashier(state.inner());
            assert_eq!(
                popis_provera_listi(app.state::<AppState>(), id, prijavljene.clone())
                    .expect_err("a cashier must not read the popis")
                    .code,
                "forbidden"
            );

            sign_in_admin(state.inner());
            let provera = popis_provera_listi(app.state::<AppState>(), id, prijavljene.clone())
                .expect("an admin should read the report");

            assert!(!provera.spremno);
            assert_eq!(
                provera
                    .nedostaju
                    .iter()
                    .map(|pregled| pregled.vrsta)
                    .collect::<Vec<_>>(),
                vec![PopisLista::Gotovina, PopisLista::Konsignacija],
                "only the declared categories with an empty lista"
            );
            let poruka = provera
                .poruka
                .clone()
                .expect("an unready popis needs a reason");
            assert!(
                poruka.contains("gotovina po apoenima") && poruka.contains("čl. 11 st. 1"),
                "the message must name the lista and its article, said: {poruka}"
            );

            let connection = state.db().open().expect("database should open");
            for lista in [PopisLista::Gotovina, PopisLista::Konsignacija] {
                save_line(
                    &connection,
                    id,
                    None,
                    &lista_line(lista),
                    "2026-12-31T09:25:00Z",
                )
                .expect("the declared lista should take its line");
            }

            let provera = popis_provera_listi(app.state::<AppState>(), id, prijavljene)
                .expect("an admin should read the report");
            assert!(provera.spremno, "the declared liste are no longer empty");
            assert!(provera.nedostaju.is_empty());
            assert_eq!(provera.poruka, None);
        });
    }

    // -----------------------------------------------------------------
    // Req. 34 — the čl. 9 st. 2 perpetual-inventory shortcut
    // -----------------------------------------------------------------

    /// Req. 34 / design §4 — the shortcut is a conditional gate, not a default.
    /// Čl. 9 st. 2 lets a shop that keeps a continuous quantity-and-value record
    /// enter its book state as the state po popisu **only** „под условом да је у
    /// току године извршен попис имовине и да су вишкови и мањкови утврђени тим
    /// пописом прокњижени“. With no such popis behind it the reference is a claim
    /// about a document that does not exist, and storing it unchecked would let
    /// the shop skip a count it owes.
    #[test]
    fn the_perpetual_shortcut_is_refused_without_a_posted_in_year_popis() {
        with_app("popis_perpetual_gate", |app| {
            let state = app.state::<AppState>();
            let mut connection = state.db().open().expect("database should open");

            let mut request = open_request();
            request.perpetual_odluka_ref = Some("Odluka 7/2026".into());
            let error = open_popis(&mut connection, &request, "2026-12-31T08:00:00Z")
                .expect_err("the shortcut must be refused with nothing behind it");

            assert_eq!(error.code(), "popis_nema_popisa_u_toku_godine");
            assert!(
                error.to_string().contains("čl. 9 st. 2"),
                "the refusal must name the article it enforces, said: {error}"
            );
            assert_eq!(
                session_count(state.inner()),
                0,
                "a refused open must persist no session"
            );

            // An in-year popis that only got as far as counting is not „извршен и
            // прокњижен“ either — the gate is the knjiženje, not the attempt.
            let nezavrsen = seeded_count_on(state.inner(), "KOS-2", "2026-06-30");
            assert_eq!(session_status(state.inner(), nezavrsen), "counting");
            let error = open_popis(&mut connection, &request, "2026-12-31T08:00:00Z")
                .expect_err("an unposted in-year popis does not open the shortcut");
            assert_eq!(error.code(), "popis_nema_popisa_u_toku_godine");
        });
    }

    /// The other side of the gate, and what keeps the refusal above from being a
    /// blanket „no“: once an in-year popis is posted the reference is accepted and
    /// stored, because that is the čl. 9 st. 2 condition met.
    #[test]
    fn the_perpetual_shortcut_is_accepted_once_an_in_year_popis_is_posted() {
        with_app("popis_perpetual_gate_open", |app| {
            let state = app.state::<AppState>();
            walk_to_posted_on(state.inner(), "KOS-7", "2026-06-30");
            let mut connection = state.db().open().expect("database should open");

            let mut request = open_request();
            request.perpetual_odluka_ref = Some("Odluka 7/2026".into());
            let session = open_popis(&mut connection, &request, "2026-12-31T08:00:00Z")
                .expect("a posted in-year popis opens the čl. 9 st. 2 shortcut");

            assert_eq!(
                session.perpetual_odluka_ref.as_deref(),
                Some("Odluka 7/2026")
            );
        });
    }

    /// The year is part of the condition: čl. 9 st. 2 says „у току године“, and a
    /// popis posted in a different business year says nothing about this one.
    #[test]
    fn a_posted_popis_from_another_year_does_not_open_the_shortcut() {
        with_app("popis_perpetual_gate_other_year", |app| {
            let state = app.state::<AppState>();
            walk_to_posted_on(state.inner(), "KOS-8", "2025-06-30");
            let mut connection = state.db().open().expect("database should open");

            let mut request = open_request();
            request.perpetual_odluka_ref = Some("Odluka 7/2026".into());
            let error = open_popis(&mut connection, &request, "2026-12-31T08:00:00Z")
                .expect_err("last year's popis does not excuse this year's count");

            assert_eq!(error.code(), "popis_nema_popisa_u_toku_godine");
        });
    }

    // -----------------------------------------------------------------
    // Req. 41 — the posting lock
    // -----------------------------------------------------------------

    #[test]
    fn posting_write_locks_the_popis() {
        with_app("popis_posting_write_locks", |app| {
            let state = app.state::<AppState>();
            let id = walk_to_posted(state.inner(), "KOS-1");
            let mut connection = state.db().open().expect("database should open");

            let posted = load_session(&connection, id).expect("the session should load");
            assert_eq!(posted.status, PopisStatus::Posted);
            assert_eq!(posted.posted_at.as_deref(), Some("2027-01-05T11:00:00Z"));

            for error in [
                save_line(
                    &connection,
                    id,
                    None,
                    &line_input("KOS-9", 1_000),
                    "2027-01-06T09:00:00Z",
                )
                .expect_err("a posted popis takes no new line"),
                save_line(
                    &connection,
                    id,
                    Some(posted.linije[0].id),
                    &line_input("KOS-1", 9_000),
                    "2027-01-06T09:00:00Z",
                )
                .expect_err("a posted popis takes no line edit"),
                sign_phase_b(
                    &mut connection,
                    id,
                    &["Miloš Đurđević".to_string()],
                    "2027-01-06T09:00:00Z",
                )
                .expect_err("a posted popis takes no further potpis"),
                post_popis(&connection, id, "2027-01-06T09:00:00Z")
                    .expect_err("a posted popis is not posted twice"),
            ] {
                assert_eq!(error.code(), "popis_proknjizen", "said: {error}");
            }

            // The engine is the backstop, not the command layer's politeness.
            let raw = connection.execute(
                "UPDATE popis_sessions SET prodajno_mesto = 'Drugi objekat' WHERE id = ?1",
                params![id],
            );
            assert!(
                raw.is_err(),
                "the v20 lock must refuse a write that goes around the commands"
            );
        });
    }

    /// Req. 41 with ZoRač čl. 8 st. 4 — the correction path is a NEW document. The
    /// assertion is both halves: the edit is refused, and the new session opens and
    /// leaves the posted one exactly as it was.
    #[test]
    fn a_correction_after_posting_takes_a_new_session_rather_than_an_edit() {
        with_app("popis_correction_is_a_new_session", |app| {
            let state = app.state::<AppState>();
            let posted_id = walk_to_posted(state.inner(), "KOS-1");
            let mut connection = state.db().open().expect("database should open");
            let posted = load_session(&connection, posted_id).expect("the session should load");

            let mut corrected = line_input("KOS-1", 6_000);
            corrected.knjigovodstvena_kolicina_milli = Some(6_000);
            let error = save_line(
                &connection,
                posted_id,
                Some(posted.linije[0].id),
                &corrected,
                "2027-01-06T09:00:00Z",
            )
            .expect_err("a posted popis is corrected by a new popis, never by an edit");
            assert_eq!(error.code(), "popis_proknjizen");
            assert!(
                error.to_string().contains("novim popisom"),
                "the refusal must say what the correction path IS, said: {error}"
            );

            let mut request = open_request();
            request.datum_popisa = "2027-01-06".into();
            request.odluka_ref = Some("Odluka 1/2027 — ispravka".into());
            let ispravka = open_popis(&mut connection, &request, "2027-01-06T09:05:00Z")
                .expect("the corrective popis should open");

            assert_ne!(ispravka.id, posted_id);
            assert_eq!(ispravka.status, PopisStatus::Draft);

            let unchanged = load_session(&connection, posted_id).expect("the session should load");
            assert_eq!(unchanged.linije[0].stvarna_kolicina_milli, 7_000);
            assert_eq!(
                unchanged.linije[0].knjigovodstvena_kolicina_milli,
                Some(KNJIGOVODSTVENO_STANJE_MILLI)
            );
        });
    }

    // -----------------------------------------------------------------
    // Req. 40 — the komisija warning
    // -----------------------------------------------------------------

    /// Req. 40 — warn, never block. The warning has to carry three things and the
    /// assertions are one per thing: **who** it is about, **which** provision, and
    /// that nothing was stopped. The last is not politeness: a warning that read
    /// like a refusal would be an operator string promising behaviour this module
    /// does not implement. The čl. 6 st. 1–2 shodna primena stays labelled
    /// unresolved (§6 R-5) rather than asserted either way.
    #[test]
    fn a_goods_handling_member_warns_by_name_and_does_not_block() {
        with_app("popis_commission_warning", |app| {
            let state = app.state::<AppState>();
            let mut connection = state.db().open().expect("database should open");

            let mut request = open_request();
            request.komisija = vec![
                KomisijaClanInput {
                    ime: "Ana Jovanović".into(),
                    uloga: "predsednik".into(),
                    rukuje_imovinom: true,
                },
                KomisijaClanInput {
                    ime: "Miloš Đurđević".into(),
                    uloga: "clan".into(),
                    rukuje_imovinom: false,
                },
            ];

            let session = open_popis(&mut connection, &request, "2026-12-31T08:00:00Z")
                .expect("a flagged member must not block the popis");

            assert_eq!(session.komisija.len(), 2);
            assert_eq!(session.upozorenja.len(), 1);
            let upozorenje = &session.upozorenja[0];
            assert!(
                upozorenje.contains("Ana Jovanović") && upozorenje.contains("čl. 5 st. 1"),
                "the warning must name who it is about and why, said: {upozorenje}"
            );
            assert!(
                upozorenje.contains("nije zaustavljen"),
                "the copy must say the popis was NOT stopped, said: {upozorenje}"
            );
            assert!(
                upozorenje.contains("nije razjašnjeno"),
                "the čl. 6 st. 1–2 shodna primena is unresolved (R-5) and the copy must say so, \
                 said: {upozorenje}"
            );

            // Nothing was blocked: the popis walks on with the flagged member.
            start_count(&connection, session.id, "2026-12-31T09:00:00Z")
                .expect("a flagged member must not stop the count");
        });
    }

    // -----------------------------------------------------------------
    // Validation and the admin gate
    // -----------------------------------------------------------------

    #[test]
    fn an_unknown_lista_vrsta_is_refused_before_the_check_constraint() {
        with_app("popis_unknown_lista_vrsta", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            let connection = state.db().open().expect("database should open");

            let mut input = line_input("KOS-2", 1_000);
            input.lista_vrsta = "ostalo".into();
            let error = save_line(&connection, id, None, &input, "2026-12-31T09:30:00Z")
                .expect_err("an unknown lista must be refused");

            assert_eq!(error.code(), "popis_nepoznata_lista");
        });
    }

    #[test]
    fn an_unreadable_count_date_is_refused_rather_than_stored() {
        with_app("popis_unreadable_datum", |app| {
            let state = app.state::<AppState>();
            let mut connection = state.db().open().expect("database should open");

            let mut request = open_request();
            request.datum_popisa = "31.12.2026.".into();
            let error = open_popis(&mut connection, &request, "2026-12-31T08:00:00Z")
                .expect_err("an unreadable count date must not open a popis");

            assert_eq!(error.code(), "popis_neispravan_datum");
            assert_eq!(session_count(state.inner()), 0);
        });
    }

    #[test]
    fn popis_open_is_rejected_for_a_cashier() {
        with_app("popis_open_rejected_for_cashier", |app| {
            sign_in_cashier(app.state::<AppState>().inner());

            let error = popis_open(app.state::<AppState>(), open_request())
                .expect_err("a cashier must not open a popis");

            assert_eq!(error.code, "forbidden");
            assert_eq!(session_count(app.state::<AppState>().inner()), 0);
        });
    }

    #[test]
    fn popis_list_is_rejected_for_a_cashier_and_served_to_an_admin() {
        with_app("popis_list_admin_gate", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");

            sign_in_cashier(state.inner());
            let error =
                popis_list(app.state::<AppState>()).expect_err("a cashier must not read the popis");
            assert_eq!(error.code, "forbidden");

            sign_in_admin(state.inner());
            let sessions =
                popis_list(app.state::<AppState>()).expect("an admin should read the popis");
            assert_eq!(sessions.len(), 1);
            assert_eq!(sessions[0].id, id);
            assert_eq!(sessions[0].status, PopisStatus::Counting);
            assert_eq!(sessions[0].broj_linija, 1);
        });
    }

    #[test]
    fn a_missing_session_is_not_found() {
        with_app("popis_missing_session", |app| {
            let state = app.state::<AppState>();
            let connection = state.db().open().expect("database should open");

            let error = load_session(&connection, 404).expect_err("a missing popis must not load");

            assert_eq!(error.code(), "not_found");
        });
    }

    // -----------------------------------------------------------------
    // Req. 37 — the izveštaj o popisu (PoP čl. 13 st. 1), and req. 38's rok
    // -----------------------------------------------------------------

    /// ZoRač čl. 44 st. 1's own rok for FY2026. Configured rather than assumed,
    /// which is the whole point of req. 38: the popis module does not own this date.
    const ROK_PREDAJE_FI: &str = "2027-03-31";
    /// 60 days back from it — PoP čl. 13 st. 2, first limb.
    const ROK_IZVESTAJA: &str = "2027-01-30";

    /// The cena the obračun puts on the counted stavka (para). Distinct from the
    /// article's own sale price so a valuation that read the catalog instead of the
    /// popisna lista would show.
    const CENA_MINOR: i64 = 249_900;

    fn set_rok_predaje_fi(state: &AppState, rok: Option<&str>) {
        sign_in_admin(state);
        save_podesavanja(
            state,
            &PopisPodesavanja {
                rok_predaje_fi: rok.map(str::to_string),
            },
        )
        .expect("the filing deadline should save");
    }

    fn potpun_narativ() -> IzvestajNarativ {
        IzvestajNarativ {
            uzroci_neslaganja: "Manjak potiče od zamene koja nije evidentirana.".into(),
            predlozi_za_likvidaciju_razlika: "Prebijanje manjka i viška po osnovu zamene.".into(),
            nacin_knjizenja: "Manjak na teret troškova, višak u prihode.".into(),
            primedbe_lica_koja_rukuju_vrednostima: "Nema primedbi.".into(),
            ostale_primedbe_i_predlozi: "Predlaže se kontrolni popis u junu.".into(),
        }
    }

    fn izvestaj_request() -> IzvestajRequest {
        IzvestajRequest {
            prijavljene_liste: vec![PopisLista::Roba],
            narativ: potpun_narativ(),
        }
    }

    /// A popis walked to `computed` with the čl. 8 st. 5 potpis behind it and a cena
    /// on the counted stavka — the earliest state in which an izveštaj can carry the
    /// knjigovodstveno stanje and the razlike at all.
    fn seeded_izvestaj_popis(state: &AppState, sifra: &str) -> i64 {
        let id = seeded_count(state, sifra);
        let mut connection = state.db().open().expect("database should open");
        sign_phase_a(
            &mut connection,
            id,
            &["Miloš Đurđević".to_string()],
            "2026-12-31T17:00:00Z",
        )
        .expect("the čl. 8 st. 5 potpis should record");
        compute_differences(&connection, id, "2027-01-02T09:00:00Z")
            .expect("the obračun should open");

        let line_id: i64 = connection
            .query_row(
                "SELECT id FROM popis_lines WHERE session_id = ?1 ORDER BY id LIMIT 1",
                params![id],
                |row| row.get(0),
            )
            .expect("the counted line should exist");
        let mut input = line_input(sifra, 7_000);
        input.cena_minor = Some(CENA_MINOR);
        save_line(
            &connection,
            id,
            Some(line_id),
            &input,
            "2027-01-02T09:30:00Z",
        )
        .expect("the obračun should price the stavka");

        id
    }

    /// Everything a write to this popis would move: the session's own stamp, every
    /// line with its three figures and its stamp, and the number of potpisi. Read
    /// straight out of the tables rather than off `PopisSessionView`, which carries
    /// no `updated_at` and would let a stray write past unnoticed.
    fn popis_otisak(state: &AppState, id: i64) -> String {
        let connection = state.db().open().expect("database should open");
        connection
            .query_row(
                "SELECT s.status || '|' || s.updated_at || '|' || COALESCE(s.posted_at, '-')
                        || '|' || (SELECT COUNT(*) FROM popis_signatures WHERE session_id = s.id)
                        || '|' || COALESCE((SELECT GROUP_CONCAT(
                                 l.id || ':' || l.stvarna_kolicina_milli
                                      || ':' || COALESCE(l.knjigovodstvena_kolicina_milli, '-')
                                      || ':' || COALESCE(l.cena_minor, '-')
                                      || ':' || l.updated_at, ';')
                             FROM popis_lines l WHERE l.session_id = s.id), '-')
                 FROM popis_sessions s
                 WHERE s.id = ?1",
                params![id],
                |row| row.get(0),
            )
            .expect("the session should exist")
    }

    /// Req. 37 — the izveštaj carries **all eight** čl. 13 st. 1 elements, in the
    /// article's order, each named and attributed, and the five the commission wrote
    /// carry the text it wrote. The three computed ones carry no text: they are read
    /// out of the popis, and the figures below them are what answers them.
    #[test]
    fn the_izvestaj_carries_all_eight_prescribed_elements() {
        with_app("popis_izvestaj_elements", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));

            let pre = popis_otisak(state.inner(), id);
            let izvestaj = compose_izvestaj(state.inner(), id, &izvestaj_request())
                .expect("a complete izveštaj should compose");
            assert_eq!(
                pre,
                popis_otisak(state.inner(), id),
                "the izveštaj is composed from the popis and never writes to it"
            );

            assert_eq!(
                izvestaj
                    .elementi
                    .iter()
                    .map(|element| element.element)
                    .collect::<Vec<_>>(),
                IzvestajElement::ALL.to_vec(),
                "all eight prescribed elements, in the article's order"
            );
            for pregled in &izvestaj.elementi {
                assert_eq!(pregled.naziv, pregled.element.naziv());
                assert!(pregled.pravni_osnov.contains("čl. 13 st. 1"));
                assert!(!pregled.uputstvo.is_empty());
                assert_eq!(
                    pregled.tekst.is_some(),
                    pregled.element.narativni(),
                    "{:?} carries text exactly when the commission writes it",
                    pregled.element
                );
            }

            let narativ = potpun_narativ();
            for element in IzvestajElement::ALL {
                let pregled = izvestaj
                    .elementi
                    .iter()
                    .find(|pregled| pregled.element == element)
                    .expect("every element is present");
                assert_eq!(
                    pregled.tekst.as_deref(),
                    narativ.tekst(element),
                    "{element:?} must carry exactly what was written for it"
                );
            }
        });
    }

    /// The plan's second named behaviour: **an izveštaj with any prescribed element
    /// empty is refused.** The refusal is `crate::popis::ensure_izvestaj_kompletan`'s
    /// so a screen and the generator cannot word it two ways, and nothing about the
    /// popis moves — a refused izveštaj is not a half-composed one.
    #[test]
    fn an_izvestaj_missing_one_element_is_refused_and_writes_nothing() {
        with_app("popis_izvestaj_incomplete", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));

            let pre = popis_otisak(state.inner(), id);

            let mut request = izvestaj_request();
            request.narativ.nacin_knjizenja = "   ".into();
            let error = compose_izvestaj(state.inner(), id, &request)
                .expect_err("an unanswered element must stop the izveštaj");

            assert_eq!(error.code(), "popis_izvestaj_nepotpun");
            let poruka = error.to_string();
            assert!(
                poruka.contains("način knjiženja") && poruka.contains("čl. 13 st. 1"),
                "the refusal must name the element and its article, said: {poruka}"
            );

            assert_eq!(
                pre,
                popis_otisak(state.inner(), id),
                "a refused izveštaj must not write anything"
            );
        });
    }

    /// The plan's third named behaviour: **the rok shown is the computed one** —
    /// `crate::popis::izvestaj_due`, not a date typed into this module. Čl. 14 st. 2
    /// puts the odluka o usvajanju „u roku iz člana 13. stav 2“, so the milestone
    /// carries the same date rather than a second one.
    #[test]
    fn the_izvestaj_shows_the_computed_rok_and_the_odluka_rides_on_it() {
        with_app("popis_izvestaj_rok", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));

            let izvestaj = compose_izvestaj(state.inner(), id, &izvestaj_request())
                .expect("a complete izveštaj should compose");

            assert_eq!(izvestaj.rok, ROK_IZVESTAJA);
            assert!(izvestaj.rok_pravni_osnov.contains("čl. 13 st. 2"));
            assert_eq!(
                izvestaj.odluka_o_usvajanju.rok, ROK_IZVESTAJA,
                "čl. 14 st. 2 is the same rok, not a second one"
            );
            assert!(izvestaj
                .odluka_o_usvajanju
                .pravni_osnov
                .contains("čl. 14 st. 2"));
            assert!(
                izvestaj
                    .odluka_o_usvajanju
                    .napomena
                    .contains("ne evidentira"),
                "the milestone must say plainly that the app does not record the decision, \
                 said: {}",
                izvestaj.odluka_o_usvajanju.napomena
            );

            // The rok is read out of the configured filing deadline, not out of this
            // module: move the configuration and the rok moves with it.
            set_rok_predaje_fi(state.inner(), Some("2028-03-31"));
            let pomeren = compose_izvestaj(state.inner(), id, &izvestaj_request())
                .expect("a complete izveštaj should compose");
            assert_eq!(pomeren.rok, "2028-01-31", "the leap-year answer, computed");
            assert_eq!(pomeren.odluka_o_usvajanju.rok, "2028-01-31");
        });
    }

    /// Req. 38's other half, and Task 3's open contract: the filing deadline is
    /// **configured**, never a constant in this crate. With nothing configured the
    /// annual izveštaj is refused by name rather than dated by guesswork — a rok
    /// this app invents is one the owner files on, and the čl. 58 prekršaj attaches
    /// to the popis.
    #[test]
    fn the_annual_izvestaj_is_refused_until_the_filing_deadline_is_configured() {
        with_app("popis_izvestaj_rok_unconfigured", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            sign_in_admin(state.inner());

            assert_eq!(
                load_podesavanja(state.inner())
                    .expect("the settings should read")
                    .rok_predaje_fi,
                None,
                "nothing is presumed about a rok this module does not own"
            );

            let error = compose_izvestaj(state.inner(), id, &izvestaj_request())
                .expect_err("an undated annual izveštaj must be refused");
            assert_eq!(error.code(), "popis_rok_predaje_fi_nepoznat");

            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));
            let izvestaj = compose_izvestaj(state.inner(), id, &izvestaj_request())
                .expect("a configured deadline should date the izveštaj");
            assert_eq!(izvestaj.rok, ROK_IZVESTAJA);
        });
    }

    /// The nivelacija limb (req. 33 / čl. 13 st. 2, second limb) reaches the izveštaj
    /// too, and it does **not** read the filing deadline: an in-year popis is due 30
    /// days after the count, and a generator that fell through to the annual rule
    /// would give it a rok months later than the article allows.
    #[test]
    fn a_nivelacija_izvestaj_is_due_thirty_days_after_the_count() {
        with_app("popis_izvestaj_nivelacija_rok", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            sign_in_admin(state.inner());

            // The vrsta is the session's; nothing else about the popis changes.
            state
                .db()
                .open()
                .expect("database should open")
                .execute(
                    "UPDATE popis_sessions SET vrsta = 'nivelacioni' WHERE id = ?1",
                    params![id],
                )
                .expect("the vrsta should update");

            let izvestaj = compose_izvestaj(state.inner(), id, &izvestaj_request())
                .expect("a nivelacija izveštaj needs no filing deadline");

            assert_eq!(izvestaj.vrsta, PopisVrsta::Nivelacioni);
            assert_eq!(izvestaj.rok, "2027-01-30", "31.12.2026 + 30 dana");
            assert_eq!(izvestaj.odluka_o_usvajanju.rok, "2027-01-30");
        });
    }

    /// Req. 29 at the izveštaj. The document carries the knjigovodstveno stanje and
    /// the razlike, so composing one during the count would hand the commission
    /// exactly what PoP čl. 8 st. 5 keeps back — and it would do it through a
    /// document rather than a screen, which is the leak a hidden column never
    /// closes. The seeded perpetual balance appears **nowhere** in the refusal.
    #[test]
    fn no_izvestaj_is_composed_before_the_cl_8_st_5_potpis() {
        with_app("popis_izvestaj_blind", |app| {
            let state = app.state::<AppState>();
            let id = seeded_count(state.inner(), "KOS-1");
            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));

            let error = compose_izvestaj(state.inner(), id, &izvestaj_request())
                .expect_err("an izveštaj during the count must be refused");

            assert_eq!(error.code(), "popis_izvestaj_pre_potpisa");
            let poruka = error.to_string();
            assert!(
                poruka.contains("čl. 8 st. 5"),
                "the refusal must name the article it enforces, said: {poruka}"
            );
            assert!(
                !poruka.contains(&KNJIGOVODSTVENO_STANJE_MILLI.to_string()),
                "no book quantity may leave the query layer during Phase A, said: {poruka}"
            );
        });
    }

    /// Task 5's contract, honoured: a category the shop declared present whose lista
    /// is empty stops the izveštaj, with the refusal Task 5 wrote. Čl. 13 st. 1 has
    /// the izveštaj report the stvarno stanje of the popis, and a popis that declared
    /// cash on hand and wrote none of it down reports a stanje that is not the shop's.
    #[test]
    fn a_declared_but_empty_lista_stops_the_izvestaj() {
        with_app("popis_izvestaj_liste_gate", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));

            let mut request = izvestaj_request();
            request.prijavljene_liste = vec![PopisLista::Roba, PopisLista::Gotovina];
            let error = compose_izvestaj(state.inner(), id, &request)
                .expect_err("a declared empty lista must stop the izveštaj");

            assert_eq!(error.code(), "popis_prazna_prijavljena_lista");
            assert!(error.to_string().contains("gotovina po apoenima"));
        });
    }

    /// Čl. 9 st. 1 t. 4 and t. 6 — the razlike, naturally and in value, computed in
    /// integers from the popisna lista's own cena. The manjak is negative in both
    /// dimensions and the three value figures are consistent with one another, so a
    /// reader who subtracts the two stanja gets the razlika the document prints.
    #[test]
    fn the_izvestaj_values_the_count_the_books_and_the_difference() {
        with_app("popis_izvestaj_valuation", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));

            let izvestaj = compose_izvestaj(state.inner(), id, &izvestaj_request())
                .expect("a complete izveštaj should compose");

            let po_popisu = 7_000 * CENA_MINOR / 1_000;
            let po_knjigama = 15_303_126;
            let zbir = &izvestaj.ukupno;

            assert_eq!(zbir.broj_stavki, 1);
            assert_eq!(zbir.vrednost_po_popisu_minor, po_popisu);
            assert_eq!(zbir.vrednost_po_knjigama_minor, po_knjigama);
            assert_eq!(zbir.vrednosna_razlika_minor, po_popisu - po_knjigama);
            assert!(
                zbir.vrednosna_razlika_minor < 0,
                "7 counted against 61,237 booked is a manjak"
            );
            assert_eq!(zbir.stavke_sa_manjkom, 1);
            assert_eq!(zbir.stavke_sa_viskom, 0);
            assert_eq!(zbir.stavke_bez_cene, 0);
            assert_eq!(zbir.stavke_bez_knjigovodstvenog_stanja, 0);
            assert!(zbir.potpuno);

            let roba = izvestaj
                .liste
                .iter()
                .find(|pregled| pregled.vrsta == PopisLista::Roba)
                .expect("the roba lista is always reported");
            assert_eq!(roba.zbir.vrednost_po_popisu_minor, po_popisu);
            assert_eq!(roba.zbir.vrednosna_razlika_minor, po_popisu - po_knjigama);
            assert_eq!(
                izvestaj.liste.len(),
                PopisLista::ALL.len(),
                "all six liste are reported, the empty ones included"
            );
        });
    }

    /// The honesty rule of the whole module, applied to a number. A stavka with no
    /// cena cannot be valued and a stavka with no knjigovodstveno stanje has no
    /// razlika — so the izveštaj says so instead of counting either as zero, which
    /// would report a manjak the shop does not have and a total that is not a total.
    #[test]
    fn an_unpriced_or_unbooked_stavka_is_reported_and_never_valued_as_zero() {
        with_app("popis_izvestaj_incomplete_valuation", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));

            // Čl. 11 st. 1 — a cash stavka has an apoen but no perpetual record
            // behind it, so it is priced and unbooked. Čl. 12 st. 2's undocumented
            // claim is added without a cena at all.
            let connection = state.db().open().expect("database should open");
            connection
                .execute(
                    "INSERT INTO popis_lines (session_id, lista_vrsta, naziv,
                                              stvarna_kolicina_milli, cena_minor,
                                              created_at, updated_at)
                     VALUES (?1, 'gotovina', 'Novčanica 1.000 RSD', 5000, 100000, ?2, ?2),
                            (?1, 'potrazivanja', 'Potraživanje bez isprave', 1000, NULL, ?2, ?2)",
                    params![id, "2027-01-02T09:40:00Z"],
                )
                .expect("the posebne liste should take their stavke");

            let mut request = izvestaj_request();
            request.prijavljene_liste = vec![
                PopisLista::Roba,
                PopisLista::Gotovina,
                PopisLista::Potrazivanja,
            ];
            let izvestaj = compose_izvestaj(state.inner(), id, &request)
                .expect("an incomplete valuation is reported, not refused");

            let zbir = &izvestaj.ukupno;
            assert_eq!(zbir.broj_stavki, 3);
            assert_eq!(zbir.stavke_bez_cene, 1, "the potraživanje carries no cena");
            assert_eq!(
                zbir.stavke_bez_knjigovodstvenog_stanja, 2,
                "neither posebna lista has a perpetual record behind it"
            );
            assert!(!zbir.potpuno);
            assert_eq!(
                zbir.stavke_sa_manjkom, 1,
                "only the stavka with both sides can show a manjak"
            );
            // The cash stavka is valued (5 × 1.000,00) but contributes no razlika.
            assert_eq!(
                zbir.vrednost_po_popisu_minor,
                7_000 * CENA_MINOR / 1_000 + 500_000
            );
            assert_eq!(zbir.vrednosna_razlika_minor, 1_749_300 - 15_303_126);

            let poruke = izvestaj.upozorenja.join(" | ");
            assert!(
                poruke.contains("nema upisanu cenu") && poruke.contains("knjigovodstveno stanje"),
                "the izveštaj must say which figures are incomplete, said: {poruke}"
            );
        });
    }

    /// Three warnings the izveštaj must carry and none of which may block it: the
    /// req. 40 komisija warning follows the document that names the komisija; the
    /// čl. 9 st. 3 potpis is not yet on these liste; and **the izveštaj is not kept
    /// by this application** — it is composed when it is asked for, and a shop that
    /// closed the screen would otherwise lose what it typed without being told.
    #[test]
    fn the_izvestaj_warns_without_blocking_and_says_it_is_not_stored() {
        with_app("popis_izvestaj_warnings", |app| {
            let state = app.state::<AppState>();
            seed_article(state.inner(), "KOS-1");
            let mut request = open_request();
            request.komisija = vec![KomisijaClanInput {
                ime: "Miloš Đurđević".into(),
                uloga: "jedno_lice".into(),
                rukuje_imovinom: true,
            }];
            let mut connection = state.db().open().expect("database should open");
            let session = open_popis(&mut connection, &request, "2026-12-31T08:00:00Z")
                .expect("the popis should open");
            start_count(&connection, session.id, "2026-12-31T09:00:00Z")
                .expect("the count should start");
            save_line(
                &connection,
                session.id,
                None,
                &line_input("KOS-1", 7_000),
                "2026-12-31T09:10:00Z",
            )
            .expect("a counted line should save");
            sign_phase_a(
                &mut connection,
                session.id,
                &["Miloš Đurđević".to_string()],
                "2026-12-31T17:00:00Z",
            )
            .expect("the čl. 8 st. 5 potpis should record");
            compute_differences(&connection, session.id, "2027-01-02T09:00:00Z")
                .expect("the obračun should open");
            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));

            let izvestaj = compose_izvestaj(state.inner(), session.id, &izvestaj_request())
                .expect("warnings must not block the izveštaj");

            let poruke = izvestaj.upozorenja.join(" | ");
            assert!(
                poruke.contains("Miloš Đurđević") && poruke.contains("čl. 5 st. 1"),
                "the req. 40 warning must travel with the izveštaj, said: {poruke}"
            );
            assert!(
                poruke.contains("čl. 9 st. 3"),
                "the izveštaj rests on liste that are not yet signed, said: {poruke}"
            );
            assert!(
                poruke.contains("ne čuva"),
                "the izveštaj must say that this application does not keep it, said: {poruke}"
            );
        });
    }

    /// The fifth warning limb, and the only one carrying a **statutory deadline of
    /// its own**: čl. 2 st. 6 gives the owner of tuđa roba ten days for a signed
    /// copy of the posebna lista, counted from the day of the popis. The izveštaj is
    /// the document the shop reads when the count is over, so the reminder travels
    /// with it — derived from the liste, carrying the rok, and saying plainly that
    /// this application does not deliver the lista. The negative control is asserted
    /// first: a popis with no tuđa roba on it owes nobody a copy, and a reminder
    /// that fired on every izveštaj would be the noise that teaches a shop to click
    /// past the ones that matter.
    #[test]
    fn a_konsignacija_stavka_puts_the_cl_2_st_6_rok_in_the_izvestaj() {
        with_app("popis_izvestaj_konsignacija", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));

            let bez_konsignacije = compose_izvestaj(state.inner(), id, &izvestaj_request())
                .expect("a complete izveštaj should compose");
            assert!(
                bez_konsignacije
                    .upozorenja
                    .iter()
                    .all(|poruka| !poruka.contains("čl. 2 st. 6")),
                "a popis without tuđa roba owes nobody a copy, warnings were: {:?}",
                bez_konsignacije.upozorenja
            );

            // Čl. 2 st. 5 — tuđa roba goes on its own posebna lista. Inserted the way
            // the other posebne liste are seeded here, because the obračun is open and
            // the stavka is not the one the komisija signed.
            let connection = state.db().open().expect("database should open");
            connection
                .execute(
                    "INSERT INTO popis_lines (session_id, lista_vrsta, naziv,
                                              stvarna_kolicina_milli, cena_minor,
                                              created_at, updated_at)
                     VALUES (?1, 'konsignacija', 'Haljina — vlasnik „Tekstil d.o.o.“',
                             2000, 450000, ?2, ?2)",
                    params![id, "2027-01-02T09:45:00Z"],
                )
                .expect("the konsignacija lista should take its stavka");

            let mut request = izvestaj_request();
            request.prijavljene_liste = vec![PopisLista::Roba, PopisLista::Konsignacija];
            let izvestaj = compose_izvestaj(state.inner(), id, &request)
                .expect("tuđa roba is reported, never a refusal");

            let podsetnik = izvestaj
                .upozorenja
                .iter()
                .find(|poruka| poruka.contains("čl. 2 st. 6"))
                .unwrap_or_else(|| {
                    panic!(
                        "the čl. 2 st. 6 duty must travel with the izveštaj, warnings were: {:?}",
                        izvestaj.upozorenja
                    )
                });
            assert!(
                podsetnik.contains("2027-01-10"),
                "the reminder must carry its rok — ten days from the 31 December count, said: \
                 {podsetnik}"
            );
            assert!(
                podsetnik.contains("ne dostavlja"),
                "the reminder must say the application does not deliver the lista, said: \
                 {podsetnik}"
            );
        });
    }

    /// **No operator string may promise content the composed document does not
    /// carry.** The three computed elements are answered by the figures in `ukupno`
    /// and `liste`, and those figures are money and counts of stavki: nothing
    /// anywhere in this payload is a količina, because a količina summed across
    /// komada, metara and kilograma is a number with no unit. So the uputstvo for
    /// each of the three says what the izveštaj *does* carry and sends the reader to
    /// the popisne liste for the naturalne količine — which is where čl. 9 st. 1
    /// t. 4 puts them, and where they actually are.
    #[test]
    fn the_computed_elements_promise_only_the_figures_the_izvestaj_carries() {
        with_app("popis_izvestaj_uputstva", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));

            let izvestaj = compose_izvestaj(state.inner(), id, &izvestaj_request())
                .expect("a complete izveštaj should compose");

            /// Every key in the payload, however deep — a promise is judged against
            /// the whole document and not against one struct.
            fn kljucevi(vrednost: &serde_json::Value, skup: &mut Vec<String>) {
                match vrednost {
                    serde_json::Value::Object(mapa) => {
                        for (kljuc, dete) in mapa {
                            skup.push(kljuc.clone());
                            kljucevi(dete, skup);
                        }
                    }
                    serde_json::Value::Array(niz) => {
                        for dete in niz {
                            kljucevi(dete, skup);
                        }
                    }
                    _ => {}
                }
            }
            let payload =
                serde_json::to_value(&izvestaj).expect("the izveštaj should serialize for IPC");
            let mut sva = Vec::new();
            kljucevi(&payload, &mut sva);
            let naturalna: Vec<&String> = sva
                .iter()
                .filter(|kljuc| {
                    let kljuc = kljuc.to_lowercase();
                    kljuc.contains("kolicina") || kljuc.contains("milli")
                })
                .collect();
            assert!(
                naturalna.is_empty(),
                "the izveštaj carries no natural quantity, so nothing in it may promise one — \
                 if this ever changes, the three uputstva below must change with it; found: \
                 {naturalna:?}"
            );

            for element in [
                IzvestajElement::StvarnoStanje,
                IzvestajElement::KnjigovodstvenoStanje,
                IzvestajElement::Razlike,
            ] {
                let pregled = izvestaj
                    .elementi
                    .iter()
                    .find(|pregled| pregled.element == element)
                    .expect("every element is present");
                assert!(
                    pregled.tekst.is_none(),
                    "{element:?} is computed, so nobody types it"
                );
                assert!(
                    pregled.uputstvo.contains("Izveštaj iskazuje"),
                    "{element:?} must say what the izveštaj itself carries, said: {}",
                    pregled.uputstvo
                );
                assert!(
                    pregled.uputstvo.contains("na popisnim listama"),
                    "{element:?} must send the reader to the popisne liste for the količine \
                     rather than promise them here, said: {}",
                    pregled.uputstvo
                );
            }
        });
    }

    /// Req. 36's gate must not be **opt-in from the wire**. `ensure_liste_kompletne`
    /// refuses only a *declared* lista that is empty, so a caller that omitted the
    /// declaration would pass the gate vacuously and compose an izveštaj with no
    /// req. 36 check behind it at all. `popis_provera_listi` requires the identical
    /// argument for the identical rule; so does the generator. „Ništa nije
    /// prijavljeno“ stays expressible — as an explicit empty array, which is an
    /// answer, unlike an absent field.
    #[test]
    fn an_izvestaj_request_must_declare_which_liste_the_shop_has() {
        let narativ = serde_json::to_value(potpun_narativ()).expect("the narativ should serialize");

        let error = serde_json::from_value::<IzvestajRequest>(serde_json::json!({
            "narativ": narativ.clone(),
        }))
        .expect_err("an omitted declaration must be a wire error, not an empty declaration");
        assert!(
            error.to_string().contains("prijavljeneListe"),
            "the wire error must name the missing field, said: {error}"
        );

        let prijavljeno = serde_json::from_value::<IzvestajRequest>(serde_json::json!({
            "prijavljeneListe": ["roba", "konsignacija"],
            "narativ": narativ.clone(),
        }))
        .expect("a declaration should deserialize");
        assert_eq!(
            prijavljeno.prijavljene_liste,
            vec![PopisLista::Roba, PopisLista::Konsignacija]
        );

        let nista = serde_json::from_value::<IzvestajRequest>(serde_json::json!({
            "prijavljeneListe": [],
            "narativ": narativ,
        }))
        .expect("„nothing declared“ is an answer and must stay expressible");
        assert!(nista.prijavljene_liste.is_empty());
    }

    /// The rok is only as good as the date it is computed from, and a filing
    /// deadline left over from last year computes a rok that fell **before** the
    /// popis was even taken. That is a staleness signal available without a clock —
    /// and it is a warning, not a refusal, because a genuinely late popis is still a
    /// popis and its izveštaj still has to be written.
    #[test]
    fn a_rok_that_falls_before_the_count_is_flagged_as_a_stale_setting() {
        with_app("popis_izvestaj_stale_rok", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            set_rok_predaje_fi(state.inner(), Some("2026-03-31"));

            let izvestaj = compose_izvestaj(state.inner(), id, &izvestaj_request())
                .expect("a late popis still gets its izveštaj");

            assert_eq!(izvestaj.rok, "2026-01-30");
            let poruke = izvestaj.upozorenja.join(" | ");
            assert!(
                poruke.contains("2026-03-31") && poruke.contains("pre datuma popisa"),
                "the stale filing deadline must be named, said: {poruke}"
            );
        });
    }

    /// The configured filing deadline is a date this crate stores and hands to the
    /// deadline engine, so it is validated where it is written rather than where it
    /// is used — and it is admin-only, like every other popis command.
    #[test]
    fn the_filing_deadline_setting_round_trips_and_refuses_an_unreadable_date() {
        with_app("popis_podesavanja", |app| {
            let state = app.state::<AppState>();

            sign_in_cashier(state.inner());
            assert_eq!(
                popis_podesavanja_get(app.state::<AppState>())
                    .expect_err("a cashier must not read the popis settings")
                    .code,
                "forbidden"
            );

            sign_in_admin(state.inner());
            assert_eq!(
                popis_podesavanja_get(app.state::<AppState>())
                    .expect("an admin should read the settings")
                    .rok_predaje_fi,
                None
            );

            let error = popis_podesavanja_set(
                app.state::<AppState>(),
                PopisPodesavanja {
                    rok_predaje_fi: Some("31.03.2027.".into()),
                },
            )
            .expect_err("an unreadable rok must not be stored");
            assert_eq!(error.code, "popis_neispravan_datum");

            let sacuvano = popis_podesavanja_set(
                app.state::<AppState>(),
                PopisPodesavanja {
                    rok_predaje_fi: Some(format!("  {ROK_PREDAJE_FI}  ")),
                },
            )
            .expect("a readable rok should store");
            assert_eq!(sacuvano.rok_predaje_fi.as_deref(), Some(ROK_PREDAJE_FI));
            assert_eq!(
                popis_podesavanja_get(app.state::<AppState>())
                    .expect("an admin should read the settings")
                    .rok_predaje_fi
                    .as_deref(),
                Some(ROK_PREDAJE_FI)
            );
        });
    }

    #[test]
    fn popis_izvestaj_is_rejected_for_a_cashier() {
        with_app("popis_izvestaj_admin_gate", |app| {
            let state = app.state::<AppState>();
            let id = seeded_izvestaj_popis(state.inner(), "KOS-1");
            set_rok_predaje_fi(state.inner(), Some(ROK_PREDAJE_FI));

            sign_in_cashier(state.inner());
            let error = popis_izvestaj(app.state::<AppState>(), id, izvestaj_request())
                .expect_err("a cashier must not compose the izveštaj");
            assert_eq!(error.code, "forbidden");

            sign_in_admin(state.inner());
            let izvestaj = popis_izvestaj(app.state::<AppState>(), id, izvestaj_request())
                .expect("an admin should compose the izveštaj");
            assert_eq!(izvestaj.session_id, id);
            assert_eq!(izvestaj.rok, ROK_IZVESTAJA);
        });
    }
}
