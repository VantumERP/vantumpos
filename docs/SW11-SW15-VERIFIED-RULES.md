# SW-11 / SW-15 — Verified Rule Set

Scope: encodable rules for the **cash & product compliance trio (SW-11)** — AML gotovinski prag, polog dnevnog pazara, deklaracija/GTIN — and the **onboarding profile (SW-15)** — pravna forma, PDV status, ESIR/PFR provera, retention profile. Companion to [KEP-VERIFIED-RULES.md](KEP-VERIFIED-RULES.md), [ZOT-36-37-VERIFIED-RULES.md](ZOT-36-37-VERIFIED-RULES.md) and [ZZP-REKLAMACIJE-VERIFIED-RULES.md](ZZP-REKLAMACIJE-VERIFIED-RULES.md).

**State of law: 31 July 2026.** Supersedes the corresponding rows of `docs/SERBIAN-LAW-COMPLIANCE.md` (§2 rows 3, 9, 12, 14, 21, 27, 28, 29, 32 and §3 SW-3, SW-11, SW-15), which are corrected in place against this memo.

**Method and epistemic status.** Six research findings were each attacked by an independent refutation working from primary text. Where the refutation was material or fatal, the refutation wins unless the finding carried a verbatim primary quote the refutation left unaddressed. Anything neither pass could settle from primary text is marked **UNRESOLVED** and carries no number. Two of the six findings were formally refuted (Q3 material, Q4 fatal); their surviving corrections are adopted, their broken premises are not.

**Reading key for §3:** `[LEGAL]` = statutory duty · `[LEGAL-INFERRED]` = follows from statutory text by construction, not stated in terms · `[ACCURACY]` = exists so the product does not misstate the law to a shop owner · `[PRUDENTIAL]` = good practice, **not** a legal duty and must never be presented as one.

**Pilot subject assumed throughout:** a registered retail **preduzetnik** (boutique, Novi Pazar). Where a d.o.o. profile changes the answer, both are given.

---

## 1. Headline corrections

The doc was **not** right about everything. Fourteen defects, four of them capable of putting a wrong number or a false legal duty in front of a shop owner.

| # | Doc location | Wrong claim | Correct rule | Article | Severity |
|---|---|---|---|---|---|
| 1 | Row 28, line 82 | AML cash cap → "**privredni prestup up to 2M RSD**" for the shop | A preduzetnik **cannot commit a privredni prestup at all**. Correct: **prekršaj, 100.000–300.000 RSD**, and 300.000 is a *base* maximum, not a ceiling — the law expressly permits a proportional uplift | ZPP čl. 6 st. 1; AML čl. 120 st. 2; uplift AML čl. 120 st. 8 + ZoP čl. 39 st. 4 | **CRITICAL** |
| 2 | Row 28, line 82 | "**<EUR 10.000**" (permitted-range framing) | Statute prescribes the **ban** side with an inclusive operator: **≥ 10.000 EUR is already unlawful** ("u iznosu od 10.000 evra **ili više**"). A `> threshold` comparison is a compliance bug | AML čl. 46 st. 1 | **HIGH** |
| 3 | Row 27, line 81 | "neuredna deklaracija **100k fixed** / bez deklaracije **500k–2M** RSD" | Both are pravno-lice tiers. Preduzetnik: **fixed 40.000** (defective declaration) and **50.000–500.000 + a possible 6-month-to-2-year zabrana vršenja delatnosti** (no declaration). The doc overstates the fine 4× at the top and **omits the shop-closing sanction entirely** | ZoT čl. 67 st. 3; čl. 68 st. 3; čl. 68 st. 6 | **CRITICAL** |
| 4 | Line 23 (§1a) and §5 shop-economics (line 172) | ESIR recharacterization → "shop — prekršaj **300.000–2.000.000 RSD** (ZF čl. 15 st. 1 tač. 3)" | Article and tačka are right; the amount is the pravno-lice tier (čl. 15 **st. 1**). Preduzetnik: **50.000–500.000** (čl. 15 **st. 3**). 4× overstated at both ends, in the document's central risk paragraph | ZF čl. 15 st. 1 vs st. 3 | **CRITICAL** |
| 5 | SW-3, line 100 | Reset/restore warning must cite the "**archive-law destruction-approval duty for d.o.o. shops**" | **No such duty exists for a private company.** ZAG čl. 16 st. 2 confines prior written archive approval to state organs, TA/LSG organs, ustanove, javna preduzeća and imaoci javnih ovlašćenja. A private pravno lice needs the archive's *saglasnost on the lista kategorija*, an *arhivska knjiga*, and the *30 April prepis* — a different set of duties | ZAG čl. 16 st. 2; čl. 14; čl. 9 st. 2 tač. 6–7 | **HIGH** |
| 6 | Row 21, line 74 + SW-15, line 117 | PDV status "**gates the 10y floor**" | PDV changes **which statute supplies** the floor, not whether it exists. Non-PDV shop keeping dvojno knjigovodstvo: 10 years for dnevnik i glavna knjiga under ZoRač. Gating on PDV would shorten retention for a non-PDV shop | ZoRač čl. 28 st. 4; ZPDV čl. 47 + ZPPPA čl. 114ž | **HIGH** |
| 7 | Row 21, line 74 + SW-3, line 100 | "10 years" attributed to **ZPDV čl. 47**; "10y absolute" presented as a ceiling | ZPDV čl. 47 sets **no general period** — it defers to zastarelost. The 10-year figure comes from **ZPPPA čl. 114ž**. And 10 years is **not a wall-clock ceiling**: zastoj periods are excluded from the absolute period, and čl. 114ž itself ends "osim ako ovim zakonom nije drukčije propisano". čl. 47's own "najmanje deset godina" is object-specific and runs **"po isteku kalendarske godine"** from first use / completion of čl. 32 objekti | ZPDV čl. 47; ZPPPA čl. 114ž, čl. 114z st. 2 | **HIGH** |
| 8 | Row 14, line 67 | Overtime record → "prekršaj **up to 300k RSD**" | Pravno-lice ceiling. Preduzetnik: **50.000–150.000** | ZoR čl. 276 st. 1 | **HIGH** |
| 9 | Row 9 line 62 + line 44 | čl. 47 records → "**fixed 100.000 RSD**, čl. 95 st. 2 tač. 5" | čl. 95 st. 2 is expressly "rukovalac … koji ima svojstvo **pravnog lica**". Preduzetnik: **fixed 50.000** (čl. 95 **st. 6**) | ZZPL čl. 95 st. 6 | **HIGH** |
| 10 | Row 12 line 65 + line 43 + F-3 line 140 | Privacy-by-design / čl. 23 omission → "**50.000–2.000.000 RSD**" | Pravno-lice tier (čl. 95 st. 1). Preduzetnik: **20.000–500.000** (čl. 95 st. 4). Same for a čl. 5 principle breach, whose tačka is **čl. 95 st. 1 t. 1** | ZZPL čl. 95 st. 1 vs st. 4 | **HIGH** |
| 11 | Row 27, line 81 | Frames čl. 34 as a duty on **the shop** to mark goods; status column says "**Product table lacks GTIN field**" | Two errors. (a) Creating the deklaracija and applying the mark is the duty of the **proizvođač / uvoznik** (st. 2); the retailer's exposure is a **selling** offence — but the doc also misses that **čl. 34 st. 5 does bind the trgovac directly in daljinska trgovina**, and that 35/2026 **strengthened** it (added "istaknu deklaraciju sa podacima iz stava 1."). (b) `barcode TEXT UNIQUE` **already exists** at `/Users/adnan/Projects/vantumpos/src-tauri/src/db/migrations.rs:68`; the real gaps are manufacturer / importer / country-of-origin, absent from the whole migrations file | ZoT čl. 34 st. 2, st. 5; migrations.rs:68 | **HIGH** |
| 12 | Row 32, line 86 | "Archive law (**pravna lica only; preduzetnici exempt**)" | Overstated. ZAG čl. 65 has **no preduzetnik tier**, so there is no enforceable fine — but only čl. 9 **st. 2** carries the "osim fizičkih lica" carve-out. **čl. 9 st. 1 (savesno čuvanje u sređenom i bezbednom stanju) and st. 4 have no carve-out.** No penalty ≠ no duty | ZAG čl. 9 st. 1, st. 2, st. 4; čl. 65 | **MEDIUM** |
| 13 | Row 3 line 56 + line 23 | ZF closure measure → "15/90/**365 days**", listed as a consequence of the recharacterization risk | Statute says "15 dana / 90 dana / **jedne godine**". More importantly the **predicate is only failure to record each individual retail sale via the EFU** (čl. 12 st. 1) — it does not attach to a registry-check or recharacterization failure. A separate *privremena zabrana* for ignoring a rešenje sits in čl. 13 st. 4 and is unmentioned | ZF čl. 12 st. 1–3; čl. 13 st. 4 | **MEDIUM** |
| 14 | SW-3 line 100 (legal driver) | Lists **ZPPPA čl. 175b** as a retention driver | čl. 175b is the **criminal offence of trading in evasion-enabling accounting software**. It is not a retention provision. The retention penalty for tax records is **ZPPPA čl. 178b** (preduzetnik 50.000–500.000) — ZPDV čl. 60/60a, the old PDV fine, has **ceased to be in force** | ZPPPA čl. 178b; ZPDV "Čl. 60 i 60a (Prestalo da važi)" | **MEDIUM** |

**Lower-severity precision defects** (fix in place, no product impact): row 28 cites "čl. 118 t. 43" → should be **čl. 118 st. 1 tač. 43**, and omits the d.o.o.'s second exposure (odgovorno lice 10.000–150.000, čl. 118 st. 2) · row 29 omits the **odgovorno lice tier 5.000–150.000 (čl. 7 st. 2)** and cites "čl. 7" where the offence is **čl. 7 st. 1 tač. 2)** · the abbreviation **"ZOP"** conventionally denotes *Zakon o platnom prometu*, the repealed act that carried the old next-working-day rule — use the full title + 68/2015 · row 17's KEP pin-cite "PEP čl. 2" should be **PEP čl. 2 st. 3** · row 27's "31 Jan 2020" is a derivation (čl. 73 says only "po isteku šest meseci"; in force 30.07.2019) — say **"end-January 2020"** · SW-9 (line 111) says "all **13** PEP čl. 15 st. 5 tač. 1 elements" while row 18 (line 71) says the count is **fourteen** and that "13" was a miscount; SW-9 is stale against the doc's own correction (neither Q1–Q6 re-verified the count — treat row 18 as governing and re-verify before build) · §6 item 9 can be **half-closed**: the legal rule on one-year aggregation is now settled; only the enforcement-practice half remains open.

