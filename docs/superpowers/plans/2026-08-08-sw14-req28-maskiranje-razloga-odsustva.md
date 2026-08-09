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

**Deviation 2 — `SupportSession` gained its field here rather than in Task 2**, as the self-review anticipated: `commands/audit.rs` struct + `SESSION_COLUMNS` + `read_session`. Without it the stamp would be a column no read path names, and Task 3 needs the live nalog to answer the masking question. **`src/services/types.ts::SupportSession` does NOT yet name `odsustvoOtkrivenoAt`** — that file is Task 4's, TypeScript interfaces are structural so the extra JSON field breaks nothing, but the mirror is one field behind until Task 4 closes it, and its doc comment claims to be `crate::commands::audit::SupportSession`. **Closed 09.08.2026 by Task 4:** the sentence „does NOT yet name `odsustvoOtkrivenoAt`“ is withdrawn — the field is on the interface, with the stamp-not-a-boolean reasoning carried over.

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

**The residual, pinned rather than papered over.** Every `kategorija_odsustva` value is exactly its ZEOR čl. 24 tač. 1 bucket minus `_minuta` — that derivation is `book_absence`'s whole design — so a masked row whose `sprecenost_rfzo_minuta` is 480 still says which reason it was to anyone reading the buckets. The buckets stay: ZEOR mandates them, `ukupno_neizvrseni_minuta` is what the month's totals are read off, and zeroing one would be a **false** statement rather than a withheld one („0 časova“ is not „nije prikazano“). So req. 28 is discharged for the column and for the **rendered grid** — `WorkTimeModule` draws v) and none of the nine buckets — and **not** for the two places the buckets travel in full: the **wire** and the **exported file**. The wire was understated in the first version of this paragraph, which named only the file: `WorkTimeMinutes` derives `Serialize` with no skip and `load_entries_without_reason` selects every bucket, so a masked `list_month` response carries `„sprecenostRfzoMinuta": 480` on the very entry whose `kategorijaOdsustva` is `null`, and any client reading JSON rather than pixels recovers the category with a `find(|b| b != 0)`. That is the same architecture this plan's own opening levels at `canSeeAbsenceReason`; what the mask changes for a JSON client is the encoding, not the payload. Both halves are now asserted in `the_mask_covers_the_zzpl_column_and_not_the_zeor_letters` — the struct half for the wire, a **positional** cell read for the file — so no document can claim more. **Tasks 4 and 5 must word the čl. 23 notice and the čl. 47 register to that limit: „kategorija se ne prikazuje“, never „razlog nije dostupan tehničkoj podršci“.** Closing it properly needs the bucket columns to become `Option<i64>`, which the plan's self-review deliberately forecloses and which would reach the frozen Class A `klasifikacija_json`; refusing `export_month_csv` outright while masked was considered and rejected, because the čl. 21 offline export is a duty and the file's `RAZLOG_ODSUSTVA_SKRIVEN` note states the limit truthfully.

**A second residual, added 09.08.2026 by the review pass: the mask is keyed to a LIVE nalog, and two routes end that state with no čl. 48 line naming the absence reason.** (1) `commands::audit::end_session` carries the **same** `require_admin` gate as `reveal_absence_reason`, so an operator driving the vlasnik's signed-in screen clicks „Okončaj nalog“ on the same panel and reads `kategorija_odsustva` on the next `list_month` or `export_month_csv`; the log then holds a `menjanje`/`support_session` row saying the nalog was closed and nothing saying the column became visible — the sanctioned route costs an `otkrivanje` naming `SupportAbsenceReason`, the unsanctioned one costs nothing. (2) The same lift arrives with no action at all at `expires_at` (the panel's default is 60 minutes), and nothing in the app disconnects the operator there, because SW-10 keeps the nalog a record and not an access control. Neither is a code defect — an app cannot stop someone already inside an admin session — and neither is fixed here: the two available answers (an explicit re-arm, or keying the mask on „a nalog was live at any point in this app session“) are design decisions, and a post-session mask would contradict `an_expired_nalog_withholds_nothing` head-on. Both are pinned as deliberate by `ending_the_nalog_lifts_the_mask_and_logs_no_disclosure`, recorded on `razlog_odsustva_dostupan`'s doc comment, in `docs/PROGRESS.md` §6 W-15 as item 3 beside the credential-reset chain, and in register row 12.

**Deviation 1 — `now: &str` threaded through `list_month` and `export_month_csv`.** The mask is a clock decision (`is_active` on the nalog), and Global Constraint 3 forbids deciding it in SQL. The two `#[tauri::command]` wrappers read `utc_now()` and nothing below them does. Eleven existing test call sites gained the argument and a `BEZ_NALOGA` const that says in one place what the instant means; **no existing assertion was changed, weakened or removed.**

**Deviation 2 — `WorkTimeMonth` gained `razlog_odsustva_skriven: bool`**, a field the plan did not name. It is a property of **the read**, stated by the read that performed it, so a surface can tell „nema odsustva“ from „razlog je skriven“ without asking a second command and hoping the two answers were about the same instant — which is the failure mode the plan's own architecture note levels at `canSeeAbsenceReason`. Task 4's masked cell needs exactly this. `src/services/types.ts` does **not** mirror it yet; TypeScript interfaces are structural so the extra JSON field breaks nothing, and Task 4 owns that file (it is now two fields behind: this one and `odsustvoOtkrivenoAt`). **Closed 09.08.2026 by Task 4:** the sentence „does **not** mirror it yet“ is withdrawn for both fields; `WorkTimeMonth.razlogOdsustvaSkriven` is what `AbsenceCell` reads.

**Deviation 3 — the masked export says why its column is empty.** The plan asked only that no category value reach the file. An empty „Kategorija odsustva“ cell beside 480 minutes of absence is a false statement by omission — it reads as „razlog nije evidentiran“ — so `RAZLOG_ODSUSTVA_SKRIVEN` (`:68`) goes in above the table. Its second sentence states the ZEOR residual out loud, because the first sentence alone would overclaim. The cell itself stays empty rather than carrying a Serbian marker word, which in a category column could be read back as a category.

**Deviation 4 — `close_period` reads through the withholding statement.** The frozen Class A classification is minutes and a day count and has never carried the category, so that read now does not select the column at all, whatever the nalog says. The comment records that putting the reason into the frozen record must be done there in the open, not by flipping the argument — which would silently freeze a `None` under a live nalog. `live_entry` and `write_entry`'s read-back stay unmasked on purpose, with the reasons written down: the first feeds only `id`/`verzija` inside a transaction and is never returned, the second echoes the value the caller supplied a line ago.

