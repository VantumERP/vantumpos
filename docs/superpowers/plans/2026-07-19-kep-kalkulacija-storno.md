# KEP Kalkulacija + Storno Engine (SW-9b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The 14-element kalkulacija (generated on each receipt, formalizing SW-9a's zaduženje) and the typed storno engine whose every event books to the kolona and sign the law's cause→column map dictates — never user choice.

**Architecture:** Migration v13 adds `kalkulacije` + a `kep_entries.cause` column. A pure `kep_kalkulacija::derive_kalkulacija` computes the 14 elements backward from the catalog price; the receipt hook generates the kalkulacija atomically with the 9a zaduženje and links them. `kep_storno` holds the cause→{kolona,sign,kind} map and the post functions; nivelacija additionally updates the shelf price + `price_history`. Error correction is a reversing storno. Thin admin-gated commands; HTML kalkulacija via SW-6c/SW-8.

**Tech Stack:** Rust + rusqlite; Tauri v2 commands; React + TS + shadcn/ui + Vitest/RTL.

## Global Constraints

- **Legal authority is `docs/KEP-VERIFIED-RULES.md`** (§3 kalkulacija, §4 storno); design is `docs/superpowers/specs/2026-07-19-kep-kalkulacija-storno-design.md`. If code and those disagree, STOP and report — never improvise law. A wrong booking corrupts the saldo silently.
- **DO NOT boot, launch, or run the application** (no `tauri dev`, dev/preview server, binary). Verify only via `cargo test` / `cargo build` / `bun run test` / `bun run build` / clippy / fmt. Standing user instruction.
- **`kep_entries` stays append-only.** No UPDATE/DELETE of a posted row anywhere; corrections are reversing storno. (Reset's sole DELETE is untouched.)
- **cause→{kolona, sign, kind} is a hard map, never user-overridable.** A crveni storno is stored as a **negative `amount_minor`** (subtracts from the column total).
- **Kalkulacija is catalog-price-authoritative; marža derived backward** and **may be negative** (loss-leader) — store and render it, never reject.
- **Nivelacija is the only storno that changes the product price** — it updates `products.sale_price_minor` + records `price_history` (`record_offered_price_change`, source `'update'`) + posts the KEP Δ, all in one transaction. Other storno causes post a KEP **value entry only** (no inventory movement — the KEP is a value book; avoiding the double-post that routing a popis-višak through the receive path would cause).
- Money in integer minor units; quantities milli. `rate_basis_points`: 20% PDV = 2000. Never floats.
- Serbian Latin copy with correct diacritics (šđčćž) — exact strings from the plan/memo.
- Commands `require_admin`; acting id from the session; `now`/`today` from `crate::clock::utc_now()`.
- Migrations append-only: v13 next; count assertion 12 → 13; extend `CORE_TABLES` / `EXPLICIT_INDEXES`.
- No document resembles a fiscal receipt (no QR/PIB/brojač) — SW-1 rule.
- Every task ends green on: `bun run test`, `bun run build`, `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`.
- Commit trailer:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  ```

---

### Task 1: Migration v13 — `kalkulacije` + `kep_entries.cause`

**Files:** Modify `src-tauri/src/db/migrations.rs` (append v13), `src-tauri/src/db/mod.rs` (`CORE_TABLES`, `EXPLICIT_INDEXES`, count ~line 784). Test: `src-tauri/src/db/mod.rs`.

**Interfaces:** Produces the `kalkulacije` table (spec §1 columns) + nullable `kep_entries.cause`. Consumed by Tasks 2–8.

- [ ] **Step 1: Failing test** — assert `kalkulacije` table exists with `razlika_u_ceni_minor` + `prodajna_vrednost_sa_pdv_minor` columns; assert `kep_entries` has a `cause` column; bump the count test 12 → 13.

```rust
    #[test]
    fn migration_v13_adds_kalkulacije_and_cause() {
        with_test_database("migration_v13_kalkulacije", |db| {
            let connection = db.open().expect("db open");
            assert!(schema_object_exists(&connection, "table", "kalkulacije"), "kalkulacije table");
            for col in ["razlika_u_ceni_minor", "prodajna_vrednost_sa_pdv_minor", "nabavna_cena_po_jm_minor"] {
                let exists: i64 = connection.query_row(
                    "SELECT COUNT(*) FROM pragma_table_info('kalkulacije') WHERE name = ?1", params![col], |r| r.get(0)).expect("meta");
                assert_eq!(exists, 1, "kalkulacije.{col}");
            }
            let cause: i64 = connection.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('kep_entries') WHERE name = 'cause'", [], |r| r.get(0)).expect("meta");
            assert_eq!(cause, 1, "kep_entries.cause");
        });
    }
```

- [ ] **Step 2: FAIL** → `cargo test --manifest-path src-tauri/Cargo.toml migration_v13 -- --test-threads=1`.
- [ ] **Step 3: Append v13** (after the v12 entry):

```rust
    Migration {
        version: 13,
        name: "kep_kalkulacije",
        sql: r#"
CREATE TABLE kalkulacije (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    redni_broj INTEGER NOT NULL,
    book_year INTEGER NOT NULL,
    product_id INTEGER NOT NULL REFERENCES products(id),
    poslovno_ime TEXT NOT NULL,
    prodajno_mesto TEXT NOT NULL,
    pib TEXT NOT NULL,
    trgovacki_naziv TEXT NOT NULL,
    jedinica_mere TEXT NOT NULL,
    kolicina_milli INTEGER NOT NULL,
    nabavna_cena_po_jm_minor INTEGER NOT NULL,
    vrednost_po_fakturi_minor INTEGER NOT NULL,
    razlika_u_ceni_minor INTEGER NOT NULL,
    prodajna_vrednost_bez_pdv_minor INTEGER NOT NULL,
    pdv_minor INTEGER NOT NULL,
    prodajna_vrednost_sa_pdv_minor INTEGER NOT NULL,
    prodajna_cena_po_jm_minor INTEGER NOT NULL,
    reference_type TEXT,
    reference_id INTEGER,
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    UNIQUE (book_year, redni_broj)
);
CREATE INDEX idx_kalkulacije_product ON kalkulacije(product_id, created_at);

ALTER TABLE kep_entries ADD COLUMN cause TEXT;
"#,
    },
```

- [ ] **Step 4: Register** — `"kalkulacije",` in `CORE_TABLES`; `"idx_kalkulacije_product",` in `EXPLICIT_INDEXES`.
- [ ] **Step 5: DB tests + full gates** → PASS. **Step 6: Commit**

```bash
git add src-tauri/src/db/migrations.rs src-tauri/src/db/mod.rs
git commit -m "feat(kep): migration v13 — kalkulacije + kep_entries.cause (SW-9b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: `derive_kalkulacija` — the 14 elements, backward

**Files:** Create `src-tauri/src/kep_kalkulacija.rs`; modify `src-tauri/src/lib.rs` (`mod kep_kalkulacija;`, alphabetical). Test: same file.

**Interfaces:**
```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KalkulacijaElements {
    pub vrednost_po_fakturi_minor: i64,       // 9
    pub razlika_u_ceni_minor: i64,            // 10 (signed)
    pub prodajna_vrednost_bez_pdv_minor: i64, // 11
    pub pdv_minor: i64,                        // 12
    pub prodajna_vrednost_sa_pdv_minor: i64,  // 13
}
pub fn derive_kalkulacija(kolicina_milli: i64, nabavna_po_jm_minor: i64, prodajna_po_jm_minor: i64, rate_basis_points: i64) -> KalkulacijaElements;
```
Math: `13 = round_div(kolicina_milli * prodajna_po_jm_minor, 1000)`; `11 = round_div(13 * 10000, 10000 + rate_basis_points)`; `12 = 13 - 11`; `9 = round_div(kolicina_milli * nabavna_po_jm_minor, 1000)`; `10 = 11 - 9`. Provide `fn round_div(a: i64, b: i64) -> i64 { (a + b / 2) / b }` (b > 0).

- [ ] **Step 1: Failing tests — memo §3 worked example + loss-leader:**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn worked_example_kalkulacija_50_kom() {
        // qty 50 kom = 50_000 milli; nabavna 100,00 = 10000; prodajna sa PDV 156,00/jm = 15600; PDV 20% = 2000 bp.
        let e = derive_kalkulacija(50_000, 10_000, 15_600, 2000);
        assert_eq!(e.prodajna_vrednost_sa_pdv_minor, 780_000, "13 = 7.800,00");
        assert_eq!(e.prodajna_vrednost_bez_pdv_minor, 650_000, "11 = 6.500,00");
        assert_eq!(e.pdv_minor, 130_000, "12 = 1.300,00");
        assert_eq!(e.vrednost_po_fakturi_minor, 500_000, "9 = 5.000,00");
        assert_eq!(e.razlika_u_ceni_minor, 150_000, "10 marža = 1.500,00");
    }
    #[test]
    fn negative_marza_is_allowed_loss_leader() {
        // nabavna 200,00 > prodajna 156,00 sa PDV → marža negative.
        let e = derive_kalkulacija(1_000, 20_000, 15_600, 2000);
        assert!(e.razlika_u_ceni_minor < 0, "loss-leader marža is negative, not rejected");
        assert_eq!(e.prodajna_vrednost_bez_pdv_minor + e.pdv_minor, e.prodajna_vrednost_sa_pdv_minor, "11+12=13 exact, no drift");
    }
}
```

- [ ] **Step 2: FAIL → Step 3: implement + `mod kep_kalkulacija;` → Step 4: PASS → Step 5: gates → Step 6: commit:**

```bash
git add src-tauri/src/kep_kalkulacija.rs src-tauri/src/lib.rs
git commit -m "feat(kep): derive_kalkulacija — 14 elements backward from the catalog price (SW-9b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Kalkulacija persistence + generation on receipt

