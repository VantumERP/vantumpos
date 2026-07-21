//! Year-end zaključivanje (SW-9c) — the čl. 18 data-layer lock.
//!
//! A `kep_closures` row freezes a book year. `ensure_year_open` is the gate
//! every posting path calls first; combined with the append-only ledger
//! (no UPDATE/DELETE of a posted row anywhere), a closed year cannot be
//! altered by the application — the čl. 18 *prevention* (not detection).
//!
//! The close is irreversible: there is no `reopen`. A typed confirmation
//! (`ZAKLJUČI KNJIGU`) guards against accident; the go-live reset
//! (`reset_trading_data`) is the one sanctioned escape, for practice years.
//!
//! The carry-forward is COMPUTED, not stored: `crate::kep::list_ledger` reads
//! the prior year's `krajnji_saldo_minor` as the opening. The close therefore
//! inserts NO `kep_entries` row.
//!
//! Consumed by the gate wiring (Task 4) and the command layer, so `dead_code`
//! is allowed here — mirroring the other KEP domain modules.

#![allow(dead_code)]

use rusqlite::{params, Connection};
use serde::Serialize;

use crate::app_error::AppError;
use crate::commands::settings::CompanySettings;
use crate::kep::{KepEntryView, KepLedger};

/// The exact phrase the operator must type to close a year.
pub const CLOSE_CONFIRMATION: &str = "ZAKLJUČI KNJIGU";

/// A recorded year-end close (mirrors a `kep_closures` row for the frontend).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KepClosure {
    pub book_year: i64,
    pub krajnji_saldo_minor: i64,
    pub entry_count: i64,
    pub closed_at: String,
    pub closed_by: Option<i64>,
}

/// True when `book_year` has a closure row.
pub fn is_year_closed(conn: &Connection, book_year: i64) -> Result<bool, AppError> {
    let closed: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM kep_closures WHERE book_year = ?1)",
        params![book_year],
        |row| row.get(0),
    )?;
    Ok(closed)
}

/// The gate. Rejects a write into a closed year (čl. 18 prevention).
pub fn ensure_year_open(conn: &Connection, book_year: i64) -> Result<(), AppError> {
    if is_year_closed(conn, book_year)? {
        return Err(AppError::business(
            "year_closed",
            format!("Knjiga za {book_year}. godinu je zaključena i ne može se menjati."),
        ));
    }
    Ok(())
}

/// Closes `book_year` irreversibly. Validates the typed confirmation, rejects a
/// double-close, computes the krajnji saldo from `list_ledger` (which already
/// includes the carry-in), records the closure, and inserts NO ledger entry.
pub fn close_year(
    conn: &mut Connection,
    book_year: i64,
    confirmation: &str,
    acting: i64,
    now: &str,
) -> Result<KepClosure, AppError> {
    if confirmation != CLOSE_CONFIRMATION {
        return Err(AppError::validation(
            "Potvrda nije ispravna. Ukucajte tačno: ZAKLJUČI KNJIGU.",
            serde_json::json!({ "field": "confirmation" }),
        ));
    }

    let tx = conn.transaction()?;
    if is_year_closed(&tx, book_year)? {
        return Err(AppError::business(
            "invalid_state",
            "Godina je već zaključena.",
        ));
    }

    let ledger = crate::kep::list_ledger(&tx, book_year)?;
    let krajnji_saldo_minor = ledger.saldo_minor;
    let entry_count = ledger.entries.len() as i64;

    tx.execute(
        "INSERT INTO kep_closures (
            book_year, krajnji_saldo_minor, entry_count, closed_at, closed_by, created_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?4)",
        params![book_year, krajnji_saldo_minor, entry_count, now, acting],
    )?;
    tx.commit()?;

    Ok(KepClosure {
        book_year,
        krajnji_saldo_minor,
        entry_count,
        closed_at: now.to_string(),
        closed_by: Some(acting),
    })
}

/// Default rows per printed book page (§5.5 page mechanics).
pub const BOOK_ROWS_PER_PAGE: usize = 30;

/// Everything the signable close document shows (čl. 17 st. 4).
pub struct KepCloseView {
    pub company: CompanySettings,
    pub closure: KepClosure,
    pub opening_saldo_minor: i64,
    pub zaduzenje_total_minor: i64,
    pub razduzenje_total_minor: i64,
}

