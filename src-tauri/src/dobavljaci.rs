//! Dobavljači i primljene isprave — the supplier's isprava o nabavci.
//!
//! Legal authority: `docs/KEP-VERIFIED-RULES.md` (§3 for the ZoT čl. 29 st. 1
//! field list and the verbatim source-document list, §2.1 for PEP čl. 15 st. 4
//! kolona 3, §8 t. 5 for what is *not* pinned). Design:
//! `docs/superpowers/specs/2026-08-09-dobavljac-primljena-isprava-design.md`.
//!
//! **What this module does not do.** ZoT čl. 29 st. 1 is a duty to *possess* the
//! supplier's isprava. Recording a broj and a datum is not possession. This
//! module stores the čl. 29 st. 1 identifying data and the operator's own
//! assertion that the paper is filed — it holds no document. `poseduje_ispravu`
//! defaults to 0 and is only ever set by an explicit operator act
//! (`confirm_possession`), the same posture v16 took for
//! `cash_movements.documented_per_pravilnik`: store the assertion, never presume
//! it. A presumption here would hand the owner a false clear on the one duty that
//! is about physically having the thing.
#![allow(dead_code)]

use crate::app_error::AppError;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

/// The kind of isprava. KEP-VERIFIED-RULES §3 lists the valid source documents
/// verbatim — *faktura, otpremnica, faktura-otpremnica, dostavnica, interna
/// prenosnica, prijemnica ili druga odgovarajuća isprava za robu* — and that list
/// **ends in an open clause**. `Druga` is that clause: a document kind the
/// Pravilnik allows but does not name is representable, carrying the operator's
/// own naziv, because refusing it would be this software narrowing a rule the
/// regulation left open. The database column has no CHECK for the same reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VrstaIsprave {
    Faktura,
    Otpremnica,
    FakturaOtpremnica,
    Dostavnica,
    InternaPrenosnica,
    Prijemnica,
    /// „druga odgovarajuća isprava za robu“ — the naziv is the operator's.
    Druga(String),
}

impl VrstaIsprave {
    /// The six named kinds, in the order §3 gives them. Offered by the UI; never
    /// used to *reject* a value, since the seventh possibility is open-ended.
    pub const IMENOVANE: [&'static str; 6] = [
        "faktura",
        "otpremnica",
        "faktura-otpremnica",
        "dostavnica",
        "interna prenosnica",
        "prijemnica",
    ];

    /// Parses a stored/submitted vrsta. A value outside the six named kinds is
    /// **not an error** — it becomes `Druga`. Only a blank one is refused, since
    /// kolona 3 must name the document (PEP čl. 15 st. 4) and „“ names nothing.
    pub fn parse(value: &str) -> Result<Self, AppError> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Err(validation_error(
                "Vrsta isprave je obavezna.",
                "vrstaIsprave",
            ));
        }
        Ok(match trimmed.to_lowercase().as_str() {
            "faktura" => Self::Faktura,
            "otpremnica" => Self::Otpremnica,
            "faktura-otpremnica" => Self::FakturaOtpremnica,
            "dostavnica" => Self::Dostavnica,
            "interna prenosnica" => Self::InternaPrenosnica,
            "prijemnica" => Self::Prijemnica,
            _ => Self::Druga(trimmed.to_string()),
        })
    }

    /// The naziv as it goes into kolona 3 and into storage.
    pub fn naziv(&self) -> &str {
        match self {
            Self::Faktura => "faktura",
            Self::Otpremnica => "otpremnica",
            Self::FakturaOtpremnica => "faktura-otpremnica",
            Self::Dostavnica => "dostavnica",
            Self::InternaPrenosnica => "interna prenosnica",
            Self::Prijemnica => "prijemnica",
            Self::Druga(naziv) => naziv,
        }
    }
}