**Files:** Modify `src-tauri/src/kep_kalkulacija.rs` (persist), `src-tauri/src/kep.rs` (`post_receipt_zaduzenje` gains a `reference_type: &str`), `src-tauri/src/commands/inventory.rs` (the receipt hook ~line 490–500). Test: `src-tauri/src/commands/inventory.rs`.

**Interfaces:**
- Consumes: `derive_kalkulacija` (T2); company settings; the product's name/unit/sale_price/tax rate.
- Produces: `kep_kalkulacija::create_kalkulacija(tx: &Transaction, product_id, kolicina_milli, nabavna_po_jm_minor, reference_type: Option<&str>, reference_id: Option<i64>, acting: i64, now: &str) -> Result<i64, AppError>` (returns the new kalkulacija id; allocates `redni_broj = MAX+1` per book_year; loads company settings + product for the elements). `post_receipt_zaduzenje` signature gains `reference_type: &str` (was hardcoded `'inventory_movement'`).

Semantics: in the receipt hook, **before** posting the zaduženje, call `create_kalkulacija` to get its id, then call `post_receipt_zaduzenje(..., reference_type = "kalkulacija", reference_id = Some(kalkulacija_id), ...)` — the kalkulacija is the linking isprava. All in the existing `apply_inventory_adjustment` transaction (atomic). The zaduženje's amount is unchanged (still `qty × sale_price` = element 13). `create_kalkulacija` reads the product's `sale_price_minor` (element 14) + its tax `rate_basis_points` and calls `derive_kalkulacija`.

