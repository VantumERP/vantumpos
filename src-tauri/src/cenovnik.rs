//! SW-12 — the machine-readable cenovnik, rendered.
//!
//! Pure: no database, no clock, no filesystem. Rows in, file out — so what the
//! shop publishes under ZZP čl. 6 st. 2 is decided by one function that can be
//! read end to end, and the archive's `content_hash` is a function of the
//! catalog rather than of whatever order a SELECT happened to return.
//!
//! Legal authority: `docs/REMAINING-SW-VERIFIED-RULES.md` §2b, §4 reqs. 10, 16.
//! Design: `docs/superpowers/specs/2026-08-01-sw12-cenovnik-design.md`.
//!
//! **No format is legally mandated** — the čl. 6 st. 7 bylaw does not exist, so
//! the shape below is the de facto data.gov.rs practice (req. 16), not law, and
//! swapping it is a routine release rather than a compliance event.
//!
//! The API is consumed by the publish path and the archive (Tasks 3, 4 and 7),
//! so `dead_code` is allowed here while it lands ahead of them — mirroring the
//! other domain modules (`kep`, `reklamacije`, `campaign_evidence`).

#![allow(dead_code)]

use std::fmt::Write as _;

use sha2::{Digest, Sha256};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// The published columns, in order (req. 16).
///
/// ASCII on purpose: these are machine field names in a file meant to be parsed,
/// not operator-facing prose, and the de facto shape spells them without
/// diacritics. The *data* carries Serbian Latin verbatim — which is what the BOM
/// below is for.
pub const COLUMNS: [&str; 8] = [
    "sifra",
    "barkod",
    "naziv",
    "jedinica_mere",
    "prodajna_cena",
    "jedinicna_cena",
    "jedinica_za_jedinicnu_cenu",
    "datum_azuriranja",
];

const SEPARATOR: char = ';';

/// CRLF, per RFC 4180. `str::lines` reads either, so nothing in-tree depends on
/// the choice; a strict third-party parser might.
const LINE_ENDING: &str = "\r\n";

/// One catalog row as the published file sees it.
///
/// The unit-price pair is carried in its stored shape — the measure plus the
/// content of one selling unit on the schema-wide milli scale (v19) — rather
/// than pre-divided by the caller, so the division and its empty cases live in
/// one tested place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CenovnikRow {
    pub sifra: String,
    pub barkod: Option<String>,
    pub naziv: String,
    /// The measure the goods are SOLD in (`products.unit_of_measure`).
    pub jedinica_mere: String,
    pub prodajna_cena_minor: i64,
    /// The measure the jedinična cena is EXPRESSED in
    /// (`products.jedinicna_cena_jedinica`). `None` means the shop has not said,
    /// and nothing here will guess one.
    pub jedinicna_cena_jedinica: Option<String>,
    /// The content of one selling unit in that measure, value × 1000
    /// (`products.jedinicna_cena_sadrzaj_milli`): a 0,75 l bottle is `750`.
    pub jedinicna_cena_sadrzaj_milli: Option<i64>,
    /// RFC3339, as every timestamp in this crate travels.
    pub datum_azuriranja: String,
}

impl CenovnikRow {
    /// The jedinična cena in para per one unit of the measure, or `None` when the
    /// row cannot state one.
    ///
    /// Čl. 6 st. 2's second sentence pulls st. 1 into the published file, so this
    /// is not decoration. But an unconfigured product yields `None` and publishes
    /// an empty cell: a jedinična cena inferred from a package size nobody
    /// entered would be published as fact, and the shop is answerable under
    /// čl. 6 st. 4 for what it publishes. A missing cell is a visible gap; a
    /// guessed one is not.
    pub fn jedinicna_cena_minor(&self) -> Option<i64> {
        self.jedinicna_cena_jedinica.as_ref()?;
        let Some(sadrzaj_milli) = self.jedinicna_cena_sadrzaj_milli else {
            // One selling unit IS one of the measure (v19), so the two coincide.
            return Some(self.prodajna_cena_minor);
        };
        if sadrzaj_milli <= 0 {
            // The v19 CHECK refuses this; a row that reached us anyway divides by
            // nothing, and no cell beats a wrong one.
            return None;
        }
        // i128 so an absurd price cannot overflow the ×1000 and wrap into a
        // plausible-looking unit price; rounded to the nearest para, because
        // truncation publishes 6,66 where the true figure is 6,67.
        let scaled = i128::from(self.prodajna_cena_minor) * 1000;
        let rounded = (scaled + i128::from(sadrzaj_milli) / 2) / i128::from(sadrzaj_milli);
        i64::try_from(rounded).ok()
    }
}