const DOC_STYLE: &str = "\
@page { margin: 1cm }\n\
body { font-family: sans-serif; color: #111; margin: 1cm; }\n\
h1 { font-size: 1.3rem; }\n\
.meta { color: #444; margin: 0.1rem 0; }\n\
table { border-collapse: collapse; width: 100%; margin: 0.75rem 0; }\n\
th, td { border: 1px solid #999; padding: 0.25rem 0.5rem; text-align: left; }\n\
td.amount, th.amount { text-align: right; white-space: nowrap; }\n\
tr.donos td, tr.svega td { font-weight: bold; background: #f0f0f0; }\n\
.page { page-break-after: always; }\n\
.page:last-child { page-break-after: auto; }\n\
.sign { margin-top: 3rem; display: flex; justify-content: space-between; }\n\
footer { margin-top: 2rem; color: #666; font-size: 0.85rem; }\n";

fn doc_head(title: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"sr-Latn\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>{}</title>\n<style>\n{}</style>\n</head>\n<body>\n",
        escape_html(title),
        DOC_STYLE
    )
}

fn company_header(company: &CompanySettings, book_year: i64) -> String {
    format!(
        "<p class=\"meta\">Trgovac: {}</p>\n\
         <p class=\"meta\">PIB: {}</p>\n\
         <p class=\"meta\">Prodajni objekat: {}</p>\n\
         <p class=\"meta\">KNJIGA EVIDENCIJE PROMETA ZA {}. GODINU</p>\n",
        escape_html(&company.shop_name),
        escape_html(&company.pib),
        escape_html(&company.address),
        book_year
    )
}

/// The signable electronic-close document (čl. 17 st. 4): opening carry-in, the
/// year's zaduženje/razduženje totals, the krajnji saldo, and a signature line.
pub fn render_close_html(view: &KepCloseView) -> String {
    let mut html = doc_head("Zaključenje knjige evidencije prometa");
    html.push_str(&format!(
        "<h1>Zaključenje knjige za {}</h1>\n",
        view.closure.book_year
    ));
    html.push_str(&company_header(&view.company, view.closure.book_year));

    html.push_str("<table>\n<tbody>\n");
    for (label, minor) in [
        ("Početno stanje (donos)", view.opening_saldo_minor),
        ("Ukupno zaduženje (kolona 4)", view.zaduzenje_total_minor),
        ("Ukupno razduženje (kolona 5)", view.razduzenje_total_minor),
        ("KRAJNJI SALDO", view.closure.krajnji_saldo_minor),
    ] {
        html.push_str(&format!(
            "<tr><th>{label}</th><td class=\"amount\">{} RSD</td></tr>\n",
            format_rsd_minor(minor)
        ));
    }
    html.push_str(&format!(
        "<tr><th>Broj stavki</th><td class=\"amount\">{}</td></tr>\n",
        view.closure.entry_count
    ));
    html.push_str(&format!(
        "<tr><th>Datum zaključenja</th><td>{}</td></tr>\n",
        escape_html(date_only(&view.closure.closed_at))
    ));
    html.push_str("</tbody>\n</table>\n");

    html.push_str(
        "<div class=\"sign\"><span>M.P. ______________</span>\
         <span>ODGOVORNO LICE ______________</span></div>\n",
    );
    html.push_str("<footer>Interni dokument. Nije fiskalni dokument.</footer>\n</body>\n</html>\n");
    html
}

