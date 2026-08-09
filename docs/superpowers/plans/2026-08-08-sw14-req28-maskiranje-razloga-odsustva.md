# SW-14 req. 28 — mask the absence reason from remote support, and log the unmask

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop `kategorija_odsustva` — a ZZPL čl. 17 posebna vrsta podataka in all but name — from reaching a remote-support operator working under a čl. 46 nalog, unless the shop unmasks it for that session, and record the unmask in the čl. 48 log.

**Architecture:** The masking happens **backend-side**, in the read paths, keyed off SW-10's live support nalog. Today's `canSeeAbsenceReason` is a frontend boolean (`WorkTimeModule.tsx:255`, `isPayrollRole`), so the reason is already serialised to any client that asks and the screen merely declines to draw it — masking by CSS over data the wire already carried. The unmask is a property of the **nalog**, not of the log, so it is a column on `support_sessions` (migration v23) rather than a state derived by reading `audit_events` back: SW-10 deliberately keeps that log a record and not an access control, and reading it is deliberately not logged.

**Tech Stack:** Rust (rusqlite, serde) + Tauri v2; React 19 + TypeScript + shadcn/ui + vitest; bun.

## Global Constraints

- **Time in whole minutes; money in minor units; quantities milli-units.** Never floating point.
- **Timestamps RFC3339 passed as a `now: &str` param.** Never `datetime('now')` in decision code.
- **Serbian Latin diacritics** in every operator string; Serbian quotes open `„` and close `“`.
- **Migrations APPEND-ONLY.** Head is **v22**, so this is **v23 and v23 only**. Never edit v1–v22.
- **No fine figure or currency amount outside `legal.rs`.** This cycle adds no notice. Note for the record: `legal.rs::all_notices` is **13**, not the 9 an earlier plan stated.
- **No ZEOR figure anywhere** — §6 W-1 unresolved.
- **The absence reason must never enter `audit_events`.** `audit.rs::reject_forbidden_content` is the čl. 5 st. 1 t. 3 write boundary; the unmask line records **that** the reason was revealed and for which session, never a category value. A log that records the secret it exists to protect is worse than no log.
- **A document, a generated artefact or an operator string must never assert behaviour the code lacks, and must never deny behaviour the code has.**
- **Never boot the app.** Verify only via the gates.
- Commit trailer: `Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>`

**Gates (six).** `--test-threads=1` is retired; the cargo gate is plain `cargo test`.

```
bun run test
bun run build
cargo test --manifest-path src-tauri/Cargo.toml
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
git diff --check
```

**Governing law:** [SW14-VERIFIED-RULES.md](../../SW14-VERIFIED-RULES.md) §4 req. 28 verbatim — *„Remote support must **not** see the absence reason by default: mask the column and payroll screens unless the shop explicitly unmasks for that session, and **log the unmask** (feeds SW-10).“* Also §4 req. 24 (the čl. 12 st. 1 tač. 3 / čl. 17 st. 2 tač. 2 bases) and §6 **W-15**, which this feature sharpens: it is what first puts čl. 17-adjacent data on the machine the vendor reaches.

**Baseline:** cargo **1016** passed, bun **547** passed / 35 files, all six green, migration **v22**, HEAD `e7c79a3`.

---

## File structure

**Modified Rust:** `db/migrations.rs` + `db/mod.rs` (v23), `commands/audit.rs` (the unmask verb), `commands/worktime.rs` (the three read paths), `audit.rs` (object-type vocabulary if a new variant is needed).
**Modified frontend:** `src/app/worktime/WorkTimeModule.tsx`, `src/app/worktime/MyHoursPanel.tsx`, `src/app/privacy/` (the unmask control belongs beside Daljinska podrška), `src/services/{types,ports,local-adapter,mock-adapter}.ts`.
**Modified docs:** `docs/PROGRESS.md`, `docs/SERBIAN-LAW-COMPLIANCE.md`, `docs/compliance/obavestenje-zaposlenima.md` (the čl. 23 notice must say the masking exists), `docs/compliance/evidencija-obrade-cl47.md` and/or `cl47.rs` if the register describes this processing.