- [ ] **Step 1: Failing test** — a receive produces exactly one `kalkulacije` row whose `prodajna_vrednost_sa_pdv_minor` equals the receipt's `qty × sale_price` (memo 780000 for 50 kom @ 156,00), and the `kep_entries` receipt row's `reference_type='kalkulacija'` with `reference_id` = that kalkulacija's id. Corrections/write-offs create no kalkulacija.
- [ ] **Step 2: FAIL → Step 3: implement** (widen `post_receipt_zaduzenje`; add `create_kalkulacija`; wire the hook). Update the one existing `post_receipt_zaduzenje` call in inventory.rs to pass `"kalkulacija"`. → **Step 4: PASS → Step 5: gates → Step 6: commit:**

```bash
git add src-tauri/src/kep_kalkulacija.rs src-tauri/src/kep.rs src-tauri/src/commands/inventory.rs
git commit -m "feat(kep): generate the kalkulacija on receipt, atomic with the zaduženje (SW-9b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Kalkulacija HTML document + export

**Files:** Modify `src-tauri/src/kep_kalkulacija.rs` (render + a view loader). Test: same file.

**Interfaces:**
- Produces: `pub fn load_kalkulacija(conn, id) -> Result<KalkulacijaView, AppError>` (all columns, camelCase Serialize); `pub fn render_kalkulacija_html(view: &KalkulacijaView) -> String`. Reuse the SW-6c renderer idioms (`<!doctype html>`, inline `<style>`, `escape_html`, `format_rsd_minor`, no `<script>`, no QR/PIB/brojač, non-fiscal footer).

Content: header „Kalkulacija cene" with poslovno ime / prodajno mesto / PIB / redni broj / datum; a table of all 14 elements labelled in Serbian (Trgovački naziv, Jedinica mere, Količina, Nabavna cena/jm, Vrednost po fakturi, Razlika u ceni (marža), Prodajna vrednost bez PDV, PDV, Prodajna vrednost sa PDV, Prodajna cena/jm); footer „Interni dokument. Nije fiskalni dokument."

- [ ] **Step 1: Failing tests** — `render_kalkulacija_html` contains „Kalkulacija cene", the escaped product name, „7.800,00" and „1.500,00", starts `<!doctype html>`, no `<script>`; a negative marža renders with a sign. **Step 2: FAIL → Step 3: implement → Step 4: PASS → Step 5: gates → Step 6: commit:**

```bash
git add src-tauri/src/kep_kalkulacija.rs
git commit -m "feat(kep): kalkulacija HTML isprava (SW-9b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Storno engine — the cause→column map + non-nivelacija causes

