//! The popis state machine, the PoP čl. 8 st. 5 blind-count property and the
//! čl. 13 st. 2 deadline engine (SW-16).
//!
//! Legal authority: `docs/REMAINING-SW-VERIFIED-RULES.md` §2c and §4 reqs. 29–42.
//! Design: `docs/superpowers/specs/2026-08-01-sw16-popis-design.md`.
//!
//! Everything here is pure: the state machine decides on the state it is handed
//! and the deadline engine on the dates it is handed. **No wall clock is read in
//! this module** — a statutory rok that moves with the machine's clock is not a
//! rok. Consumed by the command layer, so `dead_code` is allowed here — mirroring
//! the other domain modules.

#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use time::{Date, Duration};

use crate::app_error::AppError;
use crate::cash_deposit::parse_iso_date;

/// The six states of a popis, in the bylaw's own sequence. The stored values are
/// [`PopisStatus::as_db_str`] and they are the vocabulary of the v20
/// `popis_sessions.status` CHECK — a state this enum did not model would be
/// refused by neither the blind-count guard nor the posting lock, so the two
/// lists are asserted equal by test rather than kept in step by hand.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PopisStatus {
    /// The odluka, the komisija and the plan rada (čl. 8 st. 1–2) exist; nothing
    /// has been counted.
    Draft,
    /// Phase A. The natural count of čl. 9 st. 1 t. 1 is under way.
    Counting,
    /// The čl. 8 st. 5 signature is on the counted state. **This is the moment
    /// the book quantities may be released**, and nothing earlier is.
    CountedSigned,
    /// Phase B. Differences and valuation, čl. 9 st. 1 t. 3–6.
    Computed,
    /// The čl. 9 st. 3 signature on the printed, computed liste.
    ComputedSigned,
    /// Čl. 14 st. 3 — the result is knjižen. The last state; a correction is a
    /// new popis (ZoRač čl. 8 st. 4).
    Posted,
}

impl PopisStatus {
    pub const ALL: [Self; 6] = [
        Self::Draft,
        Self::Counting,
        Self::CountedSigned,
        Self::Computed,
        Self::ComputedSigned,
        Self::Posted,
    ];

    /// The value stored in `popis_sessions.status`.
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Counting => "counting",
            Self::CountedSigned => "counted_signed",
            Self::Computed => "computed",
            Self::ComputedSigned => "computed_signed",
            Self::Posted => "posted",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|status| status.as_db_str() == value)
    }

    /// What the state is called in front of the shop owner. Deliberately not the
    /// stored key: „counted_signed“ names a column value, and a refusal that
    /// quotes one is a refusal nobody in the shop can act on.
    pub fn naziv(self) -> &'static str {
        match self {
            Self::Draft => "priprema popisa",
            Self::Counting => "brojanje",
            Self::CountedSigned => "stvarno stanje potpisano",
            Self::Computed => "obračun razlika",
            Self::ComputedSigned => "obračunate liste potpisane",
            Self::Posted => "proknjižen popis",
        }
    }
}

/// The five events that move a popis. One per statutory step that changes the
/// legal character of the document — counting, the two potpisi, the obračun and
/// the knjiženje.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PopisEvent {
    /// Open Phase A: the commission starts the natural count (čl. 9 st. 1 t. 1).
    Count,
    /// The čl. 8 st. 5 signature on the counted state.
    SignA,
    /// Enter Phase B: differences and valuation (čl. 9 st. 1 t. 3–6).
    Compute,
    /// The čl. 9 st. 3 signature on the printed, computed liste.
    SignB,
    /// Čl. 14 st. 3 — book the result.
    Post,
}

impl PopisEvent {
    pub const ALL: [Self; 5] = [
        Self::Count,
        Self::SignA,
        Self::Compute,
        Self::SignB,
        Self::Post,
    ];

    pub fn naziv(self) -> &'static str {
        match self {
            Self::Count => "početak brojanja",
            Self::SignA => "potpis stvarnog stanja (čl. 8 st. 5)",
            Self::Compute => "obračun razlika",
            Self::SignB => "potpis obračunatih listi (čl. 9 st. 3)",
            Self::Post => "knjiženje rezultata (čl. 14 st. 3)",
        }
    }
}

/// The only legal path through a popis: `draft → counting → counted_signed →
/// computed → computed_signed → posted`, one event per arrow. Every other pair
/// is refused, and two refusals carry their own code because they are the two a
/// shop will actually meet.
///
/// The refusal that matters is `Compute` out of Phase A. Čl. 9 st. 1 orders the
/// steps and čl. 8 st. 5 makes the order a duty: the obračun reads book
/// quantities, so it may not begin until the counted state is written into the
/// liste and signed. Refusing it here is what keeps Phase B unreachable by
/// ordering rather than by screen sequence.
///
/// This function decides on `current` alone, which is a claim the caller makes.
/// It is therefore the *ordering* half of čl. 8 st. 5 and not the whole of it —
/// see [`book_quantities_released`] for the release condition, and the v20
/// triggers for the write refusal that stands behind both.
pub fn advance(current: PopisStatus, event: PopisEvent) -> Result<PopisStatus, AppError> {
    // Checked before the sequence, so a posted popis answers „the popis is
    // closed“ to every event rather than „that step is out of order“ — the same
    // order, and the same wording, the v20 posting lock uses.
    posting_lock(current)?;

    match (current, event) {
        (PopisStatus::Draft, PopisEvent::Count) => Ok(PopisStatus::Counting),
        (PopisStatus::Counting, PopisEvent::SignA) => Ok(PopisStatus::CountedSigned),
        (PopisStatus::CountedSigned, PopisEvent::Compute) => Ok(PopisStatus::Computed),
        (PopisStatus::Computed, PopisEvent::SignB) => Ok(PopisStatus::ComputedSigned),
        (PopisStatus::ComputedSigned, PopisEvent::Post) => Ok(PopisStatus::Posted),
        (PopisStatus::Draft | PopisStatus::Counting, PopisEvent::Compute) => {
            Err(AppError::business(
                "popis_not_signed",
                "Obračun ne može da počne pre nego što se stvarno stanje unese u popisne liste \
                 i pre nego što članovi komisije potpišu te liste (PoP čl. 8 st. 5).",
            ))
        }
        _ => Err(AppError::business(
            "popis_nedozvoljen_prelaz",
            format!(
                "Korak „{}“ nije moguć iz stanja „{}“.",
                event.naziv(),
                current.naziv()
            ),
        )),
    }
}

/// Req. 41 / PoP čl. 14 st. 3 with ZoRač čl. 8 st. 4 — the one refusal a posted
/// popis gives, to a transition and to a row write alike.
///
/// It lives here, and [`advance`] calls it, so that the module has **one**
/// definition of that sentence rather than one per caller: the command layer
/// refuses edits that are not transitions at all (a line write, a komisija row),
/// and a second hand-written copy of the wording would be free to drift from
/// this one and from the v20 trigger both spellings answer for.
pub fn posting_lock(status: PopisStatus) -> Result<(), AppError> {
    if status == PopisStatus::Posted {
        return Err(AppError::business(
            "popis_proknjizen",
            "Proknjižen popis se ne menja — ispravka se sprovodi novim popisom.",
        ));
    }

    Ok(())
}

