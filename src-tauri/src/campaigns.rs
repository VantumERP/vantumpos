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
use crate::price_history::{
    compute_prethodna_cena, load_offering_state, record_offered_price_change, IncomputableReason,
    OfferingState, PrethodnaCenaResult,
};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use time::format_description::well_known::Rfc3339;
use time::{Date, Duration, OffsetDateTime};

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
/// čl. 37 st. 11's „već" is adversative: the two-price duty is *replaced* by a
/// mandatory duty to state the percentage, never waived alongside it. See the
/// memo §2.6 — „A ≤3-day akcija showing neither is unlawful."
const MSG_H6B: &str =
    "Za isticanje samo procenta unesite jasno određenje procenta sniženja (čl. 37 st. 11).";
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

// Warnings are ADVISORY. Not one of them blocks a save, and their absence is
// never a finding that a promotion is lawful — čl. 38 st. 4 is an open standard
// („zanemarljivo kratak period", „prividno sniženje") that no query settles.
// Every message therefore names the risk and leaves the judgement to the user.
const MSG_W1: &str = "Prethodna cena je važila kraće od 3 dana u referentnom periodu — rizik „zanemarljivo kratkog perioda\" (čl. 38 st. 4).";
const MSG_W2: &str = "Artikal je bio u drugoj kampanji koja je počela u poslednjih 30 dana — ponovljene akcije obaraju prethodnu cenu.";
const MSG_W3: &str =
    "Istaknuti procenat važi za manje od petine aktivnog asortimana (čl. 38 st. 2).";
const MSG_W4: &str = "Evidencija cena ne pokriva ceo referentni period — proverite podatke.";
const MSG_W5: &str = "Na kasi je zabeležena cena niža od izračunate prethodne cene — strože tumačenje „primenjivao\" može zahtevati nižu prethodnu cenu.";
const MSG_W6: &str = "Kampanja je prošla deklarisani datum isteka — završite je i vratite cene.";

/// čl. 38 st. 4 names „zanemarljivo kratak period" and defines nothing. The
/// memo (§5 „Warnings") reads the risk zone as an anchor in force for only 1–2
/// days, so three full days inside the window is the first period we stay quiet
/// about. This is a HEURISTIC for a warning, never a rule: it may not block,
/// and a longer period is not a defence.
const TOKEN_PERIOD_DAYS: i64 = 3;

/// The čl. 37 st. 3 window is 30 days, so a campaign that started within 30 days
/// either side has already pushed this product's offered prices down inside it.
const REPEAT_CAMPAIGN_RADIUS_DAYS: i64 = 30;

/// čl. 38 st. 2: „najmanje jednu petinu robe u asortimanu".
const ASSORTMENT_FRACTION: i64 = 5;

const MSG_CAMPAIGN_INVALID: &str = "Kampanja nije ispravna.";
const MSG_CAMPAIGN_NOT_FOUND: &str = "Kampanja nije pronađena.";
const MSG_DRAFT_ONLY_UPDATE: &str = "Samo nacrt kampanje može da se menja.";
const MSG_DRAFT_ONLY_CANCEL: &str = "Samo nacrt kampanje može da se otkaže.";
const MSG_DRAFT_ONLY_ACTIVATE: &str = "Samo nacrt kampanje može da se aktivira.";
const MSG_ACTIVE_ONLY_STEP: &str = "Cena artikla može da se menja samo na aktivnoj kampanji.";
const MSG_ACTIVE_ONLY_END: &str = "Samo aktivna kampanja može da se završi.";
const MSG_ITEM_NOT_IN_CAMPAIGN: &str = "Artikal nije u ovoj kampanji.";
const MSG_NO_RETURN_PRICE: &str =
    "Za ovaj artikal nije poznata cena na koju se vraća — unesite je ručno.";

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
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
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
    // only type that may run „dok traju zalihe". The parsed date is carried
    // alongside its source string so the caps below can run off
    // `declared_duration_days` itself.
    let end_date = match input.ends_on.as_deref() {
        Some(ends_on) => match parse_rfc3339(ends_on, "endsOn") {
            Ok(end) => Some((end.date(), ends_on)),
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
    //
    // The caps below count days through `declared_duration_days` rather than a
    // second copy of the arithmetic: 60/31/3 are statutory calendar-day counts
    // (čl. 37 st. 9/10/11, čl. 36 st. 9), and two implementations of that count
    // could drift apart — leaving the tested one and the enforcing one at odds.
    let duration_days = match (start_date, end_date) {
        (Some(start), Some((end, _))) if end < start => {
            violations.push(Violation::new("h14c", MSG_H14C));
            None
        }
        // Both strings parsed above, so this cannot fail.
        (Some(_), Some((_, ends_on))) => Some(declared_duration_days(&input.starts_on, ends_on)?),
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

    // h6b — the st. 11 carve-out SUBSTITUTES the percentage for the two prices;
    // it does not waive both. A percentage-only campaign with no percentage
    // declared instructs the shop to display nothing at all.
    if display_mode == DISPLAY_PERCENTAGE && input.headline_percent.is_none() {
        violations.push(Violation::new("h6b", MSG_H6B));
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

/// The čl. 37 st. 3–4 reference window for one item, exactly as
/// `compute_prethodna_cena` framed it: `from` inclusive, `to` exclusive.
struct AnchorWindow {
    from: String,
    to: String,
}

/// Rebuilds the window the anchor was computed over — the SAME interval logic,
/// re-derived from the stored `anchor_window_days` rather than guessed at.
/// Recomputing the window length here would be a second implementation of st.
/// 3–4 to get wrong, and it would drift from the anchor it is supposed to
/// describe. Only a computed anchor HAS a window: a manual one is a human
/// figure with a justification, and promotivna has no anchor at all.
fn anchor_window(
    input: &CampaignInput,
    anchor: &ItemAnchor,
) -> Result<Option<AnchorWindow>, AppError> {
    if anchor.anchor_status != "computed" {
        return Ok(None);
    }
    let Some(window_days) = anchor.anchor_window_days else {
        return Ok(None);
    };
    let start = parse_rfc3339(&input.starts_on, "startsOn")?;
    let from = (start - Duration::days(window_days))
        .format(&Rfc3339)
        .map_err(|source| AppError::InvalidState(format!("Vreme nije dostupno: {source}")))?;
    Ok(Some(AnchorWindow {
        from,
        to: input.starts_on.clone(),
    }))
}

/// How long the anchor price was actually in force INSIDE the window.
///
/// Reads the same LEAD timeline the MIN came from, but fetches the intervals
/// into Rust: the clipping is date arithmetic, and date arithmetic lives in the
/// `time` crate, never in SQL. Time outside the window does not count — an
/// anchor that held for months before the window and two days inside it was, as
/// far as čl. 37 st. 3 is concerned, a two-day price.
fn anchor_in_force(
    conn: &Connection,
    anchor: &ItemAnchor,
    anchor_price_minor: i64,
    window: &AnchorWindow,
) -> Result<Duration, AppError> {
    let mut statement = conn.prepare(
        "WITH timeline AS (
            SELECT price_minor,
                   effective_from AS valid_from,
                   LEAD(effective_from) OVER (
                       PARTITION BY product_id ORDER BY effective_from, id
                   ) AS valid_to
            FROM price_history
            WHERE product_id = ?1
         )
         SELECT valid_from, valid_to
         FROM timeline
         WHERE price_minor = ?2
           AND valid_from < ?4
           AND (valid_to IS NULL OR valid_to > ?3)",
    )?;
    let intervals = statement
        .query_map(
            params![
                anchor.product_id,
                anchor_price_minor,
                window.from,
                window.to
            ],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
        )?
        .collect::<Result<Vec<_>, _>>()?;

    let window_from = parse_rfc3339(&window.from, "windowFrom")?;
    let window_to = parse_rfc3339(&window.to, "windowTo")?;
    let mut total = Duration::ZERO;
    for (valid_from, valid_to) in intervals {
        let from = parse_rfc3339(&valid_from, "effectiveFrom")?.max(window_from);
        let to = match valid_to {
            Some(valid_to) => parse_rfc3339(&valid_to, "effectiveFrom")?.min(window_to),
            None => window_to,
        };
        if to > from {
            total += to - from;
        }
    }
    Ok(total)
}

/// The lowest per-unit price the till actually charged for this product inside
/// the window, in minor units.
///
/// LINE-LEVEL ONLY, and that is the whole caveat: the discount is spread back
/// over the line's quantity, so a one-off manual rebate on a single receipt is
/// indistinguishable here from a genuinely lower offered price. That is why w5
/// hedges („strože tumačenje… može zahtevati") instead of asserting that the
/// anchor is wrong. Voids and returns carry their own `document_type` and are
/// excluded — they are not offers.
fn lowest_till_unit_price(
    conn: &Connection,
    product_id: i64,
    window: &AnchorWindow,
) -> Result<Option<i64>, AppError> {
    let lowest: Option<i64> = conn.query_row(
        "SELECT MIN((si.unit_price_minor * si.quantity_milli - si.discount_minor * 1000)
                    / si.quantity_milli)
         FROM sale_items si
         JOIN sales s ON s.id = si.sale_id
         WHERE si.product_id = ?1
           AND s.document_type = 'sale'
           AND s.created_at >= ?2
           AND s.created_at < ?3",
        params![product_id, window.from, window.to],
        |row| row.get(0),
    )?;
    Ok(lowest)
}

/// w2 — another campaign on the same product that started within 30 days.
///
/// Cancelled campaigns are excluded: a cancelled campaign was never announced
/// and never moved a price, so it cannot have depressed anything. Drafts stay
/// in — the point is to warn BEFORE the two campaigns collide. The radius is
/// built in Rust and compared lexicographically against RFC3339.
fn has_recent_campaign(
    conn: &Connection,
    product_id: i64,
    input: &CampaignInput,
    exclude_campaign_id: Option<i64>,
) -> Result<bool, AppError> {
    let start = parse_rfc3339(&input.starts_on, "startsOn")?;
    let format_bound = |moment: OffsetDateTime| {
        moment
            .format(&Rfc3339)
            .map_err(|source| AppError::InvalidState(format!("Vreme nije dostupno: {source}")))
    };
    let from = format_bound(start - Duration::days(REPEAT_CAMPAIGN_RADIUS_DAYS))?;
    let to = format_bound(start + Duration::days(REPEAT_CAMPAIGN_RADIUS_DAYS))?;

    let exists: i64 = conn.query_row(
        "SELECT EXISTS (
            SELECT 1
            FROM campaign_items
            JOIN campaigns ON campaigns.id = campaign_items.campaign_id
            WHERE campaign_items.product_id = ?1
              AND campaigns.status != 'cancelled'
              AND campaigns.id != COALESCE(?2, -1)
              AND campaigns.starts_on >= ?3
              AND campaigns.starts_on <= ?4
         )",
        params![product_id, exclude_campaign_id, from, to],
        |row| row.get(0),
    )?;
    Ok(exists == 1)
}

/// w3 — čl. 38 st. 2: the headline percent must reach „najmanje jednu petinu
/// robe u asortimanu". Integer arithmetic throughout: the per-item discount is
/// floored, and the fifth is tested as `reaching * 5 < assortment` rather than
/// through a float ratio. It is a CAMPAIGN-level finding — no single item is at
/// fault for the size of the assortment — so `product_id` stays `None`.
///
/// The denominator is the ACTIVE assortment: goods not on offer are not „roba u
/// asortimanu" the shopper can be steered toward.
fn headline_misses_a_fifth(
    conn: &Connection,
    input: &CampaignInput,
    anchors: &[ItemAnchor],
    headline_percent: i64,
) -> Result<bool, AppError> {
    let mut reaching: i64 = 0;
    for item in &input.items {
        let anchor_price = anchors
            .iter()
            .find(|anchor| anchor.product_id == item.product_id)
            .and_then(|anchor| anchor.prethodna_cena_minor)
            .filter(|price| *price > 0);
        // No anchor means no percentage can be claimed for this item at all, so
        // it cannot help carry the headline.
        let Some(anchor_price) = anchor_price else {
            continue;
        };
        let discount_percent = (anchor_price - item.campaign_price_minor) * 100 / anchor_price;
        if discount_percent >= headline_percent {
            reaching += 1;
        }
    }

    let assortment: i64 = conn.query_row(
        "SELECT COUNT(*) FROM products WHERE active = 1",
        [],
        |row| row.get(0),
    )?;
    Ok(reaching * ASSORTMENT_FRACTION < assortment)
}

/// The advisory half of the rule set (W1–W6).
///
/// Warnings NEVER block: every one of them points at a standard the software
/// cannot decide (čl. 38 st. 4's „zanemarljivo kratak period", the reach of
/// „primenjivao", the completeness of the trader's own records). An empty list
/// means nothing we mechanically check flagged — it is not a legal clearance.
///
/// `anchors` are supplied by the caller and are never re-derived here: the
/// detail view passes the FROZEN snapshot (čl. 37 st. 5) while the wizard's
/// dry-run passes a fresh draft snapshot. `exclude_campaign_id` names the
/// campaign this input belongs to — it keeps a campaign from reading as its own
/// repeat (w2), and it is what `now` is measured against (w6): only a campaign
/// that already exists can be past its declared expiry.
pub fn compute_warnings(
    conn: &Connection,
    input: &CampaignInput,
    anchors: &[ItemAnchor],
    exclude_campaign_id: Option<i64>,
    now: &str,
) -> Result<Vec<Violation>, AppError> {
    let mut warnings = Vec::new();

    for anchor in anchors {
        // w4 — say the record is short; never silently compute over the gap.
        if anchor.anchor_truncated {
            warnings.push(Violation::for_product("w4", MSG_W4, anchor.product_id));
        }

        if has_recent_campaign(conn, anchor.product_id, input, exclude_campaign_id)? {
            warnings.push(Violation::for_product("w2", MSG_W2, anchor.product_id));
        }

        // w1 and w5 both speak about the reference window, so they only apply
        // where one exists — i.e. to a computed anchor.
        let (Some(window), Some(anchor_price)) =
            (anchor_window(input, anchor)?, anchor.prethodna_cena_minor)
        else {
            continue;
        };

        if anchor_in_force(conn, anchor, anchor_price, &window)? < Duration::days(TOKEN_PERIOD_DAYS)
        {
            warnings.push(Violation::for_product("w1", MSG_W1, anchor.product_id));
        }

        if lowest_till_unit_price(conn, anchor.product_id, &window)?
            .is_some_and(|charged| charged < anchor_price)
        {
            warnings.push(Violation::for_product("w5", MSG_W5, anchor.product_id));
        }
    }

    // w3 — only a claimed headline percent can miss the fifth.
    if let Some(headline_percent) = input.headline_percent {
        if headline_misses_a_fifth(conn, input, anchors, headline_percent)? {
            warnings.push(Violation::new("w3", MSG_W3));
        }
    }

    // w6 — reuses `is_overdue`, the one predicate that decides what „past the
    // declared expiry" means, so the warning and the view flag cannot disagree.
    if let Some(id) = exclude_campaign_id {
        let stored: Option<(String, Option<String>)> = conn
            .query_row(
                "SELECT status, ends_on FROM campaigns WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((status, ends_on)) = stored {
            if is_overdue(&status, ends_on.as_deref(), now) {
                warnings.push(Violation::new("w6", MSG_W6));
            }
        }
    }

    Ok(warnings)
}

/// Everything the wizard needs to decide, in one pass: what blocks (`hard`),
/// what merely deserves a second look (`warnings`), and the snapshot both were
/// judged against (`anchors`).
///
/// The split is the point. `hard` is empty ⇒ no rule we can mechanically check
/// was broken. It is NOT „this promotion is legal".
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub hard: Vec<Violation>,
    pub warnings: Vec<Violation>,
    pub anchors: Vec<ItemAnchor>,
}

