//! The ZZPL čl. 47 evidencija radnji obrade, generated rather than hand-kept
//! (req. 28).
//!
//! **Why it is generated.** Čl. 47 st. 1 asks the rukovalac to record seven
//! things about every radnja obrade, and t. 6 asks for the *rok posle čijeg
//! isteka se brišu određene vrste podataka o ličnosti* — a period **per vrsta
//! podataka**, not one blanket sentence for the shop. A hand-kept document
//! drifts from the program the moment a retention class moves, and a register
//! that claims a period the till does not apply is worse than none: it is a
//! written statement to the Poverenik that the shop cannot stand behind. So
//! every rok on this register is read out of the shared `retention_policies`
//! table at generation time, and the row carries the class key beside the prose
//! so the claim can be traced back to the row that governs it.
//!
//! **What this register is, and what it is not.** It is the **rukovalac's**
//! record under čl. 47 st. 1. The obrađivač record under st. 4 is a different
//! record with four elements instead of seven, and it is kept by the obrađivač
//! — Actaer — about its own processing. This module never generates it and the
//! export says so, because a shop that files the processor's record as its own
//! has filed the wrong document.
//!
//! **The register is kept `trajno`** (čl. 47 st. 7), and this is the one place
//! in the ZZPL trio where `trajno` is the correct answer. The audit log (req. 6)
//! and the breach log (req. 49) both have **no** prescribed period, and copying
//! st. 7 onto either of them would put the product in permanent breach of
//! storage limitation — so `no_log_without_a_prescribed_period_is_printed_as_trajno`
//! is a test and not a comment.
//!
//! **No fine figure appears here.** `legal.rs` is the only module allowed to
//! hold one, and this document goes to the Poverenik, where a number reads as
//! the shop's own admission of its exposure.

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::State;

use crate::app_error::{AppError, CommandError};
use crate::audit::{AuditAction, AuditDraft, AuditObjectType};
use crate::clock::utc_now;
use crate::commands::auth::require_admin;
use crate::commands::reports::ExportedFile;
use crate::commands::settings::CompanySettings;
use crate::retention::RecordClass;
use crate::state::AppState;

/// One generated radnja obrade, as `processing_activities` stores it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessingActivity {
    pub id: i64,
    /// The stable key of the generated radnja. Regeneration is an upsert on it.
    pub kljuc: String,
    /// St. 1 t. 1.
    pub rukovalac_naziv: String,
    pub rukovalac_kontakt: Option<String>,
    /// St. 1 t. 2.
    pub svrha_obrade: String,
    /// St. 1 t. 3.
    pub vrsta_lica: String,
    pub vrsta_podataka: String,
    /// St. 1 t. 4.
    pub vrsta_primalaca: Option<String>,
    /// St. 1 t. 5.
    pub prenos_u_druge_drzave: Option<String>,
    pub mere_zastite_prenosa: Option<String>,
    /// St. 1 t. 6 — the period, per category.
    pub rok_cuvanja: Option<String>,
    /// The `retention_policies` class the rok was read from, or `None` where the
    /// app applies no configured period to this radnja.
    pub retention_record_class: Option<RecordClass>,
    /// St. 1 t. 7 — the general description of the čl. 50 st. 1 measures.
    pub opis_mera_zastite: Option<String>,
    pub updated_at: String,
}

/// One radnja obrade as this crate describes it, before the shop's own
/// configuration is folded in.
///
/// The prose is `&'static str` on purpose. These are the shop's statements to
/// the Poverenik about what it processes and why, and they belong in review-able
/// source beside the code that performs the processing — not in a table an
/// operator can quietly edit into agreement with whatever the till happens to do.
struct Template {
    /// The stable upsert key. Never rendered to the operator and never sent to
    /// `audit_events`, whose object ids are numeric.
    kljuc: &'static str,
    /// St. 1 t. 2.
    svrha_obrade: &'static str,
    /// St. 1 t. 3, first half.
    vrsta_lica: &'static str,
    /// St. 1 t. 3, second half.
    vrsta_podataka: &'static str,
    /// St. 1 t. 4.
    vrsta_primalaca: &'static str,
    /// St. 1 t. 6 — *why* the period is what it is. The period the app actually
    /// applies is appended from `retention_policies` at generation time.
    rok_osnov: &'static str,
    /// The class the applied period is read from. `None` where the app applies
    /// no configured period to this radnja, which the rok then says in words.
    retention: Option<RecordClass>,
    /// St. 1 t. 7, the part specific to this radnja. The shop-wide measures are
    /// derived from configuration and prefixed to it.
    mere: &'static str,
}

/// Čl. 47 st. 1 t. 5, for every radnja. The honest answer is that no transfer
/// has been established and that two things could establish one — where the
/// remote-support provider's servers are, and where the shop points its backup
/// folder. Answering *„ne“* flatly would assert a fact this program cannot check.
///
/// The prompt names what t. 5 asks for — the state and the safeguards — and
/// stops there. No transfer-chapter article range is printed: this crate has
/// never verified one, and a wrong pin-cite on a document the Poverenik reads
/// costs more than the missing one saves.
const PRENOS_OPSTI: &str = "Nije utvrđen prenos u druge države ni u međunarodne organizacije. \
     Obrada se odvija lokalno, na kasi u Republici Srbiji. Ako se rezervne kopije smeštaju na \
     uslugu čiji su serveri van Republike Srbije, ovde se upisuju naziv te države i opis mera \
     zaštite prenosa (ZZPL čl. 47 st. 1 tač. 5).";

/// The same question where it actually bites: the obrađivač reaches the till
/// over a remote-support tool whose infrastructure the shop does not choose.
const PRENOS_PODRSKA: &str = "Nije utvrđen prenos u druge države ni u međunarodne organizacije. \
     Sesiju daljinske podrške odobrava rukovalac unapred. Ako alat za daljinsku podršku \
     saobraćaj vodi preko servera van Republike Srbije, ovde se upisuju naziv te države i opis \
     mera zaštite prenosa (ZZPL čl. 47 st. 1 tač. 5) — to je pitanje za pružaoca alata, a ne \
     pretpostavka programa.";