**One test failed for a reason worth recording.** The first run of the export sweep tripped on „porodiljsko“ — not a leaked value but a substring of the fixed heading „đ) Časovi **porodiljsko**g i skraćenog radnog vremena roditelja (min)“, which every export prints masked or not. The sweep now runs over the file minus its heading line (`bez_zaglavlja`), so what it forbids is a category **value in a cell**; the helper's doc comment records the trap.

**Not touched, deliberately:** `WorkTimeModule.tsx:255`'s `canSeeAbsenceReason` is still the role-driven frontend gate, now a second layer over a backend that decides — Task 4 owns the surfaces. **Task 4 closed the two items below on 09.08.2026** — the masked cell states itself and the ispravka form was checked and left alone — so „Left for Task 4“ reads as the record of what was owed, not as open work.

**Left for Task 4 — two items, and the first one is a falsehood currently on master.**

1. **The masked grid cell says „—“.** `AbsenceCell` (`WorkTimeModule.tsx:1056`) tests `if (!entry.kategorijaOdsustva) return „—“` **before** it consults `canSeeAbsenceReason`, and „—“ on that column has always meant „this day is not an absence“. Under a live nalog the backend now sends `kategorijaOdsustva: null` for a real absence, so the „Odsustvo“ cell reads „—“ on a row whose „Ukupno neizvršeni“ cell reads 480 — the same false-by-omission reading `RAZLOG_ODSUSTVA_SKRIVEN` was added to stop in the exported file, left standing on the screen. Because the null branch runs first it also regresses §4 req. 25's *„every other role sees `odsutan` plus an hour total“*: while a nalog is live **every** role, payroll included, gets „—“ instead. Nothing tests it. The backend already ships what the fix needs (`WorkTimeMonth.razlog_odsustva_skriven`, camelCase on the wire); Task 4 must mirror it into `src/services/types.ts`, pass it to `AbsenceCell`, render a stated „Odsutan (razlog skriven)“, and pin it with a vitest. The component's doc comment now records the defect rather than claiming behaviour it lacks.
2. **The ispravka form, conditionally.** *If* a correction form is ever prefilled from an entry, a masked entry would prefill an empty category select and write the blank over a real category. It is **not** reachable today — `emptyForm`/`resetForm` (`WorkTimeModule.tsx:179`, `:360`) are the only writers of the form state, so no correction form is prefilled from a row — and the first version of this paragraph stated it as a live hazard, which it is not. It is recorded as a constraint on Task 4's design, not as a defect.

