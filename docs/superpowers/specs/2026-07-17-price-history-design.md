# Price History (SW-6a) — Design

**Date:** 2026-07-17
**Status:** Approved design, ready for implementation plan
**Legal basis:** `docs/ZOT-36-37-VERIFIED-RULES.md` (adversarially verified 17.07.2026; 64/64 load-bearing rules survived). That memo is the authority for every rule below — this spec does not restate law, it encodes it.

## Scope

**This cycle is SW-6a only:** the append-only offered-price log, the prethodna cena computation for ZoT čl. 37 **st. 3 and st. 4**, and retention. Staged deliberately, because **the price log can never be backfilled** — every day without it is permanently missing evidence, and it is the one part of SW-6 with that property.

Deferred to later cycles (explicitly out of scope here):
- **SW-6b** — campaign entity, the closed 4-type enum, the **st. 5 frozen progressive anchor**, hard validations (seasonal windows/2×yr/≤60d, akcijska ≤31d, promotivna ≤60d, rasprodaja grounds + no-restocking), warnings, attestations.
- **SW-6c** — reports, label data, walk-away inspector export.
- **SW-8** — the printing stack (physical shelf labels).

**st. 5 is out of scope on purpose:** the frozen anchor is defined relative to a campaign's start, and there are no campaigns until 6b. Building it now would mean inventing a campaign concept to hang it on.

## Locked decisions

| Decision | Choice |
|---|---|
| Staging | Log + computation first; campaigns (6b) and output (6c) follow. |
| Timeline representation | **Append-only event rows; intervals derived at query time.** `price_minor IS NULL` = offering ended (explicit gap). No UPDATE, no DELETE, ever. |
| Regulator inquiry | Skipped. čl. 36/37 contain **no delegation clause**, so no bylaw can lawfully add sniženje rules — the statute is the complete specification. |
| Go-live reset | **Clears price history and re-seeds** (see §5). This is the one place we deviate from the memo's "never prune", and it is deliberate. |

## 1. Schema — migration v9

Migration count assertion moves **8 → 9**. `price_history` joins `CORE_TABLES`; `idx_price_history_product` joins `EXPLICIT_INDEXES`.

```sql
CREATE TABLE price_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    effective_from TEXT NOT NULL,
    price_minor INTEGER CHECK (price_minor IS NULL OR price_minor >= 0),
    source TEXT NOT NULL CHECK (source IN ('create', 'update', 'import', 'deactivate', 'reactivate', 'seed')),
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_price_history_product ON price_history(product_id, effective_from);
```

- `price_minor NULL` means **the product stopped being offered** at `effective_from`. This is the mechanism that makes gaps explicit; everything in §4 about returning seasonal stock depends on it.
- `effective_from` vs `created_at`: separate on purpose. `effective_from` is when the price took effect (business time); `created_at` is when the row was written (system time). They coincide today, but conflating them would block any future back-dated correction.
- `source` values are stable internal identifiers (English), never shown to users.
- `ON DELETE CASCADE` mirrors the existing `inventory_balances` pattern for schema consistency. Note that **no application path deletes a product** — the catalog exposes deactivation only (`set_product_active`), and the sole `DELETE FROM products` in the tree is in a db-layer test. The clause is defensive, not a live route by which history can vanish.
- **Append-only is a codebase invariant, not a DB constraint.** SQLite cannot express "INSERT only". It is enforced by there being no UPDATE/DELETE statement against this table anywhere in the codebase, and by an invariant test (§7) that asserts price rows survive every mutation path.

## 2. Capture points — four, not three

Every path that changes what price is offered must append a row, **inside the same transaction as the product write** (a price change that is recorded non-atomically is worse than none — it can diverge).

| Path | File | Row appended |
|---|---|---|
| `create_product` | `commands/catalog.rs:271` | `source='create'`, `price_minor = sale_price_minor` |
| `update_product` | `commands/catalog.rs:345` | `source='update'` — **only when `sale_price_minor` actually changes** (compare to the stored value first; an edit to name/SKU must not pollute the timeline) |
| `set_product_active` | `commands/catalog.rs:432` | `active=false` → `source='deactivate'`, `price_minor = NULL`; `active=true` → `source='reactivate'`, `price_minor = <current sale_price_minor>` |
| `commit_product_rows` | `importer.rs:692` | `source='import'` on both the UPDATE (732) and INSERT (761) branches; only when the price actually changes on the UPDATE branch. Already takes `&Transaction` — atomicity is free. |