/// The twelve radnje this program performs. Every `retention_policies` class
/// appears at least once, which
/// `every_configured_retention_class_reaches_the_register` enforces: a period the
/// app applies and the register never disclosed is a čl. 47 st. 1 t. 6 gap.
const TEMPLATES: &[Template] = &[
    Template {
        kljuc: "evidencija_zaposlenih",
        svrha_obrade: "Vođenje evidencije o zaposlenim licima (ZEOR čl. 5) i ispunjenje obaveza \
             iz radnog, penzijskog, zdravstvenog i poreskog zakonodavstva.",
        vrsta_lica: "Zaposleni i radno angažovana lica kod rukovaoca.",
        vrsta_podataka: "Podaci iz ZEOR čl. 5 tač. 1–25: identitet i matični broj, datum i mesto \
             rođenja, prebivalište i adresa stana, mesto rada i radno mesto, radno vreme, vrsta i \
             trajanje radnog odnosa, stručna sprema i osposobljenost, dani plaćenog odsustva i \
             dani privremene sprečenosti za rad. Dani privremene sprečenosti za rad su posebna \
             vrsta podataka o ličnosti (ZZPL čl. 17 st. 1) — evidentira se broj dana, bez \
             dijagnoze, doznake i slobodnog teksta. Broj osiguranih članova porodice vodi se kao \
             broj, nikada kao spisak.",
        vrsta_primalaca: "Nadležni državni organi kada zakon to nalaže (inspekcija rada, Poreska \
             uprava, RFZO, PIO fond); knjigovođa po ugovoru o obradi; Actaer kao obrađivač samo \
             tokom sesije daljinske podrške koju je rukovalac odobrio.",
        rok_osnov: "ZEOR čl. 7 st. 2 — podaci se čuvaju trajno. Rok se ne podešava.",
        retention: Some(RecordClass::Personnel),
        mere: "Evidencija je u zasebnoj tabeli, odvojenoj od naloga za prijavu, pa uklanjanje \
             naloga ne može da je obriše; program nema nijednu radnju koja briše zapis o \
             zaposlenom, ni u bazi ni u korisničkom interfejsu.",
    },
    Template {
        kljuc: "radno_vreme",
        svrha_obrade: "Vođenje evidencije o radnom vremenu i izvođenje mesečne klasifikacije \
             časova (ZoR čl. 55; ZEOR čl. 24 tač. 1), radi obračuna zarade i dokazivanja da su \
             ograničenja radnog vremena poštovana.",
        vrsta_lica: "Zaposleni i radno angažovana lica kod rukovaoca.",
        vrsta_podataka: "Časovi po kategorijama iz ZEOR čl. 24 tač. 1: redovan rad, prekovremeni \
             rad, noćni rad, rad na dan praznika, godišnji odmor, plaćeno odsustvo i časovi \
             privremene sprečenosti za rad sa zakonskom podelom na teret poslodavca i na teret \
             RFZO-a. Časovi privremene sprečenosti su posebna vrsta podataka (ZZPL čl. 17 st. 1) \
             i vode se kao časovi, bez dijagnoze.",
        vrsta_primalaca: "Nadležni državni organi kada zakon to nalaže; knjigovođa za obračun \
             zarade.",
        rok_osnov: "ZEOR čl. 25 st. 3 — zaključena mesečna klasifikacija čuva se trajno. Rok se \
             ne podešava.",
        retention: Some(RecordClass::WorktimeClassification),
        mere: "Zaključen mesec se ne prepisuje: ispravka je nova verzija zapisa koja nosi ko je, \
             kada i zašto ispravio, pa raniji sadržaj ostaje vidljiv.",
    },
    Template {
        kljuc: "prekovremeni_rad",
        svrha_obrade: "Vođenje posebne evidencije o prekovremenom radu (ZoR čl. 55 st. 6).",
        vrsta_lica: "Zaposleni koji su radili prekovremeno.",
        vrsta_podataka: "Dan, broj časova prekovremenog rada i osnov iz ZoR čl. 53 st. 1 na koji \
             se poslodavac poziva.",
        vrsta_primalaca: "Inspekcija rada po zahtevu; knjigovođa za obračun zarade.",
        rok_osnov: "Zakon ne propisuje rok čuvanja ove evidencije. Primenjuje se odbrambeni \
             minimum koji nadživljava rok zastarelosti iz ZoP čl. 84 i rok iz ZoR čl. 196, i \
             pomera se samo unapred.",
        retention: Some(RecordClass::WorktimeOvertimeLog),
        mere: "Evidencija je odvojena od mesečne klasifikacije, pa zaključenje meseca ne može da \
             je skrati.",
    },
    Template {
        kljuc: "radno_vreme_radne_verzije",
        svrha_obrade: "Unos i priprema podataka o radnom vremenu pre zaključenja meseca — radne \
             verzije i pomoćni podaci iz kojih se izvodi klasifikacija.",
        vrsta_lica: "Zaposleni i radno angažovana lica kod rukovaoca.",
        vrsta_podataka: "Nezaključeni dnevni unosi časova i pomoćni podaci o vremenu, u istim \
             kategorijama kao zaključena klasifikacija.",
        vrsta_primalaca: "Ne otkrivaju se nikome van rukovaoca; iz njih se izvodi klasifikacija \
             koja se dalje otkriva.",
        rok_osnov: "Zakon ne propisuje rok. Radne verzije se brišu tek pošto je mesec zaključen i \
             klasifikacija izvedena, jer se do tada iz njih izvodi evidencija koja se čuva \
             trajno (ZZPL čl. 5 st. 1 tač. 5).",
        retention: Some(RecordClass::WorktimeDraft),
        mere: "Brisanje je vezano za zaključenje perioda, a ne za kalendar, pa nezaključen mesec \
             ne može da se isprazni protekom vremena.",
    },
    Template {
        kljuc: "pripisivost_prometa",
        svrha_obrade: "Pripisivanje računa, smena i promena zaliha zaposlenom koji ih je obavio, \
             radi interne kontrole i rekonstrukcije knjigovodstvenog dokumenta. Pripisivost nije \
             zakonska obaveza nego mera zaštite imovine na osnovu legitimnog interesa rukovaoca \
             (ZZPL čl. 12), na koju zaposleni ima pravo prigovora (ZZPL čl. 37 st. 1).",
        vrsta_lica: "Zaposleni koji su radili na kasi.",
        vrsta_podataka: "Interna oznaka naloga zaposlenog uz račun, smenu, gotovinski nalog i \
             zapis o zalihama. Ime, prezime i matični broj se nikada ne prepisuju na red prometa.",
        vrsta_primalaca: "Poreska uprava i inspekcija u okviru uvida u poslovne knjige; \
             knjigovođa.",
        rok_osnov: "ZoRač čl. 28 — pripisivost se čuva zajedno sa knjigovodstvenim dokumentom u \
             kome stoji, u roku propisanom za taj dokument, i nema zaseban rok koji bi se \
             podešavao.",
        retention: None,
        mere: "Na redu prometa stoji samo interna oznaka naloga (pseudonimizacija, ZZPL čl. 42 \
             st. 1 tač. 1); ime se čita iz odvojene evidencije naloga i može da se ukloni bez \
             diranja knjigovodstvenog zapisa.",
    },
    Template {
        kljuc: "nalozi_i_pristup",
        svrha_obrade: "Vođenje korisničkih naloga za pristup kasi i provera identiteta pri \
             prijavi, radi kontrole pristupa podacima (ZZPL čl. 50 st. 1).",
        vrsta_lica: "Zaposleni koji imaju nalog na kasi.",
        vrsta_podataka: "Korisničko ime, ime za prikaz, uloga, status naloga i heš PIN-a odnosno \
             lozinke (argon2). PIN i lozinka se ne čuvaju u čitljivom obliku.",
        vrsta_primalaca: "Ne otkrivaju se nikome van rukovaoca; Actaer kao obrađivač može da \
             vidi nalog tokom odobrene sesije daljinske podrške.",
        rok_osnov: "Zakon ne propisuje rok. Heš PIN-a i lozinke uklanja se danom prestanka \
             radnog odnosa, a ne po isteku roka (ZZPL čl. 5 st. 1 tač. 5 i čl. 42 st. 2); nalog \
             i evidencija o zaposlenom pri tome ostaju.",
        retention: Some(RecordClass::Credentials),
        mere: "Korisničko ime ostaje zauzeto i posle deaktivacije, pa nalog ne može da se dodeli \
             drugom licu; uklanjanje heša je automatsko i vremenski vođeno, ne zavisi od toga da \
             li se neko setio da ga pokrene.",
    },
    Template {
        kljuc: "evidencija_pristupa",
        svrha_obrade: "Evidentiranje radnji nad podacima o ličnosti — unosa, izmene, uvida, \
             otkrivanja, upoređivanja i brisanja — radi ocene zakonitosti obrade, internog \
             nadzora i obezbeđivanja integriteta i bezbednosti podataka. Ovu evidenciju rukovalac \
             vodi kao sopstvenu meru zaštite: ZZPL čl. 48 obavezuje nadležni organ koji podatke \
             obrađuje u posebne svrhe, pa je ovde uzor za sadržaj, a ne osnov obaveze.",
        vrsta_lica: "Zaposleni koji su radnju izvršili.",
        vrsta_podataka: "Vreme radnje, interna oznaka naloga koji ju je izvršio, vrsta radnje iz \
             zatvorene liste, vrsta i interna oznaka zapisa nad kojim je izvršena, šifra razloga \
             i klasa primaoca. U evidenciju ne ulaze vrednosti podataka, JMBG, broj platne \
             kartice, adresa, telefon, slobodan tekst niti tekst pretrage — program ih odbija na \
             upisu, a ne po dogovoru.",
        vrsta_primalaca: "Poverenik za informacije od javnog značaja i zaštitu podataka o \
             ličnosti, na zahtev; nadležni organi u postupku koji vode.",
        rok_osnov: "Zakon ne propisuje rok. Primenjuje se podrazumevani rok koji nadživljava rok \
             zastarelosti iz ZoP čl. 84 i pomera se samo unapred. Ova evidencija se ne čuva \
             trajno — ZZPL čl. 47 st. 7 propisuje trajno čuvanje za evidenciju radnji obrade, a \
             ne za evidenciju pristupa.",
        retention: Some(RecordClass::AccessLog),
        mere: "Zapisi su povezani u lanac heševa i program nema nijednu radnju koja menja ili \
             briše upisan red; uklanjanje reda po isteku roka vidi se kao prekid lanca.",
    },
    Template {
        kljuc: "tehnicka_podrska",
        svrha_obrade: "Odobravanje i evidentiranje sesija daljinske tehničke podrške, tako da \
             svaka obrada od strane obrađivača ima dokumentovan nalog rukovaoca (ZZPL čl. 46).",
        vrsta_lica: "Zaposleni koji odobrava sesiju i zaposleni čiji podaci mogu da budu vidljivi \
             tokom sesije.",
        vrsta_podataka: "Interna oznaka naloga koji je odobrio sesiju, vreme odobrenja, opseg i \
             trajanje odobrenja, vreme početka i završetka sesije i podatak o opozivu.",
        vrsta_primalaca: "Actaer kao obrađivač.",
        rok_osnov: "Zakon ne propisuje rok. Nalog je dokaz da je obrada obrađivača bila \
             dokumentovana, pa se čuva dok traje ta dokazna svrha; poseban rok još nije podešen \
             u programu i do tada se ništa ne briše.",
        retention: None,
        mere: "Sesija se odobrava unapred, sa izričitim opsegom i trajanjem, i sama od sebe \
             ističe; obrađivač ne može da je produži.",
    },
    Template {
        kljuc: "evidencija_povreda",
        svrha_obrade: "Dokumentovanje svake povrede podataka o ličnosti (ZZPL čl. 52 st. 6), radi \
             dokazivanja postupanja po celom članu 52 (st. 7) i pripreme obaveštenja Povereniku.",
        vrsta_lica: "Lica na koja se odnose podaci obuhvaćeni povredom — evidentira se njihov \
             broj, nikada spisak.",
        vrsta_podataka: "Opis povrede, opis mogućih posledica, preduzete mere, vreme saznanja, \
             vreme nastanka i otkrivanja, vrste obuhvaćenih podataka, procena rizika, odluka o \
             obaveštavanju sa obrazloženjem i podaci o obaveštavanju lica po čl. 53.",
        vrsta_primalaca: "Poverenik za informacije od javnog značaja i zaštitu podataka o \
             ličnosti; lica na koja se podaci odnose kada je obaveštavanje obavezno (ZZPL \
             čl. 53).",
        rok_osnov: "Zakon ne propisuje rok čuvanja ove evidencije. ZZPL čl. 47 st. 7 propisuje \
             trajno čuvanje za evidenciju radnji obrade, a ne za evidenciju povreda, pa se ni ova \
             evidencija ne čuva trajno. Rok još nije podešen u programu i do tada se ništa ne \
             briše.",
        retention: None,
        mere: "Evidenciji pristupa samo administrator; ne ulazi u redovne izveštaje, u izvoze \
             podataka niti u materijal koji se šalje tehničkoj podršci.",
    },
    Template {
        kljuc: "reklamacije",
        svrha_obrade: "Vođenje evidencije primljenih reklamacija i postupanje po njima (ZZP \
             čl. 63 st. 8 za reklamacije podnete od 1. avgusta 2026, ZZP čl. 55 st. 8 za ranije).",
        vrsta_lica: "Potrošači koji su izjavili reklamaciju.",
        vrsta_podataka: "Ime i prezime podnosioca, kontakt koji je sam dao, podaci o robi, opis \
             nesaobraznosti i zahtev, datum prijema i tok postupka sa ishodom.",
        vrsta_primalaca: "Tržišna inspekcija po zahtevu; telo za vansudsko rešavanje \
             potrošačkog spora kada se takav postupak pokrene.",
        rok_osnov: "ZZP čl. 63 st. 6 (ranije čl. 55 st. 6) — evidencija se čuva najmanje dve \
             godine od dana podnošenja reklamacije. To je donja granica, a ne rok brisanja: \
             program ne prikazuje datum do kog reklamacija mora da se obriše i ne briše je sam.",
        retention: None,
        mere: "Evidencija se vodi u istoj bazi, pod istom kontrolom pristupa; kontakt potrošača \
             se koristi samo za postupanje po toj reklamaciji.",
    },
    Template {
        kljuc: "popis_imovine",
        svrha_obrade:
            "Sprovođenje popisa imovine i obaveza i usklađivanje knjigovodstvenog stanja \
             sa stvarnim stanjem (ZoRač čl. 20 i čl. 21) i dokazivanje da je popis vodila komisija \
             sastavljena onako kako propis traži (Pravilnik o popisu, čl. 5 st. 1 i čl. 6).",
        vrsta_lica: "Članovi komisije za popis, odnosno jedno lice koje kod preduzetnika vrši \
             popis, i lica koja potpisuju popisne liste.",
        vrsta_podataka: "Ime i prezime, uloga u popisu (predsednik komisije, član ili jedno lice) \
             i podatak o tome da li lice rukuje imovinom koja se popisuje — taj podatak se vodi \
             zato što lica koja rukuju imovinom ne mogu biti određena u komisiju. Uz svaki potpis \
             beleže se ime potpisnika i vreme potpisivanja. Ostalo u popisnim listama odnosi se na \
             robu, gotovinu, potraživanja i obaveze, a ne na lica.",
        vrsta_primalaca: "Poreska uprava u nadzoru nad evidentiranjem poslovnih promena (ZoRač \
             čl. 56 st. 1); knjigovođa koji knjiži utvrđene razlike; vlasnik tuđe robe, kome \
             obveznik dostavlja primerak potpisane posebne popisne liste (Pravilnik o popisu, \
             čl. 2 st. 6) — tu listu dostavlja obveznik, program je ne šalje nikome.",
        rok_osnov:
            "ZoRač čl. 28 st. 7 — isprave na osnovu kojih se unose podaci u poslovne knjige \
             čuvaju se pet godina, a rok se računa od poslednjeg dana poslovne godine na koju se \
             odnose (čl. 28 st. 9). Nijedan propis ne imenuje popisne liste izričito, pa je ovo \
             zaključak a ne izričita odredba; rok se pomera samo unapred. Izveštaj o popisu se ne \
             čuva u aplikaciji — sastavlja se i štampa na zahtev, pa štampani primerak čuva \
             obveznik.",
        retention: Some(RecordClass::PopisDokumentacija),
        mere: "Podaci iz knjigovodstva o količinama ne izdaju se komisiji pre nego što je stvarno \
             stanje upisano u popisne liste i pre nego što su te liste potpisane (Pravilnik o \
             popisu, čl. 8 st. 5); to ograničenje je postavljeno u samoj bazi i u upitima, a ne u \
             izgledu ekrana. Proknjižen popis se više ne menja — ispravka ide kroz novi popis.",
    },
    Template {
        kljuc: "evidencija_radnji_obrade",
        svrha_obrade: "Vođenje ove evidencije o radnjama obrade (ZZPL čl. 47 st. 1) i stavljanje \
             na uvid Povereniku na njegov zahtev (ZZPL čl. 47 st. 8).",
        vrsta_lica: "Ne obrađuju se podaci o licima na koja se podaci odnose. Evidencija opisuje \
             radnje obrade i sadrži kontakt podatke rukovaoca.",
        vrsta_podataka: "Naziv i kontakt podaci rukovaoca, svrhe obrade, vrste lica i vrste \
             podataka, vrste primalaca, podaci o prenosu u druge države, rokovi čuvanja po \
             kategorijama i opšti opis mera zaštite.",
        vrsta_primalaca: "Poverenik za informacije od javnog značaja i zaštitu podataka o \
             ličnosti, na zahtev (ZZPL čl. 47 st. 8).",
        rok_osnov: "ZZPL čl. 47 st. 7 — evidencija o radnjama obrade čuva se trajno. Rok se ne \
             podešava.",
        retention: Some(RecordClass::ProcessingRegister),
        mere: "Evidencija se generiše iz podešavanja programa i iz tabele rokova čuvanja, pa ne \
             može da navede rok koji program ne primenjuje; svako pregenerisanje koje nešto \
             promeni ostavlja trag u evidenciji pristupa.",
    },
];

