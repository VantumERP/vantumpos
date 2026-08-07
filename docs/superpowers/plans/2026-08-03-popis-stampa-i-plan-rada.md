# SW-16 follow-on — Popisna lista print (reqs. 31/32) + plan rada & odluka (req. 35)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the two SW-16 gaps the final review named as the module's largest: there is no printed popisna lista at all (reqs. 31/32 — the biggest hole against PoP čl. 9 st. 3), and the plan rada / odluka o popisu are unvalidated free-text with no generation and no čl. 8 st. 2 approval (req. 35).

**Architecture:** A pure `popis_print.rs` renders from `PopisSessionView` and takes the phase as an explicit parameter, so the čl. 8 st. 5 blind count is a property of the renderer and not merely inherited from a view that happens to withhold. Two exports, one per statutory signature event. The plan rada and odluka become generated documents with a recorded approval, reusing the `campaigns::write_export` → `openForPrint` stack that reklamacije, KEP and campaigns already use.

**Tech Stack:** Rust (rusqlite, serde) + Tauri v2 commands; React 19 + TypeScript + shadcn/ui + vitest; bun.

## Global Constraints

- **Money is integer minor units (para); quantities milli-units.** Never floating point.
- **Timestamps RFC3339 passed as a `now: &str` param.** Never `datetime('now')` in decision code.
- **Serbian Latin diacritics** in every operator string; Serbian quotes open `„` and close `“`.
- **Migrations APPEND-ONLY.** Read the current count from `db/mod.rs` first — SW-16 left it at **20**, so this is **v21** and v21 only. Never edit v1–v20.
- **NO fine figure outside `legal.rs`**; a new notice goes into **both** guard lists with the asserted count bumped.
- **THE PHASE A PRINT MUST CARRY NO BOOK QUANTITY, NO RAZLIKA AND NO VALUE.** PoP čl. 8 st. 5 forbids releasing book data to the commission before the counted state is written and signed — a print is a release. The renderer must withhold structurally, not by trusting its input.
- **There is no obrazac for the popisna lista.** Never label our output a *propisani obrazac*. The izveštaj content, by contrast, IS prescribed (čl. 13 st. 1).
- **A document, a generated artefact or an operator string must never promise behaviour the code does not implement.** This project shipped that defect **six** times; `cl47.rs`'s sweep guard now catches one class of it. Anything this cycle adds must be true of the code.
- **Never boot the app.** Verify only via the gates.
- Commit trailer: `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`

**Gates:** the standard six.

**Governing law:** [REMAINING-SW-VERIFIED-RULES.md](../../REMAINING-SW-VERIFIED-RULES.md) §2c, §3 V5, §4 reqs. 31, 32, 35. Design: [2026-08-01-sw16-popis-design.md](../specs/2026-08-01-sw16-popis-design.md) §2, §5.

**Baseline:** cargo 902 passed, bun 505 passed, all six green, migration v20.

---

## File structure

**New Rust:** `src-tauri/src/popis_print.rs` (pure renderers: popisna lista, odluka, plan rada).
**Modified Rust:** `db/migrations.rs`, `db/mod.rs`, `commands/popis.rs`, `lib.rs`.
**Modified frontend:** `src/app/popis/PopisModule.tsx`, `src/app/popis/CountSheet.tsx`, `services/{types,ports,local-adapter,mock-adapter}.ts` (+ tests).

---

### Task 1: Migration v21 — the čl. 8 st. 2 approval

`popis_sessions` gains:
- `plan_rada_odobrio` TEXT — the name of the lice iz čl. 4 st. 2 who approved the plan (for a preduzetnik, the owner personally).
- `plan_rada_odobreno_at` TEXT — when.
- `odluka_doneta_at` TEXT — when the odluka o popisu i obrazovanju komisije was issued.

All nullable: an existing session has none, and a popis may legitimately be opened before the plan is approved. **A CHECK pairing the two plan columns** (`(plan_rada_odobrio IS NULL) = (plan_rada_odobreno_at IS NULL)`) so an approval can never be half-recorded — čl. 8 st. 2 requires the plan to be *approved*, and an approver with no date, or a date with no approver, is not an approval.

The posting write-lock must extend to these columns; check whether v20's trigger already covers the whole row or names columns, and match it.

- [x] **Step 1: Write the failing tests** — the three columns exist; the pairing CHECK refuses each half-state; the posting lock covers them; **a v20-seeded survival test applying `MIGRATIONS[..20]` to a raw Connection, seeding a session, `drop(conn)`, then `Db::new`, asserting the seeded row survives verbatim** — mirror commit `270796c`. A test that seeds after `Db::new` proves nothing.
- [x] **Steps 2–5:** run red, implement, run green, commit.

**Shipped.** v21 `popis_plan_rada_approval_and_odluka_date`, three `ALTER TABLE … ADD COLUMN`s, no trigger work: v20's `trg_popis_sessions_zakljucan` fires on the whole row (`WHEN OLD.status = 'posted'`), so the req. 41 lock already reached the new columns — asserted by test rather than assumed. **Deviation:** each column also carries a non-empty CHECK (`… IS NULL OR … <> ''`) beyond the pairing the plan specified, because a blank approver names nobody and a blank stamp dates nothing — the half-state wearing a value. Nothing is backfilled; an upgraded popis reads back unapproved.

---

### Task 2: `popis_print.rs` — the popisna lista renderer, pure

**Interfaces:** `PrintFaza { A, B }`, `render_popisna_lista(company: &CompanySettings, view: &PopisSessionView, faza: PrintFaza) -> String`.

The header block (req. 32): obveznik (naziv, adresa), PIB i matični broj, maloprodajni objekat, datum popisa, period, broj liste, vrsta popisa. One table per `lista_vrsta` present, each with its own heading naming its article. A signature block listing every commission member by name and uloga, with a ruled line each — and the date.

Columns by phase:

| | Faza A (čl. 8 st. 5) | Faza B (čl. 9 st. 3) |
|---|---|---|
| šifra, naziv, vrsta, jedinica mere | ✅ | ✅ |
| stvarna količina | ✅ | ✅ |
| bliži opis | ✅ | ✅ |
| iznos (gotovina, potraživanja) | ✅ | ✅ |
| **knjigovodstvena količina** | ❌ | ✅ |
| **razlika** | ❌ | ✅ |
| **cena / vrednosti** (ostale liste) | ❌ | ✅ |

