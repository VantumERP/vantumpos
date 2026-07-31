# SW-11 + SW-15 — Cash, Product & Onboarding Compliance — Design

**Date:** 2026-07-31 · **Status:** approved (design) · **Verified law:** [SW11-SW15-VERIFIED-RULES.md](../../SW11-SW15-VERIFIED-RULES.md)

Four semi-independent subsystems shipped in one cycle, in dependency order:

| Phase | Deliverable | Depends on |
|---|---|---|
| **A — SW-15** | Shop profile + `legal.rs` (single source of every fine figure) + CI guard | — |
| **B — SW-11a** | AML cash-cap assessment + NBS rate + `bank_transfer` tender | A (`pravna_forma`) |
| **C — SW-11c** | Declaration data on products + goods-receipt check | A (`distance_selling`) |
| **D — SW-11b** | Deposit aging (FIFO buckets, 7 radni dani) | — |

**Every requirement below traces to a numbered item in §3 of the verified rule set.** Requirements tagged `[PRUDENTIAL]` there must never be presented to the operator as a legal duty; that distinction is load-bearing and is asserted in tests.

---

## 0. Why phase A comes first

Verification found a **systemic** defect in the compliance report: four separate fine figures were the *pravno lice* tier copied onto a shop that is a **preduzetnik**, which cannot commit a privredni prestup at all (ZPP čl. 6 st. 1). Those were corrected in commit `3d5aede`.

The corrections are worthless if the next feature hard-codes a constant. Phase A therefore builds the mechanism that makes the class of error unrepresentable *before* any new legal string enters the app. This is requirement 27.

---

## 1. Migration v15 (single migration, all four phases)

Bumps `migration_count` in `src-tauri/src/db/mod.rs` from 14 to **15**.

### 1.1 `products` — declaration identity (req. 21–24)

```sql
ALTER TABLE products ADD COLUMN manufacturer_name TEXT;
ALTER TABLE products ADD COLUMN importer_name TEXT;
ALTER TABLE products ADD COLUMN country_of_origin TEXT;
ALTER TABLE products ADD COLUMN official_goods_code TEXT;
ALTER TABLE products ADD COLUMN barcode_kind TEXT
    CHECK (barcode_kind IS NULL OR barcode_kind IN ('gtin','internal','none'));
ALTER TABLE products ADD COLUMN declaration_checked_at TEXT;
ALTER TABLE products ADD COLUMN declaration_checked_by INTEGER REFERENCES users(id);
```

`barcode_kind` is **nullable and NOT backfilled**. `NULL` means *unclassified*. We must not assert that an existing `barcode` is a GTIN — req. 24 forbids reporting an in-house code as a GTIN, and a migration cannot know which is which. Classification is an operator action surfaced by a report.

`official_goods_code` ships **unused** (req. 23). No import, no sync, no validation — the jedinstveni šifarnik does not exist and the amending act sets no deadline for its pravilnik. It exists solely so that its arrival is not a painful migration.

`country_of_origin` accepts the literal string `"EU"` (ZoT čl. 34 st. 7) — do **not** validate against an ISO-3166 list alone.

### 1.2 `cash_movements` — rebuild to widen the CHECK (req. 14)

SQLite cannot alter a CHECK, so this is a table rebuild in the style of migration v6:

```sql
CREATE TABLE cash_movements_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    shift_id INTEGER NOT NULL REFERENCES shifts(id) ON DELETE CASCADE,
    movement_type TEXT NOT NULL CHECK (movement_type IN
        ('pay_in','pay_out','bank_deposit','bank_withdrawal')),
    amount_minor INTEGER NOT NULL CHECK (amount_minor > 0),
    reason TEXT,
    bank_reference TEXT,
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
INSERT INTO cash_movements_next (id, shift_id, movement_type, amount_minor, reason, bank_reference, user_id, created_at)
SELECT id, shift_id, movement_type, amount_minor, reason, NULL, user_id, created_at FROM cash_movements;
DROP TABLE cash_movements;
ALTER TABLE cash_movements_next RENAME TO cash_movements;
CREATE INDEX idx_cash_movements_shift ON cash_movements(shift_id);
CREATE INDEX idx_cash_movements_created_at ON cash_movements(created_at);
```

