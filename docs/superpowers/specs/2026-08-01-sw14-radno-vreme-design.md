# SW-14 — Evidencija radnog vremena — Design

**Date:** 2026-08-01 · **Status:** approved (design) · **Verified law:** [SW14-VERIFIED-RULES.md](../../SW14-VERIFIED-RULES.md)

Every requirement below traces to a numbered item in §4 of the verified rule set. Items tagged `[PRUDENTIAL]` there must never be presented to the operator as a legal duty — that distinction is asserted in tests, exactly as in SW-11/SW-15.

---

## 0. What verification changed about this feature

Three findings reshaped the scope before any code was written:

1. **Serbia imposes no general daily working-time record.** ZoR čl. 55 st. 6 requires a daily record of **overtime only**. Croatia, FBiH and North Macedonia require the general one — which is where the backlog's "clock-in/out independent of register shifts" framing came from. We build an hours record, not a punch clock.
2. **No obrazac is prescribed.** Neither čl. 55 st. 6 nor čl. 122 carries a delegation clause, so no bylaw can exist under them, and none does. "Pravilnik 120/2014" does not exist in Serbian law. The module designs its own layout — and must never call any output a *propisani obrazac*.
3. **The register is the smaller fine.** Missing register = 50.000–150.000 (čl. 276 st. 1 u vezi sa tač. 1a). **Ordering overtime beyond the čl. 53 caps = 200.000–400.000 (čl. 274 st. 1 tač. 3)** — ~2.7× larger, and absent from the compliance register until 01.08.2026. **The cap checks are therefore the feature, not a nicety around it.**

**Scope decisions taken with the founder:** protection groups (čl. 87–91) are **in**, with the full guard set; preraspodela ships as a **flag that disables automatic overtime derivation** rather than a full čl. 56–58 averaging engine.

---

## 1. The primitive: hours, one row per (employee, calendar date)

**A register shift is not an employee's working time** (req. 1). Two people can share a till, čl. 64 breaks are unpaid gaps, and a shift belongs to a register. Shifts join as **corroborating evidence only** — a side panel showing "smene tog dana", never a source the record is derived from.

**Store hours, not timestamps** (req. 2). No Serbian provision requires a start/end time. A timestamp model invents obligations the law does not impose and drags the product toward a *sistem za praćenje rada* (§5 item 6).

### 1.1 The fifteen statutory buckets

`work_time_entries` carries the ZEOR čl. 24 tač. 1 enumeration verbatim, in hours (stored as integer **minutes** to stay in integer arithmetic, rendered as hours):

| Column | ZEOR čl. 24 tač. 1 |
|---|---|
| `moguci_casovi` | a) |
| `ukupno_ostvareni_casovi` | b) |
| `efektivno_izvrseni_casovi` | b) 1st indent |
| `casovi_cekanja_i_zastoja_i_prekida` | b) 2nd indent |
| `casovi_obustave_rada_zbog_strajka` | b) 3rd indent |
| `ukupno_neizvrseni_casovi` | v) |
| `casovi_godisnjeg_odmora` | g) |
| `casovi_odmora_za_dane_drzavnih_praznika` | g) |
| `casovi_odsustva_uz_naknadu_zarade` | g) |
| `casovi_strucnog_osposobljavanja_i_usavrsavanja` | g) |
| `casovi_privremene_sprecenosti__poslodavac` | g) — ZoR čl. 115, first 30 days |
| `casovi_naknade_na_teret_drugih_poslodavaca` | d) |
| `casovi_privremene_sprecenosti__rfzo` | đ) |
| `casovi_porodiljskog_i_skracenog_rv_roditelja` | đ) |
| `casovi_neplacenog_odsustva` | e) |
| `casovi_prekovremenog_rada` | ž) — **also the ZoR čl. 55 st. 6 column** |

**The poslodavac/RFZO sick-leave split is statutory, not a design choice** — the phrase appears twice in čl. 24 tač. 1, under g) and under đ). What stays prohibited is diagnosis, doznaka number, ICD code, free text and attachments (§5 item 4).