- [x] **Step 1: Write the failing tests**

```rust
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

        assert!(!html.contains("9.999"), "a book quantity reached the čl. 8 st. 5 print: {html}");
        assert!(!html.contains("1.234"), "a razlika reached the čl. 8 st. 5 print: {html}");
        assert!(
            !html.to_lowercase().contains("knjigovodstven"),
            "the čl. 8 st. 5 print must not even carry the column heading: {html}"
        );
    }

    #[test]
    fn the_phase_b_print_carries_the_book_column_and_the_razlika() {
        let view = computed_view();
        let html = render_popisna_lista(&company(), &view, PrintFaza::B);
        assert!(html.to_lowercase().contains("knjigovodstven"), "{html}");
        assert!(html.to_lowercase().contains("razlika"), "{html}");
    }

    /// Req. 32 — the header identifies the obveznik and the outlet, because a
    /// signed sheet with no header identifies nothing.
    #[test]
    fn the_header_carries_the_obveznik_the_outlet_and_the_list_number() {
        let html = render_popisna_lista(&company(), &view_with_lines(), PrintFaza::A);
        for needle in ["PIB", "Matični broj", "Maloprodajni objekat", "Broj liste", "Datum popisa"] {
            assert!(html.contains(needle), "header missing „{needle}“: {html}");
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
        for article in ["čl. 10 st. 3", "čl. 10 st. 4", "čl. 11 st. 1", "čl. 12 st. 2", "čl. 2 st. 5"] {
            assert!(html.contains(article), "missing the section for {article}: {html}");
        }
    }
```

- [x] **Steps 2–5:** run red, implement, run green, commit.

**Shipped.** `render_popisna_lista(company, view, faza)` over `PopisSessionView`, with the čl. 8 st. 5 sheet built by `red_faza_a` — a function that never names `knjigovodstvena_kolicina_milli` or `razlika_milli`, asserted against its own source. The red state was two failures: `sekcija` sent **both** phases to `red_faza_b`, so the čl. 8 st. 5 sheet printed the book quantity and the razlika, and `zaglavlje_kolona` matched on the constant `PrintFaza::B` instead of on its `faza` parameter, so every phase got the Faza B headings. Each defect was re-introduced on its own afterwards to confirm which tests catch it: the row defect trips three, the heading defect four.

**Deviation 1 — `sekcija_nerazvrstanih` now uses its phase instead of ignoring it.** The plan says nothing about the unclassified bucket; the file arrived with `_faza`. The columns stay the Faza A set on both sheets, and that is now argued rather than asserted: `iznos_kolona` reads the meaning of the money column off the lista — apoen (čl. 11 st. 1), iznos (čl. 12 st. 2), otherwise the čl. 9 st. 1 t. 5 cena — so a stavka whose lista nobody could decode has no heading its figure could truthfully sit under. Over-withholding breaches nothing, but a čl. 9 st. 3 sheet that silently drops the obračun for one stavka reads identically to a stavka whose razlika was zero, so Faza B now carries a sentence naming the omission and what closes it.

**Deviation 1a — that sentence shipped wrong the first time and was corrected under review.** As first committed it read „Razvrstajte ih u odgovarajuću popisnu listu, pa ponovo odštampajte obračunate popisne liste“ — an instruction the engine refuses in every state that can produce a čl. 9 st. 3 sheet: `save_line` counts `lista_vrsta` as a moved identity field on `computed` (`PotpisanaStavka::pomerena_polja`) and refuses the write outright on `counted_signed`, `computed_signed` and `posted`, and there is no delete path at all. That is the house-rule-9 class verbatim, and the first commit's message („no new operator-facing promise“) was wrong about it. It now states the engine's own remedy in the engine's own words — „Posle potpisa stvarnog stanja potpisana stavka se više ne menja, pa ni popisna lista kojoj pripada (PoP čl. 8 st. 5, čl. 9 st. 1 t. 1) — ispravka se sprovodi novim popisom.“ — and the reasoning is now on `sekcija_nerazvrstanih`'s doc comment, which previously argued only for the column choice. Pinned by `the_unclassified_napomena_names_the_remedy_the_engine_actually_offers`.

**Deviation 1b — the čl. 6 single person was written out of the sheet and is now written back in.** The čl. 9 st. 3 napomena stopped at „…koje potpisuju članovi komisije za popis“, dropping the statute's own next limb — „односно једно лице из члана 6. овог правилника“ (REMAINING-SW-VERIFIED-RULES.md §3 V5) — and `potpisni_blok` hardcoded „Potpisi članova komisije za popis“ over a signer whose uloga line already read „jedno lice koje vrši popis (PoP čl. 6 st. 1)“. `jedno_lice` is a first-class `popis_commission.uloga` since v20 and is the pilot's own shape, so this was a preduzetnik reading, over an attributed citation, that the print-and-sign path belongs to a body he does not have. Fixed three ways: the čl. 9 st. 3 quotation carries its limb; the čl. 8 st. 5 napomena gains the čl. 6 st. 1–2 *shodna primena* as a sentence of its own with its own citation (čl. 8 st. 5 reaches that person only through that gateway, so it is quoted as written rather than paraphrased into one sentence); and the signature heading is derived by `potpisni_naslov` — „Potpis lica koje vrši popis (PoP čl. 6 st. 1)“ when the whole roster is `jedno_lice`, otherwise both limbs in the construction `cl47.rs` already registers the data subjects under. The empty-roster line carries both limbs too. Pinned by `a_single_person_popis_is_never_called_a_commission_on_its_own_sheet`, which sweeps every prose block of both phases and fails any block that names the komisija without the čl. 6 limb.

**Deviation 2 — three tests beyond the plan's six**, because the six can all pass while the sheet is wrong: `the_phase_a_document_carries_no_phase_b_heading_and_no_derived_figure` (all six Faza B headings and all six Faza B figures, absent from Faza A **and** present on Faza B from the same view — the absence half alone passes on a renderer that prints nothing, the presence half alone passes on the broken renderer this task started from; needles built with `kolicina_celija` / `iznos_celija` / `vrednost_minor`, since a raw `8_000` needle would sail past a document that prints „8“), `every_row_carries_exactly_as_many_cells_as_its_heading_promises` (both phases, across a lista whose money column is counted, one whose is not, and the unclassified bucket — the exact mismatch the two defects produced), and `an_unclassified_stavka_keeps_the_count_columns_on_the_computed_sheet_and_says_so`.

