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
| Tier-resolved legal copy | `legal.rs` — the only module allowed to hold a fine figure; every SW-11/SW-15 figure resolves through it, and a test pins that "privredni prestup" is unreachable under the preduzetnik regime. The SW-7 reklamacija amounts followed on 31.07.2026: `legal.rs::reklamacija_breach` is regime-versioned as well as tier-resolved, rides on `ReklamacijaView.notice`, and the frontend renders it without deriving a figure of its own. Its summary was widened on 02.08.2026 — čl. 210 st. 1 tač. 24 penalises the čl. 63 st. 3 fee ban too, so "u propisanim rokovima" understated what the figure sanctions |
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

- **Req 39 / §4 item 8 — false archive duty on screen: closed 07.08.2026 (`41dc298`).** At the time of
  this batch `src/app/settings/SettingsScreen.tsx:1773` told the operator „Pravna lica ne smeju
  uništavati dokumentarni materijal bez pismenog odobrenja arhiva.“ ZAG čl. 16 st. 2 confines prior
  written archive approval to the **public sector**; the memo (§1 row 5) required the claim removed, and
  it was — deleted outright, with the neutral ZAG čl. 9 st. 1 custody note req. 39 asks for beside it in
  place of silence. Pre-existing (SW-3, `062250d`), not introduced by this batch.
- **Req 35–38, 42, 43 — retention: the shared table shipped 01.08.2026 (v17); the general engine did
  not. Re-stated 08.08.2026** — this bullet read *„No `retention_class`, `retain_until`, `legal_hold` or
  upward-only extension exists anywhere in the schema“* for a week after v17 built all four, which is the
  same defect as a false promise pointed the other way. `retention_policies` (`db/migrations.rs:734`)
  carries `record_class`, `retain_until`, `legal_hold` and `never_purge`; `retention.rs` is the shared
  table req. 42 mandates, with nine declared classes in `retention::RecordClass::ALL`, and v18 gives
  `processing_activities` a `retention_record_class` foreign key onto it so the čl. 47 register cannot
  state a rok the app does not apply. `retention::extend_retain_until` is req. 36's upward-only
  extension: it **refuses** a candidate earlier than the stored floor rather than clamping it, and
  refuses a `trajno` class outright. What is still owed within req. 36 is register row 20's own residual
  — no general upward-only `retain_until` engine over trading data; the declared classes are the
  worktime, personnel, credential, access-log, čl. 47, cenovnik and popis ones only, and the KEP book
  carries its own 5-year floor in `kep_close.rs::retention_floor`. Genuinely unbuilt, and unchanged: the
  čl. 32 objekti/ulaganja register (req 37) and the documented plain-text archival export (req 43). The
  same screen line (`SettingsScreen.tsx:1771`) attributed the 10-year floor to **ZPDV čl. 47** until
  07.08.2026 (`41dc298`), when the dialog was bound to `commands::backup::ROK_CUVANJA_PRAVNI_OSNOV` —
  ZoRač čl. 28 st. 4; ZPPPA čl. 114ž — the string the go-live tombstone had carried since 31.07.2026.
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

- **Req 39 / §4 item 8 — false archive duty on screen: closed 07.08.2026 (`41dc298`).** When this batch
  shipped, `src/app/settings/SettingsScreen.tsx:1773` told the operator „Pravna lica ne smeju uništavati
  dokumentarni materijal bez pismenog odobrenja arhiva.“ ZAG čl. 16 st. 2 confines prior written archive
  approval to the public sector, so the sentence sent a preduzetnik for a permission no article asks of
  him, one click above an irreversible delete. It was deleted outright rather than hedged, and the
  neutral ZAG čl. 9 st. 1 custody note req. 39 asks for in the same breath — savesno čuvanje u sređenom i
  bezbednom stanju — stands in its place, so the withdrawal does not read as „archive law does not reach
  you“ (§4 item 8). Pre-existing (SW-3, `062250d`); untouched by *this* batch. **What req. 39 leaves
  open:** the pravno-lice half — lista kategorija sa saglasnošću nadležnog javnog arhiva, arhivska
  knjiga, 30 April prepis — is profile-aware copy nobody has written, although `pravna_forma` is stored.
- **Req 35–38, 42, 43 — retention: the shared table shipped 01.08.2026 (v17); the general engine did
  not. Re-stated 08.08.2026** — like its twin in the batch above, this bullet read *„No
  `retention_class`, `retain_until`, `legal_hold` or upward-only extension in the schema“* after v17 had
  built all four. `retention_policies` (`db/migrations.rs:734`) carries `record_class`, `retain_until`,
  `legal_hold` and `never_purge`; `retention.rs` is the shared table req. 42 mandates, with nine classes
  in `retention::RecordClass::ALL` and a `retention_record_class` foreign key from v18's
  `processing_activities`. `retention::extend_retain_until` is req. 36's upward-only extension and
  refuses a shortening rather than clamping it — so *„what req. 36's upward-only `retain_until` will
  have to implement“*, as this bullet used to end, is owed only for the part row 20 still names: no
  general engine over trading data, the declared classes being the worktime, personnel, credential,
  access-log, čl. 47, cenovnik and popis ones. Still genuinely unbuilt: the čl. 32 objekti/ulaganja
  register (req 37) and the documented plain-text archival export (req 43). `SettingsScreen.tsx:1771`
  attributed the 10-year floor to **ZPDV čl. 47**, and framed it as a ceiling („do 10 godina“), until
  07.08.2026 (`41dc298`): the dialog now prints `commands::backup::ROK_CUVANJA_PRAVNI_OSNOV` — ZoRač
  čl. 28 st. 4; ZPPPA čl. 114ž — as a **floor** („najmanje 10 godina … rok se može produžiti, a nikada
  se ne skraćuje“), which is what §1 row 7 corrected.
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
| 4 — protection | `worktime.rs::check_protection` — čl. 87 (8 h/day for a minor; the 35 h/week leg **closed 07.08.2026**, one cycle after this batch — see the section below), **čl. 88 st. 1 bans prekovremeni *and* preraspodela**, čl. 91 st. 1 (dete do 3) and st. 2 (**samohrani roditelj — threshold SEVEN**, plus `dete_tezak_invalid` with no age limit) require a stored written consent **dated before the day worked**, čl. 90 warns rather than blocks. `derives_overtime_automatically` returns `false` under preraspodela — čl. 58 hours are not overtime | `defcecf`, `8c04a05` |
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
claims more than the code delivers. `worktime.rs::the_cl_87_weekly_leg_is_not_checked` pinned the gap in
behaviour — six eight-hour days raise nothing — so building the leg would break the test and force the
prose to be re-stated in the same commit. *(That is what happened on 07.08.2026: both that test and
`docs_guard::no_document_claims_the_cl_87_weekly_leg_is_enforced` were deleted by their own
instructions, and the second was replaced by its inverse,
`no_document_says_the_cl_87_weekly_leg_is_still_unbuilt`, which now fails on a document that denies the
leg. See the section below.)*

Latest migration: **v17**.