---

### Task 1: Migration v23 — the unmask stamp on the nalog

`support_sessions` gains `odsustvo_otkriveno_at TEXT` (nullable, non-empty CHECK) — when the shop revealed the absence reason for that session. Nullable because the default is masked and most sessions never unmask.

**There is no re-mask verb and no boolean.** A stamp records an irreversible disclosure: once the support operator has seen the column, flipping a flag back does not unsee it, and a boolean would let the UI claim it did. The stamp also expires with the session, because the nalog is already hard-bounded at 24 h.

- [x] **Step 1: Write the failing tests** — the column exists and is nullable; the non-empty CHECK refuses `''`; a **v22-seeded survival test** applying `MIGRATIONS[..22]` to a raw `Connection`, seeding a `support_sessions` row, `drop(conn)`, then `Db::new`, asserting the seeded row survives verbatim and reads back with `odsustvo_otkriveno_at` NULL. A test that seeds after `Db::new` proves nothing — mirror the v21/v22 survival tests.
- [x] **Steps 2–5:** run red, implement, run green, full cargo suite once, commit.

**Shipped.** v23 `support_session_odsustvo_unmask_stamp`, one `ALTER TABLE support_sessions ADD COLUMN odsustvo_otkriveno_at TEXT CHECK (odsustvo_otkriveno_at IS NULL OR odsustvo_otkriveno_at <> '')`. Nullable, never backfilled, no re-mask verb and no boolean — the migration's own comment carries the reasoning: an operator who has read `kategorija_odsustva` does not unread it, so a flag that could go back to `false` would let the surface above the table claim a disclosure was undone. The stamp needs no expiry, because v18's `expires_at` already bounds the nalog at 24 h. Three tests: `db::migrations::tests::migration_v23_adds_the_absence_reason_unmask_stamp` (column present, default NULL, `''` refused, an RFC3339 instant accepted), `db::migrations::tests::migration_v23_preserves_pre_existing_rows` (the v22-seeded survival test the plan specifies) and `commands::audit::tests::a_nalog_reports_whether_the_absence_reason_was_unmasked_under_it`.

**The red state.** The two migration tests failed on `no such column: odsustvo_otkriveno_at` — the survival test reaching that read only after applying `MIGRATIONS[..22]` and seeding through the raw connection, which is the half that could have silently no-opped. The audit test failed to compile, `E0609: no field odsustvo_otkriveno_at on type SupportSession`. Afterwards the CHECK was dropped on its own to confirm it is load-bearing: both migration tests trip, on the fresh database and on the upgraded one.

**Deviation 1 — the migration-count pin moved from 22 to 23.** `db::tests::migrate_is_idempotent_and_records_initial_migration_once` (`db/mod.rs:867`) asserts the head literally, and the full suite caught it. The pin follows the head; nothing was weakened. No change to `CORE_TABLES` or `EXPLICIT_INDEXES` — v23 adds no table and no index.

**Deviation 2 — `SupportSession` gained its field here rather than in Task 2**, as the self-review anticipated: `commands/audit.rs` struct + `SESSION_COLUMNS` + `read_session`. Without it the stamp would be a column no read path names, and Task 3 needs the live nalog to answer the masking question. **`src/services/types.ts::SupportSession` does NOT yet name `odsustvoOtkrivenoAt`** — that file is Task 4's, TypeScript interfaces are structural so the extra JSON field breaks nothing, but the mirror is one field behind until Task 4 closes it, and its doc comment claims to be `crate::commands::audit::SupportSession`.

**Deviation 3 — the survival test asserts more than the plan's minimum.** It also seeds a `work_time_entries` absence row (`sprecenost_rfzo`, 480 minutes) and asserts it survives verbatim — v23 masks a READ, it deletes no column and rewrites no absence — and it re-asserts, on the upgraded database, both the new non-empty CHECK and v18's `CHECK (expires_at > granted_at)`, so a silent table rebuild would show.

---

### Task 2: The unmask verb, and the audit line that must not carry the secret

**Interfaces:** `support_reveal_absence_reason(state, now) -> SupportSession`, admin-gated **inside the domain function**.

