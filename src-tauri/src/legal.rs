//! The single source of every statutory fine figure in the application.
//!
//! Every amount here is tier-resolved from `ShopProfile::pravna_forma`. A
//! preduzetnik cannot commit a privredni prestup at all (Zakon o privrednim
//! prestupima čl. 6 st. 1), so quoting the pravno-lice tier to one is not a
//! rounding error — it is a false statement of the law. When the legal form is
//! unknown we render no figure rather than a plausible one.
//!
//! Verified against primary text on 31.07.2026 — see
//! `docs/SW11-SW15-VERIFIED-RULES.md` §2.
//!
//! Rendered by the AML, cash-deposit and declaration surfaces (Tasks 8, 13 and
//! 19), so `dead_code` is allowed here until that wiring lands — mirroring the
//! other domain modules.

#![allow(dead_code)]

use serde::Serialize;

use crate::commands::settings::{PravnaForma, ShopProfile};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LegalNotice {
    /// What the duty is, in plain Serbian.
    pub summary: String,
    /// The tier-correct penalty, or `None` when the legal form is unset.
    pub penalty: Option<String>,
    /// Article reference, so the operator can hand it to an inspector.
    pub citation: String,
    /// `false` marks a prudential practice. The UI must not call it an obaveza.
    pub is_legal_duty: bool,
}

fn tiered(profile: &ShopProfile, preduzetnik: &str, pravno_lice: &str) -> Option<String> {
    match profile.pravna_forma {
        Some(PravnaForma::Preduzetnik) => Some(preduzetnik.to_string()),
        Some(PravnaForma::PravnoLice) => Some(pravno_lice.to_string()),
        None => None,
    }
}

/// AML čl. 46 st. 1 — receiving cash of 10.000 EUR or more.
pub fn aml_cash_cap(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "Zabranjeno je primiti gotovinu u iznosu od 10.000 evra ili više \
                  u dinarskoj protivvrednosti. Iznos se mora uplatiti na tekući račun."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna od 100.000 do 300.000 dinara (čl. 120 st. 2), \
             a u srazmeri s vrednošću robe i više (čl. 120 st. 8).",
            "Privredni prestup: novčana kazna od 100.000 do 2.000.000 dinara \
             (čl. 118 st. 1 tač. 43), uz kaznu za odgovorno lice od 10.000 do 150.000 dinara \
             (čl. 118 st. 2).",
        ),
        citation: "Zakon o sprečavanju pranja novca i finansiranja terorizma, čl. 46 st. 1. \
                   Nadzor: tržišna inspekcija (čl. 110 st. 6)."
            .to_string(),
        is_legal_duty: true,
    }
}

/// Zakon 68/2015 čl. 3 st. 1 — deposit of cash received on any basis.
pub fn cash_deposit_duty(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "Dinare primljene u gotovom po bilo kom osnovu treba uplatiti na tekući \
                  račun u roku od sedam radnih dana."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna od 10.000 do 500.000 dinara (čl. 7 st. 3).",
            "Prekršaj: novčana kazna od 50.000 do 2.000.000 dinara (čl. 7 st. 1 tač. 2), \
             uz kaznu za odgovorno lice od 5.000 do 150.000 dinara (čl. 7 st. 2).",
        ),
        citation: "Zakon o obavljanju plaćanja pravnih lica, preduzetnika i fizičkih lica \
                   koja ne obavljaju delatnost (Sl. glasnik RS, br. 68/2015), čl. 3 st. 1. \
                   Nadzor: Poreska uprava (čl. 6)."
            .to_string(),
        is_legal_duty: true,
    }
}

/// ZoT čl. 68 st. 1 tač. 9 — selling goods with NO declaration. The severe tier.
pub fn declaration_missing(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "Prodaja robe bez deklaracije. Deklaraciju obezbeđuje proizvođač, \
                  odnosno uvoznik, ali za prodaju takve robe odgovara trgovac."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna od 50.000 do 500.000 dinara (čl. 68 st. 3), \
             uz moguću zaštitnu meru zabrane vršenja delatnosti od šest meseci do dve godine \
             (čl. 68 st. 6).",
            "Prekršaj: novčana kazna od 500.000 do 2.000.000 dinara (čl. 68 st. 1), \
             uz kaznu za odgovorno lice od 50.000 do 150.000 dinara (čl. 68 st. 2) i moguću \
             zaštitnu meru zabrane vršenja delatnosti (čl. 68 st. 6).",
        ),
        citation: "Zakon o trgovini, čl. 34 st. 1–2, čl. 68 st. 1 tač. 9.".to_string(),
        is_legal_duty: true,
    }
}

