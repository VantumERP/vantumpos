# KEP (Evidencija prometa) — Verified Rule Set for SW-9

Scope: definitive, encodable rules for the Knjiga evidencije prometa (KEP) for the VantumPOS pilot — a **preduzetnik doing trgovina na malo** in Novi Pazar. Every load-bearing statement is pinned to primary text (Pravilnik / ZoT / Zakon o računovodstvu) with član/stav/tačka + URL. Corrected rules only; unsupported items are dropped or flagged in §8. As-of date 19.07.2026.

Primary sources (cited inline by short name):
- **PEP** = Pravilnik o evidenciji prometa, „Sl. glasnik RS" br. 99/2015 i 44/2018 – dr. zakon — https://www.paragraf.rs/propisi/pravilnik_o_evidenciji_prometa.html (corroborated: https://porezionline.rs/propis_prikaz.php?cID=994). NB: the older `…/propisi_download/…pdf` link 404s; use the HTML.
- **ZoT** = Zakon o trgovini, „Sl. glasnik RS" 52/2019 i 35/2026 — https://www.paragraf.rs/propisi/zakon_o_trgovini.html; amendment law 35/2026 — https://www.paragraf.rs/izmene_i_dopune/230426-zakon-o-izmenama-i-dopunama-zakona-o-trgovini.html
- **ZoRač** = Zakon o računovodstvu, „Sl. glasnik RS" 73/2019 i 44/2021 – dr. zakon — https://www.paragraf.rs/propisi/zakon-o-racunovodstvu-2020.html

---

## 1. Governing law + current validity

