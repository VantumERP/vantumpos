//! KEP (evidencija prometa) value ledger — append-only auto-postings (SW-9a).
//!
//! Legal authority: `docs/KEP-VERIFIED-RULES.md`. Design:
//! `docs/superpowers/specs/2026-07-19-kep-ledger-design.md`.
//!
//! This module owns redni-broj allocation, the receipt zaduženje, and the
//! derived-saldo ledger view. Rows are INSERT-only (PEP čl. 14): nothing here
//! UPDATEs or DELETEs a `kep_entries` row (only the go-live reset wipes them).
//!
//! The load-bearing rule: a receipt zaduženje books **retail value with PDV**
//! (`quantity_milli * sale_price_minor / 1000`), never the nabavna/purchase
//! price. That is the memo's flagged false-assurance bug and the reason the
//! ledger reconciles to what an inspector expects.
//!
//! The public API is consumed incrementally by Tasks 3–5 (the receipt hook,
//! the daily razduženje, the admin commands), so `dead_code` is allowed here —
//! mirroring the other domain modules (`reklamacije`, `campaign_evidence`).

#![allow(dead_code)]

use crate::app_error::AppError;
use rusqlite::{params, Connection, Transaction};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

fn parse_rfc3339(value: &str, field: &str) -> Result<OffsetDateTime, AppError> {
    OffsetDateTime::parse(value, &Rfc3339).map_err(|source| {
        AppError::validation(
            format!("Datum nije ispravan: {source}"),
            serde_json::json!({ "field": field }),
        )
    })
}

/// The `dan.mesec` (kolona 2) rendering of a booking timestamp, e.g. `04.07`.
fn dan_mesec(rfc3339: &str) -> Result<String, AppError> {
    let dt = parse_rfc3339(rfc3339, "entryDate")?;
    Ok(format!("{:02}.{:02}", dt.day(), dt.month() as u8))
}

/// Calendar year of an RFC3339 timestamp — the ledger's `book_year` partition.
pub fn book_year_of(rfc3339: &str) -> Result<i64, AppError> {
    Ok(i64::from(parse_rfc3339(rfc3339, "now")?.year()))
}

/// The next monotonic, gap-free `redni_broj` for a `book_year` (`MAX+1`).
///
/// Allocated inside the posting transaction so concurrent posts can't collide
/// (the `UNIQUE (book_year, redni_broj)` constraint is the backstop). The
/// sequence restarts at 1 for each new book year.
pub fn next_redni_broj(conn: &Connection, book_year: i64) -> Result<i64, AppError> {
    let next: i64 = conn.query_row(
        "SELECT COALESCE(MAX(redni_broj), 0) + 1 FROM kep_entries WHERE book_year = ?1",
        params![book_year],
        |row| row.get(0),
    )?;
    Ok(next)
}

/// Books a goods-receipt zaduženje (kolona 4) at retail value with PDV.
///
/// `amount_minor = quantity_milli * sale_price_minor / 1000` — the product's
/// `sale_price_minor` (retail incl. PDV), **never** the purchase price. Written
/// inside the caller's transaction so a receipt can never exist un-booked.
#[allow(clippy::too_many_arguments)]
pub fn post_receipt_zaduzenje(
    tx: &Transaction<'_>,
    product_id: i64,
    quantity_milli: i64,
    sale_price_minor: i64,
    opis: &str,
    document_date: Option<&str>,
    reference_id: Option<i64>,
    acting_user_id: i64,
    now: &str,
) -> Result<(), AppError> {
    let _ = product_id; // not stored on the entry; kept for a self-describing call site
    let book_year = book_year_of(now)?;
    let redni_broj = next_redni_broj(tx, book_year)?;
    let amount_minor = quantity_milli * sale_price_minor / 1000;
    tx.execute(
        "INSERT INTO kep_entries (
            book_year, redni_broj, entry_date, document_date, opis,
            kolona, amount_minor, kind, entry_source,
            reference_type, reference_id, user_id, created_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5,
            'zaduzenje', ?6, 'receipt', 'auto',
            'inventory_movement', ?7, ?8, ?3
        )",
        params![
            book_year,
            redni_broj,
            now,
            document_date,
            opis,
            amount_minor,
            reference_id,
            acting_user_id,
        ],
    )?;
    Ok(())
}

/// One rendered ledger row: a `zaduzenje`/`razduzenje` pair with a `dan.mesec`
/// booking date. Exactly one of `zaduzenje_minor`/`razduzenje_minor` is `Some`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KepEntryView {
    pub redni_broj: i64,
    pub datum: String,
    pub opis: String,
    pub zaduzenje_minor: Option<i64>,
    pub razduzenje_minor: Option<i64>,
    pub kind: String,
}

/// A book year's ledger: its rows and the derived running saldo.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KepLedger {
    pub book_year: i64,
    pub entries: Vec<KepEntryView>,
    pub saldo_minor: i64,
}

