//! Reklamacije documents — potvrda o prijemu + prodajno-mesto notice.
//!
//! Legal authority: `docs/ZZP-REKLAMACIJE-VERIFIED-RULES.md`. Design:
//! `docs/superpowers/specs/2026-07-19-reklamacije-design.md`.
//!
//! Pure renderers over a `ReklamacijaView`: self-contained HTML in the SW-6c
//! shape (`<!doctype html>`, inline `<style>`, every dynamic value HTML-escaped,
//! no `<script>`, no QR/PIB/brojač, a footer that marks the page non-fiscal).
//! The potvrda carries the register number prominently and NO due-date — memo
//! §3's „bez odlaganja" is not a countdown. Privacy: the potvrda carries only
//! this one complaint's data.

// The renderers are consumed by the export commands added in a later SW-7 task;
// keep the staged API green here.
#![allow(dead_code)]

use crate::reklamacije::ReklamacijaView;

/// Escapes the five HTML-significant characters so dynamic values (a filer name
/// may contain `<`, `&`, `"`) can never break out of the document.
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

/// The date portion (`YYYY-MM-DD`) of an RFC3339 timestamp for human display.
fn date_only(rfc3339: &str) -> &str {
    rfc3339.get(..10).unwrap_or(rfc3339)
}

/// Renders the potvrda o prijemu reklamacije: a self-contained HTML confirmation
/// carrying the register number prominently, the filing/issue dates, and the
/// filer/goods/complaint/remedy — every dynamic value escaped. No due-date
/// („bez odlaganja" is not a timer, memo §3), no `<script>`, no QR/PIB/brojač;
/// a footer marks it non-fiscal. Privacy: only this complaint's own data.
pub fn render_potvrda_html(view: &ReklamacijaView) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"sr-Latn\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<title>Potvrda o prijemu reklamacije</title>\n");
    html.push_str(
        "<style>\n\
         @page { margin: 1cm }\n\
         body { font-family: sans-serif; color: #111; margin: 1cm; }\n\
         h1 { font-size: 1.4rem; }\n\
         .register { font-size: 1.6rem; font-weight: bold; margin: 0.75rem 0; }\n\
         .meta { color: #444; }\n\
         table { border-collapse: collapse; margin: 0.5rem 0; }\n\
         th, td { border: 1px solid #999; padding: 0.25rem 0.5rem; text-align: left; vertical-align: top; }\n\
         th { white-space: nowrap; }\n\
         footer { margin-top: 2rem; color: #666; font-size: 0.85rem; }\n\
         </style>\n</head>\n<body>\n",
    );

    html.push_str("<h1>Potvrda o prijemu reklamacije</h1>\n");
    html.push_str(
        "<p class=\"meta\">Trgovac je dužan da potrošaču bez odlaganja izda potvrdu o prijemu \
         reklamacije, odnosno saopšti broj pod kojim je reklamacija zavedena u evidenciji \
         primljenih reklamacija (čl. 55 st. 7 / čl. 63 st. 7).</p>\n",
    );

    // Register number — carried prominently; it is the whole point of the potvrda.
    html.push_str(&format!(
        "<div class=\"register\">Broj reklamacije: {}</div>\n",
        view.register_number
    ));

    let kontakt = view.kontakt.as_deref().unwrap_or("—");
    html.push_str("<table>\n");
    html.push_str(&format!(
        "<tr><th>Datum prijema reklamacije</th><td>{}</td></tr>\n",
        escape_html(date_only(&view.filed_at))
    ));
    html.push_str(&format!(
        "<tr><th>Datum izdavanja potvrde</th><td>{}</td></tr>\n",
        escape_html(date_only(&view.datum_izdavanja_potvrde))
    ));
    html.push_str(&format!(
        "<tr><th>Podnosilac reklamacije</th><td>{}</td></tr>\n",
        escape_html(&view.podnosilac_ime_prezime)
    ));
    html.push_str(&format!(
        "<tr><th>Kontakt</th><td>{}</td></tr>\n",
        escape_html(kontakt)
    ));
    html.push_str(&format!(
        "<tr><th>Podaci o robi</th><td>{}</td></tr>\n",
        escape_html(&view.podaci_o_robi)
    ));
    html.push_str(&format!(
        "<tr><th>Opis nesaobraznosti</th><td>{}</td></tr>\n",
        escape_html(&view.opis_nesaobraznosti)
    ));
    html.push_str(&format!(
        "<tr><th>Zahtev potrošača</th><td>{}</td></tr>\n",
        escape_html(&view.zahtev)
    ));
    html.push_str("</table>\n");

    // NEW regime only — gated on the Option, never on a regime test of this
    // renderer's own. An old-regime potvrda must not promise a right that
    // 88/2021 čl. 55 st. 3 does not give.
    if let Some(no_fee) = view.no_fee_notice.as_deref() {
        html.push_str(&format!("<p class=\"meta\">{}</p>\n", escape_html(no_fee)));
    }

    html.push_str(
        "<footer>Ova potvrda nije fiskalni dokument.</footer>\n\
         </body>\n</html>\n",
    );
    html
}

