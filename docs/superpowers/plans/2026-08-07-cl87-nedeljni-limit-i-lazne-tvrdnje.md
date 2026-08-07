# SW-14 req. 12 weekly leg + the two false claims on the Podešavanja screen

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enforce the ZoR čl. 87 cap of **35 časova nedeljno** for an employee under 18 — the one leg of SW-14 req. 12 that was never built — and withdraw two false statements the Podešavanja screen makes to the operator: an archive duty that binds only the public sector, and a retention floor attributed to the wrong statute.

**Architecture:** The weekly leg goes where the daily leg already is — `worktime::check_protection` — because čl. 87 **blocks** rather than being overridable, and putting it beside `assess_caps` would make it look like a čl. 53 cap the operator can walk through with a ground. `check_protection` gains a `week: &[DayHours]` parameter; the one production caller already loads that week for `assess_caps`, so nothing new is read from the database. The two screen strings are corrected and then pinned by `docs_guard`, so UI copy cannot drift from the citation `backup.rs` already got right.

**Tech Stack:** Rust (rusqlite, serde) + Tauri v2; React 19 + TypeScript + shadcn/ui + vitest; bun.

## Global Constraints

- **Money is integer minor units (para); quantities milli-units; time in whole minutes.** Never floating point.
- **Timestamps RFC3339 passed as a `now: &str` param.** Never `datetime('now')` in decision code.
- **Serbian Latin diacritics** in every operator string; Serbian quotes open `„` and close `“`.
- **NO migration in this cycle.** Head is **v22** and stays v22 — every column this plan needs already exists. Never edit v1–v22.
- **NO fine figure outside `legal.rs`.** This cycle adds no notice, so the `all_notices` count stays **9**.
- **No ZEOR figure anywhere** — §6 W-1 is unresolved and `legal.rs`'s guard enforces it. čl. 87 is **ZoR**, not ZEOR; do not let that slip.
- **A document, a generated artefact or an operator string must never promise or assert behaviour the code does not implement** — and, as this cycle's register work shows, must not **deny** behaviour the code does implement either. Both directions are the same defect.
- **Never boot the app.** Verify only via the gates.
- **Do not weaken, skip or delete a test to make a change pass** — except the two this plan names explicitly, which exist to be deleted at exactly this moment and say so in their own doc comments.
- Commit trailer: `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`

**Gates (six).** `--test-threads=1` was retired in `c93e28e`; the cargo gate is now plain `cargo test`.

