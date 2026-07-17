// The tests construct these inputs but only read what each rule needs, so
// `dead_code` fires under `cfg(test)` too — the expectation cannot be gated on
// `not(test)`. Task 8 (command layer) must DELETE this line; an unfulfilled
// expectation is a clippy error, which is the point.
#![expect(
    dead_code,
    reason = "wired up by the campaigns command layer in a later task"
)]
//! Campaign/sniženje domain: the closed four-type entity and its rules.
//!
//! Legal authority: `docs/ZOT-36-37-VERIFIED-RULES.md`. Design:
//! `docs/superpowers/specs/2026-07-17-campaign-engine-design.md`.
//!
//! This module owns the DB-independent half of the validation set. Nothing
//! here affirms that a promotion is lawful — čl. 38 st. 4 (misleading
//! commercial practice) can bite even when the čl. 37 st. 3 arithmetic is
//! right. A clean `validate_shape` means "no rule we can mechanically check
//! was broken", never "this is legal".

use crate::app_error::AppError;
use time::format_description::well_known::Rfc3339;
use time::{Date, OffsetDateTime};

/// čl. 37 st. 6–7. Exhaustive three grounds, no default.
pub const TYPE_RASPRODAJA: &str = "rasprodaja";
/// čl. 37 st. 8–9. Start window + 60-day cap + 2/yr.
pub const TYPE_SEZONSKO: &str = "sezonsko_snizenje";
/// čl. 37 st. 10–11. 31-day cap; percentage-only display iff ≤ 3 days.
pub const TYPE_AKCIJSKA: &str = "akcijska_prodaja";
/// čl. 36 st. 9 — NOT a sniženje. Defined against a *future* regular price,
/// so it never routes through the prethodna cena validator.
pub const TYPE_PROMOTIVNA: &str = "promotivna_prodaja";

const DISPLAY_TWO_PRICES: &str = "two_prices";
const DISPLAY_PERCENTAGE: &str = "percentage";

/// čl. 37 st. 6: „prestanka poslovanja trgovca, prestanka poslovanja u
/// određenim objektima ili prestanka prodaje određene robe" — a closed list
/// with no „naročito"/„i sl.", so the user must pick one of exactly these.
const RASPRODAJA_GROUNDS: [&str; 3] = [
    "prestanak_poslovanja",
    "prestanak_u_objektu",
    "prestanak_prodaje_robe",
];

const MSG_H1: &str = "Vrsta kampanje nije ispravna.";
const MSG_H2: &str =
    "Sezonsko sniženje mora početi u periodu 25.12–10.01. ili 01.07–15.07. (čl. 37 st. 8).";
const MSG_H4: &str = "Sezonsko sniženje može trajati najviše 60 dana (čl. 37 st. 9 u vezi st. 8).";
const MSG_H5: &str = "Akcijska prodaja može trajati najviše 31 dan (čl. 37 st. 10).";
const MSG_H6: &str = "Isticanje samo procenta je dozvoljeno isključivo za akcijsku prodaju sa rokom važenja do 3 dana (čl. 37 st. 11).";
const MSG_H7D: &str = "Promotivna prodaja može trajati najviše 60 dana (čl. 36 st. 9).";
const MSG_H10: &str = "Za rasprodaju izaberite jedan od tri zakonska osnova (čl. 37 st. 6).";
const MSG_H12A: &str = "Potvrdite da je sezona protekla (čl. 37 st. 8).";
const MSG_H12B: &str = "Potvrdite da je roba na rasprodaji fizički izdvojena (čl. 37 st. 7).";
const MSG_H14A: &str = "Datum početka nije ispravan.";
const MSG_H14B: &str =
    "Datum isteka je obavezan (osim za rasprodaju — „dok traju zalihe\") (čl. 36 st. 2 t. 3).";
