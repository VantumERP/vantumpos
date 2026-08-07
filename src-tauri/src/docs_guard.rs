//! Guards over the compliance prose that describes this crate.
//!
//! `docs/SERBIAN-LAW-COMPLIANCE.md`, `docs/PROGRESS.md` and the templates under
//! `docs/compliance/` are read as a description of what the code does — by the
//! founder, by whoever advises the shop, and eventually by an inspector. A row
//! that credits a module with a statutory leg the module does not implement is
//! a compliance defect in its own right: it withdraws the only pointer to a real
//! gap, and it withdraws it first for the provisions protecting the most exposed
//! people. Four such claims shipped with SW-14 and nothing could see them,
//! because nothing in this crate had ever read the documents.
//!
//! Each guard below pins one prose claim against the code that has to back it.
//! **When a leg is genuinely built, delete its guard and re-state the prose in
//! the same commit** — never relax an assertion to make a stale sentence pass.
//!
//! Test-only: the module is declared `#[cfg(test)]` in `lib.rs`, so the
//! documents are embedded in the test binary and never in the shipped app.

const REGISTER: &str = include_str!("../../docs/SERBIAN-LAW-COMPLIANCE.md");
const PROGRESS: &str = include_str!("../../docs/PROGRESS.md");
const NOTICE: &str = include_str!("../../docs/compliance/obavestenje-zaposlenima.md");
const EVIDENCIJA_CL47: &str = include_str!("../../docs/compliance/evidencija-obrade-cl47.md");

/// Every line of `text` containing `needle`, numbered from 1 the way an editor
/// numbers them so a failure message points straight at the line to fix.
fn lines_with<'a>(text: &'a str, needle: &str) -> Vec<(usize, &'a str)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .map(|(index, line)| (index + 1, line))
        .collect()
}

/// Every piece of prose this crate is answerable for: the three documents, plus
/// the `napomena` strings the retention table stores beside each class. The
/// notes are prose in exactly the sense this module guards — they are written
/// into `retention_policies` and read back by whoever asks why a record is
/// still there — so a claim is no safer for being a Rust string literal.
fn prose_sources() -> Vec<(String, String)> {
    let mut sources = vec![
        (
            "docs/SERBIAN-LAW-COMPLIANCE.md".to_string(),
            REGISTER.to_string(),
        ),
        ("docs/PROGRESS.md".to_string(), PROGRESS.to_string()),
        (
            "docs/compliance/obavestenje-zaposlenima.md".to_string(),
            NOTICE.to_string(),
        ),
        (
            "docs/compliance/evidencija-obrade-cl47.md".to_string(),
            EVIDENCIJA_CL47.to_string(),
        ),
    ];
    sources.extend(crate::retention::RecordClass::ALL.into_iter().map(|class| {
        (
            format!("retention.rs napomena (`{}`)", class.key()),
            class.napomena().to_string(),
        )
    }));

    sources
}

/// House rule: a Serbian quotation opens „ and closes “ — never with the ASCII
/// `"`. The 01.08.2026 sweep (`eb49dbb`) closed 17 of them by hand across five
/// templates and stopped at the templates, so the register kept every one of
/// its own — including the čl. 55 st. 6 label the worktime module prints on
/// every rendering, which was then rewritten twice without anyone noticing. A
/// hand sweep is not a rule; this is.
///
/// Only a quotation **opened with „** is judged. English prose in the register
/// quotes with a plain pair of ASCII marks and is left alone — the defect is the
/// mismatched pair, where the reader sees a Serbian opening and a typewriter
/// closing and cannot tell whether the quotation ended there or ran on.
#[test]
fn no_compliance_prose_closes_a_serbian_quotation_with_an_ascii_quote() {
    for (label, text) in prose_sources() {
        // The line the open quotation mark is on, while one is open. Quotations
        // wrap across lines in `PROGRESS.md`, so this is tracked over the whole
        // document rather than per line.
        let mut opened_at: Option<usize> = None;
        let mut line_no = 1usize;

        for character in text.chars() {
            match character {
                '\n' => line_no += 1,
                '„' => opened_at = opened_at.or(Some(line_no)),
                '“' => opened_at = None,
                '"' => {
                    assert!(
                        opened_at.is_none(),
                        "{label}:{line_no} closes with an ASCII \" a quotation opened with „ on \
                         line {}. The house rule is „…“ — an ASCII closing quote leaves the reader \
                         unable to tell where the quoted statute or operator string ends.",
                        opened_at.unwrap_or(line_no)
                    );
                }
                _ => {}
            }
        }
    }
}