**Gates (this task's, run once each, never concurrently):** `cargo test` **1037 passed / 0 failed** (the figure first recorded here was 1036 — see the reconciliation below); `cargo clippy --all-targets --all-features --locked -D warnings` clean; `cargo fmt --check` clean (rustfmt applied once, over test-body line wrapping only); `git diff --check` clean. `bun` untouched — no TypeScript changed.

**The count reconciles; the „1036“ above was a misread and is withdrawn.** This paragraph used to say 1036 measured minus the 8 tests added since Task 2 is 1028, one below Task 2's **1029**, and pointed at „one of the two earlier counts“. The review pointed the other way and was right. Re-measured on the reviewed tree: `cargo test -- --list` reports **1037** collected and `grep -rn '#\[test\]' src-tauri/src` counts **1037** attributes — every one a bare `#[test]` line, no `#[ignore]`, no cfg-gated module — so the collected count and the attribute count agree and Task 2's 1029 + 7 (this task) + 1 (`7bc450b`) lands exactly on it. Nothing was being collected out; the reported 1036 was simply wrong. **The findings pass adds one more test and the gate now reads 1038.**

**Shipped — review findings closed (two blocking, nine major; one new test, four strengthened, five re-stated documents).** The shape of them: a task about a mask shipped two file-level assertions that could not see what they claimed, a structural guard narrower than its own doc comment, and three documents denying the control it had just built.

1. **Both file-level export assertions were satisfied by the wrong column.** `csv.contains("480")` was read as *„the masked export still states the absence hours“*, but `radni_dan` gives every fixture day `moguci_minuta: 480`, so the substring is in column a) whatever becomes of the absence; `csv.contains("đ) Časovi … RFZO (min)")` was read as the export residual, but that string is a member of `WorkTimeMinutes::LABELS` and `month_to_csv` prints the whole label row on every export, masked or not — so it is true of an empty register. Both are now **positional**, through two new test helpers: `polja_dana` (the data row for a date, split — no field of a data row is ever quoted) and `celija_minuta` (a cell by column NAME, index derived from `WorkTimeMinutes::COLUMNS` + 2). The masked export is asserted to carry 480 in v) **and** in the đ) bucket, and `""` in the „Kategorija odsustva“ cell itself; the unmasked one to carry `sprecenost_rfzo` in that same cell. **Mutation:** making `month_to_csv` emit `0` for every bucket while masked — the exact „mask the hours too“ mistake — now fails both tests on `left: 0, right: 480`, where the old assertions passed because the totals row still contained „480“.
2. **The structural guard was evadable and its stated reason was false.** It forbade `e.kategorija_odsustva` and `row.get("kategorija_odsustva")` on the ground that the bare name would false-fire on `kategorija_odsustva: None` — which lives in `entry_from_row`, **outside** the scanned region; the region contains the string zero times. The two needles left the obvious refactor open: `kategorija_odsustva` unqualified is valid, unambiguous SQL there (`users` has no such column) and `row.get::<_, Option<String>>` or a positional `row.get(5)` reads it back past both. The needle is now the bare column name and the doc comment says what is true. **Mutation:** adding `kategorija_odsustva AS kategorija_odsustva` to the masked SELECT leaves all 30 other worktime tests green and trips this guard — which is the whole reason it exists.
3. **`my_hours_still_shows_the_employee_their_own_absence_reason` never observed the nalog it is named for.** `my_hours` reads no clock and consults no nalog, so deleting `izdaj_nalog` left the test green. It now asserts the divergence inside itself: under one nalog live 09:00–10:00, `list_month` at 09:20 returns `kategorija_odsustva: None` and `razlog_odsustva_skriven: true`, and `my_hours` for the same employee still carries `Some("sprecenost_rfzo")` with the flag false. **Mutation:** deleting the `izdaj_nalog` line now fails it.
4. **Two of `my_hours`'s three recorded reasons were wrong.** Reason 2 claimed the payload is *„bounded by whoever is signed in“*; it is not — the credential-reset chain in Task 5's checklist reaches any employee's month with the only trace a login, and house rule 11 bars a comment asserting a bound the code lacks. It now states the real ground (a certain čl. 26 harm against a disclosure the mask cannot stop) and names the chain as a residual. Reason 3 claimed a masked „Moji sati“ would leave the employee unable to tell a withheld reason from an unrecorded one — refuted by `razlog_odsustva_skriven`, added in the same commit. It is gone; the decision stands on reasons 1 and 2.
5. **Two doc comments overclaimed the mask.** `load_entries_without_reason`'s *„there is no value in the row … to spill“* is not true of the row it builds — it carries all nine ZEOR buckets, and a debug print of a masked row still says which reason it was. Scoped to „no **category** value“, with the buckets-are-the-limit clause pulled into the same sentence so the two cannot be read apart, and the residual paragraph now names **where** it travels: the wire and the file.
6. **The three false denials, closed rather than deferred.** `docs/SERBIAN-LAW-COMPLIANCE.md` row 12 and `docs/PROGRESS.md` `:335`, `:512`, `:638`, `:1395` all said the masking is not built. All five are re-stated in the house pattern — a dated stamp quoting the sentence withdrawn — as the **partial** it is, and pinned by the new `docs_guard::no_document_says_the_absence_reason_mask_is_unbuilt`. Two halves, because one of the bullets said nothing false in words and read as open only because of the list it sat in: a categorical-denial sweep over the sentence around the column (quoted withdrawals exempt, located by the marker's own offset), and a requirement that every block naming the column also name one of `razlog_odsustva_dostupan` / `odsustvo_otkriveno_at` / `reveal_absence_reason`, all three bound to crate items so a rename is a compile error. Both halves proven red by mutation and reverted.

---

### Task 4: The surfaces — the unmask control, the masked column, and the notice

**Files:** `src/app/privacy/` (the unmask control beside Daljinska podrška, where SW-10's one legal duty already lives), `src/app/worktime/WorkTimeModule.tsx`, `src/app/worktime/MyHoursPanel.tsx`, `src/services/*`.

- The masked cell must **say it is masked**, not render empty. An empty cell reads as „no absence recorded“, which is a different and false statement.
- The unmask control is the **shop's**, admin-gated, and states plainly that it is irreversible for that session and that it is logged. No re-mask button, because there is no re-mask verb.
- The mock adapter must model the masked state and the refusal, or the frontend tests prove nothing.
- The čl. 23 notice (`docs/compliance/obavestenje-zaposlenima.md`) must now say the reason is masked from remote support by default and that an unmask is recorded — and must claim **only** that. If the čl. 47 register (`cl47.rs`) describes this processing, the same sentence goes there and no further.

- [x] **Step 1: Write the failing tests** — the masked cell renders its „skriveno“ state rather than blank; the unmask control is absent for a cashier; the control names the irreversibility and the logging; `docs_guard` pins the notice's new sentence against the code that performs it.
- [x] **Steps 2–5.**

**Shipped.** The three surfaces, the double behind them and the two documents.

**The masked cell says it is masked.** `AbsenceCell` (`WorkTimeModule.tsx:1107`) takes `razlogSkriven` from the read and tests it **before** the `null` category, so „—“ can no longer stand on a row that books 480 minutes of absence. Masked and any absence minutes booked renders „Odsutan (razlog skriven)“; masked and none renders „—“, because „Odsutan“ on a worked day would invent an absence. **Corrected 09.08.2026 by the findings pass — the version first shipped here read only v), and this paragraph said so:** the sentence *„Masked and `ukupnoNeizvrseniMinuta > 0` renders „Odsutan (razlog skriven)“ … because v) is the sum of the nine ZEOR čl. 24 tač. 1 non-worked buckets“* is **withdrawn as the whole rule**. v) is nine of the **ten** categories; the tenth, `obustava_rada_strajk`, books into `obustavaRadaStrajkMinuta`, which `derive_totals` puts inside b). The predicate is now `ukupnoNeizvrseniMinuta + obustavaRadaStrajkMinuta > 0`. The **why** is not in the cell — it is a banner above the register (`:874`), `RAZLOG_SKRIVEN_OBJASNJENJE` (`:1057`), which names the čl. 46 nalog, says the hour count is still shown and points at Privatnost → Daljinska podrška. The cell carries the same sentence as a `title`. `MyHoursPanel.tsx:186` passes `hours.razlogOdsustvaSkriven` rather than a literal `false`: `my_hours` masks nothing and deliberately so, but a panel that asserts a backend property instead of reporting one goes stale silently.

**The unmask control is the shop's, and it is beside the nalog.** `SupportApprovalPanel` (`:69`, the unmask block at `:270`), inside the live-nalog branch, admin-gated on a new optional `currentUser` — absent means closed, the house rule, threaded `AppShell.tsx:513` → `PrivacyModule` → the panel. It states the withholding, then the two facts req. 28's third limb turns on: the disclosure is for **this nalog only** and **ne može da se povuče**, and it **se upisuje u evidenciju pristupa (ZZPL čl. 48)**. Once `odsustvoOtkrivenoAt` is set the button is replaced by an alert carrying the instant — one disclosure per nalog, not a repeatable action. **There is no re-mask control anywhere**, and three tests hold that: a button sweep on the panel, and a port-surface assertion in both `local-adapter.test.ts` and the mock test that exactly one `PrivacyService` method matches `/absence|mask/i`. With no live nalog the panel says so in the nalog alert („Kategorija odsustva se skriva samo dok nalog važi“) and offers nothing — the mask is not permanent and the copy must not imply it is.

**The double models the mask and the refusal.** `mock-adapter.ts`: `razlogOdsustvaDostupan()` (`:3567`) is the same question the backend asks — no live nalog masks nothing, a live nalog masks unless that nalog carries the stamp — and `buildMonth` now takes the answer as an **argument**, so all four call sites state it: `listMonth` and `exportCsv` ask, `myHours` passes `true` (the viewer is the data subject), `closePeriod` passes `false` (the frozen Class A record has never carried the category). `revealAbsenceReason` refuses with the backend's own `support_bez_naloga_za_otkrivanje` and its own sentence, and is idempotent. Its doc comment says out loud what a double **cannot** copy: the backend withholds by running a statement that never names the column, and an in-memory array has no query to leave a column out of, so the mock reproduces the observable contract and does not pretend to reproduce the structural one.

