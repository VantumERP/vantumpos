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
///
/// `reference_type` names the linking isprava: SW-9b routes it through the
/// kalkulacija (`"kalkulacija"`, `reference_id = kalkulacija.id`); callers with
/// no kalkulacija pass the raw movement type.
#[allow(clippy::too_many_arguments)]
pub fn post_receipt_zaduzenje(
    tx: &Transaction<'_>,
    product_id: i64,
    quantity_milli: i64,
    sale_price_minor: i64,
    opis: &str,
    document_date: Option<&str>,
    reference_type: &str,
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
            ?7, ?8, ?9, ?3
        )",
        params![
            book_year,
            redni_broj,
            now,
            document_date,
            opis,
            amount_minor,
            reference_type,
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

/// Books a trading day's razduženje (kolona 5) from the day's sales total.
///
/// The amount is `override_amount_minor` when given (recorded as a `manual`
/// posting — the certified ESIR figure legally governs), otherwise the sum of
/// completed `sale` documents on `date`. Idempotent on the **sales day**
/// (`document_date`), not the booking time (`entry_date`): a second post for
/// the same day is rejected while later days still book. `entry_date = now`.
pub fn post_daily_sales(
    conn: &mut Connection,
    date: &str,
    override_amount_minor: Option<i64>,
    acting_user_id: i64,
    now: &str,
) -> Result<KepEntryView, AppError> {
    let already_posted: bool = conn.query_row(
        "SELECT EXISTS(
            SELECT 1 FROM kep_entries WHERE kind = 'daily_sales' AND document_date = ?1
         )",
        params![date],
        |row| row.get(0),
    )?;
    if already_posted {
        return Err(AppError::business(
            "already_posted",
            "Dnevni promet za taj dan je već proknjižen.",
        ));
    }

    let (amount_minor, entry_source) = match override_amount_minor {
        Some(amount) => (amount, "manual"),
        None => {
            let total: i64 = conn.query_row(
                "SELECT COALESCE(SUM(total_minor), 0) FROM sales
                 WHERE document_type = 'sale' AND status = 'completed'
                   AND date(created_at) = date(?1)",
                params![date],
                |row| row.get(0),
            )?;
            (total, "auto")
        }
    };

    let book_year = book_year_of(now)?;
    let opis = format!("Dnevni promet {date}");
    let tx = conn.transaction()?;
    let redni_broj = next_redni_broj(&tx, book_year)?;
    tx.execute(
        "INSERT INTO kep_entries (
            book_year, redni_broj, entry_date, document_date, opis,
            kolona, amount_minor, kind, entry_source,
            reference_type, reference_id, user_id, created_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5,
            'razduzenje', ?6, 'daily_sales', ?7,
            'sales_day', NULL, ?8, ?3
        )",
        params![
            book_year,
            redni_broj,
            now,
            date,
            opis,
            amount_minor,
            entry_source,
            acting_user_id,
        ],
    )?;
    tx.commit()?;

    Ok(KepEntryView {
        redni_broj,
        datum: dan_mesec(now)?,
        opis,
        zaduzenje_minor: None,
        razduzenje_minor: Some(amount_minor),
        kind: "daily_sales".to_string(),
    })
}

/// The ledger's posting-health status surfaced in the UI.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KepStatus {
    /// Completed sales days with no `daily_sales` entry whose T+1 deadline has
    /// elapsed (`today > day + 1`), oldest first.
    pub overdue_sales_days: Vec<String>,
    /// Received lots whose receipt zaduženje is missing — defensive; the atomic
    /// hook (Task 3) guarantees zero.
    pub unbooked_receipt_count: i64,
}