### 1.1 What survived verification unchanged — do not touch

- **Row 29 (7-working-day cash deposit).** Both tiers exact: 50.000–2.000.000 pravno lice / 10.000–500.000 preduzetnik. The rival "istog dana, a najkasnije narednog radnog dana" rule was tested and is the **repealed** predecessor.
- **Row 27 dating.** GTIN/QR under ZoT čl. 34 st. 8 is **old law**, applicable since end-January 2020, and 35/2026 added exactly one word ("identifikacionom"). Confirmed verbatim against the amending act.
- **Row 24 (sniženje) and row 22a (reklamacija penalty).** Fixed 100.000 / 40.000 / 10.000 under čl. 67 st. 1 t. 8 with no zaštitne mere; ZZP 88/2021 čl. 188 st. 3 = 30.000 → ZZP 35/2026 čl. 210 st. 3 = 100.000. The only fine amount currently rendered in the shipped UI is correct.
- **Row 22's cutover.** "1 or 2 Aug 2026 — UNRESOLVED" can be **closed**: ZZP 35/2026 čl. 220 defers application three months from entry into force (01.05.2026) and čl. 219 repeals 88/2021 only on početak primene → **01.08.2026**. `CUTOVER_DATE = 2026-08-01` is right on the merits, not merely as a safe default.
- **Row 5 / posture rule 4.** ZPPPA čl. 175a's cumulative elements ("not in the register **AND** serving to avoid recording retail turnover") are correctly stated.
- **Row 30 (SEF retail carve-out).** The doc already carries the two "osim za" exceptions and the 7-day window. **The Q5 research proposed replacing this with "retail movement is carved out of both" — that would be a regression. Reject it.**

---

## 2. Verified rules, per topic

### Q1 — AML cash-acceptance cap

**Encodable rule.** Any person selling goods or real estate or providing services in Serbia must not **receive** cash from a customer or third party for that payment in an amount of **10.000 EUR or more** in dinar countervalue (operator `>=`). The ban covers one or several mutually linked cash transactions, or one or several contracts **within a period of one year**; the amount must instead be paid into a bank account. A cash deposit made by the buyer **into the seller's own bank account** is not "receiving cash" as to the seller. The trigger is the **cash tendered**, not the invoice total — a 15.000 EUR sale settled 5.000 cash + 10.000 card does not breach st. 1.

**In force.** The 10.000 cap **and** the linked-cash-transaction aggregation have applied since 113/2017. The **multi-contract / one-year limb, the mandatory-bank-transfer command, the st. 2 loan/real-estate extension and the st. 3 carve-out are new in 94/2024, in force 06.12.2024.** (The research's claim that the whole aggregation rule is new is wrong; the pre-amendment text aggregated linked cash transactions already.)

> **čl. 46 st. 1** — "Lice koje se bavi prodajom robe i nepokretnosti ili vršenjem usluga u Republici Srbiji ne sme od stranke ili trećeg lica da primi gotov novac za njihovo plaćanje u iznosu od 10.000 evra ili više u dinarskoj protivvrednosti, bez obzira na to da li se radi o jednoj ili više međusobno povezanih gotovinskih transakcija ili jednom ili više ugovora u periodu od godinu dana, već se navedeni novčani iznos mora uplatiti na račun otvoren kod banke."
> **čl. 46 st. 3** — "Uplata gotovog novca … na račun lica iz st. 1. i 2. ovog člana, otvorenog kod banke se ne smatra primanjem gotovog novca u smislu ovog člana u odnosu na ta lica."
> **čl. 110 st. 6** — "Ministarstvo nadležno za inspekcijski nadzor u oblasti trgovine vrši nadzor nad primenom odredbe člana 46. stav 1. ovog zakona."
> **čl. 8 st. 1 tač. 2** (the definitional hook for currency conversion) — "… po zvaničnom srednjem kursu Narodne banke Srbije na dan izvršenja transakcije (u daljem tekstu: u dinarskoj protivvrednosti)"
> Source: Zakon o sprečavanju pranja novca i finansiranja terorizma, "Sl. glasnik RS" br. 113/2017, 91/2019, 153/2020, 92/2023, 94/2024, 19/2025 — https://www.paragraf.rs/propisi/zakon_o_sprecavanju_pranja_novca_i_finansiranja_terorizma.html (PDF: https://www.paragraf.rs/propisi_download/zakon_o_sprecavanju_pranja_novca_i_finansiranja_terorizma.pdf). Amending act 94/2024: https://www.parlament.gov.rs/upload/archive/files/lat/pdf/zakoni/14_saziv/2656-24%20-%20Lat..pdf
> *Do not cite the mfin.gov.rs .docx (now returns an error page) or apml.gov.rs/REPOSITORY/458_zospnft.pdf (that is the superseded 2009 law).*

**Binds whom.** **Every trader**, not only AML obveznici. čl. 46 says "**Lice**" where čl. 44 and čl. 45 in the same subsection say "Obveznik ne sme"; čl. 120 st. 5–7 punish natural persons who can never be obveznici. **The counter-argument that must be recorded and answered, not ignored:** čl. 116 frames čl. 117–120 as the penalties for breaches "od strane obveznika iz člana 4. stav 1. tačka 1) … i člana 4. st. 2. i 3." A strict-construction defence would run through čl. 116 — it fails because čl. 120 st. 5–7 reach categorically non-obveznik natural persons, so čl. 116 cannot be exhaustive. Supervision of st. 1 is the **tržišna inspekcija** (čl. 110 st. 6) — the same body that checks KEP, cenovnik and sniženje.

**The shop is NOT an obveznik** under čl. 4, so it has **no** čl. 47 st. 1 reporting duty (the separate 15.000 EUR trigger), no CDD, no ovlašćeno lice, no lista indikatora. **Scope trap:** selling **art objects**, precious metals or stones or products thereof with payments of 10.000 EUR or more makes it a full obveznik under **čl. 4 st. 1 tač. 19 podtač. (3)** — and that provision's own threshold uses "po **zvaničnom kursu** Narodne banke Srbije **na dan plaćanja**", a different formula from čl. 8's srednji kurs on the transaction date. Podtač. (4) adds art-object storage/trade in free zones, ports and warehouses.

**Penalties** (hook: čl. 118 st. 1 tač. 43):

| Subject | Category | Base range | Uplift |
|---|---|---|---|
| Pravno lice | **privredni prestup** | 100.000–2.000.000 (čl. 118 st. 1 t. 43) | čl. 119a → ZPP čl. 18 st. 2, up to 20× value |
| Odgovorno lice u pravnom licu | privredni prestup | 10.000–150.000 (čl. 118 st. 2) | — |
| **Preduzetnik** | **prekršaj** | **100.000–300.000 (čl. 120 st. 2)** | čl. 120 st. 8 → ZoP čl. 39 st. 4, up to 20× value, capped at 5× the ZoP čl. 39 st. 1 maximum → **derived ceiling 2.500.000** |
| Fizičko lice (non-business) | prekršaj | 50.000–150.000 (čl. 120 st. 4) | čl. 120 st. 8 |
| Fizičko lice, zajam / kupoprodaja nepokretnosti (st. 2) | prekršaj | 50.000–150.000 (čl. 120 st. 5) | enacted at 5.000–50.000 by 94/2024, raised by 19/2025 |

**Can a preduzetnik be the subject of a privredni prestup? NO** — see Q6. The 2.000.000 ceiling is legally unavailable against this shop; the base range is 100.000–300.000. **But do not present 300.000 as a maximum** — the derived uplift ceiling (2.500.000) exceeds the doc's own 2M figure. UI copy: *"od 100.000 do 300.000 RSD, a u srazmeri s vrednošću robe i više."*

**Limitation.** Preduzetnik prekršaj: ZoP čl. 84 st. 1 — 1 year, absolute 2 (st. 7). AML prescribes no longer period. Pravno lice privredni prestup: ZPP čl. 37 st. 1 — 3 years.

**Zaštitna mera.** AML čl. 120 **prescribes none**. Whether ZoP čl. 55 st. 2 could nonetheless supply one is **UNRESOLVED** (§5).

**Currency conversion — `[LEGAL-INFERRED]`, not verbatim.** čl. 46 names no rate and no valuation date. The conclusion "NBS zvanični srednji kurs on the day the transaction is executed" is imported from čl. 8 st. 1 tač. 2's "(u daljem tekstu: u dinarskoj protivvrednosti)" legislative shorthand — a strong construction, confirmed to exist verbatim — but čl. 4 st. 1 tač. 19 uses a divergent formula, showing the drafters were not consistent. See §5.

---

### Q2 — Seven-working-day cash deposit

**Governing act.** *Zakon o obavljanju plaćanja pravnih lica, preduzetnika i fizičkih lica koja ne obavljaju delatnost*, "Sl. glasnik RS" **br. 68/2015 — single gazette entry, no amendments** as at 31.07.2026 (PIS register renders the title as "68/2015-3"; mfin.gov.rs lists 68/2015 alone). Applied from 01.10.2015. **Do not abbreviate it "ZOP".**

**Encodable rule.** Pravna lica and preduzetnici must pay dinars received in cash **"po bilo kom osnovu"** onto their own tekući račun **within seven working days**. **All** cash received on **any** basis is caught — not merely customer takings. Partial deposits are lawful; the statute requires the full amount on the account by the deadline, not a single instalment.

> **Zakon 68/2015, čl. 3 st. 1** — "Pravna lica i preduzetnici su dužni da dinare primljene u gotovom po bilo kom osnovu uplate na svoj tekući račun u roku od sedam radnih dana."
> **čl. 3 st. 2** (only statutory carve-out) — "Odredba stava 1. ovog člana ne primenjuje se na lica iz tog stava koja imaju ovlašćenje nadležnog organa za obavljanje menjačkih poslova…"
> **čl. 3 st. 3** (why the duty is not a cash-flow trap) — the bank must pay a preduzetnik cash back out immediately and free of charge up to 600.000 RSD; the excess by the next working day, still free.
> **čl. 6** — "Nadzor nad primenom odredaba ovog zakona kod pravnih lica i preduzetnika vrši Ministarstvo finansija – Poreska uprava."
> **čl. 7 st. 1 tač. 2)** — "ako dinare primljene u gotovom po bilo kom osnovu ne uplati na svoj tekući račun u roku od sedam radnih dana (član 3. stav 1);"
> Source: https://www.paragraf.rs/propisi/zakon_o_obavljanju_placanja_pravnih_lica_preduzetnika_i_fizickih_lica_koja_ne_obavljaju_delatnost.html · Cyrillic cross-check: https://www.neobilten.com/zakon-o-obavljanju-placanja-pravnih-lica-preduzetnika-i-fizickih-lica-koja-ne-obavljaju-delatnost/

