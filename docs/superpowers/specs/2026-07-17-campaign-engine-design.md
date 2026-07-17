# Campaign Engine (SW-6b) — Design

**Date:** 2026-07-17
**Status:** Approved direction (full-6b scope chosen by the user); revised once end-to-end before spec-freeze
**Legal authority:** `docs/ZOT-36-37-VERIFIED-RULES.md` (§2.4–§2.6, §3, §5.1, §5.2, §6). This spec encodes that memo; where they could ever disagree, the memo wins.
**Builds on:** SW-6a (merged): append-only `price_history`, `compute_prethodna_cena` (čl. 37 st. 3–4), catalog/import capture, reset re-seed.

## Scope

Everything the memo lists for 6b, in one cycle: the campaign entity over the closed 4-type enum, the st. 5 frozen anchor, all hard validations, the soft warnings, the attestations, the per-article perishability flag, and the rasprodaja no-restocking hook into inventory receiving. Deferred to 6c: label rendering/printing, inspector export, reports. Deferred generally: multi-store (`prodajno_mesto`) scoping — one install = one store, as in 6a.

## Locked decisions

| Decision | Choice |
|---|---|
| Scope | Full 6b in one cycle (user choice). |
| st. 5 „primenjivao" source | **Offered log** (shelf price = the price regime in force), plus a **till-divergence warning** when `sale_items` shows a lower charged price in the window. Models both datasets without silently conflating them (memo §2.4). |
| Seasonal 2×/yr counting | **By start date against its own calendar year**, counted only for campaigns that were actually **activated**. Surfaced in the UI as a stated assumption (memo §6.5 stays open). |
| Campaigns drive prices | **Yes** — activation/step/end write through `products.sale_price_minor` + `price_history` in one transaction. The register already sells at `sale_price_minor`, so till, log, and campaign cannot diverge. |
| Rasprodaja restocking | **Hard block** on receiving an in-rasprodaja SKU while the campaign is active; ending the campaign lifts it. No override machinery — deliberate strict choice, čl. 37 st. 7 cited in the error. |
| Overdue campaigns | **No auto-end.** A loud overdue indicator instead; unattended price mutation is riskier than a warning. |
| Perishable / <15d anchors | Manual entry with mandatory justification; display duty never suppressed. |

## 1. Schema — migration v10

Count assertion 9 → 10. New tables join `CORE_TABLES`; new indexes join `EXPLICIT_INDEXES`.

```sql
CREATE TABLE campaigns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    campaign_type TEXT NOT NULL CHECK (campaign_type IN ('rasprodaja', 'sezonsko_snizenje', 'akcijska_prodaja', 'promotivna_prodaja')),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active', 'ended', 'cancelled')),
    starts_on TEXT NOT NULL,                -- RFC3339
    ends_on TEXT,                           -- NULL only for rasprodaja ("dok traju zalihe")
    display_mode TEXT NOT NULL DEFAULT 'two_prices' CHECK (display_mode IN ('two_prices', 'percentage')),
    rasprodaja_ground TEXT CHECK (rasprodaja_ground IN ('prestanak_poslovanja', 'prestanak_u_objektu', 'prestanak_prodaje_robe')),
    special_conditions TEXT,                -- čl. 36 st. 2 tač. 4
    reduced_utility_reason TEXT,            -- čl. 36 st. 3, when goods have umanjena upotrebna vrednost
    marketing_label TEXT,                   -- free text, NO legal effect ("Black Friday" etc.)
    season_attested INTEGER NOT NULL DEFAULT 0 CHECK (season_attested IN (0, 1)),
    separation_attested INTEGER NOT NULL DEFAULT 0 CHECK (separation_attested IN (0, 1)),
    activated_at TEXT,
    ended_at TEXT,
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_campaigns_type_start ON campaigns(campaign_type, starts_on);

CREATE TABLE campaign_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    product_id INTEGER NOT NULL REFERENCES products(id),
    campaign_price_minor INTEGER NOT NULL CHECK (campaign_price_minor >= 0),  -- snižena cena / intro price
    prethodna_cena_minor INTEGER CHECK (prethodna_cena_minor IS NULL OR prethodna_cena_minor >= 0),
    anchor_status TEXT NOT NULL CHECK (anchor_status IN ('computed', 'manual', 'none')),  -- 'none' only for promotivna
    anchor_window_days INTEGER,
    anchor_truncated INTEGER NOT NULL DEFAULT 0 CHECK (anchor_truncated IN (0, 1)),
    anchor_reason TEXT,                     -- IncomputableReason that forced 'manual', if any
    anchor_justification TEXT,              -- mandatory when anchor_status = 'manual'
    future_regular_price_minor INTEGER CHECK (future_regular_price_minor IS NULL OR future_regular_price_minor >= 0),  -- promotivna only
    pre_campaign_price_minor INTEGER,       -- captured at activation; default return price at end
    UNIQUE (campaign_id, product_id)
);
CREATE INDEX idx_campaign_items_campaign ON campaign_items(campaign_id);
CREATE INDEX idx_campaign_items_product ON campaign_items(product_id);

ALTER TABLE products ADD COLUMN perishable INTEGER NOT NULL DEFAULT 0 CHECK (perishable IN (0, 1));
ALTER TABLE products ADD COLUMN perishable_justification TEXT;
```