**Files:** Create `src-tauri/src/kep_storno.rs`; modify `src-tauri/src/lib.rs` (`mod kep_storno;`). Test: same file.

**Interfaces:**
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StornoCause {
    NivelacijaUp, NivelacijaDown, PdvRateUp, PdvRateDown,
    SupplierReturn, CustomerReturn, Otpis, ManjakOdluka, Rashod, PopisVisak, PopisManjak,
}
pub struct Posting { pub kolona: &'static str, pub kind: &'static str, pub cause: &'static str, pub negative: bool }
pub fn posting_for(cause: StornoCause) -> Posting;   // the HARD map — the single source of column/sign/kind
pub struct BasisDoc { pub naziv: String, pub broj: String, pub datum: String }  // serde camelCase Deserialize
pub fn post_value_storno(tx: &Transaction, cause: StornoCause, product_id: i64, quantity_milli: i64, basis: &BasisDoc, acting: i64, now: &str) -> Result<(), AppError>;
```

`posting_for` (the map, tested exhaustively):

| cause | kolona | kind | negative |
|---|---|---|---|
| NivelacijaUp / PdvRateUp | zaduzenje | nivelacija_up | false |
| NivelacijaDown / PdvRateDown | zaduzenje | nivelacija_down_storno | true |
| SupplierReturn | zaduzenje | supplier_return_storno | true |
| Otpis / ManjakOdluka / Rashod | zaduzenje | nivelacija_down_storno | true |
| CustomerReturn | razduzenje | customer_return_storno | true |
| PopisVisak | zaduzenje | popis_visak | false |
| PopisManjak | razduzenje | popis_manjak | false |

`post_value_storno` (for every cause **except** the two nivelacija causes, which change price — Task 6): amount = `round_div(quantity_milli × product.sale_price_minor, 1000)`, stored **negative** when `posting.negative`; `opis` = `"{basis.naziv} br. {basis.broj} od {basis.datum}"`; `cause` column = `posting.cause`; `redni_broj = next_redni_broj`; append-only INSERT. (Nivelacija/PdvRate causes are rejected here with `AppError::business("invalid_state", "Nivelacija menja cenu — koristite nivelaciju.")` so a caller can't post them value-only.)

- [ ] **Step 1: Failing tests — memo §4.5 worked examples:**

```rust
    // supplier return / otpis 35 kom @ 156,00 -> -5.460,00 crveni storno in kolona 4
    #[test]
    fn otpis_books_negative_in_kolona_4() { /* seed product sale_price 15600 + 35 kom; post_value_storno(Otpis, 35_000, ...); assert last kep row kolona='zaduzenje', amount_minor=-546000, kind='nivelacija_down_storno', cause='otpis' */ }
    // customer contract-rescission return 1 kom -> -156,00 crveni storno in kolona 5
    #[test]
    fn customer_return_books_negative_in_kolona_5() { /* post_value_storno(CustomerReturn, 1_000, ...); assert kolona='razduzenje', amount_minor=-15600, kind='customer_return_storno' */ }
    // popis višak 10 kom -> +kolona 4 ; popis manjak 10 kom -> +kolona 5
    #[test]
    fn popis_visak_kolona_4_manjak_kolona_5() { /* two posts; assert columns + positive signs */ }
    #[test]
    fn posting_map_is_exhaustive_and_fixed() {
        assert_eq!(posting_for(StornoCause::CustomerReturn).kolona, "razduzenje");
        assert_eq!(posting_for(StornoCause::Otpis).kolona, "zaduzenje");
        assert!(posting_for(StornoCause::NivelacijaDown).negative);
        assert!(!posting_for(StornoCause::PopisVisak).negative);
    }
    #[test]
    fn nivelacija_causes_are_rejected_by_value_storno() { /* post_value_storno(NivelacijaUp, ...) -> invalid_state */ }
```

- [ ] **Step 2: FAIL → Step 3: implement (+ `mod kep_storno;`) → Step 4: PASS → Step 5: gates → Step 6: commit:**

```bash
git add src-tauri/src/kep_storno.rs src-tauri/src/lib.rs
git commit -m "feat(kep): typed storno engine — cause→kolona map + value entries (SW-9b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: Nivelacija — price change + KEP Δ in one transaction

**Files:** Modify `src-tauri/src/kep_storno.rs`. Test: same file.

**Interfaces:**
- Consumes: `posting_for`; `price_history::{load_offering_state, record_offered_price_change, OfferingState}`; `inventory_balances` for on-hand qty.
- Produces: `pub fn post_nivelacija(tx: &Transaction, product_id: i64, new_sale_price_minor: i64, basis: &BasisDoc, acting: i64, now: &str) -> Result<(), AppError>`.

Semantics (one transaction): load old `sale_price_minor` + on-hand `quantity_milli` (`inventory_balances`, default 0). `delta_per_unit = new − old`; `amount = round_div(on_hand_milli × delta_per_unit.abs(), 1000)`. Update `products.sale_price_minor = new` (+ `updated_at`); record the price change via `record_offered_price_change(tx, product_id, before, OfferingState { active: <current>, price_minor: new }, "update", Some(acting), now)`. Post the KEP entry: kolona `zaduzenje`; `kind` = `nivelacija_up` if `new > old` else `nivelacija_down_storno`; `amount_minor` = `amount` (negative when down); `cause = "nivelacija"`; opis from `basis`. Reject `new == old` (`validation`, "Nova cena je jednaka staroj."). No-op amount when on-hand is 0 is allowed (price still updates; the KEP Δ is 0 — still post a 0 row? No — if on-hand is 0 there is no value to revalue; skip the KEP entry but still update the price, and note it in the return).

- [ ] **Step 1: Failing tests — memo §4.5:**

```rust
    #[test]
    fn upward_nivelacija_books_plus_700_in_kolona_4_and_updates_price() {
        // seed product sale_price 15600, on-hand 35 kom; post_nivelacija(new=17600); 35 * 2000 /1000... = +700,00
        // assert kep row kolona='zaduzenje', amount_minor=70000, kind='nivelacija_up';
        // assert products.sale_price_minor now 17600; assert a price_history 'update' row exists.
    }
    #[test]
    fn downward_nivelacija_books_negative_storno() {
        // new=13600 from 15600 on 35 kom -> -700,00 crveni storno; kind='nivelacija_down_storno'; amount_minor=-70000.
    }
```

- [ ] **Step 2: FAIL → Step 3: implement → Step 4: PASS → Step 5: gates → Step 6: commit:**

```bash
git add src-tauri/src/kep_storno.rs
git commit -m "feat(kep): nivelacija updates price + price_history + KEP delta atomically (SW-9b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: Error correction — reversing storno

**Files:** Modify `src-tauri/src/kep.rs` (or `kep_storno.rs`). Test: same file.

**Interfaces:**
- Produces: `pub fn correct_entry(tx: &Transaction, target_redni_broj: i64, book_year: i64, correct_amount_minor: i64, basis: &BasisDoc, acting: i64, now: &str) -> Result<(), AppError>`.

Semantics: load the target row (`book_year`, `redni_broj`) — its `kolona` + `amount_minor`. Append a reversing entry: same `kolona`, `amount_minor = -target.amount_minor`, `kind='error_storno'`, `cause='ispravka'`, `opis` referencing the target RB („Storno stavke RB {n}"). Then append the corrected entry: same `kolona`, `amount_minor = correct_amount_minor`, `kind='error_correction'`, opis from `basis`. Both bear `now` (current date; never back-dated). The original row is untouched (no UPDATE/DELETE). Reject a missing target (`not_found`).

- [ ] **Step 1: Failing test** — seed a receipt zaduženje of 870000 (mistyped); `correct_entry(target, 780000)`; assert the target row unchanged, a `-870000` error_storno in the same kolona, a `+780000` error_correction, and the derived saldo net effect is `−90000` vs the erroneous state. **Step 2: FAIL → Step 3: implement → Step 4: PASS → Step 5: gates → Step 6: commit:**

```bash
git add src-tauri/src/kep.rs
git commit -m "feat(kep): reversing-storno error correction (SW-9b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 8: Command layer + registration

**Files:** Modify `src-tauri/src/commands/kep.rs`, `src-tauri/src/lib.rs`. Test: `src-tauri/src/commands/kep.rs`.

**Interfaces (all `require_admin`, acting id from session, now from `utc_now()`):**
- `kep_list_kalkulacije(book_year) -> Vec<KalkulacijaSummary>`; `kep_export_kalkulacija(id) -> ExportedFile` (writes `kalkulacija-{redni_broj}.html` to `exports/`, reusing the campaigns export write helper); `kep_nivelacija(product_id, new_sale_price_minor, basis)`; `kep_post_adjustment(cause: String, product_id, quantity_milli, basis)` (maps the string to `StornoCause`, rejects unknown; nivelacija causes rejected here — use `kep_nivelacija`); `kep_correct_entry(target_redni_broj, book_year, correct_amount_minor, basis)`. Each domain call opens `state.db()`, runs in a transaction, commits. Register all in `lib.rs`.

- [ ] **Step 1: Failing tests** — cashier → `forbidden` on `kep_nivelacija` and `kep_post_adjustment`; an admin happy path: `kep_post_adjustment("otpis", product, 35_000, basis)` appends a negative kolona-4 row; `kep_export_kalkulacija` writes a file containing the product name. **Step 2: FAIL → Step 3: implement + register → Step 4: PASS → Step 5: gates → Step 6: commit:**

```bash
git add src-tauri/src/commands/kep.rs src-tauri/src/lib.rs
git commit -m "feat(kep): admin-gated kalkulacija/nivelacija/storno/correction commands (SW-9b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 9: Frontend service contract

**Files:** Modify `src/services/types.ts`, `src/services/ports.ts`, `src/services/local-adapter.ts`, `src/services/mock-adapter.ts`. Test: `src/services/local-adapter.test.ts`.

**Interfaces:** TS DTOs mirroring the Rust structs (camelCase); `StornoCauseId = "supplier_return" | "customer_return" | "otpis" | "manjak_odluka" | "rashod" | "popis_visak" | "popis_manjak"` (nivelacija has its own method); `BasisDoc = { naziv, broj, datum }`. `KepService` gains: `listKalkulacije(bookYear)`, `exportKalkulacija(id)`, `nivelacija(productId, newSalePriceMinor, basis)`, `postAdjustment(cause, productId, quantityMilli, basis)`, `correctEntry(targetRedniBroj, bookYear, correctAmountMinor, basis)` — added to the existing `KepService` (from 9a). Local adapter maps 1:1 to the command names; mock adapter stubs them. Every `PosServices`/mock construction stays type-complete.

- [ ] Failing adapter-mapping test → FAIL → implement → PASS → full gates → commit:

```bash
git add src/services
git commit -m "feat(kep): frontend contract for kalkulacija/nivelacija/storno (SW-9b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 10: KEP module UI — adjustment form + correction + kalkulacija print

**Files:** Modify `src/app/kep/KepModule.tsx`, `src/app/kep/KepModule.test.tsx`.

**Interfaces:** Consumes the `KepService` surface (T9) + `PrintService.openForPrint` (SW-8).

Content:
- **„Nova izmena"** form: a `cause` select (Serbian labels: `Nivelacija naviše`, `Nivelacija naniže`, `Povraćaj dobavljaču`, `Povraćaj kupca (raskid ugovora)`, `Otpis`, `Manjak po odluci`, `Rashod`, `Višak po popisu`, `Manjak po popisu`) → the **derived kolona + sign shown read-only** („Knjiži se u kolonu 4 kao zaduženje" / „…kolonu 5" / „crveni storno") so the shop sees why; the product picker; quantity (or, for nivelacija, the new price); the basis document (naziv/broj/datum). A **saldo-effect line** („Efekat na saldo: −5.460,00") before posting. Nivelacija routes to `nivelacija`; the rest to `postAdjustment`.
- **„Ispravi stavku"** action on a ledger row → a dialog (correct amount + basis) calling `correctEntry`.
- **Kalkulacije**: a list with „Štampaj kalkulaciju" → `runPrint(() => exportKalkulacija(id))` (the SW-8 export-then-open helper).

Tests (RTL, mock services): selecting `otpis` shows „kolonu 4" + „crveni storno" read-only and posting calls `postAdjustment("otpis", …)`; selecting a nivelacija shows the price field and calls `nivelacija`; „Ispravi stavku" calls `correctEntry`; „Štampaj kalkulaciju" exports then opens.

- [ ] Failing tests → FAIL → implement → PASS → full gates → commit:

```bash
git add src/app/kep
git commit -m "feat(kep): adjustment form (cause-derived kolona) + correction + kalkulacija print (SW-9b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Self-Review Notes

- **Spec coverage:** §1 schema → T1; §2 kalkulacija (derive/generate/print) → T2/T3/T4; §3 storno map + value causes → T5, nivelacija → T6; §4 error correction → T7; §5 commands → T8; §6 UI → T10; §7 tests → worked examples in T2 (kalkulacija), T5/T6 (storno), T7 (correction).
- **Type consistency:** `KalkulacijaElements`, `StornoCause`/`Posting`/`BasisDoc`, `posting_for`, `post_value_storno`, `post_nivelacija`, `correct_entry` defined once (T2/T5/T6/T7), mirrored in T9; command names in T8 = invoke names in T9. `round_div` defined in T2, reused by T5/T6.
- **Append-only preserved:** every storno/correction is an INSERT; `correct_entry` never touches the target row; the only UPDATE is `products.sale_price_minor` in nivelacija (T6), not `kep_entries`.
- **No double-post:** storno causes post value entries only; nivelacija updates price (not stock); no cause routes through the receive path (which would fire the 9a zaduženje). The receipt hook (T3) is the sole zaduženje source.
- **No boot:** derivations/renderers/postings are pure or DB-only; UI is RTL-tested; nothing needs the app running.