**Deviation 2a — a fourth test, added under review, because nothing pinned column ORDER.** Every document-level assertion in the module was a `contains`, a `!contains` or a count, and all three are invariant under a permutation of the cells: swapping the „Vrednost po popisu“ and „Vrednost po knjigama“ cells in `red_faza_b` printed a manjak as a višak on the fixture's own numbers with all 18 tests green, and so did swapping the stvarna količina with the čl. 11 st. 1 apoen in `red_faza_a`. `the_cells_of_a_row_sit_in_the_order_their_headings_promise` re-parses the heading row and the first body row of one section back out of the emitted markup, in emission order, and compares both against the čl. 9 st. 1 t. 1–6 sequence written out literally — one section per phase. Both permutations were re-introduced in isolation afterwards and each trips exactly this test and nothing else, which is the point: the renderer was already correct, the guard was missing.

**OPEN — req. 32's „editable default template“ limb did NOT ship.** Req. 32 reads „Ship the derived column set as an **editable default template**“ and design §2 opens with „An **editable default template**, not a statutory-form renderer“. What shipped is a fixed layout: `zaglavlje_kolona` and the two row renderers emit a hardcoded column set with no template, no setting and no operator control, and no later task in this cycle adds one. Everything else req. 32 asks for — header, the čl. 8 st. 4 / čl. 9 st. 1 t. 1–6 column set, the signature block, the no-obrazac claim — is built and tested. **Nothing operator-facing overstates this**: the footer says the layout is internal and that no obrazac exists, which is true either way. The limb is a product gap, not a false claim, and Task 6 must leave it in the „Still open“ list rather than closing req. 32 whole.

**The `#![allow(dead_code)]` cannot be narrowed yet — measured, not assumed.** Removing it reports every one of the fifteen items in the file as unused, `render_popisna_lista` included, because nothing outside the module calls in until Task 3's export commands land. Narrowing would mean fifteen attributes and would have to come straight back off. It stays module-level, as in `kep_close.rs` and `reklamacije_docs.rs`.

---

### Task 3: The two export commands

**Interfaces:** `popis_export_lista(state, id, faza) -> ExportedFile`, admin-gated, following `kep_export_book` exactly (`commands/kep.rs:303`): load the view, render, `campaigns::write_export(state, &file_name, &html, row_count)`.

**The phase is not the caller's choice — derive it.** A caller asking for Faza B on an unsigned session must be refused, not obeyed: the whole čl. 8 st. 5 property collapses if the frontend picks. Use the session's own `faza_a_potpisana` / status.

- [x] **Step 1: Write the failing tests** — a `counting` session exports the Faza A sheet; requesting the computed sheet before the čl. 8 st. 5 potpis is refused with a typed error; a `computed` session exports the Faza B sheet; the file name carries the session id and the phase; a cashier is refused.
- [x] **Steps 2–5.**

**Shipped.** `popis_export_lista(state, id, faza: Option<PrintFaza>)` in `commands/popis.rs`, registered in `lib.rs`, in `kep_export_book`'s shape: `require_admin`, open, `load_session`, load the company settings, render, `campaigns::write_export`. The file is `popisne-liste-{id}-faza-{a|b}.html` and `row_count` is the number of stavke. The phase is decided by `faza_stampe(status, faza_a_potpisana, trazena)`, a pure function beside the command, and a čl. 9 st. 3 sheet asked for without the čl. 8 st. 5 potpis is `popis_obracunate_liste_pre_potpisa`. **The guard runs before the render and therefore before the write**, so a refusal leaves nothing on disk — asserted on the path `write_export` itself resolves, not on the return value.

**Deviation 1 — one command, not „the two export commands“ the heading names.** The plan's own Interfaces line is already singular, and two commands would hand the caller the phase back by letting it pick which one to call — the exact collapse the plan's self-review found. One entry point, one derivation.

**Deviation 2 — the phase parameter is `Option<PrintFaza>`, and it is a check rather than a choice.** Omitted means „print what this popis has“; named, it is put through `faza_stampe`. This mirrors `popis_nivelacija_obuhvat`'s `Option<NivelacijaObuhvat>`, which is the module's existing shape for „a default the caller may narrow but not widen“. The derivation reads `book_quantities_released(status, faza_a_potpisana)` — the released two-limb predicate over the two stored facts — and **deliberately never reads `PopisSessionView::knjigovodstvo_dostupno`**, which is that same predicate already evaluated by `load_session`: a guard whose answer comes from a boolean somebody else computed is a guard by convention.

**Deviation 3 — an explicit Faza A is honoured in every state, including after the potpis.** Not a hedge: PoP čl. 2 st. 6 gives the owner of tuđa roba ten days to receive a primerak of the **signed** posebna popisna lista, and the signed document is the counted state, so the reprint has to stay available once the obračun opens. Over-withholding breaches nothing; refusing that reprint would refuse a duty the bylaw imposes. `draft` is not gated either — čl. 8 st. 4 has the liste given to the komisija *before* the count, so an empty čl. 8 st. 5 sheet is a document the bylaw asks for.

**Deviation 4 — `popis_print.rs`'s `#![allow(dead_code)]` came off, closing Task 2's OPEN note.** That note's condition („nothing outside the module calls in until Task 3's export commands land“) is now met, and it was re-measured rather than assumed: with the command registered, `cargo build` reports no unused item in the file at all, so the attribute went rather than being narrowed to fifteen. The module doc that justified it was rewritten to say what is now true. Task 4 must add `render_odluka` / `render_plan_rada` **together with** their commands, or the lib target will warn them dead under `-D warnings`.

**The red state and what catches what.** Red was a compile failure naming exactly the three things that did not exist — `popis_export_lista`, `faza_stampe`, `PrintFaza` in this scope. Afterwards three plausible defects were re-introduced one at a time: dropping the refusal (the guard obeys the caller) trips `the_computed_sheet_is_refused_before_the_cl_8_st_5_potpis`, `a_session_claiming_computed_without_the_potpis_still_gets_the_faza_a_sheet` and `the_print_phase_is_derived_from_both_limbs_of_cl_8_st_5`; **dropping the potpis limb** and deriving off `status` alone trips the last two; writing the document first and checking afterwards trips four, including both „a refused export must leave no document behind“ assertions.