Semantics of the two new types:

- **`bank_deposit`** — cash leaving the till *to the business account*. Behaves as an outflow for expected-cash purposes (like `pay_out`) **and** discharges deposit-aging buckets. `bank_reference` carries the uplatnica/izvod reference.
- **`bank_withdrawal`** — cash entering the till *from the shop's own account* (change float). Behaves as an inflow for expected-cash purposes (like `pay_in`) but is **NOT SUBJECT** to the deposit duty (Pravilnik 77/2011 čl. 5 st. 2).

**Consequence that must not be missed — expected cash.** Both new types are real till movements, so they change the shift's expected cash. The current derivation counts only `pay_in` and `pay_out`, in three places: the shift-summary SQL (`shifts.rs:361–362`) and the two arithmetic sites (`shifts.rs:268`, `shifts.rs:388`). All three must learn the new types:

```
expected_cash = opening + cash_sales + (pay_in + bank_withdrawal) - (pay_out + bank_deposit)
```

Miss this and a shop that deposits its takings mid-shift shows a phantom cash shortfall at close — an anti-theft false positive, in the module whose whole purpose is trustworthy reconciliation. This is its own plan task with a dedicated test, not a footnote to the migration.

**A forward-migration data-survival test is required** (repo convention, and this rebuild touches live money rows): seed v14 `cash_movements` rows, migrate, assert every row survives with its amounts and timestamps intact.

### 1.3 `sale_payments` — rebuild to admit the third tender (req. 4)

Same rebuild shape; CHECK becomes `payment_method IN ('cash','card','bank_transfer')`. Same data-survival test.

### 1.4 `sales` — AML decision provenance (req. 3)

```sql
ALTER TABLE sales ADD COLUMN aml_cash_minor INTEGER;
ALTER TABLE sales ADD COLUMN aml_rate_minor INTEGER;
ALTER TABLE sales ADD COLUMN aml_rate_date TEXT;
ALTER TABLE sales ADD COLUMN aml_rate_source TEXT;
ALTER TABLE sales ADD COLUMN aml_ack_reason TEXT;
```

Written **only when an assessment actually fired** (cash line at or above the soft threshold). All `NULL` on an ordinary sale. The point is that an inspection can reproduce the decision: what cash amount, at what rate, of what date, from what source.

### 1.5 `non_working_days` (req. 13)

```sql
CREATE TABLE non_working_days (
    day TEXT PRIMARY KEY,          -- 'YYYY-MM-DD'
    label TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

Seeded with Serbian state holidays for 2026 and 2027 (fixed dates plus that year's Orthodox Easter Friday/Monday). Admin-editable, because the list is annual and the app must not silently rot. **Not** seeded beyond 2027 — a wrong future holiday is worse than an absent one, and the aging report warns when it computes a deadline past the seeded horizon.

---

## 2. Phase A — SW-15 shop profile + `legal.rs`

### 2.1 `shop_profile` settings key

Stored under the existing key-value `settings` table (same pattern as `company` / `receipt_numbering` / `sales`), new constant `SHOP_PROFILE_KEY = "shop_profile"`.

```rust
pub struct ShopProfile {
    pub pravna_forma: Option<PravnaForma>,   // None = UNSET; NEVER inferred
    pub pdv_obveznik: Option<bool>,
    pub distance_selling: bool,              // default false
    pub lpfr_in_premises: Option<bool>,
    pub esir_elements: Vec<EsirElement>,
}