### 1.2 Advisory columns, labelled as such

`nocni_casovi`, `casovi_rada_na_praznik`, and optional clock-in/out — all tagged in code **and** in the UI as `izračunato radi provere usklađenosti`, never `zakonom propisano polje` (req. 4). A named constant carries the label so a copy edit cannot flatten the distinction, and a test asserts the advisory columns never render under the statutory heading.

### 1.3 Absence category is a closed enum

Enforced by a DB `CHECK`, not UI validation (req. 3). **Zero free-text on an absence row.** A free-text column that exists will eventually hold a diagnosis, and that converts a bounded čl. 17 st. 2 tač. 2 data point into unbounded health data on a shop-counter PC the vendor can reach over AnyDesk.

---

## 2. Compliance checks — the part that carries the bigger fine

### 2.1 The čl. 53 caps (req. 7)

Validated at entry: overtime **≤ 8 h in any calendar week** (st. 2) and total **≤ 12 h in any day including overtime** (st. 3). Warn loudly and require an explicit override with a reason — do **not** hard-block, because the record must be able to describe what actually happened, including an unlawful day. A register that refuses to record reality is worse than useless: it hides the čl. 274 exposure instead of surfacing it.

**No annual overtime counter** (req. 8). No annual ceiling exists in ZoR; inventing one would put a fabricated rule in front of the shop.

### 2.2 Preraspodela — the flag, not the engine

Per-employee `radi_u_preraspodeli`. When set:
- automatic `hours > 8 ⇒ prekovremeni` derivation is **disabled** — under čl. 58 preraspodela hours are not overtime, and a naive rule would inflate the very register that is penalised (req. 9);
- overtime is entered manually;
- the UI states plainly that the app does **not** compute the čl. 56 st. 4 / čl. 57 averages, and that the čl. 57 st. 4 conversion for an employee *"koji se saglasio"* is the shop's call.

This is the founder-approved shape. Full čl. 56–58 averaging is explicitly out of scope and recorded as such.

### 2.3 Protection groups (čl. 87–91) — full guard set

New employee profile fields, each minimal and purpose-bound:

| Field | Guard |
|---|---|
| `datum_rodjenja` | Under 18 → **reject** any overtime or preraspodela entry (čl. 88 st. 1); cap 35 h/week and 8 h/day (čl. 87); block night hours except the čl. 88 st. 2 exceptions |
| `datum_rodjenja_najmladjeg_deteta` + `samohrani_roditelj` | Parent of a child **up to 3** (čl. 91 st. 1), or **samohrani roditelj of a child up to SEVEN** or a težak invalid (čl. 91 st. 2) → require a stored written-consent flag before overtime or night hours can be saved. **The threshold is seven, not fourteen** (req. 12) |
| `trudnoca_ili_dojenje` (boolean + date) | čl. 90 is **not** an unconditional block — it fires on a nalaz nadležnog zdravstvenog organa. Store the boolean + date only; **never the underlying medical evidence** (req. 13) |

The night-work threshold in čl. 62 st. 2 (≥3 h/day or ⅓ of the week) is an **advisory flag only** — the reassignment duty fires on a health authority's opinion, not automatically (req. 14).

Rest-period checks (čl. 64, 66, 67) ship as `[PRUDENTIAL]` warnings (req. 15).

---

## 3. `legal.rs` additions

Two new notices, both resolving their tier from `ShopProfile.pravna_forma` and **both added to the enumerated guard list** — that list is the module's entire safety property:

- `overtime_record_missing()` — preduzetnik *50.000 do 150.000 (čl. 276 st. 1 u vezi sa tač. 1a)*; pravno lice 150.000–300.000 + odgovorno lice 10.000–20.000 (st. 2). **Suppress the odgovorno-lice line entirely under the preduzetnik regime** (req. 29).
- `overtime_caps_exceeded()` — preduzetnik **200.000–400.000** (čl. 274 st. 1 tač. 3 + st. 2).

