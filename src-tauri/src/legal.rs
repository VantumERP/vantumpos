//! The single source of every statutory fine figure in the application.
//!
//! Every amount here is tier-resolved from `ShopProfile::pravna_forma`. A
//! preduzetnik cannot commit a privredni prestup at all (Zakon o privrednim
//! prestupima čl. 6 st. 1), so quoting the pravno-lice tier to one is not a
//! rounding error — it is a false statement of the law. When the legal form is
//! unknown we render no figure rather than a plausible one.
//!
//! Verified against primary text on 31.07.2026 — see
//! `docs/SW11-SW15-VERIFIED-RULES.md` §2. The two ZoR working-time notices are
//! verified against `docs/SW14-VERIFIED-RULES.md` §3 W1, and the ZZP čl. 6
//! cenovnik notice against `docs/REMAINING-SW-VERIFIED-RULES.md` §3 V2 and §2b.
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

/// ZF čl. 6 st. 4 — at least one L-PFR in every business premises.
///
/// The statute never licenses a V-PFR-only setup for a physical shop: čl. 6
/// st. 3 joins the two device kinds with „i/ili“, and st. 4 excuses the L-PFR
/// floor for exactly two obligors — retail conducted **exclusively** over the
/// internet, and retail of the obligor's **own used** movable assets. Deciding
/// whether either carve-out applies is the shop's answer to give, so this
/// function only states the duty and resolves the tier; the surface decides
/// whether to show it.
///
/// ZF prescribes prekršaji only — it contains no privredni prestup for any
/// tier — and the preduzetnik row is čl. 15 **st. 3**, not st. 1.
pub fn lpfr_required(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "U svakom poslovnom prostoru i poslovnoj prostoriji mora da radi najmanje \
                  jedan lokalni procesor fiskalnih računa (L-PFR) — uređaj koji izdaje račun \
                  i bez interneta. Zakon izuzima samo obveznika koji promet na malo obavlja \
                  isključivo putem interneta i obveznika koji obavlja promet na malo \
                  sopstvenih korišćenih pokretnih materijalnih sredstava."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna od 50.000 do 500.000 dinara (čl. 15 st. 3).",
            "Prekršaj: novčana kazna od 300.000 do 2.000.000 dinara (čl. 15 st. 1).",
        ),
        citation: "Zakon o fiskalizaciji, čl. 6 st. 4; prekršaj: čl. 15 st. 1 tač. 4.".to_string(),
        is_legal_duty: true,
    }
}

/// ZoR čl. 55 st. 6 — the daily overtime register.
pub fn overtime_record_missing(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "Poslodavac je dužan da vodi dnevnu evidenciju o prekovremenom radu \
                  zaposlenih."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna od 50.000 do 150.000 dinara \
             (čl. 276 st. 1 u vezi sa tač. 1a).",
            "Prekršaj: novčana kazna od 150.000 do 300.000 dinara (čl. 276 st. 1 tač. 1a), \
             uz kaznu za odgovorno lice od 10.000 do 20.000 dinara (čl. 276 st. 2).",
        ),
        citation: "Zakon o radu, čl. 55 st. 6. Nadzor: inspektor rada. \
                   Ovi članovi ne propisuju zaštitnu meru."
            .to_string(),
        is_legal_duty: true,
    }
}

/// ZoR čl. 53 — the overtime caps. This is the LARGER exposure, ~2.7× the
/// missing-register fine, and it is why the cap checks are the feature.
pub fn overtime_caps_exceeded(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "Prekovremeni rad ne može trajati duže od osam časova nedeljno, \
                  niti ukupno radno vreme sa prekovremenim duže od 12 časova dnevno."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna od 200.000 do 400.000 dinara \
             (čl. 274 st. 1 tač. 3 u vezi sa st. 2).",
            "Prekršaj: novčana kazna od 600.000 do 1.500.000 dinara (čl. 274 st. 1 tač. 3), \
             uz kaznu za odgovorno lice od 30.000 do 150.000 dinara (čl. 274 st. 3).",
        ),
        citation: "Zakon o radu, čl. 53 st. 2 i st. 3. Nadzor: inspektor rada. \
                   Ovi članovi ne propisuju zaštitnu meru."
            .to_string(),
        is_legal_duty: true,
    }
}

/// ZoR čl. 57 st. 5 — the preraspodela ceiling, sixty hours a week.
///
/// A separate notice from [`overtime_caps_exceeded`], because it is a separate
/// rule set and a separate offence. čl. 58 says hours worked in preraspodela are
/// **not** prekovremeni rad, so čl. 53 st. 2 (eight hours of overtime a week)
/// and čl. 53 st. 3 (twelve hours a day in total) do not bind an employee in
/// preraspodela at all — quoting them at him states a rule that does not apply.
/// And the offence article differs: čl. 53 is čl. 274 st. 1 tač. 3, while čl. 57
/// and čl. 60 are **tač. 4**.
///
/// Both tačke resolve through the same st. 2 for a preduzetnik, so the amount is
/// identical either way. That is precisely why the wrong tačka is easy to ship
/// and invisible to a figure-only guard — the damage is a wrong article and an
/// inapplicable statement of the law in the operator's hands.
pub fn preraspodela_caps_exceeded(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "U slučaju preraspodele radnog vremena, radno vreme ne može da traje duže \
                  od 60 časova nedeljno. Časovi ostvareni u preraspodeli ne smatraju se \
                  prekovremenim radom (čl. 58)."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna od 200.000 do 400.000 dinara \
             (čl. 274 st. 1 tač. 4 u vezi sa st. 2).",
            "Prekršaj: novčana kazna od 600.000 do 1.500.000 dinara (čl. 274 st. 1 tač. 4), \
             uz kaznu za odgovorno lice od 30.000 do 150.000 dinara (čl. 274 st. 3).",
        ),
        citation: "Zakon o radu, čl. 57 st. 5. Nadzor: inspektor rada. \
                   Ovi članovi ne propisuju zaštitnu meru."
            .to_string(),
        is_legal_duty: true,
    }
}

