# VantumPOS — Implementation Progress Report

> **✅ Status update (2026-06-26):** All 12 recommendations in this report are now **implemented** on branch `feat/mvp-completion`. See the **MVP Completion Update** section immediately below. The rest of this document is the original audit baseline (the "before").

**Date:** 2026-06-26
**Overall status:** The application is a genuinely working, local-first POS — every one of the 9 modules is real (no placeholder screens, no stub commands), with green builds and tests; the remaining work is polish, audit-attribution wiring, and test-breadth rather than architecture.
**Overall completion:** **88%** (simple average of the 9 audited module percentages: 94, 91, 92, 86, 90, 86, 80, 90, 80 → 789 / 9 = 87.7 ≈ 88%).
**Basis:** All figures are *audited* estimates measured against `docs/module-specs`. Where the independent adversarial audit disagreed with the original assessment, the audited number is treated as the source of truth and the disagreement is called out per module.

---

## MVP Completion Update (2026-06-26)

The 12 recommendations below were implemented as a 13-task TDD plan
(`docs/superpowers/plans/2026-06-26-vantumpos-mvp-completion.md`, design at
`docs/superpowers/specs/2026-06-26-vantumpos-mvp-completion-design.md`) on branch `feat/mvp-completion`.
Each task was implemented by a fresh agent, reviewed for spec compliance + quality, fixed where needed,
and a final adversarial whole-branch review passed **Ready to merge: Yes**.

**Outcome:** MVP scope is complete — all 12 recommendations closed; every touched module now meets its
`docs/module-specs` Acceptance Criteria and the 7-point Definition of Done. The role-gating model is now
coherent and complete across the app.

**Final verification gates (all green at HEAD):**

| Gate | Result |
|---|---|
| `bun run test` | **89 passed** / 0 failed (was 57) |
| `bun run build` | pass (tsc + vite) |
| `cargo test -- --test-threads=1` | **98 passed** / 0 failed (was 67) |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | clean |
| `cargo fmt --check` | clean |

**Recommendations closed:**

| # | Item | Status | Key commits |
|---|---|---|---|
| 1 | Auth/shift backend tests (deactivated-login, close-shift expected-cash) | ✅ Done | `234e3bb` |
| 2 | Admin role-gating for receipt-sequence updates | ✅ Done | `64e145d`, `32b754b` |
| 3 | VAT rate edit/deactivate workflow | ✅ Done | `4a976ff` |
| 4 | Thread real session user into Inventory & Receipts (drop `userId:1`) | ✅ Done | `4d492d5` |
| 5 | Render swallowed preview/validation error in Register | ✅ Done | `b98d610` |
| 6 | Receipts loading/empty/error states + void/return reason text | ✅ Done | `102ec81` |
| 7 | Reports shift/cashier backend filters + role-gate + FE state tests | ✅ Done | `699c4f9` |
| 8 | Remove dead `ProductCatalogScreen.tsx`; deep-link product ledger | ✅ Done | `c3c54bc` |
| 9 | Import job-detail drill-down + unknown-VAT/required-mapping tests; docs count fix | ✅ Done | `93fe051` |
| 10 | Settings/Backup component tests + age-based stale detection | ✅ Done | `3c06647` |
| 11 | Shell header reads company name + forward-migration data-survival test | ✅ Done | `ce9d5c1`, `199031e` |
| 12 | Reconcile sales/inventory write divergence (shared `write_stock_movement`) | ✅ Done | `1ecd922` |

**Role-gating model (final, coherent):** one authoritative server-derived `require_admin` (in `auth.rs`,
unspoofable — derived from the session, never a client-supplied role). Gated: all state-changing settings
writes (company / tax rate / receipt numbering), the 4 user-management commands, the 3 state-changing
backup commands (incl. the destructive `backup_restore`), and all 8 `reports_*` commands (Reports is an
admin-only surface). Left open (consumed by non-admin flows): settings/company/tax **reads** (shell header,
VAT calc) and backup status/list reads. Commits `790553a`, `b1366a5`.

**Other correctness wins:** sales and inventory now share a single transaction-safe stock-write helper
with a consistent negative-stock guard; the shell header silently falls back to "VantumPOS" (now with a
diagnostic warn) if the company read fails; the forward-migration regression test now asserts real v1 data
survives the v2 table rebuild.

**Follow-up shipped (2026-06-26):** the shift/cashier report filters are now surfaced in the UI — two
selects ("Smena" / "Kasir", default "Sve smene" / "Svi kasiri" = unfiltered) on the Reports screen, with
a dedicated admin-gated `reports_list_shifts` command sourcing the shift list and `users_list` sourcing
the cashiers. The filters apply across **all sales-derived sections** (daily turnover, payments, product
and category sales, and CSV export); `query_low_stock` is intentionally unfiltered (inventory state).
Branch `feat/reports-shift-cashier-filters` (commits `7d8174e`, `e9c6e5e`), all gates green (bun 95,
cargo 103), reviewed Ready to merge. Remaining minor review nits (logged during execution) are
non-blocking.

---

## Compliance Backlog Update — SW-11 + SW-15 (2026-07-31)

The cash & product compliance trio (**SW-11a/b/c**) and the **SW-15** onboarding profile shipped as a
22-task TDD plan (`docs/superpowers/plans/2026-07-31-sw11-sw15.md`, design at
`docs/superpowers/specs/2026-07-31-sw11-sw15-cash-product-onboarding-design.md`, verified legal rule set
at `docs/SW11-SW15-VERIFIED-RULES.md`). Register rows 21, 27, 28 and 29 in
`docs/SERBIAN-LAW-COMPLIANCE.md` were re-stated accordingly.

| Item | Shipped as |
|---|---|
| SW-15 onboarding profile | `settings.rs::ShopProfile` — `pravna_forma` / `pdv_obveznik` / `distance_selling`, all tri-state and **never inferred**; `EsirElement` rows kept as an *interna beleška o proveri*, never an *evidencija* |
| Tier-resolved legal copy | `legal.rs` — the only module allowed to hold a fine figure; every SW-11/SW-15 figure resolves through it, and a test pins that "privredni prestup" is unreachable under the preduzetnik regime. **Known exception, pre-existing (SW-7):** `src/app/reklamacije/ReklamacijeModule.tsx:128` still hard-codes the regime-versioned reklamacija amounts (`30.000` / `100.000`) and renders them at line 497 — to be folded into `legal.rs` as an SW-7 follow-up |
| SW-11a AML cash cap | `aml.rs` (inclusive `>=`, subject is the cash line of the tender) + `nbs_rate.rs` (zvanični srednji kurs, manual fallback, no silent pass); asked via `sales_assess_cash_payment`, re-evaluated and persisted by `sales_complete`; `bank_transfer` tender ships the statute's own remedy |
| SW-11b polog aging | `cash_deposit.rs` — working-day arithmetic over a seeded, operator-editable `non_working_days` table, FIFO buckets clocked from **receipt**, deadline as a date, and (narrowed 01.08.2026, migration v16) only a `bank_withdrawal` the operator asserts as `documented_per_pravilnik` leaves the subject base. Advisory only, no hard block |
| SW-11c deklaracija | declaration columns on `products` (v15) + `catalog.rs::validate_declaration` (mandatory **iff** `distance_selling`), `catalog_declaration_gaps` report, goods-receipt warnings in `inventory.rs` |
| Go-live reset | preserves configuration (`shop_profile`, `eur_rate`, `non_working_days`), clears compliance state, and discloses the deklaracija wipe |

**Verification gates at the end of the 22-task plan (`f96df45`):** bun 274 passed / 19 files,
cargo 456 passed, build + clippy + fmt + `git diff --check` clean.

### SW-11 / SW-15 fix batch (2026-08-01)

A post-ship adversarial review of the module against `docs/SW11-SW15-VERIFIED-RULES.md` found six
defects (D1, D3, D5, D6–D8) plus four documentation errors. All are now closed on `master`.

| Defect | What was wrong | Fixed by |
|---|---|---|
| **D1 — blocking** | `settings_get_eur_rate` / `settings_refresh_eur_rate` / `settings_set_manual_eur_rate` existed in Rust but were absent from `PosServices`, so a **real install had no way to obtain a rate at all**. `sales_assess_cash_payment` therefore always returned `rateUnavailable` and the AML čl. 46 st. 1 check could never run outside the mock adapter | `2b586ee` — the three commands added to `ports.ts` / `local-adapter.ts`, registered in `lib.rs`, and surfaced as the admin **„Kurs“** tab (`SettingsScreen.tsx::EurRatePanel`): NBS refresh, band-checked manual fallback, staleness verdict |
| **D3** | The till's AML verdict is debounced; between the last keystroke and the verdict landing, „Završi prodaju“ decided the soft block on the *previous* tender — a fast operator walked straight through a breach | `5fc0390` — `RegisterScreen` tracks `assessedCashMinor` and re-asks `assessCashPayment` for the exact cash line about to be booked before deciding |
| **D5** | Answering **„ne“** to the L-PFR question surfaced nothing. §3 req 32 requires the ZF čl. 6 st. 4 duty **and** its čl. 15 st. 1 tač. 4 penalty at the shop's own tier, unless one of the two statutory carve-outs is claimed | `3c724c0` — `legal.rs::lpfr_required` (tier-resolved, enumerated in the `all_notices` guard), `settings_lpfr_notice`, and the two carve-out tri-states (`lpfrCarveOutInternetOnly`, `lpfrCarveOutOwnUsedAssets`) on `ShopProfilePanel` |
| **D6–D8** | The per-sale verdict read as clearance for the buyer; čl. 46 st. 1 also reaches linked cash transactions and contracts inside one year, which the software cannot compute (§3 req 5 requires that limitation be stated **in writing**) | `848dcb4` — disclosure on the till's AML notice and an `AmlAggregationDisclosure` panel beside the rate settings |
| Docs | Gate baselines, the `legal.rs` exclusivity claim, register row 27 and the retention citation were overstated; the `reset_trading_data` tombstone still cited **ZPDV čl. 47** as the general retention floor | `9fa88b8` + `backup.rs` — the tombstone now cites **ZoRač čl. 28 st. 4 + ZPPPA čl. 114ž**, with a test that bars `ZPDV` from that string |

**Verification gates (all green at HEAD, `848dcb4`):**

| Gate | Result |
|---|---|
| `bun run test` | **296 passed** / 0 failed, 19 files (was 274 pre-batch) |
| `bun run build` | pass (tsc + vite, 2732 modules) |
| `cargo test -- --test-threads=1` | **460 passed** / 0 failed (was 456 pre-batch) |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `git diff --check` | clean |

**Still open after that batch** — superseded by the residuals batch below, which closed D2, D4 and the
D1 residual. (Requirement numbers are `docs/SW11-SW15-VERIFIED-RULES.md` §3.)

- **Req 39 / §4 item 8 — false archive duty still on screen.** `src/app/settings/SettingsScreen.tsx:1773`
  tells the operator „Pravna lica ne smeju uništavati dokumentarni materijal bez pismenog odobrenja
  arhiva.“ ZAG čl. 16 st. 2 confines prior written archive approval to the **public sector**; the memo
  (§1 row 5) requires that claim removed. Pre-existing (SW-3, `062250d`), not introduced by this batch.
- **Req 35–38, 42, 43 — retention engine.** No `retention_class`, `retain_until`, `legal_hold` or
  upward-only extension exists anywhere in the schema; the čl. 32 objekti/ulaganja register (req 37) and
  the documented plain-text archival export (req 43) are not built. The KEP book carries its own 5-year
  floor in `kep_close.rs::retention_floor`. The same screen line (`SettingsScreen.tsx:1771`) still
  attributes the 10-year floor to **ZPDV čl. 47** — the miscitation corrected in `backup.rs` was not
  carried into the UI copy.
- **Req 5(b)(c) — one-year aggregation.** The limitation is now disclosed, but the optional buyer tag and
  the rolling 365-day running total are not built. `[PRUDENTIAL]` mechanism; blocked on §5 Q-3 (lawful
  ZZPL basis for a customer-identity store).
- **Req 17 — polog evidence trail.** The „Izveštaj o nedeponovanom gotovom novcu“ renders on screen; no
  printable/CSV export for the knjigovođa.
- **Req 29 — preduzetnik constant lint** (>500.000 range / >150.000 fixed) is not implemented; the
  `legal.rs` guard covers the forbidden substrings and the UNSET case only.
- **Req 40 — SEF / e-otpremnica copy** does not exist yet, so the korporacijska-kartica and
  public-sector-buyer exceptions have nowhere to be stated.
- **`legal.rs` exclusivity, known exception:** `src/app/reklamacije/ReklamacijeModule.tsx:128` still
  hard-codes the regime-versioned reklamacija amounts — an SW-7 follow-up.

### SW-11 residuals batch — migration v16, D4, D2, D1-residual (2026-08-01)

The three defects the previous batch left open. `src-tauri/src/legal.rs` is **untouched** by this batch
(`git diff 891d40a..HEAD --name-only` returns no `legal.rs`), and the only Serbian-formatted figure it
introduces is **150.000** — the Pravilnik 77/2011 čl. 2 st. 3 daily undocumented-withdrawal lane, a
statutory threshold, not a fine.

| Defect | What was wrong | Fixed by |
|---|---|---|
| **Schema** | `compliance_log.event_type` admitted only `('trading_data_reset','backup_restored')`, so the AML audit row had nowhere to land; `cash_movements` had no way to record the Pravilnik čl. 2 st. 2/st. 3 assertion | `4068727` — **migration v16**. Table rebuild widens the CHECK to admit `aml_cash_threshold` (`migrations.rs:579`); `ALTER TABLE cash_movements ADD COLUMN documented_per_pravilnik INTEGER CHECK (… IS NULL OR … IN (0,1))` (`migrations.rs:593`), nullable and **deliberately not backfilled** |
| **D4 — req 8** `[PRUDENTIAL]` | The čl. 46 st. 1 soft block captured a reason but wrote no immutable audit entry, so an inspection could not reproduce the decision from the log alone | `d92cca6` — `sales.rs::insert_aml_breach_event` (`sales.rs:679`) writes one `aml_cash_threshold` row **on the sale's own transaction** (`sales.rs:297–313`), carrying sale id, local receipt number, cash in para, threshold, rate + date + source, and the operator's reason. Stamped with the sale's `created_at`, never `datetime('now')`. A `near_threshold` warning writes nothing — nothing below the cap is unlawful |
| **D2 — req 14** `[LEGAL]` | `load_cash_inflows` carried **every** `bank_withdrawal` with `subject = false`. Pravilnik 77/2011 čl. 5 st. 2 relieves only dinars paid out per čl. 2 st. 2 or st. 3; req 14 says in terms *do not exclude all bank withdrawals*. Shrinking the subject base hands the owner the false „izmireno“ state req 10 forbids, and a withdraw-then-redeposit float cycle under-reported twice | `b0adfe6` — the `bank_withdrawal` arm is split in two (`cash_deposit.rs:664–676`): only `documented_per_pravilnik = 1` returns `subject = 0`; `NULL` and `0` are **both** subject, written as an explicit arm because `= 1` and `<> 1` do not partition rows in SQL. Recorded per movement (`shifts.rs:153–157`), dropped on any direction other than a podizanje. Report footer, CSV, summary tile and module docs now state the narrowed rule and still name it bylaw-level relief |
| **D1 residual** | Podešavanja → Kurs gave the rate an operator surface, but nothing ever fetched it. On a fresh install `load_eur_rate()` stayed `None`, so the čl. 46 st. 1 check silently did not run until an admin happened to find the tab | `25168ba` — `settings.rs::auto_refresh_eur_rate_if_due` (`settings.rs:620`) attempts one NBS fetch on the first **admin** session of a calendar day, kicked off detached from `auth_login` (`auth.rs:68–74`) so it can never delay a login, sale or shift. The last **attempt** is persisted (`settings.rs:651`), not the last success, so an offline shop pays the 8 s timeout once a day. Plus an admin-only readiness badge in the shell status strip (`AppShell.tsx:515–583`) that names the unrun check, says selling is not blocked, and carries no penalty figure |

**Verification gates (all six green at HEAD, `25168ba`; every command exited `0`):**

| Gate | Result |
|---|---|
| `bun run test` | **304 passed** / 0 failed, 19 files (was 296) |
| `bun run build` | pass — tsc + vite, 2732 modules transformed |
| `cargo test -- --test-threads=1` | **476 passed** / 0 failed, 0 ignored (was 460) |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `git diff --check` | clean |

Latest migration: **v16**.

**Still open after this batch** (requirement numbers are `docs/SW11-SW15-VERIFIED-RULES.md` §3):

- **Req 39 / §4 item 8 — false archive duty still on screen.** `src/app/settings/SettingsScreen.tsx:1773`
  still tells the operator „Pravna lica ne smeju uništavati dokumentarni materijal bez pismenog odobrenja
  arhiva.“ ZAG čl. 16 st. 2 confines prior written archive approval to the public sector. Pre-existing
  (SW-3, `062250d`); untouched by this batch.
- **Req 35–38, 42, 43 — retention engine.** No `retention_class`, `retain_until`, `legal_hold` or
  upward-only extension in the schema; no čl. 32 objekti/ulaganja register (req 37); no documented
  plain-text archival export (req 43). `SettingsScreen.tsx:1771` still attributes the 10-year floor to
  **ZPDV čl. 47** — the miscitation corrected in `backup.rs` was never carried into the UI copy.
- **Req 5(b)(c) — one-year aggregation.** Disclosed in writing, but the optional buyer tag and the rolling
  365-day running total are not built. `[PRUDENTIAL]`; blocked on §5 Q-3.
- **Req 15, second half — the 3-day advance announcement** for withdrawals over 1.500.000 RSD
  (Pravilnik čl. 3 st. 1) is not surfaced anywhere. The 150.000 RSD/day lane now is. `[PRUDENTIAL]`.