**Bylaw layer (kept alive by čl. 8 st. 2).** *Pravilnik o uslovima i načinu plaćanja u gotovom novcu u dinarima…*, "Sl. glasnik RS" br. 77/2011.

> **Pravilnik čl. 5 st. 1** — "Pravna lica i fizička lica koja obavljaju delatnost, najkasnije u roku od sedam radnih dana, gotov novac primljen po bilo kom osnovu uplaćuju na svoj račun kod banke."
> **Pravilnik čl. 5 st. 2** — "Pod gotovim novcem u smislu stava 1. ovog člana ne podrazumeva se iznos dinara koji je isplaćen sa tekućeg računa **u skladu sa članom 2. st. 2. i 3. ovog pravilnika**."
> **Pravilnik čl. 2 st. 3** — "Izuzetno od stava 2. ovog člana, plaćanja, odnosno isplate do iznosa od 150.000 dinara dnevno vršiće se bez podnošenja na uvid dokumentacije iz tog stava."
> **Pravilnik čl. 3 st. 1** — withdrawals over 1.500.000 RSD must be announced to the bank three days in advance, except payroll.
> Source: https://www.paragraf.rs/propisi/pravilnik_o_uslovima_i_nacinu_placanja_u_gotovom_novcu_u_dinarima_za_pravna_lica_i_za_fizicka_lica.html

**Float exclusion — carries a condition the research dropped.** The exclusion covers only dinars paid out **in compliance with Pravilnik čl. 2 st. 2** (against original documentation submitted to the bank na uvid i overu) **or čl. 2 st. 3** (the 150.000 RSD/day undocumented lane). **Flag it as bylaw-level relief:** the statutory duty and the penalty contain no exclusion at all, and čl. 8 st. 2 preserves the Pravilnik only "ukoliko nije u suprotnosti sa ovim zakonom" — a carve-out narrowing a statutory "po bilo kom osnovu" duty is exactly what that clause reaches. The app may rely on it; the doc must not call it settled.

**Binds whom.** "Pravna lica i preduzetnici", flatly — **including a paušalac and regardless of PDV status**; the statute keys to the *status* preduzetnik, not the tax regime. Only licensed menjači are carved out. Fizička lica koja ne obavljaju delatnost are outside čl. 3.

**Penalties** (all **prekršaj**; the act contains **no** privredni prestup):

| Subject | Range | Article |
|---|---|---|
| Pravno lice | 50.000–2.000.000 | čl. 7 st. 1 tač. 2) |
| Odgovorno lice u pravnom licu | 5.000–150.000 | čl. 7 st. 2 |
| **Preduzetnik** | **10.000–500.000** | čl. 7 st. 3 |

**Can a preduzetnik be the subject? YES** — as a prekršaj, personally, under st. 3. There is no separate odgovorno-lice fine for him. Supervisor: **Poreska uprava**. No zaštitna mera is prescribed.

**Blagajnički maksimum: none exists.** The strings "blagajn" and "maksimum" appear **zero times** in Zakon 68/2015, in Pravilnik 77/2011 čl. 1–7, and in the 2011 ZPP amendment bill. No propis prescribes a cash-on-hand ceiling for a privredno društvo or a preduzetnik. Any "Odluka o blagajničkom maksimumu" is an internal act with no statutory number and no prekršaj attached.

**Corrected history** (use this; the research's version was wrong on instrument, date and stav): the "istog dana, a najkasnije narednog radnog dana" rule was **Zakon o platnom prometu čl. 32 stav 3**. It was replaced by the seven-working-day rule on **17 May 2011** by *Zakon o izmenama i dopunama ZPP*, "Sl. glasnik RS" br. 31/2011 (published 09.05.2011), which put the new rule in čl. 32 stav 4 and simultaneously downgraded the offence from privredni prestup to prekršaj. Pravilnik 77/2011 (in force 22.10.2011) **restated** an already-statutory rule; it did not originate it. The Zakon o platnim uslugama (139/2014) later terminated ZPP čl. 3–46 wholesale ("Čl. 3-46 (Prestalo da važi)"). **Bottom line unchanged: no "same day / next working day" pazar deadline has existed for an ordinary retailer since 17 May 2011.**

**"Radni dan" is undefined.** Neither act defines it, and there is no borrowable definition — the phrase occurs exactly **once** in the whole Zakon o platnim uslugama (139/2014, 44/2018, 64/2024), in the platni-sistem operating-rules article, with no definition.

---

### Q3 — Deklaracija and the machine-readable identification mark

**Encodable rule — the duty-holder split is three-way, not two-way.**

- **Creating the deklaracija and marking the goods:** **proizvođač**, or **uvoznik** for imported goods (st. 2). A reselling boutique does not generate declarations or assign GTINs for stock she buys in.
- **BUT st. 2 is not the only stav naming an obligee.** **st. 5 expressly binds the trgovac in daljinska trgovina** — and 35/2026 **strengthened** it. Pre-amendment the trgovac had only "da učine dostupnim podatke iz stava 1."; the current text adds "da **istaknu deklaraciju sa podacima iz stava 1. ovog člana i te podatke** učine dostupnim." **This is the one genuinely new retailer-facing obligation in čl. 34 on 1 May 2026** — the research treated it as a non-event.
- **st. 3** — the retailer may not alter or remove declaration data on goods in retail.
- **st. 4** — conspicuous display at the point of sale, one of three ways; tač. 3 permits a catalogue or other material free to consumers at the sales point, *"pre kupovine na način na kojem se potrošači ne dovode u zabludu"*.
- **The retailer's liability is a SELLING offence** under both penalty tačke ("prodaje robu…"), so it cannot be defended by pointing upstream at the supplier. The practical control is a **goods-receipt gate**, not a marking function.
- **st. 9–10 (jedinstveni šifarnik robe) bind the Ministarstvo and the Ministar, not the shop.** The amending act sets **no deadline** for that pravilnik and has no article past čl. 27.

> **čl. 34 st. 2** — "Proizvođač, odnosno za robu iz uvoza uvoznik, dužan je da snabde robu deklaracijom sa tačnim podacima iz stava 1. ovog člana."
> **čl. 34 st. 5** (current) — "U daljinskoj trgovini trgovci su dužni da istaknu deklaraciju sa podacima iz stava 1. ovog člana i te podatke učine dostupnim potrošaču pre kupovine u obliku i na način koji je neposredno i stalno dostupan."
> **čl. 34 st. 3** — "Podaci iz deklaracije za robu koja se nalazi u trgovini na malo ne mogu da se menjaju ili uklanjaju."
> **čl. 34 st. 8** (as amended 35/2026) — "Roba pored podataka iz stava 1. ovog člana mora biti označena mašinski čitljivom identifikacionom oznakom (GTIN identifikacijom, QR kodom i dr.)."
> **35/2026 čl. 11** — "U stavu 8. posle reči: „čitljivom" dodaje se reč: „identifikacionom". Posle stava 8. dodaju se novi st. 9. i 10 […] Dosadašnji st. 9. i 10. postaju st. 11. i 12."
> **ZoT (52/2019) čl. 73** — "…osim odredbe člana 34. stav 8. ovog zakona koja će se primenjivati po isteku šest meseci od dana stupanja na snagu ovog zakona."
> Sources: https://www.paragraf.rs/propisi/zakon_o_trgovini.html · amending act 35/2026: https://www.paragraf.rs/izmene_i_dopune/230426-zakon-o-izmenama-i-dopunama-zakona-o-trgovini.html · pre-amendment 52/2019: https://propisi.net/zakon-o-trgovini/

**Dating.** ZoT 52/2019 published 22.07.2019, in force 30.07.2019; čl. 73 delayed st. 8 by six months → applicable since **end-January 2020**. It is **not** a 2026 obligation. Sl. glasnik RS 35/2026 dated 23.04.2026, čl. 27 an unqualified eighth-day clause → **01.05.2026**. Old st. 9–10 became st. 11–12: a **renumbering trap** for any pre-May-2026 citation.

**čl. 34 st. 1 lists SEVEN data points**, not six: naziv · vrsta · tip i model · količina u jedinici mere ili komadu · poslovno ime proizvođača · (for imports) poslovno ime uvoznika · zemlja proizvodnje. Per st. 7, the literal value **"EU"** is valid where the item is produced in an EU member state — so do not validate country of origin against an ISO-3166 list alone. Per st. 6 the data must be in Serbian, Cyrillic or Latin script.

**Penalties.**

| Offence | Pravno lice | Odgovorno / fizičko lice | **Preduzetnik** | Zaštitna mera |
|---|---|---|---|---|
| Sells with a **defective** declaration — čl. 67 st. 1 tač. 6) *(was tač. 4 under 52/2019)* | **fixed 100.000** | fixed 10.000 | **fixed 40.000** (st. 3) | **None prescribed** — čl. 67 has only three stavovi |
| Sells **with no** declaration — čl. 68 st. 1 tač. 9) | 500.000–2.000.000 | 50.000–150.000 | **50.000–500.000** (st. 3) | **Yes — zabrana vršenja određene delatnosti, 6 months to 2 years (čl. 68 st. 6)** |