**Deliberately not built** (`docs/SW14-VERIFIED-RULES.md` §5) — read these as decisions, not as gaps:
payroll of any kind (čl. 24 tačke 2–3 are the accountant's), an obračun zarade, the čl. 108 uplifts,
any free-text/diagnosis/doznaka field on an absence row, biometric clock-in, an annual overtime counter,
a `hours > 8 ⇒ prekovremeni` rule during preraspodela, an employee-side export, and any consent UI.

**Still open after this batch** (requirement numbers are `docs/SW14-VERIFIED-RULES.md` §4):

- **Req 12, weekly leg — the čl. 87 cap of 35 časova nedeljno for an employee under 18 — was closed on
  07.08.2026** and is moved out of this list into the section that records the work. What it said stands
  as the reason that work was done, and is kept here rather than deleted: `check_protection` enforced only
  the 8 h/day leg (`MINOR_DAILY_CAP_MINUTES`); the weekly leg needed the employee's week, which that
  signature did not carry and no caller supplied. The under-18 čl. 88 st. 1 bans on prekovremeni and
  preraspodela *were* enforced, so a minor could not accumulate the week through overtime — but a minor
  scheduled 7 h a day across six days raised nothing. `MINOR_WEEKLY_CAP_MINUTES` and
  `ProtectionKind::MaloletanNedeljniLimit` now close it, and the block **refuses the row**: čl. 87 states
  the prohibition itself, so it is not an overridable čl. 53 cap. What it refuses is the write that
  **raises** the week — an entry adding nothing to the register (a correction downwards, a day of pure
  absence) cannot be the write that puts a minor over the cap, and refusing it would make a week that was
  already above the cap when the guard turned on permanently unwritable. The total counts only the days the
  employee **was** under 18, and drops rows whose `dan` is not a civil date, because over-counting here
  refuses a lawful day instead of asking for a ground. **One limb of req. 12 did not close with
  it:** the čl. 88 st. 2 **night** ban, which needs the čl. 62 night computation `DayHours` does not carry
  — the same reason the night limbs of čl. 90 and čl. 91 are absent.
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
- **Req 28 — remote-support masking** of the absence-reason column, with the unmask logged.
  **Re-stated twice on 09.08.2026, and the second re-statement is the one that matters.** The bullet
  first ended *„is not built (depends on SW-10)“*; it was then re-stated to owe three things, of which
  **two shipped the same afternoon** and the sentence naming them is withdrawn — *„Still owed: a surface
  that reaches the verb, a grid cell that says skriveno rather than the em dash, and the ZEOR čl. 24
  tač. 1 buckets“*. What the code does: `commands/worktime.rs::razlog_odsustva_dostupan` withholds the
  category from every register read and every export while a čl. 46 nalog is live,
  `support_sessions.odsustvo_otkriveno_at` (v23) is the shop's per-nalog unmask stamp,
  `commands::audit::reveal_absence_reason` writes the čl. 48 line for it carrying no category, no
  employee name and no month, `SupportApprovalPanel` is the vlasnik's admin-gated control beside
  Daljinska podrška, and `AbsenceCell` renders „Odsutan (razlog skriven)“ instead of an em dash.
  **It is still a partial, for one reason rather than three:** the ZEOR čl. 24 tač. 1 buckets ride on
  the JSON payload and on the exported file, so a client reading either recovers the reason, and the
  rendered grid discloses the tenth category by subtraction. Register row 12 states the whole of it.
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
| `b495425` | Three compliance rows credited the code with legs it does not have — the čl. 87 weekly cap, the čl. 57 st. 5 ceiling attributed to `assess_caps` rather than to its caller, and an unqualified „no fine figure outside `legal.rs`“ | The rows were re-stated, and the defect class got a guard: **`src-tauri/src/docs_guard.rs`** (test-only, `#[cfg(test)]` in `lib.rs`) embeds `SERBIAN-LAW-COMPLIANCE.md`, `PROGRESS.md`, three `docs/compliance/` templates — the čl. 23 notice, the čl. 47 evidencija, and the anti-evazioni memo since 07.08.2026 — **and** the `retention.rs` `napomena` strings, and fails when a line claims more than the code delivers |
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
- **SW-14 req. 28 — remote-support masking** of the absence-reason column, with the unmask logged.
  **Re-stated 09.08.2026, and again the same day when the surfaces landed (`7db90f7`, `2801dff`).** This
  bullet said *„is still not built“* while `commands::worktime::list_month` and `export_month_csv` were
  already withholding `kategorija_odsustva` under a live čl. 46 nalog (the decision is
  `razlog_odsustva_dostupan`, read off the nalog and never out of `audit_events`), while the unmask was
  already a stamp on `support_sessions.odsustvo_otkriveno_at` (v23) with no re-mask verb behind it, and
  while `commands::audit::reveal_absence_reason` was already writing one čl. 48 line carrying no
  category, no employee name and no month. Its replacement then listed three owed limbs, of which two —
  *„no surface reaches the unmask verb“* and *„the payroll grid renders the em dash for a withheld
  reason“* — are withdrawn: the vlasnik's admin-gated unmask sits on `SupportApprovalPanel` beside
  Daljinska podrška and the masked cell says „Odsutan (razlog skriven)“. What is genuinely owed is the
  third, and register row 12 states it: the mask does not reach the ZEOR čl. 24 tač. 1 buckets — every
  category is its bucket minus `_minuta`, so the JSON payload and the exported CSV still say which
  reason it was. „Moji sati“ is left unmasked deliberately, on the ZZPL čl. 26 ground.
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
- **SW-14 req. 28** — remote-support masking of the absence-reason column. **Re-stated 09.08.2026:**
  carried here as an open item, it is a partial as of that day — the backend withholds the category
  under a live čl. 46 nalog (`commands/worktime.rs::razlog_odsustva_dostupan`), the unmask is a per-nalog
  stamp (`support_sessions.odsustvo_otkriveno_at`, v23), `commands::audit::reveal_absence_reason`
  logs it, and the shop reaches the verb from Privatnost → Daljinska podrška. The sentence
  *„the limbs that remain are the unmask control, the masked grid cell and the ZEOR čl. 24 tač. 1
  buckets“* is withdrawn to its last clause: only the buckets remain, and register row 12 states them.
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
| **10** — jedinična cena + jedinica mere in the file | **Built.** The eight columns ship, the unconfigured article publishes an empty cell rather than a guessed one, and both writers of `products` — the catalog form and the CSV import — now refuse a wrong figure through **one** validator, `cenovnik::validate_jedinicna_cena` (D2, then D4 below) |
| **11** — publish rides the write | **Built.** Four paths, each threading `record_offered_price_change`'s own answer; the catalog path adds the jedinična cena, the one published price that log does not watch |
| **12** — till guard | **Built.** Warns and never blocks, above-published direction only, names its exhibit, logs a `cenovnik_price_divergence` row; arms only off a file a target accepted |
| **13** — anonymously fetchable endpoint | **NOT built and disclosed.** There is no endpoint. The seven fetchability rules ship as a contract in `PublishTarget`'s doc comment with a test that keeps them there; the acceptance criteria are F-13 (b) |
| **14** — published-price archive | **Built.** Immutable dated snapshots, current derived not stored, čl. 213 two-year floor, purge that never reaches an outlet's current file |
| **15** — hosted cenovnik endpoint | **NOT built and disclosed.** Founder decision **F-13**. A shop with a website discharges čl. 6 st. 2 today by pointing the local folder at what that site serves |
| **16** — CSV default, mapping in configuration | **Half met and disclosed.** The shape ships; `cenovnik::COLUMNS` is a constant, so a swap is a release. D1 removed the doc sentence that said otherwise |
| **17** — document that the čl. 6 st. 6 standard does not exist | **Done.** §6 items 4 and 12 carry the verified absence, the quarterly Sl. glasnik RS trigger through 01.05.2027, and the *practice, not law* label |
| **18** — 100.000 preduzetnik, `200.000` unreachable | **Done.** `legal.rs::cenovnik_not_published`, and the blanket guard on the fixed-sum phrasing *„200.000 dinara“* — the bare literal cannot be the needle, because ZoR čl. 274 st. 2 legitimately fines a preduzetnik *„od 200.000 do 400.000“* elsewhere in the product |

**D4 — the import path clobbered the jedinica mere and could publish a wrong jedinična cena. FIXED
02.08.2026.** `[LEGAL]` D2 closed the catalog form. It did **not** close `importer.rs`, which writes
`products` with its own `UPDATE`/`INSERT` and never called `normalize_product_request`. Both legs were
reproduced as failing tests before either was fixed:

- **The measure was rewritten from a column nobody mapped.** `unit_of_measure` is `required: false` in
  the mapping step (`ImportWizard.tsx:109`), and when unmapped `optional_value` returned `""`, which
  the importer substituted with **`"kom"`**. A routine price-update import therefore silently rewrote
  the jedinica mere of every article it matched. **Fix:** `supplied_unit_of_measure` returns `None` for
  an unmapped column *and* for a blank cell in a mapped one — both are „not supplied“, neither is a
  measure — and the UPDATE writes `unit_of_measure = COALESCE(?5, unit_of_measure)`, so a file that
  says nothing about the measure leaves the shop's own configuration standing. A **CREATE** still
  defaults to `"kom"`: a new row has no prior truth to destroy, and the code says so at the parameter.
  `required: false` on the mapping step is now correct rather than dangerous — unmapped genuinely means
  unsupplied.
- **The clobbered measure then orphaned the jedinična cena pair.** `jedinicna_cena_jedinica` /
  `jedinicna_cena_sadrzaj_milli` are untouched by that UPDATE, and the v19 CHECK constrains only the
  pair against itself, never against `unit_of_measure`. So an article legitimately configured under the
  NULL-sadržaj convention — `unit_of_measure = 'l'`, jedinica `'l'`, sadržaj blank, which is exactly
  what D2's gate *accepts* — came out of an import as `unit_of_measure = 'kom'` with jedinica still
  `'l'`, and the batch's own republish published the D2 figure verbatim: the archived body read
  `SKU-1;;Hleb;kom;372.00;372.00;l;02-08-2026` — *372,00 RSD po litru* for goods the same row says are
  sold **po komadu**. **Fix:** the D2 rule moved out of `commands::catalog` into
  `cenovnik::validate_jedinicna_cena`, and **both** writers now call it — the form maps the returned
  `JedinicnaCenaDefect` to its Serbian field error, the importer to a **row** error, so a refused row
  surfaces in the per-row report the operator already reads instead of failing the file wholesale. The
  importer judges the state the row would *leave* the product in: the measure the file supplies, or the
  stored one when it supplies none, against the product's own pair. One rule, one home — the defect
  existed precisely because there were two write paths and only one guard. A CREATE needs no check: the
  import stores no pair, so a new row has nothing to orphan.

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

### SW-16 — Popis (2026-08-03)

The godišnji and the nivelacioni popis shipped as a ten-task TDD plan
(`docs/superpowers/plans/2026-08-01-sw16-popis.md`, design at
`docs/superpowers/specs/2026-08-01-sw16-popis-design.md`) against the verified rule set in
`docs/REMAINING-SW-VERIFIED-RULES.md` §2c, §3 V5 and §4 reqs. 29–42. Baseline at plan time was
`841ee23` — cargo **792**, bun **450**, migration **v19**. *(The task brief quoted 784; that is the
count at `7399559`/`72797f0`, two commits earlier — `841ee23`, the last SW-12 fix, added eight before
SW-16 started. The +107 below is measured from `841ee23` and is entirely SW-16's, because every commit
after it is a popis commit.)*

**What is deliberately not in it, stated first because the register would otherwise read as a closed
item. There is no print and no export for a popisna lista.** PoP čl. 9 st. 3 authorises the computer
for the obračun *„uz štampanje“* popisnih lista koje potpisuju članovi komisije — printing is express in
the bylaw — and this cycle ships no renderer for one. The count sheet, the liste and the izveštaj are
screens; the shop has to produce the paper the members sign outside this application. Req. 32's header
block (obveznik,
PIB/MB, maloprodajni objekat, broj liste), the per-stavka vrednost columns and the **editable default
template** go with it: `PopisLineView` carries no vrednost, so that half is not frontend-only work.
No task in this plan owned it and none of the copy pretends otherwise — the signature step says the
liste are printed and signed by the members and that recording the potpis here **does not replace the
one on paper**. *(**Closed 07.08.2026 by reqs. 31/32/35** — `popis_export_lista`,
`popis_export_odluka` and `popis_export_plan_rada` write three documents into `exports/`. The
paragraph stands as the disclosure that work answers; it is a dated record of SW-16's own scope. Two
of the limbs it names did **not** close with it: req. 32's **editable default template**, and the
izveštaj, which is still composed on demand and shown on screen. See the 07.08.2026 section below.)*

**And the constraint the whole module is shaped around.** PoP čl. 8 st. 5 forbids releasing book
quantities to the komisija before the counted state is written and signed, so the blindness is enforced
at the **query layer** and anchored on the `faza = 'a'` potpis — never on `status`, which is a claim any
`UPDATE` can make. The v20 write guard protects `popis_lines.knjigovodstvena_kolicina_milli`; the duty
is wider, because the perpetual stanje also sits in `inventory_balances` and is derivable from
`inventory_movements` with no trigger in the way. Every blind read is therefore asserted against a
**seeded perpetual balance that must appear nowhere in the serialized response**, so the withheld half
cannot pass vacuously, and adding an „očekivano“ column by joining `inventory_balances` fails exactly
those tests and nothing else.

| Task | Shipped | Commits |
|---|---|---|
| 1 — schema | **Migration v20** (`popis_stores_blind_count_and_posting_lock`): `popis_sessions`, `popis_lines`, `popis_signatures`, `popis_commission`. Two module invariants live in the engine because migrations are append-only and neither could be added later without a second migration — the čl. 8 st. 5 blind count as a **write guard** anchored on the potpis, and the req. 41 **posting lock** over the session, its stavke, its komisija and any new potpis, with `popis_signatures` immutable from the moment each signature is taken. No popis table is written with `INSERT OR REPLACE`: a REPLACE is a DELETE plus an INSERT no `BEFORE UPDATE` trigger sees. `DELETE` stays open for the req. 42 purge, as v18 and v19 each reasoned. Deviations: identifiers and stored enum values are ASCII-folded per the schema-wide convention (`uskladjivanje_potvrdjeno_at`, `ostecena`, `potrazivanja`), and `popis_commission.uloga` is a closed enum (`predsednik / clan / jedno_lice`) so the čl. 6 st. 1 one-person popis is expressible rather than implied | `3e39197`, `75cc299` |
| 2 — state machine | `popis.rs`: `draft → counting → counted_signed → computed → computed_signed → posted`, with **all thirty (status, event) pairs swept exhaustively** and the error *code* asserted rather than a bare `is_err`. The čl. 8 st. 5 predicate is split on purpose — `book_quantities_visible(status)` is only the status limb and is **module-private**, so „call the whole condition“ is a compile error rather than a doc comment, and `book_quantities_released(status, phase_a_signed)` is what a query layer consults. The enums are pinned to the v20 `CHECK` read back off `sqlite_master` **and** to their own IPC wire form, because serde derives the wire name from the variant identifier while `as_db_str` is a hand-written literal | `d92181b`, `1b74804` |
| 3 — deadline engine | `izvestaj_due`: `rok za predaju FI − 60 dana` for the godišnji popis, `datum popisa + 30 dana` for the nivelacioni. FY2026 → 30.01.2027, FY2027 → **31.01.2028 (the leap case a hardcoded table gets wrong)**, FY2028 → 30.01.2029. The annual rok does not move when the count date does (čl. 13 st. 2 anchors it on the rok za dostavljanje) and the nivelacija limb does not read the filing deadline at all. **One test asserts the source rather than an answer** — no behavioural test can tell a computed 30.01.2027 from a hardcoded one — so the module is scanned for a `gggg-MM-dd` literal and for every wall-clock reader `src/clock.rs` actually exports; the review found the first version of that guard missed `crate::clock::utc_now`, this repository's own canonical reader with 47 call sites. Deviations: the function returns `Result` (it parses two dates it does not own) and takes `Option<&str>` for the filing deadline (čl. 13 st. 2's second limb does not read one) | `a337b54`, `5d043cf` |
| 4 — commands | `commands/popis.rs`, admin-gated. `popis_open` refuses without the **čl. 20 st. 3** usklađivanje confirmation (req. 39). A Phase A payload carrying a book quantity is **refused by name**, never silently dropped. `read_lines` is **two statements rather than one and a filter**: while the data is withheld the query names neither the column nor `inventory_balances`/`inventory_movements`, so there is no value in the row for a refactor or a log line to spill. `popis_sign_phase_a` writes potpis → state → book quantities, and the order is behaviourally proven — reversing it fails five tests because the v20 guard aborts a book quantity written before its potpis. A šifra that resolves to nothing keeps its NULL rather than a zero: gotovina and potraživanja have no perpetual record behind them and a fabricated zero would report a manjak the shop does not have. The review pinned the **identity** of a signed stavka, not only its količina (čl. 8 st. 4 + čl. 9 st. 1 t. 1 — seven fields, refused by name when moved), and implemented the **req. 34 gate**: a `perpetual_odluka_ref` is refused unless a same-year, earlier, `posted` popis exists | `fbfc24d`, `349cc79` |
| 5 — the six liste | `PopisLista`, `konsignacija_rok`, `nedostajuce_liste`, `ensure_liste_kompletne` + the per-lista rules. **Presence is a parameter, not a stored declaration** — nothing in this database says that part of the stock is damaged or that a rail belongs to somebody else, and v20 is spent — so the person taking the popis declares it and the declaration becomes binding. The apoen is `cena_minor` in whole notes and coins (čl. 11 st. 1 is exactly a signed integer minor amount) and the čl. 8 st. 5 potpis freezes it, since a signed „5 × 1.000“ that became „5 × 5.000“ in the obračun would leave the potpis attesting to a cash count nobody took. **No denomination whitelist** — that would present a prudential narrowing as čl. 11 st. 1 and refuse a lawful count of devize. The čl. 2 st. 6 reminder is derived from the liste, carries `datum popisa + 10 dana` and says the app does not deliver the lista | `314db22`, `392c425` |
| 6 — the izveštaj | The eight čl. 13 st. 1 elements as a **closed enum**, five written by the komisija and three computed so nobody is asked to retype the stanje they have just counted. **Composed on demand and stored nowhere** — `compose_izvestaj` is read-only end to end, asserted by a fingerprint over `updated_at`, every line's three figures and the potpis count — and the copy says so, because a shop that typed five paragraphs and closed the screen would otherwise lose them silently. Čl. 8 st. 5 gates it too: a document carrying the knjigovodstveno stanje hands the komisija through prose what a hidden column keeps back. Task 3's open contract closed here — the filing deadline is the `settings.popis` → `rokPredajeFi` key, validated where it is written, and an unset one refuses the annual izveštaj by name rather than presuming 31 March. Valuation is `količina × cena ÷ 1000` in `i128`, rounded **away from zero** so a manjak is never quietly made smaller, refused rather than wrapped. **No natural totals** — mixed units on one lista sum to a number with no unit. The review found the čl. 2 st. 6 limb uncovered, two uputstvo strings promising naturalne količine the payload does not carry, and `#[serde(default)]` making the req. 36 gate opt-in from the wire | `40f8a85`, `5f13bc4` |
| 7 — nivelacija + KEP hook | The čl. 21 duty and the scope narrowing are **two strings**, and the guard is mechanical: no obuhvat string may contain a duty word (`morate`, `dužni ste`, `dužan je`, `obavezno`, `obavezan je`, `nalaže` — „obaveza“ deliberately excluded, since the copy must be able to deny one). The obligation is derived from **`price_history`, not `kep_entries`** — SW-9b writes no ledger row when on-hand is 0, and an obligation derived from the ledger would lose exactly that repricing silently — is **reported, never auto-created** (an app-opened session would assert a čl. 20 st. 3 usklađivanje nobody performed), and only a **`posted`** nivelacija popis discharges it. The scope list is the čl. 8 st. 4 artefact: four fields, **no količina from any source**, and `CeoObjekat` is `products.active = 1` because filtering on what the books say we have leaks stock through the presence of a line. `kep_nivelacija` returns the notice and `KepModule` renders it as a persistent block, so the duty has one wording; posting a nivelacija popis leaves the ledger fingerprint byte-identical | `ffde4af`, `f564631` |
| 8 — legal + retention | `legal.rs::popis_not_conducted` — the **eleventh** notice, in `all_notices`, in the duplicated inline list, asserted count bumped to **11** — printing the preduzetnik **čl. 58, 100.000 do 500.000**. The plan's conditional did not fire: §3 V5 does state a preduzetnik figure, so no tier renders `None`. What the memo warns about is the other direction — **čl. 57 st. 1 tač. 12) is a *privredni prestup*** that ZPP čl. 6 st. 1 confines to a pravno lice, so its 100.000–3.000.000 band would overstate the pilot's ceiling roughly sixfold; **„3.000.000“ is now a blanket needle** in `FORBIDDEN_TO_A_PREDUZETNIK`. Retention is `RecordClass::PopisDokumentacija` on the shared table with the **5-year floor of ZoRač čl. 28 st. 7**, counted by a new clock: `business_year_floor` resolves every day of one business year to the same 31 December per **st. 9**, so a nivelacija counted on 14 May 2026 and the godišnji popis of 31.12.2026 expire together instead of the first going seven months early. The class **holds personal data** — `popis_commission` names each popisivač — hence the twelfth čl. 47 radnja, `popis_imovine`. The plan's „panic-safe floor pattern from SW-9c“ is taken as the **fail-safe**, not as the later-of-two-anchors shape: the popis has one statutory clock, and a `posted_at` limb would have added a year to the ordinary case. The test found a real underflow before the implementation shipped — `{:04}` renders year −4 as `-004`, a success-shaped wrong answer | `dd1940d` |
| 9 — frontend | `src/app/popis/{CountSheet,IzvestajPanel,PopisModule}.tsx` + the `PopisService` port, fifteen wire types, the local adapter, an in-memory double that enforces the same gates, and Popis in the shell. Keyed on the backend's `knjigovodstvoDostupno` — **never on `status`** — and asserted against a payload that carries the book figures anyway, which no real backend sends. Every edit affordance mirrors a write the backend accepts; the three closed states give three different reasons. **`CountSheet` takes `session`, not `status` + `lines`** — the plan's snippet spells the status `countedSigned`, a camelCase form that exists nowhere on the wire. The review moved the req. 36 declaration **out of the izveštaj and onto the count sheet**, where it can still be acted on: asked at the izveštaj it could only fire after the čl. 8 st. 5 potpis, when the only remedy is a whole new popis. It also rendered all six liste including the empty ones, and fixed a negative knjigovodstvena količina making a stavka unsavable | `8760a3a`, `94d1337`, `f41615b` |
| 10 — docs + gates | This section; register row 19 and the SW-16 row in §3 re-stated, both carrying the **missing print/export** and the three open items **R-5**, **R-6**, **R-7**. *(Both cells were re-stated again on 07.08.2026, when the print and the export shipped — see the section below; R-5, R-6 and R-7 are untouched and stand.)* | this commit |

