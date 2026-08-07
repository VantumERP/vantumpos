//! The printed popis documents (reqs. 31, 32, 35) — pure renderers over a
//! [`PopisSessionView`]: the two popisne liste, the odluka o popisu i obrazovanju
//! komisije and the plan rada.
//!
//! Legal authority: `docs/REMAINING-SW-VERIFIED-RULES.md` §2c and §3 V5. PoP
//! čl. 9 st. 3 says the obračun may be done on a computer *„уз штампање пописних
//! листа које потписују чланови комисије за попис, односно једно лице из члана 6.
//! овог правилника“*, so the paper is the compliant path and not a convenience —
//! req. 31. The limb after the *односно* is quoted here because it is quoted on
//! the sheet: a preduzetnik's popis may lawfully be taken by one person, and the
//! document must not state the rule in a form that leaves them out.
//!
//! **The phase is a parameter, not an inference.** PoP čl. 8 st. 5 forbids giving
//! the commission the book quantities before the counted state is written into the
//! liste and signed, and handing somebody a sheet IS giving it to them. So the
//! Faza A document is built by a code path that never names
//! `knjigovodstvena_kolicina_milli` or `razlika_milli` at all — the same two
//! statements rather than one-and-a-filter idiom `commands::popis::read_lines`
//! uses. The renderer must stay correct when the view it is handed is *not* blind:
//! a query layer that regressed, a cached view, a future caller that loads with
//! the release already granted. Withholding by trusting the input would make the
//! čl. 8 st. 5 property a property of somebody else's SELECT.
//!
//! **No obrazac exists for the popisna lista** (§2c ii): Pravilnik 89/2020 runs
//! čl. 1–16 with no prilog and no column list, so this layout is ours and the
//! document says so rather than dressing itself up as a prescribed form. The same
//! answer covers the odluka and the plan rada — the pravilnik has no prilog at all
//! — and the *izveštaj* is the opposite case, čl. 13 st. 1 prescribing its content,
//! built elsewhere in `commands::popis::compose_izvestaj`.
//!
//! **The pre-count pair states what is recorded and never what would look
//! finished.** Čl. 8 st. 2 requires the plan rada to be approved by the lice iz
//! čl. 4 st. 2, and both documents report that approval out of the v21 columns:
//! where none is recorded they say so, and where only half of one is recorded they
//! still say so. Nothing here supplies an approver, a date of issue or a schedule
//! of its own — the obveznik's registered name is on file precisely so that it
//! cannot be mistaken for the person who approved anything — and an approval
//! recorded over a schedule the application never received is printed as exactly
//! that, since čl. 8 st. 1 puts the plan rada in the komisija's hands and a shop
//! may lawfully keep it on paper.
//!
//! **Neither document decides who takes the popis.** [`sastav_popisa`] reads the
//! roster into three states and not two: an empty roster is the ordinary state of
//! the odluka, which *is* the appointment, so resolving that silence into
//! „komisija“ would tell a preduzetnik on his own appointment paper that the
//! popis belongs to a body he does not have.
//!
//! Rendering only. Nothing here reads the database, writes a row, or promises that
//! anything is sent anywhere. What puts a document on disk is
//! `commands::popis::popis_export_lista`, `popis_export_odluka` and
//! `popis_export_plan_rada`, and the first of those — not this module — decides
//! which of the two sheets a given popis may be printed as.
//!
//! Unlike `kep_close.rs` and `reklamacije_docs.rs` this module carries no
//! `#![allow(dead_code)]`: since the export command landed, every item here is on
//! a path that starts at a registered `#[tauri::command]`, so the compiler is left
//! free to say when one stops being.

use serde::{Deserialize, Serialize};

use crate::commands::popis::{KomisijaClanView, PopisLineView, PopisSessionView};
use crate::commands::settings::CompanySettings;
use crate::popis::{vrednost_minor, PopisLista, MILLI};

/// Which of the two statutory documents is being printed.
///
/// The two are not layouts of one sheet: they are the two signature events of
/// req. 30, and each one is signed over a different set of facts. A caller that
/// picks the wrong one does not produce an ugly document, it produces a čl. 8
/// st. 5 breach — which is why the export command derives this from the session
/// rather than accepting it from the frontend.
///
/// It crosses the IPC boundary all the same, as `„a“` / `„b“` — but as a *request*
/// the command checks against the session's own potpis, never as an instruction it
/// carries out. See `commands::popis::faza_stampe`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrintFaza {
    /// The natural count, signed before any book quantity is released.
    A,
    /// The obračun — differences and valuation — after the čl. 8 st. 5 potpis.
    B,
}

impl PrintFaza {
    /// What the document calls itself in front of the commission.
    pub fn naziv(self) -> &'static str {
        match self {
            Self::A => "popisne liste stvarnog stanja",
            Self::B => "obračunate popisne liste",
        }
    }

    /// The provision the document is printed under.
    pub fn pravni_osnov(self) -> &'static str {
        match self {
            Self::A => "PoP čl. 8 st. 5",
            Self::B => "PoP čl. 9 st. 3",
        }
    }

    /// The stable key an export file name is built from.
    pub fn kljuc(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
        }
    }
}

// ---------------------------------------------------------------------------
// The popisna lista
// ---------------------------------------------------------------------------

/// Req. 31 / req. 32 — the printable popisne liste of one popis, one table per
/// lista that has stavke, with the čl. 8 st. 5 columns withheld structurally in
/// Faza A.
pub fn render_popisna_lista(
    company: &CompanySettings,
    view: &PopisSessionView,
    faza: PrintFaza,
) -> String {
    let mut html = doc_head(&format!("Popisne liste — {}", faza.naziv()));

    html.push_str(&format!(
        "<h1>POPISNE LISTE — {}</h1>\n",
        escape_html(&faza.naziv().to_uppercase())
    ));
    html.push_str(&zaglavlje(company, view, faza));
    html.push_str(&napomena_faze(faza));

    let mut razvrstane = 0usize;
    for lista in PopisLista::ALL {
        let linije: Vec<&PopisLineView> = view
            .linije
            .iter()
            .filter(|linija| linija.lista_vrsta == lista.as_db_str())
            .collect();
        if linije.is_empty() {
            continue;
        }
        razvrstane += linije.len();
        html.push_str(&sekcija(view, lista, &linije, faza));
    }

    // A stavka whose lista this build does not know is a stavka the v20 CHECK
    // should have refused — but it is still a line the commission counted, and a
    // signed document that quietly drops one says something untrue about the
    // stanje. It gets a section of its own rather than disappearing.
    if razvrstane < view.linije.len() {
        html.push_str(&sekcija_nerazvrstanih(view, faza));
    }

    if view.linije.is_empty() {
        html.push_str("<p class=\"prazno\">Nijedna popisna lista nema stavke.</p>\n");
    }

    html.push_str(&potpisni_blok(view, faza));
    html.push_str(
        "<footer>Zakon ne propisuje obrazac popisne liste — raspored kolona je interni \
         (Pravilnik o popisu, čl. 1–16, nema priloga sa obrascem). Interni dokument. Nije \
         fiskalni dokument.</footer>\n</body>\n</html>\n",
    );
    html
}

/// Req. 32's header block. A signed sheet whose header identifies nobody
/// identifies nothing, so the obveznik, its PIB and matični broj, the objekat, the
/// popis's own dates and its vrsta are on every printed phase.
fn zaglavlje(company: &CompanySettings, view: &PopisSessionView, faza: PrintFaza) -> String {
    let mut redovi = zaglavlje_redovi(company, view);
    redovi.push((
        "Faza",
        format!(
            "{} ({})",
            escape_html(faza.naziv()),
            escape_html(faza.pravni_osnov())
        ),
    ));
    zaglavlje_tabela(&redovi)
}

/// Who the obveznik is and which popis this is — the block every document of this
/// module opens with, so the odluka, the plan rada and both sheets identify the
/// same popis in the same words.
fn zaglavlje_redovi(
    company: &CompanySettings,
    view: &PopisSessionView,
) -> Vec<(&'static str, String)> {
    let period = match (view.period_from.as_deref(), view.period_to.as_deref()) {
        (Some(from), Some(to)) => format!("{from} — {to}"),
        (Some(from), None) => format!("od {from}"),
        (None, Some(to)) => format!("do {to}"),
        (None, None) => "—".to_string(),
    };

    vec![
        ("Obveznik", escape_html(&company.shop_name)),
        ("Adresa", tekst(Some(company.address.as_str()))),
        ("PIB", tekst(Some(company.pib.as_str()))),
        (
            "Matični broj",
            tekst(Some(company.registration_number.as_str())),
        ),
        ("Maloprodajni objekat", escape_html(&view.prodajno_mesto)),
        ("Vrsta popisa", escape_html(view.vrsta.naziv())),
        ("Datum popisa", escape_html(&view.datum_popisa)),
        ("Period popisa", escape_html(&period)),
        ("Popis br.", view.id.to_string()),
    ]
}

fn zaglavlje_tabela(redovi: &[(&str, String)]) -> String {
    let mut html = String::from("<table class=\"zaglavlje\">\n<tbody>\n");
    for (oznaka, vrednost) in redovi {
        html.push_str(&format!("<tr><th>{oznaka}</th><td>{vrednost}</td></tr>\n"));
    }
    html.push_str("</tbody>\n</table>\n");
    html
}

/// What the phase means, in the commission's own words.
///
/// The Faza A sentence deliberately does not use the word the Faza B column
/// heading uses: the čl. 8 st. 5 sheet must not carry the notion at all, not even
/// as a promise of what comes later.
///
/// **Neither quotation stops before the single person.** Čl. 9 st. 3 names its own
/// signatories as *„чланови комисије за попис, односно једно лице из члана 6. овог
/// правилника“* and the limb after the *односно* is the operative one for a
/// preduzetnik — which is the pilot's shape, not an edge case: `jedno_lice` is a
/// first-class `popis_commission.uloga` since v20. Čl. 8 st. 5 speaks only of the
/// komisija, and it reaches that person through the čl. 6 st. 2 *shodna primena*,
/// so the two are quoted the way they are written: the limb inside the čl. 9 st. 3
/// sentence, the shodna primena as a sentence of its own with its own citation.
/// Truncating either one would hand somebody a signed sheet stating the rule that
/// governs them in a form that excludes them.
fn napomena_faze(faza: PrintFaza) -> String {
    let tekst = match faza {
        PrintFaza::A => {
            "Ova popisna lista sadrži samo stvarno stanje utvrđeno brojanjem, merenjem odnosno \
             procenom. Podaci iz poslovnih knjiga o količinama ne daju se komisiji za popis pre \
             upisivanja stvarnog stanja u popisne liste i pre nego što članovi komisije za popis \
             potpišu te liste (PoP čl. 8 st. 5). Odredbe koje se odnose na komisiju za popis \
             shodno se primenjuju i na jedno lice koje popis vrši kod mikro pravnog lica i \
             preduzetnika (PoP čl. 6 st. 1 i st. 2)."
        }
        PrintFaza::B => {
            "Utvrđivanje razlika i vrednosno obračunavanje posle naturalnog popisa mogu se vršiti \
             i na računaru, uz štampanje popisnih listi koje potpisuju članovi komisije za popis, \
             odnosno jedno lice iz člana 6. ovog pravilnika (PoP čl. 9 st. 3)."
        }
    };
    format!("<p class=\"napomena\">{}</p>\n", escape_html(tekst))
}

/// One lista, headed by its own naziv and the article that requires it. The six
/// are separate lists by law (req. 36), not tabs of one sheet, so each gets its
/// own broj liste and its own signature-bearing table.
fn sekcija(
    view: &PopisSessionView,
    lista: PopisLista,
    linije: &[&PopisLineView],
    faza: PrintFaza,
) -> String {
    let mut html = String::from("<section class=\"lista\">\n");
    html.push_str(&format!(
        "<h2>{}</h2>\n<p class=\"meta\">Pravni osnov: {} · Broj liste: {} · Broj stavki: {}</p>\n",
        escape_html(lista.naziv()),
        escape_html(lista.pravni_osnov()),
        escape_html(&broj_liste(view, lista)),
        linije.len()
    ));
    html.push_str("<table>\n<thead>\n");
    html.push_str(&zaglavlje_kolona(lista, faza));
    html.push_str("</thead>\n<tbody>\n");
    for linija in linije {
        html.push_str(&match faza {
            PrintFaza::A => red_faza_a(linija, lista),
            PrintFaza::B => red_faza_b(linija),
        });
    }
    html.push_str("</tbody>\n</table>\n</section>\n");
    html
}