/// ZoT čl. 67 st. 1 tač. 6 — selling with a defective declaration. Fixed, lower.
pub fn declaration_defective(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "Prodaja robe sa neurednom ili nepropisnom deklaracijom. Podaci iz \
                  deklaracije ne smeju da se menjaju ni uklanjaju u maloprodaji."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna u fiksnom iznosu od 40.000 dinara (čl. 67 st. 3). \
             Zaštitne mere nisu propisane.",
            "Prekršaj: novčana kazna u fiksnom iznosu od 100.000 dinara (čl. 67 st. 1), \
             uz kaznu za odgovorno lice od 10.000 dinara (čl. 67 st. 2). \
             Zaštitne mere nisu propisane.",
        ),
        citation: "Zakon o trgovini, čl. 34 st. 3–4, čl. 67 st. 1 tač. 6.".to_string(),
        is_legal_duty: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::settings::{PravnaForma, ShopProfile};

    fn profile(forma: Option<PravnaForma>) -> ShopProfile {
        ShopProfile {
            pravna_forma: forma,
            ..ShopProfile::default()
        }
    }

    /// Every public notice, under the preduzetnik regime, in one place. Adding a
    /// new copy function without adding it here is a visible omission in review.
    fn all_notices(p: &ShopProfile) -> Vec<LegalNotice> {
        vec![
            aml_cash_cap(p),
            cash_deposit_duty(p),
            declaration_missing(p),
            declaration_defective(p),
        ]
    }

    #[test]
    fn preduzetnik_never_sees_privredni_prestup_or_the_pravno_lice_figures() {
        let p = profile(Some(PravnaForma::Preduzetnik));

        for notice in all_notices(&p) {
            let rendered = format!(
                "{} {} {}",
                notice.summary,
                notice.penalty.clone().unwrap_or_default(),
                notice.citation
            );
            // Case-insensitive: the pravno-lice copy opens the sentence with
            // "Privredni prestup", so a case-sensitive guard would wave the
            // capitalised form straight through.
            let haystack = rendered.to_lowercase();
            for forbidden in ["privredni prestup", "2.000.000", "300.000,00 do 2.000.000"] {
                assert!(
                    !haystack.contains(forbidden),
                    "preduzetnik copy must not contain {forbidden:?}; got: {rendered}"
                );
            }
        }
    }

    #[test]
    fn unset_legal_form_renders_no_figure_at_all() {
        let p = profile(None);

        for notice in all_notices(&p) {
            assert!(
                notice.penalty.is_none(),
                "an UNSET legal form must render no penalty figure: {:?}",
                notice.penalty
            );
        }
    }

    #[test]
    fn aml_penalty_is_tier_correct_and_does_not_present_a_base_max_as_a_ceiling() {
        let preduzetnik = aml_cash_cap(&profile(Some(PravnaForma::Preduzetnik)));
        let penalty = preduzetnik.penalty.expect("preduzetnik penalty is known");
        assert!(penalty.contains("100.000"), "base minimum: {penalty}");
        assert!(penalty.contains("300.000"), "base maximum: {penalty}");
        assert!(
            penalty.contains("i više"),
            "300.000 is a base maximum, not a ceiling (čl. 120 st. 8): {penalty}"
        );
        assert!(preduzetnik.is_legal_duty);

        let pravno = aml_cash_cap(&profile(Some(PravnaForma::PravnoLice)));
        let penalty = pravno.penalty.expect("pravno lice penalty is known");
        assert!(
            penalty.to_lowercase().contains("privredni prestup"),
            "{penalty}"
        );
        assert!(penalty.contains("2.000.000"), "{penalty}");
    }

    #[test]
    fn cash_deposit_tiers_are_the_verified_ranges_not_the_odgovorno_lice_row() {
        // Zakon 68/2015 čl. 7 carries three rows, and the preduzetnik one sits
        // between the other two. Mis-copying the odgovorno-lice row (st. 2) onto
        // the preduzetnik is invisible to the forbidden-substring guard, because
        // it quotes neither "privredni prestup" nor "2.000.000".
        let preduzetnik = cash_deposit_duty(&profile(Some(PravnaForma::Preduzetnik)));
        let penalty = preduzetnik.penalty.expect("preduzetnik penalty is known");
        assert!(
            penalty.contains("10.000 do 500.000"),
            "preduzetnik range is čl. 7 st. 3: {penalty}"
        );
        assert!(penalty.contains("čl. 7 st. 3"), "{penalty}");
        assert!(
            !penalty.contains("čl. 7 st. 2"),
            "st. 2 is the odgovorno-lice row; a preduzetnik has no odgovorno lice: {penalty}"
        );
        assert!(
            !penalty.contains("5.000 do 150.000"),
            "the odgovorno-lice range must never be quoted to a preduzetnik: {penalty}"
        );
        assert!(preduzetnik.is_legal_duty);

        let pravno = cash_deposit_duty(&profile(Some(PravnaForma::PravnoLice)));
        let penalty = pravno.penalty.expect("pravno lice penalty is known");
        assert!(penalty.contains("50.000 do 2.000.000"), "{penalty}");
        assert!(penalty.contains("čl. 7 st. 1 tač. 2"), "{penalty}");
        assert!(
            penalty.contains("5.000 do 150.000") && penalty.contains("čl. 7 st. 2"),
            "the odgovorno-lice row belongs on the pravno-lice tier: {penalty}"
        );
        assert!(
            !penalty.to_lowercase().contains("privredni prestup"),
            "68/2015 contains no privredni prestup at all: {penalty}"
        );
    }

    #[test]
    fn defective_declaration_fixed_sums_are_tier_correct() {
        // ZoT čl. 67 prescribes fixed amounts, not ranges — the word "fiksnom"
        // is what puts it in prekršajni-nalog territory (ZoP čl. 168), so it is
        // load-bearing copy, not decoration.
        let preduzetnik = declaration_defective(&profile(Some(PravnaForma::Preduzetnik)));
        let penalty = preduzetnik.penalty.expect("preduzetnik penalty is known");
        assert!(penalty.contains("40.000"), "čl. 67 st. 3: {penalty}");
        assert!(
            penalty.contains("fiksnom"),
            "a fixed sum, not a range: {penalty}"
        );
        assert!(
            !penalty.contains("100.000"),
            "100.000 is the pravno-lice sum (čl. 67 st. 1): {penalty}"
        );

        let pravno = declaration_defective(&profile(Some(PravnaForma::PravnoLice)));
        let penalty = pravno.penalty.expect("pravno lice penalty is known");
        assert!(penalty.contains("100.000"), "čl. 67 st. 1: {penalty}");
        assert!(
            penalty.contains("fiksnom"),
            "a fixed sum, not a range: {penalty}"
        );
        assert!(
            penalty.contains("10.000"),
            "odgovorno lice, čl. 67 st. 2: {penalty}"
        );
    }

    #[test]
    fn declaration_notices_separate_the_two_offences_and_carry_the_ban() {
        let p = profile(Some(PravnaForma::Preduzetnik));

        let missing = declaration_missing(&p);
        let missing_penalty = missing.penalty.expect("known");
        assert!(missing_penalty.contains("50.000"), "{missing_penalty}");
        assert!(missing_penalty.contains("500.000"), "{missing_penalty}");
        assert!(
            missing_penalty.contains("zabran"),
            "the 6mo-2yr activity ban is the sanction that closes the shop: {missing_penalty}"
        );

        let defective = declaration_defective(&p);
        let defective_penalty = defective.penalty.expect("known");
        assert!(defective_penalty.contains("40.000"), "{defective_penalty}");
        assert!(
            !defective_penalty.contains("zabran"),
            "čl. 67 prescribes no zaštitne mere: {defective_penalty}"
        );
    }
}
