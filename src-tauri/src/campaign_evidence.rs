//! Campaign evidence assembly (ZoT čl. 36–37, čl. 48).
//!
//! Legal authority: `docs/ZOT-36-37-VERIFIED-RULES.md`. Design:
//! `docs/superpowers/specs/2026-07-17-campaign-evidence-design.md`.
//!
//! This module is read-only over the 6a/6b data (`campaigns`, `campaign_items`,
//! `products`, `price_history`). It assembles a campaign's evidentiary record —
//! the frozen anchors and the actual offered-price rows that support each
//! prethodna cena — so the record can be rendered to self-contained HTML.
//! It never touches `users`, `sales`, `sale_items`, `shifts`, or other
//! campaigns: the walk-away inspector document must carry only this campaign's
//! own articles, anchors, and supporting rows.

// The assembled record and its structs are consumed by the HTML renderers and
// export commands added in later SW-6c tasks; keep the staged API green here.
#![allow(dead_code)]

use crate::app_error::AppError;
use rusqlite::{params, Connection, OptionalExtension};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

/// One offered-price interval that overlaps the anchor window.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportingRow {
    pub effective_from: String,
    pub price_minor: i64,
}

/// A campaign line with its frozen anchor and (for computed anchors) the
/// supporting offered-price rows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceItem {
    pub product_id: i64,
    pub product_name: String,
    pub sku: String,
    pub campaign_price_minor: i64,
    pub anchor_status: String,
    pub prethodna_cena_minor: Option<i64>,
    pub anchor_window_days: Option<i64>,
    pub anchor_truncated: bool,
    pub anchor_reason: Option<String>,
    pub anchor_justification: Option<String>,
    pub future_regular_price_minor: Option<i64>,
    pub window_from: Option<String>,
    pub window_to: Option<String>,
    pub supporting_rows: Vec<SupportingRow>,
}

/// A campaign and all of its evidence lines.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignEvidence {
    pub campaign_id: i64,
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
    pub items: Vec<EvidenceItem>,
}

fn parse_rfc3339(value: &str, field: &str) -> Result<OffsetDateTime, AppError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|source| {
        AppError::validation(
            format!("Datum nije ispravan: {source}"),
            serde_json::json!({ "field": field }),
        )
    })
}