**Four tests beyond the plan's five**, because the five can all pass while the property is gone: `a_session_claiming_computed_without_the_potpis_still_gets_the_faza_a_sheet` (a row forced into `computed` with zero signatures — the state čl. 8 st. 5 is *about*, and the only one that separates the two-limb predicate from a status check), `the_counted_state_sheet_stays_printable_after_the_obracun_has_opened` (Deviation 3's čl. 2 st. 6 reprint), `the_print_phase_is_derived_from_both_limbs_of_cl_8_st_5` (all six states × both limbs, with the expected column **written out literally** rather than recomputed from `book_quantities_released` — a table that re-derived the rule from the function under test would agree with any rule at all), and `the_print_phase_wire_form_is_the_frontend_contract`. The čl. 8 st. 5 test is also stronger than the plan asked: it walks a popis to `computed` so the `popis_lines` row really holds the perpetual 61.237, forces the session back to `counting`, asserts the **database still holds it**, and only then asserts the bytes on disk carry neither the quantity, nor the razlika, nor the heading, nor the word.

**Correction to Task 5's premise — the three named strings do NOT go false here.** `retention.rs:430`, `cl47.rs:335` and `commands/popis.rs:1899` all scope their „program ga ne štampa i ne izvozi“ to the **izveštaj**, and nothing in this cycle exports the izveštaj, so all three stay true and must not be flipped. The string that actually went false is `src/app/popis/PopisModule.tsx` — „Odštampajte popisne liste … program ih ne štampa i ne izvozi“ — together with the comment above it. **It has now been withdrawn here** (see the review pass below) rather than deferred to Task 5: a false sentence standing for the length of a task is a false sentence shipped if the task never runs. Task 6 must not read the other three as needing a change.

**Review pass — three findings closed, all with tests.**

*The export tests were not hermetic.* `db::test_database_path` puts every test database directly in `std::env::temp_dir()`, so `campaigns::write_export` resolved **one** `$TMPDIR/exports/` for the whole test binary; every fresh database allocates popis id 1, so five of these tests wrote, read back and deleted the same two absolute paths. Under a plain `cargo test` (no `--test-threads=1`, which is what the brief tells implementers to run for a targeted subset) they deleted each other's artefacts, and worse, `!putanja.exists()` — the assertion that pins the guard running *before* the write — could have gone green because another test removed the document. Fixed locally rather than repo-wide: `commands::popis::tests::with_app` now puts the database one level down, in a folder of its own, the way `cenovnik::with_publish_folder` already does it, and tears the folder down with `remove_dir_all`. That makes `exports/` private per test, so the eight by-hand `remove_file` cleanups — including the two pre-act ones that caused the contention — are gone with it, along with the WAL debris. `every_popis_test_exports_into_a_directory_of_its_own` pins it: it asserts two `with_app` runs resolve different export paths, that neither `exports/` is the temp root's, and that the folder does not survive the teardown. Red before the fix with both paths printed identical. **`db::test_database_path` itself was left alone** — changing it would move ~60 teardowns across the crate in a commit that is meant to close a popis finding, and no other module's export names collide (each of `kep-knjiga-*`, `kalkulacija-{id}`, `obavestenje-reklamacije`, `evidencija-radnji-obrade-cl47` has exactly one writer). It remains the better global fix and is recorded here as the open option, not as a closed one.

*Nothing pinned the obveznik.* The command loads the shop's `CompanySettings` and req. 32 makes the obveznik, PIB and matični broj part of the header, but every test database here is bare, so `load_company_settings` returned `CompanySettings::default()` in all of them and swapping the lookup for that default produced byte-identical HTML across the whole crate — measured, not assumed: with the lookup replaced, exactly zero tests failed before and exactly one after. `the_exported_sheet_carries_the_obveznik_the_shop_registered` seeds a distinctive obveznik through `save_company_settings` and asserts all three on the file, plus the absence of the default name so the fallback cannot pass for the lookup.

*The frontend string was withdrawn.* `PopisModule.tsx`'s potpis paragraph no longer claims the program cannot print or export the liste, and it does not claim the reverse either — this panel still has no button, so announcing an export would be the same defect facing the other way. It states the operator's duty and stops. `does not deny the print and the export the backend now has` in `PopisModule.test.tsx` asserts, on both phases, that the paragraph carries neither „ne štampa“ nor „ne izvozi“; the izveštaj's own sentence is untouched and still pinned by `IzvestajPanel.test.tsx`.

---

### Task 4: Req. 35 — generate the odluka and the plan rada, and record the approval

**Interfaces:** `render_odluka(company, view) -> String`, `render_plan_rada(company, view) -> String` in `popis_print.rs`; `popis_export_odluka`, `popis_export_plan_rada`, and `popis_odobri_plan(state, id, odobrio, now)`.

The **odluka o popisu i obrazovanju komisije** carries: obveznik, datum donošenja, the popis it orders (vrsta, objekat, datum, period), and the commission by name and uloga. The **plan rada** (čl. 8 st. 1–2) carries the same header plus the schedule and the assignment, from `plan_rada_json` where the shop has supplied it.

`popis_odobri_plan` records `plan_rada_odobrio` + `plan_rada_odobreno_at` **in one write** (the v21 CHECK enforces the pairing anyway), refuses a blank approver, refuses a posted popis, and writes an audit line.

**Do not invent the approver.** For a preduzetnik the lice iz čl. 4 st. 2 is the owner personally (PoP čl. 4 st. 2 → ZoRač čl. 43 st. 3), so **default the field to the company name and let it be edited** — never auto-approve, because an approval nobody performed is exactly the false-record class this project keeps catching.

- [x] **Step 1: Write the failing tests** — the odluka names every commission member and the popis it orders; the plan rada renders the shop's `plan_rada_json` when present and says plainly when it is absent rather than printing an empty schedule; approving records both columns; approving with a blank name is refused; approving a posted popis is refused; nothing auto-approves.
- [x] **Steps 2–5.**