**`price_history.source` CHECK rebuild** (same migration): SQLite cannot alter a CHECK, so rebuild the table exactly as migration v6 rebuilt `sale_payments` — `CREATE price_history_next` with the widened CHECK `('create','update','import','deactivate','reactivate','seed','campaign_start','campaign_step','campaign_end')`, `INSERT ... SELECT` **including `id`** (row identity is evidentiary), `DROP`, `RENAME`, recreate `idx_price_history_product`. Append-only discipline is unaffected: the rebuild is a schema migration, not a data mutation — values are copied verbatim.

**The anchor lives per item, never per campaign.** Every SKU has its own price history; a store-wide akcija has as many anchors as articles. Snapshotted when the item is added, re-snapshotted only while the campaign is still a **draft** and its `starts_on` changes (a draft is unannounced — nothing legal has attached to it yet), and **immutable from activation onward** — the memo calls lazy recompute "the exact manipulation st. 5 exists to prevent" (each successive cut would pollute the window and drift the displayed prior price downward). Anti-evasion invariant test: no code path changes `prethodna_cena_minor` on any item of a campaign whose `activated_at` is set.

## 2. Domain module — `src-tauri/src/campaigns.rs`

Owns the validation engine, anchor snapshotting, and lifecycle transitions. Command layer (`commands/campaigns.rs`) stays thin, all admin-gated (`require_admin`, acting id from the session).

**Lifecycle:** `draft → active → ended`, plus `draft → cancelled`. Draft is freely editable (items, prices, dates) and **every mutation re-runs the full validation engine**. Activation re-validates everything (another sezonsko may have been activated since the draft was written), then in ONE transaction: capture each item's `pre_campaign_price_minor`, write each item's `campaign_price_minor` into `products.sale_price_minor` via `record_offered_price_change` with `source='campaign_start'`, set promotivna items `active=1` (their first offering), stamp `activated_at`. A **markdown step** (`adjust_item_price`) on an active campaign validates the new price (still below the frozen anchor; the anchor itself is never touched) and writes through with `source='campaign_step'`. **Ending** writes each item's return price — default `pre_campaign_price_minor`, per-item overridable, promotivna items forced to `future_regular_price_minor` — with `source='campaign_end'`, stamps `ended_at`. Cancel is draft-only and touches no prices.

## 3. Hard validations (block, with čl.-cited Serbian messages)

| # | Rule | Basis |
|---|---|---|
| H1 | `campaign_type` ∈ the closed four; `marketing_label` never influences rules | čl. 37 st. 1; čl. 36 st. 9 |
| H2 | Sezonsko: `starts_on` ∈ 25.12–10.01 ∪ 01.07–15.07 | čl. 37 st. 8 |
| H3 | Sezonsko: ≤2 **activated** per calendar year of `starts_on` (assumption surfaced in UI) | čl. 37 st. 8; memo §6.5 |
| H4 | Sezonsko: duration ≤60 days | čl. 37 st. 9 (purposive: attaches to st. 8) |
| H5 | Akcijska: duration ≤31 days | čl. 37 st. 10 |
| H6 | `display_mode='percentage'` only for akcijska with **declared** duration ≤3 days; any extension re-validates and rejects if it would exceed 3 days while percentage-labelled; never neither-prices-nor-percentage | čl. 37 st. 11 |
| H7 | Promotivna: duration ≤60 days; `future_regular_price_minor` required; every item has **zero non-NULL offered-price rows before `starts_on`** (never previously offered — exact, no invented thresholds); items enter inactive and are first offered by activation | čl. 36 st. 9 |
| H8 | Promotivna never gets an anchor (`anchor_status='none'`, all prethodna fields NULL) and never passes through the prethodna validator | memo §5.2 |
| H9 | Sniženje types (rasprodaja/sezonsko/akcijska): every item's `campaign_price_minor` **< `prethodna_cena_minor`** (below, not equal). Promotivna exempt (its lower price is permissive — „može") | čl. 37 st. 6/8/10 |
| H10 | Rasprodaja: exactly one of the three statutory grounds selected; `ends_on` nullable only here | čl. 37 st. 6; čl. 36 st. 2 t. 3 |
| H11 | Rasprodaja: while active, `inventory_receive` **rejects** receiving any in-rasprodaja product (error cites čl. 37 st. 7; ending the campaign lifts the block). Other SKUs unaffected | čl. 37 st. 7 |
| H12 | Attestations required to activate: sezonsko → `season_attested` („nakon proteka sezone" — software cannot verify a season elapsed and does not pretend to); rasprodaja → `separation_attested` (physical separation) | čl. 37 st. 7–8 |
| H13 | Perishable items and `TooNewInAssortment`/`NotOfferedInWindow`/`NoHistory` items: `anchor_status='manual'` with `prethodna_cena_minor` + `anchor_justification` required. Display duty never suppressed | čl. 37 st. 2 vs st. 3; memo §2.6, §6.1, §6.3 |
| H14 | `starts_on` ≤ `ends_on`; dates RFC3339; items non-empty; prices integer minor units |  |

## 4. Soft warnings (advisory; never a green checkmark; never block)

| # | Warning | Basis |
|---|---|---|
| W1 | Anchor price was in force <3 days total within the window → „zanemarljivo kratak period" risk | čl. 38 st. 4 |
| W2 | Same SKU in ≥2 campaigns starting within 30 days → the ratchet-down effect collapses the advertisable discount | memo §2.5 |
| W3 | `headline_percent` set and applies to <20% of the active assortment | čl. 38 st. 2 |
| W4 | Any item's anchor `truncated` (log doesn't cover the window) | 6a carry-through |
| W5 | Till-divergence: `sale_items` shows a per-line effective unit price below the computed anchor inside the window → „stricter „primenjivao" reading may require a lower anchor". Line-level only (sale-level discounts not allocated per item) — limitation stated in the warning | memo §2.4 |
| W6 | Campaign past its declared `ends_on` and still active → loud overdue badge; prices remain until manually ended | čl. 36 st. 2 t. 3 |

