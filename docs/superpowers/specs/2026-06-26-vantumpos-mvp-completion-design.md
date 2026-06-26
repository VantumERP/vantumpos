# Design: VantumPOS MVP Completion

Date: 2026-06-26
Status: Approved design — basis for the implementation plan

## Context

The audited progress report (`docs/PROGRESS.md`, 2026-06-26) found VantumPOS at ~88% with all nine
modules real and CI green (frontend 57/57, backend 67/67, build + clippy `-D warnings` + `cargo fmt`
clean). The remaining work is not missing features but targeted correctness, compliance, UI-state, and
test-breadth gaps, captured as 12 prioritized recommendations in the report's "Recommended Next Steps".

This design closes all 12 so every module satisfies its `docs/module-specs` Acceptance Criteria and the
7-point Definition of Done in `docs/module-specs/README.md`.

## Goal / Definition of Done (global)

The effort is done when:

- All 12 recommendations below are implemented.
- Every change is delivered TDD: the spec-named failing test (or a new failing test that encodes the
  acceptance criterion) is written first, then the implementation makes it pass.
- All five verification gates are green:
  - `bun run test`
  - `bun run build`
  - `cd src-tauri && cargo test -- --test-threads=1`
  - `cd src-tauri && cargo clippy --all-targets --all-features --locked -- -D warnings`
  - `cd src-tauri && cargo fmt --check`
- No `userId: 1` (or other hard-coded operator) remains in the frontend; the real session user is threaded
  everywhere a movement/void/return/sale is recorded.
- Admin-only surfaces are role-gated at the Rust layer (authoritative) and hidden/disabled in the UI.
- A final adversarial code-review pass is clean.
- `docs/PROGRESS.md` is updated to reflect the new state.

## Implementation Standards (required)

These are hard constraints for every work unit:

- **UI / shadcn:** Reuse existing shadcn components in `src/components/ui/*` — check them before writing
  any UI. Add new components only via `bunx shadcn@latest add <component>` honoring `components.json`.
  (No dedicated "shadcn skill"/MCP is installed in this environment; this is the equivalent practice.)
  Apply the `frontend-design` skill for visual/UX quality.
- **React:** Apply the `vercel-react-best-practices` skill.
- **Tauri:** Apply the `tauri-v2` skill (commands, IPC, capabilities/permissions).
- **Rust:** Apply the `rust-best-practices` skill (ownership, `Result` error handling, tests).
- **Model:** Run all subagent / workflow work on the Opus model.
- **Local-first invariant:** No fiscalization, Medusa, or cloud dependencies are introduced. Money and
  quantities stay integer minor/milli units. Errors keep the `{code, message, details}` contract with
  Serbian operator-facing messages.

## Execution Approach

Autonomous, in this session, grouped by code area (not parallel git worktrees, because items 2/3/10 all
touch `settings`, and 4/6 touch `receipts` — parallel agents would conflict). Strict TDD per work unit;
run the relevant gate(s) continuously, full suite at group boundaries and at the end. One final
consolidated review is presented to the user; no per-module checkpoint.

Work proceeds in three groups, in this order:

- **Group A — Correctness & compliance** (items 4, 2, 5, 1, 12)
- **Group B — Module feature completion** (items 3, 6, 7, 8, 9)
- **Group C — Foundation polish & test breadth** (items 10, 11)

## Open Assumptions To Verify During Planning

1. **Session role availability.** Items 2 and 7 require role-based gating. Confirm whether the auth
   session (`src-tauri/src/state.rs`, `commands/auth.rs`) already carries the user's `role`. If not,
   plumbing the role into the session/DTO is part of items 2/7.
2. **`settings_update_receipt` shape.** Confirm the current signature before adding a session/role
   argument (item 2).
3. **Sales/inventory write path (item 12).** Prefer the lighter fix — extract a shared
   movement/balance helper used by both `commands/sales.rs` and `commands/inventory.rs`, or add the
   missing negative-stock guard to the sales path — over a large rewrite, to limit blast radius.
4. **Settings file overlap.** Items 2, 3, 10 all touch `commands/settings.rs` /
   `app/settings/SettingsScreen.tsx`; sequence them together to avoid churn.

## Work Units

Each unit lists the change, the test(s) to write first, and the acceptance criterion. File paths are the
expected primary touch-points from `docs/PROGRESS.md`; planning may refine them.

### Group A — Correctness & compliance

