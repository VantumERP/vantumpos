//! SW-17 — the internal record of every personal-data breach (ZZPL čl. 52).
//!
//! **Log first, decide notifiability second** (req. 43). Čl. 52 st. 6 binds the
//! rukovalac to document *„svaku povredu“* — with no risk qualifier anywhere in
//! it. The risk gate sits one stav up, in st. 1, and it governs only whether the
//! **Poverenik** must be told. A wizard that asks „is this notifiable?“ and
//! discards the No answers inverts the statute and produces the Poverenik's own
//! zero-score finding (KL-001 pitanje 6). So notifiability here is a *derived
//! flag on a row that always exists*, never a gate in front of one.
//!
//! **Why the record carries far more than the three st. 6 elements** (req. 44).
//! St. 7 says the documentation must let the Poverenik establish whether the
//! rukovalac complied with *the whole article*. Facts, consequences and remedial
//! measures alone cannot do that, so the row also carries: the immutable
//! saznanje instant that starts the 72 h clock; the incident's own occurred and
//! discovered instants, which are different facts; the risk assessment; the
//! explicit notify / do-not-notify decision **with its reasoning**; when the
//! Poverenik was actually told; the st. 2 delay justification; and a separate
//! čl. 53 block for the affected individuals.
//!
//! **What this module never does.** It never edits `saznanje_at` — a movable
//! clock anchor would make the st. 2 delay reason optional in hindsight, and v18
//! carries a `BEFORE UPDATE` trigger that refuses it at the storage layer too.
//! It never records identities of affected individuals: the prescribed obrazac
//! asks for *„broj lica na koja se podaci odnose“*, a count, and this row holds
//! a count. And it never lets the incident's prose reach `audit_events`, whose
//! exclusion list (req. 4) governs that table absolutely — the čl. 48 st. 2 line
//! about a breach record carries the record's id and nothing else.
//!
//! `now` is always a parameter, and every deadline is decided by comparing
//! parsed instants — never by string ordering and never by `datetime('now')`.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use tauri::State;
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime, UtcOffset};

use crate::app_error::{AppError, CommandError};
use crate::audit::{AuditAction, AuditDraft, AuditObjectType, AuditReason, AuditRecipient};
use crate::clock::utc_now;
use crate::commands::audit::{append_audit_event, record_audit, AuditEntry};
use crate::commands::reports::ExportedFile;
use crate::commands::settings::CompanySettings;
use crate::legal::LegalNotice;
use crate::state::AppState;

use super::auth::require_admin;

/// Pravilnik 40/2019 čl. 3 — a flat 72 časa od saznanja, without the statute's
/// *„bez nepotrebnog odlaganja, ili, ako je to moguće“* softener. This is the
/// number the countdown and the st. 2 delay gate are both measured against.
const ROK_OBAVESTAVANJA_SATI: i64 = 72;

/// The three čl. 52 st. 6 elements are descriptions of an incident, so they are
/// free text — but a register is not a notes field, and an unbounded column is
/// how a roster of names ends up in one.
const MAX_OPIS_ZNAKOVA: usize = 2000;

/// A shorter bound for the reasoning fields, which are justifications rather
/// than narratives.
const MAX_OBRAZLOZENJE_ZNAKOVA: usize = 1000;

/// The risk assessment's outcome. Čl. 52 st. 1 turns on *rizik*, čl. 53 st. 1 on
/// *visok rizik*, and the two duties run to different addressees — so one column
/// with three values answers both questions and neither is inferred from the
/// other.
///
/// The codes MUST stay identical to the `risk_outcome` CHECK list in v18.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskOutcome {
    BezRizika,
    Rizik,
    VisokRizik,
}

impl RiskOutcome {
    pub const ALL: [Self; 3] = [Self::BezRizika, Self::Rizik, Self::VisokRizik];

    pub fn as_code(self) -> &'static str {
        match self {
            Self::BezRizika => "bez_rizika",
            Self::Rizik => "rizik",
            Self::VisokRizik => "visok_rizik",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.as_code() == code)
    }

    /// How the assessment reads on the Pravilnik 40/2019 obrazac — the statute's
    /// own wording of the two thresholds, so the Poverenik reads the test he
    /// applies rather than this app's shorthand for it.
    pub fn label(self) -> &'static str {
        match self {
            Self::BezRizika => {
                "povreda ne može da proizvede rizik po prava i slobode fizičkih lica"
            }
            Self::Rizik => "povreda može da proizvede rizik po prava i slobode fizičkih lica",
            Self::VisokRizik => {
                "povreda može da proizvede visok rizik po prava i slobode fizičkih lica"
            }
        }
    }
}

/// The decision the rukovalac actually took, recorded as a decision rather than
/// inferred from the presence of `poverenik_notified_at`. „We have not told them
/// yet“ and „we decided not to tell them“ are different facts and st. 7 asks the
/// Poverenik to be able to tell them apart.
///
/// The codes MUST stay identical to the `notify_decision` CHECK list in v18.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotifyDecision {
    Obavestiti,
    NeObavestiti,
}

impl NotifyDecision {
    pub const ALL: [Self; 2] = [Self::Obavestiti, Self::NeObavestiti];

    pub fn as_code(self) -> &'static str {
        match self {
            Self::Obavestiti => "obavestiti",
            Self::NeObavestiti => "ne_obavestiti",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.as_code() == code)
    }
}

/// The three čl. 53 st. 3 exceptions, closed. If the affected individuals were
/// not told about a high-risk breach, the record must say **which** of the three
/// the rukovalac relied on — „we decided not to“ is not one of them.
///
/// The codes MUST stay identical to the `cl53_izuzetak` CHECK list in v18.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cl53Izuzetak {
    /// St. 3 t. 1 — protective measures were already applied to the data that
    /// was breached (encryption being the statute's own example).
    PrimenjeneMereZastite,
    /// St. 3 t. 2 — measures taken afterwards mean the high risk can no longer
    /// materialise.
    NaknadneMere,
    /// St. 3 t. 3 — individual notification would take a disproportionate
    /// amount of time and resources, so a public notice is given instead.
    NesrazmeranUtrosakVremenaISredstava,
}

impl Cl53Izuzetak {
    pub const ALL: [Self; 3] = [
        Self::PrimenjeneMereZastite,
        Self::NaknadneMere,
        Self::NesrazmeranUtrosakVremenaISredstava,
    ];

    pub fn as_code(self) -> &'static str {
        match self {
            Self::PrimenjeneMereZastite => "primenjene_mere_zastite",
            Self::NaknadneMere => "naknadne_mere",
            Self::NesrazmeranUtrosakVremenaISredstava => "nesrazmeran_utrosak_vremena_i_sredstava",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|value| value.as_code() == code)
    }

    /// Each exception as čl. 53 st. 3 states it, with its tačka — the obrazac
    /// has to say which one was relied on, and a code says nothing to a reader.
    pub fn label(self) -> &'static str {
        match self {
            Self::PrimenjeneMereZastite => {
                "primenjene su mere zaštite zbog kojih su podaci nerazumljivi neovlašćenim \
                 licima (čl. 53 st. 3 t. 1)"
            }
            Self::NaknadneMere => {
                "naknadno su preduzete mere kojima je otklonjen visok rizik (čl. 53 st. 3 t. 2)"
            }
            Self::NesrazmeranUtrosakVremenaISredstava => {
                "obaveštavanje svakog lica zahtevalo bi nesrazmeran utrošak vremena i sredstava, \
                 pa je dato javno obaveštenje (čl. 53 st. 3 t. 3)"
            }
        }
    }
}

/// What the operator submits — the whole record, on both the opening write and
/// every later correction.
///
/// `saznanje_at` is present on the update path too, so that a client which
/// round-trips the record cannot silently drop it; [`update_breach`] compares it
/// against the stored instant and refuses a change rather than ignoring one.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BreachDraft {
    /// The 72 h anchor — čl. 52 st. 1 says *„od saznanja“*, not from the
    /// incident. Immutable once written.
    pub saznanje_at: String,
    /// When the breach happened, if it is known. A separate fact from below.
    pub occurred_at: Option<String>,
    /// When it was discovered. Discovery by a technician is not the same event
    /// as the rukovalac's saznanje, which is why all three columns exist.
    pub discovered_at: Option<String>,
    /// §6 R-3, req. 47: when an obrađivač is in the chain, čl. 52 st. 3 gives two
    /// candidate anchors. Both are recorded and neither is silently chosen.
    pub obradjivac_saznanje_at: Option<String>,
    pub rukovalac_obavesten_at: Option<String>,
    /// Čl. 52 st. 6 — činjenice o povredi.
    pub opis: String,
    /// Čl. 52 st. 6 — njene posledice.
    pub posledice: String,
    /// Čl. 52 st. 6 — preduzete mere za njihovo otklanjanje.
    pub mere: String,
    /// Obrazac section 2 point (3): a COUNT of the individuals concerned. The
    /// prescribed form asks for a number and never for a roster.
    pub broj_lica: Option<i64>,
    pub kategorije_podataka: Option<String>,
    pub risk_outcome: Option<RiskOutcome>,
    pub notify_decision: Option<NotifyDecision>,
    pub notify_obrazlozenje: Option<String>,
    pub poverenik_notified_at: Option<String>,
    /// Čl. 52 st. 2 — mandatory once the 72 h have run without the Poverenik
    /// being told.
    pub delay_reason: Option<String>,
    pub lica_obavestena: Option<bool>,
    pub lica_obavestena_at: Option<String>,
    pub cl53_izuzetak: Option<Cl53Izuzetak>,
    pub cl53_izuzetak_obrazlozenje: Option<String>,
}

/// One stored breach as the operator reads it: every column, plus the four
/// answers that are computed from `now` and are therefore never stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Breach {
    pub id: i64,
    pub saznanje_at: String,
    pub occurred_at: Option<String>,
    pub discovered_at: Option<String>,
    pub obradjivac_saznanje_at: Option<String>,
    pub rukovalac_obavesten_at: Option<String>,
    pub opis: String,
    pub posledice: String,
    pub mere: String,
    pub broj_lica: Option<i64>,
    pub kategorije_podataka: Option<String>,
    pub risk_outcome: Option<RiskOutcome>,
    pub notify_decision: Option<NotifyDecision>,
    pub notify_obrazlozenje: Option<String>,
    pub poverenik_notified_at: Option<String>,
    pub delay_reason: Option<String>,
    pub lica_obavestena: Option<bool>,
    pub lica_obavestena_at: Option<String>,
    pub cl53_izuzetak: Option<Cl53Izuzetak>,
    pub cl53_izuzetak_obrazlozenje: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// Saznanje + 72 h (Pravilnik 40/2019 čl. 3). The countdown's end.
    pub rok_obavestavanja_istice_at: String,
    /// Čl. 52 st. 1 — derived from the risk assessment, `None` until one exists.
    /// **The record persists either way**; this flag never decided whether it
    /// was written (req. 43).
    pub notifiable: Option<bool>,
    /// Čl. 52 st. 2 — whether a delay justification is owed as of `now`.
    pub delay_reason_required: bool,
    /// Čl. 53 st. 1 — whether the affected individuals must be told.
    pub obavestavanje_lica_obavezno: bool,
}