/// `retention::assert_never_purge_intact` is the fence behind every „ne briše
/// se“ claim about the `trajno` classification, and it runs in exactly one
/// place: inside the `reset_trading_data` transaction. It cannot run on the
/// restore path — `restore_backup` replaces the whole database file, so the
/// register comes back at whatever the snapshot holds, and a count-based abort
/// would refuse every legitimate rewind rather than only the lossy ones. There
/// is no backup-prune path in the crate at all.
///
/// So a line that puts restore or backup-pruning beside the go-live reset as
/// something the class is immune to promises a deletion-immunity the code does
/// not implement — and the first place it promised it was the čl. 23 notice
/// handed to the employee whose hours those are. Naming the paths is fine; the
/// caveat that says what actually protects them has to travel on the same line.
#[test]
fn no_trajno_claim_promises_immunity_from_restore_or_backup_pruning() {
    // Wording that asserts the class cannot be touched. Stems, not whole words:
    // „izuzet iz brisanja“ and „aplikacija je izuzima“ are the same claim.
    const IMMUNITY: [&str; 4] = ["unreachable", "immune", "izuz", "dodir"];
    // The restore path, in either language („vraćanje/vraćanja iz rezervne
    // kopije“).
    const RESTORE: [&str; 2] = ["restore", "vraćanj"];
    // The pre-restore safety copy is the only thing standing on that path.
    const RESTORE_CAVEAT: [&str; 3] = ["pre_restore", "pre-restore", "pre vraćanja"];
    // Discarding old backup files.
    const PRUNE: [&str; 3] = ["prune", "pruning", "čišćenje"];
    // …which nothing in this crate does, and which is what such a line must say.
    const PRUNE_CAVEAT: [&str; 2] = ["ne postoji", "does not exist"];

    for (label, text) in prose_sources() {
        for (index, line) in text.lines().enumerate() {
            let line_no = index + 1;
            let line = line.to_lowercase();
            if !IMMUNITY.iter().any(|needle| line.contains(needle)) {
                continue;
            }

            if RESTORE.iter().any(|needle| line.contains(needle)) {
                assert!(
                    RESTORE_CAVEAT.iter().any(|needle| line.contains(needle)),
                    "{label}:{line_no} says the trajno classification is untouched by a restore. \
                     `restore_backup` replaces the whole database file — the register comes back \
                     as the snapshot holds it, and `assert_never_purge_intact` runs only inside \
                     the go-live reset transaction. Name the pre_restore safety copy as the \
                     protection on that path, or drop the claim."
                );
            }

            if PRUNE.iter().any(|needle| line.contains(needle)) {
                assert!(
                    PRUNE_CAVEAT.iter().any(|needle| line.contains(needle)),
                    "{label}:{line_no} says the trajno classification is untouched by backup \
                     pruning. No backup-prune path exists in this crate, so the claim is vacuous \
                     — say that it does not exist („ne postoji“) instead of implying a guarded \
                     path."
                );
            }
        }
    }
}

/// The other half of the same defect: the guard above only fires on a claim, so
/// deleting the sentence would satisfy it while leaving the reader with nothing.
/// These two strings are where a person is told how long the classification is
/// kept — the čl. 23 notice handed to the employee, and the note the retention
/// table stores beside the class — and both have to carry the two facts that are
/// actually true of a restore: the pre-restore safety copy is the protection,
/// and no automatic cleanup of old backups exists.
#[test]
fn the_trajno_retention_row_names_what_actually_protects_it_on_a_restore() {
    let rows = lines_with(NOTICE, "ZEOR čl. 7 st. 2");
    assert!(
        !rows.is_empty(),
        "docs/compliance/obavestenje-zaposlenima.md must keep the ZEOR čl. 7 st. 2 retention row"
    );

    for (line_no, line) in rows {
        assert!(
            line.contains("pre vraćanja"),
            "docs/compliance/obavestenje-zaposlenima.md:{line_no} tells an employee how the trajno \
             classification is kept without saying what happens on a restore. A restore rewinds \
             the whole database, including this register; the safety copy the app takes „pre \
             vraćanja“ is the only protection on that path and the employee has to be told so."
        );
        assert!(
            line.contains("ne postoji"),
            "docs/compliance/obavestenje-zaposlenima.md:{line_no} must say that automatic cleanup \
             of old backups „ne postoji“ — the app has no backup-prune path, and an employee \
             reading about „čišćenje starih rezervnih kopija“ would otherwise assume one exists \
             and is guarded."
        );
    }

    let napomena = crate::retention::RecordClass::WorktimeClassification.napomena();
    assert!(
        napomena.contains("pre vraćanja") && napomena.contains("ne postoji"),
        "the note stored beside the trajno class must state the same two facts as the čl. 23 \
         notice — the pre-restore safety copy is the protection on a restore, and no backup-prune \
         path exists: {napomena}"
    );
}