/// ZZPL čl. 52 — the breach notification, and the one figure SW-17 may print.
///
/// **The penalised act is the failure to NOTIFY.** Čl. 95 st. 1 tač. 24
/// describes its biće as *„ne obavesti Poverenika o povredi bezbednosti
/// podataka“*, and a preduzetnik is reached once, by st. 4. Čl. 95 st. 5 opens
/// with a standalone *fizičko lice* limb rather than reaching only an odgovorno
/// lice u pravnom licu, so it is not a harmless extra sentence on the
/// preduzetnik tier — quoting it would fine the same person twice for one act.
///
/// **The internal čl. 52 st. 6 record carries NO figure here** (req. 50). Not
/// keeping it is a different act from not notifying, and under ZoP čl. 3 plus
/// strict construction of prekršajne norme it is genuinely unsettled whether the
/// trailing *„suprotno članu 52. ovog zakona“* reaches it; no case law and no
/// Poverenik mišljenje resolve it. So the summary says the amount is not clearly
/// prescribed — and stops there. It must not say there is no consequence: čl. 79
/// st. 2 tač. 2 and tač. 4 give the Poverenik an opomena and a binding nalog
/// regardless of any fine, the Poverenik scores st. 6 as a standalone audited
/// item (KL-001 pitanje 6), and because st. 7 makes that documentation the
/// vehicle for proving compliance, a missing record destroys the defence to the
/// tač. 24 charge — which converts straight back into the figure above.
///
/// The deadline in the summary is the Pravilnik 40/2019 čl. 3 one: a flat 72 h
/// od saznanja, without the statute's *„bez nepotrebnog odlaganja, ili, ako je
/// to moguće“* softener. It is the number any countdown in this app measures.
pub fn breach_notification_missing(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "Rukovalac je dužan da o povredi podataka o ličnosti koja može da proizvede \
                  rizik po prava i slobode fizičkih lica obavesti Poverenika bez nepotrebnog \
                  odlaganja, a najkasnije u roku od 72 časa od saznanja za povredu. Ako ne \
                  postupi u tom roku, dužan je da obrazloži zašto. Kazna je propisana za \
                  izostanak obaveštavanja. Za sam izostanak interne dokumentacije o povredi \
                  (čl. 52 st. 6) kazna nije jednoznačno propisana i ovde se ne navodi — ali \
                  bez te dokumentacije nema se čime dokazati da je po članu 52 postupljeno, \
                  a Poverenik može da izrekne opomenu i obavezujući nalog sa rokom."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna od 20.000 do 500.000 dinara \
             (čl. 95 st. 1 tač. 24 u vezi sa st. 4).",
            "Prekršaj: novčana kazna od 50.000 do 2.000.000 dinara (čl. 95 st. 1 tač. 24), \
             uz kaznu za odgovorno lice od 5.000 do 150.000 dinara (čl. 95 st. 5).",
        ),
        citation: "Zakon o zaštiti podataka o ličnosti, čl. 52 st. 1 i st. 2; interna \
                   dokumentacija: čl. 52 st. 6 i st. 7; obaveštavanje lica: čl. 53. \
                   Rok i obrazac: Pravilnik o obrascu obaveštavanja Poverenika o povredi \
                   podataka o ličnosti (Sl. glasnik RS, br. 40/2019), čl. 3 i čl. 2. \
                   Nadzor: Poverenik."
            .to_string(),
        is_legal_duty: true,
    }
}