/// The lines whose `lista_vrsta` this build cannot decode, kept visible rather
/// than dropped. Rendered with the Faza A columns whatever the phase, because a
/// stavka nobody could classify is a stavka nobody can safely value either.
///
/// **Why the phase does not widen these columns, said deliberately.** Which of
/// three things the money column holds is a property of the lista and of nothing
/// else — [`iznos_kolona`] reads an apoen off čl. 11 st. 1, an iznos off čl. 12
/// st. 2 and otherwise the čl. 9 st. 1 t. 5 cena — so a stavka whose lista is
/// unknown has no heading its figure could be printed under truthfully, and its
/// knjigovodstvena količina would be a comparison against a lista nobody named.
/// Withholding on the čl. 9 st. 3 sheet breaches nothing; over-withholding never
/// does. But a signed sheet that quietly leaves the obračun off one stavka reads
/// as a stavka that had no razlika, so the phase is used to say what is missing
/// and what closes it, rather than being ignored.
///
/// **What the napomena may say is fixed by what the engine does, not by what would
/// be convenient.** The čl. 9 st. 3 sheet exists only after the čl. 8 st. 5 potpis,
/// and from that moment `commands::popis::save_line` refuses every move of
/// `lista_vrsta`: on `computed` through `PotpisanaStavka::pomerena_polja`, which
/// names the field „vrsta popisne liste“, and on `counted_signed`,
/// `computed_signed` and `posted` outright. Telling the operator to reclassify the
/// stavka would therefore instruct an action refused 100 % of the time the sentence
/// is printed. The sheet states the engine's own remedy instead, in the engine's
/// own words — novi popis.
fn sekcija_nerazvrstanih(view: &PopisSessionView, faza: PrintFaza) -> String {
    let linije: Vec<&PopisLineView> = view
        .linije
        .iter()
        .filter(|linija| PopisLista::from_db_str(&linija.lista_vrsta).is_none())
        .collect();

    let mut html = String::from("<section class=\"lista\">\n");
    html.push_str(&format!(
        "<h2>Stavke koje nisu razvrstane ni u jednu popisnu listu</h2>\n\
         <p class=\"meta\">Broj stavki: {}</p>\n",
        linije.len()
    ));
    if faza == PrintFaza::B {
        html.push_str(&format!(
            "<p class=\"napomena\">{}</p>\n",
            escape_html(
                "Za ove stavke nije iskazan obračun jer se ne zna kojoj popisnoj listi \
                 pripadaju. Posle potpisa stvarnog stanja potpisana stavka se više ne menja, pa \
                 ni popisna lista kojoj pripada (PoP čl. 8 st. 5, čl. 9 st. 1 t. 1) — ispravka \
                 se sprovodi novim popisom."
            )
        ));
    }
    html.push_str("<table>\n<thead>\n");
    html.push_str(&zaglavlje_kolona(PopisLista::Roba, PrintFaza::A));
    html.push_str("</thead>\n<tbody>\n");
    for linija in linije {
        html.push_str(&red_faza_a(linija, PopisLista::Roba));
    }
    html.push_str("</tbody>\n</table>\n</section>\n");
    html
}

/// The column set, chosen by phase and lista and by nothing else.
fn zaglavlje_kolona(lista: PopisLista, faza: PrintFaza) -> String {
    let mut html = String::from(
        "<tr><th>Šifra</th><th>Naziv</th><th>Vrsta</th><th>Jedinica mere</th>\
         <th class=\"amount\">Stvarna količina</th>",
    );
    match faza {
        PrintFaza::A => {
            if iznos_prebrojan(lista) {
                html.push_str(&format!(
                    "<th class=\"amount\">{}</th>",
                    iznos_kolona(lista)
                ));
            }
        }
        PrintFaza::B => {
            html.push_str(
                "<th class=\"amount\">Knjigovodstvena količina</th>\
                 <th class=\"amount\">Razlika (višak/manjak)</th>",
            );
            html.push_str(&format!(
                "<th class=\"amount\">{}</th>\
                 <th class=\"amount\">Vrednost po popisu</th>\
                 <th class=\"amount\">Vrednost po knjigama</th>\
                 <th class=\"amount\">Vrednosna razlika</th>",
                iznos_kolona(lista)
            ));
        }
    }
    html.push_str("<th>Bliži opis</th></tr>\n");
    html
}

/// One čl. 8 st. 5 row.
///
/// **This function must never name a Phase B field**, and a test asserts its own
/// source to that effect. Selecting everything and blanking it here would look the
/// same from the outside and be a different thing: the value would exist, one
/// refactor or one debug print away from the paper the commission is handed.
fn red_faza_a(linija: &PopisLineView, lista: PopisLista) -> String {
    let mut red = String::from("<tr>");
    red.push_str(&identitet_celije(linija));
    red.push_str(&format!(
        "<td class=\"amount\">{}</td>",
        format_kolicina(linija.stvarna_kolicina_milli)
    ));
    if iznos_prebrojan(lista) {
        red.push_str(&iznos_celija(linija.cena_minor));
    }
    red.push_str(&format!(
        "<td>{}</td></tr>\n",
        tekst(linija.blizi_opis.as_deref())
    ));
    red
}

/// One čl. 9 st. 3 row: the count, the book quantity, the naturalna razlika and
/// the čl. 9 st. 1 t. 5–6 valuation.
///
/// The three vrednosti use `crate::popis::vrednost_minor` — the same integer
/// arithmetic the izveštaj's rollup uses, so the printed sheet and the izveštaj
/// cannot disagree about a figure. A product that will not fit an `i64` renders
/// „—“ rather than a wrapped number, because a wrapped value is a manjak reported
/// as a višak.
fn red_faza_b(linija: &PopisLineView) -> String {
    let vrednost_po_popisu = linija
        .cena_minor
        .and_then(|cena| vrednost_minor(linija.stvarna_kolicina_milli, cena));
    let vrednost_po_knjigama = linija
        .cena_minor
        .zip(linija.knjigovodstvena_kolicina_milli)
        .and_then(|(cena, knjigovodstvena)| vrednost_minor(knjigovodstvena, cena));
    let vrednosna_razlika = linija
        .cena_minor
        .zip(linija.razlika_milli)
        .and_then(|(cena, razlika)| vrednost_minor(razlika, cena));

    let mut red = String::from("<tr>");
    red.push_str(&identitet_celije(linija));
    red.push_str(&format!(
        "<td class=\"amount\">{}</td>",
        format_kolicina(linija.stvarna_kolicina_milli)
    ));
    red.push_str(&kolicina_celija(linija.knjigovodstvena_kolicina_milli));
    red.push_str(&kolicina_celija(linija.razlika_milli));
    red.push_str(&iznos_celija(linija.cena_minor));
    red.push_str(&iznos_celija(vrednost_po_popisu));
    red.push_str(&iznos_celija(vrednost_po_knjigama));
    red.push_str(&iznos_celija(vrednosna_razlika));
    red.push_str(&format!(
        "<td>{}</td></tr>\n",
        tekst(linija.blizi_opis.as_deref())
    ));
    red
}

/// The čl. 8 st. 4 identity every phase carries: nomenklaturni broj/šifra, naziv,
/// vrsta and jedinica mere.
fn identitet_celije(linija: &PopisLineView) -> String {
    format!(
        "<td>{}</td><td>{}</td><td>{}</td><td>{}</td>",
        tekst(linija.sifra.as_deref()),
        escape_html(&linija.naziv),
        tekst(linija.vrsta.as_deref()),
        tekst(linija.jedinica_mere.as_deref())
    )
}

/// Req. 30 — the commission signs, so the paper has somewhere to sign.
///
/// **The heading is read off the roster, not hardcoded.** PoP čl. 6 st. 1 lets the
/// popis of a mikro pravno lice or a preduzetnik be taken by one person, and čl. 6
/// st. 2 extends the commission provisions to that person only *shodno* — they are
/// not a komisija of one, which is why [`uloga_naziv`] gives them a name of their
/// own. A block headed „Potpisi članova komisije za popis“ directly above a signer
/// whose uloga line reads „jedno lice koje vrši popis“ has the document
/// contradicting itself about who signed it. Where the roster is mixed, empty or
/// unknown the heading carries both limbs, in the construction `cl47.rs` already
/// registers the data subjects under.
fn potpisni_blok(view: &PopisSessionView, faza: PrintFaza) -> String {
    let mut html = format!(
        "<section class=\"potpisi\">\n<h2>{} — {}</h2>\n",
        escape_html(potpisni_naslov(view)),
        escape_html(faza.pravni_osnov())
    );

    if view.komisija.is_empty() {
        html.push_str(
            "<p class=\"prazno\">Nije evidentiran nijedan član komisije za popis, odnosno jedno \
             lice koje vrši popis (PoP čl. 6 st. 1).</p>\n",
        );
    } else {
        html.push_str("<div class=\"potpisi-red\">\n");
        for clan in &view.komisija {
            html.push_str(&potpis_clana(clan));
        }
        html.push_str("</div>\n");
    }

    html.push_str(&format!(
        "<p class=\"meta\">Datum popisa: {}</p>\n\
         <p class=\"meta\">Mesto i datum potpisivanja: ______________________</p>\n</section>\n",
        escape_html(&view.datum_popisa)
    ));
    html
}

/// What to call the people who sign, given who they are. A roster that is entirely
/// `jedno_lice` is the čl. 6 st. 1 shape and gets the čl. 6 st. 1 name; anything
/// else — a komisija, a mixed roster, or a roster nobody has filled in yet — gets
/// both limbs, because a heading may not decide the popis is a komisija when the
/// evidencija does not say so.
fn potpisni_naslov(view: &PopisSessionView) -> &'static str {
    match sastav_popisa(view) {
        Sastav::JednoLice => "Potpis lica koje vrši popis (PoP čl. 6 st. 1)",
        Sastav::Komisija | Sastav::Neodredjen => {
            "Potpisi članova komisije za popis, odnosno jednog lica koje vrši popis"
        }
    }
}

fn potpis_clana(clan: &KomisijaClanView) -> String {
    potpis_linija(Some(&clan.ime), uloga_naziv(&clan.uloga))
}

/// One ruled line with the uloga under it. `ime` is `None` where the app holds no
/// name for the person who signs — the lice iz čl. 4 st. 2 on an odluka nobody has
/// approved a plan under — and an em dash is printed rather than a guess: the
/// obveznik's registered name is the shop's, not that person's.
fn potpis_linija(ime: Option<&str>, uloga: &str) -> String {
    format!(
        "<div class=\"potpis\"><div class=\"ime\">{}</div>\
         <div class=\"uloga\">{}</div>\
         <div class=\"linija\"></div>\
         <div class=\"oznaka\">svojeručni potpis</div></div>\n",
        tekst(ime),
        escape_html(uloga)
    )
}

/// The commission roles in front of the shop. The stored keys are ASCII-folded
/// column values and no operator string may quote one, so an uloga this build does
/// not know is reported as unrecognised rather than printed raw.
fn uloga_naziv(uloga: &str) -> &'static str {
    match uloga {
        "predsednik" => "predsednik komisije za popis",
        "clan" => "član komisije za popis",
        "jedno_lice" => "jedno lice koje vrši popis (PoP čl. 6 st. 1)",
        _ => "uloga nije prepoznata",
    }
}

/// Req. 32's broj liste. Built from the lista's fixed place among the six rather
/// than from its position in this popis, so the same lista carries the same number
/// on every print of the same popis whatever else was counted.
fn broj_liste(view: &PopisSessionView, lista: PopisLista) -> String {
    let redni = PopisLista::ALL
        .iter()
        .position(|kandidat| *kandidat == lista)
        .map_or(0, |index| index + 1);
    format!("{}-{}", view.id, redni)
}

/// What the price column holds on this lista — the same column carries three
/// different things and only the heading says which.
///
/// Čl. 11 st. 1 makes it the **apoen** and čl. 12 st. 2 the **iznos** of a
/// nedokumentovano potraživanje ili obaveza; on both it is the commission's own
/// figure and part of the counted state. Everywhere else it is the čl. 9 st. 1
/// t. 5 obračunska cena. The count sheet uses the same three words.
fn iznos_kolona(lista: PopisLista) -> &'static str {
    match lista {
        PopisLista::Gotovina => "Apoen",
        PopisLista::Potrazivanja => "Iznos",
        _ => "Cena",
    }
}