**`set_product_active` is the one a naive implementation misses,** and missing it silently re-creates the exact trap the legal memo calls the most dangerous in this feature: without a NULL row marking the gap, returning seasonal stock is indistinguishable from a new arrival, and §4 would apply the 15-day window to it — anchoring too high and overstating the discount.

**Acting user:** threaded as a trailing `acting_user_id: i64` parameter, exactly as `void_receipt`/`return_items` already do. The command wrappers already call `require_admin`, which returns `UserAccount`; pass `.id`. Never accept a user id from the client.

## 3. Seeding — honest, not flattering

Migration v9 inserts one row per **active** product: `source='seed'`, `effective_from = <migration timestamp>`, `price_minor = sale_price_minor`, `user_id = NULL`.

Seeding at `products.created_at` was considered and **rejected**: it would assert that today's price was offered ever since the product was created, which is a fabrication, and it would silently produce confident, wrong anchors. Seeding at migration time is truthful — we genuinely do not know what was offered before we started logging — and it makes every pre-log window detectably `truncated` (§4).

Inactive products get no seed row; they are not being offered. Their first row will be a `reactivate` when they return.

## 4. Computation — st. 3 and st. 4

A pure, well-tested query. No side effects.

```rust
pub struct PrethodnaCena {
    pub price_minor: i64,
    pub window_days: i64,
    pub window_from: String,   // inclusive
    pub window_to: String,     // exclusive
    pub truncated: bool,
}

pub enum PrethodnaCenaResult {
    Computed(PrethodnaCena),
    Incomputable(IncomputableReason),
}

pub enum IncomputableReason {
    TooNewInAssortment { age_days: i64 },  // st. 4: age < 15 — law supplies no usable window
    NotOfferedInWindow,                     // off-shelf for the entire window
    NoHistory,                              // no rows at all
}
```

**Assortment age** = `campaign_start − products.created_at`, **not** the age of the price log.

This distinction is essential and easy to get wrong. Deriving age from the first log row would mean that at migration every seeded product has age 0, so for 15 days after install *every* product would report `TooNewInAssortment` and nothing could be computed. That conflates *our log being new* with *the goods being new to the assortment* — st. 4 is about the latter. `products.created_at` always predates any history row (history begins at `create` or at the v9 `seed`), so it is the best available evidence of assortment membership.

It is also a **conservative** lower bound, and the error direction is what makes it safe: if a product sat in the catalog unoffered for a while, `created_at` overstates its assortment age, which selects the *wider* 30-day window — and a wider window can only admit more offered prices, only lower the MIN, only understate a discount. Understating is lawful; overstating is the violation. Erring toward a larger age is therefore always the safe error.

**Age reaches back across offering gaps and is never reset by a restock.** This is the returning-seasonal-goods rule (§7 test (c4)) and the single most important line in this spec: a jacket returning to the shelf after a gap is not a new arrival, so st. 4 must not apply to it.

**Window selection** (čl. 37 st. 3–4):

```
age >= 30d  -> window = 30d                      (st. 3)
15d <= age < 30d -> window = age                 (st. 4: satisfies "ne kraćem od 15 dana")
age < 15d   -> Incomputable::TooNewInAssortment   (statute supplies no window)
```

`age < 15d` **neither blocks nor computes.** There is no prohibitory language anywhere in čl. 37 attaching to short-assortment goods, so blocking a lawful sale is unjustified commercial harm; and computing over a shorter window than the statute names would invent law. The UI warns and requires a manual entry with recorded justification (§6). The two research passes disagreed on this point, so it stays a warning until counsel says otherwise.

**Window bounds:** half-open `[start − window_days, start)`, calendar days. Safe default — a wider window can only lower the MIN, i.e. only ever understate a discount. Never narrow it.

**The MIN** runs over offered prices whose validity interval **overlaps** the window, and **includes prior promotional prices**. Never filter promo periods out to "find the regular price" — that produces an unlawfully high anchor.

```sql
WITH timeline AS (
    SELECT price_minor,
           effective_from AS valid_from,
           LEAD(effective_from) OVER (
               PARTITION BY product_id ORDER BY effective_from, id
           ) AS valid_to
    FROM price_history
    WHERE product_id = ?1
)
SELECT MIN(price_minor)
FROM timeline
WHERE price_minor IS NOT NULL
  AND valid_from < :window_to
  AND (valid_to IS NULL OR valid_to > :window_from)
```

Interval `[valid_from, valid_to)` overlaps `[window_from, window_to)` iff `valid_from < window_to AND (valid_to IS NULL OR valid_to > window_from)`. `id` breaks ties on identical `effective_from`.