**A1 (rec #4) — Thread session user into Inventory & Receipts**
- Change: Replace hard-coded `userId: 1` with `session.user.id` in `src/app/inventory/InventoryScreen.tsx`
  and `src/app/ReceiptsScreen.tsx`, mirroring how `Users` already threads it. Ensure movements, voids,
  and returns record the real operator.
- Tests (first): frontend test asserting the session user id is passed into the inventory movement and
  the void/return calls.
- Acceptance: audit trail records the acting operator; no `userId: 1` remains.

**A2 (rec #2) — Enforce admin role-gating for receipt-sequence updates**
- Change: Add a session/role argument to `settings_update_receipt` (`commands/settings.rs`); reject
  non-admin callers with a structured `AppError` + Serbian message; gate the Podešavanja nav item/screen
  by role in `app/navigation.ts` / `AppShell.tsx`. Update Tauri registration/capabilities if needed.
- Tests (first): Rust — non-admin rejected, admin allowed. Frontend — nav/screen hidden or disabled for
  non-admin role.
- Acceptance: the only outright-missing business rule ("receipt sequence requires admin") is enforced at
  the Rust layer and reflected in the UI.

**A3 (rec #5) — Render swallowed preview/validation error in Register**
- Change: Surface the currently-defined-but-unrendered error state in `app/register/RegisterScreen.tsx`
  so an over-limit discount or invalid quantity shows the backend message and preserves the cart.
- Tests (first): frontend — a backend/validation error renders the message and the cart is preserved.
- Acceptance: cashiers see why completion is blocked; DoD point 5 (frontend states) met for Register.

**A4 (rec #1) — Add missing auth/shift backend tests**
- Change: No behavior change expected. Add the spec-required Rust tests: "deactivated user cannot log in"
  and "close shift derives expected cash" in `commands/auth.rs` / `commands/shifts.rs`.
- Tests (first): the two tests above (expected to pass against existing behavior; if they fail, fix the
  behavior).
- Acceptance: known regression holes are covered.

**A5 (rec #12) — Reconcile sales/inventory write divergence**
- Change: Unify the inventory movement/balance write path between `commands/sales.rs` and
  `commands/inventory.rs` via a shared helper, or add the missing negative-stock guard to the sales path,
  so the two paths cannot drift in balance math or stock rules. (Lighter option preferred — see
  assumption 3.)
- Tests (first): Rust — the sales path enforces the same negative-stock rule as the inventory path.
- Acceptance: one source of truth (or guaranteed-consistent rules) for stock writes.

### Group B — Module feature completion

**B1 (rec #3) — VAT rate edit/deactivate**
- Change: Wire `TaxRateDialog` to pass the rate `id` and expose a row edit/deactivate action in
  `app/settings/SettingsScreen.tsx`; ensure the backend supports update/deactivate (deactivate, not
  delete).
- Tests (first): frontend edit flow; backend update/deactivate.
- Acceptance: the "deactivate instead of delete" workflow is reachable.

**B2 (rec #6) — Receipts loading/empty/error states**
- Change: Add receipt-list loading + empty states and `try/catch` around search/detail reads in
  `app/ReceiptsScreen.tsx`, removing unhandled rejections; surface the persisted void/return reason text.
- Tests (first): frontend — loading, empty, and error states render; failed read is handled.
- Acceptance: DoD point 5 met for Receipts; no unhandled rejections.

**B3 (rec #7) — Reports filtering, role-gate, tests**
- Change: Extend the report query params for shift/cashier filtering in `commands/reports.rs`; role-gate
  the Reports screen; add date-range validation in `app/reports/ReportsScreen.tsx`.
- Tests (first): backend shift/cashier filter; frontend error/empty/failure states + date validation.
- Acceptance: spec filters supported; admin-only screen gated; FE state tests exist.

**B4 (rec #8) — Remove dead catalog screen, deep-link ledger**
- Change: Delete the orphaned `src/app/ProductCatalogScreen.tsx`; make the product "Lager" row action
  deep-link to that product's inventory ledger.
- Tests (first): frontend — "Lager" navigates to the product-scoped ledger.
- Acceptance: no dead code; deep link works.

**B5 (rec #9) — Import job-detail drill-down + tests**
- Change: Consume `import_get_job` in the Import history table (`app/import/ImportWizard.tsx`) for a
  job-detail drill-down; add unknown-VAT and required-mapping backend tests (`importer.rs` /
  `commands/imports.rs`); correct the "8 tests" miscount in docs.
- Tests (first): backend unknown-VAT and required-mapping; frontend drill-down opens job detail.
- Acceptance: history is inspectable; spec-named import tests exist; docs accurate.

### Group C — Foundation polish & test breadth

**C1 (rec #10) — Settings/Backup component tests + age-based stale detection**
- Change: Add `SettingsScreen.test.tsx` covering tabs, company save + toast, VAT validation, backup
  stale/failed render, and restore confirmation; make stale-backup detection age-based rather than
  `is_none`-only (`commands/backup.rs`).
- Tests (first): the five frontend tests above; backend age-based staleness test.
- Acceptance: module 08 frontend test gap closed; staleness is time-based.

**C2 (rec #11) — Shell header company name + migration regression test**
- Change: Wire the shell header to `settings_get_company` so the company shop name replaces the
  hard-coded "VantumPOS" (`app/AppShell.tsx`); add a pre-existing-old-DB forward-migration regression
  test (`db/migrations.rs`).
- Tests (first): frontend — header renders the company name from settings; Rust — old DB migrates
  forward cleanly.
- Acceptance: branding reads from settings; installed-base migration is protected.

## Final Verification & Handoff

1. Run all five gates; all green.
2. Confirm no `userId: 1` remains and role-gating is enforced in Rust.
3. Run an adversarial code-review pass over the full diff; address findings.
4. Update `docs/PROGRESS.md` to reflect completion (module percentages, DoD checklists, test counts).

## Out Of Scope

- XLSX import and additional barcode providers (the existing Open Food Facts lookup stays as-is).
- Real fiscalization, printer drivers, cash drawer, restaurant tables.
- Any cloud/Medusa integration.
- Unrelated refactoring beyond what a work unit needs.