/// The **status limb** of PoP čl. 8 st. 5 — false for every state in Phase A,
/// true from the counted state onward. Exactly the vocabulary the v20
/// `popis_lines` triggers refuse a book quantity in (`draft`, `counting`), so
/// the pure predicate and the engine guard cannot disagree about which states
/// are blind.
///
/// **This is one of the two limbs and it is module-private so that it cannot be
/// used alone to release book data.** `status` is a claim any UPDATE can make,
/// and a session can be born in `counted_signed` with no potpis behind it —
/// which is precisely the failure čl. 8 st. 5 names. The visibility is the
/// enforcement: a comment saying „call the other one“ is a convention, and this
/// project's stated failure mode is guards that are conventions rather than
/// mechanisms. [`book_quantities_released`] is the released interface.
fn book_quantities_visible(status: PopisStatus) -> bool {
    !matches!(status, PopisStatus::Draft | PopisStatus::Counting)
}

/// The whole čl. 8 st. 5 release condition. The article makes both facts
/// conditions — „пре уписивања стварног стања … и пре него што чланови комисије
/// … потпишу те листе“ — so both are required: the session must have left Phase
/// A **and** a `faza = 'a'` signature must exist for it. The čl. 9 st. 3 potpis
/// is a different event and does not stand in for it.
///
/// **This predicate governs every source of a book quantity in a popis response,
/// not only `popis_lines.knjigovodstvena_kolicina_milli`.** The v20 write guard
/// protects that one column and nothing else, so it is not the boundary — the
/// perpetual book stock lives in `inventory_balances.quantity_milli` and is
/// derivable from `inventory_movements`, and both are readable by any query at
/// any time, in any popis session, with no trigger in the way. A `popis_lines`
/// list or a count sheet that joins either of them to show an „expected“ or
/// „prema knjigama“ column during `draft`/`counting` breaches čl. 8 st. 5 with
/// every test in this repository still green. What the article forbids is book
/// data reaching the commission before it has counted and signed, whatever
/// table it is read out of; so when this predicate is false, a popis response
/// carries no book quantity from any source at all.
pub fn book_quantities_released(status: PopisStatus, phase_a_signed: bool) -> bool {
    book_quantities_visible(status) && phase_a_signed
}

/// The six popisne liste of req. 36: the ordinary lista of goods in the objekat,
/// and the five **posebne popisne liste** the bylaw requires wherever their
/// category is present. They are required, not optional extras — čl. 10 st. 3,
/// čl. 10 st. 4, čl. 11 st. 1, čl. 12 st. 2 and čl. 2 st. 5 each say the category
/// is entered on a lista of its own.
///
/// The stored values are [`PopisLista::as_db_str`] and they are the vocabulary of
/// the v20 `popis_lines.lista_vrsta` CHECK, asserted equal by test for the reason
/// [`PopisStatus`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PopisLista {
    /// The ordinary lista — the goods in the objekat, counted under čl. 9 st. 1
    /// t. 1. Not a posebna lista; everything below is.
    Roba,
    /// Oštećena, zastarela i neupotrebljiva roba — čl. 10 st. 3.
    Ostecena,
    /// Roba van objekta, uključujući robu na popravci i robu kod trećeg lica —
    /// čl. 10 st. 4.
    VanObjekta,
    /// Gotovina, popisana **po apoenima** — čl. 11 st. 1. The apoen is what makes
    /// this a denomination breakdown rather than a single figure in a drawer.
    Gotovina,
    /// Nedokumentovana potraživanja i obaveze — čl. 12 st. 2. Čl. 12 st. 1 counts
    /// the documented ones from the books; these are the ones with no verodostojna
    /// isprava behind them, and they get their own lista.
    Potrazivanja,
    /// Konsignaciona i svaka druga tuđa roba — čl. 2 st. 5, with the čl. 2 st. 6
    /// ten-day duty to put a signed copy of that lista in the owner's hands. See
    /// [`konsignacija_rok`].
    Konsignacija,
}

impl PopisLista {
    pub const ALL: [Self; 6] = [
        Self::Roba,
        Self::Ostecena,
        Self::VanObjekta,
        Self::Gotovina,
        Self::Potrazivanja,
        Self::Konsignacija,
    ];

    /// The value stored in `popis_lines.lista_vrsta`.
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Roba => "roba",
            Self::Ostecena => "ostecena",
            Self::VanObjekta => "van_objekta",
            Self::Gotovina => "gotovina",
            Self::Potrazivanja => "potrazivanja",
            Self::Konsignacija => "konsignacija",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|lista| lista.as_db_str() == value)
    }

    /// What the lista is called in front of the shop. The stored keys are
    /// ASCII-folded column values and no operator string may quote one.
    pub fn naziv(self) -> &'static str {
        match self {
            Self::Roba => "roba u objektu",
            Self::Ostecena => "oštećena, zastarela i neupotrebljiva roba",
            Self::VanObjekta => "roba van objekta (na popravci i kod trećeg lica)",
            Self::Gotovina => "gotovina po apoenima",
            Self::Potrazivanja => "nedokumentovana potraživanja i obaveze",
            Self::Konsignacija => "konsignaciona i druga tuđa roba",
        }
    }

    /// The provision that requires this lista. Carried with the lista rather than
    /// written into each refusal, so the shop is always sent to the same article
    /// for the same list.
    pub fn pravni_osnov(self) -> &'static str {
        match self {
            Self::Roba => "PoP čl. 9 st. 1 t. 1",
            Self::Ostecena => "PoP čl. 10 st. 3",
            Self::VanObjekta => "PoP čl. 10 st. 4",
            Self::Gotovina => "PoP čl. 11 st. 1",
            Self::Potrazivanja => "PoP čl. 12 st. 2",
            Self::Konsignacija => "PoP čl. 2 st. 5",
        }
    }

    /// True for the five liste req. 36 calls *posebne*. The ordinary roba lista is
    /// the popis itself and is not one of them.
    pub fn posebna(self) -> bool {
        !matches!(self, Self::Roba)
    }
}

/// The liste that were declared present and have nothing on them (req. 36).
///
/// **Why the presence is declared and not derived.** A posebna lista is required
/// „where the category applies“, and whether it applies is a fact about the shop
/// that no ledger in this database holds: nothing in the books says that some of
/// the stock is damaged, that a carton is at a third party's, or that a rail of
/// dresses belongs to somebody else on consignment. So the person taking the
/// popis answers for it, and this function is what makes that answer binding
/// rather than decorative — a category declared present whose lista is empty is
/// named here, and the izveštaj gate refuses on it.
///
/// The order is [`PopisLista::ALL`] and the result carries no duplicate, because
/// what comes back is read by a person: a caller that declared the same category
/// twice owes one lista, not two.
pub fn nedostajuce_liste(
    prijavljene: &[PopisLista],
    sa_stavkama: &[PopisLista],
) -> Vec<PopisLista> {
    PopisLista::ALL
        .into_iter()
        .filter(|lista| prijavljene.contains(lista) && !sa_stavkama.contains(lista))
        .collect()
}

/// The gate req. 36 puts in front of the izveštaj: **a category the shop declared
/// present may not go to the izveštaj on an empty lista.**
///
/// Čl. 13 st. 1 has the izveštaj report the stvarno stanje, the knjigovodstveno
/// stanje and the razlike — of the whole popis. A popis that declared damaged
/// goods, goods at a third party's or cash on hand and then wrote none of them
/// down reports a stvarno stanje that is not the shop's, and every figure derived
/// from it is wrong by whatever was left out. So the declaration is checked
/// against the liste before an izveštaj can rest on them.
///
/// Pure, and separate from the report [`nedostajuce_liste`] produces, because the
/// two are read by different things: a screen wants the missing liste to render
/// them, and a generator wants to be stopped. **Task 6 calls this before it
/// composes the izveštaj** — that is what makes the declaration binding rather
/// than decorative.
pub fn ensure_liste_kompletne(
    prijavljene: &[PopisLista],
    sa_stavkama: &[PopisLista],
) -> Result<(), AppError> {
    let nedostaju = nedostajuce_liste(prijavljene, sa_stavkama);
    if nedostaju.is_empty() {
        return Ok(());
    }

    let spisak = nedostaju
        .iter()
        .map(|lista| format!("„{}“ ({})", lista.naziv(), lista.pravni_osnov()))
        .collect::<Vec<_>>()
        .join(", ");

    Err(AppError::business(
        "popis_prazna_prijavljena_lista",
        format!(
            "Popisne liste nisu potpune: prijavljeno je da postoji {spisak} — a te liste su \
             prazne. Popunite ih pre sastavljanja izveštaja o popisu (PoP čl. 13 st. 1)."
        ),
    ))
}