/// The T+1 overdue-posting warning (memo §2.5): completed sales days with no
/// `daily_sales` entry whose deadline has passed (`date(created_at) <
/// date(today, '-1 day')`, i.e. `today > day + 1`). `unbooked_receipt_count` is
/// a defensive zero — the receipt hook shares the inventory transaction.
pub fn kep_status(conn: &Connection, today: &str) -> Result<KepStatus, AppError> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT date(created_at) FROM sales
         WHERE document_type = 'sale' AND status = 'completed'
           AND date(created_at) < date(?1, '-1 day')
           AND date(created_at) NOT IN (
               SELECT date(document_date) FROM kep_entries WHERE kind = 'daily_sales'
           )
         ORDER BY date(created_at)",
    )?;
    let overdue_sales_days: Vec<String> = stmt
        .query_map(params![today], |row| row.get(0))?
        .collect::<Result<_, _>>()?;

    Ok(KepStatus {
        overdue_sales_days,
        unbooked_receipt_count: 0,
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

    /// Seeds the open shift that completed sales reference (FK backstop).
    fn seed_shift(conn: &Connection) {
        conn.execute(
            "INSERT OR IGNORE INTO shifts (
                id, user_id, opened_at, opening_cash_minor, expected_cash_minor,
                status, created_at, updated_at
             ) VALUES (1, 1, '2026-07-01T08:00:00Z', 0, 0, 'open', '2026-07-01T08:00:00Z', '2026-07-01T08:00:00Z')",
            [],
        )
        .expect("shift should insert");
    }

    /// Seeds one completed `sale` document on `created_at` for `total_minor`.
    fn seed_completed_sale(conn: &Connection, id: i64, created_at: &str, total_minor: i64) {
        conn.execute(
            "INSERT INTO sales (
                id, local_receipt_number, shift_id, cashier_id, status, fiscal_status,
                subtotal_minor, discount_minor, tax_minor, total_minor, created_at, updated_at
             ) VALUES (?1, ?2, 1, 1, 'completed', 'not_fiscalized', ?3, 0, 0, ?3, ?4, ?4)",
            params![id, format!("VP-{id:06}"), total_minor, created_at],
        )
        .expect("sale should insert");
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
                "kalkulacija",
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
                post_receipt_zaduzenje(&tx, 1, 1000, 10000, "x", None, "kalkulacija", None, 1, now)
                    .expect("post");
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
                "kalkulacija",
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

    // A trading day's razduženje (kolona 5) posts the day's sales total. The
    // day is keyed on `document_date` (the sales day), so a second post for the
    // same day is rejected while later days still book.
    #[test]
    fn daily_sales_posts_razduzenje_from_sales_total() {
        with_kep_db("kep_daily_sales", |conn| {
            seed_shift(conn);
            seed_completed_sale(conn, 1, "2026-07-05T10:00:00Z", 150000);
            seed_completed_sale(conn, 2, "2026-07-05T14:00:00Z", 84000);

            let entry = post_daily_sales(conn, "2026-07-05", None, 1, "2026-07-06T09:00:00Z")
                .expect("post");
            assert_eq!(entry.razduzenje_minor, Some(234000), "150000 + 84000");
            assert_eq!(entry.zaduzenje_minor, None);
            assert_eq!(entry.kind, "daily_sales");

            let source: String = conn
                .query_row(
                    "SELECT entry_source FROM kep_entries WHERE kind='daily_sales'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(source, "auto", "computed total is an auto posting");

            // A second post for the same sales day is rejected (idempotent on
            // the sales day, not the booking time).
            let err = post_daily_sales(conn, "2026-07-05", None, 1, "2026-07-06T09:05:00Z")
                .expect_err("second post must reject");
            assert_eq!(err.code(), "already_posted");

            // The next day still books (distinct document_date).
            post_daily_sales(conn, "2026-07-06", None, 1, "2026-07-07T09:00:00Z")
                .expect("next day still books");
        });
    }

    #[test]
    fn daily_sales_override_uses_amount_and_marks_manual() {
        with_kep_db("kep_daily_override", |conn| {
            seed_shift(conn);
            seed_completed_sale(conn, 1, "2026-07-05T10:00:00Z", 150000);

            // The certified ESIR figure governs; the override records it was set
            // manually and uses the given amount, not the 150000 sales sum.
            let entry =
                post_daily_sales(conn, "2026-07-05", Some(250000), 1, "2026-07-06T09:00:00Z")
                    .expect("post");
            assert_eq!(entry.razduzenje_minor, Some(250000));

            let source: String = conn
                .query_row(
                    "SELECT entry_source FROM kep_entries WHERE kind='daily_sales'",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(source, "manual");
        });
    }

    // T+1 overdue-posting warning (memo §2.5): a completed sales day with no
    // daily_sales entry surfaces once `today > day + 1`, and drops once posted.
    #[test]
    fn status_flags_overdue_day_and_clears_after_posting() {
        with_kep_db("kep_status_overdue", |conn| {
            seed_shift(conn);
            seed_completed_sale(conn, 1, "2026-07-05T10:00:00Z", 234000);

            let status = kep_status(conn, "2026-07-08T09:00:00Z").expect("status");
            assert_eq!(status.overdue_sales_days, vec!["2026-07-05".to_string()]);
            assert_eq!(status.unbooked_receipt_count, 0);

            post_daily_sales(conn, "2026-07-05", None, 1, "2026-07-08T09:00:00Z").expect("post");

            let status = kep_status(conn, "2026-07-08T09:00:00Z").expect("status");
            assert!(
                status.overdue_sales_days.is_empty(),
                "posted day is no longer overdue"
            );
        });
    }

    // The T+1 boundary is exclusive: today == day + 1 is not yet overdue.
    #[test]
    fn status_does_not_flag_a_day_before_t_plus_one_elapses() {
        with_kep_db("kep_status_boundary", |conn| {
            seed_shift(conn);
            seed_completed_sale(conn, 1, "2026-07-05T10:00:00Z", 234000);

            // The day after (T+1) is the deadline, not yet overdue.
            let status = kep_status(conn, "2026-07-06T09:00:00Z").expect("status");
            assert!(
                status.overdue_sales_days.is_empty(),
                "day + 1 is the deadline, not overdue yet"
            );
        });
    }
}
