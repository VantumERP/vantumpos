//! KEP storno engine — the cause→{kolona, sign, kind} hard map (SW-9b).
//!
//! Legal authority: `docs/KEP-VERIFIED-RULES.md` (§4 nivelacija + storno). Design:
//! `docs/superpowers/specs/2026-07-19-kep-kalkulacija-storno-design.md`.
//!
//! `posting_for` is the SINGLE source of a storno's column, sign, and kind — an
//! exhaustive, fixed map, never user-overridable. A misrouted storno silently
//! corrupts the saldo an inspector checks first (§4.3). A downward nivelacija /
//! PDV-rate cut / otpis / manjak-po-odluci / rashod / supplier return books a
//! **crveni storno in kolona 4** (negative); a **customer** contract-rescission
//! return books a **crveni storno in kolona 5**; a popis **višak** books
//! **+kolona 4** and a popis **manjak** books **+kolona 5**.
//!
//! A crveni storno is stored as a NEGATIVE `amount_minor` (subtracts from the
//! column total). Nivelacija/PDV-rate causes change the product price and are
//! rejected by `post_value_storno` — they route through `post_nivelacija`
//! (Task 6), which revalues the product in one transaction.
//!
//! Rows are append-only (PEP čl. 14): nothing here UPDATEs or DELETEs a posted
//! `kep_entries` row. Consumed by the command layer (Task 8), so `dead_code`
//! is allowed here — mirroring the other domain modules.

#![allow(dead_code)]

use crate::app_error::AppError;
use crate::kep::{book_year_of, next_redni_broj};
use rusqlite::{params, Transaction};

/// The finer legal cause of a storno — never user-chosen for its column/sign
/// (that is `posting_for`'s job), but stored so causes sharing a `kind` (otpis
/// vs rashod vs manjak-po-odluci) stay legally distinct.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StornoCause {
    NivelacijaUp,
    NivelacijaDown,
    PdvRateUp,
    PdvRateDown,
    SupplierReturn,
    CustomerReturn,
    Otpis,
    ManjakOdluka,
    Rashod,
    PopisVisak,
    PopisManjak,
}

/// The resolved posting for a cause: which column, whether it is a crveni storno
/// (negative amount), the ledger `kind`, and the finer `cause` string persisted
/// on the row. Produced only by `posting_for`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Posting {
    pub kolona: &'static str,
    pub kind: &'static str,
    pub cause: &'static str,
    pub negative: bool,
}

/// The HARD map (§4.3): the single source of a storno's column, sign, and kind.
/// Exhaustive over `StornoCause` and fixed — never overridable by a caller.
pub fn posting_for(cause: StornoCause) -> Posting {
    match cause {
        // Upward nivelacija / PDV-rate rise: a normal zaduženje in kolona 4.
        StornoCause::NivelacijaUp => Posting {
            kolona: "zaduzenje",
            kind: "nivelacija_up",
            cause: "nivelacija_up",
            negative: false,
        },
        StornoCause::PdvRateUp => Posting {
            kolona: "zaduzenje",
            kind: "nivelacija_up",
            cause: "pdv_rate_up",
            negative: false,
        },
        // Downward nivelacija / PDV-rate cut: crveni storno in kolona 4.
        StornoCause::NivelacijaDown => Posting {
            kolona: "zaduzenje",
            kind: "nivelacija_down_storno",
            cause: "nivelacija_down",
            negative: true,
        },
        StornoCause::PdvRateDown => Posting {
            kolona: "zaduzenje",
            kind: "nivelacija_down_storno",
            cause: "pdv_rate_down",
            negative: true,
        },
        // Supplier return: crveni storno in kolona 4 (PEP čl. 15 st. 5 tač. 5).
        StornoCause::SupplierReturn => Posting {
            kolona: "zaduzenje",
            kind: "supplier_return_storno",
            cause: "supplier_return",
            negative: true,
        },
        // Otpis / manjak-po-odluci / rashod: crveni storno in kolona 4. They
        // share the down-storno kind but keep a distinct `cause`.
        StornoCause::Otpis => Posting {
            kolona: "zaduzenje",
            kind: "nivelacija_down_storno",
            cause: "otpis",
            negative: true,
        },
        StornoCause::ManjakOdluka => Posting {
            kolona: "zaduzenje",
            kind: "nivelacija_down_storno",
            cause: "manjak_odluka",
            negative: true,
        },
        StornoCause::Rashod => Posting {
            kolona: "zaduzenje",
            kind: "nivelacija_down_storno",
            cause: "rashod",
            negative: true,
        },
        // Customer contract-rescission return: crveni storno in kolona 5
        // (PEP čl. 15 st. 6 tač. 3) — the ONLY storno in the razduženje column.
        StornoCause::CustomerReturn => Posting {
            kolona: "razduzenje",
            kind: "customer_return_storno",
            cause: "customer_return",
            negative: true,
        },
        // Popis differences (PEP čl. 16 st. 2): višak → +kolona 4, manjak →
        // +kolona 5. Positive entries, not crveni storna.
        StornoCause::PopisVisak => Posting {
            kolona: "zaduzenje",
            kind: "popis_visak",
            cause: "popis_visak",
            negative: false,
        },
        StornoCause::PopisManjak => Posting {
            kolona: "razduzenje",
            kind: "popis_manjak",
            cause: "popis_manjak",
            negative: false,
        },
    }
}

