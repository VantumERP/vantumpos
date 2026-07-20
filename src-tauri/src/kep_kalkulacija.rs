//! KEP kalkulacija cene — the 14 elements derived backward from the catalog price.
//!
//! Legal authority: `docs/KEP-VERIFIED-RULES.md` (§3 kalkulacija). Design:
//! `docs/superpowers/specs/2026-07-19-kep-kalkulacija-storno-design.md`.
//!
//! Catalog-price-authoritative: element 13 (prodajna vrednost sa PDV) is the
//! anchor; marža (element 10) is derived last and MAY be negative (loss-leader).
//! 11 + 12 == 13 exactly (no rounding drift).
#![allow(dead_code)]

use crate::app_error::AppError;
use crate::commands::settings::CompanySettings;
use crate::kep::book_year_of;
use rusqlite::{params, OptionalExtension, Transaction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KalkulacijaElements {
    pub vrednost_po_fakturi_minor: i64,       // 9
    pub razlika_u_ceni_minor: i64,            // 10 (signed)
    pub prodajna_vrednost_bez_pdv_minor: i64, // 11
    pub pdv_minor: i64,                       // 12
    pub prodajna_vrednost_sa_pdv_minor: i64,  // 13
}

fn round_div(a: i64, b: i64) -> i64 {
    (a + b / 2) / b
}

pub fn derive_kalkulacija(
    kolicina_milli: i64,
    nabavna_po_jm_minor: i64,
    prodajna_po_jm_minor: i64,
    rate_basis_points: i64,
) -> KalkulacijaElements {
    let prodajna_vrednost_sa_pdv_minor = round_div(kolicina_milli * prodajna_po_jm_minor, 1000);
    let prodajna_vrednost_bez_pdv_minor = round_div(
        prodajna_vrednost_sa_pdv_minor * 10_000,
        10_000 + rate_basis_points,
    );
    let pdv_minor = prodajna_vrednost_sa_pdv_minor - prodajna_vrednost_bez_pdv_minor;
    let vrednost_po_fakturi_minor = round_div(kolicina_milli * nabavna_po_jm_minor, 1000);
    let razlika_u_ceni_minor = prodajna_vrednost_bez_pdv_minor - vrednost_po_fakturi_minor;
    KalkulacijaElements {
        vrednost_po_fakturi_minor,
        razlika_u_ceni_minor,
        prodajna_vrednost_bez_pdv_minor,
        pdv_minor,
        prodajna_vrednost_sa_pdv_minor,
    }
}

/// Generates and persists a receipt's kalkulacija (the formal isprava), returning
/// its new id. Runs inside the caller's receive transaction so it is atomic with
/// the 9a zaduženje.
///
/// Snapshots elements 1-3 (company header) from settings, reads elements 5/6/14 +
/// the PDV rate from the product line, derives 9-13 backward (`derive_kalkulacija`,
/// marža may be negative), and allocates `redni_broj = MAX+1` per `book_year`.
/// Element 13 (`prodajna_vrednost_sa_pdv_minor`) equals the zaduženje's amount
/// (`qty × sale_price`), so linking it changes no ledger value.
#[allow(clippy::too_many_arguments)]
pub fn create_kalkulacija(
    tx: &Transaction<'_>,
    product_id: i64,
    kolicina_milli: i64,
    nabavna_po_jm_minor: i64,
    reference_type: Option<&str>,
    reference_id: Option<i64>,
    acting_user_id: i64,
    now: &str,
) -> Result<i64, AppError> {
    // Elements 1-3: the company header, snapshotted from settings at creation.
    let company_json: Option<String> = tx
        .query_row(
            "SELECT value_json FROM settings WHERE key = 'company'",
            [],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let company: CompanySettings = match company_json {
        Some(json) => serde_json::from_str(&json).map_err(|source| {
            AppError::InvalidState(format!("Podešavanja nisu ispravna: {source}"))
        })?,
        None => CompanySettings::default(),
    };

    // Elements 5, 6, 14 + the PDV rate, from the product line.
    let (trgovacki_naziv, jedinica_mere, prodajna_po_jm_minor, rate_basis_points): (
        String,
        String,
        i64,
        i64,
    ) = tx.query_row(
        "SELECT p.name, p.unit_of_measure, p.sale_price_minor, t.rate_basis_points
         FROM products p JOIN tax_rates t ON t.id = p.tax_rate_id
         WHERE p.id = ?1",
        params![product_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;

    // Elements 9-13, derived backward from the catalog price.
    let elements = derive_kalkulacija(
        kolicina_milli,
        nabavna_po_jm_minor,
        prodajna_po_jm_minor,
        rate_basis_points,
    );

    let book_year = book_year_of(now)?;
    let redni_broj: i64 = tx.query_row(
        "SELECT COALESCE(MAX(redni_broj), 0) + 1 FROM kalkulacije WHERE book_year = ?1",
        params![book_year],
        |row| row.get(0),
    )?;

    tx.execute(
        "INSERT INTO kalkulacije (
            redni_broj, book_year, product_id,
            poslovno_ime, prodajno_mesto, pib,
            trgovacki_naziv, jedinica_mere, kolicina_milli,
            nabavna_cena_po_jm_minor, vrednost_po_fakturi_minor, razlika_u_ceni_minor,
            prodajna_vrednost_bez_pdv_minor, pdv_minor, prodajna_vrednost_sa_pdv_minor,
            prodajna_cena_po_jm_minor, reference_type, reference_id, created_by, created_at
        ) VALUES (
            ?1, ?2, ?3,
            ?4, ?5, ?6,
            ?7, ?8, ?9,
            ?10, ?11, ?12,
            ?13, ?14, ?15,
            ?16, ?17, ?18, ?19, ?20
        )",
        params![
            redni_broj,
            book_year,
            product_id,
            company.shop_name,
            company.address,
            company.pib,
            trgovacki_naziv,
            jedinica_mere,
            kolicina_milli,
            nabavna_po_jm_minor,
            elements.vrednost_po_fakturi_minor,
            elements.razlika_u_ceni_minor,
            elements.prodajna_vrednost_bez_pdv_minor,
            elements.pdv_minor,
            elements.prodajna_vrednost_sa_pdv_minor,
            prodajna_po_jm_minor,
            reference_type,
            reference_id,
            acting_user_id,
            now,
        ],
    )?;

    Ok(tx.last_insert_rowid())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worked_example_kalkulacija_50_kom() {
        // qty 50 kom = 50_000 milli; nabavna 100,00 = 10000; prodajna sa PDV 156,00/jm = 15600; PDV 20% = 2000 bp.
        let e = derive_kalkulacija(50_000, 10_000, 15_600, 2000);
        assert_eq!(e.prodajna_vrednost_sa_pdv_minor, 780_000, "13 = 7.800,00");
        assert_eq!(e.prodajna_vrednost_bez_pdv_minor, 650_000, "11 = 6.500,00");
        assert_eq!(e.pdv_minor, 130_000, "12 = 1.300,00");
        assert_eq!(e.vrednost_po_fakturi_minor, 500_000, "9 = 5.000,00");
        assert_eq!(e.razlika_u_ceni_minor, 150_000, "10 marža = 1.500,00");
    }
    #[test]
    fn negative_marza_is_allowed_loss_leader() {
        // nabavna 200,00 > prodajna 156,00 sa PDV → marža negative.
        let e = derive_kalkulacija(1_000, 20_000, 15_600, 2000);
        assert!(
            e.razlika_u_ceni_minor < 0,
            "loss-leader marža is negative, not rejected"
        );
        assert_eq!(
            e.prodajna_vrednost_bez_pdv_minor + e.pdv_minor,
            e.prodajna_vrednost_sa_pdv_minor,
            "11+12=13 exact, no drift"
        );
    }
}
