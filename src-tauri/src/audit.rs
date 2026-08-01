//! The audit log's two structural guarantees: the SHA-256 hash chain that makes
//! tampering loud, and the write-boundary filter that keeps personal data out.
//!
//! **SW-10 is a PRUDENTIAL control, never a legal duty.** No ZZPL provision
//! obliges a private rukovalac to record who accessed personal data — čl. 48 and
//! čl. 51 bind only a nadležni organ u posebne svrhe, and čl. 50 appears nowhere
//! in čl. 95, so no prekršaj attaches to not having this log at all. What it does
//! discharge is an OUTCOME duty: čl. 5 st. 2 odgovornost za postupanje and the
//! čl. 41 st. 1 ability to predočiti. Its field list is modelled on čl. 48 st. 2
//! and čl. 51 st. 2 t. 7 — the only two content specs Serbian law has for a log.
//!
//! Everything here is pure: no clock, no connection, no state. The command layer
//! (Tasks 3 and 4) supplies `now` and the database.

// The command layer that consumes this lands in Tasks 3–5, mirroring the other
// domain modules (`worktime.rs`) which allow the same until their wiring arrives.
#![allow(dead_code)]

use std::fmt::Write as _;

use sha2::{Digest, Sha256};

use crate::app_error::AppError;

/// The `prev_hash` of the first row. Empty, not a sentinel string: a genesis
/// marker that looked like a hash could be forged onto a later row to make a
/// truncated log verify as if it started there.
pub const GENESIS_PREV_HASH: &str = "";

/// ZZPL čl. 48 st. 1's verbs, transliterated to Serbian Latin without diacritics
/// for the storage code: *unos, menjanje, uvid, otkrivanje (uključujući i
/// prenos), upoređivanje, brisanje*. Closed, because a log whose vocabulary
/// drifts cannot be read back as evidence of anything.
///
/// The codes MUST stay identical to the `CHECK (action IN (…))` list on
/// `audit_events` in migration v18.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditAction {
    Unos,
    Menjanje,
    Uvid,
    Otkrivanje,
    Uporedjivanje,
    Brisanje,
}

impl AuditAction {
    pub const ALL: [Self; 6] = [
        Self::Unos,
        Self::Menjanje,
        Self::Uvid,
        Self::Otkrivanje,
        Self::Uporedjivanje,
        Self::Brisanje,
    ];

    pub fn as_code(self) -> &'static str {
        match self {
            Self::Unos => "unos",
            Self::Menjanje => "menjanje",
            Self::Uvid => "uvid",
            Self::Otkrivanje => "otkrivanje",
            Self::Uporedjivanje => "uporedjivanje",
            Self::Brisanje => "brisanje",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_code() == code)
    }

    /// ZZPL čl. 48 st. 2 requires the *razlog* for exactly these two verbs.
    fn requires_reason(self) -> bool {
        matches!(self, Self::Uvid | Self::Otkrivanje)
    }
}

/// The čl. 48 st. 2 *razlog*, as a closed enum. The one column that could
/// plausibly have been free text is the one an operator would eventually type a
/// customer's name into.
///
/// The codes MUST stay identical to the `reason_code` CHECK list in migration v18.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditReason {
    Inspekcija,
    ZahtevLica,
    InterniNadzor,
    ObradaReklamacije,
    ObracunZarade,
    TehnickaPodrska,
    SudskiIliUpravniPostupak,
    BezbednosniIncident,
    ZakonskaObaveza,
    AutomatskoCiscenje,
}

impl AuditReason {
    pub const ALL: [Self; 10] = [
        Self::Inspekcija,
        Self::ZahtevLica,
        Self::InterniNadzor,
        Self::ObradaReklamacije,
        Self::ObracunZarade,
        Self::TehnickaPodrska,
        Self::SudskiIliUpravniPostupak,
        Self::BezbednosniIncident,
        Self::ZakonskaObaveza,
        Self::AutomatskoCiscenje,
    ];