/// The two popis obligations, which are two modes and not one mode with a flag:
/// the annual popis at the balance date (ZoRač čl. 20 st. 2) and the popis on a
/// retail price change in a maloprodajni objekat (ZoRač čl. 21, PoP čl. 3). They
/// carry **different statutory roks** — see [`izvestaj_due`] — so a session whose
/// vrsta is unknown can be given neither.
///
/// The stored values are [`PopisVrsta::as_db_str`] and they are the vocabulary of
/// the v20 `popis_sessions.vrsta` CHECK, asserted equal by test for the same
/// reason [`PopisStatus`] is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PopisVrsta {
    /// ZoRač čl. 20 st. 2 — the popis na datum bilansa, taken when the redovni
    /// godišnji finansijski izveštaj is prepared.
    Godisnji,
    /// ZoRač čl. 21 — the popis on a change of retail selling prices. The most
    /// POS-relevant trigger in the module, and an in-year popis for čl. 13 st. 2.
    Nivelacioni,
}

impl PopisVrsta {
    pub const ALL: [Self; 2] = [Self::Godisnji, Self::Nivelacioni];

    /// The value stored in `popis_sessions.vrsta`.
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Godisnji => "godisnji",
            Self::Nivelacioni => "nivelacioni",
        }
    }

    pub fn from_db_str(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|vrsta| vrsta.as_db_str() == value)
    }

    /// What the mode is called in front of the shop owner — the stored keys are
    /// ASCII-folded column values and no operator string may quote one.
    pub fn naziv(self) -> &'static str {
        match self {
            Self::Godisnji => "godišnji popis",
            Self::Nivelacioni => "popis po nivelaciji",
        }
    }
}

/// PoP čl. 13 st. 2, first limb — the izveštaj about the annual popis is due
/// „najkasnije 60 dana **pre** isteka roka za dostavljanje redovnog godišnjeg
/// finansijskog izveštaja“.
pub const IZVESTAJ_ROK_DANA_PRE_PREDAJE_FI: i64 = 60;

/// PoP čl. 13 st. 2, second limb — for a popis taken during the year the izveštaj
/// is due „najkasnije 30 dana **po** izvršenom popisu“.
pub const IZVESTAJ_ROK_DANA_PO_POPISU: i64 = 30;

/// The rok for the izveštaj o popisu, as `gggg-MM-dd` (req. 38).
///
/// **Computed, never tabulated.** PoP čl. 13 st. 2 gives two limbs and this
/// function is both of them: an annual popis counts
/// [`IZVESTAJ_ROK_DANA_PRE_PREDAJE_FI`] days back from the FS filing deadline, an
/// in-year popis counts [`IZVESTAJ_ROK_DANA_PO_POPISU`] days forward from the
/// count. Each limb reads only its own anchor, because that is all the article
/// gives it: the annual rok does not move when the count is taken early, and the
/// nivelacija rok does not know the filing deadline exists.
///
/// **Why a table is wrong and not merely inelegant.** 31 March is day 90 of a
/// common year and day 91 of a leap one, so with the ZoRač čl. 44 st. 1 rok the
/// answer is 30 January 2027 for FY2026, **31 January 2028** for FY2027 and 30
/// January 2029 for FY2028. A list typed out by hand is wrong in the leap year
/// and right on either side of it.
///
/// **Why the filing deadline is a parameter.** ZoRač čl. 44 st. 1 sets 31 March
/// *„osim ako posebnim zakonom nije drukčije uređeno“* — the rok is not this
/// module's to own, and a statutory or special-law change must be a configuration
/// edit rather than a code change. The caller supplies it; `None` is refused for
/// the annual limb rather than filled in with a guess.
///
/// **Čl. 14 st. 2 rides on this same date.** The odluka o usvajanju izveštaja is
/// due *„u roku iz člana 13. stav 2“*, so it is not a second deadline and gets no
/// second function: the izveštaj and the odluka are one milestone (req. 38), and
/// they are surfaced from this one answer.
///
/// **No clock is read.** The rok is a function of the two dates it is given, and
/// of nothing else — including today.
pub fn izvestaj_due(
    vrsta: PopisVrsta,
    datum_popisa: &str,
    fs_filing_deadline: Option<&str>,
) -> Result<String, AppError> {
    let rok = match vrsta {
        PopisVrsta::Godisnji => {
            let filing = fs_filing_deadline.ok_or_else(|| {
                AppError::business(
                    "popis_rok_predaje_fi_nepoznat",
                    "Rok za izveštaj o godišnjem popisu se računa unazad od roka za dostavljanje \
                     redovnog godišnjeg finansijskog izveštaja (PoP čl. 13 st. 2), a taj rok nije \
                     podešen.",
                )
            })?;

            popis_datum(
                filing,
                "rok za dostavljanje redovnog godišnjeg finansijskog izveštaja",
            )?
            .checked_sub(Duration::days(IZVESTAJ_ROK_DANA_PRE_PREDAJE_FI))
        }
        PopisVrsta::Nivelacioni => popis_datum(datum_popisa, "datum popisa")?
            .checked_add(Duration::days(IZVESTAJ_ROK_DANA_PO_POPISU)),
    };

    // `checked_*` rather than the panicking arithmetic, and `iso_datum` rather
    // than a bare `format!`: the inputs are dates the shop typed or a setting
    // holds, and a rok far enough out to leave the calendar — in either direction
    // — must be a refusal the owner can read, not a crash and not a string that
    // no date column would accept handed back as an answer.
    rok.and_then(iso_datum).ok_or_else(|| {
        AppError::business(
            "popis_rok_van_kalendara",
            format!(
                "Rok za izveštaj o popisu („{}“) izlazi iz kalendara — proverite datum popisa i \
                 rok za dostavljanje finansijskog izveštaja.",
                vrsta.naziv()
            ),
        )
    })
}

/// PoP čl. 2 st. 6 — the signed posebna popisna lista for tuđa roba reaches its
/// owner „најкасније у року од десет дана од дана на који је попис извршен“.
pub const KONSIGNACIJA_ROK_DANA: i64 = 10;

/// The rok for putting the signed konsignaciona popisna lista in the owner's
/// hands, as `gggg-MM-dd` (req. 36).
///
/// **Anchored on the count date and on nothing else.** Čl. 2 st. 6 says „од дана
/// на који је попис извршен“, so neither potpis moves it, the knjiženje does not
/// move it, and today does not move it — a rok that slid with the app's clock
/// would tell a shop that has already missed it that it has ten days left.
///
/// Computed the same way [`izvestaj_due`] is and refused the same way: an
/// unreadable count date does not become a guessed rok, because this one ends in
/// another person's hands and the shop plans a delivery around it.
pub fn konsignacija_rok(datum_popisa: &str) -> Result<String, AppError> {
    popis_datum(datum_popisa, "datum popisa")?
        .checked_add(Duration::days(KONSIGNACIJA_ROK_DANA))
        .and_then(iso_datum)
        .ok_or_else(|| {
            AppError::business(
                "popis_rok_van_kalendara",
                "Rok za dostavljanje potpisane popisne liste vlasniku tuđe robe izlazi iz \
                 kalendara — proverite datum popisa.",
            )
        })
}