/// The dry run behind the wizard's live feedback. Takes a FRESH snapshot, so it
/// is only ever valid for a draft or an unsaved input — never for an activated
/// campaign, whose anchor is frozen (čl. 37 st. 5). The detail view reads the
/// stored snapshot instead; see `get_campaign`.
pub fn validation_report(
    conn: &Connection,
    input: &CampaignInput,
    exclude_campaign_id: Option<i64>,
    now: &str,
) -> Result<ValidationReport, AppError> {
    let (anchors, hard) = validate_campaign(conn, input, exclude_campaign_id)?;
    let warnings = compute_warnings(conn, input, &anchors, exclude_campaign_id, now)?;
    Ok(ValidationReport {
        hard,
        warnings,
        anchors,
    })
}

/// One item as the wizard and the detail view see it: the stored snapshot plus
/// the product identity, never a recomputation. Reading a campaign must not be
/// able to move the čl. 37 st. 5 anchor.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignItemView {
    pub product_id: i64,
    pub product_name: String,
    pub sku: String,
    pub campaign_price_minor: i64,
    pub prethodna_cena_minor: Option<i64>,
    pub anchor_status: String,
    pub anchor_window_days: Option<i64>,
    pub anchor_truncated: bool,
    pub anchor_reason: Option<String>,
    pub anchor_justification: Option<String>,
    pub future_regular_price_minor: Option<i64>,
    pub pre_campaign_price_minor: Option<i64>,
}

/// `warnings` stays empty here; Task 6 fills it. An empty list is „nothing we
/// mechanically check flagged", never „this promotion is lawful" (čl. 38 st. 4).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignView {
    pub id: i64,
    pub campaign_type: String,
    pub status: String,
    pub starts_on: String,
    pub ends_on: Option<String>,
    pub display_mode: String,
    pub headline_percent: Option<i64>,
    pub rasprodaja_ground: Option<String>,
    pub special_conditions: Option<String>,
    pub reduced_utility_reason: Option<String>,
    pub marketing_label: Option<String>,
    pub season_attested: bool,
    pub separation_attested: bool,
    pub activated_at: Option<String>,
    pub ended_at: Option<String>,
    pub overdue: bool,
    pub items: Vec<CampaignItemView>,
    pub warnings: Vec<Violation>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignSummary {
    pub id: i64,
    pub campaign_type: String,
    pub status: String,
    pub starts_on: String,
    pub ends_on: Option<String>,
    pub marketing_label: Option<String>,
    pub item_count: i64,
    pub overdue: bool,
}