const MSG_H14C: &str = "Datum isteka mora biti posle datuma početka.";
const MSG_H14D: &str = "Kampanja mora imati bar jedan artikal.";
const MSG_H14E: &str = "Način isticanja nije ispravan.";

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignItemInput {
    pub product_id: i64,
    pub campaign_price_minor: i64,
    #[serde(default)]
    pub manual_prethodna_minor: Option<i64>,
    #[serde(default)]
    pub anchor_justification: Option<String>,
    #[serde(default)]
    pub future_regular_price_minor: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignInput {
    pub campaign_type: String,
    pub starts_on: String,
    #[serde(default)]
    pub ends_on: Option<String>,
    pub display_mode: String,
    #[serde(default)]
    pub headline_percent: Option<i64>,
    #[serde(default)]
    pub rasprodaja_ground: Option<String>,
    #[serde(default)]
    pub special_conditions: Option<String>,
    #[serde(default)]
    pub reduced_utility_reason: Option<String>,
    #[serde(default)]
    pub marketing_label: Option<String>,
    #[serde(default)]
    pub season_attested: bool,
    #[serde(default)]
    pub separation_attested: bool,
    pub items: Vec<CampaignItemInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Violation {
    pub code: &'static str,
    pub message: String,
    pub product_id: Option<i64>,
}

impl Violation {
    fn new(code: &'static str, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            product_id: None,
        }
    }
}

fn parse_rfc3339(value: &str, field: &str) -> Result<OffsetDateTime, AppError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|source| {
        AppError::validation(
            format!("Datum nije ispravan: {source}"),
            serde_json::json!({ "field": field }),
        )
    })
}

/// Declared campaign length in days, **inclusive of both endpoints**: the memo's
/// worked example (b) treats 05.07 → 02.09 as exactly the 60-day cap, so a
/// single-day campaign is 1, not 0. Times of day are irrelevant — the statute
/// speaks in calendar days — so both instants collapse to their date.
pub fn declared_duration_days(starts_on: &str, ends_on: &str) -> Result<i64, AppError> {
    let start: Date = parse_rfc3339(starts_on, "startsOn")?.date();
    let end: Date = parse_rfc3339(ends_on, "endsOn")?.date();
    Ok((end - start).whole_days() + 1)
}

/// čl. 37 st. 8: „započinje u razdoblju između 25. decembra i 10. januara i 1.
/// i 15. jula." The winter window straddles the year boundary, so this is a
/// (month, day) match — a single date-range comparison would reject 28.12 or
/// accept 20.01. The constraint binds the START only; the end date is never
/// validated against the window.
fn starts_in_seasonal_window(date: Date) -> bool {
    let (month, day) = (date.month() as u8, date.day());
    matches!((month, day), (12, 25..=31) | (1, 1..=10) | (7, 1..=15))
}