/// A retention row that tells an employee something *is deleted* is a promise,
/// and the only thing that can keep it is a sweep that names the class.
/// `commands::personnel::PurgeableClass` is that list in full — two variants,
/// `Credentials` and `AccessLog` — and a class absent from it is discarded by
/// nothing. `retention::draft_purge_eligible` and `overtime_log_purge_eligible`
/// are **gates, not sweeps**: they answer *may this go yet*, and every call site
/// is inside `retention.rs`'s own `#[cfg(test)]` module.
///
/// So the Class B row that told an employee their drafts *„Brišu se pošto je
/// mesec zaključen“* promised a deletion nothing performs — while the `napomena`
/// stored beside that same class was written correctly as a not-before bound
/// (*„Brišu se **tek** pošto je period zatvoren“*). The notice contradicted the
/// machine-readable text it mirrors, and this is the second time this document
/// promised the employee something the code never had (see `66636a3`).
///
/// The whitelist below is an **exhaustive** match on `PurgeableClass`, so a new
/// promise costs a new variant, and a new variant costs a sweep.
///
/// **The register is read here too**, and it is the second document to have made
/// the same promise: the SW-14 row said the raw punch events are *„bounded and
/// purged on period close“* while the notice was being corrected one file over.
/// That is the document counsel and an inspector actually read, so a guard that
/// stops at the employee notice stops one reader short.
///
/// **The two documents are cleared against different lists, and they have to
/// be.** `PurgeableClass` is `commands::personnel::purge_expired_classes`'s own
/// argument type, so on the čl. 23 notice — which is only ever about an
/// employee's data — it is both exhaustive and self-enforcing. It stopped being
/// the whole answer for the *register* when SW-12 shipped a second, entirely
/// separate sweep: `commands::cenovnik::purge_expired_snapshots`, wired into
/// launch and the 6-hourly timer in `lib.rs` beside the personnel one, discards
/// `RecordClass::CenovnikArchive` — a class that holds no personal data, has no
/// row in the čl. 23 notice, and could never have a `PurgeableClass` variant.
/// Leaving it out would have made this guard reject a true sentence and tell its
/// author to add a variant to a type that must not have one. So the register arm
/// matches on `RecordClass` instead, exhaustively: every class in the shared
/// retention table answers either with the register's word for it **and the
/// sweep that discards it**, or with `None` and the reason there is none.
#[test]
fn no_retention_row_promises_a_purge_no_job_performs() {
    use crate::commands::personnel::PurgeableClass;
    use crate::retention::RecordClass;

    // A sweep stated as something that happens, in the notice's Serbian:
    // „brišu se“, „briše se“, „uklanjaju se“, „uklanja se“.
    const PROMISE_SR: [&str; 4] = ["brišu se", "briše se", "uklanjaju se", "uklanja se"];
    // …and in the register's English. The bare stem „purge“ is deliberately
    // absent: the register uses it for the sweep that exists
    // (`purge_expired_classes`), for the gates that are not sweeps, and for the
    // type name that fences them. What asserts an event is the finished form.
    const PROMISE_EN: [&str; 4] = ["purged", "purges", "is deleted", "are deleted"];
    // The one word that turns the promise back into what it really is — the
    // earliest permitted day, not an event: „Brišu se **tek** pošto…“.
    const NOT_BEFORE: &str = "tek ";

    // How each document names the classes a job really sweeps, lowercased for
    // the comparison. The notice arm is an exhaustive match on `PurgeableClass`;
    // the register arm is an exhaustive match on `retention::RecordClass` — see
    // the doc comment above for why the two lists differ.
    let swept_notice: Vec<&str> = PurgeableClass::ALL
        .iter()
        .map(|class| match class {
            PurgeableClass::Credentials => "pin i lozinka",
            PurgeableClass::AccessLog => "evidencija pristupa",
        })
        .collect();
    let swept_register: Vec<&str> = RecordClass::ALL
        .iter()
        .filter_map(|class| match class {
            // `commands::personnel::purge_expired_classes` — the two
            // `PurgeableClass` variants, in the register's words.
            RecordClass::Credentials => Some("credential"),
            RecordClass::AccessLog => Some("access log"),
            // `commands::cenovnik::purge_expired_snapshots` (SW-12 req. 14) —
            // the čl. 213 sweep, which never reaches an outlet's current file.
            RecordClass::CenovnikArchive => Some("cenovnik archive"),
            // `trajno`: ZEOR čl. 7 st. 2 for the two worktime/personnel rows,
            // ZZPL čl. 47 st. 7 for the register. `never_purge` makes them
            // unreachable by every sweep, so no document may say they go.
            RecordClass::WorktimeClassification
            | RecordClass::Personnel
            | RecordClass::ProcessingRegister => None,
            // `retention::{draft_purge_eligible, overtime_log_purge_eligible,
            // popis_purge_eligible}` are **gates, not sweeps** — they answer
            // *may this go yet*, and every call site is inside `retention.rs`'s
            // own test module. The popis class joins them for a second reason
            // too: PoP čl. 14 st. 3 write-locks a posted popis, so the only
            // thing that could ever discard one is a sweep written on purpose.
            RecordClass::WorktimeOvertimeLog
            | RecordClass::WorktimeDraft
            | RecordClass::PopisDokumentacija => None,
        })
        .collect();

    for (label, text, promises, swept, cleared_against) in [
        (
            "docs/compliance/obavestenje-zaposlenima.md",
            NOTICE,
            PROMISE_SR.as_slice(),
            &swept_notice,
            "`commands::personnel::PurgeableClass`",
        ),
        (
            "docs/SERBIAN-LAW-COMPLIANCE.md",
            REGISTER,
            PROMISE_EN.as_slice(),
            &swept_register,
            "an exhaustive match on `retention::RecordClass` — `purge_expired_classes` for the \
             credentials and the access log, `commands::cenovnik::purge_expired_snapshots` for \
             the cenovnik archive",
        ),
    ] {
        for (index, line) in text.lines().enumerate() {
            let line_no = index + 1;
            // The retention table is where the per-class claim is made; §2's
            // prose narrates the same sweeps and is answerable to the rows it
            // summarises. The register is a table throughout.
            if !line.starts_with('|') {
                continue;
            }

            let lowercase = line.to_lowercase();
            if !promises.iter().any(|needle| lowercase.contains(needle)) {
                continue;
            }
            if lowercase.contains(NOT_BEFORE) {
                continue;
            }

            assert!(
                swept.iter().any(|subject| lowercase.contains(subject)),
                "{label}:{line_no} states that a class of data is deleted. This document is \
                 cleared against {cleared_against}: the classes a purge job actually sweeps are \
                 {swept:?}, and this row is none of them. \
                 `retention::draft_purge_eligible` and `overtime_log_purge_eligible` are gates \
                 with no production caller, so they delete nothing. State the not-before bound \
                 the stored `napomena` states („Brišu se tek pošto…“), or write the sweep and \
                 name the class in the list this document is cleared against."
            );
        }
    }
}