    pub fn as_code(self) -> &'static str {
        match self {
            Self::Inspekcija => "inspekcija",
            Self::ZahtevLica => "zahtev_lica",
            Self::InterniNadzor => "interni_nadzor",
            Self::ObradaReklamacije => "obrada_reklamacije",
            Self::ObracunZarade => "obracun_zarade",
            Self::TehnickaPodrska => "tehnicka_podrska",
            Self::SudskiIliUpravniPostupak => "sudski_ili_upravni_postupak",
            Self::BezbednosniIncident => "bezbednosni_incident",
            Self::ZakonskaObaveza => "zakonska_obaveza",
            Self::AutomatskoCiscenje => "automatsko_ciscenje",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|reason| reason.as_code() == code)
    }
}

/// Čl. 48 st. 2 „identiteta primaoca“, recorded as a CLASS of recipient. A named
/// recipient would be personal data about that recipient, sitting in the table
/// whose whole point (req. 4) is to hold none — the class answers the statutory
/// question without reproducing the problem.
///
/// The codes MUST stay identical to the `recipient` CHECK list in migration v18.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditRecipient {
    LiceNaKojeSePodaciOdnose,
    PoreskaUprava,
    Inspekcija,
    Poverenik,
    SudIliJavniTuzilac,
    Mup,
    Knjigovodja,
    ObradjivacTehnickePodrske,
    Banka,
    DrugiOrgan,
}

impl AuditRecipient {
    pub const ALL: [Self; 10] = [
        Self::LiceNaKojeSePodaciOdnose,
        Self::PoreskaUprava,
        Self::Inspekcija,
        Self::Poverenik,
        Self::SudIliJavniTuzilac,
        Self::Mup,
        Self::Knjigovodja,
        Self::ObradjivacTehnickePodrske,
        Self::Banka,
        Self::DrugiOrgan,
    ];

    pub fn as_code(self) -> &'static str {
        match self {
            Self::LiceNaKojeSePodaciOdnose => "lice_na_koje_se_podaci_odnose",
            Self::PoreskaUprava => "poreska_uprava",
            Self::Inspekcija => "inspekcija",
            Self::Poverenik => "poverenik",
            Self::SudIliJavniTuzilac => "sud_ili_javni_tuzilac",
            Self::Mup => "mup",
            Self::Knjigovodja => "knjigovodja",
            Self::ObradjivacTehnickePodrske => "obradjivac_tehnicke_podrske",
            Self::Banka => "banka",
            Self::DrugiOrgan => "drugi_organ",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|recipient| recipient.as_code() == code)
    }
}

/// One row about to be appended to `audit_events`.
///
/// The struct IS the exclusion list's first line of defence: there is no
/// `before_value`, no `after_value`, no `note` and no `query` field, so a value
/// payload has nowhere to sit (req. 4). The two remaining textual fields are
/// scanned by [`reject_forbidden_content`] before any write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditDraft {
    /// RFC3339, supplied by the caller. This module never reads a clock.
    pub at: String,
    /// Čl. 48 st. 2 „identiteta lica“ — an internal user id, NEVER a name.
    /// `None` for the time-driven purge line (req. 23), which has no human actor.
    pub actor_user_id: Option<i64>,
    pub action: AuditAction,
    /// A code constant: `sale`, `employee`, `personnel_record`, …
    pub object_type: String,
    /// An opaque internal id, never the object's contents.
    pub object_id: String,
    pub reason_code: Option<AuditReason>,
    pub recipient: Option<AuditRecipient>,
    pub support_session_id: Option<i64>,
}

/// A row read back out of `audit_events`, carrying the two chain columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRow {
    pub id: i64,
    pub at: String,
    pub actor_user_id: Option<i64>,
    pub action: AuditAction,
    pub object_type: String,
    pub object_id: String,
    pub reason_code: Option<AuditReason>,
    pub recipient: Option<AuditRecipient>,
    pub support_session_id: Option<i64>,
    pub prev_hash: String,
    pub hash: String,
}

