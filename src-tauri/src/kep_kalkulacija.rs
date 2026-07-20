//! KEP kalkulacija cene — the 14 elements derived backward from the catalog price.
//!
//! Legal authority: `docs/KEP-VERIFIED-RULES.md` (§3 kalkulacija). Design:
//! `docs/superpowers/specs/2026-07-19-kep-kalkulacija-storno-design.md`.
//!
//! Catalog-price-authoritative: element 13 (prodajna vrednost sa PDV) is the
//! anchor; marža (element 10) is derived last and MAY be negative (loss-leader).
//! 11 + 12 == 13 exactly (no rounding drift).
#![allow(dead_code)]

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