**Can a preduzetnik be the subject? YES, of both — as prekršaji.** The čl. 68 route also reaches him with the activity ban, which for a one-shop boutique is the real risk, not the dinar amount. Because čl. 67 is a **fixed** amount, ZoP čl. 168 st. 1 puts it in prekršajni-nalog territory and st. 2 **bars** the ordinary court route; čl. 68's range goes to court. Limitation: ZoT čl. 70 st. 1 — **2 years** (absolute 4 via ZoP čl. 84 st. 7).

**Do not write "no amount changed in 2026."** 35/2026 čl. 25 cut ZoT čl. 69 st. 1 from 50.000–500.000 to **50.000–150.000** (fizičko lice trading without being a registered trgovac). It does not touch a registered preduzetnik, but the blanket claim is false. 35/2026 čl. 26 added **čl. 69a** (mitigating/aggravating circumstances, incl. "radnje koje je trgovac preduzeo kako bi ublažio … štetu") — real effect on the čl. 68 range, almost none on čl. 67's fixed sums; a timestamped goods-receipt check has genuine mitigation value there.

**Is a bare missing GTIN punishable, and under which tier? UNRESOLVED.** Neither penalty tačka names st. 8; both are drafted around the *deklaracija*, and st. 8 frames the mark as "pored podataka iz stava 1." The tačke's bare "(član 34)" cuts the other way. Stakes: fixed 40.000 vs 50.000–500.000 + shop closure. **Attach no number.**

**Structural caveat.** The 12-stav count of čl. 34 rests on a single consolidated source; an independent copy renders it with 11. The amending act adds "novi st. 9. i 10." in the plural and renumbers two stavovi, which resolves it in favour of 12 — but the count is single-sourced.

---

### Q4 — Retention

**⚠ The single most important correction in this document.** The research proposed adding a "paušal / prosto knjigovodstvo vs dvojno knjigovodstvo" onboarding branch putting a boutique on a flat 5-year floor. **That branch is legally unavailable to a retail shop and must not be built.**

> **ZPDG čl. 40 st. 2** — "Pravo na paušalno oporezivanje ne može se priznati obvezniku iz stava 1. ovog člana: … 2) koji obavlja delatnost iz oblasti: **trgovine na veliko i trgovine na malo**, hotela i restorana … 5) koji je **evidentiran kao obveznik poreza na dodatu vrednost**…"
> **ZPDG čl. 40 st. 3** (the only retail exception) — "…obvezniku koji trgovinsku ili ugostiteljsku delatnost obavlja u **kiosku, prikolici ili sličnom montažnom ili pokretnom objektu** može se, na njegov zahtev, odobriti da porez plaća na paušalno utvrđen prihod."
> **ZPDG čl. 43 st. 2** — "Preduzetnik iz člana 32. stav 2. ovog zakona koji porez plaća na stvarni prihod, **vodi knjige po sistemu dvojnog knjigovodstva** u skladu sa zakonom kojim se uređuje računovodstvo."
> **ZPDG čl. 43 st. 3** — prosto knjigovodstvo is confined to "preduzetnik poljoprivrednik i preduzetnik drugo lice" (čl. 32 st. 3 i 4).
> Source: https://www.paragraf.rs/propisi/zakon_o_porezu_na_dohodak_gradjana.html

**Consequence.** A registered retail preduzetnik on stvarni prihod keeps **dvojno knjigovodstvo**, which puts her squarely inside ZoRač via čl. 2 t. 3 and čl. 4 st. 1. **Default every registered retail/hospitality shop into the full ZoRač čl. 28 profile.** The ZPDG čl. 48 flat 5-year floor is reachable only on an explicitly derived kiosk/mobile-object non-PDV profile — never as a user-selectable shortcut.

**Encodable periods.**

| Class | Period | Clock start | Article |
|---|---|---|---|
| Finansijski izveštaji, izveštaji o reviziji, Statistički izveštaj | 20 y | last day of the business year (st. 9) | ZoRač čl. 28 st. 2 |
| Godišnji izveštaj o poslovanju | 20 y | " | čl. 28 st. 3 |
| Dnevnik i glavna knjiga | **10 y** | " | čl. 28 st. 4 |
| Pomoćne knjige | 5 y | **od dana njihovog zaključivanja** | čl. 28 st. 5 |
| Isplatne liste / analitičke evidencije zarada | **trajno** | — | čl. 28 st. 6 |
| Isprave na osnovu kojih se unose podaci u poslovne knjige | 5 y | last day of the business year | čl. 28 st. 7 |
| Isprave platnog prometa | 5 y | " | čl. 28 st. 8 |
| **KEP** (by renvoi) | 5 y | from zaključivanje | PEP čl. 19 + ZoRač čl. 28 st. 5 |
| Evidencija reklamacija | ≥ 2 y | **od dana podnošenja** | ZZP 35/2026 čl. 63 st. 6 / ZZP 88/2021 čl. 55 st. 6 |
| Evidencija o zaposlenima / o zaradama | **trajno** | — | ZEOR čl. 7 st. 2, čl. 25 st. 3 |
| PDV evidencija + underlying documentation | until expiry of zastarelost | see below | ZPDV čl. 47 |
| Dokumentacija za čl. 32 **objekte i ulaganja u objekte** | ≥ 10 y | **po isteku kalendarske godine** of first use / completion | ZPDV čl. 47, 2nd limb |

> **ZPDV čl. 47** — "Obveznik je dužan da čuva evidenciju iz člana 46. ovog zakona i dokumentaciju na osnovu koje vodi ovu evidenciju **do isteka roka zastarelosti za utvrđivanje i naplatu PDV**, odnosno najmanje deset godina **po isteku kalendarske godine** od momenta prve upotrebe objekata i završetka ulaganja u objekte iz člana 32. ovog zakona."
> **ZPPPA čl. 114ž** — "Pravo na utvrđivanje, naplatu, povraćaj, poreski kredit, refakciju, refundaciju … uvek zastareva u roku od deset godina od isteka godine u kojoj je porez trebalo utvrditi ili naplatiti … **osim ako ovim zakonom nije drukčije propisano**."
> **ZPPPA čl. 114z st. 2** — "Vreme trajanja zastoja zastarelosti iz stava 1. ovog člana **ne računa se u apsolutni rok za zastarelost**."
> **ZoRač čl. 28 st. 12** (omitted by the research; a direct constraint on cloud hosting) — "Računovodstvene isprave, poslovne knjige i finansijski izveštaji čuvaju se **u poslovnim prostorijama pravnog lica, odnosno preduzetnika, odnosno kod pravnih lica ili preduzetnika kojima je povereno vođenje poslovnih knjiga**."
> **ZoRač čl. 28 st. 11 t. 4** — "…kao i **rezervna baza podataka na drugoj lokaciji**."
> **ZoRač čl. 28 st. 13** — "Ako se poslovne knjige vode na računaru, uporedo sa memorisanim podacima, pravno lice, odnosno preduzetnik mora da obezbedi i **memorisanje aplikativnog softvera** kako bi podaci bili dostupni kontroli."
> **PEP čl. 19** — "Evidencija prometa robe i dokumentacija na osnovu koje su vršena evidentiranja čuva se na način, na mestu i u roku u kojem se čuvaju pomoćne knjige u skladu sa propisima o računovodstvu."
> **ZF čl. 8 st. 5** — after transmission to PURS the obveznik "nema obavezu daljeg čuvanja podataka o izdatim fiskalnim računima" — **the fiscal device is not the archive**.
> Sources: ZPDV https://www.paragraf.rs/propisi/zakon_o_porezu_na_dodatu_vrednost.html · ZPPPA https://www.paragraf.rs/propisi/zakon_o_poreskom_postupku_i_poreskoj_administraciji.html · ZoRač ("Sl. glasnik RS" br. 73/2019 i 44/2021 - dr. zakon, **no 2024–2026 amendment**) https://www.paragraf.rs/propisi/zakon_o_racunovodstvu.html · PEP https://www.paragraf.rs/propisi/pravilnik_o_evidenciji_prometa.html · ZF https://www.paragraf.rs/propisi/zakon-o-fiskalizaciji-republike-srbije.html

**Engineering floor.** `retain_until = 31 December of (fiscal_year + 10)` — derived from **ZPPPA čl. 114ž**, not from ZPDV čl. 47 — for every shop, PDV or not (non-PDV gets 10 years from ZoRač čl. 28 st. 4). It must be **extendable and never auto-shrinking**, because čl. 114d restarts the relative period on any PU action and čl. 114z st. 2 excludes zastoj from the absolute period. For the **December** tax period the answer may be year+11 (the prijava falls due 15 January of Y+1) — the extension mechanism must cover it.

**Penalties.**

| Breach | Pravno lice | Odgovorno lice | **Preduzetnik** |
|---|---|---|---|
| Tax records not kept/retained — **ZPPPA čl. 178b** *(ZPDV čl. 60/60a have ceased to be in force)* | 100.000–2.000.000 (st. 1) | 10.000–100.000 (st. 7) | **50.000–500.000 (st. 2)** — prekršaj |
| ZoRač čl. 28 breach | **privredni prestup** 100.000–3.000.000 (čl. 57 st. 1 t. 16) | 20.000–150.000 (čl. 57 st. 2) | **prekršaj 100.000–500.000 (čl. 58)** |
| ZZPL over-retention (čl. 5 st. 1 t. 5) | 50.000–2.000.000 (čl. 95 st. 1 **t. 1**) | — | **20.000–500.000 (čl. 95 st. 4)** |
| ZAG čl. 65 | 50.000–2.000.000 (st. 1) | 5.000–150.000 (st. 2) | **no tier exists** |

**Can a preduzetnik be the subject?** ZPPPA čl. 178b — **yes** (prekršaj). ZoRač čl. 57 — **no** (privredni prestup); he answers under čl. 58 as a prekršaj. ZAG čl. 65 — **no tier**, therefore no enforceable fine; **but that is not the same as no duty** (ZAG čl. 9 st. 1 and st. 4 have no "osim fizičkih lica" carve-out, and čl. 65 st. 1 tač. 2 penalises breach of the whole of čl. 9 — see §5).