/// The two liste whose money column belongs to the counted state, so čl. 8 st. 5
/// does not keep it back — it has no perpetual record behind it to keep back.
fn iznos_prebrojan(lista: PopisLista) -> bool {
    matches!(lista, PopisLista::Gotovina | PopisLista::Potrazivanja)
}

// ---------------------------------------------------------------------------
// Req. 35 — the odluka o popisu and the plan rada (PoP čl. 8 st. 1–2)
// ---------------------------------------------------------------------------

/// The odluka o popisu i obrazovanju komisije: what popis is ordered, and who
/// takes it.
///
/// PoP čl. 4 st. 2 puts the organisation and correctness of the popis on the lice
/// iz čl. 43 st. 3 ZoRač — for a preduzetnik that is the preduzetnik personally,
/// a direct assignment and not a *shodna primena* — so the document names that
/// responsibility and leaves the signature to that person. It appoints nobody by
/// itself and dates itself with nothing: `odluka_doneta_at` is written by no
/// command in this build, and a document that filled its own date in would date an
/// act nobody recorded.
pub fn render_odluka(company: &CompanySettings, view: &PopisSessionView) -> String {
    let sastav = sastav_popisa(view);
    let naslov = match sastav {
        Sastav::JednoLice => "ODLUKA O POPISU I ODREĐIVANJU LICA KOJE VRŠI POPIS",
        Sastav::Komisija => "ODLUKA O POPISU I OBRAZOVANJU KOMISIJE ZA POPIS",
        Sastav::Neodredjen => {
            "ODLUKA O POPISU I OBRAZOVANJU KOMISIJE ZA POPIS, ODNOSNO ODREĐIVANJU JEDNOG LICA KOJE \
             VRŠI POPIS"
        }
    };

    let mut html = doc_head(&format!("Odluka o popisu — popis br. {}", view.id));
    html.push_str(&format!("<h1>{}</h1>\n", escape_html(naslov)));

    let mut redovi = zaglavlje_redovi(company, view);
    redovi.push(("Oznaka odluke", tekst(view.odluka_ref.as_deref())));
    redovi.push(("Datum donošenja", datum_donosenja(view)));
    html.push_str(&zaglavlje_tabela(&redovi));

    html.push_str(&format!(
        "<p class=\"napomena\">{}</p>\n",
        escape_html(
            "Za organizaciju i pravilnost popisa imovine i obaveza odgovorno je lice iz člana 43. \
             stav 3. Zakona o računovodstvu (PoP čl. 4 st. 2), a to je kod preduzetnika sam \
             preduzetnik — on finansijske izveštaje potpisuje lično (ZoRač čl. 43 st. 3)."
        )
    ));

    html.push_str("<section class=\"odredba\">\n<h2>Predmet odluke</h2>\n");
    html.push_str(&format!(
        "<p class=\"meta\">{}</p>\n",
        escape_html(&format!(
            "Određuje se popis imovine i obaveza u maloprodajnom objektu „{}“, sa stanjem na dan \
             {}. Vrsta popisa: {}.",
            view.prodajno_mesto,
            view.datum_popisa,
            view.vrsta.naziv()
        ))
    ));
    html.push_str(&format!(
        "<p class=\"meta\">{}</p>\n",
        escape_html(match sastav {
            Sastav::JednoLice => {
                "Popis vrši jedno lice koje se određuje ovom odlukom (PoP čl. 6 st. 1). Odredbe \
                 koje se odnose na komisiju za popis shodno se primenjuju i na to lice (PoP čl. 6 \
                 st. 2)."
            }
            Sastav::Komisija => {
                "Popis vrši komisija za popis koja se obrazuje ovom odlukom, u sastavu navedenom u \
                 nastavku."
            }
            // Neither limb may be dropped where the evidencija does not say which
            // one this popis is, and neither may the sastav be described: an empty
            // roster has none to point at and a mixed one is not the sastav of
            // either body. The section below states what is recorded.
            Sastav::Neodredjen => {
                "Popis vrši komisija za popis koja se obrazuje ovom odlukom, odnosno jedno lice \
                 koje se ovom odlukom određuje (PoP čl. 6 st. 1). Odredbe koje se odnose na \
                 komisiju za popis shodno se primenjuju i na to lice (PoP čl. 6 st. 2)."
            }
        })
    ));
    html.push_str("</section>\n");

    html.push_str(&sastav_sekcija(view));
    html.push_str(&odobrenje_sekcija(view));

    html.push_str("<section class=\"potpisi\">\n<h2>Potpis</h2>\n<div class=\"potpisi-red\">\n");
    html.push_str(&potpis_linija(None, ULOGA_CL_4_ST_2));
    html.push_str(
        "</div>\n<p class=\"meta\">Mesto i datum potpisivanja: \
                   ______________________</p>\n</section>\n",
    );
    html.push_str(&podnozje());
    html
}

/// The plan rada (PoP čl. 8 st. 1) with its čl. 8 st. 2 approval.
///
/// *„Комисија за попис имовине и обавеза пре почетка пописа сачињава план рада по
/// коме ће вршити попис. / План рада комисије за попис имовине и обавеза одобрава
/// лице из члана 4. став 2. овог правилника.“* The schedule itself is the shop's —
/// nothing here invents one, and where the shop supplied none the document says so
/// in a sentence rather than printing an empty table, because an empty schedule on
/// a document somebody signs reads as „there is no work to do“.
pub fn render_plan_rada(company: &CompanySettings, view: &PopisSessionView) -> String {
    let naslov = match sastav_popisa(view) {
        Sastav::JednoLice => "PLAN RADA LICA KOJE VRŠI POPIS",
        Sastav::Komisija => "PLAN RADA KOMISIJE ZA POPIS",
        Sastav::Neodredjen => "PLAN RADA KOMISIJE ZA POPIS, ODNOSNO JEDNOG LICA KOJE VRŠI POPIS",
    };

    let mut html = doc_head(&format!("Plan rada — popis br. {}", view.id));
    html.push_str(&format!("<h1>{}</h1>\n", escape_html(naslov)));

    let mut redovi = zaglavlje_redovi(company, view);
    redovi.push(("Oznaka odluke", tekst(view.odluka_ref.as_deref())));
    html.push_str(&zaglavlje_tabela(&redovi));

    html.push_str(&format!(
        "<p class=\"napomena\">{}</p>\n",
        escape_html(
            "Komisija za popis imovine i obaveza pre početka popisa sačinjava plan rada po kome će \
             vršiti popis, a taj plan odobrava lice iz člana 4. stav 2. ovog pravilnika (PoP čl. 8 \
             st. 1 i st. 2). Odredbe koje se odnose na komisiju za popis shodno se primenjuju i na \
             jedno lice koje popis vrši kod mikro pravnog lica i preduzetnika (PoP čl. 6 st. 1 i \
             st. 2)."
        )
    ));

    html.push_str(&sastav_sekcija(view));

    html.push_str("<section class=\"plan\">\n<h2>Raspored i zaduženja</h2>\n");
    match view.plan_rada_json.as_deref().and_then(raspored_html) {
        Some(raspored) => html.push_str(&raspored),
        None => html.push_str(&format!(
            "<p class=\"prazno\">{}</p>\n",
            escape_html(
                "Plan rada nije unet u aplikaciju. Plan rada se sačinjava pre početka popisa (PoP \
                 čl. 8 st. 1) — upišite raspored i zaduženja na odštampanom primerku pre nego što \
                 popis počne."
            )
        )),
    }
    html.push_str("</section>\n");

    html.push_str(&odobrenje_sekcija(view));

    html.push_str("<section class=\"potpisi\">\n<h2>Potpisi</h2>\n<div class=\"potpisi-red\">\n");
    for clan in &view.komisija {
        html.push_str(&potpis_linija(Some(&clan.ime), uloga_naziv(&clan.uloga)));
    }
    html.push_str(&potpis_linija(
        view.plan_rada_odobrio.as_deref(),
        ULOGA_CL_4_ST_2,
    ));
    html.push_str("</div>\n</section>\n");
    html.push_str(&podnozje());
    html
}

/// What the čl. 4 st. 2 person is called beside the line they sign on. Kept in one
/// place because both documents carry that line and a second wording would let
/// them disagree about who answers for the popis.
const ULOGA_CL_4_ST_2: &str =
    "lice odgovorno za organizaciju i pravilnost popisa (PoP čl. 4 st. 2)";

/// Čl. 8 st. 2, reported on both documents and never inferred.
///
/// **Half a record is no approval.** v21 pairs `plan_rada_odobrio` with
/// `plan_rada_odobreno_at` in a CHECK, but this renderer is handed a view and not a
/// row — a cached view or a regressed query could carry one without the other — and
/// čl. 8 st. 2 is satisfied by an approver AND a time, so anything less prints as
/// unapproved.
///
/// **And the approval says what it was recorded over.** Čl. 8 st. 1 makes the plan
/// rada the komisija's own artefact, not this application's: `plan_rada_json` is an
/// optional field on the open form and nothing else in the crate writes it, so a
/// shop that keeps its schedule on paper is the ordinary case rather than an edge,
/// and `odobri_plan_rada` rightly does not refuse it — refusing would deny a čl. 8
/// st. 2 act that really happened. What may not happen is „Plan rada je odobren“
/// standing a few lines under „Plan rada nije unet u aplikaciju“ with nothing
/// connecting them, on a sheet somebody signs. Presence is decided by
/// [`raspored_html`], the same function that lays the schedule out, so the two
/// sections cannot disagree about whether there is one.
fn odobrenje_sekcija(view: &PopisSessionView) -> String {
    let mut html = String::from("<section class=\"odobrenje\">\n<h2>Odobrenje plana rada</h2>\n");
    match (
        view.plan_rada_odobrio.as_deref(),
        view.plan_rada_odobreno_at.as_deref(),
    ) {
        (Some(ko), Some(kada)) => {
            let mut recenica = format!(
                "Plan rada je odobren (PoP čl. 8 st. 2). Odobrio: {ko}. Odobrenje evidentirano: \
                 {kada}."
            );
            // What the app holds, and nothing beyond it: that the schedule is on
            // paper somewhere is what čl. 8 st. 1 requires of the komisija, not
            // something this row records — a document that asserted the primerak
            // exists would vouch for a paper nobody here has seen.
            if !plan_rada_u_aplikaciji(view) {
                recenica.push_str(
                    " U aplikaciji je evidentirano odobrenje, a ne i sadržina plana rada — \
                     raspored i zaduženja na koje se odobrenje odnosi nisu uneti u aplikaciju \
                     (PoP čl. 8 st. 1).",
                );
            }
            html.push_str(&format!(
                "<p class=\"napomena\">{}</p>\n",
                escape_html(&recenica)
            ));
        }
        _ => html.push_str(&format!(
            "<p class=\"prazno\">{}</p>\n",
            escape_html(
                "Plan rada nije odobren — u aplikaciji nije evidentirano odobrenje lica iz člana \
                 4. stav 2. Pravilnika o popisu (PoP čl. 8 st. 2), koje je kod preduzetnika sam \
                 preduzetnik."
            )
        )),
    }
    html.push_str("</section>\n");
    html
}