impl AuditRow {
    /// The hashed projection of the row — every column the chain covers.
    pub fn as_draft(&self) -> AuditDraft {
        AuditDraft {
            at: self.at.clone(),
            actor_user_id: self.actor_user_id,
            action: self.action,
            object_type: self.object_type.clone(),
            object_id: self.object_id.clone(),
            reason_code: self.reason_code,
            recipient: self.recipient,
            support_session_id: self.support_session_id,
        }
    }
}

/// The result of walking a log from its genesis row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChainVerdict {
    Intact,
    /// The zero-based index of the first row whose stored hash or whose link to
    /// its predecessor does not reconcile. A deletion surfaces here too: the
    /// survivor's `prev_hash` no longer matches the row now in front of it.
    BrokenAt(usize),
}

/// SHA-256 over the previous hash and every field of the draft.
///
/// Fields are length-prefixed rather than delimiter-joined, so no combination of
/// values can be rearranged into the same byte stream as another — a chain whose
/// preimage is ambiguous is not tamper-evident.
pub fn chain_hash(prev: &str, draft: &AuditDraft) -> String {
    let actor = draft
        .actor_user_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    let session = draft
        .support_session_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    let fields: [&str; 9] = [
        prev,
        &draft.at,
        &actor,
        draft.action.as_code(),
        &draft.object_type,
        &draft.object_id,
        draft.reason_code.map(AuditReason::as_code).unwrap_or(""),
        draft.recipient.map(AuditRecipient::as_code).unwrap_or(""),
        &session,
    ];

    let mut hasher = Sha256::new();
    for field in fields {
        hasher.update(field.len().to_string().as_bytes());
        hasher.update(b":");
        hasher.update(field.as_bytes());
    }

    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Walks the whole log from its genesis row and reports the first break.
///
/// `rows` must be the log in `id` order starting at the genesis row; a window cut
/// out of the middle would fail the genesis check by construction. Anchoring on
/// genesis is what makes a deleted FIRST row detectable — without it, lopping the
/// head off a log would verify as a shorter but intact chain.
pub fn verify_chain(rows: &[AuditRow]) -> ChainVerdict {
    let mut expected_prev = GENESIS_PREV_HASH.to_string();
    for (index, row) in rows.iter().enumerate() {
        // The link. A removed row shows up here: the survivor still carries the
        // vanished row's hash in `prev_hash`, which no longer matches the row now
        // in front of it.
        if row.prev_hash != expected_prev {
            return ChainVerdict::BrokenAt(index);
        }
        // The row's own contents. An edited column shows up here even though the
        // links on both sides still reconcile.
        if row.hash != chain_hash(&row.prev_hash, &row.as_draft()) {
            return ChainVerdict::BrokenAt(index);
        }
        expected_prev = row.hash.clone();
    }
    ChainVerdict::Intact
}

/// A shape that must never reach `audit_events`, with the reason it is barred.
///
/// The variants are reported by CODE only. The offending value is never carried
/// into the error — an error payload that echoes a JMBG back to the UI has
/// already done the leaking the filter exists to prevent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForbiddenShape {
    /// Names, addresses and search queries carry whitespace; internal ids and
    /// code constants do not. A query for a customer's name IS personal data
    /// about that customer (req. 4).
    Razmak,
    /// A thirteen-digit run. An EAN-13 barcode shares the shape and is refused
    /// with it — this log references rows by internal id, so losing the
    /// occasional barcode costs far less than admitting a matični broj.
    Jmbg,
    /// A 13–19 digit run that satisfies Luhn: the card PAN shape. ZZPL aside,
    /// a PAN in an application log is a PCI-DSS finding on its own.
    KarticaPan,
    /// Nine digits or more. Nothing this log legitimately references is that
    /// long, and a telephone number is (req. 4).
    DugackiNizCifara,
}