**`truncated`** = the product's earliest `effective_from` is later than `window_from`, i.e. the log does not cover the full window. Reported, never hidden: a truncated result is stated as truncated rather than silently computed.

**"nudio" vs "primenjivao":** st. 3 keys on *offered* — this log. st. 5 keys on *applied*, a different dataset (what was actually charged, where register discounts can differ from the shelf price). They are deliberately not conflated; st. 5 lands in 6b.

## 5. Retention — and the one deviation

No pruning. Ever. The duty is ~5 years (Pravilnik čl. 19 → Zakon o računovodstvu čl. 28), and **never on a rolling 30-day window** — the 30 days is a lookback, not a lifetime. Nothing in this cycle deletes price history, and the invariant test in §7 pins that.

**Deviation — go-live reset clears price history and re-seeds.** `reset_trading_data` is explicitly the pre-production tool: it already requires admin + exact typed confirmation, forces a safety backup, and writes a permanent tombstone. Practice prices were **never offered to a consumer**, so retaining them would let fake training data drive a real anchor — a wrong number with legal consequences. Clearing and re-seeding at reset is therefore both safe and more correct. The existing tombstone records it.

This is the only point where this spec departs from the legal memo's "never prune" instruction, and it is a deliberate, reasoned exception rather than an oversight.

## 6. Surfacing

Read-only advisory in the catalog product form. When an admin lowers `sale_price_minor`, show:
- the computed prethodna cena, the window used, and its date bounds;
- „Prethodna cena izračunata prema čl. 37 st. 3." — **never** a green „this promotion is legal" verdict (čl. 38 st. 4 can bite even where the st. 3 arithmetic is perfect);
- a truncation notice when the log does not cover the window;
- for `TooNewInAssortment`: „Roba je u asortimanu kraće od 15 dana — zakon ne propisuje jasan referentni period. Unesite prethodnu cenu ručno i obrazložite." plus the manual entry.

No enforcement, no blocking. The display duty (st. 2) attaches to the physical shelf label; labels need SW-8.

## 7. Test plan

The memo's worked examples become tests **verbatim** — same SKUs, dates, and dinar amounts, so a reviewer can diff them against the legal source:

- **(a)** `KOSULJA-M-42`, campaign 20.07.2026 → **4.990,00**; the June akcija at 4.290,00 correctly falls outside the window; the 01.07 *increase* to 5.290,00 does not become the anchor (MIN, not "price before the cut").
- **(a′) ratchet-down variant** — same SKU, campaign 12.07.2026 → **4.290,00** (window captures the earlier akcija).
- **(c2)** `MAJICA-S-38`, 22 days in assortment → window = 22d → **2.290,00**.
- **(c3)** `PATIKE`-style 6-day item → `Incomputable::TooNewInAssortment { age_days: 6 }` — asserts it neither blocks nor computes.
- **(c4) the returning-jacket trap** — `JAKNA-Z-L` offered, gap, re-offered 10 days before a sniženje → **st. 3 with full history across the gap**, NOT st. 4. This is the test that would fail on the naive implementation.

Plus: NULL-row gap handling; `truncated` detection against seeded history; MIN includes promotional lows; interval-overlap boundary cases (interval ending exactly at `window_from`, starting exactly at `window_to`); tie-break on identical `effective_from`; a non-price product edit appends **no** row; deactivate→reactivate produces NULL then price rows; import writes history atomically (rollback leaves none).

**Invariant tests:** no mutation path (update, deactivate, import, void/return, restore) removes or alters an existing price_history row; reset clears and re-seeds (the deliberate exception).

## Non-goals

- Campaign entity, the 4-type enum, st. 5 frozen anchor, seasonal/duration validation (→ 6b).
- Label rendering, printing, inspector export, reports (→ 6c, SW-8).
- `prodajno_mesto` scoping: one VantumPOS install = one prodajno mesto. Documented assumption, revisit before multi-store — the memo flags this as low-risk single-store.
- Applied-price („primenjivao") dataset for st. 5 (→ 6b).
- Advertising (čl. 38) — a separate regime with a different term, „ranija cena". This validator must never be reused for ad copy.

## Acceptance criteria

- Every path that changes an offered price appends exactly one history row, atomically with the product write, attributed to the session user.
- Deactivation records a gap; reactivation records a return; a returning product computes under st. 3 across the gap.
- Prethodna cena matches the memo's worked examples to the dinar.
- An item <15 days in assortment is neither blocked nor silently computed.
- A window not covered by the log reports `truncated`.
- No code path deletes or updates price history, except the go-live reset's deliberate clear-and-reseed.
- All gates green: `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `git diff --check`.