**Archive law.** Prior written archive approval before destruction is **public-sector only**:
> **ZAG čl. 16 st. 2** — "Dokumentarni materijal nastao radom i delovanjem **državnih organa i organizacija, organa teritorijalne autonomije i jedinica lokalne samouprave, ustanova, javnih preduzeća i imalaca javnih ovlašćenja**, čiji je rok čuvanja istekao uništava se po pribavljenom odobrenju u pismenoj formi nadležnog javnog arhiva."
> **ZAG čl. 9 st. 2 t. 9** — "odabira arhivsku građu i izdvaja radi uništenja bezvredan dokumentarni materijal kojem je istekao rok čuvanja, **godinu dana od dana isteka utvrđenog roka**;"
> Source: https://www.paragraf.rs/propisi/zakon-o-arhivskoj-gradji-i-arhivskoj-delatnosti.html

**No ZZPL conflict.** Statutory retention **is** the purpose (čl. 12 st. 1 t. 3), and čl. 30 st. 5 t. 2 disapplies the erasure right where processing is necessary to comply with a legal obligation. The conflict runs the other way: **after expiry**, retention becomes unlawful (čl. 5 st. 1 t. 5), and ZAG čl. 9 st. 2 t. 9 independently requires selecting expired material for destruction within one year.

**ZZP wording note.** ZZP 35/2026 čl. 63 st. 6 is **materially unchanged**, not identical, from ZZP 88/2021 čl. 55 st. 6: the duty-holder moved from "Prodavac" to "**Trgovac**", widening who must keep the register. The 2-year-from-podnošenje period is the same, so the Aug-2026 cutover does **not** affect retention.

---

### Q5 — ESIR registry verification and the per-premises L-PFR

**Encodable rule — verification duty, no recording duty.**

> **ZF čl. 6 st. 8** — "Обвезник фискализације **мора проверити пре отпочињања коришћења** електронског фискалног уређаја да ли је употреба његових елемената (процесор фискалних рачуна и електронски систем за издавање рачуна) одобрена од стране Пореске управе."
> **ZF čl. 6 st. 4** — "Обвезник фискализације, **осим обвезника фискализације који обавља промет на мало искључиво путем интернета, односно промет на мало сопствених коришћених покретних материјалних средстава**, који се определи за коришћење електронског фискалног уређаја из става 3. тачка 2) овог члана, дужан је да у сваком свом пословном простору и пословној просторији … обезбеди несметан рад и најмање један електронски фискални уређај из става 3. тачка 1) овог члана."
> Source: https://www.purs.gov.rs/upload/media/2025/2/4/382229/Zakonofiskalizaciji.pdf (ZF, "Sl. glasnik RS" 153/2020, 96/2021, 138/2022 — **no post-2022 amendment**)

- **čl. 6 st. 8 is a real, mandatory, pre-use duty on the SHOP** — the exact hook SW-15's go-live checklist step needs. The doc's citation "ZF čl. 6" is too coarse.
- **It carries no direct penalty.** čl. 6 st. 8 appears in **none** of čl. 15–18 (their predicates are exactly čl. 4/2, 5/2, 6/1, 6/4, 8/1 · čl. 7, 8/2 · čl. 9/1-3 · čl. 6/6). The sanction bites only through the *outcome*: if the unverified element turns out unapproved or revoked, the shop commits **čl. 15 st. 1 tač. 3**.
- **There is no duty anywhere to record, file, retain or produce the ESIR naziv / verzija / IB.** The only setup filing the shop ever makes is the čl. 9 PGJO premises data, and Pravilnik 31/2021+93/2021 čl. 2's field list is exhaustive — 12 fields (PIB, naziv, naziv/tip prostora, geolokacija, adresa, delatnosti, datumi, kvadratura, status) with **no ESIR or PFR field**. **Storing the identifiers is voluntary self-documentation, not a compliance record.**
- **≥1 L-PFR per premises is mandatory** for physical retail; čl. 6 st. 3 joins t.1 and t.2 with "**i/ili**", so it is not a strict either/or. The statute never licenses "V-PFR only" for a shop — it only excuses the L-PFR floor for the two carve-outs.

**Registry.** Uredba 32/2021 čl. 11 st. 1: "Poreska uprava vodi dostupan registar odobrenih elemenata elektronskog fiskalnog uređaja". Live columns, verified: R.br. · **Tip EFU** (ESIR | LPFR) · Naziv EFU · **Verzija** · IB elementa EFU · podnosilac/dobavljač · PIB i matični broj · broj i datum rešenja o odobrenju · **broj i datum rešenja o ukidanju odobrenja** · vrsta odobrenja. Revocation power: Uredba čl. 10 st. 1. **The version column is the sharp edge** — approval attaches to a specific version string, so a silently upgraded ESIR whose version is not listed is not the approved element. https://www.purs.gov.rs/sr/eFiskalizacija/registar-odobrenih-elemenata-efu.html

**Penalties (ZF — all prekršaji; ZF contains no privredni prestup).**

| Article | Pravno lice | Odgovorno lice | **Preduzetnik** | Fizičko lice |
|---|---|---|---|---|
| čl. 15 st. 1 (incl. t. 3 unapproved element; t. 4 no L-PFR) | 300.000–2.000.000 | 20.000–150.000 | **50.000–500.000 (st. 3)** | 20.000–150.000 |
| čl. 16 (bezbednosni element; 5-day transmission) | fixed 300.000 | fixed 50.000 | **fixed 150.000** | fixed 50.000 |
| čl. 17 (PGJO premises data) | fixed 200.000 | fixed 30.000 | **fixed 100.000** | fixed 30.000 |
| čl. 18 — **dobavljač** only (predicate čl. 6 st. 6) | fixed 300.000 | fixed 50.000 | **fixed 150.000** | — |

**Can a preduzetnik be the subject? YES** to all of the above, as prekršaji. **čl. 12 is a different instrument**: zabrana obavljanja delatnosti of **15 dana / 90 dana / jedne godine**, per premises, escalating on 2nd/3rd findings **within 24 months**, triggered **only** by failure to record each individual retail sale via the EFU (čl. 12 st. 1) and ordered by rešenje (čl. 14 st. 1). čl. 13 st. 4 adds a separate *privremena zabrana* for not acting on a čl. 13 st. 1 rešenje. **Do not render čl. 12 next to the registry-check step** — a registry-check failure cannot close the shop.

**Criminal exposure of holding the identifiers: none from storage as such.** Both ZPPPA čl. 175a and čl. 175b criminalise the **identical** act list including "**drži**" — the research's contrast (that only 175b reaches the shop) is wrong. čl. 175a's elements are **cumulative** (not in the register **AND** serving to avoid recording retail turnover); a passive identifier field satisfies neither limb of the second. čl. 175b is a pure function test. Both: 1–5 years' imprisonment, mera bezbednosti 1–5 years directed expressly at the odgovorno lice **and the preduzetnik**, plus confiscation. **The real residual risk is presentational**: the moment those identifiers appear on VantumPOS *output* next to sale lines, the document starts to read as a receipt surrogate.

**Positive compliance argument the doc misses.** ZoRač čl. 8 st. 4: "…dužan je da koristi računovodstveni softver koji omogućava funkcionisanje sistema internih računovodstvenih kontrola i **onemogućava brisanje proknjiženih poslovnih promena**." For a ZoRač-bound shop this turns VantumPOS's append-only ledger from a 175b *defence* into an affirmative *requirement it satisfies* — the strongest available answer to "why can't I delete this sale?"

**SEF / e-otpremnice — the carve-out is NOT absolute.**

> **ZEF čl. 3 st. 2 tač. 1** — "promet na malo i primljeni avans za promet na malo u skladu sa zakonom kojim se uređuje fiskalizacija, **OSIM ZA: (1)** promet na malo koji se vrši **imaocu korporacijske kartice** … i primljeni avans za taj promet, **(2)** promet na malo koji se vrši **subjektu javnog sektora**, ako je subjekt javnog sektora podneo zahtev za izdavanje elektronske fakture **u roku od sedam dana** od dana izvršenog prometa na malo."
> Source: https://www.paragraf.rs/propisi/zakon-o-elektronskom-fakturisanju.html

"Subjekt privatnog sektora" = **obveznik PDV** (ZEF čl. 2 tač. 3), so a non-PDV shop is outside ZEF unless it registers voluntarily. **ZEO** uses the same PDV gate and the same retail carve-out (čl. 3 st. 2); private-to-private send **and** receive from **01.10.2027** (public sector, excise flows and private→public already from 01.01.2026). **KEP does not branch on anything** — PEP **čl. 2 st. 3** binds every retail trgovac regardless of form, PDV status or tax regime.

---

### Q6 — Privredni prestup vs prekršaj: the master branch

**Encodable rule.** **A preduzetnik cannot commit a privredni prestup.**

> **ZPP čl. 6 st. 1** — "Za privredni prestup može biti odgovorno pravno lice i odgovorno lice u pravnom licu."
> **ZPP čl. 8 st. 1** — "Odgovornim licem … smatra se lice kome je poveren određen krug poslova u oblasti privrednog ili finansijskog poslovanja **u pravnom licu** …"
> **ZPD čl. 83 st. 1** — "Preduzetnik je poslovno sposobno **fizičko lice** koje obavlja delatnost u cilju ostvarivanja prihoda …"
> **ZoP čl. 29** — "Preduzetnik odgovara za prekršaj koji učini pri vršenju svoje delatnosti."
> Sources: https://www.paragraf.rs/propisi/zakon_o_privrednim_prestupima.html ("Sl. list SFRJ" 4/77 … "Sl. glasnik RS" 101/2005, unamended) · https://www.paragraf.rs/propisi/zakon_o_privrednim_drustvima.html · https://www.paragraf.rs/propisi/zakon_o_prekrsajima.html

