# Dobavljač i primljena isprava — capturing the supplier's isprava o nabavci

**Design, 09.08.2026.** Closes the capture limbs of §2 row 16 of
[SERBIAN-LAW-COMPLIANCE.md](../../SERBIAN-LAW-COMPLIANCE.md). Legal authority is
[KEP-VERIFIED-RULES.md](../../KEP-VERIFIED-RULES.md) — §3 for the čl. 29 st. 1
field list and PEP čl. 12 st. 4/st. 5, §2.1 for kolona 3, §2.7 for the worked
ledger row. **No proposition here is drawn from anywhere else.** Where the
verified-rules document flags something as unpinned, this design carries the flag
forward rather than resolving it.

---

## 1. The problem, stated exactly

ZoT čl. 29 st. 1 is a duty to **possess the supplier's isprava o nabavci**
(otpremnica / faktura / …). The build satisfies none of it, and the reason is
structural rather than a missing field:

- `kep_kalkulacija.rs::create_kalkulacija`, called from
  [inventory.rs:575](../../../src-tauri/src/commands/inventory.rs) inside the
  receive transaction, writes the fourteen-element kalkulacija into `kalkulacije`
  (v13). That is the shop's **own** price document — the isprava that justifies
  the kolona-4 zaduženje (§3) — not the supplier's isprava o nabavci.
- The identity on it (`poslovno_ime`, `prodajno_mesto`, `pib`) is snapshotted
  from `settings.company`, i.e. the **trgovac's own**. There is no dobavljač
  entity anywhere in the crate: no table, no column, no request field.
- `kalkulacije.reference_type` / `reference_id` is an untyped pair aimed at an
  inventory movement. The only screen that receives goods,
  `InventoryScreen.tsx:502`, sends `productId`, `quantityMilli` and `reason` and
  nothing else, so both columns are NULL on every receipt this build produces.
- Nabavna cena is read from the catalog's standing `products.purchase_price_minor`.
  The receive dialog offers no per-delivery input, so element 9 (*vrednost robe po
  fakturi dobavljača*) is computed from a number no faktura ever moved.
- The kolona-3 opis is composed by `receipt_opis` as „Prijem robe“ /
  „Prijem robe — {razlog}“, so **PEP čl. 15 st. 4** — which requires the document
  naziv, broj and datum, and for a nabavka additionally the dobavljač's poslovno
  ime — is not composed either.

Two slots already exist and have never been filled: `post_receipt_zaduzenje`
takes a `document_date: Option<&str>` that the receive path passes `None` for,
and `InventoryAdjustmentRequest` carries a `purchase_price_minor` the frontend
never sends.

---

## 2. What this design does *not* claim

The single most important boundary, repeated in the schema comments, in the
operator-facing copy and in row 16:

> **The application records the isprava's identifying data and the operator's own
> assertion that the document is held. It does not hold the document.**

Recording a broj and a datum is not possession. A build that captured those
fields and then let row 16 read as satisfied would be the false-assurance defect
this register has corrected repeatedly — and in the more dangerous direction,
since the reader would stop looking for the paper.

The posture chosen instead is the one migration v16 already established for
`cash_movements.documented_per_pravilnik`: **store the operator's assertion, do
not presume it.** `poseduje_ispravu` defaults to 0, and an isprava nobody has
asserted stays visibly unasserted.

---

## 3. Schema — migration v23, `dobavljaci_and_primljene_isprave`

The runner is append-only and v22 is the newest, so a new version is the only
path.

```sql
CREATE TABLE dobavljaci (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    poslovno_ime TEXT NOT NULL,
    adresa TEXT NOT NULL DEFAULT '',
    pib TEXT NOT NULL DEFAULT '',
    maticni_broj_bpg TEXT NOT NULL DEFAULT '',
    fizicko_lice INTEGER NOT NULL DEFAULT 0 CHECK (fizicko_lice IN (0, 1)),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1)),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE primljene_isprave (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dobavljac_id INTEGER NOT NULL REFERENCES dobavljaci(id),
    dobavljac_poslovno_ime TEXT NOT NULL,
    dobavljac_adresa TEXT NOT NULL DEFAULT '',
    dobavljac_pib TEXT NOT NULL DEFAULT '',
    dobavljac_maticni_broj_bpg TEXT NOT NULL DEFAULT '',
    dobavljac_fizicko_lice INTEGER NOT NULL DEFAULT 0
        CHECK (dobavljac_fizicko_lice IN (0, 1)),
    vrsta TEXT NOT NULL,
    broj TEXT NOT NULL,
    datum TEXT NOT NULL,
    poseduje_ispravu INTEGER NOT NULL DEFAULT 0 CHECK (poseduje_ispravu IN (0, 1)),
    poseduje_potvrdio INTEGER REFERENCES users(id),
    poseduje_potvrdjeno_at TEXT,
    napomena TEXT,
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);

ALTER TABLE kalkulacije ADD COLUMN isprava_id INTEGER REFERENCES primljene_isprave(id);
```