/// The DB-independent rules (H1/H2/H4/H5/H6/H10/H12/H14). Collects *all*
/// violations rather than returning on the first, so the wizard can show the
/// user everything wrong at once. `Err` is reserved for inputs we cannot even
/// interpret; a bad-but-interpretable input is a `Violation`.
pub fn validate_shape(input: &CampaignInput) -> Result<Vec<Violation>, AppError> {
    let mut violations = Vec::new();

    let campaign_type = input.campaign_type.as_str();
    let known_type = matches!(
        campaign_type,
        TYPE_RASPRODAJA | TYPE_SEZONSKO | TYPE_AKCIJSKA | TYPE_PROMOTIVNA
    );
    if !known_type {
        violations.push(Violation::new("h1", MSG_H1));
    }
    let is_rasprodaja = campaign_type == TYPE_RASPRODAJA;

    // h14a — an unparseable start date is a violation, not an Err: the wizard
    // must still surface every other problem alongside it.
    let start_date = match parse_rfc3339(&input.starts_on, "startsOn") {
        Ok(start) => Some(start.date()),
        Err(_) => {
            violations.push(Violation::new("h14a", MSG_H14A));
            None
        }
    };

    // h14b — čl. 36 st. 2 t. 3 requires a declared expiry; rasprodaja is the
    // only type that may run „dok traju zalihe".
    let end_date = match input.ends_on.as_deref() {
        Some(ends_on) => match parse_rfc3339(ends_on, "endsOn") {
            Ok(end) => Some(end.date()),
            Err(_) => {
                violations.push(Violation::new("h14c", MSG_H14C));
                None
            }
        },
        None => {
            if !is_rasprodaja {
                violations.push(Violation::new("h14b", MSG_H14B));
            }
            None
        }
    };

    // h14c — an end before the start is not a duration, so no cap is evaluated.
    let duration_days = match (start_date, end_date) {
        (Some(start), Some(end)) if end < start => {
            violations.push(Violation::new("h14c", MSG_H14C));
            None
        }
        (Some(start), Some(end)) => Some((end - start).whole_days() + 1),
        _ => None,
    };

    if input.items.is_empty() {
        violations.push(Violation::new("h14d", MSG_H14D));
    }

    let display_mode = input.display_mode.as_str();
    if !matches!(display_mode, DISPLAY_TWO_PRICES | DISPLAY_PERCENTAGE) {
        violations.push(Violation::new("h14e", MSG_H14E));
    }

    // h2 — start window, sezonsko only (čl. 37 st. 8).
    if campaign_type == TYPE_SEZONSKO {
        if let Some(start) = start_date {
            if !starts_in_seasonal_window(start) {
                violations.push(Violation::new("h2", MSG_H2));
            }
        }
    }

    // h4 / h5 / h7d — three unrelated duration caps. Encoding them as one rule
    // would be a real defect: 60 (čl. 37 st. 9), 31 (čl. 37 st. 10), 60 (čl. 36
    // st. 9) attach to different types via different articles.
    if let Some(days) = duration_days {
        match campaign_type {
            TYPE_SEZONSKO if days > 60 => violations.push(Violation::new("h4", MSG_H4)),
            TYPE_AKCIJSKA if days > 31 => violations.push(Violation::new("h5", MSG_H5)),
            TYPE_PROMOTIVNA if days > 60 => violations.push(Violation::new("h7d", MSG_H7D)),
            _ => {}
        }
    }

    // h6 — čl. 37 st. 11 exempts DISPLAY only, and only for an akcijska prodaja
    // of at most 3 days. An unknown duration cannot establish the exemption.
    if display_mode == DISPLAY_PERCENTAGE
        && !(campaign_type == TYPE_AKCIJSKA && duration_days.is_some_and(|days| days <= 3))
    {
        violations.push(Violation::new("h6", MSG_H6));
    }

    // h10 — čl. 37 st. 6's closed list. Rasprodaja must carry a ground; no other
    // type may claim one.
    let ground = input.rasprodaja_ground.as_deref();
    if is_rasprodaja {
        if !ground.is_some_and(|value| RASPRODAJA_GROUNDS.contains(&value)) {
            violations.push(Violation::new("h10", MSG_H10));
        }
    } else if ground.is_some() {
        violations.push(Violation::new("h10", MSG_H10));
    }

    // h12a / h12b — physical-world facts („nakon proteka sezone", „fizički
    // izdvoji") the software cannot verify. Attestation only; do not pretend.
    if campaign_type == TYPE_SEZONSKO && !input.season_attested {
        violations.push(Violation::new("h12a", MSG_H12A));
    }
    if is_rasprodaja && !input.separation_attested {
        violations.push(Violation::new("h12b", MSG_H12B));
    }

    Ok(violations)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_input(campaign_type: &str) -> CampaignInput {
        CampaignInput {
            campaign_type: campaign_type.to_string(),
            starts_on: "2026-07-05T00:00:00Z".to_string(),
            ends_on: Some("2026-07-20T00:00:00Z".to_string()),
            display_mode: "two_prices".to_string(),
            headline_percent: None,
            rasprodaja_ground: None,
            special_conditions: None,
            reduced_utility_reason: None,
            marketing_label: None,
            season_attested: true,
            separation_attested: false,
            items: vec![CampaignItemInput {
                product_id: 1,
                campaign_price_minor: 9900,
                manual_prethodna_minor: None,
                anchor_justification: None,
                future_regular_price_minor: None,
            }],
        }
    }

    fn codes(violations: &[Violation]) -> Vec<&'static str> {
        violations.iter().map(|violation| violation.code).collect()
    }

    #[test]
    fn duration_is_inclusive_of_both_endpoints() {
        // Memo worked example (b): 05.07 -> 02.09 is exactly the 60-day cap.
        assert_eq!(
            declared_duration_days("2026-07-05T00:00:00Z", "2026-09-02T00:00:00Z")
                .expect("duration"),
            60
        );
        assert_eq!(
            declared_duration_days("2026-07-05T00:00:00Z", "2026-07-05T00:00:00Z")
                .expect("duration"),
            1
        );
    }

    #[test]
    fn sezonsko_inside_july_window_passes_h2() {
        let input = base_input(TYPE_SEZONSKO);
        let violations = validate_shape(&input).expect("validate");
        assert!(
            !codes(&violations).contains(&"h2"),
            "05.07 is inside 01–15.07: {violations:?}"
        );
    }

    #[test]
    fn sezonsko_outside_windows_fails_h2() {
        let mut input = base_input(TYPE_SEZONSKO);
        input.starts_on = "2026-03-01T00:00:00Z".to_string();
        input.ends_on = Some("2026-03-20T00:00:00Z".to_string());
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h2"));
    }

    #[test]
    fn winter_window_straddles_the_year_boundary() {
        let mut december = base_input(TYPE_SEZONSKO);
        december.starts_on = "2026-12-28T00:00:00Z".to_string();
        december.ends_on = Some("2027-01-30T00:00:00Z".to_string());
        assert!(!codes(&validate_shape(&december).expect("validate")).contains(&"h2"));

        let mut january = base_input(TYPE_SEZONSKO);
        january.starts_on = "2026-01-08T00:00:00Z".to_string();
        january.ends_on = Some("2026-02-10T00:00:00Z".to_string());
        assert!(!codes(&validate_shape(&january).expect("validate")).contains(&"h2"));

        let mut late_january = base_input(TYPE_SEZONSKO);
        late_january.starts_on = "2026-01-11T00:00:00Z".to_string();
        late_january.ends_on = Some("2026-02-10T00:00:00Z".to_string());
        assert!(codes(&validate_shape(&late_january).expect("validate")).contains(&"h2"));
    }

    #[test]
    fn sezonsko_over_60_days_fails_h4() {
        let mut input = base_input(TYPE_SEZONSKO);
        input.ends_on = Some("2026-09-03T00:00:00Z".to_string()); // 61 days
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h4"));
    }

    #[test]
    fn akcijska_over_31_days_fails_h5() {
        let mut input = base_input(TYPE_AKCIJSKA);
        input.starts_on = "2026-05-01T00:00:00Z".to_string();
        input.ends_on = Some("2026-06-01T00:00:00Z".to_string()); // 32 days
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h5"));
    }

    #[test]
    fn percentage_display_needs_akcijska_of_three_days_or_less() {
        let mut four_day = base_input(TYPE_AKCIJSKA);
        four_day.starts_on = "2026-05-01T00:00:00Z".to_string();
        four_day.ends_on = Some("2026-05-04T00:00:00Z".to_string()); // 4 days
        four_day.display_mode = "percentage".to_string();
        assert!(codes(&validate_shape(&four_day).expect("validate")).contains(&"h6"));

        let mut three_day = base_input(TYPE_AKCIJSKA);
        three_day.starts_on = "2026-05-01T00:00:00Z".to_string();
        three_day.ends_on = Some("2026-05-03T00:00:00Z".to_string()); // 3 days
        three_day.display_mode = "percentage".to_string();
        assert!(!codes(&validate_shape(&three_day).expect("validate")).contains(&"h6"));

        let mut sezonsko_pct = base_input(TYPE_SEZONSKO);
        sezonsko_pct.display_mode = "percentage".to_string();
        assert!(codes(&validate_shape(&sezonsko_pct).expect("validate")).contains(&"h6"));
    }

    #[test]
    fn promotivna_needs_end_date_within_60_days() {
        let mut input = base_input(TYPE_PROMOTIVNA);
        input.season_attested = false;
        input.ends_on = Some("2026-09-03T00:00:00Z".to_string()); // 61 days
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h7d"));
    }

    #[test]
    fn rasprodaja_requires_a_statutory_ground_and_allows_open_end() {
        let mut input = base_input(TYPE_RASPRODAJA);
        input.season_attested = false;
        input.separation_attested = true;
        input.ends_on = None; // "dok traju zalihe"
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h10"));

        input.rasprodaja_ground = Some("prestanak_prodaje_robe".to_string());
        let violations = validate_shape(&input).expect("validate");
        assert!(!codes(&violations).contains(&"h10"));
        assert!(
            !codes(&violations).contains(&"h14b"),
            "rasprodaja may omit ends_on"
        );
    }

    #[test]
    fn non_rasprodaja_missing_end_date_fails_h14b() {
        let mut input = base_input(TYPE_AKCIJSKA);
        input.ends_on = None;
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h14b"));
    }

    #[test]
    fn attestations_gate_sezonsko_and_rasprodaja() {
        let mut sezonsko = base_input(TYPE_SEZONSKO);
        sezonsko.season_attested = false;
        assert!(codes(&validate_shape(&sezonsko).expect("validate")).contains(&"h12a"));

        let mut rasprodaja = base_input(TYPE_RASPRODAJA);
        rasprodaja.rasprodaja_ground = Some("prestanak_poslovanja".to_string());
        rasprodaja.ends_on = None;
        rasprodaja.separation_attested = false;
        assert!(codes(&validate_shape(&rasprodaja).expect("validate")).contains(&"h12b"));
    }

    #[test]
    fn unknown_type_and_empty_items_fail() {
        let mut input = base_input("outlet");
        input.items.clear();
        let violations = validate_shape(&input).expect("validate");
        assert!(codes(&violations).contains(&"h1"));
        assert!(codes(&violations).contains(&"h14d"));
    }
}