**The čl. 23 notice, and only what the code does.** `docs/compliance/obavestenje-zaposlenima.md:59` — one paragraph in §2.1: the category is not shown while a nalog is live, the podatak is not read from the database rather than merely not printed, the employer may disclose it for that one nalog, the disclosure is written into the evidencija pristupa, cannot be taken back for that nalog and dies with it — and then the limit in the same paragraph: **„Skriva se sama kolona“**, the hour counts per statutory absence type stay visible and the type can still be inferred from them. „Moji sati“ is stated as unmasked, because a čl. 26 right that quietly narrowed during a support call would be the worse defect. Pinned by `docs_guard::the_cl_23_notice_states_the_absence_reason_mask_and_overclaims_nothing`, bound to `reveal_absence_reason`, `SupportSession::odsustvo_otkriveno_at` and `WorkTimeMonth::razlog_odsustva_skriven` as crate items, so a rename is a compile error. Two halves: one **block** must carry both facts, and no sentence about the category and support may say „ne vidi“, „ne može da vidi“, „nije dostupan/dostupna“ or „ne saznaje“ — the mask covers the column, not the reason, and a čl. 23 statement wrong in the employee's favour is the worst direction to be wrong in.

**The čl. 47 register, once.** The same sentence in `cl47.rs`'s `radno_vreme.mere` and nowhere else, pinned by `cl47::tests::one_register_entry_states_the_absence_reason_mask_and_no_other_does` — the measure protects the working-time record, so repeating it under `tehnicka_podrska` would read as two controls and the second copy is the one that goes stale in a generated document. The test also asserts it reaches the **generated** rows, not just the constant.

**The red state, and four mutations.** 14 vitest failures and 2 cargo failures, each for its own reason: `revealAbsenceReason is not a function` in both adapters, no unmask control on the panel, the masked cell rendering „—“, the notice block absent, and `found []` for the čl. 47 entry. Then: (1) `return true ?` in place of the v) test makes the masked cell claim an absence on a worked day and trips `does not invent an absence on a worked day while the column is masked`; (2) adding „Tehnička podrška tako ne vidi kategoriju odsustva“ to the notice trips the overclaim sweep with the offending sentence quoted; (3) copying the measure sentence into `tehnicka_podrska.mere` trips the čl. 47 test on `["radno_vreme", "tehnicka_podrska"]`; (4) the red run itself proved the cell test, since the pre-change branch order is exactly the defect.

**Deviation 1 — one test assertion was written wrong and was narrowed, not deleted.** The first draft of „says the reason is hidden“ swept the **whole screen** for „sprečenost“ and failed on the entry form's category `<option>` list, which is the closed **vocabulary** and is on screen for the shop's own admin whether or not a nalog is live. It names nobody's month, so it is not a leak; the assertion is now scoped to the register `<table>` and the comment records why. The masked cell itself is read **positionally** (`celijaOdsustva`, index 2) rather than by text, because every zero minute column renders „—“ too and a row-wide search for that string says nothing about the column under test — the same trap the Rust export sweep hit.

**Deviation 2 — `currentUser` was threaded into `PrivacyModule`, a component the plan did not name.** The panel needed a role and the module is what has one to give. Optional at both hops, absent-means-closed, and `PrivacyModule.test.tsx`'s existing render without it still passes — nothing was weakened. Privatnost is already `adminOnly` in `navigation.ts` and the verb is `require_admin`-gated inside the domain function, so this is the third of three layers.

**Deviation 3 — the docs_guard shape is „one block states it“, not „every block that mentions it states it“.** The sibling guard uses the second form over `PROGRESS.md` bullets, and it cannot work here: the čl. 23 st. 3 re-delivery header at the top of the notice legitimately names both the category and the nalog za daljinsku podršku without describing the mask, and the whole front-matter blockquote collapses into one markdown block. Measured, not reasoned — that is exactly how the first version failed. The existence form is still bounded to one block, so „ne prikazuje“ five paragraphs from the subject does not satisfy it, and the failure message lists the blocks that named the subject and stopped short.

**The residual this task could not close, recorded on the code rather than left to be found.** A category booked with **zero** minutes moves no bucket at all, so while masked such a day renders „—“ and is indistinguishable from a day with no absence. That is the whole of what is left open, and the reason is that nothing on the payload changes. **Corrected 09.08.2026 by the findings pass:** the sentences *„Closing it needs a per-entry „there is an absence“ bit, and the only way to derive one is a predicate over `kategorija_odsustva` — the very column the masked statement must not name“* are **withdrawn as false**. `obustavaRadaStrajkMinuta` is on the masked payload — `load_entries_without_reason` selects every bucket — so the bit was derivable for the tenth category without naming the withheld column, and the guard `the_masked_register_read_never_names_the_absence_reason_column` never stood in the way. What the first version recorded as the residual was in fact a live falsehood at any minute count for one category in ten; only the genuinely zero-minute corner remains. `AbsenceCell`'s doc comment now says that and no more.

**Task 3's second hand-over — the ispravka form — was checked and needed nothing.** Every writer of the entry form state is either `emptyForm` (`:346`, `:367`, and `resetForm` at `:415`) or a direct keystroke handler; **no** path prefills the form from a row, so a masked entry cannot prefill an empty category select and write the blank over a real category. The constraint on the design stands for whoever adds a prefill: it must not take `kategorijaOdsustva` from a read that may have withheld it.

**Unchanged, deliberately:** `canSeeAbsenceReason` stays the §4 req. 25 role gate, a second layer over a backend that decides. Task 3's ZEOR-bucket residual is untouched and is now stated in three more places (the notice, the čl. 47 measure, the panel copy) — always as „skriva se sama kolona“, never as „razlog nije dostupan“.