pub struct EsirElement {
    pub naziv: String,
    pub verzija: String,      // req. 33 — the version is part of the approval
    pub ib: String,
    pub tip: EsirTip,         // ESIR | LPFR
    pub checked_on: Option<String>,
}
```

`pravna_forma: None` is a real state, not a default. Requirement 27 is explicit that it must never be inferred — inferring "probably a preduzetnik" from a 9-digit PIB would reintroduce exactly the guessing that produced the tier errors. Any legal copy requested while it is `None` returns the **tier-neutral** form (see 2.2).

### 2.2 `src-tauri/src/legal.rs` — the only place a fine figure exists

Public surface is a small set of pure functions, each taking `&ShopProfile`:

```rust
pub fn aml_cash_cap(profile: &ShopProfile) -> LegalNotice;
pub fn cash_deposit_duty(profile: &ShopProfile) -> LegalNotice;
pub fn declaration_duty(profile: &ShopProfile) -> LegalNotice;

pub struct LegalNotice {
    pub summary: String,        // what the duty is
    pub penalty: Option<String>,// None when pravna_forma is UNSET
    pub citation: String,       // article, for the operator to hand an inspector
    pub is_legal_duty: bool,    // false for [PRUDENTIAL] items — drives UI framing
}
```

Rules the module encodes, each from §2 of the verified set:

- **preduzetnik** → AML `100.000–300.000 RSD (čl. 120 st. 2), a u srazmeri s vrednošću robe i više`. Never the word *privredni prestup*. Never 300.000 framed as a ceiling (req. 6).
- **pravno lice** → AML `privredni prestup 100.000–2.000.000 (čl. 118 st. 1 tač. 43)` + odgovorno lice tier.
- **UNSET** → `penalty: None`; UI shows "Unesite pravnu formu radnje da bi se prikazao tačan iznos" and links to the profile tab. Silence beats a wrong number.
- Declaration: the **two-state** model (req. 26) — *deklaracija odsutna* (50.000–500.000 + possible 6mo–2yr ban) vs *deklaracija neuredna* (fixed 40.000). **No figure at all for a bare missing GTIN** — that tier is genuinely unresolved (§5 Q-7 / master §6 item 16).

`is_legal_duty: false` items render with a distinct, non-alarming treatment and the word *preporuka*, never *obaveza*.

### 2.3 The CI guard (req. 28)

A Rust test, not a lint: iterate **every** public function in `legal.rs` under a `preduzetnik` profile and assert the rendered `summary + penalty + citation` contains none of the forbidden substrings — `privredni prestup`, `2.000.000`, `2M`. A second case asserts the UNSET profile yields `penalty: None` everywhere. The test enumerates the functions explicitly so that adding a new copy function without adding it to the guard is a visible omission in review.

Requirement 29's broader lint (flag any preduzetnik constant above the 500.000/150.000 shapes) is **deliberately out of scope** — it needs a project-wide string scan, and the enumerated guard covers the only module allowed to hold such constants.

### 2.4 UI

New **"Radnja / Profil"** section on the existing Settings screen (`SettingsScreen.tsx` already has the hand-rolled tab bar; follow it rather than introducing shadcn `Tabs` mid-file). Contents:

- Pravna forma (radio, no preselection), PDV status, prodaja na daljinu.
- ESIR/PFR panel labelled **"ZF čl. 6 st. 8: proveriti pre otpočinjanja korišćenja"**, with plain text stating that **no law requires the shop to store or produce these identifiers** and that this row is an **interna beleška o proveri**, never an *evidencija* (req. 33). Deep link to the PURS Registar, plus the check recipe: match naziv + verzija + IB, confirm the "rešenje o ukidanju" cell is empty. Re-confirmation prompt when `checked_on` is older than a year.
- Per-premises L-PFR assertion with the two čl. 6 st. 4 carve-out questions (req. 32).

**Hard rule, asserted by test (req. 41):** ESIR identifiers must not appear in any sale-itemizing output. A test greps the receipt/KEP/kalkulacija render paths for the profile fields.

---

## 3. Phase B — SW-11a AML cash cap

### 3.1 Evaluation

> **Correction as shipped (31.07.2026).** This section originally said the assessment was
> *"evaluated in `sales_preview`"*. It is not, and could not be: `SaleDraftRequest` carries only
> `items` + `receiptDiscount` — **no tender split** — so a preview has nothing to assess. Widening it
> would have made every preview a payment question. The shipped design is instead a **dedicated
> `sales_assess_cash_payment(cash_minor)` command** (`commands/sales.rs`), called by the till whenever
> the **cash input changes** — which still puts the verdict in front of the cashier *before* the money
> changes hands, the property the original wording was reaching for. `sales_complete` re-evaluates
> server-side and persists the provenance (`assess_sale_cash`), so the warning cannot be dodged by a
> client that never asked.

Evaluated by `sales_assess_cash_payment` as the cash line is entered (so the cashier sees it **before**
tendering) and re-evaluated — and persisted — in `sales_complete`.

```rust
pub struct AmlAssessment {
    pub cash_minor: i64,
    pub threshold_minor: i64,           // 10_000 EUR at the applied rate
    pub fallback_threshold_minor: i64,  // stand-in cap, may only warn, never assert a breach
    pub breached: bool,                 // cash_minor >= threshold_minor
    pub near_threshold: bool,           // soft band: >= soft_minor, < threshold_minor
    pub rate_unavailable: bool,         // no rate cached: the check could not run
    pub rate: Option<EurRate>,          // None exactly when rate_unavailable
    pub notice: LegalNotice,
}
```

- Input is the **cash line of the payment split only** (req. 1). A 15.000 € sale settled 5.000 cash + 10.000 card is not a breach on the face of čl. 46 st. 1.
- Operator is **`>=`** (req. 2). A dedicated boundary test asserts that *exactly* the threshold breaches.
- A **soft threshold** (default 80% of the cap, configurable) triggers a non-blocking informational state and the optional buyer tag; only the hard threshold shows the breach state.

### 3.2 The rate — NBS with a manual fallback (req. 3)

Fetched from the NBS public rate source, mirroring the Open Food Facts pattern already in `catalog.rs`: `ureq` agent with an explicit short timeout, and — critically — **the parse split out as a pure function** so it is unit-tested with fixture bodies and never needs the network.

```rust
fn fetch_nbs_middle_rate(on: &str) -> Result<EurRate, AppError>;   // network, not unit-tested
pub fn parse_nbs_middle_rate(body: &str, on: &str) -> Result<EurRate, AppError>;  // pure, fully tested
```

`EurRate { rate_minor, rate_date, source: Nbs | Manual }` cached under settings key `eur_rate`.

Behaviour, in order:
1. Use today's cached NBS rate if present.
2. Otherwise attempt a fetch (bounded timeout, admin-triggered or on first assessment of the day).
3. On failure, fall back to the newest cached rate — NBS or manual — and render a **staleness warning** naming the rate's date.
4. If no rate exists at all, the assessment is **unavailable**: show that the check could not run and offer manual entry. **Never** silently pass and never block the sale.

A failed fetch must never block a sale, a shift, or a day-close. The shop is local-first; the network is a convenience.

### 3.3 The lawful alternative (req. 4)

New tender **`bank_transfer`** — "Uplata na tekući račun prodavnice". čl. 46 st. 1 does not merely prohibit, it commands that the amount *"mora uplatiti na račun otvoren kod banke"*, and st. 3 confirms a buyer's deposit into the shop's account is not "receiving cash". Without this the warning is a dead end and the cash is taken anyway.

Verified blast radius:

| Consumer | Effect | Action |
|---|---|---|
| `expected_cash` (`shifts.rs:268`, `:388`) | Correct as-is **for the tender** — the SQL matches `'cash'` explicitly, so a `bank_transfer` sale is rightly not expected in the till. (Separately, the two new *cash-movement* types **do** change it — see §1.2.) | Add a regression test pinning this |
| `query_payment_methods` (`reports.rs:497`) | Already generic (`GROUP BY payment_method`, `ELSE 2` ordering) | Extend the ordering arm; test |
| KEP daily razduženje | Sums **total** daily promet, not per method — legally correct | Add a test proving a bank-transfer sale is included |
| `reports_daily_turnover` (`reports.rs:331,343,408`) | **Regression** — hard-codes `cash_minor`/`card_minor`; a third method would be invisible | Own task: add the third bucket end-to-end (Rust → types → adapters → UI) |

### 3.4 Soft block + reason (req. 8, `[PRUDENTIAL]`)

On breach: a confirm step requiring a typed reason, recorded in `aml_ack_reason` and in the compliance log. **Not** a hard block — the statutory ban binds the shop regardless of what the software allows, and a hard block would strand a legitimate mixed-tender sale. Framed as a warning about the shop's own exposure, with the tier-correct figure from `legal.rs`.

### 3.5 Buyer aggregation (req. 5)

Per-sale check ships now. The rolling **365-day** (not calendar-year) aggregate ships as an **optional** buyer tag on cash sales above the soft threshold. The settings panel states in writing that **untagged sales cannot be aggregated by the software and the legal duty binds regardless** — the honest limitation, and the reason we do not build a customers table (master §6 item 9: doing so without a lawful ZZPL basis creates its own exposure).

---

## 4. Phase C — SW-11c declaration data

### 4.1 Where the check lives

At **goods receipt**, not checkout (req. 25). The retailer's liability is a *selling* offence, so intake is where the decision actually is; and a timestamped intake record is concrete mitigation under the new ZoT čl. 69a tač. 4 (a mitigating circumstance in sentencing — **not** a defence, and the copy must not promise otherwise).

`inventory_receive` warns — never blocks — when a received product lacks `manufacturer_name` / `country_of_origin` (and `importer_name` where origin is outside RS). The operator can record the check, setting `declaration_checked_at` / `declaration_checked_by`.

### 4.2 The `distance_selling` switch

- `distance_selling = false` → fields are `[PRUDENTIAL]`. Advisory warning only. Copy must not call them a legal duty.
- `distance_selling = true` → fields become **required** for any active product (ZoT čl. 34 st. 5, strengthened 1.5.2026: the trgovac must himself *istaknuti deklaraciju* and keep the st. 1 data continuously available pre-purchase). Save-time validation on the product form; a blocking-severity item in the readiness report.

One flag, both regimes, no later migration — which is what "još ne, ali se planira" requires.

### 4.3 Reports

A **"Artikli bez podataka deklaracije"** report (missing identity fields, and `barcode_kind IS NULL` as a separate *unclassified* bucket). This is also the surface where the operator reclassifies barcodes; GTIN-8/12/13/14 length + check-digit validation runs **only** for `barcode_kind = 'gtin'` (req. 24).

---

## 5. Phase D — SW-11b deposit aging

### 5.1 Buckets

A bucket is one **trading date's** subject cash. Sources (req. 10):

```
subject_in(date) = SUM(sale_payments.amount_minor WHERE payment_method='cash' AND sale on date)
                 + SUM(cash_movements WHERE movement_type='pay_in'      AND on date)
                 - SUM(cash_movements WHERE movement_type='pay_out'     AND on date)   -- till-funded spend