**Never render any ZEOR figure** (req. 29, §5 item 16). čl. 50 st. 1's 500.000–1.000.000 is the pravno-lice tier; čl. 51's 300.000–500.000 names *"fizičko lice koje ima zaposlene"* and **exceeds the ZoP čl. 39 st. 1 tač. 1 ceiling** for that class, so the tier is genuinely unresolved. The app says only that a separate statute imposes a broader dataset and permanent retention, and that the figure is unsettled.

Inspector is **inspektor rada**. Say *"ovi članovi ne propisuju zaštitnu meru"* — never *"zabrana nije moguća"* (req. 30, ZoP čl. 55 st. 2 unresolved).

A CI guard mirroring SW-15's asserts the literal `300.000` is unreachable as the preduzetnik's čl. 276 exposure.

---

## 4. Retention — two classes, one shared table

**Class A** — the derived, period-closed hour classification per employee per month → **`trajno`**, structurally unreachable by every purge, reset, restore and backup-prune path (req. 18).
**Class B** — raw entry drafts and any advisory clock data → bounded, purged once the period is closed and Class A is derived.

`trajno` attaches to the **derived** classification (ZEOR čl. 25 st. 3 / čl. 7 st. 2), **not** to an event stream. Keeping minute-level punches plus a bolovanje flag forever is a čl. 5 st. 1 tač. 3 and tač. 5 breach (§5 item 10).

**Period close is a real state transition, not a report** (req. 19): derive and freeze Class A → mark Class B purge-eligible → irreversible without an audited unlock. Without it there is no defensible moment at which raw data stops being necessary.

Standalone overtime log where not folded into a wage record: floor **3 years**, upward-only (req. 20). **Never surface "2 years" or "6 years"** — the first is a Serbian blog error, the second is Croatian law.

This reuses the **single shared retention table** mandated by SW11-SW15 §3 req. 42. That table does not exist yet; this cycle creates it, and SW-3/SW-13 adopt it later. Independent notions of "expired" guarantee that one path deletes what another is obliged to keep.

---

## 5. Surfaces

**Admin module "Radno vreme"** (`adminOnly` nav item): per-employee monthly grid, the čl. 53 cap warnings, period close, and the export.

**Per-employee read-only "Moji sati"** (req. 23) — discharges ZoR čl. 83 st. 1 and ZZPL čl. 26 at once, and is far cheaper than answering an access request by hand.

**Export** (req. 21): a plain per-employee monthly sheet plus CSV, columns mapped 1:1 onto ZEOR čl. 24 tač. 1. **Must render offline, from the till** — ZIN čl. 20 st. 7 / čl. 21 tač. 4 require production *"u obliku u kojem ih nadzirani subjekat poseduje i čuva"*. Header copy: *"Evidencija prekovremenog rada — ZoR čl. 55 st. 6. Zakon ne propisuje obrazac."* (req. 22).

**No employee signature** (req. 23). If an attestation is ever built it is labelled *"interna potvrda zaposlenog"* and is optional.

**Append-only with a superseding-row correction log** carrying who/when/why, original struck through (req. 6). **Never back-date** — "dnevnu" in čl. 55 st. 6 is what makes a reconstructed month look like a fabrication.

---

## 6. Privacy surface

- **Basis is a constant, never asked** (req. 24): čl. 12 st. 1 tač. 3 for the statutory classes, tač. 2 for contract computation, **čl. 17 st. 2 tač. 2** for the absence-hour category. **No consent UI anywhere in the employee surface.**
- **The absence reason is a special-category column with its own access gate** (req. 25) — owner/payroll role only. Every other role sees `odsutan` plus an hour total. Never on a counter-facing screen or a generic employee export.
- **Remote support must not see it by default** (req. 28): mask unless the shop explicitly unmasks for that session, and log the unmask (feeds SW-10).
- **Regenerate and re-deliver the čl. 23 notice before the module goes live** (req. 26, čl. 23 st. 3). Gate: the module refuses to activate for an employee until an acknowledgement dated on/after the feature's introduction is recorded. `docs/compliance/obavestenje-zaposlenima.md` gains the posebne-vrste row, named recipients, and the two-class retention statement.