/// Čl. 47 st. 1 t. 1. The register is the document that says who the rukovalac
/// is, so a field the shop has not filled in is printed as a visible gap rather
/// than skipped: an inspector reading „nije uneto“ knows to ask, and a reader of
/// a silently shortened line does not.
/// The čl. 47 st. 1 t. 6 prose of every radnja, paired with the class its
/// applied period is read from — for `docs_guard`, which reads these the way it
/// reads the documents.
///
/// This is the one register column that is a **claim about behaviour**: t. 6 is
/// the *rok posle čijeg isteka se brišu određene vrste podataka*, and it goes to
/// the Poverenik. `no_stored_retention_note_claims_a_period_no_command_can_move`
/// pins each string against the class it is wired to.
#[cfg(test)]
pub(crate) fn retention_prose() -> Vec<(&'static str, Option<RecordClass>, &'static str)> {
    TEMPLATES
        .iter()
        .map(|template| (template.kljuc, template.retention, template.rok_osnov))
        .collect()
}

fn rukovalac_kontakt(company: &CompanySettings) -> String {
    fn ili_prazno(value: &str) -> String {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            "[nije uneto u podešavanjima]".to_string()
        } else {
            trimmed.to_string()
        }
    }

    format!(
        "Adresa: {}. PIB: {}. Matični broj: {}. Telefon: {}.",
        ili_prazno(&company.address),
        ili_prazno(&company.pib),
        ili_prazno(&company.registration_number),
        ili_prazno(&company.phone),
    )
}

/// Čl. 47 st. 1 t. 7 — *opšti opis mera zaštite iz čl. 50 st. 1*, read off the
/// configuration rather than asserted.
///
/// The two backup limbs are the reason this is derived at all. A register that
/// printed *„rezervne kopije su šifrovane“* on a till where no passphrase has
/// been set would be a false statement to the regulator about a čl. 50 st. 1
/// measure, so the sentence follows the setting in both directions and says
/// plainly when a measure is not in place. No tačka is pinned inside čl. 50
/// st. 1: this crate has verified the stav and not the enumeration under it.
fn mere_zastite(state: &AppState) -> Result<String, AppError> {
    let backup = crate::commands::backup::load_backup_status(state)?;

    let automatske = if backup.automatic_backup_enabled {
        "automatsko pravljenje rezervnih kopija je uključeno"
    } else {
        "automatsko pravljenje rezervnih kopija NIJE uključeno"
    };
    let sifrovanje = if backup.encryption_configured {
        "rezervne kopije se šifruju lozinkom koju je rukovalac postavio"
    } else {
        "šifrovanje rezervnih kopija NIJE podešeno, pa se kopije zapisuju nešifrovano"
    };

    Ok(format!(
        "Opšte mere (ZZPL čl. 50 st. 1), iste za sve radnje obrade: kontrola pristupa po ulogama \
         (administrator i kasir) uz obaveznu prijavu; lozinke i PIN-ovi se čuvaju samo kao \
         argon2 heš; baza radi lokalno na kasi, u WAL režimu; evidencija pristupa je povezana u \
         lanac heševa i ne može da se menja; sesije daljinske podrške odobrava rukovalac unapred, \
         sa opsegom i trajanjem. Prema trenutnim podešavanjima: {automatske}; {sifrovanje}. Mera \
         specifična za ovu radnju obrade: "
    ))
}

