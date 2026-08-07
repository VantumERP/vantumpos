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

/// `pub(crate)` for one reader outside this module: `legal.rs`'s own test module
/// holds `all_notices`, the exhaustive list of every fine figure this crate can
/// render, and that list is private to it by design. Register row 24 denies that
/// the list carries a sniženje notice, so the guard over that denial has to run
/// where the list lives — and embedding the register a second time there would
/// leave two copies to keep in step.
pub(crate) const REGISTER: &str = include_str!("../../docs/SERBIAN-LAW-COMPLIANCE.md");
const PROGRESS: &str = include_str!("../../docs/PROGRESS.md");
const NOTICE: &str = include_str!("../../docs/compliance/obavestenje-zaposlenima.md");
const EVIDENCIJA_CL47: &str = include_str!("../../docs/compliance/evidencija-obrade-cl47.md");
/// The anti-evazioni memo (SW-4). Its stated purpose is a standing written
/// rebuttal handed to an inspector, and its §4 describes, feature by feature,
/// what the go-live reset shows the operator — so it is a description of this
/// crate in exactly the sense the module doc means, and it went a week
/// describing a dialog the app no longer had. Embedded 07.08.2026.
const MEMO: &str = include_str!("../../docs/compliance/memo-uskladjenost-fiskalizacije.md");

/// Every line of `text` containing `needle`, numbered from 1 the way an editor
/// numbers them so a failure message points straight at the line to fix.
fn lines_with<'a>(text: &'a str, needle: &str) -> Vec<(usize, &'a str)> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| line.contains(needle))
        .map(|(index, line)| (index + 1, line))
        .collect()
}

/// The clause of `line` that the byte offset `at` falls inside.
///
/// A register row is one line, but it is also a paragraph's worth of separate
/// claims about six different articles. A guard that judges the whole line
/// cannot tell *which* claim a word belongs to, so it either misses the wording
/// it exists to catch or fires on a neighbouring statement that is perfectly
/// true — and the second failure is the worse one, because the way out of it is
/// to phrase the true statement around the guard.
///
/// The breaks are the separators this repository's prose actually uses: `;`, the
/// em dash (never the en dash, which sits inside „čl. 87–91“), the parentheses,
/// and the markdown cell pipe. A full stop is deliberately not one — „čl. 87“
/// carries one, and splitting there would cut a citation in half.
fn clause_around(line: &str, at: usize) -> &str {
    const BREAKS: [char; 5] = [';', '—', '(', ')', '|'];

    let start = line[..at]
        .char_indices()
        .rev()
        .find(|(_, ch)| BREAKS.contains(ch))
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let end = line[at..]
        .char_indices()
        .find(|(_, ch)| BREAKS.contains(ch))
        .map_or(line.len(), |(index, _)| at + index);

    line[start..end].trim()
}

/// `true` when the byte offset `at` holds a full stop that actually ends a
/// sentence — one followed by a space and a capital letter.
///
/// [`clause_around`] refuses the full stop outright because „čl. 87“ carries
/// one. The lookahead buys the character back: „čl. 28 st. 4“ is followed by a
/// digit and „čl. 16 st. 2 confines“ by a lower-case word, so neither is cut in
/// half, while „…rezervna kopija. Pravna lica ne smeju…“ splits where a reader
/// would split it.
fn ends_a_sentence(text: &str, at: usize) -> bool {
    let mut rest = text[at..].chars();
    rest.next() == Some('.')
        && rest.next() == Some(' ')
        && rest.next().is_some_and(char::is_uppercase)
}

/// The sentence the byte offset `at` falls inside.
///
/// A claim is made in a sentence, and judging a wider window is how a guard
/// starts accusing a developer of a statement three paragraphs away — the
/// failure [`clause_around`]'s doc comment calls the worse one, because the way
/// out of it is to phrase a true statement around the guard. The breaks are the
/// end of a sentence plus the structure that ends a statement as firmly as a
/// full stop does: a JSX element boundary, an interpolation brace, a markdown
/// cell pipe. Text is expected whitespace-collapsed, the way the JSX guards
/// read a `.tsx` file.
fn sentence_around(text: &str, at: usize) -> &str {
    let (start, end) = sentence_span(text, at);
    text[start..end].trim()
}

/// The same window as [`sentence_around`], as byte offsets into `text`.
///
/// A guard that has to decide *where inside the window* a second needle sits —
/// „is this denial the one the paragraph withdraws, or the one it makes?“ —
/// cannot answer it from a `&str` it can no longer locate. Offsets are what
/// [`inside_a_serbian_quotation`] takes.
fn sentence_span(text: &str, at: usize) -> (usize, usize) {
    const BREAKS: [char; 5] = ['<', '>', '{', '}', '|'];

    let start = text[..at]
        .char_indices()
        .rev()
        .find(|(index, ch)| BREAKS.contains(ch) || ends_a_sentence(text, *index))
        .map_or(0, |(index, ch)| index + ch.len_utf8());
    let end = text[at..]
        .char_indices()
        .find(|(index, ch)| BREAKS.contains(ch) || ends_a_sentence(text, at + index))
        .map_or(text.len(), |(index, _)| at + index);

    (start, end)
}

/// The markdown block the byte offset `at` falls inside, collapsed onto one
/// line, with the block's own first line number.
///
/// [`lines_with`] is right for the register, where a row is a line. It is wrong
/// for `PROGRESS.md`, whose residual lists wrap one bullet across four lines:
/// „…`SettingsScreen.tsx:1771` still“ ends a line and „attributes the 10-year
/// floor to **ZPDV čl. 47**“ opens the next, so a line-local guard sees the
/// file reference and the claim about it as two unrelated statements and passes
/// on both. A block ends at the next bullet, **numbered item**, table row,
/// heading or blank line.
///
/// The numbered item was added 07.08.2026: this repository writes its review-fix
/// records as `1.` … `7.`, and without it a seven-item list collapses into one
/// block, so a guard reports the whole list as the offending text and cannot say
/// which item it is about. That is the same over-wide window
/// [`sentence_around`]'s doc comment describes, one structure up.
fn markdown_block_around(text: &str, at: usize) -> (usize, String) {
    fn starts_a_block(line: &str) -> bool {
        let trimmed = line.trim_start();
        let numbered = trimmed.split_once(". ").is_some_and(|(head, _)| {
            !head.is_empty() && head.chars().all(|ch| ch.is_ascii_digit())
        });
        line.trim().is_empty()
            || trimmed.starts_with("- ")
            || trimmed.starts_with("* ")
            || trimmed.starts_with('|')
            || trimmed.starts_with('#')
            || numbered
    }

    let mut offset = 0usize;
    let lines: Vec<(usize, &str)> = text
        .lines()
        .map(|line| {
            let entry = (offset, line);
            offset += line.len() + 1;
            entry
        })
        .collect();

    let index = lines
        .iter()
        .rposition(|(start, _)| *start <= at)
        .unwrap_or(0);
    let mut start = lines[..=index]
        .iter()
        .rposition(|(_, line)| starts_a_block(line))
        .unwrap_or(0);
    if lines[start].1.trim().is_empty() && start < index {
        start += 1;
    }
    let end = lines[index + 1..]
        .iter()
        .position(|(_, line)| starts_a_block(line))
        .map_or(lines.len(), |ahead| index + 1 + ahead);

    let block = lines[start..end]
        .iter()
        .map(|(_, line)| line.trim())
        .collect::<Vec<_>>()
        .join(" ");

    (start + 1, block)
}

/// Every byte offset in `text` at which `needle` occurs, ignoring case.
///
/// `str::match_indices` is case-sensitive, which is how a guard on the stem
/// „arhiv“ passes a sentence that opens „Arhiv mora…“ — the ordinary way a
/// Serbian sentence starts, and therefore the ordinary way a withdrawn claim
/// comes back. `needle` must already be lower-case; the offsets are into `text`
/// itself, so a failure message still quotes the copy in its real casing rather
/// than a folded copy of it.
fn match_indices_ci(text: &str, needle: &str) -> Vec<usize> {
    debug_assert!(
        needle == needle.to_lowercase(),
        "match_indices_ci folds the haystack only: „{needle}“"
    );
    let folded: Vec<char> = needle.chars().collect();

    text.char_indices()
        .filter(|(at, _)| {
            let mut haystack = text[*at..].chars().flat_map(char::to_lowercase);
            folded
                .iter()
                .all(|expected| haystack.next() == Some(*expected))
        })
        .map(|(at, _)| at)
        .collect()
}