**Verification gates — all six run from the repo root, every command exited `0`:**

| Gate | Result | Exit |
|---|---|---|
| `bun run test` | **503 passed** / 0 failed, 33 files (was 450 / 30) | `0` |
| `bun run build` | tsc + vite, dist written; only the pre-existing chunk-size advisory | `0` |
| `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | **899 passed**; 0 failed, 0 ignored, 0 measured, 0 filtered out (was 792) | `0` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings` | clean, no warnings | `0` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | clean, no output | `0` |
| `git diff --check` | clean, no output | `0` |

Net **+107 cargo / +53 bun** over the plan baseline. Latest migration: **v20**.

**Whose tests those are.** All of them are SW-16's — every commit after `841ee23` is a popis commit.
Cargo: **61** in `commands/popis.rs`, **36** in `popis.rs`, **4** in `db/migrations.rs`, **3** in
`retention.rs`, **2** in `legal.rs`, **1** in `commands/kep.rs`. Bun: **22** in
`src/app/popis/PopisModule.test.tsx`, **18** in `CountSheet.test.tsx`, **10** in
`IzvestajPanel.test.tsx`, **2** in `src/services/local-adapter.test.ts`, **1** in
`src/app/kep/KepModule.test.tsx`.

**Deliberately not built** — read these as decisions, not as gaps, except the first, which is a gap and
is labelled one. **No print and no export for a popisna lista** (above) — a real hole against čl. 9
st. 3, disclosed rather than dressed up, and **closed on 07.08.2026** by the section below. No stored
izveštaj: the document is composed when asked for,
which is why req. 42's retention floor reaches the liste, which are rows, and not the izveštaj, which
lives on paper. No auto-created nivelacija popis. No seeded popisne liste from the nivelacija scope —
`stvarna_kolicina_milli` is NOT NULL, so a seeded stavka would carry a count of zero that nothing
distinguishes from a counted zero, i.e. a manjak of the whole stanje on a document that is evidence. No
denomination whitelist on the gotovina lista. No invented required fields on the other four liste —
nothing in čl. 2 st. 5, čl. 10 st. 3–4 or čl. 12 st. 2 prescribes one, and a refusal there would block a
lawful count in the name of a duty that does not exist. No summed naturalna količina. No `update`,
`delete` or `reopen` verb on the port: čl. 14 st. 3 with ZoRač čl. 8 st. 4 makes a correction a new
document. No automatic deletion of popis documentation — `popis_purge_eligible` is a gate, like the two
beside it, and the stored napomena says so.

**Residuals batch (03.08.2026).** Three defects and two undisclosed gaps, none of which changes a
statutory answer; the gates run at **cargo 901 / bun 505**, migration still **v20**.

| Defect | Fixed |
|---|---|
| **The sixth false-promise artefact**, and the first to reach an inspector-facing document. `retention.rs`'s stored napomena, the `cl47.rs` čl. 47 register entry and `commands/popis.rs`'s own izveštaj warning all said the izveštaj *„sastavlja se i štampa na zahtev“* / *„odštampajte ga“*. **There is no print and no export anywhere in the popis module** — `PopisService` has fifteen methods and none exports, and no popis screen calls `PrintService.openForPrint`. All three now say the izveštaj is assembled on demand and **shown on screen**, that the program neither prints nor exports it, and that the printed and signed copy is the obveznik's own to make and keep. The existing retention test pinned only the neighbouring sentence, which is why nothing caught it: it now pins the *„ne štampa“* half **and** asserts the withdrawn clause cannot come back |
| **The čl. 8 st. 5 signature step cited čl. 9 st. 3.** `PopisModule` printed *„…ne zamenjuje potpis na papiru (PoP čl. 9 st. 3)“* under **every** `korak.potpis`, including `counting`, whose own `pravniOsnov` is čl. 8 st. 5 — two different articles for one act, and čl. 9 st. 3's *„uz štampanje“* governs the liste printed **after** the natural count, not the čl. 8 st. 5 potpis. The article is now `korak.pravniOsnov`, never a literal, and the test asserts both phases and that neither shows the other's article. The same sentence now also says the program does not print the liste |
| **The čl. 12 st. 2 iznos was write-only during the count.** The blind read carved `cena_minor` out for `gotovina` only, on the ground that elsewhere it is an obračun figure — which does not hold for nedokumentovana potraživanja i obaveze, whose iznos is the commission's **own** figure, has no perpetual record behind it, and is the only substantive figure that lista carries. The till offered the field, the row came back „—“, the edit form reopened empty, and the Phase A `snapshot_hash` — taken from that same blind read — recorded the amount as absent under a potpis the commission had given over it. **Chosen fix: extend the carve-out**, on the identical čl. 11 st. 1 reasoning; withholding the input instead would have left the čl. 12 st. 2 lista with no amount on the document the commission signs, which is worse evidence, not better. The freeze follows the read — what the blind read carries, the potpis freezes — so a moved iznos is refused in the obračun by name (`popis_prebrojani_iznos_potpisan`; the shipped `popis_apoen_potpisan` code is unchanged), the obračun's „what you may still fill in“ string no longer offers a cena that lista does not have, and the count sheet calls the column **Iznos** with its own article and disables it in the obračun. The four liste where the cena really is the čl. 9 st. 1 t. 5 figure are untouched, asserted as the negative control |

> **Superseded 07.08.2026 — two sentences in the table above have gone stale and are corrected here
> rather than rewritten in place, because the batch is a dated record of what was true then.** The
> first row's *„There is no print and no export anywhere in the popis module“* and the second row's
> *„The same sentence now also says the program does not print the liste“* were both true at
> **cargo 901 / bun 505**. `popis_export_lista`, `popis_export_odluka` and `popis_export_plan_rada`
> write three documents into `exports/`, `PopisService` exports through four verbs, and `PopisModule`
> calls `PrintService.openForPrint`. The **izveštaj** is still neither printed nor exported — nothing
> in the crate writes one to a file — and that is the whole of what the three „program ga ne štampa i
> ne izvozi“ sentences ever claimed, so all three stand, each now paired with a sentence naming what
> the program *does* produce. Task 6 of the popis-štampa plan restates reqs. 31/32/35 below.

**Still open after this batch.**

- **Reqs. 31/32 and 35 were the two largest holes in this list and were closed on 07.08.2026.** They
  are moved out of it and into the section below, which records the work that answers them. What they
  said stands as the reason that work was done, and is kept here rather than deleted: čl. 9 st. 3's
  *„uz štampanje“* is express, SW-8 had shipped a printing stack, and no task in the SW-16 plan wired
  the two together, so until it landed the shop printed the liste from somewhere else; and
  `plan_rada_json` / `odluka_ref` were free text captured once at `open_popis` and echoed back, with
  nothing generating either document and **no approval recorded at all**, although PoP čl. 8 st. 2
  requires the plan to be *approved* by the lice iz čl. 4 st. 2 — for a preduzetnik the owner
  personally — which the v20 comment at `db/migrations.rs:1169` had already named as that column's
  whole purpose. **Three limbs did not close with them and travel to the list below:** req. 32's
  **editable default template**; the **izveštaj**, which is inside this hole, was not disclosed as
  such until 03.08.2026, and is still composed on demand and shown on screen; and `odluka_doneta_at`,
  which v21 added and which no verb writes.
- **Req. 40's second limb has no code.** The first limb is present and correct — popisivači are named
  persons, `popis_commission.rukuje_imovinom` carries the čl. 5 st. 1 flag, and the warning never
  blocks. *„Reuse the flag for ZoRač čl. 10 st. 5 (control of računovodstvene isprave)“* has **zero
  occurrences in the crate**: `rukuje_imovinom` is read in exactly one place, `komisija_upozorenja`,
  and no code path anywhere consults it when računovodstvene isprave are controlled.
- **`perpetual_odluka_ref` validates itself and is then never consumed.** `ensure_perpetual_shortcut`
  refuses a reference with no posted in-year popis behind it (req. 34), and after that the column is
  only read back into the view. **No code path shortens or skips the count**, so čl. 9 st. 2's *„the
  exception excuses step 2) only“* is not modelled at all — the natural count of t. 1 is what the
  module always requires. The direction is the conservative one and nothing is unsound, but the field
  today buys a gate and no behaviour.