Rules:
- Refuses when there is **no live session** — unmasking nothing is not a thing the shop can consent to, and a stamp with no nalog behind it is a record of an event that did not happen.
- Idempotent on an already-unmasked session: return the session unchanged rather than re-stamping, so the log carries one disclosure per nalog and the first (earliest) instant survives.
- Writes **one** `audit_events` row in the **same transaction** as the stamp, using the SW-10 write door `record_audit`. The object is the **support session** (`AuditObjectType::SupportSession`), the object id is the session id. **No category value, no employee name, no month, no employee id** — `reject_forbidden_content` is the boundary and the test must prove the line passes it.
- `now` is a parameter; the clock is read only at the `#[tauri::command]` boundary.

- [x] **Step 1: Write the failing tests** — unmasking with no live session is refused with a typed error; unmasking stamps the column and writes exactly one audit row; the audit row names the session and carries no `kategorija_odsustva` value (assert every one of the ten v17 category strings is absent from the serialised row); a second unmask does not re-stamp and does not write a second row; a cashier is refused; the chain still verifies afterwards.
- [x] **Steps 2–5.**

**Shipped.** `commands::audit::reveal_absence_reason(state, now) -> SupportSession` (`commands/audit.rs:382`), admin-gated by `require_admin` on its first line — inside the domain function, not in the wrapper — with the thin `#[tauri::command] support_reveal_absence_reason` above it (`:132`) reading the clock and registered in `lib.rs:313`. One transaction carries both the `UPDATE support_sessions SET odsustvo_otkriveno_at` and the `append_audit_event` line, so a stamp can never exist without its record or a record without its stamp. Five tests: `the_absence_reason_cannot_be_unmasked_without_a_live_nalog`, `unmasking_stamps_the_nalog_and_logs_exactly_one_line`, `the_unmask_line_carries_no_category_no_name_and_no_month`, `a_second_unmask_neither_re_stamps_nor_writes_a_second_line`, `a_cashier_cannot_unmask_the_absence_reason`.

**The line.** `otkrivanje` / `support_session` / `object_id` = the session id / razlog `tehnicka_podrska` / primalac `obradjivac_tehnicke_podrske` / `actor_user_id` = the vlasnik / `support_session_id` = the nalog. It passes `reject_forbidden_content` for the right reason on both halves: the čl. 48 st. 2 half because an unmask genuinely *is* a disclosure to the obrađivač with a real razlog, not because two fields were filled to get past a CHECK; the čl. 5 st. 1 t. 3 half because `AuditDraft` has no field a value payload could sit in and the one free-form field holds a bare integer. The actor differs from `request_access`'s deliberately: the entry line has `None`, because the support engineer holds no account, whereas the unmask is the **vlasnik's** decision and the log must say so.

**The red state, and both mutations.** The five tests failed to compile — `E0425: cannot find function reveal_absence_reason`, plus `E0603` on the private `KATEGORIJE_ODSUSTVA`. Afterwards two mutations proved the two assertions that could most easily have been vacuous. (1) Setting the unmask line's `object_id` to `"2026-07"` — the register's month — leaves `reject_forbidden_content` **satisfied**: `2026-07` is a valid hyphen-grouped internal id, no digit run reaches nine, so the write boundary passes it and only `the_unmask_line_carries_no_category_no_name_and_no_month` catches it. That is why the seeded absence sits in July while the unmask happens in August: the čl. 48 st. 2 instant legitimately carries an August date, so „no month of the register“ is asserted against a month the timestamp cannot supply by accident. (2) Removing the idempotency guard trips `a_second_unmask_neither_re_stamps_nor_writes_a_second_line` on the stamp moving from 09:20 to 09:40.

**Deviation 1 — `append_audit_event`, not `record_audit`.** The plan named `record_audit` as the write door, but that function opens its **own** connection and transaction, and this task requires the stamp and the line to commit together. The module's own doc comment already resolves this: „a caller that DOES own one … calls `append_audit_event` with its own `Connection` instead. Both go through the same write boundary; there is no third way in.“ `grant_access`, `request_access` and `end_session` all do exactly this. No boundary was bypassed.