```
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

**Governing law:** [SW14-VERIFIED-RULES.md](../../SW14-VERIFIED-RULES.md) §4 req. 12 and the §3 W4b table row (*„Under 18 | overtime and preraspodela banned; ≤ 35 h/week and ≤ 8 h/day | čl. 88 st. 1; čl. 87; čl. 88 st. 2“*). For Task 3: [SW11-SW15-VERIFIED-RULES.md](../../SW11-SW15-VERIFIED-RULES.md) §3 req. 39 and §4 item 8 (ZAG čl. 16 st. 2), and §2 Q4 for the retention floor.

**Baseline:** cargo **988** passed, bun **539** passed / 35 files, all six green, migration **v22**, HEAD `c93e28e`.

---

## File structure

**Modified Rust:** `src-tauri/src/worktime.rs` (the weekly leg, its constant, its `ProtectionKind` variant, the deleted gap test), `src-tauri/src/commands/worktime.rs` (the one production call site), `src-tauri/src/docs_guard.rs` (one guard inverted, one added).
**Modified frontend:** `src/app/settings/SettingsScreen.tsx` (two strings) + `SettingsScreen.test.tsx` if one exists for that dialog.
**Modified docs:** `docs/PROGRESS.md`, `docs/SERBIAN-LAW-COMPLIANCE.md`.

**No new file.** The weekly leg is four fields' worth of arithmetic beside an existing one; a module of its own would separate two halves of a single sentence of čl. 87.

---

### Task 1: The čl. 87 weekly leg

**Files:**
- Modify: `src-tauri/src/worktime.rs` — `ProtectionKind` (~:202), `MINOR_DAILY_CAP_MINUTES` (~:164), `check_protection` (~:262), and the test module.
- Modify: `src-tauri/src/commands/worktime.rs:629` — the sole production call site.

**Interfaces:**
- Consumes: `DayHours { dan, efektivno_minuta, prekovremeni_minuta }`; `in_same_iso_week(a, b) -> bool` (`worktime.rs:148`, Monday-based, already used by `assess_caps`); `EmployeeProtection`; `is_younger_than(..., PUNOLETSTVO_GODINA)`.
- Produces: `pub const MINOR_WEEKLY_CAP_MINUTES: i64 = 35 * 60;`, `ProtectionKind::MaloletanNedeljniLimit`, and the **new signature**
  `pub fn check_protection(p: &EmployeeProtection, day: &str, entry: &DayHours, week: &[DayHours]) -> Vec<ProtectionBlock>`.

**Why the signature widens rather than a sibling function being added.** There are 28 call sites, exactly **one** of which is production (`commands/worktime.rs:629`); the other 27 are tests. A sibling `check_weekly_protection` would leave two doors, and a future caller that walks through only the first gets a guard that is complete by convention — the failure mode this module's own doc comments criticise. Widening costs 27 mechanical test edits that pass `&[]`, which is an honest statement: those tests assert the daily legs with no other day in the week.

**The de-duplication rule is not optional.** `week` is the employee's stored days. The stored row for `day` itself must be dropped before summing, or the version being assessed is counted twice — `assess_caps` (`worktime.rs:110-117`) already solves this with `d.dan != day && in_same_iso_week(&d.dan, day)`. **Reuse that predicate; do not re-derive it.**

- [x] **Step 1: Write the failing tests**

```rust
    /// ZoR čl. 87 caps an employee under 18 at 35 časova nedeljno. Six eight-hour
    /// days is 48 h and breaks it, while satisfying the daily leg every single
    /// day — which is precisely why the daily leg alone never raised anything.
    #[test]
    fn six_eight_hour_days_break_the_cl_87_weekly_cap_for_a_minor() {
        let p = protection_born("2009-09-01");
        let week: Vec<DayHours> = [
            "2026-08-03", "2026-08-04", "2026-08-05", "2026-08-06", "2026-08-07",
        ]
        .iter()
        .map(|dan| DayHours { dan: (*dan).to_string(), efektivno_minuta: 480, prekovremeni_minuta: 0 })
        .collect();

        // The sixth day: 5 × 8 h stored + 8 h now = 48 h.
        let blocks = check_protection(&p, "2026-08-08", &day(480, 0), &week);

        assert!(
            blocks.iter().any(|b| b.kind == ProtectionKind::MaloletanNedeljniLimit),
            "48 h in one week must raise the čl. 87 weekly leg: {blocks:?}"
        );
        assert!(
            blocks.iter().any(|b| b.kind == ProtectionKind::MaloletanNedeljniLimit && b.blocking),
            "čl. 87 is a prohibition, not an overridable cap: {blocks:?}"
        );
    }

    /// Exactly 35 h is lawful — the cap is „do 35 časova“, so the breach is
    /// strictly above it. Off by one here refuses a week the law allows.
    #[test]
    fn exactly_thirty_five_hours_is_within_the_cl_87_weekly_cap() {
        let p = protection_born("2009-09-01");
        let week: Vec<DayHours> = ["2026-08-03", "2026-08-04", "2026-08-05", "2026-08-06"]
            .iter()
            .map(|dan| DayHours { dan: (*dan).to_string(), efektivno_minuta: 420, prekovremeni_minuta: 0 })
            .collect();

        // 4 × 7 h stored + 7 h now = 35 h exactly.
        let blocks = check_protection(&p, "2026-08-07", &day(420, 0), &week);

        assert!(
            !blocks.iter().any(|b| b.kind == ProtectionKind::MaloletanNedeljniLimit),
            "35 h is the cap, not a breach of it: {blocks:?}"
        );
    }

    /// The stored row for the day being assessed must not be counted beside the
    /// version replacing it, or a correction that LOWERS the hours reads as a
    /// breach. This is the defect `assess_caps` already guards against.
    #[test]
    fn the_stored_row_for_the_day_under_assessment_is_not_double_counted() {
        let p = protection_born("2009-09-01");
        // The whole week is stored, including a 10 h row for the day we reassess.
        let week: Vec<DayHours> = [
            ("2026-08-03", 480), ("2026-08-04", 480), ("2026-08-05", 480), ("2026-08-06", 600),
        ]
        .iter()
        .map(|(dan, m)| DayHours { dan: (*dan).to_string(), efektivno_minuta: *m, prekovremeni_minuta: 0 })
        .collect();

        // Correcting 2026-08-06 down to 4 h: 3 × 8 h + 4 h = 28 h, inside the cap.
        let blocks = check_protection(&p, "2026-08-06", &day(240, 0), &week);

        assert!(
            !blocks.iter().any(|b| b.kind == ProtectionKind::MaloletanNedeljniLimit),
            "the stored 10 h row was counted beside the 4 h correction replacing it: {blocks:?}"
        );
    }

    /// Days in a neighbouring week are not this week's hours. `in_same_iso_week`
    /// is Monday-based, so 2026-08-02 (a Sunday) belongs to the week before.
    #[test]
    fn a_neighbouring_week_does_not_feed_the_cl_87_total() {
        let p = protection_born("2009-09-01");
        let week: Vec<DayHours> = ["2026-07-28", "2026-07-29", "2026-07-30", "2026-07-31", "2026-08-02"]
            .iter()
            .map(|dan| DayHours { dan: (*dan).to_string(), efektivno_minuta: 480, prekovremeni_minuta: 0 })
            .collect();

        let blocks = check_protection(&p, "2026-08-03", &day(480, 0), &week);

        assert!(
            !blocks.iter().any(|b| b.kind == ProtectionKind::MaloletanNedeljniLimit),
            "last week's 40 h reached this week's čl. 87 total: {blocks:?}"
        );
    }

    /// An adult is not capped at 35 h by čl. 87 at all — the article speaks only
    /// of an employee under 18. Guarding this stops the leg leaking onto the
    /// whole workforce, which would refuse an ordinary 40-hour week.
    #[test]
    fn the_cl_87_weekly_leg_does_not_reach_an_adult() {
        let p = protection_born("1990-01-01");
        let week: Vec<DayHours> = ["2026-08-03", "2026-08-04", "2026-08-05", "2026-08-06", "2026-08-07"]
            .iter()
            .map(|dan| DayHours { dan: (*dan).to_string(), efektivno_minuta: 480, prekovremeni_minuta: 0 })
            .collect();

        let blocks = check_protection(&p, "2026-08-08", &day(480, 0), &week);

        assert!(
            !blocks.iter().any(|b| b.kind == ProtectionKind::MaloletanNedeljniLimit),
            "čl. 87 reaches „zaposleni mlađi od 18 godina“ only: {blocks:?}"
        );
    }