- **Req 17 — polog evidence trail: CSV ships, print does not.** *Correcting the previous batch's entry* —
  the CSV export exists end to end (`commands/cash_deposit.rs:41` → `ports.ts:360` →
  `CashDepositReport.tsx:95` „Izvezi CSV“). Only the printable view for the knjigovođa is missing.
- **Req 29 — preduzetnik constant lint** (>500.000 range / >150.000 fixed) is not implemented; the
  `legal.rs` guard covers the forbidden substrings and the UNSET case only.
- **Req 34 — ESIR re-confirmation prompt** (annual, or on any reported ESIR update) is not built; the
  registry check is still one-time.
- **Req 40 — SEF / e-otpremnica copy** does not exist yet, so the korporacijska-kartica and
  public-sector-buyer exceptions have nowhere to be stated.
- **Req 9 — rule-version date gating** is conditional on a historical/retrospective AML report, which is
  not built. Not a gap today; a precondition on that report.
- **`legal.rs` exclusivity, known exception:** `src/app/reklamacije/ReklamacijeModule.tsx:128` still
  hard-codes the regime-versioned reklamacija amounts — an SW-7 follow-up, in flight separately.

---

### SW-14 — Evidencija radnog vremena (2026-08-01)

The ZoR čl. 55 st. 6 daily overtime register, the čl. 53 caps that carry the larger fine, the čl. 87–91
protection guards and a two-class retention model, shipped as a ten-task TDD plan
(`docs/superpowers/plans/2026-08-01-sw14-radno-vreme.md`) against the verified rule set in
`docs/SW14-VERIFIED-RULES.md` §4. Baseline at plan time was `907017c` — cargo **476**, bun **304**,
migration **v16**.

| Task | Shipped | Commits |
|---|---|---|
| 1 — schema | **Migration v17**: `work_time_entries` (one row per employee/day, the 15 ZEOR čl. 24 tač. 1 minute buckets, versioned append-only correction chain, a partial unique index giving one live row per day), `work_time_periods`, `retention_policies`, employee-profile columns on `users`. Absence category is a closed `CHECK` enum with **zero free-text columns**, and the bucket column is **derived** from the category (`{kategorija}_minuta`), enforced by a test that parses the live `CHECK` | `4265190`, `6f9de79`, `6b41c43` |
| 2 — caps | `worktime.rs::assess_caps` — čl. 53 st. 2 (≤ 8 h prekovremenog per **calendar week, Monday-based**) and st. 3 (≤ 12 h daily total incl. overtime); civil-date arithmetic reused from `cash_deposit.rs` rather than re-derived. The stored version of the day under assessment is not double-counted | `d5ad982`, `b3a2d4c` |
| 3 — penalty copy | `legal.rs::overtime_record_missing` (čl. 276 st. 1 u vezi sa tač. 1a) and `overtime_caps_exceeded` (čl. 274 st. 1 tač. 3), both tier-resolved from `pravna_forma`; the čl. 276 st. 2 odgovorno-lice line is suppressed for a preduzetnik. A test guards that **no ZEOR figure** is reachable anywhere, and three previously vacuous guards were closed | `6ae257e`, `c0fc959` |
| 4 — protection | `worktime.rs::check_protection` — čl. 87 (8 h/day for a minor; the 35 h/week leg is not checked), **čl. 88 st. 1 bans prekovremeni *and* preraspodela**, čl. 91 st. 1 (dete do 3) and st. 2 (**samohrani roditelj — threshold SEVEN**, plus `dete_tezak_invalid` with no age limit) require a stored written consent **dated before the day worked**, čl. 90 warns rather than blocks. `derives_overtime_automatically` returns `false` under preraspodela — čl. 58 hours are not overtime | `defcecf`, `8c04a05` |
| 5 — commands | `commands/worktime.rs`: `worktime_list_month`, `save_entry`, `correct_entry`, `close_period`, `export_csv`, `my_hours`, `notices`. Not one `UPDATE` against `work_time_entries`; a correction is a new `verzija` row carrying who/when/why. `require_admin` is the **first statement of the domain function**, not of the `#[tauri::command]` wrapper. A cap breach records the day and asks for a čl. 53 st. 1 ground; a čl. 87–91 block refuses the row. Preraspodela is branched onto the čl. 57 st. 5 60 h/week ceiling, and a month cannot close before it ends | `5a87e4a`, `93c95e7` |
| 6 — retention | `retention.rs` — the single shared table SW11-SW15 §3 req. 42 mandates. `WorktimeClassification` = `trajno` + `never_purge`, unreachable by the go-live reset (`assert_never_purge_intact` runs **inside** that transaction in `backup.rs`) — **restore is not fenced and cannot be**: `restore_backup` replaces the database file, so the register returns as the snapshot holds it, the `pre_restore` safety copy is the only protection on that path, and the `backup_restored` event records the never-purge counts on both sides; a count-based refusal cannot tell a legitimately older backup from data loss and would leave a shop unable to restore at all. A backup-prune path **does not exist** in the crate, so that limb guards nothing; `WorktimeOvertimeLog` carries an upward-only **3-year** floor applied to each record's own `dan`, with a fail-safe that refuses to purge when no floor is stored; `WorktimeDraft` is bounded by the period close | `ee7fcca`, `0c96a6a` |
| 7 — Radno vreme UI | `src/app/worktime/WorkTimeModule.tsx` — monthly grid, cap warnings with the override ground, period close, CSV export, and the absence category behind `canSeeAbsenceReason`. Non-blocking čl. 87–91 findings are surfaced rather than swallowed, and the recorded day is guarded | `6807309`, `2f576d7` |
| 8 — Moji sati | `src/app/worktime/MyHoursPanel.tsx` — read-only own-month view resolved from the server-side session, **no employee picker and no export control** (ZoR čl. 83 st. 1 + ZZPL čl. 26 in one surface). Reachable off-shift | `c57b9bd`, `ae9c00d` |
| 9 — employee profile | `commands/users.rs` — the čl. 87–91 flags plus ZEOR čl. 44 st. 2 `zanimanje_sifra` / `kvalifikacija_sifra` as codes. **No consent UI**: `saglasnost_prekovremeni_od` records that a written consent exists and when. `trudnoca_ili_dojenje` is a boolean + date, never free text, and is kept out of `users_list` | `f796de3`, `a69f1b0` |

**Verification gates (all six green at HEAD; every command exited `0`):**

| Gate | Result |
|---|---|
| `bun run test` | **355 passed** / 0 failed, 22 files (was 304 / 19) |
| `bun run build` | pass — tsc + vite |
| `cargo test -- --test-threads=1` | **549 passed** / 0 failed, 0 ignored (was 476) |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `git diff --check` | clean |

New tests: 29 in `worktime.rs`, 14 in `commands/worktime.rs`, 9 in `retention.rs`, plus additions in
`legal.rs`, `db/migrations.rs`, `commands/users.rs` and `commands/backup.rs`; 50 frontend tests across
`WorkTimeModule.test.tsx`, `MyHoursPanel.test.tsx` and `UserDialog.test.tsx`.

**Review fix (01.08.2026).** Four claims in this file and in `SERBIAN-LAW-COMPLIANCE.md` described legs
the code does not have: the čl. 87 weekly cap credited to `check_protection`, the čl. 57 st. 5 ceiling
credited to `assess_caps` rather than to its caller, an unqualified "no fine figure outside `legal.rs`",
and — in the notice handed to employees — ZZPL čl. 95 st. 1 **tač. 20** where the čl. 23 offence is
**tač. 8**. All four are corrected above, and the defect class now has a guard:
`src-tauri/src/docs_guard.rs` (test-only) reads the three documents and fails when a compliance row
claims more than the code delivers. `worktime.rs::the_cl_87_weekly_leg_is_not_checked` pins the gap in
behaviour — six eight-hour days raise nothing — so building the leg breaks the test and forces the prose
to be re-stated in the same commit.

Latest migration: **v17**.