**Deviation 2 — the command list pin had to be restated, and was tightened rather than loosened.** `no_audit_command_can_edit_or_delete_a_logged_row` (`commands/audit.rs:2366`) asserts the `commands::audit::` registrations in `lib.rs` literally, and the full suite caught the new verb. The list now names it, with a comment recording why it does not weaken the req. 7 claim — it APPENDS a line and updates `support_sessions`, never a logged row — and a second assertion was added: exactly **one** registered command may mention `absence_reason`, so a future re-mask verb cannot be slipped in beside it by updating the list. The needle is `absence_reason` rather than `mask`, which would have false-fired on any rename to `unmask`.

**Deviation 3 — `KATEGORIJE_ODSUSTVA` became `pub(crate)`** (`commands/worktime.rs:69`), a one-token visibility change plus a comment. The house rule requires asserting all ten v17 category strings are absent from the written row; a hand-copied list in the audit test would silently stop covering an eleventh category the day one is added. The constant is not yet pinned to v17's own CHECK by any test — `migration_v17_derives_every_absence_bucket_from_its_category` parses the schema, not this constant — so **constant-vs-schema drift remains unguarded**, which is worktime's question and not this task's.

**Refusal codes.** `support_bez_naloga_za_otkrivanje` is new and distinct from `request_access`'s `support_bez_naloga`, so Task 4 can say the right sentence; a cashier gets the ordinary `forbidden` from `require_admin`. Both refusals leave the stamp NULL and write no line. **Nothing is wired to the frontend yet** — `src/services/*` and the mock adapter are Task 4's, and until then no surface can reach this verb.

**Shipped — review findings closed (four fixes, four new tests).** Review found the first shape of the line indefensible in three ways and the write boundary short of the guarantee this task claims for it.

1. **The unmask line was indistinguishable from the čl. 46 entry line.** Both carried `otkrivanje` / `support_session` / the same session id / `tehnicka_podrska` / `obradjivac_tehnicke_podrske`, so the čl. 48 st. 4 izvod showed two rows minutes apart that differ in nothing an operator reads. „Log the unmask“ is not discharged by a row that cannot be read AS the unmask; the only discriminator was a NULL-vs-set actor, a convention written down nowhere and one that collapses the day any other otkrivanje under a nalog acquires an actor. Fixed by a new closed-vocabulary variant `AuditObjectType::SupportAbsenceReason` (`audit.rs:217`, code `support_absence_reason`, prose „Razlog odsustva u evidenciji radnog vremena“ in `object_type_label`), object id still the session id. Naming the FIELD CLASS costs nothing under čl. 5 st. 1 t. 3 — it is a code constant carrying no employee, no month and no category value — and v18 pins no vocabulary for that column (its CHECK is shape-only, `migrations.rs:802`), so this is a code change and not a migration. `unmasking_stamps_the_nalog_and_logs_exactly_one_line` now asserts the two rows differ in a field other than the actor, and the new `the_izvod_tells_the_unmask_apart_from_the_support_entry` asserts the difference survives into the exported document, compared on the „Vrsta objekta“ column rather than on whole lines — the two hashes always differ, so a line-level `assert_ne!` would have passed on a pair of rows nobody can tell apart. `every_stored_code_renders_as_serbian_prose` moved 37 → 38 codes.