- **Čl. 9 st. 2's *usvojen* limb is still unchecked.** Req. 34's gate verifies that an in-year popis was
  *izvršen i proknjižen*; the čl. 14 st. 2 odluka o usvajanju has no stored fact to check, so the
  module states that it does not record the decision instead of implying that it does.
- **Presence of a category is a declaration, not a fact the books hold.** `ensure_liste_kompletne`
  refuses a *declared* lista that is empty; it cannot know about a damaged carton nobody declared.
  Closing that would need a schema for the fact, which v20 does not have.
- **The čl. 2 st. 6 consignment copy is a reminder, not a delivery.** The app computes the ten-day rok
  and says in the same sentence that it does not send the lista to the owner.
- **R-5, R-6, R-7 are unresolved law**, not engineering. R-5 (goods-handler exclusion *shodno* to a
  one-person popis) is handled as warn-never-block; R-6 (purely electronic potpis under čl. 9 st. 3) is
  handled by defaulting to print-and-sign and saying the recorded potpis does not replace the paper
  one; R-7 (no provision names popisne liste expressly) is handled by carrying the five years as the
  inference they are, in the napomena the shop reads. Full text in
  `docs/REMAINING-SW-VERIFIED-RULES.md` §6.
- **The komisija class holds employee data when the shop appoints an employee to it**, and
  `docs/compliance/obavestenje-zaposlenima.md` has no row about it. The čl. 23 notice belongs to SW-13;
  `docs_guard` records what such a row would have to be called („popisne liste“) rather than inventing
  one here.

---

### Popis — the printed liste, the odluka and the plan rada (reqs. 31/32/35) (2026-08-07)

The two largest holes SW-16 disclosed closed as a six-task TDD plan
(`docs/superpowers/plans/2026-08-03-popis-stampa-i-plan-rada.md`, design at
`docs/superpowers/specs/2026-08-01-sw16-popis-design.md` §2 and §5) against the verified rule set in
`docs/REMAINING-SW-VERIFIED-RULES.md` §2c, §3 V5 and §4 reqs. 30, 31, 32, 35, 36 and 41. The plan's
own baseline was cargo **902** / bun **505** at migration **v20**. The cycle opened for real at
`a85a194`, the merge that brought master's reklamacija no-fee migration in and renumbered the popis
plan-rada migration to **v21** beneath it, and every count below is measured from that merge. **No
migration was added after it** — v21 already carried the three columns the plan needed — so the head
stays **v22**.

**The load-bearing property, stated first, because the whole cycle is arranged around it.** PoP čl. 8
st. 5 forbids releasing book data to the popisna komisija before the counted state is written into the
liste and signed, and handing somebody a printed sheet **is** releasing it. So the čl. 8 st. 5 sheet
withholds **structurally**: `red_faza_a` is a code path that never names
`knjigovodstvena_kolicina_milli` or `razlika_milli` at all — asserted against its own source, because
selecting them and blanking them would look identical from outside and be the wrong thing — and the
phase is decided by `faza_stampe` off `book_quantities_released(status, faza_a_potpisana)`, the two
stored facts, and deliberately never off the view's own `knjigovodstvo_dostupno`, which is that same
predicate somebody else has already evaluated. The screen may narrow a request to Faza A and can never
ask for Faza B; the backend refuses the obračun sheet before the potpis **by name** and refuses it
**before the render**, so a refused export leaves nothing on disk — asserted on the path
`write_export` itself resolves, not on the return value.

| Task | Shipped | Commits |
|---|---|---|
| 1 — the čl. 8 st. 2 approval | **Migration v21** (`popis_plan_rada_approval_and_odluka_date`): `plan_rada_odobrio`, `plan_rada_odobreno_at` and `odluka_doneta_at` on `popis_sessions`, all nullable, with a CHECK **pairing** the two approval columns so an approval can never be half-recorded, and a non-empty CHECK on each beyond what the plan asked — a blank approver names nobody and a blank stamp dates nothing, which is the half-state wearing a value. No trigger work: v20's `trg_popis_sessions_zakljucan` fires on the whole row (`WHEN OLD.status = 'posted'`), so the req. 41 posting lock already reached the new columns, and that is **asserted by test rather than assumed**. Nothing is backfilled — an upgraded popis reads back unapproved. A v20-seeded survival test applies `MIGRATIONS[..20]` to a raw connection, seeds, drops it and reopens through `Db::new` | `cb43d06`, `a85a194` |
| 2 — the renderer | `popis_print.rs`: `render_popisna_lista(company, view, faza)` over `PopisSessionView`, the req. 32 header block, one section per lista naming its own article, and a signature line per member. The red state was two real defects — `sekcija` sent **both** phases to `red_faza_b`, and `zaglavlje_kolona` matched on the constant `PrintFaza::B` instead of on its parameter. Deviations: the unclassified bucket keeps the Faza A columns and the čl. 9 st. 3 sheet says so in the engine's own words (a signed stavka does not move, so the remedy is a new popis — the first wording promised an edit `save_line` refuses); the čl. 6 st. 1 single person is never called a komisija, `potpisni_naslov` derives the heading from the roster, and a sweep fails any prose block naming the komisija without the čl. 6 limb. **Column order is pinned by re-parsing the heading row and the first body row back out of the emitted markup** — every other assertion in the module is a `contains`, a `!contains` or a count, and all three survive a permutation of the cells that prints a manjak as a višak | `1bd9575` |
| 3 — the export | `popis_export_lista(state, id, faza: Option<PrintFaza>)` in `kep_export_book`'s shape — `require_admin`, load, render, `campaigns::write_export` — writing `popisne-liste-{id}-faza-{a|b}.html`. **One command, not two**, because two would hand the caller the phase back by letting it pick which to call. An explicit Faza A is honoured in every state, including after the potpis: PoP čl. 2 st. 6 gives the owner of tuđa roba ten days to receive a primerak of the **signed** posebna popisna lista, so that reprint has to survive the obračun. The review made the export tests hermetic — `with_app` now puts each database in a folder of its own, so `exports/` is private per test and five tests stopped writing, reading back and deleting the same two absolute paths — and pinned the obveznik, which every bare test database had been letting `CompanySettings::default()` satisfy | `ec1bde8`, `f89814a` |
| 4 — odluka, plan rada, approval | `render_odluka` / `render_plan_rada` on the header block all four documents now share, plus `popis_export_odluka`, `popis_export_plan_rada` and `popis_odobri_plan`. **Nothing auto-approves:** a blank approver is refused by name rather than substituted from the registered obveznik, because a server-side default is an auto-approval wearing a default's clothes. The approval writes one čl. 48 audit line under a new `AuditObjectType::PopisSession` — `unos` the first time and `menjanje` on a re-record, since two `unos` lines would report two approvals where there was one and a correction — and no personal name reaches it. The čl. 47 register gained the new personal datum. The review qualified an approval standing over a schedule the application does not hold, and stopped an **empty or mixed** roster being declared a komisija: `Sastav::Neodredjen` is now a third state, because the odluka *is* the appointment and printing it before anybody is appointed is its primary use | `8a932bc`, `11673e6` |
| 5 — the screen | `DokumentiPanel` in `PopisModule.tsx` — two `role="group"` blocks with four print actions over the export-then-`openForPrint` stack `KepModule` and `ReklamacijeModule` already use, plus the pre-filled, editable čl. 8 st. 2 approval field. `CountSheet.tsx` was **not** touched and could not be: its own req. 29 guard sweeps the component for `/knjigovodstven/i`, and the plan's required copy for the counting step is *„bez knjigovodstvenih količina“* — loosening that guard to fit copy is the one thing the house rules forbid, so the block sits in `PopisDetail` instead. The module is asserted **never to send `„b“` in any state**. The three „program ga ne štampa i ne izvozi“ sentences were **not** flipped — all three scope to the izveštaj and all three are still true — but each was incomplete beside a module that had gained three exports, so each keeps its denial verbatim and gains a sentence naming what the program does write. The `cl47.rs` output sweep was amended deliberately and then corrected in review: the escape is scoped to **`izvozi` alone**, bound to the three commands by typed function pointer, with `štampa`, `šalje` and `podnosi` keeping the original negate-it-or-do-not-say-it rule | `0108ab5`, `784c88b` |
| 6 — docs + gates | This section; register row 19 and the §3 SW-16 row **re-stated** rather than annotated again, with the dated disclosure left here where it belongs; two guards in `docs_guard.rs` (below) | this commit |

**Verification gates — all six run from the repo root, every command exited `0`:**

| Gate | Result | Exit |
|---|---|---|
| `bun run test` | **536 passed** / 0 failed, 35 files (was 512 / 34) | `0` |
| `bun run build` | tsc + vite, dist written; only the pre-existing chunk-size advisory | `0` |
| `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1` | **978 passed**; 0 failed, 0 ignored, 0 measured, 0 filtered out (was 977 at the Task 5 review pass) | `0` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings` | clean, no warnings | `0` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | clean, no output | `0` |
| `git diff --check` | clean, no output | `0` |

Net **+62 cargo / +24 bun** over `a85a194`, plus the **2** migration tests Task 1 shipped before that
merge — **64** cargo tests for the cycle. The cargo figure is the crate's `#[test]` count at `a85a194`
(**916**) against this commit's (**978**), and the run corroborates it: 978 attributes, 978 tests run.
Latest migration: **v22**, unchanged by this cycle.