Plus indices on `dobavljaci(poslovno_ime)`, `primljene_isprave(dobavljac_id, datum)`
and `kalkulacije(isprava_id)`, and a unique index on
`primljene_isprave(dobavljac_id, vrsta, broj, datum)` so the same document cannot
be entered twice against one supplier.

### 3.1 Why the identity is snapshotted rather than joined

`primljene_isprave` copies the supplier's identity at creation, exactly as
`create_kalkulacija` copies `settings.company`. A kalkulacija reprinted next year
must render what it rendered when it was issued; joining live would let an edit to
a `dobavljaci` row silently rewrite documents already issued, which is the
retroactive mutation PEP čl. 14 st. 1 exists to prevent. The master table buys
reuse — a PIB and a matični broj typed once rather than per delivery, those being
exactly the fields where a typo is the defect.

### 3.2 Why `vrsta` carries no CHECK constraint

§3 lists the valid source documents verbatim and closes with *„ili druga
odgovarajuća isprava za robu“*. A closed SQL enum would convert that open clause
into a refusal of a lawful document. The six named kinds are offered by the UI and
validated in Rust against an enum with an open `Druga(String)` variant; the column
stays free text so **the schema itself never refuses an isprava the Pravilnik
allows**.

### 3.3 Why `fizicko_lice` is a column

PEP čl. 15 st. 4 (§2.1) requires the dobavljač's *poslovno ime* in kolona 3, and
for a natural-person supplier *ime i prebivalište* instead. That is verified text,
so the branch is encodable. `adresa` serves as prebivalište on that branch.

### 3.4 Why the matični broj column has a deliberately unpinned label

§8 t. 5 records that the čl. 29 st. 1 label list was read from neobilten and is
**not pinned against an official consolidated text**. `maticni_broj_bpg` therefore
holds the datum under a name that spans both readings, and **no code comment, UI
string or register cell asserts that the label list is verified**. If a
consolidated text later contradicts the labelling, the stored data survives and
only presentation moves.

### 3.5 Rejected shapes

- **Reusing `kalkulacije.reference_type`/`reference_id`.** Already aimed at the
  operator's own free reference; overloading it makes two meanings
  indistinguishable at read time.
- **A separate `isprava_stavke` line table.** `kalkulacije` already stores naziv,
  jedinica mere, količina and both prices per line. A line table would duplicate
  all of it and create a second place for them to disagree.
- **A live join to `dobavljaci`.** §3.1.
- **Storing the document file.** The only option that moves toward actual
  possession, but it needs a `RecordClass`, a čl. 47 radnja, backup and ZZPL
  treatment — its own cycle. Recorded here as the honest next step, not as
  something this design half-does.

---

## 4. Receive form

`InventoryAdjustmentRequest` gains `isprava_id: Option<i64>`. In receive mode the
dialog gains:

- an **isprava block** — pick an existing isprava, or create one inline
  (dobavljač picker with inline create, vrsta, broj, datum, possession checkbox);
- a **nabavna cena po jedinici mere** input for this delivery.

### 4.1 The catalog write must stop being silent

`apply_inventory_adjustment` already overwrites `products.purchase_price_minor`
whenever the request supplies one ([inventory.rs:533](../../../src-tauri/src/commands/inventory.rs)).
The frontend has never supplied one, so that write has never fired. **Shipping the
input makes it fire on every receipt.** Behaviour is kept — a last-in cost is what
the catalog figure is for, and margin displays read it — but the form states that
saving updates the article's nabavna cena in the catalog. A rewrite triggered by a
field the operator has just met, with no notice, is the class of surprise this
register keeps having to correct.

---

