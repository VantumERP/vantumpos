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
use crate::price_history::{compute_prethodna_cena, IncomputableReason, PrethodnaCenaResult};
use rusqlite::{params, Connection, OptionalExtension};
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
const MSG_H7A: &str = "Promotivna prodaja je samo za robu koja se prvi put uvodi u ponudu — artikal je već bio u ponudi (čl. 36 st. 9).";
const MSG_H7B: &str =
    "Za promotivnu prodaju unesite redovnu cenu koja će važiti nakon isteka (čl. 36 st. 9).";
const MSG_H7C: &str =
    "Artikal za promotivnu prodaju ne sme biti aktivan pre početka — ponudu započinje aktivacija kampanje.";
const MSG_H7D: &str = "Promotivna prodaja može trajati najviše 60 dana (čl. 36 st. 9).";
const MSG_H9: &str = "Snižena cena mora biti niža od prethodne cene (čl. 37 st. 6/8/10).";
const MSG_H10: &str = "Za rasprodaju izaberite jedan od tri zakonska osnova (čl. 37 st. 6).";
const MSG_H12A: &str = "Potvrdite da je sezona protekla (čl. 37 st. 8).";
const MSG_H12B: &str = "Potvrdite da je roba na rasprodaji fizički izdvojena (čl. 37 st. 7).";
const MSG_H13A: &str = "Za ovaj artikal prethodna cena mora biti uneta ručno uz obrazloženje.";
const MSG_H13B: &str = "Artikal nije u ponudi — sniženje je moguće samo za aktivne artikle.";
const MSG_H14A: &str = "Datum početka nije ispravan.";
const MSG_H14B: &str =
    "Datum isteka je obavezan (osim za rasprodaju — „dok traju zalihe\") (čl. 36 st. 2 t. 3).";
const MSG_H14C: &str = "Datum isteka mora biti posle datuma početka.";
const MSG_H14D: &str = "Kampanja mora imati bar jedan artikal.";
const MSG_H14E: &str = "Način isticanja nije ispravan.";
const MSG_H14F: &str = "Artikal nije pronađen.";

/// čl. 37 st. 8 caps sezonsko at two per calendar year, counted by START date.
fn msg_h3(year: i32) -> String {
    format!(
        "Sezonsko sniženje je dozvoljeno najviše dva puta godišnje — u {year}. su već održana dva (čl. 37 st. 8; broji se po datumu početka u kalendarskoj godini)."
    )
}

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

    fn for_product(code: &'static str, message: &str, product_id: i64) -> Self {
        Self {
            code,
            message: message.to_string(),
            product_id: Some(product_id),
        }
    }
}

/// The čl. 37 st. 5 anchor as captured for one item. `anchor_status` is
/// `"none"` ONLY for promotivna prodaja (čl. 36 st. 9 defines it against a
/// *future* regular price, so it has no prethodna cena to anchor against and
/// must never pass through the st. 3–4 validator).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemAnchor {
    pub product_id: i64,
    pub anchor_status: &'static str,
    pub prethodna_cena_minor: Option<i64>,
    pub anchor_window_days: Option<i64>,
    pub anchor_truncated: bool,
    pub anchor_reason: Option<String>,
    pub anchor_justification: Option<String>,
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

/// The manual branch of H13a. Reached when the anchor cannot be *computed*
/// lawfully — perishable goods, or any `IncomputableReason`. The duty to
/// display a prethodna cena is never suppressed by the software's inability to
/// derive one: we demand a human figure plus a written justification instead.
fn manual_anchor(
    item: &CampaignItemInput,
    reason: &'static str,
    violations: &mut Vec<Violation>,
) -> ItemAnchor {
    let justification = item
        .anchor_justification
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());

    match (item.manual_prethodna_minor, justification) {
        (Some(price_minor), Some(text)) => ItemAnchor {
            product_id: item.product_id,
            anchor_status: "manual",
            prethodna_cena_minor: Some(price_minor),
            anchor_window_days: None,
            anchor_truncated: false,
            anchor_reason: Some(reason.to_string()),
            anchor_justification: Some(text.to_string()),
        },
        _ => {
            violations.push(Violation::for_product("h13a", MSG_H13A, item.product_id));
            ItemAnchor {
                product_id: item.product_id,
                anchor_status: "manual",
                prethodna_cena_minor: None,
                anchor_window_days: None,
                anchor_truncated: false,
                anchor_reason: Some(reason.to_string()),
                anchor_justification: None,
            }
        }
    }
}

