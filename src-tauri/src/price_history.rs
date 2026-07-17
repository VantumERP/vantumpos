//! Append-only offered-price log (ZoT čl. 37 st. 3–4).
//!
//! Legal authority: `docs/ZOT-36-37-VERIFIED-RULES.md`. Design:
//! `docs/superpowers/specs/2026-07-17-price-history-design.md`.
//!
//! Rows are INSERT-only. Nothing in this crate may UPDATE or DELETE them —
//! the log is the shop's evidence that a displayed prethodna cena was correct,
//! and it is retained ~5 years. A NULL `price_minor` means the product stopped
//! being offered at `effective_from`; that explicit gap is what distinguishes
//! returning seasonal stock from a genuinely new arrival.

use crate::app_error::AppError;
use rusqlite::{params, Connection, OptionalExtension};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};

/// What the shop is offering for a product: an inactive product is not offered
/// at all, so its price is not an "offered price" in the sense of čl. 37 st. 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfferingState {
    pub active: bool,
    pub price_minor: i64,
}

pub fn load_offering_state(
    conn: &Connection,
    product_id: i64,
) -> rusqlite::Result<Option<OfferingState>> {
    conn.query_row(
        "SELECT active, sale_price_minor FROM products WHERE id = ?1",
        params![product_id],
        |row| {
            Ok(OfferingState {
                active: row.get::<_, i64>(0)? != 0,
                price_minor: row.get(1)?,
            })
        },
    )
    .optional()
}

/// Appends a row IFF the *offered* price actually changed. `before == None`
/// means the product is being created. Call inside the same transaction as the
/// product write: a price change recorded non-atomically can diverge from the
/// catalog, which is worse than not recording it.
pub fn record_offered_price_change(
    conn: &Connection,
    product_id: i64,
    before: Option<OfferingState>,
    after: OfferingState,
    source: &str,
    acting_user_id: Option<i64>,
    now_rfc3339: &str,
) -> rusqlite::Result<()> {
    let was_offered = before.map(|state| state.active).unwrap_or(false);

    // Some(x) => append a row carrying x; None => nothing to record.
    let price_to_log: Option<Option<i64>> = match (was_offered, after.active) {
        (false, true) => Some(Some(after.price_minor)), // offering started or resumed
        (true, false) => Some(None),                    // offering ended -> explicit gap
        (true, true) => {
            let before_price = before
                .expect("was_offered == true implies before is Some")
                .price_minor;
            if before_price == after.price_minor {
                None
            } else {
                Some(Some(after.price_minor))
            }
        }
        (false, false) => None, // never offered before or after
    };

    let Some(price_minor) = price_to_log else {
        return Ok(());
    };

    conn.execute(
        "INSERT INTO price_history (
            product_id, effective_from, price_minor, source, user_id, created_at
         )
         VALUES (?1, ?2, ?3, ?4, ?5, ?2)",
        params![product_id, now_rfc3339, price_minor, source, acting_user_id],
    )?;

    Ok(())
}

/// A prethodna cena computed under ZoT čl. 37 st. 3–4.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrethodnaCena {
    pub price_minor: i64,
    pub window_days: i64,
    /// Inclusive, RFC3339.
    pub window_from: String,
    /// Exclusive, RFC3339.
    pub window_to: String,
    /// True when the log does not cover the whole window — say so, never
    /// silently compute over a partial window.
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IncomputableReason {
    /// čl. 37 st. 4 names a window „ne kraćem od 15 dana" that cannot be
    /// observed for younger goods, and supplies no fallback. Neither block
    /// (no prohibitory language exists) nor guess.
    TooNewInAssortment {
        age_days: i64,
    },
    /// Off the shelf for the entire window.
    NotOfferedInWindow,
    NoHistory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrethodnaCenaResult {
    Computed(PrethodnaCena),
    Incomputable(IncomputableReason),
}

fn parse_rfc3339(value: &str, field: &str) -> Result<OffsetDateTime, AppError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|source| {
        AppError::validation(
            format!("Datum nije ispravan: {source}"),
            serde_json::json!({ "field": field }),
        )
    })
}