**Gates (this task's, run once each, never concurrently):** `cargo test` **1040 passed / 0 failed**; `cargo clippy --all-targets --all-features --locked -D warnings` clean; `cargo fmt --check` clean (rustfmt applied once, over test-body wrapping in `cl47.rs` only); `git diff --check` clean; `bunx tsc --noEmit` clean; `bunx vitest run src/app/worktime src/app/privacy src/services` **166 passed / 12 files**.

**Shipped — review findings closed (three blocking, three major; four new tests, one new assertion inside an existing test).** Six findings, three of them the same defect written twice. The shape of them: a mask that still denied one absence in ten, three documents that denied the one carve-out the code deliberately keeps open, and the only wiring in the feature that fails silently.

1. **The masked cell rendered „—“ on a full shift of štrajk, and two new doc comments said that could not happen.** The branch asked `ukupnoNeizvrseniMinuta > 0`, but `derive_totals` sums **nine** of the ten categories' buckets into v); `obustava_rada_strajk` books into `obustava_rada_strajk_minuta`, which goes into b) — the repo's own save test pins it, asserting a 300-minute strike leaves v) at **zero**. So the exact defect the commit message called „gone“ („a row booking 480 minutes of absence read as a row with none, for every role, payroll included“) was still live for one category in ten, at **any** minute count, through a picker option the shipped form offers. The predicate is now `ukupnoNeizvrseniMinuta + obustavaRadaStrajkMinuta > 0`; both fields are on `WorkTimeMinutes` for every read, so no statement names `kategorija_odsustva` and `the_masked_register_read_never_names_the_absence_reason_column` is untouched. New vitest `says the reason is hidden on a štrajk day, which books outside v)`, **proven red first** on the shipped code with `Received: —`. The three consequential false statements are restated rather than quietly patched: `AbsenceCell`'s residual paragraph (the residual is the zero-minute booking, not „zero-minute categories“, and the bit never required the withheld column), and this plan's two paragraphs above, each carrying the withdrawn sentence.
2. **`types.ts`'s „the rendered register is covered“ was false in the same place.** The grid draws b), efektivno izvršeni and čekanja i zastoji as well as v), and b) − efektivno − čekanja **is** `obustava_rada_strajk_minuta` — so the rendered register discloses the tenth category in full, by subtraction, while the comment claimed cover. Restated at `types.ts:1633`: the nine v) buckets are withheld on screen, the tenth is not, and removing a drawn column would gut the ZEOR čl. 24 tač. 1 register, so the gap is stated rather than closed.
3. **The panel copy and the čl. 47 measure denied the „Moji sati“ carve-out.** Both said the category „se ne čita iz baze“ with no qualifier; `my_hours` calls `load_month(state, user_id, godina, mesec, true)` **unconditionally** (`worktime.rs:518`) and its own doc comment says an employee opening „Moji sati“ under a live nalog shows their own category to the obrađivač. The čl. 23 notice already carried the carve-out — which is itself the proof that „u evidenciji radnog vremena“ was read as covering that screen. The panel is the worse of the two, because that is the surface on which the vlasnik decides whether to widen what the obrađivač may process, and it was telling them the datum is not read at all. Both now scope the claim to „u pregledu i izvozu evidencije radnog vremena“ and carry the notice's own sentence: *„Pregled „Moji sati“, kojim zaposleni vidi sopstvene časove, ne skriva se ni tada — pravo iz ZZPL čl. 26 pripada zaposlenom i ne ograničava se zbog sesije podrške.“* Pinned on both sides: a fourth needle „moji sati“ in `one_register_entry_states_the_absence_reason_mask_and_no_other_does` (**red first**, „radno_vreme.mere lacks „moji sati““) and the new vitest `does not deny the „Moji sati“ carve-out the backend keeps open` (**red first**, `Unable to find an element with the text: /moji sati/i`). `RAZLOG_SKRIVEN_OBJASNJENJE` got the same scoping — „se u ovoj evidenciji ne prikazuje“ — because it was categorical too, even standing in front of the register.
4. **The čl. 47 working-time entry described masking an obrađivač it did not list as a recipient.** `evidencija_zaposlenih.vrsta_primalaca` names it; `radno_vreme.vrsta_primalaca` said only „Nadležni državni organi …; knjigovođa …“, so a single entry told the Poverenik in field 5 that remote support does not receive the working-time record and in field 8 that a column of it is masked from them. Field 5 was the wrong one: `list_month` and `export_month_csv` are exactly what the operator reaches under a nalog. The obrađivač is now named in `evidencija_zaposlenih`'s own words, so the two entries state one recipient set, and a second assertion inside the same test requires it. **Mutation:** removing the clause fails on „radno_vreme.vrsta_primalaca must name the obrađivač its own mere describe masking“. The omission predated the mask; the mask is what turned it into a self-contradiction inside one čl. 47 st. 1 entry.
5. **Both `currentUser` hops were unpinned, and the gate is absent-means-closed.** Nothing threw when the prop was dropped — the unmask button simply never rendered and the shop could never lift the mask in the running app, with the whole bun suite green: every `SupportApprovalPanel` test supplies `currentUser` itself, `PrivacyModule.test.tsx` supplied none, and there is no `AppShell.test.tsx`. Two tests, one per hop: `PrivacyModule` „hands the vlasnik's role to the čl. 46 panel, and hands nothing when it has nothing“ (both arms, so the vlasnik arm fails if the module stops passing the user down and the no-user arm fails if the panel ever defaults the gate open), and in `src/App.test.tsx` „threads the signed-in vlasnik into Privatnost, so the req. 28 unmask is reachable“ — an admin signed into the shell, clicking Privatnost, reaching the control. **Both mutations run separately:** deleting `currentUser={session.user}` at `AppShell.tsx:513` fails the App test alone (1 failed / 54 passed); deleting `currentUser={currentUser}` at `PrivacyModule.tsx:51` fails both (2 failed / 53 passed), since the shell test runs through both hops. Deviation 2 above claimed „`PrivacyModule.test.tsx`'s existing render without it still passes — nothing was weakened“; that was true and is still true, but it was also the reason the hop was invisible, and the pair of arms is what fixes it.

**Deviation — the AppShell hop is now tested, where the finding offered „or state in the plan that it is unpinned“.** The cheaper option was declined: `src/App.test.tsx` already has the exact pattern (`Object.assign(createMockServices(), { initialSession })` plus a nav click, as in „lets a cashier reach Moji sati“), so pinning it cost one stub and one click, and a recorded gap in a plan does not stop the prop being deleted.

**Not changed, and why.** `live_entry` (`worktime.rs:1229`) does read `kategorija_odsustva` while a nalog is live, and the finding is right that a bare „se ne čita iz baze“ is false of it. Scoping the two sentences to the **pregled and the izvoz** closes that too, without a second clause: `live_entry` feeds `write_entry` the predecessor's `id` and `verzija` inside a transaction and is never returned to a caller, so it is not a pregled and not an izvoz. The čl. 23 notice is unchanged — it was the one document that got this right.

**Gates after the findings pass (run once each, never concurrently):** `cargo test` **1040 passed / 0 failed** (unchanged — the pass adds one cargo *assertion* and one needle inside an existing test, not a new `#[test]`); `cargo clippy --all-targets --all-features --locked -D warnings` clean; `cargo fmt --check` clean, nothing to reformat; `bunx tsc --noEmit` clean; `git diff --check` clean; `bunx vitest run src/app/worktime src/app/privacy src/services src/App.test.tsx` **217 passed / 13 files** (was 166 / 12 over the narrower set; +3 new vitests and `src/App.test.tsx` now in scope).

---

### Task 5: Docs and the full gate run