/// The other half of the same defect, and the reason the pair exists: the guard
/// above fires only on a promise, so softening the row to a bound satisfies it
/// while leaving the employee unable to tell that **nothing sweeps this class at
/// all**. A bound with no sweep behind it reads as a schedule.
///
/// Class B (`retention::RecordClass::WorktimeDraft`) is that class, so the row
/// has to carry both facts — the not-before bound, and that no automatic
/// deletion exists — and the stored `napomena` has to keep the bound it already
/// states, because the two are read as one claim.
#[test]
fn the_class_b_retention_row_says_no_automatic_purge_exists_for_it() {
    let rows = lines_with(NOTICE, "Radne verzije unosa");
    assert!(
        !rows.is_empty(),
        "docs/compliance/obavestenje-zaposlenima.md must keep the Class B \
         (`retention::RecordClass::WorktimeDraft`) retention row"
    );

    for (line_no, line) in rows {
        assert!(
            line.contains("tek pošto"),
            "docs/compliance/obavestenje-zaposlenima.md:{line_no} states the Class B retention as \
             an event rather than as the not-before bound it is. Nothing purges this class; \
             `draft_purge_eligible` only answers whether it *may* go. Mirror the stored `napomena`: \
             „Brišu se tek pošto je mesec zaključen i klasifikacija izvedena“."
        );
        assert!(
            line.contains("Automatsko brisanje") && line.contains("ne postoji"),
            "docs/compliance/obavestenje-zaposlenima.md:{line_no} gives the employee a bound with \
             no sweep behind it, which reads as a schedule. No purge job names this class — say so \
             on the same line („Automatsko brisanje … ne postoji“), the way the trajno rows say \
             that automatic cleanup of old backups „ne postoji“."
        );
    }

    let napomena = crate::retention::RecordClass::WorktimeDraft.napomena();
    assert!(
        napomena.contains("tek pošto"),
        "the note stored beside Class B must keep the not-before bound the čl. 23 notice mirrors — \
         it is the machine-readable half of the same sentence: {napomena}"
    );
}

/// Wording that promises the shop can move a retention period: „rok je
/// podesiv“, „uz mogućnost produženja“, „rok se pomera samo unapred“. The first
/// is a stem, so „podesiva“ and „podesivi rokovi“ trip the same guard.
///
/// The verb **„podešava se“ is deliberately absent.** It is the word the
/// negations are built out of — „rok se ne podešava“, „nema zaseban rok koji bi
/// se podešavao“ — and a stem that matches both halves of a contradiction
/// classifies neither. What is left are phrases that only ever appear in a
/// promise.
const ADJUSTABLE_CLAIM: [&str; 4] = [
    "podesiv",
    "mogućnost produženja",
    "pomera se samo unapred",
    "pomera samo unapred",
];

/// The negation of the one stem above that can be negated in place. Without it
/// a future „rok nije podesiv“ would read as the claim it denies.
const NOT_ADJUSTABLE: [&str; 2] = ["nije podesiv", "nisu podesiv"];

fn claims_an_adjustable_period(text: &str) -> bool {
    let lowercase = text.to_lowercase();
    if NOT_ADJUSTABLE
        .iter()
        .any(|needle| lowercase.contains(needle))
    {
        return false;
    }

    ADJUSTABLE_CLAIM
        .iter()
        .any(|needle| lowercase.contains(needle))
}

/// A period that „moves only forward“ is a **setting**, and a setting nothing
/// can set is the same defect as a purge nothing performs — one document further
/// on. Four strings promised it at once: two rows of the čl. 23 notice, the
/// `napomena` stored beside the access-log class, and the `rok_osnov` the čl. 47
/// register prints for the Poverenik.
///
/// `commands::retention::AdjustableClass` is the list of classes a registered
/// command can actually move, and it is **exhaustive**: a class that reaches
/// `retention_policies` without a variant here fails
/// `every_class_that_is_not_trajno_is_named_by_an_adjustable_variant`, and a
/// variant added without a command does not compile. So a note claiming an
/// adjustable period for a class outside that list is a promise nothing keeps.
///
/// This half is exact rather than textual: the class each string belongs to is
/// known, so nothing has to be inferred from the prose.
#[test]
fn no_stored_retention_note_claims_a_period_no_command_can_move() {
    use crate::commands::retention::AdjustableClass;
    use crate::retention::RecordClass;

    for class in RecordClass::ALL {
        let napomena = class.napomena();
        if !claims_an_adjustable_period(napomena) {
            continue;
        }

        assert!(
            AdjustableClass::from_key(class.key()).is_some(),
            "the note stored beside „{}“ promises an adjustable rok. \
             `commands::retention::AdjustableClass` names every class a registered command can \
             move, and this class is not one of them — either give it a variant (and therefore a \
             command) or state the rok as the fixed period it is: {napomena}",
            class.key()
        );
    }

    // The same claim where it is read by the Poverenik rather than by the shop:
    // čl. 47 st. 1 t. 6 is the register's own retention column.
    for (kljuc, class, rok_osnov) in crate::cl47::retention_prose() {
        if !claims_an_adjustable_period(rok_osnov) {
            continue;
        }

        let movable = class
            .map(|class| AdjustableClass::from_key(class.key()).is_some())
            .unwrap_or(false);
        assert!(
            movable,
            "the čl. 47 register tells the Poverenik that the rok for „{kljuc}“ moves forward. \
             Only `commands::retention::AdjustableClass` classes can be moved, and this radnja \
             is wired to {class:?} — wire it to a class the shop can actually move, or drop the \
             claim: {rok_osnov}"
        );
    }
}

