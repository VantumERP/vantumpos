# SW-16 — Popis (godišnji i nivelacioni) — Design

**Date:** 2026-08-01 · **Status:** proposed · **Verified law:** [REMAINING-SW-VERIFIED-RULES.md](../../REMAINING-SW-VERIFIED-RULES.md) §2c, §3 V5, §4 reqs. 29–42

---

## 0. The constraint that reshapes the module

**The old roadmap spec mandated a screen that would put the customer in breach.**

It called for "printable popisne liste (article, unit, **counted vs book qty**, price, value, difference)". But **PoP čl. 8 st. 5 forbids releasing book quantities to the popis commission before the counted state is written into the liste and the members have signed them.** A side-by-side expected-vs-counted count sheet is exactly the thing the provision exists to prevent — it turns a count into a confirmation.

So the module is built around a **two-phase blind count** (req. 29), and that single constraint drives most of the rest of the design.

**Second scope answer:** no obrazac exists for the popisna lista (Pravilnik 89/2020 runs čl. 1–16 and ends at the minister's signature — no prilog, no column list), so **we design our own layout**. Do **not** carry over the SW-9c KEP print-fidelity constraints; that module reproduces a real statutory 5-column obrazac, this one does not. **But the *izveštaj o popisu* does have prescribed content** (čl. 13 st. 1) and must be a structured template, not free text.

---

## 1. Two phases, two signatures, two snapshots

| | Phase A — brojanje | Phase B — obračun |
|---|---|---|
| Shows | naziv, šifra, jedinica mere, **stvarna količina** (entered), bliži opis | everything from A **plus** knjigovodstvena količina, razlika, cena, vrednosti |
| Hides | **any book quantity, anywhere** | — |
| Ends with | čl. 8 st. 5 signature; the counted state is frozen | čl. 9 st. 3 signature on the printed, computed liste |

**Two distinct signature events, two immutable snapshots**, each with its own print/sign timestamp (req. 30). Phase B is unreachable until Phase A is signed — enforced in the state machine, not by UI ordering.

**Print-and-sign is the default compliant path** (req. 31) — čl. 9 st. 3 says *"uz štampanje"* expressly. A purely electronic signature is an **unverified deviation** (§6 R-6) and must not be presented in-product as compliant.

---

## 2. The column set (req. 32)

An **editable default template**, not a statutory-form renderer:

- header: obveznik, PIB/MB, maloprodajni objekat, datum, period, broj liste
- nomenklaturni broj/šifra, naziv, vrsta, jedinica mere (čl. 8 st. 4)
- stvarna količina + bliži opis (čl. 9 st. 1 t. 1)
- knjigovodstvena naturalna količina (t. 3) — **Phase B only**
- naturalna razlika višak/manjak (t. 4) — **Phase B only**
- cena (t. 5), vrednost po popisu / po knjigama / vrednosna razlika (t. 6) — **Phase B only**
- signature block

**Do not build the Pravilnik 140/2004 prosto-knjigovodstvo column set.** That regime is legally unavailable to a retail preduzetnik (ZPDG čl. 40 st. 2 t. 2 excludes retail from paušal; čl. 43 st. 3 confines prosto knjigovodstvo to poljoprivrednik/drugo lice), so shipping its columns would invite the shop into a regime it cannot use.

---

## 3. Nivelacija popis is a first-class mode (req. 33)

A popis is mandatory **on change of retail selling prices** in a maloprodajni objekat — ZoRač čl. 21, PoP čl. 3. This is the most POS-relevant trigger of the whole module and the register missed it entirely until 01.08.2026.

It ties directly to the **KEP nivelacija already shipped** (SW-9b): a price change posts a KEP delta *and* raises a popis obligation. Its izveštaj deadline is **30 days** after the popis, not the 60-day annual rule.

**Scoping the nivelacija count to only the repriced articles is `[PRUDENTIAL]`, not `[LEGAL]`** — it is borrowed by analogy from Pravilnik 140/2004 čl. 16, which governs a regime unavailable to this customer. Neither ZoRač čl. 21 nor PoP čl. 3 scopes it. Offer the narrowed scope as a default with the reasoning visible, and let the shop widen it.

---

## 4. The perpetual-inventory shortcut is a gate, not a default (req. 34)

VantumPOS *does* keep a continuous quantity-and-value record, so čl. 9 st. 2 is available — **but only** where the app can point to a completed, adopted **and posted** in-year popis. Otherwise force a physical count.

Store the odluka reference. And record the anchor precisely: **the exception excuses step 2) only.**