/// Captures the čl. 37 st. 5 anchor for every item, as of `input.starts_on`.
///
/// This is the DRAFT-time snapshot. St. 5 freezes the anchor at activation, so
/// callers must never re-run this against a campaign whose `activated_at` is
/// set — a draft is unannounced and may be re-snapshotted freely.
pub fn snapshot_anchors(
    conn: &Connection,
    input: &CampaignInput,
) -> Result<(Vec<ItemAnchor>, Vec<Violation>), AppError> {
    let mut anchors = Vec::new();
    let mut violations = Vec::new();

    for item in &input.items {
        let perishable: Option<i64> = conn
            .query_row(
                "SELECT perishable FROM products WHERE id = ?1",
                params![item.product_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(perishable) = perishable else {
            violations.push(Violation::for_product("h14f", MSG_H14F, item.product_id));
            continue;
        };

        // čl. 36 st. 9: promotivna prodaja is not a sniženje. It has no
        // prethodna cena at all — computing one would invent a comparison the
        // statute does not make.
        if input.campaign_type == TYPE_PROMOTIVNA {
            anchors.push(ItemAnchor {
                product_id: item.product_id,
                anchor_status: "none",
                prethodna_cena_minor: None,
                anchor_window_days: None,
                anchor_truncated: false,
                anchor_reason: None,
                anchor_justification: None,
            });
            continue;
        }

        // Perishability short-circuits the computation even when a window
        // price exists: the log's price for perishable goods reflects
        // end-of-life markdowns, not the st. 3 „najniža cena" the shopper is
        // being invited to compare against.
        if perishable == 1 {
            anchors.push(manual_anchor(item, "perishable", &mut violations));
            continue;
        }

        match compute_prethodna_cena(conn, item.product_id, &input.starts_on)? {
            PrethodnaCenaResult::Computed(computed) => anchors.push(ItemAnchor {
                product_id: item.product_id,
                anchor_status: "computed",
                prethodna_cena_minor: Some(computed.price_minor),
                anchor_window_days: Some(computed.window_days),
                anchor_truncated: computed.truncated,
                anchor_reason: None,
                anchor_justification: None,
            }),
            PrethodnaCenaResult::Incomputable(reason) => {
                let reason = match reason {
                    IncomputableReason::TooNewInAssortment { .. } => "too_new_in_assortment",
                    IncomputableReason::NotOfferedInWindow => "not_offered_in_window",
                    IncomputableReason::NoHistory => "no_history",
                };
                anchors.push(manual_anchor(item, reason, &mut violations));
            }
        }
    }

    Ok((anchors, violations))
}

/// The rules that need the database: the sezonsko quota, promotivna's
/// never-before-offered gate, and the below-the-anchor rule.
///
/// `exclude_campaign_id` lets a campaign re-validate without counting itself.
pub fn validate_against_db(
    conn: &Connection,
    input: &CampaignInput,
    anchors: &[ItemAnchor],
    exclude_campaign_id: Option<i64>,
) -> Result<Vec<Violation>, AppError> {
    let mut violations = Vec::new();

    // h3 — čl. 37 st. 8: at most two sezonska sniženja per calendar year,
    // counted by START date. Only ACTIVATED campaigns consume the quota: a
    // draft was never announced and a cancelled one never happened. The year
    // bounds are built in Rust and compared lexicographically against RFC3339
    // — `strftime` on the string would be a second date parser to get wrong.
    if input.campaign_type == TYPE_SEZONSKO {
        if let Ok(start) = parse_rfc3339(&input.starts_on, "startsOn") {
            let year = start.date().year();
            let from = format!("{year}-01-01");
            let to = format!("{}-01-01", year + 1);
            let activated: i64 = conn.query_row(
                "SELECT COUNT(*) FROM campaigns
                 WHERE campaign_type = 'sezonsko_snizenje'
                   AND activated_at IS NOT NULL
                   AND starts_on >= ?1
                   AND starts_on < ?2
                   AND id != COALESCE(?3, -1)",
                params![from, to, exclude_campaign_id],
                |row| row.get(0),
            )?;
            if activated >= 2 {
                violations.push(Violation::new("h3", &msg_h3(year)));
            }
        }
    }

    for item in &input.items {
        let active: Option<i64> = conn
            .query_row(
                "SELECT active FROM products WHERE id = ?1",
                params![item.product_id],
                |row| row.get(0),
            )
            .optional()?;
        // A missing product is h14f, already reported by `snapshot_anchors`.
        let Some(active) = active else { continue };
        let active = active == 1;

        if input.campaign_type == TYPE_PROMOTIVNA {
            // h7a — čl. 36 st. 9's „prvi put uvodi u ponudu" is exact: ANY
            // non-NULL offered price before the start disqualifies. No
            // tolerance window; a NULL row is an offering gap, not an offer.
            let previously_offered: i64 = conn.query_row(
                "SELECT EXISTS (
                    SELECT 1 FROM price_history
                    WHERE product_id = ?1 AND price_minor IS NOT NULL AND effective_from < ?2
                 )",
                params![item.product_id, input.starts_on],
                |row| row.get(0),
            )?;
            if previously_offered == 1 {
                violations.push(Violation::for_product("h7a", MSG_H7A, item.product_id));
            }
            // h7c — the offering must BEGIN with the campaign, so the item
            // cannot already be on sale.
            if active {
                violations.push(Violation::for_product("h7c", MSG_H7C, item.product_id));
            }
            // h7b — the future regular price is what makes promotivna legible.
            if item
                .future_regular_price_minor
                .is_none_or(|price| price <= 0)
            {
                violations.push(Violation::for_product("h7b", MSG_H7B, item.product_id));
            }
            continue;
        }

        // h13b — a sniženje reduces a price that is currently offered.
        if !active {
            violations.push(Violation::for_product("h13b", MSG_H13B, item.product_id));
        }

        // h9 — čl. 37 st. 6/8/10: STRICTLY below. An equal price is not a
        // sniženje. Skipped when the anchor is missing: h13a already fired and
        // a second violation would only obscure the fix.
        if let Some(prethodna_cena_minor) = anchors
            .iter()
            .find(|anchor| anchor.product_id == item.product_id)
            .and_then(|anchor| anchor.prethodna_cena_minor)
        {
            if item.campaign_price_minor >= prethodna_cena_minor {
                violations.push(Violation::for_product("h9", MSG_H9, item.product_id));
            }
        }
    }

    Ok(violations)
}

/// The whole mechanical rule set: shape + anchor snapshot + DB rules, every
/// violation merged so the wizard shows the user all of it at once.
///
/// An empty violation list means „no rule we can mechanically check was
/// broken". It is NOT a finding that the promotion is lawful — čl. 38 st. 4
/// (misleading commercial practice) can bite when every number here is right.
pub fn validate_campaign(
    conn: &Connection,
    input: &CampaignInput,
    exclude_campaign_id: Option<i64>,
) -> Result<(Vec<ItemAnchor>, Vec<Violation>), AppError> {
    let mut violations = validate_shape(input)?;
    let (anchors, anchor_violations) = snapshot_anchors(conn, input)?;
    violations.extend(anchor_violations);
    violations.extend(validate_against_db(
        conn,
        input,
        &anchors,
        exclude_campaign_id,
    )?);
    Ok((anchors, violations))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};
    use rusqlite::params;

    fn with_campaign_db(test_name: &str, test: impl FnOnce(&Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("db init");
            let connection = db.open().expect("open");
            connection
                .execute_batch(
                    "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                     VALUES (1, 'PDV 20', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');",
                )
                .expect("seed tax rate");
            test(&connection);
        }
        std::fs::remove_file(&path).expect("cleanup");
    }

    fn seed_product(conn: &Connection, id: i64, price: i64, active: bool, created_at: &str) {
        conn.execute(
            "INSERT INTO products (id, name, sku, sale_price_minor, purchase_price_minor,
                                   tax_rate_id, minimum_stock_milli, active, created_at, updated_at)
             VALUES (?1, ?2, ?2, ?3, 0, 1, 0, ?4, ?5, ?5)",
            params![
                id,
                format!("P{id}"),
                price,
                if active { 1 } else { 0 },
                created_at
            ],
        )
        .expect("seed product");
        if active {
            conn.execute(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (?1, ?2, ?3, 'create', ?2)",
                params![id, created_at, price],
            )
            .expect("seed history");
        }
    }

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

    #[test]
    fn snapshot_computes_anchor_for_established_item() {
        with_campaign_db("anchor_computed", |conn| {
            seed_product(conn, 1, 1290000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 990000;
            let (anchors, violations) = snapshot_anchors(conn, &input).expect("snapshot");
            assert!(violations.is_empty(), "{violations:?}");
            assert_eq!(anchors[0].anchor_status, "computed");
            assert_eq!(anchors[0].prethodna_cena_minor, Some(1290000));
        });
    }

    #[test]
    fn perishable_item_requires_manual_anchor_with_justification() {
        with_campaign_db("anchor_perishable", |conn| {
            seed_product(conn, 1, 50000, true, "2026-01-01T00:00:00Z");
            conn.execute("UPDATE products SET perishable = 1 WHERE id = 1", [])
                .expect("mark perishable");

            let input = base_input(TYPE_AKCIJSKA);
            let (_, violations) = snapshot_anchors(conn, &input).expect("snapshot");
            assert!(codes(&violations).contains(&"h13a"));

            let mut with_manual = base_input(TYPE_AKCIJSKA);
            with_manual.items[0].manual_prethodna_minor = Some(48000);
            with_manual.items[0].anchor_justification =
                Some("Cena sa police, rok trajanja 3 dana".to_string());
            let (anchors, violations) = snapshot_anchors(conn, &with_manual).expect("snapshot");
            assert!(violations.is_empty(), "{violations:?}");
            assert_eq!(anchors[0].anchor_status, "manual");
            assert_eq!(anchors[0].prethodna_cena_minor, Some(48000));
            assert_eq!(anchors[0].anchor_reason.as_deref(), Some("perishable"));
        });
    }

    #[test]
    fn below_anchor_rule_rejects_equal_or_higher_price() {
        with_campaign_db("h9_below_anchor", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 1000000; // equal — not below
            let (anchors, _) = snapshot_anchors(conn, &input).expect("snapshot");
            let violations = validate_against_db(conn, &input, &anchors, None).expect("validate");
            assert!(codes(&violations).contains(&"h9"));
        });
    }

    #[test]
    fn seasonal_quota_counts_only_activated_campaigns_in_the_start_year() {
        with_campaign_db("h3_quota", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            // Two ACTIVATED seasonal campaigns in 2026 + one draft + one cancelled.
            conn.execute_batch(
                "INSERT INTO campaigns (campaign_type, status, starts_on, ends_on, season_attested, activated_at, created_at, updated_at)
                 VALUES
                 ('sezonsko_snizenje','ended','2026-01-05T00:00:00Z','2026-02-20T00:00:00Z',1,'2026-01-05T08:00:00Z','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                 ('sezonsko_snizenje','active','2026-07-01T00:00:00Z','2026-08-25T00:00:00Z',1,'2026-07-01T08:00:00Z','2026-06-20T00:00:00Z','2026-06-20T00:00:00Z'),
                 ('sezonsko_snizenje','draft','2026-07-02T00:00:00Z','2026-08-01T00:00:00Z',1,NULL,'2026-06-20T00:00:00Z','2026-06-20T00:00:00Z'),
                 ('sezonsko_snizenje','cancelled','2026-07-03T00:00:00Z','2026-08-01T00:00:00Z',1,NULL,'2026-06-20T00:00:00Z','2026-06-20T00:00:00Z');",
            )
            .expect("seed campaigns");

            let mut input = base_input(TYPE_SEZONSKO);
            input.items[0].campaign_price_minor = 900000;
            let (anchors, _) = snapshot_anchors(conn, &input).expect("snapshot");
            let violations = validate_against_db(conn, &input, &anchors, None).expect("validate");
            assert!(
                codes(&violations).contains(&"h3"),
                "third activated-in-2026 must be blocked"
            );

            // A December start in the same year still counts against 2026;
            // a 2027 January start does not.
            let mut next_year = base_input(TYPE_SEZONSKO);
            next_year.starts_on = "2027-01-05T00:00:00Z".to_string();
            next_year.ends_on = Some("2027-02-10T00:00:00Z".to_string());
            next_year.items[0].campaign_price_minor = 900000;
            let (anchors, _) = snapshot_anchors(conn, &next_year).expect("snapshot");
            let violations =
                validate_against_db(conn, &next_year, &anchors, None).expect("validate");
            assert!(!codes(&violations).contains(&"h3"));
        });
    }

    #[test]
    fn promotivna_rejects_previously_offered_or_active_items() {
        with_campaign_db("h7_promotivna", |conn| {
            seed_product(conn, 1, 800000, true, "2026-01-01T00:00:00Z"); // offered: has history
            seed_product(conn, 2, 800000, false, "2026-07-01T00:00:00Z"); // never offered, inactive

            let mut input = base_input(TYPE_PROMOTIVNA);
            input.season_attested = false;
            input.items = vec![
                CampaignItemInput {
                    product_id: 1,
                    campaign_price_minor: 700000,
                    manual_prethodna_minor: None,
                    anchor_justification: None,
                    future_regular_price_minor: Some(900000),
                },
                CampaignItemInput {
                    product_id: 2,
                    campaign_price_minor: 700000,
                    manual_prethodna_minor: None,
                    anchor_justification: None,
                    future_regular_price_minor: Some(900000),
                },
            ];
            let (anchors, _) = snapshot_anchors(conn, &input).expect("snapshot");
            assert!(anchors.iter().all(|anchor| anchor.anchor_status == "none"));
            let violations = validate_against_db(conn, &input, &anchors, None).expect("validate");
            let product_one: Vec<_> = violations
                .iter()
                .filter(|violation| violation.product_id == Some(1))
                .collect();
            assert!(product_one.iter().any(|violation| violation.code == "h7a"));
            assert!(product_one.iter().any(|violation| violation.code == "h7c"));
            assert!(
                !violations
                    .iter()
                    .any(|violation| violation.product_id == Some(2)),
                "never-offered inactive item is exactly what promotivna is for: {violations:?}"
            );
        });
    }

    #[test]
    fn snizenje_on_inactive_item_fails_h13b() {
        with_campaign_db("h13b_inactive", |conn| {
            seed_product(conn, 1, 1000000, false, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 900000;
            let (anchors, _) = snapshot_anchors(conn, &input).expect("snapshot");
            let violations = validate_against_db(conn, &input, &anchors, None).expect("validate");
            assert!(codes(&violations).contains(&"h13b"));
        });
    }
}