2. **The write boundary never looked at `at`, and two doc comments denied it.** `reject_forbidden_content` checked the razlog, the primalac and `object_id`'s shape; `at` is a plain `String`, v18 declares the column `TEXT NOT NULL` with no CHECK at all, and the INSERT bound it verbatim — so `at: "2026-08-01T09:20:00Z#sprecenost_rfzo"` reached `audit_events` and got hashed into the chain, which is precisely what the constraint above forbids. Only the ten-category loop caught it, and only for this one call site. Now the boundary parses `at` as RFC3339 (`audit.rs:657`) and refuses anything else with `audit_forbidden_content` / `{"polje":"at","oblik":"nije_trenutak"}`, echoing no value. Two tests at the boundary: `an_at_that_is_not_an_instant_is_refused` (six shapes, including the one above, plus the refusal-must-not-leak assertions) and `a_real_instant_passes_the_boundary` (Z, fractional, and a +02:00 offset — a boundary that refused `utc_now()` would take the log down with it). **Mutation:** setting the unmask draft to `at: format!("{now}#sprecenost_rfzo")` now fails at `append_audit_event` with `audit_forbidden_content` and rolls the transaction back, where before it wrote the row. The `AuditDraft` and `reject_forbidden_content` doc comments were rewritten to name **two** free-form fields and say what constrains each; the „only free-form field is `object_id`“ claim is gone from `reveal_absence_reason` too.

3. **An unmask taken before the operator entered logged an `otkrivanje` to a primalac that received nothing.** Reachable entirely through shipped paths: grant 09:00 → reveal 09:05 → `end_session` 09:10, which takes the REVOKE branch because `started_at` is still NULL — a nalog nobody ever used, with a čl. 48 st. 2 disclosure row permanently against it. The module already draws this line elsewhere: `grant_access` logs the nalog as an `unos` with no razlog and no primalac because „the nalog itself is not an access to anyone's data“, and `request_access` is what logs the otkrivanje. So the unmask now follows the same rule (`commands/audit.rs:425`): `started_at.is_some()` → `Otkrivanje` + razlog + primalac; `started_at.is_none()` → `Unos` with both `None`, the object type unchanged so the row still says which field class was opened. New test `unmasking_before_the_operator_enters_records_no_disclosure` runs grant → reveal → `end_session`, asserts the revoke branch was actually taken, and sweeps the whole log for any `otkrivanje` or any `obradjivac_tehnicke_podrske` recipient. **Option (a) — refusing the pre-entry unmask — was considered and rejected:** it would force the vlasnik to decide only while the call is live, and the authorisation is a perfectly real thing to record; option (b) records it truthfully instead of suppressing it.

4. **The izvod called the remote-support engineer „Automatska obrada (bez korisnika)“** — pre-existing, correctly flagged as out of Task 2's scope, closed here because it is a false statement in the document handed to the Poverenik and finding 1 had made that column load-bearing. `request_access` writes `actor_user_id: None` on purpose and rightly, but `actor_label`'s absent-actor arm was written for the req. 23 time-driven purge. It now has a third arm keyed on `support_session_id`, already on the row: „Obrađivač tehničke podrške (bez korisničkog naloga)“. `the_izvod_never_calls_the_support_engineer_automatic_processing` exports both kinds of actorless line and asserts the purge row keeps its own string, so the fix cannot over-fire. **Deviation — one frontend file was touched in a backend task:** `AuditLogPanel.tsx` printed the same falsehood in the „Lice“ column, and leaving a known-false operator string on screen while fixing the document would have the panel and the izvod disagree about the same row. `actorLabel` there mirrors the Rust rule; the name-only rendering for a present actor is left as it was, because `renders_no_per_cashier_aggregate` pins it and §5 item 3 is the reason. One vitest: „does not call the remote-support engineer automatic processing“.

