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

use rusqlite::{params, Connection, OptionalExtension};

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
}