**Shipped.** `render_odluka(company, view)` and `render_plan_rada(company, view)` in `popis_print.rs`, `popis_export_odluka` / `popis_export_plan_rada` / `popis_odobri_plan` in `commands/popis.rs` (registered in `lib.rs`), and `odobri_plan_rada(state, id, odobrio, now)` as the inner verb the command wraps with `clock::utc_now`. The header block the popisne liste already had is now `zaglavlje_redovi` + `zaglavlje_tabela`, shared by all four documents so they identify the same popis in the same words, and `potpis_clana` is expressed through a new `potpis_linija(ime: Option<&str>, uloga)` — the čl. 4 st. 2 line has no name to print. `PopisSessionView` gained `plan_rada_odobrio`, `plan_rada_odobreno_at` and `odluka_doneta_at`, read straight off the v21 columns. The red state was a compile failure naming exactly those three fields and the five new functions.

**Nothing auto-approves, and the „default to the company name“ limb is deliberately *not* backend behaviour.** `odobri_plan_rada` refuses a blank or whitespace-only approver by name (`popis_plan_rada_bez_odobravaoca`) rather than substituting `CompanySettings::shop_name`: the plan's instruction is about the **field's default in the UI**, and a server-side substitution would be an auto-approval wearing a default's clothes — the shop's registered name is not the person, and it is on file precisely so it cannot be mistaken for one. Task 5 owns the pre-filled, editable field. The refusal is the command's own and does not lean on v21's CHECK, which would reach the operator as a raw constraint failure.

**`odluka_doneta_at` is written by nothing, and that is recorded rather than papered over.** No verb sets it this cycle and none should have: exporting a draft odluka is not *donošenje odluke*, so a command that stamped the column while rendering would record a čl. 4 st. 2 act nobody performed. The generated odluka therefore prints „nije evidentiran u aplikaciji — upišite ga na odštampanom primerku“ in its own header cell, and prints the stored value the moment some later verb records one. **Residual for Task 6's „Still open“ list:** req. 35's odluka has a date column and no way to fill it in.

**Deviation 1 — a new `AuditObjectType::PopisSession`.** The čl. 48 line had nowhere truthful to sit: the vocabulary's ten codes are `sale`, `employee`, … `backup`, and reusing one would have misdescribed the object. Added with its Serbian label („Popis imovine i obaveza“), the `ALL` array at 11, the vocabulary list in `audit.rs` and the 36→37 count in `commands/audit.rs`. v18 pins no list for `object_type` — its CHECK is a shape — so this is a code change and not a migration, as that enum's own doc comment says. **The action is `unos` the first time and `menjanje` on a re-record**, because two `unos` lines would report two approvals where there was one and a correction. `append_audit_event` shares the approval's transaction, the way `retention::extend_policy` does it.

**Deviation 2 — the čl. 47 register was extended, because the app now stores a personal datum it did not name.** `cl47.rs`'s `popis_imovine` template listed the komisija, the single person and the potpisnici; `plan_rada_odobrio` adds the lice iz čl. 4 st. 2 and the time of its recording. Both clauses are pure data-category statements with no „program“ subject, so the sweep guard `the_generated_register_promises_no_output_the_program_cannot_produce` is untouched and needed no amendment — **the guard was not weakened and no output claim was added anywhere.** The three „program ga ne štampa i ne izvozi“ sentences still scope to the izveštaj and are still true.

**Deviation 3 — `src/services/types.ts` (+ the mock adapter and two fixtures) took the three new view fields now, not in Task 5.** `PopisSessionView` in TS documents itself as the mirror of the Rust struct, and `popis_get` already returns the fields; leaving the type stale for the length of a task is the same class of defect as a stale document. No service method, port or UI shipped with it — the mock's approval columns are always `null`, which is true of a double with no approval verb.

**Deviation 4 — the plan rada renders text that is not JSON.** v20 named the column `plan_rada_json`, but nothing validates it and `PopisModule.tsx` writes a plain `<Input>` into it, so most shops will type a sentence. `raspored_html` parses, lays out an object as a table and an array as a list, and on a parse failure prints the text verbatim — a document that silently dropped what it could not parse would report that the commission had no plan when it had one. An empty object, an empty array, a null and a blank string all resolve to „no schedule“ and get the čl. 8 st. 1 sentence instead of an empty table. `serde_json` here has no `preserve_order`, so an object's keys print in key order; an array keeps its order, which is the shape a sequence of steps belongs in anyway.

**Deviation 5 — the čl. 6 single person, again, before it could be shipped wrong.** An „odluka o obrazovanju komisije“ over one appointee is Deviation 1b of Task 2 wearing a different document, so the title, the operative sentence and the roster heading are all derived from the roster (`jedno_lice_popis`, extracted out of `potpisni_naslov`), and the čl. 8 st. 1 quotation on the plan rada carries the čl. 6 st. 1–2 *shodna primena* in the same block. Pinned by `a_single_person_popis_is_never_called_a_commission_on_the_generated_documents`, which reuses Task 2's `prozni_blokovi` sweep over both new documents.

**Deviation 6 — the odluka carries the čl. 5 st. 1 objection.** The odluka *is* the appointment, so a roster member flagged `rukuje_imovinom` is named on it with the exclusion and with §6 R-5's own uncertainty about whether it reaches the čl. 6 st. 1 single person. It warns and never blocks, in the wording `komisija_upozorenja` already uses.

