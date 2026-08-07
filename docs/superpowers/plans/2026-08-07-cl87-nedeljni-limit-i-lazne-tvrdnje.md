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

**Review fixes shipped (07.08.2026).** Four findings, four defects, each closed with a regression test whose red was proven before the fix; cargo **1004 passed / 0 failed** (1000 + 4), `bunx vitest run src/app/UserDialog.test.tsx src/app/worktime` **62 passed / 3 files** (59 + 3), clippy, `fmt --check`, `tsc --noEmit` and `git diff --check` clean.

1. **The correction path had no command-level proof at all.** Every čl. 87 assertion at this layer drove `save_entry`, and on an original the stored row for the day is absent — so `evidentirano_za_dan` is 0 and the `unos_minuta > evidentirano_za_dan` half of the guard was never decided by real SQL. `commands::worktime::tests::a_correction_that_raises_a_minors_week_past_the_cl_87_cap_is_refused` (`commands/worktime.rs:2141`) records Mon–Thu 8 h plus a 3 h Friday (exactly 35 h) and then corrects Friday to 8 h: refused with both figures („već je evidentirano 35 č 00 min“ / „bilo bi 40 č 00 min“), and `list_month` still holds five rows with Friday at verzija 1 and 180 minutes. **Red proven by mutation:** narrowing the refusal at `commands/worktime.rs:636` to `block.blocking && korekcija.is_none()` — the plausible „an append-only register must stay correctable“ refactor — let the ispravka save as row id 6, verzija 2, `weekly_total_minutes: 2400`, with the finding computed and discarded. `write_entry` was restored immediately and its `git diff` shows only the test.

2. **A čl. 87 refusal outlived the attempt that raised it.** `WorkTimeModule`'s catch set a different subset of the five pieces of assessment state per branch and cleared none of the others, so within one employee and one month a blocking finding survived into the next attempt: a minor's Saturday refused on čl. 87, then any day that already has a row, and the operator read „Unos nije dozvoljen … već je evidentirano 35 č“ above „Dan nije evidentiran — Za ovaj dan već postoji unos“ — a destructive alert asserting a refusal that did not happen. The `cap_override_required` variant is worse, because that branch also clears `saveError`: the operator is told the day is prohibited outright while the module is in fact asking for a čl. 53 razlog that would record it. The whole assessment is now dropped at the top of the catch (`WorkTimeModule.tsx:416`), the rationale the `[employeeId, godina, mesec]` effect already states for a selector change. Two vitests, both red before the fix on the surviving „Unos nije dozvoljen“ title: „does not let a čl. 87 refusal survive the next attempt“ and „does not leave a čl. 87 refusal standing beside a čl. 53 cap warning“.

3. **Both čl. 87 legs are blind to `casovi_cekanja_i_zastoja_minuta`, which the same write books as hours worked — and nothing said so.** `derive_totals` builds ZEOR čl. 24 tač. 1 b) `ukupno_ostvareni_minuta` out of efektivno + čekanje/zastoj + štrajk, while `DayHours` carries efektivno and prekovremeni only, so a minor's week of 420 + 120 × 5 stands in the register as 45 č ostvarenih and reaches čl. 87 as exactly 35 č with nothing raised. **The arithmetic was deliberately not changed.** SW14-VERIFIED-RULES §4 req. 12 and the §3 W4b row say only „≤ 35 h/week and ≤ 8 h/day“ and do not resolve which bucket the 35 časova is measured over; deciding it here would apply an invented construction as a hard refusal, and `assess_caps` shares the convention so the change is not local either. What shipped is the disclosure the module's own standard requires: `check_protection`'s doc comment now states which buckets both legs count, that the b)-total question is **open**, and what has to change if it is answered the other way (`worktime.rs:319-347`); `MINOR_WEEKLY_CAP_MINUTES` points at it; and the poruka now names both what its two figures are the sum of and what they leave out — *„… već je evidentirano {} č {:02} min efektivnog i prekovremenog rada, a sa ovim danom bilo bi {} č {:02} min. U oba zbira nisu uračunati časovi čekanja, zastoja i prekida u radu ni časovi obustave rada zbog štrajka.“* Without it the operator holds two irreconcilable numbers and no way to tell which the guard used. Tests: `worktime::tests::the_cl_87_poruka_names_the_buckets_its_figures_count` and the end-to-end `commands::worktime::tests::the_cl_87_weekly_total_leaves_out_the_cekanje_the_same_write_books_as_ostvareni`, which records the 45 h week in silence and then pins the disclosure on the refusal that follows; the frontend fixture at `WorkTimeModule.test.tsx:563` was re-stated to the exact new backend string and asserts the disclosure reaches the alert. **Open, and handed to Task 4's residual list rather than answered here: does čl. 87's 35 časova count časovi čekanja, zastoja i prekida and časovi obustave rada zbog štrajka?** It needs the same treatment as a §6 W-item — a lawyer against the ZoR text — and until it is answered the leg is the narrower of the two readings.