/// Čl. 47 st. 1 t. 6, in two halves that must not be confused: the legal reason
/// the period is what it is, and the period the program **actually applies**.
///
/// The second half is read out of `retention_policies` at generation time, so
/// the register cannot claim a period the till does not enforce — and a class
/// whose floor the shop has pushed forward shows the pushed value, not the
/// seeded one.
fn rok_cuvanja(conn: &Connection, template: &Template) -> Result<String, AppError> {
    let mut rok = template.rok_osnov.to_string();

    let Some(class) = template.retention else {
        return Ok(rok);
    };

    let policy = crate::retention::load_policy(conn, class)?;
    rok.push_str(" Rok koji program primenjuje: ");
    if policy.never_purge {
        rok.push_str("trajno, bez datuma isteka.");
    } else if let Some(until) = policy.retain_until.as_deref() {
        rok.push_str(&format!("ništa se ne briše pre {until}."));
    } else {
        rok.push_str("nije upisan, pa se ništa ne briše.");
    }
    if policy.legal_hold {
        rok.push_str(" Na snazi je zabrana brisanja (pravni zastoj).");
    }

    Ok(rok)
}

const ACTIVITY_COLUMNS: &str = "id, kljuc, rukovalac_naziv, rukovalac_kontakt, svrha_obrade,
     vrsta_lica, vrsta_podataka, vrsta_primalaca, prenos_u_druge_drzave, mere_zastite_prenosa,
     rok_cuvanja, retention_record_class, opis_mera_zastite, updated_at";

/// A stored `retention_record_class` this build no longer knows reads back as
/// `None` rather than as an error. That is not a swallowed failure: `matches`
/// then sees a difference, the next generation rewrites the row from the current
/// class list, and the čl. 48 line records that it did — self-healing beats
/// refusing to render the register an inspection is asking for.
fn read_activity(row: &rusqlite::Row<'_>) -> rusqlite::Result<ProcessingActivity> {
    let stored_class: Option<String> = row.get(11)?;
    Ok(ProcessingActivity {
        id: row.get(0)?,
        kljuc: row.get(1)?,
        rukovalac_naziv: row.get(2)?,
        rukovalac_kontakt: row.get(3)?,
        svrha_obrade: row.get(4)?,
        vrsta_lica: row.get(5)?,
        vrsta_podataka: row.get(6)?,
        vrsta_primalaca: row.get(7)?,
        prenos_u_druge_drzave: row.get(8)?,
        mere_zastite_prenosa: row.get(9)?,
        rok_cuvanja: row.get(10)?,
        retention_record_class: stored_class.as_deref().and_then(RecordClass::from_key),
        opis_mera_zastite: row.get(12)?,
        updated_at: row.get(13)?,
    })
}

fn read_register(conn: &Connection) -> Result<Vec<ProcessingActivity>, AppError> {
    let mut statement = conn.prepare(&format!(
        "SELECT {ACTIVITY_COLUMNS} FROM processing_activities ORDER BY id"
    ))?;
    let rows = statement.query_map([], read_activity)?;
    rows.collect::<rusqlite::Result<Vec<_>>>()
        .map_err(AppError::from)
}

/// Everything a generation writes, before it is compared with what is stored.
struct Rendered {
    kljuc: &'static str,
    rukovalac_naziv: String,
    rukovalac_kontakt: String,
    svrha_obrade: &'static str,
    vrsta_lica: &'static str,
    vrsta_podataka: &'static str,
    vrsta_primalaca: &'static str,
    prenos_u_druge_drzave: &'static str,
    rok_cuvanja: String,
    retention_record_class: Option<RecordClass>,
    opis_mera_zastite: String,
}

impl Rendered {
    /// Whether the stored row already says exactly this. Every column the
    /// generator writes is compared — a field left out here would be a field
    /// that silently stops being regenerated.
    ///
    /// `mere_zastite_prenosa` is deliberately **not** among them, and that is
    /// the one column the generator does not own. Čl. 47 st. 1 t. 5's second
    /// half is the answer this program cannot compute, and both prenos entries
    /// tell the operator so in words — *„ovde se upisuju naziv te države i opis
    /// mera zaštite prenosa“*. Comparing it would make a filled-in slot look
    /// like a drifted row, rewrite it away on the next launch and log a čl. 48
    /// *menjanje* for the erasure. So it is neither compared here nor touched by
    /// the UPDATE in [`generate`]; a fresh row is inserted with `NULL` because a
    /// row nobody has answered for yet has nothing to report.
    fn matches(&self, stored: &ProcessingActivity) -> bool {
        stored.kljuc == self.kljuc
            && stored.rukovalac_naziv == self.rukovalac_naziv
            && stored.rukovalac_kontakt.as_deref() == Some(self.rukovalac_kontakt.as_str())
            && stored.svrha_obrade == self.svrha_obrade
            && stored.vrsta_lica == self.vrsta_lica
            && stored.vrsta_podataka == self.vrsta_podataka
            && stored.vrsta_primalaca.as_deref() == Some(self.vrsta_primalaca)
            && stored.prenos_u_druge_drzave.as_deref() == Some(self.prenos_u_druge_drzave)
            && stored.rok_cuvanja.as_deref() == Some(self.rok_cuvanja.as_str())
            && stored.retention_record_class == self.retention_record_class
            && stored.opis_mera_zastite.as_deref() == Some(self.opis_mera_zastite.as_str())
    }
}

/// Regenerates the register from the current configuration.
///
/// **Ungated on purpose.** This runs at launch, before anybody has signed in —
/// a register that only exists once an administrator remembers to press a button
/// is the missing register req. 28 exists to prevent. The operator-facing entry
/// points ([`regenerate`], [`list`], [`export`]) carry `require_admin`.
///
/// The whole run is one transaction, so the rows and the čl. 48 lines that
/// describe them commit or roll back together. A row whose rendered content is
/// byte-identical to what is stored is not rewritten and not logged: the log
/// records changes, and a launch that changed nothing must not fill it with
/// claims that it did.
pub(crate) fn generate(state: &AppState, now: &str) -> Result<Vec<ProcessingActivity>, AppError> {
    let company = crate::commands::settings::load_company_settings(state)?;
    let naziv = {
        let trimmed = company.shop_name.trim();
        if trimmed.is_empty() {
            "[naziv rukovaoca nije unet u podešavanjima]".to_string()
        } else {
            trimmed.to_string()
        }
    };
    let kontakt = rukovalac_kontakt(&company);
    let opste_mere = mere_zastite(state)?;
    let actor_user_id = state.session_user_id()?;

    let mut connection = state.db().open()?;
    let tx = connection.transaction()?;

    for template in TEMPLATES {
        let rendered = Rendered {
            kljuc: template.kljuc,
            rukovalac_naziv: naziv.clone(),
            rukovalac_kontakt: kontakt.clone(),
            svrha_obrade: template.svrha_obrade,
            vrsta_lica: template.vrsta_lica,
            vrsta_podataka: template.vrsta_podataka,
            vrsta_primalaca: template.vrsta_primalaca,
            prenos_u_druge_drzave: if template.kljuc == "tehnicka_podrska" {
                PRENOS_PODRSKA
            } else {
                PRENOS_OPSTI
            },
            rok_cuvanja: rok_cuvanja(&tx, template)?,
            retention_record_class: template.retention,
            opis_mera_zastite: format!("{opste_mere}{}", template.mere),
        };

        let stored: Option<ProcessingActivity> = tx
            .query_row(
                &format!("SELECT {ACTIVITY_COLUMNS} FROM processing_activities WHERE kljuc = ?1"),
                params![template.kljuc],
                read_activity,
            )
            .optional()?;

        let (id, action) = match stored {
            Some(existing) if rendered.matches(&existing) => continue,
            Some(existing) => {
                tx.execute(
                    // `mere_zastite_prenosa` is absent on purpose: st. 1 t. 5's
                    // safeguards slot belongs to whoever answers it, and a
                    // regeneration is not an answer. See [`Rendered::matches`].
                    "UPDATE processing_activities
                        SET rukovalac_naziv = ?2, rukovalac_kontakt = ?3, svrha_obrade = ?4,
                            vrsta_lica = ?5, vrsta_podataka = ?6, vrsta_primalaca = ?7,
                            prenos_u_druge_drzave = ?8,
                            rok_cuvanja = ?9, retention_record_class = ?10,
                            opis_mera_zastite = ?11, updated_at = ?12
                      WHERE id = ?1",
                    params![
                        existing.id,
                        rendered.rukovalac_naziv,
                        rendered.rukovalac_kontakt,
                        rendered.svrha_obrade,
                        rendered.vrsta_lica,
                        rendered.vrsta_podataka,
                        rendered.vrsta_primalaca,
                        rendered.prenos_u_druge_drzave,
                        rendered.rok_cuvanja,
                        rendered.retention_record_class.map(RecordClass::key),
                        rendered.opis_mera_zastite,
                        now,
                    ],
                )?;
                (existing.id, AuditAction::Menjanje)
            }
            None => {
                tx.execute(
                    "INSERT INTO processing_activities
                         (kljuc, rukovalac_naziv, rukovalac_kontakt, svrha_obrade, vrsta_lica,
                          vrsta_podataka, vrsta_primalaca, prenos_u_druge_drzave,
                          mere_zastite_prenosa, rok_cuvanja, retention_record_class,
                          opis_mera_zastite, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10, ?11, ?12, ?12)",
                    params![
                        rendered.kljuc,
                        rendered.rukovalac_naziv,
                        rendered.rukovalac_kontakt,
                        rendered.svrha_obrade,
                        rendered.vrsta_lica,
                        rendered.vrsta_podataka,
                        rendered.vrsta_primalaca,
                        rendered.prenos_u_druge_drzave,
                        rendered.rok_cuvanja,
                        rendered.retention_record_class.map(RecordClass::key),
                        rendered.opis_mera_zastite,
                        now,
                    ],
                )?;
                (tx.last_insert_rowid(), AuditAction::Unos)
            }
        };

        // Req. 4: the row's internal id and nothing else. The register names the
        // shop, its address and its telephone, and none of that may reach a
        // table whose whole point is to hold no personal data — `object_id` is
        // whitelisted to digits, so the `kljuc` could not travel here either.
        crate::commands::audit::append_audit_event(
            &tx,
            &AuditDraft {
                at: now.to_string(),
                actor_user_id,
                action,
                object_type: AuditObjectType::ProcessingActivity,
                object_id: id.to_string(),
                reason_code: None,
                recipient: None,
                support_session_id: None,
            },
        )?;
    }

    let register = read_register(&tx)?;
    tx.commit()?;
    Ok(register)
}