/// A supplier master record. Reused across deliveries so a PIB and a matični broj
/// are typed once rather than per delivery — those being exactly the fields where
/// a typo is the čl. 29 st. 1 defect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Dobavljac {
    pub id: i64,
    pub poslovno_ime: String,
    pub adresa: String,
    pub pib: String,
    /// Spans matični broj and BPG under one name — see the v23 migration comment
    /// and KEP-VERIFIED-RULES §8 t. 5: the label list is **not pinned** against an
    /// official consolidated text, so nothing here asserts that it is.
    pub maticni_broj_bpg: String,
    /// PEP čl. 15 st. 4: a natural-person supplier goes into kolona 3 as *ime i
    /// prebivalište* rather than poslovno ime.
    pub fizicko_lice: bool,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveDobavljacRequest {
    pub id: Option<i64>,
    pub poslovno_ime: String,
    #[serde(default)]
    pub adresa: String,
    #[serde(default)]
    pub pib: String,
    #[serde(default)]
    pub maticni_broj_bpg: String,
    #[serde(default)]
    pub fizicko_lice: bool,
}

/// A received supplier document. The identity fields are a **snapshot** taken at
/// creation, not a join: a kalkulacija reprinted next year must render what it
/// rendered when it was issued, and a live join would let an edit to the
/// `dobavljaci` row rewrite documents already issued.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PrimljenaIsprava {
    pub id: i64,
    pub dobavljac_id: i64,
    pub dobavljac_poslovno_ime: String,
    pub dobavljac_adresa: String,
    pub dobavljac_pib: String,
    pub dobavljac_maticni_broj_bpg: String,
    pub dobavljac_fizicko_lice: bool,
    pub vrsta: String,
    pub broj: String,
    /// The document's own date (`YYYY-MM-DD`) — PEP čl. 15 st. 3's kolona-3
    /// document date, never the kolona-2 booking date.
    pub datum: String,
    /// The operator's assertion that the paper isprava is held. **Not proof of
    /// possession** — the application stores no document.
    pub poseduje_ispravu: bool,
    pub poseduje_potvrdio: Option<i64>,
    pub poseduje_potvrdjeno_at: Option<String>,
    pub napomena: Option<String>,
    pub created_at: String,
}