/// Computes the prethodna cena for a sniženje starting at `campaign_start_rfc3339`.
///
/// Legal authority: ZoT čl. 37 st. 3 (30-day lowest OFFERED price) and st. 4
/// (shorter assortment age, window floor of 15 days). st. 5 (the frozen
/// progressive anchor) is deliberately NOT implemented here — it is defined
/// against a campaign's start and belongs with the campaign entity (SW-6b).
pub fn compute_prethodna_cena(
    conn: &Connection,
    product_id: i64,
    campaign_start_rfc3339: &str,
) -> Result<PrethodnaCenaResult, AppError> {
    let start = parse_rfc3339(campaign_start_rfc3339, "campaignStart")?;

    // Assortment age comes from products.created_at — NOT from the price log.
    // Deriving it from the log would make every product freshly seeded by
    // migration v9 look brand new and report TooNewInAssortment for 15 days.
    let created_at: Option<String> = conn
        .query_row(
            "SELECT created_at FROM products WHERE id = ?1",
            params![product_id],
            |row| row.get(0),
        )
        .optional()?;
    let created_at = created_at.ok_or_else(|| AppError::not_found("Artikal nije pronađen."))?;
    let created = parse_rfc3339(&created_at, "createdAt")?;

    let age_days = (start - created).whole_days();

    // čl. 37 st. 3 / st. 4.
    let window_days = if age_days >= 30 {
        30
    } else if age_days >= 15 {
        age_days
    } else {
        return Ok(PrethodnaCenaResult::Incomputable(
            IncomputableReason::TooNewInAssortment { age_days },
        ));
    };

    let window_from = (start - Duration::days(window_days))
        .format(&Rfc3339)
        .map_err(|source| AppError::InvalidState(format!("Vreme nije dostupno: {source}")))?;
    let window_to = campaign_start_rfc3339.to_string();

    let earliest: Option<String> = conn.query_row(
        "SELECT MIN(effective_from) FROM price_history WHERE product_id = ?1",
        params![product_id],
        |row| row.get(0),
    )?;
    let Some(earliest) = earliest else {
        return Ok(PrethodnaCenaResult::Incomputable(
            IncomputableReason::NoHistory,
        ));
    };
    let truncated = earliest.as_str() > window_from.as_str();

    // Intervals derive from consecutive rows: row i covers
    // [effective_from_i, effective_from_{i+1}). A NULL price_minor is an
    // offering gap and is excluded. The MIN deliberately INCLUDES earlier
    // promotional prices — filtering them out to "find the regular price"
    // would produce an unlawfully high anchor.
    let min_price: Option<i64> = conn.query_row(
        "WITH timeline AS (
            SELECT price_minor,
                   effective_from AS valid_from,
                   LEAD(effective_from) OVER (
                       PARTITION BY product_id ORDER BY effective_from, id
                   ) AS valid_to
            FROM price_history
            WHERE product_id = ?1
         )
         SELECT MIN(price_minor)
         FROM timeline
         WHERE price_minor IS NOT NULL
           AND valid_from < ?3
           AND (valid_to IS NULL OR valid_to > ?2)",
        params![product_id, window_from, window_to],
        |row| row.get(0),
    )?;

    match min_price {
        Some(price_minor) => Ok(PrethodnaCenaResult::Computed(PrethodnaCena {
            price_minor,
            window_days,
            window_from,
            window_to,
            truncated,
        })),
        None => Ok(PrethodnaCenaResult::Incomputable(
            IncomputableReason::NotOfferedInWindow,
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn conn_with_product(active: bool, price_minor: i64) -> (Connection, i64) {
        let connection = Connection::open_in_memory().expect("in-memory db");
        connection
            .execute_batch(
                "CREATE TABLE products (id INTEGER PRIMARY KEY AUTOINCREMENT, sale_price_minor INTEGER NOT NULL, active INTEGER NOT NULL, created_at TEXT NOT NULL);
                 CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT);
                 CREATE TABLE price_history (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
                    effective_from TEXT NOT NULL,
                    price_minor INTEGER CHECK (price_minor IS NULL OR price_minor >= 0),
                    source TEXT NOT NULL CHECK (source IN ('create','update','import','deactivate','reactivate','seed')),
                    user_id INTEGER REFERENCES users(id),
                    created_at TEXT NOT NULL);",
            )
            .expect("schema should create");
        // Foreign keys are enforced (rusqlite's bundled SQLite defaults them on),
        // so the acting user the tests pass as `Some(1)` must actually exist.
        connection
            .execute("INSERT INTO users (id) VALUES (1)", [])
            .expect("acting user should insert");
        connection
            .execute(
                "INSERT INTO products (sale_price_minor, active, created_at) VALUES (?1, ?2, '2026-01-01T00:00:00Z')",
                rusqlite::params![price_minor, if active { 1 } else { 0 }],
            )
            .expect("product should insert");
        let id = connection.last_insert_rowid();
        (connection, id)
    }

    fn rows(conn: &Connection) -> Vec<(Option<i64>, String)> {
        let mut stmt = conn
            .prepare("SELECT price_minor, source FROM price_history ORDER BY id")
            .expect("prepare");
        let mapped = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .expect("query");
        mapped.collect::<Result<Vec<_>, _>>().expect("collect")
    }

    #[test]
    fn creating_an_active_product_records_the_offered_price() {
        let (conn, id) = conn_with_product(true, 499000);
        record_offered_price_change(
            &conn,
            id,
            None,
            OfferingState {
                active: true,
                price_minor: 499000,
            },
            "create",
            Some(1),
            "2026-07-17T00:00:00Z",
        )
        .expect("record");
        assert_eq!(rows(&conn), vec![(Some(499000), "create".to_string())]);
    }

    #[test]
    fn creating_an_inactive_product_records_nothing() {
        let (conn, id) = conn_with_product(false, 499000);
        record_offered_price_change(
            &conn,
            id,
            None,
            OfferingState {
                active: false,
                price_minor: 499000,
            },
            "create",
            Some(1),
            "2026-07-17T00:00:00Z",
        )
        .expect("record");
        assert!(
            rows(&conn).is_empty(),
            "an unoffered product has no offered price to log"
        );
    }

    #[test]
    fn deactivating_records_a_null_gap_row() {
        let (conn, id) = conn_with_product(true, 499000);
        record_offered_price_change(
            &conn,
            id,
            Some(OfferingState {
                active: true,
                price_minor: 499000,
            }),
            OfferingState {
                active: false,
                price_minor: 499000,
            },
            "deactivate",
            Some(1),
            "2026-07-17T00:00:00Z",
        )
        .expect("record");
        assert_eq!(rows(&conn), vec![(None, "deactivate".to_string())]);
    }

    #[test]
    fn reactivating_records_the_price_again() {
        let (conn, id) = conn_with_product(false, 529000);
        record_offered_price_change(
            &conn,
            id,
            Some(OfferingState {
                active: false,
                price_minor: 529000,
            }),
            OfferingState {
                active: true,
                price_minor: 529000,
            },
            "reactivate",
            Some(1),
            "2026-07-17T00:00:00Z",
        )
        .expect("record");
        assert_eq!(rows(&conn), vec![(Some(529000), "reactivate".to_string())]);
    }

    #[test]
    fn changing_the_price_while_active_records_it() {
        let (conn, id) = conn_with_product(true, 499000);
        record_offered_price_change(
            &conn,
            id,
            Some(OfferingState {
                active: true,
                price_minor: 499000,
            }),
            OfferingState {
                active: true,
                price_minor: 429000,
            },
            "update",
            Some(1),
            "2026-07-17T00:00:00Z",
        )
        .expect("record");
        assert_eq!(rows(&conn), vec![(Some(429000), "update".to_string())]);
    }

    #[test]
    fn an_unchanged_price_records_nothing() {
        let (conn, id) = conn_with_product(true, 499000);
        record_offered_price_change(
            &conn,
            id,
            Some(OfferingState {
                active: true,
                price_minor: 499000,
            }),
            OfferingState {
                active: true,
                price_minor: 499000,
            },
            "update",
            Some(1),
            "2026-07-17T00:00:00Z",
        )
        .expect("record");
        assert!(
            rows(&conn).is_empty(),
            "editing name/SKU must not pollute the price timeline"
        );
    }

    #[test]
    fn changing_the_price_while_inactive_records_nothing() {
        let (conn, id) = conn_with_product(false, 499000);
        record_offered_price_change(
            &conn,
            id,
            Some(OfferingState {
                active: false,
                price_minor: 499000,
            }),
            OfferingState {
                active: false,
                price_minor: 429000,
            },
            "update",
            Some(1),
            "2026-07-17T00:00:00Z",
        )
        .expect("record");
        assert!(
            rows(&conn).is_empty(),
            "a price edit on an unoffered product is not an offered price"
        );
    }

    #[test]
    fn load_offering_state_reads_active_and_price() {
        let (conn, id) = conn_with_product(true, 499000);
        assert_eq!(
            load_offering_state(&conn, id).expect("load"),
            Some(OfferingState {
                active: true,
                price_minor: 499000
            })
        );
        assert_eq!(load_offering_state(&conn, 999).expect("load"), None);
    }

    fn conn_with_timeline(created_at: &str, rows: &[(&str, Option<i64>)]) -> (Connection, i64) {
        let (connection, id) = conn_with_product(true, 0);
        connection
            .execute(
                "UPDATE products SET created_at = ?1 WHERE id = ?2",
                rusqlite::params![created_at, id],
            )
            .expect("created_at should set");
        for (effective_from, price_minor) in rows {
            connection
                .execute(
                    "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                     VALUES (?1, ?2, ?3, 'update', ?2)",
                    rusqlite::params![id, effective_from, price_minor],
                )
                .expect("history row should insert");
        }
        (connection, id)
    }

    // Worked example (a) — ZOT-36-37-VERIFIED-RULES.md §2.7.
    // KOSULJA-M-42, campaign starts 20.07.2026. Window [20.06, 20.07).
    // The June akcija at 4.290,00 falls OUTSIDE the window; the 01.07 price
    // INCREASE to 5.290,00 does not become the anchor (MIN, not "price before").
    #[test]
    fn worked_example_a_simple_reduction_anchors_at_4990() {
        let (conn, id) = conn_with_timeline(
            "2026-04-01T00:00:00Z",
            &[
                ("2026-04-01T00:00:00Z", Some(499000)),
                ("2026-06-10T00:00:00Z", Some(429000)),
                ("2026-06-15T00:00:00Z", Some(499000)),
                ("2026-07-01T00:00:00Z", Some(529000)),
            ],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-20T00:00:00Z").expect("compute");
        match result {
            PrethodnaCenaResult::Computed(value) => {
                assert_eq!(value.price_minor, 499000, "prethodna cena must be 4.990,00");
                assert_eq!(value.window_days, 30);
                assert!(!value.truncated);
            }
            other => panic!("expected Computed, got {other:?}"),
        }
    }

    // Worked example (a') — the ratchet-down effect. Starting 8 days earlier
    // pulls the June akcija INTO the window and collapses the anchor.
    #[test]
    fn worked_example_a_prime_earlier_start_ratchets_anchor_down_to_4290() {
        let (conn, id) = conn_with_timeline(
            "2026-04-01T00:00:00Z",
            &[
                ("2026-04-01T00:00:00Z", Some(499000)),
                ("2026-06-10T00:00:00Z", Some(429000)),
                ("2026-06-15T00:00:00Z", Some(499000)),
                ("2026-07-01T00:00:00Z", Some(529000)),
            ],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-12T00:00:00Z").expect("compute");
        match result {
            PrethodnaCenaResult::Computed(value) => assert_eq!(value.price_minor, 429000),
            other => panic!("expected Computed, got {other:?}"),
        }
    }

    // Worked example (c2) — 22 days in assortment -> st. 4 window = 22 days.
    #[test]
    fn worked_example_c2_short_assortment_uses_actual_age_window() {
        let (conn, id) = conn_with_timeline(
            "2026-06-25T00:00:00Z",
            &[
                ("2026-06-25T00:00:00Z", Some(249000)),
                ("2026-07-08T00:00:00Z", Some(229000)),
            ],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-17T00:00:00Z").expect("compute");
        match result {
            PrethodnaCenaResult::Computed(value) => {
                assert_eq!(value.price_minor, 229000, "prethodna cena must be 2.290,00");
                assert_eq!(
                    value.window_days, 22,
                    "window is the item's actual age, not 30"
                );
            }
            other => panic!("expected Computed, got {other:?}"),
        }
    }

    // Worked example (c3) — 6 days in assortment. The law supplies no usable
    // window: neither block nor compute.
    #[test]
    fn worked_example_c3_too_new_is_incomputable_not_blocked_and_not_guessed() {
        let (conn, id) = conn_with_timeline(
            "2026-07-11T00:00:00Z",
            &[("2026-07-11T00:00:00Z", Some(100000))],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-17T00:00:00Z").expect("compute");
        assert_eq!(
            result,
            PrethodnaCenaResult::Incomputable(IncomputableReason::TooNewInAssortment {
                age_days: 6
            })
        );
    }

    // Worked example (c4) — THE TRAP. A jacket returning to the shelf after a
    // gap is NOT a new arrival: st. 3 with the full 30-day window applies,
    // reaching across the gap. A naive "days since last offered" implementation
    // would use ~10 days here and anchor too high.
    #[test]
    fn worked_example_c4_returning_seasonal_goods_use_st3_not_st4() {
        let (conn, id) = conn_with_timeline(
            "2025-10-01T00:00:00Z",
            &[
                ("2025-10-01T00:00:00Z", Some(1290000)),
                ("2026-02-28T00:00:00Z", None), // offering ended
                ("2026-07-05T00:00:00Z", Some(1290000)), // back on the shelf
            ],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-15T00:00:00Z").expect("compute");
        match result {
            PrethodnaCenaResult::Computed(value) => {
                assert_eq!(
                    value.window_days, 30,
                    "returning stock is not a new arrival"
                );
                assert_eq!(value.price_minor, 1290000);
            }
            other => panic!("expected Computed, got {other:?}"),
        }
    }

    #[test]
    fn a_product_off_shelf_for_the_whole_window_is_not_offered_in_window() {
        let (conn, id) = conn_with_timeline(
            "2025-10-01T00:00:00Z",
            &[
                ("2025-10-01T00:00:00Z", Some(1290000)),
                ("2026-02-28T00:00:00Z", None),
            ],
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-15T00:00:00Z").expect("compute");
        assert_eq!(
            result,
            PrethodnaCenaResult::Incomputable(IncomputableReason::NotOfferedInWindow)
        );
    }

    #[test]
    fn a_window_reaching_before_the_log_is_reported_truncated() {
        let (conn, id) = conn_with_timeline(
            "2026-01-01T00:00:00Z",
            &[("2026-07-10T00:00:00Z", Some(499000))], // log starts mid-window (a v9 seed)
        );
        let result = compute_prethodna_cena(&conn, id, "2026-07-20T00:00:00Z").expect("compute");
        match result {
            PrethodnaCenaResult::Computed(value) => {
                assert!(
                    value.truncated,
                    "the log does not cover the full 30-day window"
                );
                assert_eq!(value.price_minor, 499000);
            }
            other => panic!("expected Computed, got {other:?}"),
        }
    }

    #[test]
    fn a_product_with_no_history_reports_no_history() {
        let (conn, id) = conn_with_timeline("2026-01-01T00:00:00Z", &[]);
        let result = compute_prethodna_cena(&conn, id, "2026-07-20T00:00:00Z").expect("compute");
        assert_eq!(
            result,
            PrethodnaCenaResult::Incomputable(IncomputableReason::NoHistory)
        );
    }
}
