//! NBS official middle rate for EUR.
//!
//! The parse is deliberately split from the fetch so it is unit-tested from
//! fixtures with no network — the same shape as the Open Food Facts lookup in
//! `commands/catalog.rs`.
//!
//! Consumed by the NBS refresh command and the AML assessment (Tasks 6 and 7),
//! so `dead_code` is allowed here until that wiring lands — mirroring the other
//! domain modules.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::app_error::AppError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RateSource {
    Nbs,
    Manual,
}

/// `rate_minor` is **para per 1 EUR** (117,2345 RSD/EUR -> 11723), rounded
/// half-up to the para. Every consumer depends on this unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EurRate {
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

/// Rounds a decimal RSD/EUR string to para, half-up, without floating point.
fn rsd_string_to_para(raw: &str) -> Option<i64> {
    let normalized = raw.replace(',', ".");
    let (whole, frac) = match normalized.split_once('.') {
        Some((w, f)) => (w, f),
        None => (normalized.as_str(), ""),
    };
    let whole: i64 = whole.trim().parse().ok()?;
    let mut digits: Vec<u8> = frac.bytes().filter(u8::is_ascii_digit).collect();
    let third = digits.get(2).copied().unwrap_or(b'0');
    digits.resize(2, b'0');
    let frac_value: i64 = String::from_utf8(digits).ok()?.parse().unwrap_or(0);
    let mut para = whole.checked_mul(100)?.checked_add(frac_value)?;
    if third >= b'5' {
        para = para.checked_add(1)?;
    }
    Some(para)
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

/// NBS renders `dd.MM.yyyy`; the app stores ISO `yyyy-MM-dd` everywhere.
fn normalize_nbs_date(raw: &str) -> Option<String> {
    let parts: Vec<&str> = raw.trim().trim_end_matches('.').split('.').collect();
    if parts.len() != 3 {
        return None;
    }
    let (day, month, year) = (parts[0], parts[1], parts[2]);
    if year.len() != 4 || !year.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(format!("{year}-{month:0>2}-{day:0>2}"))
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
            "117,2345 RSD/EUR rounds to 11723 para"
        );
        assert_eq!(rate.rate_date, "2026-07-31");
        assert_eq!(rate.source, RateSource::Nbs);
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