- [x] Move req. 28 out of the „Still open“ lists in `docs/PROGRESS.md` and restate the SW-14 register row and row 15 in `docs/SERBIAN-LAW-COMPLIANCE.md`. **Name `docs/SERBIAN-LAW-COMPLIANCE.md` row 12 (privacy by design) explicitly**: it is neither the SW-14 row nor row 15, it carried the flattest denial of the three — *„remote-support masking of the absence-reason column is not built“* — and a checklist read literally would have left it standing. **The four PROGRESS.md bullets (`:335`, `:512`, `:638`, `:1395`) and register row 12 were already re-stated by Task 3's findings pass**, together with `docs_guard::no_document_says_the_absence_reason_mask_is_unbuilt`, which now fails the crate on a categorical denial and on a block that names the column without naming what performs it. What Task 5 owes is the SW-14 row, row 15 and a section of its own — and none of them may say „built“: no surface reaches the unmask verb, so the honest statement is partial.
- [x] Record what did **not** ship: whether `my_hours` is masked and why, and that this feature sharpens §6 **W-15** — the vendor now demonstrably reaches čl. 17-adjacent data, which is a question for the F-1 lawyer review rather than something code closes. **The W-15 residual must name the credential-reset chain**, which is a real and almost unlogged route to the column: an operator holding the vlasnik's admin session calls `users_update` (`commands/users.rs:89`) → `update_user` re-hashes any employee's PIN (`:215`) and writes **no** audit event on that branch (only the deactivation branch calls `clear_credentials`, `:277`) → `auth_login` as that employee → `worktime_my_hours` returns that employee's month with `kategorija_odsustva` unmasked. The only trace is a login. It is why `my_hours`'s doc comment no longer claims the session bounds what the operator can reach.
- [x] Run all six gates; report exact counts and exit codes. Commit.

**Shipped.** One new `docs/PROGRESS.md` section (`:1577`, dated 2026-08-09) in the voice of the sections
above it — a per-task table carrying the real commit hashes (`520fe6b`; `fe4c3b4`, `ecf2d48`;
`7bc450b`, `7db90f7`, `ed15afd`; `2801dff`, `6dc4085`), the six-gate table with the counts measured for
it, a residual paragraph, the two deliberate non-decisions, the W-15 paragraph and the migration head
(**v23**). In `docs/SERBIAN-LAW-COMPLIANCE.md`: §2 row 12 re-stated, §2 row **15** re-stated, the §3
**SW-14** row given a req. 28 block of its own, and the §2 revision note now records the sweep. Four
`PROGRESS.md` bullets (`:335`, `:519`, `:655`, `:1407`) re-stated a second time. Gates, run once each
from the repo root and never concurrently, every command exit `0`: `bun run test` **568 passed / 0
failed, 36 files** (was 547 / 35); `bun run build` ok; `cargo test` **1040 passed / 0 failed, 0
ignored, 0 measured, 0 filtered out** (was 1016); clippy clean; `cargo fmt --check` clean, nothing to
reformat; `git diff --check` clean. `cargo test docs_guard` **28 passed** both before and after the
gate table was written into the file the guard embeds.

**Deviation 1 — this task's own stated ground was stale, and is withdrawn.** The checklist above says
*„none of them may say built: no surface reaches the unmask verb, so the honest statement is partial“*.
Task 4 shipped that surface hours before this task started. The conclusion survives and the reason does
not: req. 28 is a partial because of the **ZEOR čl. 24 tač. 1 buckets**, which carry the reason onto the
JSON payload and into the exported file whatever the mask does, and which give up `obustava_rada_strajk`
on the rendered grid by subtraction. Every row re-stated here says „partial“ on that ground and on no
other.

**Deviation 2 — the four bullets and register row 12 needed a SECOND re-statement, which this checklist
said they did not.** It records them as *„already re-stated by Task 3's findings pass“*, and they were —
but each of those re-statements listed **three** owed limbs, two of which (*„no surface reaches the
unmask verb“* and *„the payroll grid renders the em dash“*) Task 4 closed the same afternoon. Left
alone they would have been the same defect the guard beside them exists to bar, pointed one requirement
further in. Each is re-stated in the house pattern, quoting the sentence withdrawn.

**Deviation 3 — four `docs_guard.rs` doc comments and panic strings were edited in a docs task.** The
guard's own prose listed the two closed limbs as open, in the doc comment, in the `PORICANJE` note and
in the failure message it prints to the next author — a guard against stale denials, itself carrying
one. **No assertion, needle, list or bound changed**; the diff is comment and message text, and the 28
`docs_guard` tests pass unchanged in count. `src/app/worktime/WorkTimeModule.tsx`'s `AbsenceCell`
comment was touched for the same reason, one word of date.

**Deviation 4 — a nine-site date correction the plan did not ask for.** Every commit in this cycle is
dated **09.08.2026** per `git log`; four `PROGRESS.md` blocks and five doc comments stamped the backend
half 08.08.2026, apparently from this plan's own filename, and register row 12 said the false denial
stood *„for a day“* when the gap was under an hour. All ten corrected; the čl. 87 cycle's genuine
08.08.2026 stamps were checked and left alone.

**Deviation 5 — req. 28 was moved out of the „Still open“ lists by re-statement, not by deletion.** It
is still named in both documents, and must be: it is still a partial, and
`docs_guard::no_document_says_the_absence_reason_mask_is_unbuilt` asserts `pomena > 0` per document —
a register silent about a čl. 46 control the shop is running is the same defect as one that denies it.

**Unclosed, and recorded rather than fixed here.** The ZEOR-bucket residual (needs `Option<i64>` bucket
columns, which reach the frozen Class A `klasifikacija_json`); the zero-minute absence booking, which
moves no bucket and so renders „—“ while masked; the credential-reset chain — `users_update` →
`update_user` re-hashes a PIN and writes no audit line → `auth_login` → `worktime_my_hours` — which is
recorded against §6 W-15 and is a question about what a credential reset must log, not about req. 28.
**One item Task 2 left unclosed did close inside this cycle and is recorded so it is not carried
forward:** constant-vs-schema drift on `KATEGORIJE_ODSUSTVA` — `7bc450b` holds the constant and v17's
own `CHECK` to the same ten values, in both directions, which is what keeps the ten-category leak
sweep on the čl. 48 unmask line honest as the vocabulary grows.

---

### Whole-branch review pass (09.08.2026) — ten findings, all closed

One blocking, nine major, two of them the same defect written twice. The shape of them: two places
where the mask changed what is TRUE without the surface beside it changing what it SAYS, one ordering
of two shipped verbs that nothing had ever run, one write path the mask reached without anybody
noticing, and three guards that asserted the presence of a phrase rather than the claim it carries.