/// Renders the published file: BOM, header, one line per row, CRLF-terminated.
///
/// Rows are emitted in `sifra` order regardless of the order given (ties keep
/// their input order), so the same catalog always renders the same bytes and
/// therefore the same [`content_hash`]. Without that, a caller changing its
/// ORDER BY would look like a price change to the archive.
pub fn render_csv(rows: &[CenovnikRow]) -> String {
    let mut ordered: Vec<&CenovnikRow> = rows.iter().collect();
    ordered.sort_by(|left, right| left.sifra.cmp(&right.sifra));

    // The BOM is not cosmetic: without it a spreadsheet opening the file under a
    // legacy codepage renders „Košulja“ as mojibake, and the naziv is what a
    // consumer matches the goods on.
    let mut out = String::from('\u{feff}');
    out.push_str(&line(&COLUMNS.map(|column| column.to_string())));

    for row in ordered {
        let jedinicna_cena = row
            .jedinicna_cena_minor()
            .map(format_para_2dec)
            .unwrap_or_default();
        out.push_str(&line(&[
            escape(&row.sifra),
            // Quoted as text so a consumer honouring CSV quoting keeps a 13-digit
            // barcode a string: read as a number, `0123456789012` loses its
            // leading zero and stops matching the goods.
            row.barkod.as_deref().map(as_text).unwrap_or_default(),
            escape(&row.naziv),
            escape(&row.jedinica_mere),
            format_para_2dec(row.prodajna_cena_minor),
            jedinicna_cena,
            row.jedinicna_cena_jedinica
                .as_deref()
                .map(escape)
                .unwrap_or_default(),
            format_date(&row.datum_azuriranja),
        ]));
    }

    out
}

/// SHA-256 of the rendered body, lowercase hex — the archive's handle on „which
/// file was published“ (čl. 6 st. 5) and the only cheap way to tell a republish
/// that changed something from one that changed nothing.
pub fn content_hash(body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body.as_bytes());

    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

fn line(fields: &[String]) -> String {
    let mut out = fields.join(&SEPARATOR.to_string());
    out.push_str(LINE_ENDING);
    out
}

/// RFC 4180 escaping against this file's separator.
fn escape(value: &str) -> String {
    if value.contains([SEPARATOR, '"', '\n', '\r']) {
        as_text(value)
    } else {
        value.to_string()
    }
}