#[tauri::command]
pub fn breaches_list(state: State<'_, AppState>) -> Result<Vec<Breach>, CommandError> {
    list_breaches(state.inner(), &utc_now()?).map_err(Into::into)
}

#[tauri::command]
pub fn breaches_record(
    state: State<'_, AppState>,
    draft: BreachDraft,
) -> Result<Breach, CommandError> {
    record_breach(state.inner(), draft, &utc_now()?).map_err(Into::into)
}

#[tauri::command]
pub fn breaches_update(
    state: State<'_, AppState>,
    id: i64,
    draft: BreachDraft,
) -> Result<Breach, CommandError> {
    update_breach(state.inner(), id, draft, &utc_now()?).map_err(Into::into)
}

/// Read-only and deliberately not admin-gated: the panel has to be able to state
/// the exposure before anything is recorded.
#[tauri::command]
pub fn breaches_notice(state: State<'_, AppState>) -> Result<LegalNotice, CommandError> {
    notice(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn breaches_export_obrazac(
    state: State<'_, AppState>,
    id: i64,
) -> Result<ExportedFile, CommandError> {
    export_obrazac(state.inner(), id, &utc_now()?).map_err(Into::into)
}

/// Every recorded breach, newest saznanje first.
///
/// Admin-gated (req. 48): the breach log is the shop's own compliance file, it
/// is excluded from routine exports and support bundles, and in a three-employee
/// shop the incident description is very often about a colleague.
///
/// Reading is deliberately NOT logged, for the same reason reading the audit log
/// is not: a panel that refreshes would write the shop an evidencija of nothing
/// but its own refreshes, and čl. 48 is the prudential control whose only value
/// is that someone can still read it. The event worth recording is the
/// disclosure that leaves the till — the Poverenik obrazac — and that is where
/// the čl. 48 st. 2 otkrivanje line belongs.
pub fn list_breaches(state: &AppState, now: &str) -> Result<Vec<Breach>, AppError> {
    require_admin(state)?;

    let connection = state.db().open()?;
    let mut statement = connection.prepare(&format!(
        "SELECT {BREACH_COLUMNS} FROM data_breaches ORDER BY saznanje_at DESC, id DESC"
    ))?;
    let stored = statement
        .query_map([], read_breach)?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    stored
        .into_iter()
        .map(|breach| with_derived(breach, now))
        .collect()
}

/// Opens the record for one breach. It always succeeds on a well-formed draft —
/// there is no notifiability gate in front of it (req. 43), and nothing here
/// reads the risk assessment to decide whether the row is worth keeping.
///
/// The record and its čl. 48 st. 2 line share one transaction, so a logged
/// breach is a breach that was recorded.
pub fn record_breach(state: &AppState, draft: BreachDraft, now: &str) -> Result<Breach, AppError> {
    let acting = require_admin(state)?;
    let draft = &validate(draft, now)?;

    let risk = draft.risk_outcome.map(RiskOutcome::as_code);
    let decision = draft.notify_decision.map(NotifyDecision::as_code);
    let izuzetak = draft.cl53_izuzetak.map(Cl53Izuzetak::as_code);
    let obavestena = draft.lica_obavestena.map(i64::from);

    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;

    tx.execute(
        "INSERT INTO data_breaches
             (saznanje_at, occurred_at, discovered_at, obradjivac_saznanje_at,
              rukovalac_obavesten_at, opis, posledice, mere, broj_lica, kategorije_podataka,
              risk_outcome, notify_decision, notify_obrazlozenje, poverenik_notified_at,
              delay_reason, lica_obavestena, lica_obavestena_at, cl53_izuzetak,
              cl53_izuzetak_obrazlozenje, created_by, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                 ?17, ?18, ?19, ?20, ?21, ?21)",
        params![
            draft.saznanje_at,
            draft.occurred_at,
            draft.discovered_at,
            draft.obradjivac_saznanje_at,
            draft.rukovalac_obavesten_at,
            draft.opis,
            draft.posledice,
            draft.mere,
            draft.broj_lica,
            draft.kategorije_podataka,
            risk,
            decision,
            draft.notify_obrazlozenje,
            draft.poverenik_notified_at,
            draft.delay_reason,
            obavestena,
            draft.lica_obavestena_at,
            izuzetak,
            draft.cl53_izuzetak_obrazlozenje,
            acting.id,
            now,
        ],
    )?;
    let breach_id = tx.last_insert_rowid();

    append_audit_event(
        &tx,
        &audit_line(AuditAction::Unos, acting.id, breach_id, now),
    )?;

    let stored = load_breach(&tx, breach_id)?;
    tx.commit()?;
    with_derived(stored, now)
}

/// Corrects an open record as the investigation proceeds — every column except
/// the clock anchor.
///
/// The anchor is checked **before** the rest of the draft is validated, so that
/// an attempt to move it is reported as what it is rather than as whatever
/// downstream rule the moved deadline happens to trip. Its column is then simply
/// absent from the `SET` list: v18's trigger is declared `UPDATE OF saznanje_at`
/// and never even runs, which is the difference between a guard and a race.
pub fn update_breach(
    state: &AppState,
    id: i64,
    draft: BreachDraft,
    now: &str,
) -> Result<Breach, AppError> {
    let acting = require_admin(state)?;

    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;

    let existing = load_breach(&tx, id)?;

    // Parsed instants, never strings: RFC3339 renders the same moment with and
    // without a fractional part, and refusing an equivalent re-serialisation
    // would make the record uneditable by any client that round-trips it.
    let submitted = parse_rfc3339(&draft.saznanje_at, "saznanjeAt")?;
    let anchored = parse_rfc3339(&existing.saznanje_at, "saznanjeAt")?;
    if submitted != anchored {
        return Err(AppError::business(
            "povreda_saznanje_nepromenljivo",
            "Vreme saznanja za povredu je nepromenljivo — od njega teče rok od 72 časa \
             (ZZPL čl. 52 st. 1). Ako je uneto pogrešno, evidentirajte novu povredu.",
        ));
    }

    let draft = &validate(draft, now)?;

    let risk = draft.risk_outcome.map(RiskOutcome::as_code);
    let decision = draft.notify_decision.map(NotifyDecision::as_code);
    let izuzetak = draft.cl53_izuzetak.map(Cl53Izuzetak::as_code);
    let obavestena = draft.lica_obavestena.map(i64::from);

    tx.execute(
        "UPDATE data_breaches
            SET occurred_at = ?1, discovered_at = ?2, obradjivac_saznanje_at = ?3,
                rukovalac_obavesten_at = ?4, opis = ?5, posledice = ?6, mere = ?7,
                broj_lica = ?8, kategorije_podataka = ?9, risk_outcome = ?10,
                notify_decision = ?11, notify_obrazlozenje = ?12, poverenik_notified_at = ?13,
                delay_reason = ?14, lica_obavestena = ?15, lica_obavestena_at = ?16,
                cl53_izuzetak = ?17, cl53_izuzetak_obrazlozenje = ?18, updated_at = ?19
          WHERE id = ?20",
        params![
            draft.occurred_at,
            draft.discovered_at,
            draft.obradjivac_saznanje_at,
            draft.rukovalac_obavesten_at,
            draft.opis,
            draft.posledice,
            draft.mere,
            draft.broj_lica,
            draft.kategorije_podataka,
            risk,
            decision,
            draft.notify_obrazlozenje,
            draft.poverenik_notified_at,
            draft.delay_reason,
            obavestena,
            draft.lica_obavestena_at,
            izuzetak,
            draft.cl53_izuzetak_obrazlozenje,
            now,
            id,
        ],
    )?;

    append_audit_event(&tx, &audit_line(AuditAction::Menjanje, acting.id, id, now))?;

    let stored = load_breach(&tx, id)?;
    tx.commit()?;
    with_derived(stored, now)
}

/// The čl. 52 exposure, resolved against the shop's stored legal form.
///
/// The figure comes from `legal.rs` and from nowhere else, and it is the
/// **notification** tier — req. 50 forbids attaching any amount to a pure
/// documentation failure under st. 6.
pub fn notice(state: &AppState) -> Result<LegalNotice, AppError> {
    let profile = crate::commands::settings::load_shop_profile(state)?;
    Ok(crate::legal::breach_notification_missing(&profile))
}

// ---------------------------------------------------------------------------
// The Pravilnik 40/2019 obrazac (req. 45)
// ---------------------------------------------------------------------------

/// Renders the prescribed obaveštenje for one record and writes it to
/// `exports/`, ready to print, sign and file.
///
/// **There is no submission API and this must not grow one.** Pravilnik čl. 5
/// is the whole filing route — *u pisanom obliku, neposredno ili putem pošte*,
/// with a scanned copy to the Poverenik's mailbox as the alternative — so the
/// deliverable is a document and the sending is the operator's act. The route
/// is printed on the form itself, because a form that does not say how it is
/// filed invites the assumption that the app filed it.
///
/// This is also the one place in this module that writes a čl. 48 st. 2
/// **otkrivanje** line: reading the log is not a disclosure, and rendering the
/// obrazac is — it is the copy that leaves the till, and the primalac is named.
/// The line is written after the file exists, because an export that failed
/// disclosed nothing.
pub fn export_obrazac(state: &AppState, id: i64, now: &str) -> Result<ExportedFile, AppError> {
    require_admin(state)?;

    let stored = {
        let connection = state.db().open()?;
        load_breach(&connection, id)?
    };
    let breach = with_derived(stored, now)?;
    let company = crate::commands::settings::load_company_settings(state)?;

    let exported = super::campaigns::write_export(
        state,
        &format!("obrazac-povreda-podataka-{id}.html"),
        &render_obrazac_html(&company, &breach),
        1,
    )?;

    record_audit(
        state,
        AuditEntry {
            action: AuditAction::Otkrivanje,
            object_type: AuditObjectType::DataBreach,
            object_id: id.to_string(),
            reason_code: Some(AuditReason::BezbednosniIncident),
            recipient: Some(AuditRecipient::Poverenik),
            support_session_id: None,
        },
        now,
    )?;

    Ok(exported)
}

/// The obrazac annexed to Pravilnik 40/2019, verbatim: five numbered sections in
/// the prescribed order, the sub-fields of sections 1 and 2 with the pravilnik's
/// own labels, then the „Prilog:“ slot and the mesto/datum + Ime i prezime +
/// Potpis block it ends with.
///
/// **Every value on it is a value the shop holds.** Section 2 (4) asks for the
/// number of *podaci* whose security was breached and this record holds a number
/// of *lica* — a different count — so that slot stays empty for the operator
/// rather than being answered with the wrong number; the same goes for section 1
/// (3), which asks for a lice za zaštitu podataka this shop is not required to
/// appoint (čl. 56 st. 2) and has not configured. The place, the date and the
/// signer's name are left blank too: they are written when the form is signed,
/// and an unsigned form must not assert a signing date that has not happened or
/// a signatory who has not agreed to be one.
///
/// The countdown cites **Pravilnik čl. 3**, which restates the čl. 52 st. 1
/// deadline as a flat 72 h od saznanja without the statute's *„bez nepotrebnog
/// odlaganja“* softener — and the softener is deliberately nowhere on the page,
/// because printed beside a deadline it reads as permission to miss it.
pub fn render_obrazac_html(company: &CompanySettings, breach: &Breach) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"sr-Latn\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<title>Obaveštenje o povredi podataka o ličnosti</title>\n");
    html.push_str(
        "<style>\n\
         @page { margin: 1.2cm }\n\
         body { font-family: sans-serif; color: #111; margin: 1.2cm; line-height: 1.45; }\n\
         h1 { font-size: 1.3rem; text-align: center; }\n\
         h2 { font-size: 1rem; margin: 1.2rem 0 0.4rem; }\n\
         .meta { color: #444; font-size: 0.9rem; }\n\
         .rok { border: 1px solid #999; padding: 0.4rem 0.6rem; }\n\
         table { border-collapse: collapse; width: 100%; }\n\
         th, td { border: 1px solid #999; padding: 0.3rem 0.5rem; text-align: left; \
         vertical-align: top; }\n\
         th { width: 40%; font-weight: normal; }\n\
         .unos { border: 1px solid #999; padding: 0.5rem; min-height: 3rem; }\n\
         .potpis { margin-top: 2.5rem; text-align: right; }\n\
         .potpis p { margin: 0.35rem 0; }\n\
         </style>\n</head>\n<body>\n",
    );

    html.push_str("<h1>OBAVEŠTENJE O POVREDI PODATAKA O LIČNOSTI</h1>\n");
    html.push_str(
        "<p class=\"meta\">Obrazac iz člana 2 Pravilnika o obrascu obaveštenja o povredi \
         podataka o ličnosti („Službeni glasnik RS“, broj 40/2019).</p>\n",
    );

    let istekao = if breach.delay_reason_required {
        " Rok je istekao — uz obaveštenje se navode razlozi zbog kojih nije postupljeno u tom \
         roku (ZZPL čl. 52 st. 2)."
    } else {
        ""
    };
    html.push_str(&format!(
        "<p class=\"rok\"><strong>Rok:</strong> obaveštenje se dostavlja Povereniku u roku od \
         72 časa od saznanja za povredu (Pravilnik 40/2019 čl. 3). Saznanje: {}. Rok ističe: \
         {}.{istekao}</p>\n",
        escape_html(&instant_for_print(&breach.saznanje_at)),
        escape_html(&instant_for_print(&breach.rok_obavestavanja_istice_at)),
    ));
    html.push_str(
        "<p class=\"meta\"><strong>Način dostavljanja (Pravilnik 40/2019 čl. 5):</strong> \
         u pisanom obliku, neposredno ili putem pošte; skenirani primerak može da se dostavi \
         na povredapodataka@poverenik.rs. Program ne dostavlja obaveštenje — obrazac se \
         štampa, potpisuje i dostavlja.</p>\n",
    );

    // 1) Podaci o rukovaocu — three sub-fields.
    html.push_str("<h2>1) Podaci o rukovaocu</h2>\n<table>\n");
    html.push_str(&obrazac_row(
        "(1) naziv rukovaoca",
        Some(company.shop_name.as_str()),
    ));
    html.push_str(&obrazac_row(
        "(2) adresa/sedište",
        Some(company.address.as_str()),
    ));
    html.push_str(&obrazac_row(
        "(3) ime i kontakt podaci lica za zaštitu podataka o ličnosti rukovaoca ili informacije \
         o drugom načinu na koji se mogu dobiti podaci o povredi",
        None,
    ));
    html.push_str("</table>\n");

    // 2) Podaci o povredi podataka — five sub-fields.
    html.push_str("<h2>2) Podaci o povredi podataka</h2>\n<table>\n");
    html.push_str(&obrazac_row(
        "(1) opis prirode povrede podataka, uključujući okolnosti koje se odnose na povredu",
        Some(breach.opis.as_str()),
    ));
    html.push_str(&obrazac_row(
        "(2) vrsta podataka o ličnosti",
        breach.kategorije_podataka.as_deref(),
    ));
    let broj_lica = breach.broj_lica.map(|broj| broj.to_string());
    html.push_str(&obrazac_row(
        "(3) broj lica na koja se podaci odnose",
        broj_lica.as_deref(),
    ));
    html.push_str(&obrazac_row(
        "(4) broj podataka o ličnosti čija je bezbednost povređena",
        None,
    ));
    let povreda_at = breach.occurred_at.as_deref().map(instant_for_print);
    html.push_str(&obrazac_row(
        "(5) datum i vreme povrede bezbednosti podataka (ukoliko je poznat, ili prema proceni)",
        povreda_at.as_deref(),
    ));
    html.push_str("</table>\n");

    // 3) and 4) — the other two čl. 52 st. 6 elements, as free text.
    html.push_str("<h2>3) Opis mogućih posledica povrede</h2>\n");
    html.push_str(&format!(
        "<p class=\"unos\">{}</p>\n",
        escape_html(&breach.posledice)
    ));
    html.push_str(
        "<h2>4) Opis mera koje je rukovalac preduzeo ili čije je preduzimanje predloženo</h2>\n",
    );
    html.push_str(&format!(
        "<p class=\"unos\">{}</p>\n",
        escape_html(&breach.mere)
    ));

    // 5) Ostali podaci — everything else the record holds that bears on this
    // notification. A fact the record does not hold raises no row: an empty
    // labelled slot reads as an unanswered question, and these are not questions
    // the obrazac asks.
    html.push_str("<h2>5) Ostali podaci od značaja za obaveštavanje o povredi podataka</h2>\n");
    html.push_str("<table>\n");
    html.push_str(&obrazac_row(
        "Datum i vreme saznanja za povredu (ZZPL čl. 52 st. 1)",
        Some(instant_for_print(&breach.saznanje_at).as_str()),
    ));
    html.push_str(&obrazac_row(
        "Rok za obaveštavanje Poverenika (Pravilnik 40/2019 čl. 3)",
        Some(instant_for_print(&breach.rok_obavestavanja_istice_at).as_str()),
    ));
    for (label, value) in [
        ("Datum otkrivanja povrede", breach.discovered_at.as_deref()),
        (
            "Obrađivač je saznao za povredu (ZZPL čl. 52 st. 3)",
            breach.obradjivac_saznanje_at.as_deref(),
        ),
        (
            "Rukovalac obavešten od strane obrađivača (ZZPL čl. 52 st. 3)",
            breach.rukovalac_obavesten_at.as_deref(),
        ),
    ] {
        if let Some(value) = value {
            html.push_str(&obrazac_row(label, Some(instant_for_print(value).as_str())));
        }
    }
    if let Some(outcome) = breach.risk_outcome {
        html.push_str(&obrazac_row(
            "Procena rizika (ZZPL čl. 52 st. 1 i čl. 53 st. 1)",
            Some(outcome.label()),
        ));
    }
    if let Some(notified) = breach.poverenik_notified_at.as_deref() {
        html.push_str(&obrazac_row(
            "Obaveštenje dostavljeno Povereniku",
            Some(instant_for_print(notified).as_str()),
        ));
    }
    if breach.delay_reason_required || breach.delay_reason.is_some() {
        html.push_str(&obrazac_row(
            "Razlozi zbog kojih obaveštenje nije dostavljeno u roku (ZZPL čl. 52 st. 2)",
            breach.delay_reason.as_deref(),
        ));
    }
    if let Some(obavestena) = breach.lica_obavestena {
        let value = if obavestena {
            match breach.lica_obavestena_at.as_deref() {
                Some(at) => format!("da, dana {}", instant_for_print(at)),
                None => "da".to_string(),
            }
        } else {
            "ne".to_string()
        };
        html.push_str(&obrazac_row(
            "Lica na koja se podaci odnose obaveštena (ZZPL čl. 53 st. 1)",
            Some(value.as_str()),
        ));
    }
    if let Some(izuzetak) = breach.cl53_izuzetak {
        html.push_str(&obrazac_row(
            "Izuzetak od obaveštavanja lica (ZZPL čl. 53 st. 3)",
            Some(izuzetak.label()),
        ));
    }
    if let Some(obrazlozenje) = breach.cl53_izuzetak_obrazlozenje.as_deref() {
        html.push_str(&obrazac_row(
            "Obrazloženje izuzetka (ZZPL čl. 53 st. 3)",
            Some(obrazlozenje),
        ));
    }
    html.push_str("</table>\n");

    // Pravilnik čl. 4 st. 1 makes the čl. 47 evidencija a mandatory attachment,
    // so the Prilog slot names it rather than leaving the operator to guess.
    html.push_str(
        "<p><strong>Prilog:</strong> evidencija radnji obrade koja se odnosi na podatke koji su \
         bili predmet povrede, a koju rukovalac vodi u skladu sa članom 47 Zakona \
         (Pravilnik 40/2019 čl. 4 st. 1).</p>\n",
    );

    html.push_str("<div class=\"potpis\">\n");
    html.push_str("<p>U ______________________________, dana ______________________. godine</p>\n");
    html.push_str("<p>Za RUKOVAOCA</p>\n");
    html.push_str("<p>______________________________</p>\n");
    html.push_str("<p>Ime i prezime</p>\n");
    html.push_str("<p>______________________________</p>\n");
    html.push_str("<p>Potpis</p>\n");
    html.push_str("</div>\n");

    html.push_str("</body>\n</html>\n");
    html
}