```

`bank_withdrawal` is **excluded from the subject base entirely** — it is the owner's own float returning (Pravilnik 77/2011 čl. 5 st. 2). Without this exclusion the app would age the float as undeposited pazar and generate false violations; this is the single defect the verification pass caught in this phase.

The exclusion is **bylaw-level relief** and the UI labels it as such: the statute's `"po bilo kom osnovu"` and its penalty contain no exclusion, and čl. 8 st. 2 preserves the Pravilnik only *"ukoliko nije u suprotnosti sa ovim zakonom"*. The app may rely on it; it must not present it as settled. (§5 Q-6 is open on whether till-funded expenses reduce the duty at all — the `pay_out` subtraction above is the practice-tolerant reading and is flagged in the report footer.)

Aggregation is **per trading date as an implementation convention**, and the UI says so (req. 11) — the statute keys to receipt of the cash, and mentions no dnevni izveštaj or Z-report anywhere.

### 5.2 Drawdown and deadline

- `bank_deposit` movements discharge the **oldest open bucket first** (FIFO). Partial deposits are lawful; never all-or-nothing (req. 12).
- `deadline(bucket) = receipt_date + 7 radni dani`, computed against `non_working_days`, displayed as a **date** — not a countdown.
- `saturday_is_working_day` setting, **default true** (req. 13): counting Saturdays yields the earlier, conservative deadline. "Radni dan" is statutorily undefined, and the tooltip says so.

### 5.3 Surface

Advisory banner + a printable/CSV **"Izveštaj o nedeponovanom gotovom novcu"** for the knjigovođa (req. 17): date, amount, deadline, buckets discharged, bank reference, user. **No hard block** anywhere — not on sales, not on shift close, not on day-close (req. 16). A Poreska uprava check is documentary.

A till-ceiling field, if it ever ships, is labelled **"interni blagajnički maksimum — nije zakonska obaveza"** with **no default** (req. 18). No propis sets one.

---

## 6. Cross-cutting

**Money and time.** Integer minor units (para) throughout; quantities milli-units; every timestamp an RFC3339 `now: &str` parameter, never `datetime('now')` in decision code. Bucket and deadline arithmetic is pure and takes the clock as a parameter, so it is testable without freezing time.

**Serbian diacritics** (č, ć, š, ž, đ) in all operator copy.

**Purged copy (req. 19–20):** no "istog dana / narednog radnog dana" anywhere; the act is cited by full title + "Sl. glasnik RS, br. 68/2015, čl. 3 st. 1; kazne čl. 7 st. 1 tač. 2) i st. 3", never as "ZOP".

**Admin gating.** Profile writes, the rate, holiday-table edits and the aging report are `require_admin`, following the established server-derived pattern in `auth.rs`. The AML assessment itself is visible to a cashier at the till — it has to be.

**Go-live reset.** The reset must clear AML acknowledgements, deposit-aging state and declaration-check stamps, but **preserve** the shop profile and the holiday table (configuration, not trading data). This mirrors how SW-9c handled the KEP reset and needs an explicit test.

### Testing

- **Rust:** `legal.rs` tier resolution incl. the UNSET case and the enumerated CI guard; AML `>=` boundary (at, just below, just above); mixed-tender non-breach; NBS parse from fixtures (valid, malformed, missing date) with **no network in tests**; rate-unavailable degradation; FIFO drawdown incl. partial and over-deposit; `bank_withdrawal` excluded from the subject base; 7-working-day arithmetic across a holiday and across a Saturday under both settings; both migration data-survival tests; expected-cash unchanged by a `bank_transfer` **tender**; expected-cash **correctly changed** by `bank_deposit` / `bank_withdrawal` movements (all three derivation sites); KEP razduženje includes a bank-transfer sale; reset preserves profile and holidays.
- **Frontend:** profile form validation and the UNSET → "no figure shown" path; AML warning render + reason capture; staleness warning; declaration required-vs-advisory under both `distance_selling` values; aging report empty/loading/error states.

### Out of scope

Cenovnik export (SW-12), audit log (SW-10), work-time records (SW-14), employee lifecycle (SW-13), popis (SW-16), the čl. 32 asset register and the full per-record `retention_class` machinery (reqs. 36–37, 42–43 — they belong with SW-3/SW-13 and would double this cycle), textile labelling fields (req. 20 of §4: explicitly excluded), any APML reporting path, and any customers table.

---

## Open items carried into implementation

| # | Item | Handling |
|---|---|---|
| Q-7 | Is a bare missing GTIN čl. 67 (40k) or čl. 68 (50k–500k + ban)? | **No fine figure rendered** for a missing GTIN. Two-state model only. |
| Q-4 | Which NBS rate and on which date? | Srednji kurs on the transaction date; the rate and date are persisted on the sale so a different answer is re-derivable. |
| Q-5 | Is Saturday a radni dan? | Configurable, conservative default true, stated in the tooltip. |
| Q-6 | Do till-funded expenses reduce the polog duty? | Practice-tolerant reading implemented; flagged in the report footer. |
| Q-3 | Lawful ZZPL basis for identifying a buyer to aggregate cash | Buyer tag is optional and off by default; limitation stated in writing. No customers table. |