/// One Tailwind type-scale class, in **milli-rem**, or `None` when the token is
/// not a size at all.
///
/// PVFR čl. 2 st. 8–10 states a ratio, and a ratio is arithmetic — so the two
/// non-fiscal banners are measured rather than asserted to be present and called
/// large enough. Milli-rem keeps it in integers, per the house rule that no
/// decision in this crate is taken in floating point; a font size is not money,
/// but a comparison that decides a compliance claim by a rounding mode is the
/// same defect wherever it sits.
///
/// The figures are the framework defaults, and they are the shipped ones:
/// `src/App.css` declares no `--text-*` in its `@theme inline` block and sets no
/// `html`/`body` font-size, so nothing overrides them. A modifier after the
/// slash is a line height — `text-xs/relaxed` is still `text-xs` — and the
/// wrapping punctuation of a JSX class string is trimmed, because
/// `cn("w-full caption-bottom text-xs", className)` ends the token with `",`.
fn tailwind_type_scale(raw: &str) -> Option<(&str, i64)> {
    let trimmed = raw.trim_matches(|ch: char| "\"'`(){},".contains(ch));
    let token = trimmed.split('/').next()?;
    let milli_rem = match token {
        "text-xs" => 750,
        "text-sm" => 875,
        "text-base" => 1000,
        "text-lg" => 1125,
        "text-xl" => 1250,
        "text-2xl" => 1500,
        "text-3xl" => 1875,
        "text-4xl" => 2250,
        _ => return None,
    };
    Some((token, milli_rem))
}

/// The type-scale class of the element that **encloses** `anchor`, searching
/// backwards from it to that element's own `className`.
///
/// The banner is written text-last — `className="… text-2xl …"` on one line and
/// „OVO NIJE FISKALNI RAČUN“ on the next — so the size that applies to the
/// anchor is the last one before it inside the same attribute. `text-center` and
/// `text-destructive` sit in the same string and are not sizes, which is why the
/// scan is by token and not by substring.
fn type_scale_before<'a>(source: &'a str, anchor: &str) -> Option<(&'a str, i64)> {
    let at = source.find(anchor)?;
    let start = source[..at].rfind("className")?;
    source[start..at]
        .split_whitespace()
        .filter_map(tailwind_type_scale)
        .next_back()
}

/// The first type-scale class **after** `anchor`, within `tokens` whitespace-
/// separated words of it.
///
/// The shadcn primitives are written attribute-first — `data-slot="dialog-content"`
/// then the class string — so the size that a child inherits is the first one
/// after the slot. The token budget keeps the scan inside the element it started
/// in rather than running on into the next component in the file.
fn type_scale_after<'a>(source: &'a str, anchor: &str, tokens: usize) -> Option<(&'a str, i64)> {
    let at = source.find(anchor)?;
    source[at..]
        .split_whitespace()
        .take(tokens)
        .find_map(tailwind_type_scale)
}

/// `true` when the byte offset `at` sits inside a Serbian quotation.
///
/// A guard over a claim has to tell an **assertion** from a **report of one**.
/// `docs/PROGRESS.md` is this repository's record of what was true when, and a
/// dated correction that cannot quote the sentence it withdraws is worth
/// nothing — the same reasoning
/// [`no_compliance_template_states_a_reset_warning_the_dialog_does_not_show`]
/// applies to a line marked `Ispravka`, expressed here as punctuation instead of
/// a keyword. Judged on the nearest preceding quotation mark rather than by
/// counting from the top of the file, so one unbalanced pair somewhere else in a
/// 1500-line document cannot invert the answer for every later claim.
///
/// It does **not** weaken any guard that uses it: every stale sentence this
/// module has caught — the register's *„app currently prints nothing“*, the
/// *„Req. 12 — CLOSED“* header, the reset dialog's archive duty — was written as
/// the document's own assertion, unquoted.
fn inside_a_serbian_quotation(text: &str, at: usize) -> bool {
    text[..at]
        .rfind(['„', '“'])
        .is_some_and(|index| text[index..].starts_with('„'))
}