## 5. Commands (all `require_admin`; acting id from session)

`campaigns_list` (with status/type filters + overdue flag) · `campaigns_get` (items with anchors, validations, warnings) · `campaigns_create` (draft; full validation; anchor snapshot per item) · `campaigns_update` (draft-only; re-snapshot anchors if `starts_on` changed; full re-validation) · `campaigns_validate` (dry-run for the wizard: hard failures + warnings without persisting) · `campaigns_activate` · `campaigns_adjust_item_price` (markdown step) · `campaigns_end` (per-item return-price overrides) · `campaigns_cancel` (draft-only). Catalog: `SaveProductRequest`/`ProductSummary` gain `perishable` + `perishableJustification` (a non-price field — no history row).

## 6. UI

**Campaigns screen** (admin, new tab in the back office): list with status/type/overdue badges; a create wizard — type first (the four, with plain-language legal notes and the quota/window preview for sezonsko), then dates/display-mode/type-specific fields (grounds, future price, attestations), then items with per-item anchor preview (computed value + window + stav citation matching the branch, truncation, manual-entry path with justification when required), then a validation summary (hard failures block Save; warnings listed, explicitly non-blocking); detail view with lifecycle actions (activate / adjust price / end with return-price table / cancel) and every stored attestation and justification visible. **Catalog form:** perishable checkbox + justification field. Citation copy branches on the window/stav actually used, as fixed in 6a.

## 7. Testing

Memo worked examples verbatim: **(b)** — sezonsko 05.07.2026, anchor **11.900** frozen across steps 9.900 → 8.900 → 7.500, asserting both named wrong implementations fail (12.900 "price at first reduction"; lazy recompute → 9.900); **(c1)** — promotivna: no anchor, cap 60d, future price required, transition at end. Every H-rule gets accept+reject tests (H3 with two activated + a third rejected, cancelled/draft not counting; H6 extension re-label; H7 seeded-product rejection; H9 equal-price rejection; H11 receive-block lifted after end). Lifecycle atomicity: activation writes `campaign_start` rows for every item or none (mid-way failure rolls back); end restores; promotivna first-offer row is its first history row. Anchor immutability invariant. W1–W6 each have a triggering + non-triggering test. Frontend: wizard blocks on hard failures, shows warnings without blocking, attestation gating, overdue badge, perishable field round-trip.

## Non-goals
- Label rendering/printing, inspector export, reports (→ 6c / SW-8).
- Prekid auto-detection, upward anchor resets, seasonal-quota rolling window, auto-ending overdue campaigns, „outlet"-style types, any "this is legal" verdict (memo §5.2).
- Advertising (čl. 38) validation beyond the W1/W3 warnings — separate regime („ranija cena").
- Multi-store scoping.

## Acceptance criteria
- A campaign cannot be activated in violation of any H-rule, and every H-failure explains itself with the correct čl./stav citation.
- Activation/step/end mutate till prices and the price log atomically; the log alone reconstructs every promotional price movement with `campaign_*` provenance.
- The st. 5 anchor is snapshotted once and provably never changes; worked examples (b) and (c1) pass to the dinar.
- Rasprodaja restocking is blocked while active and released at end.
- Warnings inform without blocking; nothing anywhere affirms legality.
- All gates green: `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `git diff --check`.
