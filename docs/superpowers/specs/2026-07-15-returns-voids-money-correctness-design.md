# Returns & Voids Money-Correctness — Design

**Date:** 2026-07-15
**Status:** Approved design, ready for implementation plan
**Scope tier:** T1 pilot-blocker (gap 1.1 in `docs/COMPETITIVE-GAP-ANALYSIS.md`)
**Related:** gap 1.2 (shift attribution) and gap 1.4 (auth on mutations) are explicitly *deferred* — see Non-goals.

## Problem

A return or void today silently corrupts the two numbers a shop owner buys a POS for: the shift drawer reconciliation and the daily turnover. Two independent defects combine:

1. **The original sale's `status` is mutated.** `return_items` (`receipts.rs:591-597`) and `void_receipt` (`receipts.rs:424-430`) flip the *original* sale to `status='refunded'`/`'voided'`.
2. **The counter-document records no money.** Neither writes any `sale_payments` row, so the refund tender (cash/card leaving the drawer) is never captured. It cannot be captured today anyway: `sale_payments.amount_minor CHECK (amount_minor >= 0)` (`migrations.rs:133`) forbids a negative row.

The two money readers then disagree, because both key off the mutated `sales.status` with **different rules**:

- **Shift summary** (`shifts.rs:248`): `LEFT JOIN sales s ON s.shift_id = sh.id AND s.status = 'completed'`. Once the original flips to `refunded`, its entire cash **drops out** of the join → **phantom surplus**.
- **Daily turnover** (`reports.rs:322-347`): sums all payments but **negates** any sale where `status <> 'completed'` → the whole original is **subtracted** → **negative turnover**.

Worked example — a 10.000 RSD cash sale, then a 2.000 RSD return:
- Reality: drawer is up 8.000.
- Shift shows: opening + 0 → **+8.000 phantom surplus** vs. counted cash.
- Daily report shows: −10.000 (original negated) − 2.000 (counter negated) = **−12.000** for a day that took +8.000.

Void has the identical double-subtraction shape.

## Root cause & the invariant

The root cause is that **money is derived from a mutable status column** instead of from an immutable ledger.

**New invariant:** *Money is the signed sum of the `sale_payments` ledger. No money query reads `sales.status`.* A sale's positive payments and its counter-document's negative payments net out on their own, so the shift summary and every report compute the same sum and can no longer disagree.

## Decisions (locked with the founder)

| Decision | Choice |
|---|---|
| Scope | **Money-correctness only.** Cash in/out and `require_admin`-on-mutations are separate follow-ups. |
| Refund tender | **Void mirrors the original tenders exactly; return defaults to cash with an operator override.** |
| Cross-shift refunds | **Counter-document attaches to the current open shift; a return/void requires an open shift** (rejected otherwise, like a real till). |
| Money representation | **Only `sale_payments.amount_minor` becomes signed.** `sales` header totals stay positive magnitudes. |
| Original-sale status flip | **Kept as a display-only badge.** It is now money-inert, so the receipts UI needs no change. |

## Changes

### 1. Schema migration — `sale_payments` allows signed amounts

New migration appended to `MIGRATIONS` in `migrations.rs`, using the table-rebuild pattern already used for `backup_jobs` (`migrations.rs:186-217`):

- Create `sale_payments_next` identical to `sale_payments` except `amount_minor INTEGER NOT NULL CHECK (amount_minor <> 0)` (drop `>= 0`, forbid zero).
- `INSERT INTO sale_payments_next SELECT * FROM sale_payments`.
- `DROP TABLE sale_payments`; `ALTER TABLE sale_payments_next RENAME TO sale_payments`.
- Recreate `idx_sale_payments_sale`.
- Keep the `payment_method IN ('cash','card')` CHECK unchanged — tender expansion is out of scope.

No change to `sales` monetary CHECKs. No historical data backfill (see Non-goals).

### 2. Record refund payments on the counter-document (`receipts.rs`)

**Void (`void_receipt`)** — after inserting the void counter-document, load the original's `sale_payments` rows and insert their exact negatives against the void document:
`(cash, 6000),(card, 4000)` → `(cash, -6000),(card, -4000)`. The sale nets to zero movement.

**Return (`return_items`)** — after inserting the return counter-document, insert **one** negative row for the returned `total_minor` (already computed at `receipts.rs:492-497`) in the refund tender:
`(refund_tender, -total_minor)`, `refund_tender` ∈ {cash, card}, default `cash`.

`ReturnItemsRequest` gains `refund_tender: Option<String>` (validated against cash|card; `None` → cash). `VoidReceiptRequest` is unchanged.

### 3. Current-shift resolution + open-shift requirement (`receipts.rs`)

Both `return_items` and `void_receipt`:
- Resolve the current open shift (reuse the `load_open_shift` approach from `sales.rs:451`), and stamp the counter-document's `shift_id` with it (replacing the current `header.shift_id`).
- If no shift is open, reject with a Serbian business error (e.g. `no_open_shift` / "Otvorite smenu da biste evidentirali povrat.").