/// The roster both documents carry: who was appointed, in what uloga, and whether
/// the evidencija says they handle the assets being counted.
fn sastav_sekcija(view: &PopisSessionView) -> String {
    let mut html = format!(
        "<section class=\"sastav\">\n<h2>{}</h2>\n",
        escape_html(match sastav_popisa(view) {
            Sastav::JednoLice => "Lice koje vrši popis (PoP čl. 6 st. 1)",
            Sastav::Komisija => "Komisija za popis",
            Sastav::Neodredjen =>
                "Komisija za popis, odnosno jedno lice koje vrši popis (PoP čl. 6 st. 1)",
        })
    );

    if view.komisija.is_empty() {
        html.push_str(
            "<p class=\"prazno\">Nije evidentiran nijedan član komisije za popis, odnosno jedno \
             lice koje vrši popis (PoP čl. 6 st. 1).</p>\n",
        );
        html.push_str("</section>\n");
        return html;
    }

    html.push_str(
        "<table>\n<thead>\n<tr><th>Ime i prezime</th><th>Uloga</th>\
         <th>Rukuje imovinom koja se popisuje</th></tr>\n</thead>\n<tbody>\n",
    );
    for clan in &view.komisija {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            escape_html(&clan.ime),
            escape_html(uloga_naziv(&clan.uloga)),
            if clan.rukuje_imovinom { "da" } else { "ne" }
        ));
    }
    html.push_str("</tbody>\n</table>\n");

    // Req. 40 / PoP čl. 5 st. 1 — the odluka IS the appointment, so the objection
    // belongs on the paper that makes it. It warns and never blocks: whether the
    // exclusion reaches the čl. 6 st. 1 single person through the čl. 6 st. 2
    // shodna primena is unresolved (§6 R-5), and the module's answer to an
    // unresolved question is to say so, not to refuse.
    let rukovaoci: Vec<&str> = view
        .komisija
        .iter()
        .filter(|clan| clan.rukuje_imovinom)
        .map(|clan| clan.ime.as_str())
        .collect();
    if !rukovaoci.is_empty() {
        html.push_str(&format!(
            "<p class=\"napomena\">{}</p>\n",
            escape_html(&format!(
                "Upozorenje (PoP čl. 5 st. 1): u komisiju za popis ne mogu biti određena lica koja \
                 rukuju imovinom koja se popisuje, a za sledeća lica je evidentirano da njome \
                 rukuju: {}. Da li to važi i kada popis vrši jedno lice (PoP čl. 6 st. 1 i st. 2) \
                 nije razjašnjeno. Popis nije zaustavljen — proverite sastav pre potpisivanja.",
                rukovaoci.join(", ")
            ))
        ));
    }

    html.push_str("</section>\n");
    html
}

/// The čl. 8 st. 1 schedule as the shop supplied it, or `None` where it supplied
/// nothing.
///
/// **The stored value is not guaranteed to be JSON.** v20 named the column
/// `plan_rada_json`, but nothing validates it and the field in front of the
/// operator is a plain text input, so most shops will type a sentence. A parse that
/// succeeds is laid out; one that fails prints the text verbatim, because a
/// document that silently dropped what it could not parse would report that the
/// commission had no plan when it had one.
///
/// An empty object, an empty array, a null and a blank string all mean the same
/// thing and all return `None` — the caller then states the absence. `serde_json`
/// is built here without `preserve_order`, so an object's fields print in key
/// order rather than in the order they were typed; an array keeps its order, which
/// is the shape a sequence of steps belongs in anyway.
/// Whether the application holds the čl. 8 st. 1 schedule itself, as opposed to
/// holding only a record about it. Asked of the layout function rather than of the
/// column, so „the app has no schedule“ means exactly what the plan rada prints.
fn plan_rada_u_aplikaciji(view: &PopisSessionView) -> bool {
    view.plan_rada_json
        .as_deref()
        .and_then(raspored_html)
        .is_some()
}

fn raspored_html(plan: &str) -> Option<String> {
    let plan = plan.trim();
    if plan.is_empty() {
        return None;
    }

    match serde_json::from_str::<serde_json::Value>(plan) {
        Ok(vrednost) => vrednost_html(&vrednost),
        Err(_) => Some(format!("<p class=\"raspored\">{}</p>\n", escape_html(plan))),
    }
}

fn vrednost_html(vrednost: &serde_json::Value) -> Option<String> {
    match vrednost {
        serde_json::Value::Null => None,
        serde_json::Value::Object(polja) => {
            let redovi: String = polja
                .iter()
                .map(|(kljuc, vrednost)| {
                    format!(
                        "<tr><th>{}</th><td>{}</td></tr>\n",
                        escape_html(kljuc),
                        vrednost_html(vrednost).unwrap_or_else(|| "—".to_string())
                    )
                })
                .collect();
            (!redovi.is_empty()).then(|| {
                format!("<table class=\"raspored\">\n<tbody>\n{redovi}</tbody>\n</table>\n")
            })
        }
        serde_json::Value::Array(stavke) => {
            let tacke: String = stavke
                .iter()
                .map(|stavka| {
                    format!(
                        "<li>{}</li>\n",
                        vrednost_html(stavka).unwrap_or_else(|| "—".to_string())
                    )
                })
                .collect();
            (!tacke.is_empty()).then(|| format!("<ul class=\"raspored\">\n{tacke}</ul>\n"))
        }
        serde_json::Value::String(tekst) => {
            let tekst = tekst.trim();
            (!tekst.is_empty())
                .then(|| format!("<p class=\"raspored\">{}</p>\n", escape_html(tekst)))
        }
        drugo => Some(format!(
            "<p class=\"raspored\">{}</p>\n",
            escape_html(&drugo.to_string())
        )),
    }
}

/// The odluka's own date. Nothing in this build writes `odluka_doneta_at`, so on
/// every popis the pilot has it is NULL — and the document says that rather than
/// dating itself with the day it was printed, which would record a čl. 4 st. 2 act
/// on a day nobody chose.
fn datum_donosenja(view: &PopisSessionView) -> String {
    match view.odluka_doneta_at.as_deref() {
        Some(kada) => escape_html(kada),
        None => escape_html("nije evidentiran u aplikaciji — upišite ga na odštampanom primerku"),
    }
}

/// Which of the two čl. 6 shapes this popis has — and the third state, where the
/// evidencija does not say.
///
/// PoP čl. 6 st. 1 lets the popis of a mikro pravno lice or a preduzetnik be taken
/// by one person, and čl. 6 st. 2 extends the commission provisions to that person
/// only *shodno*, so the two are not interchangeable names for one body.
/// [`Sastav::Neodredjen`] is not a tidy-up: `open_popis` imposes no non-empty
/// roster and the odluka **is** the appointment, so printing it before anybody is
/// appointed is the ordinary way to reach these documents. A renderer that
/// resolved that silence — or a roster that mixes the two ulogas — into „komisija“
/// would tell a preduzetnik, on the paper that appoints him, that the popis
/// belongs to a body he does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Sastav {
    /// Every recorded member is the čl. 6 st. 1 single person.
    JednoLice,
    /// Nobody recorded is, so this is the komisija the pravilnik speaks of.
    Komisija,
    /// Nobody is recorded at all, or the roster carries both ulogas at once.
    Neodredjen,
}

fn sastav_popisa(view: &PopisSessionView) -> Sastav {
    let ukupno = view.komisija.len();
    let pojedinacno = view
        .komisija
        .iter()
        .filter(|clan| clan.uloga == "jedno_lice")
        .count();
    match pojedinacno {
        _ if ukupno == 0 => Sastav::Neodredjen,
        0 => Sastav::Komisija,
        isto if isto == ukupno => Sastav::JednoLice,
        _ => Sastav::Neodredjen,
    }
}