/// ZZP čl. 6 — the machine-readable cenovnik, and the čl. 210 fixed fine.
///
/// Verified against `docs/REMAINING-SW-VERIFIED-RULES.md` §3 V2 and §2b.
///
/// **The preduzetnik sum is the fixed 100.000 of čl. 210 st. 3.** Čl. 210 st. 1
/// is the pravno-lice row and st. 2 the odgovorno-lice one, which čl. 210 st. 2
/// ties to a *pravno lice* and therefore never reaches him. ZZP contains zero
/// occurrences of *privredni prestup*, so the ZPP čl. 6 st. 1 trap does not
/// arise here and every tier is a prekršaj. Because every amount is fixed,
/// enforcement runs by prekršajni nalog from the tržišna inspekcija (ZoP čl. 168
/// st. 1) — which is what makes ZoP čl. 173 st. 1's half-within-eight-days the
/// number the shop actually pays.
///
/// **Čl. 210 was not in the čl. 220 carve-out.** That carve-out named čl. 4
/// st. 1 and čl. 6 only, so the duty has run since the law entered into force
/// while the offence provision waited out the three months. No copy may date the
/// shop's exposure to the first of May 2026.
///
/// **The no-website case is unresolved (§2b).** Čl. 6 st. 2 says *„na svojoj
/// internet stranici“* — a possessive presupposing a site — and no ZZP provision
/// obliges a trader to have one; extending a prekršaj to an unwritten duty to
/// create one runs into lex certa (ZoP čl. 3). The summary says so plainly,
/// because the alternative is telling a shop it is already in breach on a
/// question the text does not settle. What IS settled, and is stated first, is
/// that st. 2 draws no line at all — not at the trader's size and not at whether
/// he already has a site — so there is no carve-out to hide behind either.
pub fn cenovnik_not_published(profile: &ShopProfile) -> LegalNotice {
    LegalNotice {
        summary: "Trgovac je dužan da na svojoj internet stranici, posebno za svaki prodajni \
                  objekat, objavi cenovnik u digitalnom obliku pogodnom za automatsku obradu i \
                  da ga ažurira u realnom vremenu, kako bi odgovarao trenutnim cenama. \
                  U cenovniku se, kao i na prodajnom mestu, ističu prodajna i jedinična cena. \
                  Izuzetka od ove obaveze nema — ni prema veličini trgovca, ni prema tome da li \
                  trgovac ima internet stranicu. Ali zakon nigde ne propisuje obavezu trgovca da \
                  ima internet stranicu, pa za trgovca koji je nema nije razjašnjeno da li je \
                  dužan da je izradi: to pitanje nije rešeno ni podzakonskim aktom, ni \
                  mišljenjem, ni sudskom praksom."
            .to_string(),
        penalty: tiered(
            profile,
            "Prekršaj: novčana kazna u fiksnom iznosu od 100.000 dinara (čl. 210 st. 3). \
             Plaćanjem polovine — 50.000 dinara — u roku od osam dana od prijema prekršajnog \
             naloga prihvata se odgovornost za prekršaj i oslobađa plaćanja druge polovine \
             (Zakon o prekršajima, čl. 173 st. 1).",
            "Prekršaj: novčana kazna u fiksnom iznosu od 200.000 dinara (čl. 210 st. 1 tač. 1), \
             uz kaznu za odgovorno lice u pravnom licu u fiksnom iznosu od 50.000 dinara \
             (čl. 210 st. 2).",
        ),
        citation: "Zakon o zaštiti potrošača (Sl. glasnik RS, br. 35/2026), čl. 6 st. 1–3; \
                   prekršaj: čl. 210 st. 1 tač. 1. Čl. 6 se primenjuje od stupanja zakona na \
                   snagu, ali čl. 210 nije obuhvaćen izuzetkom iz čl. 220, pa se primenjuje tek \
                   po isteku tri meseca od tog dana. Nadzor: tržišna inspekcija — po čl. 207 \
                   tač. 1 inspektor najpre zapisnikom nalaže otklanjanje nepravilnosti u \
                   ostavljenom roku (čl. 206 st. 1), a ako se ne otkloni — donosi rešenje \
                   (čl. 206 st. 4). Ako se ni po rešenju ne postupi, inspektor rešenjem izriče \
                   privremenu zabranu prometa robe na koju se mera odnosi (čl. 206 st. 5), a \
                   nepostupanje po rešenju je i poseban prekršaj (čl. 209 st. 1 tač. 40). \
                   Zastarelost: čl. 213, dve godine od izvršenja."
            .to_string(),
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

    /// Every public notice, in one place. This list is what the
    /// forbidden-substring, UNSET and ZEOR guards iterate, so a notice function
    /// that is missing from it is an entirely unguarded fine figure. The
    /// asserted length in `every_notice_function_is_enumerated_in_the_guard`
    /// is what makes an omission fail rather than pass silently.
    fn all_notices(p: &ShopProfile) -> Vec<LegalNotice> {
        vec![
            aml_cash_cap(p),
            cash_deposit_duty(p),
            declaration_missing(p),
            declaration_defective(p),
            lpfr_required(p),
            overtime_record_missing(p),
            overtime_caps_exceeded(p),
            preraspodela_caps_exceeded(p),
            breach_notification_missing(p),
            cenovnik_not_published(p),
        ]
    }

    /// A notice that is not in `all_notices` is an unguarded fine figure,
    /// because neither the forbidden-substring guard nor the UNSET guard nor
    /// the ZEOR guard ever sees it.
    ///
    /// The containment loop below compares `all_notices` against a hand-written
    /// copy of the same list, so on its own it catches only a one-sided typo —
    /// never a function that was simply never added to either list. The
    /// asserted count is what closes that hole: a new notice cannot be
    /// introduced without this test being edited.
    #[test]
    fn every_notice_function_is_enumerated_in_the_guard() {
        let p = profile(Some(PravnaForma::Preduzetnik));
        let enumerated = all_notices(&p);

        assert_eq!(
            enumerated.len(),
            10,
            "adding a notice function means adding it to all_notices, to the \
             list below, AND bumping this count — an omission from both lists \
             is otherwise invisible"
        );

        for notice in [
            aml_cash_cap(&p),
            cash_deposit_duty(&p),
            declaration_missing(&p),
            declaration_defective(&p),
            lpfr_required(&p),
            overtime_record_missing(&p),
            overtime_caps_exceeded(&p),
            preraspodela_caps_exceeded(&p),
            breach_notification_missing(&p),
            cenovnik_not_published(&p),
        ] {
            assert!(
                enumerated.contains(&notice),
                "a notice missing from all_notices is an unguarded fine figure: {notice:?}"
            );
        }
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

    /// ZF čl. 15 carries four rows and the preduzetnik one is **st. 3**. Quoting
    /// st. 1 to a preduzetnik would multiply his floor sixfold, and ZF contains
    /// no privredni prestup at all — every ZF sanction is a prekršaj, for every
    /// tier.
    #[test]
    fn lpfr_tiers_are_the_zf_ranges_and_the_preduzetnik_row_is_st_3() {
        let preduzetnik = lpfr_required(&profile(Some(PravnaForma::Preduzetnik)));
        let penalty = preduzetnik.penalty.expect("preduzetnik penalty is known");
        assert!(
            penalty.contains("50.000 do 500.000"),
            "preduzetnik range is čl. 15 st. 3: {penalty}"
        );
        assert!(penalty.contains("čl. 15 st. 3"), "{penalty}");
        assert!(
            !penalty.contains("300.000"),
            "300.000 is the pravno-lice floor (čl. 15 st. 1): {penalty}"
        );
        assert!(preduzetnik.is_legal_duty);

        let pravno = lpfr_required(&profile(Some(PravnaForma::PravnoLice)));
        let penalty = pravno.penalty.expect("pravno lice penalty is known");
        assert!(penalty.contains("300.000 do 2.000.000"), "{penalty}");
        assert!(penalty.contains("čl. 15 st. 1"), "{penalty}");
        assert!(
            !penalty.to_lowercase().contains("privredni prestup"),
            "ZF prescribes prekršaji only — it contains no privredni prestup: {penalty}"
        );
    }

    /// The duty article and the offence article are different provisions, and an
    /// operator handing a citation to an inspector needs both. The summary has
    /// to name the two čl. 6 st. 4 carve-outs, because they are the only excuses
    /// the statute gives — and a shop that qualifies must not read the notice as
    /// applying to it.
    #[test]
    fn lpfr_notice_cites_the_duty_the_offence_and_both_carve_outs() {
        let notice = lpfr_required(&profile(None));

        assert!(
            notice.penalty.is_none(),
            "an UNSET legal form renders no figure: {:?}",
            notice.penalty
        );
        assert!(
            notice.citation.contains("čl. 6 st. 4"),
            "the duty: {}",
            notice.citation
        );
        assert!(
            notice.citation.contains("čl. 15 st. 1 tač. 4"),
            "the offence: {}",
            notice.citation
        );
        assert!(
            notice.summary.contains("isključivo putem interneta"),
            "carve-out 1: {}",
            notice.summary
        );
        assert!(
            notice.summary.contains("sopstvenih korišćenih pokretnih"),
            "carve-out 2: {}",
            notice.summary
        );
        assert!(
            notice.is_legal_duty,
            "čl. 6 st. 4 is an obaveza, never a preporuka"
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

    #[test]
    fn overtime_notices_are_tier_correct_and_carry_no_zeor_figure() {
        let p = profile(Some(PravnaForma::Preduzetnik));

        let missing = overtime_record_missing(&p);
        let penalty = missing.penalty.expect("known");
        assert!(penalty.contains("50.000"), "{penalty}");
        assert!(penalty.contains("150.000"), "{penalty}");
        assert!(
            !penalty.contains("300.000"),
            "300.000 is the pravno-lice tier for čl. 276 st. 1: {penalty}"
        );
        assert!(
            !penalty.contains("odgovorno lice"),
            "čl. 276 st. 2 does not reach a preduzetnik: {penalty}"
        );

        let caps = overtime_caps_exceeded(&p);
        let penalty = caps.penalty.expect("known");
        assert!(
            penalty.contains("200.000") && penalty.contains("400.000"),
            "{penalty}"
        );
    }

    /// The two numbers in this summary are a statement of the law to a shop
    /// owner, and no other test looks at `summary` at all. ZoR čl. 53 st. 2
    /// caps overtime at eight hours a week; st. 3 caps the whole day at 12
    /// hours **including** overtime. Drifted upward, the copy would tell a
    /// preduzetnik that an unlawful roster is lawful — understating an exposure
    /// that is itself the čl. 274 st. 2 fine of 200.000 do 400.000 dinara.
    #[test]
    fn overtime_caps_summary_quotes_the_cl_53_limits_and_cites_both_stavovi() {
        // Summary and citation carry no figure, so the tier is irrelevant here
        // and the UNSET arm exercises exactly the same two strings.
        let notice = overtime_caps_exceeded(&profile(None));

        assert!(
            notice.summary.contains("osam časova nedeljno"),
            "čl. 53 st. 2 — overtime may not exceed eight hours a week: {}",
            notice.summary
        );
        assert!(
            notice.summary.contains("12 časova dnevno"),
            "čl. 53 st. 3 — no more than 12 hours a day in total: {}",
            notice.summary
        );
        assert!(
            notice
                .summary
                .contains("ukupno radno vreme sa prekovremenim"),
            "the 12 h cap is total working time INCLUDING overtime, not 12 h of \
             overtime on top of a full day: {}",
            notice.summary
        );
        assert!(
            notice.citation.contains("čl. 53 st. 2 i st. 3"),
            "both stavovi — the weekly cap and the daily cap are separate: {}",
            notice.citation
        );
        assert!(
            notice.is_legal_duty,
            "čl. 53 is an obaveza, never a preporuka"
        );
    }

    /// Preraspodela is a different rule set and a different offence tačka, and
    /// the čl. 53 notice may not stand in for it.
    ///
    /// čl. 58 says preraspodela is not prekovremeni rad, so the čl. 53 st. 2/st. 3
    /// summary — eight hours of overtime a week, twelve hours a day in total —
    /// states rules that do **not** bind an employee in preraspodela. And čl. 274
    /// st. 1 tač. 3 is the čl. 53 offence: čl. 57 and čl. 60 sit in **tač. 4**.
    /// The amount happens to be identical (both resolve through st. 2), so a
    /// wrong tačka ships no wrong figure — it ships a wrong article and an
    /// inapplicable statement of the law, which is what an inspector reads.
    #[test]
    fn preraspodela_caps_notice_cites_cl_57_st_5_and_the_tacka_4_offence() {
        let notice = preraspodela_caps_exceeded(&profile(Some(PravnaForma::Preduzetnik)));

        assert!(
            notice.summary.contains("60 časova nedeljno"),
            "čl. 57 st. 5 — the preraspodela ceiling is sixty hours a week: {}",
            notice.summary
        );
        assert!(
            !notice.summary.contains("12 časova dnevno"),
            "čl. 57 has no daily leg; quoting the čl. 53 st. 3 figure states a \
             rule čl. 58 makes inapplicable: {}",
            notice.summary
        );
        assert!(
            !notice.summary.contains("osam časova nedeljno"),
            "čl. 53 st. 2 caps prekovremeni rad, and preraspodela is not \
             prekovremeni rad (čl. 58): {}",
            notice.summary
        );
        assert!(
            notice.citation.contains("čl. 57 st. 5"),
            "the duty: {}",
            notice.citation
        );
        assert!(
            !notice.citation.contains("čl. 53"),
            "čl. 53 is not the provision breached here: {}",
            notice.citation
        );

        let penalty = notice.penalty.expect("preduzetnik tier is known");
        assert!(
            penalty.contains("čl. 274 st. 1 tač. 4"),
            "čl. 57 and čl. 60 are tač. 4, not tač. 3: {penalty}"
        );
        assert!(
            !penalty.contains("tač. 3"),
            "tač. 3 is the čl. 53 offence: {penalty}"
        );
        assert!(
            penalty.contains("200.000 do 400.000") && penalty.contains("st. 2"),
            "the preduzetnik row is čl. 274 st. 2, the same amount as tač. 3: {penalty}"
        );
        assert!(
            notice.is_legal_duty,
            "čl. 57 st. 5 is an obaveza, never a preporuka"
        );

        let pravno = preraspodela_caps_exceeded(&profile(Some(PravnaForma::PravnoLice)));
        let penalty = pravno.penalty.expect("pravno lice tier is known");
        assert!(penalty.contains("600.000 do 1.500.000"), "{penalty}");
        assert!(
            penalty.contains("30.000 do 150.000") && penalty.contains("čl. 274 st. 3"),
            "the odgovorno-lice row belongs on the pravno-lice tier: {penalty}"
        );
    }

    /// Req. 50 has two halves and both are copy, not arithmetic.
    ///
    /// **The figure that IS printed** is the notification tier: the biće in
    /// ZZPL čl. 95 st. 1 tač. 24 is described as *„ne obavesti Poverenika o
    /// povredi bezbednosti podataka“*, and a preduzetnik is fined once, under
    /// st. 4. St. 5 opens with a standalone *fizičko lice* limb, so it is not
    /// simply „the odgovorno-lice row“ — but a registered preduzetnik is
    /// reached by st. 4 as lex specialis and quoting st. 5 at him would fine
    /// him twice for one act.
    ///
    /// **The figure that must NOT be printed** is any amount for a pure čl. 52
    /// st. 6 documentation failure. That tier is unresolved: keeping no internal
    /// record is a different act from failing to notify, and no Serbian case law
    /// or Poverenik mišljenje settles whether the trailing *„suprotno članu 52“*
    /// reaches it. Silence about the amount is required — but the copy must not
    /// swing the other way and assert the narrow construction as settled either,
    /// because čl. 79 st. 2 corrective powers and the destroyed tač. 24 defence
    /// are both live consequences.
    #[test]
    fn breach_notice_prints_the_notification_tier_and_no_documentation_only_figure() {
        let preduzetnik = breach_notification_missing(&profile(Some(PravnaForma::Preduzetnik)));
        let penalty = preduzetnik.penalty.expect("preduzetnik penalty is known");
        assert!(
            penalty.contains("20.000 do 500.000"),
            "the preduzetnik row is čl. 95 st. 4: {penalty}"
        );
        assert!(
            penalty.contains("tač. 24"),
            "st. 4 sets the amount; the biće stays čl. 95 st. 1 tač. 24: {penalty}"
        );
        assert!(penalty.contains("st. 4"), "{penalty}");
        assert!(
            !penalty.contains("st. 5"),
            "a preduzetnik is fined once, under st. 4; adding st. 5 fines him \
             twice for one act: {penalty}"
        );
        assert!(
            !penalty.contains("odgovorno lice"),
            "a preduzetnik has no odgovorno lice: {penalty}"
        );
        assert!(
            preduzetnik.is_legal_duty,
            "čl. 52 is an obaveza — unlike čl. 50, it IS penalised"
        );

        let pravno = breach_notification_missing(&profile(Some(PravnaForma::PravnoLice)));
        let penalty = pravno.penalty.expect("pravno lice penalty is known");
        assert!(penalty.contains("50.000 do 2.000.000"), "{penalty}");
        assert!(penalty.contains("čl. 95 st. 1 tač. 24"), "{penalty}");
        assert!(
            penalty.contains("5.000 do 150.000") && penalty.contains("st. 5"),
            "the odgovorno-lice row belongs on the pravno-lice tier: {penalty}"
        );

        // The documentation-only tier, under every profile including UNSET.
        for forma in [
            Some(PravnaForma::Preduzetnik),
            Some(PravnaForma::PravnoLice),
            None,
        ] {
            let notice = breach_notification_missing(&profile(forma));
            let rendered = format!(
                "{} {} {}",
                notice.summary,
                notice.penalty.clone().unwrap_or_default(),
                notice.citation
            );

            assert!(
                notice.summary.contains("nije jednoznačno propisana"),
                "the čl. 52 st. 6 documentation tier is unresolved and the copy \
                 must say so rather than quote a number: {}",
                notice.summary
            );
            assert!(
                !rendered.contains("ne propisuje kaznu"),
                "asserting that no prekršaj exists for the documentation failure \
                 states the narrow construction as settled; it is not: {rendered}"
            );
            assert!(
                !rendered.to_lowercase().contains("nema posledice"),
                "čl. 79 st. 2 opomena and nalog are live consequences: {rendered}"
            );
            assert!(
                notice.summary.contains("čl. 52 st. 6"),
                "st. 7 makes the internal record the vehicle for proving \
                 compliance, so it belongs in the summary the operator reads: {}",
                notice.summary
            );
        }

        // Pravilnik 40/2019 čl. 3 restates the deadline as a flat 72 h od
        // saznanja, without the statute's „bez nepotrebnog odlaganja“ softener,
        // and it is what any countdown in this app is measured against.
        let citation = breach_notification_missing(&profile(None)).citation;
        assert!(citation.contains("40/2019"), "{citation}");
        assert!(
            citation.contains("čl. 3"),
            "the countdown article: {citation}"
        );
        assert!(
            citation.contains("čl. 52 st. 1"),
            "the duty and its 72 h deadline: {citation}"
        );
    }

    /// ZEOR čl. 50/51 tiers are unresolved — čl. 51 exceeds the ZoP čl. 39
    /// ceiling for a "fizičko lice koje ima zaposlene". Silence beats a wrong
    /// number, so no ZEOR amount may appear in any notice, under any profile.
    ///
    /// The amount check must be form-independent, not literal. This module
    /// writes ranges as „od X do Y dinara“, but every table in `docs/` writes
    /// them with an EN DASH (300.000–500.000) — and a paste out of the memo is
    /// the single most likely way a quarantined figure ever lands here.
    /// Normalising away everything that is not a digit or a dot collapses en
    /// dash, em dash, hyphen, whitespace and the word „do“ alike, so every
    /// separator form reduces to the same needle.
    ///
    /// Note the one known collision: ZoR čl. 273 st. 1 carries the same
    /// 300.000–500.000 preduzetnik range. No notice quotes it today, and if one
    /// ever must, this guard fails loudly first — which is the right way round
    /// for a figure that must never ship by accident.
    #[test]
    fn no_zeor_figure_is_reachable_in_any_notice() {
        // Keeps digits and dots, drops everything else — separators, the word
        // „do“, and any dash variant — so the needle is the bare range.
        fn digits_and_dots(s: &str) -> String {
            s.chars()
                .filter(|c| c.is_ascii_digit() || *c == '.')
                .collect()
        }

        for forma in [
            Some(PravnaForma::Preduzetnik),
            Some(PravnaForma::PravnoLice),
            None,
        ] {
            let p = profile(forma);
            for notice in all_notices(&p) {
                let rendered = format!(
                    "{} {} {}",
                    notice.summary,
                    notice.penalty.clone().unwrap_or_default(),
                    notice.citation
                );

                let normalised = digits_and_dots(&rendered);
                // čl. 50 st. 1 (pravno lice) and čl. 51 ("fizičko lice koje
                // ima zaposlene"), in whatever punctuation they arrive.
                for forbidden in ["500.0001.000.000", "300.000500.000"] {
                    assert!(
                        !normalised.contains(forbidden),
                        "no ZEOR figure may reach an operator; found {forbidden:?} \
                         (normalised) in: {rendered}"
                    );
                }

                // "ZEOR" is an internal abbreviation that appears in no operator
                // string in this module — every citation spells the statute out
                // — so guarding on it alone can never fire. Match the name.
                let haystack = rendered.to_lowercase();
                for forbidden in ["evidencijama u oblasti rada", "zeor"] {
                    assert!(
                        !haystack.contains(forbidden),
                        "no ZEOR notice may reach an operator while čl. 50/51 is \
                         unresolved; found {forbidden:?} in: {rendered}"
                    );
                }
            }
        }
    }

    /// ZZP čl. 210 carries three rows and the preduzetnik one is **st. 3**.
    ///
    /// Every amount is fixed, which is what puts the offence in prekršajni-nalog
    /// territory (ZoP čl. 168 st. 1) and makes the ZoP čl. 173 st. 1 halving a
    /// real number for the shop rather than a footnote — so „fiksnom“ and the
    /// eight-day window are load-bearing copy, not decoration. ZZP contains zero
    /// occurrences of *privredni prestup*, so every tier here is a prekršaj.
    #[test]
    fn cenovnik_fine_is_the_fixed_cl_210_st_3_sum_with_the_eight_day_halving() {
        let preduzetnik = cenovnik_not_published(&profile(Some(PravnaForma::Preduzetnik)));
        let penalty = preduzetnik.penalty.expect("preduzetnik penalty is known");
        assert!(penalty.contains("100.000"), "čl. 210 st. 3: {penalty}");
        assert!(
            penalty.contains("fiksnom"),
            "a fixed sum, not a range — this is what makes it issuable by \
             prekršajni nalog: {penalty}"
        );
        assert!(penalty.contains("čl. 210 st. 3"), "{penalty}");
        assert!(
            penalty.contains("50.000") && penalty.contains("osam dana"),
            "half within eight days of the prekršajni nalog (ZoP čl. 173 st. 1) \
             is the amount the shop actually pays: {penalty}"
        );
        assert!(
            penalty.contains("čl. 173 st. 1"),
            "the halving is ZoP's rule, not ZZP's — cite it: {penalty}"
        );
        assert!(
            !penalty.contains("odgovorno lice"),
            "čl. 210 st. 2 reaches only an odgovorno lice u pravnom licu; \
             a preduzetnik has none: {penalty}"
        );
        assert!(preduzetnik.is_legal_duty);

        let pravno = cenovnik_not_published(&profile(Some(PravnaForma::PravnoLice)));
        let penalty = pravno.penalty.expect("pravno lice penalty is known");
        assert!(penalty.contains("200.000"), "čl. 210 st. 1: {penalty}");
        assert!(penalty.contains("čl. 210 st. 1 tač. 1"), "{penalty}");
        assert!(
            penalty.contains("50.000")
                && penalty.contains("odgovorno lice")
                && penalty.contains("čl. 210 st. 2"),
            "the odgovorno-lice row belongs on the pravno-lice tier: {penalty}"
        );
        assert!(
            !penalty.to_lowercase().contains("privredni prestup"),
            "ZZP prescribes prekršaji only — it contains no privredni prestup: {penalty}"
        );
    }

    /// The summary is the shop's statement of what čl. 6 asks of it, and §2b is
    /// the reason it may not be written as a finding of breach.
    ///
    /// Čl. 6 st. 2's second sentence pulls st. 1 into the published file, so the
    /// jedinična cena belongs in the duty the operator reads (req. 10); st. 3 is
    /// the real-time leg; and the no-website case is **unresolved** — čl. 6 st. 2
    /// says *„na svojoj internet stranici“*, a possessive presupposing a site,
    /// and no ZZP provision obliges a trader to have one.
    #[test]
    fn cenovnik_notice_states_the_duty_and_leaves_the_no_website_case_open() {
        // Summary and citation carry no figure, so the UNSET arm exercises
        // exactly the same two strings the other tiers render.
        let notice = cenovnik_not_published(&profile(None));

        assert!(
            notice.penalty.is_none(),
            "an UNSET legal form renders no figure: {:?}",
            notice.penalty
        );
        assert!(
            notice.summary.contains("posebno za svaki prodajni objekat"),
            "one cenovnik per prodajni objekat, not one per trader: {}",
            notice.summary
        );
        assert!(
            notice.summary.contains("jedinična cena"),
            "čl. 6 st. 2's second sentence pulls st. 1 in, so the unit price is \
             part of the duty (req. 10): {}",
            notice.summary
        );
        assert!(
            notice.summary.contains("u realnom vremenu"),
            "čl. 6 st. 3 — a nightly batch is not compliance: {}",
            notice.summary
        );
        assert!(
            notice.summary.contains("ne propisuje obavezu")
                && notice.summary.contains("internet stranicu"),
            "§2b: no ZZP provision obliges a trader to have a website, and the \
             copy must say so rather than imply the duty resolves itself: {}",
            notice.summary
        );

        let haystack = format!("{} {}", notice.summary, notice.citation).to_lowercase();
        for forbidden in ["u prekršaju", "kršite", "prekršili ste", "niste u skladu"] {
            assert!(
                !haystack.contains(forbidden),
                "§2b — the no-website case is unresolved, so no copy may find the \
                 shop in breach; found {forbidden:?} in: {haystack}"
            );
        }

        assert!(
            notice.citation.contains("čl. 6 st. 1"),
            "the duty: {}",
            notice.citation
        );
        assert!(
            notice.citation.contains("čl. 210 st. 1 tač. 1"),
            "the offence: {}",
            notice.citation
        );
        assert!(
            notice.citation.contains("čl. 220") && notice.citation.contains("tri meseca"),
            "čl. 6 applies from entry into force but čl. 210 was NOT in the \
             čl. 220 carve-out — the deferral has to travel with the offence: {}",
            notice.citation
        );
        assert!(
            notice.citation.contains("čl. 213"),
            "the two-year limitation is the retention floor's authority too: {}",
            notice.citation
        );
        assert!(
            notice.is_legal_duty,
            "čl. 6 is an obaveza, never a preporuka"
        );
    }

    /// Req. 18's CI guard: the pravno-lice sum must be unreachable as a
    /// preduzetnik's čl. 6 exposure.
    ///
    /// It cannot be a needle on the blanket forbidden-substring guard above,
    /// because „200.000“ is a **correct** preduzetnik figure elsewhere in this
    /// module — ZoR čl. 274 st. 2 fines him 200.000 do 400.000 — so forbidding
    /// it outright would fail on two lawful notices and the guard would be
    /// deleted rather than fixed. Req. 18 words it narrowly for the same reason:
    /// the string must not be reachable *as a čl. 6 exposure*. So the scope is
    /// every ZZP čl. 6 notice, which is what any future one will also be.
    ///
    /// The emptiness assertion is the other half: a filter that matches nothing
    /// passes every loop below it, and a renamed citation would silently turn
    /// this guard off.
    #[test]
    fn no_zzp_cl_6_notice_quotes_the_pravno_lice_sum_to_a_preduzetnik() {
        fn is_zzp_cl_6(notice: &LegalNotice) -> bool {
            notice.citation.contains("Zakon o zaštiti potrošača")
                && notice.citation.contains("čl. 6")
        }

        let p = profile(Some(PravnaForma::Preduzetnik));
        let guarded: Vec<LegalNotice> = all_notices(&p).into_iter().filter(is_zzp_cl_6).collect();

        assert!(
            !guarded.is_empty(),
            "no ZZP čl. 6 notice was found to guard — a citation rewrite would \
             switch this guard off in silence"
        );

        for notice in guarded {
            let rendered = format!(
                "{} {} {}",
                notice.summary,
                notice.penalty.clone().unwrap_or_default(),
                notice.citation
            );

            for forbidden in ["200.000", "čl. 210 st. 2", "odgovorno lice"] {
                assert!(
                    !rendered.contains(forbidden),
                    "a preduzetnik's čl. 6 exposure is the fixed 100.000 of \
                     čl. 210 st. 3; found {forbidden:?} in: {rendered}"
                );
            }

            // „čl. 210 st. 1“ is deliberately NOT a needle: st. 1 tač. 1 is the
            // biće, and st. 3 reaches the preduzetnik by referring back to it
            // („Za prekršaj iz stava 1. ovog člana kazniće se i preduzetnik“) —
            // the same shape as čl. 15 st. 1 tač. 4 → st. 3 in `lpfr_required`
            // and čl. 95 st. 1 tač. 24 → st. 4 in `breach_notification_missing`.
            // Dropping the offence article would be a worse notice, not a safer
            // one. What must never happen is quoting st. 1 without resolving the
            // amount through st. 3.
            assert!(
                !rendered.contains("čl. 210") || rendered.contains("čl. 210 st. 3"),
                "a notice that names čl. 210 to a preduzetnik must resolve the \
                 amount through st. 3: {rendered}"
            );
        }
    }

    /// Čl. 220's carve-out named čl. 4 st. 1 and čl. 6 — and **not** čl. 210, so
    /// the offence provision did not apply from the day the law entered into
    /// force. Dating a shop's exposure to the first of May 2026 overstates it by
    /// three months, in the one direction a shop cannot check.
    ///
    /// Applied to every notice, not only the ZZP one: no notice in this module
    /// names a calendar date today, and one that ever needs to should fail here
    /// loudly first.
    #[test]
    fn no_notice_dates_an_exposure_to_the_first_of_may_2026() {
        for forma in [
            Some(PravnaForma::Preduzetnik),
            Some(PravnaForma::PravnoLice),
            None,
        ] {
            let p = profile(forma);
            for notice in all_notices(&p) {
                let rendered = format!(
                    "{} {} {}",
                    notice.summary,
                    notice.penalty.clone().unwrap_or_default(),
                    notice.citation
                );

                for forbidden in ["1. maja 2026", "1. maj 2026", "01.05.2026", "1.5.2026"] {
                    assert!(
                        !rendered.contains(forbidden),
                        "čl. 210 was not in the čl. 220 carve-out, so no exposure \
                         may be dated to the first of May; found {forbidden:?} \
                         in: {rendered}"
                    );
                }
            }
        }
    }
}