**Still unclosed after this pass.** Constant-vs-schema drift on `KATEGORIJE_ODSUSTVA` (unchanged, worktime's question). `src/services/types.ts::SupportSession` is still one field behind, and `objectType` there is a plain `string`, so the new code needs no mirror. Nothing else frontend-side is wired; Task 4 still owns the surfaces.

**Gates after the findings pass (six, run once, never concurrently):** `cargo test` **1029 passed / 0 failed**; `cargo clippy --all-targets --all-features --locked -D warnings` clean; `cargo fmt --check` clean (rustfmt applied once); `bun run test` **548 passed / 35 files**; `bun run build` ok; `git diff --check` clean.

---

### Task 3: Mask the three read paths

**This is the load-bearing task.** Today `kategorija_odsustva` leaves the backend on every read and the screen decides. After this task the **backend** decides, and the frontend gate stays only as a second layer.

The three paths in `commands/worktime.rs`: `list_month` (the row build at ~:985), `export_csv` (~:1380) and `my_hours`. Work out for each whether it is reachable under a support nalog and mask accordingly.

**`my_hours` is the exception and must be reasoned about, not copied.** It is the employee's own view of their own month, resolved from the server-side session. Decide deliberately whether a support operator can even reach it, and record the decision — do not mask it reflexively if the answer is that they cannot.

Mask by **withholding at the query or the row build**, in the shape `commands::popis::read_lines` uses for čl. 8 st. 5: while masked, the value is `None` because nothing was read, not because something read was dropped. A `SELECT` that fetches the category and blanks it in Rust looks identical from outside and is a different thing — the value exists, one refactor or one debug print from the support operator's screen.

The masked read must remain **truthful**: an entry with an absence still reports that there **is** an absence and its minutes. Only the *reason* is withheld. A masked read that hides the absence itself would corrupt the register's totals on screen.

- [x] **Step 1: Write the failing tests** — with a live nalog and no unmask, `list_month` returns entries whose `kategorija_odsustva` is `None` while `odsustvo_minuta` is unchanged; `export_csv` writes no category value into the file (assert on the **file bytes**, not the return value); after `support_reveal_absence_reason` the same reads carry the category; with **no** live session nothing is masked (the payroll role's ordinary day is untouched); an expired session masks nothing, because the nalog is over. Add a source-level guard mirroring `the_phase_a_row_renderer_never_names_a_phase_b_field` if the masked path can be expressed as its own code path.
- [x] **Steps 2–5.**

**Shipped.** The backend decides. `load_entries` (`commands/worktime.rs:1072`) is now a two-line dispatcher over **two whole statements**: `load_entries_with_reason` (`:1087`) selects the column as before, and `load_entries_without_reason` (`:1140`) does not name it anywhere — its SELECT list has no `kategorija_odsustva` and its row closure calls the shared `entry_from_row(row, None)`, so while a nalog is masking there is no value in the row to drop, to log or to print. `commands::popis::read_lines` is the pattern and its reasoning is quoted into the doc comment. The question itself is one function, `razlog_odsustva_dostupan` (`:455`): no live nalog → nothing is masked; a live nalog → masked unless v23's `odsustvo_otkriveno_at` is set on that nalog. The unmask is read off the nalog, never by reading `audit_events` back.

**The three paths.** `list_month` (`:428`) masks; `export_month_csv` (`:642`) masks because it goes through `list_month`, which is the point — an export that carried what the screen withholds would leave the čl. 17-adjacent column in a file that outlives the nalog. **`my_hours` (`:505`) does not mask, and the reason is not that a support operator cannot reach it.** They can: the command is session-gated only, so an operator remote-controlling the till can invoke it for whoever is signed in, and the doc comment says so rather than pretending otherwise. It is left open because §4 req. 25's carve-out names this very function as the ZZPL čl. 26 discharge, because req. 28's words are „mask the column and payroll screens“ and a person reading their own row is neither, and because masking the payload would withhold from the subject to prevent a disclosure it cannot prevent — the operator sees the pixels, not the JSON. What is left over is procedural and belongs to §6 W-15.

**The red state, and three mutations.** The eight tests failed to compile: `E0432` on `RAZLOG_ODSUSTVA_SKRIVEN` and ten `E0061`s — `list_month`/`export_month_csv` took four arguments, not five. Then: (1) making `razlog_odsustva_dostupan` always return `true` trips five tests; (2) **adding `e.kategorija_odsustva AS ignorisano` to the masked SELECT while still passing `None` to the row builder leaves every behavioural test green and trips only `the_masked_register_read_never_names_the_absence_reason_column`** — which is exactly why the structural guard exists, since a read that fetches and drops is invisible from outside; (3) flipping `my_hours` to masked trips `my_hours_still_shows_the_employee_their_own_absence_reason`. Eight tests: `a_live_nalog_withholds_the_absence_reason_and_leaves_the_absence`, `an_expired_nalog_withholds_nothing`, `the_unmasked_nalog_carries_the_absence_reason_again`, `the_exported_file_carries_no_absence_reason_while_the_nalog_is_live`, `my_hours_still_shows_the_employee_their_own_absence_reason`, `the_masked_register_read_never_names_the_absence_reason_column`, `the_mask_covers_the_zzpl_column_and_not_the_zeor_letters`, plus the `bez_zaglavlja` helper the export sweep needed.

**The residual, pinned rather than papered over.** Every `kategorija_odsustva` value is exactly its ZEOR čl. 24 tač. 1 bucket minus `_minuta` — that derivation is `book_absence`'s whole design — so a masked row whose `sprecenost_rfzo_minuta` is 480 still says which reason it was to anyone reading the buckets. The buckets stay: ZEOR mandates them, `ukupno_neizvrseni_minuta` is what the month's totals are read off, and zeroing one would be a **false** statement rather than a withheld one („0 časova“ is not „nije prikazano“). So req. 28 is discharged for the column and for the screen — `WorkTimeModule`'s grid renders v) and none of the nine buckets — and **not** for the exported file, which carries every letter. `the_mask_covers_the_zzpl_column_and_not_the_zeor_letters` pins it so no document can claim more. **Tasks 4 and 5 must word the čl. 23 notice and the čl. 47 register to that limit.** Closing it properly needs the bucket columns to become `Option<i64>`, which the plan's self-review deliberately forecloses and which would reach the frozen Class A `klasifikacija_json`.

**Deviation 1 — `now: &str` threaded through `list_month` and `export_month_csv`.** The mask is a clock decision (`is_active` on the nalog), and Global Constraint 3 forbids deciding it in SQL. The two `#[tauri::command]` wrappers read `utc_now()` and nothing below them does. Eleven existing test call sites gained the argument and a `BEZ_NALOGA` const that says in one place what the instant means; **no existing assertion was changed, weakened or removed.**

**Deviation 2 — `WorkTimeMonth` gained `razlog_odsustva_skriven: bool`**, a field the plan did not name. It is a property of **the read**, stated by the read that performed it, so a surface can tell „nema odsustva“ from „razlog je skriven“ without asking a second command and hoping the two answers were about the same instant — which is the failure mode the plan's own architecture note levels at `canSeeAbsenceReason`. Task 4's masked cell needs exactly this. `src/services/types.ts` does **not** mirror it yet; TypeScript interfaces are structural so the extra JSON field breaks nothing, and Task 4 owns that file (it is now two fields behind: this one and `odsustvoOtkrivenoAt`).

**Deviation 3 — the masked export says why its column is empty.** The plan asked only that no category value reach the file. An empty „Kategorija odsustva“ cell beside 480 minutes of absence is a false statement by omission — it reads as „razlog nije evidentiran“ — so `RAZLOG_ODSUSTVA_SKRIVEN` (`:68`) goes in above the table. Its second sentence states the ZEOR residual out loud, because the first sentence alone would overclaim. The cell itself stays empty rather than carrying a Serbian marker word, which in a category column could be read back as a category.

**Deviation 4 — `close_period` reads through the withholding statement.** The frozen Class A classification is minutes and a day count and has never carried the category, so that read now does not select the column at all, whatever the nalog says. The comment records that putting the reason into the frozen record must be done there in the open, not by flipping the argument — which would silently freeze a `None` under a live nalog. `live_entry` and `write_entry`'s read-back stay unmasked on purpose, with the reasons written down: the first feeds only `id`/`verzija` inside a transaction and is never returned, the second echoes the value the caller supplied a line ago.

**One test failed for a reason worth recording.** The first run of the export sweep tripped on „porodiljsko“ — not a leaked value but a substring of the fixed heading „đ) Časovi **porodiljsko**g i skraćenog radnog vremena roditelja (min)“, which every export prints masked or not. The sweep now runs over the file minus its heading line (`bez_zaglavlja`), so what it forbids is a category **value in a cell**; the helper's doc comment records the trap.

**Not touched, deliberately:** `WorkTimeModule.tsx:255`'s `canSeeAbsenceReason` is still the role-driven frontend gate, now a second layer over a backend that decides — Task 4 owns the surfaces. **Left for Task 4:** with a masked grid, an ispravka form prefilled from a masked entry would offer an empty category select and write the blank back over a real category. That is a UI question this task cannot answer, and it is now reachable.

**Gates (this task's, run once each, never concurrently):** `cargo test` **1036 passed / 0 failed**; `cargo clippy --all-targets --all-features --locked -D warnings` clean; `cargo fmt --check` clean (rustfmt applied once, over test-body line wrapping only); `git diff --check` clean. `bun` untouched — no TypeScript changed.

**One count that does not add up, stated rather than smoothed over.** This task's commit adds **7** `#[test]`s, and the tree it was measured on also carries `7bc450b` — a commit that landed on master from another worker mid-task and adds **1** more. 1036 measured minus those 8 is **1028**, one below the **1029** Task 2 recorded above. Nothing here removed or renamed a test (`git show` on this commit shows no deleted `#[test]`), so the discrepancy is in one of the two earlier counts, not in this change. Left for whoever reconciles the register.

---

### Task 4: The surfaces — the unmask control, the masked column, and the notice

**Files:** `src/app/privacy/` (the unmask control beside Daljinska podrška, where SW-10's one legal duty already lives), `src/app/worktime/WorkTimeModule.tsx`, `src/app/worktime/MyHoursPanel.tsx`, `src/services/*`.

- The masked cell must **say it is masked**, not render empty. An empty cell reads as „no absence recorded“, which is a different and false statement.
- The unmask control is the **shop's**, admin-gated, and states plainly that it is irreversible for that session and that it is logged. No re-mask button, because there is no re-mask verb.
- The mock adapter must model the masked state and the refusal, or the frontend tests prove nothing.
- The čl. 23 notice (`docs/compliance/obavestenje-zaposlenima.md`) must now say the reason is masked from remote support by default and that an unmask is recorded — and must claim **only** that. If the čl. 47 register (`cl47.rs`) describes this processing, the same sentence goes there and no further.

- [ ] **Step 1: Write the failing tests** — the masked cell renders its „skriveno“ state rather than blank; the unmask control is absent for a cashier; the control names the irreversibility and the logging; `docs_guard` pins the notice's new sentence against the code that performs it.
- [ ] **Steps 2–5.**

---

### Task 5: Docs and the full gate run

- [ ] Move req. 28 out of the „Still open“ lists in `docs/PROGRESS.md` and restate the SW-14 register row and row 15 in `docs/SERBIAN-LAW-COMPLIANCE.md`.
- [ ] Record what did **not** ship: whether `my_hours` is masked and why, and that this feature sharpens §6 **W-15** — the vendor now demonstrably reaches čl. 17-adjacent data, which is a question for the F-1 lawyer review rather than something code closes.
- [ ] Run all six gates; report exact counts and exit codes. Commit.

---

## Self-review

**Coverage.** Req. 28's three limbs map to Tasks 3 (mask the column), 2 (log the unmask), and 4 (the shop explicitly unmasks). „Mask the payroll screens“ is Task 4's frontend half. Migration v23 carries the stamp Task 2 writes and Task 3 reads.

**Placeholders.** Tasks 1–5 name required behaviours and exact assertions rather than pasting bodies, following the repo's seeding pattern. Every behaviour is a concrete assertion against a named symbol.

**Type consistency.** `support_reveal_absence_reason` (Task 2) returns the existing `SupportSession`, which gains one field in Task 1 and is already understood by the frontend — so no new wire type. `AuditObjectType::SupportSession` already exists.

**One gap found during review, folded in.** Task 3 originally said „mask the three read paths“ uniformly, which would have masked `my_hours` — the employee's view of their **own** absence reason — on the strength of a rule written about the vendor. An employee is not remote support, and req. 28 says nothing about hiding a person's own data from them; ZZPL čl. 26 points the other way. Task 3 now requires that path to be reasoned about and the decision recorded.

**One thing this plan deliberately does not do.** It does not mask any other column. The absence reason is singled out because it is the one field in this register that is health-adjacent; masking more would be inventing a rule, and masking less would miss req. 28's subject.
