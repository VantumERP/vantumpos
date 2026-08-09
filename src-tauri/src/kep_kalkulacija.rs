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
use rusqlite::{params, Connection, OptionalExtension, Transaction};

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

/// A kalkulacija that has just been written: its id, and the redni broj allocated
/// for it. The redni broj travels back with the id because kolona 3 names the
/// kalkulacija by it (PEP čl. 15 st. 4, and §2.7's worked ledger row) — re-reading
/// it from the caller would be a second query for a number this function has
/// already computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreatedKalkulacija {
    pub id: i64,
    pub redni_broj: i64,
}

/// Generates and persists a receipt's kalkulacija (the formal isprava). Runs
/// inside the caller's receive transaction so it is atomic with the 9a zaduženje.
///
/// Snapshots elements 1-3 (company header) from settings, reads elements 5/6/14 +
/// the PDV rate from the product line, derives 9-13 backward (`derive_kalkulacija`,
/// marža may be negative), and allocates `redni_broj = MAX+1` per `book_year`.
/// Element 13 (`prodajna_vrednost_sa_pdv_minor`) equals the zaduženje's amount
/// (`qty × sale_price`), so linking it changes no ledger value.
///
/// `isprava` is the **supplier's** isprava o nabavci (ZoT čl. 29 st. 1) when the
/// operator attached one — a different document from this one in every sense: the
/// kalkulacija is the shop's *own* price document and its header identity is the
/// trgovac's, snapshotted from settings. `None` is a receipt with no supplier
/// document attached, representable on purpose because the receive path warns and
/// never blocks.
#[allow(clippy::too_many_arguments)]
pub fn create_kalkulacija(
    tx: &Transaction<'_>,
    product_id: i64,
    kolicina_milli: i64,
    nabavna_po_jm_minor: i64,
    reference_type: Option<&str>,
    reference_id: Option<i64>,
    isprava_id: Option<i64>,
    acting_user_id: i64,
    now: &str,
) -> Result<CreatedKalkulacija, AppError> {
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
    crate::kep_close::ensure_year_open(tx, book_year)?;
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
            prodajna_cena_po_jm_minor, reference_type, reference_id, isprava_id,
            created_by, created_at
        ) VALUES (
            ?1, ?2, ?3,
            ?4, ?5, ?6,
            ?7, ?8, ?9,
            ?10, ?11, ?12,
            ?13, ?14, ?15,
            ?16, ?17, ?18, ?21,
            ?19, ?20
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
            isprava_id,
        ],
    )?;

    Ok(CreatedKalkulacija {
        id: tx.last_insert_rowid(),
        redni_broj,
    })
}

/// A persisted kalkulacija, all columns, for the export/print surface (camelCase
/// so the frontend contract mirrors it 1:1).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KalkulacijaView {
    pub id: i64,
    pub redni_broj: i64,
    pub book_year: i64,
    pub product_id: i64,
    pub poslovno_ime: String,
    pub prodajno_mesto: String,
    pub pib: String,
    pub trgovacki_naziv: String,
    pub jedinica_mere: String,
    pub kolicina_milli: i64,
    pub nabavna_cena_po_jm_minor: i64,
    pub vrednost_po_fakturi_minor: i64,
    pub razlika_u_ceni_minor: i64,
    pub prodajna_vrednost_bez_pdv_minor: i64,
    pub pdv_minor: i64,
    pub prodajna_vrednost_sa_pdv_minor: i64,
    pub prodajna_cena_po_jm_minor: i64,
    pub reference_type: Option<String>,
    pub reference_id: Option<i64>,
    /// The supplier's isprava o nabavci this receipt was booked against (ZoT
    /// čl. 29 st. 1), or `None` where none was attached. Carried so the link is
    /// visible to a reader of the document; this build does not yet render the
    /// supplier's document *on* the printed kalkulacija.
    pub isprava_id: Option<i64>,
    pub created_by: Option<i64>,
    pub created_at: String,
}

/// A kalkulacija list row (redni broj, product, and the two headline values) for
/// the KEP module's kalkulacije list — the print action loads the full view.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KalkulacijaSummary {
    pub id: i64,
    pub redni_broj: i64,
    pub book_year: i64,
    pub trgovacki_naziv: String,
    pub kolicina_milli: i64,
    pub razlika_u_ceni_minor: i64,
    pub prodajna_vrednost_sa_pdv_minor: i64,
    pub created_at: String,
}

