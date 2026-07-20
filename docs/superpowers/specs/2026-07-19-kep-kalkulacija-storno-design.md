# KEP Kalkulacija + Storno Engine (SW-9b) — Design

**Date:** 2026-07-19
**Status:** Approved (one cycle; catalog price authoritative; manual nivelacija)
**Legal authority:** `docs/KEP-VERIFIED-RULES.md` — §3 (the 14-element kalkulacija) and §4 (the cause→column storno map). This spec encodes that memo; it does not restate law.
**Builds on:** SW-9a (the `kep_entries` append-only ledger; its `kind` enum already carries the 9b kinds; `post_receipt_zaduzenje`, `next_redni_broj`, `book_year_of`), SW-6a (`record_offered_price_change` → `price_history`), SW-6c/SW-8 (HTML renderer + print), company settings.

## Scope

The isprave and adjustments that keep the KEP value ledger correct: the **14-element kalkulacija** generated on each receipt (formalizing 9a's provisional zaduženje), and the **typed storno engine** where every event's `{kolona, sign}` is derived from a fixed cause (never user choice) — nivelacija, supplier/customer returns, otpis/manjak/rashod, popis višak/manjak, and reversing-storno error correction.

## Confirmed decisions

| Decision | Choice |
|---|---|
| Kalkulacija price | **Catalog `sale_price` authoritative; marža derived backward.** All 14 elements computed from qty + nabavna + retail price + tax rate. |
| 9b shape | **One cycle** — kalkulacija + storno together. |
| Nivelacija | **Updates the catalog price** (and `price_history`) as part of the revaluation — the KEP nivelacija and the shelf-price change are one transaction. |
| Campaign→KEP nivelacija | **Deferred** (manual nivelacija only). A campaign markdown is legally also a nivelacija; auto-posting it is a big cross-subsystem coupling with double-count risk, so it is a named follow-up. Until then, a shop running campaigns posts the corresponding nivelacija manually or the KEP saldo overstates stock value. |
| Storno vs inventory | **The KEP storno posts VALUE entries only; it does NOT auto-apply inventory movements** (that would double-post — e.g. a popis-višak routed through the receive path would also fire 9a's receipt zaduženje). The KEP is a value book fed by isprave; physical stock is the separate inventory ledger, reconciled at popis (9c). |

## 1. Schema — migration v13

Count assertion 12 → 13. `kalkulacije` joins `CORE_TABLES`; its index joins `EXPLICIT_INDEXES`.

```sql
CREATE TABLE kalkulacije (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    redni_broj INTEGER NOT NULL,                     -- kalkulacija sequence (element 4), per book_year
    book_year INTEGER NOT NULL,
    product_id INTEGER NOT NULL REFERENCES products(id),
    -- header identity (elements 1-3), snapshotted from company settings at creation:
    poslovno_ime TEXT NOT NULL,
    prodajno_mesto TEXT NOT NULL,
    pib TEXT NOT NULL,
    -- per-line (elements 5-14), all integer minor units / milli:
    trgovacki_naziv TEXT NOT NULL,                   -- 5
    jedinica_mere TEXT NOT NULL,                      -- 6
    kolicina_milli INTEGER NOT NULL,                 -- 7
    nabavna_cena_po_jm_minor INTEGER NOT NULL,       -- 8
    vrednost_po_fakturi_minor INTEGER NOT NULL,      -- 9  = 8 × 7
    razlika_u_ceni_minor INTEGER NOT NULL,           -- 10 = 11 − 9 (may be negative: loss-leader)
    prodajna_vrednost_bez_pdv_minor INTEGER NOT NULL,-- 11 = round(13 / (1+rate))
    pdv_minor INTEGER NOT NULL,                       -- 12 = 13 − 11
    prodajna_vrednost_sa_pdv_minor INTEGER NOT NULL, -- 13 = 7 × 14  → posts to KEP kolona 4
    prodajna_cena_po_jm_minor INTEGER NOT NULL,      -- 14 = product sale_price (sa PDV)
    reference_type TEXT,                             -- the receipt/faktura doc
    reference_id INTEGER,
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    UNIQUE (book_year, redni_broj)
);
CREATE INDEX idx_kalkulacije_product ON kalkulacije(product_id, created_at);

ALTER TABLE kep_entries ADD COLUMN cause TEXT;   -- finer legal cause where a kind is shared (otpis/rashod/manjak/nivelacija↓ all = nivelacija_down_storno)
```

## 2. Kalkulacija — derived backward (`src-tauri/src/kep_kalkulacija.rs`)

Pure derivation, unit-tested:

```rust
pub struct KalkulacijaElements {
    pub vrednost_po_fakturi_minor: i64,       // 9
    pub razlika_u_ceni_minor: i64,            // 10 (signed)
    pub prodajna_vrednost_bez_pdv_minor: i64, // 11
    pub pdv_minor: i64,                        // 12
    pub prodajna_vrednost_sa_pdv_minor: i64,  // 13  → KEP kolona 4
}
pub fn derive_kalkulacija(kolicina_milli: i64, nabavna_po_jm_minor: i64, prodajna_po_jm_minor: i64, rate_basis_points: i64) -> KalkulacijaElements
```

Given qty, nabavna/jm (element 8), prodajna/jm sa PDV (element 14 = `sale_price`), and the PDV rate:
- `13 = round(kolicina_milli × prodajna_po_jm_minor / 1000)` (qty is milli).
- `11 = round(13 × 10000 / (10000 + rate_basis_points))` — bez-PDV base; **12 = 13 − 11** (so PDV + base sum to 13 exactly, no drift).
- `9 = round(kolicina_milli × nabavna_po_jm_minor / 1000)`.
- **`10 (marža) = 11 − 9`** — the backward-derived margin; **may be negative** (nabavna > prodajna, a loss-leader) — that is legal, so it is stored and rendered, never rejected.

**Generation on receipt.** In the same transaction as 9a's `post_receipt_zaduzenje`, generate the kalkulacija (elements 1–3 snapshotted from company settings; 5–7 from the product + receipt; 8 from the receipt's `purchase_price_minor`; 14 from the product's `sale_price_minor`; 9–13 derived) and link the zaduženje to it (`reference_type='kalkulacija', reference_id=<kalkulacija.id>`). The zaduženje amount already equals element 13 (both are `qty × sale_price`), so no ledger value changes — 9b adds the formal isprava, not a re-post.

**Kalkulacija HTML isprava.** A printable document (SW-6c renderer shape) listing all 14 elements, opened via SW-8. Header identity from the snapshot; non-fiscal footer; no QR/PIB-as-fiscal-block.

## 3. Storno engine (`src-tauri/src/kep_storno.rs`) — cause→{kolona, sign} is a hard map

```rust
pub enum StornoCause {
    NivelacijaUp, NivelacijaDown, PdvRateUp, PdvRateDown,
    SupplierReturn, CustomerReturn, Otpis, ManjakOdluka, Rashod,
    PopisVisak, PopisManjak,
}
```

Derived, never user-chosen:

| Cause | kolona | sign | kind | amount |
|---|---|---|---|---|
| NivelacijaUp / PdvRateUp | 4 | + | `nivelacija_up` | `(new − old retail sa PDV) × on-hand qty` |
| NivelacijaDown / PdvRateDown | 4 | − storno | `nivelacija_down_storno` | `|new − old| × on-hand qty` |
| SupplierReturn | 4 | − storno | `supplier_return_storno` | `qty × retail sa PDV` |
| Otpis / ManjakOdluka / Rashod | 4 | − storno | `nivelacija_down_storno` | `qty × retail sa PDV` |
| CustomerReturn (raskid/odustanak) | 5 | − storno | `customer_return_storno` | `qty × retail sa PDV` |
| PopisVisak | 4 | + | `popis_visak` | `qty × retail sa PDV` |
| PopisManjak | 5 | + | `popis_manjak` | `qty × retail sa PDV` |

Each event stores its **basis document** (naziv/broj/datum → composed into `opis`) and the finer `cause` (so otpis vs rashod vs manjak, sharing a kind, stay legally distinct). A crveni-storno amount is stored as a **negative** `amount_minor` (subtracts from the column total); the print renders it visually distinct (parentheses/red).

**Nivelacija is the one cause that also changes the product price** (approved): `kep_nivelacija(product_id, new_sale_price_minor, basis)` — in one transaction — reads the old price + on-hand qty (`inventory_balances`) + tax rate, updates `products.sale_price_minor`, records the change via `record_offered_price_change` (source `'update'` → `price_history`), and posts the KEP kolona-4 Δ entry. The other causes post a KEP value entry only (the physical stock write-off stays the shop's separate inventory action; §Scope).

## 4. Error correction — reversing storno (§4.4)

A posted row is never edited or deleted. `correct_entry(target_redni_broj, correct_amount_minor, kolona, basis, ...)`:
1. appends a **reversing crveni storno in the SAME column** = the negation of the target's amount, `kind='error_storno'`, `opis` referencing the target RB;
2. appends the corrected entry (`kind='error_correction'`).
Two new rows bearing the **current** date (never back-dated); the original is untouched. Net effect corrects the running total.

## 5. Commands (`commands/kep.rs`, all `require_admin`, acting id from session, now/today from `utc_now()`)

`kep_export_kalkulacija(kalkulacija_id) -> ExportedFile`; `kep_list_kalkulacije(book_year)`; `kep_nivelacija(product_id, new_sale_price_minor, basis)`; `kep_post_adjustment(cause, product_id, quantity_milli, basis)` (the non-nivelacija causes); `kep_correct_entry(target_redni_broj, correct_amount_minor, basis)`. Register all in `lib.rs`. The receipt kalkulacija generation hooks the existing `apply_inventory_adjustment` receive path (Task in the plan), atomic with the 9a zaduženje.

## 6. UI

The KEP module gains:
- **„Nova izmena"** (adjustment) form: pick the `cause` → the **derived kolona + sign shown read-only** (the shop sees *why* it books where it does), the product + quantity (or, for nivelacija, the new price), and the basis document; a **saldo-effect preview** before posting.
- **„Ispravi stavku"** (reversing storno) action on a ledger row.
- **Kalkulacije** list + „Štampaj kalkulaciju" (SW-8).

Company settings must carry poslovno ime / prodajno mesto / PIB for the kalkulacija header; if PIB is empty, warn (the kalkulacija needs it) but don't block.

## 7. Testing

Memo worked examples verbatim:
- **Kalkulacija** (§3): qty 50, nabavna 100,00, prodajna sa PDV 156,00/jm, 20% PDV → element 13 = **7.800,00**, 11 = 6.500,00, 12 = 1.300,00, 9 = 5.000,00, **10 (marža) = 1.500,00**; and a loss-leader case → negative marža rendered.
- **Storno** (§4.5): upward nivelacija 156→176 on 35 kom → **+700,00 kolona 4**; downward 156→136 → **−700,00 crveni storno kolona 4**; customer contract-rescission return 1 kom → **−156,00 crveni storno kolona 5**; popis višak → kolona 4 +, popis manjak → kolona 5 +.
- **Error correction:** an 8.700 mistyped as element-13 → reversing −8.700 + correct +7.800 (net −900), original row intact.
- **Cause→column map is not overridable**; nivelacija updates `products.sale_price` + records `price_history`; the kalkulacija links to the zaduženje and the zaduženje value is unchanged from 9a; append-only invariant (no update/delete of a posted `kep_entries` row).

## Non-goals
- Campaign→KEP auto-nivelacija (deferred; manual nivelacija only).
- Auto-applying inventory movements from a KEP storno (KEP is a value book; stock is the separate ledger; reconciled at popis in 9c).
- Year-end freeze, full-book print with page mechanics, retention clock (9c).
- Multi-location interna prenosnica (single store).

## Acceptance criteria
- Every receipt generates a 14-element kalkulacija (catalog-price-authoritative, marža derived, loss-leaders allowed) linked to its zaduženje, atomically.
- Every storno event books to the kolona and sign the memo's cause→column map dictates, with its basis document — never user-overridable.
- A nivelacija updates the shelf price and the price log and posts the KEP Δ in one transaction.
- Error correction is a reversing storno + re-entry; no posted row is ever edited or deleted.
- All gates green: `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `git diff --check`.
