//! Admin-gated command layer over the popis (SW-16, reqs. 29–41).
//!
//! Thin in the same sense the other compliance modules are thin: every *decision*
//! about what may follow what lives in `crate::popis` — the state machine, the
//! čl. 8 st. 5 release predicate and the čl. 13 st. 2 deadline engine are pure and
//! are not re-derived here. What lives here is the part that touches rows: the
//! sessions, the liste, the two potpisi and the knjiženje.
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
use crate::popis::{advance, book_quantities_released, PopisEvent, PopisStatus, PopisVrsta};
use crate::state::AppState;

/// The six `popis_lines.lista_vrsta` values of migration v20 (req. 36). Which of
/// them is *required* when its category is present is Task 5's rule; this module
/// only refuses a value the column would refuse anyway, so the shop reads a
/// sentence rather than a CHECK-constraint failure.
const LISTE_VRSTE: [&str; 6] = [
    "roba",
    "ostecena",
    "van_objekta",
    "gotovina",
    "potrazivanja",
    "konsignacija",
];

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
    /// here, because nothing in the schema records that decision yet; Task 6 owns
    /// it and owes this gate its second limb.
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
    /// One of [`LISTE_VRSTE`].
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
    /// Req. 40 — warnings, never blocks.
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
fn read_lines(
    connection: &Connection,
    session_id: i64,
    released: bool,
) -> Result<Vec<PopisLineView>, AppError> {
    if !released {
        let mut statement = connection.prepare(
            "SELECT id, lista_vrsta, sifra, naziv, vrsta, jedinica_mere,
                    stvarna_kolicina_milli, blizi_opis
             FROM popis_lines
             WHERE session_id = ?1
             ORDER BY id",
        )?;
        let rows = statement.query_map(params![session_id], |row| {
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
                cena_minor: None,
            })
        })?;

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
    let upozorenja = komisija_upozorenja(&komisija);
    let linije = read_lines(connection, id, knjigovodstvo_dostupno)?;

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
        upozorenja,
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
/// usvajanju is a separate artefact that nothing records yet (Task 6), so this
/// gate does not check it and no string here claims it did.
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
}

fn read_potpisana_stavka(
    connection: &Connection,
    session_id: i64,
    line_id: i64,
) -> Result<PotpisanaStavka, AppError> {
    connection
        .query_row(
            "SELECT lista_vrsta, sifra, naziv, vrsta, jedinica_mere,
                    stvarna_kolicina_milli, blizi_opis
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
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("Stavka popisne liste nije pronađena."))
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

    if !LISTE_VRSTE.contains(&input.lista_vrsta.as_str()) {
        return Err(AppError::business(
            "popis_nepoznata_lista",
            format!(
                "Nepoznata vrsta popisne liste „{}“ (PoP čl. 2 st. 5, čl. 10–12).",
                input.lista_vrsta
            ),
        ));
    }
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
        PopisStatus::Draft | PopisStatus::Counting => {}
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
                         st. 1 t. 1), a razlikuje se: {polja}. U obračunu se popunjavaju samo \
                         cena i knjigovodstvena količina — ispravka same stavke sprovodi se novim \
                         popisom."
                    ),
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
}