/// The textual half of the same guard, on the one document that is handed to a
/// person rather than generated: the čl. 23 notice. Here the class is not
/// carried by the line, so the subject phrase is — and the list of subjects is
/// an **exhaustive match** on `AdjustableClass`, so a new variant costs a
/// decision about what the employee is told it is called.
#[test]
fn no_notice_row_claims_an_adjustable_period_for_a_class_no_command_can_move() {
    use crate::commands::retention::AdjustableClass;

    // How the čl. 23 notice names each movable class, in its own words.
    let movable: Vec<&str> = AdjustableClass::ALL
        .iter()
        .map(|class| match class {
            AdjustableClass::WorktimeOvertimeLog => "prekovremen",
            AdjustableClass::WorktimeDraft => "radne verzije",
            AdjustableClass::Credentials => "pin i lozinka",
            AdjustableClass::AccessLog => "evidencija pristupa",
            // Not an employee's data at all — the čl. 23 notice has no row about
            // it, and this arm exists so that a class added to the shared
            // retention table still costs a decision about what, if anything,
            // the employee is told it is called.
            AdjustableClass::CenovnikArchive => "arhiva objavljenih cenovnika",
            // The popis documentation names the komisija members, and PoP čl. 5
            // st. 1 keeps lica koja rukuju imovinom off it — so where the shop
            // appoints an employee, this class does hold an employee's data. The
            // čl. 23 notice has no row about it today (the čl. 47 register,
            // which the Poverenik reads, does), and this string is what such a
            // row would have to be called if one is ever added.
            AdjustableClass::PopisDokumentacija => "popisne liste",
        })
        .collect();

    let lines: Vec<&str> = NOTICE.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let line_no = index + 1;
        if !claims_an_adjustable_period(line) {
            continue;
        }

        // A table row is a self-contained per-class claim and is judged alone —
        // otherwise a movable neighbour in the same table would vouch for it.
        // Running prose is hard-wrapped at ~100 columns, so the subject of the
        // sentence is routinely one line above the claim; there the context is
        // the paragraph plus the heading the reader arrived through.
        let context = if line.starts_with('|') {
            line.to_lowercase()
        } else {
            let heading = lines[..index]
                .iter()
                .rev()
                .find(|candidate| candidate.starts_with('#'))
                .copied()
                .unwrap_or_default();
            let start = lines[..index]
                .iter()
                .rposition(|candidate| candidate.trim().is_empty())
                .map_or(0, |blank| blank + 1);
            let end = lines[index..]
                .iter()
                .position(|candidate| candidate.trim().is_empty())
                .map_or(lines.len(), |blank| index + blank);
            format!("{heading} {}", lines[start..end].join(" ")).to_lowercase()
        };

        assert!(
            movable.iter().any(|subject| context.contains(subject)),
            "docs/compliance/obavestenje-zaposlenima.md:{line_no} tells an employee that a \
             retention period is adjustable and moves only forward. \
             `commands::retention::AdjustableClass` names every class a registered command can \
             move — {movable:?} — and this line names none of them. Either build the setting for \
             the class this line is about, or state the fixed period the code applies."
        );
    }
}

/// **Inverted 07.08.2026, when the leg it guarded was built.** Its predecessor,
/// `no_document_claims_the_cl_87_weekly_leg_is_enforced`, required every line
/// naming 35 h to mark the leg unbuilt, because only `MINOR_DAILY_CAP_MINUTES`
/// existed and the register credited `check_protection` with both legs of
/// čl. 87. `MINOR_WEEKLY_CAP_MINUTES` and `ProtectionKind::MaloletanNedeljniLimit`
/// now exist and `commands::worktime::write_entry` refuses on them, so that
/// guard's own instruction — delete it when the leg lands — has come due.
///
/// It is inverted rather than dropped. A false *denial* is the same defect as a
/// false promise pointed the other way, and it is the more dangerous half in a
/// register: a row that says a protection for an employee under 18 is unbuilt
/// withdraws the reader's only pointer to a guard the shop is actually running,
/// and the guard that used to sit here would have *demanded* that denial forever.
/// Deleting it outright would have left the next stale sentence about čl. 87
/// landing in exactly the place the last one did.
///
/// Bound to the constant rather than only to the words, so renaming the cap
/// stops this file compiling instead of leaving the prose unbacked. The check is
/// line-local for the same reason `popis_register_rows` is: a markdown row is one
/// line, and a correction filed three sections away leaves the row itself reading
/// as an open gap.
#[test]
fn no_document_says_the_cl_87_weekly_leg_is_still_unbuilt() {
    const UNBUILT: [&str; 3] = ["is not checked", "nije proveren", "not implemented"];
    let cap: i64 = crate::worktime::MINOR_WEEKLY_CAP_MINUTES;
    assert_eq!(cap, 35 * 60, "ZoR čl. 87 — 35 časova nedeljno, in minutes");

    for (doc, text) in [
        ("docs/SERBIAN-LAW-COMPLIANCE.md", REGISTER),
        ("docs/PROGRESS.md", PROGRESS),
    ] {
        for needle in ["35 h", "35 časova"] {
            for (line_no, line) in lines_with(text, needle) {
                let stale: Vec<&str> = UNBUILT
                    .iter()
                    .copied()
                    .filter(|marker| line.contains(marker))
                    .collect();
                assert!(
                    stale.is_empty(),
                    "{doc}:{line_no} names the ZoR čl. 87 weekly leg (\"{needle}\") and still \
                     says {stale:?}. The leg shipped 07.08.2026 — \
                     `worktime::MINOR_WEEKLY_CAP_MINUTES`, \
                     `ProtectionKind::MaloletanNedeljniLimit`, and a blocking refusal in \
                     `commands::worktime::write_entry`. Re-state the line: a document that \
                     denies a čl. 87 protection the code runs is the same defect as one that \
                     promises a protection it lacks."
                );
            }
        }
    }
}

/// `worktime::assess_caps` hard-codes `preraspodela_weekly_cap_exceeded` to
/// `false` — it cannot see whether the employee is in preraspodela. The čl. 57
/// st. 5 branch is taken by the caller,
/// `commands::worktime::assess_caps_for_employee`. Pointing an auditor at
/// `assess_caps` for that leg sends them to a function that provably returns
/// `false` for it, so wherever the register puts the two side by side it must
/// also name the caller.
#[test]
fn the_cl_57_st_5_ceiling_is_attributed_to_its_caller() {
    for (line_no, line) in lines_with(REGISTER, "57 st. 5") {
        if !line.contains("assess_caps") {
            continue;
        }
        assert!(
            line.contains("commands/worktime.rs"),
            "docs/SERBIAN-LAW-COMPLIANCE.md:{line_no} attributes ZoR čl. 57 st. 5 to \
             `assess_caps`, which always leaves that leg false. The branch is taken in \
             commands/worktime.rs::assess_caps_for_employee — name it on the same line."
        );
    }
}