**No headcount gate, no plan-tier gate** (req. 31). There is no small-employer exemption; a 1–3-employee boutique is fully bound.

---

## 7. What must NOT be built

The verified set lists 21 items (§5). The ones that constrain this design directly:

1. **Stay OUT of payroll (F-12), and the line runs inside ZEOR čl. 24.** Tačka 1 (hours) is ours. **Tačke 2–3 (bruto/neto zarada, porezi, doprinosi, dodaci, naknade, otpremnina, regres) are the accountant's** and must not be modelled, stored, imported or displayed.
2. **Never produce an obračun zarade** — ZoR čl. 121 st. 6 makes it an *izvršna isprava*, a directly enforceable instrument. A POS emitting one creates an enforceable debt document from unaudited data.
3. **Never compute the čl. 108 uplifts** (110 % praznik / 26 % noć / 26 % prekovremeni / 0,4 % minuli rad). Emit hour counts; payroll applies percentages.
4. **No biometric clock-in, ever.** ZZPL čl. 17 st. 1 prohibits it for unique identification, no čl. 17 st. 2 exception rescues it for a boutique, and the Poverenik's published position is that biometrics for working-time control is disproportionate. PIN or card. If a customer asks, refuse in writing.
5. **Do not cross into a *sistem za praćenje rada*:** no geolocation on punch, no idle/active detection, no screenshots, no keystroke counts, no productivity scoring, no cashier league tables, no automated lateness alerts. Any one plausibly triggers a mandatory DPIA **plus** a prior Poverenik opinion on a 60+45-day clock.
6. **No promet attribution to a named cashier inside this feature.**
7. **No e-Bolovanje integration** — the preduzetnik deadline is 1 Jan 2027 and the flow is HR/accounting, not POS. Sick leave is two hour buckets and nothing else.
8. **Never say "ZoR-compliant time tracking" while logging only overtime** — čl. 55 st. 6 does not discharge the ZEOR čl. 23–24 duty or the ZoR čl. 122 duty.
9. **No "Pravilnik 120/2014"** anywhere — code, docs, UI, marketing.
10. **Regional portability:** this module is **not** portable to FBiH/HR/MK, which do prescribe content and forms. Any future expansion needs a jurisdiction profile, not an assumption.

---

## 8. Testing

**Rust:** the 15 buckets round-trip; the čl. 53 weekly and daily caps at the boundary (7/8/9 h overtime in a week; 11/12/13 h in a day); under-18 rejection; the **age-seven** samohrani-roditelj threshold specifically (the classic wrong number is fourteen); consent required before overtime for a čl. 91 parent; preraspodela disables derivation; period close freezes Class A and marks Class B; retention class assignment; `legal.rs` tier resolution for both new notices plus their presence in the enumerated guard; **a test asserting no ZEOR figure is reachable in any rendered notice**; migration data-survival following the pattern established in `270796c`.

**Frontend:** advisory columns never render under the statutory heading; the absence reason is invisible to a non-payroll role; "Moji sati" is read-only; the export header carries the *"Zakon ne propisuje obrazac"* line; the čl. 53 warning offers an override with a reason rather than a dead end.

---

## 9. Open items carried into implementation

| # | Item | Handling |
|---|---|---|
| W-1 | ZEOR čl. 51 tier conflicts with the ZoP čl. 39 ceiling | **Render no ZEOR figure.** Say the figure is unsettled. |
| W-2 | Whether a surviving ZEOR obrazac binds the wage record | Out of scope — we build the ZoR record, and never label output a *propisana evidencija o zaradama*. |
| W-7 | čl. 62 st. 3 union opinion before introducing night work, where no union exists | Surface as an advisory note; do not gate. |
| W-8 | Retention class of POS shift-open/close records | **Do not auto-purge them on a ZZPL clock** — they are simultaneously cash-control artefacts and time evidence. |
| — | ZoP čl. 55 st. 2 zaštitna mera | Say čl. 273–276a prescribe none; never say a ban is impossible. |
