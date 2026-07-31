//! NBS official middle rate for EUR.
//!
//! The parse is deliberately split from the fetch so it is unit-tested from
//! fixtures with no network — the same shape as the Open Food Facts lookup in
//! `commands/catalog.rs`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::app_error::AppError;

const NBS_TIMEOUT_SECONDS: u64 = 8;
const NBS_RATE_URL: &str =
    "https://www.nbs.rs/ExchangeRateXmlOldWS/ExchangeRateXmlOld.asmx/GetCurrentExchangeRate";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RateSource {
    Nbs,
    Manual,
}

/// `rate_minor` is **para per 1 EUR** (117,2345 RSD/EUR -> 11723), truncated
/// (floored) to the para. Every consumer depends on this unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EurRate {
    /// Para per 1 EUR, **floored** — never rounded up, so the derived dinar
    /// threshold (`10_000 * rate_minor`, Task 7) can never exceed the
    /// statutory one and open an under-blocking window at the margin.
    ///
    /// Para precision drops the NBS 3rd and 4th decimals, so the persisted
    /// `aml_rate_minor` is the floored rate, not the verbatim published rate.
    pub rate_minor: i64,
    pub rate_date: String,
    pub source: RateSource,
}

fn tag_value<'a>(body: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = body.find(&open)? + open.len();
    let end = body[start..].find(&close)? + start;
    Some(body[start..end].trim())
}

/// Converts a decimal RSD/EUR string to para without floating point,
/// **truncated (floored)** to the para so the derived dinar threshold can never
/// exceed the statutory one. Rounding up would lift the AML čl. 46 st. 1 block
/// boundary above the real one and let an unlawful cash amount through.
///
/// A value it cannot read exactly — a group separator, a sign, any non-digit —
/// is refused rather than guessed at.
fn rsd_string_to_para(raw: &str) -> Option<i64> {
    let normalized = raw.trim().replace(',', ".");
    if normalized.matches('.').count() > 1 {
        return None;
    }
    let (whole, frac) = match normalized.split_once('.') {
        Some((w, f)) => (w, f),
        None => (normalized.as_str(), ""),
    };
    if whole.is_empty() || !whole.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let whole: i64 = whole.parse().ok()?;
    // Only the first two fractional digits survive; the rest are dropped, never
    // carried into a rounding bump.
    let digit = |index: usize| -> i64 {
        frac.as_bytes()
            .get(index)
            .map_or(0, |b| i64::from(b - b'0'))
    };
    let frac_value = digit(0) * 10 + digit(1);
    whole.checked_mul(100)?.checked_add(frac_value)
}

pub fn parse_nbs_middle_rate(body: &str) -> Result<EurRate, AppError> {
    let unavailable = || {
        AppError::business(
            "nbs_rate_unavailable",
            "Kurs Narodne banke Srbije nije dostupan u odgovoru.",
        )
    };

    let eur_block = body
        .split("<ExchangeRate>")
        .find(|block| block.contains("<CurrencyCodeAlfaChar>EUR</CurrencyCodeAlfaChar>"))
        .ok_or_else(unavailable)?;

    // NBS quotes some currencies per 100 units. `rate_minor` is defined as para
    // per *one* EUR, so a EUR entry priced per anything else is a refusal — a
    // silent 100x rate would push the AML threshold out of reach and the
    // block would never fire.
    if tag_value(eur_block, "UnitValue").is_some_and(|unit| unit != "1") {
        return Err(unavailable());
    }

    let middle = tag_value(eur_block, "MiddleRate").ok_or_else(unavailable)?;
    let rate_minor = rsd_string_to_para(middle).ok_or_else(unavailable)?;

    let raw_date = tag_value(body, "Date").ok_or_else(unavailable)?;
    let rate_date = normalize_nbs_date(raw_date).ok_or_else(unavailable)?;

    Ok(EurRate {
        rate_minor,
        rate_date,
        source: RateSource::Nbs,
    })
}

/// The network call. Deliberately not unit-tested — the parse above is where
/// the logic lives, and it is fixture-tested. Any failure is a `Business` error
/// the caller degrades from: an unreachable NBS must never block a sale, it may
/// only leave the cached rate standing and stale.
pub fn fetch_nbs_middle_rate() -> Result<EurRate, AppError> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(NBS_TIMEOUT_SECONDS))
        .build();

    match agent.get(NBS_RATE_URL).call() {
        Ok(response) => {
            let body = response.into_string().map_err(|source| {
                AppError::business(
                    "nbs_rate_unavailable",
                    format!("Odgovor NBS-a nije čitljiv: {source}"),
                )
            })?;
            parse_nbs_middle_rate(&body)
        }
        Err(error) => Err(AppError::business(
            "nbs_rate_unavailable",
            format!("Kurs NBS-a trenutno nije dostupan: {error}"),
        )),
    }
}

/// NBS renders `dd.MM.yyyy`; the app stores ISO `yyyy-MM-dd` everywhere.
///
/// Every field is validated: this date is persisted as `aml_rate_date` on the
/// sale and has to be reproducible at inspection, and Task 6 compares it against
/// today to detect staleness — a nonsense date would be stale forever.
fn normalize_nbs_date(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.trim().trim_end_matches('.').split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let (day, month, year) = (parts[0], parts[1], parts[2]);
    if year.len() != 4 || !year.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let day = date_field(day, 1, 31)?;
    let month = date_field(month, 1, 12)?;
    Some(format!("{year}-{month:02}-{day:02}"))
}