/// A storno's basis isprava (naziv/broj/datum), composed into the KEP `opis`.
#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BasisDoc {
    pub naziv: String,
    pub broj: String,
    pub datum: String,
}

fn round_div(a: i64, b: i64) -> i64 {
    (a + b / 2) / b
}

/// Posts a value-only KEP storno for every cause **except** the two nivelacija
/// causes (and the two PDV-rate causes) — those change the product price and
/// route through `post_nivelacija` (Task 6), so they are rejected here.
///
/// `amount = round_div(quantity_milli × product.sale_price_minor, 1000)`, stored
/// **negative** when `posting.negative` (a crveni storno subtracts from the
/// column total). `opis = "{naziv} br. {broj} od {datum}"`. Append-only INSERT
/// inside the caller's transaction; no inventory is touched (the physical
/// write-off stays the shop's separate action).
pub fn post_value_storno(
    tx: &Transaction<'_>,
    cause: StornoCause,
    product_id: i64,
    quantity_milli: i64,
    basis: &BasisDoc,
    acting: i64,
    now: &str,
) -> Result<(), AppError> {
    let posting = posting_for(cause);
    if matches!(
        cause,
        StornoCause::NivelacijaUp
            | StornoCause::NivelacijaDown
            | StornoCause::PdvRateUp
            | StornoCause::PdvRateDown
    ) {
        return Err(AppError::business(
            "invalid_state",
            "Nivelacija menja cenu — koristite nivelaciju.",
        ));
    }

    let sale_price_minor: i64 = tx
        .query_row(
            "SELECT sale_price_minor FROM products WHERE id = ?1",
            params![product_id],
            |row| row.get(0),
        )
        .map_err(|source| match source {
            rusqlite::Error::QueryReturnedNoRows => AppError::not_found("Proizvod nije pronađen."),
            other => AppError::from(other),
        })?;

    let magnitude = round_div(quantity_milli * sale_price_minor, 1000);
    let amount_minor = if posting.negative {
        -magnitude
    } else {
        magnitude
    };

    let book_year = book_year_of(now)?;
    let redni_broj = next_redni_broj(tx, book_year)?;
    let opis = format!("{} br. {} od {}", basis.naziv, basis.broj, basis.datum);

    tx.execute(
        "INSERT INTO kep_entries (
            book_year, redni_broj, entry_date, document_date, opis,
            kolona, amount_minor, kind, entry_source,
            reference_type, reference_id, user_id, created_at, cause
        ) VALUES (
            ?1, ?2, ?3, NULL, ?4,
            ?5, ?6, ?7, 'manual',
            'product', ?8, ?9, ?3, ?10
        )",
        params![
            book_year,
            redni_broj,
            now,
            opis,
            posting.kolona,
            amount_minor,
            posting.kind,
            product_id,
            acting,
            posting.cause,
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};
    use crate::kep::list_ledger;
    use rusqlite::Connection;

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

    /// Seeds a tax rate and a product with the given retail price.
    fn seed_product(conn: &Connection, id: i64, sale_price_minor: i64) {
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
             ) VALUES (?1, ?2, ?3, ?4, 0, 1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            params![
                id,
                format!("Proizvod {id}"),
                format!("SKU-{id}"),
                sale_price_minor
            ],
        )
        .expect("product should insert");
    }

    fn basis() -> BasisDoc {
        BasisDoc {
            naziv: "Popisna lista".to_string(),
            broj: "4".to_string(),
            datum: "05.07.2026".to_string(),
        }
    }

    fn last_row(conn: &Connection) -> (String, i64, String, String, String) {
        conn.query_row(
            "SELECT kolona, amount_minor, kind, cause, opis
             FROM kep_entries ORDER BY id DESC LIMIT 1",
            [],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, String>(4)?,
                ))
            },
        )
        .expect("a kep row")
    }

    // §4.5: supplier return / otpis 35 kom @ 156,00 -> -5.460,00 crveni storno
    // in kolona 4.
    #[test]
    fn otpis_books_negative_in_kolona_4() {
        with_kep_db("storno_otpis", |conn| {
            seed_product(conn, 1, 15600); // 156,00 retail
            let tx = conn.transaction().expect("tx");
            post_value_storno(
                &tx,
                StornoCause::Otpis,
                1,
                35_000,
                &basis(),
                1,
                "2026-07-08T09:00:00Z",
            )
            .expect("post");
            tx.commit().expect("commit");

            let (kolona, amount, kind, cause, opis) = last_row(conn);
            assert_eq!(kolona, "zaduzenje");
            assert_eq!(amount, -546000, "35 x 156,00 crveni storno");
            assert_eq!(kind, "nivelacija_down_storno");
            assert_eq!(cause, "otpis");
            assert_eq!(opis, "Popisna lista br. 4 od 05.07.2026");
        });
    }

    #[test]
    fn supplier_return_books_negative_in_kolona_4() {
        with_kep_db("storno_supplier", |conn| {
            seed_product(conn, 1, 15600);
            let tx = conn.transaction().expect("tx");
            post_value_storno(
                &tx,
                StornoCause::SupplierReturn,
                1,
                35_000,
                &basis(),
                1,
                "2026-07-08T09:00:00Z",
            )
            .expect("post");
            tx.commit().expect("commit");

            let (kolona, amount, kind, cause, _) = last_row(conn);
            assert_eq!(kolona, "zaduzenje");
            assert_eq!(amount, -546000);
            assert_eq!(kind, "supplier_return_storno");
            assert_eq!(cause, "supplier_return");
        });
    }

    // §4.5: customer contract-rescission return of 1 kom @ 156,00 -> -156,00
    // crveni storno in kolona 5.
    #[test]
    fn customer_return_books_negative_in_kolona_5() {
        with_kep_db("storno_customer", |conn| {
            seed_product(conn, 1, 15600);
            let tx = conn.transaction().expect("tx");
            post_value_storno(
                &tx,
                StornoCause::CustomerReturn,
                1,
                1_000,
                &basis(),
                1,
                "2026-07-08T09:00:00Z",
            )
            .expect("post");
            tx.commit().expect("commit");

            let (kolona, amount, kind, cause, _) = last_row(conn);
            assert_eq!(kolona, "razduzenje");
            assert_eq!(amount, -15600, "1 x 156,00 crveni storno");
            assert_eq!(kind, "customer_return_storno");
            assert_eq!(cause, "customer_return");

            // The crveni storno in kolona 5 RAISES the saldo (goods back on hand).
            let ledger = list_ledger(conn, 2026).expect("ledger");
            assert_eq!(ledger.saldo_minor, 15600, "0 - (-15600) = +15600");
        });
    }

    // §4.3 popis differences: višak +kolona 4, manjak +kolona 5.
    #[test]
    fn popis_visak_kolona_4_manjak_kolona_5() {
        with_kep_db("storno_popis", |conn| {
            seed_product(conn, 1, 15600);

            let tx = conn.transaction().expect("tx");
            post_value_storno(
                &tx,
                StornoCause::PopisVisak,
                1,
                10_000,
                &basis(),
                1,
                "2026-07-08T09:00:00Z",
            )
            .expect("visak");
            tx.commit().expect("commit");
            let (kolona, amount, kind, cause, _) = last_row(conn);
            assert_eq!(kolona, "zaduzenje");
            assert_eq!(amount, 156000, "positive, not a storno");
            assert_eq!(kind, "popis_visak");
            assert_eq!(cause, "popis_visak");

            let tx = conn.transaction().expect("tx");
            post_value_storno(
                &tx,
                StornoCause::PopisManjak,
                1,
                10_000,
                &basis(),
                1,
                "2026-07-08T09:05:00Z",
            )
            .expect("manjak");
            tx.commit().expect("commit");
            let (kolona, amount, kind, cause, _) = last_row(conn);
            assert_eq!(kolona, "razduzenje");
            assert_eq!(amount, 156000, "positive, not a storno");
            assert_eq!(kind, "popis_manjak");
            assert_eq!(cause, "popis_manjak");
        });
    }

    #[test]
    fn posting_map_is_exhaustive_and_fixed() {
        assert_eq!(
            posting_for(StornoCause::CustomerReturn).kolona,
            "razduzenje"
        );
        assert_eq!(posting_for(StornoCause::Otpis).kolona, "zaduzenje");
        assert_eq!(posting_for(StornoCause::ManjakOdluka).kolona, "zaduzenje");
        assert_eq!(posting_for(StornoCause::Rashod).kolona, "zaduzenje");
        assert_eq!(posting_for(StornoCause::SupplierReturn).kolona, "zaduzenje");
        assert!(posting_for(StornoCause::NivelacijaDown).negative);
        assert!(posting_for(StornoCause::PdvRateDown).negative);
        assert!(!posting_for(StornoCause::NivelacijaUp).negative);
        assert!(!posting_for(StornoCause::PopisVisak).negative);
        assert!(!posting_for(StornoCause::PopisManjak).negative);
        // Otpis/manjak/rashod share the kind but keep distinct causes.
        assert_eq!(
            posting_for(StornoCause::Otpis).kind,
            "nivelacija_down_storno"
        );
        assert_eq!(posting_for(StornoCause::Otpis).cause, "otpis");
        assert_eq!(posting_for(StornoCause::Rashod).cause, "rashod");
        assert_eq!(
            posting_for(StornoCause::ManjakOdluka).cause,
            "manjak_odluka"
        );
    }

    // The two nivelacija (and PDV-rate) causes change the product price and must
    // route through post_nivelacija — post_value_storno rejects them.
    #[test]
    fn nivelacija_causes_are_rejected_by_value_storno() {
        with_kep_db("storno_reject_nivelacija", |conn| {
            seed_product(conn, 1, 15600);
            for cause in [
                StornoCause::NivelacijaUp,
                StornoCause::NivelacijaDown,
                StornoCause::PdvRateUp,
                StornoCause::PdvRateDown,
            ] {
                let tx = conn.transaction().expect("tx");
                let err =
                    post_value_storno(&tx, cause, 1, 1_000, &basis(), 1, "2026-07-08T09:00:00Z")
                        .expect_err("nivelacija must be rejected");
                assert_eq!(err.code(), "invalid_state");
                drop(tx);
            }
        });
    }
}