/// The full-book paginated print (§5.5), rendered as the statutory obrazac
/// (§2.1 / worked example §2.7): the **fixed five columns** — RB, Datum, Opis,
/// Zaduženje (4), Razduženje (5), with **no saldo column** — split into pages of
/// `rows_per_page`. Every page opens with a **DONOS** (cumulative carry-in) row
/// and closes with a **SVEGA ZA PRENOS** (cumulative carry-out) row; both carry
/// the running Σ kolona-4 and Σ kolona-5 totals in their own columns (PEP čl. 10
/// st. 2). The opening carry-in (prior year's krajnji saldo, čl. 17 st. 2) is
/// the first page's DONOS kolona-4 value, with kolona 5 = 0. The **krajnji
/// saldo** is a derived line under the last page (`Σ4 − Σ5`, čl. 16 st. 2), not
/// a column. Pages are numbered `Strana k / n`.
pub fn render_book_html(
    company: &CompanySettings,
    ledger: &KepLedger,
    rows_per_page: usize,
) -> String {
    let per_page = rows_per_page.max(1);
    let mut html = doc_head("Knjiga evidencije prometa");
    html.push_str(&company_header(company, ledger.book_year));

    let mut chunks: Vec<&[KepEntryView]> = ledger.entries.chunks(per_page).collect();
    // A quiet year (no entries) still prints one page so its DONOS (opening
    // carry-in) and the derived krajnji saldo appear — the book is never blank.
    if chunks.is_empty() {
        chunks.push(&[]);
    }
    let page_total = chunks.len();

    // Cumulative kolona-4 / kolona-5 totals carried across pages. The opening
    // carry-in (prior krajnji saldo) enters as a kolona-4 zaduženje; kolona 5 = 0
    // (§2.7 DONOS = 10.000,00 / 0,00).
    let mut cum_zaduzenje = ledger.opening_saldo_minor;
    let mut cum_razduzenje = 0i64;

    for (page_index, chunk) in chunks.iter().enumerate() {
        let page_number = page_index + 1;
        html.push_str("<div class=\"page\">\n");
        html.push_str(&format!(
            "<p class=\"meta\">Strana {page_number} / {page_total}</p>\n"
        ));
        html.push_str(
            "<table>\n<thead>\n<tr>\
             <th>RB</th><th>Datum</th><th>Opis</th>\
             <th class=\"amount\">Zaduženje (4)</th>\
             <th class=\"amount\">Razduženje (5)</th></tr>\n</thead>\n<tbody>\n",
        );

        // DONOS — the cumulative carry-in for this page (first page = opening
        // stock as kolona-4 zaduženje, kolona 5 = 0).
        html.push_str(&format!(
            "<tr class=\"donos\"><td></td><td></td><td>DONOS</td>\
             <td class=\"amount\">{}</td><td class=\"amount\">{}</td></tr>\n",
            format_rsd_minor(cum_zaduzenje),
            format_rsd_minor(cum_razduzenje)
        ));

        for entry in chunk.iter() {
            cum_zaduzenje += entry.zaduzenje_minor.unwrap_or(0);
            cum_razduzenje += entry.razduzenje_minor.unwrap_or(0);
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td>\
                 <td class=\"amount\">{}</td><td class=\"amount\">{}</td></tr>\n",
                entry.redni_broj,
                escape_html(&entry.datum),
                escape_html(&entry.opis),
                entry
                    .zaduzenje_minor
                    .map(format_rsd_minor)
                    .unwrap_or_default(),
                entry
                    .razduzenje_minor
                    .map(format_rsd_minor)
                    .unwrap_or_default(),
            ));
        }

        // SVEGA ZA PRENOS — the cumulative carry-out (= the next page's DONOS).
        html.push_str(&format!(
            "<tr class=\"svega\"><td></td><td></td><td>SVEGA ZA PRENOS</td>\
             <td class=\"amount\">{}</td><td class=\"amount\">{}</td></tr>\n",
            format_rsd_minor(cum_zaduzenje),
            format_rsd_minor(cum_razduzenje)
        ));

        html.push_str("</tbody>\n</table>\n");

        // The krajnji saldo is a DERIVED line under the last page (saldiranje
        // kolona 4 i 5, čl. 16 st. 2) — never a sixth column.
        if page_number == page_total {
            html.push_str(&format!(
                "<p class=\"meta\"><strong>KRAJNJI SALDO (kolona 4 − kolona 5): \
                 {} − {} = {} RSD</strong></p>\n",
                format_rsd_minor(cum_zaduzenje),
                format_rsd_minor(cum_razduzenje),
                format_rsd_minor(cum_zaduzenje - cum_razduzenje)
            ));
        }

        html.push_str("</div>\n");
    }

    html.push_str("<footer>Interni dokument. Nije fiskalni dokument.</footer>\n</body>\n</html>\n");
    html
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

/// Formats integer minor units as grouped RSD: `900000 → "9.000,00"`.
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

/// `2027-01-05T09:00:00Z` → `2027-01-05` for display.
fn date_only(rfc3339: &str) -> &str {
    rfc3339.split('T').next().unwrap_or(rfc3339)
}