/// One date in, strictly `gggg-MM-dd` — exactly the shape every date column in
/// the v20 schema is GLOB-checked into. Strict on purpose: '2027-3-31' and an
/// RFC3339 stamp are both refused rather than truncated or repaired, because a
/// rok this module invents from a malformed input is one the shop will file on.
fn popis_datum(value: &str, polje: &str) -> Result<Date, AppError> {
    parse_iso_date(value).ok_or_else(|| {
        AppError::business(
            "popis_rok_neispravan_datum",
            format!(
                "Polje „{polje}“ mora da bude datum u obliku gggg-MM-dd, a primljeno je „{value}“."
            ),
        )
    })
}

/// The rok as `gggg-MM-dd`, or `None` when the computed date cannot be written
/// in that shape at all.
///
/// The shape is a precondition and not a formatting detail. `{:04}` renders year
/// −1 as `-001`, so an engine that formatted unconditionally would answer
/// „-001-12-03“ — neither `gggg-MM-dd` nor anything the v20 date columns' GLOB
/// accepts — and would answer it as a **success**, handing the caller a rok that
/// cannot be stored with no reason to look twice. Year 0 formats cleanly but is
/// refused alongside it: it is not a year any filing rok falls in, and „a real
/// Gregorian year“ is one rule where „a year that happens to format“ is two.
fn iso_datum(date: Date) -> Option<String> {
    (date.year() >= 1).then(|| {
        format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            u8::from(date.month()),
            date.day()
        )
    })
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};

    use super::{
        advance, book_quantities_released, book_quantities_visible, ensure_liste_kompletne,
        izvestaj_due, konsignacija_rok, nedostajuce_liste, PopisEvent, PopisLista, PopisStatus,
        PopisVrsta,
    };
    use crate::app_error::AppError;
    use crate::db::{test_database_path, Db};

    fn with_test_database(test_name: &str, test: impl FnOnce(&Connection)) {
        let path = test_database_path(test_name);

        {
            let db = Db::new(&path).expect("database should initialize");
            let connection = db.open().expect("database should open");
            test(&connection);
        }

        std::fs::remove_file(&path).expect("test database should be removed");
    }

    /// Seed one popis session directly in `status`. `posted_at` follows the
    /// status because the v20 schema binds the two.
    fn seed_session(connection: &Connection, id: i64, status: PopisStatus) {
        seed_session_of(connection, id, PopisVrsta::Godisnji, status);
    }

    fn seed_session_of(connection: &Connection, id: i64, vrsta: PopisVrsta, status: PopisStatus) {
        let posted_at: Option<&str> =
            (status == PopisStatus::Posted).then_some("2027-01-15T18:00:00Z");

        connection
            .execute(
                "INSERT INTO popis_sessions (id, vrsta, prodajno_mesto, datum_popisa, status,
                                             posted_at, created_at, updated_at)
                 VALUES (?1, ?2, 'Butik Centar', '2026-12-31', ?3, ?4,
                         '2026-12-31T08:00:00Z', '2026-12-31T08:00:00Z')",
                params![id, vrsta.as_db_str(), status.as_db_str(), posted_at],
            )
            .unwrap_or_else(|error| {
                panic!("a {vrsta:?} session in {status:?} should insert: {error}")
            });
    }

    /// Take the čl. 8 st. 5 potpis — the second limb of the release condition.
    fn sign_phase_a(connection: &Connection, session_id: i64) {
        connection
            .execute(
                "INSERT INTO popis_signatures (session_id, faza, potpisnik, potpisano_at,
                                               snapshot_hash, created_at)
                 VALUES (?1, 'a', 'Miloš Đurđević', '2026-12-31T17:00:00Z', 'hash-a',
                         '2026-12-31T17:00:00Z')",
                params![session_id],
            )
            .unwrap_or_else(|error| panic!("the čl. 8 st. 5 signature should record: {error}"));
    }

    /// Try to store a book quantity — the payload čl. 8 st. 5 governs.
    fn write_book_quantity(connection: &Connection, session_id: i64) -> rusqlite::Result<usize> {
        connection.execute(
            "INSERT INTO popis_lines (session_id, lista_vrsta, naziv, stvarna_kolicina_milli,
                                      knjigovodstvena_kolicina_milli, created_at, updated_at)
             VALUES (?1, 'roba', 'Košulja', 7000, 6000,
                     '2026-12-31T09:00:00Z', '2026-12-31T09:00:00Z')",
            params![session_id],
        )
    }

    fn refusal_message(result: rusqlite::Result<usize>) -> String {
        match result {
            Err(rusqlite::Error::SqliteFailure(_, Some(message))) => message,
            other => panic!("expected the write to be refused, got {other:?}"),
        }
    }

    /// The `'…'` tokens of a closed CHECK, read back off the live schema rather
    /// than copied out of the migration source.
    fn check_vocabulary(connection: &Connection, table: &str, column: &str) -> Vec<String> {
        let schema: String = connection
            .query_row(
                "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
                [table],
                |row| row.get(0),
            )
            .unwrap_or_else(|error| panic!("the {table} schema should load: {error}"));

        let opening = format!("CHECK ({column} IN (");
        let start = schema
            .find(&opening)
            .unwrap_or_else(|| panic!("{table} must carry a closed {column} CHECK"))
            + opening.len();
        let rest = &schema[start..];
        let end = rest.find("))").expect("the CHECK should close");

        let mut values: Vec<String> = rest[..end]
            .split(',')
            .map(|token| token.trim().trim_matches('\'').to_string())
            .collect();
        values.sort();
        values
    }

    /// PoP čl. 8 st. 5: book quantities must not reach the commission before the
    /// counted state is written and signed. A UI that merely hides them is not
    /// enough — the property belongs to the state machine.
    #[test]
    fn book_quantities_are_invisible_until_phase_a_is_signed() {
        for status in [PopisStatus::Draft, PopisStatus::Counting] {
            assert!(!book_quantities_visible(status), "{status:?} must be blind");
        }
        for status in [
            PopisStatus::CountedSigned,
            PopisStatus::Computed,
            PopisStatus::ComputedSigned,
            PopisStatus::Posted,
        ] {
            assert!(book_quantities_visible(status), "{status:?} may reveal");
        }
    }

    #[test]
    fn phase_b_cannot_be_entered_without_the_phase_a_signature() {
        let error = advance(PopisStatus::Counting, PopisEvent::Compute)
            .expect_err("computing before the čl. 8 st. 5 signature must be refused");
        assert!(matches!(
            error,
            AppError::Business {
                code: "popis_not_signed",
                ..
            }
        ));
    }

    #[test]
    fn a_posted_popis_admits_no_further_transition() {
        for event in [
            PopisEvent::Count,
            PopisEvent::Compute,
            PopisEvent::SignA,
            PopisEvent::SignB,
            PopisEvent::Post,
        ] {
            assert!(
                advance(PopisStatus::Posted, event).is_err(),
                "{event:?} after posting"
            );
        }
    }

    /// Čl. 9 st. 1 orders the steps and čl. 8 st. 5 makes the order a duty, so
    /// the sequence is walked end to end rather than spot-checked.
    #[test]
    fn the_bylaw_sequence_is_the_only_path_to_posted() {
        let mut status = PopisStatus::Draft;

        for (event, expected) in [
            (PopisEvent::Count, PopisStatus::Counting),
            (PopisEvent::SignA, PopisStatus::CountedSigned),
            (PopisEvent::Compute, PopisStatus::Computed),
            (PopisEvent::SignB, PopisStatus::ComputedSigned),
            (PopisEvent::Post, PopisStatus::Posted),
        ] {
            status = advance(status, event)
                .unwrap_or_else(|error| panic!("{event:?} should be legal here: {error}"));
            assert_eq!(status, expected, "after {event:?}");
        }
    }

    /// Exhaustive over all 30 (status, event) pairs: the five arrows above are
    /// the whole machine and every one of the other 25 is refused. A spot-check
    /// would leave the interesting ones — signing B before A, re-counting a
    /// counted popis, posting an unsigned one — to be discovered in the shop.
    #[test]
    fn every_pair_outside_the_sequence_is_refused() {
        let sequence = [
            (PopisStatus::Draft, PopisEvent::Count),
            (PopisStatus::Counting, PopisEvent::SignA),
            (PopisStatus::CountedSigned, PopisEvent::Compute),
            (PopisStatus::Computed, PopisEvent::SignB),
            (PopisStatus::ComputedSigned, PopisEvent::Post),
        ];

        for status in PopisStatus::ALL {
            for event in PopisEvent::ALL {
                if sequence.contains(&(status, event)) {
                    continue;
                }

                let error = advance(status, event)
                    .expect_err("only the bylaw's own sequence may be walked");
                let expected_code = match (status, event) {
                    (PopisStatus::Posted, _) => "popis_proknjizen",
                    (PopisStatus::Draft | PopisStatus::Counting, PopisEvent::Compute) => {
                        "popis_not_signed"
                    }
                    _ => "popis_nedozvoljen_prelaz",
                };
                assert_eq!(error.code(), expected_code, "{status:?} + {event:?}");
            }
        }
    }

    /// Req. 29's actual shape. `status` is a claim any UPDATE can make — a
    /// session can be born in `counted_signed` — so the status limb alone would
    /// hand the book data to a commission that never signed anything, which is
    /// the failure čl. 8 st. 5 names. Both limbs, always.
    #[test]
    fn the_status_limb_alone_does_not_release_the_book_data() {
        for status in PopisStatus::ALL {
            assert!(
                !book_quantities_released(status, false),
                "{status:?} without the čl. 8 st. 5 potpis must stay blind"
            );
            assert_eq!(
                book_quantities_released(status, true),
                book_quantities_visible(status),
                "{status:?} with the potpis must follow the status limb"
            );
        }

        assert!(
            book_quantities_visible(PopisStatus::CountedSigned)
                && !book_quantities_released(PopisStatus::CountedSigned, false),
            "a session claiming counted_signed with no potpis behind it must be refused the data"
        );
    }

    /// The enum and the v20 CHECK are one vocabulary. A state the engine stores
    /// but this enum cannot name would reach [`advance`] as nothing at all, and
    /// a state this enum names but the engine refuses would be a transition the
    /// machine allows and the database rejects.
    #[test]
    fn the_status_vocabulary_is_exactly_the_schema_check() {
        with_test_database("popis_status_vocabulary", |connection| {
            let mut modelled: Vec<String> = PopisStatus::ALL
                .into_iter()
                .map(|status| status.as_db_str().to_string())
                .collect();
            modelled.sort();

            assert_eq!(
                check_vocabulary(connection, "popis_sessions", "status"),
                modelled
            );

            for (index, status) in PopisStatus::ALL.into_iter().enumerate() {
                seed_session(connection, 100 + index as i64, status);
                assert_eq!(
                    PopisStatus::from_db_str(status.as_db_str()),
                    Some(status),
                    "{status:?} should survive the round trip through its stored value"
                );
                // The third mapping. `as_db_str` is a hand-written literal while
                // the wire form is derived from the variant *identifier*, so a
                // rename moves one and not the other: `CountedSigned` →
                // `SignedCount` would send `signed_count` over IPC while the
                // column kept `counted_signed`, and every assertion above would
                // still pass. Pinning the two to each other puts the wire form
                // behind the same schema anchor as the stored form.
                assert_eq!(
                    serde_json::to_string(&status).expect("a status should serialize"),
                    format!("\"{}\"", status.as_db_str()),
                    "{status:?} must reach the frontend as its stored value"
                );
            }

            assert!(
                PopisStatus::from_db_str("counted").is_none(),
                "an unknown stored value must not resolve to a state"
            );
        });
    }

    /// `PopisEvent` never reaches a column, so nothing in the schema can anchor
    /// it the way the status CHECK anchors `PopisStatus` — its only contract is
    /// the IPC wire form, and that form is derived from the variant identifier,
    /// so a rename silently rewrites it. Written out here so the frontend's
    /// half of the contract has to be changed deliberately rather than by
    /// renaming a variant.
    #[test]
    fn the_event_wire_form_is_the_frontend_contract() {
        for (event, wire) in [
            (PopisEvent::Count, "count"),
            (PopisEvent::SignA, "sign_a"),
            (PopisEvent::Compute, "compute"),
            (PopisEvent::SignB, "sign_b"),
            (PopisEvent::Post, "post"),
        ] {
            assert_eq!(
                serde_json::to_string(&event).expect("an event should serialize"),
                format!("\"{wire}\""),
                "{event:?} must reach the frontend as {wire}"
            );
            assert_eq!(
                serde_json::from_str::<PopisEvent>(&format!("\"{wire}\""))
                    .expect("the frontend's own string must deserialize"),
                event,
                "{wire} must come back from the frontend as {event:?}"
            );
        }
    }

    /// The pure predicate and the v20 write guard must not be able to disagree
    /// about which states are blind: the predicate is what a screen or a query
    /// consults, the guard is what actually stops the row being written, and a
    /// divergence between them is a book quantity in the commission's hands.
    #[test]
    fn the_release_predicate_agrees_with_the_engine_guard() {
        with_test_database("popis_release_predicate", |connection| {
            for (index, status) in PopisStatus::ALL.into_iter().enumerate() {
                if status == PopisStatus::Posted {
                    // Readable (the quantities are in the signed liste) but not
                    // writable: req. 41 closes the row, not the column. The
                    // refusal must say so rather than blame čl. 8 st. 5.
                    seed_session(connection, 300, status);
                    let message = refusal_message(write_book_quantity(connection, 300));
                    assert!(
                        message.contains("Proknjižen popis se ne menja"),
                        "a posted popis must refuse the write as closed, said: {message}"
                    );
                    continue;
                }

                let unsigned = 200 + index as i64 * 2;
                let signed = unsigned + 1;
                seed_session(connection, unsigned, status);
                seed_session(connection, signed, status);
                sign_phase_a(connection, signed);

                assert!(
                    !book_quantities_released(status, false),
                    "{status:?} with no potpis must be blind"
                );
                let message = refusal_message(write_book_quantity(connection, unsigned));
                assert!(
                    message.contains("PoP čl. 8 st. 5"),
                    "{status:?} with no potpis must be refused on čl. 8 st. 5, said: {message}"
                );

                let written = write_book_quantity(connection, signed);
                if book_quantities_released(status, true) {
                    written.unwrap_or_else(|error| {
                        panic!("{status:?} with the potpis should admit the book data: {error}")
                    });
                } else {
                    let message = refusal_message(written);
                    assert!(
                        message.contains("PoP čl. 8 st. 5"),
                        "{status:?} is in Phase A, so even a potpis must not release it, \
                         said: {message}"
                    );
                }
            }
        });
    }

    // ---------------------------------------------------------------------
    // The deadline engine (req. 38)
    // ---------------------------------------------------------------------

    fn due(vrsta: PopisVrsta, datum_popisa: &str, filing: Option<&str>) -> String {
        izvestaj_due(vrsta, datum_popisa, filing).unwrap_or_else(|error| {
            panic!("{vrsta:?} on {datum_popisa} should have a rok: {error}")
        })
    }

    /// PoP čl. 13 st. 2, first limb: „najkasnije 60 dana pre isteka roka za
    /// dostavljanje redovnog godišnjeg finansijskog izveštaja“. With the ZoRač
    /// čl. 44 st. 1 rok of 31 March, three consecutive years are asserted
    /// because **31 March is day 90 of a common year and day 91 of a leap one**:
    /// FY2027 is filed in 2028 and its rok lands on 31 January, a day later than
    /// the neighbours on either side. A table typed out by hand is wrong in
    /// exactly that year and right in the other two, so one year would not have
    /// caught it.
    #[test]
    fn the_annual_izvestaj_is_due_sixty_days_before_the_filing_deadline() {
        for (fiscal_year, filing, expected) in [
            (2026, "2027-03-31", "2027-01-30"),
            (2027, "2028-03-31", "2028-01-31"),
            (2028, "2029-03-31", "2029-01-30"),
        ] {
            let balance_date = format!("{fiscal_year}-12-31");
            assert_eq!(
                due(PopisVrsta::Godisnji, &balance_date, Some(filing)),
                expected,
                "FY{fiscal_year} filed by {filing}"
            );
        }
    }

    /// The annual rok counts back from the filing deadline, not forward from the
    /// count — čl. 13 st. 2 anchors it on the rok za dostavljanje. An engine that
    /// quietly anchored on `datum_popisa` would agree with the test above every
    /// time the popis happened to fall on 31 December, which for the annual popis
    /// is nearly always (ZoRač čl. 20 st. 2, „na datum bilansa“).
    #[test]
    fn the_annual_rok_does_not_move_with_the_count_date() {
        let anchored = due(PopisVrsta::Godisnji, "2026-12-31", Some("2027-03-31"));

        for datum_popisa in ["2026-11-02", "2026-12-31", "2027-01-08"] {
            assert_eq!(
                due(PopisVrsta::Godisnji, datum_popisa, Some("2027-03-31")),
                anchored,
                "a popis counted on {datum_popisa} must not move the čl. 13 st. 2 rok"
            );
        }
    }

    /// PoP čl. 13 st. 2, second limb: „najkasnije 30 dana po izvršenom popisu“ —
    /// the in-year rule the nivelacija popis of ZoRač čl. 21 runs on. The four
    /// cases cross a month end, a year end and a 29 February, so the 30 are real
    /// calendar days and not „a month“: the same 5 February is 6 March in the
    /// leap year 2028 and 7 March in 2027.
    #[test]
    fn the_nivelacija_izvestaj_is_due_thirty_days_after_the_count() {
        for (datum_popisa, expected) in [
            ("2026-11-20", "2026-12-20"),
            ("2026-12-15", "2027-01-14"),
            ("2028-02-05", "2028-03-06"),
            ("2027-02-05", "2027-03-07"),
        ] {
            assert_eq!(
                due(PopisVrsta::Nivelacioni, datum_popisa, None),
                expected,
                "a nivelacija counted on {datum_popisa}"
            );
        }
    }

    /// Req. 38's own words: the filing deadline is **a parameter**, so a change
    /// to ZoRač čl. 44 st. 1 — or the „osim ako posebnim zakonom nije drukčije
    /// uređeno“ escape in that very sentence — is a configuration edit rather
    /// than a code change. Moving the filing deadline moves the rok by exactly
    /// as much, across months of three different lengths.
    #[test]
    fn the_filing_deadline_is_a_parameter_and_not_a_table() {
        for (filing, expected) in [
            ("2027-03-31", "2027-01-30"),
            ("2027-04-30", "2027-03-01"),
            ("2027-06-30", "2027-05-01"),
        ] {
            assert_eq!(
                due(PopisVrsta::Godisnji, "2026-12-31", Some(filing)),
                expected,
                "a filing deadline of {filing}"
            );
        }
    }

    /// The two limbs share one function, so each must read only its own anchor.
    /// A fall-through in this direction is the dangerous one: it would hand an
    /// in-year nivelacija the annual rok — months later than the 30 days čl. 13
    /// st. 2 allows it — and the shop would file late believing the app.
    #[test]
    fn the_nivelacija_rok_ignores_the_filing_deadline() {
        let bez_roka = due(PopisVrsta::Nivelacioni, "2026-11-20", None);

        assert_eq!(bez_roka, "2026-12-20");
        for filing in ["2027-03-31", "2027-06-30"] {
            assert_eq!(
                due(PopisVrsta::Nivelacioni, "2026-11-20", Some(filing)),
                bez_roka,
                "the čl. 13 st. 2 in-year limb must not read the filing deadline ({filing})"
            );
        }
    }

    /// A rok that is silently wrong is worse than one the shop is told cannot be
    /// computed: the čl. 58 prekršaj attaches to the popis, and a date this
    /// module invents is one the owner will file on. Every unreadable input is
    /// refused, including the two near-misses — a one-digit month and an RFC3339
    /// stamp — that a lenient parser would accept or truncate.
    #[test]
    fn an_unreadable_date_is_refused_rather_than_guessed() {
        for (vrsta, datum_popisa, filing) in [
            (PopisVrsta::Godisnji, "2026-12-31", Some("2027-3-31")),
            (PopisVrsta::Godisnji, "2026-12-31", Some("2027-02-30")),
            (
                PopisVrsta::Godisnji,
                "2026-12-31",
                Some("2027-03-31T00:00:00Z"),
            ),
            (PopisVrsta::Godisnji, "2026-12-31", Some("")),
            (PopisVrsta::Nivelacioni, "31.12.2026.", None),
            (PopisVrsta::Nivelacioni, "2026-12-31T08:00:00Z", None),
            (PopisVrsta::Nivelacioni, "", None),
        ] {
            let error = izvestaj_due(vrsta, datum_popisa, filing)
                .expect_err("an unreadable date must not produce a rok");
            assert_eq!(
                error.code(),
                "popis_rok_neispravan_datum",
                "{vrsta:?} / {datum_popisa} / {filing:?}"
            );
        }
    }

    /// The annual limb has nothing to count back from until the filing rok is
    /// configured. It refuses, and the refusal names both the missing setting
    /// and the article — a shop that is told only „greška“ cannot act on it.
    #[test]
    fn the_annual_rok_refuses_when_the_filing_deadline_is_unknown() {
        let error = izvestaj_due(PopisVrsta::Godisnji, "2026-12-31", None)
            .expect_err("the annual rok cannot be computed without the filing deadline");

        assert_eq!(error.code(), "popis_rok_predaje_fi_nepoznat");
        let message = error.to_string();
        assert!(
            message.contains("čl. 13 st. 2") && message.contains("finansijskog izveštaja"),
            "the refusal must tell the shop what is missing and under which article, said: \
             {message}"
        );
    }

    /// The third refusal code, in both directions. `checked_add` catches the
    /// overflow, but the underflow is the one that bites: `{:04}` renders year −1
    /// as `-001`, so an unguarded engine hands back „-001-12-03“ — not
    /// `gggg-MM-dd`, not a value the v20 GLOB checks accept, and therefore a rok
    /// that **cannot be stored being returned as a success**. A refusal path that
    /// returns `Ok` is worse than no refusal path at all, because the caller has
    /// no reason to look twice.
    #[test]
    fn a_rok_outside_the_calendar_is_refused_in_both_directions() {
        for (vrsta, datum_popisa, filing) in [
            (PopisVrsta::Nivelacioni, "9999-12-31", None),
            (PopisVrsta::Godisnji, "0000-01-01", Some("0000-02-01")),
        ] {
            let error = izvestaj_due(vrsta, datum_popisa, filing)
                .expect_err("a rok outside the calendar must not be returned as a rok");

            assert_eq!(
                error.code(),
                "popis_rok_van_kalendara",
                "{vrsta:?} / {datum_popisa} / {filing:?}"
            );
        }
    }

    /// Every rok this engine returns is `gggg-MM-dd` — the shape its own doc
    /// promises and the only shape the v20 date columns are GLOB-checked into. A
    /// per-answer assertion would miss this; the shape is a property of all of
    /// them, so it is asserted over the whole reachable surface at once.
    #[test]
    fn every_rok_is_shaped_gggg_mm_dd() {
        for (vrsta, datum_popisa, filing) in [
            (PopisVrsta::Godisnji, "2026-12-31", Some("2027-03-31")),
            (PopisVrsta::Godisnji, "0001-12-31", Some("9999-03-31")),
            (PopisVrsta::Nivelacioni, "0001-01-01", None),
            (PopisVrsta::Nivelacioni, "9999-11-30", None),
        ] {
            let rok = due(vrsta, datum_popisa, filing);

            assert!(
                holds_calendar_date(&rok) && rok.len() == 10,
                "{vrsta:?} / {datum_popisa} / {filing:?} returned „{rok}“, which is not gggg-MM-dd"
            );
        }
    }

    /// The vrsta enum and the v20 `popis_sessions.vrsta` CHECK are one
    /// vocabulary, and the wire form is pinned to the stored form for the reason
    /// the status test gives: `as_db_str` is a hand-written literal while serde
    /// derives the wire form from the variant *identifier*, so a rename moves one
    /// and not the other. Here the drift is worse than a mislabelled column — the
    /// two vrste carry **different statutory roks**, 60 days before the filing
    /// deadline against 30 days after the count.
    #[test]
    fn the_vrsta_vocabulary_is_exactly_the_schema_check() {
        with_test_database("popis_vrsta_vocabulary", |connection| {
            let mut modelled: Vec<String> = PopisVrsta::ALL
                .into_iter()
                .map(|vrsta| vrsta.as_db_str().to_string())
                .collect();
            modelled.sort();

            assert_eq!(
                check_vocabulary(connection, "popis_sessions", "vrsta"),
                modelled
            );

            for (index, vrsta) in PopisVrsta::ALL.into_iter().enumerate() {
                seed_session_of(connection, 400 + index as i64, vrsta, PopisStatus::Draft);
                assert_eq!(
                    PopisVrsta::from_db_str(vrsta.as_db_str()),
                    Some(vrsta),
                    "{vrsta:?} should survive the round trip through its stored value"
                );
                assert_eq!(
                    serde_json::to_string(&vrsta).expect("a vrsta should serialize"),
                    format!("\"{}\"", vrsta.as_db_str()),
                    "{vrsta:?} must reach the frontend as its stored value"
                );
            }

            assert!(
                PopisVrsta::from_db_str("nivelacija").is_none(),
                "an unknown stored value must not resolve to a vrsta"
            );
        });
    }

    // ---------------------------------------------------------------------
    // The posebne popisne liste (req. 36)
    // ---------------------------------------------------------------------

    /// The lista enum and the v20 `popis_lines.lista_vrsta` CHECK are one
    /// vocabulary, pinned for the reason the two above are. Here the drift has its
    /// own shape: a lista the engine stores but this enum cannot name is a
    /// statutory list the module can never ask the shop for, and a lista this enum
    /// names but the engine refuses is a list the shop is asked to fill and the
    /// database throws away.
    #[test]
    fn the_lista_vocabulary_is_exactly_the_schema_check() {
        with_test_database("popis_lista_vocabulary", |connection| {
            let mut modelled: Vec<String> = PopisLista::ALL
                .into_iter()
                .map(|lista| lista.as_db_str().to_string())
                .collect();
            modelled.sort();

            assert_eq!(
                check_vocabulary(connection, "popis_lines", "lista_vrsta"),
                modelled
            );

            for lista in PopisLista::ALL {
                assert_eq!(
                    PopisLista::from_db_str(lista.as_db_str()),
                    Some(lista),
                    "{lista:?} should survive the round trip through its stored value"
                );
                assert_eq!(
                    serde_json::to_string(&lista).expect("a lista should serialize"),
                    format!("\"{}\"", lista.as_db_str()),
                    "{lista:?} must reach the frontend as its stored value"
                );
            }

            assert!(
                PopisLista::from_db_str("ostalo").is_none(),
                "an unknown stored value must not resolve to a lista"
            );
        });
    }

    /// Req. 36 is a list of articles, not a taste in categories: each posebna lista
    /// exists because one provision requires it. The pravni osnov is what the
    /// refusals and the count sheet quote, so a lista citing the wrong article — or
    /// none — would send the shop to the wrong provision.
    #[test]
    fn every_posebna_lista_names_the_article_that_requires_it() {
        for (lista, osnov) in [
            (PopisLista::Ostecena, "čl. 10 st. 3"),
            (PopisLista::VanObjekta, "čl. 10 st. 4"),
            (PopisLista::Gotovina, "čl. 11 st. 1"),
            (PopisLista::Potrazivanja, "čl. 12 st. 2"),
            (PopisLista::Konsignacija, "čl. 2 st. 5"),
        ] {
            assert!(
                lista.posebna(),
                "{lista:?} is one of the posebne popisne liste of req. 36"
            );
            assert!(
                lista.pravni_osnov().contains(osnov),
                "{lista:?} must cite {osnov}, cites „{}“",
                lista.pravni_osnov()
            );
            assert!(
                !lista.naziv().is_empty(),
                "{lista:?} needs a name a shop can read"
            );
        }

        assert!(
            !PopisLista::Roba.posebna(),
            "the ordinary lista is not one of the čl. 2 / čl. 10–12 posebne liste"
        );
        assert_eq!(
            PopisLista::ALL.len(),
            6,
            "req. 36 names five posebne liste beside the ordinary one"
        );
    }

    /// PoP čl. 2 st. 6 — „најкасније у року од десет дана од дана на који је попис
    /// извршен“. Ten calendar days from the **count date**, which is the only
    /// anchor the article gives: not the potpis, not the knjiženje and not today.
    /// The cases cross a month end, a year end and a 29 February, so the ten are
    /// calendar days and not „a week and a half“ of anything.
    #[test]
    fn the_konsignacija_copy_is_due_ten_days_after_the_count() {
        for (datum_popisa, expected) in [
            ("2026-12-31", "2027-01-10"),
            ("2026-11-25", "2026-12-05"),
            ("2028-02-20", "2028-03-01"),
            ("2027-02-20", "2027-03-02"),
        ] {
            assert_eq!(
                konsignacija_rok(datum_popisa)
                    .unwrap_or_else(|error| panic!("{datum_popisa} should have a rok: {error}")),
                expected,
                "a popis taken on {datum_popisa}"
            );
        }
    }

    /// The same refusal discipline the izveštaj rok keeps, and for the same reason:
    /// a date this module invents is one the shop will act on, and čl. 2 st. 6 puts
    /// a signed lista in another owner's hands by a day certain.
    #[test]
    fn the_konsignacija_rok_refuses_a_date_it_cannot_read_or_reach() {
        for (datum_popisa, code) in [
            ("25.11.2026.", "popis_rok_neispravan_datum"),
            ("2026-11-25T08:00:00Z", "popis_rok_neispravan_datum"),
            ("2026-11-31", "popis_rok_neispravan_datum"),
            ("", "popis_rok_neispravan_datum"),
            ("9999-12-31", "popis_rok_van_kalendara"),
        ] {
            let error = konsignacija_rok(datum_popisa)
                .expect_err("an unreadable or unreachable rok must not be returned as a rok");
            assert_eq!(error.code(), code, "for „{datum_popisa}“");
        }
    }

    /// Req. 36's actual bite. A posebna lista is required **where its category is
    /// present**, and presence is a fact about the shop that no ledger holds —
    /// nothing in the books says „we have damaged goods“ or „some of this stock
    /// belongs to somebody else“. So the person taking the popis declares it, and
    /// this is what makes the declaration binding: a category declared present
    /// whose lista is empty comes back named, in the fixed order of the enum so a
    /// caller can render it the same way twice.
    #[test]
    fn a_declared_category_with_an_empty_lista_is_missing() {
        assert_eq!(
            nedostajuce_liste(
                &[
                    PopisLista::Gotovina,
                    PopisLista::Konsignacija,
                    PopisLista::Ostecena
                ],
                &[PopisLista::Roba, PopisLista::Konsignacija],
            ),
            vec![PopisLista::Ostecena, PopisLista::Gotovina],
            "only the declared categories with nothing on their lista are missing"
        );
    }

    /// The two edges of the same rule, neither of which may drift. A shop with no
    /// consignment goods owes no consignment lista — req. 36 says „where it
    /// applies“, and a gate that demanded all six of everyone would be clicked past
    /// by the second popis. And a category declared twice is one missing lista, not
    /// two: the izveštaj gate's message is read by a person.
    #[test]
    fn an_undeclared_category_is_never_required() {
        assert!(nedostajuce_liste(&[], &[PopisLista::Roba]).is_empty());
        assert!(nedostajuce_liste(&[], &[]).is_empty());
        assert!(
            nedostajuce_liste(&[PopisLista::Roba, PopisLista::Roba], &[PopisLista::Roba])
                .is_empty()
        );
        assert_eq!(
            nedostajuce_liste(&[PopisLista::Roba, PopisLista::Roba], &[]),
            vec![PopisLista::Roba]
        );
    }

    /// The gate itself. Čl. 13 st. 1 has the izveštaj report the stvarno stanje of
    /// the popis, so an izveštaj resting on a lista the shop said exists and never
    /// filled reports a stanje that is not the shop's. The refusal names every
    /// missing lista **and** the article behind it — „nešto nedostaje“ is not
    /// something a shop owner can act on — and names nothing that is not missing.
    #[test]
    fn the_izvestaj_gate_refuses_a_declared_category_with_an_empty_lista() {
        ensure_liste_kompletne(
            &[PopisLista::Roba, PopisLista::Gotovina],
            &[PopisLista::Roba, PopisLista::Gotovina],
        )
        .expect("declared categories that have lines pass the gate");
        ensure_liste_kompletne(&[], &[]).expect("a popis that declared nothing owes no lista");

        let error = ensure_liste_kompletne(
            &[PopisLista::Roba, PopisLista::Gotovina, PopisLista::Ostecena],
            &[PopisLista::Roba, PopisLista::Konsignacija],
        )
        .expect_err("an empty declared lista must stop the izveštaj");

        assert_eq!(error.code(), "popis_prazna_prijavljena_lista");
        let poruka = error.to_string();
        for trazeno in [
            "oštećena, zastarela i neupotrebljiva roba",
            "čl. 10 st. 3",
            "gotovina po apoenima",
            "čl. 11 st. 1",
            "čl. 13 st. 1",
        ] {
            assert!(
                poruka.contains(trazeno),
                "the refusal must name „{trazeno}“, said: {poruka}"
            );
        }
        assert!(
            !poruka.contains("roba u objektu") && !poruka.contains("konsignaciona"),
            "a lista that is not missing must not be named, said: {poruka}"
        );
    }

    /// Every spelling of „read the wall clock“ this codebase can reach. The first
    /// two are one entry apart on purpose: `crate::clock::utc_now` is this
    /// repository's own canonical reader and **is not matched by `now_utc`** —
    /// neither string is a substring of the other — so a guard that scans only for
    /// the raw `time` call is blind to the way a clock would actually arrive here.
    /// The rest are the readers a future task could reach for instead of the
    /// wrapper.
    const CLOCK_READS: [&str; 7] = [
        "utc_now",
        "now_utc",
        "Utc::now",
        "Local::now",
        "now_local",
        "SystemTime::now",
        "datetime('now')",
    ];

    /// A hand-written list of clock spellings decays the moment `crate::clock`
    /// grows a second reader — and that module is precisely where a future task
    /// would go looking for the time. So the list is pinned to what the module
    /// actually exports: adding `pub fn` anything there fails this test until the
    /// deadline engine's guard has learned to scan for it.
    #[test]
    fn the_clock_guard_scans_for_every_reader_crate_clock_exports() {
        const CLOCK_SOURCE: &str = include_str!("clock.rs");

        let exported: Vec<&str> = CLOCK_SOURCE
            .lines()
            .filter_map(|line| line.trim_start().strip_prefix("pub fn "))
            .filter_map(|rest| rest.split('(').next())
            .collect();

        assert!(
            exported.contains(&"utc_now"),
            "crate::clock::utc_now is this codebase's wall clock; the scan of clock.rs found \
             {exported:?} instead, so this guard is reading the wrong file"
        );
        for reader in exported {
            assert!(
                CLOCK_READS.contains(&reader),
                "crate::clock::{reader} is a wall clock that the deadline engine's source guard \
                 does not scan for — add it to CLOCK_READS"
            );
        }
    }

    /// Design §7 t. 5 („no hardcoded deadline dates“) and the standing rule that
    /// **no wall clock may reach the deadline engine**. Both are properties of
    /// the source rather than of any single answer: a behavioural test cannot
    /// tell a computed 30 January 2027 from a hardcoded one, and it cannot tell a
    /// rok derived from its two arguments from one that read the system clock and
    /// happened to agree today. So the engine's own half of this file is what is
    /// asserted. Comment lines are skipped — the module doc cites a dated design
    /// file, and prose is not what computes a rok.
    #[test]
    fn the_engine_holds_no_calendar_date_and_reads_no_clock() {
        const SOURCE: &str = include_str!("popis.rs");

        let engine = SOURCE
            .split("#[cfg(test)]")
            .next()
            .expect("this module must have a non-test half");

        for (number, line) in engine.lines().enumerate() {
            let code = line.trim_start();
            if code.starts_with("//") {
                continue;
            }

            assert!(
                !holds_calendar_date(code),
                "line {} hardcodes a calendar date; a rok is computed from the dates it is \
                 given: {code}",
                number + 1
            );
            for clock in CLOCK_READS {
                assert!(
                    !code.contains(clock),
                    "line {} reads a clock; a statutory rok is a function of the dates it is \
                     given and of nothing else: {code}",
                    number + 1
                );
            }
        }
    }

    /// `gggg-MM-dd` anywhere in a line, which is the shape every date in this
    /// schema is stored and passed in.
    fn holds_calendar_date(line: &str) -> bool {
        let bytes = line.as_bytes();

        bytes.windows(10).any(|window| {
            window.iter().enumerate().all(|(index, byte)| match index {
                4 | 7 => *byte == b'-',
                _ => byte.is_ascii_digit(),
            })
        })
    }
}