/// One prescribed field and its answer. `None` prints an empty cell — a slot the
/// operator fills in by hand, never a value this app invented for it.
fn obrazac_row(label: &str, value: Option<&str>) -> String {
    format!(
        "<tr><th>{}</th><td>{}</td></tr>\n",
        escape_html(label),
        value.map(escape_html).unwrap_or_default()
    )
}

/// `01.08.2026. 09:00 (UTC)` — the written Serbian form, resolved to UTC so that
/// an instant recorded with an offset and the same instant recorded as `Z` read
/// identically on paper. The zone is named rather than assumed.
///
/// Anything unparseable prints as stored: every column this renderer reads was
/// parsed on the way in, and swallowing a value on a form filed with a regulator
/// would be worse than showing it raw.
fn instant_for_print(value: &str) -> String {
    let Ok(parsed) = OffsetDateTime::parse(value.trim(), &Rfc3339) else {
        return value.trim().to_string();
    };
    let utc = parsed.to_offset(UtcOffset::UTC);
    format!(
        "{:02}.{:02}.{:04}. {:02}:{:02} (UTC)",
        utc.day(),
        u8::from(utc.month()),
        utc.year(),
        utc.hour(),
        utc.minute()
    )
}

/// Escapes the five HTML-significant characters, so an incident description that
/// contains `<` or `&` can never break out of the document.
fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Validation — čl. 52 st. 2, čl. 52 st. 7 and čl. 53 st. 3
// ---------------------------------------------------------------------------