/// [`generate`] behind the administrator gate — the operator-facing verb.
pub fn regenerate(state: &AppState, now: &str) -> Result<Vec<ProcessingActivity>, AppError> {
    require_admin(state)?;
    generate(state, now)
}

/// The register as stored. Admin-gated: it carries the shop's identity and it is
/// the document an inspection asks for.
pub fn list(state: &AppState) -> Result<Vec<ProcessingActivity>, AppError> {
    require_admin(state)?;
    let connection = state.db().open()?;
    read_register(&connection)
}

/// Writes the register to `exports/`, ready to print.
///
/// It regenerates first, because the two things this file is for — the čl. 47
/// st. 8 *uvid* and the Pravilnik 40/2019 čl. 4 st. 1 *Prilog* to a breach
/// notification — are both handed to the Poverenik, and a stale copy of the
/// shop's own record is the wrong document to hand over.
///
/// **No čl. 48 st. 2 `otkrivanje` line is written here, and that is deliberate.**
/// An `otkrivanje` row must name the class of primalac, and this program does
/// not know it: the breach obrazac is addressed to the Poverenik by the
/// prescribed form itself, while the register is printed for an inspection, for
/// a filing, or for the shop's own review. Writing `poverenik` on every export
/// would put a guess into the one table that exists to hold only what is known.
pub fn export(state: &AppState, now: &str) -> Result<ExportedFile, AppError> {
    require_admin(state)?;
    let register = generate(state, now)?;
    let company = crate::commands::settings::load_company_settings(state)?;

    crate::commands::campaigns::write_export(
        state,
        "evidencija-radnji-obrade-cl47.html",
        &render_html(&company, &register),
        register.len(),
    )
}

/// The register as a printable document: a header carrying the article and the
/// st. 7 / st. 8 rules, one block per radnja obrade with the seven st. 1
/// elements in order, and a closing note placing the st. 4 obrađivač record with
/// the party that keeps it.
pub fn render_html(company: &CompanySettings, activities: &[ProcessingActivity]) -> String {
    let mut html = String::new();
    html.push_str("<!doctype html>\n<html lang=\"sr-Latn\">\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<title>Evidencija o radnjama obrade (ZZPL čl. 47)</title>\n");
    html.push_str(
        "<style>\n\
         @page { margin: 1.2cm }\n\
         body { font-family: sans-serif; color: #111; margin: 1.2cm; line-height: 1.45; }\n\
         h1 { font-size: 1.3rem; text-align: center; }\n\
         h2 { font-size: 1rem; margin: 1.4rem 0 0.4rem; }\n\
         .meta { color: #444; font-size: 0.9rem; }\n\
         table { border-collapse: collapse; width: 100%; margin-bottom: 0.6rem; }\n\
         th, td { border: 1px solid #999; padding: 0.3rem 0.5rem; text-align: left; \
         vertical-align: top; }\n\
         th { width: 32%; font-weight: normal; }\n\
         </style>\n</head>\n<body>\n",
    );

    html.push_str("<h1>EVIDENCIJA O RADNJAMA OBRADE</h1>\n");
    html.push_str(
        "<p class=\"meta\">Evidencija rukovaoca iz ZZPL čl. 47 st. 1. Program je sastavlja iz \
         podešavanja i iz tabele rokova čuvanja; ne unosi se ručno.</p>\n",
    );
    html.push_str(&format!(
        "<p><strong>Rukovalac:</strong> {}</p>\n",
        escape_html(if company.shop_name.trim().is_empty() {
            "[naziv rukovaoca nije unet u podešavanjima]"
        } else {
            company.shop_name.trim()
        })
    ));
    html.push_str(
        "<p class=\"meta\">Evidencija se čuva trajno (ZZPL čl. 47 st. 7) i stavlja se na uvid \
         Povereniku na njegov zahtev (ZZPL čl. 47 st. 8). Izuzeće za rukovaoca sa manje od 250 \
         zaposlenih ovde ne važi, i to po oba osnova iz čl. 47 st. 9 koja se stiču: obrada nije \
         povremena (tač. 2), a obuhvata i posebne vrste podataka o ličnosti iz čl. 17 st. 1 \
         (tač. 3).</p>\n",
    );

    for activity in activities {
        html.push_str(&format!(
            "<h2>{}</h2>\n<table>\n",
            escape_html(&naslov_radnje(&activity.kljuc))
        ));
        html.push_str(&red(
            "1) Naziv i kontakt podaci rukovaoca",
            &format!(
                "{} {}",
                activity.rukovalac_naziv,
                activity.rukovalac_kontakt.as_deref().unwrap_or_default()
            ),
        ));
        html.push_str(&red("2) Svrha obrade", &activity.svrha_obrade));
        html.push_str(&red(
            "3) Vrsta lica i vrsta podataka o ličnosti",
            &format!("{} {}", activity.vrsta_lica, activity.vrsta_podataka),
        ));
        html.push_str(&red(
            "4) Vrsta primalaca",
            activity.vrsta_primalaca.as_deref().unwrap_or("—"),
        ));
        html.push_str(&red(
            "5) Prenos u druge države i međunarodne organizacije",
            activity
                .prenos_u_druge_drzave
                .as_deref()
                .unwrap_or("Nije utvrđen."),
        ));
        html.push_str(&red(
            "6) Rok čuvanja",
            activity.rok_cuvanja.as_deref().unwrap_or("—"),
        ));
        html.push_str(&red(
            "7) Opšti opis mera zaštite",
            activity.opis_mera_zastite.as_deref().unwrap_or("—"),
        ));
        html.push_str("</table>\n");
    }

    html.push_str(
        "<h2>Evidencija obrađivača (ZZPL čl. 47 st. 4)</h2>\n\
         <p>Ovo nije deo ove evidencije. Evidenciju o vrstama radnji obrade koje vrši u ime \
         rukovaoca vodi obrađivač, o sopstvenoj obradi i sa četiri stavke iz čl. 47 st. 4. \
         Rukovalac je ne sastavlja i ne dostavlja umesto obrađivača.</p>\n",
    );
    html.push_str(
        "<p class=\"meta\">Lice za zaštitu podataka nije imenovano: nijedan od uslova iz ZZPL \
         čl. 56 st. 2 nije ispunjen. Koristi se naziv „kontakt za zaštitu podataka“, jer \
         dobrovoljno imenovanje lica za zaštitu podataka aktivira obaveze objavljivanja kontakta \
         i obaveštavanja Poverenika.</p>\n",
    );

    html.push_str("</body>\n</html>\n");
    html
}

/// The operator-facing heading for a generated `kljuc`. The key itself is a
/// database identifier and never reaches a printed page.
fn naslov_radnje(kljuc: &str) -> String {
    let naslov = match kljuc {
        "evidencija_zaposlenih" => "Evidencija o zaposlenim licima",
        "radno_vreme" => "Evidencija o radnom vremenu i mesečna klasifikacija časova",
        "prekovremeni_rad" => "Posebna evidencija o prekovremenom radu",
        "radno_vreme_radne_verzije" => "Unos i priprema podataka o radnom vremenu",
        "pripisivost_prometa" => "Pripisivost prometa i smena zaposlenima",
        "nalozi_i_pristup" => "Korisnički nalozi i pristup kasi",
        "evidencija_pristupa" => "Evidencija pristupa podacima o ličnosti",
        "tehnicka_podrska" => "Sesije daljinske tehničke podrške",
        "evidencija_povreda" => "Evidencija povreda podataka o ličnosti",
        "reklamacije" => "Evidencija primljenih reklamacija",
        "evidencija_radnji_obrade" => "Evidencija o radnjama obrade",
        other => other,
    };
    naslov.to_string()
}

fn red(oznaka: &str, vrednost: &str) -> String {
    format!(
        "<tr><th>{}</th><td>{}</td></tr>\n",
        escape_html(oznaka),
        escape_html(vrednost.trim())
    )
}

/// Escapes the five HTML-significant characters, so a shop name that contains
/// `<` or `&` can never break out of the document.
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

#[tauri::command]
pub fn cl47_list(state: State<'_, AppState>) -> Result<Vec<ProcessingActivity>, CommandError> {
    list(state.inner()).map_err(Into::into)
}