/// Offered-price rows overlapping `[window_from, window_to)`. Same overlap
/// predicate `compute_prethodna_cena` uses (`price_history.rs`), minus the MIN:
/// the evidence document must show every supporting row, not just the winner.
fn supporting_rows_in_window(
    conn: &Connection,
    product_id: i64,
    window_from: &str,
    window_to: &str,
) -> Result<Vec<SupportingRow>, AppError> {
    let mut stmt = conn.prepare(
        "WITH timeline AS (
            SELECT price_minor,
                   effective_from AS valid_from,
                   LEAD(effective_from) OVER (
                       PARTITION BY product_id ORDER BY effective_from, id
                   ) AS valid_to
            FROM price_history
            WHERE product_id = ?1
         )
         SELECT valid_from, price_minor
         FROM timeline
         WHERE price_minor IS NOT NULL
           AND valid_from < ?3
           AND (valid_to IS NULL OR valid_to > ?2)
         ORDER BY valid_from",
    )?;
    let rows = stmt
        .query_map(params![product_id, window_from, window_to], |row| {
            Ok(SupportingRow {
                effective_from: row.get(0)?,
                price_minor: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows)
}

/// Assembles the evidentiary record for one campaign. Pure SELECT — no writes.
pub fn assemble_evidence(
    conn: &Connection,
    campaign_id: i64,
) -> Result<CampaignEvidence, AppError> {
    let mut evidence = conn
        .query_row(
            "SELECT campaign_type, status, starts_on, ends_on, display_mode,
                    headline_percent, rasprodaja_ground, special_conditions,
                    reduced_utility_reason, marketing_label, season_attested,
                    separation_attested, activated_at, ended_at
             FROM campaigns
             WHERE id = ?1",
            params![campaign_id],
            |row| {
                Ok(CampaignEvidence {
                    campaign_id,
                    campaign_type: row.get(0)?,
                    status: row.get(1)?,
                    starts_on: row.get(2)?,
                    ends_on: row.get(3)?,
                    display_mode: row.get(4)?,
                    headline_percent: row.get(5)?,
                    rasprodaja_ground: row.get(6)?,
                    special_conditions: row.get(7)?,
                    reduced_utility_reason: row.get(8)?,
                    marketing_label: row.get(9)?,
                    season_attested: row.get::<_, i64>(10)? != 0,
                    separation_attested: row.get::<_, i64>(11)? != 0,
                    activated_at: row.get(12)?,
                    ended_at: row.get(13)?,
                    items: Vec::new(),
                })
            },
        )
        .optional()?
        .ok_or_else(|| AppError::not_found("Kampanja nije pronađena."))?;

    let starts_on = evidence.starts_on.clone();

    let mut stmt = conn.prepare(
        "SELECT ci.product_id, p.name, p.sku, ci.campaign_price_minor,
                ci.anchor_status, ci.prethodna_cena_minor, ci.anchor_window_days,
                ci.anchor_truncated, ci.anchor_reason, ci.anchor_justification,
                ci.future_regular_price_minor
         FROM campaign_items ci
         JOIN products p ON p.id = ci.product_id
         WHERE ci.campaign_id = ?1
         ORDER BY ci.id",
    )?;

    struct ItemRow {
        product_id: i64,
        product_name: String,
        sku: String,
        campaign_price_minor: i64,
        anchor_status: String,
        prethodna_cena_minor: Option<i64>,
        anchor_window_days: Option<i64>,
        anchor_truncated: bool,
        anchor_reason: Option<String>,
        anchor_justification: Option<String>,
        future_regular_price_minor: Option<i64>,
    }

    let raw_items = stmt
        .query_map(params![campaign_id], |row| {
            Ok(ItemRow {
                product_id: row.get(0)?,
                product_name: row.get(1)?,
                sku: row.get(2)?,
                campaign_price_minor: row.get(3)?,
                anchor_status: row.get(4)?,
                prethodna_cena_minor: row.get(5)?,
                anchor_window_days: row.get(6)?,
                anchor_truncated: row.get::<_, i64>(7)? != 0,
                anchor_reason: row.get(8)?,
                anchor_justification: row.get(9)?,
                future_regular_price_minor: row.get(10)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut items = Vec::with_capacity(raw_items.len());
    for raw in raw_items {
        let (window_from, window_to, supporting_rows) = match raw.anchor_status.as_str() {
            "computed" => {
                let window_days = raw.anchor_window_days.ok_or_else(|| {
                    AppError::InvalidState(
                        "Sidro je izračunato ali nedostaje dužina perioda.".to_string(),
                    )
                })?;
                let start = parse_rfc3339(&starts_on, "startsOn")?;
                let window_from = (start - Duration::days(window_days))
                    .format(&Rfc3339)
                    .map_err(|source| {
                        AppError::InvalidState(format!("Vreme nije dostupno: {source}"))
                    })?;
                let window_to = starts_on.clone();
                let rows =
                    supporting_rows_in_window(conn, raw.product_id, &window_from, &window_to)?;
                (Some(window_from), Some(window_to), rows)
            }
            _ => (None, None, Vec::new()),
        };

        items.push(EvidenceItem {
            product_id: raw.product_id,
            product_name: raw.product_name,
            sku: raw.sku,
            campaign_price_minor: raw.campaign_price_minor,
            anchor_status: raw.anchor_status,
            prethodna_cena_minor: raw.prethodna_cena_minor,
            anchor_window_days: raw.anchor_window_days,
            anchor_truncated: raw.anchor_truncated,
            anchor_reason: raw.anchor_reason,
            anchor_justification: raw.anchor_justification,
            future_regular_price_minor: raw.future_regular_price_minor,
            window_from,
            window_to,
            supporting_rows,
        });
    }

    evidence.items = items;
    Ok(evidence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};
    use rusqlite::{params, Connection};

    fn with_db(test_name: &str, test: impl FnOnce(&Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("db init");
            let conn = db.open().expect("open");
            conn.execute_batch(
                "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                 VALUES (1, 'PDV 20', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');",
            )
            .expect("seed tax");
            test(&conn);
        }
        std::fs::remove_file(&path).expect("cleanup");
    }

    fn seed_product(conn: &Connection, id: i64, name: &str, price: i64, created_at: &str) {
        conn.execute(
            "INSERT INTO products (id, name, sku, sale_price_minor, purchase_price_minor,
                                   tax_rate_id, minimum_stock_milli, active, created_at, updated_at)
             VALUES (?1, ?2, ?2, ?3, 0, 1, 0, 1, ?4, ?4)",
            params![id, name, price, created_at],
        )
        .expect("seed product");
    }

    // Worked example (b): jacket established in assortment -> st. 3, 30-day window,
    // anchor 11.900 taken from offered rows inside [2026-06-05, 2026-07-05).
    #[test]
    fn assembles_computed_anchor_with_supporting_rows_in_window() {
        with_db("evidence_computed", |conn| {
            seed_product(conn, 1, "Jakna", 1290000, "2026-05-15T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at) VALUES
                 (1, '2026-05-15T00:00:00Z', 1290000, 'create', '2026-05-15T00:00:00Z'),
                 (1, '2026-06-21T00:00:00Z', 1190000, 'update', '2026-06-21T00:00:00Z'),
                 (1, '2026-07-01T00:00:00Z', 1290000, 'update', '2026-07-01T00:00:00Z');
                 INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode,
                                        season_attested, activated_at, created_at, updated_at)
                 VALUES (1, 'sezonsko_snizenje', 'active', '2026-07-05T00:00:00Z', '2026-09-02T00:00:00Z',
                         'two_prices', 1, '2026-07-05T00:00:00Z', '2026-07-04T00:00:00Z', '2026-07-05T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor,
                                             prethodna_cena_minor, anchor_status, anchor_window_days)
                 VALUES (1, 1, 990000, 1190000, 'computed', 30);",
            )
            .expect("seed campaign");

            let evidence = assemble_evidence(conn, 1).expect("assemble");
            assert_eq!(evidence.campaign_type, "sezonsko_snizenje");
            let item = &evidence.items[0];
            assert_eq!(item.prethodna_cena_minor, Some(1190000));
            assert_eq!(item.anchor_window_days, Some(30));
            assert_eq!(item.window_from.as_deref(), Some("2026-06-05T00:00:00Z"));
            assert_eq!(item.window_to.as_deref(), Some("2026-07-05T00:00:00Z"));
            // The 11.900 (from 21.06) and 12.900 (from 01.07 and the pre-window
            // 15.05 interval that overlaps) are in; nothing outside the window leaks.
            let prices: Vec<i64> = item
                .supporting_rows
                .iter()
                .map(|row| row.price_minor)
                .collect();
            assert!(
                prices.contains(&1190000),
                "the MIN's source row must be present: {prices:?}"
            );
            assert!(item
                .supporting_rows
                .iter()
                .all(|row| row.effective_from.as_str() < "2026-07-05T00:00:00Z"));
        });
    }

    #[test]
    fn manual_anchor_carries_justification_and_no_rows() {
        with_db("evidence_manual", |conn| {
            seed_product(conn, 1, "Jogurt", 50000, "2026-07-01T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode, created_at, updated_at)
                 VALUES (1, 'akcijska_prodaja', 'draft', '2026-07-10T00:00:00Z', '2026-07-20T00:00:00Z', 'two_prices', '2026-07-09T00:00:00Z', '2026-07-09T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor,
                                             prethodna_cena_minor, anchor_status, anchor_reason, anchor_justification)
                 VALUES (1, 1, 45000, 48000, 'manual', 'perishable', 'Cena sa police');",
            )
            .expect("seed");
            let evidence = assemble_evidence(conn, 1).expect("assemble");
            let item = &evidence.items[0];
            assert_eq!(item.anchor_status, "manual");
            assert_eq!(item.prethodna_cena_minor, Some(48000));
            assert_eq!(item.anchor_justification.as_deref(), Some("Cena sa police"));
            assert!(item.window_from.is_none());
            assert!(item.supporting_rows.is_empty());
        });
    }

    // Worked example (c1): promotivna -> no anchor, future price present.
    #[test]
    fn promotivna_has_no_anchor_and_carries_future_price() {
        with_db("evidence_promotivna", |conn| {
            seed_product(conn, 2, "Patike", 890000, "2026-07-01T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode, created_at, updated_at)
                 VALUES (1, 'promotivna_prodaja', 'draft', '2026-07-01T00:00:00Z', '2026-08-29T00:00:00Z', 'two_prices', '2026-06-30T00:00:00Z', '2026-06-30T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor,
                                             anchor_status, future_regular_price_minor)
                 VALUES (1, 2, 890000, 'none', 1090000);",
            )
            .expect("seed");
            let evidence = assemble_evidence(conn, 1).expect("assemble");
            let item = &evidence.items[0];
            assert_eq!(item.anchor_status, "none");
            assert!(item.prethodna_cena_minor.is_none());
            assert_eq!(item.future_regular_price_minor, Some(1090000));
            assert!(item.supporting_rows.is_empty());
        });
    }

    #[test]
    fn missing_campaign_is_not_found() {
        with_db("evidence_missing", |conn| {
            let error = assemble_evidence(conn, 999).expect_err("missing");
            assert_eq!(error.code(), "not_found");
        });
    }
}