1. **BLOCKING — the panel told the vlasnik the category was masked after they had unmasked it.**
`SupportApprovalPanel`'s withholding paragraph was rendered unconditionally inside the live-nalog
branch, above the `odsustvoOtkrivenoAt ? … : canUnmask ? …` ternary rather than inside its masked arm,
so after the click the panel read „kategorija odsustva se ne prikazuje … podatak se za to vreme ne
čita iz baze“ directly above „Razlog odsustva je otkriven“. `razlog_odsustva_dostupan` returns **true**
the moment the stamp is set, so both `list_month` and `export_month_csv` carry the column again — the
sentence was false for that nalog from that moment, on the one surface where the čl. 46 decision to
widen the processing is made. It is the same defect class T4R2 closed on this same paragraph one
commit earlier. Fixed by splitting the block: the revealed arm now carries its own paragraph
(„Kategorija odsustva se za ovaj nalog ponovo prikazuje u pregledu i izvozu … Otkrivanje ne može da se
povuče … sledeći nalog ponovo počinje sa skrivenom kategorijom“), and the masked arm keeps the
withholding sentence, the „Skriva se sama kolona“ limit and the „Moji sati“ carve-out — none of which
is true of a nalog that is no longer masking. Two assertions added inside „reveals through the port and
then stops offering the control“; **proven red first**, „expected document not to contain element,
found <p>…ne čita iz baze…“.

2. **An unmask taken BEFORE the operator entered was never upgraded when they did, so the čl. 48 log
recorded no disclosure at all.** `reveal_absence_reason` computes `started_at.is_some()` once, at
unmask time, and T2F3 was right that the pre-entry case is an authorisation — but only for the nalog
nobody enters, which is the only continuation `unmasking_before_the_operator_enters_records_no_disclosure`
tests, because it ends in `end_session` taking the revoke branch. On grant 09:00 → reveal 09:05 →
`request_access` 09:10 the operator reads every `kategorija_odsustva` in the month and the log holds
one `unos` with an empty Razlog and an empty Primalac. Every other test in the module calls
`request_access` **before** `reveal_absence_reason`; the reverse ordering was exercised nowhere. Fixed
in `request_access`, inside the existing transaction and after the `started_at` UPDATE: when the nalog
already carries the stamp it appends the same line the entered branch of `reveal_absence_reason`
writes — `otkrivanje` / `support_absence_reason` / `tehnicka_podrska` /
`obradjivac_tehnicke_podrske` / `actor_user_id: None`, the engineer, like the entry line beside it —
so the record is identical whichever order the two acts occur in and the earlier `unos` still stands
as the authorisation. New test `an_unmask_taken_before_entry_is_disclosed_when_the_operator_enters`
asserts exactly one such row, that the earlier `unos` survives, `ChainVerdict::Intact`, and that the
izvod's „Primalac (ZZPL čl. 48 st. 2)“ cell on that row is non-empty, read positionally as column 8.
**Proven red first** on zero matching rows. `reveal_absence_reason`'s doc comment now says the branch
decides what is true at that instant and names `request_access` as the other half of the record.

3. **An ispravka taken while the column is masked silently dropped the absence, and `close_period`
could freeze the loss.** Task 4 checked the ispravka path and concluded it „needed nothing“; it
checked the wrong direction. Nothing prefills the form, so a correction always restates the whole day
— including a category the operator cannot see while masked. Nalog live, vlasnik corrects the hours of
a `sprecenost_rfzo` day: verzija 2 lands with `kategorija_odsustva = NULL` and every bucket at zero,
and since `load_month` accumulates `ukupno` over `!zamenjen` rows the month quietly loses 480 minutes
of statutory absence — into the frozen Class A `klasifikacija_json` if a close follows. Option (a) was
taken: `guard_ispravka_not_taken_blind` (`commands/worktime.rs`) refuses with
`ispravka_razlog_odsustva_skriven` and a Serbian message naming **both** ways through (unmask for this
nalog, or correct the day once the nalog is over). **Blunt on purpose** — any ispravka of a day that
books absence minutes, not only one that drops them, because a rule that refused only the drop is
satisfied by guessing a category, which is the same corruption one click later. **It never reads the
withheld column:** the question is `ukupno_neizvrseni_minuta + obustava_rada_strajk_minuta > 0` on the
predecessor, the same pair `AbsenceCell` renders on, so the structural guard is untouched and the
refusal discloses nothing the masked register was not already showing. The mask question is asked
before the transaction opens and only for a correction. New test
`a_correction_under_a_live_nalog_may_not_silently_drop_the_absence` — refused, no verzija appended, the
month still 480; then the unmask lets the same correction through at 420; then a worked day corrects
freely. **Proven red first**, printing the exact defect (`kategorija_odsustva: None`,
`sprecenost_rfzo_minuta: 0`). The double models the refusal too, with its own vitest.
`WorkTimeModule`'s catch already renders a business message verbatim, so no frontend change was
needed.

4. **The čl. 47 register denied the obrađivač on the entry describing the very rows the mask
withholds.** T4R3 fixed `radno_vreme.vrsta_primalaca`; its sibling `radno_vreme_radne_verzije` — Class
B, the unclosed `work_time_entries` — still said „Ne otkrivaju se nikome van rukovaoca“, and those
rows are exactly what `list_month`/`export_month_csv` return for an open month. Fixed in the two
sibling entries' own words, with the „iz njih se izvodi klasifikacija“ clause kept, and
`one_register_entry_states_the_absence_reason_mask_and_no_other_does` extended with the same two-needle
assertion it already makes for `radno_vreme`. **Proven red first** on the shipped constant.

5. **Both „Moji sati“ carve-out guards asserted a phrase, not a claim.** `cl47.rs` checked
`mere.contains("moji sati")` and the vitest `getByText(/moji sati/i)`, so the exact inverse — „Pregled
„Moji sati“ se takođe skriva dok nalog važi“, the falsehood T4R2 was opened to remove — kept both
green while both surfaces denied a carve-out `my_hours` keeps open. Both now read the **sentence**
around the subject and require a withholding verb only under a negation: the Rust half through a new
`recenica_oko` helper that treats a full stop as a boundary only before a capital or an opening „, so
„ZZPL čl. 26“ does not cut the clause in half; the vitest half through the same split plus a
negation-strip. **Both proven red by pasting the inverted sentence** into `radno_vreme.mere` and into
the panel, then reverted.

