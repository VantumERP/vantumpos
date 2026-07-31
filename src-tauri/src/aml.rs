//! AML čl. 46 st. 1 — the cash-acceptance cap.
//!
//! Two things this module must get exactly right, both verified against
//! primary text (`docs/SW11-SW15-VERIFIED-RULES.md` §2 Q1):
//!
//! 1. The subject is the **cash tendered**, never the invoice total. A 15.000 €
//!    sale settled 5.000 cash + 10.000 card does not breach st. 1.
//! 2. The operator is **`>=`** — "u iznosu od 10.000 evra **ili više**".

use serde::Serialize;

use crate::commands::settings::ShopProfile;
use crate::legal::{aml_cash_cap, LegalNotice};
use crate::nbs_rate::EurRate;

pub const AML_CAP_EUR: i64 = 10_000;

/// A deliberate floor on the EUR rate, in para: 100,00 RSD per EUR.
///
/// It is NOT an exchange rate and never computes a verdict. Its only job is to
/// give a till with no cached rate a conservative stand-in cap, so it can tell
/// a 200 RSD loaf of bread from a sale where a failed check actually matters.
/// The dinar has been managed far above this floor for the whole life of the
/// currency band, so the stand-in cap always sits below the real one — gating
/// a notice on it can only warn early, never hide a breach.
const AML_FALLBACK_RATE_MINOR: i64 = 10_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AmlAssessment {
    pub cash_minor: i64,
    pub threshold_minor: i64,
    /// The stand-in cap described on [`AML_FALLBACK_RATE_MINOR`]. Advisory
    /// surfaces use it to decide whether to mention an *unavailable* check;
    /// nothing may use it to assert a breach.
    pub fallback_threshold_minor: i64,
    pub breached: bool,
    pub near_threshold: bool,
    pub rate_unavailable: bool,
    pub rate: Option<EurRate>,
    pub notice: LegalNotice,
}

pub fn assess_cash_payment(
    cash_minor: i64,
    rate: Option<&EurRate>,
    profile: &ShopProfile,
    soft_ratio_percent: i64,
) -> AmlAssessment {
    let notice = aml_cash_cap(profile);
    let fallback_threshold_minor = AML_CAP_EUR.saturating_mul(AML_FALLBACK_RATE_MINOR);

    let Some(rate) = rate else {
        return AmlAssessment {
            cash_minor,
            threshold_minor: 0,
            fallback_threshold_minor,
            breached: false,
            near_threshold: false,
            rate_unavailable: true,
            rate: None,
            notice,
        };
    };

    let threshold_minor = AML_CAP_EUR.saturating_mul(rate.rate_minor);
    let soft_minor = threshold_minor.saturating_mul(soft_ratio_percent) / 100;

    AmlAssessment {
        cash_minor,
        threshold_minor,
        fallback_threshold_minor,
        breached: cash_minor >= threshold_minor,
        near_threshold: cash_minor >= soft_minor && cash_minor < threshold_minor,
        rate_unavailable: false,
        rate: Some(rate.clone()),
        notice,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::settings::{PravnaForma, ShopProfile};
    use crate::nbs_rate::{EurRate, RateSource};

    fn rate() -> EurRate {
        EurRate {
            rate_minor: 11723,
            rate_date: "2026-07-31".to_string(),
            source: RateSource::Nbs,
        }
    }

    fn preduzetnik() -> ShopProfile {
        ShopProfile {
            pravna_forma: Some(PravnaForma::Preduzetnik),
            ..ShopProfile::default()
        }
    }

    #[test]
    fn threshold_is_ten_thousand_eur_in_para() {
        let a = assess_cash_payment(0, Some(&rate()), &preduzetnik(), 80);
        assert_eq!(a.threshold_minor, 117_230_000, "10_000 * 11723");
    }

    /// The statute says "10.000 evra ili više". A `>` comparison is a bug.
    ///
    /// `breached` and `near_threshold` must also stay mutually exclusive: the
    /// till branches on one or the other, so an overlap would render the soft
    /// "blizu praga" warning and the hard block at the same time.
    #[test]
    fn exactly_the_threshold_already_breaches() {
        let threshold = 10_000 * rate().rate_minor;

        let below = assess_cash_payment(threshold - 1, Some(&rate()), &preduzetnik(), 80);
        assert!(!below.breached, "one para below the cap is lawful");
        assert!(
            below.near_threshold,
            "but one para below the cap is still a soft warning"
        );

        let at = assess_cash_payment(threshold, Some(&rate()), &preduzetnik(), 80);
        assert!(
            at.breached,
            "exactly 10.000 EUR is ALREADY unlawful (>=, not >)"
        );
        assert!(!at.near_threshold, "a breach is not also a soft warning");

        let above = assess_cash_payment(threshold + 1, Some(&rate()), &preduzetnik(), 80);
        assert!(above.breached);
        assert!(!above.near_threshold);
    }

    #[test]
    fn soft_threshold_warns_without_breaching() {
        let threshold = 10_000 * rate().rate_minor;
        let soft = assess_cash_payment(threshold * 90 / 100, Some(&rate()), &preduzetnik(), 80);

        assert!(!soft.breached);
        assert!(
            soft.near_threshold,
            "90% of the cap is past the 80% soft line"
        );
    }

    #[test]
    fn without_a_rate_the_check_is_unavailable_rather_than_passing() {
        let a = assess_cash_payment(999_999_999, None, &preduzetnik(), 80);

        assert!(!a.breached, "we must not assert a breach we cannot compute");
        assert!(a.rate_unavailable, "and we must not silently pass either");
        assert!(a.rate.is_none());
    }

    /// A till with no cached rate still has to decide whether an unavailable
    /// check is worth interrupting the operator over — otherwise it nags on
    /// every loaf of bread and the real warning gets dismissed on sight.
    ///
    /// The stand-in cap must therefore sit BELOW any real one, so gating a
    /// notice on it can only ever warn early, never hide a genuine breach.
    #[test]
    fn the_stand_in_cap_is_conservative_and_always_present() {
        let unavailable = assess_cash_payment(0, None, &preduzetnik(), 80);
        assert_eq!(
            unavailable.fallback_threshold_minor, 100_000_000,
            "10.000 EUR at the 100,00 RSD/EUR floor"
        );

        let known = assess_cash_payment(0, Some(&rate()), &preduzetnik(), 80);
        assert_eq!(
            known.fallback_threshold_minor, unavailable.fallback_threshold_minor,
            "the stand-in cap does not depend on the rate being known"
        );
        assert!(
            known.fallback_threshold_minor < known.threshold_minor,
            "a stand-in above the real cap would hide breaches"
        );
    }

    #[test]
    fn notice_tier_follows_the_profile() {
        let a = assess_cash_payment(0, Some(&rate()), &preduzetnik(), 80);
        let penalty = a.notice.penalty.expect("known");
        assert!(penalty.contains("100.000 do 300.000"));

        let unset = assess_cash_payment(0, Some(&rate()), &ShopProfile::default(), 80);
        assert!(unset.notice.penalty.is_none(), "UNSET renders no figure");
    }
}