> Note: `load_open_shift` selects the most-recent open shift with no user predicate — the same limitation tracked as gap 1.2. This fix inherits it deliberately; it is not made worse.

### 4. Stop keying money on `status` — rewrite the readers

Two mechanical rules applied across all 14 sites:

- **Payment-sourced figures** (cash/card/total from `sale_payments`): bare `SUM(sp.amount_minor)` — sign now lives in the ledger. No `status`/`document_type` CASE.
- **Header/line-item-sourced figures** (`s.total_minor`, `si.total_minor`, quantities) and **counts**: sign/branch on `s.document_type` (`'sale'` → +, `'void'`/`'return'` → −). `document_type` never mutates.

Sites:

| File:line (current) | Query | New rule |
|---|---|---|
| `shifts.rs:244-248` | shift cash/card sum + join | drop `AND s.status='completed'`; bare signed `SUM(sp.amount_minor)` by method |
| `reports.rs:320` | daily `receipt_count` | count `document_type='sale'` |
| `reports.rs:322-344` | daily cash/card | bare `SUM(sp.amount_minor)` by method |
| `reports.rs:345` | daily `total_minor` | = cash+card (payment-based) so it stays internally consistent |
| `reports.rs:346-347` | daily refund magnitude + count | `-SUM(sp.amount_minor)` over `document_type IN ('void','return')`; count by `document_type` |
| `reports.rs:407-418` | shift-turnover count/cash/card/total | same split |
| `reports.rs:461-462` | cashier-turnover count/total | same split |
| `reports.rs:498-499` | payment-methods count/total | count `document_type='sale'`; bare signed `SUM` |
| `reports.rs:536-546` | product-sales qty/revenue/discount/**margin** | sign by `document_type` (margin formula unchanged — see Non-goals) |
| `reports.rs:596-602` | category-sales qty/revenue/discount/margin | sign by `document_type` |

### 5. Guards (`receipts.rs`)

`can_void`/`ensure_voidable` continue to require the original's (still-flipped) `status='completed'`, so **void stays blocked once a receipt has any return**. This is deliberate: the remainder is reversed through the **return** flow, which already handles partial and remaining-quantity math (`return_items` + `returned_quantity_for_item`). `can_return`/`ensure_returnable` are unchanged. See Non-goals for why proportional void-after-partial-return is not built here.

### 6. Frontend (`src/`)

- `ReturnItemsRequest` TS type (`services/types.ts`) gains `refundTender?: 'cash' | 'card'`; `local-adapter.ts` passes it through (camelCase → `request`).
- `ReceiptsScreen.tsx` return dialog: add a tender `<Select>` (default *Gotovina*), Serbian labels.
- Surface the `no_open_shift` error via the existing structured-error toast path.
- No change to the receipts list / badges (original status still flips for display).

## Non-goals (explicit)

- **Payment-tender expansion** (7 legal tenders, IPS QR) — separate gap; `sale_payments.payment_method` stays cash|card.
- **Proportional void-after-partial-return** — reverse the remainder via the return flow instead.
- **The margin/COGS bug** (`reports.rs:539-546` subtracts VAT-exclusive cost from VAT-inclusive revenue) — separate gap (stock valuation/COGS). The status→document_type swap must **not** alter the existing margin formula.
- **Shift attribution by logged-in user** (gap 1.2) and **`require_admin` on mutations** (gap 1.4) — separate follow-ups. `user_id` on the request stays client-supplied for now.
- **Historical data backfill** — the pilot DB is not yet live; the fix applies to new returns/voids. If a live DB ever needs it, a one-off backfill (write missing negative payment rows for existing counter-documents) would be a separate migration.

## Test plan (TDD — write these red first)

**Rust (`receipts.rs`, `shifts.rs`, `reports.rs` test modules):**
1. Migration: a negative `amount_minor` row inserts; a zero row is rejected.
2. Partial cash return: shift `expected_cash` = opening + (sale − refund) exactly; no phantom surplus. Daily report cash/total reflect the net.
3. Full void of a cash sale: shift nets zero movement for that sale; daily report nets zero.
4. Void mirrors a split cash+card sale → two negative payment rows matching the original split.
5. Return default tender = cash; `refund_tender='card'` records a negative card row.
6. Return/void with no open shift → `no_open_shift` error; nothing written.
7. Void blocked after a partial return with the guidance message; return still allowed for the remaining items.
8. Product/category report revenue goes negative for a returned line, keyed on `document_type` (independent of any status flip).

**Frontend (`ReceiptsScreen.test.tsx`, `local-adapter.test.ts`):**
9. Return dialog renders the tender select (default cash) and passes `refundTender` through `receipts_return_items`.
10. `no_open_shift` error renders a Serbian message.

## Acceptance criteria

- For any sequence of sales + partial/full returns + voids within a shift, `shift.expected_cash − shift.counted_cash` reflects only real drawer variance; the daily turnover equals the net of the signed payment ledger; **the shift summary and the daily report agree to the minor unit.**
- No money query references `sales.status`.
- A return/void requires an open shift and lands on that shift.
- `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `git diff --check` all pass.