/// Returns the draft trimmed, with blank optional fields collapsed to `NULL`
/// and every timestamp column parsed, or the first čl. 52 / čl. 53 rule it
/// breaks.
fn validate(draft: BreachDraft, now: &str) -> Result<BreachDraft, AppError> {
    let saznanje = parse_rfc3339(&draft.saznanje_at, "saznanjeAt")?;

    let draft = BreachDraft {
        saznanje_at: draft.saznanje_at.trim().to_string(),
        occurred_at: optional_instant(draft.occurred_at, "occurredAt")?,
        discovered_at: optional_instant(draft.discovered_at, "discoveredAt")?,
        obradjivac_saznanje_at: optional_instant(
            draft.obradjivac_saznanje_at,
            "obradjivacSaznanjeAt",
        )?,
        rukovalac_obavesten_at: optional_instant(
            draft.rukovalac_obavesten_at,
            "rukovalacObavestenAt",
        )?,
        opis: required_text(&draft.opis, "opis", "činjenice o povredi")?,
        posledice: required_text(&draft.posledice, "posledice", "posledice povrede")?,
        mere: required_text(&draft.mere, "mere", "preduzete mere za otklanjanje")?,
        broj_lica: draft.broj_lica,
        kategorije_podataka: optional_text(
            draft.kategorije_podataka,
            "kategorijePodataka",
            MAX_OBRAZLOZENJE_ZNAKOVA,
        )?,
        risk_outcome: draft.risk_outcome,
        notify_decision: draft.notify_decision,
        notify_obrazlozenje: optional_text(
            draft.notify_obrazlozenje,
            "notifyObrazlozenje",
            MAX_OBRAZLOZENJE_ZNAKOVA,
        )?,
        poverenik_notified_at: optional_instant(
            draft.poverenik_notified_at,
            "poverenikNotifiedAt",
        )?,
        delay_reason: optional_text(draft.delay_reason, "delayReason", MAX_OPIS_ZNAKOVA)?,
        lica_obavestena: draft.lica_obavestena,
        lica_obavestena_at: optional_instant(draft.lica_obavestena_at, "licaObavestenaAt")?,
        cl53_izuzetak: draft.cl53_izuzetak,
        cl53_izuzetak_obrazlozenje: optional_text(
            draft.cl53_izuzetak_obrazlozenje,
            "cl53IzuzetakObrazlozenje",
            MAX_OBRAZLOZENJE_ZNAKOVA,
        )?,
    };

    if draft.broj_lica.is_some_and(|broj| broj < 0) {
        return Err(AppError::validation(
            "Broj lica na koja se podaci odnose ne može da bude negativan.",
            serde_json::json!({ "field": "brojLica" }),
        ));
    }

    // Čl. 52 st. 7: the Poverenik must be able to establish compliance with the
    // whole article, and a bare „ne obaveštavamo“ establishes nothing. Whichever
    // way the decision went, the record has to say why it went that way.
    if draft.notify_decision.is_some() && draft.notify_obrazlozenje.is_none() {
        return Err(AppError::validation(
            "Uz odluku o obaveštavanju Poverenika mora da stoji obrazloženje — \
             dokumentacija mora da omogući Povereniku da utvrdi da li je postupljeno \
             u skladu sa članom 52 (ZZPL čl. 52 st. 7).",
            serde_json::json!({ "field": "notifyObrazlozenje" }),
        ));
    }

    let delay_reason_required = delay_reason_required(
        saznanje,
        draft.risk_outcome,
        draft.notify_decision,
        draft.poverenik_notified_at.as_deref(),
        now,
    )?;
    if delay_reason_required && draft.delay_reason.is_none() {
        return Err(AppError::validation(
            format!(
                "Rok od {ROK_OBAVESTAVANJA_SATI} časa od saznanja za povredu je istekao, \
                 a Poverenik nije obavešten — razlozi zbog kojih nije postupljeno u tom \
                 roku moraju da budu obrazloženi (ZZPL čl. 52 st. 2)."
            ),
            serde_json::json!({ "field": "delayReason" }),
        ));
    }

    // The čl. 53 block. St. 1 attaches at visok rizik; st. 3 gives three
    // exceptions and only three.
    match draft.lica_obavestena {
        Some(true) => {
            if draft.lica_obavestena_at.is_none() {
                return Err(AppError::validation(
                    "Unesite kada su lica obaveštena o povredi (ZZPL čl. 53 st. 1).",
                    serde_json::json!({ "field": "licaObavestenaAt" }),
                ));
            }
        }
        Some(false) if draft.risk_outcome == Some(RiskOutcome::VisokRizik) => {
            if draft.cl53_izuzetak.is_none() {
                return Err(AppError::validation(
                    "Povreda je procenjena kao visok rizik, a lica nisu obaveštena — \
                     mora da se navede na koji se izuzetak rukovalac poziva \
                     (ZZPL čl. 53 st. 3).",
                    serde_json::json!({ "field": "cl53Izuzetak" }),
                ));
            }
            if draft.cl53_izuzetak_obrazlozenje.is_none() {
                return Err(AppError::validation(
                    "Uz izuzetak od obaveštavanja lica mora da stoji obrazloženje \
                     (ZZPL čl. 53 st. 3 i čl. 52 st. 7).",
                    serde_json::json!({ "field": "cl53IzuzetakObrazlozenje" }),
                ));
            }
        }
        _ => {}
    }

    Ok(draft)
}

/// Čl. 52 st. 2 in one predicate: *„Ako rukovalac ne postupi u roku od 72 časa
/// od saznanja za povredu, dužan je da obrazloži razloge…“*
///
/// St. 2 asks why the **st. 1** deadline was missed, so it can only bind where
/// st. 1 bound. It takes BOTH halves of that to exempt a record: an assessment
/// finding that the breach cannot produce a risk (so st. 1 never attached) AND
/// the recorded — and, above, reasoned — decision not to notify. The decision
/// alone is not a discharge; *„odlučili smo da ne obavestimo“* against an
/// assessed rizik is the very state st. 2 exists to make the shop explain, and
/// letting it out would produce a record that tells the Poverenik `notifiable`
/// and „no justification owed“ in the same breath.
///
/// Every other state does owe the explanation once the window is spent,
/// including the state where nothing has been assessed at all: an undecided
/// breach past 72 h is precisely *„nije postupio u tom roku“*.
///
/// The boundary is inclusive of the deadline — at saznanje + 72 h exactly the
/// window is spent — and instants are compared parsed, never as strings.
fn delay_reason_required(
    saznanje: OffsetDateTime,
    risk_outcome: Option<RiskOutcome>,
    notify_decision: Option<NotifyDecision>,
    poverenik_notified_at: Option<&str>,
    now: &str,
) -> Result<bool, AppError> {
    if notify_decision == Some(NotifyDecision::NeObavestiti)
        && risk_outcome == Some(RiskOutcome::BezRizika)
    {
        return Ok(false);
    }

    let deadline = saznanje + Duration::hours(ROK_OBAVESTAVANJA_SATI);

    if let Some(notified) = poverenik_notified_at {
        if parse_rfc3339(notified, "poverenikNotifiedAt")? < deadline {
            return Ok(false);
        }
    }

    Ok(parse_rfc3339(now, "now")? >= deadline)
}

fn required_text(value: &str, field: &str, what: &str) -> Result<String, AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(AppError::validation(
            format!("Unesite {what} — to je jedan od tri elementa koje traži ZZPL čl. 52 st. 6."),
            serde_json::json!({ "field": field }),
        ));
    }
    if trimmed.chars().count() > MAX_OPIS_ZNAKOVA {
        return Err(AppError::validation(
            format!("Polje može da ima najviše {MAX_OPIS_ZNAKOVA} znakova."),
            serde_json::json!({ "field": field }),
        ));
    }
    Ok(trimmed.to_string())
}

/// Trims, collapses a blank to `NULL`, and bounds the length. A blank string and
/// an absent value are the same fact and this table stores one of them.
fn optional_text(
    value: Option<String>,
    field: &str,
    max: usize,
) -> Result<Option<String>, AppError> {
    let Some(value) = value else { return Ok(None) };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.chars().count() > max {
        return Err(AppError::validation(
            format!("Polje može da ima najviše {max} znakova."),
            serde_json::json!({ "field": field }),
        ));
    }
    Ok(Some(trimmed.to_string()))
}

/// The same, for the timestamp columns — and it parses what it keeps, so a
/// column this record reasons about can never hold something unparseable.
fn optional_instant(value: Option<String>, field: &str) -> Result<Option<String>, AppError> {
    let Some(value) = optional_text(value, field, MAX_OBRAZLOZENJE_ZNAKOVA)? else {
        return Ok(None);
    };
    parse_rfc3339(&value, field)?;
    Ok(Some(value))
}

fn parse_rfc3339(value: &str, field: &str) -> Result<OffsetDateTime, AppError> {
    OffsetDateTime::parse(value.trim(), &Rfc3339).map_err(|source| {
        AppError::validation(
            format!("Vreme nije ispravno: {source}"),
            serde_json::json!({ "field": field }),
        )
    })
}

// ---------------------------------------------------------------------------
// Storage
// ---------------------------------------------------------------------------