/// One closure as the frontend list shows it, with the retention flag.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KepClosureView {
    pub book_year: i64,
    pub krajnji_saldo_minor: i64,
    pub entry_count: i64,
    pub closed_at: String,
    pub purge_eligible: bool,
    /// The `YYYY-MM-DD` date the retention obligation ends (the floor below).
    pub retention_until: String,
}

/// The retention floor (§5.6): the earliest `YYYY-MM-DD` on which a closed book
/// may be discarded — the LATER of `31 Dec of book_year + 5` and
/// `closed_at + 5 years`. Panic-safe on a malformed `closed_at` (falls back to
/// the year-end floor rather than slicing a short string).
pub fn retention_floor(book_year: i64, closed_at: &str) -> String {
    let year_end_floor = format!("{}-12-31", book_year + 5);
    let day = date_only(closed_at);
    // day is expected as YYYY-MM-DD; shift the year by 5, keep the -MM-DD tail.
    let closed_floor = match (
        day.get(0..4).and_then(|y| y.parse::<i64>().ok()),
        day.get(4..),
    ) {
        (Some(year), Some(rest)) => format!("{}{}", year + 5, rest),
        _ => year_end_floor.clone(),
    };
    if closed_floor >= year_end_floor {
        closed_floor
    } else {
        year_end_floor
    }
}

/// Retention (§5.6): a closed book may be discarded only once `today` reaches
/// the [`retention_floor`]. Dates compare lexicographically on the ISO
/// `YYYY-MM-DD` prefix, which is correct for ISO dates. `today` on/after the
/// floor ⇒ eligible.
pub fn purge_eligible(book_year: i64, closed_at: &str, today: &str) -> bool {
    date_only(today) >= retention_floor(book_year, closed_at).as_str()
}