**Twelve tests beyond the plan's six, and each defect was re-introduced to see what catches it.** Renderer: `the_odluka_names_the_popis_it_orders_and_every_member_of_the_commission`, `the_plan_rada_renders_the_schedule_the_shop_supplied`, `the_plan_rada_prints_a_schedule_that_is_not_json_as_the_shop_typed_it`, `the_plan_rada_says_plainly_when_the_shop_supplied_no_schedule`, `both_generated_documents_carry_the_recorded_cl_8_st_2_approval`, `neither_generated_document_claims_an_approval_nobody_recorded`, `half_an_approval_is_printed_as_no_approval`, `the_odluka_leaves_the_date_of_issue_to_the_paper_until_one_is_recorded`, `the_odluka_carries_the_cl_5_st_1_objection_without_refusing_to_print`, `a_single_person_popis_is_never_called_a_commission_on_the_generated_documents`, `neither_generated_document_calls_itself_a_prescribed_form`, `the_generated_documents_escape_dynamic_values`. Commands: `the_exported_odluka_names_the_popis_it_orders_and_the_people_it_appoints`, `the_exported_plan_rada_carries_the_schedule_the_shop_supplied`, `a_popis_opened_without_a_plan_rada_exports_a_document_that_says_so`, `approving_the_plan_records_the_approver_and_the_time_in_one_write`, `a_blank_approver_is_refused_and_records_nothing`, `approving_the_plan_of_a_posted_popis_is_refused`, `nothing_in_the_popis_flow_approves_the_plan_by_itself`, `the_approval_writes_one_audit_line_and_no_personal_name_reaches_it`, `re_recording_an_approval_is_logged_as_a_change_and_not_as_a_new_record`, `a_cashier_can_neither_export_nor_approve_the_plan_rada`, `approving_a_popis_that_does_not_exist_is_refused_as_not_found`, `the_generated_document_file_names_carry_the_session`. Six defects were re-introduced one at a time afterwards: dropping the blank-name refusal trips exactly `a_blank_approver_is_refused_and_records_nothing` (and proves the point — v21's CHECK then refuses the write as `database_error`, which is not a sentence anybody can act on); dropping `posting_lock` trips exactly the posted test; putting the approver's **name** in `object_id` trips three, because `audit::reject_forbidden_content` refuses the whole write; reading the approval off `plan_rada_odobrio` alone trips `half_an_approval_is_printed_as_no_approval`; printing an empty schedule table trips the absent-schedule test; hardcoding the komisija title trips the single-person sweep.

**One residual, stated plainly.** The čl. 48 line covers the approval and nothing else in this module: opening a popis, recording the komisija and both potpisi still write no `audit_events` row, though each of them stores names too. That is pre-existing and out of this task's scope, but it is now visibly lopsided and should be decided deliberately rather than left to drift.

**Review pass — two findings closed, both with tests, both in `popis_print.rs`.**

*An approval stood over a schedule the application does not hold, unqualified.* `odobri_plan_rada` guards the approver and the write-lock and never looks at `plan_rada_json`; `render_plan_rada`'s schedule section and `odobrenje_sekcija` branch on independent inputs, so they composed — the ordinary path (open a popis leaving the plan rada field blank, then approve; there is no UPDATE for that column anywhere in the crate) produced one page carrying „Plan rada nije unet u aplikaciju“ and „Plan rada je odobren (PoP čl. 8 st. 2)“ with nothing between them. **The write is not refused**, because čl. 8 st. 1 makes the plan the komisija's artefact and not this application's — a shop that keeps its schedule on paper has broken nothing and its čl. 8 st. 2 record is real. The document is qualified instead: `odobrenje_sekcija` now asks `plan_rada_u_aplikaciji`, which routes through `raspored_html` — the same function that lays the schedule out, so the two sections cannot disagree about whether there is one — and appends „U aplikaciji je evidentirano odobrenje, a ne i sadržina plana rada — raspored i zaduženja na koje se odobrenje odnosi nisu uneti u aplikaciju (PoP čl. 8 st. 1).“ The sentence deliberately says only what the app holds: an earlier draft added that the raspored „se nalazi na primerku van aplikacije“, which would have vouched for a paper nobody here has seen. Pinned by `an_approval_over_a_schedule_the_application_does_not_hold_says_so_on_both_documents` (four empty shapes × both documents, read out of the `<section class="odobrenje">` block itself so a sentence elsewhere on the page cannot satisfy it, **plus the converse** — the qualification must not print over a schedule the app does hold) and by `an_approval_over_a_plan_kept_on_paper_is_qualified_on_the_exported_documents` on the exported bytes.

*An unrecorded roster was declared a komisija on both documents.* `jedno_lice_popis` was a two-state predicate (`!is_empty() && all(jedno_lice)`), so an empty roster fell to the komisija branch in four places at once — the odluka's title, its Predmet sentence, the plan rada's title and `sastav_sekcija`'s heading — and dropped the čl. 6 st. 1 limb from every one of them. That state is not an edge: `open_popis` imposes no non-empty roster, `OpenPopisRequest.komisija` is `#[serde(default)]`, and the odluka **is** the appointment, so printing it before anybody is appointed is the primary use case. Replaced by `Sastav { JednoLice, Komisija, Neodredjen }` and `sastav_popisa`, with all four call sites plus `potpisni_naslov` reading it. **A mixed roster now resolves to `Neodredjen` too** (it was `Komisija`): a roster carrying both ulogas says as little as an empty one, and `potpisni_naslov` already gave it both limbs, so the sheet and the pre-count pair now agree. Pinned by `a_popis_whose_roster_is_not_recorded_is_declared_neither_a_komisija_nor_a_single_person` (empty and mixed × both documents).

*The čl. 6 sweep itself was case-blind and is now the shared helper.* Both existing sweeps compared `blok.contains("komisij")` against untouched markup, so „ODLUKA O POPISU I OBRAZOVANJU KOMISIJE ZA POPIS“ read as no mention of a komisija at all — the guard skipped the one line of the document nobody can miss. Extracted as `komisija_nikad_bez_cl_6_limba` and case-folded; `a_single_person_popis_is_never_called_a_commission_on_its_own_sheet` and `…_on_the_generated_documents` now call it. **Measured, not assumed:** with the empty-roster fix in place and the fold removed, re-introducing the title defect alone left all 32 renderer tests green; with the fold, it fails on the title.

Five defects re-introduced one at a time: suppressing the qualification trips exactly the renderer test and exactly the command test; an empty roster reading as `Komisija` trips the roster test; a mixed roster reading as `Komisija` trips the same one; the title-only defect trips it only through the case-fold. No existing test was weakened — the two sweeps got strictly stronger. Gates after the pass: cargo **973 passed, 0 failed** (970 + 3), bun 513 passed / 34 files, build ✓, clippy clean, fmt clean, `git diff --check` clean. No migration; head stays v22.

---

### Task 5: Frontend — the print buttons and the plan/odluka panel

**Files:** `src/app/popis/PopisModule.tsx`, `src/app/popis/CountSheet.tsx`, `services/*`.

Per phase, a „Štampaj popisne liste“ action calling the export then `openForPrint`. The `counting` step's copy must say the sheet is the čl. 8 st. 5 one and carries no book data; the `computed` step's must say čl. 9 st. 3. An „Odluka o popisu“ and „Plan rada“ pair with their own print actions, and an approval control that captures the approver's name.