/// A `dd` or `MM` field: one or two ASCII digits inside the given range.
fn date_field(raw: &str, min: u32, max: u32) -> Option<u32> {
    let raw = raw.trim();
    if raw.is_empty() || raw.len() > 2 || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let value: u32 = raw.parse().ok()?;
    (min..=max).contains(&value).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = include_str!("fixtures/nbs_rate_sample.xml");

    #[test]
    fn parses_the_middle_rate_and_its_date() {
        let rate = parse_nbs_middle_rate(SAMPLE).expect("sample should parse");

        assert_eq!(
            rate.rate_minor, 11723,
            "117,2345 RSD/EUR truncates to 11723 para"
        );
        assert_eq!(rate.rate_date, "2026-07-31");
        assert_eq!(rate.source, RateSource::Nbs);
    }

    /// The fixture deliberately lists CHF before EUR: a parser that took the
    /// first `<MiddleRate>` in the document would set the AML čl. 46 st. 1
    /// threshold from the wrong currency, silently.
    #[test]
    fn picks_the_eur_entry_not_the_first_one() {
        let rate = parse_nbs_middle_rate(SAMPLE).expect("sample should parse");

        assert_ne!(
            rate.rate_minor, 7512,
            "the parser must select EUR, not the first currency in the document"
        );
        assert_eq!(rate.rate_minor, 11723);
    }

    /// Rounding the srednji kurs **up** would raise the derived dinar threshold
    /// above the statutory one and open an under-blocking window at exactly the
    /// margin where the block fires (VERIFIED-RULES §5 Q-4).
    #[test]
    fn rate_conversion_is_integer_only_and_never_rounds_up() {
        assert_eq!(rsd_string_to_para("117.2345"), Some(11723));
        assert_eq!(
            rsd_string_to_para("117.2355"),
            Some(11723),
            "the 3rd decimal must never bump the para up"
        );
        assert_eq!(
            rsd_string_to_para("117,2345"),
            Some(11723),
            "NBS may render the decimal separator as a comma"
        );
        assert_eq!(rsd_string_to_para("117"), Some(11700));
        assert_eq!(rsd_string_to_para("0.5"), Some(50));
    }

    /// NBS quotes some currencies per 100 units; a EUR entry priced per
    /// anything but one unit would make the threshold unreachable.
    #[test]
    fn rejects_a_eur_entry_quoted_per_more_than_one_unit() {
        let body = "<ExchangeRates><Date>31.07.2026</Date>\
             <ExchangeRate><CurrencyCodeAlfaChar>EUR</CurrencyCodeAlfaChar>\
             <UnitValue>100</UnitValue><MiddleRate>117.2345</MiddleRate>\
             </ExchangeRate></ExchangeRates>";

        let error = parse_nbs_middle_rate(body)
            .expect_err("a EUR rate quoted per 100 units must not be taken as per 1");
        assert!(matches!(
            error,
            AppError::Business {
                code: "nbs_rate_unavailable",
                ..
            }
        ));
    }

    /// A nonsense day/month would be persisted as `aml_rate_date` on the sale
    /// and would never match today, so the staleness check would jam.
    #[test]
    fn rejects_a_nonsense_date_rather_than_persisting_it() {
        let body = "<ExchangeRates><Date>aa.bb.2026</Date>\
             <ExchangeRate><CurrencyCodeAlfaChar>EUR</CurrencyCodeAlfaChar>\
             <UnitValue>1</UnitValue><MiddleRate>117.2345</MiddleRate>\
             </ExchangeRate></ExchangeRates>";

        let error = parse_nbs_middle_rate(body).expect_err("a nonsense date must not be persisted");
        assert!(matches!(
            error,
            AppError::Business {
                code: "nbs_rate_unavailable",
                ..
            }
        ));
        assert_eq!(normalize_nbs_date("31.13.2026"), None, "month 13");
        assert_eq!(normalize_nbs_date("32.07.2026"), None, "day 32");
        assert_eq!(
            normalize_nbs_date("31.7.2026").as_deref(),
            Some("2026-07-31")
        );
    }

    /// A grouped value must be refused, not silently read as 1,23 RSD/EUR.
    #[test]
    fn rejects_a_grouped_rate_rather_than_misreading_it() {
        let body = "<ExchangeRates><Date>31.07.2026</Date>\
             <ExchangeRate><CurrencyCodeAlfaChar>EUR</CurrencyCodeAlfaChar>\
             <UnitValue>1</UnitValue><MiddleRate>1.234,5678</MiddleRate>\
             </ExchangeRate></ExchangeRates>";

        let error =
            parse_nbs_middle_rate(body).expect_err("a grouped rate must not be silently misread");
        assert!(matches!(
            error,
            AppError::Business {
                code: "nbs_rate_unavailable",
                ..
            }
        ));
        assert_eq!(rsd_string_to_para("1.234,5678"), None);
        assert_eq!(rsd_string_to_para("117.23x5"), None);
        assert_eq!(rsd_string_to_para("-117.2345"), None);
        assert_eq!(rsd_string_to_para(""), None);
    }

    #[test]
    fn rejects_a_body_without_a_eur_middle_rate() {
        let error = parse_nbs_middle_rate("<ExchangeRates></ExchangeRates>")
            .expect_err("a body with no EUR entry must not silently succeed");
        assert!(matches!(
            error,
            AppError::Business {
                code: "nbs_rate_unavailable",
                ..
            }
        ));
    }

    #[test]
    fn rejects_a_malformed_body_rather_than_guessing() {
        assert!(parse_nbs_middle_rate("not xml at all").is_err());
    }
}