A full-text search of the consolidated ZPP returns **zero** occurrences of "preduzetnik". Precision the research overstated: čl. 6 st. 1 is not *literally* exhaustive — čl. 6 st. 3 extends to odgovorna lica in državni organi/mesne zajednice and čl. 6a to strana pravna lica — but neither reaches a preduzetnik, so the conclusion is unaffected. **Consequence beyond the fine: different forum (privredni sud vs prekršajni sud) and different clock (ZPP čl. 37 st. 1 three years vs ZoP čl. 84 st. 1 one year, absolute two under st. 7).**

**The ZoP čl. 39 sanity grid** — an encodable lint, not a validity test:

> **ZoP čl. 39 st. 1** — "Zakonom ili uredbom novčana kazna može se propisati u rasponu: 1) od 5.000 do 150.000 dinara za fizičko lice ili odgovorno lice; 2) od 50.000 do 2.000.000 dinara za pravno lice; **3) od 10.000 do 500.000 dinara za preduzetnika.**"
> **ZoP čl. 39 st. 2** — fixed amounts: fizičko/odgovorno lice 1.000–50.000; **preduzetnik 5.000–150.000**; pravno lice 10.000–300.000.
> **ZoP čl. 39 st. 4** — "…za prekršaje iz oblasti javnih prihoda … **prometa roba i usluga** … zakonom se mogu propisati kazne u srazmeri sa visinom pričinjene štete … ali **ne više od dvadesetostrukog iznosa tih vrednosti** s tim da **ne prelazi petostruki iznos najvećih novčanih kazni** koje se mogu izreći po odredbi stava 1. ovog člana."

Any preduzetnik figure above **500.000** (range) or **150.000** (fixed) is presumptively a mis-copied pravno-lice tier. **Caveat:** čl. 39 constrains what may be *prescribed*, so a pre-2013 statute exceeding it is not automatically invalid — treat a violation as a **warning to investigate**, never as proof of error, and never as a build failure.

**Zaštitna mera — state only what is verifiable.** ZoP čl. 51 st. 2 requires prescription ("Zaštitna mera može se propisati zakonom i uredbom") and čl. 52 st. 2's closed list of measures imposable absent prescription **excludes** zabrana vršenja određenih delatnosti. **But ZoP čl. 55 st. 2 exists and cuts the other way:**

> **ZoP čl. 55 st. 2** — "Ako propisom kojim se određuje prekršaj nisu posebno predviđeni uslovi za izricanje zaštitne mere iz stava 1. ovog člana, mera se može izreći ako učinilac prekršaja delatnost **zloupotrebi** za izvršenje prekršaja ili ako se opravdano može očekivati da bi dalje vršenje te delatnosti bilo **opasno** po život ili zdravlje ljudi ili druge zakonom zaštićene interese."
> **ZoP čl. 55 st. 3** — general duration six months to **three years**.

**Therefore the product must say "these articles prescribe no zaštitna mera" (verifiable) and never "a trading ban is not available" (contested).** Confirmed availability exists only where the special law prescribes it — **ZoT čl. 68 st. 6** (6 months–2 years, preduzetnik and fizičko lice).

**Procedure branch.** Fixed-amount offences → **prekršajni nalog** (ZoP čl. 168 st. 1), and st. 2 **bars** a zahtev za pokretanje prekršajnog postupka; st. 3 issues a separate nalog per offender. Range offences → court. *(Per standing project rule, do not put "pay half within 8 days" guidance in the product.)*

---

## 3. Encodable software requirements

### SW-11(a) — AML cash-cap warning