**~~Withdraw the disclaimers that are no longer true.~~ Superseded — read Task 3's review pass before touching a disclaimer.** This paragraph named `retention.rs`, `cl47.rs` and `commands/popis.rs` as saying the program neither prints nor exports the popis documents. That premise is wrong: all three scope the sentence to the **izveštaj**, which nothing in this cycle exports, so all three are still true and **must not be flipped**. The one string that did go false was `PopisModule.tsx`'s potpis paragraph, and Task 3 withdrew it. What is left for this task is the positive half: when the button lands, the copy beside it may describe the export — and until it lands, no string may.

- [x] **Step 1: Write the failing tests**

```tsx
  it("offers the čl. 8 st. 5 sheet during the count and says it carries no book data", async () => {
    render(<PopisModule {...countingSession} />);
    const button = screen.getByRole("button", { name: /štampaj popisne liste/i });
    expect(button).toBeInTheDocument();
    expect(screen.getByText(/bez knjigovodstvenih količina/i)).toBeInTheDocument();
  });

  it("names čl. 9 st. 3 for the computed sheet", async () => { /* … */ });

  it("never auto-approves the plan rada", async () => {
    render(<PopisModule {...draftSession} />);
    expect(screen.getByText(/plan rada nije odobren/i)).toBeInTheDocument();
  });
```

- [x] **Steps 2–5.**

**Shipped.** `PopisService` gained `exportLista(id, faza)`, `exportOdluka(id)`, `exportPlanRada(id)` and `odobriPlan(id, odobrio)` in `services/{types,ports,local-adapter,mock-adapter}.ts`, and `PopisModule.tsx` gained `DokumentiPanel` — two `role="group"` blocks („Popisne liste za štampu“, „Odluka o popisu i plan rada“) carrying four print actions over `runPrint`, in the `KepModule`/`ReklamacijeModule` export-then-`openForPrint` shape with the same three toasts. The čl. 8 st. 2 approval is a pre-filled, editable field plus an explicit „Evidentiraj odobrenje plana rada“; the approval verb bubbles to `PopisModule` beside `korak`, so a refusal reaches the module's own Alert rather than a second error channel. Red was 17 failures (13 UI + 4 double), no existing test disturbed.

**Deviation 1 — `CountSheet.tsx` was NOT touched, and could not be.** The plan names it as a file. `CountSheet`'s own req. 29 guard, `renders no book-quantity and no difference column during counting`, sweeps the entire component for `/knjigovodstven/i` **and** `/razlika/i` — and the plan's own required copy for the counting step is „bez knjigovodstvenih količina“. Putting that sentence inside `CountSheet` trips a guard that exists to catch a čl. 8 st. 5 leak, and loosening it to fit copy is the one thing house rule 13 forbids. So the documents block sits in `PopisDetail`, immediately above the sheet. The adjacent `PopisModule` assertion (`queryByText(/knjigovodstvena količina/i)` null during counting) survives on inflection alone — the copy says *„knjigovodstvenih količina“* — which is thin, so the new copy deliberately never uses the nominative singular anywhere.

**Deviation 2 — a second print action, „Štampaj potpisane liste stvarnog stanja“, offered after the potpis.** Task 3's Deviation 3 kept an explicit Faza A printable in every state for PoP čl. 2 st. 6 — ten days for a primerak of the **signed** posebna popisna lista — and without a button the module states that rok (it already renders `konsignacijaRok`) while giving the shop no way to meet it. `„a“` can only narrow, which is why naming it is safe where naming `„b“` never is. Pinned by `keeps the signed čl. 8 st. 5 sheet printable once the obračun has opened`.

**Deviation 3 — the module is asserted never to send `„b“`, in any state.** `never asks the backend for the čl. 9 st. 3 sheet` walks a popis through all five states, clicks every print button in each, and fails if any call carries `„b“`. The backend guard is the real one; this is the frontend half of the same property, and it is the test the „screen picks the phase“ defect trips.

**Deviation 4 — `mock-adapter.popis.test.ts` is new.** The brief required the double to model refusals rather than always succeed. It reproduces `faza_stampe` (a Faza B request before the potpis refused by name, not silently downgraded; an explicit `„a“` honoured in every state), the blank-approver refusal **in the backend's own order** — before the popis is looked up — and the čl. 14 st. 3 lock, with the converse asserted each time so a double that refuses everything fails too.

**Deviation 5 — `local-adapter.test.ts`'s popis port-shape assertion was widened in the open.** It is an exhaustive `Object.keys(...).sort()` list and the surface genuinely gained four verbs; the four new command names and their argument shapes were added as `toHaveBeenNthCalledWith` assertions 14–18 in the same edit, so the wire contract is pinned rather than merely allowed. Not a weakening: the list is still exhaustive and `update`/`delete`/`reopen` are still absent.

**Five defects re-introduced one at a time, each trips exactly its own test.** The screen picking the phase (`faza = dostupno ? "b" : "a"`) → `never asks the backend for the čl. 9 st. 3 sheet`. One interchangeable sentence for both phases → `names čl. 9 st. 3 on the computed sheet and čl. 8 st. 5 on the counted one`. Approving as soon as the obveznik loads → three tests. The double dropping its phase guard → `refuses the čl. 9 st. 3 sheet before the potpis`. The double substituting `companySettings.shopName` for a blank approver → the double's own test **and** the UI refusal test.

**The three disclaimers — the conclusion, stated precisely.** All three scope their denial to the **izveštaj**, and after Tasks 3–4 nothing writes an izveštaj to a file, so **none of the three was false**. All three were **incomplete**: each sat in, or described, a module that had gained three exports, and a bare *„program ga ne štampa i ne izvozi“* read beside a printing module says „this module prints nothing“ — the same defect facing the other way, and the direction nothing in the crate was watching. So each keeps its denial verbatim and gains one sentence naming what the program *does* write.