---

## 5. Documents the module generates

- **Odluka o popisu i obrazovanju komisije** + **plan rada** (PoP čl. 8 st. 1–2, req. 35), with an approval action by the *lice iz čl. 4 st. 2* — who, **for a preduzetnik, is the owner personally** (čl. 4 st. 2 → ZoRač čl. 43 st. 3, a direct assignment).
- **Separate lists, required not optional** (req. 36): impaired/obsolete/damaged goods (čl. 10 st. 3 — separate lists *or* separate columns) · goods off the premises incl. out for repair or with a third party (čl. 10 st. 4) · **cash by denomination** (čl. 11 st. 1) · undocumented receivables/payables (čl. 12 st. 2) · **consignment/third-party goods (čl. 2 st. 5), with a signed copy to the owner within 10 days of the count date (čl. 2 st. 6) — its own reminder.**
- **Izveštaj o popisu** — a structured template carrying the **eight čl. 13 st. 1 content elements** (req. 37). Not free text.
- **Odluka o usvajanju** (čl. 14 st. 2), surfaced with the izveštaj as one milestone.

---

## 6. Gates and locks

**Pre-popis reconciliation gate** (req. 39): require and record that glavna knjiga↔dnevnik and pomoćne knjige↔glavna knjiga were reconciled **before** the popis is opened. ZoRač čl. 20 st. 3 legislates the ordering, so this is a gate, not a checklist item.

**Deadline engine** (req. 38): compute `FS filing deadline − 60 days`; **do not hardcode**. 30 January 2027 for FY2026, 31 January 2028 for FY2027 (leap), 30 January 2029 for FY2028. The nivelacija mode uses its own 30-day rule.

**Commission modelling** (req. 40): popisivači are named persons with a `rukuje_imovinom` flag; **warn** when a designated member or the single person is flagged. Reuse the flag for ZoRač čl. 10 st. 5. **Warn, do not block** — the shodna primena is unresolved (§6 R-5).

**Write-lock on posting** (req. 41): once a result is posted (čl. 14 st. 3), the liste and the izveštaj are immutable; corrections go through a **new document** (ZoRač čl. 8 st. 4).

**Retention floor 5 years** (req. 42) for the liste and the izveštaj, on the footing that they are *isprave na osnovu kojih se unose podaci u poslovne knjige* (ZoRač čl. 28 st. 7), counted from the last day of the business year (st. 9). Doubly safe — even classed as pomoćne knjige the answer is 5 years (st. 5). Keep the panic-safe floor pattern from SW-9c. **Flag to counsel: no provision names popisne liste expressly (§6 R-7).**

---

## 7. What must NOT be built

1. **No book quantity visible anywhere during Phase A.** This is the single biggest constraint in the module.
2. No Pravilnik 140/2004 column set.
3. No statutory-form fidelity constraints — there is no obrazac.
4. No electronic signature presented as the compliant path.
5. No hardcoded deadline dates.
6. No blocking on a `rukuje_imovinom` commission member — warn only.
7. No edit path on a posted popis; corrections are new documents.
8. No claim that scoping a nivelacija count to the repriced articles is legally required.

---

## 8. Testing

**Rust:** Phase B is unreachable until the Phase A signature exists; a Phase A payload that carries a book quantity is **refused at the boundary**, not merely hidden in the UI; the two snapshots are immutable and separately timestamped; the deadline engine computes all three FY dates from the filing rule rather than a table; the nivelacija mode uses 30 days; the perpetual-inventory gate refuses without a posted in-year popis; posting write-locks the documents; retention resolves to a 5-year floor through the shared table; the consignment 10-day reminder fires from the count date.

**Frontend:** the Phase A sheet renders no book-quantity column and no difference column; the commission warning appears for a `rukuje_imovinom` member and does not block; the izveštaj template exposes all eight čl. 13 st. 1 fields; a posted popis has no edit affordance.

---

## 9. Open items

| # | Item | Handling |
|---|---|---|
| R-5 | Does the goods-handler exclusion apply *shodno* to a single-person popis? | Warn, never block. |
| R-6 | Is a purely electronic signature compliant under čl. 9 st. 3? | Print-and-sign is the default; electronic is an unverified deviation and is not presented as compliant. |
| R-7 | No provision names popisne liste expressly for retention | 5-year floor on the isprave footing; flag to counsel. |