/// What every generated document of this module ends with. Pravilnik 89/2020 runs
/// čl. 1–16 with no prilog, so it prescribes no form for the odluka or the plan
/// rada either — the layout is ours and the paper says so.
fn podnozje() -> String {
    "<footer>Zakon ne propisuje obrazac ovog dokumenta — raspored je interni (Pravilnik o popisu, \
     čl. 1–16, nema priloga sa obrascem). Interni dokument. Nije fiskalni dokument.</footer>\n\
     </body>\n</html>\n"
        .to_string()
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

const DOC_STYLE: &str = "\
@page { margin: 1cm }\n\
body { font-family: sans-serif; color: #111; margin: 1cm; }\n\
h1 { font-size: 1.3rem; }\n\
h2 { font-size: 1.05rem; margin-bottom: 0.1rem; }\n\
.meta { color: #444; margin: 0.1rem 0; font-size: 0.9rem; }\n\
.napomena { border: 1px solid #999; padding: 0.4rem 0.6rem; margin: 0.6rem 0; }\n\
.prazno { color: #444; font-style: italic; }\n\
table { border-collapse: collapse; width: 100%; margin: 0.5rem 0 1rem; }\n\
th, td { border: 1px solid #999; padding: 0.2rem 0.4rem; text-align: left; \
vertical-align: top; }\n\
td.amount, th.amount { text-align: right; white-space: nowrap; }\n\
table.zaglavlje { width: auto; }\n\
table.zaglavlje th { background: #f0f0f0; }\n\
section.lista { page-break-inside: avoid; }\n\
section.odredba, section.sastav, section.plan, section.odobrenje \
{ page-break-inside: avoid; }\n\
table.raspored { width: auto; }\n\
table.raspored th { background: #f0f0f0; }\n\
ul.raspored { margin: 0.2rem 0; }\n\
p.raspored { margin: 0.2rem 0; }\n\
.potpisi-red { display: flex; flex-wrap: wrap; gap: 1.5rem; margin-top: 1.5rem; }\n\
.potpis { min-width: 12rem; }\n\
.potpis .ime { font-weight: bold; }\n\
.potpis .uloga { color: #444; font-size: 0.9rem; }\n\
.potpis .linija { border-bottom: 1px solid #111; height: 2.2rem; }\n\
.potpis .oznaka { color: #444; font-size: 0.8rem; }\n\
footer { margin-top: 2rem; color: #666; font-size: 0.85rem; }\n";

fn doc_head(title: &str) -> String {
    format!(
        "<!doctype html>\n<html lang=\"sr-Latn\">\n<head>\n<meta charset=\"utf-8\">\n\
         <title>{}</title>\n<style>\n{}</style>\n</head>\n<body>\n",
        escape_html(title),
        DOC_STYLE
    )
}

/// Escapes the five HTML-significant characters so a naziv carrying `<` or `&`
/// can never break out of the document.
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

/// An optional text cell: escaped, or an em dash when there is nothing. A blank
/// string is nothing too — an empty cell on a signed sheet reads as an omission.
fn tekst(value: Option<&str>) -> String {
    match value.map(str::trim) {
        Some(text) if !text.is_empty() => escape_html(text),
        _ => "—".to_string(),
    }
}

fn kolicina_celija(milli: Option<i64>) -> String {
    match milli {
        Some(value) => format!("<td class=\"amount\">{}</td>", format_kolicina(value)),
        None => "<td class=\"amount\">—</td>".to_string(),
    }
}

fn iznos_celija(minor: Option<i64>) -> String {
    match minor {
        Some(value) => format!("<td class=\"amount\">{}</td>", format_rsd_minor(value)),
        None => "<td class=\"amount\">—</td>".to_string(),
    }
}

/// A milli-unit quantity in Serbian notation: `7_000 → "7"`, `9_999 → "9,999"`,
/// `-1_234 → "-1,234"`, `1_234_567 → "1.234,567"`. Trailing zeros of the
/// fractional part are dropped, because a boutique counting pieces should read „7“
/// and not „7,000“.
fn format_kolicina(milli: i64) -> String {
    let delilac = MILLI.unsigned_abs();
    let abs = milli.unsigned_abs();
    let celi = abs / delilac;
    let deo = abs % delilac;

    let mut out = String::new();
    if milli < 0 {
        out.push('-');
    }
    out.push_str(&grupisano(celi));
    if deo != 0 {
        let mut frakcija = format!("{deo:03}");
        while frakcija.ends_with('0') {
            frakcija.pop();
        }
        out.push(',');
        out.push_str(&frakcija);
    }
    out
}

/// Integer minor units as grouped RSD: `900000 → "9.000,00"`.
fn format_rsd_minor(minor: i64) -> String {
    let abs = minor.unsigned_abs();
    let dinari = abs / 100;
    let para = abs % 100;
    let znak = if minor < 0 { "-" } else { "" };
    format!("{znak}{},{para:02}", grupisano(dinari))
}

/// Thousands separated the Serbian way: `1234567 → "1.234.567"`.
fn grupisano(broj: u64) -> String {
    let cifre = broj.to_string();
    let bajtovi = cifre.as_bytes();
    let duzina = bajtovi.len();
    let mut out = String::with_capacity(duzina + duzina / 3);
    for (index, bajt) in bajtovi.iter().enumerate() {
        if index > 0 && (duzina - index).is_multiple_of(3) {
            out.push('.');
        }
        out.push(*bajt as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::popis::{KomisijaClanView, PopisLineView, PopisSessionView};
    use crate::popis::{PopisStatus, PopisVrsta};

    fn company() -> CompanySettings {
        CompanySettings {
            shop_name: "Butik Vantum pr Novi Pazar".to_string(),
            address: "Njegoševa 12, Novi Pazar".to_string(),
            pib: "123456789".to_string(),
            registration_number: "63012345".to_string(),
            ..CompanySettings::default()
        }
    }

    fn prazan_view() -> PopisSessionView {
        PopisSessionView {
            id: 7,
            vrsta: PopisVrsta::Godisnji,
            prodajno_mesto: "Butik Centar".into(),
            datum_popisa: "2026-12-31".into(),
            period_from: Some("2026-01-01".into()),
            period_to: Some("2026-12-31".into()),
            status: PopisStatus::Counting,
            plan_rada_json: None,
            plan_rada_odobrio: None,
            plan_rada_odobreno_at: None,
            odluka_ref: None,
            odluka_doneta_at: None,
            perpetual_odluka_ref: None,
            uskladjivanje_potvrdjeno_at: None,
            posted_at: None,
            faza_a_potpisana: false,
            faza_b_potpisana: false,
            knjigovodstvo_dostupno: false,
            komisija: Vec::new(),
            potpisi: Vec::new(),
            linije: Vec::new(),
            liste: Vec::new(),
            konsignacija_rok: None,
            upozorenja: Vec::new(),
        }
    }

    fn linija(id: i64, lista: PopisLista, naziv: &str) -> PopisLineView {
        PopisLineView {
            id,
            lista_vrsta: lista.as_db_str().to_string(),
            sifra: Some(format!("ART-{id}")),
            naziv: naziv.to_string(),
            vrsta: Some("Ženska konfekcija".into()),
            jedinica_mere: Some("kom".into()),
            stvarna_kolicina_milli: 7_000,
            blizi_opis: Some("Polica A2".into()),
            knjigovodstvena_kolicina_milli: None,
            razlika_milli: None,
            cena_minor: None,
        }
    }

    /// A counting session with two stavke on the ordinary lista.
    fn view_with_lines() -> PopisSessionView {
        let mut view = prazan_view();
        view.komisija = vec![clan(1, "Mira Marković", "predsednik")];
        view.linije = vec![
            linija(1, PopisLista::Roba, "Košulja"),
            linija(2, PopisLista::Roba, "Suknja"),
        ];
        view
    }

    fn clan(id: i64, ime: &str, uloga: &str) -> KomisijaClanView {
        KomisijaClanView {
            id,
            ime: ime.to_string(),
            uloga: uloga.to_string(),
            rukuje_imovinom: false,
        }
    }

    /// PoP čl. 5 st. 1 excludes a person who handles the assets from the komisija.
    /// Req. 40 warns and never blocks, so the odluka still prints them.
    fn clan_rukovalac(id: i64, ime: &str, uloga: &str) -> KomisijaClanView {
        KomisijaClanView {
            rukuje_imovinom: true,
            ..clan(id, ime, uloga)
        }
    }

    /// A popis whose plan rada carries the čl. 8 st. 2 approval, both columns
    /// together — the only shape v21's CHECK admits.
    fn view_odobren() -> PopisSessionView {
        let mut view = view_with_lines();
        view.plan_rada_json = Some(PLAN_RADA_JSON.to_string());
        view.plan_rada_odobrio = Some("Miloš Đurđević".into());
        view.plan_rada_odobreno_at = Some("2026-12-30T08:15:00Z".into());
        view
    }

    /// What the shop's own plan rada looks like when it was typed into the field
    /// v20 named `plan_rada_json`.
    const PLAN_RADA_JSON: &str = r#"{"raspored":"30.12.2026. od 8.00 do 14.00","zaduženja":["Mira: roba","Petar: gotovina"]}"#;

    /// The state after the čl. 8 st. 5 potpis and the obračun: the book side is
    /// released and the razlike are derived.
    fn computed_view() -> PopisSessionView {
        let mut view = view_with_lines();
        view.status = PopisStatus::Computed;
        view.faza_a_potpisana = true;
        view.knjigovodstvo_dostupno = true;
        for (index, line) in view.linije.iter_mut().enumerate() {
            line.knjigovodstvena_kolicina_milli = Some(8_000);
            line.razlika_milli = Some(line.stvarna_kolicina_milli - 8_000);
            line.cena_minor = Some(249_900 + i64::try_from(index).unwrap_or(0));
        }
        view
    }

    fn view_with_commission(imena: &[&str]) -> PopisSessionView {
        let mut view = view_with_lines();
        view.komisija = imena
            .iter()
            .enumerate()
            .map(|(index, ime)| {
                clan(
                    i64::try_from(index).unwrap_or(0) + 1,
                    ime,
                    if index == 0 { "predsednik" } else { "clan" },
                )
            })
            .collect();
        view
    }

    fn view_with_every_lista() -> PopisSessionView {
        let mut view = view_with_lines();
        view.linije = PopisLista::ALL
            .into_iter()
            .enumerate()
            .map(|(index, lista)| {
                linija(
                    i64::try_from(index).unwrap_or(0) + 1,
                    lista,
                    "Stavka na listi",
                )
            })
            .collect();
        view
    }

    /// A popis taken by the PoP čl. 6 st. 1 single person — the pilot's own
    /// shape, and a first-class `uloga` since v20.
    fn view_jedno_lice() -> PopisSessionView {
        let mut view = computed_view();
        view.komisija = vec![clan(1, "Vlasnik lično", "jedno_lice")];
        view
    }

    /// The document's prose: the phase napomena, the empty-state lines, the
    /// headings and the footer. Wherever the sheet says who counts and who signs,
    /// it says it in one of these.
    fn prozni_blokovi(html: &str) -> Vec<String> {
        let mut blokovi = Vec::new();
        for (otvara, zatvara) in [
            ("<p class=\"napomena\">", "</p>"),
            ("<p class=\"prazno\">", "</p>"),
            ("<p class=\"meta\">", "</p>"),
            ("<h1>", "</h1>"),
            ("<h2>", "</h2>"),
            ("<footer>", "</footer>"),
        ] {
            for deo in html.split(otvara).skip(1) {
                blokovi.push(
                    deo.split(zatvara)
                        .next()
                        .expect("a prose block must close")
                        .to_string(),
                );
            }
        }
        blokovi
    }

    /// Wherever a document names the komisija, the čl. 6 st. 1 limb has to be in
    /// the same block — the sweep Deviation 1b introduced, hoisted so both the
    /// sheet and the pre-count pair run the identical rule.
    ///
    /// **Case-folded, which the first version was not.** The titles are set in
    /// capitals, so a `contains("komisij")` over „ODLUKA O POPISU I OBRAZOVANJU
    /// KOMISIJE ZA POPIS“ found no mention of a komisija at all and swept the one
    /// line of the document nobody can miss.
    fn komisija_nikad_bez_cl_6_limba(html: &str, opis: &str) {
        for blok in prozni_blokovi(html) {
            let presavijen = blok.to_lowercase();
            if !presavijen.contains("komisij") {
                continue;
            }
            assert!(
                presavijen.contains("jedno lice") || presavijen.contains("jednog lica"),
                "„{blok}“ names the komisija without the čl. 6 limb ({opis})"
            );
        }
    }

    /// The `<section class="odobrenje">` of either generated document — the čl. 8
    /// st. 2 block, read on its own so an assertion about it cannot be satisfied
    /// by a sentence somewhere else on the page.
    fn odobrenje_markup(html: &str) -> &str {
        html.split("<section class=\"odobrenje\">")
            .nth(1)
            .and_then(|deo| deo.split("</section>").next())
            .unwrap_or_else(|| panic!("both generated documents carry an odobrenje block: {html}"))
    }

    /// The `<section class="lista">` whose markup carries `naslov`.
    fn sekcija_markup<'a>(html: &'a str, naslov: &str) -> &'a str {
        html.split("<section class=\"lista\">")
            .skip(1)
            .map(|sekcija| {
                sekcija
                    .split("</section>")
                    .next()
                    .expect("a lista section must end")
            })
            .find(|sekcija| sekcija.contains(naslov))
            .unwrap_or_else(|| panic!("the section „{naslov}“ was not printed: {html}"))
    }

    /// The inner text of every `<th>` and `<td>` of a markup fragment, **in the
    /// order they were emitted**. Position is the whole point: a cell count and a
    /// heading set are both invariant under a permutation.
    fn celije_redom(fragment: &str) -> Vec<String> {
        let mut celije = Vec::new();
        let mut ostatak = fragment;
        loop {
            let start = match (ostatak.find("<th"), ostatak.find("<td")) {
                (Some(th), Some(td)) => th.min(td),
                (Some(th), None) => th,
                (None, Some(td)) => td,
                (None, None) => break,
            };
            let tag = &ostatak[start..];
            let otvoren = tag.find('>').expect("a cell tag must close");
            let sadrzaj = &tag[otvoren + 1..];
            let kraj = sadrzaj.find("</").expect("a cell must close");
            celije.push(sadrzaj[..kraj].to_string());
            ostatak = &sadrzaj[kraj..];
        }
        celije
    }

    /// The heading row and the first body row of one section, as cell text.
    fn naslovi_i_prvi_red(html: &str, naslov: &str) -> (Vec<String>, Vec<String>) {
        let sekcija = sekcija_markup(html, naslov);
        let zaglavlje = sekcija
            .split("<thead>\n")
            .nth(1)
            .and_then(|rep| rep.split("</thead>").next())
            .expect("a lista table must have a heading row");
        let prvi_red = sekcija
            .split("<tbody>\n")
            .nth(1)
            .and_then(|rep| rep.split("</tr>").next())
            .expect("a lista table must have a body row");
        (celije_redom(zaglavlje), celije_redom(prvi_red))
    }

    /// Every table of one document, as `(broj naslova, broj ćelija po redu)`.
    /// Built by splitting the emitted markup rather than by asking the renderer,
    /// so a heading that disagrees with its rows has nowhere to hide.
    fn kolone_po_sekcijama(html: &str) -> Vec<(usize, Vec<usize>)> {
        html.split("<section class=\"lista\">")
            .skip(1)
            .map(|sekcija| {
                let sekcija = sekcija
                    .split("</section>")
                    .next()
                    .expect("a lista section must end");
                let naslovi = sekcija
                    .split("<thead>\n")
                    .nth(1)
                    .and_then(|rep| rep.split("</thead>").next())
                    .expect("a lista table must have a heading row");
                let telo = sekcija
                    .split("<tbody>\n")
                    .nth(1)
                    .and_then(|rep| rep.split("</tbody>").next())
                    .expect("a lista table must have a body");
                (
                    naslovi.matches("<th").count(),
                    telo.lines()
                        .filter(|red| red.starts_with("<tr>"))
                        .map(|red| red.matches("<td").count())
                        .collect(),
                )
            })
            .collect()
    }

    /// PoP čl. 8 st. 5 forbids releasing book data to the commission before the
    /// counted state is written and signed. A print IS a release, so the renderer
    /// withholds structurally — it must not become correct only because the view
    /// it was handed happened to be blind.
    #[test]
    fn the_phase_a_print_withholds_book_data_even_when_the_view_carries_it() {
        let mut view = view_with_lines();
        // A view that leaks: pretend the query layer regressed.
        view.knjigovodstvo_dostupno = true;
        for line in &mut view.linije {
            line.knjigovodstvena_kolicina_milli = Some(9_999);
            line.razlika_milli = Some(-1_234);
        }

        let html = render_popisna_lista(&company(), &view, PrintFaza::A);

        assert!(
            !html.contains("9.999"),
            "a book quantity reached the čl. 8 st. 5 print: {html}"
        );
        assert!(
            !html.contains("1.234"),
            "a razlika reached the čl. 8 st. 5 print: {html}"
        );
        // The two above are the plan's literals and they pin a grouped-integer
        // rendering of the raw milli values. They are not enough on their own:
        // this renderer prints quantities in Serbian notation, where 9_999 milli
        // is „9,999“ and −1_234 milli is „-1,234“, so a leak would sail past
        // both. The figures are therefore asserted absent in the form this
        // renderer would actually emit them in.
        for procureno in [format_kolicina(9_999), format_kolicina(-1_234)] {
            assert!(
                !html.contains(&procureno),
                "„{procureno}“ reached the čl. 8 st. 5 print: {html}"
            );
        }
        assert!(
            !html.to_lowercase().contains("knjigovodstven"),
            "the čl. 8 st. 5 print must not even carry the column heading: {html}"
        );
    }

    /// The structural half of the rule above, asserted structurally because no
    /// input can demonstrate it: a renderer that read the two fields and blanked
    /// them would pass every assertion above and still be the wrong thing — the
    /// value would exist, one refactor or one debug print away from the paper the
    /// commission is handed. `commands::popis::read_lines` withholds the same way
    /// and for the same reason, in two statements rather than one and a filter.
    #[test]
    fn the_phase_a_row_renderer_never_names_a_phase_b_field() {
        const SOURCE: &str = include_str!("popis_print.rs");

        let renderer = SOURCE
            .split("fn red_faza_a(")
            .nth(1)
            .expect("the čl. 8 st. 5 row renderer must exist")
            .split("\nfn ")
            .next()
            .expect("the čl. 8 st. 5 row renderer must end somewhere");

        for polje in ["knjigovodstvena_kolicina_milli", "razlika_milli"] {
            assert!(
                !renderer.contains(polje),
                "the čl. 8 st. 5 row renderer names `{polje}`, so the book side is only a filter \
                 away from the printed sheet"
            );
        }
    }

    #[test]
    fn the_phase_b_print_carries_the_book_column_and_the_razlika() {
        let view = computed_view();
        let html = render_popisna_lista(&company(), &view, PrintFaza::B);
        assert!(html.to_lowercase().contains("knjigovodstven"), "{html}");
        assert!(html.to_lowercase().contains("razlika"), "{html}");
        assert!(
            html.contains(&format_kolicina(8_000)),
            "the released book quantity is missing: {html}"
        );
        assert!(
            html.contains(&format_rsd_minor(249_900)),
            "the čl. 9 st. 1 t. 5 cena is missing: {html}"
        );
    }

    /// The čl. 8 st. 5 rule stated over the whole document instead of over one
    /// row: none of the Faza B headings and none of the six Faza B figures may
    /// appear on the čl. 8 st. 5 sheet, and every one of them must appear on the
    /// čl. 9 st. 3 sheet built from the same view.
    ///
    /// **Both halves matter.** The absence assertions alone pass on a renderer
    /// that prints nothing at all; the presence assertions alone pass on the
    /// renderer this task started from, which sent both phases down the Faza B
    /// path. Asserting the pair over one view is what pins the phase to the
    /// column set.
    ///
    /// The needles are built with the renderer's own formatters. A raw `8_000`
    /// or `1_749_300` would sail past a document that prints „8“ and
    /// „17.493,00“, and the bare digits would match the header, the article
    /// numbers and the PIB besides — so the whole `<td>` is the needle.
    #[test]
    fn the_phase_a_document_carries_no_phase_b_heading_and_no_derived_figure() {
        let view = computed_view();
        let faza_a = render_popisna_lista(&company(), &view, PrintFaza::A);
        let faza_b = render_popisna_lista(&company(), &view, PrintFaza::B);

        for naslov in [
            "Knjigovodstvena količina",
            "Razlika (višak/manjak)",
            "Cena",
            "Vrednost po popisu",
            "Vrednost po knjigama",
            "Vrednosna razlika",
        ] {
            assert!(
                faza_b.contains(naslov),
                "the čl. 9 st. 3 sheet lost the column „{naslov}“: {faza_b}"
            );
            assert!(
                !faza_a.contains(naslov),
                "the column „{naslov}“ reached the čl. 8 st. 5 sheet: {faza_a}"
            );
        }

        let linija = view.linije.first().expect("the fixture counts two stavke");
        let cena = linija.cena_minor.expect("the fixture prices its stavke");
        let knjigovodstvena = linija
            .knjigovodstvena_kolicina_milli
            .expect("the fixture releases the book side");
        let razlika = linija.razlika_milli.expect("the fixture derives a razlika");

        for celija in [
            kolicina_celija(Some(knjigovodstvena)),
            kolicina_celija(Some(razlika)),
            iznos_celija(Some(cena)),
            iznos_celija(vrednost_minor(linija.stvarna_kolicina_milli, cena)),
            iznos_celija(vrednost_minor(knjigovodstvena, cena)),
            iznos_celija(vrednost_minor(razlika, cena)),
        ] {
            assert!(
                faza_b.contains(&celija),
                "the čl. 9 st. 3 sheet is missing the cell {celija}: {faza_b}"
            );
            assert!(
                !faza_a.contains(&celija),
                "the cell {celija} reached the čl. 8 st. 5 sheet: {faza_a}"
            );
        }
    }

    /// A heading that promises one column set over rows that carry another is a
    /// sheet the commission signs while reading a figure under a name that is not
    /// its own — and it is the shape the two defects of this task took, one on
    /// each side. Checked on both phases, across a lista whose money column is
    /// part of the count (čl. 11 st. 1), one whose money column is not (čl. 9
    /// st. 1 t. 5) and the unclassified bucket.
    #[test]
    fn every_row_carries_exactly_as_many_cells_as_its_heading_promises() {
        let mut view = computed_view();
        view.linije
            .push(linija(3, PopisLista::Gotovina, "Novčanica 1.000"));
        view.linije
            .push(linija(4, PopisLista::Potrazivanja, "Potraživanje od kupca"));
        let mut nepoznata = linija(5, PopisLista::Roba, "Nešto nerazvrstano");
        nepoznata.lista_vrsta = "izmisljena_lista".to_string();
        view.linije.push(nepoznata);

        for faza in [PrintFaza::A, PrintFaza::B] {
            let html = render_popisna_lista(&company(), &view, faza);
            let sekcije = kolone_po_sekcijama(&html);
            assert_eq!(
                sekcije.len(),
                4,
                "three liste and the unclassified bucket: {html}"
            );
            for (naslova, redovi) in sekcije {
                assert!(!redovi.is_empty(), "a printed table with no row: {html}");
                for celija in redovi {
                    assert_eq!(
                        celija, naslova,
                        "a row of {celija} cells under {naslova} headings ({faza:?}): {html}"
                    );
                }
            }
        }
    }

    /// A cell permutation changes neither the heading set nor the cell count, so
    /// every other assertion in this module survives it — and swapping „Vrednost
    /// po popisu“ with „Vrednost po knjigama“ prints a manjak as a višak on a
    /// sheet the commission signs. Order is therefore pinned literally: the
    /// heading row and the first data row of one section are read back in the
    /// order they were emitted and compared, index by index, against the čl. 9
    /// st. 1 t. 1–6 sequence written out here by hand.
    #[test]
    fn the_cells_of_a_row_sit_in_the_order_their_headings_promise() {
        let view = computed_view();
        let prva = view.linije.first().expect("the fixture counts two stavke");
        let cena = prva.cena_minor.expect("the fixture prices its stavke");
        let knjigovodstvena = prva
            .knjigovodstvena_kolicina_milli
            .expect("the fixture releases the book side");
        let razlika = prva.razlika_milli.expect("the fixture derives a razlika");
        let iznos = |minor: Option<i64>| minor.map_or_else(|| "—".to_string(), format_rsd_minor);

        let (naslovi, celije) = naslovi_i_prvi_red(
            &render_popisna_lista(&company(), &view, PrintFaza::B),
            PopisLista::Roba.naziv(),
        );
        assert_eq!(
            naslovi,
            [
                "Šifra",
                "Naziv",
                "Vrsta",
                "Jedinica mere",
                "Stvarna količina",
                "Knjigovodstvena količina",
                "Razlika (višak/manjak)",
                "Cena",
                "Vrednost po popisu",
                "Vrednost po knjigama",
                "Vrednosna razlika",
                "Bliži opis",
            ]
        );
        assert_eq!(
            celije,
            [
                "ART-1".to_string(),
                "Košulja".to_string(),
                "Ženska konfekcija".to_string(),
                "kom".to_string(),
                format_kolicina(prva.stvarna_kolicina_milli),
                format_kolicina(knjigovodstvena),
                format_kolicina(razlika),
                format_rsd_minor(cena),
                iznos(vrednost_minor(prva.stvarna_kolicina_milli, cena)),
                iznos(vrednost_minor(knjigovodstvena, cena)),
                iznos(vrednost_minor(razlika, cena)),
                "Polica A2".to_string(),
            ],
            "the čl. 9 st. 3 row must sit under the headings it was printed with"
        );
        assert_eq!(naslovi.len(), celije.len());

        // Čl. 11 st. 1 — the apoen sits after the counted količina and before the
        // bliži opis, and swapping the two figures would misprice the till.
        let mut gotovina = prazan_view();
        let mut novcanica = linija(3, PopisLista::Gotovina, "Novčanica 1.000");
        novcanica.stvarna_kolicina_milli = 5_000;
        novcanica.cena_minor = Some(100_000);
        gotovina.linije = vec![novcanica];

        let (naslovi, celije) = naslovi_i_prvi_red(
            &render_popisna_lista(&company(), &gotovina, PrintFaza::A),
            PopisLista::Gotovina.naziv(),
        );
        assert_eq!(
            naslovi,
            [
                "Šifra",
                "Naziv",
                "Vrsta",
                "Jedinica mere",
                "Stvarna količina",
                "Apoen",
                "Bliži opis",
            ]
        );
        assert_eq!(
            celije,
            [
                "ART-3".to_string(),
                "Novčanica 1.000".to_string(),
                "Ženska konfekcija".to_string(),
                "kom".to_string(),
                format_kolicina(5_000),
                format_rsd_minor(100_000),
                "Polica A2".to_string(),
            ],
            "the čl. 8 st. 5 row must sit under the headings it was printed with"
        );
        assert_eq!(naslovi.len(), celije.len());
    }

    /// PoP čl. 6 st. 1 lets the popis of a mikro pravno lice or a preduzetnik be
    /// taken by **one person**, and čl. 6 st. 2 extends the commission provisions
    /// to that person only *shodno* — they are not a komisija of one. A sheet
    /// headed „Potpisi članova komisije za popis“ over a signer whose own uloga
    /// line reads „jedno lice koje vrši popis“ misdescribes its own signer, and a
    /// napomena that stops the čl. 9 st. 3 quotation one limb early states the
    /// governing rule in a form that excludes them. Both phases, because the
    /// signature block and the napomena are printed on both.
    #[test]
    fn a_single_person_popis_is_never_called_a_commission_on_its_own_sheet() {
        for faza in [PrintFaza::A, PrintFaza::B] {
            let html = render_popisna_lista(&company(), &view_jedno_lice(), faza);

            assert!(
                html.contains("Potpis lica koje vrši popis (PoP čl. 6 st. 1)"),
                "the signature block must be headed for the person who signs it ({faza:?}): {html}"
            );
            assert!(
                !html.contains("Potpisi članova komisije za popis"),
                "a komisija was named over a signer who is not one ({faza:?}): {html}"
            );
            assert!(
                !html.contains("član komisije za popis"),
                "the single person was cast as a član komisije ({faza:?}): {html}"
            );

            // Wherever the sheet still names the komisija — and it must, because
            // both articles are quoted — the čl. 6 limb is in the same block.
            komisija_nikad_bez_cl_6_limba(&html, &format!("{faza:?}"));
        }

        let faza_b = render_popisna_lista(&company(), &view_jedno_lice(), PrintFaza::B);
        assert!(
            faza_b.contains("odnosno jedno lice iz člana 6. ovog pravilnika"),
            "the čl. 9 st. 3 quotation must carry the limb that reaches the single person: {faza_b}"
        );
    }

    /// A stavka whose lista nobody could decode keeps the čl. 8 st. 5 columns even
    /// on the čl. 9 st. 3 sheet, because [`iznos_kolona`] reads the meaning of the
    /// money column off the lista and there is none to read: the figure would be
    /// printed under a heading that misnames it. That is a deliberate withholding
    /// and not an omission, so the sheet says it out loud — otherwise a stavka
    /// with no razlika printed and a stavka with a razlika of zero read alike.
    #[test]
    fn an_unclassified_stavka_keeps_the_count_columns_on_the_computed_sheet_and_says_so() {
        let mut view = computed_view();
        let mut nepoznata = linija(9, PopisLista::Roba, "Nešto nerazvrstano");
        nepoznata.lista_vrsta = "izmisljena_lista".to_string();
        nepoznata.knjigovodstvena_kolicina_milli = Some(4_000);
        nepoznata.razlika_milli = Some(3_000);
        nepoznata.cena_minor = Some(150_000);
        view.linije = vec![nepoznata];

        let html = render_popisna_lista(&company(), &view, PrintFaza::B);

        assert!(html.contains("Nešto nerazvrstano"), "{html}");
        for procureno in [
            kolicina_celija(Some(4_000)),
            kolicina_celija(Some(3_000)),
            iznos_celija(Some(150_000)),
        ] {
            assert!(
                !html.contains(&procureno),
                "{procureno} was printed under a heading nobody could name: {html}"
            );
        }
        assert!(
            html.contains("nije iskazan obračun jer se ne zna kojoj popisnoj listi pripadaju"),
            "the čl. 9 st. 3 sheet must name what it left out: {html}"
        );
    }

    /// The čl. 9 st. 3 sheet exists only after the čl. 8 st. 5 potpis, and from
    /// that moment `commands::popis::save_line` refuses every move of
    /// `lista_vrsta`: on `computed` through `PotpisanaStavka::pomerena_polja`,
    /// which names it „vrsta popisne liste“, and on `counted_signed`,
    /// `computed_signed` and `posted` outright. So a napomena telling the operator
    /// to reclassify the stavka instructs an action that is refused 100 % of the
    /// time the sentence is printed — the „promise the code does not implement“
    /// class this repository shipped six times. The paper must state the engine's
    /// own remedy instead, in the engine's own words.
    #[test]
    fn the_unclassified_napomena_names_the_remedy_the_engine_actually_offers() {
        let mut view = computed_view();
        let mut nepoznata = linija(9, PopisLista::Roba, "Nešto nerazvrstano");
        nepoznata.lista_vrsta = "izmisljena_lista".to_string();
        view.linije = vec![nepoznata];

        let html = render_popisna_lista(&company(), &view, PrintFaza::B);

        assert!(
            !html.to_lowercase().contains("razvrstajte"),
            "the sheet tells the operator to reclassify a stavka save_line refuses to move: {html}"
        );
        assert!(
            !html.to_lowercase().contains("odštampajte"),
            "the sheet asks for a reprint of a document it does not itself produce: {html}"
        );
        assert!(
            html.contains("ispravka se sprovodi novim popisom"),
            "the sheet must carry the remedy save_line names: {html}"
        );
        assert!(
            html.contains("PoP čl. 8 st. 5, čl. 9 st. 1 t. 1"),
            "the freeze must be cited where it is asserted: {html}"
        );
    }

    /// Req. 32 — the header identifies the obveznik and the outlet, because a
    /// signed sheet with no header identifies nothing.
    #[test]
    fn the_header_carries_the_obveznik_the_outlet_and_the_list_number() {
        let html = render_popisna_lista(&company(), &view_with_lines(), PrintFaza::A);
        for needle in [
            "PIB",
            "Matični broj",
            "Maloprodajni objekat",
            "Broj liste",
            "Datum popisa",
        ] {
            assert!(html.contains(needle), "header missing „{needle}“: {html}");
        }
        for vrednost in [
            "Butik Vantum pr Novi Pazar",
            "123456789",
            "63012345",
            "Butik Centar",
            "2026-12-31",
        ] {
            assert!(
                html.contains(vrednost),
                "header missing the value „{vrednost}“: {html}"
            );
        }
    }

    /// Req. 30 — the commission signs. A print with no signature line cannot
    /// discharge čl. 8 st. 5 or čl. 9 st. 3.
    #[test]
    fn every_commission_member_gets_a_signature_line() {
        let view = view_with_commission(&["Mira Marković", "Petar Petrović"]);
        let html = render_popisna_lista(&company(), &view, PrintFaza::A);
        assert!(html.contains("Mira Marković"), "{html}");
        assert!(html.contains("Petar Petrović"), "{html}");
        assert_eq!(
            html.matches("class=\"linija\"").count(),
            2,
            "each member needs a ruled line of their own: {html}"
        );
        assert!(
            html.contains("predsednik komisije za popis")
                && html.contains("član komisije za popis"),
            "the uloga belongs beside the name: {html}"
        );
    }

    /// §2c — no obrazac is prescribed for the popisna lista. Claiming one would
    /// misstate the law in the document itself.
    #[test]
    fn the_print_never_calls_itself_a_prescribed_form() {
        let html = render_popisna_lista(&company(), &view_with_lines(), PrintFaza::A);
        assert!(!html.to_lowercase().contains("propisani obrazac"), "{html}");
        assert!(!html.to_lowercase().contains("obrazac br"), "{html}");
    }

    /// Each of the six liste is its own document section with its own article,
    /// because they are separate lists by law, not tabs of one.
    #[test]
    fn each_present_lista_is_its_own_section_naming_its_article() {
        let view = view_with_every_lista();
        let html = render_popisna_lista(&company(), &view, PrintFaza::A);
        for article in [
            "čl. 10 st. 3",
            "čl. 10 st. 4",
            "čl. 11 st. 1",
            "čl. 12 st. 2",
            "čl. 2 st. 5",
        ] {
            assert!(
                html.contains(article),
                "missing the section for {article}: {html}"
            );
        }
        assert_eq!(
            html.matches("<section class=\"lista\">").count(),
            PopisLista::ALL.len(),
            "every present lista is a section of its own: {html}"
        );
    }

    /// A lista with no stavke is not a section — čl. 10–12 require a lista where
    /// the category is present, and a printed empty table invites a signature over
    /// nothing.
    #[test]
    fn a_lista_with_no_stavke_gets_no_section() {
        let html = render_popisna_lista(&company(), &view_with_lines(), PrintFaza::A);
        assert_eq!(
            html.matches("<section class=\"lista\">").count(),
            1,
            "only the lista that has stavke prints: {html}"
        );
        assert!(
            !html.contains("gotovina po apoenima"),
            "an empty lista printed anyway: {html}"
        );
    }

    /// Čl. 11 st. 1 and čl. 12 st. 2 put the commission's own figure in the money
    /// column, so čl. 8 st. 5 does not keep it back — and the heading has to say
    /// which of the three things that column holds.
    #[test]
    fn the_counted_money_column_prints_during_the_count_and_the_cena_does_not() {
        let mut view = prazan_view();
        view.linije = vec![
            linija(1, PopisLista::Gotovina, "Novčanica 1.000"),
            linija(2, PopisLista::Potrazivanja, "Potraživanje od dobavljača"),
            linija(3, PopisLista::Roba, "Košulja"),
        ];
        for line in &mut view.linije {
            line.cena_minor = Some(100_000);
        }

        let html = render_popisna_lista(&company(), &view, PrintFaza::A);

        assert!(
            html.contains(">Apoen<"),
            "čl. 11 st. 1 apoen missing: {html}"
        );
        assert!(
            html.contains(">Iznos<"),
            "čl. 12 st. 2 iznos missing: {html}"
        );
        assert!(
            !html.contains(">Cena<"),
            "the čl. 9 st. 1 t. 5 cena is not part of the count: {html}"
        );
    }

    /// A stavka the v20 CHECK should have made impossible is still a stavka the
    /// commission counted. Dropping it would make the printed sheet say something
    /// untrue about the stvarno stanje.
    #[test]
    fn a_stavka_on_an_unknown_lista_is_printed_rather_than_dropped() {
        let mut view = prazan_view();
        let mut nepoznata = linija(1, PopisLista::Roba, "Nešto nerazvrstano");
        nepoznata.lista_vrsta = "izmisljena_lista".to_string();
        view.linije = vec![nepoznata];

        let html = render_popisna_lista(&company(), &view, PrintFaza::A);

        assert!(html.contains("Nešto nerazvrstano"), "{html}");
        assert!(
            html.contains("nisu razvrstane"),
            "the section must say what it is: {html}"
        );
    }

    /// Every dynamic value goes through the escape, because a naziv is operator
    /// input and this document is opened in a browser to be printed.
    #[test]
    fn dynamic_values_are_escaped() {
        let mut view = view_with_lines();
        view.linije[0].naziv = "<script>alert('x')</script>".to_string();
        view.komisija = vec![clan(1, "Mira & Petar", "predsednik")];

        let html = render_popisna_lista(&company(), &view, PrintFaza::A);

        assert!(!html.contains("<script>"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(html.contains("Mira &amp; Petar"), "{html}");
    }

    /// The Serbian notation, pinned: a decimal comma, a thousands dot, and no
    /// „7,000“ where the shop counted seven pieces.
    #[test]
    fn quantities_and_amounts_are_written_the_serbian_way() {
        assert_eq!(format_kolicina(7_000), "7");
        assert_eq!(format_kolicina(9_999), "9,999");
        assert_eq!(format_kolicina(-1_234), "-1,234");
        assert_eq!(format_kolicina(1_234_567), "1.234,567");
        assert_eq!(format_kolicina(0), "0");
        assert_eq!(format_rsd_minor(900_000), "9.000,00");
        assert_eq!(format_rsd_minor(-1_050), "-10,50");
    }

    // -----------------------------------------------------------------
    // Req. 35 — the odluka o popisu and the plan rada
    // -----------------------------------------------------------------

    /// The odluka appoints the people and orders the popis, so it has to name
    /// both. A decision that identifies neither the count it orders nor the
    /// persons it appoints is not the čl. 4 st. 2 / čl. 5 act at all.
    #[test]
    fn the_odluka_names_the_popis_it_orders_and_every_member_of_the_commission() {
        let mut view = view_with_commission(&["Mira Marković", "Petar Petrović"]);
        view.odluka_ref = Some("Odluka 3/2026".into());

        let html = render_odluka(&company(), &view);

        for needle in [
            "Butik Vantum pr Novi Pazar",
            "123456789",
            "63012345",
            "Butik Centar",
            "2026-12-31",
            "2026-01-01",
            "Odluka 3/2026",
        ] {
            assert!(
                html.contains(needle),
                "the odluka is missing „{needle}“: {html}"
            );
        }
        assert!(
            html.contains(PopisVrsta::Godisnji.naziv()),
            "the odluka must say which popis it orders: {html}"
        );
        for (ime, uloga) in [
            ("Mira Marković", "predsednik komisije za popis"),
            ("Petar Petrović", "član komisije za popis"),
        ] {
            assert!(html.contains(ime), "the odluka is missing „{ime}“: {html}");
            assert!(
                html.contains(uloga),
                "the odluka must name the uloga beside the person: {html}"
            );
        }
    }

    /// Čl. 8 st. 1 makes the plan rada the commission's own schedule, so the
    /// document prints what the shop wrote rather than a shape of our own.
    #[test]
    fn the_plan_rada_renders_the_schedule_the_shop_supplied() {
        let mut view = view_with_lines();
        view.plan_rada_json = Some(PLAN_RADA_JSON.to_string());

        let html = render_plan_rada(&company(), &view);

        for needle in [
            "raspored",
            "30.12.2026. od 8.00 do 14.00",
            "zaduženja",
            "Mira: roba",
            "Petar: gotovina",
        ] {
            assert!(
                html.contains(needle),
                "the plan rada dropped „{needle}“: {html}"
            );
        }
        assert!(
            !html.contains(&escape_html(PLAN_RADA_JSON)),
            "a signed document must not carry the shop's plan as raw JSON: {html}"
        );
        assert!(!html.contains("nije unet"), "{html}");
    }

    /// The field v20 named `plan_rada_json` is a plain text input in front of the
    /// operator and nothing validates it as JSON, so most shops will type a
    /// sentence. Printing it verbatim is the only truthful thing to do with it —
    /// a document that silently dropped what it could not parse would report the
    /// commission had no plan when it had one.
    #[test]
    fn the_plan_rada_prints_a_schedule_that_is_not_json_as_the_shop_typed_it() {
        let mut view = view_with_lines();
        view.plan_rada_json = Some("Brojanje 30.12. od 8h: Mira roba, Petar gotovina".into());

        let html = render_plan_rada(&company(), &view);

        assert!(
            html.contains("Brojanje 30.12. od 8h: Mira roba, Petar gotovina"),
            "the shop's own words belong on its own plan: {html}"
        );
        assert!(!html.contains("nije unet"), "{html}");
    }

    /// An empty schedule table on a signed document reads as „there is no work to
    /// do“. The absence is stated instead — čl. 8 st. 1 requires the plan, so a
    /// missing one is a fact the person about to sign needs to see.
    #[test]
    fn the_plan_rada_says_plainly_when_the_shop_supplied_no_schedule() {
        for prazno in [None, Some(String::new()), Some("   ".to_string())] {
            let mut view = view_with_lines();
            view.plan_rada_json = prazno.clone();

            let html = render_plan_rada(&company(), &view);

            assert!(
                html.contains("Plan rada nije unet u aplikaciju"),
                "the plan rada must say the schedule is missing ({prazno:?}): {html}"
            );
            assert!(
                html.contains("čl. 8 st. 1"),
                "…and cite what requires it ({prazno:?}): {html}"
            );
            assert!(
                !html.contains("class=\"raspored\""),
                "an empty schedule must not be printed as a table ({prazno:?}): {html}"
            );
        }
    }

    /// Čl. 8 st. 2 is an act by a named person at a known time. Both documents
    /// report it, because both are handed to the same people.
    #[test]
    fn both_generated_documents_carry_the_recorded_cl_8_st_2_approval() {
        let view = view_odobren();
        for (dokument, html) in [
            ("odluka", render_odluka(&company(), &view)),
            ("plan rada", render_plan_rada(&company(), &view)),
        ] {
            assert!(
                html.contains("Miloš Đurđević"),
                "the {dokument} must name who approved the plan: {html}"
            );
            assert!(
                html.contains("2026-12-30T08:15:00Z"),
                "…and when it was recorded: {html}"
            );
            assert!(
                html.contains("čl. 8 st. 2"),
                "…under the provision that requires it: {html}"
            );
            assert!(
                !html.contains("nije odobren"),
                "an approved plan must not read as unapproved on the {dokument}: {html}"
            );
        }
    }

    /// The false-record class this project keeps catching: an approval nobody
    /// performed. Nothing here may fill the approver in — not the company name,
    /// not the commission's president, not the person who pressed print.
    #[test]
    fn neither_generated_document_claims_an_approval_nobody_recorded() {
        let view = view_with_lines();
        for (dokument, html) in [
            ("odluka", render_odluka(&company(), &view)),
            ("plan rada", render_plan_rada(&company(), &view)),
        ] {
            assert!(
                html.contains("Plan rada nije odobren"),
                "the {dokument} must say the plan is unapproved: {html}"
            );
            assert!(
                !html.contains("Odobrio"),
                "the {dokument} names an approver nobody recorded: {html}"
            );
        }
    }

    /// v21's CHECK pairs the two columns, but the renderer is handed a view and
    /// not a row — a cached view, a future caller or a regressed query could
    /// carry half a record. Half an approval is not an approval (čl. 8 st. 2), so
    /// the document reports it as none.
    #[test]
    fn half_an_approval_is_printed_as_no_approval() {
        for (odobrio, at) in [
            (Some("Miloš Đurđević".to_string()), None),
            (None, Some("2026-12-30T08:15:00Z".to_string())),
        ] {
            let mut view = view_with_lines();
            view.plan_rada_odobrio = odobrio.clone();
            view.plan_rada_odobreno_at = at.clone();

            let html = render_plan_rada(&company(), &view);

            assert!(
                html.contains("Plan rada nije odobren"),
                "half a record ({odobrio:?}, {at:?}) was printed as an approval: {html}"
            );
            assert!(!html.contains("Odobrio"), "{html}");
        }
    }

    /// Čl. 8 st. 1 puts the plan rada in the komisija's hands, not in this
    /// application's: `plan_rada_json` is an optional field on the open form,
    /// nothing else in the crate ever writes it, and a shop that keeps its
    /// schedule on paper has broken nothing. So the čl. 8 st. 2 approval of such
    /// a plan is a real record and is not refused — but the document may not
    /// print „Plan rada je odobren“ a few lines under „Plan rada nije unet u
    /// aplikaciju“ with nothing connecting them, because the sheet somebody signs
    /// then asserts an approval over a schedule that was blank when it was
    /// stamped. The approval says what it was recorded over.
    #[test]
    fn an_approval_over_a_schedule_the_application_does_not_hold_says_so_on_both_documents() {
        for prazno in [
            None,
            Some(String::new()),
            Some("   ".to_string()),
            Some("{}".to_string()),
        ] {
            let mut view = view_odobren();
            view.plan_rada_json = prazno.clone();

            for (dokument, html) in [
                ("odluka", render_odluka(&company(), &view)),
                ("plan rada", render_plan_rada(&company(), &view)),
            ] {
                let odobrenje = odobrenje_markup(&html);
                assert!(
                    odobrenje.contains("Plan rada je odobren"),
                    "the čl. 8 st. 2 record is real and still prints ({dokument}, {prazno:?}): \
                     {html}"
                );
                assert!(
                    odobrenje.contains(
                        "U aplikaciji je evidentirano odobrenje, a ne i sadržina plana rada"
                    ),
                    "an approval over a schedule the app does not hold must say so ({dokument}, \
                     {prazno:?}): {html}"
                );
            }

            let plan = render_plan_rada(&company(), &view);
            assert!(
                plan.contains("Plan rada nije unet u aplikaciju"),
                "the schedule section still reports the absence ({prazno:?}): {plan}"
            );
        }

        // …and the qualification is not printed over a schedule the app does hold,
        // which would be the same defect facing the other way.
        for (dokument, html) in [
            ("odluka", render_odluka(&company(), &view_odobren())),
            ("plan rada", render_plan_rada(&company(), &view_odobren())),
        ] {
            assert!(
                !html.contains("a ne i sadržina plana rada"),
                "the app holds this schedule, so the {dokument} must not disown it: {html}"
            );
        }
    }

    /// Nothing in this cycle writes `odluka_doneta_at`, so on every popis the
    /// pilot has it is NULL — and a generated odluka that dated itself would date
    /// an act nobody recorded. It says so and leaves the line to the pen; when a
    /// later verb does record one, the recorded date prints instead.
    ///
    /// The assertions read the header cell itself rather than sweeping the whole
    /// document: the odobrenje block a few lines below carries „nije evidentirano
    /// odobrenje“, and a needle that matched both would report the date as absent
    /// on an odluka that printed one.
    #[test]
    fn the_odluka_leaves_the_date_of_issue_to_the_paper_until_one_is_recorded() {
        let html = render_odluka(&company(), &view_with_lines());
        assert!(
            html.contains("<th>Datum donošenja</th><td>nije evidentiran u aplikaciji"),
            "an odluka with no recorded date must say so in its own cell: {html}"
        );

        let mut view = view_with_lines();
        view.odluka_doneta_at = Some("2026-12-20T10:00:00Z".into());
        let html = render_odluka(&company(), &view);
        assert!(
            html.contains("<th>Datum donošenja</th><td>2026-12-20T10:00:00Z</td>"),
            "a recorded date belongs in the cell that names it: {html}"
        );
        assert!(
            !html.contains("<th>Datum donošenja</th><td>nije evidentiran"),
            "{html}"
        );
    }

    /// Req. 40 / PoP čl. 5 st. 1 — a person who handles the assets may not be
    /// appointed, and the odluka is the appointment. The document that carries
    /// the appointment carries the objection with it; it does not refuse to
    /// print, because §6 R-5 leaves the čl. 6 st. 2 extension unresolved and the
    /// module warns rather than blocks.
    #[test]
    fn the_odluka_carries_the_cl_5_st_1_objection_without_refusing_to_print() {
        let mut view = view_with_lines();
        view.komisija = vec![
            clan_rukovalac(1, "Mira Marković", "predsednik"),
            clan(2, "Petar Petrović", "clan"),
        ];

        let html = render_odluka(&company(), &view);

        assert!(html.contains("Mira Marković"), "{html}");
        assert!(
            html.contains("čl. 5 st. 1"),
            "the odluka must cite what excludes a person who handles the assets: {html}"
        );
        assert!(
            html.contains("Petar Petrović"),
            "the rest of the roster still prints: {html}"
        );
    }

    /// The čl. 6 st. 1 single person is the pilot's own shape. An „odluka o
    /// obrazovanju komisije“ over one appointee, or a plan rada headed for a body
    /// the shop does not have, states the law in a form that excludes the person
    /// it governs — the defect Deviation 1b fixed on the popisna lista, which
    /// would otherwise walk straight back in through these two documents.
    #[test]
    fn a_single_person_popis_is_never_called_a_commission_on_the_generated_documents() {
        let view = view_jedno_lice();
        for (dokument, html) in [
            ("odluka", render_odluka(&company(), &view)),
            ("plan rada", render_plan_rada(&company(), &view)),
        ] {
            assert!(
                html.contains("jedno lice") || html.contains("jednog lica"),
                "the {dokument} must name the person it is actually about: {html}"
            );
            assert!(
                !html.contains("član komisije za popis"),
                "the single person was cast as a član komisije on the {dokument}: {html}"
            );
            assert!(
                !html.contains("OBRAZOVANJU KOMISIJE"),
                "the {dokument} forms a komisija the popis does not have: {html}"
            );

            komisija_nikad_bez_cl_6_limba(&html, dokument);
        }
    }

    /// The same reading, on the state the pilot actually reaches these documents
    /// in. `open_popis` imposes no non-empty roster and `OpenPopisRequest.komisija`
    /// is `#[serde(default)]`, and the odluka **is** the appointment — so printing
    /// it before anybody is appointed is the primary use case, not an edge. A
    /// roster that mixes the two ulogas says as little as an empty one. Neither
    /// may be resolved into „komisija“: that would tell a preduzetnik, on the
    /// paper that appoints him, that the popis belongs to a body he does not have,
    /// and it drops the čl. 6 st. 1 limb from the title, the operative sentence
    /// and the roster heading at once. The popisna lista already carries both
    /// limbs in this state (`potpisni_naslov`); these two documents did not.
    #[test]
    fn a_popis_whose_roster_is_not_recorded_is_declared_neither_a_komisija_nor_a_single_person() {
        let mut mesovit = view_with_lines();
        mesovit.komisija = vec![
            clan(1, "Vlasnik lično", "jedno_lice"),
            clan(2, "Petar Petrović", "clan"),
        ];

        for (stanje, view) in [
            ("prazan sastav", prazan_view()),
            ("mešovit sastav", mesovit),
        ] {
            for (dokument, html) in [
                ("odluka", render_odluka(&company(), &view)),
                ("plan rada", render_plan_rada(&company(), &view)),
            ] {
                komisija_nikad_bez_cl_6_limba(&html, &format!("{dokument}, {stanje}"));
                assert!(
                    html.to_lowercase().contains("jednog lica")
                        || html.to_lowercase().contains("jedno lice"),
                    "the {dokument} must leave the čl. 6 st. 1 limb open ({stanje}): {html}"
                );
            }
        }
    }

    /// §2c ii — Pravilnik 89/2020 runs čl. 1–16 with no prilog, so it prescribes
    /// no form for these two documents either. Claiming one would misstate the
    /// law in the document itself.
    #[test]
    fn neither_generated_document_calls_itself_a_prescribed_form() {
        for html in [
            render_odluka(&company(), &view_odobren()),
            render_plan_rada(&company(), &view_odobren()),
        ] {
            assert!(!html.to_lowercase().contains("propisani obrazac"), "{html}");
            assert!(!html.to_lowercase().contains("obrazac br"), "{html}");
        }
    }

    /// Every dynamic value on these documents is operator input too — the
    /// commission's names, the shop's own plan rada, and the name of whoever
    /// approved it — and both are opened in a browser to be printed.
    #[test]
    fn the_generated_documents_escape_dynamic_values() {
        let mut view = view_odobren();
        view.komisija = vec![clan(1, "Mira & Petar", "predsednik")];
        view.plan_rada_json = Some(r#"{"zadatak":"<script>alert('x')</script>"}"#.into());
        view.plan_rada_odobrio = Some("<b>Vlasnik</b>".into());
        view.odluka_ref = Some("Odluka <3>/2026".into());

        for html in [
            render_odluka(&company(), &view),
            render_plan_rada(&company(), &view),
        ] {
            assert!(!html.contains("<script>"), "{html}");
            assert!(!html.contains("<b>Vlasnik</b>"), "{html}");
            assert!(html.contains("Mira &amp; Petar"), "{html}");
        }
        assert!(
            render_plan_rada(&company(), &view).contains("&lt;script&gt;"),
            "the escaped form still has to be visible on the plan rada"
        );
    }
}