- `retention.rs:430` (`PopisDokumentacija::napomena`). **Before:** „…Izveštaj o popisu se ne čuva u aplikaciji: sastavlja se na zahtev i prikazuje na ekranu, a program ga ne štampa i ne izvozi — štampani i potpisani primerak sastavlja i čuva sam obveznik.“ **After:** unchanged, preceded by „Popisne liste, odluku o popisu i plan rada program sastavlja i izvozi u datoteku za štampu — potpisan primerak sastavlja i čuva sam obveznik, a izvezena datoteka nije potpisana isprava.“
- `cl47.rs:337` (the `popis_imovine` `rok_osnov`). Same before/after, same inserted sentence. Its `mere` clause (st. 1 t. 7) also gained „Odluka o popisu, plan rada i popisne liste sadrže imena, pa izvoz tih dokumenata iznosi imena iz baze u datoteku na disku, koja više nije zaštićena prijavom u program nego pristupom samom uređaju.“ — the export is the moment the komisija's names leave the database, and the register had no account of it.
- `commands/popis.rs:1899` (the izveštaj upozorenje). **Before:** „Ovaj izveštaj se sastavlja u trenutku kada se zatraži… Program ga ne štampa i ne izvozi — štampani primerak sastavite sami i čuvajte ga uz popisne liste.“ **After:** unchanged, plus „Popisne liste, odluku o popisu i plan rada program izvozi u datoteku za štampu; izveštaj nije među njima.“ The mock adapter's paraphrase and `IzvestajPanel.test.tsx`'s fixture were moved with it.

**The `cl47.rs` sweep guard was amended deliberately, and got stronger rather than looser.** The rule was „an output claim must be negated in the same sentence“, which left the register only false sentences and silence once the program genuinely gained an output. It is now „negate it, **or** name the documents you are claiming it for“ — and the nameable list is not a literal: `popis_export_lista`, `popis_export_odluka` and `popis_export_plan_rada` are bound as typed function pointers inside the test, so renaming one stops the test compiling (**measured**: renaming `popis_export_odluka` gives `E0425`, not a green test). An affirmative sentence must name one of the three **and** none of `BEZ_IZLAZA` (`izveštaj`). Three untrue shapes were planted into the register one at a time and each fails: „Izveštaj o popisu program štampa na zahtev“, the half-true „Program štampa popisne liste i izveštaj o popisu“, and „Program izvozi svu dokumentaciju o popisu“ which names nothing. Stripping the document names out of the real sentence fails both the sweep and the new pinned test.

**And the sweep's scope check was case-blind — found while proving the amendment.** It read the sentence as written, so „**P**rogram štampa…“ carried no lowercase „program“ and was skipped whole: two of the three probes above passed until the fold went in. That is `popis_print`'s čl. 6 sweep defect verbatim, in the guard whose job is to catch this class. Now folded, and the two probes fail as they should.

**Both denials are now pinned from the other side too**, because a sweep fires on a claim and is satisfied by silence: `cl47::tests::the_popis_entry_names_the_documents_the_program_writes_and_the_one_it_does_not` (new) and the strengthened `retention::tests::the_popis_retention_note_says_what_is_kept_what_is_not_and_on_whose_authority` and `commands::popis::tests::the_izvestaj_warns_without_blocking_and_says_it_is_not_stored` each locate the affirmative sentence by its own bounds, require all three document names inside it, and require `izveštaj` to be absent from it.

**Checked and found clean:** `docs/compliance/` names no popis output anywhere (`evidencija-obrade-cl47.md` has no popis mention at all), and `docs_guard.rs`'s guards are about purge/period/trajno claims, none of which this touches. **`docs/PROGRESS.md` was not clean** — the 03.08.2026 residuals table still asserted „**There is no print and no export anywhere in the popis module**“ and „The same sentence now also says the program does not print the liste“, both false since Task 3. They are dated records of what was true then, so they are superseded by a note under the table rather than rewritten in place; Task 6 restates reqs. 31/32/35 properly.

**Gates:** bun **530 passed / 35 files**, `bun run build` ✓, cargo **974 passed, 0 failed**, clippy clean, fmt clean, `git diff --check` clean. No migration; head stays **v22**.

**Residual for Task 6.** Req. 32's „editable default template“ limb is still unbuilt (Task 2's OPEN note) and this task adds no operator control over the layout either. The čl. 2 st. 6 *delivery* is still the shop's — the module now prints the signed lista but sends it to nobody, which is what `popis_imovine`'s `vrsta_primalaca` already says.

---

### Task 6: Docs + full gate run

- [ ] Move reqs. 31, 32 and 35 out of the „Still open“ lists in `docs/PROGRESS.md` and out of register row 19 / the §3 SW-16 row in `docs/SERBIAN-LAW-COMPLIANCE.md`, replacing them with what shipped — and **leave req. 34's *usvojen* limb and req. 40's second limb where they are**, because this cycle does not touch them.
- [ ] **Req. 32 moves only in part.** Its header, column set, signature block and no-obrazac limbs shipped in Task 2; its **„editable default template“ limb did not** — `zaglavlje_kolona` and the row renderers are a fixed layout with no operator control, and no task in this cycle adds one (see Task 2's OPEN note). Leave that limb in the „Still open“ list with a one-line reason. Closing req. 32 whole would put a false claim in two compliance documents, which is the exact class this plan's constraints forbid.
- [ ] Record honestly that the izveštaj is still screen-only if that remains true after Task 4.
- [ ] Run all six gates; report exact counts. Commit.

---

## Self-review

**Coverage.** Req. 31 (print-and-sign is the compliant path) → Tasks 2, 3, 5. Req. 32 (header, columns, editable default, no obrazac claim) → Task 2 — **except the „editable default“ limb, which Task 2 records as unbuilt and Task 6 must leave open.** Req. 35 (generate + approve) → Tasks 1, 4, 5. The §2c „no obrazac“ rule → Task 2's dedicated test. The čl. 8 st. 5 extension to print → Task 2's first test, which is the load-bearing one.

**Placeholders.** Tasks 3, 4 and 6 name required behaviours rather than pasting bodies, following the established repo seeding pattern; every behaviour is a concrete assertion. Tasks 2 and 5 carry full code.

**Type consistency.** `PrintFaza` and `render_popisna_lista` (Task 2) are consumed in Task 3. `render_odluka` / `render_plan_rada` (Task 4) reuse the same `PopisSessionView`. `ExportedFile` is the existing `reports::ExportedFile` the frontend already understands, so no new wire type.

**One gap found during review:** Task 3 originally took the phase from the caller, which would have let the frontend request the computed sheet on an unsigned session and print book quantities the query layer had withheld — reintroducing the exact defect the module exists to prevent. The phase is now derived from the session and a mismatched request is refused.