1. **`[LEGAL]` → §2 Q1.** Evaluate the **cash line of the payment split**, per sale and per aggregate. Never the grand total. A cash+card split is not a breach of st. 1 on the face of the statute.
2. **`[LEGAL]` → §2 Q1.** Comparison operator is **`>=`**. Warn/block at exactly 10.000 EUR equivalent. A `>` comparison is a compliance bug.
3. **`[LEGAL-INFERRED]` → §2 Q1.** Convert using the **NBS zvanični srednji kurs for the transaction date**; persist the rate and its date on every evaluated sale so the decision is reproducible at inspection. **Replace SW-11(a)'s "configurable rate"** — a freely configurable rate invites the wrong one. Ship a manual-entry fallback plus a staleness warning when the cached rate is not today's. *(Basis is čl. 8 st. 1 tač. 2's definitional hook, not čl. 46 itself — see §5 Q-4.)*
4. **`[LEGAL]` → §2 Q1.** Offer the lawful alternative, not only a warning: the statute **commands** "već se navedeni novčani iznos mora uplatiti na račun otvoren kod banke", and st. 3 confirms a buyer's deposit into the shop's account is not "receiving cash". Ship **"Uplata na tekući račun prodavnice (izvod iz banke)"** as a payment method. Without it the warning is a dead end and the cashier takes the cash anyway.
5. **`[LEGAL]` duty, `[PRUDENTIAL]` mechanism → §2 Q1.** The one-year aggregation is in the statute. Because there is no customers table, implement: (a) the per-sale check now; (b) an **optional** buyer tag on cash sales above a configurable soft threshold; (c) a **rolling 365-day** (not calendar-year) running total surfaced at tender time for tagged buyers. **State in writing to the shop owner that untagged sales cannot be aggregated by the software and that the legal duty binds regardless.** Apply the one-year window to both limbs (protective default).
6. **`[ACCURACY]` → §2 Q1, Q6.** Penalty copy resolved from `pravna_forma`: preduzetnik → *"prekršaj, novčana kazna od 100.000 do 300.000 RSD (čl. 120 st. 2), a u srazmeri s vrednošću robe i više (čl. 120 st. 8)"*; pravno lice → *"privredni prestup, 100.000–2.000.000 (čl. 118 st. 1 tač. 43) + odgovorno lice 10.000–150.000 (čl. 118 st. 2)"*. **Never emit "privredni prestup" under the preduzetnik regime. Never present a base maximum as a ceiling.**
7. **`[LEGAL]` → §2 Q1.** Name the correct inspector: **tržišna inspekcija** (čl. 110 st. 6) — same body as KEP, cenovnik, sniženje. One readiness screen can cover all four.
8. **`[PRUDENTIAL]`.** Soft block with mandatory reason capture and an immutable audit-log entry, rather than a hard block or a silently bypassable one.
9. **`[LEGAL]` → §2 Q1.** Date-gate the rule version if any historical/retrospective report is built: 10.000 cap and linked-cash-transaction aggregation from 113/2017; contracts/one-year limb, bank-transfer command and st. 3 carve-out **only from 06.12.2024**.

### SW-11(b) — Deposit aging

10. **`[LEGAL]` → §2 Q2.** Bucket **all cash received on any basis** ("po bilo kom osnovu") — supplier refunds, cash rent, proceeds of an asset sale, loan repayments — not only customer takings. Restricting to sale takings systematically under-reports the obligation and hands the owner a false "clean" state.
11. **`[LEGAL]` → §2 Q2.** Clock runs from **receipt of the cash**, not from shift close, dnevni izveštaj or the Z-report. Nothing in Zakon 68/2015 or Pravilnik 77/2011 mentions a dnevni izveštaj. Aggregate per trading date as an implementation convention and **label it as such in the UI**.
12. **`[LEGAL]` → §2 Q2.** Deadline = receipt date + **7 radnih dana**; display the deadline **date**, not just a countdown. Partial deposits are lawful — model each polog as drawing down the **oldest open bucket first**, never all-or-nothing.
13. **`[PRUDENTIAL]` (the calendar), `[LEGAL]` (the count) → §2 Q2, §5 Q-5.** Ship a Serbian non-working-day table; make "does Saturday count as a radni dan" configurable and **default it to true**, because counting Saturdays yields the earlier, conservative deadline. "Radni dan" is statutorily undefined — say so in the tooltip.
14. **`[LEGAL]` with caveat → §2 Q2.** Distinct movement type for cash withdrawn from the shop's own account, flagged NOT-SUBJECT — but only where the withdrawal was made **per Pravilnik čl. 2 st. 2 or st. 3**. Do not exclude all bank withdrawals. Label the exclusion as **bylaw-level relief**; the statute and the penalty contain none.
15. **`[PRUDENTIAL]`.** Informational, clearly non-mandatory: the 150.000 RSD/day undocumented-withdrawal lane (Pravilnik čl. 2 st. 3) and the 3-day advance announcement for withdrawals over 1.500.000 RSD (čl. 3 st. 1).
16. **`[PRUDENTIAL]`.** Advisory banner + report only. **No hard block** — this is fiscal hygiene supervised by Poreska uprava, not a condition of a valid sale. Do not gate sales, day-close or fiscalization on it.
17. **`[PRUDENTIAL]`.** Evidence trail per polog (date, amount, buckets discharged, bank/uplatnica reference, user) and a printable/CSV **"Izveštaj o nedeponovanom gotovom novcu"** for the knjigovođa. This is the concrete deliverable; a Poreska uprava check is documentary.
18. **`[ACCURACY]` → §2 Q2.** Any till-ceiling alert must be labelled **"interni blagajnički maksimum — nije zakonska obaveza"**, user-entered, **no default**. Presenting a ceiling as a legal requirement would be a factual misstatement.
19. **`[ACCURACY]` → §2 Q2.** Purge any "istog dana / narednog radnog dana" logic or copy. That deadline died on 17.05.2011. *(The phrase legitimately appears in čl. 3 st. 3 only for the bank's payout duty above 600.000 RSD.)*
20. **`[ACCURACY]`.** In-app legal text cites the full title + "Sl. glasnik RS, br. 68/2015, čl. 3 st. 1; kazne čl. 7 st. 1 tač. 2) i st. 3". Never "ZOP".

### SW-11(c) — Product / deklaracija data

21. **`[ACCURACY]` → §1 row 11.** Correct the doc's code gap first: `barcode TEXT UNIQUE` exists at `migrations.rs:68` (plus `external_source_barcode` at :249). Do not add a duplicate GTIN column.
22. **`[PRUDENTIAL]` for walk-in retail, `[LEGAL]` for distance selling → §2 Q3.** Add nullable `manufacturer_name`, `importer_name`, `country_of_origin`. Accept the literal value **"EU"** for origin (st. 7). For walk-in retail these mirror a physical label and are a compliance aid. **For any product exposed to a distance channel they are legally required to be displayed and continuously available pre-purchase (st. 5) — make them mandatory there.**
23. **`[PRUDENTIAL]` → §2 Q3.** Separate nullable `official_goods_code` column for the future jedinstveni šifarnik — **unused**. Format, length and check digits are unknown until the Minister's pravilnik lands; forcing it into `barcode` will require a painful migration. **Build no import/sync logic against a register that does not exist.**
24. **`[PRUDENTIAL]` → §2 Q3.** `barcode_kind` enum `{gtin, internal, none}`. Validate GTIN-8/12/13/14 length and check digit only for `gtin`; an in-house printed code must never be reported as a GTIN.
25. **`[PRUDENTIAL]`, with real mitigation value → §2 Q3.** Put the compliance feature at **goods receipt**, not checkout, because the retailer's liability is a *selling* offence: per-line "deklaracija proverena" checkbox + the identity fields on the kalkulacija screen (SW-9b), **warn — never hard-block**. A timestamped record is concrete mitigation under ZoT čl. 69a tač. 4. *(It is a mitigating circumstance in sentencing, not a defence — do not promise otherwise.)*
26. **`[ACCURACY]` → §2 Q3.** Attach **no fine figure** to a bare missing GTIN. Show the two-state model instead: *deklaracija absent* (severe, 50.000–500.000 + possible closure) vs *deklaracija defective* (fixed 40.000, ticketable). Collapsing them either terrifies or falsely reassures.

### SW-15 — Onboarding profile

27. **`[ACCURACY]` → §2 Q6.** Store `pravna_forma` as an explicit enum `{preduzetnik, pravno_lice}`, **default UNSET, never inferred**. Every legal string carrying a number resolves its tier from this field. This one field is the root cause of all four penalty-tier errors in §1.
28. **`[ACCURACY]` → §2 Q6.** CI guard: the literal "privredni prestup" must not appear in any user-facing string reachable under the preduzetnik regime.
29. **`[PRUDENTIAL]` → §2 Q6.** Lint (warning, not build failure) any preduzetnik constant > 500.000 range / > 150.000 fixed, unless annotated with the special law's invocation of ZoP čl. 39 st. 4 — today only AML čl. 120 st. 8 among the laws verified here.
30. **`[LEGAL]` → §2 Q4. Do NOT ship a free "paušal / prosto knjigovodstvo" choice for a retail shop.** Derive **dvojno knjigovodstvo** by default for every registered retail/hospitality shop, and therefore the full ZoRač čl. 28 profile. Reach the ZPDG čl. 48 5-year branch only when the shop is *both* non-PDV *and* trades from a kiosk/prikolica/pokretni objekat.
31. **`[LEGAL]` → §2 Q3. Ask whether the shop sells at distance** (webshop, Instagram, Viber ordering). This single fact flips ZoT čl. 34 st. 5 from irrelevant to a hard requirement and reverses requirement 22. **Establish it before encoding anything else in SW-11(c).**
32. **`[LEGAL]` → §2 Q5.** Per-premises assertion: *"Da li u ovom poslovnom prostoru radi najmanje jedan lokalni PFR (uređaj koji izdaje račun i bez interneta)?"* plus the two čl. 6 st. 4 carve-out questions (internet-only retail; own used movable assets). If no and neither carve-out applies, surface čl. 6 st. 4 and the čl. 15 st. 1 tač. 4 penalty **at the shop's own tier**. Never imply a V-PFR-only setup is acceptable for physical retail.
33. **`[LEGAL]` (the check) + `[PRUDENTIAL]` (the storage) → §2 Q5.** Label the ESIR panel with the real hook — **"ZF čl. 6 st. 8: proveriti pre otpočinjanja korišćenja"** — and add plain-language text that **no law requires the shop to store or produce these identifiers**. Call the stored row **"interna beleška o proveri"**, never "evidencija" or "dokaz". Store **four** fields: naziv, **verzija**, IB, tip (`ESIR` | `LPFR`), modelled as **separate registry entries** per element per premises.
34. **`[PRUDENTIAL]` → §2 Q5.** Ship the lookup recipe and the registry deep link: filter by Tip EFU / Naziv EFU / dobavljač, match **naziv + verzija + IB** against what the till reports, confirm the **"Broj i datum rešenja o ukidanju odobrenja"** cell is empty. Add a re-confirmation prompt (annual, or on any reported ESIR update) — a one-time go-live check decays because approval attaches to a version string.
35. **`[LEGAL]` → §2 Q4.** Make the PDV flag a **retention profile**, not a display toggle, and **do not gate the 10-year floor on it**. Keep the engineering floor unconditional; let the flag change only the warning text SW-3 shows. Switching a shop PDV → non-PDV must never retroactively shorten `retain_until` on records already written.
36. **`[LEGAL]` → §2 Q4.** Per-record `retention_class` + materialised `retain_until`, an explicit `legal_hold` flag, and an admin-settable **upward-only** extension. Never compute purge eligibility from a constant. Payroll and employee records are a `never_purge` class, structurally excluded from every purge, reset, restore and backup-prune path.
37. **`[LEGAL]` → §2 Q4.** Separate čl. 32 asset register (objekti, ulaganja u objekte) with its own clock: **31 December of the year of first use / completion, plus 10 years**. Do **not** present a 5-year oprema retention clock as a čl. 47 duty — čl. 47 names only objekti and ulaganja.
38. **`[LEGAL]` → §2 Q4.** KEP retention hangs off `closed_at` (PEP čl. 17 zaključivanje → ZoRač čl. 28 st. 5), which is later than the čl. 28 st. 9 alternative and therefore the safe reading of the statute's internal tension. SW-9c already writes it.
39. **`[ACCURACY]` → §2 Q4, §1 rows 5 & 12.** Archive-law copy: **remove the "d.o.o. must get written archive approval before destruction" claim**. For a **preduzetnik**, suppress the penalty figure and the čl. 9 st. 2 catalogue (arhivska knjiga, 30 April prepis, čl. 14 acts) but **keep a neutral čl. 9 st. 1 custody note** — do not tell him archive law does not apply. For a **pravno lice**, state the real duties: lista kategorija with archive saglasnost, arhivska knjiga, 30 April prepis.
40. **`[ACCURACY]` → §2 Q5.** SEF / e-otpremnica copy must carry the **two "osim za" exceptions and the 7-day request window**, and must never say "maloprodaja je izuzeta od SEF-a" unqualified. Gate on the PDV flag. **KEP is gated on nothing** — PEP čl. 2 st. 3 binds every retail trgovac unconditionally.
41. **`[PRUDENTIAL]`, enforced as a hard rule → §2 Q5.** The stored ESIR naziv/verzija/IB must never leave settings/onboarding: not on any printed or exported document that itemizes a sale, not near the "OVO NIJE FISKALNI RAČUN" banner, not in any product name, splash screen or marketing string. Storage is neutral; **presentation next to sale lines** is what feeds the recharacterization argument.
42. **`[LEGAL]` → §2 Q4.** SW-13 (ZZPL purge) and SW-3 (retention guard) must share **one** retention table. Independent notions of "expired" guarantee that one deletes what the other is obliged to keep.
43. **`[LEGAL]` → §2 Q4.** Electronic archive posture: ZoRač čl. 28 st. 11 t. 4 requires protection against modification/deletion **plus a rezervna baza podataka na drugoj lokaciji**; **st. 12** requires the isprave/knjige/izveštaji to sit in the obveznik's own business premises or with the party entrusted with the bookkeeping. **A vendor-hosted cloud archive is not automatically compliant merely because it has an off-site backup.** čl. 28 st. 13–14 additionally require preserving the application software, or storing the data in clear text with a field description and defined delimiter — ship a versioned, documented plain-text export with a field dictionary as a first-class archival artefact.

---

## 4. What must NOT be built

1. **No APML / Uprava za sprečavanje pranja novca reporting or export.** The shop is not an obveznik under AML čl. 4, so čl. 47 st. 1's 15.000 EUR reporting duty does not touch it. Do not build the path; do not let marketing imply one exists.
2. **No "split the payment across receipts" helper, and no marketing of split-tender as a compliance workaround.** Splitting to stay under the cap is exactly what "međusobno povezanih gotovinskih transakcija" targets; a convenience feature suggesting it is evidence of intent. (No written authority confirms a deliberate cash+card split on one large sale is safe from a circumvention argument — see §5.)
3. **No statutory "blagajnički maksimum" feature.** No propis sets one. Any till ceiling ships labelled as an internal policy with no default.
4. **No "same day / next working day" pazar deadline** anywhere in code or copy.
5. **Never tell a preduzetnik he faces a "privredni prestup", or show him a 2.000.000 / 300.000–2.000.000 / 500.000–2.000.000 figure** as his exposure.
6. **Never present a base fine range as a ceiling** where the special law invokes ZoP čl. 39 st. 4 or ZPP čl. 18 st. 2.
7. **Never state that a trading ban is unavailable** for AML, the deposit duty, or ZoT čl. 67. Say only that those articles prescribe none. ZoP čl. 55 st. 2 is unresolved.
8. **Never tell a preduzetnik that archive law does not apply to him.** ZAG čl. 65 has no tier for him; ZAG čl. 9 st. 1 has no carve-out.
9. **Never offer a paušal / prosto-knjigovodstvo retention profile to a registered retail shop.** ZPDG čl. 40 st. 2 t. 2 and t. 5 make it legally impossible outside the kiosk case, and the answer would cut retention from 10/20/trajno to 5 years.
10. **Never say "maloprodaja je izuzeta od SEF-a"** without the korporacijska-kartica and public-sector-buyer exceptions and the 7-day window.
11. **No fine figure for a bare missing GTIN.** Unresolved which tier applies; the gap between them is 40.000 vs 500.000 plus shop closure.
12. **No hard block at checkout on missing GTIN or declaration data for walk-in retail.** The marking duty is the producer's/importer's, the data is not required to be in any database for over-the-counter sale, and a block would strand legitimate stock. *(Distance selling is the exception — there the data is legally required.)*
13. **No ESIR/PFR identifier on any output that itemizes a sale**, in any receipt-shaped layout, or in any product/marketing string.
14. **Do not scrape or cache the PURS registry as authoritative.** It is a live table with no published API; a stale cached "approved" state is worse than no check.
15. **No import/sync logic against the jedinstveni šifarnik robe.** It does not exist, and the amending act sets no deadline for the pravilnik.
16. **No silent purge cron.** Build the purge as select → operator confirms → delete + log; ZAG čl. 9 st. 2 t. 9 frames the duty as a reviewed annual sweep.
17. **Do not treat the fiscal device as the archive** (ZF čl. 8 st. 5) or let the ESIR receipt number be rendered receipt-like — keep it a free-text reference on the sale record.
18. **Do not cite** ZPDV čl. 60/60a (repealed), ZPPPA čl. 175b as a retention rule, the mfin.gov.rs AML .docx (now blocked), or `apml.gov.rs/REPOSITORY/458_zospnft.pdf` (the superseded 2009 law).
19. **Do not put "pay half within 8 days" prekršaj-procedure guidance in the product** (standing project rule).
20. **Keep textile fibre/care/size fields out of the compliance surface.** ZoT čl. 34 st. 12 pulls in the Pravilnik o označavanju tekstilnih proizvoda, but that data lives on the physical label and is the supplier's responsibility. Model it only if the shop wants it for its own catalogue.

---

## 5. Residual open questions

| # | Question to put, and to whom | Why unresolved | Risk of proceeding |
|---|---|---|---|
| Q-1 | **To a lawyer / tržišna inspekcija:** *"Da li se rok 'u periodu od godinu dana' iz čl. 46 st. 1 ZSPNFT odnosi samo na limb 'jednom ili više ugovora', ili i na 'više međusobno povezanih gotovinskih transakcija'?"* | Comma placement is genuinely ambiguous; no APML/MF mišljenje retrievable | Low. Protective default (apply to both) over-warns at worst |
| Q-2 | **To a lawyer:** *"Da li je limb 'jednom ili više ugovora u periodu od godinu dana' uopšte kažnjiv za trgovca, kada opis prekršaja u čl. 118 st. 1 tač. 43 pominje samo jednokratno ili više povezanih gotovinskih transakcija?"* | The prohibition and the penal description do not match; strict construction in prekršaj law cuts against liability | Low for the build; a real argument the shop's lawyer would have. Do not build differently |
| Q-3 | **To Ministarstvo trgovine / tržišna inspekcija, and to a DPO:** *"Da li se od trgovca koji nije obveznik po čl. 4 očekuje da evidentira gotovinska plaćanja po kupcu tokom godinu dana, i koji je pravni osnov po ZZPL za identifikaciju kupca u tu svrhu?"* | No APML or ministry written guidance found. This is the only genuinely open half of the doc's §6 item 9 | **Material.** Building a customer-identity store without a lawful basis creates its own ZZPL exposure (čl. 95 st. 1 t. 1: preduzetnik 20.000–500.000). Ship the tag as optional and document the limitation |
| Q-4 | **To MF / APML:** *"Po kom kursu i na koji dan se 10.000 evra iz čl. 46 st. 1 preračunava u dinarsku protivvrednost?"* | čl. 46 names no rate and no date. The srednji-kurs-on-transaction-date answer is imported from čl. 8 st. 1 tač. 2's "u daljem tekstu" hook; čl. 4 st. 1 tač. 19 uses a divergent formula ("zvanični kurs … na dan plaćanja") | Low-medium. Only bites at the margin, but the margin is exactly where the block fires. Persist the rate used |
| Q-5 | **To MF / Poreska uprava:** *"Kako se računa rok od sedam radnih dana iz čl. 3 st. 1 Zakona 68/2015 — da li se dan prijema gotovine računa i da li subota predstavlja radni dan?"* | Neither the Zakon nor the Pravilnik defines "radni dan"; the Zakon o platnim uslugama uses the phrase once, undefined | Low. Conservative default (Saturday counts → earlier deadline) errs safe; a wrong assumption the other way produces late alarms |
| Q-6 | **To the shop's knjigovođa + a lawyer:** *"Da li plaćanje troška direktno iz dnevnog pazara umanjuje obavezu uplate iz čl. 3 st. 1, i da li se izuzeće iz čl. 5 st. 2 Pravilnika 77/2011 održava naspram zakonske formulacije 'po bilo kom osnovu'?"* | Practice tolerates till-funded expenses; the text does not obviously authorise them. The float carve-out lives only in a bylaw preserved "ukoliko nije u suprotnosti sa ovim zakonom", while the penalty is keyed solely to čl. 3 st. 1 | Medium. Wrong answer either over-reports (false alarms) or under-reports (false "clean" state) |
| Q-7 | **To a lawyer / tržišna inspekcija:** *"Da li nedostatak mašinski čitljive identifikacione oznake iz čl. 34 st. 8 predstavlja 'neurednu ili nepropisnu deklaraciju' (čl. 67 st. 1 tač. 6) ili 'robu bez deklaracije' (čl. 68 st. 1 tač. 9)?"* | Neither tačka names st. 8; both cross-refer generically to "(član 34)". No prekršajni-sud practice located | **High.** 40.000 fixed vs 50.000–500.000 plus a 6-month-to-2-year trading ban |
| Q-8 | **Fact question to the shop, not to a lawyer:** *does this shop sell at distance in any form?* | Never established by the research | **Fatal if unasked.** ZoT čl. 34 st. 5 turns the declaration fields from an optional aid into a stored statutory record, and reverses the "warn, never block" design |
| Q-9 | **To a lawyer / the Novi Pazar nadležni arhiv:** *"Da li je preduzetnik 'fizičko lice' u smislu izuzetka iz čl. 9 st. 2 ZAG, i da li čl. 65 st. 1 tač. 6 samostalno dosežе privatno pravno lice kada čl. 16 st. 2 po svom tekstu pokriva samo javni sektor?"* | ZAG čl. 2 t. 3 lists "preduzetnika" separately from "fizičkih lica"; čl. 9 st. 1 and st. 4 carry no carve-out; **neither research pass quoted čl. 65 st. 1 tač. 6 verbatim**, so the doc's assertion that it "fines any pravno lice" is unverified | Medium. Drives whether SW-3's reset warning is a real duty or a false one, for both shop profiles |
| Q-10 | **To a lawyer:** *"Da li se zaštitna mera zabrane vršenja delatnosti može izreći po čl. 55 st. 2 Zakona o prekršajima i kada poseban zakon (npr. ZSPNFT čl. 120, Zakon 68/2015 čl. 7, ZoT čl. 67) ne propisuje nikakvu zaštitnu meru?"* | čl. 51 st. 2 + čl. 52 st. 2 point one way; čl. 55 st. 2 points the other. No practice located | Medium. The product must not promise immunity from closure |
| Q-11 | **To the shop's knjigovođa:** *"Da li inspekcija prihvata da rok čuvanja KEP-a teče od dana zaključivanja knjige, a ne od poslednjeg dana poslovne godine?"* | ZoRač čl. 28 st. 5 and st. 9 give different starts; st. 5 is later, so it is the conservative choice | Low-medium. Wrong answer deletes early |
| Q-12 | **To a lawyer:** *"Koji propis kažnjava propuštanje ČUVANJA KEP-a (kao razliku od nevođenja) — ZoT čl. 67 st. 1 tač. 2, ZoT čl. 68 st. 1 tač. 7, ili ZPPPA čl. 178b st. 2?"* | PEP čl. 19 has no penalty of its own; the mapping is inference | Medium. Print no specific fine for KEP over-deletion in customer-facing text until answered |
| Q-13 | **To PURS (budiefiskalizovan@gov.rs):** *"Da li svaka promena verzije odobrenog ESIR-a zahteva novo odobrenje, ili manje verzije ostaju pokrivene postojećim rešenjem?"* | The registry publishes a Verzija column, implying per-version approval, but neither the Uredba nor the Tehničko uputstvo was read on this point | Low. Determines only how aggressive the SW-15 re-check prompt should be |
| Q-14 | **Monitoring, not a question:** the 27.04.2026 **Nacrt zakona o računovodstvu**, and the missing **jedinstveni šifarnik** pravilnik | ZoRač is confirmed unamended (73/2019, 44/2021) as of 31.07.2026; the šifarnik pravilnik could not be confirmed absent — **mtt.gov.rs was unreachable**, so this is *no evidence of publication*, not confirmed absence | Document rot. Re-check before any release hard-coding ZoRač čl. 2 t. 3, čl. 8 st. 4 or čl. 28 |
| Q-15 | **To the shop's accountant + a lawyer:** retention classification of POS shift / cash-movement records (labour record → trajno? pomoćna knjiga → 5 y? operational → ZZPL purge?) | Statutes do not classify POS-generated records. Carried over unresolved from the doc's §6 item 7 | Medium. Exactly the class where SW-13's purge could delete what an inspector demands |
| Q-16 | **To a lawyer:** does ZoRač čl. 28 **st. 12** (books kept in the obveznik's own premises or with the bookkeeper) permit a vendor-hosted archive? | Not addressed by either research pass | Medium if hosted backups are ever shipped; nil today (local-first) |
| Q-17 | **To a lawyer:** for the **December** PDV period, is the ZPPPA čl. 114ž absolute clock anchored to Y or Y+1, given the prijava falls due 15 January? | Not settled from text | Low, given requirement 36's upward-only extension |

**Source-hygiene note for the next verification pass.** All quotations above were read from consolidated commercial texts (paragraf.rs, neobilten.com, propisi.net) plus primary gazette/bill PDFs on parlament.gov.rs and purs.gov.rs. The state portal `pravno-informacioni-sistem.rs` renders via JavaScript and returned only page shells; `mfin.gov.rs`'s AML .docx now returns an error page; `mtt.gov.rs` was unreachable. **No quotation here has been eyeball-checked against a Službeni glasnik PDF** except ZF (purs.gov.rs) and the ZZP 35/2026 and 94/2024 bill texts (parlament.gov.rs). Do one manual pass over the four **CRITICAL**-severity figures before any of them is rendered to a shop owner.