/// An active campaign past its declared `ends_on`. There is deliberately no
/// auto-end: prices stay until a human ends the campaign, and this flag is what
/// makes that visible (čl. 36 st. 2 t. 3).
///
/// `ends_on` is the declared `datum isteka` and is **inclusive** — it names the
/// last day OF the `period važenja`, exactly as `declared_duration_days` counts
/// it. So the cutoff is the first instant of the FOLLOWING day: a campaign is
/// not overdue at any point during its declared end day. Comparing `ends_on <
/// now` instead would raise the flag from 00:00:01 on the end day and make the
/// UI order a revert while the promotion is still lawfully running.
///
/// Both sides are RFC3339 UTC, so the lexicographic compare IS the
/// chronological one. An unparseable `ends_on` cannot come from the DB (we only
/// ever write RFC3339); `false` is the conservative fallback — it withholds the
/// revert prompt rather than issuing it early.
fn is_overdue(status: &str, ends_on: Option<&str>, now: &str) -> bool {
    if status != "active" {
        return false;
    }
    let Some(ends_on) = ends_on else {
        return false;
    };
    let Ok(cutoff) = parse_rfc3339(ends_on, "endsOn").map(|end| end + Duration::days(1)) else {
        return false;
    };
    let Ok(cutoff) = cutoff.format(&Rfc3339) else {
        return false;
    };
    now >= cutoff.as_str()
}

/// Rejects on ANY hard violation, handing the whole set to the wizard in the
/// error details so the user fixes everything at once instead of one per save.
fn validated_anchors(
    conn: &Connection,
    input: &CampaignInput,
    exclude_campaign_id: Option<i64>,
) -> Result<Vec<ItemAnchor>, AppError> {
    let (anchors, violations) = validate_campaign(conn, input, exclude_campaign_id)?;
    if violations.is_empty() {
        Ok(anchors)
    } else {
        Err(AppError::validation(
            MSG_CAMPAIGN_INVALID,
            serde_json::json!({ "violations": violations }),
        ))
    }
}

/// The anchor freeze (čl. 37 st. 5) rests on this gate: a draft is unannounced
/// and may be re-snapshotted freely, an activated campaign never may. Callers
/// run it INSIDE their transaction so the check and the write cannot straddle a
/// concurrent activation.
fn require_draft(conn: &Connection, id: i64, message: &str) -> Result<(), AppError> {
    require_status(conn, id, "draft", message)
}

/// Reads the status inside the caller's transaction so the gate and the write
/// cannot straddle a concurrent lifecycle transition.
fn require_status(
    conn: &Connection,
    id: i64,
    expected: &str,
    message: &str,
) -> Result<(), AppError> {
    let status: Option<String> = conn
        .query_row(
            "SELECT status FROM campaigns WHERE id = ?1",
            params![id],
            |row| row.get(0),
        )
        .optional()?;
    let status = status.ok_or_else(|| AppError::not_found(MSG_CAMPAIGN_NOT_FOUND))?;
    if status == expected {
        Ok(())
    } else {
        Err(AppError::business("invalid_state", message))
    }
}

/// Writes the snapshot taken by `validate_campaign` — never a fresh one, so the
/// stored anchor is exactly the one the rules were checked against.
/// `pre_campaign_price_minor` stays NULL until activation captures it.
fn insert_items(
    tx: &Transaction<'_>,
    campaign_id: i64,
    input: &CampaignInput,
    anchors: &[ItemAnchor],
) -> Result<(), AppError> {
    for item in &input.items {
        let anchor = anchors
            .iter()
            .find(|anchor| anchor.product_id == item.product_id)
            .ok_or_else(|| AppError::not_found(MSG_H14F))?;
        tx.execute(
            "INSERT INTO campaign_items (
                campaign_id, product_id, campaign_price_minor, prethodna_cena_minor,
                anchor_status, anchor_window_days, anchor_truncated, anchor_reason,
                anchor_justification, future_regular_price_minor
             )
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                campaign_id,
                item.product_id,
                item.campaign_price_minor,
                anchor.prethodna_cena_minor,
                anchor.anchor_status,
                anchor.anchor_window_days,
                i64::from(anchor.anchor_truncated),
                anchor.anchor_reason,
                anchor.anchor_justification,
                item.future_regular_price_minor,
            ],
        )?;
    }

    Ok(())
}

/// Creates a draft. Hard violations refuse the write outright — a campaign that
/// breaks čl. 36/37 must not exist even as a draft to be activated later.
pub fn create_campaign(
    conn: &mut Connection,
    input: &CampaignInput,
    acting_user_id: i64,
    now: &str,
) -> Result<CampaignView, AppError> {
    let tx = conn.transaction()?;
    let anchors = validated_anchors(&tx, input, None)?;
    tx.execute(
        "INSERT INTO campaigns (
            campaign_type, status, starts_on, ends_on, display_mode, headline_percent,
            rasprodaja_ground, special_conditions, reduced_utility_reason, marketing_label,
            season_attested, separation_attested, created_by, created_at, updated_at
         )
         VALUES (?1, 'draft', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?13)",
        params![
            input.campaign_type,
            input.starts_on,
            input.ends_on,
            input.display_mode,
            input.headline_percent,
            input.rasprodaja_ground,
            input.special_conditions,
            input.reduced_utility_reason,
            input.marketing_label,
            i64::from(input.season_attested),
            i64::from(input.separation_attested),
            acting_user_id,
            now,
        ],
    )?;
    let id = tx.last_insert_rowid();
    insert_items(&tx, id, input, &anchors)?;
    tx.commit()?;

    get_campaign(conn, id, now)
}

/// Draft-only edit. Items are replaced wholesale and their anchors re-snapshot:
/// a draft was never announced, so no shopper has seen the old figures — draft
/// items are not evidence, `price_history` is.
pub fn update_campaign(
    conn: &mut Connection,
    id: i64,
    input: &CampaignInput,
    now: &str,
) -> Result<CampaignView, AppError> {
    let tx = conn.transaction()?;
    require_draft(&tx, id, MSG_DRAFT_ONLY_UPDATE)?;
    let anchors = validated_anchors(&tx, input, Some(id))?;
    tx.execute(
        "UPDATE campaigns
         SET campaign_type = ?2,
             starts_on = ?3,
             ends_on = ?4,
             display_mode = ?5,
             headline_percent = ?6,
             rasprodaja_ground = ?7,
             special_conditions = ?8,
             reduced_utility_reason = ?9,
             marketing_label = ?10,
             season_attested = ?11,
             separation_attested = ?12,
             updated_at = ?13
         WHERE id = ?1",
        params![
            id,
            input.campaign_type,
            input.starts_on,
            input.ends_on,
            input.display_mode,
            input.headline_percent,
            input.rasprodaja_ground,
            input.special_conditions,
            input.reduced_utility_reason,
            input.marketing_label,
            i64::from(input.season_attested),
            i64::from(input.separation_attested),
            now,
        ],
    )?;
    tx.execute(
        "DELETE FROM campaign_items WHERE campaign_id = ?1",
        params![id],
    )?;
    insert_items(&tx, id, input, &anchors)?;
    tx.commit()?;

    get_campaign(conn, id, now)
}

/// Draft-only, and touches no prices: a draft never moved any.
pub fn cancel_campaign(
    conn: &mut Connection,
    id: i64,
    now: &str,
) -> Result<CampaignView, AppError> {
    let tx = conn.transaction()?;
    require_draft(&tx, id, MSG_DRAFT_ONLY_CANCEL)?;
    tx.execute(
        "UPDATE campaigns SET status = 'cancelled', updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    tx.commit()?;

    get_campaign(conn, id, now)
}

/// One product's return price at the end of a campaign.
#[derive(Debug, Clone, Copy, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndOverride {
    pub product_id: i64,
    pub return_price_minor: i64,
}