**The operative KEP bylaw today is the Pravilnik o evidenciji prometa („Sl. glasnik RS" br. 99/2015 i 44/2018 – dr. zakon).** Build SW-9 against this text. No newer Pravilnik o evidenciji prometa exists as of July 2026 (verified negative across paragraf.rs, Sl. glasnik, ministry searches); 2026 bylaw activity in the trade/accounting space concerns other instruments (elektronske otpremnice, poresko računovodstvo), not evidencija prometa.

Why it is still in force despite the May-2026 ZoT overhaul:
- **ZoT čl. 71 st. 2**: „Do donošenja podzakonskih akata na osnovu ovlašćenja iz ovog zakona primenjivaće se podzakonski akti doneti do dana stupanja na snagu ovog zakona, osim odredaba koje su u suprotnosti sa ovim zakonom." (surviving-bylaw clause.) The 12-month bylaw window is **ZoT čl. 71 st. 3** and runs from the 2019 base law.
- The amendment law **35/2026 does not touch čl. 71** and sets **no new bylaw deadline** — its final article, **čl. 27**, is only „Ovaj zakon stupa na snagu osmog dana od dana objavljivanja…". So there is no legal trigger forcing a near-term replacement.

**Statutory hooks (post-35/2026 numbering):**
- **ZoT čl. 29** — duty to hold isprave o proizvodnji, nabavci i prodaji robe (incl. *nabavna cena robe, zaduženje za vlastitu robu, prodajna cena robe*), and the data goods-in-transit documents must carry (čl. 29 st. 2). These isprave are the input to the kalkulacija (§3).
- **ZoT čl. 30** — the KEP duty itself: st. 1 keep evidencija „na osnovu isprava iz člana 29."; st. 2 „za svako prodajno mesto posebno."; **new st. 5** (inserted by 35/2026 čl. 8) — daljinska-trgovina goods dispatched from a prodajni objekat may be recorded „objedinjeno na način iz stava 2."; st. 6 „učini dostupnom na prodajnom mestu."; **st. 8** delegates content/form/manner/storage to the minister (the live delegation under which a successor Pravilnik *could* appear). The Pravilnik is not contrary to the amended čl. 30, so it survives.

**Blunt status:** nothing substantive changed for the KEP in 2026. The 5-column obrazac, the 14-element kalkulacija, and the storno rules are all still governed by the 99/2015 text. Design the column/label/element/posting rules as a **versioned, data-driven layer** so a future Pravilnik under čl. 30 st. 8 can be swapped without data migration (monitor must.gov.rs / Sl. glasnik).

---

## 2. The obrazac + posting rules (encodable)

### 2.1 The five columns (fixed; do not add or rename)
PEP čl. 3 st. 2 and **čl. 15 st. 1** („Obrazac Knjiga evidencije čini pet kolona označenih brojevima od 1 do 5."):

| # | Column (obrazac header) | Content | Authority |
|---|---|---|---|
| 1 | **Red. broj** | Redni broj svakog pojedinačnog evidentiranja; monotonic, gap-free, **per prodajno mesto, per year**. The same redni broj is written onto the source isprava, and isprave are filed „po redosledu". | PEP čl. 15 st. 2 |
| 2 | **Datum evidencije (dan i mesec)** | Date the **entry is made** (booking date) — **day + month only, no year, no clock time**. Store the full timestamp internally; **print only dan.mesec**. | PEP čl. 15 st. 3 |
| 3 | **Opis evidentirane promene (naziv, broj i datum dokumenta)** | Document naziv + broj + datum; **for a nabavka, additionally the dobavljač's poslovno ime** (for a natural-person supplier: ime i prebivalište). | PEP čl. 15 st. 4 |
| 4 | **Iznos dinara — zaduženje** | Value entered into stock. Retail basis (see 2.2). | PEP čl. 15 st. 5 |
| 5 | **Iznos dinara — razduženje** | Value leaving stock (sales). See 2.3. | PEP čl. 15 st. 6 |

Page mechanics (PEP čl. 10 st. 2): numbered pages; each page sums col 4 and col 5; the sum carries to the next page as a **DONOS** (carry-in) row; each page closes with a **SVEGA ZA PRENOS** (carry-out) row. Footer block **M.P. / ODGOVORNO LICE**; header block trgovac / objekat-prodajno mesto / mesto / „KNJIGA EVIDENCIJE PROMETA ZA ___ GODINU". Note: the KEP has **no dedicated saldo column** — saldo is derived (2.4).

Kolona 2 records the *booking* date; kolona 3 carries the *document* date. **These are two different dates — never conflate them.**

### 2.2 What a goods RECEIPT posts (kolona 4)
- Price basis for **trgovina na malo** = **maloprodajne cene, cene sa PDV** (PEP čl. 15 st. 5, uvod: „…za trgovinu na malo po maloprodajnim cenama (cene sa PDV), a za trgovinu na veliko, po velikoprodajnim cenama…"). Post the **retail value WITH PDV** of the received quantity = kalkulacija element (13) *prodajna vrednost robe sa obračunatim PDV* (§3).
- **Do NOT post nabavna cena, invoice value, or value-without-PDV.** Encoding nabavna cena is the classic false-assurance bug this verification exists to prevent.
- The „(cene sa PDV)" parenthetical qualifies **only** the retail basis. Wholesale posts velikoprodajne cene with **no auto-added PDV**. A preduzetnik **outside the PDV system** still books the maloprodajna (retail selling) price — there is simply no PDV component to add.

### 2.3 What a SALE posts (kolona 5)
- **One row per trading day** = „iznos dnevnog prometa za prodatu robu" from the fiscal register: fiskalna kasa, fiskalni računi, fakture i drugi zakonom propisani dokumenti (PEP čl. 15 st. 6 tač. 1). The daily figure = fiscal-cash daily turnover **plus** any faktura sales for that day. Back it with the **dnevni izveštaj fiskalne kase** referenced in col 3. Wire SW-9 razduženje to the sales ledger's **daily total** (ESIR/L-PFR dnevni izveštaj), not to individual receipts. (Per-receipt entry is not prohibited and yields the same sum, but the prescribed unit is the daily promet.)

### 2.4 Running saldo (derived)
`saldo = Σ(kolona 4) − Σ(kolona 5)` (PEP čl. 16 st. 2: „…stanjem koje se dobija saldiranjem kolona br. 4. i 5."). It represents **vrednost robe na stanju** at retail-incl-PDV. At year-end the **krajnji saldo = vrednost robe koja se prenosi kao početno stanje u narednu godinu** (PEP čl. 17 st. 2) — carried as next year's opening zaduženje / DONOS.

### 2.5 Entry timing + into-circulation gating
- **Booking deadline** (PEP čl. 11 st. 1): all goods-turnover changes (nabavka, prodaja, povraćaj, otpis, rashod, manjak, promena cena, itd.) entered „najkasnije narednog dana za prethodni dan." → SW-9 surfaces an **overdue-posting warning** when a change dated day D is still unbooked and today > D+1. Applies to **all** event types, not just receipts.
- **Sell-after-entry gate** (PEP čl. 11 st. 2): „Primljena roba stavlja se u promet posle evidentiranja u Knjizi evidencije." → block/flag selling a received lot until its zaduženje is posted.
- **Only carve-out to the gate** (PEP čl. 11 st. 3): dnevna štampa, pasterizovano mleko, osnovne vrste hleba (hleb za dnevnu potrošnju) may sell po kalkulisanim cenama *before* the zaduženje entry, provided the entry is still made by the next day. Default this whitelist **off** for a general retailer. (PEP čl. 11 st. 4 relaxes only the *entry timing* for unpredictable promo mechanics — „treći artikal za 1 dinar" i sl. — it is **not** an exception to the sell-gate.)

### 2.6 Per prodajno mesto
One KEP book per prodajni objekat/prodajno mesto (PEP čl. 3 st. 3; ZoT čl. 30 st. 2). A shop with multiple poslovne jedinice/odeljenja *may* (optional — „može") keep a separate book per unit (PEP čl. 3 st. 4). Trade outside a fixed objekat is kept „na nivou celokupnog svog prometa" (PEP čl. 3 st. 5). Redni broj and saldo are **per book**. Pilot single-location shop = one book; never commingle two locations.

### 2.7 Worked ledger example (RSD, PDV obveznik 20%)
Opening stock carried from 2025 (krajnji saldo) = 10.000,00. Supplier „ABC d.o.o." faktura br. 125 od 03.07.2026; one lot of 50 kom (see §3 kalkulacija → prodajna vrednost sa PDV = 7.800,00). Day 05.07 fiscal promet = 2.340,00 (15 kom × 156,00).

| RB | Datum | Opis | Zaduženje (4) | Razduženje (5) |
|---|---|---|---|---|
| — | | **DONOS** | 10.000,00 | 0,00 |
| 1 | 04.07 | Kalkulacija br. 12; faktura dobavljača „ABC d.o.o." br. 125 od 03.07.2026 | 7.800,00 | |
| 2 | 06.07 | Dnevni izveštaj fiskalne kase br. 3 za 05.07.2026 | | 2.340,00 |
| — | | **SVEGA ZA PRENOS** | 17.800,00 | 2.340,00 |

**Saldo = 17.800,00 − 2.340,00 = 15.460,00** (retail value, incl. PDV, of goods on hand).

---

## 3. Kalkulacija cene — the full element list

The kalkulacija is the **isprava that justifies each kolona-4 zaduženje** on a nabavka; it is compiled „na osnovu propisanih isprava o nabavci robe" and must carry the elements enumerated in **PEP čl. 15 st. 5 tač. 1**. **The count is FOURTEEN (14), not 13** — the prior „13" was a miscount; the regulation gives an unnumbered comma list that parses to 14. Encode all 14; treat „13" as retired.

Valid source documents (verbatim): *faktura, otpremnica, faktura-otpremnica, dostavnica, interna prenosnica, prijemnica ili druga odgovarajuća isprava za robu.*

**The 14 elements, verbatim, in order:**
1. poslovno ime trgovca
2. naziv i adresu prodajnog mesta
3. PIB trgovca
4. redni broj
5. trgovački naziv robe
6. jedinicu mere
7. količinu
8. nabavnu cenu po jedinici mere
9. vrednost robe po fakturi dobavljača
10. razliku u ceni (marža)
11. prodajnu vrednost robe bez PDV
12. PDV
13. prodajnu vrednost robe sa obračunatim PDV
14. prodajnu cenu robe po jedinici mere

Structure (interpretive, not statutory labels): (1)–(3) are document-header identity; (4)–(14) are per-line. **There is NO separate „zavisni troškovi nabavke", NO „nabavna vrednost" line, and NO „stopa PDV (%)" element** — element 9 is the bare supplier-invoice value; element 12 is the PDV **amount** (not a rate). The 14 are a **minimum** („koje sadrže sledeće elemente"); any internal helper columns (e.g. a landed-cost working field, a PDV-rate helper) are non-prescribed and must not appear on the printed/inspected kalkulacija.

**Derivations to enforce** (per line): (11) = (9) + (10); (12) = PDV on (11) at the applicable stopa; (13) = (11) + (12); (14) = (13) ÷ (7). **KEP kolona-4 zaduženje amount = element (13)** for maloprodaja.

**When required:** on **every** goods receipt (nabavka) — it is the „osnov" for the col-4 entry (PEP čl. 15 st. 5 tač. 1), and received goods become sellable only after that entry (PEP čl. 11 st. 2). Chain: received document → kalkulacija → KEP zaduženje.

**Backing-document rule** (PEP čl. 12): entries are made „na osnovu verodostojnih isprava" (faktura, carinska isprava, dostavnica, otpremnica, faktura-otpremnica, interna prenosnica, prijemnica, dnevni izveštaj fiskalne kase/fiskalni dokument, zapisnik, popisna lista, revers, isprava o otkupu, potvrda i dr. — čl. 12 st. 1). Any document used as a **posting basis** must additionally carry **jediničnu cenu i vrednost robe** (čl. 12 st. 5). Documents merely accompanying goods carry the čl. 12 st. 4 set (broj/datum, identity+PIB of isporučilac/primalac/prevoznik, mesto/adresa objekta, potpisi, naziv robe, količina).

**ZoT čl. 29 st. 1 data** (statutory backstop the kalkulacija satisfies): poslovno ime, adresa, PIB i matični broj/BPG; broj i datum isprave; naziv, merna jedinica i količina robe; **nabavna cena robe; zaduženje za vlastitu robu; prodajna cena robe.** (Exact consolidated wording — see §8.)

**Worked kalkulacija (RSD, 20% PDV):**

| El. | Field | Value |
|---|---|---|
| 5 | trgovački naziv robe | Artikal X |
| 6 | jedinica mere | kom |
| 7 | količina | 50 |
| 8 | nabavna cena / jm | 100,00 |
| 9 | vrednost po fakturi dobavljača (8×7) | 5.000,00 |
| 10 | razlika u ceni / marža (30%) | 1.500,00 |
| 11 | prodajna vrednost bez PDV (9+10) | 6.500,00 |
| 12 | PDV (20% × 11) | 1.300,00 |
| 13 | **prodajna vrednost sa PDV (11+12)** | **7.800,00** ← posts to KEP kol. 4 |
| 14 | prodajna cena / jm (13÷7) | 156,00 |

(Elements 1–3 = poslovno ime / naziv+adresa prodajnog mesta / PIB in the header; 4 = redni broj.)

---

## 4. Nivelacija + storno — the column + sign, RESOLVED

### 4.1 Nivelacija lives entirely in KOLONA 4
**PEP čl. 15 st. 5 tač. 5** sits inside the paragraph that opens „U kolonu broj 4. upisuje se zaduženje trgovca…", so every item in it books to **kolona 4 (zaduženje)**. It covers: promena cene usled nivelacije, **promena stope PDV**, **vraćanje** (to supplier), **otpis** usled krađe/više sile, **manjak po odluci trgovca**, **rashod** (kalo, rastur, lom, kvar). Verbatim mechanism: „…za iznos kojim se uvećava vrednost robe, vrši se evidentiranje kao i pri nabavci robe, a u slučaju smanjenja vrednosti robe vrši se storniranje crvenim stornom (ispisivanje iznosa koji se zaokružuje). Prilikom sabiranja iznos crvenog storna oduzima se od ukupnog zbira."

**Definitive encodable rule:**
- **Upward nivelacija / value increase / PDV-rate increase** → a **positive** kolona-4 entry, booked exactly like a nabavka. Δ = (new prodajna vrednost sa PDV − old) for the **on-hand quantity**.
- **Downward nivelacija / value decrease / return-to-supplier / otpis / manjak-po-odluci / rashod** → a **crveni storno in kolona 4**: a negative amount, rendered circled/red, **subtracted** from the kolona-4 column total. **Never routed to kolona 5.**
- Basis document: price change → **popis ili podaci iz poslovnih knjiga**; vraćanje/otpis/manjak/rashod → **računovodstvena i druga isprava** (MUP zapisnik za krađu, akt osiguravajućeg društva za višu silu, itd.). Basis-document field is mandatory.

Reasoning + authority: nivelacija re-values goods **previously charged as zaduženje**, so a decrease is a negative correction *within* kolona 4; booking it in kolona 5 would falsely imply goods left the store. The structural placement (tač. 5 under the „U kolonu broj 4" stav) is decisive and matches the 2020 kontrolna-lista phrasing „kroz kolonu 4."

### 4.2 KOLONA 5 has its OWN, separate crveni storno
**PEP čl. 15 st. 6 tač. 3**: a **customer** return on rescission/withdrawal from the sale contract („povraćaja robe usled raskida, odnosno odustanka od ugovora o prodaji") → „smanjenje razduženja robe crvenim stornom … iznos crvenog storna oduzima se od ukupnog zbira." This reduces the **kolona-5** total and **raises** the saldo (goods back on hand); no separate re-zaduženje is prescribed.

### 4.3 Cause → column is a HARD MAP (never user free-choice)
There are **four** distinct storno paths across **two** columns — a misrouted storno silently corrupts the saldo:

| Event | Column | Sign | Authority |
|---|---|---|---|
| Downward nivelacija / PDV-rate cut / otpis / manjak-po-odluci / rashod | **4** | crveni storno (−) | PEP čl. 15 st. 5 tač. 5 |
| Vraćanje robe **dobavljaču** (supplier return) | **4** | crveni storno (−) | PEP čl. 15 st. 5 tač. 5 |
| Komisionar returns robu komitentu | **4** | crveni storno (−) | PEP čl. 15 st. 5 tač. 3 |
| **Customer** return — raskid/odustanak od ugovora | **5** | crveni storno (−) | PEP čl. 15 st. 6 tač. 3 |

**And two non-storno stock differences from POPIS** (PEP čl. 16 st. 2 — „višak u kolonu zaduženja, a manjak u kolonu razduženja"): a popis **višak** = **positive** entry in **kolona 4**; a popis **manjak** = **positive** entry in **kolona 5**. Note the deliberate divergence: a **manjak found at popis → kolona 5**, whereas a **manjak „po odluci trgovca" → kolona 4 crveni storno**. Different cause, different column — expose as separate event types. (Interna prenosnica, PEP čl. 15 st. 5 tač. 2: receiver posts +col4, issuer posts a col4 storno; magnitudes may differ by zavisni troškovi — relevant only if multi-location is enabled.)

Encode each event with a **typed enum** from which the target column and sign are **derived**, never chosen. Route by legal basis, not by the bare token „vraćanje/return" (it means supplier-return in col 4 **and** contract-rescission in col 5).

### 4.4 Mid-year error correction (reversing storno)
**The Pravilnik has NO explicit „ispravka greške" article** (verified: strings „ispravk"/„greš" appear nowhere in the text). The lawful path is forced by **čl. 14 st. 1** („…na način koji ne dozvoljava brisanje unetih podataka, po hronološkom redu") + the general crveni-storno device: **do not edit or delete the posted row.** Append a **reversing crveni storno in the SAME column** (negative, referencing the erroneous RB in col 3), then append the correct line. Two new chronological rows; original untouched. Because čl. 11 st. 1 + čl. 14 mandate chronology, the correction bears the **current** dan.mesec — **never back-date** it to the error's day. (Flag for counsel: this is standard Serbian bookkeeping practice and the only defensible reading, but it is an **inference**, not verbatim rule — see §8.)

### 4.5 Worked storno examples (continuing §2.7; lot has 35 kom on hand)
- **Upward nivelacija** 156 → 176 (+20/kom) on 35 kom, basis popisna lista br. 4: **+700,00 in kolona 4**. Saldo 15.460 → 16.160.
- **Downward nivelacija** 156 → 136 (−20/kom) on 35 kom, basis popisna lista br. 5: **crveni storno 700,00 in kolona 4** (subtracted). Saldo 15.460 → 14.760.
- **Customer contract-rescission return** of 1 kom @ 156,00: **crveni storno 156,00 in kolona 5** (subtracted → col5 total falls). Saldo 15.460 → **15.616** (goods back on hand).
- **Error correction**: zaduženje mistyped 8.700 instead of 7.800 at RB 1. New rows: crveni storno 8.700,00 in kolona 4 (ref RB 1), then +7.800,00 in kolona 4. Net effect −900, original preserved.

---

## 5. Year-end freeze + tamper + print + retention

### 5.1 Pre-close popis (PEP čl. 16)
Before closing, popis robe „shodno poreskim propisima i propisima o računovodstvu" (st. 1), compared to `Σcol4 − Σcol5` (st. 2). Book any difference: **višak → kolona 4, manjak → kolona 5** (§4.3).

### 5.2 Zaključivanje (PEP čl. 17)
- st. 1: after all entries for the year, „vrši se zaključivanje Knjige evidencije."
- st. 2: „Krajnji saldo predstavlja vrednost robe koja se prenosi kao početno stanje u narednu godinu." → carry it as next year's opening zaduženje / DONOS.
- **st. 4 (electronic form)**: „zaključivanje se vrši **štampanjem početne strane i krajnjeg salda**, koji se overava potpisom računopolagača, odnosno preduzetnika." Build this exact **signable print** (početna strana + krajnji saldo), separate from the full-book print. **Signature only — no pečat/seal is required** by the current text; do not add one.

### 5.3 The čl. 18 LOCK — prevention, not detection
**PEP čl. 18**: „Knjiga evidencije koja se vodi elektronski na kraju poslovne godine štiti se na način da **nije moguća izmena listova ili delova evidencije** i da se **može u svakom trenutku odštampati**." „Nije moguća izmena" = a **hard immutability lock**, enforced at the **data layer** (no UPDATE/DELETE path against a closed year — append-only store, closed-year gate, or hash-sealed snapshot). **An audit trail that merely *detects* post-hoc tampering does NOT satisfy čl. 18.** The frozen year must remain **printable indefinitely**.

### 5.4 Ongoing (all-year) no-erase duty (PEP čl. 14)
- st. 1: „…ažurno, uredno i tačno, **na način koji ne dozvoljava brisanje unetih podataka**, po hronološkom redu." → KEP rows are **append-only from creation**, all year, not merely after close. **No hard delete, no in-place edit** of a posted row; the only undo is a reversing crveni storno (§4.4).
- st. 2: responsibility rests on „lice koje se zadužuje robom: **računopolagač, odnosno preduzetnik**" — for the pilot, the entrepreneur. Bind this identity to the book; stamp it on the print footer (ODGOVORNO LICE / M.P.). „Odnosno" = as-applicable (a pravno lice's računopolagač is not a preduzetnik).

### 5.5 Print-on-demand + availability at the point of sale
- **PEP čl. 10 st. 3**: kept electronically, „na njihov zahtev, vrši se štampanje podataka" for the control organs. Provide an **always-available full-book / date-range print** with numbered pages, per-page subtotals, DONOS/SVEGA ZA PRENOS, and the 5 columns.
- **ZoT čl. 30 st. 6**: the KEP must be „dostupna na prodajnom mestu" — viewable/printable **on the sales floor**.
- **Inspector walk-away copy (ZoT čl. 48)**: the prompt references čl. 48 as the basis for handing an inspector a copy. **This article was not verified in research** — treat as a design *nice-to-have* (a one-click export/print the inspector can take) but do **not** encode a specific čl.-48 requirement until the consolidated text is confirmed (§8). The solid, verified obligations are čl. 30 st. 6 (dostupna na prodajnom mestu) + PEP čl. 10 st. 3 (print on demand).

### 5.6 Retention (PEP čl. 19 → ZoRač čl. 28)
- PEP čl. 19: KEP + its dokumentacija kept „na način, na mestu i u roku u kojem se čuvaju **pomoćne knjige** u skladu sa propisima o računovodstvu." (The KEP is subjected to the pomoćna-knjiga regime; it is not literally reclassified as one — same outcome.)
- ZoRač čl. 28: st. 5 „**Pomoćne knjige čuvaju se pet godina, od dana njihovog zaključivanja**."; st. 7 „Pet godina se čuvaju **isprave** na osnovu kojih se unose podaci u poslovne knjige."; st. 9 „Rokovi … računaju se **od poslednjeg dana poslovne godine na koju se odnose**."
- **Safe anchor (conservative synthesis of st. 5 + st. 9):** retain KEP and every feeding isprava (kalkulacije, fakture/otpremnice, dnevni fiskalni izveštaji, popisne liste, nivelacija/storno docs) for **5 full years measured from the LATER of the zaključivanje date and 31 December of that business year.** Since zaključivanje occurs at/after year-end, the two anchors sit days apart; „later-of" never under-retains. Keep the clock **per book-year**. Printability itself is time-unbounded (čl. 18).

### 5.7 What must be immutable, and when
- **From creation (all year):** every posted KEP row (no delete, no in-place edit). Corrections only by appended reversing storno.
- **At year-end zaključivanje:** the entire closed year is **sealed read-only at the data layer** (čl. 18) — the application itself cannot mutate any list or part; still printable forever.
- **Never** auto-purge before the 5-year floor (§5.6).

---

## 6. Penalties + who is bound

**Who is bound.** A **retail preduzetnik is obligated unconditionally.** PEP čl. 2: „Evidenciju prometa vode pravna lica i preduzetnici … koji obavljaju: 1) **trgovinu na malo**; 2) trgovinu na veliko, a poslovne knjige ne vode po principu dvojnog knjigovodstva; …". The double-entry carve-out is attached **only to wholesale (tač. 2)** — retail (tač. 1) has **no** threshold or bookkeeping-method condition. **No paušalac exemption**: a paušalno oporezovani preduzetnik is exempt only from *double-entry* books (it still keeps a KPO knjiga); the KEP duty applies regardless of tax regime. → Make SW-9 KEP **default-on, non-optional** for every retail preduzetnik profile; never gate it behind a „paušal" or „wholesale" toggle.

**Penalty tiers (post-35/2026; čl. 67 st. 1 wholly replaced by amendment-law čl. 23):**

| Offense | Article | Pravno lice | Odgovorno/fizičko lice | **Preduzetnik** |
|---|---|---|---|---|
| KEP kept **improperly / incompletely** („ne vodi evidenciju prometa na potpun i propisan način (član 30)") | **ZoT čl. 67 st. 1 tač. 2**, st. 2, st. 3 | 100.000,00 (fixed) | 10.000,00 | **40.000,00 (fixed)** |
| **Not keeping the KEP at all** („ne vodi evidenciju prometa (član 30)") | **ZoT čl. 68 st. 1 tač. 7**, st. 3 | 500.000–2.000.000 | 50.000–150.000 | **50.000–500.000 (range)** |
| **Missing isprave o robi** („ne poseduje odgovarajuće isprave koje prate robu … (član 29)") | **ZoT čl. 68 st. 1 tač. 6** | 500.000–2.000.000 | 50.000–150.000 | 50.000–500.000 |

Plus, for čl. 68 offenses, a possible **zaštitna mera zabrane vršenja delatnosti od šest meseci do dve godine** (ZoT čl. 68 st. 6). Prekršaj limitation: **2 years** from the day of the offense (ZoT čl. 70 st. 1).

**Compliance stakes (the false-assurance harm):** a wrong booking rule that yields an *incomplete* KEP is the 40.000-din tier; a KEP the software **silently fails to produce or keep** is the far worse **50.000–500.000 din + activity-ban** tier. SW-9 must therefore treat KEP generation as a **hard availability guarantee** (data-loss-proof, always printable) and back **every** entry with a stored čl. 29 isprava, so neither čl. 68 tač. 6 nor tač. 7 can trigger.

---

## 7. What VantumPOS must build (and must NOT) + recommended decomposition

### 7.1 Core requirements
- A per-prodajno-mesto, per-year **append-only value ledger** with monotonic redni broj, the 5 columns, day+month display in col 2, and a derived running saldo.
- **Typed event model** where each event's `{column, sign}` is derived from a fixed cause→column map (§4.3), never user free-choice.
- **Zaduženje = retail value incl. PDV** (kalkulacija element 13); **razduženje = daily fiscal promet**.
- **Kalkulacija generator** carrying all **14** elements with enforced derivations; gate „stavljanje u promet" on the zaduženje existing.
- Timing guardrails: overdue-posting (T+1) warning; sell-before-entry block/flag with the čl. 11 st. 3 whitelist off by default.
- **Popis reconciliation** (book saldo vs physical; višak→col4, manjak→col5).
- **Year-end zaključivanje**: freeze (data-layer immutability), carry krajnji saldo → next-year DONOS, signable print of početna strana + krajnji saldo.
- **Print/export**: full-book & date-range, numbered pages, DONOS/SVEGA ZA PRENOS, M.P./ODGOVORNO LICE footer, on-demand and at the point of sale.
- **Retention**: 5-year floor per book-year (§5.6), no auto-purge before it.
- Store the entrepreneur (računopolagač) identity on the book.

### 7.2 Recommended decomposition (SW-9 is large — split into three sub-cycles)
- **SW-9a — Ledger + postings from existing receipts/sales.** The 5-column data model; redni broj sequencing; col-2 dan/mesec display; append-only enforcement; derived saldo; auto-generate col-4 zaduženje from existing goods-receipts and col-5 razduženje from the existing sales/fiscal daily total; opis-promene composition (doc naziv/broj/datum + dobavljač on nabavka); timing warnings + sell-gate. *(Depends on existing receipt + sales modules.)*
- **SW-9b — Kalkulacija + nivelacija + storno.** The 14-element kalkulacija document + derivations; the typed cause→column storno engine (§4.3) with basis-document capture; upward/downward nivelacija; supplier/komision/customer returns; error-correction (reversing storno) UX; popis višak/manjak.
- **SW-9c — Year-end freeze + print/export.** Pre-close popis reconciliation; zaključivanje (data-layer seal + krajnji-saldo carry-forward); electronic-close signable print; full-book/date-range print with page mechanics; retention clock. *(Depends on 9a/9b.)*

### 7.3 MUST-NOT list
- **Do NOT invent, rename, drop, or add KEP columns.** Exactly five, in order (§2.1).
- **Do NOT post zaduženje at nabavna/invoice/without-PDV value.** Retail = prodajna vrednost sa PDV (§2.2).
- **Do NOT ship a 13-field kalkulacija.** All 14 (§3).
- **Do NOT route a downward nivelacija (or any col-4 write-down) to kolona 5**, and do NOT route a customer contract-rescission return to kolona 4 (§4.3).
- **Do NOT implement the year-end freeze as tamper *detection* (audit log only).** čl. 18 requires *prevention* — data-layer immutability (§5.3).
- **Do NOT provide any edit/delete of a posted row** (all year). Correct only by appended reversing storno; never back-date it (§4.4, §5.4).
- **Do NOT auto-purge / roll off** any book-year before the 5-year floor (§5.6).
- **Do NOT gate the KEP behind a paušal/wholesale toggle** — retail preduzetnik is unconditionally bound (§6).
- **Do NOT print a year in col 2, a clock time, or a seal** on the electronic close.
- **Do NOT hard-code labels/element-list/columns** — keep them in a versioned, swappable rules layer (§1).

---

## 8. Still unverified

1. **ZoT čl. 48 — inspector „walk-away copy".** *Tried:* not covered by the completed research passes; the amendment inventory shows čl. 46 and čl. 49 were touched by 35/2026 but čl. 48 was not individually pinned. *Question for counsel:* „Does ZoT čl. 48 (consolidated 52/2019 i 35/2026) require the trgovac to hand the inspector a copy/printout of the evidencija, and in what form/scope?" *Risk of proceeding:* low for the ledger itself — the verified čl. 30 st. 6 (dostupna na prodajnom mestu) + PEP čl. 10 st. 3 (print on demand) already require an on-demand printout. Only the specific „give the inspector a copy to take" affordance is unconfirmed; ship the export button, don't encode a čl.-48 legal claim.

2. **Successor Pravilnik / updated kontrolna lista.** *Tried:* searched paragraf.rs, Sl. glasnik, ministry — none published as of July 2026; ZoT čl. 30 st. 8 delegation is live but sets no deadline. must.gov.rs (rebranded Ministarstvo unutrašnje i spoljne trgovine) was DNS-unreachable during research, so a *draft* in javna rasprava could not be excluded. *Question for Ministarstvo:* „Is a new Pravilnik o evidenciji prometa or an updated KEP kontrolna lista in procedure?" *Risk:* low near-term; keep the rules layer data-driven and re-check before GA. If one issues, the 14-element list, column labels, and posting rules may change.

3. **Crveni-storno print rendering for an electronic KEP.** *Tried:* the text prescribes the *manual-book* convention „ispisivanje iznosa koji se zaokružuje" (circled/red) and „oduzima se od ukupnog zbira"; there is no electronic-format spec beyond čl. 18. *Question for accountant/inspector:* „For an electronic KEP printout, is red text / parentheses / an explicit „storno" marker the accepted visual for a crveni storno?" *Risk:* low — store a signed negative that subtracts from the column total; render it visually distinct (parentheses + red). Any of the three conventions should pass; pick one and be consistent.

4. **Mid-year error-correction method (reversing storno).** *Tried:* exhaustive text search — the Pravilnik has **no** explicit ispravka-greške article; the storno-reversal path is an inference from čl. 14 (no deletion) + the general storno device. *Question for accountant:* „Confirm that a reversing crveni storno + re-entry (same column, current date) is the accepted KEP correction for a data-entry error." *Risk:* low — this is standard practice and the only method consistent with čl. 14, but get a one-line sign-off since it is not verbatim.

5. **Exact consolidated wording/numbering of ZoT čl. 29 st. 1 and čl. 30 stavovi (post-35/2026).** *Tried:* substance confirmed (isprave data; per-prodajno-mesto; dostupna na prodajnom mestu; delegation), and the st.-map (st.1 duty, st.2 per mesto, new st.5 daljinska, st.6 dostupna, st.8 delegation) is corroborated by the amendment note; but one WebFetch summarizer paraphrased čl. 30, and the čl. 29 st. 1 label list was read from neobilten. *Question:* pin čl. 29 st. 1 and čl. 30 stav numbers against an official prečišćen text before hard-coding any label strings. *Risk:* low — these drive UI labels and citations, not ledger math.

6. **44/2018 „dr. zakon" provenance.** *Tried:* confirmed the operative text and every load-bearing rule verbatim; did not separately identify which 44/2018 law effected the „dr. zakon" change (it is a cross-reference update, not a rewrite of the kalkulacija list or storno rule). *Risk:* negligible for implementation; note only for a full provenance trail.