fn as_text(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

/// Integer para as `1234.56` — two decimals, a `.` decimal mark (req. 16), no
/// thousands grouping, never a float.
fn format_para_2dec(minor: i64) -> String {
    let sign = if minor < 0 { "-" } else { "" };
    let abs = minor.unsigned_abs();
    format!("{sign}{}.{:02}", abs / 100, abs % 100)
}

/// `DD-MM-YYYY` (req. 16) from an RFC3339 stamp, in the stamp's own offset — the
/// same reading `kep.rs` takes of a booking date, and this crate stamps in UTC.
///
/// A stamp that will not parse is passed through verbatim rather than blanked:
/// the row's date is evidence of when the price last moved, and a visibly wrong
/// value is a defect somebody can see, where an empty cell is one nobody can.
fn format_date(rfc3339: &str) -> String {
    match OffsetDateTime::parse(rfc3339, &Rfc3339) {
        Ok(stamp) => format!(
            "{:02}-{:02}-{:04}",
            stamp.day(),
            stamp.month() as u8,
            stamp.year()
        ),
        Err(_) => escape(rfc3339.trim()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(sifra: &str) -> CenovnikRow {
        CenovnikRow {
            sifra: sifra.to_string(),
            barkod: None,
            naziv: "Košulja".to_string(),
            jedinica_mere: "kom".to_string(),
            prodajna_cena_minor: 279_900,
            jedinicna_cena_jedinica: None,
            jedinicna_cena_sadrzaj_milli: None,
            datum_azuriranja: "2026-08-02T09:15:00Z".to_string(),
        }
    }

    /// A 0,75 l bottle at 279,00 RSD: 750 milli of `l`, so 372,00 RSD per litar.
    fn row_with_unit_price() -> CenovnikRow {
        CenovnikRow {
            naziv: "Sok od jabuke 0,75 l".to_string(),
            prodajna_cena_minor: 27_900,
            jedinicna_cena_jedinica: Some("l".to_string()),
            jedinicna_cena_sadrzaj_milli: Some(750),
            ..row("A-1")
        }
    }

    fn row_with_barcode(barkod: &str) -> CenovnikRow {
        CenovnikRow {
            barkod: Some(barkod.to_string()),
            ..row("A-1")
        }
    }

    fn row_priced(prodajna_cena_minor: i64) -> CenovnikRow {
        CenovnikRow {
            prodajna_cena_minor,
            ..row("A-1")
        }
    }

    fn data_row(csv: &str) -> String {
        csv.lines()
            .nth(1)
            .expect("one data row")
            .trim_end_matches('\r')
            .to_string()
    }

    fn data_fields(csv: &str) -> Vec<String> {
        data_row(csv).split(';').map(str::to_string).collect()
    }

    /// čl. 6 st. 2's second sentence pulls st. 1 into the published file, so a
    /// cenovnik carrying only prodajna cena does not discharge the duty.
    #[test]
    fn every_row_carries_the_unit_price_and_its_measure() {
        let csv = render_csv(&[row_with_unit_price()]);
        let header = csv.lines().next().expect("header");
        assert!(header.contains("jedinicna_cena"), "{header}");
        assert!(header.contains("jedinica_za_jedinicnu_cenu"), "{header}");
    }

    #[test]
    fn the_file_is_utf8_with_a_bom_and_semicolon_separated() {
        let csv = render_csv(&[row_with_unit_price()]);
        assert!(
            csv.starts_with('\u{feff}'),
            "a BOM keeps Excel from mangling the diacritics"
        );
        assert!(csv.lines().next().expect("header").contains(';'));
    }

    /// A 13-digit barcode read as a number loses its leading zero and stops
    /// matching the goods. Quote it as text.
    #[test]
    fn the_barcode_is_quoted_as_text() {
        let csv = render_csv(&[row_with_barcode("0123456789012")]);
        assert!(csv.contains("\"0123456789012\""), "{csv}");
    }

    #[test]
    fn prices_render_two_decimals_from_integer_para() {
        let csv = render_csv(&[row_priced(123_456)]); // 1234,56 RSD
        assert!(csv.contains("1234.56"), "{csv}");
        assert!(!csv.contains("1234.5600000"), "no float artefacts: {csv}");
    }

    #[test]
    fn the_hash_changes_when_any_price_changes() {
        let a = content_hash(&render_csv(&[row_priced(100)]));
        let b = content_hash(&render_csv(&[row_priced(101)]));
        assert_ne!(a, b);
    }

    /// The header is a contract with whoever parses the file, so its spelling and
    /// its order are pinned rather than left to whatever the constant happens to
    /// say today.
    #[test]
    fn the_header_is_the_published_column_order() {
        let csv = render_csv(&[]);
        assert_eq!(
            csv.lines().next().expect("header"),
            "\u{feff}sifra;barkod;naziv;jedinica_mere;prodajna_cena;\
             jedinicna_cena;jedinica_za_jedinicnu_cenu;datum_azuriranja"
        );
    }

    /// A shop with nothing to publish still publishes a well-formed file. The
    /// v19 archive allows `row_count = 0` for exactly this.
    #[test]
    fn an_empty_catalog_still_renders_the_header() {
        let csv = render_csv(&[]);
        assert!(csv.starts_with('\u{feff}'));
        assert_eq!(csv.lines().count(), 1, "header only: {csv:?}");
    }

    /// Req. 10's whole point: the unit price is derived from the content of one
    /// selling unit, not copied off the sale price. 279,00 RSD for 0,75 l is
    /// 372,00 RSD per litar.
    #[test]
    fn the_unit_price_is_derived_from_the_content_of_one_selling_unit() {
        let csv = render_csv(&[row_with_unit_price()]);
        let fields = data_fields(&csv);
        assert_eq!(fields[4], "279.00", "prodajna cena: {fields:?}");
        assert_eq!(fields[5], "372.00", "jedinična cena po litru: {fields:?}");
        assert_eq!(fields[6], "l", "{fields:?}");
    }

    /// A `sadržaj` of NULL means one selling unit is one of the measure, so the
    /// two prices legitimately coincide — the only case in which they may.
    #[test]
    fn a_unit_that_is_the_selling_unit_repeats_the_sale_price() {
        let row = CenovnikRow {
            jedinicna_cena_jedinica: Some("kom".to_string()),
            jedinicna_cena_sadrzaj_milli: None,
            ..row_priced(27_900)
        };
        let fields = data_fields(&render_csv(&[row]));
        assert_eq!(fields[5], "279.00", "{fields:?}");
        assert_eq!(fields[6], "kom", "{fields:?}");
    }

    /// Nothing here guesses. An unconfigured product publishes empty cells, so
    /// the gap is visible instead of being filled with a figure the shop would
    /// then be answerable for under čl. 6 st. 4.
    #[test]
    fn an_unconfigured_unit_price_leaves_the_cells_empty() {
        let unconfigured = row_priced(27_900);
        assert_eq!(unconfigured.jedinicna_cena_minor(), None);

        let fields = data_fields(&render_csv(&[unconfigured]));
        assert_eq!(fields[4], "279.00", "{fields:?}");
        assert_eq!(fields[5], "", "no guessed jedinična cena: {fields:?}");
        assert_eq!(fields[6], "", "{fields:?}");
    }

    /// Truncation would publish 6,66 where the true unit price is 6,67. Rounding
    /// is to the nearest para, and it is the published figure that must be right.
    #[test]
    fn the_unit_price_rounds_to_the_nearest_para() {
        let row = CenovnikRow {
            jedinicna_cena_jedinica: Some("kg".to_string()),
            jedinicna_cena_sadrzaj_milli: Some(300),
            ..row_priced(200)
        };
        assert_eq!(row.jedinicna_cena_minor(), Some(667));
    }

    /// The v19 CHECK refuses a non-positive sadržaj; if one ever reaches the
    /// renderer it divides by nothing, and an empty cell beats a wrong price or a
    /// panic in the publish path.
    #[test]
    fn a_nonpositive_content_publishes_no_unit_price_instead_of_dividing() {
        let row = CenovnikRow {
            jedinicna_cena_jedinica: Some("kg".to_string()),
            jedinicna_cena_sadrzaj_milli: Some(0),
            ..row_priced(27_900)
        };
        assert_eq!(row.jedinicna_cena_minor(), None);

        let fields = data_fields(&render_csv(&[row]));
        assert_eq!(fields[5], "", "{fields:?}");
    }

    #[test]
    fn a_product_without_a_barcode_leaves_the_cell_empty() {
        let fields = data_fields(&render_csv(&[row_priced(100)]));
        assert_eq!(fields[1], "", "an absent barcode is empty, not \"\"");
    }

    /// The naziv is what a consumer matches the goods on, so a semicolon or a
    /// quote inside it must not shift every following column by one.
    #[test]
    fn a_naziv_carrying_the_separator_or_a_quote_is_escaped() {
        let row = CenovnikRow {
            naziv: "Košulja; „bela“ 15\" kragna".to_string(),
            ..row_priced(100)
        };
        let rendered = data_row(&render_csv(&[row]));
        assert!(
            rendered.contains("\"Košulja; „bela“ 15\"\" kragna\""),
            "{rendered}"
        );
        assert_eq!(
            rendered.matches("1.00").count(),
            1,
            "the columns must not shift: {rendered}"
        );
    }

    #[test]
    fn the_date_renders_dd_mm_yyyy() {
        let row = CenovnikRow {
            datum_azuriranja: "2026-08-02T23:15:00Z".to_string(),
            ..row_priced(100)
        };
        let fields = data_fields(&render_csv(&[row]));
        assert_eq!(fields[7], "02-08-2026", "{fields:?}");
    }

    /// A stamp the renderer cannot read is a defect upstream. Publishing it
    /// verbatim keeps that defect visible; blanking the cell hides it.
    #[test]
    fn an_unparseable_stamp_is_published_verbatim_rather_than_blanked() {
        let row = CenovnikRow {
            datum_azuriranja: "juče".to_string(),
            ..row_priced(100)
        };
        let fields = data_fields(&render_csv(&[row]));
        assert_eq!(fields[7], "juče", "{fields:?}");
    }

    /// The archive dedupes and compares on `content_hash`, so the same catalog
    /// must render the same bytes no matter what order it arrives in — otherwise
    /// a caller's ORDER BY looks like a price change (čl. 6 st. 5).
    #[test]
    fn the_render_is_ordered_by_sifra_so_the_hash_does_not_follow_the_select() {
        let first = row("A-1");
        let second = row("B-2");
        let third = row("C-3");

        let ascending = render_csv(&[first.clone(), second.clone(), third.clone()]);
        let shuffled = render_csv(&[third, first, second]);

        assert_eq!(ascending, shuffled);
        assert_eq!(content_hash(&ascending), content_hash(&shuffled));
        let sifre: Vec<&str> = ascending
            .lines()
            .skip(1)
            .map(|row| row.split(';').next().expect("sifra"))
            .collect();
        assert_eq!(sifre, ["A-1", "B-2", "C-3"]);
    }

    #[test]
    fn the_hash_is_sixty_four_hex_characters_and_stable_for_the_same_body() {
        let body = render_csv(&[row_with_unit_price()]);
        let hash = content_hash(&body);

        assert_eq!(hash.len(), 64, "{hash}");
        assert!(
            hash.chars()
                .all(|ch| ch.is_ascii_hexdigit() && !ch.is_uppercase()),
            "{hash}"
        );
        assert_eq!(hash, content_hash(&body));
    }

    /// The BOM exists for this: the naziv reaches the file with its diacritics
    /// intact, and „ … “ are the Serbian quotes, not ASCII ones.
    #[test]
    fn serbian_latin_survives_the_render() {
        let row = CenovnikRow {
            naziv: "Čarape žute, Đorđević".to_string(),
            ..row_priced(100)
        };
        assert!(
            render_csv(&[row]).contains("Čarape žute, Đorđević"),
            "the diacritics must reach the file verbatim"
        );
    }

    /// Spelled as the literal CRLF rather than through `LINE_ENDING` on purpose:
    /// a test that reads the constant it is guarding is a tautology and passes
    /// for any line ending at all.
    #[test]
    fn every_row_ends_with_crlf() {
        let csv = render_csv(&[row_priced(100)]);
        assert_eq!(csv.matches("\r\n").count(), 2, "{csv:?}");
        assert!(csv.ends_with("\r\n"), "{csv:?}");
        assert!(
            !csv.replace("\r\n", "").contains(['\r', '\n']),
            "no bare CR or LF may survive outside a CRLF pair: {csv:?}"
        );
    }
}