const BREACH_COLUMNS: &str = "id, saznanje_at, occurred_at, discovered_at, obradjivac_saznanje_at,
     rukovalac_obavesten_at, opis, posledice, mere, broj_lica, kategorije_podataka, risk_outcome,
     notify_decision, notify_obrazlozenje, poverenik_notified_at, delay_reason, lica_obavestena,
     lica_obavestena_at, cl53_izuzetak, cl53_izuzetak_obrazlozenje, created_at, updated_at";

/// Reads one row. The three derived answers depend on `now`, which this layer
/// does not have, so they are filled in by [`with_derived`] and never stored.
///
/// The closed vocabularies are resolved through `from_code`. v18's CHECK
/// constraints hold the same three lists, and
/// `every_code_round_trips_and_is_accepted_by_the_v18_checks` writes every
/// variant through this reader, so a value that fails to resolve here is one no
/// path in this app can put in the table.
fn read_breach(row: &Row<'_>) -> rusqlite::Result<Breach> {
    let risk_outcome: Option<String> = row.get(11)?;
    let notify_decision: Option<String> = row.get(12)?;
    let cl53_izuzetak: Option<String> = row.get(18)?;
    let lica_obavestena: Option<i64> = row.get(16)?;

    Ok(Breach {
        id: row.get(0)?,
        saznanje_at: row.get(1)?,
        occurred_at: row.get(2)?,
        discovered_at: row.get(3)?,
        obradjivac_saznanje_at: row.get(4)?,
        rukovalac_obavesten_at: row.get(5)?,
        opis: row.get(6)?,
        posledice: row.get(7)?,
        mere: row.get(8)?,
        broj_lica: row.get(9)?,
        kategorije_podataka: row.get(10)?,
        risk_outcome: risk_outcome.as_deref().and_then(RiskOutcome::from_code),
        notify_decision: notify_decision
            .as_deref()
            .and_then(NotifyDecision::from_code),
        notify_obrazlozenje: row.get(13)?,
        poverenik_notified_at: row.get(14)?,
        delay_reason: row.get(15)?,
        lica_obavestena: lica_obavestena.map(|value| value != 0),
        lica_obavestena_at: row.get(17)?,
        cl53_izuzetak: cl53_izuzetak.as_deref().and_then(Cl53Izuzetak::from_code),
        cl53_izuzetak_obrazlozenje: row.get(19)?,
        created_at: row.get(20)?,
        updated_at: row.get(21)?,
        // Placeholders — every read goes through `with_derived`.
        rok_obavestavanja_istice_at: String::new(),
        notifiable: None,
        delay_reason_required: false,
        obavestavanje_lica_obavezno: false,
    })
}

/// Fills in the four answers that are computed rather than stored.
///
/// They are computed because they are functions of `now` and of the risk
/// assessment, and a stored copy would be a second source of truth that goes
/// stale the moment the clock moves past the deadline.
fn with_derived(mut breach: Breach, now: &str) -> Result<Breach, AppError> {
    let saznanje = parse_rfc3339(&breach.saznanje_at, "saznanjeAt")?;

    breach.rok_obavestavanja_istice_at = (saznanje + Duration::hours(ROK_OBAVESTAVANJA_SATI))
        .format(&Rfc3339)
        .map_err(|source| AppError::InvalidState(format!("Vreme nije dostupno: {source}")))?;

    // Čl. 52 st. 1 — the risk gate, and the ONLY place it appears. `None` until
    // an assessment exists: unknown is not the same answer as no.
    breach.notifiable = breach
        .risk_outcome
        .map(|outcome| outcome != RiskOutcome::BezRizika);

    breach.delay_reason_required = delay_reason_required(
        saznanje,
        breach.risk_outcome,
        breach.notify_decision,
        breach.poverenik_notified_at.as_deref(),
        now,
    )?;

    // Čl. 53 st. 1 — a higher threshold than st. 1's, and a different addressee.
    breach.obavestavanje_lica_obavezno = breach.risk_outcome == Some(RiskOutcome::VisokRizik);

    Ok(breach)
}

fn load_breach(conn: &Connection, id: i64) -> Result<Breach, AppError> {
    conn.query_row(
        &format!("SELECT {BREACH_COLUMNS} FROM data_breaches WHERE id = ?1"),
        params![id],
        read_breach,
    )
    .optional()?
    .ok_or_else(|| AppError::not_found("Evidencija o povredi nije pronađena."))
}

