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

**Correction to Task 5's premise — the three named strings do NOT go false here.** `retention.rs:430`, `cl47.rs:335` and `commands/popis.rs:1899` all scope their „program ga ne štampa i ne izvozi“ to the **izveštaj**, and nothing in this cycle exports the izveštaj, so all three stay true and must not be flipped. The string that actually goes false is `src/app/popis/PopisModule.tsx:855` — „Odštampajte popisne liste … program ih ne štampa i ne izvozi“ — together with the comment above it („this application has no print and no export for a popisna lista“). It is left standing here on purpose: it is a frontend string, no button reaches this command yet, and withdrawing it now would make it false in the other direction. **Task 5 must withdraw it in the same commit that adds the button**, and Task 6 must not read the other three as needing a change.

---

### Task 4: Req. 35 — generate the odluka and the plan rada, and record the approval

**Interfaces:** `render_odluka(company, view) -> String`, `render_plan_rada(company, view) -> String` in `popis_print.rs`; `popis_export_odluka`, `popis_export_plan_rada`, and `popis_odobri_plan(state, id, odobrio, now)`.

The **odluka o popisu i obrazovanju komisije** carries: obveznik, datum donošenja, the popis it orders (vrsta, objekat, datum, period), and the commission by name and uloga. The **plan rada** (čl. 8 st. 1–2) carries the same header plus the schedule and the assignment, from `plan_rada_json` where the shop has supplied it.

`popis_odobri_plan` records `plan_rada_odobrio` + `plan_rada_odobreno_at` **in one write** (the v21 CHECK enforces the pairing anyway), refuses a blank approver, refuses a posted popis, and writes an audit line.

**Do not invent the approver.** For a preduzetnik the lice iz čl. 4 st. 2 is the owner personally (PoP čl. 4 st. 2 → ZoRač čl. 43 st. 3), so **default the field to the company name and let it be edited** — never auto-approve, because an approval nobody performed is exactly the false-record class this project keeps catching.

- [ ] **Step 1: Write the failing tests** — the odluka names every commission member and the popis it orders; the plan rada renders the shop's `plan_rada_json` when present and says plainly when it is absent rather than printing an empty schedule; approving records both columns; approving with a blank name is refused; approving a posted popis is refused; nothing auto-approves.
- [ ] **Steps 2–5.**

---

### Task 5: Frontend — the print buttons and the plan/odluka panel

**Files:** `src/app/popis/PopisModule.tsx`, `src/app/popis/CountSheet.tsx`, `services/*`.

Per phase, a „Štampaj popisne liste“ action calling the export then `openForPrint`. The `counting` step's copy must say the sheet is the čl. 8 st. 5 one and carries no book data; the `computed` step's must say čl. 9 st. 3. An „Odluka o popisu“ and „Plan rada“ pair with their own print actions, and an approval control that captures the approver's name.

**Withdraw the disclaimers that are no longer true.** Three strings currently say the program neither prints nor exports the popis documents (`retention.rs`, `cl47.rs`, `commands/popis.rs`) — that was honest yesterday and becomes false the moment Task 3 lands. Update all three to describe what now exists, and keep the `cl47.rs` sweep guard passing (it will fail if a claim is left unnegated, which is the point). **The izveštaj is still not printed** — say so precisely rather than flipping the whole sentence.

- [ ] **Step 1: Write the failing tests**

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

- [ ] **Steps 2–5.**

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
