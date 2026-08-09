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

- [ ] **Step 1: Write the failing tests** — unmasking with no live session is refused with a typed error; unmasking stamps the column and writes exactly one audit row; the audit row names the session and carries no `kategorija_odsustva` value (assert every one of the ten v17 category strings is absent from the serialised row); a second unmask does not re-stamp and does not write a second row; a cashier is refused; the chain still verifies afterwards.
- [ ] **Steps 2–5.**

---

### Task 3: Mask the three read paths

**This is the load-bearing task.** Today `kategorija_odsustva` leaves the backend on every read and the screen decides. After this task the **backend** decides, and the frontend gate stays only as a second layer.

The three paths in `commands/worktime.rs`: `list_month` (the row build at ~:985), `export_csv` (~:1380) and `my_hours`. Work out for each whether it is reachable under a support nalog and mask accordingly.

**`my_hours` is the exception and must be reasoned about, not copied.** It is the employee's own view of their own month, resolved from the server-side session. Decide deliberately whether a support operator can even reach it, and record the decision — do not mask it reflexively if the answer is that they cannot.

Mask by **withholding at the query or the row build**, in the shape `commands::popis::read_lines` uses for čl. 8 st. 5: while masked, the value is `None` because nothing was read, not because something read was dropped. A `SELECT` that fetches the category and blanks it in Rust looks identical from outside and is a different thing — the value exists, one refactor or one debug print from the support operator's screen.

The masked read must remain **truthful**: an entry with an absence still reports that there **is** an absence and its minutes. Only the *reason* is withheld. A masked read that hides the absence itself would corrupt the register's totals on screen.

- [ ] **Step 1: Write the failing tests** — with a live nalog and no unmask, `list_month` returns entries whose `kategorija_odsustva` is `None` while `odsustvo_minuta` is unchanged; `export_csv` writes no category value into the file (assert on the **file bytes**, not the return value); after `support_reveal_absence_reason` the same reads carry the category; with **no** live session nothing is masked (the payroll role's ordinary day is untouched); an expired session masks nothing, because the nalog is over. Add a source-level guard mirroring `the_phase_a_row_renderer_never_names_a_phase_b_field` if the masked path can be expressed as its own code path.
- [ ] **Steps 2–5.**

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