**Whose tests those are.** Cargo: **32** in `popis_print.rs`, **25** in `commands/popis.rs`, **3** in
`cl47.rs`, **2** in `docs_guard.rs` (one in Task 5's review pass, one in this commit) and **2** in
`db/migrations.rs` (Task 1, before the merge). Bun: **18** in `src/app/popis/PopisModule.test.tsx` and
**6** in the new `src/services/mock-adapter.popis.test.ts`.

**Two guards over the register**, because each of the six false promises this project has shipped was
found by a person reading the sentence and none of them by a test. `docs_guard.rs` gained
`nothing_exports_the_izvestaj_and_both_popis_cells_say_so` — both popis cells must keep the sentence
that the izveštaj is neither printed nor exported, **and** `lib.rs`'s invoke handler must register no
command whose name says it writes one out, so the denial cannot quietly go false the way a promise
does. And `the_register_corrects_every_denial_of_the_popis_export_it_now_has` became
`the_register_states_the_popis_export_it_has_instead_of_the_denial_it_replaced`: its rule was *„correct
the denial in the same cell“*, which was right while the work was in flight, and its own comment named
the successor — the withdrawn sentence must now be **gone** from the register, and each popis cell must
name the command that replaced it. That is a tightening, not a relaxation: a cell reading as a denial
followed by two supersessions is not a statement of what the software does.

**Still open after this cycle** — the first five are this cycle's own residuals; the last carries
SW-16's forward unchanged, because nothing here touched them.

- **Req. 32's *editable default template* did not ship.** Req. 32 asks for the derived column set as an
  editable default template and design §2 opens with the same words. What shipped is a fixed layout:
  `zaglavlje_kolona` and the two row renderers emit a hardcoded column set with no template, no setting
  and no operator control, and no task in this cycle added one. Everything else req. 32 asks for — the
  header, the čl. 8 st. 4 / čl. 9 st. 1 t. 1–6 column set, the signature block, the no-obrazac claim —
  is built and tested, and **nothing operator-facing overstates it**: the footer says the layout is
  ours and that no obrazac exists, which is true either way. A product gap, not a false claim.
- **The izveštaj o popisu is still screen-only.** `popis_izvestaj` composes it on demand and returns a
  view; no command in the crate writes one to a file, and the printed, signed primerak is the
  obveznik's own to make and keep. That is what the retention napomena, the čl. 47 `popis_imovine`
  entry and the izveštaj's own upozorenje have said since 03.08.2026, and each now also names the three
  documents the program does write. The new guard above is what keeps the denial honest.
- **`odluka_doneta_at` is written by nothing.** v21 added the column and no verb sets it, deliberately:
  exporting a draft odluka is not *donošenje odluke*, and a command that stamped the column while
  rendering would record a čl. 4 st. 2 act nobody performed. The generated odluka prints *„nije
  evidentiran u aplikaciji — upišite ga na odštampanom primerku“* in its own header cell and prints the
  stored value the moment some later verb records one. So req. 35's odluka has a date field and no way
  to fill it in, which is the honest state and not a closed item.
- **The čl. 48 audit trail is lopsided, and this cycle is what made it visible.** The plan approval
  writes an `audit_events` row; opening a popis, recording the komisija and taking either potpis still
  write none, though each of them stores names too. Pre-existing, out of this cycle's scope, and it
  should be decided deliberately rather than left to drift.
- **The čl. 2 st. 6 delivery is still the shop's — and only the delivery.** This bullet first read
  *„the module now prints the signed lista and sends it to nobody“*, which over-credited on both
  limbs: what the module prints carries blank ruled lines, and what it printed was the whole popis
  under one trailing signature rather than the *posebna* popisna lista čl. 2 st. 6 names. Both limbs
  were closed by the review pass below — `popis_export_lista` takes a `lista` narrowing, every lista
  section carries its own signature block and its own page, and the screen offers a print action per
  lista that has stavke. What is still the shop's is the delivery: the app writes the paper and sends
  it to nobody, which is what the čl. 47 register's `vrsta_primalaca` already says and what the
  ten-day reminder says in the same breath.
- **Req. 40's second limb, `perpetual_odluka_ref`, the čl. 14 st. 2 *usvojen* limb, the declared
  presence of a lista category, and R-5/R-6/R-7** stand exactly as the SW-16 section above states them.
  Nothing in this cycle touched any of them.

#### Whole-branch adversarial review (07.08.2026) — seven findings, all closed

The review read the finished branch rather than any one task, and every finding it raised was either a
gap the plan never scoped or a sentence that had gone stale while the work moved under it. Six of the
seven are one class: **a statement about the code that the code stopped backing.**

- **The čl. 2 st. 6 primerak could not be produced — one file, six liste, one signature.** Raised
  twice, independently. `render_popisna_lista` emitted `potpisni_blok` once, after the last section,
  and `popis_export_lista` wrote the whole thing to one file. An operator following the module's own
  ten-day reminder had two wrong moves and no right one: hand over the bundle — every other lista of
  the popis, and on the čl. 9 st. 3 sheet their knjigovodstvene količine, cene and vrednosti — or tear
  out a section with no signature on it. Closed on both limbs. `potpisni_blok` is now emitted **once
  per lista**, names the lista it signs, and the sections carry `page-break-before` — which is also
  what čl. 8 st. 5 says in as many words, *„пре него што чланови комисије за попис потпишу те листе“*,
  those liste, each of them. And `popis_export_lista` gained a `lista` narrowing rendered by
  `render_jedna_lista`, writing `popisna-lista-{id}-{lista}-faza-{a|b}.html` with a `row_count` of what
  that document carries; `DokumentiPanel` offers it for every lista that has stavke, with the ten-day
  rok stated beside the button that produces the document it is about. The narrowing **cannot widen**:
  `faza_stampe` still decides the phase, so a narrowed čl. 9 st. 3 request before the potpis is
  refused exactly as the bundle is, and still before the write.
- **Two guard comments still explained themselves with the denial this cycle falsified.**
  `retention.rs` and `commands/popis.rs` both said the popis module ships no print and no export and
  gave the absence of an exporting service method and of an `openForPrint` caller as the evidence —
  each sitting directly above a test body that, since Task 5, asserted the opposite. Both rewritten to
  scope the denial to the **izveštaj alone**, with the historical note kept and dated. And the class is
  now swept: `no_comment_in_the_popis_stack_still_denies_the_export_the_module_has` reads the four
  files that carry the export stack and fails on the withdrawn wordings, bound by function pointer to
  the three commands.
- **The čl. 47 output sweep passed false `izvozi` claims, and the plan record said it could not.** The
  Task 5 escape was „names one of `IZLAZI` and none of `BEZ_IZLAZA`“ — an allow-noun ORed with a
  deny-noun, so any object outside both rode through. Measured: *„Program izvozi popisne liste na
  Nacionalni portal otvorenih podataka“*, *„…i evidenciju o povredama podataka o ličnosti“*,
  *„Aplikacija izvozi plan rada i knjigu evidencije prometa Poreskoj upravi“* and *„…u obliku
  propisanog obrasca“* all passed, and the pre-amendment guard had failed every one — so along
  `izvozi` the amendment had been strictly **looser**, not stronger. Three bounds now stand together
  and each was measured by dropping it: one of `IZLAZI`, none of `BEZ_IZLAZA` **or** the new
  `TUDJI_OBJEKTI`, and the verb immediately in front of its only truthful destination, `izvozi u
  datoteku`. Nine untrue shapes are probed. The residual is stated in the code: along `izvozi` this is
  necessarily looser than negate-or-stay-silent, because the register now has something true to say.
- **The obveznik lookup was unpinned on two of the three exports.** Task 3 closed exactly this hole for
  `popis_export_lista` and Task 4 then added two commands with the identical lookup and no guard. An
  odluka o popisu identifying its issuer as *„VantumPOS“* with no PIB and no matični broj — a čl. 4
  st. 2 decision naming nobody as the person who made it — was a state no test in the crate could see.
  The guard now loops all three; measured by stubbing both new lookups to `CompanySettings::default()`,
  which failed exactly one test where before it failed none.
- **The čl. 8 st. 5 button copy denied a column the sheet carries.** *„bez knjigovodstvenih količina,
  bez razlika i bez vrednosti“* — the first two clauses are true and structurally enforced, the third
  was not: `iznos_prebrojan` puts the čl. 11 st. 1 apoen and the čl. 12 st. 2 iznos on the Faza A
  sheet, and the crate proved it in one test while the screen denied it two files away. The clause is
  now *„bez vrednosnog obračuna“* and the exception is stated. Pinned from the side that knows the
  column set: `the_screen_never_denies_a_money_column_the_cl_8_st_5_sheet_prints` reads
  `PopisModule.tsx` and derives its needles from `iznos_prebrojan`, `iznos_kolona` and
  `PopisLista::pravni_osnov`, so a lista added to the counted-money set fails until the copy names it.
- **Nothing tested that the odluka and the plan rada withhold book data.** Both are printable while the
  popis is `counting` and both are handed the whole view, `linije` included. Neither renderer walks
  `linije` today, so nothing leaked — but that was a fact about this build and the only thing between
  the komisija and the book side was that `load_session` happened to return a blind view, which is
  precisely the „property of somebody else's SELECT“ the module rejects for the popisna lista. Two
  guards now: a behavioural sweep over a deliberately leaking view, and a source-level assertion that
  neither renderer names `linije` at all. Measured in both directions — a leak planted in a renderer
  body trips both, one planted in a shared helper trips only the behavioural one.

**Gates after the review pass, every command exited `0`:** cargo **988 passed**, 0 failed (was 978);
bun **539 passed** / 35 files, 0 failed (was 536); `bun run build` ✓; clippy clean; `cargo fmt
--check` clean; `git diff --check` clean. Net **+10 cargo / +3 bun** — seven cargo tests in
`popis_print.rs`, three in `commands/popis.rs`, two bun tests in `PopisModule.test.tsx` and one in
`mock-adapter.popis.test.ts`. No migration; head stays **v22**. No existing test was weakened:
`the_exported_sheet_carries_the_obveznik_the_shop_registered` became
`the_exported_documents_carry_…` and loops three commands instead of one; the frontend čl. 2 st. 6
reminder test now reads its three facts out of the warning element rather than off the whole page,
because the rok is cited beside the new per-lista button too; and the `„b“` sweep widened from two
button names to every print button on the panel.

### Test isolation — `--test-threads=1` is retired (07.08.2026)

Not a compliance item. The suite had run serially since the earliest module spec, and the flag was
copied forward into every plan since without its reason ever being written down. It turned out to have
one, and the reason was a defect rather than a constraint.

`db::test_database_path` returned a database file directly in `std::env::temp_dir()`.
`campaigns::write_export` resolves `exports/` **beside the database**, and an export file name is
typically a pure function of a row id that every fresh test database restarts at 1 — so every test in
the process shared one `exports/` and computed identical absolute paths. Under a plain `cargo test`
the export tests wrote, read back and deleted each other's files. The popis cycle above hit this and
fixed it locally in `f89814a` (`commands::popis`'s `with_app` took a folder of its own); this is that
fix made general.

| | |
|---|---|
| `test_database_path` | now returns `$TMPDIR/vantumpos-{test}-{nanos}-{seq}/test.sqlite3` — a directory per test, created eagerly. Uniqueness is the clock **and** an `AtomicU64`: two tests entering in the same nanosecond stops being hypothetical at 16 threads, and a collision would silently restore the sharing. The directory is created in the helper rather than left to `Db::new`'s `ensure_parent_directory`, because the migration tests build an installed-base database through a raw `rusqlite::Connection::open`, which creates no directories — a gap that could not show while the parent was always `$TMPDIR` |
| `remove_test_database(path)` | the matching teardown, takes the **database path** and removes its parent whole. 91 hand-written `remove_file` teardowns across 38 files now call it. Best-effort by design — a teardown is not a place to fail a passing test. It also closes a pre-existing leak: nothing in the crate ever removed the `-wal`/`-shm` sidecars |

**Measured, before and after.** Before: 1 failure in 6 parallel full-suite runs —
`commands::audit::tests::the_izvod_is_recorded_as_a_disclosure_to_the_poverenik`, whose izvod a sibling
removed between the write and the read. After: **8 parallel full-suite runs green (988 passed each)**,
plus 25 consecutive parallel runs of `commands::audit` and 20 of `commands::popis`. **187 s serial →
83 s parallel**, on the same machine, same commit.

The gate list in `docs/module-specs/00-shared-foundation.md` drops the flag and now states the rule and
the measurement, so the next person to hit a parallel-only failure reads it as a shared-path bug in
that test rather than as a reason to put the flag back. Two pre-existing patterns stay outside the
scheme and are recorded there: `cenovnik`'s `with_publish_folder` (no database involved) and
`commands/backup.rs`'s `test_backup_dir`, whose folder names are fixed rather than unique and which
leaves ~20 directories in `$TMPDIR` per run. Neither collides today.

### SW-14 req. 12's weekly leg, and three false claims withdrawn (2026-08-07)

One protection built and three statements withdrawn, as a four-task TDD plan
(`docs/superpowers/plans/2026-08-07-cl87-nedeljni-limit-i-lazne-tvrdnje.md`) against
`docs/SW14-VERIFIED-RULES.md` §4 req. 12 and the §3 W4b row, and `docs/SW11-SW15-VERIFIED-RULES.md`
§3 req. 39 / §4 item 8 / §2 Q4. Baseline `c93e28e` — cargo **988**, bun **539** / 35 files, migration
**v22**. **No migration was added; the head stays v22**, because every column the cycle needed already
existed.

**The cycle's own subject, stated first.** Three of the four tasks are about a defect this project has
now shipped in both directions: a document, a generated artefact or an operator string that **denies**
behaviour the code has is the same defect as one that promises behaviour the code lacks, and the denial
is the more dangerous half. It withdraws the reader's only pointer to a control the shop is actually
running, and on 07.08.2026 it did exactly that — a survey agent reading this file and the register
ranked two long-fixed defects as pilot blockers. The čl. 87 weekly leg in Task 1 is the one thing here
that is new behaviour; Tasks 2–4 are proof, withdrawal and re-statement.

| Task | Shipped | Commits |
|---|---|---|
| 1 — the čl. 87 weekly leg | `worktime::MINOR_WEEKLY_CAP_MINUTES` (35 × 60) beside `MINOR_DAILY_CAP_MINUTES`, `ProtectionKind::MaloletanNedeljniLimit` beside `MaloletanDnevniLimit`, and the leg itself inside `check_protection`'s existing `is_younger_than(…)` branch — **blocking, never overridable**, because čl. 87 states the prohibition itself while every leg `assess_caps` reports is a čl. 53 cap the operator walks through with a recorded ground. Signature widened to `check_protection(p, day, entry, week)`; the sole production caller is `commands/worktime.rs`, where `load_week` was hoisted so both gates read one week inside one transaction, and 26 test call sites pass `&[]`. **Three filters, each narrower than `assess_caps`'s and each for a reason this leg has and that one does not:** the stored row for `day` itself (a correction that *lowers* the day is not a breach), rows whose `dan` is not a civil date (via a new private `strictly_in_same_iso_week` — v17's GLOB CHECK lets `2026-08-32` in, and `in_same_iso_week`'s fail-open rationale is written for a cap that asks for a ground, not for a refusal), and days on which the employee was already 18. **The refusal fires only on the write that *raises* the week** (`unos_minuta > evidentirano_za_dan`): `datum_rodjenja` is nullable and unbackfilled, so the guard switches on over rows already recorded, and refusing every correction would leave the shop with „leave the 40 h standing“ or „record 3 h for a day the employee worked 8“ — hiding the čl. 274 exposure instead of surfacing it. The poruka names **two** figures and confuses neither, since the block records nothing: „već je evidentirano {} č {:02} min … a sa ovim danom bilo bi {} č {:02} min“ | `2f0e76b`, `6cd568e` |
| 2 — it reaches the write path and the screen | **A regression test only, in both halves, and the note says so rather than inventing a change.** `write_entry` already refused on *any* blocking `ProtectionBlock` and that `find(\|block\| block.blocking)` predates the cycle; `WorkTimeModule.tsx` maps every finding off `blocking` and `poruka` with **no switch over `kind`**. What landed is one test per side of the wire — `the_cl_87_weekly_leg_refuses_the_write_and_leaves_no_row_behind` (refused, poruka names „35 časova nedeljno“, „čl. 87“ and both figures, a `cap_override_razlog` buys nothing, and `list_month` still holds **five** rows) and the first test the `code === "protection_block"` branch ever had. **Red was proven by mutation, because the feature was already wired:** flipping `blocking` to `false` saved the sixth day with the finding computed and handed back on the success path, which is precisely the failure mode the task exists to bar. Two defects surfaced and were closed: a čl. 87 refusal **outlived the attempt that raised it** in `WorkTimeModule`'s catch (a minor's refused Saturday still shouting „Unos nije dozvoljen“ over the next day's „Dan nije evidentiran“), and both legs are **blind to `casovi_cekanja_i_zastoja_minuta`**, which the same write books into the ZEOR čl. 24 tač. 1 b) total — the arithmetic was deliberately **not** changed, and what shipped is the disclosure, in `check_protection`'s doc comment and in the poruka itself | `bf87ff2`, `0dd6065` |
| 3 — the two false claims on Podešavanja | The reset dialog's archive sentence is **gone, not hedged** — ZAG čl. 16 st. 2 confines prior written archive approval to the public sector and the pilot is a preduzetnik — and the retention citation moved off **ZPDV čl. 47** onto the one `backup.rs` already had right. Neither is a literal on either surface any more: `commands::backup::ROK_CUVANJA_PRAVNI_OSNOV` holds *„ZoRač čl. 28 st. 4; ZPPPA čl. 114ž“*, the tombstone formats it, and `docs_guard` requires every `;`-separated član of that constant to appear in the dialog's own `<p>`, so correcting one surface and leaving the other fails the crate. Four further defects were closed in review: the floor was **printed as a ceiling** („do 10 godina“ — SW11-SW15 §1 row 7 rates that framing HIGH beside the miscitation), req. 39's **second limb had never shipped** and three artefacts said it had (the neutral ZAG čl. 9 st. 1 custody note is now a paragraph of its own), the archive sweep was **blind to capitalisation, to verb forms and to „saglasnost“** and judged a 267-line window, and the compliance memo plus four `PROGRESS.md` residual blocks still described the withdrawn dialog | `41dc298`, `2c415b8` |
| 4 — the register said „Gap“ for things that shipped | This section, plus **six re-stated rows** in `docs/SERBIAN-LAW-COMPLIANCE.md`, five stamped §3 build rows and the §2 revision note that records the sweep. Each of rows 6, 16, 17, 18, 20 and 24 was re-verified against the named symbol before it was touched, and **exactly one limb anywhere in the sweep was flipped to a tick** — row 6's ≥2× ratio, and only after it was computed from the class names and the receipt-detail banner was raised to meet it. Everything else is a partial and each cell states what within it is still not built. Row 21 was read in the same pass and left alone, which is the other half of the instruction: an over-corrected register is this defect pointed the other way. **Row 16 was left alone in the first pass and re-stated in review** — the commit had already written the correction into the §2 note and left the false cell standing in the column an inspector reads | this commit |

**What the six rows now say, and what each still owes.** Row **6** — the „OVO NIJE FISKALNI RAČUN“
banner renders twice in the post-sale dialog and once on the receipt detail panel, with tests; the
**≥2× ratio is now computed rather than assumed** (`text-2xl` over the `text-xs` the stavke inherit,
2.0× on both surfaces, after the receipt-detail banner was raised from `text-xl` — 1.6× — in review),
and SW-1's *„every export that itemizes a sale“* limb has **no subject in this build**. Row **16** —
`kep_kalkulacija.rs::create_kalkulacija` does write a goods-receipt document inside the goods-receipt
transaction, so *„Gap (no goods-receipt documents)“* understated the code; what the shop does not hold
is the **supplier's** isprava, because `kalkulacije` (v13) carries no adresa, no matični broj/BPG and no
supplier document broj/datum, and the field-by-field čl. 29 read is still owed. Row **17** — the KEP module, thirteen commands and a shell
module of its own; **one book, not one per prodajno mesto**, `kep_entries` being partitioned by
`book_year` alone. Row **18** — the five-column obrazac, the retail-with-PDV zaduženje, the **fourteen**
kalkulacija elements, the exhaustive `posting_for` cause→column map, the T+1 warning and the čl. 18
lock on all seven production insert paths; that lock is a **domain gate each writer calls, not a
storage-layer trigger**, and nothing sweeps the book. Row **20** — the go-live reset forces a snapshot,
fences its transaction with `assert_never_purge_intact` and writes the tombstone; the **restore is
deliberately unfenced**, no backup-prune path exists at all, there is still no general upward-only
`retain_until` engine over trading data, and čl. 28's second location is operator configuration the app
never verifies. Row **24** — the append-only offered-price log with explicit offering gaps, the closed
four-type campaign enum with frozen anchors, and all three evidence/label/correction exports; the **čl.
67 st. 1 tač. 8 penalty copy does not exist in `legal.rs`**, the 5-year retention is discharged by
nothing deleting the rows rather than by a declared `RecordClass`, and the log is keyed on `product_id`
alone — `price_history` (v9) has **no prodajno-mesto column**, though §3's SW-6 line specifies
`(sku, prodajno_mesto)`, so a second outlet would share one offered-price history and one prethodna
cena. That is the single-book limitation row 17 records for the KEP, and it was missed in the first
pass.

**Verification gates — all six run from the repo root, every command exited `0`:**

| Gate | Result | Exit |
|---|---|---|
| `bun run test` | **547 passed** / 0 failed, 35 files (was 539 / 35) | `0` |
| `bun run build` | tsc + vite, dist written; only the pre-existing chunk-size advisory | `0` |
| `cargo test --manifest-path src-tauri/Cargo.toml` | **1016 passed**; 0 failed, 0 ignored, 0 measured, 0 filtered out (was 988) | `0` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings` | clean, no warnings | `0` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | clean, no output | `0` |
| `git diff --check` | clean, no output | `0` |

Net **+28 cargo / +8 bun** over `c93e28e`, in `worktime.rs`, `commands/worktime.rs`, `docs_guard.rs`,
`legal.rs`, `commands/backup.rs`, `WorkTimeModule.test.tsx`, `SettingsScreen.test.tsx`,
`UserDialog.test.tsx` and `ReceiptsScreen.test.tsx`.
The cargo figure is net of **two deletions**: `worktime::tests::the_cl_87_weekly_leg_is_not_checked` and
`docs_guard::no_document_claims_the_cl_87_weekly_leg_is_enforced`, both of which existed to pin the gap
this cycle closed and whose own doc comments instructed their removal at exactly this moment. The
second was **inverted rather than dropped** —
`docs_guard::no_document_says_the_cl_87_weekly_leg_is_still_unbuilt` now fails a document that says the
leg is unbuilt, sweeps ten markers including the register's own idiom *„Gap“*, judges the **clause**
rather than the line, and asserts per document that the leg is still *named*. Latest migration: **v22**,
unchanged.

**Whole-branch review fixes (08.08.2026).** Eight findings, seven distinct defects, every one closed
with a regression test whose red was proven by mutation before the fix landed. Four are worth naming
because of what they were: three tests that could not fail for the reason their own failure message
gives, and a guard defeated by any following word.

1. **`worktime::tests::the_stored_row_for_the_day_under_assessment_is_not_double_counted` passed with
   the de-duplication removed.** Written against a correction that *lowers* the day, it could not: the
   raise-gate added in Task 1's review makes `unos_minuta > evidentirano_za_dan` false by construction
   for every lowering write, so deleting `d.dan != day` pushed the sum over the cap and no block was
   pushed anyway. It had become a behavioural duplicate of
   `a_correction_that_lowers_a_minors_week_is_not_refused`, which pins that gate on purpose and still
   does. Re-pointed at a correction that **raises** a day inside an under-cap week — Mon–Thu 8 h with a
   2 h Friday corrected up to 3 h, exactly 35 h de-duplicated and 37 h without — and verified red by
   deleting the filter. The de-duplication was previously pinned by nothing but a string figure in a
   command test about the correction path.
2. **`strictly_in_same_iso_week`'s conjunct inside `check_protection` could be deleted with the whole
   suite green.** The age filter beside it was `is_younger_than`, which also answers `false` for a `dan`
   it cannot read, so the two were not merely overlapping — they were provably redundant, and
   `an_unreadable_stored_day_does_not_silently_refuse_a_minors_week` passed for the age filter's reason.
   The age conjunct now asks the positive question (`!is_at_least` — a new private helper: „was the
   employee already 18 on this day?“), so an unreadable row is dropped by the week predicate and by
   nothing else. Behaviour is unchanged; **both** conjuncts are now load-bearing and each was verified
   red by deletion, and `the_two_age_predicates_are_not_complements_on_a_day_that_does_not_parse`
   asserts the property the split rests on.
3. **`load_week`'s live-rows-only contract had no test at any layer.** The `MAX(verzija)` subquery was
   shared with `assess_caps`, where a superseded row costs an override prompt; this cycle made it
   load-bearing for a hard refusal and added nothing.
   `commands::worktime::tests::a_superseded_verzija_does_not_feed_the_cl_87_weekly_total` records a 35 h
   week, corrects the Friday **down** to one hour and then saves a two-hour Saturday that lands on
   exactly the cap; counting both Friday versions refuses it permanently, since `correct_entry` has
   nothing to correct. Red proven by dropping the subquery.
4. **A čl. 87 refusal still survived an attempt that never left the screen.** Task 2's review fix drops
   the assessment at the top of `WorkTimeModule`'s catch, and `submitEntry` has a **second** exit above
   it: `toRequest` returns early on a client-side validation failure, setting `saveError` and clearing
   nothing. Clearing the Datum field after a refused Saturday left „Unos nije dozvoljen … 35 časova
   nedeljno“ standing over „Datum mora biti u obliku gggg-MM-dd“ with nothing submitted at all. The drop
   moved above `toRequest`; the catch keeps its own for the one case the move does not cover, a throw
   after the success path has already applied a saved day's findings.

The other four: the over-cap dead end is now recorded rather than emergent (residual below, register
row SW-14, and `an_over_cap_minors_week_admits_no_further_worked_day`); both *„No `retention_class`,
`retain_until`, `legal_hold` or upward-only extension exists anywhere in the schema“* bullets are
re-stated against the v17 schema that carries all four and bound by
`docs_guard::no_document_denies_the_retention_schema_v17_created`, scoped so register row 20's true
*„no general upward-only `retain_until` engine over trading data“* stays statable; and
`no_document_says_this_application_prints_nothing` no longer exempts every following word — the
exemption is a two-word whitelist („receipt-like“, „nalik“) and the sweep is case-insensitive, as is
the `UNBUILT` marker list, whose *„Gap“* was matched case-sensitively.

**Where SW-14 stands after this cycle, requirement by requirement, because a partial recorded as a tick
is the defect above pointed the other way.**

- **Req. 12 — PARTIAL. The čl. 87 weekly leg closed 07.08.2026**, and the requirement did not close
  with it. Both čl. 87 legs are now enforced: 8 h/day since 01.08.2026 and **35 h/week from this
  cycle**, the latter refusing the write rather than asking for a ground. The čl. 88 st. 1
  prekovremeni and preraspodela bans and the čl. 90 / čl. 91 consent guards shipped with SW-14.
  **One limb of the four is still open:** čl. 88 st. 2's night prohibition, which is stated as a
  decision in the night-legs paragraph below and not with req. 14. Recorded as a partial for the
  reason this section opens with, and pinned by
  `docs_guard::no_document_records_sw_14_req_12_as_closed_while_the_night_leg_is_unbuilt`, which
  fails any block of either document that records this requirement as closed while no
  `ProtectionKind` decides a minor's night hours.
- **Req. 27 — CLOSED 02.08.2026, and this file says so rather than repeating a brief that called it
  open.** The čl. 47 evidencija radnji obrade ships as a generated artefact: `cl47.rs` writes
  `processing_activities` at launch and on demand, driving the st. 1 t. 6 rok **per vrsta podataka**
  out of the shared `retention_policies` rows. Nothing in this cycle touched it; it is listed here
  because it was carried into the cycle's brief as an open item and is not one.
- **Reqs. 10, 14, 15 and 16 — wholly open; req. 28 re-stated 09.08.2026.** Req. **10**: the 9-month
  reference period behind an
  explicit `kolektivni_ugovor_postoji` flag, and the čl. 61 mid-period choice — the identifier has zero
  occurrences in the crate. Req. **14**: the čl. 62 st. 2 night threshold (≥ 3 h/day or ⅓ of the week)
  as an advisory flag; `nocni_minuta` is stored and correctly tagged
  *„izračunato radi provere usklađenosti“*, and no threshold is computed over it anywhere. Req. **15**:
  the čl. 64 / 66 / 67 rest-period warnings, with the preraspodela variants — none of the three articles
  is cited in `worktime.rs`. Req. **16**: the praznik calendar and the čl. 108 st. 1 tač. 1 exclusion
  set; `rad_na_praznik_minuta` is an operator-entered advisory bucket and `non_working_days` is the
  `cash_deposit` working-day table, which is a different question. Req. **28**: remote-support masking
  of the absence-reason column with the unmask logged — **no longer open in full.** This list ended
  *„stated as open here since 02.08.2026 and still open“*; the whole feature landed 09.08.2026 as
  `commands/worktime.rs::razlog_odsustva_dostupan`, the v23 `support_sessions.odsustvo_otkriveno_at`
  stamp, `commands::audit::reveal_absence_reason` and the two surfaces — the vlasnik's unmask control on
  `SupportApprovalPanel` and the masked cell's „Odsutan (razlog skriven)“. The sentence
  *„the unmask control, the masked grid cell and the ZEOR čl. 24 tač. 1 buckets are what remain“* is
  withdrawn to its last item: the buckets are what remain, and register row 12 states them. The section
  below carries the cycle.
- **Reqs. 21 and 26 — partials, and recorded as partials.** Req. **21**'s CSV half ships:
  `worktime_export_csv` writes a per-employee month into `exports/` with the columns mapped 1:1 onto the
  ZEOR čl. 24 tač. 1 buckets, offline from the till and never labelled a propisani obrazac. What does
  not ship is the *„plain per-employee monthly sheet“* — the worktime module has no print path at all —
  and XLSX. Req. **26**'s notice half ships: `docs/compliance/obavestenje-zaposlenima.md` was re-issued
  01.08.2026 for the working-time record and again 02.08.2026. What does not ship is the **gate**: the
  module does not refuse to activate for an employee until an acknowledgement dated on or after the
  feature's introduction is recorded, and no column stores one.

**This cycle deliberately did not build the night legs, and that is a decision rather than an
oversight.** čl. 88 st. 2 (the minor's night prohibition and its exceptions) and the čl. 62 st. 2
threshold stay unenforced, and with them the night limbs of čl. 90 and čl. 91 — all four for the same
structural reason, stated in `check_protection`'s own doc comment: night hours are a čl. 62 computation
over clock times, `DayHours` carries `efektivno_minuta` and `prekovremeni_minuta` and no night bucket,
and building the leg means deciding what a night hour is before there is a verified rule that says.
Both documents state the night leg as unbuilt in a clause of its own, which is what lets the widened
`docs_guard` sweep pass on a denial that is **true**.

**Residuals carried out of this cycle.**

- **A minor's week that is already over 35 h admits no further worked day, at any value — recorded
  08.08.2026 as a known dead end, not as a finished rule.** `evidentirano_za_dan` is 0 for a day with no
  stored row, so the raise-gate `unos_minuta > evidentirano_za_dan` collapses to `unos_minuta > 0` and
  the Saturday a minor actually worked is refused at eight hours, at one hour and at one minute alike;
  `correct_entry` is no way round it, because `write_entry`'s `(Some(_), None)` arm returns `not_found`
  when there is no version to supersede. The operator's remaining routes are to omit the day (ZoR čl. 55
  / ZEOR čl. 24 incompleteness, čl. 276) or to first correct **other** days downwards — that is, to
  record fewer hours than were worked on days the write does not touch, which is the outcome
  `check_protection`'s own doc comment names as the reason the raise-gate exists. It is reachable by
  each of the three routes that leave an over-cap week standing when the guard turns on: a
  `datum_rodjenja` filled in late through `commands/users.rs`, a restored backup, and rows written
  before 07.08.2026. **Not changed, and why:** the alternative — gate on whether the week is over the
  cap *without* this day, so that what is refused is the write which *crosses* it — inverts
  `worktime::tests::raising_a_day_in_an_already_over_week_is_still_refused`, which bars the „once over,
  anything goes“ reading on purpose, and it needs a čl. 274 disclosure decided with it. Pinned as it
  stands by `commands::worktime::tests::an_over_cap_minors_week_admits_no_further_worked_day`, and
  stated in register row SW-14. **Escalated 08.08.2026 as `docs/SW14-VERIFIED-RULES.md` §6 W-16**, which
  is where it belongs: the dead end is a symptom, and the question underneath it is whether čl. 87 is a
  prohibition of the čl. 88 st. 1 kind — where refusing the entry is right — or a ceiling of the čl. 53
  kind, where the same module already records the day and demands a ground. §4 req. 12 uses *„reject …
  outright“* for čl. 88 st. 1 and *„cap“* for čl. 87 in one sentence; this module reads both as a
  refusal. Neither article says what the **record** must do once the hours have been worked, and no
  primary text read in any pass answers it. Choosing without an answer is what W-16 exists to prevent.
- **Whether čl. 87's 35 časova counts časovi čekanja, zastoja i prekida u radu and časovi obustave rada
  zbog štrajka is UNRESOLVED, and the leg takes the narrower reading until it is answered.**
  `derive_totals` books all three into the ZEOR čl. 24 tač. 1 b) `ukupno_ostvareni_minuta` while both
  čl. 87 legs and `assess_caps` sum efektivno + prekovremeni, so a minor's week can stand in the
  register at 45 č ostvarenih and reach this guard as 35 č. SW14-VERIFIED-RULES §4 req. 12 and the §3
  W4b row say only *„≤ 35 h/week and ≤ 8 h/day“* and settle nothing, so deciding it here would apply an
  invented construction as a **hard refusal**. It needs the same treatment as a §6 W-item. The
  divergence is disclosed in the poruka and pinned end to end by
  `the_cl_87_weekly_total_leaves_out_the_cekanje_the_same_write_books_as_ostvareni`.
- **An unreadable stored `dan` is dropped silently from a minor's weekly total** and nothing in the app
  reports it. A non-blocking `NeispravanDatumUProfilu` was considered and rejected: that variant's
  poruka tells the operator to fix the **birth date in the profile**, and pointing it at a stored `dan`
  would make an operator string false.
- **The mock adapter implements no čl. 87–91 guard at all.** `createMockServices().worktime.saveEntry`
  returns `protections: []` unconditionally. Pre-existing, and it *under*-states the backend rather than
  over-stating it, so nothing there asserts behaviour the code lacks.
- **Register row 16 was re-stated in review, and the čl. 29 field-by-field read is still owed.**
  `kep_kalkulacija.rs` generates a goods-receipt document, but ZoT čl. 29 st. 1 is a duty to *possess
  the supplier's isprava* and the `kalkulacije` table (v13) carries no adresa, no matični broj/BPG and
  no supplier document broj/datum. The first pass established that *„Gap (no goods-receipt documents)“*
  was false and then left the false cell in the column an inspector reads, parking the correction in a
  preamble note — which is the same defect as the staleness the sweep exists to remove. The cell now
  states both halves; what it does not do is resolve the čl. 29 read.
- **Req. 39's pravno-lice limb stays open**, unchanged from Task 3's review: the real archive duties —
  lista kategorija with the archive's saglasnost, arhivska knjiga, the 30 April prepis — are
  profile-aware copy branching on `pravna_forma`, and both guards were measured to **permit** them.
- **Four `docs/compliance/` templates are still unguarded** — `checklist-onboarding-pilota.md`,
  `pitanje-purs.md`, `runbook-povreda-podataka.md`, `ugovor-o-obradi-nacrt.md`. One thing seen and
  deliberately not changed: `checklist-onboarding-pilota.md:19` reads *„(PDV obveznik: retencioni prag
  10 godina…)“*, which reads as though PDV status gates the floor — SW11-SW15 §1 row 6 rates that a
  HIGH defect, and register row 21 says in terms that PDV status does **not** gate it.
- **The čl. 48 audit trail is still lopsided** and reklamacije reads are still not access-logged. Both
  carry forward from earlier cycles untouched.

**Review fixes shipped (07.08.2026) — seven findings on Task 4, and the task that was about stale
denials had shipped five new ones with nothing behind them.** cargo **1012 passed / 0 failed** (1007 +
5), `docs_guard` alone **25** (was 21), bun **546 passed / 35 files** (was 545). Every guard added below
was proven non-vacuous by mutation and reverted.

1. **Row 6 ticked SW-1 while one of its limbs was measurably unmet, and the tick has been earned rather
   than removed.** The cell said *„the ≥2× ratio is asserted by presence, never measured“* — but the
   ratio is written in the class names, and one surface was **short**: the receipt detail banner was
   `text-xl` (1.25rem) over a `<Table>` root of `text-xs` (0.75rem), which is 1.6×. „Nobody measured it“
   and „it falls short“ are not the same claim. The banner was raised to `text-2xl`, so both surfaces
   are now exactly 2.0× of the line item, and
   `docs_guard::the_non_fiscal_banner_is_at_least_twice_the_line_item_font` computes both ratios from
   the class names in integers (milli-rem, never floating point) **and requires row 6 to state what it
   computes**, so neither the classes nor the sentence can move alone. Red was real, not mutated: the
   guard failed on `ReceiptsScreen.tsx` before the class changed. `ReceiptsScreen.test.tsx` pins the
   class on the rendered element as well, and was verified red against `text-xl`. The cell's bolded lead
   also contradicted its own body — it named one open limb four sentences above *„Two limbs are not
   closed“* — and now names what is really open: the export limb, which has no subject in this build.
2. **Req. 12 was recorded as a tick in the paragraph that says a partial recorded as a tick is the
   defect.** Its own bullet said *„One limb of req. 12 is still open“* four lines below the header
   *„Req. 12 — CLOSED 07.08.2026.“*, and `:283` of this file and register row 120 already said it
   correctly. SW14-VERIFIED-RULES §4 states req. 12 as four limbs and the third — *„block night hours
   except the čl. 88 st. 2 exceptions“* — is unbuilt. Restated as a **PARTIAL** in the voice reqs. 21
   and 26 use, with the cross-reference pointed at the night-legs paragraph rather than at req. 14's
   bullet, which names only the čl. 62 st. 2 threshold. Pinned by
   `docs_guard::no_document_records_sw_14_req_12_as_closed_while_the_night_leg_is_unbuilt`, bound
   exhaustively to `worktime::ProtectionKind` so the day a night variant lands the file stops compiling
   and the author decides whether the requirement may finally be recorded closed. It is scoped to blocks
   arguing about čl. 87 or čl. 88, because SW-12's own requirement 12 is the cenovnik duty and its
   register row opens „✅ SHIPPED“ — firing on that would be the guard accusing a true sentence.
3. **§3 of the swept file still carried its 16.07.2026 baseline, and SW-8 denied the printing stack.**
   The sweep stopped at §2. `SW-8` read *„app currently prints nothing“* in the same commit as a row 18
   crediting the KEP mechanics to the SW-8 printing stack — five renderers write documents and seven
   modules hand them to `PrintService.openForPrint`. `SW-9` still specified *„all 13 PEP čl. 15 st. 5
   tač. 1 elements“*, the miscount row 18 corrects and `kep_kalkulacija.rs` contradicts in its first
   line. All five unstamped rows that §2 had just re-stated — SW-1, SW-3, SW-6, SW-8, SW-9 — now carry
   dated stamps pointing at the §2 row that holds the detail, and the §2 revision note records that §3
   was swept with it. `docs_guard::no_document_says_this_application_prints_nothing` fails that class of
   sentence, bound to the four renderers so a rename is a compile error. It is deliberately narrow:
   §1's *„prints nothing **receipt-like**“* is true and load-bearing, and a **module** with no print
   path is a useful thing to write, so only the unqualified claim is barred.
4. **Row 16's denial was left standing in the cell after the same commit proved it false.** Recorded
   above with the residual.
5. **Row 24 called `price_history.rs` „the log this row asks for“ without naming the missing key.**
   Recorded above; the row now states three open limbs rather than two.
6. **Five new denials shipped into the register with no guard behind any of them**, which is what
   `docs_guard`'s own module doc bars — two of them decidable in one line each against lists the crate
   exposes exhaustively. `docs_guard::no_register_row_denies_a_retention_class_the_crate_now_declares`
   clears rows 18 and 24 against `retention::RecordClass::ALL` two ways: an exhaustive `match`, so a new
   variant costs a compile error and a decision, and a sweep over `key()`, so a variant answered
   carelessly is still caught. Proven red by renaming `CenovnikArchive`'s key to `price_history_archive`.
   `legal::tests::no_register_row_denies_a_snizenje_notice_this_module_carries` does the same for row
   24's *„the penalty copy does not exist“* against `all_notices` — it lives in `legal.rs` because that
   list is private to its test module by design, and reads the register through
   `docs_guard::REGISTER`, now `pub(crate)`, so there is one embedded copy and not two. Proven red by
   re-citing `declaration_defective` at čl. 67 st. 1 tač. 8. Both lists demonstrably grow:
   `PopisDokumentacija` joined `RecordClass::ALL` and `popis_not_conducted` joined `all_notices` within
   the last three weeks.
7. **What the SW-8 row used to say is recorded here rather than in the register.** The 16.07.2026 cell
   read *„Printing/PDF stack (hard prerequisite for KEP print-on-demand, popis lists, potvrda o
   prijemu; app currently prints nothing)“* and stood for three weeks after the stack shipped. It is
   quoted in this file and not in `docs/SERBIAN-LAW-COMPLIANCE.md`, on that document's own convention —
   *„the denial is restated and not annotated“* — and because the new guard would (correctly) fail the
   register for carrying the sentence verbatim.

---

### SW-14 req. 28 — the razlog odsustva is withheld from daljinska podrška (2026-08-09)

A five-task TDD plan (`docs/superpowers/plans/2026-08-08-sw14-req28-maskiranje-razloga-odsustva.md`)
against `docs/SW14-VERIFIED-RULES.md` §4 req. 28 — *„Remote support must not see the absence reason by
default: mask the column and payroll screens unless the shop explicitly unmasks for that session, and
log the unmask“* — with §4 req. 24 for the čl. 12 st. 1 tač. 3 / čl. 17 st. 2 tač. 2 bases and §6 W-15
for the question it sharpens and does not close. Baseline `e7c79a3` — cargo **1016**, bun **547** / 35
files, migration **v22**. **Migration head is now v23, and v23 is the only migration this cycle adds.**

**What changed, in one sentence, because the architecture is the point.** Before this cycle
`kategorija_odsustva` left the backend on every read and the **screen** decided —
`WorkTimeModule.tsx::canSeeAbsenceReason` is a role boolean drawn over data the wire had already
carried, which is masking by CSS. After it the **backend** decides: while a ZZPL čl. 46 nalog za
daljinsku podršku is live, the register read and the export run a statement that never names the
column, so the value is absent because nothing read it rather than because something read it and
dropped it — the shape `commands::popis::read_lines` uses for the čl. 8 st. 5 withholding, and its
reasoning is quoted into the new function's doc comment. The role gate stays, as a second layer over a
backend that has already decided.

| Task | Shipped | Commits |
|---|---|---|
| 1 — the unmask is a stamp on the nalog, not a flag | Migration **v23** `support_session_odsustvo_unmask_stamp`: one nullable `odsustvo_otkriveno_at TEXT` on `support_sessions`, non-empty CHECK, never backfilled. **There is no re-mask verb and no boolean**, and the migration's own comment carries the reason — an operator who has read the column does not unread it, so a flag that could go back to `false` would let the surface above it claim a disclosure was undone. No expiry of its own: v18's `expires_at` already bounds the nalog at 24 h. The v22-seeded survival test applies `MIGRATIONS[..22]` to a raw `Connection`, seeds a nalog **and** an absence row, drops the connection and only then calls `Db::new` — a test that seeds after `Db::new` proves nothing about an upgrade | `520fe6b` |
| 2 — the unmask verb, and a čl. 48 line that carries no secret | `commands::audit::reveal_absence_reason(state, now)`, `require_admin` on its first line **inside** the domain function, one transaction carrying both the stamp and `append_audit_event` so neither can exist without the other. Refuses with no live nalog (`support_bez_naloga_za_otkrivanje`), idempotent on a second call so the log holds one disclosure per nalog. The line names the **field class** and not the value: a new closed-vocabulary `AuditObjectType::SupportAbsenceReason`, object id the session id, and a test asserts every one of the ten v17 `KATEGORIJE_ODSUSTVA` strings is absent from the written row. Review found three further defects and closed them: the line was indistinguishable from the čl. 46 entry line, `reject_forbidden_content` never looked at `AuditDraft::at` at all (so a category appended to the timestamp reached the hash chain), and an unmask taken *before* the operator enters logged an `otkrivanje` to a primalac that received nothing — it is now an `unos` on that branch, following `grant_access`'s own rule | `fe4c3b4`, `ecf2d48` |
| 3 — the read paths, and the decision behind them | `razlog_odsustva_dostupan` is the whole question in one function: no live nalog → nothing is masked, a live nalog → masked unless v23's stamp is set on **that** nalog, read off the nalog and never by reading `audit_events` back. `load_entries` became a two-line dispatcher over two whole statements, `load_entries_with_reason` and `load_entries_without_reason`, the second of which does not name the column anywhere — pinned by a source-level guard, because a `SELECT` that fetches and drops is invisible from outside. `now: &str` was threaded through `list_month` and `export_month_csv`, the mask being a clock decision. `close_period` reads through the withholding statement (the frozen Class A classification has never carried the category); `my_hours` deliberately does not mask. `7bc450b` pins `KATEGORIJE_ODSUSTVA` and v17's own CHECK to the same ten values, which is what lets the audit test's ten-category sweep stay honest | `7bc450b`, `7db90f7`, `ed15afd` |
| 4 — the surfaces, the čl. 23 notice and the čl. 47 measure | `AbsenceCell` reads `razlogOdsustvaSkriven` **before** the null category, so an em dash can no longer stand on a row that books absence minutes; the why is a banner above the register naming the nalog and pointing at Privatnost → Daljinska podrška. `SupportApprovalPanel` carries the vlasnik's unmask inside the live-nalog branch, admin-gated on a `currentUser` threaded `AppShell` → `PrivacyModule` → panel, stating the two facts req. 28's third limb turns on — the disclosure is for that nalog only and **ne može da se povuče**, and it is written into the evidencija pristupa. Once the stamp is set the button is replaced by the instant; **there is no re-mask control anywhere**, held by a button sweep and by a port-surface assertion in two adapters. The čl. 23 notice gained one paragraph and the čl. 47 register one `mere` sentence, both scoped to what the code does. Review closed three false statements in the same family: the masked cell rendered an em dash on a full shift of štrajk (which books outside v)), and the panel copy plus the čl. 47 measure denied the „Moji sati“ carve-out the backend keeps open | `2801dff`, `6dc4085` |
| 5 — this section, and the four rows | This section; `docs/SERBIAN-LAW-COMPLIANCE.md` row 12, row 15 and the §3 SW-14 row re-stated, with the §2 revision note recording the sweep; the four `docs/PROGRESS.md` bullets re-stated **a second time**, because the versions Task 3 wrote listed three owed limbs and Task 4 shipped two of them hours later | this commit |

**Where the mask reaches, and where it stops — the residual this cycle could not close.** Every
`kategorija_odsustva` value is exactly its ZEOR čl. 24 tač. 1 bucket minus `_minuta`; that derivation is
`book_absence`'s whole design. So req. 28 is discharged for the ZZPL column itself and for the rendered
grid's nine v) buckets, and it is **not** discharged for the two places the buckets travel in full: the
**wire**, where `WorkTimeMinutes` serialises every bucket beside a `kategorijaOdsustva` of `null`, and
the **exported file**. A client reading JSON rather than pixels recovers the category with one
`find(|b| b != 0)`. The rendered grid leaks one further category by subtraction — it draws b),
efektivno izvršeni and čekanja i zastoji, and b) minus the other two **is**
`obustava_rada_strajk_minuta`. Both halves are asserted rather than described, by
`commands::worktime::tests::the_mask_covers_the_zzpl_column_and_not_the_zeor_letters` (the struct half
for the wire, a positional cell read for the file). **The buckets stay:** ZEOR mandates them, the
month's totals are read off them, and zeroing one would be a *false* statement where the withheld
column is a silent one. Closing it properly needs the bucket columns to become `Option<i64>`, which
reaches the frozen Class A `klasifikacija_json`. Refusing the export outright while a nalog is live was
considered and rejected — the čl. 21 offline export is a duty — so the file carries a stated note above
the table instead, and every operator string in the feature says „skriva se sama kolona“ and never
„razlog nije dostupan“.

**Two smaller things this cycle deliberately did not do.**

- **`my_hours` is not masked, and the reason is not that a support operator cannot reach it.** They
  can: the command is session-gated only, so an operator remote-controlling the till can invoke it for
  whoever is signed in. It is left open because §4 req. 25's carve-out names this very function as the
  ZZPL čl. 26 discharge, because req. 28's own words are *„mask the column and payroll screens“* and a
  person reading their own row is neither, and because masking the payload would withhold from the data
  subject to prevent a disclosure the mask cannot prevent anyway — the operator is mirroring a screen
  and reads pixels. A masked „Moji sati“ would be perfectly expressible (`WorkTimeMonth` carries
  `razlog_odsustva_skriven` for exactly this), which is why that is not offered as a third reason. The
  decision is written on the function, the čl. 23 notice states it to the employee, and
  `my_hours_still_shows_the_employee_their_own_absence_reason` pins the divergence under one live nalog.
- **No other column is masked.** The absence reason is singled out because it is the one field in this
  register that is health-adjacent; masking more would be inventing a rule.

**§6 W-15 is sharpened by this cycle and is not closed by it.** The question is for the lawyer
reviewing F-1 — whether `docs/compliance/ugovor-o-obradi-nacrt.md` names posebne vrste podataka in its
ZZPL čl. 45 st. 3 clause, and whether the čl. 45 st. 4 tač. 7 delete-or-return duty covers support
artefacts — and this feature is what first put čl. 17-adjacent data on the machine the vendor reaches.
Two facts now make it demonstrable rather than theoretical, and both belong to the lawyer rather than
to code:

1. **The vendor reaches the register.** `list_month` and `export_month_csv` are exactly what an operator
   under a čl. 46 nalog invokes, which is why `radno_vreme.vrsta_primalaca` in the čl. 47 register now
   names the obrađivač — a single entry cannot tell the Poverenik in field 5 that remote support does
   not receive the record and in field 8 that a column of it is masked from them.
2. **The credential-reset chain reaches the column with the mask still on, and is almost unlogged.** An
   operator holding the vlasnik's admin session calls `users_update` (`commands/users.rs:89`) →
   `update_user` (`:184`) re-hashes any employee's PIN and writes **no** audit event on that branch —
   only the deactivation branch calls `clear_credentials` (`:278`) → `auth_login` as that employee →
   `worktime_my_hours` returns that employee's month with `kategorija_odsustva` unmasked. The only trace
   is a login. It is admin-gated and nothing bars it; it is why `my_hours`'s doc comment no longer
   claims the session bounds what the operator can reach, and it is recorded here rather than denied.
   Closing it is a decision about what a credential reset must log and whether a support operator may
   hold the vlasnik's session at all — neither of which req. 28 asks for.

**Verification gates — all six run from the repo root, one at a time, every command exited `0`:**

| Gate | Result | Exit |
|---|---|---|
| `bun run test` | **568 passed** / 0 failed, 36 files (was 547 / 35) | `0` |
| `bun run build` | tsc + vite, dist written; only the pre-existing chunk-size advisory | `0` |
| `cargo test --manifest-path src-tauri/Cargo.toml` | **1040 passed**; 0 failed, 0 ignored, 0 measured, 0 filtered out (was 1016) | `0` |
| `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings` | clean, no warnings | `0` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml -- --check` | clean, no output | `0` |
| `git diff --check` | clean, no output | `0` |

Net **+24 cargo / +21 bun** over `e7c79a3`, and the two figures reconcile against the diff rather than
against memory: `#[test]` attributes added per file are `commands/audit.rs` **+9**,
`commands/worktime.rs` **+7**, `db/migrations.rs` **+3**, `audit.rs` **+2**, `docs_guard.rs` **+2**,
`cl47.rs` **+1**; `it(` blocks added are `SupportApprovalPanel.test.tsx` **+7**,
`mock-adapter.odsustvo.test.ts` **+6** (a new file, which is the 36th), `WorkTimeModule.test.tsx`
**+5**, and one each in `App.test.tsx`, `PrivacyModule.test.tsx` and `AuditLogPanel.test.tsx`.
`local-adapter.test.ts` gained assertions inside an existing test rather than a test of its own.
**Nothing was deleted, weakened or skipped**, and three pins had to be *restated* rather than left
alone because the feature moved what they assert: the migration-count pin in `db/mod.rs` follows the
head from 22 to 23, the registered-command list in
`commands::audit::tests::no_audit_command_can_edit_or_delete_a_logged_row` names the new verb and was
**tightened** with a second assertion that exactly one registered command may mention `absence_reason`,
and `every_stored_code_renders_as_serbian_prose` moved 37 → 38 codes. Latest migration: **v23**.

**Bookkeeping note on dates.** Every commit in this cycle is dated 09.08.2026. Four blocks in this file
and five doc comments (four in `docs_guard.rs`, one on `AbsenceCell`) stamped the backend half
08.08.2026, taken from the plan's own filename rather than from `git log`; all nine are corrected here,
and register row 12's *„for a day after it was“* — the gap was under an hour — with them. The čl. 87
cycle's own 08.08.2026 stamps are correct and are untouched.

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