**Deliberately not built** (`docs/SW14-VERIFIED-RULES.md` §5) — read these as decisions, not as gaps:
payroll of any kind (čl. 24 tačke 2–3 are the accountant's), an obračun zarade, the čl. 108 uplifts,
any free-text/diagnosis/doznaka field on an absence row, biometric clock-in, an annual overtime counter,
a `hours > 8 ⇒ prekovremeni` rule during preraspodela, an employee-side export, and any consent UI.

**Still open after this batch** (requirement numbers are `docs/SW14-VERIFIED-RULES.md` §4):

- **Req 12, weekly leg — the čl. 87 cap of 35 časova nedeljno for an employee under 18 is not checked.**
  `check_protection` enforces only the 8 h/day leg (`MINOR_DAILY_CAP_MINUTES`); the weekly leg needs the
  employee's week, which that signature does not carry, and no caller supplies it. The under-18 čl. 88 st. 1
  bans on prekovremeni and preraspodela *are* enforced, so a minor cannot accumulate the week through
  overtime — but a minor scheduled 7 h a day across six days raises nothing.
- **Req 18, restore leg — the trajno classes are fenced against the go-live reset, not against a restore.**
  `assert_never_purge_intact` runs inside the `reset_trading_data` transaction. `restore_backup` replaces the
  database file, so `work_time_entries`, `work_time_periods` and `retention_policies` come back exactly as the
  restored snapshot holds them. Asserting over the counts there is **not** the fix: a snapshot older than the
  newest worked day holds fewer rows *because it is older*, so a refusal would turn away routine disaster
  recovery and leave a shop with any register rows at all unable to restore — and the file is already replaced
  by the time anything can be counted, so the only „abort“ is a second overwrite from the safety copy. What
  ships instead: the `pre_restore` safety copy stays mandatory, and `backup_restored` records the never-purge
  row counts on both sides so a rewind of the ZEOR čl. 25 st. 3 register is evidenced rather than silent. A
  real fix is a merge that carries the trajno rows across, and it needs to tell one database's `user_id` from
  another's before it can be trusted. The čl. 23 notice, the register row and the stored `napomena` were
  restated to claim only this, and `docs_guard.rs` pins them.
- **Req 16 — the holiday calendar is not encoded.** `rad_na_praznik_minuta` is an operator-entered,
  advisory-tagged bucket; the Zakon o državnim i drugim praznicima čl. 1/1a/2/3a rules and the čl. 3/čl. 5
  working-holiday exclusion set are not in code, so nothing derives the čl. 108 st. 1 tač. 1 flag.
- **Req 15 `[PRUDENTIAL]` — rest-period checks** (čl. 64, 66, 67) and their preraspodela variants are not
  implemented.
- **Req 10 — the 9-month reference period** behind `kolektivni_ugovor_postoji`, and the čl. 61
  mid-period-termination choice, are not surfaced.
- **Req 14 — the čl. 62 st. 2 night threshold** advisory flag is not computed. Consequently the
  *„odnosno noću“* leg of čl. 90 and čl. 91 is **not** enforced either: `nocni_minuta` is an
  operator-entered advisory bucket and is not visible in `DayHours`, so `check_protection` draws those
  guards on the overtime leg alone. A protected employee scheduled at night with no overtime raises
  nothing.
- **Req 26, second half — the activation gate.** The čl. 23 notice now carries the posebne-vrste row, the
  named recipients and the two-class retention statement, but the module does **not** yet refuse to
  activate for an employee until a re-delivery acknowledgement dated on/after this feature is recorded.
- **Req 27 — the čl. 47 evidencija as a generated artefact** is still a hand-maintained document
  (`docs/compliance/evidencija-obrade-cl47.md`); it is not driven off the configured purposes, recipients
  and `retention_policies` rows.
- **Req 28 — remote-support masking** of the absence-reason column, with the unmask logged, is not built
  (depends on SW-10).
- **Req 21, print half — CSV ships, the per-employee monthly print sheet does not.** Mirrors the polog
  report's open item.
- **W-1 remains open** (`docs/SW14-VERIFIED-RULES.md` §6): whether ZEOR čl. 51 reaches a preduzetnik at
  all. Until a lawyer answers, no ZEOR figure is rendered — the `legal.rs` guard enforces it.

### SW-14 write-path guards — future day, wrong period, period naming (2026-08-02)

Three defects in the one write path, `commands/worktime.rs::write_entry`, and so in both
`worktime_save_entry` and `worktime_correct_entry` at once. **No schema change — v17 is untouched**, and
`legal.rs` is not in the diff. Baseline before the batch: cargo **556**, bun **355**.

| Defect | What was wrong | Fixed by |
|---|---|---|
| **D1 — §4 req. 6 + req. 17** `[LEGAL]` | `write_entry` parsed `dan` and checked the closed-period freeze, but never compared the day against the `now` already threaded through it: `2028-03-04` recorded as `verzija 1`. Hours nobody has worked yet are a forecast, not the „dnevna evidencija“ ZoR čl. 55 st. 6 asks for — the thing that makes a register look invented, which is the čl. 276 st. 1 tač. 1a exposure the module exists to remove. `close_period` had `guard_period_has_ended` from the start; the write path had no equivalent one day at a time, and the log is append-only, so a mistyped year lands a permanent row that can only be superseded — a correction entry about a day that never happened | `f1cee97` — `guard_day_has_happened` (`commands/worktime.rs:1184`), first check in `write_entry` (`:571`). Decides off the `now` **parameter**, never a wall clock of its own, so the day is a caller's statement and the guard is testable. The day **in progress** stays writable: req. 17 wants a contemporaneous write, so the boundary is the calendar day of `now`, not the day before it. An unreadable `now` refuses with its own message rather than guessing — the row is permanent and would carry that same unreadable stamp as its `created_at`. Sits ahead of the live-row lookup, so a correction aimed at a future day is told why instead of „nema unosa za taj dan“ |
| **D2 — write integrity** | `SaveEntryRequest` carried only `user_id` and `dan`, and the month was derived from that day, so the backend could not see the mismatch the Datum field already refuses: a septembar day typed while avgust is on screen is a valid date and every backend check passed it. `list_month` filters by period, so the operator is told „Dan je evidentiran“ about a row that is not on the screen they are looking at and, in an append-only log, cannot be withdrawn — only superseded from a month they have to know to open. It also slips the closed-period freeze they would have hit: that check runs on the **day's own** month, so a stray day walks into a neighbouring month that may be open when the one on screen is shut | `55a54dc` — `godina`/`mesec` on `SaveEntryRequest` (`:253`), deliberately **not** `#[serde(default)]` (a default would let the caller the guard exists for drop it silently), and `guard_day_is_in_period` (`:1222`) at `:575`. The pair is an assertion *about* the write, never the source of the row's own month, which stays derived from `dan`. `validate_month` runs on the stated period first, so month 13 says so instead of reporting a mismatch against „13/2026“. Frontend: `toRequest` already had the selected period in hand and now sends it |
| **D3 — operator copy** | The three period refusals in the module disagreed with each other and with the screen: the new one named the month, `period_closed_error` and `guard_period_has_ended` numbered it („Period 08/2026“) while the operator had picked that month from a list of names. Neither of the two older messages had any test on its text — only on `code()` | `085e18a`, `3a36d0f` — `MESECI` and `naziv_perioda` (`:1255`, `:1279`); all three call sites read the name and write the Serbian ordinal dot themselves. Both older messages are now pinned, the „još nije završen“ case for **decembar** as well as avgust so the name has to come out of the table by index. Frontend: `MESECI` moves out of `WorkTimeModule.tsx` into `src/lib/period.ts` beside a mirroring `nazivPerioda`, and `WorkTimeModule`, `MyHoursPanel` and `mock-adapter` — which held two hard-coded copies of the `period_closed` message that would have been left numbering the month — all read it from there. Three frontend copies of the month list become one |

**Verification gates (all six green at HEAD, `3a36d0f`; every command exited `0`):**

| Gate | Result |
|---|---|
| `bun run test` | **359 passed** / 0 failed, 23 files (was 355 / 22) |
| `bun run build` | pass — tsc + vite |
| `cargo test -- --test-threads=1` | **559 passed** / 0 failed, 0 ignored (was 556) |
| `cargo clippy --all-targets --all-features --locked -- -D warnings` | clean |
| `cargo fmt --check` | clean |
| `git diff --check` | clean |

New tests: 3 in `commands/worktime.rs` (`a_day_that_has_not_happened_cannot_be_recorded`,
`a_day_outside_the_stated_period_is_refused`, `the_period_is_named_in_serbian`), plus message assertions
added to the two pre-existing period tests, which had asserted only `code()`; 4 frontend (1 in
`WorkTimeModule.test.tsx`, 3 in the new `src/lib/period.test.ts`).

Latest migration: **v17** — unchanged by this batch.

**Residuals this batch leaves:**

- **The month names now exist twice, once per language** — `crate::commands::worktime::MESECI` and
  `src/lib/period.ts`. The backend has to be able to name a period it is refusing a write for without
  being handed the name by the caller it is refusing, and the frontend has to fill a `<select>` without
  asking the backend, so neither copy can be deleted. Both are pinned string by string
  (`the_period_is_named_in_serbian`, `period.test.ts`) and each test spells the twelve names out rather
  than reading the table it is checking — that pair of tests is the whole defence against a silent
  divergence in what the operator is told.
- **`godina`/`mesec` are now required on every write.** Any future caller of `worktime_save_entry` or
  `worktime_correct_entry` must state the period; a payload without them fails deserialization rather
  than defaulting to a period nobody chose. That is the intent, and it is the one breaking change in the
  batch.

---

### SW-14 fix batch — D2, D3, D4 and the prose guards (2026-08-01)

Four defects found by review of the shipped SW-14 work. None of them changed a fine *amount*, which is
why the amount guards could not see any of them: two were wrong prose about what the code does, one was
a wrong statutory citation on a correct figure, and one was a save that succeeded when it should have
been refused.

| Commit | Defect | What shipped |
|---|---|---|
| `b495425` | Three compliance rows credited the code with legs it does not have — the čl. 87 weekly cap, the čl. 57 st. 5 ceiling attributed to `assess_caps` rather than to its caller, and an unqualified „no fine figure outside `legal.rs`“ | The rows were re-stated, and the defect class got a guard: **`src-tauri/src/docs_guard.rs`** (test-only, `#[cfg(test)]` in `lib.rs`) embeds `SERBIAN-LAW-COMPLIANCE.md`, `PROGRESS.md`, both `docs/compliance/` templates **and** the `retention.rs` `napomena` strings, and fails when a line claims more than the code delivers |
| `66636a3` | The čl. 23 notice told the employee the trajno classification could not be reached by a restore or by backup pruning. `assert_never_purge_intact` runs only inside the `reset_trading_data` transaction; `restore_backup` swaps the database file, and no backup-prune path exists in the crate at all | The notice, the register row and the stored `napomena` now claim only what is true — the `pre_restore` safety copy is the protection on that path, `backup_restored` records the never-purge row counts on both sides, and automatic cleanup of old backups „ne postoji“. Two `docs_guard` tests pin both halves |
| `1c33bf1` (D2) | `docs/compliance/evidencija-obrade-cl47.md` — the document SW-14 req. 27 names — printed a „fiksna kazna 100.000 RSD“ (the čl. 95 st. 2 *pravno-lice* tier, double the pilot's real exposure) in a record shown to the Poverenik, headed the obrađivač record „čl. 47 **st. 2**“ (the disapplication for nadležni organi; the obrađivač record is **st. 4**), and invoked only one limb of čl. 47 st. 9 | Preduzetnik tier **fiksna 50.000 (čl. 95 st. 6)**, st. 4 heading, **both** st. 9 limbs (tač. 2 — obrada nije povremena; tač. 3 — posebne vrste podataka). Three new `docs_guard` tests |
| `1216194` (D3, D4) | **D3** — a preraspodela day refused on the čl. 57 st. 5 60 h weekly ceiling appended `capsExceeded`, whose summary states the čl. 53 8 h/12 h caps that čl. 58 makes **inapplicable** to that employee and whose citation is čl. 274 st. 1 tač. 3. Preraspodela is **tač. 4**. Both tačke resolve through the same st. 2 for a preduzetnik, so no wrong figure ever shipped. **D4** — a 13 h preraspodela day saved without the čl. 53 st. 1 ground being asked for | `legal::preraspodela_caps_exceeded` — duty čl. 57 st. 5, preduzetnik čl. 274 st. 1 tač. 4 u vezi sa st. 2 — is the **eighth** notice, enumerated in `all_notices`, in the duplicated inline list in `every_notice_function_is_enumerated_in_the_guard`, and the asserted count is bumped to **8** |

**Verification gates — all six run from the repo root at `1216194`, every command exited `0`:**

| Gate | Result | Exit |
|---|---|---|
| `bun run test` | **359 passed** / 0 failed, 22 files (was 355) | `0` |
| `bun run build` | 2734 modules transformed, built in 3.64s | `0` |
| `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | **558 passed**; 0 failed, 0 ignored, 0 measured, 0 filtered out (was 549) | `0` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings` | clean, no warnings | `0` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | clean, no output | `0` |
| `git diff --check` | clean, no output | `0` |

Net **+9 cargo / +4 bun** over the SW-14 baseline. Latest migration: **v17** (unchanged — this batch adds
no schema).

**Found by the closing audit of this batch, and NOT yet fixed** — both are the same defect class the
batch was created to eliminate, in the one file it did not reach:

- **`docs/compliance/obavestenje-zaposlenima.md:10` still prints the pravno-lice fine band.** The line
  reads *„50.000–2.000.000 RSD za pravno lice … odnosno 20.000–500.000 RSD za preduzetnika“*. SW-14 §1
  row 15 was closed only in its *tačka* half (tač. 20 → tač. 8); the tier half was answered by adding the
  preduzetnik band beside the pravno-lice one rather than by removing it. `evidencija-obrade-cl47.md`
  received the opposite and correct treatment in `1c33bf1` — the preduzetnik figure printed, the
  pravno-lice stav cited without an amount — and `docs_guard.rs` has a test pinning that file
  (`the_cl_47_record_prints_only_the_preduzetnik_fine_tier`) with **no counterpart for the čl. 23
  notice**. This is the only pravno-lice figure left anywhere under `docs/compliance/`.
- **`docs/compliance/obavestenje-zaposlenima.md:84` promises a deletion the code never performs.** The
  Class B retention row tells the employee that drafts and helper time data *„Brišu se pošto je mesec
  zaključen“*. **No purge path exists in the crate**: `retention::draft_purge_eligible`,
  `overtime_log_purge_eligible` and `is_purgeable` are predicates with no production caller — every call
  site is inside `retention.rs`'s own test module. The `WorktimeDraft` `napomena` was written correctly
  (*„Brišu se **tek** pošto je period zatvoren“* — a not-before bound, not a promise), and the notice was
  not brought into line with it.

**Also open, and missing from the list above** (`docs/SW14-VERIFIED-RULES.md` §4):

- **Req 24, privacy-screen half.** The three basis strings (čl. 12 st. 1 tač. 3, tač. 2, čl. 17 st. 2
  tač. 2) are constants that must surface in the app's privacy screen *and* in the čl. 47 generator so
  they cannot drift. Neither exists — a grep for `čl. 12 st. 1` / `17 st. 2` across `src/` and
  `src-tauri/src/` returns no basis string, and there is no privacy screen under `src/app/`. The
  generator half is already tracked as Req 27. The *„build no consent UI“* half of Req 24 **is** honoured.

---

### SW-10 + SW-13 + SW-17 — ZZPL trio (2026-08-02)

The prudential evidencija pristupa and the čl. 46 remote-support nalog (**SW-10**), the three-class
employee lifecycle with a time-driven purge (**SW-13**), the internal breach record with the
Pravilnik 40/2019 obrazac (**SW-17**), and the generated čl. 47 evidencija radnji obrade that SW-17
depends on, shipped as a ten-task TDD plan (`docs/superpowers/plans/2026-08-01-zzpl-trio.md`, design at
`docs/superpowers/specs/2026-08-01-zzpl-trio-design.md`) against the verified rule set in
`docs/REMAINING-SW-VERIFIED-RULES.md` §4. Baseline at plan time was `1216194` — cargo **558**,
bun **359**, migration **v17**.

**The headline is a reclassification, not a feature.** SW-10 was on the roadmap as a čl. 50 duty and is
not one: čl. 48 and čl. 51 confine their logging duties to a *nadležni organ … u posebne svrhe*, and
čl. 50 appears nowhere in čl. 95, so **no prekršaj is prescribed for not having this log at all**. It
ships as a prudential/evidential control on čl. 5 st. 2 and čl. 41 st. 1, with čl. 48 st. 2 borrowed as
the design template. Nothing in the app, the register or the čl. 23 notice tells the operator that the
law requires it — and nothing says *"no consequence"* either, because the Poverenik's opomena and
obavezujući nalog are live. The one **legal** duty in SW-10 is čl. 46, and it is the nalog, not the log.

| Task | Shipped | Commits |
|---|---|---|
| 1 — schema | **Migration v18**, five stores: `support_sessions` (the čl. 46 nalog); `audit_events` — closed `CHECK` on the čl. 48 st. 1 verbs, actor as an **FK to `users`** rather than a name column, closed enums for `reason_code` / `recipient` / `object_type`, `prev_hash` + `hash` NOT NULL, **no `updated_at`**, and a `BEFORE UPDATE` trigger; `personnel_records` — the 25 ZEOR čl. 5 tačke, `REFERENCES users(id)` **without `ON DELETE CASCADE`** (req. 24) plus a `BEFORE DELETE` trigger; `data_breaches` — `saznanje_at` NOT NULL with an immutability trigger, `occurred_at` / `discovered_at` separate and nullable; `processing_activities` — the čl. 47 st. 1 items with a retention column per category (t. 6) | `372ab53`, `0550654` |
| 2 — `audit.rs` | SHA-256 `chain_hash` / `verify_chain_anchored`: tail truncation is reported rather than silently accepted, and the chain still verifies across a purge through a stored anchor. `reject_forbidden_content` is the **write-boundary** filter req. 4 asks for — the object id must match a whitelisted opaque-id grammar, so a JMBG, a card PAN, an address, a phone, a free-text note or a **search string** cannot enter the table even from a caller that means well. Pure: no clock, no connection, no state | `76fc5af`, `2a75be0` |
| 3 — support nalog | `grant_access` (admin-gated, explicit obim and expiry, hard-bounded at **24 h**), `request_access` (gated by the **nalog** and not by a role — čl. 46 makes the nalog the condition, and the obrađivač has no account on this till), `end_session` (`ended_at` if the session was entered, `revoked_at` if it never was), `active_session` | `38e6cba`, `d2bcefc` |
| 4 — write path + izvod | `record_audit`, the single door every feature writes through; `search`, admin-only — **reading the log is deliberately not logged**, the export is; `export_csv`, the izvod modelled on **čl. 48 st. 4** and handed over on the **čl. 49** osnov, rendering offline from the till with the period, the filter, the row count, the chain verdict and both hashes per row so the recipient can recompute the chain. Req. 7 leaves the module with three verbs and no fourth: nothing edits or removes a logged row | `a6a436a`, `ed7878b` |
| 5 — three-class split | `commands/personnel.rs`: class A in `personnel_records`, class B as the surrogate `users.id` already on the ledger (ZoRač čl. 8 st. 4), class C as the credential hashes and `audit_events`. `purge_expired_classes` is **time-driven** (launch + a 6-hourly timer in `lib.rs`, never a command an operator presses) and takes a `PurgeableClass` with exactly two variants, **neither of which is class A** — reaching the ZEOR register is a type error, not a review catch — with the whole sweep inside one transaction fenced by `assert_never_purge_intact`. Credentials go **at deactivation** (req. 21), the access log on its 2-year class, and the purge line records *which class* was discarded, never a discarded value | `f1776c0`, `8abb945` |
| 6 — breach record | `commands/breaches.rs`: every povreda is logged first and notifiability is a **derived flag on a row that always persists** (req. 43); beyond the three st. 6 elements the row carries the immutable `saznanje_at`, the separate occurred/discovered instants, the risk outcome, the notify decision **with reasoning**, the Poverenik-notified stamp, a st. 2 delay reason that becomes **mandatory once 72 h have elapsed**, and the čl. 53 block. `legal.rs::breach_notification_missing` is the **ninth** notice — in `all_notices`, in the duplicated inline list, with the asserted count bumped to **9** | `bdd4f37`, `421ed9c` |
| 7 — obrazac | `breaches_export_obrazac` renders the **Pravilnik 40/2019** obrazac in its five prescribed sections, down to the mesto/datum + Ime i prezime + Potpis block and the „Prilog:“ slot. Print/scan only — **no submission API exists** — and the countdown is the flat **72 h of Pravilnik čl. 3**. The export is a disclosure and is logged as one, so a rendered obrazac never outlives its čl. 48 st. 2 line | `497c95c`, `fb3b4a7` |
| 8 — čl. 47 register | `cl47.rs` generates the evidencija radnji obrade at every launch and on demand (`cl47_generate` / `cl47_export`). Every rok is read out of `retention_policies` **at generation time**, so the register cannot print a period the till does not apply; `trajno` is printed for this register alone (čl. 47 st. 7) and for neither log, which is a test rather than a comment | `31fa18a`, `db814ef` |
| 9 — Privatnost | `src/app/privacy/` — four panels (Daljinska podrška, Evidencija pristupa, Povrede podataka, Radnje obrade), **admin-only in the nav and admin-gated behind every command**, with Daljinska podrška first so the module's one legal duty is its first surface. The čl. 50 copy reads „nije propisan prekršaj“ and names the opomena and the obavezujući nalog that *are* consequences | `a9ff1f7`, `546987f` |
| 10 — docs + gates | This section; register rows 8, 9, 10, 11 and 12 re-stated; SW-10, SW-13 and SW-17 flipped to shipped with SW-10 kept labelled **prudential** and SW-17 re-worded per req. 50 (*provides the record required by* čl. 52 st. 6–7 — never *satisfies* the article); and the čl. 23 notice's new evidencija-pristupa section (req. 9). The review of this task caught the notice's class-B retention row promising a deletion nothing performs; the row now states the not-before bound its stored `napomena` states and says that automatic deletion of the class „ne postoji“, pinned by two new `docs_guard` tests | this commit |

**Verification gates — all six run from the repo root, every command exited `0`:**

| Gate | Result | Exit |
|---|---|---|
| `bun run test` | **400 passed** / 0 failed, 27 files (was 359 / 22) | `0` |
| `bun run build` | tsc + vite, 2740 modules transformed | `0` |
| `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | **661 passed**; 0 failed, 0 ignored, 0 measured, 0 filtered out (was 558) | `0` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings` | clean, no warnings | `0` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | clean, no output | `0` |
| `git diff --check` | clean, no output | `0` |

Net **+103 cargo / +41 bun** over the plan baseline. Latest migration: **v18**.

New Rust tests: 22 in `audit.rs`, 29 in `commands/audit.rs`, 13 in `commands/personnel.rs`, 14 in
`commands/breaches.rs`, 13 in `cl47.rs`, 2 in `docs_guard.rs`, plus additions in `db/migrations.rs`,
`retention.rs` and `legal.rs`; 37 frontend tests across the five `src/app/privacy/*.test.tsx` files.

**Deliberately not built** (`docs/REMAINING-SW-VERIFIED-RULES.md` §5 items 1–6, 14, 15, 22, 23, 24, 30)
— read these as decisions, not as gaps: no claim anywhere that ZZPL requires an audit log, and no
*„nema posledice“* either; no per-cashier aggregates or productivity view on top of the log (a new
purpose under čl. 5 st. 1 t. 2, and it walks into the SW-14 §6 W-4 *sistem za praćenje rada* gate); no
owner-editable or owner-deletable log; no `trajno` on either log — čl. 47 st. 7 governs the register
alone; no „Obriši zaposlenog“ affordance and no cascade at the personnel record; no employee name or
JMBG on a transaction row; no risk gate in front of the breach record; no invented breach form and **no
Poverenik submission API**; no fine printed for a documentation-only breach-log failure (unresolved
under ZoP čl. 3, §6 R-2); and no headcount gate or plan-tier upsell on any of the three.

**Still open after this batch:**

- **Req. 47 — the processor→controller breach leg is not built.** If Actaer is ever an obrađivač, the
  vendor→merchant notification artefact with its own timestamp — feeding the merchant's `saznanje` so the
  72 h clock is anchored to evidence rather than memory — is a separate path. The record can hold both
  instants today; nothing produces the vendor-side one. §6 R-3 is the question behind it.
- **Req. 49, redaction half — expiry deletes nothing yet for the breach log.** The requirement is to
  **redact or pseudonymise** the personal-data-bearing fields on expiry rather than drop the row, keeping
  the čl. 52 st. 7 skeleton. No breach-log retention class and no redaction path exists; the row is kept.
- **SW-14 req. 28 — remote-support masking** of the absence-reason column, with the unmask logged, is
  still not built. SW-10 was its stated dependency and has now landed, so nothing blocks it.
- **One of the two SW-14 defects found by the previous batch's closing audit is still open.**
  `docs/compliance/obavestenje-zaposlenima.md` still prints the pravno-lice fine band in its header;
  `the_cl_47_record_prints_only_the_preduzetnik_fine_tier` guards that shape for the sibling document and
  has no counterpart for the čl. 23 notice. **The other is fixed.** The class-B retention row no longer
  promises a deletion no code performs: it now states the not-before bound the stored `napomena` already
  stated (*„Brišu se **tek** pošto je mesec zaključen i klasifikacija izvedena“*) and says on the same
  line that automatic deletion of that class *„ne postoji“*, date-stamped. `draft_purge_eligible` and
  `overtime_log_purge_eligible` remain gates with no production caller, and SW-13's sweep still does not
  touch the class — the row now says exactly that. Two `docs_guard` tests pin both halves:
  `no_retention_row_promises_a_purge_no_job_performs` clears a deletion promise in the notice's
  retention table only against an **exhaustive** match on `commands::personnel::PurgeableClass`, so a new
  promise costs a new variant and a new variant costs a real sweep; and
  `the_class_b_retention_row_says_no_automatic_purge_exists_for_it` keeps a bound with no sweep behind it
  from being softened into what reads like a schedule.
- **Backup encryption is opt-in.** `backup_crypto.rs` encrypts only once an admin sets a passphrase, so
  the čl. 50 st. 2 tač. 1 measure is available rather than in force. Register row 8 now says so; SW-2
  remains the action.
- **§6 R-9 stays open** — which article of the ZZPL nadzor chapter carries the opomena and the nalog.
  The copy is written so that only the verified half (*no prekršaj*) is asserted.

---

### SW-10 req. 6 + SW-13 req. 22 — the rok is now a setting (2026-08-02)

The closing audit of the batch above found the third document in a row promising behaviour the code
lacked, and the čl. 23 notice was again the carrier. **Four strings** said the retention period was
adjustable and moved only forward: two rows of `docs/compliance/obavestenje-zaposlenima.md`, the
`napomena` stored beside `RecordClass::AccessLog`, and the `rok_osnov` the čl. 47 register prints for
the Poverenik. `retention::extend_retain_until` had the arithmetic and the upward-only rule, but every
call site was inside a `#[cfg(test)]` module: no command, no `invoke_handler` entry, no screen.

Req. 6 asks for *configurable retention with a documented default* and req. 22 for three things —
*ship a default, expose the setting, record the chosen value in the čl. 47 register*. **The setting was
built rather than the strings re-stated**, so all four now say what the program does.

| Shipped | Where |
|---|---|
| `AdjustableClass` — the classes a registered command can move, exhaustively. Four variants, none of them `trajno`: the ZEOR čl. 5 evidencija, the frozen monthly classification and the čl. 47 register have no variant, so reaching them is a compile error rather than a review catch — the same shape `personnel::PurgeableClass` uses for the purge | `commands/retention.rs` |
| `retention_list_policies` / `retention_extend_policy` — admin-gated **inside the domain function**, not in the wrapper. The list carries every class, `trajno` ones included, because *„rok se ne podešava“* is the answer to a question the operator would otherwise ask by trying. There is **no shortening verb and no clearing verb**: an earlier date is refused, never clamped, and the refusal is prose the operator can act on | `commands/retention.rs`, `lib.rs` |
| The čl. 48 line. Moving a rok decides how long the evidencija pristupa itself survives, so it is a `menjanje` on `retention_policy` carrying the row id and nothing else (req. 4), in the same transaction as the write | `commands/retention.rs` |
| Req. 22's third limb. The command regenerates the čl. 47 register in the same call, so the chosen value reaches st. 1 t. 6 immediately rather than at the next launch | `commands/retention.rs` → `cl47::generate` |
| „Rokovi čuvanja“ in Podešavanja — the rok in force per class, its `napomena`, and a date field only where a command can actually move it. It sits under Podešavanja and not under Privatnost on purpose: the evidencije are records the rukovalac keeps and nothing may switch off, while ZZPL čl. 5 st. 1 tač. 5 leaves the *period* to the shop | `src/app/settings/RetentionPanel.tsx` |
| Two `docs_guard` tests. `no_stored_retention_note_claims_a_period_no_command_can_move` is exact — the class each string belongs to is known — and covers both the stored `napomena` and the register's `rok_osnov`; `no_notice_row_claims_an_adjustable_period_for_a_class_no_command_can_move` reads the čl. 23 notice by subject against an **exhaustive** match on `AdjustableClass`, so a class added without a command fails the guard | `docs_guard.rs` |

**What the setting does and does not do, stated once.** `retain_until` is the earliest day on which a
class may be discarded, not a day on which anything is discarded — the sweep is gated by it, so pushing
it forward keeps every row of that class until at least that day. The screen and the register both say
it in those words (*„ništa se ne briše pre …“*), and nothing anywhere offers to shorten it.

**Verification gates — all six run from the repo root, every command exited `0`:**

| Gate | Result | Exit |
|---|---|---|
| `bun run test` | **407 passed** / 0 failed, 28 files (was 400 / 27) | `0` |
| `bun run build` | tsc + vite | `0` |
| `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | **670 passed**; 0 failed, 0 ignored (was 661) | `0` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings` | clean, no warnings | `0` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | clean, no output | `0` |
| `git diff --check` | clean, no output | `0` |

Net **+9 cargo / +7 bun**. Latest migration: **v18** — this batch adds none.

---

### ZZPL fix batch — final gate run and closing audit (2026-08-02)

`4fbe147` (D2–D4) landed without a gate table, so the counts recorded immediately above are two cargo
tests behind the tree. This section carries the true figures and the closing audit of the whole ZZPL
effort — SW-10, SW-13, SW-17 and the generated čl. 47 evidencija radnji obrade that SW-17 depends on.

**Verification gates — all six run from the repo root, every command exited `0`:**

| Gate | Result | Exit |
|---|---|---|
| `bun run test` | **407 passed** / 0 failed, 28 files | `0` |
| `bun run build` | tsc + vite, dist written; only the pre-existing chunk-size advisory | `0` |
| `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | **672 passed**; 0 failed, 0 ignored, 0 measured, 0 filtered out (was 670 — `4fbe147` added two) | `0` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings` | clean, no warnings | `0` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | clean, no output | `0` |
| `git diff --check` | clean, no output | `0` |

Against the ZZPL-trio plan baseline (`1216194` — cargo **558**, bun **359**, migration **v17**) the whole
effort is net **+114 cargo / +48 bun**. Latest migration: **v18**, and the fix batch adds none.

**What the fix batch closed.** Four defects, each one a document asserting behaviour the code did not
have, and in each case a guard that could not see it because nothing read that line.

| # | Closed | Commit |
|---|---|---|
| D1 | Req. 6 and req. 22 — the rok čuvanja is a **setting**, not a promise. `AdjustableClass` + `retention_list_policies` / `retention_extend_policy` + „Rokovi čuvanja“ in Podešavanja, with the čl. 47 register regenerated in the same call so req. 22's third limb lands immediately | `0ca0f90` |
| D2 | The register re-introduced the wrong offence tačka for `legal.rs::preraspodela_caps_exceeded`. čl. 57 and čl. 60 sit in **čl. 274 st. 1 tač. 4**; tač. 3 is the čl. 53 offence. The preduzetnik amount is identical either way, so no figure guard could ever have caught it | `4fbe147` |
| D3 | 26 Serbian quotations in the register opened with „ and closed with an ASCII mark. All closed, and `no_compliance_prose_closes_a_serbian_quotation_with_an_ascii_quote` now reads every document this crate embeds plus the stored `napomena` strings | `4fbe147` |
| D4 | The register asserted SW-14's raw punch events are purged on period close. `draft_purge_eligible` is a **gate, not a sweep**, with no production caller; the row now says so, and `no_retention_row_promises_a_purge_no_job_performs` reads the register as well as the čl. 23 notice | `4fbe147` |
| — | Earlier in the same effort: the čl. 23 notice's class-B retention row, which promised a deletion nothing performs | `0604420` |

**Closing audit — findings, honestly stated.**

1. **`legal.rs` guard integrity: intact.** Nine `pub fn … -> LegalNotice` functions exist and no
   `LegalNotice` is constructed anywhere else in the crate. All nine appear in `all_notices`, all nine
   appear in the duplicated hand-written list inside
   `every_notice_function_is_enumerated_in_the_guard`, and the asserted count is **9**. The forbidden-
   substring, UNSET and ZEOR guards therefore reach every notice.
2. **One operator- or employee-facing document still prints a pravno-lice fine band.**
   `docs/compliance/obavestenje-zaposlenima.md` header. It is mitigated — the preduzetnik tier is printed
   beside it with the correct stav, and the sentence says the amount depends on the employer's legal form
   — but it remains the only such document with no guard, while its sibling
   `evidencija-obrade-cl47.md` is pinned by `the_cl_47_record_prints_only_the_preduzetnik_fine_tier`.
   Already disclosed above; restated here because the closing audit confirmed it is the **only** one.
3. **The generated čl. 47 register does not over-claim.** `cl47::mere_zastite` reads the backup settings
   and states in both directions whether encryption and automatic copies are actually in force;
   `cl47::rok_cuvanja` reads `retention_policies` at generation time, so the register cannot print a
   period the till does not apply; the two `prenos` prompts deliberately refuse a flat *„ne“* the program
   cannot verify. `evidencija_povreda` and `tehnicka_podrska` both say plainly that no rok is configured
   and that nothing is discarded until one is.

**Still open — by requirement number in `docs/REMAINING-SW-VERIFIED-RULES.md` §4.**

Previously disclosed, unchanged:

- **Req. 47 (SW-17)** — the obrađivač→rukovalac breach leg is not built. The row can hold both instants;
  nothing produces the vendor-side one. §6 R-3 is the question behind it.
- **Req. 49 (SW-17)** — neither half is built: there is no breach-log retention class in
  `RecordClass` and no redaction path, so the row is simply kept. The heading above called this „the
  redaction half“; both halves are open.
- **SW-14 req. 28** — remote-support masking of the absence-reason column. Its stated dependency
  (SW-10) has landed, so nothing blocks it.
- **SW-2** — backup encryption is opt-in, so the čl. 50 st. 2 tač. 1 measure is available rather than in
  force. Register row 8 and `cl47::mere_zastite` both say so.
- **§6 R-9** — which article of the ZZPL nadzor chapter carries the opomena and the nalog. Only the
  verified half (*no prekršaj is prescribed for čl. 50*) is asserted anywhere.

Found by this closing audit and **not previously disclosed**:

- **Req. 26 (SW-13), ledger half.** Per-category retention floors are enforced in code for the seven
  `RecordClass` variants, and personnel correctly has no configurable period at all — but **no variant
  covers the ledger categories**, so the ZoRač čl. 28 floors and the st. 5 / st. 9 clock split the
  requirement calls out are not enforced by the shared table. Register row 21 discloses the same gap as
  „a general upward-only `retain_until` engine over trading data“; PROGRESS.md did not. SW-3 is the
  action.
- **Req. 48 (SW-17), encryption-at-rest limb.** Admin-only RBAC ✓, own čl. 47 entry ✓, exclusion from
  routine exports and support bundles ✓ — but the database is **plaintext on disk**: `rusqlite` is built
  with `bundled` / `backup` / `functions` and no `sqlcipher` feature. The requirement asks for encryption
  at rest for this store specifically; that limb is unbuilt, and it is a different question from SW-2,
  which concerns the backup file.
- **Req. 1 (SW-10), middle row of the three-row penalty cell.** The čl. 42 row lives in register row 12
  and the *čl. 50 → no prekršaj prescribed* row in register row 8, but the **čl. 46 → čl. 95 st. 1
  t. 23** row exists only in code doc-comments (`commands/audit.rs`, `db/migrations.rs`) and never in the
  compliance matrix itself.
- **Req. 5 (SW-10), fourth limb.** The čl. 23 notice adopts all four purposes of čl. 48 st. 3 verbatim;
  the čl. 47 register's `evidencija_pristupa` svrha carries three of them. čl. 47 st. 1 t. 2 is what the
  Poverenik reads, so the two documents should not differ on the stated purpose.
- **Two compliance templates sit outside `docs_guard::prose_sources`.**
  `docs/compliance/runbook-povreda-podataka.md` and `docs/compliance/ugovor-o-obradi-nacrt.md` are not
  embedded, and both still describe SW-10 and SW-17 as future work („budući ekran“, „budući modul …
  do njegovog uvođenja obrađivač vodi ručnu evidenciju sesija“) although both shipped on 02.08.2026.
  The runbook additionally cites **čl. 53 st. 1** for the controller→Poverenik 72 h clock; the clock is
  **čl. 52 st. 1**, which is what `commands/breaches.rs` and register row 10 both cite, and čl. 53 is the
  duty toward the affected individuals.
- **`evidencija-obrade-cl47.md` A.7 lists kriptozaštita rezervnih kopija as an applied čl. 50 measure**
  without the opt-in caveat. The generated register states the same measure conditionally, in both
  directions; the hand-kept narrative version does not, and it is the copy handed over on request.
- **The čl. 23 notice §4 asserts flatly that there is no cross-border transfer**, where the generated
  register deliberately refuses that flat assertion because the program cannot check it and §6.5 / R-5 is
  unresolved. The notice carries a bracketed reconciliation note, but the assertion is made first.
- **`cl47::Template::mere` is the one behaviour-claim column of the generated register that no guard
  reads.** `retention_prose()` exposes `kljuc`, the retention class and `rok_osnov`; the per-radnja
  measures string is a claim about what the program does and reaches the Poverenik unguarded.

Nothing above regressed a gate: all six are green at 407 bun / 672 cargo, and migration **v18** is
unchanged.

---

### SW-12 — Mašinski čitljiv cenovnik (2026-08-02)

The published cenovnik, the archive of every version of it, the till guard that keeps the shop to the
prices it published, and the folder the file lands in, shipped as a nine-task TDD plan
(`docs/superpowers/plans/2026-08-01-sw12-cenovnik.md`, design at
`docs/superpowers/specs/2026-08-01-sw12-cenovnik-design.md`) against the verified rule set in
`docs/REMAINING-SW-VERIFIED-RULES.md` §2b, §3 V2 and §4 reqs. 10–18. Baseline at plan time was
`0920ac6` — cargo **672**, bun **407**, migration **v18**.

**What is deliberately not in it, stated first because the register would otherwise read as a closed
item.** ZZP čl. 6 st. 2 requires publication *„na svojoj internet stranici“*. This cycle builds
everything up to and including writing the file into a folder the shop nominates — it does **not** put
that folder on the internet, because a hosted endpoint means Actaer running a public service carrying
its customers' prices, with its own uptime, cost and liability, and čl. 6 st. 4 binds the shop to
whatever that service serves. That is a founder decision and it is now **F-13** in the register's §4.
Čl. 6 **st. 6**'s account on the Nacionalni portal otvorenih podataka is not built either, and could
not be: st. 6 keys the format to a standard *„propisan podzakonskim aktom“*, and that bylaw does not
exist (§6 items 4 and 12).

**And what the product must never say.** Whether a trader **without** a website must create one is
**unresolved** — st. 2's possessive presupposes a site, no ZZP provision obliges any trader to have
one, and extending a prekršaj to an unwritten duty runs into lex certa (ZoP čl. 3). So no screen and no
document says the pilot is in breach today; the panel's strongest sentence is *„nije podešeno mesto
objave“*. Nor does anything date the exposure to 1 May 2026: čl. 6 is in the čl. 220 carve-out, **čl. 210
is not**.

| Task | Shipped | Commits |
|---|---|---|
| 1 — schema | **Migration v19**: `cenovnik_snapshots` — immutable, **no `updated_at`**, a `BEFORE UPDATE` trigger covering every identity and content column (**`id` included**, because it is both the current-row tie-break and the handle a divergence record names) and a second trigger letting `published_at` / `published_target` be written exactly once — and `products.jedinicna_cena_jedinica` / `jedinicna_cena_sadrzaj_milli` on the schema-wide milli scale (a 0,75 l bottle is `750`). An outlet's **current** cenovnik is derived, `ORDER BY generated_at DESC, id DESC LIMIT 1`, so there is no pointer two writers could desynchronise. `DELETE` is deliberately left open so the čl. 213 purge in Task 4 can reach an expired row | `3e4da52`, `1b13f76` |
| 2 — the render | `cenovnik.rs`, pure: no database, no clock, no filesystem. `render_csv` emits the de facto data.gov.rs shape — **UTF-8 with BOM, `;` separator, DD-MM-YYYY, two decimals from integer para at the boundary, 13-digit barcode quoted as text** — in `sifra` order, so the same catalog always renders the same bytes and therefore the same `content_hash` (SHA-256). `published_prices` is the exact inverse and finds its columns **by name out of the file's own header**. Eight columns, not six: `jedinicna_cena` and `jedinica_za_jedinicnu_cenu` are req. 10, because čl. 6 st. 2's second sentence pulls st. 1 into the published file | `f2592fb`, `87da10f` |
| 3 — republish on write | Čl. 6 st. 3 says *„ažurira u realnom vremenu“*, so publication rides the write that moved a price — a nightly batch is a defect against st. 3, not a simplification. **Four paths**: `commands::catalog`, `campaigns` (activation, markdown step, ending), `importer` (**once per batch**, since the file is the whole catalog) and `kep_storno::post_nivelacija` — whose returned verdict `commands::kep` republishes on, after its own commit — each threading `record_offered_price_change`'s own answer instead of re-deciding what a price move is; the catalog path adds the jedinična cena, the one published price that log does not watch. It runs **after `tx.commit()`** — a publish failure is logged and the price stands. Plain `INSERT` only, **never `INSERT OR REPLACE`**, with its own assertion: a REPLACE is a DELETE plus an INSERT no trigger sees. The archive key is minted once and frozen in `settings.cenovnik_prodajno_mesto`, derived from settings and never taken from the frontend | `5f11623`, `47b0148` |
| 4 — archive + retention | Čl. 6 st. 5 asks the trader to enable a comparison of *„prethodno objavljenih cena“* with the realtime ones, so a publication is never overwritten: `list_snapshots` reads an outlet's lineage newest-first and `read_snapshot` returns the body byte for byte. `RecordClass::CenovnikArchive` joins the shared retention table with the **two-year floor of the čl. 213 zastarelost**, upward-only — and it is the one class in that table holding **no personal data**, which is what keeps it out of the čl. 47 register. `purge_expired_snapshots` runs at launch and on the 6-hourly timer in `lib.rs`, never touches an outlet's current file however old it is, and a test scans the crate to prove it is the **only** code that deletes from the table — no trigger can tell a purge from a cover-up, so the constraint is a property of the code | `58df8fa`, `376cf0f` |
| 5 — till guard | `sales_assess_price_integrity` runs against the draft cart, so the operator learns before the money moves. It **warns and never blocks** — „Završi prodaju“ stays enabled — fires only in the above-published direction (a discount is not a breach, which is also what keeps it to one comparison), and names the snapshot it was measured against, because čl. 6 st. 4 binds the shop to the file in force and not to the catalog that file was rendered from. `sales_complete` writes a `cenovnik_price_divergence` row into the never-deleted `compliance_log`. **No file a target accepted means no guard and no block** — the comparison reads the newest snapshot whose `published_at` is set, so an archive of files that went nowhere (the `NotConfigured` default) arms nothing, and čl. 6 st. 4 is never cited against a trader who has published nothing; a check that could not run says so rather than reading as an all-clear | `62504b8` |
| 6 — čl. 210 notice | `legal.rs::cenovnik_not_published` — the **tenth** notice, in `all_notices`, in the duplicated inline list, asserted count bumped to **10**. Preduzetnik: **fixed 100.000 (čl. 210 st. 3)**, halved to 50.000 on payment within eight days of a prekršajni nalog (ZoP čl. 173 st. 1). The citation carries the escalation ladder that actually matters — čl. 207 t. 1 → čl. 206 st. 1 zapisnik → st. 4 rešenje → st. 5 privremena zabrana prometa → čl. 209 st. 1 t. 40 — and the čl. 213 limitation. Req. 18's guard makes the pravno-lice **200.000 unreachable under the preduzetnik profile**, and `51e923d` widened it from this one notice to the blanket check | `2edf93e`, `51e923d` |
| 7 — publish targets | `PublishTarget` with `LocalFolderTarget` and a `NotConfigured` default that is a **state, not an error**: the snapshot is still rendered and archived, and a price save never fails because the hosting question is open. Req. 13's seven fetchability rules — no login, no CAPTCHA or bot-fight rule, no JS-rendering requirement, no `robots.txt` disallow, no aggressive rate limit, a stable URL per prodajni objekat, `text/csv; charset=utf-8` — are written into the trait's doc comment as a **contract on any future implementation**, because čl. 6 st. 5 is a duty to *enable* and bot protection would manufacture a breach for our own customer. The local write stages through a per-publish temporary so a reader never sees a half-written cenovnik | `e1e4f99`, `c6baf79` |
| 8 — frontend | Podešavanja → **Cenovnik** (self-loading, the RetentionPanel pattern): the duty, the state of the publishing target, the outlet's archive newest-first, any prior file openable byte for byte. `cenovnik_get_notice` resolves čl. 210 against the **stored** legal form and an unset form answers `penalty: null` rather than letting a screen pick a tier. The till warning. And the product sheet's unit-price pair, sent on **every** save because `SaveProductRequest` replaces the whole row. **No „objavi sada“ button**: publication rides the price write, and a second answer to *„when did this shop last publish“* is the one thing an operator could believe wrongly. The review caught two live defects — the package content collapsing by a thousand through `toLocaleString("sr-RS")`'s dot separator, and the panel promising a file a fresh install never makes because `publish_current` archives nothing until the prodajni objekat is identified | `47de099`, `0ce4756` |
| 9 — docs + gates | This section; register row 25 and the SW-12 row in §3 re-stated; **F-13** opened for the hosting decision; §6 items 4 and 12 carry the čl. 6 st. 7 bylaw's verified absence, the **quarterly Sl. glasnik RS monitoring trigger for *cenovnik* / *zaštita potrošača* through the čl. 216 deadline of 01.05.2027**, and the *practice, not law* label on the data.gov.rs shape (req. 17) | this commit |

**Verification gates — all six run from the repo root, every command exited `0`:**

| Gate | Result | Exit |
|---|---|---|
| `bun run test` | **447 passed** / 0 failed, 30 files (was 407 / 28) | `0` |
| `bun run build` | tsc + vite, dist written; only the pre-existing chunk-size advisory | `0` |
| `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | **781 passed**; 0 failed, 0 ignored, 0 measured, 0 filtered out (was 672) | `0` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings` | clean, no warnings | `0` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | clean, no output | `0` |
| `git diff --check` | clean, no output | `0` |

Net **+109 cargo / +40 bun** over the plan baseline. Latest migration: **v19**.

**Whose tests those are, honestly split.** SW-12 itself contributes **+105 cargo** — 37 in `cenovnik.rs`,
43 in `commands/cenovnik.rs`, 10 in `commands/sales.rs`, 5 in `legal.rs`, 4 in `db/migrations.rs`, 3
in `retention.rs`, 2 in `importer.rs`, 1 in `campaigns.rs` — and **+35 bun**: 17 in
`src/app/settings/CenovnikPanel.test.tsx`, 6 in `CatalogModule.test.tsx`, 6 in
`src/services/local-adapter.test.ts`, 5 in `RegisterScreen.test.tsx`, 1 in `SettingsScreen.test.tsx`.
The remaining **+4 cargo / +5 bun** are two SW-14 fix commits that landed in the same window
(`3a36d0f`, `a18d9fe`) and are not SW-12's.

**Deliberately not built** — read these as decisions, not as gaps. No hosted endpoint and no public URL
(**F-13**). No čl. 6 st. 6 open-data-portal upload while the standard it is keyed to does not exist. No
„objavi sada“ button, and no nightly regeneration job — both would be second answers to *when did this
shop last publish*. No hard block at the till: čl. 6 st. 4 is a duty to adhere to published prices, not
a reason a sale cannot complete, and a POS that refuses to sell is a POS the shop stops using. No
guessed jedinična cena — an article whose measure and package content the shop has not entered
publishes an **empty** cell, because a missing cell is a visible gap and an inferred one is a false
statement the shop answers for under čl. 6 st. 4. No claim of breach anywhere, and no exposure dated to
1 May 2026.

**Still open after this batch.**

- **Req. 15 — the hosted endpoint.** The trait, its seven-rule contract and the archive are ready for
  it; the decision is F-13. Until it lands, a shop with a website complies by pointing the local folder
  at what that site serves, and a shop without one is in the unresolved §2b position.
- **Čl. 6 st. 6 — the Nacionalni portal otvorenih podataka account**, blocked on the missing čl. 6 st. 7
  bylaw rather than on engineering. The monitoring trigger is §6 item 12.
- **Req. 16's *„column mapping in configuration“* is only half true.** `cenovnik::COLUMNS` is a
  constant, not a setting — a schema swap is a code change and a release, not something the shop can do.
  That is the right trade while no format is prescribed, but the requirement's wording is stronger than
  the code, and this is the disclosure rather than a claim that it was met.
- **A publish failure is visible only in the log.** `republish_after_price_move` logs and returns; the
  Cenovnik panel shows the archive and the configured target, so an operator can infer staleness from a
  missing recent snapshot, but nothing raises the failure at the moment it happens.
- **`docs_guard::no_retention_row_promises_a_purge_no_job_performs` was cleared against the wrong list
  for the register**, and this task fixed it. `commands::personnel::PurgeableClass` is exhaustive for
  the čl. 23 notice, which is only ever about an employee's data; it stopped being the whole answer for
  the register when Task 4 shipped a second sweep, `commands::cenovnik::purge_expired_snapshots`, for a
  class that holds no personal data and could never have a `PurgeableClass` variant. The register arm
  now matches exhaustively on `retention::RecordClass`, each variant naming either the sweep that
  discards it or the reason there is none.
- **The till guard armed off files nobody had published**, and the Task 9 review caught it in the one
  place it matters — the row above said *„no published snapshot means no guard“* while
  `current_published_cenovnik` selected the outlet's newest snapshot with no filter on `published_at`.
  Since `PublishTargetSettings::NotConfigured` is the `#[default]` and `publish_current` archives with
  `published_at = NULL`, that is not a corner case but the **pilot's own expected state while F-13 is
  open**: an identified outlet, no mesto objave, a snapshot per price write and not one of them
  fetchable. The consequence was operator-facing and permanent — a destructive
  „Cena je iznad objavljenog cenovnika“ at the till and a never-deleted `compliance_log` row citing
  čl. 6 st. 4 against a trader who had published nothing, on a provision that reaches only
  *„Trgovac koji objavi cenovnik iz stava 2“*. **The code was changed, not the sentence**: the lookup
  now takes the newest snapshot whose `published_at` is set. So „current“ for the guard is deliberately
  *not* „current“ for the archive list and the purge — those answer *which file did this outlet render
  last*, this answers *which file can the public still fetch*, and a newer body no target accepted has
  displaced nothing anywhere a consumer can look. **One consequence, disclosed rather than fixed:**
  the čl. 213 purge fences an outlet's *newest* snapshot, not its newest *published* one, so a shop
  whose last publication is over two years old and which has archived something since loses the guard's
  exhibit and the guard falls silent. That errs toward saying nothing rather than toward accusing, which
  is the safe direction, and it is the only direction this trade is allowed to fail in.

---

### SW-12 fix batch — the guard's published-file filter and the half-configured jedinična cena (2026-08-02)

Two review passes over the surface SW-12 had just shipped. **In both, the code was changed and not the
sentence that described it** — which is the direction this project has got wrong five times, every one
of them in something a shop owner actually reads.

| # | Defect | Fix |
|---|---|---|
| Task 9 review | `current_published_cenovnik` took the outlet's newest snapshot with **no filter on `published_at`**, while the register and this section both said *„no published snapshot means no guard and no block“*. Since `PublishTargetSettings::NotConfigured` is the `#[default]` and `publish_current` archives with `published_at = NULL`, that is not a corner case but **the pilot's own expected state while F-13 is open**. The consequence was operator-facing and permanent: a destructive „Cena je iznad objavljenog cenovnika“ at the till and a never-deleted `compliance_log` row citing čl. 6 st. 4 against a trader who had published nothing — on a provision reaching only *„Trgovac koji objavi cenovnik iz stava 2“* | `7577d80` — the lookup now takes the newest snapshot whose `published_at` is set, so „current“ for the guard is deliberately **not** „current“ for the archive list and the čl. 213 purge. Three tests, each failing without the filter; `seed_published_till` now publishes through an accepting target so the existing till tests still exercise the fired path for the right reason |
| **D2 — the mirror half-state** `[LEGAL]` | `normalize_product_request` rejected only a *sadržaj with no jedinica*. The mirror — **a jedinica with no sadržaj** — passed, and `CenovnikRow::jedinicna_cena_minor` then published the sale price **as** the jedinična cena. A 0,75 l bottle at 279,00 whose operator typed „l“ and left the package content blank published `279.00;l` where the figure is `372.00` — a wrong **published** jedinična cena the shop answers for under ZZP čl. 6 st. 1 and st. 4, with no warning at any layer | `7399559` — both layers now refuse that pair when the measure differs from the jedinica mere (case-insensitively), which is the only combination where the v19 NULL-sadržaj convention is *provably* wrong; a shop that really sells one litre per piece still says so with a sadržaj of 1. The convention itself now appears in the field help, so a deliberate blank reads as deliberate. +3 cargo, +3 bun |
| **D1 — doc overstated the code** `[ACCURACY]` | The SW-12 clause in `docs/SERBIAN-LAW-COMPLIANCE.md` said the column mapping *„is configuration and a schema swap is a routine release“*. It is the constant `cenovnik::COLUMNS`: swapping it is a **code change and a release we ship**, not something the shop can do | `7399559` |
| **D3 — comment overstated the schema** `[ACCURACY]` | The comment above the pair's validation claimed the v19 CHECK *„refuses both half-states“*. It refuses one — `sadrzaj_milli IS NULL OR (>0 AND jedinica IS NOT NULL)` — and the permitted one is exactly what made D2 possible | `7399559` |

**Verification gates — all six run from the repo root, every command exited `0`:**

| Gate | Result | Exit |
|---|---|---|
| `bun run test` | **450 passed** / 0 failed, 30 files (was 447) | `0` |
| `bun run build` | tsc + vite, dist written; only the pre-existing chunk-size advisory | `0` |
| `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | **784 passed**; 0 failed, 0 ignored, 0 measured, 0 filtered out (was 781) | `0` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings` | clean, no warnings | `0` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | clean, no output | `0` |
| `git diff --check` | clean, no output | `0` |

Net **+3 cargo / +3 bun** over the Task 9 baseline. Latest migration: **v19**, unchanged and frozen.

**Where reqs. 10–18 stand after this batch, stated as status and not as achievement.**

| Req. | State |
|---|---|
| **10** — jedinična cena + jedinica mere in the file | **Built, with one hole found during this gate run and NOT fixed — see below.** The eight columns ship, the unconfigured article publishes an empty cell rather than a guessed one, and D2 closed the catalog-form route to a wrong figure |
| **11** — publish rides the write | **Built.** Four paths, each threading `record_offered_price_change`'s own answer; the catalog path adds the jedinična cena, the one published price that log does not watch |
| **12** — till guard | **Built.** Warns and never blocks, above-published direction only, names its exhibit, logs a `cenovnik_price_divergence` row; arms only off a file a target accepted |
| **13** — anonymously fetchable endpoint | **NOT built and disclosed.** There is no endpoint. The seven fetchability rules ship as a contract in `PublishTarget`'s doc comment with a test that keeps them there; the acceptance criteria are F-13 (b) |
| **14** — published-price archive | **Built.** Immutable dated snapshots, current derived not stored, čl. 213 two-year floor, purge that never reaches an outlet's current file |
| **15** — hosted cenovnik endpoint | **NOT built and disclosed.** Founder decision **F-13**. A shop with a website discharges čl. 6 st. 2 today by pointing the local folder at what that site serves |
| **16** — CSV default, mapping in configuration | **Half met and disclosed.** The shape ships; `cenovnik::COLUMNS` is a constant, so a swap is a release. D1 removed the doc sentence that said otherwise |
| **17** — document that the čl. 6 st. 6 standard does not exist | **Done.** §6 items 4 and 12 carry the verified absence, the quarterly Sl. glasnik RS trigger through 01.05.2027, and the *practice, not law* label |
| **18** — 100.000 preduzetnik, `200.000` unreachable | **Done.** `legal.rs::cenovnik_not_published`, and the blanket guard on the fixed-sum phrasing *„200.000 dinara“* — the bare literal cannot be the needle, because ZoR čl. 274 st. 2 legitimately fines a preduzetnik *„od 200.000 do 400.000“* elsewhere in the product |

**Newly identified and still open — the import path can publish a wrong jedinična cena, and clobbers the
jedinica mere on the way.** `[LEGAL]` D2 closed the catalog form. It did **not** close `importer.rs`,
which writes `products` with its own `UPDATE`/`INSERT` and never calls `normalize_product_request`.
Two things follow, and the second is the one that reaches the published file:

- `unit_of_measure` is `required: false` in the mapping step (`ImportWizard.tsx:109`), and when it is
  unmapped `optional_value` returns `""`, which `importer.rs:744` substitutes with **`"kom"`**. A
  routine price-update import therefore **silently rewrites the jedinica mere of every article it
  matches**, whatever the shop had configured.
- The pair `jedinicna_cena_jedinica` / `jedinicna_cena_sadrzaj_milli` is left untouched by that UPDATE,
  and the v19 CHECK constrains only the pair against itself, never against `unit_of_measure`. So an
  article legitimately configured under the NULL-sadržaj convention — `unit_of_measure = 'l'`,
  jedinica `'l'`, sadržaj blank, which is exactly what D2's new gate *accepts* — comes out of an import
  as `unit_of_measure = 'kom'` with jedinica still `'l'`, and the batch's own republish
  (`importer.rs:450`) then publishes the D2 figure verbatim. Reproduced against the shipped tree: the
  archived body reads `SKU-1;;Hleb;kom;372.00;372.00;l;02-08-2026` — *372,00 RSD po litru* for goods the
  same row says are sold **po komadu**. Same wrong published number as D2, same čl. 6 st. 1 / st. 4
  exposure, and **no warning at any layer**, because the operator never typed the unit at all.

**Also open, and smaller.** Nothing previews the computed jedinična cena at the point of entry: the
product sheet takes the measure and the package content and shows no derived figure, so a sadržaj typed
as `0,075` instead of `0,75` publishes 3.720,00 per litre and the only place that figure is legible is
the archived file in Podešavanja → Cenovnik, after publication. And `unit_of_measure` is a **published
column** (`jedinica_mere`) that no path republishes on: `republish_cenovnik` fires on
`offer_changed || unit_price_changed`, so a catalog edit that changes only the measure leaves the
published file carrying the old one until the next price move. That is the deliberate čl. 6 st. 3
reading — *„trenutnim cenama“* — and it is recorded here as a consequence of it, not as a defect.

Everything listed under *Still open after this batch* in the section above stands unchanged: the hosted
endpoint (F-13), the čl. 6 st. 6 portal account, req. 16's constant, a publish failure that is visible
only in the log, and the purge fence that can leave the guard silent rather than wrong.

---

## Executive Summary

VantumPOS is a Tauri + React + SQLite POS built strictly local-first (no fiscalization, no Medusa, no cloud). The shared foundation is essentially complete and is the strongest module; auth/shifts, catalog, register/sales, and inventory are all real and working end-to-end; receipts/returns, reports, import, and settings/backup are functionally implemented but carry the bulk of the remaining gaps. Two systemic issues recur across the application: (1) several frontend screens hard-code `userId: 1` for the operator instead of threading the real session user, weakening audit trails; and (2) frontend test breadth lags backend test breadth, with two modules (06, 08) missing spec-required UI tests entirely. The single largest audit-vs-assessment disagreement is module 08 (Settings/Backup), revised down 4 points because the VAT screen is create-only and admin role-gating is absent at every layer.

| Module | Completion % | Status | Top gap |
|---|---|---|---|
| 00 Shared Foundation | 94% | Complete | Shell header hard-codes "VantumPOS" instead of reading `settings_get_company` |
| 01 Auth & Shifts | 91% | Mostly done | Spec-required "deactivated user cannot log in" backend test absent; Close Shift omits opened-at/cashier fields |
| 02 Catalog & Products | 92% | Complete | "Lager" row action navigates generically instead of deep-linking the product ledger; orphaned dead-code screen remains |
| 03 Inventory | 86% | Mostly done | Frontend hard-codes `userId:1`; ledger omits user/reference columns; sales duplicates inventory write logic |
| 04 Register & Sales | 90% | Mostly done | Preview/validation errors are silently swallowed (error state defined but never rendered) |
| 05 Receipts & Returns | 86% | Mostly done | No loading/empty states + unhandled rejection on search/detail failure; void/return hard-code `userId:1` |
| 06 Reports | 80% | Mostly done | shift/cashier filters unsupported in backend; screen not admin-gated; no error/empty frontend tests |
| 07 Import | 90% | Mostly done | No job-detail drill-down UI; missing unknown-VAT/required-mapping tests; XLSX absent (optional) |
| 08 Settings & Backup | 80% | Mostly done | Admin role-gating for receipt sequence missing everywhere; no Settings component tests; VAT screen is create-only |

---

## Build & Test Health

| Command | Result | Summary |
|---|---|---|
| `bun run test` | Pass | vitest: 6 test files, **57 tests passed**, 0 failed (4.88s) |
| `bun run build` | Pass | tsc typecheck + vite build succeeded; 2709 modules transformed (1.35s); only a non-fatal "chunk > 500 kB" warning |
| `cd src-tauri && cargo test -- --test-threads=1` | Pass | **67 passed**, 0 failed (lib tests; main.rs 0, doc-tests 0); ~14s |
| `cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings` | Pass | Compiled clean, zero warnings under `-D warnings` |
| `cd src-tauri && cargo fmt --check` | Pass | No formatting diffs; exit 0 |

**Backend tests:** 67 passed / 0 failed. **Frontend tests:** 57 passed / 0 failed. The toolchain is healthy and CI-green.

**What blocks the Definition of Done despite green CI:** the failures are not in *what runs* but in *what is not yet tested or wired*. DoD point 6 (happy + failure tests) is the systemic weak spot — module 08 ships **no** `SettingsScreen.test.tsx` (all 5 spec frontend tests absent) and module 06 has no error/empty/loading frontend tests. DoD point 5 (frontend states) is only partial in modules 04 (preview errors swallowed), 05 (no loading/empty + unhandled rejections), and 06 (product/category tables lack empty state). One business-rule gap is an outright miss: module 08's "Receipt sequence updates require admin role" is unenforced at every layer. These are the items standing between "mostly done" and a fully signed-off MVP.

---

## Module Details

### 00 — Shared Foundation — 94% — Complete

Audited 94% (unchanged from assessment). The contract every other module builds on is genuinely implemented, not scaffolded.

**What's done**
- SQLite wrapper enabling `foreign_keys=ON` + `busy_timeout=5000` (`db/mod.rs:33-42`).
- Transactional, append-only migration runner: 5 versioned migrations inside one `conn.transaction()`, recorded in `_migrations`; v1 creates all 14 required tables with money/quantity CHECK constraints (`db/migrations.rs:16-166`, `256-287`).
- Bootstrap admin seeded once with an argon2-hashed PIN only when `users` is empty (`db/mod.rs:49-76`).
- Typed `AppError → CommandError {code, message, details}` with stable codes and Serbian operator messages (`app_error.rs:81-127`).
- RFC3339 UTC clock (`clock.rs:6`); argon2 hashing (`security.rs:6-23`); `AppState` holding Db + in-memory session.
- All 10 canonical commands plus `get_app_health` registered in `lib.rs:23-79`.
- Strict service-port layering: only `local-adapter.ts` imports `@tauri-apps/api` (grep-verified); full TS ports + DTOs + local and mock adapters; RSD integer-money helpers using BigInt (`lib/money.ts`).
- Real shadcn sidebar shell (`AppShell.tsx:198-305`) with login, session loading/error states, service-backed footer, and status badges.

**Partial**
- Shared shell status completeness: user/role/shift/DB/backup are all shown; only the shop-name element is absent (`AppShell.tsx:244-288`).
- Login `foundationCards` lean slightly toward the "marketing hero" pattern the spec discourages, though guarded by a no-internal-terms test (`navigation.ts:65-81`, `App.test.tsx:218-232`).

**Missing**
- (Low) Shop name not shown in shell header — `AppShell.tsx:208` hard-codes "VantumPOS"/"Lokalna kasa" and never calls `settings_get_company`, which exists.
- (Low) No dedicated unit tests for `clock.rs` / `security.rs` (only indirect coverage).
- (Low) No end-to-end v1→v5 forward-migration regression on a pre-existing old DB (only idempotency + added-column tests).

**Backend commands (present / registered):** `get_app_health`, `auth_login`, `shift_open`, `catalog_search_products`, `inventory_receive`, `sales_complete`, `receipts_search`, `reports_daily_turnover`, `import_validate`, `settings_update_company`, `backup_create` — all present and registered (`lib.rs:23-79`).

**Frontend screen:** Real. The foundation "screen" is the AppShell itself (shadcn sidebar + login). UI states present: loading (BackendStatus, session spinner), ready/error/migration-not-ready, validation (login/open-shift). Only deviation: hard-coded shop name.

**Business rules**

| Rule | Status |
|---|---|
| Money never floating point; integer minor units end to end | Met |
| Quantities stored as integer milli-units | Met |
| Append-only migrations; initial migration immutable | Met |
| Transaction boundary in Rust, one per use case | Met |
| Stable error codes + Serbian operator messages | Met |
| One open shift per user enforced at DB level | Met |
| Bootstrap admin seeded once with hashed credential | Met |

**Acceptance criteria**

| Criterion | Met |
|---|---|
| Service-port layering; no direct Tauri invoke in screens | Yes |
| All canonical snake_case commands present & registered | Yes |
| Money as integer minor units; backend integer math | Yes |
| Quantities milli-units; ISO timestamps backend-created | Yes |
| All 14 tables via initial migration | Yes |
| Schema changes append-only; v1 unedited | Yes |
| State-changing transaction boundary in Rust | Yes |
| Error contract `{code,message,details}` with stable codes | Yes |
| Serbian-Latin operator messages; technical cause hidden | Yes |
| Each nav item renders a distinct real screen | Yes |
| Shell footer backed by auth state (not hard-coded Admin) | Yes |
| Shell shows shop name, user, role, shift, DB, backup | Partial (shop name hard-coded) |
| Frontend loading/empty/validation/error states | Yes |
| Tests cover happy + failure paths | Yes |
| App runs local-first (no fiscal/Medusa/cloud) | Yes |

**Definition-of-Done checklist**
1. Distinct useful screen — Met
2. UI backed by service interfaces — Met
3. Rust owns state-changing validation/transactions — Met
4. Structured codes + Serbian messages — Met
5. Frontend loading/empty/validation/error — Met
6. Tests happy + failure — Met
7. Local-first, no fiscal/Medusa/cloud — Met

**Test coverage & gaps:** Backend — schema/index/FK/pragma tests, cascade delete, negative-money/zero-qty CHECK rejections, migration idempotency (count==5), admin seeding, health response. Frontend — money valid/invalid, adapter command-name mapping, AppShell login/session/backend states, operator-copy guard. Gaps: no direct `clock.rs`/`security.rs` unit tests; no pre-existing-old-DB forward-migration regression; no automated assertion that the shell surfaces the company shop name.

**Auditor notes:** Confidence high; the audit independently verified the assessment and found it honest and not over-stated, agreeing at 94%. No genuine placeholders/stubs found (all "placeholder" hits are HTML input attributes). Two *under-claims*: the foundation actually ships ~8000 lines of real downstream command code exercising the contracts, and one-open-shift-per-user is defended at two layers (partial UNIQUE index + pre-insert guard), stronger than the summary implied.

**Key risks:** Shell shop-name remains unwired despite `settings_get_company` existing; `clock.rs`/`security.rs` regressions would only surface via higher-level tests; no explicit old-DB forward-migration regression means a future destructive ALTER could go uncaught.

**Evidence:** `lib.rs:23-79`; `db/migrations.rs:11-254`, `256-287`; `app_error.rs:81-127`; `db/mod.rs:33-42`, `49-76`; `src/services/local-adapter.ts:1`; `AppShell.tsx:198-305`; `BackendStatus.tsx:20-82`; `lib/money.ts`; `App.test.tsx`, `money.test.ts`, `local-adapter.test.ts`.

---

### 01 — Auth & Shifts — 91% — Mostly done

Audited **91%** vs assessed 93% — a **material disagreement** (−2). The audit trimmed it because the spec names two concrete deliverables that are absent (see Auditor notes).

**What's done**
- All 10 backend commands present and registered (`lib.rs:25-34`): `auth_get_session/login/logout`, `users_list/create/update/deactivate`, `shift_get_current/open/close`.
- Argon2 PIN/password hashing in Rust (`security.rs:6-23`); bootstrap admin PIN 1234 (`db/mod.rs:49-72`).
- Login validates trimmed input, rejects deactivated users and bad credentials, updates `last_login_at`, sets process-local session (`auth.rs:81-119`).
- Shift open rejects negative cash and second open shift; DB unique partial index `idx_shifts_one_open_per_user` (`migrations.rs:181-184`). Shift close requires non-optional counted cash, derives expected cash from completed cash payments only, `WHERE status='open'` guard (`shifts.rs:132-189`).
- Users CRUD with admin-only guard, unique username, role validation, create wrapped in a transaction (`users.rs:54-302`).
- Real screens: Login, Open Shift, Close Shift (AlertDialog confirmation), Users (Table + Dialog) — all service-backed (`AppShell.tsx:447-1178`). Shell header/footer data-driven from session.

**Partial**
- Close Shift screen completeness: functional, but omits the spec-listed "opened at" and cashier-name fields (`AppShell.tsx:757-801`).
- Frontend empty states: loading/validation/error solid; explicit empty state is the weak spot (users table renders blank, `AppShell.tsx:908-993`).
- Expected-cash vs partial returns: a partial item return flips the whole original sale to `refunded` (`receipts.rs:593`), over-excluding that sale's cash from a shift's expected cash.

**Missing**
- (Medium) Backend test "deactivated user cannot log in" — spec explicitly requires it; behavior exists (`auth.rs:93`) but is untested.
- (Low) Close Shift "opened at" + cashier fields not displayed.
- (Low) No explicit empty state for the Users list.

**Backend commands (present / registered):** `auth_get_session` (lib.rs:26), `auth_login` (27), `auth_logout` (28), `users_list` (29), `users_create` (30), `users_update` (31), `users_deactivate` (32), `shift_get_current` (33), `shift_open` (34), `shift_close` (34) — all present and registered.

**Frontend screen:** Real. Login, Open Shift, Close Shift (AlertDialog), Users (Table + Dialog). States present: loading (session spinner, users loading badge), validation (opening cash, user form), error (Alerts). Weakness: no Empty component for an empty users list.

**Business rules**

| Rule | Status |
|---|---|
| Deactivated users cannot log in | Met (untested) |
| Only one open shift per user | Met (double-guarded) |
| Cashier cannot complete sale without open shift | Met |
| Closing shift requires counted cash | Met |
| Expected cash from completed cash payments minus refunds/voids | Met (partial-return edge nuance) |
| Closing a shift does not delete/mutate sales | Met |

**Acceptance criteria**

| Criterion | Met |
|---|---|
| No hard-coded Admin / "Smena nije otvorena" fixed text | Yes |
| Sign in → open shift → close → shell status updates | Yes |
| Kasa sale can depend on `shiftService.getCurrentShift` | Yes |

**Definition-of-Done checklist**
1. Distinct useful screen — Met
2. Service-backed — Met
3. Rust validation/transactions — Met
4. Structured Serbian errors — Met
5. Frontend states — Partial (no explicit empty state)
6. Tests happy + failure — Met
7. Local-first — Met

**Test coverage & gaps:** Backend — login success/fail, second-shift-fails, close-stores-counted-cash, admin seeding (`commands/mod.rs:24-142`). Frontend — login routing, Serbian login error, invalid opening cash, real user+shift in shell, adapter mapping. Gaps: no deactivated-login test; no close-shift happy-path/difference test; no Users CRUD UI test; no expected-cash-derivation test; no admin-guard test.

**Auditor notes:** Confidence high; substantially agrees, every claim source-checked. **Over-claim corrected:** TS contracts match method *signatures* but type *names* differ from the spec (impl `AuthSession`/`UserAccount` vs spec `AppSession`/`UserSummary`), so "verbatim" overstates it. Also flagged: the Users "Deaktiviraj" action fires immediately with no confirmation dialog (`AppShell.tsx:957-986`). **Under-claims:** DB CHECK constraints give a third money-validation layer; login self-heals against mid-login deactivation. Dead code (not a stub): `AppShell.tsx:189` has two identical branches.

**Key risks:** Process-local session (re-login on restart, no multi-window sharing); shift-open does its duplicate check + INSERT without an explicit transaction (the unique index is the real race guard, surfacing as `database_error` rather than the friendly validation message); partial returns over-exclude cash; the missing deactivated-login test means a regression letting inactive users in would pass CI.

**Evidence:** `lib.rs:23-79`; `auth.rs:45-198`; `shifts.rs:43-291`; `users.rs:23-309`; `security.rs:1-23`; `app_error.rs:81-127`; `migrations.rs:181-228`; `db/mod.rs:49-72`; `commands/mod.rs:14-142`; `ports.ts:81-98`; `local-adapter.ts:71-87`; `AppShell.tsx:119-1178`; `App.test.tsx:86-203`.

---

### 02 — Catalog & Products — 92% — Complete

Audited **92%** vs assessed 93% (−1, minor). The audit found the assessment honest and slightly conservative.

**What's done**
- All 8 spec commands in `catalog.rs` registered in `lib.rs:63-72`, plus a bonus `catalog_lookup_product_by_barcode` (Open Food Facts enrichment, `lib.rs:69`).
- Real dense screen `CatalogModule.tsx` wired at `AppShell.tsx:346` for `activeId "products"`: Artikli/Kategorije tabs, InputGroup search, all five filters (category/active/VAT + low-stock + missing-barcode), Table with every spec column, Sheet form with every spec field (plus quick + bulk entry), category management.
- All writes in Rust transactions: `create_product` (271), `update_product` (353), `set_product_active` (435), `save_category` (467).
- Validation in Rust: required name/sku/unit, non-negative prices, category exists, active tax-rate ref, unique SKU/barcode with structured Serbian errors `duplicate_sku`/`duplicate_barcode` (827, 848).
- Product list LEFT JOINs `inventory_balances` for live stock (630). Migration v5 adds provenance columns (`migrations.rs:243-253`).

**Partial**
- Product updates not changing historical `sale_items` snapshots — structurally satisfied (`update_product` touches only `products`) but no catalog-level immutability test.
- "Existing tests and build pass" — comprehensive tests exist; here confirmed green by the global Build & Test Health run (57 FE / 67 BE pass).

**Missing**
- (Low) Deep-link "view ledger" from a product row to that product's Lager ledger — the row action navigates to inventory generally (`CatalogModule.tsx:983`).
- (Low) No frontend test for the category create/edit Sheet happy path.
- (Low) No backend tests for the `low_stock`/`missing_barcode` SQL filter branches.
- (Low) Orphaned legacy `src/app/ProductCatalogScreen.tsx` (281 lines) is dead code, never imported — should be deleted.

**Backend commands (present / registered):** `catalog_list_products` (63), `catalog_search_products` (64), `catalog_get_product` (65), `catalog_create_product` (66), `catalog_update_product` (67), `catalog_set_product_active` (68), `catalog_list_categories` (71), `catalog_save_category` (72) — all present/registered; bonus `catalog_lookup_product_by_barcode` (69).

**Frontend screen:** Real and dense. States present: loading (Spinner "Ucitavanje artikala"), empty (Empty component for products + categories), validation (`validateProductForm` + FieldError), error (loadError + `commandFieldErrors` placing duplicate errors on sku/barcode). Caveat: orphaned `ProductCatalogScreen.tsx` is dead code, not the live screen.

**Business rules**

| Rule | Status |
|---|---|
| Product name required | Met |
| SKU/code required and unique | Met |
| Barcode optional but unique when present | Met |
| Sale/purchase price cannot be negative | Met |
| VAT must reference an active tax rate | Met |
| Deactivation hides from Kasa search, keeps history | Met |
| Updates do not change historical sale_items snapshots | Met (cross-module, untested at catalog level) |

**Acceptance criteria**

| Criterion | Met |
|---|---|
| Artikli shows a real catalog screen, not a placeholder | Yes |
| User can create and edit a product | Yes |
| Kasa can reuse `searchProducts` (hides inactive) | Yes |
| Existing tests and build pass | Yes (confirmed by global CI run) |

**Definition-of-Done checklist**
1. Distinct useful screen — Met
2. Service-backed — Met
3. Rust validation/transactions — Met
4. Structured Serbian errors — Met
5. Frontend states — Met
6. Tests happy + failure — Met
7. Local-first — Met

**Test coverage & gaps:** Backend — create persists, duplicate_sku, duplicate_barcode, inactive hidden from search, list includes stock, update, set-active, save-category, provenance, OFF mapping, migration columns. Frontend — module opens with filters/rows, required-field validation, duplicate error placement, deactivate from row, barcode lookup, bulk entry, adapter mapping. Gaps: no `low_stock`/`missing_barcode` backend filter tests; no category UI happy-path test; no catalog-level snapshot-immutability test.

**Auditor notes:** Confidence high; agrees. **Over-claim corrected:** migration v5's test asserts provenance columns exist on a *fresh* DB (all 5 migrations applied) — it is not a true v4-seeded forward-migration regression, so calling it that is slightly generous. **Under-claims:** the barcode-lookup and bulk-entry flows are substantial, well-tested bonus features; `saveCategory` mapping is exercised in adapter tests. Only "placeholder" hit is a legitimate HTML input attribute. Net −1 for the minor polish gaps and orphaned dead code.

**Key risks:** `catalog_lookup_product_by_barcode` makes a live HTTP call to Open Food Facts (`catalog.rs:507-516`) — the one network dependency, but optional and degrades gracefully; orphaned `ProductCatalogScreen.tsx` could mislead maintenance.

**Evidence:** `catalog.rs:156-235`, `271/353/435/467`, `789-855`, `1116-1430`; `lib.rs:63-72`; `CatalogModule.tsx:158-779`; `AppShell.tsx:346-353`; `ports.ts:100-110`; `local-adapter.ts:88-107`; `mock-adapter.ts:430-517`; `migrations.rs:64-79`, `243-253`; `App.test.tsx:313-535`; `src/app/ProductCatalogScreen.tsx` (orphaned).

---

### 03 — Inventory — 86% — Mostly done

Audited **86%** vs assessed 88% — a **material disagreement** (−2), mainly for the `userId:1` placeholder and two missing spec ledger columns.

**What's done**
- All 5 commands present and registered (`inventory.rs:103-167`; `lib.rs:43-47`).
- Single transactional helper `apply_inventory_adjustment` validates input, enforces negative-stock rules, writes exactly one `inventory_movements` row + one `inventory_balances` upsert atomically (`inventory.rs:306-404`).
- Negative-stock guard → `insufficient_stock`/"Nema dovoljno zaliha." when `allow_negative_stock=0` (339-344). Validation: qty≠0, receive/write-off positive, correction/write-off require reason, purchase price ≥ 0 (475-516).
- Stock list with search/category/stock_state filters and SQL-computed low_stock (169-236); ledger with running balance, newest-first (238-304).
- Real `InventoryScreen` wired at `AppShell.tsx:368-369`: filterable table, receive/correction/write-off dialogs, ledger Sheet. States: Skeleton loading, Empty, Alert error + toast, FieldError validation.
- FK preserves ledger: `inventory_movements.product_id REFERENCES products(id)` with default RESTRICT (no cascade); `PRAGMA foreign_keys=ON` verified (`db/mod.rs:37`, test 504-511).

**Partial**
- Acceptance "Kasa can reuse stock helpers": helpers are `pub` and transactional, but `sales.rs:263-287` reimplements movement/balance logic inline and the movement enum lacks sale/return/void — so the capability exists in principle but is not shared.
- Ledger columns: implemented date/type/delta/balance/reason; spec's reference type/id and user are not rendered.
- Stock list "unit": folded into the quantity display ("5 kom") rather than a discrete column.

**Missing**
- (Medium) Ledger "user" field absent from backend DTO and UI (`inventory.rs:82-93`, `244-258`).
- (Medium) Frontend hard-codes `userId: 1` (`InventoryScreen.tsx:468`) instead of the session user.
- (Low) No dedicated backend tests for write-off success or correction; none for zero-qty/positive validation.
- (Low) True mid-transaction rollback not exercised (insufficient_stock returns before any INSERT).
- (Low) Receive purchase-price snapshot not surfaced in UI (optional per spec).

**Backend commands (present / registered):** `inventory_list_stock` (43), `inventory_get_product_ledger` (44), `inventory_receive` (45), `inventory_correct` (46), `inventory_write_off` (47) — all present/registered.

**Frontend screen:** Real operational screen. States: loading (Skeleton), empty (list + ledger), error (destructive Alert + toast), validation (client `parseQuantityInput` + FieldError + backend errors). No placeholder text (test asserts "Radni modul" absent). Gaps: ledger omits user/reference columns; `userId` hard-coded.

**Business rules**

| Rule | Status |
|---|---|
| Quantity cannot be zero | Met |
| Receive quantity must be positive | Met |
| Correction can be positive or negative | Met |
| Write-off stored as negative movement | Met |
| Reject resulting negative balance for non-negative products | Met |
| All movements must include user id when available | Partial (frontend hard-codes userId:1) |
| Product deletion does not erase ledger if movements exist | Met (RESTRICT FK; no explicit test) |

**Acceptance criteria**

| Criterion | Met |
|---|---|
| Clicking Lager shows stock and movement UI | Yes |
| Inventory actions persist to SQLite, visible after refresh | Yes |
| Kasa sale completion can reuse stock helpers | Partial |

**Definition-of-Done checklist**
1. Distinct useful screen — Met
2. Service-backed — Met
3. Rust validation/transactions — Met
4. Structured Serbian errors — Met
5. Frontend states — Met
6. Tests happy + failure — Met
7. Local-first — Met

**Test coverage & gaps:** Backend — receive increases balance + writes movement; write-off rejects negative + rolls back; ledger newest-first with running balance; low-stock filter (`inventory.rs:667-811`). Frontend — Lager stock list + receive + ledger; Serbian errors; adapter mapping/determinism. Gaps: no write-off-success or standalone correction test; no zero-qty/positive-validation test; no true post-write rollback test; no dedicated `InventoryScreen.test.tsx`; no product-deletion-blocked test.

**Auditor notes:** Confidence high; agrees, no over-claims. Stress-tested the FK claim and confirmed RESTRICT behavior is real. **Stub-like finding:** `userId:1` is the only stub-like element — `session.user` is available in AppShell but not threaded through. **Divergence risk:** `sales.rs:263-289` re-implements inventory writes with a *different* balance strategy (incremental add vs absolute set) and **no negative-stock guard** on the sale path. **Under-claims:** the ledger UI already labels sale/return/void movement types (forward-compatible), and the mock adapter faithfully replicates the negative-stock guard. Net −2 for user-attribution + two missing spec ledger columns; "mostly done" is the right bucket.

**Key risks:** Movements record `userId=1`, so the audit trail does not reflect the real operator; sales duplicating inventory logic risks divergence in negative-stock handling and balance math; adding ledger user attribution later requires a backend change; several failure paths are untested.

**Evidence:** `inventory.rs:103-167`, `306-404`, `475-552`, `667-811`; `lib.rs:43-47`; `migrations.rs:64-97`; `InventoryScreen.tsx:98-716`; `AppShell.tsx:368-369`; `ports.ts:117-123`; `local-adapter.ts:114-123`; `mock-adapter.ts:164-203`; `App.test.tsx:537-590`; `sales.rs:263-287` (divergent inline logic).

---

### 04 — Register & Sales — 90% — Mostly done

Audited **90%** (unchanged). The audit found the assessment well-calibrated and notably honest about its own gaps.

**What's done**
- Both spec commands present, registered (`lib.rs:72-73`), wired in local adapter.
- `complete_sale_transaction` runs the full flow in ONE transaction (`connection.transaction()` at `sales.rs:185`, `tx.commit()` at 331): load open shift; `compute_sale` recalculates line gross/discount/included-VAT/totals from DB prices; validate stock honoring `allow_negative_stock`; validate payments; assign local receipt number; hard-set `not_fiscalized`; insert sale/items(with snapshots)/payments/movements + balance upsert; bump `shifts.expected_cash_minor`.
- Real dense cashier `RegisterScreen.tsx`: search/scan InputGroup, cart table with qty steppers + item/receipt discounts, totals/payment/change panel, AlertDialog clear-cart, completed-sale "Lokalni racun" Dialog (no fiscalization claims), behind open-shift gating (`AppShell.tsx:330-344`).
- Backed only by `SalesService`/`CatalogService` (no direct invoke).

**Partial**
- Transaction Flow step 1 "validate authenticated/current user": cashier identity is inferred from the most recent open shift, not a passed session (`sales.rs:466-486`). Acceptable for single-register but weaker than the literal spec step.
- Frontend validation/preview error states: `PreviewState` defines an `error` variant and the catch sets it (`RegisterScreen.tsx:124-131`), but the JSX renders only the loading branch (428) — a failing `sales_preview` silently zeros totals and disables Complete with no message.
- Backend "ignores tampered totals" test: covered by design — `CompleteSaleRequest` carries no client totals, so tampering is structurally impossible.

**Missing**
- (Low) Optional helper `sales_get_next_receipt_number_preview` not implemented.
- (Low) Cash-change metadata computed/returned but not persisted on the sale.
- (Low) Sale-level note not implemented.
- (Low) Percentage discount input absent in the UI (backend supports Percent; screen only sends amount).

**Backend commands (present / registered):** `sales_preview` (lib.rs:72), `sales_complete` (lib.rs:73) — both present/registered.

**Frontend screen:** Real cashier screen. States: loading (Spinner during preview), empty (Empty cart), completion error (Alert preserving cart). Gap: preview-time validation/error variant is defined but never rendered.

**Business rules**

| Rule | Status |
|---|---|
| Sale cannot complete without open shift | Met |
| Empty cart cannot complete | Met |
| Quantity cannot be zero | Met |
| Discount cannot make line/sale total negative | Met |
| Cash overpayment allowed for change | Met |
| Stored payment amount not inflated beyond total | Met |
| Card overpayment not allowed | Met |
| Mixed payment must exactly cover total (except cash change) | Met |
| Receipt item snapshots unchanged if product later changes | Met |

**Acceptance criteria**

| Criterion | Met |
|---|---|
| Kasa is a real cashier screen | Yes |
| Local sale completes end-to-end | Yes |
| Stock decreases | Yes |
| Receipt appears in Receipts module data later | Partial (cross-module path exists, no test) |
| No fiscalization claims in UI | Yes |

**Definition-of-Done checklist**
1. Distinct useful screen — Met
2. Service-backed — Met
3. Rust validation/transactions — Met
4. Structured Serbian errors — Met
5. Frontend states — Partial (preview error never rendered)
6. Tests happy + failure — Met
7. Local-first — Met

**Test coverage & gaps:** Backend (6) — preview recalculation, cash-sale insert + stock decrement, shift-required, insufficient-stock rollback, mixed payment, payment mismatch. Frontend (5) — scan adds item, qty+discount updates preview, cash change, complete shows receipt (asserts no `/fiskal/i`), backend error keeps cart. Gaps: no explicit tampered-totals test (mitigated by design); happy-path test doesn't directly assert a `sale_items` row; no percent-discount path test; no cross-module Receipts surfacing test.

**Auditor notes:** Confidence high; agrees, keeps 90%, no over-claims, no placeholders/TODOs. The one substantive DoD deviation (swallowed preview errors) is correctly flagged. **Under-claims:** the cross-module Receipts read path exists in real code (`receipts.rs:300` queries `FROM sales`), so "Receipt appears later" is structurally stronger than "partial" implies; the recompute path is both tested and tamper-proof by design.

**Key risks:** Preview/validation errors silently swallowed (cashier sees zeroed totals + disabled Complete with no explanation); cashier identity trusts the latest open shift; cash change not persisted (historical receipts can't show tendered/change); UI emits only fixed-amount discounts, so the backend percent + tax-allocation path is exercised only by tests/mock.

**Evidence:** `sales.rs:157-171`, `180-333`, `335-423`, `488-557`, `870-1033`; `lib.rs:72-73`; `local-adapter.ts:108-113`; `mock-adapter.ts:518-543`; `ports.ts:112-115`; `RegisterScreen.tsx` (error variant 72-77 defined, only loading rendered 428); `AppShell.tsx:330-344`; `RegisterScreen.test.tsx`; `migrations.rs:99-136`.

---

### 05 — Receipts & Returns — 86% — Mostly done

Audited **86%** vs assessed 88% — a **material disagreement** (−2), for the audit-attribution gap, a partially-satisfied rollback test, and the missing forward-migration regression.

**What's done**
- All 4 commands present, registered (`lib.rs:48-51`).
- Full void: linked "void" sales row + negative `sale_items` + `inventory_movements` (type "void") + balance restore + original status→"voided", one transaction (`receipts.rs:322-436`).
- Partial return: validates remaining = original − already_returned, creates linked "return" doc with proportional totals + negative items + inventory restore, one transaction (438-603).
- Search with all 6 filters (215-315); detail with items/payments/totals, linked docs, can_void/can_return flags, returned_quantity per item (605-767).
- Migration v4 adds document_type CHECK('sale','void','return'), void_reason, return_reason, original_sale_item_id + indexes (`migrations.rs:230-242`).
- Real two-pane screen wired at `AppShell.tsx:372-374`; void AlertDialog (reason required), return Sheet (per-item qty + reason); original sale never deleted (only INSERT + status UPDATE).

**Partial**
- Auditable void/return attribution: backend records cashier_id + user_id + reason, but the frontend hard-codes `userId:1` for both (`ReceiptsScreen.tsx:173,236`).
- Frontend DoD states: validation + mutation-error present; loading, list-empty, and search/load-error states absent.
- `getReceipt` contract deviation: returns `Option/null` (maps not_found → null) vs spec `Promise<ReceiptDetail>`.

**Missing**
- (Low) Receipt list loading state (no Spinner/Skeleton).
- (Low) Receipt list empty state (empty TableBody, no "Nema racuna").
- (Medium) Search/detail load error handling — `runSearch`/`showDetail` have no try/catch (`ReceiptsScreen.tsx:133-157`); a backend failure becomes an unhandled rejection with no operator UI.
- (Low) No true mid-transaction rollback test (excessive-qty test only proves pre-write rejection).
- (Low) No dedicated v3→v4 forward-migration regression test.

**Backend commands (present / registered):** `receipts_search` (48), `receipts_get` (49), `receipts_void` (50), `receipts_return_items` (51) — all present/registered.

**Frontend screen:** Real two-pane (search filters + results Table left; detail panel with items/payments/totals/linked docs + actions right). Void AlertDialog requires reason; return Sheet has per-item qty + reason. States: validation + mutation-error present; loading, list-empty, and search/load-error **absent** (`hasLoadingEmptyErrorStates: false`).

**Business rules**

| Rule | Status |
|---|---|
| Completed receipt can be voided once | Met |
| Partial return cannot exceed sold minus already returned | Met |
| Return/void writes inventory movements | Met |
| Return/void updates inventory balances | Met |
| Original receipt remains readable | Met |
| Returned doc uses negative quantities + explicit return/void type | Met |

**Acceptance criteria**

| Criterion | Met |
|---|---|
| Racuni is a real history and operations screen | Yes |
| Original sale is never deleted | Yes |
| Voids/returns are auditable and stock-correct | Partial (stock-correct yes; auditable weakened by hard-coded userId) |

**Definition-of-Done checklist**
1. Distinct useful screen — Met
2. Service-backed — Met
3. Rust validation/transactions — Met
4. Structured Serbian errors — Met
5. Frontend states — Partial (no loading/empty; unhandled read errors)
6. Tests happy + failure — Met
7. Local-first — Met

**Test coverage & gaps:** Backend (6) — search returns completed sale; detail includes items/payments/flags; void creates linked doc + restores stock; void rejects duplicate; return rejects excessive qty (no inventory change); return creates linked doc for selected qty. Frontend (3 in App.test.tsx) — opens Racuni; requires reason before void; applies partial return + shows linked doc; adapter tests. Gaps: no true post-write rollback test; no loading/empty/error UI tests (states not implemented); no v3→v4 forward-migration test; detail "totals + linked status" only weakly asserted.

**Auditor notes:** Confidence high; agrees, no over-claims, no stubs. **Under-claims:** the hard-coded `userId:1` is a **systemic project pattern** (Inventory does it too), and the persisted void/return reason text is loaded from the backend but never displayed in the UI. Nudged to 86 because three gaps touch explicit spec/foundation requirements (auditability, the spec-listed rollback test, the forward-migration regression) rather than pure polish.

**Key risks:** Hard-coded `userId:1` always attributes void/return to user 1; unhandled promise rejections on search/detail failure leave no operator feedback; search hard-capped at LIMIT 100 with no pagination (large histories silently truncate); rollback is structurally correct but not directly proven.

**Evidence:** `receipts.rs:175-213`, `322-603`, `913-967`, `1176-1516`; `lib.rs:48-51`; `migrations.rs:230-242`; `app_error.rs:82-85`; `ReceiptsScreen.tsx:133-257`, `394-425`; `ports.ts:125-130`; `local-adapter.ts:125-130`; `mock-adapter.ts:588-644`; `App.test.tsx:592-646`.

---

### 06 — Reports — 80% — Mostly done

Audited **80%** vs assessed 82% — a **material disagreement** (−2). The audit went lower because shift/cashier filtering has **no backend support at all** and the spec's "Product ledger report" scope line is entirely absent from this module.

**What's done**
- All 8 commands as real SQL in `reports.rs`, registered (`lib.rs:35-42`), exposed via `ReportsService` (`ports.ts:140-149`), mapped (`local-adapter.ts:141-157`), mocked (`mock-adapter.ts:729-840`), typed (`types.ts:486-614`).
- Real `ReportsScreen.tsx` wired at `AppShell.tsx:364-366`: Promet/Artikli/Lager/Izvoz tabs, 5 metric cards, BarChart, tables matching spec columns.
- Net-of-voids math across all queries (311-313, tested 1063-1067); labeled margin estimate from `purchase_price_minor` ("Marza je procena na osnovu nabavne cene"); low-stock query; CSV export with Serbian headers for all 7 report types written to a local exports dir.

**Partial**
- Empty states: handled for daily chart, low-stock, and CompactTable; ProductSalesTable + CategorySalesTable render empty bodies with no Empty component.
- Acceptance "real admin screen": real and functional, but not role-restricted to admin/owner.
- Backend per-query coverage: daily/payment/product/low-stock + CSV(daily) tested; shift, cashier, category queries untested; CSV tested only for daily turnover.
- Structured Serbian errors: infra errors serialize fine, but report commands do no domain validation (e.g. no invalid-date-range code).

**Missing**
- (Medium) Common filters shift/cashier/category/product — UI exposes only date range; `toProductSalesQuery` hard-codes categoryId/productId to null; shift/cashier have **no backend query support**.
- (Low) Date-range validation state (bad/inverted ranges silently return empty).
- (Low) Admin/owner role gating for the screen (cashiers can currently see turnover + margin).
- (Medium) Frontend empty/error/failure-path tests entirely absent.

**Backend commands (present / registered):** `reports_daily_turnover` (35), `reports_shift_turnover` (36), `reports_cashier_turnover` (37), `reports_payment_methods` (38), `reports_product_sales` (39), `reports_category_sales` (40), `reports_low_stock` (41), `reports_export_csv` (42) — all present/registered.

**Frontend screen:** Real, non-stub. States: loading (Skeleton), error (Alert + sonner toast), several empty states. Caveats: only date-range filter; no validation state; product/category tables lack empty state; not admin-gated.

**Business rules**

| Rule | Status |
|---|---|
| Voids/returns reflected consistently (net decision) | Met |
| Margin is a labeled estimate from purchase price | Met |
| Reports must not mutate data | Met |
| CSV uses Serbian-friendly headers | Met |
| Use sale-item snapshots for historical product reporting | Met |

**Acceptance criteria**

| Criterion | Met |
|---|---|
| Izvestaji is a real admin screen | Partial (real, but not role-gated) |
| Owner can answer daily total / cash-card / best products / low stock | Yes |
| CSV export works for daily turnover and product sales | Yes |

**Definition-of-Done checklist**
1. Distinct useful screen — Met
2. Service-backed — Met
3. Rust validation/transactions — Met (read-only; no domain validation)
4. Structured Serbian errors — Partial (no report-specific codes)
5. Frontend states — Partial (no validation; product/category empty states missing)
6. Tests happy + failure — Partial (no error/empty/failure FE tests)
7. Local-first — Met

**Test coverage & gaps:** Backend (5) — daily turnover net-of-voids, payment methods, product sales net qty/revenue/margin, low stock, CSV daily with Serbian headers. Frontend — renders tabs, applies date filters, exports CSV + path toast; adapter command-name test. Gaps: no error/export-failure/empty/loading FE tests; no shift/cashier/category backend query tests; CSV only tested for daily; no category/product filter param test (UI never sends them).

**Auditor notes:** Confidence high; broadly agrees but lowered to 80%. **Over-claims corrected:** (1) the filters gap is deeper than "UI doesn't send them" — `ReportDateQuery` has only from/to; shift/cashier filtering is unsupported by the queries themselves; (2) the spec scope line "Product ledger report" (`06-reports.md:27`) is entirely absent here (exists only in the inventory module) and was unmentioned. **Under-claims:** "real admin screen" is harshly marked partial — the primary meaning (real, non-stub screen) is fully met and the spec never explicitly mandates runtime role enforcement; backend CSV is genuinely complete for all 7 types, not just the 3 exposed. No stubs/placeholders.

**Key risks:** Cashiers can view all turnover/margin (no role gate); category sales group by *current* category, so reclassification shifts historical numbers (acknowledged in UI); unvalidated date ranges silently yield empty reports; the spec's analytical filters are largely unavailable.

**Evidence:** `reports.rs:208-280`, `282-608`, `610-813`, `1048-1176`; `lib.rs:35-42`; `ports.ts:140-149`; `local-adapter.ts:141-157`; `mock-adapter.ts:729-840`; `types.ts:486-614`; `ReportsScreen.tsx`; `ReportsScreen.test.tsx`; `reports-adapter.test.ts`; `AppShell.tsx:364-366` (no role gate); `06-reports.md:27` (absent ledger report).

---

### 07 — Import — 90% — Mostly done

Audited **90%** vs assessed 88% — a **material disagreement** (+2, the only upward revision). The audit raised it because two "partial" deductions were overstated (unknown-VAT hard-fail is an allowed policy; Progress is a sanctioned loading component).

**What's done**
- All 5 commands present, registered (`lib.rs:52-56`).
- Transactional commit: `commit_import` re-validates and rejects on `error_count>0` **before** opening the transaction (`importer.rs:361-377`), then wraps `import_jobs` + `import_job_rows` + products/categories + inventory movements/balances in one rusqlite transaction (382-436); test proves no partial writes on failure (1636-1666).
- CSV parsing with BOM strip, quoted fields, delimiter auto-detect (`;`/`,`/tab); product required/optional fields; duplicate matching order barcode→sku→name-as-warning; initial-stock match + positive-qty + "receive" movement with reference_type "import"; category create/update; VAT resolution by basis points or name; Serbian decimal-comma money/qty parsing.
- Real wizard `ImportWizard.tsx` (629 lines): type select, local file read, alias auto-mapping, column-mapping selects, dry-run validation Table with row numbers, commit AlertDialog, persisted history; wired (`AppShell.tsx:376-378`) through ports + local/mock adapters.

**Partial**
- Loading state: `isBusy` disables buttons + a step Progress bar; no spinner during async work (low severity — Progress is sanctioned).
- Unknown-VAT policy: hard-fail only; spec allows fail OR warn, so the rule is effectively met; only configurability + a test are missing.
- Frontend test breadth: 2 flow tests + adapter mapping; no dedicated `ImportWizard.test.tsx`; "Validiraj disabled when required field unmapped" untested.

**Missing**
- (Low) XLSX import — CSV only; spec marks XLSX optional, so an allowed omission.
- (Low) Job-detail drill-down UI — `import_get_job`/`getImportJob` exist and return per-row detail, but the history table has no click-through.
- (Low) Visible spinner during async file-read/validate/commit.

**Backend commands (present / registered):** `import_read_headers` (52), `import_validate` (53), `import_commit` (54), `import_list_jobs` (55), `import_get_job` (56) — all present/registered (`import_get_job` wired in adapters but not consumed by the UI).

**Frontend screen:** Real multi-section wizard. States: error Alert, success Alert, validation table + error Alert, empty states for no-error-rows and empty history. Loading state weak (button-disable + step Progress, no spinner). No job-detail drill-down despite `getImportJob` being available.

**Business rules**

| Rule | Status |
|---|---|
| Product required fields (name, sale price, VAT, SKU/code or barcode) | Met |
| Product optional fields handled | Met |
| Duplicate matching order barcode → SKU → name-as-warning | Met |
| Import never deletes existing products | Met |
| Initial stock matched by barcode or SKU/code | Met |
| Initial stock quantity required and positive | Met |
| Initial stock writes receive/correction movement | Met |
| In-file duplicate detection | Met |
| Unknown VAT rate fails or warns per policy | Met (hard-fail; configurability + test absent) |

**Acceptance criteria**

| Criterion | Met |
|---|---|
| Import is a real migration tool, not a file-picker placeholder | Yes |
| User can validate a CSV before writing data | Yes |
| Commit is transactional | Yes |
| Import history is persisted | Yes |

**Definition-of-Done checklist**
1. Distinct useful screen — Met
2. Service-backed — Met
3. Rust validation/transactions — Met
4. Structured Serbian errors — Met
5. Frontend states — Met (loading weak but present)
6. Tests happy + failure — Met
7. Local-first — Met

**Test coverage & gaps:** Backend (10) — header detection, requires-mapping, invalid-money with row number, warns-existing-barcode, commit transactional, commit rejects invalid (no partial writes), initial-stock receive movement, unknown-VAT row error, known-VAT row accepted, initial-stock required-mapping. Frontend — validates rows before commit ("Red 2" + commit disabled), commits valid CSV after dry-run + history; adapter mapping. Gaps: no category import test; no `import_get_job` retrieval test; no dedicated `ImportWizard.test.tsx`; no read-headers/parse-error path test.

**Auditor notes:** Confidence high; agrees on the core conclusion and raised to 90%. **Over-claims corrected:** the "8 backend tests" bullet was a miscount — the suite now has **10** `#[test]` functions after adding the spec-named unknown-VAT (fail + resolve) and initial-stock required-mapping tests; and marking the unknown-VAT rule as not-met understates compliance since hard-fail is an allowed policy. **Under-claims:** loading-state severity is low (not medium — Progress is sanctioned); FE coverage is slightly better than implied (4 of 5 spec FE scenarios). **Notes (not stubs):** `import_jobs.error_rows` is hard-coded to 0 on commit (correct given commit blocks on any error); when SKU is unmapped, commit copies barcode into the NOT-NULL UNIQUE sku column (intentional workaround).

**Key risks:** Minimal loading UX on large CSVs; commit re-parses/re-validates the file (correct but duplicative); `error_rows` never reflects skipped rows; no XLSX path means XLSX-only clients must convert to CSV first.

**Evidence:** `imports.rs:1-46`; `importer.rs:316-505`, `520-690`, `692-921`, `1484-1696`; `lib.rs:52-56`; `migrations.rs:138-175`; `ImportWizard.tsx:1-630`; `AppShell.tsx:376-378`; `ports.ts:132-138`; `local-adapter.ts:132-140`; `mock-adapter.ts:646-728`; `App.test.tsx:648-722`.

---

### 08 — Settings & Backup — 80% — Mostly done

Audited **80%** vs assessed 84% — the **largest material disagreement** (−4). The audit went lower because, beyond the two named high-severity gaps, the VAT screen is create-only (the spec's "deactivate instead of delete" workflow is unreachable by an operator).

**What's done**
- All 10 commands present, registered (`lib.rs:57-62`, `74-78`), plus a bonus `backup_update_settings`.
- Real tabbed `SettingsScreen.tsx` (Radnja/PDV/Racuni/Korisnici/Backup) wired at `AppShell.tsx:355-361` with the users panel injected; loading/error/empty/validation states.
- Rust-owned validation + transactions: company validation incl. 9-digit PIB and RSD-only currency; tax-rate save in a transaction; receipt validation; backup job insert in a transaction.
- Backup uses the SQLite **backup API** (`source.backup(...)`, not file copy); restore uses `conn.restore` + re-migrate; pre-restore backup created first; restore gated by exact "VRATI PODATKE" in both Rust and the UI AlertDialog.
- Migration v2 rebuilds `backup_jobs` to add file_size_bytes + completed_at and widen backup_type to include restore/pre_restore; receipt numbering genuinely consumed/incremented inside the sales transaction (`sales.rs:597-630`).

**Partial**
- Stale-backup detection: `stale = last_successful_backup.is_none()` — flags only "never backed up", not age-based, so an old-but-once-successful backup gives no "Backup kasni" warning.
- VAT hard-delete protection: satisfied at API level (no delete command; deactivate via `active` flag) but **the UI offers no way to edit/deactivate an existing rate** (see Auditor notes).
- Tabs: hand-rolled `role=tablist` buttons instead of the prescribed shadcn Tabs.
- No dedicated v1→v2 backup_jobs migration-forward data-preservation test.

**Missing**
- (High) Admin role-gating for receipt sequence updates — `settings_update_receipt`/`save_receipt_settings` take no user/role argument and perform no admin check; navigation carries no role requirement and the screen renders for any role.
- (High) Frontend component tests for Settings/Backup — none of the 5 spec FE tests exist; no `SettingsScreen.test.tsx` (only adapter-level wiring tests).

**Backend commands (present / registered):** `settings_get_company` (57), `settings_update_company` (58), `settings_list_tax_rates` (59), `settings_save_tax_rate` (60), `settings_get_receipt` (61), `settings_update_receipt` (62, no admin check), `backup_get_status` (74), `backup_create` (76), `backup_restore` (77), `backup_list_jobs` (78) — all present/registered; bonus `backup_update_settings` (75).

**Frontend screen:** Real tabbed admin screen. States: loading badge, error Alert, empty backup-jobs row, validation messages, error Alerts. Deviations: tabs are custom buttons (not shadcn Tabs); currency field correctly disabled; **VAT tab is create-only** (no edit/deactivate affordance).

**Business rules**

| Rule | Status |
|---|---|
| PIB field exists with basic validation | Met |
| Currency is RSD and not freely changed | Met |
| VAT rates cannot be hard-deleted; deactivate instead | Met at API level / unreachable in UI |
| Receipt sequence updates require admin role | Missing |
| Restore replaces data and requires confirmation | Met |
| Restore creates a pre-restore backup first | Met |

**Acceptance criteria**

| Criterion | Met |
|---|---|
| Podesavanja is a real admin screen | Yes |
| Shop profile and VAT settings persist | Yes |
| Manual backup works | Yes |
| Restore is guarded by confirmation | Yes |
| Backup status available for shell warning | Yes (stale logic is is_none-only) |

**Definition-of-Done checklist**
1. Distinct useful screen — Met
2. Service-backed — Met
3. Rust validation/transactions — Met
4. Structured Serbian errors — Met
5. Frontend states — Met
6. Tests happy + failure — Partial (no FE component tests)
7. Local-first — Met

**Test coverage & gaps:** Backend (8) — company round-trip, invalid-PIB reject, tax-rate create/update, receipt numbering round-trip, backup settings drive status, manual backup creates SQLite copy + success job, restore rejects missing file, restore requires confirmation text. Frontend — adapter command-name + payload mapping + mock round-trips only. Gaps: no `SettingsScreen.test.tsx`; missing all 5 spec FE tests (tab content, company save + toast, VAT validation render, backup stale/failed render, restore confirmation); no "unreadable file" restore test; no v1→v2 migration-forward data-preservation test.

**Auditor notes:** Confidence high; broadly agrees, adjusted 84→80. **Over-claims corrected:** (1) the VAT "deactivate instead of delete" rule is only met at the API level — `TaxRateDialog` always submits `id:null` and resets to empty, and rows have no edit affordance, so an operator cannot edit/deactivate an existing rate; (2) admin role-gating is missing at **every** layer (Rust command takes no session arg; nav items carry no role; the settings screen renders with no role check), so a cashier reaching Podesavanja can change receipt numbering. **Under-claims:** receipt numbering is atomically consumed/incremented in the sales transaction; migration v2 carefully backfills `completed_at` and widens the CHECK. No placeholder pages — VAT is create-only (a missing-feature gap, not a stub).

**Key risks:** Receipt numbering changeable by any role (data-integrity/audit risk); UI behavior untested at component level (form/toast/validation/restore-gate regressions would pass CI); stale-backup warning only fires when no backup ever existed; restore-then-re-migrate behavior on a corrupt/older-schema backup is untested.

**Evidence:** `settings.rs:105-352`; `backup.rs:67-482`; `lib.rs:57-78`; `SettingsScreen.tsx:1-953` (VAT create-only 481-551); `AppShell.tsx:355-361`, `417-444`; `ports.ts` (Settings/Backup); `local-adapter.ts:47-70`; `mock-adapter.ts:207-313`; `migrations.rs:159-218`; `sales.rs:597-630`; `local-adapter.test.ts:90-154`, `499-522`.

---

## Cross-Cutting Observations

**Shared-foundation health is excellent and is carrying the app.** The foundation (module 00) is genuinely complete: strict service-port layering (only `local-adapter.ts` imports Tauri), a transactional append-only migration runner, a uniform `{code, message, details}` error contract with Serbian operator messages, BigInt integer-money helpers, and every canonical command registered. Crucially, the foundation already ships ~8000 lines of real downstream command code, so the contracts are exercised by real callers, not theory. This is why every higher module could be rated "mostly done" or better.

**Systemic test gaps skew toward the frontend.** Backend coverage is strong (67 passing Rust tests, with transactional rollback and failure-path tests in nearly every module). The weak spot is UI test breadth: module 08 ships **zero** Settings component tests (all 5 spec FE tests absent), module 06 has no error/empty/failure FE tests, and modules 03/05 lack dedicated screen test files. Several spec-named tests are missing module by module (01 deactivated-login, 07 unknown-VAT/required-mapping, 05 true post-write rollback). None of these fail CI today precisely because they don't exist — which is the risk.

**Command registration is uniformly correct.** Across all 9 modules every spec command is present *and* registered in the `lib.rs` invoke_handler, plus three intentional bonus commands (catalog barcode lookup, backup settings update). The only registration-adjacent finding is that `import_get_job` is wired through the adapters but not yet consumed by the Import UI — a missing drill-down, not a broken wire.

**Two recurring patterns hold back MVP polish.** (1) **Hard-coded `userId:1`** in Inventory and Receipts/Returns frontends breaks the operator audit trail even though `session.user.id` is available and is correctly threaded into other screens (Users). (2) **No role enforcement** on admin-only surfaces — Reports and the whole Settings screen render for any logged-in role, and the spec's "receipt sequence requires admin" rule is unenforced at every layer. Both are small, localized wiring fixes with outsized correctness/compliance impact.

**Local-first integrity is intact everywhere.** Every module runs against local SQLite with `not_fiscalized` defaults and no Medusa/cloud references. The single network touch is the optional Open Food Facts barcode lookup in Catalog, which degrades gracefully and never blocks local use.

---

## Recommended Next Steps

Prioritized to reach MVP, following the spec's Recommended Order (01 → 02 → 03 → 04 → 05 → 06 → 07 → 08) and front-loading the highest-severity, lowest-effort correctness fixes:

1. **(01) Add the spec-required "deactivated user cannot log in" backend test** and a close-shift expected-cash-derivation test. Behavior exists; this closes a known regression hole cheaply.
2. **(08) Enforce admin role-gating for receipt-sequence updates** end to end — add a session/role argument to `settings_update_receipt`, gate the Podesavanja nav/screen by role, and reject non-admin callers in Rust. This is the only outright-missing business rule in the app.
3. **(08) Make the VAT screen edit/deactivate an existing rate** (wire `TaxRateDialog` to pass the rate `id` and expose a row edit/deactivate action), making the "deactivate instead of delete" workflow actually reachable.
4. **(03, 05) Thread the real session user into Inventory and Receipts** to replace `userId:1`, restoring a correct audit trail for movements, voids, and returns.
5. **(04) Render the swallowed preview/validation error state** in RegisterScreen so cashiers see why an over-limit discount or invalid quantity blocks completion.
6. **(05) Add receipt-list loading + empty states and try/catch around search/detail reads**, eliminating the current unhandled rejections; surface the persisted void/return reason text.
7. **(06) Add backend shift/cashier filtering** (extend the report query params) and **role-gate the Reports screen**; add the missing error/empty/failure FE tests and date-range validation.
8. **(02) Delete the orphaned `ProductCatalogScreen.tsx`** and deep-link the product "Lager" row action to that product's ledger.
9. **(07) Surface job-detail drill-down** by consuming `import_get_job` in the history table; add the unknown-VAT and required-mapping tests; correct the "8 tests" miscount in docs.
10. **(08) Add the missing Settings/Backup component tests** (tabs, company save + toast, VAT validation, backup stale/failed render, restore confirmation) and make stale-backup detection age-based rather than `is_none`-only.
11. **(00) Wire the shell header to `settings_get_company`** so the company shop name replaces the hard-coded "VantumPOS", and add a pre-existing-old-DB forward-migration regression test to protect the installed base.
12. **Reconcile the sales/inventory write divergence (03/04):** unify the inventory movement/balance write path (or add a negative-stock guard to the sales path) so the two code paths cannot drift in balance math or stock rules.

Items 1-4 are the highest-leverage correctness/compliance fixes and should land first; items 5-12 are polish and test-breadth that move the four "mostly done" modules to "complete" and lift the overall figure above 88%.