impl ForbiddenShape {
    fn code(self) -> &'static str {
        match self {
            Self::Razmak => "razmak",
            Self::Jmbg => "jmbg",
            Self::KarticaPan => "kartica_pan",
            Self::DugackiNizCifara => "dugacak_niz_cifara",
        }
    }

    fn message(self) -> &'static str {
        match self {
            Self::Razmak => concat!(
                "Vrednost sa razmakom ne sme da uđe u evidenciju pristupa: imena, ",
                "adrese i tekst pretrage nose razmak, interne oznake ne ",
                "(ZZPL čl. 5 st. 1 t. 3)."
            ),
            Self::Jmbg => concat!(
                "Vrednost oblika JMBG ne sme da uđe u evidenciju pristupa ",
                "(ZZPL čl. 5 st. 1 t. 3)."
            ),
            Self::KarticaPan => concat!(
                "Vrednost oblika broja platne kartice ne sme da uđe u evidenciju ",
                "pristupa (ZZPL čl. 5 st. 1 t. 3)."
            ),
            Self::DugackiNizCifara => concat!(
                "Predugačak niz cifara — telefon ili sličan lični podatak — ne sme ",
                "da uđe u evidenciju pristupa (ZZPL čl. 5 st. 1 t. 3)."
            ),
        }
    }
}

/// Every maximal run of ASCII digits in `value`.
fn digit_runs(value: &str) -> Vec<&str> {
    let mut runs = Vec::new();
    let mut start: Option<usize> = None;
    for (index, byte) in value.bytes().enumerate() {
        match (byte.is_ascii_digit(), start) {
            (true, None) => start = Some(index),
            (false, Some(from)) => {
                runs.push(&value[from..index]);
                start = None;
            }
            _ => {}
        }
    }
    if let Some(from) = start {
        runs.push(&value[from..]);
    }
    runs
}

/// The Luhn check digit, the only cheap way to tell a card PAN from a long
/// internal number. SQLite cannot run it, which is why this half of the
/// exclusion list lives here and not in the v18 CHECK constraints.
fn passes_luhn(digits: &str) -> bool {
    let mut sum: u32 = 0;
    for (position, byte) in digits.bytes().rev().enumerate() {
        let mut digit = u32::from(byte - b'0');
        if position % 2 == 1 {
            digit *= 2;
            if digit > 9 {
                digit -= 9;
            }
        }
        sum += digit;
    }
    sum.is_multiple_of(10)
}

fn forbidden_shape(value: &str) -> Option<ForbiddenShape> {
    if value.chars().any(char::is_whitespace) {
        return Some(ForbiddenShape::Razmak);
    }
    for run in digit_runs(value) {
        let length = run.len();
        if length == 13 {
            return Some(ForbiddenShape::Jmbg);
        }
        if (13..=19).contains(&length) && passes_luhn(run) {
            return Some(ForbiddenShape::KarticaPan);
        }
        if length >= 9 {
            return Some(ForbiddenShape::DugackiNizCifara);
        }
    }
    None
}