/// The stems that turn a mention of an arhiv into the claim ZAG čl. 16 st. 2
/// does not support: that material may not be destroyed without the archive's
/// prior approval. Returns the pair that fired, for the failure message.
///
/// Two stems, not one phrase. „bez pismenog odobrenja **nadležnog javnog**
/// arhiva“ is the statute's own wording and walks straight past a literal, and
/// „odobren“ alone is blind to „odobri“, „odobriti“, „odobrava“ and to
/// „saglasnost“, which is the word the register itself uses for the archive's
/// real consent. The destruction stem is what keeps the guard off the duties
/// that are **real**: the lista kategorija sa saglasnošću nadležnog javnog
/// arhiva, the arhivska knjiga and the 30 April prepis are all statable, as
/// long as they are not stated as a precondition of destroying anything —
/// which is precisely the conflation §1 row 5 corrects.
fn claims_an_archive_destruction_approval(recenica: &str) -> Option<(&'static str, &'static str)> {
    const ODOBRENJE: [&str; 3] = ["odobr", "saglasn", "dozvol"];
    const UNISTENJE: [&str; 4] = ["uništ", "unis", "briš", "bris"];

    let lowered = recenica.to_lowercase();
    let odobrenje = ODOBRENJE.into_iter().find(|stem| lowered.contains(stem))?;
    let unistenje = UNISTENJE.into_iter().find(|stem| lowered.contains(stem))?;

    Some((odobrenje, unistenje))
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
        (
            "docs/compliance/memo-uskladjenost-fiskalizacije.md".to_string(),
            MEMO.to_string(),
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
/// stops this file compiling instead of leaving the prose unbacked. The search
/// is line-local for the same reason `popis_register_rows` is — a markdown row
/// is one line, and a correction filed three sections away leaves the row itself
/// reading as an open gap — but it is judged *clause*-local, per
/// [`clause_around`]: the SW-14 row states the čl. 87 weekly leg as shipped and
/// the čl. 88 st. 2 night leg as unbuilt on the same line, and the second denial
/// is TRUE. Judged line-wide, the marker list has to stay so thin that the
/// repository's own word for a gap („Gap“) slips through it, and a true denial
/// has to be phrased around the guard — which is how a check becomes decorative.
///
/// Each document must also still *name* the leg. Every other guard in this file
/// asserts presence before it judges wording (`:174`, `:362`, `:654`) and this
/// one was the exception: with no hits the loop simply did not run, so deleting
/// the prose passed green. Silence is the same harm as a stale denial — it
/// withdraws the reader's only pointer to a guard the shop is running.
#[test]
fn no_document_says_the_cl_87_weekly_leg_is_still_unbuilt() {
    /// Lower-case, per [`match_indices_ci`]'s contract. They were compared with
    /// `str::contains` until 08.08.2026, which is how „Gap“ was written and a
    /// lower-case „gap“ — or a „Not enforced“ opening a table cell — walked
    /// past. Same defect, same file, as the „Arhiv mora…“ one that put
    /// `match_indices_ci` here in the first place.
    const UNBUILT: [&str; 10] = [
        "is not checked",
        "nije proveren",
        "not implemented",
        "nije implementiran",
        "not built",
        "unbuilt",
        "not enforced",
        "nije sprovedeno",
        "nema proveru",
        "gap",
    ];
    let cap: i64 = crate::worktime::MINOR_WEEKLY_CAP_MINUTES;
    assert_eq!(cap, 35 * 60, "ZoR čl. 87 — 35 časova nedeljno, in minutes");

    for (doc, text) in [
        ("docs/SERBIAN-LAW-COMPLIANCE.md", REGISTER),
        ("docs/PROGRESS.md", PROGRESS),
    ] {
        let mut pomena = 0usize;
        for needle in ["35 h", "35 časova"] {
            for (line_no, line) in lines_with(text, needle) {
                for (at, _) in line.match_indices(needle) {
                    pomena += 1;
                    let clause = clause_around(line, at);
                    let stale: Vec<&str> = UNBUILT
                        .iter()
                        .copied()
                        .filter(|marker| !match_indices_ci(clause, marker).is_empty())
                        .collect();
                    assert!(
                        stale.is_empty(),
                        "{doc}:{line_no} names the ZoR čl. 87 weekly leg (\"{needle}\") and the \
                         clause it sits in still says {stale:?} — „{clause}“. The leg shipped \
                         07.08.2026: `worktime::MINOR_WEEKLY_CAP_MINUTES`, \
                         `ProtectionKind::MaloletanNedeljniLimit`, and a blocking refusal in \
                         `commands::worktime::write_entry` pinned by \
                         `commands::worktime::tests::\
                         a_birth_date_filled_in_later_does_not_lock_a_minors_recorded_week`. \
                         Re-state the clause: a document that denies a čl. 87 protection the \
                         code runs is the same defect as one that promises a protection it \
                         lacks. A denial about a *different* leg — čl. 88 st. 2 night work is \
                         genuinely unbuilt — belongs in a clause of its own."
                    );
                }
            }
        }
        assert!(
            pomena > 0,
            "{doc} must keep naming the ZoR čl. 87 weekly leg („35 h“ / „35 časova“) — \
             `worktime::MINOR_WEEKLY_CAP_MINUTES` refuses a minor's row on it, and a register \
             that says nothing about a protection the shop is running is the same defect as one \
             that denies it. Re-state the line rather than deleting it."
        );
    }
}

/// The screen that explains čl. 87 to the operator must state **both** of its
/// legs, because the code enforces both.
///
/// `AppShell.tsx`'s „Datum rođenja“ field is the only place in the application
/// that says what filling that column in does, and the paragraph above it says
/// the čl. 87–91 checks are derived from these fields. Since 07.08.2026 the same
/// column also switches on `MINOR_WEEKLY_CAP_MINUTES`, a hard refusal at 35
/// časova nedeljno. A description that names only the eight-hour daily leg
/// attributes to čl. 87 half of its own sentence, and the half it drops is the
/// one that actually binds a scheduled minor: six six-hour days satisfy the leg
/// the screen names on every single day and are refused on the sixth save with a
/// figure the operator was never shown.
///
/// This is the **denial** direction of the rule this module exists for. A screen
/// is prose in exactly the sense the register is — it is the reader's only
/// account of what the code does — and a stale denial is no safer for being
/// rendered in a dialog instead of a markdown table. `UserDialog.test.tsx` pins
/// the field's label and its article citations; nothing pinned the figures.
///
/// JSX wraps a sentence across source lines, so the file is judged with its
/// whitespace collapsed and each claim is judged inside the `FieldDescription`
/// it belongs to — the same reasoning as [`clause_around`], with the element
/// boundary standing in for the punctuation. Bound to both constants so that
/// renaming either cap stops this file compiling rather than leaving the screen
/// unbacked.
#[test]
fn the_profile_screen_states_both_legs_of_cl_87() {
    const APP_SHELL: &str = include_str!("../../src/app/AppShell.tsx");
    /// The daily leg's figure, in the words the screen uses.
    const DNEVNO: &str = "osam časova dnevno";
    /// The weekly one's.
    const NEDELJNO: &str = "35 časova nedeljno";

    assert_eq!(
        crate::worktime::MINOR_DAILY_CAP_MINUTES,
        8 * 60,
        "ZoR čl. 87 — osam časova dnevno, in minutes"
    );
    assert_eq!(
        crate::worktime::MINOR_WEEKLY_CAP_MINUTES,
        35 * 60,
        "ZoR čl. 87 — 35 časova nedeljno, in minutes"
    );

    let text = APP_SHELL.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut pomena = 0usize;
    for (at, _) in text.match_indices(DNEVNO) {
        pomena += 1;
        let start = text[..at]
            .rfind("<FieldDescription>")
            .map_or(0, |index| index);
        let end = text[at..]
            .find("</FieldDescription>")
            .map_or(text.len(), |index| at + index);
        let opis = text[start..end].trim();
        assert!(
            opis.contains(NEDELJNO),
            "src/app/AppShell.tsx tells the operator that ZoR čl. 87 caps an employee under 18 \
             at „{DNEVNO}“ and stops there — „{opis}“. `worktime::MINOR_WEEKLY_CAP_MINUTES` \
             refuses the write at „{NEDELJNO}“ on the strength of the very field this text \
             describes, and six six-hour days satisfy the daily leg on every day and are still \
             refused. State both legs: denying a protection the code runs is the same defect as \
             promising one it lacks."
        );
    }
    assert!(
        pomena > 0,
        "src/app/AppShell.tsx no longer tells the operator what filling „Datum rođenja“ in does. \
         That field switches on both legs of ZoR čl. 87 plus the čl. 88 st. 1 bans, and it is \
         nullable and unbackfilled — silence about a guard the shop is running is the same harm \
         as a stale denial. Re-state the description rather than deleting it."
    );
}

/// The reset dialog must state the retention floor the tombstone records, and
/// must not send the shop for an archive approval no article asks of it.
///
/// Two claims, one paragraph of `GoLiveResetCard`, both wrong since SW-3
/// (`062250d`) and both read by an owner one click away from a delete that
/// cannot be undone.
///
/// **The floor.** `reset_trading_data`'s `compliance_log` tombstone said
/// „10y (ZoRač čl. 28; ZPDV čl. 47)“ until 31.07.2026, when `9fa88b8` corrected
/// it to [`crate::commands::backup::ROK_CUVANJA_PRAVNI_OSNOV`] on
/// SW11-SW15-VERIFIED-RULES.md §2 Q4 — ZPDV čl. 47 supplies no general period at
/// all. The dialog kept the withdrawn string verbatim, so the application stated
/// two different floors for one duty depending on which surface the reader was
/// on, and the surface with the wrong one is the surface the owner reads. The
/// screen is therefore bound to the constant and not to a copy of it: correcting
/// one surface and leaving the other is exactly how these two came apart.
///
/// **The archive approval.** ZAG čl. 16 st. 2 confines prior written approval of
/// the nadležni javni arhiv to državni organi, organi teritorijalne autonomije i
/// jedinica lokalne samouprave, ustanove, javna preduzeća i imaoci javnih
/// ovlašćenja (§1 row 5, §3 req. 39). The pilot is a preduzetnik, and the
/// sentence sent him for a permission no article asks of him — an operator string
/// asserting a duty the law does not impose, which is the same defect as one
/// denying a duty it does.
///
/// **The floor is a floor.** §1 row 7 records two defects in the SW-3 sentence,
/// not one: the ZPDV attribution *and* „10y absolute“ presented as a **ceiling**.
/// „do 10 godina“ is „up to 10 years“. ZPPPA čl. 114z st. 2 excludes zastoj from
/// the absolute period and čl. 114ž itself ends „osim ako ovim zakonom nije
/// drukčije propisano“, which is why req. 36 makes `retain_until` upward-only and
/// why `SERBIAN-LAW-COMPLIANCE.md:76` says it in terms — „10 years is a floor,
/// NOT a wall-clock ceiling“. A ceiling printed one click above an irreversible
/// delete is a number the shop cannot act on, so the ceiling markers are barred
/// as well as required — the first correction of this sentence fixed the citation
/// and left the framing, and a guard that pinned only the citation would have
/// held the framing in place.
///
/// **The custody note.** §3 req. 39 has two limbs, and the second is „keep a
/// neutral čl. 9 st. 1 custody note — do not tell him archive law does not
/// apply“. ZAG čl. 9 st. 1 (savesno čuvanje u sređenom i bezbednom stanju)
/// carries none of the „osim fizičkih lica“ carve-out that st. 2 does (§1 row
/// 12), so it reaches this shop; the ZoRač/ZPPPA čuvanje sentence is an
/// accounting and tax retention period under different statutes and is not that
/// duty. Withdrawing the false claim and leaving nothing satisfies half of req.
/// 39, so the note is required here rather than merely permitted.
///
/// The archive claim is judged per [`claims_an_archive_destruction_approval`],
/// on stems inside **one sentence** rather than on the phrase that was
/// withdrawn. It deliberately does not bar the archive duties that are **real**
/// — the lista kategorija with the arhiv's saglasnost, the arhivska knjiga, the
/// 30 April prepis — nor may the screen ever tell a preduzetnik that archive law
/// does not reach him (§4 item 8). Only approval-as-a-precondition-of-destruction
/// is barred, and stating a real duty in a sentence of its own is the way to
/// state it.
///
/// JSX wraps a sentence across source lines, so the file is judged with its
/// whitespace collapsed. The retention citation is judged inside the `<p>` it
/// belongs to, because a citation belongs to its paragraph; the archive claim is
/// judged inside its sentence, because the only lower-case „arhiv“ in this file
/// sits in a JSX comment inside no `<p>` at all, and the enclosing-paragraph
/// search then spanned 267 lines — the passphrase card, the whole backup form and
/// two dialogs — so any „odobren“ anywhere in that span accused the developer of
/// a claim about an arhiv. Source comments are swept along with the copy,
/// deliberately: the note recording why the claim was withdrawn is written in
/// English for exactly that reason, and restating the withdrawn duty in Serbian
/// beside the dialog it was removed from is how it would find its way back into
/// the dialog.
#[test]
fn the_reset_dialog_matches_the_tombstone_and_claims_no_archive_approval() {
    const SETTINGS_SCREEN: &str = include_str!("../../src/app/settings/SettingsScreen.tsx");
    /// The sentence carrying the floor, in the words the dialog uses.
    const ROK: &str = "čuvanje evidencija najmanje 10 godina";
    /// Ways of writing the same period as a wall-clock ceiling.
    const TAVANICA: [&str; 4] = [
        "do 10 godina",
        "najviše 10 godina",
        "najduže 10 godina",
        "maksimalno 10 godina",
    ];
    /// The čl. 9 st. 1 custody duty, in the statute's own terms.
    const CUVANJE: [&str; 2] = ["ZAG čl. 9 st. 1", "u sređenom i bezbednom stanju"];

    /// The `<p>` element `at` falls inside, or `None` when it falls inside no
    /// paragraph — a JSX comment, an attribute, a `<title>`. A `</p>` between
    /// the opening tag and `at` is the proof that the candidate closed before
    /// `at` was reached; without that test the search silently walks backwards
    /// past every intervening paragraph and judges the whole file.
    fn paragraph_around(text: &str, at: usize) -> Option<&str> {
        let start = text[..at]
            .rmatch_indices("<p")
            .find(|(index, _)| matches!(text[index + 2..].chars().next(), Some('>' | ' ')))
            .map(|(index, _)| index)?;
        if text[start..at].contains("</p>") {
            return None;
        }
        let end = text[at..].find("</p>").map(|ahead| at + ahead)?;

        Some(text[start..end].trim())
    }

    let osnov = crate::commands::backup::ROK_CUVANJA_PRAVNI_OSNOV;
    assert!(
        !osnov.contains("ZPDV"),
        "SW11-SW15-VERIFIED-RULES.md §2 Q4 — ZPDV čl. 47 sets no general retention period; its \
         „najmanje deset godina“ limb is object-specific to the čl. 32 objekti i ulaganja. \
         `commands::backup::ROK_CUVANJA_PRAVNI_OSNOV` is what both the compliance tombstone and \
         the reset dialog print: „{osnov}“"
    );

    let text = SETTINGS_SCREEN
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let claim_around = |at: usize| -> &str {
        paragraph_around(&text, at).unwrap_or_else(|| sentence_around(&text, at))
    };

    let mut pomena = 0usize;
    for at in match_indices_ci(&text, &ROK.to_lowercase()) {
        pomena += 1;
        let odlomak = claim_around(at);
        for clan in osnov.split(';').map(str::trim) {
            assert!(
                odlomak.contains(clan),
                "src/app/settings/SettingsScreen.tsx warns about the retention floor without \
                 naming „{clan}“ — „{odlomak}“. The floor the app records in `compliance_log` is \
                 `commands::backup::ROK_CUVANJA_PRAVNI_OSNOV` („{osnov}“); a dialog citing \
                 anything else leaves the shop with two floors for one duty."
            );
        }
        assert!(
            !odlomak.contains("ZPDV"),
            "src/app/settings/SettingsScreen.tsx attributes the retention floor to ZPDV — \
             „{odlomak}“. §2 Q4: čl. 47 supplies no general period, and `backup.rs` stopped \
             citing it for this duty on 31.07.2026. Cite „{osnov}“, the string the tombstone \
             carries."
        );
    }
    assert!(
        pomena > 0,
        "src/app/settings/SettingsScreen.tsx no longer warns („{ROK}“) before the go-live reset. \
         The floor is real — `commands::backup::ROK_CUVANJA_PRAVNI_OSNOV` — and the reset is \
         irreversible, so silence in front of the confirmation box is worse than the wrong \
         citation was. Re-state the sentence rather than deleting it."
    );

    for tavanica in TAVANICA {
        // The first occurrence is the whole finding: one ceiling on this screen
        // is one too many, and quoting it is what the message is for.
        if let Some(&at) = match_indices_ci(&text, tavanica).first() {
            let recenica = sentence_around(&text, at);
            panic!(
                "src/app/settings/SettingsScreen.tsx prints the retention period as a ceiling \
                 („{tavanica}“) — „{recenica}“. SW11-SW15-VERIFIED-RULES.md §1 row 7 rates that \
                 framing a defect of its own: ZPPPA čl. 114z st. 2 keeps zastoj out of the \
                 absolute period and čl. 114ž ends „osim ako ovim zakonom nije drukčije \
                 propisano“, so 10 years is a floor that only ever moves up. Say „najmanje“."
            );
        }
    }

    for stem in CUVANJE {
        assert!(
            text.contains(stem),
            "src/app/settings/SettingsScreen.tsx dropped the neutral ZAG čl. 9 st. 1 custody note \
             („{stem}“). §3 req. 39 asks for two things and the second is this one: withdraw the \
             destruction-approval claim **and** keep a neutral custody note, because čl. 9 st. 1 \
             carries no „osim fizičkih lica“ carve-out (§1 row 12) and a screen that answers the \
             withdrawal with silence tells the preduzetnik archive law does not reach him — \
             §4 item 8, the thing this file may never say."
        );
    }

    for at in match_indices_ci(&text, "arhiv") {
        let recenica = sentence_around(&text, at);
        if let Some((odobrenje, unistenje)) = claims_an_archive_destruction_approval(recenica) {
            panic!(
                "src/app/settings/SettingsScreen.tsx tells the shop an arhiv has to approve a \
                 destruction — „{recenica}“ (stems „{odobrenje}“ + „{unistenje}“). ZAG čl. 16 \
                 st. 2 confines that approval to državni organi, organi TA i JLS, ustanove, javna \
                 preduzeća i imaoci javnih ovlašćenja (§1 row 5, §3 req. 39), and the pilot is a \
                 preduzetnik, so the sentence sends him for a permission no article asks of him. \
                 Withdraw the claim; a hedged version is still an assertion he cannot act on. The \
                 archive duties that are real — lista kategorija sa saglasnošću, arhivska knjiga, \
                 30 April prepis — are statable, in a sentence that does not make one of them the \
                 precondition of a destruction."
            );
        }
    }
}

/// A template under `docs/compliance/` may not describe a reset warning the
/// dialog does not show.
///
/// The memo's §4 is a feature-by-feature account of the go-live reset, written
/// to be handed to an inspector as a standing rebuttal, and it went a week
/// stating the withdrawn citation and the withdrawn archive duty after the
/// dialog stopped showing either. Nothing could see it: the module doc says the
/// templates under `docs/compliance/` are read as a description of what the code
/// does, but only two of them were embedded, and the memo — the one written for
/// a reader outside the company — was not among them.
///
/// Scoped to a line that describes the reset **and** names the horizon, so the
/// register's own SW-3 requirement row, which legitimately cites ZPDV čl. 47 for
/// the čl. 32 objekti limb while correcting the general period, is not swept up
/// in it. Bound to [`crate::commands::backup::ROK_CUVANJA_PRAVNI_OSNOV`] like the
/// dialog is: three surfaces now print one string, and the next correction cannot
/// land on two of them.
#[test]
fn no_compliance_template_states_a_reset_warning_the_dialog_does_not_show() {
    /// The reset, in the words these templates use for it.
    const RESET: [&str; 2] = ["reset", "brisanj"];
    /// A dated correction states what the application **stopped** saying, and
    /// has to quote the withdrawn wording to be worth anything — `PROGRESS.md`
    /// stamps its own residuals the same way. It is a different speech act from
    /// a description of the live screen, and judging the two alike would leave a
    /// memo that may not show its work. The correction must therefore say so in
    /// terms; a sentence that merely mentions the past does not qualify.
    const ISPRAVKA: &str = "Ispravka";

    let osnov = crate::commands::backup::ROK_CUVANJA_PRAVNI_OSNOV;
    let mut pomena = 0usize;

    for (label, text) in prose_sources()
        .into_iter()
        .filter(|(label, _)| label.starts_with("docs/compliance/"))
    {
        for (line_no, line) in lines_with(&text, "10 godina") {
            let lowered = line.to_lowercase();
            if !RESET.iter().any(|needle| lowered.contains(needle)) || line.contains(ISPRAVKA) {
                continue;
            }
            pomena += 1;

            for clan in osnov.split(';').map(str::trim) {
                assert!(
                    line.contains(clan),
                    "{label}:{line_no} describes the retention warning the reset dialog shows and \
                     does not name „{clan}“. The dialog and the `compliance_log` tombstone both \
                     print `commands::backup::ROK_CUVANJA_PRAVNI_OSNOV` („{osnov}“); a template \
                     handed to a lawyer as a description of this app has to print the same one."
                );
            }
            assert!(
                !line.contains("ZPDV"),
                "{label}:{line_no} attributes the reset warning's retention horizon to ZPDV. \
                 §2 Q4: čl. 47 supplies no general period, `backup.rs` stopped citing it on \
                 31.07.2026 and the dialog stopped on 07.08.2026. Cite „{osnov}“."
            );

            for at in match_indices_ci(line, "arhiv") {
                let recenica = sentence_around(line, at);
                if let Some((odobrenje, unistenje)) =
                    claims_an_archive_destruction_approval(recenica)
                {
                    panic!(
                        "{label}:{line_no} says the reset warning states an archive approval duty \
                         — „{recenica}“ (stems „{odobrenje}“ + „{unistenje}“). ZAG čl. 16 st. 2 \
                         confines that approval to the public sector (§1 row 5), so no shop \
                         profile owes it; and the dialog has not shown any archive obligation \
                         since 07.08.2026, so the sentence also describes a screen that does not \
                         exist. Withdraw it."
                    );
                }
            }
        }
    }

    assert!(
        pomena > 0,
        "no template under docs/compliance/ describes the reset's retention warning any more. \
         The memo's §4 is the written answer to „why does this application delete trading data at \
         all“ and the warning is one of the three things that answer rests on. Re-state the \
         sentence rather than deleting it."
    );
}

/// The residual lists may not say the reset dialog still carries the two claims
/// it withdrew.
///
/// This is the defect this cycle exists to remove, pointed at the cycle's own
/// work: on 07.08.2026 a stale register led a survey agent to rank two long-fixed
/// bugs as pilot blockers. `PROGRESS.md`'s **Still open** lists said in the
/// present tense that `SettingsScreen.tsx` tells the operator pravna lica need
/// the arhiv's written odobrenje, and that the same screen attributes the 10-year
/// floor to ZPDV čl. 47. `41dc298` made both sentences false and did not touch
/// the lists.
///
/// Judged over the markdown **block**, per [`markdown_block_around`]: these
/// bullets wrap across four lines and the file reference, the denial and the
/// citation land on three different ones. The markers are the register's own
/// present-tense idiom rather than the bare word „still“, because a block may
/// perfectly well say a different thing is still open beside this one — req. 43's
/// archival export genuinely is.
#[test]
fn no_document_says_the_reset_dialog_still_carries_the_two_withdrawn_claims() {
    /// „This is on screen right now“, in the words these lists use.
    const STALE: [&str; 7] = [
        "still tells",
        "still says",
        "still carries",
        "still attributes",
        "still on screen",
        "carried into the ui",
        "nije preneto",
    ];

    let osnov = crate::commands::backup::ROK_CUVANJA_PRAVNI_OSNOV;
    let mut zapis = 0usize;

    for (doc, text) in [
        ("docs/SERBIAN-LAW-COMPLIANCE.md", REGISTER),
        ("docs/PROGRESS.md", PROGRESS),
    ] {
        for at in match_indices_ci(text, "settingsscreen.tsx") {
            let (line_no, blok) = markdown_block_around(text, at);
            let lowered = blok.to_lowercase();
            if osnov.split(';').all(|clan| blok.contains(clan.trim())) {
                zapis += 1;
            }

            let Some(marker) = STALE.into_iter().find(|marker| lowered.contains(marker)) else {
                continue;
            };
            assert!(
                !blok.contains("ZPDV"),
                "{doc}:{line_no} says („{marker}“) the reset dialog still attributes the retention \
                 floor to ZPDV — „{blok}“. It stopped on 07.08.2026 (`41dc298`); the dialog now \
                 prints `commands::backup::ROK_CUVANJA_PRAVNI_OSNOV` („{osnov}“), the string the \
                 tombstone carries. Stamp the bullet closed. A register that reports a fixed \
                 defect as live is the same defect as one that reports a live defect as fixed, \
                 and it costs the next reader the same day."
            );
            if lowered.contains("arhiv") {
                if let Some((odobrenje, unistenje)) = claims_an_archive_destruction_approval(&blok)
                {
                    panic!(
                        "{doc}:{line_no} says („{marker}“) the reset dialog still sends the shop \
                         for an archive approval — „{blok}“ (stems „{odobrenje}“ + \
                         „{unistenje}“). The sentence was deleted outright on 07.08.2026 \
                         (`41dc298`) and the neutral ZAG čl. 9 st. 1 custody note §3 req. 39 asks \
                         for stands in its place. Stamp the bullet closed rather than leaving a \
                         survey to rank it a pilot blocker."
                    );
                }
            }
        }
    }

    assert!(
        zapis > 0,
        "no line of docs/PROGRESS.md or docs/SERBIAN-LAW-COMPLIANCE.md records what the reset \
         dialog cites. It prints `commands::backup::ROK_CUVANJA_PRAVNI_OSNOV` („{osnov}“) after a \
         week of citing ZPDV čl. 47, and a register silent about a correction is how the same \
         miscitation comes back. Name the screen and both članovi in one block."
    );
}

/// No document may say the shared retention schema does not exist. v17 built it.
///
/// The sibling above catches the reset dialog's two withdrawn claims; this one
/// catches the third stale sentence in the very same bullets, which `2c415b8`
/// edited without touching. Both residual lists read *„No `retention_class`,
/// `retain_until`, `legal_hold` or upward-only extension exists anywhere in the
/// schema“* while `retention_policies` (v17, `db/migrations.rs:734`) declares all
/// four columns, `retention::extend_retain_until` is the upward-only extension
/// itself — it **refuses** a shortening rather than clamping it — and
/// `docs/SERBIAN-LAW-COMPLIANCE.md` row 120 says in terms that *„`retention.rs`
/// is the shared retention table SW11-SW15 §3 req. 42 mandates“*. The repository
/// asserted a control in one document and denied it in another, which is the
/// state that let a survey agent re-open closed work on 07.08.2026.
///
/// **The window is the column name's own neighbours, and it is that narrow on
/// purpose.** Register row 20's *„there is still no general upward-only
/// `retain_until` engine over trading data“* is TRUE, names `retain_until`, and
/// must stay statable; so must *„Genuinely unbuilt: the čl. 32 objekti register
/// (req 37)“* three clauses further along the same bullet. A sentence window is
/// no help — markdown prose ends sentences on a backtick or an asterisk, so
/// [`ends_a_sentence`]'s capital-letter lookahead runs the window across half a
/// section — and a markdown block is wider still. What is judged instead is the
/// **word immediately before the column name** and the clause immediately after
/// it: *„No `retention_class`“* is the shape that stood for a week, *„`retain_until`
/// does not exist“* is the same claim inverted, and *„no general upward-only
/// `retain_until` engine“* is neither, because the word before the name is
/// „upward-only“. A quoted denial is exempt for [`inside_a_serbian_quotation`]'s
/// reason: both bullets now quote the sentence they withdraw, and a dated
/// correction that cannot show what it corrected is worth nothing.
///
/// Bound to the crate three ways, so the prose cannot outlive the code: the four
/// needles are the fields of `retention::RetentionPolicy`, destructured here so
/// a rename is a compile error rather than a stale paragraph; `RecordClass::ALL`
/// and `extend_retain_until` are referenced as items. Presence is asserted before
/// wording, like every sibling — deleting the paragraph is not a way to pass.
#[test]
fn no_document_denies_the_retention_schema_v17_created() {
    /// The `retention_policies` columns v17 creates. `retention_class` is not
    /// one of them and never was — it is the name both residual lists used for
    /// the thing they said did not exist, so it is swept as written.
    const KOLONE: [&str; 5] = [
        "record_class",
        "retention_class",
        "retain_until",
        "legal_hold",
        "never_purge",
    ];

    /// The word that, standing immediately before a column name, denies it.
    /// Lower-case; the backtick and the markdown around the name are trimmed
    /// before the comparison.
    const PRE: [&str; 6] = ["no", "not", "nema", "nijedan", "bez", "without"];

    /// The same claim inverted — a denial that follows the name instead. Matched
    /// against the clause after it, so *„`retain_until` engine over trading data
    /// does not exist“* is reached and the next sentence is not.
    const POSLE: [&str; 4] = ["does not exist", "do not exist", "ne postoji", "ne postoje"];

    /// How far after the name `POSLE` is looked for. One clause, not one
    /// paragraph — see the doc comment on why a wider window cannot work here.
    const POSLE_PROZOR: usize = 48;

    // Renaming any of the four stops this file compiling instead of leaving the
    // needles above pointing at a column nothing declares.
    let crate::retention::RetentionPolicy {
        record_class: _,
        retain_until: _,
        legal_hold: _,
        never_purge: _,
    } = crate::retention::RetentionPolicy {
        record_class: crate::retention::RecordClass::WorktimeClassification,
        retain_until: None,
        legal_hold: false,
        never_purge: true,
    };
    let klase = crate::retention::RecordClass::ALL;
    let _upward_only = crate::retention::extend_retain_until;
    assert!(
        klase.len() >= 7,
        "the declared classes both residual lists enumerate are these: {:?}",
        klase.map(crate::retention::RecordClass::key)
    );

    let mut pomena = 0usize;
    for (doc, text) in [
        ("docs/SERBIAN-LAW-COMPLIANCE.md", REGISTER),
        ("docs/PROGRESS.md", PROGRESS),
    ] {
        for kolona in KOLONE {
            for at in match_indices_ci(text, kolona) {
                pomena += 1;
                if inside_a_serbian_quotation(text, at) {
                    continue;
                }

                // The word immediately before the name, with the opening
                // backtick and the markdown emphasis around it trimmed off.
                let pre = text[..at].trim_end_matches(['`', '*', '_']).trim_end();
                let rec_pre = pre
                    .rsplit(|ch: char| ch.is_whitespace())
                    .next()
                    .unwrap_or_default()
                    .trim_matches(|ch: char| !ch.is_alphanumeric())
                    .to_lowercase();

                // The clause after the name, cut at the first full stop so a
                // denial in the *next* sentence is not charged to this one.
                let posle_od = at + kolona.len();
                let kraj = text.len().min(posle_od + POSLE_PROZOR);
                let mut posle = &text[posle_od..kraj];
                while !text.is_char_boundary(posle_od + posle.len()) {
                    posle = &posle[..posle.len() - 1];
                }
                let posle = posle.split('.').next().unwrap_or_default().to_lowercase();

                let poricanje = if PRE.contains(&rec_pre.as_str()) {
                    format!("„{rec_pre}“ immediately before it")
                } else if let Some(marker) = POSLE.into_iter().find(|m| posle.contains(m)) {
                    format!("„{marker}“ immediately after it")
                } else {
                    continue;
                };

                let line = text[..at].lines().count();
                let (start, end) = sentence_span(text, at);
                panic!(
                    "{doc}:{line} denies that the retention schema has „{kolona}“ — {poricanje}, \
                     in „{}“. `retention_policies` shipped with **v17** on 01.08.2026 carrying \
                     `record_class`, `retain_until`, `legal_hold` and `never_purge`; \
                     `retention::extend_retain_until` is the upward-only extension and refuses a \
                     shortening rather than clamping it; `retention::RecordClass::ALL` declares \
                     {} classes. Re-state the sentence with what shipped and what within req. 36 \
                     is still owed — register row 20 puts it as „no general upward-only \
                     `retain_until` engine over trading data“ — rather than denying a retention \
                     control the crate runs.",
                    text[start..end].trim(),
                    klase.len()
                );
            }
        }

        assert!(
            pomena > 0,
            "{doc} names none of the `retention_policies` columns {KOLONE:?}. The shared \
             retention table SW11-SW15 §3 req. 42 mandates is `retention.rs`, and a register \
             silent about a control the shop is running is the same defect as one that denies \
             it. Re-state the paragraph rather than deleting it."
        );
        pomena = 0;
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

/// „OVO NIJE FISKALNI RAČUN“ has to be at **least twice** the line-item font,
/// and „at least twice“ is arithmetic — so this guard computes it.
///
/// Register row 6 stated the obligation as *„unerasable, ≥2× font“* and §3's
/// SW-1 build line as *„≥2× line-item font“*, and until 07.08.2026 the row
/// discharged both with a tick and the sentence *„the ≥2× ratio is asserted by
/// presence, never measured“*. That sentence was the defect: the ratio is not
/// unknowable, it is written in the class names, and one of the two surfaces was
/// **short** — the receipt detail panel carried `text-xl` (1.25rem) over a
/// `<Table>` root of `text-xs` (0.75rem), which is 1.6×, under a ✓ in a document
/// an inspector reads. „Nobody measured it“ and „it falls short“ are not the
/// same claim, and only the second one is true of a class name.
///
/// The reference element is the **line item**, per SW-1's own wording: the
/// stavka rows of the post-sale dialog, which carry no size class of their own
/// and inherit `DialogContent`'s `text-xs/relaxed`, and the stavka rows of the
/// receipt detail panel, which are a `<Table>` whose root is `text-xs`. The
/// dialog's Ukupno/Kusur summary lines are `text-sm` and its title `text-sm`;
/// row 6 states that divergence rather than hiding behind it, because the
/// Pravilnik's own reference element is not resolved in any verified-rules
/// document and the narrower reading is the one the software is measured on.
///
/// Bound from both sides, the way every sibling guard is: the arithmetic fails
/// if a class moves, and the register has to state the ratio the classes
/// actually produce, so raising the banner without re-stating the row fails too.
/// The two surfaces currently converge on one sentence, which is a property of
/// today's classes and not of the guard — the moment they diverge, the register
/// owes two.
#[test]
fn the_non_fiscal_banner_is_at_least_twice_the_line_item_font() {
    const REGISTER_SCREEN: &str = include_str!("../../src/app/register/RegisterScreen.tsx");
    const RECEIPTS_SCREEN: &str = include_str!("../../src/app/ReceiptsScreen.tsx");
    const UI_DIALOG: &str = include_str!("../../src/components/ui/dialog.tsx");
    const UI_TABLE: &str = include_str!("../../src/components/ui/table.tsx");
    /// The banner itself. Serbian Latin with diacritics, as it renders.
    const BANNER: &str = "OVO NIJE FISKALNI RAČUN";

    for (surface, banner_source, banner_file, stavke_source, stavke_anchor, stavke_what) in [
        (
            "the post-sale dialog",
            REGISTER_SCREEN,
            "src/app/register/RegisterScreen.tsx",
            UI_DIALOG,
            "data-slot=\"dialog-content\"",
            "`DialogContent` (src/components/ui/dialog.tsx), which the stavka rows inherit \
             because they carry no size class of their own",
        ),
        (
            "the receipt detail panel",
            RECEIPTS_SCREEN,
            "src/app/ReceiptsScreen.tsx",
            UI_TABLE,
            "data-slot=\"table\"",
            "the `<Table>` root (src/components/ui/table.tsx), which is what the stavka rows are",
        ),
    ] {
        let (banner_class, banner) =
            type_scale_before(banner_source, BANNER).unwrap_or_else(|| {
                panic!(
                    "{banner_file} no longer carries „{BANNER}“ inside an element with a Tailwind \
                 type-scale class. PVFR čl. 2 st. 8–10 is what puts the banner on {surface} at \
                 all; if the markup moved, re-point this guard in the same commit rather than \
                 letting the ratio go unmeasured again."
                )
            });
        let (stavka_class, stavka) = type_scale_after(stavke_source, stavke_anchor, 60)
            .unwrap_or_else(|| {
                panic!(
                    "the line-item font of {surface} can no longer be read: {stavke_what} no \
                     longer sets a Tailwind type-scale class within 60 tokens of \
                     `{stavke_anchor}`. The ≥2× ratio is measured against it, so re-point this \
                     guard rather than dropping the measurement."
                )
            });

        assert!(
            banner >= 2 * stavka,
            "{surface} renders „{BANNER}“ at `{banner_class}` ({banner} milli-rem) over a \
             line-item font of `{stavka_class}` ({stavka} milli-rem) — {ratio_tenths}.{ratio_rest}×, \
             short of the ≥2× PVFR čl. 2 st. 8–10 requires and §3's SW-1 line states as \
             „≥2× line-item font“. Raise the banner (or lower the line-item type) until the \
             ratio is met, and re-state register row 6 in the same commit: a ✓ over a \
             measurably unmet limb is a false claim in a compliance document. \
             `{banner_file}`, line-item font from {stavke_what}",
            ratio_tenths = banner * 10 / stavka / 10,
            ratio_rest = banner * 10 / stavka % 10,
        );

        let izmereno = format!(
            "{banner_class} over {stavka_class} = {}.{}×",
            banner * 10 / stavka / 10,
            banner * 10 / stavka % 10
        );
        assert!(
            REGISTER.contains(&izmereno),
            "docs/SERBIAN-LAW-COMPLIANCE.md row 6 must state the ratio {surface} actually renders \
             — „{izmereno}“ — and it does not. The figure is knowable from the class names, so \
             „asserted by presence, never measured“ is not an available answer, and a row that \
             states a ratio the code no longer produces is the same defect as one that denies a \
             control the code runs."
        );
    }
}

/// Two register rows deny that a body of data has a retention class, and
/// `retention::RecordClass::ALL` is the whole of the shared retention table —
/// so the day the list grows, the rows have to be re-stated in the same commit.
///
/// Row 18: *„the KEP has no `RecordClass` on the shared retention table“*.
/// Row 24: *„`retention::RecordClass` has no price-history or campaign
/// variant“*. Both are true today and both are the **denial** direction of this
/// module's rule — a register that says a control is absent withdraws the
/// reader's only pointer to it the moment it arrives. The list demonstrably
/// grows: `PopisDokumentacija` joined `ALL` within the last three weeks, and on
/// the day a price-history or a KEP class joins it, these two rows would
/// silently revert to denying a control the shop is running with nothing
/// failing. That is precisely the state rows 6, 17, 18, 20 and 24 were in on
/// 16.07.2026, and it is why the 07.08.2026 sweep happened.
///
/// Pinned two ways on purpose. The `match` is **exhaustive**, so a new variant
/// costs a compile error and a decision here rather than a reviewer's memory;
/// the key sweep then catches a variant whose arm was answered carelessly,
/// because `key()` is the string the class is stored under and cannot be
/// answered away. Presence is asserted before wording, like every sibling guard:
/// deleting the sentence is not a way to pass.
#[test]
fn no_register_row_denies_a_retention_class_the_crate_now_declares() {
    use crate::retention::RecordClass;

    /// The two denials, with the stem a class key would carry if it falsified
    /// one. `campaign` covers `campaigns` as well; `kep` is the book.
    const PORICANJA: [(&str, &str); 3] = [
        (
            "price_history",
            "row 24 — „`retention::RecordClass` has no price-history or campaign variant“",
        ),
        (
            "campaign",
            "row 24 — „`retention::RecordClass` has no price-history or campaign variant“",
        ),
        (
            "kep",
            "row 18 — „the KEP has no `RecordClass` on the shared retention table“",
        ),
    ];

    for (needle, row) in [
        (
            "`retention::RecordClass` has no price-history or campaign variant",
            "row 24",
        ),
        (
            "the KEP has no `RecordClass` on the shared retention table",
            "row 18",
        ),
    ] {
        assert!(
            REGISTER.contains(needle),
            "docs/SERBIAN-LAW-COMPLIANCE.md {row} no longer says „{needle}“. If the class shipped, \
             re-state the row with what it holds and what sweeps it; if it did not, restore the \
             sentence — silence about a five-year retention duty reads as though it is discharged."
        );
    }

    for class in RecordClass::ALL {
        // Exhaustive, so a new variant stops this file compiling and the author
        // decides whether either row has stopped being true. Every arm below is
        // a class whose data is neither the offered-price log, nor a campaign,
        // nor the KEP book: the three worktime/personnel classes and the čl. 47
        // register are ZEOR/ZZPL bodies, the credentials and the access log are
        // SW-13 class C, the cenovnik archive is the ZoT čl. 35 published file,
        // and the popis dokumentacija is the PoP čl. 14 stocktake papers.
        let falsifies_a_row = match class {
            RecordClass::WorktimeClassification
            | RecordClass::WorktimeOvertimeLog
            | RecordClass::WorktimeDraft
            | RecordClass::Personnel
            | RecordClass::Credentials
            | RecordClass::AccessLog
            | RecordClass::ProcessingRegister
            | RecordClass::CenovnikArchive
            | RecordClass::PopisDokumentacija => false,
        };
        assert!(
            !falsifies_a_row,
            "`retention::RecordClass::{class:?}` was answered as falsifying a register denial and \
             the register was not re-stated in the same commit."
        );

        let key = class.key();
        for (stem, row) in PORICANJA {
            assert!(
                !key.contains(stem),
                "`retention::RecordClass::{class:?}` is stored as „{key}“, which reads as a class \
                 for data docs/SERBIAN-LAW-COMPLIANCE.md {row} says has none. Re-state that row \
                 in this commit — name the class, its period and what sweeps it — rather than \
                 leaving the register denying a retention control the crate now declares."
            );
        }
    }
}

/// No document may record SW-14 req. 12 as **CLOSED** while one of its four
/// limbs is unbuilt.
///
/// SW14-VERIFIED-RULES.md §4 states req. 12 as one sentence with four limbs, and
/// the third is *„block night hours except the čl. 88 st. 2 exceptions“*. It is
/// not built: `worktime::ProtectionKind` carries no night variant for a minor,
/// `DayHours` carries no night bucket, and `check_protection`'s own doc comment
/// says so. `docs/PROGRESS.md` nevertheless carried the header
/// *„**Req. 12 — CLOSED 07.08.2026.**“* four lines above its own sentence
/// *„One limb of req. 12 is still open“*, in the very paragraph that states the
/// standard being broken — *„because a partial recorded as a tick is the defect
/// above pointed the other way“*.
///
/// The failure it enables is concrete and has already happened once in this
/// repository, one document over: a survey agent greps the status headers, reads
/// req. 12 as done, and retires the čl. 88 st. 2 night ban from the backlog. The
/// uppercase word is what is barred, because it is this file's own convention
/// for a status header — *„Req. 27 — CLOSED 02.08.2026“* is a true one. A leg
/// closing is still statable in the ordinary lower-case way, which is what the
/// corrected bullet says: *„PARTIAL. The čl. 87 weekly leg closed 07.08.2026“*.
///
/// Bound to `ProtectionKind` exhaustively, so the day the night leg ships this
/// file stops compiling and the author decides whether req. 12 may finally be
/// recorded closed — rather than the guard quietly outliving the gap it pins.
#[test]
fn no_document_records_sw_14_req_12_as_closed_while_the_night_leg_is_unbuilt() {
    use crate::worktime::ProtectionKind;

    /// The čl. 88 st. 2 limb, in the words both documents use for it.
    const NOCNI: &str = "čl. 88 st. 2";
    /// The status headers this repository writes for a finished requirement.
    const ZATVORENO: [&str; 2] = ["CLOSED", "✅"];

    fn blocks_a_night_entry(kind: ProtectionKind) -> bool {
        match kind {
            // čl. 88 st. 1 (prekovremeni, preraspodela), čl. 87 (dnevni,
            // nedeljni), čl. 90–91 (saglasnost, trudnoća) and the profile-date
            // finding. `TrudnocaNocniIPrekovremeni` names night work, but it is
            // the čl. 90 pregnancy leg over an operator-entered advisory bucket
            // — it decides nothing about a minor and does not discharge čl. 88
            // st. 2, which is a prohibition with its own three exceptions.
            ProtectionKind::MaloletanPrekovremeni
            | ProtectionKind::MaloletanPreraspodela
            | ProtectionKind::MaloletanDnevniLimit
            | ProtectionKind::MaloletanNedeljniLimit
            | ProtectionKind::SaglasnostRoditelja
            | ProtectionKind::TrudnocaNocniIPrekovremeni
            | ProtectionKind::NeispravanDatumUProfilu => false,
        }
    }

    let night_leg_built = [
        ProtectionKind::MaloletanPrekovremeni,
        ProtectionKind::MaloletanPreraspodela,
        ProtectionKind::MaloletanDnevniLimit,
        ProtectionKind::MaloletanNedeljniLimit,
        ProtectionKind::SaglasnostRoditelja,
        ProtectionKind::TrudnocaNocniIPrekovremeni,
        ProtectionKind::NeispravanDatumUProfilu,
    ]
    .into_iter()
    .any(blocks_a_night_entry);
    assert!(
        !night_leg_built,
        "a `ProtectionKind` now blocks a minor's night hours, so SW-14 req. 12 may finally be \
         recorded as closed — re-state `docs/PROGRESS.md` and register row 120 in this commit and \
         retire this guard, per this module's rule that a built leg loses its guard and gains a \
         sentence."
    );

    let mut imenovan = 0usize;
    for (label, text) in [
        ("docs/PROGRESS.md", PROGRESS),
        ("docs/SERBIAN-LAW-COMPLIANCE.md", REGISTER),
    ] {
        for at in match_indices_ci(text, "req. 12") {
            let (line, block) = markdown_block_around(text, at);
            // „req. 12“ is not a unique name: SW-12's own requirement 12 is the
            // cenovnik publication duty and its register row opens „✅ SHIPPED
            // 02.08.2026“, which is both true and none of this guard's
            // business. What identifies SW-14's req. 12 is its subject matter —
            // it is the čl. 87 / čl. 88 requirement — so a block that argues
            // about neither article is a different requirement 12 and is left
            // alone. Firing on it would be the failure `clause_around`'s doc
            // comment calls the worse one.
            if !block.contains("čl. 87") && !block.contains("čl. 88") {
                continue;
            }
            if block.contains(NOCNI) {
                imenovan += 1;
            }
            for marker in ZATVORENO {
                // A quoted header is a report of what a document used to say,
                // not a claim about the requirement — see
                // [`inside_a_serbian_quotation`]. The occurrence is judged
                // where it sits in the file, not in the collapsed block.
                let tvrdi = text
                    .match_indices(marker)
                    .filter(|(index, _)| !inside_a_serbian_quotation(text, *index))
                    .any(|(index, _)| {
                        let (_, susedni) = markdown_block_around(text, index);
                        susedni == block
                    });
                assert!(
                    !tvrdi,
                    "{label}:{line} records SW-14 req. 12 as „{marker}“. One of its four limbs is \
                     unbuilt — čl. 88 st. 2's night prohibition, which no `ProtectionKind` \
                     decides and which `DayHours` carries no bucket for. Record it the way reqs. \
                     21 and 26 are recorded, as a PARTIAL naming the open limb: a partial \
                     recorded as a tick is what withdraws the reader's only pointer to a real \
                     gap. Block: „{block}“"
                );
            }
        }
    }
    assert!(
        imenovan > 0,
        "no block of docs/PROGRESS.md or docs/SERBIAN-LAW-COMPLIANCE.md names SW-14 req. 12 \
         beside „{NOCNI}“ any more. Deleting the disclosure is not how this guard is satisfied — \
         the night prohibition is the limb that is still open, and the documents are where it is \
         recorded."
    );
}

/// No document may say this application prints nothing.
///
/// §3's SW-8 line read *„Printing/PDF stack (hard prerequisite for KEP
/// print-on-demand, popis lists, potvrda o prijemu; **app currently prints
/// nothing**)“* until 07.08.2026, in the same file and the same commit as a row
/// 18 that said the KEP mechanics shipped *„on the SW-8 printing stack“*. Five
/// renderers write documents and seven modules hand them to
/// `PrintService.openForPrint`; the denial was a 16.07.2026 baseline that
/// nobody swept when §2 was swept.
///
/// Bound to the renderers rather than to the prose alone, so renaming one stops
/// this file compiling rather than leaving the register's claim unbacked. The
/// needle is deliberately narrow — *„prints nothing“* and its two variants, not
/// *„no print path“* — because a **module** with no print path is a true and
/// useful thing to write, and `docs/PROGRESS.md` writes exactly that about the
/// worktime module. What is barred is the claim about the application.
///
/// **Only the unqualified claim is barred**, and the word after the needle is
/// what decides. §1 of the register opens with *„VantumPOS issues no receipts,
/// **prints nothing receipt-like**, never talks to a PFR“* — the fiscal posture
/// this whole document rests on, true and load-bearing. A guard that fires on it
/// is the failure [`clause_around`]'s doc comment calls the worse one, because
/// the way out of it is to phrase a true statement around the guard. A full
/// stop, a comma or a closing bracket leaves the denial standing about the
/// application, which is how SW-8 carried *„app currently prints nothing)“* for
/// three weeks after the printing stack shipped.
///
/// **`NARROWS` is a whitelist, and it was a blanket exemption until 08.08.2026.**
/// The rule read *„a following word narrows the denial“* and was implemented as
/// „any following word at all“, which exempts *„prints nothing today“*, *„prints
/// nothing whatsoever“*, *„prints nothing of its own“*, *„ne štampa ništa danas“*
/// — every intensifier except the two that had needles of their own. The stale
/// sentence this guard was written for, *„app currently prints nothing)“*, was
/// caught only because a `)` happened to follow it; at *„…prints nothing at
/// present)“* the guard was silent. Two words are whitelisted, because two words
/// legitimately narrow the claim in this repository today, and the failure
/// message names the word it found so that a third is a decision somebody takes
/// rather than a hole somebody falls through. The match is case-insensitive for
/// [`match_indices_ci`]'s own reason: *„Prints nothing.“* opening a sentence is
/// the ordinary way a denial comes back.
#[test]
fn no_document_says_this_application_prints_nothing() {
    /// The renderers behind `PrintService.openForPrint`. Referenced as function
    /// items so a rename is a compile error, not a stale sentence.
    fn renderers_exist() {
        let _popis = crate::popis_print::render_popisna_lista;
        let _kalkulacija = crate::kep_kalkulacija::render_kalkulacija_html;
        let _reklamacija = crate::reklamacije_docs::render_potvrda_html;
        let _kampanja = crate::campaign_evidence::render_evidence_html;
    }
    renderers_exist();

    /// The bare denial. Lower-case, per [`match_indices_ci`]'s contract.
    ///
    /// The intensifiers („at all“, „yet“) no longer need needles of their own:
    /// they are the word after the needle and `NARROWS` does not carry them.
    const PORICANJA: [&str; 3] = ["prints nothing", "print nothing", "ne štampa ništa"];

    /// The only words that narrow the denial to a class of document.
    ///
    /// „receipt-like“ is `docs/SERBIAN-LAW-COMPLIANCE.md` §1; „nalik“ is the
    /// memo's *„ne štampa ništa **nalik** fiskalnom računu“*. Both are true and
    /// both are load-bearing. Everything else — „today“, „yet“, „whatsoever“,
    /// „at all“, „of its own“, „so far“, „danas“ — leaves the claim standing
    /// about the application and is reported with the word that was found.
    const NARROWS: [&str; 2] = ["receipt-like", "nalik"];

    for (label, text) in prose_sources() {
        for poricanje in PORICANJA {
            for at in match_indices_ci(&text, poricanje) {
                let ostatak = &text[at + poricanje.len()..];
                // The word after the needle, stripped of the markdown and the
                // punctuation that wrap it — §1's own „receipt-like“ is written
                // **bold** where `docs/PROGRESS.md` quotes it. A needle followed
                // by punctuation or by the end of the text has no next word at
                // all, and the claim stands unqualified.
                let sledeca = if ostatak.starts_with(char::is_whitespace) {
                    ostatak.split_whitespace().next().unwrap_or_default()
                } else {
                    ""
                };
                let rec = sledeca
                    .trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '-')
                    .to_lowercase();
                if NARROWS.contains(&rec.as_str()) {
                    continue;
                }
                // A quoted denial is a dated record of what a document used to
                // say — see [`inside_a_serbian_quotation`]. `docs/PROGRESS.md`
                // is where this register's convention puts one.
                if inside_a_serbian_quotation(&text, at) {
                    continue;
                }
                let line = text[..at].lines().count();
                let nastavak = if rec.is_empty() {
                    "with no word after it".to_string()
                } else {
                    format!("followed by „{rec}“, which does not narrow it")
                };
                panic!(
                    "{label}:{line} says „{poricanje}“ of the application, {nastavak}. This app \
                     has printed since 19.07.2026: `popis_print`, `kep_close::render_book_html`, \
                     `kep_kalkulacija::render_kalkulacija_html`, `reklamacije_docs`, \
                     `campaign_evidence` and `cl47` render, and `PrintService.openForPrint` is \
                     the one hand-off seven modules use. A stale denial withdraws the reader's \
                     only pointer to a control the shop is running, which is the half of this \
                     rule that is easier to leave standing. Narrow the claim to the class of \
                     document it is really about — the words this guard accepts are \
                     {NARROWS:?} — or re-state it."
                );
            }
        }
    }
}