/// Renders the „Obaveštenje o načinu i mestu prijema reklamacija" — the
/// statutory display notice for the prodajno mesto (memo §4(b), čl. 55 st. 4 /
/// čl. 63 st. 4). Static Serbian text, no dynamic values; same non-fiscal,
/// no-`<script>`, no-QR/PIB rules as the other documents.
pub fn render_notice_html() -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"sr-Latn\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<title>Obaveštenje o načinu i mestu prijema reklamacija</title>\n");
    html.push_str(
        "<style>\n\
         @page { margin: 1cm }\n\
         body { font-family: sans-serif; color: #111; margin: 1cm; }\n\
         h1 { font-size: 1.4rem; }\n\
         p { margin: 0.5rem 0; }\n\
         footer { margin-top: 2rem; color: #666; font-size: 0.85rem; }\n\
         </style>\n</head>\n<body>\n",
    );

    html.push_str("<h1>Obaveštenje o načinu i mestu prijema reklamacija</h1>\n");
    html.push_str(
        "<p>Reklamacije se primaju na prodajnom mestu, u toku radnog vremena trgovca.</p>\n",
    );
    html.push_str(
        "<p>Potrošač reklamaciju može izjaviti usmeno, pisanim putem ili elektronskim putem.</p>\n",
    );
    html.push_str(
        "<p>Uz reklamaciju potrošač dostavlja robu i račun ili drugi dokaz o kupovini (izvod, \
         slip i sl.).</p>\n",
    );
    html.push_str(
        "<p>Trgovac bez odlaganja izdaje pisanu ili elektronsku potvrdu o prijemu reklamacije, \
         sa brojem pod kojim je reklamacija zavedena u evidenciji primljenih reklamacija.</p>\n",
    );
    html.push_str(
        "<p>Trgovac odgovara potrošaču pisanim ili elektronskim putem najkasnije u roku od 8 dana \
         od dana prijema reklamacije.</p>\n",
    );
    html.push_str(
        "<p>Zabranjeno je naplatiti utvrđivanje nesaobraznosti (čl. 63 st. 3). Otklanjanje \
         nesaobraznosti — popravka ili zamena — je bez naknade (čl. 56 st. 1).</p>\n",
    );
    html.push_str(
        "<p>Nemogućnost dostavljanja ambalaže ne može biti uslov za rešavanje reklamacije.</p>\n",
    );
    html.push_str(
        "<p>U toku radnog vremena obezbeđeno je prisustvo lica ovlašćenog za prijem reklamacija.</p>\n",
    );

    html.push_str(
        "<footer>Obaveštenje istaknuto u skladu sa čl. 55 st. 4 / čl. 63 st. 4. Nije fiskalni \
         dokument.</footer>\n\
         </body>\n</html>\n",
    );
    html
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{remove_test_database, test_database_path, Db};
    use crate::reklamacije::{create_reklamacija, get_reklamacija, ReklamacijaInput};
    use rusqlite::Connection;

    fn with_reklamacija_db(test_name: &str, test: impl FnOnce(&mut Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("db init");
            let mut connection = db.open().expect("open");
            test(&mut connection);
        }
        remove_test_database(&path);
    }

    fn intake_input(filed_at: &str, filer: &str) -> ReklamacijaInput {
        ReklamacijaInput {
            podnosilac_ime_prezime: filer.into(),
            kontakt: Some("060/123-456".into()),
            podaci_o_robi: "Frižider Beko".into(),
            opis_nesaobraznosti: "Ne hladi".into(),
            zahtev: "Zamena".into(),
            roba_kind: "tehnicka".into(),
            filed_at: filed_at.into(),
        }
    }

    #[test]
    fn potvrda_carries_the_fee_ban_only_for_a_new_regime_record() {
        with_reklamacija_db("docs_potvrda_no_fee", |conn| {
            let new = create_reklamacija(
                conn,
                &intake_input("2026-09-01T00:00:00Z", "Petar Petrović"),
                1,
                "2026-09-01T08:00:00Z",
            )
            .unwrap();
            let html = render_potvrda_html(&new);
            assert!(
                html.contains("Zabranjeno je naplatiti utvrđivanje nesaobraznosti"),
                "a new-regime potvrda must carry čl. 63 st. 3"
            );

            // A pre-cutover complaint is governed by 88/2021 čl. 55 st. 3, which
            // has no fee ban. Printing one on its potvrda tells the consumer they
            // have a right the law does not give them for this complaint.
            let old = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z", "Marija Marić"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            let html = render_potvrda_html(&old);
            assert!(
                !html.contains("Zabranjeno je naplatiti"),
                "an old-regime potvrda must not assert the fee ban"
            );
        });
    }

    #[test]
    fn prodajno_mesto_notice_always_states_the_fee_ban() {
        // The display notice is static and forward-looking — every complaint
        // lodged from today on is new-regime — so it states current law.
        let html = render_notice_html();
        assert!(html.contains("Zabranjeno je naplatiti utvrđivanje nesaobraznosti"));
        assert!(html.contains("čl. 63 st. 3"));
    }

    #[test]
    fn potvrda_carries_register_number_escaped_filer_and_non_fiscal_line() {
        with_reklamacija_db("docs_potvrda", |conn| {
            let view = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z", "Petar & <b>Petrović</b>"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            let html = render_potvrda_html(&view);

            assert!(html.starts_with("<!doctype html>"));
            assert!(html.contains("Potvrda o prijemu reklamacije"));
            // Register number, prominently present.
            assert!(
                html.contains(&view.register_number.to_string()),
                "the register number must appear on the potvrda"
            );
            // Filer name escaped — never the raw markup.
            assert!(html.contains("Petar &amp; &lt;b&gt;Petrović&lt;/b&gt;"));
            assert!(!html.contains("Petar & <b>Petrović</b>"));
            // Non-fiscal, no script.
            assert!(html.contains("Ova potvrda nije fiskalni dokument."));
            assert!(!html.contains("<script"));
            // No due-date/timer: „bez odlaganja" is not a countdown (memo §3).
            // filed 2026-06-01 tehnička → answer_due 06-09, base resolution 07-01.
            assert!(
                !html.contains("2026-06-09"),
                "the potvrda must not show the answer due-date"
            );
            assert!(
                !html.contains("2026-07-01"),
                "the potvrda must not show the resolution due-date"
            );
        });
    }

    // Privacy (memo §6): the potvrda carries ONLY this complaint's own data —
    // never another complainant filed in the same register.
    #[test]
    fn potvrda_contains_only_this_complaint() {
        with_reklamacija_db("docs_potvrda_privacy", |conn| {
            let first = create_reklamacija(
                conn,
                &intake_input("2026-06-01T00:00:00Z", "Ana Anić"),
                1,
                "2026-06-01T08:00:00Z",
            )
            .unwrap();
            create_reklamacija(
                conn,
                &intake_input("2026-06-02T00:00:00Z", "TAJNI-PODNOSILAC"),
                1,
                "2026-06-02T08:00:00Z",
            )
            .unwrap();
            // Re-read the first so its view is unrelated to the second's insert.
            let view = get_reklamacija(conn, first.id, "2026-06-05T00:00:00Z").unwrap();
            let html = render_potvrda_html(&view);
            assert!(
                html.contains("Ana Anić"),
                "the complaint's own filer appears"
            );
            assert!(
                !html.contains("TAJNI-PODNOSILAC"),
                "another complainant must not leak into this potvrda"
            );
        });
    }

    #[test]
    fn potvrda_escapes_all_dynamic_values() {
        with_reklamacija_db("docs_potvrda_escape", |conn| {
            let mut input = intake_input("2026-06-01T00:00:00Z", "Petar Petrović");
            input.podaci_o_robi = "Roba <x> & \"y\"".into();
            input.opis_nesaobraznosti = "Kvar <script>".into();
            input.zahtev = "Zamena & popravka".into();
            let view = create_reklamacija(conn, &input, 1, "2026-06-01T08:00:00Z").unwrap();
            let html = render_potvrda_html(&view);
            assert!(html.contains("Roba &lt;x&gt; &amp; &quot;y&quot;"));
            assert!(html.contains("Kvar &lt;script&gt;"));
            assert!(html.contains("Zamena &amp; popravka"));
            assert!(!html.contains("<script"));
        });
    }

    #[test]
    fn notice_is_the_statutory_display_notice() {
        let html = render_notice_html();
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("Obaveštenje o načinu i mestu prijema reklamacija"));
        assert!(!html.contains("<script"));
        assert!(html.contains("Nije fiskalni dokument."));
    }
}