4. **The one screen that explains čl. 87 to the operator still stated the daily leg alone.** The „Datum rođenja“ `FieldDescription` (`AppShell.tsx:1962`) read „… ne radi duže od osam časova dnevno …“ under a paragraph saying the čl. 87–91 checks are derived from these fields, while the same column switches on a hard refusal at 35 časova nedeljno: an admin who schedules a minor six 6-hour days satisfies the leg the screen names on every day and is refused on the sixth save with a figure the app never showed them. That is the denial direction of the house rule, inside Task 2's own title. The description now names both legs, and it is pinned twice: `UserDialog.test.tsx`'s „uz Datum rođenja navodi obe granice čl. 87 — i dnevnu i nedeljnu“, and `docs_guard::the_profile_screen_states_both_legs_of_cl_87`, which `include_str!`s `AppShell.tsx`, judges each claim inside the `FieldDescription` it belongs to (JSX wraps a sentence across source lines, so the file is read with its whitespace collapsed — `clause_around`'s reasoning with the element boundary standing in for punctuation), asserts presence before wording like every sibling guard, and is bound to **both** `MINOR_DAILY_CAP_MINUTES` and `MINOR_WEEKLY_CAP_MINUTES` so renaming either cap stops the crate compiling. Both were verified red against the old copy.

**One thing left open on purpose.** The mock adapter still implements no čl. 87–91 guard (recorded above); nothing changed there.

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

- [x] **Step 1: Write the failing tests.** In `docs_guard.rs`, a guard that reads `SettingsScreen.tsx` via `include_str!` and fails if it contains `ZPDV` on a line about the retention floor, or contains `odobrenja arhiva`. Bind it to the citation `backup.rs` uses so the two cannot drift apart again — the existing `docs_guard` tests show the pattern. Add a vitest assertion that the dialog names ZoRač čl. 28 st. 4 and ZPPPA čl. 114ž.
- [x] **Step 2: Run red.** Both must fail against the current copy — verify the failure message names the real string.
- [x] **Step 3: Implement.** Delete the archive sentence outright; do not soften it into a hedge, because a hedge is still an assertion the shop cannot act on. Correct the citation to `ZoRač čl. 28 st. 4; ZPPPA čl. 114ž`. Keep the backup sentence, which is true and useful.
- [x] **Step 4: Run green.** **Step 5: Full gates, commit.**

**Shipped.** *(This paragraph records the state at `41dc298`; the review fixes below re-state the copy and correct two sentences of it — read them together.)* The paragraph in `GoLiveResetCard` (`SettingsScreen.tsx:1852`) read, at that commit, *„Zakon zahteva čuvanje evidencija do 10 godina (ZoRač čl. 28 st. 4; ZPPPA čl. 114ž). Pre brisanja se obavezno pravi rezervna kopija — čuvajte je trajno.“* The archive sentence is **gone, not hedged**, and a source comment in its place records what was withdrawn and why. The citation is no longer a literal on either surface: `commands::backup::ROK_CUVANJA_PRAVNI_OSNOV` (`backup.rs:39`) holds *„ZoRač čl. 28 st. 4; ZPPPA čl. 114ž“*, the tombstone builds `format!("10y ({ROK_CUVANJA_PRAVNI_OSNOV})")` at `backup.rs:586` — byte-identical to the string it printed before, so `reset_trading_data_writes_compliance_tombstone_that_survives_wipe` and its `!detail_json.contains("ZPDV")` assertion passed unchanged — and `docs_guard::the_reset_dialog_matches_the_tombstone_and_claims_no_archive_approval` (`docs_guard.rs:793`) requires every `;`-separated član of that constant to appear in the dialog's own `<p>`. Correcting one surface and leaving the other now fails the crate. Two vitests pin the operator side: the existing „shows the 10-year retention warning in the reset dialog“ gained the two citations plus `not.toHaveTextContent("ZPDV")`, and „does not send the shop for an archive approval ZAG čl. 16 st. 2 asks only of the public sector“ asserted ~~the dialog carries no `arhiv` and no `odobren`~~ while keeping „rezervna kopija“ — so the fix cannot degrade into deleting the whole paragraph. **Re-stated 07.08.2026 (review fix 4):** that assertion was far broader than the test's own title and barred two things the verified rules require the screen to be able to say; it is now the three-stem, one-sentence rule the Rust guard uses.

**Red was verified three times, once per assertion**, each message quoting the live copy: (1) the citation half failed on „…(ZoRač čl. 28; ZPDV čl. 47)…“ naming the missing „ZoRač čl. 28 st. 4“; (2) after the citation alone was corrected, the guard failed again on the still-present archive sentence — proving the archive half is not carried by the first; (3) the ZPDV clause was proven by mutation, appending „; ZPDV čl. 47“ to the corrected citation so both required članovi were present, which fired the ZPDV assertion alone. The mutation was reverted immediately. Both vitests were red on the real dialog text before the copy changed.

**Deviations, three.**

1. **The archive claim is judged on the stems `arhiv` + `odobren` inside one paragraph, not on the literal `odobrenja arhiva`.** The statute's own wording is „bez pismenog odobrenja **nadležnog javnog** arhiva“, which contains neither of the two words adjacently and would have walked straight past the literal the plan named. ~~The stem pair catches every case form and any interposed adjective.~~ **Re-stated 07.08.2026 (review fix 3):** it caught any interposed adjective and every case form *of the noun*, but not a capitalised „Arhiv“ and not the verb forms „odobri / odobriti / odobrava“, nor „saglasnost“ — the two commonest ways the sentence would have come back. It deliberately does **not** bar the archive duties that are real (lista kategorija saglasnost, arhivska knjiga, 30 April prepis), and §4 item 8 — never tell a preduzetnik archive law does not reach him — is respected: nothing was added denying the duty. ~~and the čuvanje sentence that remains is the čl. 9 st. 1 custody note in terms he can act on.~~ **That last clause was false and is withdrawn (review fix 2):** the čuvanje sentence is a ZoRač/ZPPPA accounting and tax retention period, not the ZAG čl. 9 st. 1 custody duty, so req. 39's second limb was unmet until the review fix shipped the note itself.
2. **`backup.rs` is in the diff, although Task 3's file list names only `SettingsScreen.tsx` and `docs_guard.rs`.** The plan's own instruction — bind the guard „to the citation `backup.rs` uses rather than to a copy of the string“ — is not satisfiable while that citation is a literal inside a `json!` macro; binding to a copy is what let the two drift for a week. The change is a literal lifted into a documented `pub const` with no behavioural difference, and `backup.rs`'s existing tombstone test is the proof.
3. **`SettingsScreen.tsx` also gained a JSX comment**, in the house's comment density, recording both withdrawn claims and why each is false. The guard sweeps source comments along with the copy — so the note is written in English on purpose, and the doc comment says so: restating the withdrawn duty in Serbian next to the dialog is how it would find its way back into the dialog.

**Gates measured at this commit** (`DEVELOPER_DIR=/Library/Developer/CommandLineTools` prefixed, per Task 1's environment note): cargo **1005 passed / 0 failed** (1004 + 1), `bunx vitest run src/app/settings` **67 passed / 4 files** (66 + 1), `docs_guard` alone 19 (18 + 1), `backup` alone 31 unchanged, clippy clean, `cargo fmt --check` clean, `bunx tsc --noEmit` clean, `git diff --check` clean.

**Review fixes shipped (07.08.2026).** Eleven findings, six distinct defects, each closed with a regression test whose behaviour was proven by mutation in both directions. The sentence the dialog now carries is *„Zakon zahteva čuvanje evidencija najmanje 10 godina (ZoRač čl. 28 st. 4; ZPPPA čl. 114ž) — rok se može produžiti, a nikada se ne skraćuje. Pre brisanja se obavezno pravi rezervna kopija — čuvajte je trajno.“*, followed by a paragraph of its own: *„Dokumentaciju čuvajte savesno, u sređenom i bezbednom stanju (ZAG čl. 9 st. 1).“*

1. **The floor was printed as a ceiling, and the guard required it to be.** „do 10 godina“ is „up to 10 years“ — SW11-SW15 §1 row 7 records that framing as a HIGH defect *beside* the ZPDV miscitation, because ZPPPA čl. 114z st. 2 keeps zastoj out of the absolute period and čl. 114ž ends „osim ako ovim zakonom nije drukčije propisano“; `SERBIAN-LAW-COMPLIANCE.md:76` says it in terms. The first correction fixed the citation and left the framing, then pinned it: `ROK` embedded „do 10 godina“ and a `pomena > 0` assertion made the phrase mandatory under a message calling it a floor. The needle is now „čuvanje evidencija najmanje 10 godina“, four ceiling markers are barred file-wide, and `SettingsScreen.test.tsx`'s matcher moved with it.
2. **Req. 39's second limb was never shipped, and three artefacts said it was.** The requirement is two-limbed — withdraw the destruction-approval claim **and** keep a neutral čl. 9 st. 1 custody note — and only čl. 9 **st. 2** carries the „osim fizičkih lica“ carve-out (§1 row 12), so st. 1 reaches this shop. The source comment, the commit body and deviation 1 above all asserted the remaining čuvanje sentence *was* that note; it is an accounting and tax period under different statutes. The note now ships as a paragraph of its own, with no penalty figure and no čl. 9 st. 2 catalogue, and is pinned by `docs_guard` (`CUVANJE`) and by the new vitest „keeps the neutral ZAG čl. 9 st. 1 custody note req. 39 asks for in place of the withdrawn claim“. The false equivalence is struck from all three artefacts.
3. **The archive sweep was blind to capitalisation, to verb forms and to „saglasnost“, and it judged a 267-line window.** The file's only lower-case „arhiv“ sits in a JSX comment inside no `<p>`, so `rfind("<p ")` walked back to the passphrase card at `:1589` and forward to `:1856`: any „odobren“ anywhere in that span accused the developer of a claim about an arhiv, while „Arhiv mora pismeno odobriti uništavanje…“ produced no match index at all. Judging is now sentence-local (`sentence_around`, which buys back the full stop with a capital-letter lookahead so „čl. 28 st. 4“ is never cut), matching is case-insensitive (`match_indices_ci`), and the claim is `arhiv` + `odobr`/`saglasn`/`dozvol` + `uništ`/`bris` **in one sentence** (`claims_an_archive_destruction_approval`), so the real duties stay statable. `paragraph_around` now returns `None` when a `</p>` intervenes rather than silently widening.
4. **The vitest banned every mention of the archive.** Its title promised to bar the approval claim; `not.toHaveTextContent(/arhiv/i)` barred the čl. 9 st. 1 note req. 39 requires and the pravno-lice copy row 88 asks for. It now splits the dialog's `textContent` into sentences and applies the same three-stem rule as the Rust guard. **Proven by mutation in both directions:** the withdrawn sentence, capitalised, turns it red; „Lista kategorija dokumentarnog materijala sa rokovima čuvanja donosi se uz saglasnost nadležnog javnog arhiva.“ — the true, required duty — leaves it green. An intermediate narrowing that only excluded `arhiv` beside an approval stem still failed that second mutation and was itself replaced.
5. **The compliance memo carried both withdrawn claims and no guard read it.** `memo-uskladjenost-fiskalizacije.md:22` — a NACRT written as a standing rebuttal for a lawyer — still said the reset „prikazuje upozorenje o retencionom horizontu (10 godina; ZoRač čl. 28; ZPDV čl. 47 + ZPPPA čl. 114ž) i obavezi arhivske saglasnosti za d.o.o. prodavnice“, describing a dialog that had not existed for a day. `docs_guard::prose_sources` covered only two of the seven `docs/compliance/` templates. `MEMO` is now embedded, §4 prints the same constant the dialog and the tombstone do plus the čl. 9 st. 1 note, and a dated **Ispravka** in the memo's own voice records both withdrawals. The new guard `no_compliance_template_states_a_reset_warning_the_dialog_does_not_show` sweeps every embedded template.
6. **`PROGRESS.md`'s residual lists said both claims were still on screen.** Four blocks (`:121`, `:128`, `:175`, `:186`) — two of them in the *current* „Still open after this batch“ list — reported defects `41dc298` had closed. This is the exact failure the plan's preamble describes: on 07.08.2026 a stale register led a survey agent to rank two long-fixed bugs as pilot blockers. All four are stamped closed with what shipped, and `no_document_says_the_reset_dialog_still_carries_the_two_withdrawn_claims` now fails any block that pairs a `SettingsScreen.tsx` reference with a present-tense denial and either ZPDV or the archive-approval triple. It reads the markdown **block** (`markdown_block_around`), because these bullets wrap across four lines and the file reference, the denial and the citation land on three different ones.

**Deviations in the review fixes, four.**

1. **Three files outside the finding list are in the diff**, each because the fix made a sentence in them false: `docs/compliance/memo-uskladjenost-fiskalizacije.md` (finding 5), `docs/PROGRESS.md` (finding 6, plus `:378`, which said `docs_guard` embeds „both `docs/compliance/` templates“ — it is three now), and this plan file.
2. **The memo guard skips a line marked `Ispravka`.** A dated correction has to quote what was withdrawn to be worth anything, and judging it like a description of the live screen would leave the memo unable to show its work. The skip is documented at the constant and is the same scoping decision `no_document_says_the_cl_87_weekly_leg_is_still_unbuilt` makes for the čl. 88 st. 2 night leg.
3. **Req. 39's pravno-lice limb is recorded as open rather than built.** „State the real duties: lista kategorija with archive saglasnost, arhivska knjiga, 30 April prepis“ is profile-aware copy branching on `pravna_forma`; writing it inside a review fix would be a new feature. Both guards were verified to *permit* it (mutation F2/M4 above), and the residual is stated in `PROGRESS.md`'s req. 39 bullet so Task 4 does not close it.
4. **Four `docs/compliance/` templates are still unguarded** — `checklist-onboarding-pilota.md`, `pitanje-purs.md`, `runbook-povreda-podataka.md`, `ugovor-o-obradi-nacrt.md`. Only the memo was named by the review and only the memo was embedded. One thing seen while there and deliberately **not** changed: `checklist-onboarding-pilota.md:19` reads „(PDV obveznik: retencioni prag 10 godina…)“, which reads as though PDV status gates the floor — §1 row 6 rates that a HIGH defect. It is not this cycle's finding and is handed to Task 4.

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

- [x] **Step 1:** For each row, grep the named symbol and record what you found. **If the code does not support restating the row, leave the row alone and say so** — an over-corrected register is the defect this task exists to remove.
- [x] **Step 2:** Restate only the verified rows, in the register's existing voice, each with a `Re-stated 07.08.2026` stamp like the rows above them. State what ships **and what within that row still does not** — several of these are partials, not completions.
- [x] **Step 3:** Add the cycle's section to `docs/PROGRESS.md`: what shipped, the six gates with exact measured counts, the residuals, and the note that req. 12's weekly leg is now closed while reqs. 10, 14, 15, 16, 21, 26, 27, 28 of SW-14 remain open.
- [x] **Step 4:** Run `cargo test docs_guard` — it embeds both documents and will catch a row that now claims more than the code delivers. **Step 5: Full gates, commit.**

**Shipped.** All five candidate rows were re-verified against the code before anything was written, and
all five were re-stated — **none of them as a tick**. Each carries a `Re-stated 07.08.2026` stamp, what
ships with the symbol that ships it, and what within the row still does not. Row **6**: `NonFiscalBanner`
(`RegisterScreen.tsx:1289`) renders at `:922` **and** `:983` — above the stavke and below the ESIR nudge —
plus `ReceiptsScreen.tsx:632`, pinned by `RegisterScreen.test.tsx:471`/`:486` and
`ReceiptsScreen.test.tsx:263`; still open are SW-1's export limb, which has **no subject in this build**
(nothing the crate writes or prints itemizes a sale), and the ≥2× ratio, which is asserted by presence and
never measured. Row **17**: `kep.rs`, thirteen commands in `lib.rs:240–252`, `navigation.ts:55`; still open
is the per-prodajno-mesto book — `kep_entries` (v12) is partitioned by `book_year` alone. Row **18**:
`list_ledger` / `render_book_html`, `post_receipt_zaduzenje` at retail-with-PDV, the **fourteen** elements
in `derive_kalkulacija`, the exhaustive `kep_storno::posting_for`, `kep_status`'s T+1 list, and
`ensure_year_open` on all seven production insert paths (`kep.rs:84`/`:241`/`:358–359`,
`kep_kalkulacija.rs:110`, `kep_storno.rs:210`/`:270`); still open is that the čl. 18 lock is a domain gate
rather than a storage-layer trigger, and that nothing sweeps the book. Row **20**: `reset_trading_data`
(`backup.rs:483`) forces a snapshot at `:493`, takes `never_purge_row_counts` at `:510`, calls
`assert_never_purge_intact` at `:590` and tombstones with `ROK_CUVANJA_PRAVNI_OSNOV` (`:39`); still open
are the unfenced restore (by design), the non-existent prune path, the general `retain_until` engine, and
the second location. Row **24**: `price_history.rs` with NULL-as-offering-gap and the typed
`IncomputableReason`, `campaigns.rs`'s closed four-type enum and `snapshot_anchors`, and
`campaign_evidence.rs`'s three renderers wired at `lib.rs:220–223`; still open are the čl. 67 st. 1 tač. 8
penalty copy (absent from `legal.rs`) and a declared `RecordClass` for the offered-price log. The §2
preamble gained a dated revision note recording the sweep and why the understatement direction is the more
dangerous half. `docs/PROGRESS.md` gained the cycle's section: Tasks 1–3, the six gates with measured
counts, the SW-14 requirement-by-requirement status, the night legs as a decision, and seven residuals.

**Deviations, four, all recorded rather than hidden.**

1. **The brief listed SW-14 req. 27 as open and it is not — it closed 02.08.2026.** `cl47.rs` ships the
   čl. 47 evidencija radnji obrade as a generated artefact, driving the st. 1 t. 6 rok per vrsta podataka
   out of `retention_policies`; register row 9 has said *„Re-stated 02.08.2026 — now generated“* since
   then. Writing it into `PROGRESS.md` as open would have been the exact defect this task exists to
   remove, so the section states it as **closed** and says in terms that it appears there only because the
   brief carried it in as open. Of the eight requirements named, **10, 14, 15, 16 and 28 are wholly open**
   (each verified by grep: `kolektivni_ugovor` has zero occurrences, no čl. 62 st. 2 threshold is computed
   over `nocni_minuta`, čl. 64/66/67 are cited nowhere in `worktime.rs`, there is no praznik calendar or
   čl. 108 exclusion set, and no masking exists), and **21 and 26 are partials** — req. 21's CSV ships and
   its printable monthly sheet and XLSX do not; req. 26's notice was re-issued and the activation gate on
   a recorded acknowledgement does not exist.
2. **Register rows 16 and 21 were read in the same pass and deliberately left alone**, per Step 1's own
   instruction. Row 21's residual is already current. Row 16 (*„Gap (no goods-receipt documents)“*) is the
   one this pass could **not** settle: `kep_kalkulacija.rs` does generate a goods-receipt document, but ZoT
   čl. 29 st. 1 is a duty to *possess the supplier's isprava* and `kalkulacije` (v13) carries no adresa, no
   matični broj/BPG and no supplier document broj/datum — so the denial understates the code while a tick
   would overstate it, and re-stating it needs a field-by-field čl. 29 read this task did not perform. The
   §2 revision note records that decision and its reason rather than leaving the row silently skipped.
3. **The plan's Global Constraints say `all_notices` stays 9; it is in fact 13** (`legal.rs:502` —
   SW-12 bumped it to 10 and SW-16 to 11, and `reklamacija_breach` counts twice). The constraint's *intent*
   was satisfied exactly: **`legal.rs` is not in this task's diff at all**, no fine figure or currency
   amount was added anywhere, and no ZEOR figure appears. The stale figure is recorded here so the next
   plan does not copy it forward.
4. **Two mutations were run against the finished documents to prove the guards are not passing vacuously**,
   and both were reverted immediately with the diff checked back to zero. Planting *„is not enforced“*
   beside „35 h/week“ in the new `PROGRESS.md` section failed
   `no_document_says_the_cl_87_weekly_leg_is_still_unbuilt` at `docs/PROGRESS.md:1294`; replacing row 20's
   *„the `pre_restore` safety copy is the protection on that path“* with an immunity claim failed
   `no_trajno_claim_promises_immunity_from_restore_or_backup_pruning` at
   `docs/SERBIAN-LAW-COMPLIANCE.md:77`. The second is the more useful of the two: it proves the restated
   row sits **inside** a guarded class rather than beside one, and that its caveats are load-bearing.

**Gates measured at this commit** (`DEVELOPER_DIR=/Library/Developer/CommandLineTools` prefixed, per Task
1's environment note): `bun run test` **545 passed / 35 files** (was 539), `bun run build` clean bar the
pre-existing chunk-size advisory, `cargo test` **1007 passed / 0 failed** (was 988), `docs_guard` alone
**21 passed**, clippy clean, `cargo fmt --check` clean, `git diff --check` clean. Net **+19 cargo / +6
bun** over `c93e28e` for the whole cycle. **This task added no test and changed no Rust** — it is a
documentation task, and inventing a guard to look busy is what deviation 4 was run instead of.

**Review fixes shipped (08.08.2026, dated 07.08.2026 in the documents with the rest of the cycle).**
Seven findings, seven defects, and the shape of them is worth stating: the task whose whole subject is
that a stale denial is as bad as a false promise shipped a false tick, a false status header, an
unswept §3, a cell it had itself proved false, a missing limb and five unguarded denials. cargo
**1012 passed / 0 failed** (1007 + 5), `docs_guard` alone **25** (was 21), `bun run test` **546 passed
/ 35 files** (was 545), `bun run build` clean bar the pre-existing chunk advisory, clippy,
`cargo fmt --check` and `git diff --check` clean. Net **+24 cargo / +7 bun** over `c93e28e` for the
cycle. **This task now does change Rust and does add tests, which reverses deviation 4** — the finding
that mattered most was that five new denials went into the inspector-facing register with nothing
behind any of them, and two of them were decidable in one line each against lists the crate already
exposes exhaustively.

1. **Row 6's ✓ stood over a measurably unmet limb, and the limb was closed rather than recorded.** The
   cell said the ≥2× ratio is *„asserted by presence, never measured“*; it is written in the class
   names, and the receipt detail banner was `text-xl` (1.25rem) over a `<Table>` root of `text-xs`
   (0.75rem) — **1.6×**. „Nobody measured it“ and „it falls short“ are different claims and only the
   second was true. `ReceiptsScreen.tsx`'s banner is now `text-2xl`, so both surfaces are exactly 2.0×
   of the line item, and `docs_guard::the_non_fiscal_banner_is_at_least_twice_the_line_item_font`
   computes both ratios from the class names **in integers** (milli-rem — a comparison that decides a
   compliance claim by a rounding mode is the house rule's defect wherever it sits) and requires row 6
   to state what it computes, so neither the classes nor the sentence moves alone. Red was real rather
   than mutated. `ReceiptsScreen.test.tsx` pins the class on the rendered element and was verified red
   against `text-xl`. The row's bolded lead also contradicted its own body four sentences later and now
   names the one limb that is genuinely open. **Correction to the finding as written:** it computed the
   dialog at 1.5×, on the premise that the stavka rows inherit the Tailwind base 1rem. They do not —
   `DialogContent` carries `text-xs/relaxed` (`src/components/ui/dialog.tsx:54`), so the dialog was
   already at exactly 2.0× and the receipt detail was the only short surface.
2. **`docs/PROGRESS.md` recorded req. 12 as CLOSED four lines above its own „one limb is still open“.**
   Restated as a PARTIAL in reqs. 21/26's voice, cross-reference repointed at the night-legs paragraph
   rather than req. 14's bullet. Pinned by
   `docs_guard::no_document_records_sw_14_req_12_as_closed_while_the_night_leg_is_unbuilt`, bound
   exhaustively to `worktime::ProtectionKind`, and scoped to blocks arguing about čl. 87 or čl. 88
   because SW-12's *own* requirement 12 is the cenovnik duty and its register row opens „✅ SHIPPED“.
3. **§3 was never swept.** SW-8 said the app prints nothing while row 18 credited the KEP mechanics to
   the SW-8 printing stack in the same commit; SW-9 still specified 13 kalkulacija elements. SW-1,
   SW-3, SW-6, SW-8 and SW-9 now carry dated stamps pointing at their §2 rows, and
   `docs_guard::no_document_says_this_application_prints_nothing` bars the class of sentence, bound to
   four renderers. It is narrow on purpose: §1's *„prints nothing **receipt-like**“* is true and
   load-bearing, and a **module** with no print path is a useful thing to write.
4. **Row 16 was re-stated.** The first pass wrote the correction into the §2 note and left the false
   cell in the column an inspector reads; the note's own sentence now sits in the cell, and the čl. 29
   field-by-field read is still recorded as owed.
5. **Row 24 gained its third open limb** — `price_history` (v9) has no prodajno-mesto column, though
   §3's SW-6 line specifies `(sku, prodajno_mesto)` and the row's duty is a display at the prodajno
   mesto. Exactly the limb row 17 was scrupulous about for the KEP.
6. **The five new denials are now pinned.**
   `docs_guard::no_register_row_denies_a_retention_class_the_crate_now_declares` clears rows 18 and 24
   against `retention::RecordClass::ALL` two ways — an exhaustive `match` (a new variant is a compile
   error and a decision) and a sweep over `key()` (a variant answered carelessly is still caught) —
   proven red by renaming `CenovnikArchive`'s key. `legal::tests::no_register_row_denies_a_snizenje_notice_this_module_carries`
   does the same for the *„penalty copy does not exist“* denial against `all_notices`; it lives in
   `legal.rs` because that list is private to its test module by design, and reads the register through
   `docs_guard::REGISTER`, now `pub(crate)`, so one copy is embedded and not two. Proven red by
   re-citing `declaration_defective` at čl. 67 st. 1 tač. 8.

**Deviations in the review fixes, four.**

1. **`src/app/ReceiptsScreen.tsx` and `src/app/ReceiptsScreen.test.tsx` are in the diff**, although Task
   4's file list is two markdown documents. The finding offered recording the gap or closing it; closing
   it is a one-class change that removes a real shortfall from a shipping screen, and recording it would
   have left the app knowingly short of the Pravilnik with the fix already written in the failure
   message.
2. **`markdown_block_around` now treats a numbered item as a block start.** Without it a seven-item
   review list collapses into one block and a guard reports the whole list as the offending text —
   the over-wide window `sentence_around`'s doc comment describes, one structure up. It changes the
   scoping of one pre-existing guard as well, and the full suite is green.
3. **Both new prose guards exempt a claim inside a Serbian quotation** (`inside_a_serbian_quotation`).
   `docs/PROGRESS.md` is this repository's record of what was true when, and a dated correction that
   cannot quote the sentence it withdraws is worth nothing — the same scoping decision the memo guard
   makes for a line marked `Ispravka`, expressed as punctuation instead of a keyword. Both guards were
   re-verified red **after** the exemption landed, against the real withdrawn sentences.
4. **The SW-8 baseline sentence is quoted in `docs/PROGRESS.md` and not in the register.** That is the
   register's own convention — *„the denial is restated and not annotated“* — and it is also what the
   new guard requires, since the register carries no quotation exemption it would need to invoke.

---

## Whole-branch review fixes (08.08.2026)

Eight findings, seven distinct defects, each closed with a regression test whose red was proven by
mutation before the fix landed. cargo **1016 passed / 0 failed** (1012 + 4), `bun run test` **547
passed / 35 files** (546 + 1), `bun run build` clean bar the pre-existing chunk advisory, clippy,
`cargo fmt --check` and `git diff --check` clean. Net **+28 cargo / +8 bun** over `c93e28e` for the
cycle. Migration head **v22**, unchanged; `legal.rs` is not in this diff and no fine figure, currency
amount or ZEOR figure was added anywhere.

**The shape of these findings is worth stating.** Three of them are tests that could not fail for the
reason their own failure message gives — a mutation-proof discipline applied to the *behaviour* and not
to the *test*, twice over, in a cycle whose subject is claims nothing stands behind.

1. **The de-duplication test passed with the de-duplication removed.**
   `the_stored_row_for_the_day_under_assessment_is_not_double_counted` drove a correction that *lowers*
   the day, and the raise-gate that landed in Task 1's review makes `unos_minuta > evidentirano_za_dan`
   false by construction for every lowering write — so deleting `d.dan != day` pushed the sum over the
   cap and no block was pushed anyway. The test had silently become a behavioural duplicate of
   `a_correction_that_lowers_a_minors_week_is_not_refused` (`worktime.rs:1112`), which pins the
   raise-gate on purpose and still does. **Re-pointed at the only shape that separates the two
   implementations** — Mon–Thu at 8 h with a 2 h Friday corrected **up** to 3 h: 35 h exactly when the
   superseded row is dropped, 37 h and a refused lawful ispravka when it is not. Verified red by
   deleting `d.dan != day &&` at `worktime.rs:451`. **Deviation:** the finding offered keeping the old
   lowering case under a truthful name; it was folded into its existing duplicate instead, and the
   test's doc comment records why in full. No coverage was lost — the lowering shape is asserted by
   `a_correction_that_lowers_a_minors_week_is_not_refused` and, end to end, by
   `a_birth_date_filled_in_later_does_not_lock_a_minors_recorded_week`.
2. **`strictly_in_same_iso_week`'s conjunct could be deleted with the whole suite green.** The plan's
   Task 1 review note disclosed the overlap with the age filter and understated it: `is_younger_than`
   also answers `false` for a `dan` it cannot read, so the two were **provably redundant** and
   `an_unreadable_stored_day_does_not_silently_refuse_a_minors_week` passed for the age filter's
   reason. The age conjunct now asks the positive question — a new private `is_at_least`
   (`worktime.rs:606`), negated: *drop the day only when we know the employee had turned 18* — so an
   unreadable row reaches the week predicate and is dropped there, once, by the helper whose documented
   job it is. **Behaviour is identical** on every readable pair and on the unreadable one; what changed
   is that each conjunct now answers one question. Both were verified red by deletion
   (`an_unreadable_stored_day_…` for the week predicate,
   `the_cl_87_weekly_total_counts_only_the_days_the_employee_was_under_eighteen` for the age one), and
   `the_two_age_predicates_are_not_complements_on_a_day_that_does_not_parse` asserts the property the
   split rests on, because a reviewer meeting `!is_at_least(..)` will reasonably want to simplify it
   back. This is option (b) of the finding; option (a) — documenting the redundancy as defensive — was
   rejected because the finding's own acceptance test (delete the conjunct, require red) is
   unsatisfiable under it.
3. **`load_week`'s live-rows-only contract had no test at any layer.** `check_protection`'s doc comment
   states it as a precondition in terms; the `MAX(verzija)` subquery was written for `assess_caps`,
   where a superseded row costs an override prompt, and this cycle made it load-bearing for a hard
   refusal without adding anything.
   `commands::worktime::tests::a_superseded_verzija_does_not_feed_the_cl_87_weekly_total`
   (`commands/worktime.rs:2269`) records a 35 h week, corrects the Friday **down** to one hour and then
   saves a two-hour Saturday that lands on exactly the cap. Red proven by dropping the subquery from
   `load_week`'s SQL: the Saturday is refused at 36 h, and `correct_entry` cannot rescue it because
   there is no version to supersede. It was the only test in the suite that moved.
4. **A čl. 87 refusal still survived an attempt that never left the screen.** Task 2's review fix drops
   the assessment at the top of `WorkTimeModule`'s catch; `submitEntry` has a **second** exit above it,
   where `toRequest` returns early on a client-side validation failure setting `saveError` and clearing
   nothing. Clearing the Datum field after a refused Saturday left the destructive „Unos nije dozvoljen
   … 35 časova nedeljno … već je evidentirano 35 č 00 min“ standing over „Dan nije evidentiran — Datum
   mora biti u obliku gggg-MM-dd“, with nothing submitted at all. The drop moved above `toRequest`
   (`WorkTimeModule.tsx:375`); the catch keeps its own for the one case the move does not cover — a
   throw after the success path has already applied a saved day's findings — and both comments now say
   which case each covers. Vitest: „does not let a čl. 87 refusal survive an attempt that never leaves
   the screen“, red before the fix on the surviving alert, and it asserts `saveEntry` was called once.
5. **The over-cap dead end is recorded rather than emergent.** In a week already above 35 h no further
   worked day can be recorded at any value — `evidentirano_za_dan` is 0 for a day with no stored row,
   so the raise-gate collapses to `unos_minuta > 0`, and `correct_entry` returns `not_found`. **The
   behaviour was deliberately not changed**, which is option (b) of the finding: option (a) — refuse
   only the write that *crosses* the cap — inverts
   `worktime::tests::raising_a_day_in_an_already_over_week_is_still_refused`, which bars the „once
   over, anything goes“ reading on purpose, so taking it would have meant deleting a test this plan
   does not name and deciding a čl. 274 disclosure alongside. What shipped is
   `commands::worktime::tests::an_over_cap_minors_week_admits_no_further_worked_day`, driving the
   finding's exact scenario (five 8-hour days with no birth date, backfill, then the Saturday at 480,
   60 and 1 minute, then the ispravka, then the absence that does record), plus the residual in
   `docs/PROGRESS.md` and a dated paragraph in register row SW-14.
6. **Two `docs/PROGRESS.md` bullets still denied the retention schema in the present tense.** Both read
   *„No `retention_class`, `retain_until`, `legal_hold` or upward-only extension exists anywhere in the
   schema“* while v17 (`db/migrations.rs:734`) declares all four columns, `retention.rs` is the shared
   table req. 42 mandates with nine classes, v18's `processing_activities` carries a
   `retention_record_class` foreign key onto it, and `retention::extend_retain_until` **refuses** a
   shortening rather than clamping it. `2c415b8` edited exactly these blocks and left the denial
   standing. Both are re-stated with what shipped and what within req. 36 is still owed, in register
   row 20's own words, and reqs. 37 and 43 are kept open because they genuinely are. Pinned by
   `docs_guard::no_document_denies_the_retention_schema_v17_created`, bound to the crate three ways —
   the four needles are `retention::RetentionPolicy`'s fields, destructured so a rename is a compile
   error, and `RecordClass::ALL` and `extend_retain_until` are referenced as items.
7. **`no_document_says_this_application_prints_nothing` was defeated by any following word, and by
   capitalisation.** `ostatak.starts_with(' ')` exempted every following word rather than the narrowing
   ones, so *„prints nothing today“*, *„prints nothing whatsoever“* and *„ne štampa ništa danas“* all
   passed, and the stale sentence the guard exists for was caught only because a `)` happened to follow
   it. The exemption is now a two-word whitelist — „receipt-like“ (§1) and „nalik“ (the memo) — the
   failure message names the word it found, and matching moved to `match_indices_ci`. The sibling
   `UNBUILT` list had the same case bug in `"Gap"` and is lower-cased and matched the same way. All
   four holes were verified red by planting the sentence and reverting.

**Deviations, four, all recorded rather than hidden.**

1. **`docs_guard::sentence_span` was extracted from `sentence_around`.** The new retention guard has to
   decide *where inside a window* a second needle sits, which a `&str` it can no longer locate cannot
   answer. `sentence_around` now calls it and is otherwise unchanged.
2. **The retention guard does not judge a sentence, and the first version that did was wrong.** It
   failed on its own dated correction, because markdown prose ends sentences on a backtick or an
   asterisk and [`ends_a_sentence`]'s capital-letter lookahead then runs the window across half a
   section, swallowing the *true* denials three clauses away („Genuinely unbuilt: the čl. 32 objekti
   register“). What is judged is the word immediately before the column name and the clause
   immediately after it. Register row 20's *„no general upward-only `retain_until` engine over trading
   data“* is green under it, which is the point.
3. **`worktime::is_at_least` is a new private helper**, and the plan's Task 1 said the weekly leg is
   four fields' worth of arithmetic beside an existing one. It is six lines and it exists so that a
   deletion is detectable; the alternative was a doc comment saying the guard cannot be tested.
4. **`docs/PROGRESS.md`'s gate table and net counts were re-stated** (1012 → 1016 cargo, 546 → 547
   bun), and the cycle section gained a review-fix paragraph. Register row SW-14 gained the dead-end
   paragraph. No other document moved.

**One thing not closed.** The two čl. 87 legs still count `efektivno_minuta + prekovremeni_minuta` and
not the ZEOR čl. 24 tač. 1 b) total; that question is unresolved, disclosed in the poruka and in
`check_protection`'s doc comment, and stays the first residual of this cycle.