## 5. Wiring

**`create_kalkulacija`** takes an `Option<&PrimljenaIsprava>` and writes
`isprava_id`. Its return type moves from `i64` to `CreatedKalkulacija { id,
redni_broj }`, because the opis needs the redni broj. Two call sites: the receive
path and one test.

**`receipt_opis`** composes the čl. 15 st. 4 string, following the §2.7 worked
ledger row rather than inventing a shape:

| case | opis |
|---|---|
| pravno lice | `Kalkulacija br. 12; otpremnica dobavljača „ABC d.o.o.“ br. 125 od 03.07.2026` |
| fizičko lice | `Kalkulacija br. 12; otpremnica dobavljača Petar Petrović, Novi Pazar br. 7 od 03.07.2026` |
| no isprava | `Prijem robe` / `Prijem robe — {razlog}` — today's text, unchanged |

The no-isprava branch is deliberately left alone. An unattached receipt should
read as exactly what it is; dressing it up would be the same defect as a false
tick in the register.

**`post_receipt_zaduzenje`** receives the isprava's `datum` as `document_date`.
That column exists for PEP čl. 15 st. 3's split between the booking date (kolona
2) and the document date (kolona 3) — *„these are two different dates — never
conflate them“* — and has been NULL on every receipt so far.

**Commands** (registered in `lib.rs`): `dobavljaci_list`, `dobavljac_save`,
`isprave_list`, `isprava_create`, `isprava_confirm_possession`.

---

## 6. The missing-isprava warning

Per PEP čl. 12 st. 1 an entry is made *„na osnovu verodostojnih isprava“*, so a
receipt with no isprava is worth saying out loud. It is **not** worth refusing:
blocking would strand a pallet the shop physically holds, it reverses a posture
this codebase has settled twice (`collect_declaration_warnings` is infallible by
construction precisely so it cannot become a hard block), and the goods sitting
unbooked is the worse offence tier.

So the receipt, the kalkulacija and the zaduženje commit exactly as today, and the
result carries an advisory in a field **separate** from `declarationWarnings` —
the čl. 34 warning is about the manufacturer's marking, this is about the posting
basis, and merging them would blur which duty is unmet.

It carries a new `legal.rs` notice for ZoT čl. 68 st. 1 tač. 6 at the
**preduzetnik** tier already stated in row 16. This is the **14th** notice, so
`all_notices()`, the hand-written list in the guard test and the asserted count
move together. The pravno-lice band is printed nowhere.

---

## 7. Guards

A new `docs_guard` test holds the §2 fence: **no register row may claim the
application holds, stores or possesses the supplier's isprava** while no
file-storage path exists, and row 16 must name `primljene_isprave` once it does.
This is the same shape as the existing popis-export and „prints nothing“ guards —
the register is prevented from drifting in *either* direction, toward a false
promise or toward a false denial.

---

## 8. Row 16

Re-stated with a dated `Re-stated 09.08.2026` stamp.

**Closing:** supplier poslovno ime, adresa, PIB and matični broj/BPG; the isprava's
broj and datum; per-delivery nabavna cena; and the čl. 15 st. 4 kolona-3
composition, which was a separate open limb that capturing the document closes as
a side effect.

**Stated as still open, not quietly dropped:**

1. **Possession itself.** An assertion is not the document. The app records that
   the operator said the isprava is held; it holds nothing.
2. **„zaduženje za vlastitu robu“.** That limb of the čl. 29 st. 1 list has no
   code at all — there is no own-production concept anywhere in the crate — and
   this design does not fabricate one.
3. **§8 t. 5.** The label list stays unpinned against an official consolidated
   text, and the row keeps saying so.

---

## 9. Tests

- migration v23 applies on a fresh and on an upgraded database;
- a later edit to a `dobavljaci` row does not change an isprava already created
  (the §3.1 snapshot property, asserted rather than commented);
- `isprava_id` persists onto the kalkulacija through the receive transaction;
- both opis branches, including the natural-person one;
- `document_date` lands on the `kep_entries` row and differs from the booking date;
- a receipt with no isprava still commits the movement, the kalkulacija and the
  zaduženje, and warns;
- a receipt with an isprava does not warn;
- `vrsta` accepts a document kind outside the six named ones (§3.2);
- the notice count guard moves from 13 to 14 with the new notice enumerated.