/// The offence tačka of the preraspodela breach, where a reader outside the
/// crate meets it. `legal::preraspodela_caps_exceeded` prints **čl. 274 st. 1
/// tač. 4** and `legal`'s own test pins that its penalty never says „tač. 3“
/// (`1216194`) — tač. 3 is the čl. 53 offence, and čl. 57/čl. 60 sit in tač. 4.
///
/// Nothing read the same attribution in the register, and the register lost it:
/// the preraspodela clause was inserted **before** the citation belonging to
/// `overtime_caps_exceeded`, stranding „(čl. 274 st. 1 tač. 3)“ behind the new
/// clause where it reads as preraspodela's own tačka. The amount is identical
/// under the preduzetnik tier, so no figure guard can see it — what is wrong is
/// the article, in the one document counsel and an inspector read.
///
/// Judged window: from the function name to the next clause separator — an em
/// dash, or the next `legal.rs::` reference. The tač. 3 that legitimately
/// belongs to `overtime_caps_exceeded` earlier on the same row is not this
/// function's citation and is not judged here.
#[test]
fn the_preraspodela_citation_is_tacka_4_and_never_tacka_3() {
    const MARKER: &str = "preraspodela_caps_exceeded";

    let rows = lines_with(REGISTER, MARKER);
    assert!(
        !rows.is_empty(),
        "docs/SERBIAN-LAW-COMPLIANCE.md must keep naming `legal.rs::{MARKER}` — it is the only \
         place the register reports that the čl. 57 st. 5 ceiling has penalty copy at all"
    );

    for (line_no, line) in rows {
        for (start, _) in line.match_indices(MARKER) {
            let tail = &line[start + MARKER.len()..];
            let end = [tail.find('—'), tail.find("legal.rs::")]
                .into_iter()
                .flatten()
                .min()
                .unwrap_or(tail.len());
            let citation = &tail[..end];

            assert!(
                citation.contains("čl. 274 st. 1 tač. 4"),
                "docs/SERBIAN-LAW-COMPLIANCE.md:{line_no} names `legal.rs::{MARKER}` without its \
                 offence tačka. It prints čl. 274 st. 1 tač. 4 — čl. 57 and čl. 60 sit in tač. 4 \
                 — and the register is where that attribution is read: {citation}"
            );
            assert!(
                !citation.contains("tač. 3"),
                "docs/SERBIAN-LAW-COMPLIANCE.md:{line_no} attaches čl. 274 st. 1 tač. 3 to \
                 `legal.rs::{MARKER}`. tač. 3 is the čl. 53 offence and belongs to \
                 `overtime_caps_exceeded`; the preduzetnik amount is the same either way, so \
                 nothing but this guard can see the wrong article: {citation}"
            );
        }
    }
}

/// The SW-14 row asserted that „no fine figure exists outside `legal.rs`“. That
/// is the target invariant, not the state of the tree: `ReklamacijeModule.tsx`
/// still hard-codes the regime-versioned reklamacija amounts, which the SW-15
/// row seven lines below documents as an open SW-7 follow-up. A global claim
/// deletes the only pointer to it, so the claim must be scoped to what SW-14
/// shipped or carry the exception with it.
#[test]
fn the_no_fine_figure_claim_stays_scoped_to_what_sw_14_shipped() {
    for (line_no, line) in lines_with(REGISTER, "fine figure") {
        if !line.contains("outside `legal.rs`") {
            continue;
        }
        assert!(
            line.contains("SW-14 introduces") || line.contains("ReklamacijeModule"),
            "docs/SERBIAN-LAW-COMPLIANCE.md:{line_no} claims globally that no fine figure lives \
             outside legal.rs. src/app/reklamacije/ReklamacijeModule.tsx hard-codes the \
             regime-versioned reklamacija amounts — scope the claim to SW-14 or carry the \
             known-exception pointer."
        );
    }
}

/// Failing to deliver the ZZPL čl. 23 notice is čl. 95 st. 1 **tač. 8** („licu
/// na koje se podaci odnose ne pruži informacije iz člana 23. st. 1. do 3.“) —
/// see `docs/SW14-VERIFIED-RULES.md` §3 W5. **tač. 20** is the čl. 42
/// privacy-by-design offence and belongs to register row 12, not to the document
/// handed to an employee.
#[test]
fn the_cl_23_notice_cites_the_right_offence_tacka() {
    assert!(
        NOTICE.contains("čl. 95 st. 1 tač. 8"),
        "docs/compliance/obavestenje-zaposlenima.md must cite ZZPL čl. 95 st. 1 tač. 8 as the \
         offence for failing to deliver the čl. 23 notice"
    );

    let wrong = lines_with(NOTICE, "tač. 20");
    assert!(
        wrong.is_empty(),
        "docs/compliance/obavestenje-zaposlenima.md cites ZZPL čl. 95 st. 1 tač. 20, which is the \
         čl. 42 privacy-by-design offence, not the čl. 23 notice: {wrong:?}"
    );
}