/// Lists a book year's kalkulacije, newest first — the list surface the export
/// command's summaries feed. Read-only; no isprava is generated here.
pub fn list_kalkulacije(
    conn: &Connection,
    book_year: i64,
) -> Result<Vec<KalkulacijaSummary>, AppError> {
    let mut statement = conn.prepare(
        "SELECT id, redni_broj, book_year, trgovacki_naziv, kolicina_milli,
                razlika_u_ceni_minor, prodajna_vrednost_sa_pdv_minor, created_at
         FROM kalkulacije
         WHERE book_year = ?1
         ORDER BY redni_broj DESC",
    )?;
    let rows = statement
        .query_map(params![book_year], |row| {
            Ok(KalkulacijaSummary {
                id: row.get(0)?,
                redni_broj: row.get(1)?,
                book_year: row.get(2)?,
                trgovacki_naziv: row.get(3)?,
                kolicina_milli: row.get(4)?,
                razlika_u_ceni_minor: row.get(5)?,
                prodajna_vrednost_sa_pdv_minor: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Loads one kalkulacija by id (all 14 elements + header snapshot). A missing id
/// is `not_found`, never a silent empty document.
pub fn load_kalkulacija(conn: &Connection, id: i64) -> Result<KalkulacijaView, AppError> {
    conn.query_row(
        "SELECT id, redni_broj, book_year, product_id,
                poslovno_ime, prodajno_mesto, pib,
                trgovacki_naziv, jedinica_mere, kolicina_milli,
                nabavna_cena_po_jm_minor, vrednost_po_fakturi_minor, razlika_u_ceni_minor,
                prodajna_vrednost_bez_pdv_minor, pdv_minor, prodajna_vrednost_sa_pdv_minor,
                prodajna_cena_po_jm_minor, reference_type, reference_id, created_by, created_at,
                isprava_id
         FROM kalkulacije WHERE id = ?1",
        params![id],
        |row| {
            Ok(KalkulacijaView {
                id: row.get(0)?,
                redni_broj: row.get(1)?,
                book_year: row.get(2)?,
                product_id: row.get(3)?,
                poslovno_ime: row.get(4)?,
                prodajno_mesto: row.get(5)?,
                pib: row.get(6)?,
                trgovacki_naziv: row.get(7)?,
                jedinica_mere: row.get(8)?,
                kolicina_milli: row.get(9)?,
                nabavna_cena_po_jm_minor: row.get(10)?,
                vrednost_po_fakturi_minor: row.get(11)?,
                razlika_u_ceni_minor: row.get(12)?,
                prodajna_vrednost_bez_pdv_minor: row.get(13)?,
                pdv_minor: row.get(14)?,
                prodajna_vrednost_sa_pdv_minor: row.get(15)?,
                prodajna_cena_po_jm_minor: row.get(16)?,
                reference_type: row.get(17)?,
                reference_id: row.get(18)?,
                created_by: row.get(19)?,
                created_at: row.get(20)?,
                isprava_id: row.get(21)?,
            })
        },
    )
    .optional()?
    .ok_or_else(|| AppError::not_found(format!("Kalkulacija {id} ne postoji.")))
}

/// Escapes the five HTML-significant characters so dynamic values (a trgovački
/// naziv may contain `<`, `&`, `"`) can never break out of the document.
fn escape_html(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Renders integer minor units (para) as Serbian currency: `780000 → "7.800,00"`
/// — `.` groups thousands, `,` separates para, a leading `-` for a negative
/// (loss-leader marža). Never floats.
fn format_rsd_minor(minor: i64) -> String {
    let negative = minor < 0;
    let abs = minor.unsigned_abs();
    let dinars = abs / 100;
    let para = abs % 100;

    let digits = dinars.to_string();
    let bytes = digits.as_bytes();
    let len = bytes.len();
    let mut grouped = String::with_capacity(len + len / 3);
    for (i, byte) in bytes.iter().enumerate() {
        if i > 0 && (len - i).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(*byte as char);
    }

    let sign = if negative { "-" } else { "" };
    format!("{sign}{grouped},{para:02}")
}

/// Renders a milli quantity (element 7) for display: `50000 → "50"`, keeping any
/// fractional milli as three decimals (`1500 → "1.500"`).
fn format_qty_milli(quantity_milli: i64) -> String {
    if quantity_milli % 1000 == 0 {
        (quantity_milli / 1000).to_string()
    } else {
        let whole = quantity_milli / 1000;
        let fraction = (quantity_milli % 1000).abs();
        format!("{whole}.{fraction:03}")
    }
}

/// The date portion (`YYYY-MM-DD`) of an RFC3339 timestamp for human display.
fn date_only(rfc3339: &str) -> &str {
    rfc3339.get(..10).unwrap_or(rfc3339)
}

/// Renders the kalkulacija cene isprava: a self-contained HTML document listing
/// all 14 elements in Serbian — header identity (1-4) then the per-line table
/// (5-14) — every dynamic value escaped, the marža rendered signed (loss-leaders
/// allowed). SW-6c shape: `<!doctype html>`, inline `<style>`, no `<script>`, no
/// QR/brojač fiscal block; a footer marks it non-fiscal (SW-1).
pub fn render_kalkulacija_html(view: &KalkulacijaView) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"sr-Latn\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<title>Kalkulacija cene</title>\n");
    html.push_str(
        "<style>\n\
         @page { margin: 1cm }\n\
         body { font-family: sans-serif; color: #111; margin: 1cm; }\n\
         h1 { font-size: 1.4rem; }\n\
         .meta { color: #444; margin: 0.15rem 0; }\n\
         table { border-collapse: collapse; margin: 0.75rem 0; }\n\
         th, td { border: 1px solid #999; padding: 0.25rem 0.5rem; text-align: left; }\n\
         td.amount { text-align: right; white-space: nowrap; }\n\
         footer { margin-top: 2rem; color: #666; font-size: 0.85rem; }\n\
         </style>\n</head>\n<body>\n",
    );

    // Header — elements 1-4 (identity snapshot + redni broj + datum).
    html.push_str("<h1>Kalkulacija cene</h1>\n");
    html.push_str(&format!(
        "<p class=\"meta\">Poslovno ime: {}</p>\n",
        escape_html(&view.poslovno_ime)
    ));
    html.push_str(&format!(
        "<p class=\"meta\">Prodajno mesto: {}</p>\n",
        escape_html(&view.prodajno_mesto)
    ));
    html.push_str(&format!(
        "<p class=\"meta\">PIB: {}</p>\n",
        escape_html(&view.pib)
    ));
    html.push_str(&format!(
        "<p class=\"meta\">Redni broj: {}</p>\n",
        view.redni_broj
    ));
    html.push_str(&format!(
        "<p class=\"meta\">Datum: {}</p>\n",
        escape_html(date_only(&view.created_at))
    ));

    // Per-line — elements 5-14, labelled in Serbian.
    html.push_str("<table>\n<tbody>\n");
    html.push_str(&format!(
        "<tr><th>Trgovački naziv</th><td>{}</td></tr>\n",
        escape_html(&view.trgovacki_naziv)
    ));
    html.push_str(&format!(
        "<tr><th>Jedinica mere</th><td>{}</td></tr>\n",
        escape_html(&view.jedinica_mere)
    ));
    html.push_str(&format!(
        "<tr><th>Količina</th><td class=\"amount\">{}</td></tr>\n",
        escape_html(&format_qty_milli(view.kolicina_milli))
    ));
    for (label, minor) in [
        ("Nabavna cena / jm", view.nabavna_cena_po_jm_minor),
        ("Vrednost po fakturi", view.vrednost_po_fakturi_minor),
        ("Razlika u ceni (marža)", view.razlika_u_ceni_minor),
        (
            "Prodajna vrednost bez PDV",
            view.prodajna_vrednost_bez_pdv_minor,
        ),
        ("PDV", view.pdv_minor),
        (
            "Prodajna vrednost sa PDV",
            view.prodajna_vrednost_sa_pdv_minor,
        ),
        ("Prodajna cena / jm", view.prodajna_cena_po_jm_minor),
    ] {
        html.push_str(&format!(
            "<tr><th>{label}</th><td class=\"amount\">{} RSD</td></tr>\n",
            format_rsd_minor(minor)
        ));
    }
    html.push_str("</tbody>\n</table>\n");

    html.push_str(
        "<footer>Interni dokument. Nije fiskalni dokument.</footer>\n\
         </body>\n</html>\n",
    );
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{remove_test_database, test_database_path, Db};
    use rusqlite::Connection;

    /// Runs `test` against a freshly migrated temp DB (`Db::new` seeds admin id 1).
    fn with_kalkulacija_db(test_name: &str, test: impl FnOnce(&mut Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("db init");
            let mut connection = db.open().expect("open");
            test(&mut connection);
        }
        remove_test_database(&path);
    }

    /// Seeds a tax rate (20% PDV) and a product priced at 156,00 retail / 100,00
    /// nabavna, unit `kom` — the memo §3 worked example line.
    fn seed_product(conn: &Connection) {
        conn.execute(
            "INSERT OR IGNORE INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
             VALUES (1, 'PDV 20', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .expect("tax rate should insert");
        conn.execute(
            "INSERT INTO products (
                id, name, sku, unit_of_measure, sale_price_minor, purchase_price_minor,
                tax_rate_id, created_at, updated_at
             ) VALUES (1, 'Artikal <X>', 'SKU-1', 'kom', 15600, 10000, 1,
                       '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            [],
        )
        .expect("product should insert");
    }

    /// The memo §3 worked example as a persisted view (element 13 = 7.800,00,
    /// marža = 1.500,00), product name carrying markup so escaping is exercised.
    fn sample_view() -> KalkulacijaView {
        KalkulacijaView {
            id: 1,
            redni_broj: 12,
            book_year: 2026,
            product_id: 1,
            poslovno_ime: "Vantum Market".into(),
            prodajno_mesto: "Bulevar 1, Beograd".into(),
            pib: "123456789".into(),
            trgovacki_naziv: "Artikal <X> & \"Y\"".into(),
            jedinica_mere: "kom".into(),
            kolicina_milli: 50_000,
            nabavna_cena_po_jm_minor: 10_000,
            vrednost_po_fakturi_minor: 500_000,
            razlika_u_ceni_minor: 150_000,
            prodajna_vrednost_bez_pdv_minor: 650_000,
            pdv_minor: 130_000,
            prodajna_vrednost_sa_pdv_minor: 780_000,
            prodajna_cena_po_jm_minor: 15_600,
            reference_type: Some("kalkulacija".into()),
            reference_id: Some(7),
            isprava_id: None,
            created_by: Some(1),
            created_at: "2026-07-04T09:00:00Z".into(),
        }
    }

    #[test]
    fn kalkulacija_html_lists_elements_escapes_and_is_non_fiscal() {
        let html = render_kalkulacija_html(&sample_view());
        assert!(html.starts_with("<!doctype html>"), "SW-6c document shape");
        assert!(html.contains("Kalkulacija cene"), "document header");
        // Product name (element 5) escaped — never the raw markup.
        assert!(html.contains("Artikal &lt;X&gt; &amp; &quot;Y&quot;"));
        assert!(!html.contains("Artikal <X> & \"Y\""));
        // Memo §3 worked example: element 13 = 7.800,00, marža (element 10) = 1.500,00.
        assert!(
            html.contains("7.800,00"),
            "element 13 prodajna vrednost sa PDV"
        );
        assert!(
            html.contains("1.500,00"),
            "element 10 razlika u ceni (marža)"
        );
        // Header identity (elements 1-4).
        assert!(html.contains("Vantum Market"));
        assert!(html.contains("123456789"));
        // No scripts; non-fiscal footer (SW-1: never resembles a fiscal receipt).
        assert!(!html.contains("<script"), "no scripts");
        assert!(html.contains("Interni dokument. Nije fiskalni dokument."));
    }

    #[test]
    fn kalkulacija_html_renders_negative_marza_with_sign() {
        let mut view = sample_view();
        view.razlika_u_ceni_minor = -150_000; // loss-leader: nabavna > prodajna
        let html = render_kalkulacija_html(&view);
        assert!(
            html.contains("-1.500,00"),
            "a negative marža renders with a sign, never rejected"
        );
    }

    #[test]
    fn load_then_render_round_trips_the_receipt_kalkulacija() {
        with_kalkulacija_db("kalkulacija_load_render", |conn| {
            seed_product(conn);
            let tx = conn.transaction().expect("tx");
            let created = create_kalkulacija(
                &tx,
                1,
                50_000, // 50 kom in milli
                10_000, // nabavna 100,00 / jm
                Some("kalkulacija"),
                Some(7),
                None, // no supplier isprava attached
                1,
                "2026-07-04T09:00:00Z",
            )
            .expect("create");
            tx.commit().expect("commit");

            let view = load_kalkulacija(conn, created.id).expect("load");
            assert_eq!(created.redni_broj, 1, "returned redni broj matches the row");
            assert_eq!(view.redni_broj, 1, "first kalkulacija of the book year");
            assert_eq!(
                view.isprava_id, None,
                "a receipt with no supplier isprava stays representable"
            );
            assert_eq!(
                view.prodajna_vrednost_sa_pdv_minor, 780_000,
                "element 13 = qty × sale_price = 7.800,00"
            );
            assert_eq!(view.razlika_u_ceni_minor, 150_000, "marža = 1.500,00");
            assert_eq!(view.trgovacki_naziv, "Artikal <X>");

            let html = render_kalkulacija_html(&view);
            assert!(html.contains("7.800,00"));
            // Product name escaped on the way out.
            assert!(html.contains("Artikal &lt;X&gt;"));
        });
    }

    #[test]
    fn load_kalkulacija_missing_id_is_not_found() {
        with_kalkulacija_db("kalkulacija_missing", |conn| {
            let err = load_kalkulacija(conn, 999).expect_err("missing id");
            assert_eq!(err.code(), "not_found");
        });
    }

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