/// Lists recorded closures newest-first, each with its retention flag.
pub fn list_closures(conn: &Connection, today: &str) -> Result<Vec<KepClosureView>, AppError> {
    let mut stmt = conn.prepare(
        "SELECT book_year, krajnji_saldo_minor, entry_count, closed_at
         FROM kep_closures ORDER BY book_year DESC",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (book_year, krajnji_saldo_minor, entry_count, closed_at) = row?;
        let purge_eligible = purge_eligible(book_year, &closed_at, today);
        let retention_until = retention_floor(book_year, &closed_at);
        out.push(KepClosureView {
            book_year,
            krajnji_saldo_minor,
            entry_count,
            closed_at,
            purge_eligible,
            retention_until,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::settings::CompanySettings;
    use crate::db::{test_database_path, Db};
    use crate::kep::{KepEntryView, KepLedger};

    /// Opens a migrated connection over a throwaway database.
    fn with_conn(test_name: &str, test: impl FnOnce(&mut Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("database should initialize");
            let mut conn = db.open().expect("database should open");
            test(&mut conn);
        }
        std::fs::remove_file(&path).expect("test database should be removed");
    }

    /// Seeds a receipt zaduženje so the closed saldo is a real figure.
    fn seed_zaduzenje(conn: &Connection, book_year: i64, redni_broj: i64, amount_minor: i64) {
        conn.execute(
            "INSERT INTO kep_entries
                (book_year, redni_broj, entry_date, opis, kolona, amount_minor, kind, entry_source, user_id, created_at)
             VALUES (?1, ?2, '2026-06-01T00:00:00Z', 'Prijem robe', 'zaduzenje', ?3, 'receipt', 'auto', 1, '2026-06-01T00:00:00Z')",
            params![book_year, redni_broj, amount_minor],
        )
        .expect("seed zaduženje");
    }

    #[test]
    fn close_year_records_saldo_and_count() {
        with_conn("close_year_records", |conn| {
            seed_zaduzenje(conn, 2026, 1, 780000);
            seed_zaduzenje(conn, 2026, 2, 120000);

            let closure = close_year(conn, 2026, CLOSE_CONFIRMATION, 1, "2027-01-05T09:00:00Z")
                .expect("close should succeed");

            assert_eq!(closure.krajnji_saldo_minor, 900000, "780000 + 120000");
            assert_eq!(closure.entry_count, 2);
            assert!(is_year_closed(conn, 2026).expect("closed check"));

            // The close inserts NO kep_entries row.
            let entry_count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM kep_entries WHERE book_year = 2026",
                    [],
                    |r| r.get(0),
                )
                .expect("count");
            assert_eq!(entry_count, 2, "no entry inserted by the close");
        });
    }

    #[test]
    fn close_year_rejects_wrong_confirmation() {
        with_conn("close_year_wrong_confirmation", |conn| {
            let err = close_year(conn, 2026, "zakljuci", 1, "2027-01-05T09:00:00Z")
                .expect_err("wrong confirmation must reject");
            assert_eq!(err.code(), "validation_error");
            assert!(!is_year_closed(conn, 2026).expect("still open"));
        });
    }

    #[test]
    fn close_year_rejects_double_close() {
        with_conn("close_year_double", |conn| {
            close_year(conn, 2026, CLOSE_CONFIRMATION, 1, "2027-01-05T09:00:00Z")
                .expect("first close");
            let err = close_year(conn, 2026, CLOSE_CONFIRMATION, 1, "2027-01-06T09:00:00Z")
                .expect_err("second close must reject");
            assert_eq!(err.code(), "invalid_state");
        });
    }

    #[test]
    fn ensure_year_open_gates_a_closed_year() {
        with_conn("ensure_year_open_gate", |conn| {
            ensure_year_open(conn, 2026).expect("open year passes");
            close_year(conn, 2026, CLOSE_CONFIRMATION, 1, "2027-01-05T09:00:00Z").expect("close");
            let err = ensure_year_open(conn, 2026).expect_err("closed year rejects");
            assert_eq!(err.code(), "year_closed");
            ensure_year_open(conn, 2027).expect("a later open year still passes");
        });
    }

    fn company() -> CompanySettings {
        CompanySettings {
            shop_name: "STR Delta".to_string(),
            address: "Kralja Petra 1, Novi Sad".to_string(),
            pib: "123456789".to_string(),
            ..CompanySettings::default()
        }
    }

    #[test]
    fn close_html_carries_krajnji_saldo_and_signature() {
        let view = KepCloseView {
            company: company(),
            closure: KepClosure {
                book_year: 2026,
                krajnji_saldo_minor: 900000,
                entry_count: 2,
                closed_at: "2027-01-05T09:00:00Z".to_string(),
                closed_by: Some(1),
            },
            opening_saldo_minor: 0,
            zaduzenje_total_minor: 900000,
            razduzenje_total_minor: 0,
        };
        let html = render_close_html(&view);
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("Zaključenje knjige za 2026"));
        assert!(
            html.contains("<th>KRAJNJI SALDO</th><td class=\"amount\">9.000,00 RSD</td>"),
            "the KRAJNJI SALDO row carries the 900000 minor krajnji saldo"
        );
        assert!(html.contains("ODGOVORNO LICE"), "signature line");
        assert!(html.contains("Nije fiskalni dokument"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn book_html_paginates_with_donos_and_svega() {
        // 65 entries at rows_per_page = 30 → 3 pages (30 / 30 / 5).
        let mut entries = Vec::new();
        for i in 1..=65 {
            entries.push(KepEntryView {
                redni_broj: i,
                datum: "01.06".to_string(),
                opis: format!("Stavka {i}"),
                zaduzenje_minor: Some(10000),
                razduzenje_minor: None,
                kind: "receipt".to_string(),
            });
        }
        let ledger = KepLedger {
            book_year: 2026,
            entries,
            opening_saldo_minor: 50000,
            saldo_minor: 700000,
        };
        let html = render_book_html(&company(), &ledger, 30);
        assert!(html.contains("Strana 1 / 3"));
        assert!(html.contains("Strana 3 / 3"));
        assert!(!html.contains("Strana 4"), "65 rows @ 30 → exactly 3 pages");
        // The obrazac is fixed at five columns — no saldo column (§2.1).
        assert!(
            !html.contains("<th class=\"amount\">Saldo</th>"),
            "the printed KEP has no saldo column"
        );
        assert!(html.contains("page-break-after"));
        // First DONOS carries the opening as a kolona-4 zaduženje, kolona 5 = 0
        // (§2.7 DONOS = opening / 0,00).
        assert!(
            html.contains(
                "<td>DONOS</td><td class=\"amount\">500,00</td>\
                 <td class=\"amount\">0,00</td>"
            ),
            "first DONOS = opening carry-in in kolona 4"
        );
        // Final SVEGA ZA PRENOS carries the cumulative kolona-4 total
        // (50000 opening + 65×10000 = 700000) and kolona-5 total (0).
        assert!(
            html.contains(
                "<td>SVEGA ZA PRENOS</td><td class=\"amount\">7.000,00</td>\
                 <td class=\"amount\">0,00</td>"
            ),
            "final carry-out = cumulative Σ kolona 4 / Σ kolona 5"
        );
        // Krajnji saldo is a DERIVED line (Σ4 − Σ5), not a column.
        assert!(
            html.contains("7.000,00 − 0,00 = 7.000,00 RSD"),
            "krajnji saldo derived under the last page"
        );
        assert!(!html.contains("<script"));
    }

    #[test]
    fn book_html_renders_one_page_for_empty_ledger() {
        // A year that carried an opening balance but booked nothing still prints
        // one page with its DONOS (opening) and a derived KRAJNJI SALDO.
        let ledger = KepLedger {
            book_year: 2027,
            entries: Vec::new(),
            opening_saldo_minor: 900000,
            saldo_minor: 900000,
        };
        let html = render_book_html(&company(), &ledger, 30);
        assert!(
            html.contains("Strana 1 / 1"),
            "a quiet year still prints one page"
        );
        assert!(
            html.contains(
                "<td>DONOS</td><td class=\"amount\">9.000,00</td>\
                 <td class=\"amount\">0,00</td>"
            ),
            "DONOS carries the opening carry-in in kolona 4"
        );
        assert!(
            html.contains("9.000,00 − 0,00 = 9.000,00 RSD"),
            "krajnji saldo of a quiet year = opening, derived as Σ4 − Σ5"
        );
    }

    #[test]
    fn purge_eligible_uses_the_later_floor() {
        // 2026 book year, closed early 2027-01-05.
        // 31 Dec 2026 + 5y = 2031-12-31; closed_at + 5y = 2032-01-05 (the later).
        assert!(!purge_eligible(
            2026,
            "2027-01-05T09:00:00Z",
            "2031-12-31T00:00:00Z"
        ));
        assert!(!purge_eligible(
            2026,
            "2027-01-05T09:00:00Z",
            "2032-01-04T00:00:00Z"
        ));
        assert!(purge_eligible(
            2026,
            "2027-01-05T09:00:00Z",
            "2032-01-05T00:00:00Z"
        ));

        // A close done ON 31 Dec 2026 → both floors 2031-12-31; eligible from then.
        assert!(!purge_eligible(
            2026,
            "2026-12-31T23:00:00Z",
            "2031-12-30T00:00:00Z"
        ));
        assert!(purge_eligible(
            2026,
            "2026-12-31T23:00:00Z",
            "2031-12-31T00:00:00Z"
        ));
    }

    #[test]
    fn retention_floor_takes_the_later_anchor() {
        // closed_at + 5y (2032-01-05) is later than 31 Dec 2026 + 5y (2031-12-31).
        assert_eq!(retention_floor(2026, "2027-01-05T09:00:00Z"), "2032-01-05");
        // A close ON 31 Dec 2026: both anchors coincide at 2031-12-31.
        assert_eq!(retention_floor(2026, "2026-12-31T23:00:00Z"), "2031-12-31");
        // A malformed closed_at falls back to the year-end floor — never panics.
        assert_eq!(retention_floor(2026, "bad"), "2031-12-31");
        assert_eq!(retention_floor(2026, ""), "2031-12-31");
    }

    #[test]
    fn list_closures_reports_retention() {
        with_conn("list_closures_retention", |conn| {
            seed_zaduzenje(conn, 2026, 1, 900000);
            close_year(conn, 2026, CLOSE_CONFIRMATION, 1, "2027-01-05T09:00:00Z")
                .expect("close 2026");

            let recent = list_closures(conn, "2028-06-01T00:00:00Z").expect("list");
            assert_eq!(recent.len(), 1);
            assert_eq!(recent[0].book_year, 2026);
            assert_eq!(recent[0].krajnji_saldo_minor, 900000);
            assert!(!recent[0].purge_eligible, "well within 5 years");
            assert_eq!(
                recent[0].retention_until, "2032-01-05",
                "the later of closed_at+5y and 31-Dec-2026+5y"
            );

            let aged = list_closures(conn, "2032-02-01T00:00:00Z").expect("list");
            assert!(aged[0].purge_eligible, "past the 5-year floor");
        });
    }
}