/// The čl. 47 record is the third template the 31.07.2026 penalty-tier sweep
/// never reached. ZZPL **čl. 95 st. 2** is expressly *„rukovalac … koji ima
/// svojstvo pravnog lica“*, so its fixed 100.000 has the wrong subject in a
/// document written for a preduzetnik boutique: the sanction that reaches this
/// shop is **čl. 95 st. 6** — a fixed **50.000** for any st. 2 prekršaj. See
/// `docs/SW14-VERIFIED-RULES.md` §1 row 17 and §3 W5(a).
///
/// The figure matters because of where this file goes. It is handed to the shop
/// and shown to the Poverenik on request, so a number in it reads as the shop's
/// own exposure — printing the pravno-lice sum doubles it and, worse, hides the
/// stav that actually applies, which is the whole defect class the sweep exists
/// to eliminate.
#[test]
fn the_cl_47_record_prints_only_the_preduzetnik_fine_tier() {
    let wrong = lines_with(EVIDENCIJA_CL47, "100.000");
    assert!(
        wrong.is_empty(),
        "docs/compliance/evidencija-obrade-cl47.md prints the pravno-lice fixed sum, which ZZPL \
         čl. 95 st. 2 reserves for a „rukovalac … koji ima svojstvo pravnog lica“. The pilot is a \
         preduzetnik: the figure is a fixed 50.000 under čl. 95 st. 6. Offending lines: {wrong:?}"
    );

    assert!(
        EVIDENCIJA_CL47.contains("50.000"),
        "docs/compliance/evidencija-obrade-cl47.md must state the preduzetnik figure — a fixed \
         50.000 RSD — wherever it names the consequence of not keeping the record"
    );
    assert!(
        EVIDENCIJA_CL47.contains("čl. 95 st. 6"),
        "docs/compliance/evidencija-obrade-cl47.md must cite ZZPL čl. 95 st. 6 as the stav that \
         supplies the preduzetnik sanction. Naming only the st. 2 tačka leaves the reader on the \
         pravno-lice tier, which is how the 100.000 got there in the first place."
    );
}

/// ZZPL čl. 47 **st. 2** is the disapplication for *nadležni organi* processing
/// in the posebne svrhe of čl. 13; the obrađivač's own record is **st. 4**. st. 9
/// settles it in its own words — *„Odredbe st. 1. i 4. ovog člana ne primenjuju
/// se…“* — because the exemption it grants would be incoherent if the processor
/// record lived anywhere else. See `docs/SW14-VERIFIED-RULES.md` §1 row 16.
///
/// A heading is the one line an inspector reads to decide which record they are
/// looking at, so a wrong stav there misfiles the whole section B.
#[test]
fn the_processor_record_is_headed_cl_47_st_4() {
    let wrong = lines_with(EVIDENCIJA_CL47, "čl. 47 st. 2");
    assert!(
        wrong.is_empty(),
        "docs/compliance/evidencija-obrade-cl47.md cites ZZPL čl. 47 st. 2, which disapplies the \
         article for nadležni organi u posebne svrhe. The obrađivač record is čl. 47 st. 4 — \
         st. 9 names „st. 1. i 4.“ as the two records the article creates: {wrong:?}"
    );

    assert!(
        EVIDENCIJA_CL47.contains("čl. 47 st. 4"),
        "docs/compliance/evidencija-obrade-cl47.md must head the obrađivač record with ZZPL \
         čl. 47 st. 4"
    );
}

/// The <250 exemption in čl. 47 st. 9 falls on **both** of the limbs this app
/// triggers, and the record has to say so, because each limb is an independent
/// and separately contestable ground. Limb 2 („obrada nije povremena“) rests on
/// the shop's daily cadence — a factual claim someone could argue with. Limb 3
/// („posebne vrste podataka … iz člana 17. stav 1.“) rests on the absence-hour
/// category, and it is not arguable: `work_time_entries.kategorija_odsustva`
/// carries `sprecenost_poslodavac` and `sprecenost_rfzo`, and the fact of
/// medical incapacity on identified dates is a podatak o zdravstvenom stanju
/// with or without a diagnosis. See `docs/SW14-VERIFIED-RULES.md` §3 W5(a),
/// which is explicit that the file must absorb the second limb.
#[test]
fn the_cl_47_record_invokes_both_limbs_that_destroy_the_250_exemption() {
    assert!(
        EVIDENCIJA_CL47.contains("povremena"),
        "docs/compliance/evidencija-obrade-cl47.md must keep the čl. 47 st. 9 tač. 2 limb — daily \
         POS processing is not „povremena“"
    );

    for needle in ["st. 9 tač. 3", "posebne vrste podataka", "čl. 17 st. 1"] {
        assert!(
            EVIDENCIJA_CL47.contains(needle),
            "docs/compliance/evidencija-obrade-cl47.md invokes only one limb of ZZPL čl. 47 st. 9 \
             — it is missing \"{needle}\". SW-14 destroys the <250 exemption on tač. 3 as well: \
             the absence-hour category (sprecenost_poslodavac / sprecenost_rfzo) is a posebna \
             vrsta podataka iz čl. 17 st. 1, and that limb is the one nobody can argue with."
        );
    }
}