/// The ledger for a book year: rows ordered by `redni_broj`, with the derived
/// `saldo_minor = Σzaduženje − Σrazduženje` (memo §2.4).
pub fn list_ledger(conn: &Connection, book_year: i64) -> Result<KepLedger, AppError> {
    let mut stmt = conn.prepare(
        "SELECT redni_broj, entry_date, opis, kolona, amount_minor, kind
         FROM kep_entries
         WHERE book_year = ?1
         ORDER BY redni_broj",
    )?;
    let rows = stmt.query_map(params![book_year], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;

    let mut entries = Vec::new();
    let mut saldo_minor: i64 = 0;
    for row in rows {
        let (redni_broj, entry_date, opis, kolona, amount_minor, kind) = row?;
        let datum = dan_mesec(&entry_date)?;
        let (zaduzenje_minor, razduzenje_minor) = if kolona == "zaduzenje" {
            saldo_minor += amount_minor;
            (Some(amount_minor), None)
        } else {
            saldo_minor -= amount_minor;
            (None, Some(amount_minor))
        };
        entries.push(KepEntryView {
            redni_broj,
            datum,
            opis,
            zaduzenje_minor,
            razduzenje_minor,
            kind,
        });
    }

    Ok(KepLedger {
        book_year,
        entries,
        saldo_minor,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};

    /// Runs `test` against a freshly migrated temp DB. `Db::new` seeds the admin
    /// user (id 1), which the `user_id` FK references.
    fn with_kep_db(test_name: &str, test: impl FnOnce(&mut Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("db init");
            let mut connection = db.open().expect("open");
            test(&mut connection);
        }
        std::fs::remove_file(&path).expect("cleanup");
    }

    /// Seeds a tax rate and a product with the given retail/purchase prices.
    fn seed_product(conn: &Connection, id: i64, sale_price_minor: i64, purchase_price_minor: i64) {
        conn.execute(
            "INSERT OR IGNORE INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
             VALUES (1, 'PDV 20', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .expect("tax rate should insert");
        conn.execute(
            "INSERT INTO products (
                id, name, sku, sale_price_minor, purchase_price_minor,
                tax_rate_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            params![
                id,
                format!("Proizvod {id}"),
                format!("SKU-{id}"),
                sale_price_minor,
                purchase_price_minor,
            ],
        )
        .expect("product should insert");
    }

    // Memo §2.7 worked example: 50 kom at retail 156,00 -> zaduženje 7.800,00
    // (NOT the 100,00 nabavna). Then a daily razduženje 2.340,00 (Task 4 posts
    // that; here just assert the receipt zaduženje value + saldo of a manual seed).
    #[test]
    fn receipt_posts_retail_with_pdv_not_nabavna() {
        with_kep_db("kep_receipt_retail", |conn| {
            seed_product(conn, 1, 15600, 10000); // sale_price 156,00 ; purchase 100,00
            let tx = conn.transaction().expect("tx");
            post_receipt_zaduzenje(
                &tx,
                1,
                50_000, /* 50 kom in milli */
                15600,
                "Kalkulacija br. 12",
                Some("2026-07-03T00:00:00Z"),
                Some(7),
                1,
                "2026-07-04T09:00:00Z",
            )
            .expect("post");
            tx.commit().expect("commit");
            let ledger = list_ledger(conn, 2026).expect("ledger");
            assert_eq!(ledger.entries.len(), 1);
            assert_eq!(
                ledger.entries[0].zaduzenje_minor,
                Some(780000),
                "50 x 156,00 retail incl PDV"
            );
            assert_eq!(ledger.entries[0].datum, "04.07");
            assert_eq!(ledger.saldo_minor, 780000);
        });
    }

    #[test]
    fn redni_broj_is_monotonic_per_book_year() {
        with_kep_db("kep_redni_broj", |conn| {
            seed_product(conn, 1, 10000, 5000);
            for (i, now) in ["2026-07-04T09:00:00Z", "2026-07-05T09:00:00Z"]
                .iter()
                .enumerate()
            {
                let tx = conn.transaction().expect("tx");
                post_receipt_zaduzenje(&tx, 1, 1000, 10000, "x", None, None, 1, now).expect("post");
                tx.commit().expect("commit");
                let _ = i;
            }
            // A 2027 posting restarts the sequence for its year.
            let tx = conn.transaction().expect("tx");
            post_receipt_zaduzenje(
                &tx,
                1,
                1000,
                10000,
                "x",
                None,
                None,
                1,
                "2027-01-02T09:00:00Z",
            )
            .expect("post");
            tx.commit().expect("commit");
            let rb: Vec<i64> = {
                let mut s = conn
                    .prepare(
                        "SELECT redni_broj FROM kep_entries WHERE book_year=2026 ORDER BY redni_broj",
                    )
                    .unwrap();
                s.query_map([], |r| r.get(0))
                    .unwrap()
                    .collect::<Result<_, _>>()
                    .unwrap()
            };
            assert_eq!(rb, vec![1, 2]);
            let rb2027: i64 = conn
                .query_row(
                    "SELECT redni_broj FROM kep_entries WHERE book_year=2027",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(rb2027, 1, "2027 restarts");
        });
    }
}
