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

- [ ] **Step 1: Write the failing tests** — the three columns exist; the pairing CHECK refuses each half-state; the posting lock covers them; **a v20-seeded survival test applying `MIGRATIONS[..20]` to a raw Connection, seeding a session, `drop(conn)`, then `Db::new`, asserting the seeded row survives verbatim** — mirror commit `270796c`. A test that seeds after `Db::new` proves nothing.
- [ ] **Steps 2–5:** run red, implement, run green, commit.

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

- [ ] **Step 1: Write the failing tests**

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

- [ ] **Steps 2–5:** run red, implement, run green, commit.

---

### Task 3: The two export commands

**Interfaces:** `popis_export_lista(state, id, faza) -> ExportedFile`, admin-gated, following `kep_export_book` exactly (`commands/kep.rs:303`): load the view, render, `campaigns::write_export(state, &file_name, &html, row_count)`.

**The phase is not the caller's choice — derive it.** A caller asking for Faza B on an unsigned session must be refused, not obeyed: the whole čl. 8 st. 5 property collapses if the frontend picks. Use the session's own `faza_a_potpisana` / status.

- [ ] **Step 1: Write the failing tests** — a `counting` session exports the Faza A sheet; requesting the computed sheet before the čl. 8 st. 5 potpis is refused with a typed error; a `computed` session exports the Faza B sheet; the file name carries the session id and the phase; a cashier is refused.
- [ ] **Steps 2–5.**

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
- [ ] Record honestly that the izveštaj is still screen-only if that remains true after Task 4.
- [ ] Run all six gates; report exact counts. Commit.

---

## Self-review

**Coverage.** Req. 31 (print-and-sign is the compliant path) → Tasks 2, 3, 5. Req. 32 (header, columns, editable default, no obrazac claim) → Task 2. Req. 35 (generate + approve) → Tasks 1, 4, 5. The §2c „no obrazac“ rule → Task 2's dedicated test. The čl. 8 st. 5 extension to print → Task 2's first test, which is the load-bearing one.

**Placeholders.** Tasks 3, 4 and 6 name required behaviours rather than pasting bodies, following the established repo seeding pattern; every behaviour is a concrete assertion. Tasks 2 and 5 carry full code.

**Type consistency.** `PrintFaza` and `render_popisna_lista` (Task 2) are consumed in Task 3. `render_odluka` / `render_plan_rada` (Task 4) reuse the same `PopisSessionView`. `ExportedFile` is the existing `reports::ExportedFile` the frontend already understands, so no new wire type.

**One gap found during review:** Task 3 originally took the phase from the caller, which would have let the frontend request the computed sheet on an unsigned session and print book quantities the query layer had withheld — reintroducing the exact defect the module exists to prevent. The phase is now derived from the session and a mismatched request is refused.