impl PrimljenaIsprava {
    /// The dobavljač as kolona 3 must name them (PEP čl. 15 st. 4): poslovno ime
    /// for a pravno lice, **ime i prebivalište** for a natural person.
    pub fn kolona3_dobavljac(&self) -> String {
        if self.dobavljac_fizicko_lice && !self.dobavljac_adresa.trim().is_empty() {
            format!(
                "{}, {}",
                self.dobavljac_poslovno_ime.trim(),
                self.dobavljac_adresa.trim()
            )
        } else {
            self.dobavljac_poslovno_ime.trim().to_string()
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateIspravaRequest {
    pub dobavljac_id: i64,
    pub vrsta: String,
    pub broj: String,
    pub datum: String,
    /// Whether the operator asserts, at creation, that the document is held.
    #[serde(default)]
    pub poseduje_ispravu: bool,
    pub napomena: Option<String>,
}

fn validation_error(message: &str, field: &str) -> AppError {
    AppError::validation(message, serde_json::json!({ "field": field }))
}

fn normalized(value: &str) -> String {
    value.trim().to_string()
}

/// `YYYY-MM-DD`, the shape every other date column in this crate stores. Rejected
/// rather than coerced: a datum that silently became today's date would put the
/// booking date into kolona 3, which is the conflation §2.1 warns against.
fn validate_datum(value: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    let bytes = trimmed.as_bytes();
    let shaped = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| matches!(i, 4 | 7) || b.is_ascii_digit());
    if !shaped {
        return Err(validation_error(
            "Datum isprave nije ispravan (očekuje se GGGG-MM-DD).",
            "datum",
        ));
    }
    Ok(trimmed.to_string())
}

pub fn save_dobavljac(
    conn: &Connection,
    request: SaveDobavljacRequest,
    now: &str,
) -> Result<i64, AppError> {
    let poslovno_ime = normalized(&request.poslovno_ime);
    if poslovno_ime.is_empty() {
        return Err(validation_error(
            "Poslovno ime dobavljača je obavezno.",
            "poslovnoIme",
        ));
    }

    match request.id {
        Some(id) => {
            let changed = conn.execute(
                "UPDATE dobavljaci
                 SET poslovno_ime = ?1, adresa = ?2, pib = ?3, maticni_broj_bpg = ?4,
                     fizicko_lice = ?5, updated_at = ?6
                 WHERE id = ?7",
                params![
                    poslovno_ime,
                    normalized(&request.adresa),
                    normalized(&request.pib),
                    normalized(&request.maticni_broj_bpg),
                    i64::from(request.fizicko_lice),
                    now,
                    id,
                ],
            )?;
            if changed == 0 {
                return Err(AppError::not_found(format!("Dobavljač {id} ne postoji.")));
            }
            Ok(id)
        }
        None => {
            conn.execute(
                "INSERT INTO dobavljaci (
                    poslovno_ime, adresa, pib, maticni_broj_bpg, fizicko_lice,
                    created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
                params![
                    poslovno_ime,
                    normalized(&request.adresa),
                    normalized(&request.pib),
                    normalized(&request.maticni_broj_bpg),
                    i64::from(request.fizicko_lice),
                    now,
                ],
            )?;
            Ok(conn.last_insert_rowid())
        }
    }
}

pub fn list_dobavljaci(conn: &Connection) -> Result<Vec<Dobavljac>, AppError> {
    let mut statement = conn.prepare(
        "SELECT id, poslovno_ime, adresa, pib, maticni_broj_bpg, fizicko_lice, active
         FROM dobavljaci WHERE active = 1 ORDER BY poslovno_ime",
    )?;
    let rows = statement
        .query_map([], |row| {
            Ok(Dobavljac {
                id: row.get(0)?,
                poslovno_ime: row.get(1)?,
                adresa: row.get(2)?,
                pib: row.get(3)?,
                maticni_broj_bpg: row.get(4)?,
                fizicko_lice: row.get::<_, i64>(5)? == 1,
                active: row.get::<_, i64>(6)? == 1,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Creates the isprava, snapshotting the supplier's identity as it stands now.
pub fn create_isprava(
    conn: &Connection,
    request: CreateIspravaRequest,
    acting_user_id: i64,
    now: &str,
) -> Result<i64, AppError> {
    let vrsta = VrstaIsprave::parse(&request.vrsta)?;
    let broj = normalized(&request.broj);
    if broj.is_empty() {
        return Err(validation_error("Broj isprave je obavezan.", "broj"));
    }
    let datum = validate_datum(&request.datum)?;

    let dobavljac = conn
        .query_row(
            "SELECT poslovno_ime, adresa, pib, maticni_broj_bpg, fizicko_lice
             FROM dobavljaci WHERE id = ?1",
            params![request.dobavljac_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            },
        )
        .optional()?
        .ok_or_else(|| {
            AppError::not_found(format!("Dobavljač {} ne postoji.", request.dobavljac_id))
        })?;

    // The assertion is only recorded when it is actually made; an unasserted
    // isprava carries no potvrdio and no timestamp, so „nobody said so“ and
    // „somebody said so“ are distinguishable rather than both reading as 0.
    let (potvrdio, potvrdjeno_at) = if request.poseduje_ispravu {
        (Some(acting_user_id), Some(now.to_string()))
    } else {
        (None, None)
    };

    conn.execute(
        "INSERT INTO primljene_isprave (
            dobavljac_id, dobavljac_poslovno_ime, dobavljac_adresa, dobavljac_pib,
            dobavljac_maticni_broj_bpg, dobavljac_fizicko_lice,
            vrsta, broj, datum, poseduje_ispravu, poseduje_potvrdio,
            poseduje_potvrdjeno_at, napomena, created_by, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)",
        params![
            request.dobavljac_id,
            dobavljac.0,
            dobavljac.1,
            dobavljac.2,
            dobavljac.3,
            dobavljac.4,
            vrsta.naziv(),
            broj,
            datum,
            i64::from(request.poseduje_ispravu),
            potvrdio,
            potvrdjeno_at,
            request.napomena.as_deref().map(str::trim),
            acting_user_id,
            now,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

const ISPRAVA_COLUMNS: &str = "id, dobavljac_id, dobavljac_poslovno_ime, dobavljac_adresa,
     dobavljac_pib, dobavljac_maticni_broj_bpg, dobavljac_fizicko_lice,
     vrsta, broj, datum, poseduje_ispravu, poseduje_potvrdio,
     poseduje_potvrdjeno_at, napomena, created_at";

fn map_isprava(row: &rusqlite::Row<'_>) -> rusqlite::Result<PrimljenaIsprava> {
    Ok(PrimljenaIsprava {
        id: row.get(0)?,
        dobavljac_id: row.get(1)?,
        dobavljac_poslovno_ime: row.get(2)?,
        dobavljac_adresa: row.get(3)?,
        dobavljac_pib: row.get(4)?,
        dobavljac_maticni_broj_bpg: row.get(5)?,
        dobavljac_fizicko_lice: row.get::<_, i64>(6)? == 1,
        vrsta: row.get(7)?,
        broj: row.get(8)?,
        datum: row.get(9)?,
        poseduje_ispravu: row.get::<_, i64>(10)? == 1,
        poseduje_potvrdio: row.get(11)?,
        poseduje_potvrdjeno_at: row.get(12)?,
        napomena: row.get(13)?,
        created_at: row.get(14)?,
    })
}

/// Loads one isprava inside the caller's receive transaction.
pub fn load_isprava_tx(tx: &Transaction<'_>, id: i64) -> Result<PrimljenaIsprava, AppError> {
    tx.query_row(
        &format!("SELECT {ISPRAVA_COLUMNS} FROM primljene_isprave WHERE id = ?1"),
        params![id],
        map_isprava,
    )
    .optional()?
    .ok_or_else(|| AppError::not_found(format!("Isprava {id} ne postoji.")))
}

pub fn list_isprave(conn: &Connection, limit: i64) -> Result<Vec<PrimljenaIsprava>, AppError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {ISPRAVA_COLUMNS} FROM primljene_isprave
         ORDER BY datum DESC, id DESC LIMIT ?1"
    ))?;
    let rows = statement
        .query_map(params![limit], map_isprava)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Records the operator's assertion that the document is held. One-way: there is
/// a verb for asserting possession and none for un-asserting it, because the
/// interesting direction is a shop claiming to hold paper it does not have, and
/// an assertion already made is a fact about what was said.
pub fn confirm_possession(
    conn: &Connection,
    isprava_id: i64,
    acting_user_id: i64,
    now: &str,
) -> Result<(), AppError> {
    let changed = conn.execute(
        "UPDATE primljene_isprave
         SET poseduje_ispravu = 1, poseduje_potvrdio = ?1, poseduje_potvrdjeno_at = ?2
         WHERE id = ?3 AND poseduje_ispravu = 0",
        params![acting_user_id, now, isprava_id],
    )?;
    if changed == 0 {
        // Either the id is unknown or the assertion already stands. The second is
        // not an error, so distinguish before failing.
        let exists: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM primljene_isprave WHERE id = ?1)",
            params![isprava_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::not_found(format!(
                "Isprava {isprava_id} ne postoji."
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{remove_test_database, test_database_path, Db};

    fn with_db(test_name: &str, test: impl FnOnce(&mut Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("db init");
            let mut connection = db.open().expect("open");
            test(&mut connection);
        }
        remove_test_database(&path);
    }

    fn seed_dobavljac(conn: &Connection, fizicko: bool) -> i64 {
        save_dobavljac(
            conn,
            SaveDobavljacRequest {
                id: None,
                poslovno_ime: "ABC d.o.o.".into(),
                adresa: "Bulevar 1, Novi Pazar".into(),
                pib: "123456789".into(),
                maticni_broj_bpg: "20123456".into(),
                fizicko_lice: fizicko,
            },
            "2026-07-03T09:00:00Z",
        )
        .expect("save dobavljač")
    }

    fn isprava_request(dobavljac_id: i64) -> CreateIspravaRequest {
        CreateIspravaRequest {
            dobavljac_id,
            vrsta: "otpremnica".into(),
            broj: "125".into(),
            datum: "2026-07-03".into(),
            poseduje_ispravu: false,
            napomena: None,
        }
    }

    /// §3's source-document list ends in an open clause, so a kind outside the six
    /// named ones is representable rather than refused.
    #[test]
    fn an_unnamed_document_kind_is_druga_not_an_error() {
        let parsed = VrstaIsprave::parse("zapisnik o prijemu").expect("open clause");
        assert_eq!(
            parsed,
            VrstaIsprave::Druga("zapisnik o prijemu".into()),
            "„druga odgovarajuća isprava za robu“ must survive parsing"
        );
        assert_eq!(parsed.naziv(), "zapisnik o prijemu");
        // ...but kolona 3 has to name the document, so a blank one is refused.
        assert!(VrstaIsprave::parse("   ").is_err());
    }

    #[test]
    fn the_six_named_kinds_round_trip() {
        for naziv in VrstaIsprave::IMENOVANE {
            let parsed = VrstaIsprave::parse(naziv).expect("named kind");
            assert_eq!(parsed.naziv(), naziv);
            assert!(
                !matches!(parsed, VrstaIsprave::Druga(_)),
                "{naziv} is named in §3 and must not fall through to Druga"
            );
        }
    }

    /// The §3.1 snapshot property, asserted rather than commented: editing the
    /// supplier after the fact must not rewrite an isprava already issued.
    #[test]
    fn editing_a_dobavljac_does_not_rewrite_an_isprava_already_created() {
        with_db("dobavljac_snapshot", |conn| {
            let dobavljac_id = seed_dobavljac(conn, false);
            let isprava_id = create_isprava(
                conn,
                isprava_request(dobavljac_id),
                1,
                "2026-07-04T09:00:00Z",
            )
            .expect("create isprava");

            save_dobavljac(
                conn,
                SaveDobavljacRequest {
                    id: Some(dobavljac_id),
                    poslovno_ime: "XYZ d.o.o.".into(),
                    adresa: "Druga 2, Beograd".into(),
                    pib: "987654321".into(),
                    maticni_broj_bpg: "20999999".into(),
                    fizicko_lice: false,
                },
                "2026-08-01T09:00:00Z",
            )
            .expect("edit dobavljač");

            let isprave = list_isprave(conn, 10).expect("list");
            let isprava = isprave
                .iter()
                .find(|row| row.id == isprava_id)
                .expect("the isprava");
            assert_eq!(
                isprava.dobavljac_poslovno_ime, "ABC d.o.o.",
                "a document already issued must render what it rendered when issued"
            );
            assert_eq!(isprava.dobavljac_pib, "123456789");
        });
    }

    /// PEP čl. 15 st. 4: poslovno ime for a pravno lice, ime i prebivalište for a
    /// natural person.
    #[test]
    fn kolona3_names_a_natural_person_with_their_prebivaliste() {
        with_db("dobavljac_kolona3", |conn| {
            let pravno = seed_dobavljac(conn, false);
            let isprava = load_one(
                conn,
                create_isprava(conn, isprava_request(pravno), 1, "2026-07-04T09:00:00Z").unwrap(),
            );
            assert_eq!(isprava.kolona3_dobavljac(), "ABC d.o.o.");

            let fizicko = save_dobavljac(
                conn,
                SaveDobavljacRequest {
                    id: None,
                    poslovno_ime: "Petar Petrović".into(),
                    adresa: "Novi Pazar".into(),
                    pib: String::new(),
                    maticni_broj_bpg: String::new(),
                    fizicko_lice: true,
                },
                "2026-07-03T09:00:00Z",
            )
            .expect("save");
            let mut request = isprava_request(fizicko);
            request.broj = "7".into();
            let isprava = load_one(
                conn,
                create_isprava(conn, request, 1, "2026-07-04T09:00:00Z").unwrap(),
            );
            assert_eq!(
                isprava.kolona3_dobavljac(),
                "Petar Petrović, Novi Pazar",
                "a natural-person supplier goes into kolona 3 with prebivalište"
            );
        });
    }

    fn load_one(conn: &Connection, id: i64) -> PrimljenaIsprava {
        list_isprave(conn, 50)
            .expect("list")
            .into_iter()
            .find(|row| row.id == id)
            .expect("isprava")
    }

    /// Possession is an assertion somebody makes, never a default.
    #[test]
    fn possession_is_unasserted_until_somebody_asserts_it() {
        with_db("dobavljac_possession", |conn| {
            let dobavljac_id = seed_dobavljac(conn, false);
            let id = create_isprava(
                conn,
                isprava_request(dobavljac_id),
                1,
                "2026-07-04T09:00:00Z",
            )
            .expect("create");

            let isprava = load_one(conn, id);
            assert!(!isprava.poseduje_ispravu, "defaults to unasserted");
            assert!(
                isprava.poseduje_potvrdio.is_none() && isprava.poseduje_potvrdjeno_at.is_none(),
                "nobody said so — that must be distinguishable from somebody having said so"
            );

            confirm_possession(conn, id, 1, "2026-07-05T10:00:00Z").expect("confirm");
            let isprava = load_one(conn, id);
            assert!(isprava.poseduje_ispravu);
            assert_eq!(isprava.poseduje_potvrdio, Some(1));
            assert_eq!(
                isprava.poseduje_potvrdjeno_at.as_deref(),
                Some("2026-07-05T10:00:00Z")
            );
        });
    }

    #[test]
    fn confirming_possession_on_a_missing_isprava_is_not_found() {
        with_db("dobavljac_possession_missing", |conn| {
            let err =
                confirm_possession(conn, 999, 1, "2026-07-05T10:00:00Z").expect_err("missing id");
            assert_eq!(err.code(), "not_found");
        });
    }

    /// A datum that silently became today would put the booking date into kolona
    /// 3 — the conflation §2.1 warns against in as many words.
    #[test]
    fn a_malformed_datum_is_refused_rather_than_coerced() {
        with_db("dobavljac_datum", |conn| {
            let dobavljac_id = seed_dobavljac(conn, false);
            for bad in ["03.07.2026", "2026-7-3", "", "juče"] {
                let mut request = isprava_request(dobavljac_id);
                request.datum = bad.into();
                let err = create_isprava(conn, request, 1, "2026-07-04T09:00:00Z")
                    .expect_err("malformed datum");
                assert_eq!(err.code(), "validation_error", "rejected {bad}");
            }
        });
    }

    #[test]
    fn a_blank_broj_is_refused() {
        with_db("dobavljac_broj", |conn| {
            let dobavljac_id = seed_dobavljac(conn, false);
            let mut request = isprava_request(dobavljac_id);
            request.broj = "  ".into();
            let err =
                create_isprava(conn, request, 1, "2026-07-04T09:00:00Z").expect_err("blank broj");
            assert_eq!(err.code(), "validation_error");
        });
    }
}