6. **The mock's idempotency test could not fail on the mutation it is named for.** `mock-adapter.ts`'s
`now` is a frozen module constant, so `revealAbsenceReason`'s `?? now` could be deleted — the double
re-stamping on every call — and `expect(drugi.odsustvoOtkrivenoAt).toBe(prvi.odsustvoOtkrivenoAt)`
still passed on two copies of one string. Fixed with `sledeciTrenutak()`, an instant that advances one
second per call, used by the two `??`-guarded stamps (`revealAbsenceReason` and `enterSupportSession`);
the assertion stands and the test now also witnesses that the clock moved. **The mutation was applied
first to confirm the test was vacuous (6 passed), then re-applied after the fix to confirm it bites
(„expected '…10:00:03Z' to be '…10:00:02Z“), then reverted.**

7. **Two Rust doc comments still claimed the mask covers the rendered grid.** `load_entries_without_reason`
and — worse — the doc comment of `the_mask_covers_the_zzpl_column_and_not_the_zeor_letters`, which
register row 12 cites as the pin for the residual, so a future author reading the pin was told the grid
is closed. It is not: the grid draws b), Efektivno izvršeni and Čekanja i zastoji as separate columns
and `derive_totals` makes b) their sum plus `obustava_rada_strajk_minuta`, so the tenth category comes
off the drawn register by subtraction — which `types.ts`, `PROGRESS.md` and row 12 all already said.
Both restated in the house pattern with the withdrawn sentence quoted and dated. Comment-only; the
assertions were always right.

8. **„withholds the category from every register read“ — `my_hours` contradicts it.**
`PROGRESS.md:340` and register row 12 both carried it; `my_hours` calls `load_month(.., true)`
unconditionally and never asks the question, which
`my_hours_still_shows_the_employee_their_own_absence_reason` pins. Both re-stated to name
`worktime_list_month` and `worktime_export_csv` and to carry the čl. 26 carve-out, quoting what was
withdrawn. `docs_guard::no_document_says_the_absence_reason_mask_is_unbuilt` gained a **third half**:
any block naming the mask and claiming „every register read“ / „every read“ must name the exception.
It **found a second instance the two existing halves had passed** — `PROGRESS.md`'s „What changed, in
one sentence“ block, which now carries the carve-out too — and was **proven red on the primary target**
by restoring the original sentence, then reverted.

9 and 10. **The mask lifts when the nalog is ended or outlived, with no čl. 48 line, and that route
was recorded nowhere.** `end_session` carries the *same* `require_admin` gate as the unmask, so an
operator driving the vlasnik's screen closes the nalog and reads the column on the next read at the
cost of a `menjanje` row that says only that the nalog closed; the same lift arrives at `expires_at`
with no action at all. Strictly cheaper than the credential-reset chain the residual inventory records
in detail, and absent from all four places that inventory lives. Not fixed — an app cannot stop
someone already inside an admin session, the two code answers (an explicit re-arm, or keying the mask
on „a nalog was live at any point in this app session“) are design decisions, and a post-session mask
would contradict `an_expired_nalog_withholds_nothing` head-on. **Recorded in all four places** —
`razlog_odsustva_dostupan`'s doc comment, this plan's Task 3 residual, `PROGRESS.md` §6 W-15 as item 3
beside the credential-reset chain, and register row 12 — and **pinned as deliberate** by the new
`ending_the_nalog_lifts_the_mask_and_logs_no_disclosure`, which asserts both halves: the mask lifts one
minute after the click, and no line in the log names `support_absence_reason` on that route.

**Deviation — one behaviour changed on a finding the review filed as „major“, not as a bug.** Finding 3
offered three options and this pass took (a), the refusal, which is the only one of the three that a
second frontend cannot route around and the only one that protects the frozen Class A record. It
narrows what an operator may do during a live nalog: an absence day is not correctable until the nalog
is unmasked or over. That is stated on the guard, in the refusal itself and here.

**Deviation — `enterSupportSession` in the double got the advancing instant too**, though only
`revealAbsenceReason` was named. Both carry the same `?? now` idempotency shape and the same
untestability; leaving the sibling frozen would have left a known-vacuous assertion in place for
whoever writes the next test against it.

**Not changed, and why.** `unmasking_before_the_operator_enters_records_no_disclosure` was left exactly
as it is — it is right about the nalog nobody enters, and the new test covers the continuation rather
than replacing it. `an_expired_nalog_withholds_nothing` was left alone for the same reason finding 9
names it: a post-session mask would contradict it, and no rule nobody asked for was invented here.

**Nothing was deleted, weakened or skipped.** Three tests gained assertions
(`reveals through the port and then stops offering the control`,
`does not deny the „Moji sati“ carve-out the backend keeps open`,
`one_register_entry_states_the_absence_reason_mask_and_no_other_does`,
`keeps one disclosure per nalog rather than re-stamping`) and four tests are new: three cargo
(`an_unmask_taken_before_entry_is_disclosed_when_the_operator_enters`,
`a_correction_under_a_live_nalog_may_not_silently_drop_the_absence`,
`ending_the_nalog_lifts_the_mask_and_logs_no_disclosure`) and one vitest
(`refuses a correction taken while the category is masked`).

**Gates after the review pass (six, run once each from the repo root, never concurrently, every command
exit `0`):** `cargo test` **1043 passed** / 0 failed, 0 ignored, 0 measured, 0 filtered out (was 1040);
`cargo clippy --all-targets --all-features --locked -- -D warnings` clean, zero warnings;
`cargo fmt --check` clean, no output (rustfmt applied once, over test-body wrapping); `bun run test`
**569 passed** / 0 failed, 36 files (was 568); `bun run build` ok, only the pre-existing chunk-size
advisory; `git diff --check` clean. Migration head unchanged at **v23** — this pass adds none.

---

## Self-review

**Coverage.** Req. 28's three limbs map to Tasks 3 (mask the column), 2 (log the unmask), and 4 (the shop explicitly unmasks). „Mask the payroll screens“ is Task 4's frontend half. Migration v23 carries the stamp Task 2 writes and Task 3 reads.

**Placeholders.** Tasks 1–5 name required behaviours and exact assertions rather than pasting bodies, following the repo's seeding pattern. Every behaviour is a concrete assertion against a named symbol.

**Type consistency.** `support_reveal_absence_reason` (Task 2) returns the existing `SupportSession`, which gains one field in Task 1 and is already understood by the frontend — so no new wire type. `AuditObjectType::SupportSession` already exists.

**One gap found during review, folded in.** Task 3 originally said „mask the three read paths“ uniformly, which would have masked `my_hours` — the employee's view of their **own** absence reason — on the strength of a rule written about the vendor. An employee is not remote support, and req. 28 says nothing about hiding a person's own data from them; ZZPL čl. 26 points the other way. Task 3 now requires that path to be reasoned about and the decision recorded.

**One thing this plan deliberately does not do.** It does not mask any other column. The absence reason is singled out because it is the one field in this register that is health-adjacent; masking more would be inventing a rule, and masking less would miss req. 28's subject.