/// The čl. 48 st. 2 line about a breach record: who, when, what kind of object,
/// which record, and why — with the record itself referenced by its opaque id.
///
/// Not one character of the incident's prose travels with it. Req. 4 bars value
/// payloads, names and free text from `audit_events` absolutely, and in a shop
/// this size the description of a breach is very often a description of a
/// colleague; [`append_audit_event`]'s write boundary would refuse it anyway,
/// but nothing here offers it one.
fn audit_line(action: AuditAction, actor_user_id: i64, breach_id: i64, now: &str) -> AuditDraft {
    AuditDraft {
        at: now.to_string(),
        actor_user_id: Some(actor_user_id),
        action,
        object_type: AuditObjectType::DataBreach,
        object_id: breach_id.to_string(),
        reason_code: Some(AuditReason::BezbednosniIncident),
        recipient: None,
        support_session_id: None,
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::params;

    use super::{
        export_obrazac, list_breaches, record_breach, render_obrazac_html, update_breach, Breach,
        BreachDraft, Cl53Izuzetak, NotifyDecision, RiskOutcome,
    };
    use crate::commands::settings::CompanySettings;
    use crate::db::{test_database_path, Db};
    use crate::state::AppState;

    /// 09:00 on 1 August 2026 — every instant in these tests is an offset from
    /// it, so the 72 h boundary lands on a readable wall-clock time.
    const SAZNANJE: &str = "2026-08-01T09:00:00Z";
    /// Saznanje + 71 h 59 min. One minute inside the Pravilnik čl. 3 window.
    const PRE_ROKA: &str = "2026-08-04T08:59:00Z";
    /// Saznanje + 72 h 00 min exactly. The first instant the window is spent.
    const NA_ROKU: &str = "2026-08-04T09:00:00Z";

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

    fn sign_in_cashier(state: &AppState) -> i64 {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, pin_hash, active,
                                    created_at, updated_at)
                 VALUES ('radnica', 'Milica Milićević', 'cashier', 'pin-hes',
                         1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        let cashier_id = connection.last_insert_rowid();
        state
            .set_session_user_id(cashier_id)
            .expect("cashier session should set");
        cashier_id
    }

    /// The three čl. 52 st. 6 elements and nothing else — the minimum a record
    /// may be opened with, which is the point of req. 43.
    fn minimal_draft() -> BreachDraft {
        BreachDraft {
            saznanje_at: SAZNANJE.to_string(),
            opis: "Nestao je službeni telefon sa aplikacijom za pristup kasi.".to_string(),
            posledice: "Moguć neovlašćen pristup podacima o kupcima iz reklamacija.".to_string(),
            mere: "Naloge na uređaju smo odmah zaključali i lozinke promenili.".to_string(),
            ..BreachDraft::default()
        }
    }

    /// Every textual column of every logged row, concatenated. What is not in
    /// here never reached the evidencija pristupa.
    fn audit_table_text(state: &AppState) -> String {
        let connection = state.db().open().expect("database should open");
        let mut statement = connection
            .prepare(
                "SELECT at, COALESCE(actor_user_id, -1), action, object_type, object_id,
                        COALESCE(reason_code, ''), COALESCE(recipient, ''),
                        COALESCE(support_session_id, -1), prev_hash, hash
                 FROM audit_events ORDER BY id",
            )
            .expect("statement should prepare");
        let rows = statement
            .query_map([], |row| {
                Ok(format!(
                    "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                ))
            })
            .expect("query should run")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("rows should read");
        rows.join("\n")
    }

    fn only_breach(state: &AppState, now: &str) -> Breach {
        let mut breaches = list_breaches(state, now).expect("the log should read");
        assert_eq!(breaches.len(), 1, "exactly one record was opened");
        breaches.remove(0)
    }

    /// Req. 43. Čl. 52 st. 6 covers *„svaku povredu“* with no risk qualifier —
    /// the qualifier lives in st. 1 and governs only notification. So a breach
    /// assessed as carrying no risk at all, with an explicit decision not to
    /// tell the Poverenik, must still leave a complete record behind; the flag
    /// is derived from the assessment and never gated the write.
    #[test]
    fn a_non_notifiable_breach_still_persists() {
        with_state("a_non_notifiable_breach_still_persists", |state| {
            sign_in_admin(state);

            // Opened before any assessment exists at all. The record comes first.
            let opened = record_breach(state, minimal_draft(), SAZNANJE)
                .expect("an unassessed breach must be recordable");
            assert_eq!(
                opened.notifiable, None,
                "with no risk assessment yet the flag is unknown, not false"
            );

            let assessed = update_breach(
                state,
                opened.id,
                BreachDraft {
                    risk_outcome: Some(RiskOutcome::BezRizika),
                    notify_decision: Some(NotifyDecision::NeObavestiti),
                    notify_obrazlozenje: Some(
                        "Telefon je bio šifrovan i daljinski obrisan pre nego što je \
                         iko mogao da mu pristupi."
                            .to_string(),
                    ),
                    ..minimal_draft()
                },
                SAZNANJE,
            )
            .expect("a decision not to notify must be recordable, not a discard");

            assert_eq!(
                assessed.notifiable,
                Some(false),
                "bez rizika — čl. 52 st. 1 does not attach"
            );
            assert_eq!(assessed.notify_decision, Some(NotifyDecision::NeObavestiti));
            assert!(
                !assessed.obavestavanje_lica_obavezno,
                "čl. 53 st. 1 turns on visok rizik"
            );

            let stored = only_breach(state, SAZNANJE);
            assert_eq!(stored.id, opened.id);
            assert_eq!(stored.opis, minimal_draft().opis, "činjenice o povredi");
            assert_eq!(stored.posledice, minimal_draft().posledice, "posledice");
            assert_eq!(stored.mere, minimal_draft().mere, "preduzete mere");
            assert_eq!(
                stored.notifiable,
                Some(false),
                "the record a non-notifiable breach leaves behind is a full one"
            );
        });
    }

    /// Čl. 52 st. 2 anchors to *saznanje*, and Pravilnik 40/2019 čl. 3 restates
    /// the window as a flat 72 h. One minute inside it the justification is
    /// optional; at the boundary itself it is owed.
    ///
    /// The last arm is the exemption that keeps the rule honest: st. 2 asks why
    /// the rukovalac did not act *within the st. 1 deadline*, so a recorded and
    /// reasoned decision that st. 1 never attached has nothing to explain.
    #[test]
    fn the_delay_reason_becomes_mandatory_exactly_at_seventy_two_hours_after_saznanje() {
        with_state(
            "the_delay_reason_becomes_mandatory_exactly_at_seventy_two_hours_after_saznanje",
            |state| {
                sign_in_admin(state);

                let opened =
                    record_breach(state, minimal_draft(), SAZNANJE).expect("record should open");
                assert_eq!(
                    opened.rok_obavestavanja_istice_at, NA_ROKU,
                    "the countdown ends 72 h after saznanje (Pravilnik čl. 3)"
                );
                assert!(!opened.delay_reason_required);

                // 71 h 59 min — inside the window.
                let inside = update_breach(state, opened.id, minimal_draft(), PRE_ROKA)
                    .expect("inside the 72 h window no justification is owed");
                assert!(
                    !inside.delay_reason_required,
                    "at 71 h 59 min the st. 2 duty has not attached"
                );
                assert_eq!(inside.delay_reason, None);

                // 72 h 00 min — the window is spent.
                let error = update_breach(state, opened.id, minimal_draft(), NA_ROKU)
                    .expect_err("at 72 h exactly the st. 2 justification is mandatory");
                assert_eq!(error.code(), "validation_error");
                assert!(
                    error.to_string().contains("čl. 52 st. 2"),
                    "the operator is told which stav is owed: {}",
                    error
                );

                let supplied = update_breach(
                    state,
                    opened.id,
                    BreachDraft {
                        delay_reason: Some(
                            "Povredu smo utvrdili u petak uveče, a obim smo mogli da \
                             procenimo tek posle vikenda."
                                .to_string(),
                        ),
                        ..minimal_draft()
                    },
                    NA_ROKU,
                )
                .expect("with the justification supplied the record saves");
                assert!(supplied.delay_reason_required);
                assert!(supplied.delay_reason.is_some());

                // A reasoned decision that st. 1 never attached owes no st. 2
                // justification — but the reasoning itself is not optional.
                let missing_reasoning = update_breach(
                    state,
                    opened.id,
                    BreachDraft {
                        risk_outcome: Some(RiskOutcome::BezRizika),
                        notify_decision: Some(NotifyDecision::NeObavestiti),
                        ..minimal_draft()
                    },
                    NA_ROKU,
                )
                .expect_err("a decision without reasoning does not discharge st. 7");
                assert_eq!(missing_reasoning.code(), "validation_error");

                let not_notifiable = update_breach(
                    state,
                    opened.id,
                    BreachDraft {
                        risk_outcome: Some(RiskOutcome::BezRizika),
                        notify_decision: Some(NotifyDecision::NeObavestiti),
                        notify_obrazlozenje: Some(
                            "Podaci su bili šifrovani i ključ nije bio na uređaju.".to_string(),
                        ),
                        ..minimal_draft()
                    },
                    NA_ROKU,
                )
                .expect("st. 2 explains a missed st. 1 deadline, and st. 1 never attached");
                assert!(!not_notifiable.delay_reason_required);
                assert_eq!(not_notifiable.notifiable, Some(false));

                // The exemption reaches exactly that far. Where the assessment
                // says rizik, st. 1 DID attach, and „odlučili smo da ne
                // obavestimo“ is not a discharge of it — it is the very state
                // st. 2 exists to make the shop explain.
                let decided_against_despite_risk = update_breach(
                    state,
                    opened.id,
                    BreachDraft {
                        risk_outcome: Some(RiskOutcome::VisokRizik),
                        notify_decision: Some(NotifyDecision::NeObavestiti),
                        notify_obrazlozenje: Some(
                            "Procenili smo da je incident interne prirode.".to_string(),
                        ),
                        ..minimal_draft()
                    },
                    NA_ROKU,
                )
                .expect_err("a decision not to notify does not undo an attached st. 1 duty");
                assert_eq!(decided_against_despite_risk.code(), "validation_error");
                assert!(
                    decided_against_despite_risk
                        .to_string()
                        .contains("čl. 52 st. 2"),
                    "the operator is told which stav is owed: {decided_against_despite_risk}"
                );

                // And an unassessed record is not an exemption either: „nije
                // postupio u tom roku“ covers the shop that never decided.
                let never_assessed = update_breach(
                    state,
                    opened.id,
                    BreachDraft {
                        risk_outcome: None,
                        notify_decision: Some(NotifyDecision::NeObavestiti),
                        notify_obrazlozenje: Some(
                            "Još uvek utvrđujemo obim incidenta.".to_string(),
                        ),
                        ..minimal_draft()
                    },
                    NA_ROKU,
                )
                .expect_err("without an assessment nothing establishes that st. 1 never attached");
                assert_eq!(never_assessed.code(), "validation_error");

                // And notifying inside the window closes the question for good.
                let in_time = update_breach(
                    state,
                    opened.id,
                    BreachDraft {
                        risk_outcome: Some(RiskOutcome::Rizik),
                        notify_decision: Some(NotifyDecision::Obavestiti),
                        notify_obrazlozenje: Some(
                            "Podaci o kupcima iz reklamacija mogu da budu dostupni trećem licu."
                                .to_string(),
                        ),
                        poverenik_notified_at: Some(PRE_ROKA.to_string()),
                        ..minimal_draft()
                    },
                    NA_ROKU,
                )
                .expect("a notification inside the window needs no justification");
                assert!(!in_time.delay_reason_required);
                assert_eq!(in_time.notifiable, Some(true));
            },
        );
    }

    /// The clock anchor cannot be moved. A saznanje that drifts forward makes
    /// the st. 2 delay reason optional in hindsight and rewrites the deadline
    /// the Poverenik measures the shop against — so the command refuses the
    /// change outright rather than ignoring it, and v18's trigger refuses it
    /// again underneath, for anything that reaches the table another way.
    #[test]
    fn saznanje_at_is_immutable_once_set() {
        with_state("saznanje_at_is_immutable_once_set", |state| {
            sign_in_admin(state);

            let opened =
                record_breach(state, minimal_draft(), SAZNANJE).expect("record should open");

            let error = update_breach(
                state,
                opened.id,
                BreachDraft {
                    saznanje_at: "2026-08-03T09:00:00Z".to_string(),
                    ..minimal_draft()
                },
                "2026-08-03T10:00:00Z",
            )
            .expect_err("moving the clock anchor must be refused, not ignored");
            assert_eq!(error.code(), "povreda_saznanje_nepromenljivo");
            assert!(
                error.to_string().contains("čl. 52 st. 1"),
                "the operator is told which provision anchors it: {}",
                error
            );

            let stored = only_breach(state, SAZNANJE);
            assert_eq!(
                stored.saznanje_at, SAZNANJE,
                "the refused update left the anchor exactly where it was"
            );
            assert_eq!(
                stored.rok_obavestavanja_istice_at, NA_ROKU,
                "and therefore left the deadline where it was"
            );

            // The storage layer refuses it too, for anything that never went
            // through this module.
            let connection = state.db().open().expect("database should open");
            let raw = connection.execute(
                "UPDATE data_breaches SET saznanje_at = ?1 WHERE id = ?2",
                params!["2026-08-03T09:00:00Z", opened.id],
            );
            let raw = raw.expect_err("v18's trigger must abort a direct UPDATE");
            assert!(
                raw.to_string().contains("nepromenljivo"),
                "the trigger's own message reaches the caller: {raw}"
            );

            assert_eq!(
                only_breach(state, SAZNANJE).saznanje_at,
                SAZNANJE,
                "and the row is unchanged after the aborted UPDATE"
            );
        });
    }

    /// Čl. 53 st. 1 makes the affected individuals the addressee once a breach
    /// may produce a **visok rizik** for them. St. 3 gives three exceptions and
    /// only three — so a record that says the individuals were not told must
    /// name which one was relied on and why. „We decided not to“ is not among
    /// them, and it is not representable here.
    #[test]
    fn the_cl_53_block_records_which_exception_was_relied_on_when_subjects_were_not_told() {
        with_state(
            "the_cl_53_block_records_which_exception_was_relied_on_when_subjects_were_not_told",
            |state| {
                sign_in_admin(state);

                let high_risk = BreachDraft {
                    risk_outcome: Some(RiskOutcome::VisokRizik),
                    broj_lica: Some(37),
                    kategorije_podataka: Some("Ime i prezime, broj telefona".to_string()),
                    ..minimal_draft()
                };

                let silent = record_breach(
                    state,
                    BreachDraft {
                        lica_obavestena: Some(false),
                        ..high_risk.clone()
                    },
                    SAZNANJE,
                )
                .expect_err(
                    "a high-risk breach the subjects were not told about needs an izuzetak",
                );
                assert_eq!(silent.code(), "validation_error");
                assert!(
                    silent.to_string().contains("čl. 53 st. 3"),
                    "the operator is told the exception must come from st. 3: {}",
                    silent
                );

                let unreasoned = record_breach(
                    state,
                    BreachDraft {
                        lica_obavestena: Some(false),
                        cl53_izuzetak: Some(Cl53Izuzetak::NaknadneMere),
                        ..high_risk.clone()
                    },
                    SAZNANJE,
                )
                .expect_err("a bare exception code proves nothing to the Poverenik");
                assert_eq!(unreasoned.code(), "validation_error");

                let recorded = record_breach(
                    state,
                    BreachDraft {
                        lica_obavestena: Some(false),
                        cl53_izuzetak: Some(Cl53Izuzetak::PrimenjeneMereZastite),
                        cl53_izuzetak_obrazlozenje: Some(
                            "Podaci na uređaju su bili šifrovani, a ključ nije bio na njemu."
                                .to_string(),
                        ),
                        ..high_risk.clone()
                    },
                    SAZNANJE,
                )
                .expect("with the exception and its reasoning the record saves");

                assert!(
                    recorded.obavestavanje_lica_obavezno,
                    "čl. 53 st. 1 attaches at visok rizik"
                );
                assert_eq!(recorded.lica_obavestena, Some(false));
                assert_eq!(
                    recorded.cl53_izuzetak,
                    Some(Cl53Izuzetak::PrimenjeneMereZastite),
                    "the record says WHICH st. 3 exception was relied on"
                );
                assert!(recorded.cl53_izuzetak_obrazlozenje.is_some());
                assert_eq!(
                    recorded.broj_lica,
                    Some(37),
                    "the obrazac asks for a count of the individuals, never a roster"
                );

                // Telling them is the other branch, and it needs no exception —
                // but it does need the date it happened.
                let undated = update_breach(
                    state,
                    recorded.id,
                    BreachDraft {
                        lica_obavestena: Some(true),
                        ..high_risk.clone()
                    },
                    SAZNANJE,
                )
                .expect_err("„we told them“ without a date is not a fact the Poverenik can check");
                assert_eq!(undated.code(), "validation_error");

                let told = update_breach(
                    state,
                    recorded.id,
                    BreachDraft {
                        lica_obavestena: Some(true),
                        lica_obavestena_at: Some("2026-08-01T15:00:00Z".to_string()),
                        ..high_risk.clone()
                    },
                    SAZNANJE,
                )
                .expect("telling the individuals needs no st. 3 exception");
                assert_eq!(told.lica_obavestena, Some(true));
                assert_eq!(told.cl53_izuzetak, None);
                assert!(told.obavestavanje_lica_obavezno);
            },
        );
    }

    /// Req. 48 — the breach log is admin-only. It is the shop's own compliance
    /// file, and in a three-employee shop the incident description is very often
    /// about a colleague.
    #[test]
    fn the_breach_log_is_admin_only() {
        with_state("the_breach_log_is_admin_only", |state| {
            sign_in_admin(state);
            record_breach(state, minimal_draft(), SAZNANJE).expect("admin may record");

            sign_in_cashier(state);

            for error in [
                record_breach(state, minimal_draft(), SAZNANJE)
                    .expect_err("a kasir may not open a breach record"),
                update_breach(state, 1, minimal_draft(), SAZNANJE)
                    .expect_err("a kasir may not correct one"),
                list_breaches(state, SAZNANJE).expect_err("a kasir may not read the log"),
            ] {
                assert_eq!(error.code(), "forbidden");
            }
        });
    }

    /// Req. 4 is a prohibition, not a preference: the incident's prose — which
    /// in this shop names people — must not follow the record into the čl. 48
    /// evidencija pristupa. The line about a breach record carries its id.
    #[test]
    fn the_audit_line_carries_the_record_id_and_none_of_the_incident_text() {
        with_state(
            "the_audit_line_carries_the_record_id_and_none_of_the_incident_text",
            |state| {
                let admin_id = sign_in_admin(state);

                let draft = BreachDraft {
                    opis: "Milica Marković je izgubila službeni telefon.".to_string(),
                    posledice: "Podaci kupca Petrovića mogli su da budu dostupni.".to_string(),
                    mere: "Uređaj je daljinski obrisan.".to_string(),
                    kategorije_podataka: Some("Ime i prezime, adresa".to_string()),
                    ..minimal_draft()
                };
                let opened =
                    record_breach(state, draft.clone(), SAZNANJE).expect("the record should open");
                update_breach(state, opened.id, draft.clone(), SAZNANJE)
                    .expect("the correction should save");

                let logged = audit_table_text(state);
                for forbidden in [
                    "Milica",
                    "Marković",
                    "Petrović",
                    "telefon",
                    "adresa",
                    &draft.opis,
                    &draft.posledice,
                    &draft.mere,
                ] {
                    assert!(
                        !logged.contains(forbidden),
                        "the incident prose must never reach audit_events; found \
                         {forbidden:?} in: {logged}"
                    );
                }

                let rows: Vec<&str> = logged.lines().collect();
                assert_eq!(rows.len(), 2, "one line for the unos, one for the izmena");
                assert!(rows[0].contains("|unos|data_breach|"), "{}", rows[0]);
                assert!(rows[1].contains("|menjanje|data_breach|"), "{}", rows[1]);
                for row in &rows {
                    assert!(
                        row.contains(&format!("|data_breach|{}|", opened.id)),
                        "the line references the record by opaque id: {row}"
                    );
                    assert!(
                        row.contains(&format!("|{admin_id}|")),
                        "čl. 48 st. 2 identitet lica, as a user id: {row}"
                    );
                    assert!(
                        row.contains("|bezbednosni_incident|"),
                        "čl. 48 st. 2 razlog: {row}"
                    );
                }
            },
        );
    }

    /// The three closed vocabularies are the contract with v18's CHECK lists. A
    /// code that round-trips in Rust but is not in the CHECK is a runtime
    /// constraint failure on the operator's screen, so every variant is actually
    /// written to the table here.
    #[test]
    fn every_code_round_trips_and_is_accepted_by_the_v18_checks() {
        for outcome in RiskOutcome::ALL {
            assert_eq!(RiskOutcome::from_code(outcome.as_code()), Some(outcome));
        }
        for decision in NotifyDecision::ALL {
            assert_eq!(
                NotifyDecision::from_code(decision.as_code()),
                Some(decision)
            );
        }
        for izuzetak in Cl53Izuzetak::ALL {
            assert_eq!(Cl53Izuzetak::from_code(izuzetak.as_code()), Some(izuzetak));
        }
        assert_eq!(RiskOutcome::from_code("visok rizik"), None);
        assert_eq!(Cl53Izuzetak::from_code("odluka rukovaoca"), None);

        with_state(
            "every_code_round_trips_and_is_accepted_by_the_v18_checks",
            |state| {
                sign_in_admin(state);

                for outcome in RiskOutcome::ALL {
                    for decision in NotifyDecision::ALL {
                        for izuzetak in Cl53Izuzetak::ALL {
                            let stored = record_breach(
                                state,
                                BreachDraft {
                                    risk_outcome: Some(outcome),
                                    notify_decision: Some(decision),
                                    notify_obrazlozenje: Some(
                                        "Procena rizika je dokumentovana.".to_string(),
                                    ),
                                    lica_obavestena: Some(false),
                                    cl53_izuzetak: Some(izuzetak),
                                    cl53_izuzetak_obrazlozenje: Some(
                                        "Razlog je zabeležen uz odluku.".to_string(),
                                    ),
                                    ..minimal_draft()
                                },
                                SAZNANJE,
                            )
                            .expect("every code in the enum must satisfy the v18 CHECK");

                            assert_eq!(stored.risk_outcome, Some(outcome));
                            assert_eq!(stored.notify_decision, Some(decision));
                            assert_eq!(stored.cl53_izuzetak, Some(izuzetak));
                        }
                    }
                }

                assert_eq!(
                    list_breaches(state, SAZNANJE)
                        .expect("the log should read")
                        .len(),
                    RiskOutcome::ALL.len() * NotifyDecision::ALL.len() * Cl53Izuzetak::ALL.len(),
                );
            },
        );
    }

    // -----------------------------------------------------------------------
    // Task 7 — the Pravilnik 40/2019 obrazac (req. 45)
    // -----------------------------------------------------------------------

    /// The five prescribed section headings, verbatim, in the order the obrazac
    /// annexed to Pravilnik 40/2019 puts them in.
    const SEKCIJE: [&str; 5] = [
        "1) Podaci o rukovaocu",
        "2) Podaci o povredi podataka",
        "3) Opis mogućih posledica povrede",
        "4) Opis mera koje je rukovalac preduzeo ili čije je preduzimanje predloženo",
        "5) Ostali podaci od značaja za obaveštavanje o povredi podataka",
    ];

    fn company() -> CompanySettings {
        CompanySettings {
            shop_name: "Butik Milena preduzetnik".to_string(),
            address: "Njegoševa 12, Beograd".to_string(),
            pib: "111222333".to_string(),
            ..CompanySettings::default()
        }
    }

    /// The value the obrazac prints for one prescribed field — the contents of
    /// the cell that follows the field's label.
    fn polje(html: &str, label: &str) -> String {
        let at = html
            .find(label)
            .unwrap_or_else(|| panic!("the obrazac must carry the field {label:?}"));
        let rest = &html[at + label.len()..];
        let cell = rest
            .find("<td")
            .unwrap_or_else(|| panic!("the field {label:?} must be followed by its cell"));
        let start = cell
            + rest[cell..]
                .find('>')
                .expect("the opening cell tag must close")
            + 1;
        let end = start
            + rest[start..]
                .find("</td>")
                .expect("the cell must close")
                .to_owned();
        rest[start..end].trim().to_string()
    }

    /// Asserts the markers appear in the document in exactly this order.
    fn redom(html: &str, markers: &[&str]) {
        let mut previous = 0usize;
        for marker in markers {
            let at = html
                .find(marker)
                .unwrap_or_else(|| panic!("the obrazac must carry {marker:?}"));
            assert!(
                at >= previous,
                "{marker:?} is out of order — it appears at {at}, before position {previous}"
            );
            previous = at;
        }
    }

    /// A record with something in every column the obrazac can draw on, so the
    /// renderer is exercised at its widest.
    fn full_draft() -> BreachDraft {
        BreachDraft {
            occurred_at: Some("2026-07-31T18:20:00Z".to_string()),
            discovered_at: Some("2026-08-01T08:40:00Z".to_string()),
            obradjivac_saznanje_at: Some("2026-08-01T07:00:00Z".to_string()),
            rukovalac_obavesten_at: Some("2026-08-01T08:55:00Z".to_string()),
            broj_lica: Some(37),
            kategorije_podataka: Some("Ime i prezime, broj telefona".to_string()),
            risk_outcome: Some(RiskOutcome::VisokRizik),
            notify_decision: Some(NotifyDecision::Obavestiti),
            notify_obrazlozenje: Some("Povreda može da ugrozi prava kupaca.".to_string()),
            poverenik_notified_at: Some(PRE_ROKA.to_string()),
            lica_obavestena: Some(false),
            cl53_izuzetak: Some(Cl53Izuzetak::PrimenjeneMereZastite),
            cl53_izuzetak_obrazlozenje: Some(
                "Podaci na uređaju su bili šifrovani, a ključ nije bio na njemu.".to_string(),
            ),
            ..minimal_draft()
        }
    }

    /// Req. 45 — the obrazac is reproduced **verbatim** in its five-section
    /// structure. Not a summary of it, not a re-ordering of it: the Poverenik
    /// receives the form his own pravilnik prescribes, with the record's answers
    /// in the prescribed slots.
    #[test]
    fn the_obrazac_renders_all_five_prescribed_sections_in_order() {
        with_state(
            "the_obrazac_renders_all_five_prescribed_sections_in_order",
            |state| {
                sign_in_admin(state);
                let breach =
                    record_breach(state, full_draft(), SAZNANJE).expect("the record should open");
                let html = render_obrazac_html(&company(), &breach);

                redom(&html, &SEKCIJE);

                // Section 1 — three sub-fields, and the two the shop's own
                // settings answer are answered.
                redom(
                    &html,
                    &[
                        "1) Podaci o rukovaocu",
                        "(1) naziv rukovaoca",
                        "(2) adresa/sedište",
                        "(3) ime i kontakt podaci lica za zaštitu podataka o ličnosti",
                        "2) Podaci o povredi podataka",
                    ],
                );
                assert_eq!(polje(&html, "(1) naziv rukovaoca"), company().shop_name);
                assert_eq!(polje(&html, "(2) adresa/sedište"), company().address);

                // Section 2 — five sub-fields, in the prescribed order.
                redom(
                    &html,
                    &[
                        "2) Podaci o povredi podataka",
                        "(1) opis prirode povrede podataka",
                        "(2) vrsta podataka o ličnosti",
                        "(3) broj lica na koja se podaci odnose",
                        "(4) broj podataka o ličnosti čija je bezbednost povređena",
                        "(5) datum i vreme povrede bezbednosti podataka",
                        "3) Opis mogućih posledica povrede",
                    ],
                );
                assert_eq!(
                    polje(&html, "(1) opis prirode povrede podataka"),
                    breach.opis
                );
                assert_eq!(
                    polje(&html, "(2) vrsta podataka o ličnosti"),
                    "Ime i prezime, broj telefona"
                );
                assert_eq!(polje(&html, "(3) broj lica na koja se podaci odnose"), "37");
                assert_eq!(
                    polje(&html, "(5) datum i vreme povrede bezbednosti podataka"),
                    "31.07.2026. 18:20 (UTC)",
                    "the incident's own instant, not saznanje"
                );

                // Sections 3 and 4 are the other two čl. 52 st. 6 elements.
                assert!(html.contains(&breach.posledice), "posledice");
                assert!(html.contains(&breach.mere), "preduzete mere");

                // Section 5 carries what st. 7 needs and section 2 has no slot
                // for: the saznanje anchor and the čl. 53 block.
                redom(
                    &html,
                    &[
                        "5) Ostali podaci od značaja za obaveštavanje o povredi podataka",
                        "Datum i vreme saznanja za povredu",
                        "Lica na koja se podaci odnose obaveštena",
                        "Izuzetak od obaveštavanja lica",
                    ],
                );
                assert_eq!(
                    polje(&html, "Datum i vreme saznanja za povredu"),
                    "01.08.2026. 09:00 (UTC)"
                );
            },
        );
    }

    /// Req. 45 — the form ends with the „Prilog:“ slot and the mesto/datum +
    /// Ime i prezime + Potpis block, in that order and with nothing after it.
    ///
    /// The Prilog is not decoration: Pravilnik čl. 4 st. 1 makes the čl. 47
    /// evidencija radnji obrade a **mandatory** attachment, so the slot names
    /// what has to travel with the form.
    #[test]
    fn the_obrazac_ends_with_the_prilog_and_the_signature_block() {
        with_state(
            "the_obrazac_ends_with_the_prilog_and_the_signature_block",
            |state| {
                sign_in_admin(state);
                let breach = record_breach(state, minimal_draft(), SAZNANJE)
                    .expect("the record should open");
                let html = render_obrazac_html(&company(), &breach);

                redom(
                    &html,
                    &[
                        "5) Ostali podaci od značaja za obaveštavanje o povredi podataka",
                        "Prilog:",
                        "evidencija radnji obrade",
                        "čl. 4 st. 1",
                        ", dana ",
                        ". godine",
                        "Za RUKOVAOCA",
                        "Ime i prezime",
                        "Potpis",
                    ],
                );

                let after =
                    &html[html.find("Potpis").expect("the signature line") + "Potpis".len()..];
                for section in SEKCIJE {
                    assert!(
                        !after.contains(section),
                        "nothing of the form follows Potpis, found {section:?}"
                    );
                }

                // The signer writes the place, the date and the name by hand —
                // an unsigned form must not assert a signing date that has not
                // happened, and the person who signs za rukovaoca is not
                // necessarily whoever is logged in.
                assert_eq!(
                    html.matches("Ime i prezime").count(),
                    1,
                    "„Ime i prezime“ belongs to the signature block and to nothing else"
                );
            },
        );
    }

    /// Pravilnik 40/2019 čl. 3 restates the čl. 52 st. 1 deadline as a **flat**
    /// 72 h od saznanja, without the statute's *„bez nepotrebnog odlaganja, ili,
    /// ako je to moguće“* softener. That is the article an in-app countdown
    /// cites, and the softener must not travel beside it — a form that prints
    /// both tells the shop the deadline is negotiable.
    #[test]
    fn the_seventy_two_hour_countdown_cites_pravilnik_cl_3() {
        with_state(
            "the_seventy_two_hour_countdown_cites_pravilnik_cl_3",
            |state| {
                sign_in_admin(state);
                let opened = record_breach(state, minimal_draft(), SAZNANJE)
                    .expect("the record should open");
                let html = render_obrazac_html(&company(), &opened);

                let rok = html
                    .lines()
                    .find(|line| line.contains("72 časa"))
                    .expect("the obrazac states the deadline");
                assert!(
                    rok.contains("Pravilnik") && rok.contains("čl. 3"),
                    "the countdown cites its own source: {rok}"
                );
                assert!(
                    rok.contains("04.08.2026. 09:00 (UTC)"),
                    "and prints saznanje + 72 h as an instant: {rok}"
                );
                assert!(
                    !html.contains("bez nepotrebnog odlaganja"),
                    "čl. 3 carries no softener and neither does the countdown"
                );

                // Past the deadline the st. 2 explanation travels on the form
                // itself, in the section that has room for it.
                let late = update_breach(
                    state,
                    opened.id,
                    BreachDraft {
                        delay_reason: Some(
                            "Povredu smo utvrdili u petak uveče, a obim tek posle vikenda."
                                .to_string(),
                        ),
                        ..minimal_draft()
                    },
                    NA_ROKU,
                )
                .expect("with the justification supplied the record saves");
                let late_html = render_obrazac_html(&company(), &late);
                assert!(late.delay_reason_required);
                assert!(
                    late_html.contains("čl. 52 st. 2"),
                    "the late form names the stav that asks for the explanation"
                );
                assert!(
                    late_html.contains(late.delay_reason.as_deref().expect("razlog")),
                    "and carries the explanation itself"
                );
            },
        );
    }

    /// The obrazac is a statement to the regulator, so every value on it has to
    /// be a value the shop actually holds. Two slots have no counterpart in this
    /// record and must stay empty for the operator to complete by hand — and
    /// **(4) broj podataka** must never be filled from **(3) broj lica**: they
    /// are different counts, and answering one with the other is a false
    /// statement to the Poverenik.
    ///
    /// There is no roster anywhere: the form asks for a count of the individuals
    /// and this record holds a count.
    #[test]
    fn the_obrazac_prints_no_field_the_record_does_not_hold() {
        with_state(
            "the_obrazac_prints_no_field_the_record_does_not_hold",
            |state| {
                sign_in_admin(state);
                let full =
                    record_breach(state, full_draft(), SAZNANJE).expect("the record should open");
                let html = render_obrazac_html(&company(), &full);

                assert_eq!(polje(&html, "(3) broj lica na koja se podaci odnose"), "37");
                assert_eq!(
                    polje(&html, "(4) broj podataka o ličnosti čija je bezbednost povređena"),
                    "",
                    "a count of records is not a count of people and the record holds no such count"
                );
                assert_eq!(
                    polje(&html, "(3) ime i kontakt podaci lica za zaštitu podataka o ličnosti"),
                    "",
                    "no lice za zaštitu podataka is configured, so the slot is left to be filled in"
                );

                // A record opened with nothing but the three st. 6 elements
                // leaves every optional slot empty rather than inventing a zero,
                // a date or a risk assessment nobody made.
                let minimal = record_breach(state, minimal_draft(), SAZNANJE)
                    .expect("a minimal record should open");
                let bare = render_obrazac_html(&company(), &minimal);
                for label in [
                    "(2) vrsta podataka o ličnosti",
                    "(3) broj lica na koja se podaci odnose",
                    "(4) broj podataka o ličnosti čija je bezbednost povređena",
                    "(5) datum i vreme povrede bezbednosti podataka",
                ] {
                    assert_eq!(
                        polje(&bare, label),
                        "",
                        "{label:?} has no answer in the record and must print none"
                    );
                }
                for absent in ["Procena rizika", "Izuzetak od obaveštavanja lica"] {
                    assert!(
                        !bare.contains(absent),
                        "{absent:?} was never recorded, so the form does not raise it"
                    );
                }

                // No submission API exists (req. 45) — the form says how it is
                // actually filed (Pravilnik čl. 5) and the app never sends it.
                assert!(
                    html.contains("Pravilnik 40/2019 čl. 5")
                        && html.contains("povredapodataka@poverenik.rs"),
                    "the filing route is on the form because there is no other one"
                );
            },
        );
    }

    /// The obrazac is the disclosure that leaves the till, so it — and not the
    /// panel that merely displays the log — is where the čl. 48 st. 2
    /// **otkrivanje** line belongs, naming the Poverenik as the primalac. And
    /// like every other verb on this record it is admin-only (req. 48).
    #[test]
    fn the_export_records_the_cl_48_otkrivanje_line_and_is_admin_only() {
        with_state(
            "the_export_records_the_cl_48_otkrivanje_line_and_is_admin_only",
            |state| {
                let admin_id = sign_in_admin(state);
                let breach =
                    record_breach(state, full_draft(), SAZNANJE).expect("the record should open");

                let exported =
                    export_obrazac(state, breach.id, SAZNANJE).expect("the obrazac should render");
                let written =
                    std::fs::read_to_string(&exported.path).expect("the file should be written");
                std::fs::remove_file(&exported.path).expect("the export should be removable");

                assert_eq!(exported.mime_type, "text/html");
                for section in SEKCIJE {
                    assert!(written.contains(section), "{section:?} reached the file");
                }

                let rows: Vec<String> = audit_table_text(state)
                    .lines()
                    .map(str::to_string)
                    .collect();
                let disclosure = rows.last().expect("the export writes a line").clone();
                assert!(
                    disclosure.contains("|otkrivanje|data_breach|"),
                    "čl. 48 st. 2: what was done and to which object: {disclosure}"
                );
                assert!(
                    disclosure.contains(&format!("|data_breach|{}|", breach.id)),
                    "referenced by opaque id: {disclosure}"
                );
                assert!(
                    disclosure.contains("|poverenik|"),
                    "čl. 48 st. 2 primalac: {disclosure}"
                );
                assert!(
                    disclosure.contains(&format!("|{admin_id}|")),
                    "čl. 48 st. 2 identitet lica: {disclosure}"
                );
                for forbidden in [breach.opis.as_str(), breach.posledice.as_str()] {
                    assert!(
                        !audit_table_text(state).contains(forbidden),
                        "the incident prose never follows the export into audit_events"
                    );
                }

                sign_in_cashier(state);
                let refused = export_obrazac(state, breach.id, SAZNANJE)
                    .expect_err("a kasir may not export the obrazac");
                assert_eq!(refused.code(), "forbidden");
            },
        );
    }
}