#[tauri::command]
pub fn cl47_generate(state: State<'_, AppState>) -> Result<Vec<ProcessingActivity>, CommandError> {
    regenerate(state.inner(), &utc_now()?).map_err(Into::into)
}

#[tauri::command]
pub fn cl47_export(state: State<'_, AppState>) -> Result<ExportedFile, CommandError> {
    export(state.inner(), &utc_now()?).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{export, generate, list, regenerate, render_html, ProcessingActivity};
    use crate::commands::settings::CompanySettings;
    use crate::db::{test_database_path, Db};
    use crate::retention::{
        is_purgeable, load_policy, seed_retention_policies, RecordClass, NEVER_PURGE_TABLES,
    };
    use crate::state::AppState;

    const NOW: &str = "2026-08-02T09:00:00Z";
    const KASNIJE: &str = "2026-09-15T11:30:00Z";

    /// The audit log's key, and the breach log's — req. 48 makes both of them
    /// processing operations in their own right.
    const KLJUC_EVIDENCIJA_PRISTUPA: &str = "evidencija_pristupa";
    const KLJUC_EVIDENCIJA_POVREDA: &str = "evidencija_povreda";
    /// The register's own entry — the one row that is legitimately `trajno`.
    const KLJUC_REGISTAR: &str = "evidencija_radnji_obrade";

    fn with_state(test_name: &str, test: impl FnOnce(&AppState)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let state = AppState::new(db);
            seed_retention_policies(&state, NOW).expect("retention classes should seed");
            test(&state);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    fn sign_in_admin(state: &AppState) -> i64 {
        let admin_id: i64 = state
            .db()
            .open()
            .expect("database should open")
            .query_row("SELECT id FROM users WHERE username = 'admin'", [], |row| {
                row.get(0)
            })
            .expect("bootstrap admin should exist");
        state
            .set_session_user_id(admin_id)
            .expect("admin session should set");
        admin_id
    }

    fn sign_in_cashier(state: &AppState) -> i64 {
        let connection = state.db().open().expect("database should open");
        connection
            .execute(
                "INSERT INTO users (username, display_name, role, pin_hash, active,
                                    created_at, updated_at)
                 VALUES ('radnica', 'Milica Milićević', 'cashier', 'pin-hes',
                         1, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("cashier should insert");
        let cashier_id = connection.last_insert_rowid();
        state
            .set_session_user_id(cashier_id)
            .expect("cashier session should set");
        cashier_id
    }

    fn by_kljuc<'a>(register: &'a [ProcessingActivity], kljuc: &str) -> &'a ProcessingActivity {
        register
            .iter()
            .find(|activity| activity.kljuc == kljuc)
            .unwrap_or_else(|| panic!("the register must carry a `{kljuc}` radnja obrade"))
    }

    fn audit_rows(state: &AppState) -> Vec<(String, String, String)> {
        let connection = state.db().open().expect("database should open");
        let mut statement = connection
            .prepare("SELECT action, object_type, object_id FROM audit_events ORDER BY id")
            .expect("statement should prepare");
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .expect("query should run")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("rows should read");
        rows
    }

    /// Every textual column of every logged row. What is not in here never
    /// reached the evidencija pristupa.
    fn audit_table_text(state: &AppState) -> String {
        let connection = state.db().open().expect("database should open");
        let mut statement = connection
            .prepare(
                "SELECT at, action, object_type, object_id, COALESCE(reason_code, ''),
                        COALESCE(recipient, ''), prev_hash, hash
                 FROM audit_events ORDER BY id",
            )
            .expect("statement should prepare");
        let rows = statement
            .query_map([], |row| {
                Ok(format!(
                    "{}|{}|{}|{}|{}|{}|{}|{}",
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                ))
            })
            .expect("query should run")
            .collect::<rusqlite::Result<Vec<_>>>()
            .expect("rows should read");
        rows.join("\n")
    }

    /// Every string the register puts in front of a reader, concatenated.
    fn register_text(register: &[ProcessingActivity]) -> String {
        register
            .iter()
            .map(|activity| {
                [
                    activity.kljuc.as_str(),
                    activity.rukovalac_naziv.as_str(),
                    activity.rukovalac_kontakt.as_deref().unwrap_or(""),
                    activity.svrha_obrade.as_str(),
                    activity.vrsta_lica.as_str(),
                    activity.vrsta_podataka.as_str(),
                    activity.vrsta_primalaca.as_deref().unwrap_or(""),
                    activity.prenos_u_druge_drzave.as_deref().unwrap_or(""),
                    activity.mere_zastite_prenosa.as_deref().unwrap_or(""),
                    activity.rok_cuvanja.as_deref().unwrap_or(""),
                    activity.opis_mera_zastite.as_deref().unwrap_or(""),
                ]
                .join("\n")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Čl. 47 st. 1 t. 6 asks for the *rok posle čijeg isteka se brišu određene
    /// vrste podataka* — the period is recorded **per category**, not once for
    /// the shop. So every generated radnja carries its own rok, the roks are not
    /// one repeated sentence, and where a `retention_policies` class governs the
    /// radnja the register prints what that row actually stores rather than a
    /// period the till does not apply.
    #[test]
    fn the_register_records_a_retention_period_for_every_category() {
        with_state("cl47_rok_per_category", |state| {
            sign_in_admin(state);
            let register = generate(state, NOW).expect("the register should generate");

            assert!(
                register.len() >= 8,
                "the register must cover the shop's processing, not a sample: {}",
                register.len()
            );

            for activity in &register {
                let rok = activity
                    .rok_cuvanja
                    .as_deref()
                    .unwrap_or_else(|| panic!("{} carries no rok (st. 1 t. 6)", activity.kljuc));
                assert!(
                    !rok.trim().is_empty(),
                    "{} carries an empty rok (st. 1 t. 6)",
                    activity.kljuc
                );
            }

            let distinct = register
                .iter()
                .filter_map(|activity| activity.rok_cuvanja.clone())
                .collect::<std::collections::BTreeSet<_>>();
            assert!(
                distinct.len() >= 5,
                "one blanket sentence repeated across every radnja is not a rok per category: \
                 {distinct:#?}"
            );

            // The class-backed roks say what the shared table stores, not a
            // second opinion about it.
            let connection = state.db().open().expect("database should open");
            let zaposleni = by_kljuc(&register, "evidencija_zaposlenih");
            assert_eq!(
                zaposleni.retention_record_class,
                Some(RecordClass::Personnel),
                "the ZEOR register's rok has to be traceable to the class that governs it"
            );
            let personnel = load_policy(&connection, RecordClass::Personnel)
                .expect("the personnel class is seeded");
            assert!(personnel.never_purge);
            assert!(
                zaposleni
                    .rok_cuvanja
                    .as_deref()
                    .expect("a rok")
                    .contains("trajno"),
                "the class is never_purge, so the register must read trajno: {:?}",
                zaposleni.rok_cuvanja
            );

            let prekovremeni = by_kljuc(&register, "prekovremeni_rad");
            let overtime = load_policy(&connection, RecordClass::WorktimeOvertimeLog)
                .expect("the overtime class is seeded");
            let floor = overtime
                .retain_until
                .expect("the overtime class has a floor");
            assert!(
                prekovremeni
                    .rok_cuvanja
                    .as_deref()
                    .expect("a rok")
                    .contains(&floor),
                "the register must print the floor the app actually applies ({floor}): {:?}",
                prekovremeni.rok_cuvanja
            );
        });
    }

    /// Req. 48. The breach log is itself a processing operation and gets its own
    /// entry; the same is true of the evidencija pristupa, which processes the
    /// employees' data in order to record who touched what. A register that
    /// silently omits the two logs the ZZPL trio introduced is the register an
    /// inspector reads against the product that is running.
    #[test]
    fn the_audit_log_and_the_breach_log_are_their_own_processing_operations() {
        with_state("cl47_logs_are_activities", |state| {
            sign_in_admin(state);
            let register = generate(state, NOW).expect("the register should generate");

            let pristup = by_kljuc(&register, KLJUC_EVIDENCIJA_PRISTUPA);
            assert_eq!(
                pristup.retention_record_class,
                Some(RecordClass::AccessLog),
                "the audit log's rok is the access-log class"
            );
            assert!(
                pristup.vrsta_lica.contains("Zaposlen"),
                "the audit log's data subjects are the employees whose actions it records: {}",
                pristup.vrsta_lica
            );
            // Req. 4, restated where the operator reads it: the exclusion list
            // is a property of the log and belongs in its register entry.
            for isključeno in ["JMBG", "adresa", "telefon", "pretrag"] {
                assert!(
                    pristup.vrsta_podataka.contains(isključeno),
                    "the register entry must name what the log never holds ({isključeno}): {}",
                    pristup.vrsta_podataka
                );
            }

            let povrede = by_kljuc(&register, KLJUC_EVIDENCIJA_POVREDA);
            assert!(
                povrede.svrha_obrade.contains("čl. 52 st. 6"),
                "the breach log's purpose is documenting every povreda under čl. 52 st. 6: {}",
                povrede.svrha_obrade
            );
            assert!(
                povrede.svrha_obrade.contains("okumentovanje"),
                "req. 48 words the purpose „dokumentovanje povreda“: {}",
                povrede.svrha_obrade
            );
            assert!(
                povrede
                    .opis_mera_zastite
                    .as_deref()
                    .is_some_and(|mere| mere.contains("administrator")),
                "req. 48: admin-only, and the register says so: {:?}",
                povrede.opis_mera_zastite
            );
            assert!(
                povrede.vrsta_lica.contains("broj"),
                "the breach record holds a COUNT of affected people, never a roster: {}",
                povrede.vrsta_lica
            );
        });
    }

    /// Čl. 47 st. 7 — „Evidencije iz st. 1, 3, 4. i 6. ovog člana vode se u
    /// pismenom obliku, što obuhvata i elektronski oblik i čuvaju se trajno.“
    /// This is the one record in the trio that is `trajno`, and the claim has to
    /// be backed by the shared retention table and by the transaction-level
    /// fence, not by prose alone.
    #[test]
    fn the_register_itself_is_kept_trajno() {
        with_state("cl47_register_is_trajno", |state| {
            sign_in_admin(state);
            let register = generate(state, NOW).expect("the register should generate");

            let connection = state.db().open().expect("database should open");
            let policy = load_policy(&connection, RecordClass::ProcessingRegister)
                .expect("the register's own class must be seeded");
            assert!(policy.never_purge, "čl. 47 st. 7 — trajno");
            assert_eq!(policy.retain_until, None, "trajno has no end date");
            assert!(
                !is_purgeable(&policy, "9999-12-31"),
                "no calendar date ever reaches the čl. 47 register"
            );

            assert!(
                NEVER_PURGE_TABLES.contains(&"processing_activities"),
                "the trajno claim needs the transaction-level fence behind it, not only a policy row"
            );

            let sopstveni = by_kljuc(&register, KLJUC_REGISTAR);
            assert_eq!(
                sopstveni.retention_record_class,
                Some(RecordClass::ProcessingRegister)
            );
            let rok = sopstveni.rok_cuvanja.as_deref().expect("a rok");
            assert!(rok.contains("trajno"), "{rok}");
            assert!(
                rok.contains("čl. 47 st. 7"),
                "the trajno claim carries the stav that makes it true: {rok}"
            );
        });
    }

    /// Req. 6 and req. 49, on the same page as req. 28 — and this is exactly the
    /// mistake generating the register makes easy. Neither the evidencija
    /// pristupa nor the evidencija povreda has a prescribed period, and copying
    /// čl. 47 st. 7's *trajno* onto either of them would put the product in
    /// permanent breach of storage limitation.
    #[test]
    fn no_log_without_a_prescribed_period_is_printed_as_trajno() {
        with_state("cl47_no_trajno_on_the_logs", |state| {
            sign_in_admin(state);
            let register = generate(state, NOW).expect("the register should generate");

            let connection = state.db().open().expect("database should open");

            for kljuc in [KLJUC_EVIDENCIJA_PRISTUPA, KLJUC_EVIDENCIJA_POVREDA] {
                let activity = by_kljuc(&register, kljuc);
                let rok = activity.rok_cuvanja.as_deref().expect("a rok");

                // A rok has two halves and they have to be asserted apart. The
                // statutory half is the template's static prose, and it already
                // says „ne čuva trajno“ for both logs — so any check the prose
                // can satisfy on its own says nothing about the half that
                // matters. Here the prose is asserted for what it is: a positive
                // requirement that the register disclaim st. 7 in words.
                assert!(
                    rok.contains("Zakon ne propisuje rok"),
                    "{kljuc} has no statutory period and the register has to say so: {rok}"
                );
                assert!(
                    rok.contains("ne čuva trajno"),
                    "{kljuc} must say in words that čl. 47 st. 7's trajno is not its rule: {rok}"
                );

                // The operative half: the period the program actually applies,
                // read out of `retention_policies` at generation time. This is
                // the half a mis-wired `Template::retention` moves, and the one
                // that would make the entry contradict itself.
                assert!(
                    !rok.contains("primenjuje: trajno"),
                    "{kljuc}: the period the program applies reads trajno — čl. 47 st. 7 \
                     prescribes trajno for the evidencija radnji obrade and for no other \
                     record: {rok}"
                );
                if let Some(class) = activity.retention_record_class {
                    let policy =
                        load_policy(&connection, class).expect("the wired class is seeded");
                    assert!(
                        !policy.never_purge,
                        "{kljuc} is wired to the `{}` class, whose never_purge puts it beyond \
                         every purge — that is čl. 47 st. 7's answer for the register alone",
                        class.key()
                    );
                }
            }
        });
    }

    /// The register is derived, so generating it twice must leave the database
    /// exactly as the first run left it — same rows, same `updated_at`, and no
    /// second čl. 48 line claiming a change that did not happen.
    #[test]
    fn regenerating_the_register_changes_nothing_and_logs_nothing() {
        with_state("cl47_regeneration_is_idempotent", |state| {
            sign_in_admin(state);

            let first = generate(state, NOW).expect("the register should generate");
            let audit_after_first = audit_rows(state);
            assert!(
                !audit_after_first.is_empty(),
                "the first generation writes the rows and records that it did"
            );

            let second = generate(state, KASNIJE).expect("regeneration should succeed");

            assert_eq!(first, second, "regeneration is a no-op on unchanged inputs");
            assert_eq!(
                audit_rows(state),
                audit_after_first,
                "a regeneration that changed nothing must not log a change"
            );

            let count: i64 = state
                .db()
                .open()
                .expect("database should open")
                .query_row("SELECT COUNT(*) FROM processing_activities", [], |row| {
                    row.get(0)
                })
                .expect("count should read");
            assert_eq!(
                count,
                first.len() as i64,
                "regeneration must upsert on `kljuc`, never append a second copy"
            );
        });
    }

    /// Čl. 47 st. 1 t. 5's second half — the safeguards for a transfer — is the
    /// one slot on this register the program does not know the answer to, and
    /// both prenos entries tell the operator in Serbian that *„ovde se upisuju
    /// naziv te države i opis mera zaštite prenosa“*. A register that invites a
    /// statutory entry and then erases it on the next launch is worse than one
    /// that never asked: the column belongs to whoever fills it in, so
    /// regeneration must leave it alone — and must not log a čl. 48 *menjanje*
    /// for a row it did not actually change.
    #[test]
    fn a_recorded_transfer_safeguard_survives_regeneration() {
        const MERE: &str =
            "Prenos u Irsku; standardne ugovorne klauzule i šifrovan saobraćaj (ZZPL čl. 65).";

        with_state("cl47_transfer_safeguard_survives", |state| {
            sign_in_admin(state);
            let first = generate(state, NOW).expect("the register should generate");
            let podrska_id = by_kljuc(&first, "tehnicka_podrska").id;

            state
                .db()
                .open()
                .expect("database should open")
                .execute(
                    "UPDATE processing_activities SET mere_zastite_prenosa = ?2 WHERE id = ?1",
                    rusqlite::params![podrska_id, MERE],
                )
                .expect("the st. 1 t. 5 safeguards slot should accept an entry");

            let audit_before = audit_rows(state);

            // Nothing the generator owns has changed, so this run has to be the
            // same no-op it is for every other row.
            let second = generate(state, KASNIJE).expect("regeneration should succeed");
            assert_eq!(
                by_kljuc(&second, "tehnicka_podrska")
                    .mere_zastite_prenosa
                    .as_deref(),
                Some(MERE),
                "regeneration erased the operator's čl. 47 st. 1 t. 5 safeguards"
            );
            assert_eq!(
                audit_rows(state),
                audit_before,
                "an entry the generator does not own is not a change it may claim in the \
                 evidencija pristupa"
            );

            // And a run that genuinely does rewrite the row must still leave the
            // column to its owner.
            crate::commands::settings::save_company_settings(
                state,
                crate::commands::settings::CompanySettingsRequest {
                    shop_name: "Butik Đurđevak".to_string(),
                    address: "Njegoševa 12, Beograd".to_string(),
                    pib: "111222333".to_string(),
                    registration_number: "64123456".to_string(),
                    phone: "0113456789".to_string(),
                    logo_path: None,
                    currency: "RSD".to_string(),
                },
            )
            .expect("company settings should save");

            let third = generate(state, KASNIJE).expect("regeneration should succeed");
            assert_eq!(
                by_kljuc(&third, "tehnicka_podrska")
                    .mere_zastite_prenosa
                    .as_deref(),
                Some(MERE),
                "a rewrite of the generated columns must not take the operator's entry with it"
            );
            assert_eq!(
                by_kljuc(&third, "tehnicka_podrska").rukovalac_naziv,
                "Butik Đurđevak",
                "…while the generated columns still follow the configuration"
            );
        });
    }

    /// This module's whole job is quoting čl. 47 correctly, so the one place it
    /// quotes st. 7 verbatim is pinned to the repo's verified text of that stav.
    /// The enumerated stavovi and the *pismeni oblik* limb are both part of the
    /// rule, and a quotation that drops either is a misquote inside the module
    /// least able to afford one.
    #[test]
    fn the_quoted_cl_47_st_7_matches_the_verified_text() {
        const SOURCE: &str = include_str!("cl47.rs");
        const VERIFIED_RULES: &str = include_str!("../../docs/REMAINING-SW-VERIFIED-RULES.md");

        let red = VERIFIED_RULES
            .lines()
            .find(|line| line.contains("**ZZPL čl. 47 st. 7**"))
            .expect("docs/REMAINING-SW-VERIFIED-RULES.md must carry the verified čl. 47 st. 7");
        let od = red.find('"').expect("the verified text is quoted") + 1;
        let do_ = red.rfind('"').expect("the verified text is quoted");
        let verified = &red[od..do_];
        assert!(
            verified.contains("čuvaju se trajno"),
            "the verified line was parsed wrong: {verified}"
        );

        // Doc comments wrap, so the comparison runs on the source with the
        // `///` markers stripped and every run of whitespace collapsed.
        let source = SOURCE
            .lines()
            .map(|line| line.trim().trim_start_matches("///"))
            .collect::<Vec<_>>()
            .join(" ");
        let source = source.split_whitespace().collect::<Vec<_>>().join(" ");

        assert!(
            source.contains(verified),
            "cl47.rs quotes ZZPL čl. 47 st. 7 as something other than the verified text. \
             The verified stav reads: „{verified}“"
        );
    }

    /// A changed input is the other half of idempotence: when the shop's own
    /// identity changes the register has to follow it, stamp the change and
    /// record it as a čl. 48 st. 1 *menjanje* — not as a fresh *unos*.
    #[test]
    fn a_changed_configuration_regenerates_the_row_and_records_a_menjanje() {
        with_state("cl47_changed_configuration", |state| {
            sign_in_admin(state);
            let first = generate(state, NOW).expect("the register should generate");
            let unosa = audit_rows(state).len();

            crate::commands::settings::save_company_settings(
                state,
                crate::commands::settings::CompanySettingsRequest {
                    shop_name: "Butik Đurđevak".to_string(),
                    address: "Njegoševa 12, Beograd".to_string(),
                    pib: "111222333".to_string(),
                    registration_number: "64123456".to_string(),
                    phone: "0113456789".to_string(),
                    logo_path: None,
                    currency: "RSD".to_string(),
                },
            )
            .expect("company settings should save");

            let second = generate(state, KASNIJE).expect("regeneration should succeed");

            assert!(
                second
                    .iter()
                    .all(|activity| activity.rukovalac_naziv == "Butik Đurđevak"),
                "st. 1 t. 1 follows the configured identity"
            );
            assert!(
                second.iter().all(|activity| activity.updated_at == KASNIJE),
                "a row that changed carries the instant it changed"
            );
            assert_ne!(first, second);

            let logged = audit_rows(state);
            assert_eq!(
                logged.len(),
                unosa + second.len(),
                "one line per changed radnja"
            );
            assert!(
                logged[unosa..]
                    .iter()
                    .all(|(action, object_type, _)| action == "menjanje"
                        && object_type == "processing_activity"),
                "a rewritten row is a čl. 48 st. 1 menjanje: {:?}",
                &logged[unosa..]
            );
        });
    }

    /// The register is generated off the retention table, so a class that holds
    /// personal data and appears nowhere in the register is a period the shop
    /// applies and never disclosed. This is the test that keeps the two in step
    /// when a later feature adds a class.
    ///
    /// **The boundary runs through `RecordClass::personal_data`, in both
    /// directions.** `retention_policies` is the app's shared retention table
    /// (SW11-SW15 §3 req. 42) and holds whatever period this app applies to
    /// anything; čl. 47 st. 1 records radnje obrade **podataka o ličnosti**. A
    /// class holding no personal data — SW-12's archive of published cenovnici
    /// is the first — therefore owes this register nothing, and putting it here
    /// would be a misstatement to the Poverenik in the shop's own name.
    #[test]
    fn every_configured_retention_class_reaches_the_register() {
        with_state("cl47_covers_every_retention_class", |state| {
            sign_in_admin(state);
            let register = generate(state, NOW).expect("the register should generate");

            for class in RecordClass::ALL {
                let disclosed = register
                    .iter()
                    .any(|activity| activity.retention_record_class == Some(class));
                assert_eq!(
                    disclosed,
                    class.personal_data(),
                    "`{}` holds {} personal data, so the čl. 47 register {} name it",
                    class.key(),
                    if class.personal_data() { "" } else { "no" },
                    if class.personal_data() {
                        "must"
                    } else {
                        "must not"
                    }
                );
            }
        });
    }

    /// The non-negotiable: `legal.rs` is the only module allowed to hold a fine
    /// figure. This document is handed to the shop and shown to the Poverenik,
    /// where a number reads as the shop's own statement of its exposure — and
    /// the čl. 95 st. 2 / st. 6 tiers are exactly the pair a generated file
    /// would get wrong.
    #[test]
    fn the_generated_register_carries_no_fine_figure() {
        with_state("cl47_no_fine_figure", |state| {
            sign_in_admin(state);
            let register = generate(state, NOW).expect("the register should generate");
            let company = CompanySettings::default();
            let haystack = format!(
                "{}\n{}",
                register_text(&register),
                render_html(&company, &register)
            );

            for figure in [
                "50.000",
                "100.000",
                "200.000",
                "500.000",
                "2.000.000",
                "RSD",
                "dinara",
            ] {
                assert!(
                    !haystack.contains(figure),
                    "the čl. 47 register must carry no fine figure and no currency amount \
                     („{figure}“) — legal.rs is the only module allowed to hold one"
                );
            }
        });
    }

    /// The export is what the čl. 47 st. 8 *uvid* and the Pravilnik 40/2019
    /// čl. 4 st. 1 *Prilog* are both made of, so it has to carry all seven st. 1
    /// elements, and it has to say plainly that the st. 4 obrađivač record is a
    /// different record kept by a different party.
    #[test]
    fn the_export_carries_the_seven_st_1_elements_and_disowns_the_st_4_record() {
        with_state("cl47_export", |state| {
            sign_in_admin(state);
            let exported = export(state, NOW).expect("the register should export");
            let html =
                std::fs::read_to_string(&exported.path).expect("the export should be on disk");

            assert_eq!(exported.mime_type, "text/html");
            assert!(
                html.contains("ZZPL čl. 47 st. 1"),
                "the export names the article it reproduces"
            );
            for tacka in [
                "1) Naziv i kontakt",
                "2) Svrha obrade",
                "3) Vrsta lica",
                "4) Vrsta primalaca",
                "5) Prenos",
                "6) Rok čuvanja",
                "7) Opšti opis mera zaštite",
            ] {
                assert!(html.contains(tacka), "st. 1 element missing: {tacka}");
            }
            assert!(
                html.contains("čl. 47 st. 4"),
                "the export has to place the obrađivač record"
            );
            assert!(
                html.contains("obrađivač"),
                "…and say whose record that is: it is not the shop's"
            );
            assert!(
                html.contains("čl. 47 st. 7"),
                "the export states that this register is kept trajno"
            );
            assert_eq!(exported.row_count, list(state).expect("a register").len());

            std::fs::remove_file(&exported.path).expect("the export should be removable");
        });
    }

    /// Generating the register touches the čl. 47 record, not personal data —
    /// so the čl. 48 line carries the row's internal id and nothing else. The
    /// register's own prose (which names the shop, its address and its phone)
    /// must never reach `audit_events`, whose exclusion list governs that table
    /// absolutely (req. 4).
    #[test]
    fn the_cl_48_line_carries_the_row_id_and_none_of_the_register_prose() {
        with_state("cl47_audit_line_is_opaque", |state| {
            sign_in_admin(state);
            crate::commands::settings::save_company_settings(
                state,
                crate::commands::settings::CompanySettingsRequest {
                    shop_name: "Butik Đurđevak".to_string(),
                    address: "Njegoševa 12, Beograd".to_string(),
                    pib: "111222333".to_string(),
                    registration_number: "64123456".to_string(),
                    phone: "0113456789".to_string(),
                    logo_path: None,
                    currency: "RSD".to_string(),
                },
            )
            .expect("company settings should save");

            let register = generate(state, NOW).expect("the register should generate");
            let logged = audit_rows(state);

            assert_eq!(
                logged.len(),
                register.len(),
                "one line per generated radnja"
            );
            for (action, object_type, object_id) in &logged {
                assert_eq!(action, "unos");
                assert_eq!(object_type, "processing_activity");
                assert!(
                    object_id.bytes().all(|byte| byte.is_ascii_digit()),
                    "the object id is an internal row id, never a key or a name: {object_id}"
                );
            }

            let text = audit_table_text(state);
            for tajna in [
                "Butik",
                "Đurđevak",
                "Njegoševa",
                "0113456789",
                "111222333",
                "evidencija_pristupa",
            ] {
                assert!(
                    !text.contains(tajna),
                    "„{tajna}“ reached the evidencija pristupa: {text}"
                );
            }
        });
    }

    /// The register names the shop, its address and its phone, and it is the
    /// document an inspector asks for. Reading, regenerating and exporting it
    /// are all administrator actions.
    #[test]
    fn the_register_is_admin_only() {
        with_state("cl47_admin_only", |state| {
            sign_in_admin(state);
            generate(state, NOW).expect("the register should generate");

            sign_in_cashier(state);
            for refusal in [
                list(state).err(),
                regenerate(state, NOW).err(),
                export(state, NOW).err(),
            ] {
                let error = refusal.expect("a kasir must not reach the čl. 47 register");
                assert!(
                    format!("{error:?}").contains("forbidden"),
                    "the refusal is the shared admin gate: {error:?}"
                );
            }
        });
    }
}