/// The register's popis row disclosed a real gap — čl. 9 st. 3's *„uz
/// štampanje“* had no code behind it — in the strongest terms the file has:
/// *„there is no print or export for a popisna lista“*, twice, once in row 19
/// and once in the §3 SW-16 row. The popis module now writes three documents
/// into `exports/`, so both sentences became false in the same commit that made
/// them false, and nothing here could see it: all eighteen guards above are
/// about quotation marks, retention promises, penalty tiers and the čl. 47
/// record, and none of them reads a claim about output.
///
/// A false *denial* is the mirror of the six false promises this module exists
/// to catch, and it is the more dangerous half in a register: it withdraws the
/// only pointer a reader has to a capability the shop is now taken to have, and
/// it does so in the document handed to whoever advises the shop.
///
/// So the denial is pinned to the code that contradicts it, line-locally. A row
/// is one line of a markdown table, so requiring the correction on the same line
/// is requiring it in the same cell — a superseding note filed three sections
/// away would leave the row itself reading as a closed item.
///
/// **Amended 07.08.2026, and the amendment tightens the rule rather than
/// relaxing it.** *„Correct it in the same cell“* was the right rule while the
/// requirements were being built one task at a time, and the comment above
/// named its own successor: delete the denial and this guard goes quiet, which
/// is the right outcome once reqs. 31/32/35 are **written up** rather than
/// corrected in place. That is what the rule is now — the withdrawn sentence
/// must be **gone** from the register, and each popis cell must name the command
/// that replaced it. A cell that reads as a denial followed by two supersessions
/// is not a statement of what the software does, and the register is the one
/// document counsel and an inspector read for exactly that. Nothing is lost by
/// the deletion: the dated disclosure stays in `docs/PROGRESS.md`, which records
/// what was true when, and this guard is deliberately not run over that file for
/// the same reason.
#[test]
fn the_register_states_the_popis_export_it_has_instead_of_the_denial_it_replaced() {
    for poricanje in [
        "there is no print or export for a popisna lista",
        "there is no print and no export for a popisna lista",
        "`PopisService` has fifteen methods and none exports",
    ] {
        let zaostalo = lines_with(REGISTER, poricanje);
        assert!(
            zaostalo.is_empty(),
            "docs/SERBIAN-LAW-COMPLIANCE.md still says \"{poricanje}\", which stopped being true \
             when `popis_export_lista`, `popis_export_odluka` and `popis_export_plan_rada` \
             shipped. The register states the software's current state, so the denial is restated \
             and not annotated — `docs/PROGRESS.md` is where the dated disclosure belongs. \
             Offending lines: {zaostalo:?}"
        );
    }

    for (celija, line, text) in popis_register_rows() {
        assert!(
            text.contains("popis_export_lista"),
            "docs/SERBIAN-LAW-COMPLIANCE.md:{line} is {celija} and describes the popis module \
             without naming `popis_export_lista`. čl. 9 st. 3's „uz štampanje“ is what this row \
             is about; a reader takes the row alone, so the command that discharges it has to be \
             in the cell."
        );
    }
}

/// The two cells of `docs/SERBIAN-LAW-COMPLIANCE.md` that describe the popis
/// module — §2's register row 19 and §3's SW-16 row — as `(name, line number,
/// text)`. A markdown table row is one line, so a cell is judged on its own
/// line: that is the whole point of a per-row status column.
///
/// Bound to the command it describes rather than only to the words, so renaming
/// the export stops this file compiling instead of leaving the register's claim
/// unbacked.
fn popis_register_rows() -> Vec<(&'static str, usize, &'static str)> {
    let _liste: crate::commands::popis::ExportListaFn = crate::commands::popis::popis_export_lista;

    let rows: Vec<(&'static str, usize, &'static str)> = REGISTER
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            if line.starts_with("| 19 |") {
                Some(("§2 register row 19", index + 1, line))
            } else if line.starts_with("| SW-16 |") {
                Some(("the §3 SW-16 row", index + 1, line))
            } else {
                None
            }
        })
        .collect();

    assert_eq!(
        rows.len(),
        2,
        "docs/SERBIAN-LAW-COMPLIANCE.md must keep both popis cells — §2 row 19 (the obligation) \
         and the §3 SW-16 row (what shipped against it). Found: {rows:?}"
    );
    rows
}

/// The izveštaj o popisu is the one popis document this crate does **not** write
/// to a file. `commands::popis::popis_izvestaj` composes it on demand and hands
/// back a view for the screen; no command anywhere writes one out, which is what
/// the three „program ga ne štampa i ne izvozi“ strings have always meant and
/// what both popis cells now say in English.
///
/// A denial goes false the way a promise does — by the code moving under it —
/// and this one goes false the moment somebody ships the fourth export and
/// restates nothing. So it is pinned from both sides: the register must carry
/// the sentence, and `lib.rs`'s invoke handler must contain no command that both
/// names an izveštaj and writes it out. The handler list is the right place to
/// look for the second half, because a command no `generate_handler!` names is
/// one no operator can reach — the reasoning
/// `cl47::tests::every_export_the_sweep_allows_is_registered_as_a_command` uses
/// for the three exports that do exist.
#[test]
fn nothing_exports_the_izvestaj_and_both_popis_cells_say_so() {
    const LIB: &str = include_str!("lib.rs");
    /// The register's own words for it, in both cells.
    const PORICANJE: &str = "neither printed nor exported";
    /// A handler name that would write one out. ASCII, because a Rust
    /// identifier is.
    const IZLAZ: [&str; 4] = ["export", "izvoz", "print", "stampa"];

    for (celija, line, text) in popis_register_rows() {
        assert!(
            text.contains("izveštaj") && text.contains(PORICANJE),
            "docs/SERBIAN-LAW-COMPLIANCE.md:{line} is {celija} and no longer says that the \
             izveštaj o popisu is „{PORICANJE}“. Three documents are exported and this one is not, \
             so a cell that names the exports without excepting the izveštaj reads as though the \
             whole module prints — the same defect facing the other way."
        );
    }

    for line in LIB.lines() {
        let unos = line.trim().trim_end_matches(',');
        if !unos.starts_with("commands::") || !unos.to_lowercase().contains("izvestaj") {
            continue;
        }
        let unos_lower = unos.to_lowercase();
        assert!(
            !IZLAZ.iter().any(|verb| unos_lower.contains(verb)),
            "`{unos}` is registered as a command, and its name says it writes an izveštaj out. \
             Both popis cells of docs/SERBIAN-LAW-COMPLIANCE.md say the izveštaj is \
             „{PORICANJE}“, and so do `retention.rs`'s stored napomena, the čl. 47 register's \
             `popis_imovine` entry and the izveštaj's own upozorenje in `commands/popis.rs`. \
             Restate all of them in the same commit — a stale denial withdraws the reader's only \
             pointer to a capability the shop is now taken to have."
        );
    }
}