/// The write boundary. Every path that appends to `audit_events` calls this
/// first; the exclusion list is code, not convention (req. 4).
///
/// Two kinds of rule, one gate:
///
/// 1. **Čl. 48 st. 2 completeness** — an `uvid` or `otkrivanje` without a
///    *razlog*, or an `otkrivanje` without a class of *primalac*, is not the
///    record the article describes. Migration v18 carries the same two CHECKs;
///    they are repeated here so the refusal reaches the operator as a sentence
///    rather than as a raw SQLite constraint failure.
/// 2. **The čl. 5 st. 1 t. 3 exclusion list** — a shape scan over the only two
///    textual fields the draft has. There is no value payload, no note and no
///    query field to scan, because [`AuditDraft`] has none.
pub fn reject_forbidden_content(draft: &AuditDraft) -> Result<(), AppError> {
    if draft.action.requires_reason() && draft.reason_code.is_none() {
        return Err(AppError::business(
            "audit_missing_reason",
            format!(
                "Radnja „{}“ ne može da se upiše u evidenciju pristupa bez razloga (ZZPL čl. 48 st. 2).",
                draft.action.as_code()
            ),
        ));
    }

    if draft.action == AuditAction::Otkrivanje && draft.recipient.is_none() {
        return Err(AppError::business(
            "audit_missing_recipient",
            "Otkrivanje ne može da se upiše u evidenciju pristupa bez klase primaoca (ZZPL čl. 48 st. 2).",
        ));
    }

    for (field, value) in [
        ("object_type", draft.object_type.as_str()),
        ("object_id", draft.object_id.as_str()),
    ] {
        if let Some(shape) = forbidden_shape(value) {
            return Err(AppError::business_with_details(
                "audit_forbidden_content",
                shape.message(),
                // Field name and shape code only — never the value.
                serde_json::json!({ "polje": field, "oblik": shape.code() }),
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture chain the chain tests walk. `row(t, id)` re-derives every
    /// earlier row of this table, so a plain `vec![row("a", 1), row("b", 2),
    /// row("c", 3)]` is a genuinely linked chain and not three unrelated rows.
    const FIXTURE: [(&str, i64); 3] = [("a", 1), ("b", 2), ("c", 3)];

    fn fixture_draft(object_type: &str, id: i64) -> AuditDraft {
        AuditDraft {
            at: format!("2026-08-01T09:0{id}:00Z"),
            actor_user_id: Some(800),
            action: AuditAction::Unos,
            object_type: object_type.to_string(),
            object_id: id.to_string(),
            reason_code: None,
            recipient: None,
            support_session_id: None,
        }
    }

    fn row(object_type: &str, id: i64) -> AuditRow {
        let mut prev_hash = GENESIS_PREV_HASH.to_string();
        for (earlier_type, earlier_id) in FIXTURE {
            if earlier_id >= id {
                break;
            }
            prev_hash = chain_hash(&prev_hash, &fixture_draft(earlier_type, earlier_id));
        }
        let draft = fixture_draft(object_type, id);
        let hash = chain_hash(&prev_hash, &draft);
        AuditRow {
            id,
            at: draft.at,
            actor_user_id: draft.actor_user_id,
            action: draft.action,
            object_type: draft.object_type,
            object_id: draft.object_id,
            reason_code: draft.reason_code,
            recipient: draft.recipient,
            support_session_id: draft.support_session_id,
            prev_hash,
            hash,
        }
    }

    fn draft_with(action: AuditAction, reason: Option<AuditReason>) -> AuditDraft {
        AuditDraft {
            at: "2026-08-01T09:00:00Z".to_string(),
            actor_user_id: Some(800),
            action,
            object_type: "employee".to_string(),
            object_id: "3".to_string(),
            reason_code: reason,
            // Present so that the otkrivanje leg below fails on the missing
            // razlog and on nothing else.
            recipient: match action {
                AuditAction::Otkrivanje => Some(AuditRecipient::Inspekcija),
                _ => None,
            },
            support_session_id: None,
        }
    }

    fn draft_with_object_id(object_id: &str) -> AuditDraft {
        AuditDraft {
            object_id: object_id.to_string(),
            ..draft_with(AuditAction::Unos, None)
        }
    }

    /// A search has no column of its own — and that is deliberate. The only way a
    /// query text can reach the table is disguised as the opaque id, so that is
    /// where the boundary has to catch it.
    fn draft_with_query(query: &str) -> AuditDraft {
        AuditDraft {
            object_type: "pretraga".to_string(),
            object_id: query.to_string(),
            ..draft_with(AuditAction::Uvid, Some(AuditReason::InterniNadzor))
        }
    }

    #[test]
    fn the_chain_detects_a_tampered_row() {
        let rows = vec![row("a", 1), row("b", 2), row("c", 3)];
        assert_eq!(verify_chain(&rows), ChainVerdict::Intact);

        let mut tampered = rows.clone();
        tampered[1].object_id = "999".to_string();
        assert!(matches!(verify_chain(&tampered), ChainVerdict::BrokenAt(1)));
    }

    #[test]
    fn a_deleted_row_breaks_the_chain_too() {
        let rows = vec![row("a", 1), row("b", 2), row("c", 3)];
        let mut gapped = rows.clone();
        gapped.remove(1);
        assert!(matches!(verify_chain(&gapped), ChainVerdict::BrokenAt(_)));
    }

    /// Lopping the head off a log must not verify as a shorter but intact chain —
    /// that is the one deletion a link-only walk cannot see.
    #[test]
    fn a_deleted_genesis_row_breaks_the_chain() {
        let rows = vec![row("a", 1), row("b", 2), row("c", 3)];
        let mut beheaded = rows.clone();
        beheaded.remove(0);
        assert!(matches!(verify_chain(&beheaded), ChainVerdict::BrokenAt(0)));
    }

    /// The support session link is čl. 48 st. 2 evidence about WHO was inside the
    /// shop's data. If it sits outside the digest it can be cut off a row in
    /// silence, so the chain must cover it.
    #[test]
    fn the_support_session_link_is_covered_by_the_chain() {
        let draft = draft_with(AuditAction::Unos, None);
        let linked = AuditDraft {
            support_session_id: Some(7),
            ..draft.clone()
        };
        assert_ne!(chain_hash("", &draft), chain_hash("", &linked));
    }

    /// ZZPL čl. 48 st. 2 requires a *razlog* for access and disclosure.
    #[test]
    fn uvid_and_otkrivanje_require_a_reason_code() {
        for action in [AuditAction::Uvid, AuditAction::Otkrivanje] {
            let draft = draft_with(action, None);
            assert!(
                reject_forbidden_content(&draft).is_err(),
                "{action:?} needs a razlog"
            );
        }
        for action in [AuditAction::Unos, AuditAction::Menjanje] {
            let draft = draft_with(action, None);
            assert!(reject_forbidden_content(&draft).is_ok());
        }
    }

    /// Čl. 48 st. 2 asks *who received the data*. An otkrivanje that does not
    /// answer it is not the record the article describes.
    #[test]
    fn otkrivanje_requires_a_recipient_class() {
        let draft = AuditDraft {
            recipient: None,
            ..draft_with(AuditAction::Otkrivanje, Some(AuditReason::ZakonskaObaveza))
        };
        assert!(reject_forbidden_content(&draft).is_err());

        let named = draft_with(AuditAction::Otkrivanje, Some(AuditReason::ZakonskaObaveza));
        assert!(reject_forbidden_content(&named).is_ok());
    }

    /// A log that accumulates personal data breaches čl. 5 st. 1 t. 3 and
    /// enlarges the very attack surface it exists to shrink.
    #[test]
    fn a_jmbg_shaped_value_is_refused_at_the_write_boundary() {
        let draft = draft_with_object_id("1234567890123"); // 13 digits
        let error =
            reject_forbidden_content(&draft).expect_err("a JMBG-shaped id must not reach the log");
        assert!(matches!(
            error,
            AppError::Business {
                code: "audit_forbidden_content",
                ..
            }
        ));
    }

    /// The refusal must not become the leak. An error message or detail payload
    /// that echoes the JMBG back has already put it in front of the UI and into
    /// whatever records the failure.
    #[test]
    fn the_refusal_never_echoes_the_forbidden_value() {
        for shaped in ["1234567890123", "4111111111111111", "Petar Petrović"] {
            let draft = draft_with_object_id(shaped);
            let error = reject_forbidden_content(&draft).expect_err("must be refused");
            let AppError::Business {
                message, details, ..
            } = &error
            else {
                panic!("expected a business error, got {error:?}");
            };
            assert!(
                !message.contains(shaped),
                "the message echoed {shaped}: {message}"
            );
            let rendered = serde_json::to_string(details).expect("details serialize");
            assert!(
                !rendered.contains(shaped),
                "the details echoed {shaped}: {rendered}"
            );
        }
    }

    #[test]
    fn a_search_query_string_is_refused() {
        // A query for a customer's name IS personal data about that customer.
        let draft = draft_with_query("Petar Petrović");
        assert!(reject_forbidden_content(&draft).is_err());
    }

    #[test]
    fn a_card_pan_shaped_value_is_refused() {
        let draft = draft_with_object_id("4111111111111111");
        assert!(reject_forbidden_content(&draft).is_err());
    }

    /// Req. 4 names the phone number alongside the JMBG and the PAN. Nothing this
    /// log legitimately references is nine digits long, so the run length is the
    /// whole test — no attempt to recognise a dialling plan.
    #[test]
    fn a_phone_shaped_value_is_refused() {
        for shaped in ["0641234567", "+381641234567", "381641234567"] {
            let draft = draft_with_object_id(shaped);
            assert!(
                reject_forbidden_content(&draft).is_err(),
                "{shaped} must not reach the log"
            );
        }
    }

    /// Each rule must earn its place. The nine-digit catch-all refuses a PAN and
    /// a JMBG on its own, so without pinning the reported shape the Luhn check and
    /// the thirteen-digit check could both be deleted and the suite stay green —
    /// and the operator would be told „predugačak niz cifara“ about a card number.
    #[test]
    fn each_forbidden_shape_is_reported_by_its_own_code() {
        for (value, expected) in [
            ("Petar Petrović", "razmak"),
            ("1234567890123", "jmbg"),
            ("4111111111111111", "kartica_pan"),
            ("0641234567", "dugacak_niz_cifara"),
        ] {
            let draft = draft_with_object_id(value);
            let error = reject_forbidden_content(&draft).expect_err("must be refused");
            let AppError::Business {
                details: Some(details),
                ..
            } = &error
            else {
                panic!("expected details on {value}, got {error:?}");
            };
            assert_eq!(
                details.get("oblik").and_then(serde_json::Value::as_str),
                Some(expected),
                "wrong shape reported for {value}"
            );
        }
    }

    /// The exclusions must not swallow the ids the log exists to reference.
    #[test]
    fn an_opaque_internal_id_passes() {
        for clean in ["3", "10422", "2026-000117"] {
            let draft = draft_with_object_id(clean);
            assert!(
                reject_forbidden_content(&draft).is_ok(),
                "{clean} is an internal id and must pass"
            );
        }
    }

    /// The object TYPE is scanned too: it was the last free-text hole on the
    /// table before v18 gave it the same shape as the id.
    #[test]
    fn a_name_smuggled_into_the_object_type_is_refused() {
        let draft = AuditDraft {
            object_type: "Marko Marković".to_string(),
            ..draft_with(AuditAction::Unos, None)
        };
        assert!(reject_forbidden_content(&draft).is_err());
    }

    /// Every code must round-trip, because the codes are the contract with the
    /// migration v18 CHECK lists — a drift here is a runtime constraint failure.
    #[test]
    fn every_code_round_trips_and_matches_the_v18_check_lists() {
        let actions: Vec<&str> = AuditAction::ALL.iter().map(|a| a.as_code()).collect();
        assert_eq!(
            actions,
            vec![
                "unos",
                "menjanje",
                "uvid",
                "otkrivanje",
                "uporedjivanje",
                "brisanje"
            ]
        );
        for action in AuditAction::ALL {
            assert_eq!(AuditAction::from_code(action.as_code()), Some(action));
        }
        assert_eq!(AuditAction::from_code("izmisljeno"), None);

        let reasons: Vec<&str> = AuditReason::ALL.iter().map(|r| r.as_code()).collect();
        assert_eq!(
            reasons,
            vec![
                "inspekcija",
                "zahtev_lica",
                "interni_nadzor",
                "obrada_reklamacije",
                "obracun_zarade",
                "tehnicka_podrska",
                "sudski_ili_upravni_postupak",
                "bezbednosni_incident",
                "zakonska_obaveza",
                "automatsko_ciscenje"
            ]
        );
        for reason in AuditReason::ALL {
            assert_eq!(AuditReason::from_code(reason.as_code()), Some(reason));
        }

        let recipients: Vec<&str> = AuditRecipient::ALL.iter().map(|r| r.as_code()).collect();
        assert_eq!(
            recipients,
            vec![
                "lice_na_koje_se_podaci_odnose",
                "poreska_uprava",
                "inspekcija",
                "poverenik",
                "sud_ili_javni_tuzilac",
                "mup",
                "knjigovodja",
                "obradjivac_tehnicke_podrske",
                "banka",
                "drugi_organ"
            ]
        );
        for recipient in AuditRecipient::ALL {
            assert_eq!(
                AuditRecipient::from_code(recipient.as_code()),
                Some(recipient)
            );
        }
    }
}