```

- [x] **Step 2: Delete the two guards that exist to die here.** Remove `worktime::tests::the_cl_87_weekly_leg_is_not_checked` (~:687) and `docs_guard::no_document_claims_the_cl_87_weekly_leg_is_enforced` (~:564). Both doc comments instruct exactly this and forbid weakening them instead. In `docs_guard.rs`, replace the deleted test with its **inverse**: a guard that fails if a document says the weekly leg is *unbuilt* (`"is not checked"`, `"nije proveren"`, `"not implemented"`) on a line naming `35 h` / `35 časova`. Otherwise the next stale-denial defect lands in the same place the last one did.

- [x] **Step 3: Run to verify failure** — `cargo test --manifest-path src-tauri/Cargo.toml worktime -- --quiet`. Expected: the five new tests FAIL on the unknown variant `MaloletanNedeljniLimit` and the arity of `check_protection`.

- [x] **Step 4: Implement.** Add the constant beside `MINOR_DAILY_CAP_MINUTES` with its own doc comment citing čl. 87; add the `ProtectionKind` variant with a doc comment in the register's voice; widen the signature; inside the existing `is_younger_than(...)` branch, sum the week and push the block. Operator message in the voice of its siblings, naming the figure and the article — for example: *„Zaposleni mlađi od 18 godina može raditi najviše 35 časova nedeljno (ZoR čl. 87). Za ovu nedelju je evidentirano {n} časova.“* Update `commands/worktime.rs:629` to pass the `week` it already has in scope for `assess_caps_for_employee` at `:639`, and the 27 test call sites to pass `&[]`.

- [x] **Step 5: Run green, then the full gates. Commit.**

**Shipped.** `MINOR_WEEKLY_CAP_MINUTES = 35 * 60` sits beside `MINOR_DAILY_CAP_MINUTES` (`worktime.rs:166`), `ProtectionKind::MaloletanNedeljniLimit` beside `MaloletanDnevniLimit` (`:224`), and the leg itself is inside the existing `is_younger_than(...)` branch of `check_protection` (`:334`), summing `efektivno_minuta + prekovremeni_minuta` over the plan's own predicate `d.dan != day && in_same_iso_week(&d.dan, day)` plus the entry, breached strictly above. `blocking: true`, and nothing about it touches `assess_caps` or the override mechanism. The signature is now `check_protection(p, day, entry, week)` (`:301`); the sole production caller is `commands/worktime.rs:629`, where `load_week` was **hoisted above** `check_protection` so both gates read one week inside one transaction. All 26 remaining test call sites pass `&[]`. Five tests added exactly as written; `worktime::tests::the_cl_87_weekly_leg_is_not_checked` deleted; `docs_guard::no_document_claims_the_cl_87_weekly_leg_is_enforced` deleted and replaced by its inverse `no_document_says_the_cl_87_weekly_leg_is_still_unbuilt`, bound to `MINOR_WEEKLY_CAP_MINUTES` so renaming the constant breaks compilation rather than leaving the prose unbacked. Gates measured at this commit: cargo **992 passed / 0 failed** (988 + 5 − 1), clippy clean, `cargo fmt --check` clean, `git diff --check` clean, `tsc --noEmit` clean.

**Deviations, all four recorded rather than hidden.**

1. **Three document lines were re-stated in this task, not in Task 4.** Inverting the `docs_guard` test made it fail immediately on `docs/SERBIAN-LAW-COMPLIANCE.md:118` and `docs/PROGRESS.md:213` / `:254`, which all said the weekly leg „is not checked“ — that denial is now false. The register row states the shipped leg and that it refuses the row; PROGRESS's SW-14 task row carries a dated *closed 07.08.2026* note in its own cell; the „Still open after this batch“ bullet is re-stated as closed on the exact pattern reqs. 31/32/35 used at `PROGRESS.md:955`, keeping what it said as the reason the work was done. **Task 4 should not restate these three again.** The 01.08.2026 review-fix paragraph (`PROGRESS.md:241`) was also corrected, because it named the test this task deleted.
2. **The čl. 88 st. 2 night leg was written into both documents as still unbuilt**, in the place the withdrawn weekly-leg denial vacated. Removing the denial without it would have let the register read as though the whole of req. 12 closed; `DayHours` still carries no night bucket, exactly as the plan's Self-review says.
3. **`src/services/types.ts:1700` gained `"maloletanNedeljniLimit"`.** The plan says the frontend contract is untouched in Task 1, but `WorkTimeProtectionKind` is a hand-kept mirror of the Rust enum and a mirror that omits a value the backend now sends is the same defect as a document that denies it. No component switches on `kind` (it is a React key and the `poruka` is rendered), so nothing else moved; `tsc --noEmit` passes.
4. **`worktime::tests::no_protection_message_carries_a_fine_figure` gained a week and its counts moved 5 → 6 and 7 → 8.** Not a weakening: the test's own doc comment says a kind no fixture raises has its poruka outside the guard, and this is the one message built with `format!`. The fixture now carries four stored eight-hour days so the weekly leg fires and its text is swept for a fine figure like the rest.

**Environment note for the orchestrator.** `xcrun` on this machine reports *„You have not agreed to the Xcode license agreements“*, which fails the linker and even `git diff --check`. Every command above was run with `DEVELOPER_DIR=/Library/Developer/CommandLineTools` prefixed; nothing in the repository was changed for it.

**Review fixes shipped (07.08.2026).** Twelve findings, four distinct defects, all closed with a regression test each; cargo **999 passed / 0 failed** (992 + 7), clippy, `fmt --check` and `git diff --check` clean.

1. **The refusal was unescapable and is now gated on the write raising the week.** `unos_minuta > evidentirano_za_dan` beside the cap comparison (`worktime.rs:421`), where `evidentirano_za_dan` is the stored row for `day` that the `dan != day` filter drops. A week already over 35 h when the guard turns on — `datum_rodjenja` is nullable and unbackfilled, so `commands/users.rs` switches the guards on over rows already recorded, and a restored backup does the same — could otherwise never be corrected **downwards** at any value, and a day of pure absence was refused on the strength of five rows the write does not touch. Tests: `a_correction_that_lowers_a_minors_week_is_not_refused`, `a_zero_hour_row_is_never_refused_by_the_weekly_leg`, `raising_a_day_in_an_already_over_week_is_still_refused` (the pin against over-correcting — it was already green and stays green), and the end-to-end `commands::worktime::tests::a_birth_date_filled_in_later_does_not_lock_a_minors_recorded_week`, which drives `save_entry`/`correct_entry` through the real write path after `datum_rodjenja` lands late. `blocking` stays `true`; nothing became overridable.
2. **The poruka said „evidentirano je“ about a total the same call refuses to record.** It now names both figures and confuses neither: *„… Za dane ove kalendarske nedelje u kojima je zaposleni mlađi od 18 godina već je evidentirano {} č {:02} min, a sa ovim danom bilo bi {} č {:02} min.“* Pinned inside `six_eight_hour_days_break_the_cl_87_weekly_cap_for_a_minor`, which now asserts „već je evidentirano 40 č 00 min“, „bilo bi 48 č 00 min“ and the *absence* of „evidentirano 48“. This is a deliberate deviation from Step 4's sample string, which carried the same defect.
3. **Days after the eighteenth birthday no longer feed the minor's weekly total, and unreadable rows no longer feed it either.** čl. 87 caps „zaposleni mlađi od 18 godina života“, so the filter gained `is_younger_than(..., PUNOLETSTVO_GODINA)` per stored day; and `in_same_iso_week` was replaced in this leg by a new private `strictly_in_same_iso_week` (`worktime.rs:171`), because that function's fail-open rationale is written for `assess_caps`, where over-counting asks for a ground — here it refuses a write, and v17's GLOB CHECK lets `2026-08-32` into the column. `in_same_iso_week`'s doc comment now names both consumers and says the safe direction inverts with the consequence. Tests: `the_cl_87_weekly_total_counts_only_the_days_the_employee_was_under_eighteen`, `an_unreadable_stored_day_does_not_silently_refuse_a_minors_week`, `the_two_week_predicates_differ_only_on_a_day_that_does_not_parse`. **Deviation from the plan's „reuse `assess_caps`'s predicate, do not re-derive it“:** two of the three filters are deliberately narrower here, and the doc comment states each reason beside the čl. 53 one.

   Two things about this one are stated rather than glossed. The review suggested pairing the drop with a **non-blocking `NeispravanDatumUProfilu`** naming the unreadable `dan`; that was not done, because that variant's own doc comment scopes it to „a profile date the operator entered“ and its poruka tells the operator to fix the *birth date in the profile* — pointing it at a stored `dan` would make an operator string false, and a new `ProtectionKind` widens the serialized contract and its frontend mirror for a row `write_entry` cannot create. An unreadable stored `dan` is therefore dropped **silently** from the čl. 87 total, and nothing in the app reports it. Second, inside `check_protection` the strict predicate currently **overlaps** the age filter — `is_younger_than` also fails closed on an unreadable day — so `an_unreadable_stored_day_does_not_silently_refuse_a_minors_week` passes for two independent reasons. The overlap is incidental, both doc comments now say so, and the new predicate test asserts the one input on which the two week predicates actually differ, so the strict one cannot quietly decay into a synonym.
4. **`docs_guard::no_document_says_the_cl_87_weekly_leg_is_still_unbuilt` was under-worded, mis-scoped and vacuous.** `UNBUILT` went 3 → 10 markers (adding „Gap“, the register's own idiom, plus „not built“, „unbuilt“, „not enforced“, „nije implementiran“, „nije sprovedeno“, „nema proveru“); judging moved from the whole line to the clause, via a new `clause_around` helper (`docs_guard.rs:47`) splitting on `;`, the em dash, the parentheses and the cell pipe; and a per-document presence assertion was added, the one every sibling guard already had. Red was verified in both directions before the fix landed: the naive widening failed on `SERBIAN-LAW-COMPLIANCE.md:118`, where the „still unbuilt“ denial is TRUE because it is about the čl. 88 st. 2 **night** leg — which is the evidence the scoping is load-bearing — and removing „35 časova“ from that line failed the presence assertion.

**Two further document edits, both required by the above.** `PROGRESS.md:213` said „the 35 h/week leg was **not built in this batch**“, which the widened guard correctly fails; it now reads „the 35 h/week leg **closed 07.08.2026**, one cycle after this batch“. And both `SERBIAN-LAW-COMPLIANCE.md:118` and the `PROGRESS.md` req. 12 bullet now state that what is refused is the write which *raises* the week, that the total counts only the days the employee was under 18, and that unreadable rows are dropped — a register saying only „refuses the row“ would read as though a minor's over-cap week cannot be touched at all, which is the assert-what-the-code-lacks direction of the same house rule.

---

### Task 2: The weekly leg reaches the write path and the screen

**Files:**
- Modify: `src-tauri/src/commands/worktime.rs` — `write_entry` (~:571-640).
- Test: same file's test module; `src/app/worktime/WorkTimeModule.test.tsx` if the block surfaces differently.

**Interfaces:** consumes `ProtectionKind::MaloletanNedeljniLimit` from Task 1. Produces no new type — a blocking `ProtectionBlock` already refuses the write and already renders.

Task 1 makes `check_protection` capable of the finding. This task proves it actually **refuses the row** end to end, because a guard the write path computes and discards is the same as no guard.

- [x] **Step 1: Write the failing test** — in `commands/worktime.rs`'s test module, drive the real write path: seed an employee under 18, save five eight-hour days through `worktime_save_entry`, then attempt a sixth. Assert the sixth is **refused**, that the error names čl. 87, and that `worktime_list_month` shows **five** live rows afterwards — the refusal must leave nothing behind, since the register is append-only and a permanent row is the thing being prevented.
- [x] **Step 2: Run red** — confirm the sixth day currently saves.
- [x] **Step 3: Implement** whatever wiring the test proves missing. If `write_entry` already refuses on any blocking `ProtectionBlock`, this task is a regression test only — say so plainly in the Shipped note rather than inventing a change.
- [x] **Step 4: Run green.** **Step 5: Full gates, commit.**

**Shipped — and this task is a regression test only, in both halves.** `write_entry` already refuses on *any* blocking `ProtectionBlock` (`commands/worktime.rs:636`), and that `find(|block| block.blocking)` predates this cycle: Task 1's variant was carried by it the moment it existed, so no production Rust changed here. The frontend needed nothing either — `WorkTimeModule.tsx:613` maps every finding off `blocking` and `poruka` with **no switch over `kind`**, so `maloletanNedeljniLimit` reaches the operator as a destructive „Unos nije dozvoljen“ alert without a branch being added for it, and `WorkTimeProtectionKind` (`src/services/types.ts:1700`) already carried the value from Task 1. Nothing in this task was invented to look busy.

What landed is two tests, one per side of the wire.

- `commands::worktime::tests::the_cl_87_weekly_leg_refuses_the_write_and_leaves_no_row_behind` (`commands/worktime.rs:2025`) — profile filled in **before** the first write, unlike its neighbour `a_birth_date_filled_in_later_does_not_lock_a_minors_recorded_week`, so the guard is live for every day of the week. It asserts the refusal (`protection_block`), that the poruka names „35 časova nedeljno“, „čl. 87“ and both figures („već je evidentirano 35 č 00 min“ / „bilo bi 43 č 00 min“), that a `cap_override_razlog` does **not** buy the day (the protection gate runs before the čl. 53 caps and čl. 87 has no override), and then lists the month: **five** rows, none of them `2026-08-08`, all live, `efektivno` exactly 2100 minutes. The count is the point — a computed-and-discarded guard leaves a row an append-only register can never withdraw.
- `WorkTimeModule protection findings › surfaces the blocking čl. 87 weekly finding a refused save carries` (`src/app/worktime/WorkTimeModule.test.tsx:560`) — the `code === "protection_block"` branch at `WorkTimeModule.tsx:428` had **no test at all**, so a refusal the operator cannot see was previously unpinned. It feeds the exact serialized payload, variant name included, and asserts the blocking title, the poruka, the citation, and the *absence* of „Napomena o zaštiti zaposlenog“ — a prohibition dressed as a note is the same defect as no note.

**Deviations, four.**

1. **„Five eight-hour days then a sixth“ cannot be written through the real path, and the test says so in code.** With the guard live, the **fifth** eight-hour day is already 40 h and is refused. The recorded week is Mon–Thu at 8 h plus a short 3 h Friday — exactly 35 h, which pins the „do 35 časova“ boundary end to end as a bonus — and the sixth day (Saturday, 8 h) is the refused one. The plan's two checkable assertions hold verbatim: the sixth is refused, and five live rows stand.
2. **Driven through `save_entry` / `list_month`, not the `worktime_save_entry` / `worktime_list_month` wrappers.** Those wrappers take their `now` from `utc_now()`, and `2026-08-08` is in the future against the build machine's clock, so the wrapper would refuse the sixth day on `guard_day_has_happened` — the wrong reason — and would tie a statutory test to the wall clock, against this module's own rule 3. `write_entry` is the sole write path both wrappers reach. To keep the command layer pinned anyway, the refusal is converted with `CommandError::from` and the test asserts the serialized `details.protections[0]` is `{"kind":"maloletanNedeljniLimit","blocking":true}` — the exact object `WorkTimeModule`'s `protectionsFrom` reads.
3. **Red was proven by mutation, because the feature was already wired.** Flipping the weekly block's `blocking: true` → `false` in `worktime.rs` made the sixth day **save** — row id 6, `2026-08-08`, `weekly_total_minutes: 2580` — with the finding computed and handed back on the success path, which is precisely the failure mode this task exists to bar. `worktime.rs` was restored immediately and its `git diff` is empty. The frontend test was verified the same way, by deleting `setProtections(protectionsFrom(error))`: the alert never rendered and the operator saw only „Dan nije evidentiran“.
4. **`src/app/worktime/WorkTimeModule.test.tsx` was touched although the plan lists it as conditional** („if the block surfaces differently“). It does not surface differently — it surfaces through an untested branch, which is the same risk with none of the visibility.

**One thing recorded and deliberately not fixed here.** `createMockServices().worktime.saveEntry` (`src/services/mock-adapter.ts:3675`) returns `protections: []` unconditionally and implements **no** čl. 87–91 guard — not the weekly leg, not the daily leg, not the čl. 88 st. 1 overtime ban. That is pre-existing and predates this cycle; the demo double *under*-states the backend rather than over-stating it, so nothing there asserts behaviour the code lacks, and building the guards into the double is not this plan's scope.

**Gates measured at this commit** (`DEVELOPER_DIR=/Library/Developer/CommandLineTools` prefixed, per Task 1's environment note): cargo **1000 passed / 0 failed** (999 + 1), `bunx vitest run src/app/worktime` **45 passed / 2 files** (44 + 1), `bunx tsc --noEmit` clean, clippy clean, `cargo fmt --check` clean, `git diff --check` clean.

---

### Task 3: Withdraw the two false claims on the Podešavanja screen

**Files:**
- Modify: `src/app/settings/SettingsScreen.tsx:1833-1836`.
- Modify: `src-tauri/src/docs_guard.rs` — one new guard.

The paragraph inside the reset confirmation dialog currently reads, verbatim:

> Zakon zahteva čuvanje evidencija do 10 godina (ZoRač čl. 28; **ZPDV čl. 47**). Pre brisanja se obavezno pravi rezervna kopija — čuvajte je trajno. **Pravna lica ne smeju uništavati dokumentarni materijal bez pismenog odobrenja arhiva.**

Two defects, both `[LEGAL]`, both pre-existing since SW-3:

1. **The archive duty is false as applied.** ZAG čl. 16 st. 2 confines prior written archive approval to the **public sector**. SW11-SW15 §3 req. 39 / §4 item 8 requires the claim removed. The pilot is a preduzetnik; the sentence tells them they need permission they do not need.
2. **The retention floor cites the wrong statute.** `backup.rs:567-571` already corrected this — *„the general 10-year floor is ZPPPA čl. 114ž (apsolutna zastarelost) plus ZoRač čl. 28 st. 4 (dnevnik i glavna knjiga)“* — and a test there bars `ZPDV` from the tombstone string. The UI copy was never brought into line, so the app states two different floors for the same duty depending on which surface you read.

- [ ] **Step 1: Write the failing tests.** In `docs_guard.rs`, a guard that reads `SettingsScreen.tsx` via `include_str!` and fails if it contains `ZPDV` on a line about the retention floor, or contains `odobrenja arhiva`. Bind it to the citation `backup.rs` uses so the two cannot drift apart again — the existing `docs_guard` tests show the pattern. Add a vitest assertion that the dialog names ZoRač čl. 28 st. 4 and ZPPPA čl. 114ž.
- [ ] **Step 2: Run red.** Both must fail against the current copy — verify the failure message names the real string.
- [ ] **Step 3: Implement.** Delete the archive sentence outright; do not soften it into a hedge, because a hedge is still an assertion the shop cannot act on. Correct the citation to `ZoRač čl. 28 st. 4; ZPPPA čl. 114ž`. Keep the backup sentence, which is true and useful.
- [ ] **Step 4: Run green.** **Step 5: Full gates, commit.**

---

### Task 4: The register says „Gap“ for five things that shipped

**Files:** `docs/SERBIAN-LAW-COMPLIANCE.md`, `docs/PROGRESS.md`.

The register understates in at least five rows. This is the **mirror** of the false-promise class this project has fought six times, and it is not harmless: on 07.08.2026 it led a survey agent to rank two long-fixed bugs as pilot blockers.

**Verify each against the code before restating it — do not take this table's word for it, and do not take mine.** Candidates found by survey:

| Row | Status cell says | What the code shows |
|---|---|---|
| 6 | „Gap (no banner on sale summary)“ | `OVO NIJE FISKALNI RAČUN` is in `ReceiptsScreen.tsx` and `RegisterScreen.tsx`, with tests |
| 17 | „Gap (no KEP module)“ | `src-tauri/src/kep.rs` and the KEP commands exist and are wired |
| 18 | „Gap (no KEP, no price docs)“ | `kep_kalkulacija.rs`, `kep_storno.rs`, `kep_close.rs` exist |
| 20 | „Gap: reset/restore delete trading data with no retention guard“ | `retention.rs` + `assert_never_purge_intact` inside `reset_trading_data` |
| 24 | „Gap (**no price history at all**)“ | `price_history.rs` is a full module; `campaigns.rs` consumes `prethodna_cena` |

- [ ] **Step 1:** For each row, grep the named symbol and record what you found. **If the code does not support restating the row, leave the row alone and say so** — an over-corrected register is the defect this task exists to remove.
- [ ] **Step 2:** Restate only the verified rows, in the register's existing voice, each with a `Re-stated 07.08.2026` stamp like the rows above them. State what ships **and what within that row still does not** — several of these are partials, not completions.
- [ ] **Step 3:** Add the cycle's section to `docs/PROGRESS.md`: what shipped, the six gates with exact measured counts, the residuals, and the note that req. 12's weekly leg is now closed while reqs. 10, 14, 15, 16, 21, 26, 27, 28 of SW-14 remain open.
- [ ] **Step 4:** Run `cargo test docs_guard` — it embeds both documents and will catch a row that now claims more than the code delivers. **Step 5: Full gates, commit.**

---

## Self-review

**Spec coverage.** SW-14 §4 req. 12 weekly leg → Tasks 1 and 2 (Task 1 the rule, Task 2 the write path, because a computed-and-discarded guard is the failure mode this codebase names by hand). SW11-SW15 §3 req. 39 / §4 item 8 → Task 3. The §2 Q4 retention citation → Task 3. Register accuracy → Task 4.

**Placeholders.** Task 1 carries all five tests in full. Tasks 2–4 name required behaviours and exact strings rather than pasting bodies, following the repo's established seeding pattern; every behaviour is a concrete assertion against a named symbol or a quoted string.

**Type consistency.** `MINOR_WEEKLY_CAP_MINUTES`, `ProtectionKind::MaloletanNedeljniLimit` and the four-argument `check_protection` are defined in Task 1 and consumed unchanged in Task 2. `DayHours` and `in_same_iso_week` are existing and unmodified. No new wire type, so the frontend contract is untouched except for the two strings in Task 3.

**Two gaps found during review, both folded in.** Task 1 originally left `docs_guard::no_document_claims_the_cl_87_weekly_leg_is_enforced` deleted with nothing in its place, which would have removed the only thing standing between this cycle and the *next* stale claim about čl. 87 — it is now inverted rather than dropped. And Task 4 originally said „restate rows 6, 17, 18, 20, 24“ on the strength of one survey agent's spot-check; it now requires the implementer to re-verify each row against the code and to leave any unverified row alone, because restating a row on a bad survey is the same defect pointed the other way.

**One thing this plan deliberately does not do.** It does not touch the čl. 87 **night-work** leg (čl. 88 st. 2) or the čl. 62 st. 2 night threshold (SW-14 req. 14). `nocni_minuta` is an operator-entered advisory bucket that `DayHours` does not carry, so the night legs of čl. 90 and čl. 91 stay unenforced and must remain recorded as open in Task 4's residual list.