---

## Self-review

**Spec coverage.** SW-14 §4 req. 12 weekly leg → Tasks 1 and 2 (Task 1 the rule, Task 2 the write path, because a computed-and-discarded guard is the failure mode this codebase names by hand). SW11-SW15 §3 req. 39 / §4 item 8 → Task 3. The §2 Q4 retention citation → Task 3. Register accuracy → Task 4.

**Placeholders.** Task 1 carries all five tests in full. Tasks 2–4 name required behaviours and exact strings rather than pasting bodies, following the repo's established seeding pattern; every behaviour is a concrete assertion against a named symbol or a quoted string.

**Type consistency.** `MINOR_WEEKLY_CAP_MINUTES`, `ProtectionKind::MaloletanNedeljniLimit` and the four-argument `check_protection` are defined in Task 1 and consumed unchanged in Task 2. `DayHours` and `in_same_iso_week` are existing and unmodified. No new wire type, so the frontend contract is untouched except for the two strings in Task 3.

**Two gaps found during review, both folded in.** Task 1 originally left `docs_guard::no_document_claims_the_cl_87_weekly_leg_is_enforced` deleted with nothing in its place, which would have removed the only thing standing between this cycle and the *next* stale claim about čl. 87 — it is now inverted rather than dropped. And Task 4 originally said „restate rows 6, 17, 18, 20, 24“ on the strength of one survey agent's spot-check; it now requires the implementer to re-verify each row against the code and to leave any unverified row alone, because restating a row on a bad survey is the same defect pointed the other way.

**One thing this plan deliberately does not do.** It does not touch the čl. 87 **night-work** leg (čl. 88 st. 2) or the čl. 62 st. 2 night threshold (SW-14 req. 14). `nocni_minuta` is an operator-entered advisory bucket that `DayHours` does not carry, so the night legs of čl. 90 and čl. 91 stay unenforced and must remain recorded as open in Task 4's residual list.