/// Rebuilds the `CampaignInput` that the stored campaign represents, so
/// activation can re-run the whole rule set against today's world.
///
/// Only a MANUAL anchor round-trips as an *input*: a computed one is derived
/// from `price_history`, and feeding it back in would launder a stale
/// computation into a user-attested figure (čl. 37 st. 5).
fn load_input(conn: &Connection, id: i64) -> Result<CampaignInput, AppError> {
    let input: Option<CampaignInput> = conn
        .query_row(
            "SELECT campaign_type, starts_on, ends_on, display_mode, headline_percent,
                    rasprodaja_ground, special_conditions, reduced_utility_reason,
                    marketing_label, season_attested, separation_attested
             FROM campaigns
             WHERE id = ?1",
            params![id],
            |row| {
                Ok(CampaignInput {
                    campaign_type: row.get(0)?,
                    starts_on: row.get(1)?,
                    ends_on: row.get(2)?,
                    display_mode: row.get(3)?,
                    headline_percent: row.get(4)?,
                    rasprodaja_ground: row.get(5)?,
                    special_conditions: row.get(6)?,
                    reduced_utility_reason: row.get(7)?,
                    marketing_label: row.get(8)?,
                    season_attested: row.get::<_, i64>(9)? == 1,
                    separation_attested: row.get::<_, i64>(10)? == 1,
                    items: Vec::new(),
                })
            },
        )
        .optional()?;
    let mut input = input.ok_or_else(|| AppError::not_found(MSG_CAMPAIGN_NOT_FOUND))?;

    let mut statement = conn.prepare(
        "SELECT product_id, campaign_price_minor, prethodna_cena_minor, anchor_status,
                anchor_justification, future_regular_price_minor
         FROM campaign_items
         WHERE campaign_id = ?1
         ORDER BY id",
    )?;
    input.items = statement
        .query_map(params![id], |row| {
            let stored_anchor: Option<i64> = row.get(2)?;
            let anchor_status: String = row.get(3)?;
            Ok(CampaignItemInput {
                product_id: row.get(0)?,
                campaign_price_minor: row.get(1)?,
                manual_prethodna_minor: match anchor_status.as_str() {
                    "manual" => stored_anchor,
                    _ => None,
                },
                anchor_justification: row.get(4)?,
                future_regular_price_minor: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(input)
}

/// Moves one product's offered price and logs the move in the SAME transaction.
/// Nothing in this module may touch `products.sale_price_minor` without going
/// through here: a price the till charges but the log does not carry destroys
/// the evidence that čl. 37 st. 3–4 requires the shop to keep.
///
/// Returns the before-state and whether what the shop offers actually moved.
/// The second half is SW-12 req. 11's trigger (ZZP čl. 6 st. 3): a sniženje that
/// lowers the shelf price has to reach the published cenovnik too, and it is the
/// log's own answer rather than a second comparison that could drift from it.
fn move_offered_price(
    tx: &Transaction<'_>,
    product_id: i64,
    price_minor: i64,
    active: bool,
    source: &str,
    acting_user_id: i64,
    now: &str,
) -> Result<(OfferingState, bool), AppError> {
    let before =
        load_offering_state(tx, product_id)?.ok_or_else(|| AppError::not_found(MSG_H14F))?;
    tx.execute(
        "UPDATE products SET sale_price_minor = ?2, active = ?3, updated_at = ?4 WHERE id = ?1",
        params![product_id, price_minor, i64::from(active), now],
    )?;
    let offer_changed = record_offered_price_change(
        tx,
        product_id,
        Some(before),
        OfferingState {
            active,
            price_minor,
        },
        source,
        Some(acting_user_id),
        now,
    )?;

    Ok((before, offer_changed))
}

/// Announces the campaign: re-validates, then moves every item's till price in
/// ONE transaction (čl. 36 st. 2 — the offer starts as a whole; a half-applied
/// campaign would advertise prices the till does not charge).
///
/// Re-validation is NOT a re-snapshot. The world may have changed since the
/// draft was written — the quota may be gone, an item may have been pulled —
/// so every rule runs again; but the stored anchors stand exactly as the last
/// draft edit snapshotted them. From this commit on they are frozen: čl. 37
/// st. 5 pins the prethodna cena to what was true when the campaign was set,
/// and no later path in this module writes that column.
pub fn activate_campaign(
    conn: &mut Connection,
    id: i64,
    acting_user_id: i64,
    now: &str,
) -> Result<CampaignView, AppError> {
    let tx = conn.transaction()?;
    require_draft(&tx, id, MSG_DRAFT_ONLY_ACTIVATE)?;
    let input = load_input(&tx, id)?;
    // Validates only — the returned snapshot is deliberately discarded.
    validated_anchors(&tx, &input, Some(id))?;

    let mut prices_moved = false;
    for item in &input.items {
        // Promotivna items go active here — activation IS the first offering
        // (čl. 36 st. 9). Sniženje items are already active (h13b).
        let (before, offer_changed) = move_offered_price(
            &tx,
            item.product_id,
            item.campaign_price_minor,
            true,
            "campaign_start",
            acting_user_id,
            now,
        )?;
        prices_moved |= offer_changed;
        // What `end_campaign` returns the price to, absent an override.
        tx.execute(
            "UPDATE campaign_items SET pre_campaign_price_minor = ?3
             WHERE campaign_id = ?1 AND product_id = ?2",
            params![id, item.product_id, before.price_minor],
        )?;
    }

    tx.execute(
        "UPDATE campaigns SET status = 'active', activated_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    tx.commit()?;
    republish_cenovnik(conn, prices_moved, now);

    get_campaign(conn, id, now)
}

/// SW-12 req. 11 — ZZP čl. 6 st. 3 wants the published cenovnik to match the
/// outlet's current prices *„u realnom vremenu“*, and a campaign moves exactly
/// those. After the commit, never before it: čl. 6 st. 4 binds the shop to what
/// it published, so a target must not be handed a price that could still roll
/// back. A publish failure never fails the campaign — the prices are already
/// durable and the till is already charging them.
fn republish_cenovnik(conn: &Connection, prices_moved: bool, now: &str) {
    if prices_moved {
        crate::commands::cenovnik::republish_after_price_move(conn, now);
    }
}

/// A markdown step on a running campaign (memo §2.7 worked example (b)).
///
/// The step is checked against the STORED anchor and never recomputes it. A
/// lazy recompute here would let the campaign's own earlier price into the
/// window and ratchet the displayed prethodna cena down toward the current
/// price — the memo names that as a breach of st. 5 that understates the
/// discount. This function does not write `prethodna_cena_minor`.
pub fn adjust_item_price(
    conn: &mut Connection,
    campaign_id: i64,
    product_id: i64,
    new_price_minor: i64,
    acting_user_id: i64,
    now: &str,
) -> Result<CampaignView, AppError> {
    let tx = conn.transaction()?;
    require_status(&tx, campaign_id, "active", MSG_ACTIVE_ONLY_STEP)?;

    let item: Option<(String, Option<i64>)> = tx
        .query_row(
            "SELECT campaigns.campaign_type, campaign_items.prethodna_cena_minor
             FROM campaign_items
             JOIN campaigns ON campaigns.id = campaign_items.campaign_id
             WHERE campaign_items.campaign_id = ?1 AND campaign_items.product_id = ?2",
            params![campaign_id, product_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let (campaign_type, prethodna_cena_minor) =
        item.ok_or_else(|| AppError::not_found(MSG_ITEM_NOT_IN_CAMPAIGN))?;

    // h9 — čl. 37 st. 6/8/10, against the frozen anchor. Promotivna has no
    // prethodna cena to be below (čl. 36 st. 9), so the rule does not apply.
    if campaign_type != TYPE_PROMOTIVNA {
        if let Some(anchor) = prethodna_cena_minor {
            if new_price_minor >= anchor {
                return Err(AppError::validation(
                    MSG_H9,
                    serde_json::json!({
                        "violations": [Violation::for_product("h9", MSG_H9, product_id)]
                    }),
                ));
            }
        }
    }

    let (_, prices_moved) = move_offered_price(
        &tx,
        product_id,
        new_price_minor,
        true,
        "campaign_step",
        acting_user_id,
        now,
    )?;
    tx.execute(
        "UPDATE campaign_items SET campaign_price_minor = ?3
         WHERE campaign_id = ?1 AND product_id = ?2",
        params![campaign_id, product_id, new_price_minor],
    )?;
    tx.commit()?;
    republish_cenovnik(conn, prices_moved, now);

    get_campaign(conn, campaign_id, now)
}

/// Ends the campaign and returns every item's till price, in ONE transaction.
///
/// The return price is the override if the user gave one, else the declared
/// future regular price for promotivna (čl. 36 st. 9 — its expiry transitions
/// TO that price; h7b makes its absence impossible), else the price the item
/// carried before activation. There is deliberately no auto-end: prices move
/// only when a human ends the campaign.
pub fn end_campaign(
    conn: &mut Connection,
    id: i64,
    overrides: &[EndOverride],
    acting_user_id: i64,
    now: &str,
) -> Result<CampaignView, AppError> {
    let tx = conn.transaction()?;
    require_status(&tx, id, "active", MSG_ACTIVE_ONLY_END)?;

    let campaign_type: String = tx.query_row(
        "SELECT campaign_type FROM campaigns WHERE id = ?1",
        params![id],
        |row| row.get(0),
    )?;

    let mut statement = tx.prepare(
        "SELECT product_id, future_regular_price_minor, pre_campaign_price_minor
         FROM campaign_items
         WHERE campaign_id = ?1
         ORDER BY id",
    )?;
    let items = statement
        .query_map(params![id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, Option<i64>>(1)?,
                row.get::<_, Option<i64>>(2)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);

    let mut prices_moved = false;
    for (product_id, future_regular_price_minor, pre_campaign_price_minor) in items {
        let return_price_minor = overrides
            .iter()
            .find(|override_| override_.product_id == product_id)
            .map(|override_| override_.return_price_minor)
            .or(if campaign_type == TYPE_PROMOTIVNA {
                future_regular_price_minor
            } else {
                pre_campaign_price_minor
            })
            .ok_or_else(|| AppError::business("invalid_state", MSG_NO_RETURN_PRICE))?;

        let (_, offer_changed) = move_offered_price(
            &tx,
            product_id,
            return_price_minor,
            true,
            "campaign_end",
            acting_user_id,
            now,
        )?;
        prices_moved |= offer_changed;
    }

    tx.execute(
        "UPDATE campaigns SET status = 'ended', ended_at = ?2, updated_at = ?2 WHERE id = ?1",
        params![id, now],
    )?;
    tx.commit()?;
    republish_cenovnik(conn, prices_moved, now);

    get_campaign(conn, id, now)
}

fn load_items(conn: &Connection, campaign_id: i64) -> Result<Vec<CampaignItemView>, AppError> {
    let mut statement = conn.prepare(
        "SELECT campaign_items.product_id,
                products.name,
                products.sku,
                campaign_items.campaign_price_minor,
                campaign_items.prethodna_cena_minor,
                campaign_items.anchor_status,
                campaign_items.anchor_window_days,
                campaign_items.anchor_truncated,
                campaign_items.anchor_reason,
                campaign_items.anchor_justification,
                campaign_items.future_regular_price_minor,
                campaign_items.pre_campaign_price_minor
         FROM campaign_items
         JOIN products ON products.id = campaign_items.product_id
         WHERE campaign_items.campaign_id = ?1
         ORDER BY campaign_items.id",
    )?;
    let items = statement
        .query_map(params![campaign_id], |row| {
            Ok(CampaignItemView {
                product_id: row.get(0)?,
                product_name: row.get(1)?,
                sku: row.get(2)?,
                campaign_price_minor: row.get(3)?,
                prethodna_cena_minor: row.get(4)?,
                anchor_status: row.get(5)?,
                anchor_window_days: row.get(6)?,
                anchor_truncated: row.get::<_, i64>(7)? == 1,
                anchor_reason: row.get(8)?,
                anchor_justification: row.get(9)?,
                future_regular_price_minor: row.get(10)?,
                pre_campaign_price_minor: row.get(11)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(items)
}

pub fn get_campaign(conn: &Connection, id: i64, now: &str) -> Result<CampaignView, AppError> {
    let view: Option<CampaignView> = conn
        .query_row(
            "SELECT id, campaign_type, status, starts_on, ends_on, display_mode, headline_percent,
                    rasprodaja_ground, special_conditions, reduced_utility_reason, marketing_label,
                    season_attested, separation_attested, activated_at, ended_at
             FROM campaigns
             WHERE id = ?1",
            params![id],
            |row| {
                let status: String = row.get(2)?;
                let ends_on: Option<String> = row.get(4)?;
                Ok(CampaignView {
                    id: row.get(0)?,
                    campaign_type: row.get(1)?,
                    starts_on: row.get(3)?,
                    display_mode: row.get(5)?,
                    headline_percent: row.get(6)?,
                    rasprodaja_ground: row.get(7)?,
                    special_conditions: row.get(8)?,
                    reduced_utility_reason: row.get(9)?,
                    marketing_label: row.get(10)?,
                    season_attested: row.get::<_, i64>(11)? == 1,
                    separation_attested: row.get::<_, i64>(12)? == 1,
                    activated_at: row.get(13)?,
                    ended_at: row.get(14)?,
                    overdue: is_overdue(&status, ends_on.as_deref(), now),
                    status,
                    ends_on,
                    items: Vec::new(),
                    warnings: Vec::new(),
                })
            },
        )
        .optional()?;
    let mut view = view.ok_or_else(|| AppError::not_found(MSG_CAMPAIGN_NOT_FOUND))?;
    view.items = load_items(conn, id)?;
    // The warnings are computed against the STORED anchors, never a fresh
    // snapshot: čl. 37 st. 5 freezes the anchor at activation, and merely
    // OPENING the campaign must not be able to move it. After activation the
    // offered price IS the campaign price, so a re-snapshot here would anchor
    // the campaign on itself and quietly report a 0% discount as fine.
    let anchors = stored_anchors(&view.items);
    view.warnings = compute_warnings(conn, &load_input(conn, id)?, &anchors, Some(id), now)?;

    Ok(view)
}

/// Reads the frozen snapshot back out of `campaign_items` in the shape the
/// warning pass expects. This is a projection, not a computation — nothing here
/// may consult `price_history`.
fn stored_anchors(items: &[CampaignItemView]) -> Vec<ItemAnchor> {
    items
        .iter()
        .map(|item| ItemAnchor {
            product_id: item.product_id,
            // `campaign_items.anchor_status` is CHECK-constrained to exactly
            // these three, so the fallback is unreachable; „none" is the
            // conservative one — it claims no window and no anchor.
            anchor_status: match item.anchor_status.as_str() {
                "computed" => "computed",
                "manual" => "manual",
                _ => "none",
            },
            prethodna_cena_minor: item.prethodna_cena_minor,
            anchor_window_days: item.anchor_window_days,
            anchor_truncated: item.anchor_truncated,
            anchor_reason: item.anchor_reason.clone(),
            anchor_justification: item.anchor_justification.clone(),
        })
        .collect()
}

pub fn list_campaigns(conn: &Connection, now: &str) -> Result<Vec<CampaignSummary>, AppError> {
    let mut statement = conn.prepare(
        "SELECT campaigns.id,
                campaigns.campaign_type,
                campaigns.status,
                campaigns.starts_on,
                campaigns.ends_on,
                campaigns.marketing_label,
                (SELECT COUNT(*) FROM campaign_items WHERE campaign_items.campaign_id = campaigns.id)
         FROM campaigns
         ORDER BY campaigns.starts_on DESC, campaigns.id DESC",
    )?;
    let summaries = statement
        .query_map([], |row| {
            let status: String = row.get(2)?;
            let ends_on: Option<String> = row.get(4)?;
            Ok(CampaignSummary {
                id: row.get(0)?,
                campaign_type: row.get(1)?,
                starts_on: row.get(3)?,
                marketing_label: row.get(5)?,
                item_count: row.get(6)?,
                overdue: is_overdue(&status, ends_on.as_deref(), now),
                status,
                ends_on,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(summaries)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};
    use rusqlite::params;

    fn with_campaign_db(test_name: &str, test: impl FnOnce(&Connection)) {
        with_campaign_db_mut(test_name, |connection| test(connection));
    }

    /// Persistence takes `&mut Connection` (it opens transactions), so the
    /// read-only fixture reborrows out of this one.
    fn with_campaign_db_mut(test_name: &str, test: impl FnOnce(&mut Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("db init");
            let mut connection = db.open().expect("open");
            connection
                .execute_batch(
                    "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                     VALUES (1, 'PDV 20', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');",
                )
                .expect("seed tax rate");
            test(&mut connection);
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

    /// čl. 37 st. 11 substitutes the percentage for the two prices — it never
    /// waives both. A lawful ≤3-day akcija must still say a number.
    #[test]
    fn percentage_display_without_a_percentage_fails_h6b() {
        let mut bare = base_input(TYPE_AKCIJSKA);
        bare.starts_on = "2026-05-01T00:00:00Z".to_string();
        bare.ends_on = Some("2026-05-03T00:00:00Z".to_string()); // 3 days
        bare.display_mode = "percentage".to_string();
        bare.headline_percent = None;

        let bare_codes = codes(&validate_shape(&bare).expect("validate"));
        // The duration is lawful, so h6 must stay silent — h6b is the finding.
        assert!(!bare_codes.contains(&"h6"));
        assert!(bare_codes.contains(&"h6b"));

        let mut declared = bare.clone();
        declared.headline_percent = Some(20);
        assert!(!codes(&validate_shape(&declared).expect("validate")).contains(&"h6b"));

        // Two-price display carries no percentage duty at all (st. 2).
        let mut two_prices = base_input(TYPE_AKCIJSKA);
        two_prices.headline_percent = None;
        assert!(!codes(&validate_shape(&two_prices).expect("validate")).contains(&"h6b"));
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

    #[test]
    fn create_rejects_hard_violations_with_details() {
        with_campaign_db_mut("create_rejects_mut", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 1000000; // h9
            let error = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z")
                .expect_err("h9 must block create");
            assert_eq!(error.code(), "validation_error");
        });
    }

    #[test]
    fn create_then_get_round_trips_with_frozen_snapshot() {
        with_campaign_db_mut("create_get_round_trip", |conn| {
            seed_product(conn, 1, 1290000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 990000;
            let created = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z").expect("create");
            assert_eq!(created.status, "draft");
            assert_eq!(created.items[0].prethodna_cena_minor, Some(1290000));

            let fetched = get_campaign(conn, created.id, "2026-07-01T00:00:00Z").expect("get");
            assert_eq!(fetched.items.len(), 1);
            assert_eq!(fetched.items[0].anchor_status, "computed");
            assert!(!fetched.overdue);
        });
    }

    #[test]
    fn update_is_draft_only_and_resnapshots() {
        with_campaign_db_mut("update_draft_only", |conn| {
            seed_product(conn, 1, 1290000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 990000;
            let created = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z").expect("create");

            // Cheaper price appears before the draft is edited: re-snapshot must see it.
            conn.execute(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-06-20T00:00:00Z', 1190000, 'update', '2026-06-20T00:00:00Z')",
                [],
            )
            .expect("history row");
            let updated =
                update_campaign(conn, created.id, &input, "2026-07-02T00:00:00Z").expect("update");
            assert_eq!(
                updated.items[0].prethodna_cena_minor,
                Some(1190000),
                "draft edit re-snapshots"
            );

            conn.execute(
                "UPDATE campaigns SET status = 'active', activated_at = '2026-07-05T00:00:00Z' WHERE id = ?1",
                params![created.id],
            )
            .expect("force active");
            let error = update_campaign(conn, created.id, &input, "2026-07-06T00:00:00Z")
                .expect_err("active campaign must not be editable");
            assert_eq!(error.code(), "invalid_state");
        });
    }

    #[test]
    fn cancel_is_draft_only_and_overdue_flags_past_end() {
        with_campaign_db_mut("cancel_overdue", |conn| {
            seed_product(conn, 1, 1290000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 990000;
            let created = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z").expect("create");

            conn.execute(
                "UPDATE campaigns SET status='active', activated_at='2026-07-05T00:00:00Z' WHERE id=?1",
                params![created.id],
            )
            .expect("force");
            let error = cancel_campaign(conn, created.id, "2026-07-06T00:00:00Z")
                .expect_err("active cannot cancel");
            assert_eq!(error.code(), "invalid_state");

            let view = get_campaign(conn, created.id, "2026-08-01T00:00:00Z").expect("get");
            assert!(view.overdue, "past declared end while active");
            let list = list_campaigns(conn, "2026-08-01T00:00:00Z").expect("list");
            assert!(list[0].overdue);
        });
    }

    /// `ends_on` is the declared `datum isteka` — the LAST day of the period
    /// važenja (čl. 36 st. 2 t. 3), inclusive exactly as `declared_duration_days`
    /// counts it. The flag drives the "vratite cene" prompt, so firing it during
    /// the end day would order a revert while the promotion is still lawful.
    #[test]
    fn overdue_treats_declared_end_day_as_inclusive() {
        let ends_on = Some("2026-07-20T00:00:00Z");

        assert!(
            !is_overdue("active", ends_on, "2026-07-20T00:00:00Z"),
            "first instant of the declared end day is inside the campaign"
        );
        assert!(
            !is_overdue("active", ends_on, "2026-07-20T10:00:00Z"),
            "the whole declared end day is still lawfully running"
        );
        assert!(
            !is_overdue("active", ends_on, "2026-07-20T23:59:59Z"),
            "last instant of the declared end day is inside the campaign"
        );
        assert!(
            is_overdue("active", ends_on, "2026-07-21T00:00:00Z"),
            "overdue at the first instant of the following day"
        );

        assert!(
            !is_overdue("draft", ends_on, "2026-07-21T00:00:00Z"),
            "only an active campaign can be overdue"
        );
        assert!(
            !is_overdue("active", None, "2026-07-21T00:00:00Z"),
            "no declared end, nothing to be past"
        );
    }

    // Memo worked example (b) — ZOT-36-37-VERIFIED-RULES.md §2.7.
    // JAKNA-Z-L applied/offered: 12.900 (15.05–20.06), 11.900 (21.06–30.06),
    // 12.900 (01.07→). Sezonsko starts 05.07.2026: anchor = 11.900, FROZEN
    // across markdown steps 9.900 → 8.900 → 7.500. Both named wrong
    // implementations must be impossible: 12.900 ("price at first reduction")
    // and lazy recompute (which would yield 9.900 at step 2).
    /// Design §7's mandated invariant, pinned directly rather than relying on
    /// worked_example_b's incidental coverage: once a campaign is activated its
    /// announced prethodna cena is frozen (čl. 37 st. 5). Lazy recompute is the
    /// exact manipulation that stav exists to prevent, so this test drives every
    /// post-activation mutation at an activated campaign and asserts the column
    /// never moves — a guard-rail against a future code path, not today's code.
    #[test]
    fn activated_campaign_anchor_is_immutable_across_every_mutation() {
        with_campaign_db_mut("anchor_immutable", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");

            let mut input = base_input(TYPE_AKCIJSKA);
            input.starts_on = "2026-07-05T00:00:00Z".to_string();
            input.ends_on = Some("2026-07-20T00:00:00Z".to_string());
            input.items[0].campaign_price_minor = 900000;

            let created = create_campaign(conn, &input, 1, "2026-07-04T00:00:00Z").expect("create");
            activate_campaign(conn, created.id, 1, "2026-07-05T00:00:00Z").expect("activate");

            let anchor_at_activation: Option<i64> = conn
                .query_row(
                    "SELECT prethodna_cena_minor FROM campaign_items WHERE campaign_id = ?1",
                    params![created.id],
                    |row| row.get(0),
                )
                .expect("anchor should read");
            assert_eq!(anchor_at_activation, Some(1000000));

            // A cheaper price appears in the window AFTER activation. A lazy
            // recompute would drag the announced anchor down to it.
            conn.execute(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-06-25T00:00:00Z', 700000, 'update', '2026-06-25T00:00:00Z')",
                [],
            )
            .expect("late history row");

            // update_campaign is refused outright on an activated campaign.
            let error = update_campaign(conn, created.id, &input, "2026-07-06T00:00:00Z")
                .expect_err("activated campaign must not be editable");
            assert_eq!(error.code(), "invalid_state");

            // A markdown step and the end both leave the anchor untouched.
            adjust_item_price(conn, created.id, 1, 800000, 1, "2026-07-07T00:00:00Z")
                .expect("step");
            end_campaign(conn, created.id, &[], 1, "2026-07-20T00:00:00Z").expect("end");

            let anchor_after: Option<i64> = conn
                .query_row(
                    "SELECT prethodna_cena_minor FROM campaign_items WHERE campaign_id = ?1",
                    params![created.id],
                    |row| row.get(0),
                )
                .expect("anchor should read");
            assert_eq!(
                anchor_after, anchor_at_activation,
                "the announced prethodna cena must never move once activated (čl. 37 st. 5)"
            );
        });
    }

    #[test]
    fn worked_example_b_progressive_anchor_snapshots_and_freezes_at_11900() {
        with_campaign_db_mut("worked_example_b", |conn| {
            seed_product(conn, 1, 1290000, true, "2026-05-15T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at) VALUES
                 (1, '2026-06-21T00:00:00Z', 1190000, 'update', '2026-06-21T00:00:00Z'),
                 (1, '2026-07-01T00:00:00Z', 1290000, 'update', '2026-07-01T00:00:00Z');",
            )
            .expect("timeline");

            let mut input = base_input(TYPE_SEZONSKO);
            input.starts_on = "2026-07-05T00:00:00Z".to_string();
            input.ends_on = Some("2026-09-02T00:00:00Z".to_string()); // exactly 60 days
            input.items[0].campaign_price_minor = 990000; // step 1: 9.900

            let created = create_campaign(conn, &input, 1, "2026-07-04T00:00:00Z").expect("create");
            assert_eq!(
                created.items[0].prethodna_cena_minor,
                Some(1190000),
                "anchor is the MIN, not the first-reduction price"
            );

            let activated =
                activate_campaign(conn, created.id, 1, "2026-07-05T00:00:00Z").expect("activate");
            assert_eq!(activated.status, "active");

            // Till price now 9.900 with campaign_start provenance.
            let (till, source): (i64, String) = conn
                .query_row(
                    "SELECT p.sale_price_minor, ph.source FROM products p
                     JOIN price_history ph ON ph.product_id = p.id
                     WHERE p.id = 1 ORDER BY ph.id DESC LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("till state");
            assert_eq!((till, source.as_str()), (990000, "campaign_start"));

            // Steps 2 and 3: the anchor NEVER moves.
            let after_step2 =
                adjust_item_price(conn, created.id, 1, 890000, 1, "2026-07-20T00:00:00Z")
                    .expect("step 2");
            assert_eq!(
                after_step2.items[0].prethodna_cena_minor,
                Some(1190000),
                "frozen — lazy recompute would say 990000"
            );
            let after_step3 =
                adjust_item_price(conn, created.id, 1, 750000, 1, "2026-08-05T00:00:00Z")
                    .expect("step 3");
            assert_eq!(after_step3.items[0].prethodna_cena_minor, Some(1190000));

            // A step at-or-above the anchor is rejected.
            let error = adjust_item_price(conn, created.id, 1, 1190000, 1, "2026-08-06T00:00:00Z")
                .expect_err("must stay below the anchor");
            assert_eq!(error.code(), "validation_error");

            // End: default return = pre-campaign price (12.900).
            let ended =
                end_campaign(conn, created.id, &[], 1, "2026-09-02T00:00:00Z").expect("end");
            assert_eq!(ended.status, "ended");
            let (till, source): (i64, String) = conn
                .query_row(
                    "SELECT p.sale_price_minor, ph.source FROM products p
                     JOIN price_history ph ON ph.product_id = p.id
                     WHERE p.id = 1 ORDER BY ph.id DESC LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("till state");
            assert_eq!((till, source.as_str()), (1290000, "campaign_end"));
        });
    }

    // Memo worked example (c1): genuine first introduction -> promotivna.
    // No anchor, ≤60 days, declared future regular price, first offering IS the
    // activation, expiry transitions to the declared regular price.
    #[test]
    fn worked_example_c1_promotivna_first_offer_and_transition() {
        with_campaign_db_mut("worked_example_c1", |conn| {
            seed_product(conn, 2, 890000, false, "2026-07-01T00:00:00Z"); // inactive, never offered

            let mut input = base_input(TYPE_PROMOTIVNA);
            input.season_attested = false;
            input.starts_on = "2026-07-01T00:00:00Z".to_string();
            input.ends_on = Some("2026-08-29T00:00:00Z".to_string()); // 60 days
            input.items = vec![CampaignItemInput {
                product_id: 2,
                campaign_price_minor: 890000,
                manual_prethodna_minor: None,
                anchor_justification: None,
                future_regular_price_minor: Some(1090000),
            }];

            let created = create_campaign(conn, &input, 1, "2026-06-30T00:00:00Z").expect("create");
            assert_eq!(created.items[0].anchor_status, "none");
            assert_eq!(created.items[0].prethodna_cena_minor, None);

            activate_campaign(conn, created.id, 1, "2026-07-01T00:00:00Z").expect("activate");
            // First offering: product is now active and its FIRST history row is campaign_start.
            let (active, first_source): (i64, String) = conn
                .query_row(
                    "SELECT p.active, (SELECT source FROM price_history WHERE product_id = 2 ORDER BY id LIMIT 1)
                     FROM products p WHERE p.id = 2",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("state");
            assert_eq!((active, first_source.as_str()), (1, "campaign_start"));

            end_campaign(conn, created.id, &[], 1, "2026-08-29T00:00:00Z").expect("end");
            let till: i64 = conn
                .query_row(
                    "SELECT sale_price_minor FROM products WHERE id = 2",
                    [],
                    |row| row.get(0),
                )
                .expect("price");
            assert_eq!(
                till, 1090000,
                "expiry transitions to the declared regular price"
            );
        });
    }

    #[test]
    fn activation_re_validates_the_changed_world() {
        with_campaign_db_mut("activate_revalidates", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_SEZONSKO);
            input.items[0].campaign_price_minor = 900000;
            let created = create_campaign(conn, &input, 1, "2026-06-01T00:00:00Z").expect("create");

            // Two other seasonal campaigns get ACTIVATED after this draft was written.
            conn.execute_batch(
                "INSERT INTO campaigns (campaign_type, status, starts_on, ends_on, season_attested, activated_at, created_at, updated_at)
                 VALUES
                 ('sezonsko_snizenje','active','2026-07-01T00:00:00Z','2026-08-01T00:00:00Z',1,'2026-07-01T00:00:00Z','2026-06-20T00:00:00Z','2026-06-20T00:00:00Z'),
                 ('sezonsko_snizenje','ended','2026-01-02T00:00:00Z','2026-02-01T00:00:00Z',1,'2026-01-02T00:00:00Z','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
            )
            .expect("seed rivals");

            let error = activate_campaign(conn, created.id, 1, "2026-07-05T00:00:00Z")
                .expect_err("quota is now exhausted — activation must re-validate");
            assert_eq!(error.code(), "validation_error");
        });
    }

    #[test]
    fn activation_is_atomic_across_items() {
        with_campaign_db_mut("activate_atomic", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            seed_product(conn, 2, 800000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items = vec![
                CampaignItemInput {
                    product_id: 1,
                    campaign_price_minor: 900000,
                    manual_prethodna_minor: None,
                    anchor_justification: None,
                    future_regular_price_minor: None,
                },
                CampaignItemInput {
                    product_id: 2,
                    campaign_price_minor: 700000,
                    manual_prethodna_minor: None,
                    anchor_justification: None,
                    future_regular_price_minor: None,
                },
            ];
            let created = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z").expect("create");
            // Sabotage item 2 so activation's write-through fails mid-way.
            //
            // The plan sabotaged by DELETEing product 2, but `campaign_items`
            // has a FOREIGN KEY to `products`, so that delete is rejected
            // outright — and even if it landed it would fail in RE-VALIDATION
            // (h14f), before the write loop ever ran, proving nothing about
            // atomicity. This trigger instead aborts the UPDATE for item 2
            // AFTER item 1's price has already moved inside the transaction,
            // which is the actual mid-way failure the invariant is about.
            conn.execute_batch(
                "CREATE TRIGGER sabotage_item_two BEFORE UPDATE ON products
                 WHEN NEW.id = 2
                 BEGIN SELECT RAISE(ABORT, 'sabotage'); END;",
            )
            .expect("sabotage");

            let result = activate_campaign(conn, created.id, 1, "2026-07-05T00:00:00Z");
            // Specifically the sabotaged WRITE must be what failed. Asserting
            // only `is_err` would keep passing if this ever degraded into a
            // validation rejection, which would exercise no rollback at all.
            assert_eq!(
                result
                    .expect_err("sabotaged write must fail activation")
                    .code(),
                "database_error",
                "the failure must come from item 2's write, not from validation"
            );

            // NOTHING moved: product 1 keeps its price and no campaign_start rows exist.
            let price: i64 = conn
                .query_row(
                    "SELECT sale_price_minor FROM products WHERE id = 1",
                    [],
                    |row| row.get(0),
                )
                .expect("price");
            assert_eq!(price, 1000000);
            let starts: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM price_history WHERE source = 'campaign_start'",
                    [],
                    |row| row.get(0),
                )
                .expect("count");
            assert_eq!(starts, 0, "activation must be all-or-nothing");
        });
    }

    /// Identifies the prodajni objekat so the SW-12 publish path has an archive
    /// lineage to key on; without it nothing is published at all.
    fn seed_outlet(conn: &Connection) {
        conn.execute(
            "INSERT INTO settings (key, value_json, updated_at)
             VALUES ('company', ?1, '2026-01-01T00:00:00Z')",
            params![serde_json::json!({
                "shopName": "Butik Ana",
                "address": "Bulevar oslobođenja 1, Novi Sad",
                "pib": "",
                "registrationNumber": "",
                "phone": "",
                "logoPath": null,
                "currency": "RSD",
            })
            .to_string()],
        )
        .expect("company settings should seed");
    }

    fn published_bodies(conn: &Connection) -> Vec<String> {
        let mut statement = conn
            .prepare("SELECT body FROM cenovnik_snapshots ORDER BY id")
            .expect("snapshot query should prepare");
        let rows = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("snapshot query should run");
        rows.collect::<Result<Vec<_>, _>>()
            .expect("snapshot rows should read")
    }

    /// SW-12 req. 11 — ZZP čl. 6 st. 3 obliges the published cenovnik to match
    /// the outlet's current prices „u realnom vremenu“. A sniženje that lowers
    /// the shelf price while the published file still shows the old one is
    /// exactly the mismatch st. 3 targets; a campaign ENDING is the same defect
    /// in the other direction, and it additionally leaves the till guard
    /// comparing every sale against a stale lower published price.
    #[test]
    fn activating_and_ending_a_campaign_each_republish_the_cenovnik() {
        with_campaign_db_mut(
            "activating_and_ending_a_campaign_each_republish_the_cenovnik",
            |conn| {
                seed_product(conn, 1, 1_000_000, true, "2026-01-01T00:00:00Z");
                seed_outlet(conn);
                let mut input = base_input(TYPE_AKCIJSKA);
                input.items[0].campaign_price_minor = 900_000;
                let created =
                    create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z").expect("create");
                assert!(
                    published_bodies(conn).is_empty(),
                    "a draft moves no price and publishes nothing"
                );

                activate_campaign(conn, created.id, 1, "2026-07-05T00:00:00Z").expect("activate");
                let after_activation = published_bodies(conn);
                assert_eq!(after_activation.len(), 1, "{after_activation:?}");
                assert!(
                    after_activation[0].contains("9000.00"),
                    "the sniženje price must reach the published file: {:?}",
                    after_activation[0]
                );

                end_campaign(conn, created.id, &[], 1, "2026-07-20T00:00:00Z").expect("end");
                let after_end = published_bodies(conn);
                assert_eq!(after_end.len(), 2, "{after_end:?}");
                assert!(
                    after_end[1].contains("10000.00"),
                    "the price the campaign returned to must be republished: {:?}",
                    after_end[1]
                );
            },
        );
    }

    // ---- Warnings W1–W6 -------------------------------------------------
    //
    // Advisory only. A warning never blocks a save, and their ABSENCE is never
    // a finding that the promotion is lawful — čl. 38 st. 4 is an open standard
    // no query can settle.

    fn item(product_id: i64, campaign_price_minor: i64) -> CampaignItemInput {
        CampaignItemInput {
            product_id,
            campaign_price_minor,
            manual_prethodna_minor: None,
            anchor_justification: None,
            future_regular_price_minor: None,
        }
    }

    fn warnings_with(warnings: &[Violation], code: &str) -> Vec<Option<i64>> {
        warnings
            .iter()
            .filter(|warning| warning.code == code)
            .map(|warning| warning.product_id)
            .collect()
    }

    #[test]
    fn w1_flags_anchor_in_force_under_three_days() {
        with_campaign_db_mut("w1_token_period", |conn| {
            // 1.000 for months, then 800 for exactly 2 days inside the window, back to 1.000.
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at) VALUES
                 (1, '2026-06-20T00:00:00Z', 800000, 'update', '2026-06-20T00:00:00Z'),
                 (1, '2026-06-22T00:00:00Z', 1000000, 'update', '2026-06-22T00:00:00Z');",
            )
            .expect("timeline");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 700000;
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(report.hard.is_empty(), "{:?}", report.hard);
            // Anchor is 800000 (the MIN) but held only 2 days -> w1. Pinning it
            // is what makes the w1 assertion mean anything: against a 1.000.000
            // anchor the in-force sum would be ~28 days and w1 would be right
            // to stay silent. The report must hand back the snapshot its
            // findings were judged against.
            assert_eq!(
                report.anchors[0].prethodna_cena_minor,
                Some(800000),
                "the token-period warning must be about the MIN, not the regular price"
            );
            assert_eq!(warnings_with(&report.warnings, "w1"), vec![Some(1)]);
        });
    }

    /// The boundary is the memo's „1–2 days" reading: three full days inside the
    /// window is a period, not a token. Warning on it would train the user to
    /// dismiss the flag that čl. 38 st. 4 actually needs them to read.
    #[test]
    fn w1_silent_when_the_anchor_held_three_full_days() {
        with_campaign_db_mut("w1_real_period", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at) VALUES
                 (1, '2026-06-20T00:00:00Z', 800000, 'update', '2026-06-20T00:00:00Z'),
                 (1, '2026-06-23T00:00:00Z', 1000000, 'update', '2026-06-23T00:00:00Z');",
            )
            .expect("timeline");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 700000;
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(report.hard.is_empty(), "{:?}", report.hard);
            assert!(warnings_with(&report.warnings, "w1").is_empty());
        });
    }

    #[test]
    fn w2_flags_repeat_campaign_within_30_days() {
        with_campaign_db_mut("w2_repeat", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            // A campaign on the same product that started 10 days earlier.
            conn.execute_batch(
                "INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode, created_at, updated_at)
                 VALUES (99, 'akcijska_prodaja', 'ended', '2026-06-25T00:00:00Z', '2026-06-27T00:00:00Z', 'two_prices', '2026-06-25T00:00:00Z', '2026-06-25T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor, anchor_status)
                 VALUES (99, 1, 900000, 'computed');",
            )
            .expect("neighbouring campaign");

            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 700000;
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(report.hard.is_empty(), "{:?}", report.hard);
            assert_eq!(warnings_with(&report.warnings, "w2"), vec![Some(1)]);

            // Outside the 30-day radius -> silent.
            conn.execute(
                "UPDATE campaigns SET starts_on = '2026-05-20T00:00:00Z' WHERE id = 99",
                [],
            )
            .expect("move the neighbour back");
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(warnings_with(&report.warnings, "w2").is_empty());

            // A cancelled campaign was never announced, so it never depressed
            // anything — it must not raise the repeat flag.
            conn.execute(
                "UPDATE campaigns SET starts_on = '2026-06-25T00:00:00Z', status = 'cancelled' WHERE id = 99",
                [],
            )
            .expect("cancel the neighbour");
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(warnings_with(&report.warnings, "w2").is_empty());
        });
    }

    #[test]
    fn w3_flags_headline_percent_under_one_fifth_of_assortment() {
        with_campaign_db_mut("w3_assortment", |conn| {
            for id in 1..=10 {
                seed_product(conn, id, 1000000, true, "2026-01-01T00:00:00Z");
            }
            let mut input = base_input(TYPE_AKCIJSKA);
            input.headline_percent = Some(50);
            input.items = vec![item(1, 500000)];

            // 1 of 10 reaches the headline -> 1*5 < 10 -> under one fifth.
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(report.hard.is_empty(), "{:?}", report.hard);
            assert_eq!(
                warnings_with(&report.warnings, "w3"),
                vec![None],
                "čl. 38 st. 2 is about the assortment, not about one item"
            );

            // 2 of 10 is exactly one fifth, and st. 2 says „najmanje jednu petinu".
            input.items.push(item(2, 500000));
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(warnings_with(&report.warnings, "w3").is_empty());
        });
    }

    /// Only items that actually REACH the headline count toward the fifth: a
    /// 30% markdown is not evidence for a „do 50%" claim.
    #[test]
    fn w3_counts_only_items_reaching_the_headline_percent() {
        with_campaign_db_mut("w3_headline_reach", |conn| {
            for id in 1..=10 {
                seed_product(conn, id, 1000000, true, "2026-01-01T00:00:00Z");
            }
            let mut input = base_input(TYPE_AKCIJSKA);
            input.headline_percent = Some(50);
            // Two items, but one is only 30% off -> numerator 1, not 2.
            input.items = vec![item(1, 500000), item(2, 700000)];
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert_eq!(warnings_with(&report.warnings, "w3"), vec![None]);
        });
    }

    #[test]
    fn w4_flags_truncated_anchor() {
        with_campaign_db_mut("w4_truncated", |conn| {
            // Product 1 predates its own log (the v9 backfill case): the log
            // opens INSIDE the 30-day window, so the MIN covers only part of it.
            seed_product(conn, 1, 1000000, false, "2026-01-01T00:00:00Z");
            conn.execute_batch(
                "UPDATE products SET active = 1 WHERE id = 1;
                 INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-06-20T00:00:00Z', 1000000, 'seed', '2026-06-20T00:00:00Z');",
            )
            .expect("partial history");
            // Product 2's log covers the whole window.
            seed_product(conn, 2, 1000000, true, "2026-01-01T00:00:00Z");

            let mut input = base_input(TYPE_AKCIJSKA);
            input.items = vec![item(1, 700000), item(2, 700000)];
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(report.hard.is_empty(), "{:?}", report.hard);
            assert_eq!(
                warnings_with(&report.warnings, "w4"),
                vec![Some(1)],
                "only the item whose log falls short of the window"
            );
        });
    }

    #[test]
    fn w5_flags_till_price_below_anchor() {
        with_campaign_db_mut("w5_till_divergence", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            // A till line inside the anchor window at 700 while the OFFERED
            // price logged for that day was 1.000.
            conn.execute_batch(
                "INSERT INTO shifts (id, user_id, opened_at, opening_cash_minor, expected_cash_minor, status, created_at, updated_at)
                 VALUES (1, 1, '2026-06-15T08:00:00Z', 0, 0, 'open', '2026-06-15T08:00:00Z', '2026-06-15T08:00:00Z');
                 INSERT INTO sales (id, local_receipt_number, shift_id, cashier_id, status, fiscal_status, subtotal_minor, discount_minor, tax_minor, total_minor, created_at, updated_at)
                 VALUES (1, 'VP-000001', 1, 1, 'completed', 'not_fiscalized', 700000, 0, 0, 700000, '2026-06-15T09:00:00Z', '2026-06-15T09:00:00Z');
                 INSERT INTO sale_items (id, sale_id, product_id, product_name, product_sku, quantity_milli, unit_price_minor, discount_minor, tax_rate_basis_points, tax_minor, total_minor)
                 VALUES (1, 1, 1, 'P1', 'P1', 1000, 700000, 0, 2000, 0, 700000);",
            )
            .expect("till line");

            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 600000;
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(report.hard.is_empty(), "{:?}", report.hard);
            assert_eq!(warnings_with(&report.warnings, "w5"), vec![Some(1)]);
            // The finding is LINE-level: it cannot tell a per-line rebate from a
            // lower offered price, so the copy must hedge rather than assert.
            let message = &report
                .warnings
                .iter()
                .find(|warning| warning.code == "w5")
                .expect("w5")
                .message;
            assert!(
                message.contains("strože tumačenje"),
                "w5 must carry its own limitation: {message}"
            );

            // The same line at the offered price is no divergence.
            conn.execute(
                "UPDATE sale_items SET unit_price_minor = 1000000, total_minor = 1000000 WHERE id = 1",
                [],
            )
            .expect("realign the line");
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(warnings_with(&report.warnings, "w5").is_empty());
        });
    }

    /// Sales OUTSIDE the anchor window say nothing about the anchor: the window
    /// here must be the same one `compute_prethodna_cena` used.
    #[test]
    fn w5_ignores_sales_outside_the_anchor_window() {
        with_campaign_db_mut("w5_outside_window", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO shifts (id, user_id, opened_at, opening_cash_minor, expected_cash_minor, status, created_at, updated_at)
                 VALUES (1, 1, '2026-03-01T08:00:00Z', 0, 0, 'open', '2026-03-01T08:00:00Z', '2026-03-01T08:00:00Z');
                 INSERT INTO sales (id, local_receipt_number, shift_id, cashier_id, status, fiscal_status, subtotal_minor, discount_minor, tax_minor, total_minor, created_at, updated_at)
                 VALUES (1, 'VP-000001', 1, 1, 'completed', 'not_fiscalized', 700000, 0, 0, 700000, '2026-03-01T09:00:00Z', '2026-03-01T09:00:00Z');
                 INSERT INTO sale_items (id, sale_id, product_id, product_name, product_sku, quantity_milli, unit_price_minor, discount_minor, tax_rate_basis_points, tax_minor, total_minor)
                 VALUES (1, 1, 1, 'P1', 'P1', 1000, 700000, 0, 2000, 0, 700000);",
            )
            .expect("old till line");

            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 600000;
            let report =
                validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(warnings_with(&report.warnings, "w5").is_empty());
        });
    }

    #[test]
    fn w6_appears_on_overdue_campaign_view() {
        with_campaign_db_mut("w6_overdue", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 700000;
            let created = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z").expect("create");
            activate_campaign(conn, created.id, 1, "2026-07-05T00:00:00Z").expect("activate");

            // Declared end is 20.07 and it is inclusive — still lawfully running.
            let view = get_campaign(conn, created.id, "2026-07-20T12:00:00Z").expect("get");
            assert!(!view.overdue);
            assert!(warnings_with(&view.warnings, "w6").is_empty());

            let view = get_campaign(conn, created.id, "2026-07-21T00:00:00Z").expect("get");
            assert!(view.overdue);
            assert_eq!(warnings_with(&view.warnings, "w6"), vec![None]);
        });
    }

    /// The detail view must read the FROZEN anchor (čl. 37 st. 5), never take a
    /// fresh one. Activation moves the offered price to 700.000, so a warning
    /// pass that re-snapshotted would anchor on 700.000 and silently agree with
    /// the campaign's own price.
    #[test]
    fn campaign_view_warnings_use_the_frozen_anchor() {
        with_campaign_db_mut("w_view_frozen_anchor", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 700000;
            let created = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z").expect("create");
            activate_campaign(conn, created.id, 1, "2026-07-05T00:00:00Z").expect("activate");

            let view = get_campaign(conn, created.id, "2026-07-06T00:00:00Z").expect("get");
            assert_eq!(view.items[0].prethodna_cena_minor, Some(1000000));
            // Its OWN campaign_items row must not raise the repeat flag.
            assert!(
                warnings_with(&view.warnings, "w2").is_empty(),
                "a campaign is not its own repeat"
            );
        });
    }
}
