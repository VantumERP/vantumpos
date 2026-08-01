# SW-14 — Verified Rule Set (Evidencija radnog vremena)

Scope: encodable rules for the **per-employee working-time record (SW-14)** — the dnevna evidencija o prekovremenom radu, the statutory hour classification, absence categories, the overtime caps the record must make checkable, retention, signature, and the ZZPL treatment of sick-leave data. Companion to [SW11-SW15-VERIFIED-RULES.md](SW11-SW15-VERIFIED-RULES.md), [KEP-VERIFIED-RULES.md](KEP-VERIFIED-RULES.md), [ZOT-36-37-VERIFIED-RULES.md](ZOT-36-37-VERIFIED-RULES.md) and [ZZP-REKLAMACIJE-VERIFIED-RULES.md](ZZP-REKLAMACIJE-VERIFIED-RULES.md).

**State of law: 1 August 2026.** Supersedes `docs/SERBIAN-LAW-COMPLIANCE.md` §2 rows **14** and **15**, §3 **SW-14**, **§6 item 11**, and `docs/COMPETITIVE-GAP-ANALYSIS.md` line 85. Also corrects two files in `docs/compliance/` (§1 rows 15–17).

**Method and epistemic status.** Five research findings were each attacked by an independent refutation working from primary text. Four refutations were **fatal**, one **high**. Where a refutation was material or fatal it wins, unless the finding carried a verbatim primary quote the refutation left unaddressed. Anything neither pass could settle from primary text is marked **UNRESOLVED** and carries no number. Three of the five findings put a wrong figure, a wrong stav or a wrong statutory threshold into a *product instruction* — this memo strips those out.

**Reading key for §4:** `[LEGAL]` = statutory duty · `[LEGAL-INFERRED]` = follows from statutory text by construction, not stated in terms · `[ACCURACY]` = exists so the product does not misstate the law to a shop owner · `[PRUDENTIAL]` = good practice, **not** a legal duty and must never be presented as one.

**Pilot subject assumed throughout:** a registered retail **preduzetnik** (boutique, Novi Pazar, 2–3 employees). Where a d.o.o. profile changes the answer, both are given.

---

> ### The two facts that reshape the feature
>
> **1. ZEOR IS STILL IN FORCE.** Zakon o evidencijama u oblasti rada, "Sl. list SRJ", br. 46/96 i "Sl. glasnik RS", br. 101/2005 - dr. zakon i 36/2009 - dr. zakon, is live law on 01.08.2026 — never repealed, never replaced. **But it is a torso:** čl. 8–22 and čl. 26–40 ceased to have effect on **23.05.2009** by čl. 110 Zakona o zapošljavanju i osiguranju za slučaj nezaposlenosti ("Sl. glasnik RS" 36/2009). What survives for an employer is the **evidencija o zaposlenim licima (čl. 5–7)**, the **evidencija o zaradama zaposlenih lica (čl. 23–25)**, the **evidencija o korisnicima prava invalidskog osiguranja (čl. 41–43)**, plus čl. 3–4 and čl. 44–52. Its fine amounts are **not** stale SRJ leftovers — they were reset in dinars by čl. 81 Zakona o izmenama zakona kojima su određene novčane kazne… ("Sl. glasnik RS" 101/2005). A preduzetnik with employees is bound, as a *"fizičko lice koje ima zaposlene"* (čl. 3 st. 1). Verified independently on four sources including the University of Belgrade Faculty of Law text.
>
> **2. NO OBRAZAC EXISTS FOR THE RECORD SW-14 BUILDS — but "layout is free" must NOT be carried across to the ZEOR wage record.** See §2, which is the section that decides the module's shape.

---

## 1. Headline corrections

Row 14's *numbers* survived verification intact — it is the only row in this area that the 31.07.2026 penalty-tier sweep got right. **Row 15 did not, and it repeats verbatim the exact defect that sweep was created to eliminate.** Seventeen defects follow, three capable of putting a wrong number or a wrong statutory threshold in front of a shop owner.

| # | Doc location | Wrong claim | Correct rule | Article | Severity |
|---|---|---|---|---|---|
| 1 | **Row 15, line 70** | Wage records "…kept PERMANENTLY; **fines 500k–1M RSD**", subject column **"Shop"** | 500.000–1.000.000 is expressly *"preduzeće ili drugo pravno lice"*. The pilot is a preduzetnik. **The sweep of 31.07.2026 corrected nine rows and missed this one** — it is the same read-the-opening-stav defect, still live | ZEOR čl. 50 st. 1 vs čl. 51 | **CRITICAL** |
| 2 | Row 15, line 70 | *(implied replacement)* preduzetnik = 300.000–500.000 | **Do not print any ZEOR number.** čl. 51 names *"fizičko lice koje ima zaposlene"* and never says "preduzetnik"; 300.000–500.000 sits **above** the ZoP čl. 39 st. 1 tač. 1 ceiling for a fizičko lice (5.000–150.000) and inside the preduzetnik band (10.000–500.000). Provenance resolved (101/2005 čl. 81), the tier conflict is not — see §6 W-1 | ZEOR čl. 51; ZoP čl. 39 st. 1 | **HIGH — UNRESOLVED** |
| 3 | Row 15, line 70 | Legal basis given as "ZEOR čl. 24, 25 st. 3, 50–51" | Omits the three articles that do the work: **čl. 3 st. 1** (the only article that binds a preduzetnik at all), **čl. 23** (what the wage record captures — separately penalised at čl. 50 st. 1 tač. 2), and **čl. 44 st. 1–2** (records must be kept *po propisanim jedinstvenim metodološkim principima* and coded per the *jedinstveni kodeks šifara*) | ZEOR čl. 3 st. 1, čl. 23, čl. 44 | **HIGH** |
| 4 | Row 15, line 70 | Row treats ZEOR as prescribing per-employee **daily** hour tracking | ZEOR prescribes **classified period totals** (čl. 24 tač. 1), not timestamps and not a daily cadence. The only *daily* duty in Serbian law is ZoR čl. 55 st. 6, and it covers **overtime only** | ZEOR čl. 24 tač. 1; ZoR čl. 55 st. 6 | **HIGH** |
| 5 | Row 15, line 70 | Row is silent on the 2009 partial repeal | ZEOR čl. 8–22 and čl. 26–40 ceased 23.05.2009. čl. 50 st. 1 tač. 4), 5), 6), 8) and 9) reference dead articles; only tač. 1), 2), 3) and 7) have live referents. A reader could implement repealed obligations | ZZOSN čl. 110 | **MEDIUM** |
| 6 | Row 15 + `compliance/*.md` | "trajno" applied as a blanket to all working-time data | **Two classes, two clocks.** `trajno` binds the *evidencija o zaposlenim licima* (čl. 7 st. 2) and the *evidencija o zaradama* (čl. 25 st. 3) — and by čl. 24 tač. 1 ž) the overtime **hours** ride inside the latter. It does **not** bind raw punch events, device/IP metadata or draft rosters. Note also that **čl. 4 st. 2 is not a retention rule** — *"Evidencije određene ovim zakonom vode se trajno"* is continuity of maintenance. Cite **čl. 25 st. 3** | ZEOR čl. 4 st. 2 vs čl. 7 st. 2, čl. 25 st. 3 | **HIGH** |
| 7 | Row 14, line 69 | Cited as "čl. 276 st. 1 tač. 1a (pravno lice) / **preduzetnik tier same čl. 276**" | Vague where precision matters. **There is no separate preduzetnik stav in čl. 276.** Both tiers sit in the *uvodna rečenica* of st. 1 — unlike čl. 273, 274 and 275, each of which puts the preduzetnik in its own st. 2. Cite **"čl. 276 st. 1 (uvodna rečenica) u vezi sa tač. 1a)"**. This drafting asymmetry is *why* the original "up to 300k" error happened | ZoR čl. 276 st. 1 vs čl. 273/274/275 st. 2 | **MEDIUM** |
| 8 | Row 14, line 69 | Frames the overtime register as the working-time exposure | **The register is the smaller fine.** Ordering overtime beyond the čl. 53 caps is čl. 274 st. 1 tač. 3 — preduzetnik **200.000–400.000**, up to ~2.7× the 50.000–150.000 record-keeping fine. Absent from the register entirely | ZoR čl. 274 st. 1 tač. 3 + st. 2 | **HIGH** |
| 9 | **§6 item 11, line 192** | Open: "Whether any bylaw prescribes a mandatory FORM…" | **CLOSED — NO.** See §2. Verified four ways. **"Pravilnik 120/2014" does not exist** and must be struck | §2 | **RESOLVED** |
| 10 | §6 item 11, line 192 | Mitigation reads "free-form electronic record sufficient" | True for the **ZoR čl. 55 st. 6** record. **False if generalised** to the ZEOR wage record, whose methodology, entry rules and codes are prescribed (Odluka, Sl. list SRJ 40/97, tač. 2 i 3; Odluka o Jedinstvenom kodeksu šifara, Sl. glasnik RS 56/2018, 101/2020, 74/2021) | ZEOR čl. 44; Odluka tač. 2 | **HIGH** |
| 11 | **`COMPETITIVE-GAP-ANALYSIS.md` line 85** | "Mandatory (Zakon o radu + **Pravilnik 120/2014**), **2-year retention**…" | Two fabrications in one cell. No such pravilnik exists. **ZoR contains no retention period at all** — a full-text grep for `čuvaju se \| čuva se \| trajno \| arhiv` over the consolidated ZoR returns **zero** hits. The "2 years" figure traces to a single blog (radnapravda.rs); the "6 years" variant is Croatian law. *(The "50k–150k for a preduzetnik" half of that cell is correct.)* | ZoR (negative); §2 | **HIGH** |
| 12 | `COMPETITIVE-GAP-ANALYSIS.md` line 85 | "You **already store the raw material** (users + shift open/close)" | **A POS register shift is not an employee's working time.** Breaks count as working time (ZoR čl. 64 st. 5); an employee can be at work and off a till; two employees can share one register. The record is per-**employee**, per-**calendar day**. Row 14's own status line ("shifts are per-register, not per-employee") is the correct one | ZoR čl. 64 st. 5 | **MEDIUM** |
| 13 | Register — **missing row** | — | **e-Bolovanje** (the *"109/2025 - dr. zakon"* already showing in the ZoR citation line) is not in the register. It disapplies ZoR čl. 103 and čl. 179 st. 3 tač. 2 **and** ZZO čl. 152 st. 5 i čl. 154 st. 6 (čl. 14), and gives a **preduzetnik with employees until 1 January 2027** to register (čl. 13); penalty čl. 10 st. 2 = 10.000–50.000. Add it as a **"Shop, not software"** row so nobody mistakes it for a VantumPOS feature | Zakon 109/2025 čl. 10, 13, 14, 15 | **MEDIUM** |
| 14 | Register — **missing row** | — | ZoR **čl. 83 st. 1** gives the employee a statutory right of inspection, rectification and erasure *vis-à-vis the employer*, independent of ZZPL čl. 26. It is cheap to discharge in software and is not in the register | ZoR čl. 83 st. 1 | **LOW** |
| 15 | **`compliance/obavestenje-zaposlenima.md` line 5** | "Propust: novčana kazna 50.000–2.000.000 RSD (**čl. 95 st. 1 tač. 20**)" | **Two errors in one parenthesis.** (a) Failing to give the čl. 23 information is **st. 1 tač. 8**; tač. 20 is the čl. 42 privacy-by-design offence. (b) The amount is the pravno-lice tier — for a preduzetnik it is **20.000–500.000 (čl. 95 st. 4)**. The 31.07.2026 sweep fixed this exact figure in register rows 9 and 10 and **did not propagate into the template** | ZZPL čl. 95 st. 1 tač. 8; st. 4 | **HIGH** |
| 16 | **`compliance/evidencija-obrade-cl47.md` line 21** | Processor record headed "(ZZPL **čl. 47 st. 2**)" | čl. 47 **st. 2** is the disapplication for *nadležni organi u posebne svrhe*. The obrađivač record is **čl. 47 st. 4** — confirmed by st. 9's own opening *"Odredbe st. 1. i 4. ovog člana ne primenjuju se…"* | ZZPL čl. 47 st. 4 | **MEDIUM** |
| 17 | **`compliance/evidencija-obrade-cl47.md` lines 5 and 30** | "fiksna kazna **100.000 RSD**" (čl. 95 st. 2 tač. 5 and tač. 6) | Pravno-lice tier, in a document written for a preduzetnik. **Preduzetnik: fixed 50.000 (čl. 95 st. 6).** Same defect class, third file | ZZPL čl. 95 st. 6 | **HIGH** |

### 1.1 What survived verification unchanged — do not touch

- **Row 14's figures.** `ZoR čl. 55 st. 6` is the correct article *and* the correct stav (čl. 55 has exactly six stavovi; st. 6 is the last). `čl. 276 st. 1 tač. 1a` is right including the unusual "1a" designation. `Preduzetnik 50.000–150.000` is verbatim. Confirmed on four independent reads **and against the enacting instrument itself** — Zakon o izmenama i dopunama Zakona o radu, "Sl. glasnik RS" 113/2017, čl. 2 (added čl. 55 st. 6) and čl. 7 (rewrote the čl. 276 st. 1 uvodna rečenica and inserted tač. 1a). Neither provision has been touched since.
- **ZoR has no privredni prestup at all.** čl. 273, 274, 275, 276 and 276a all read *"kazniće se za prekršaj"*. Grep-verified. **A preduzetnik is therefore directly liable**, and the ZPP čl. 6 st. 1 branch does not bite here.
- **ZoR prescribes no zaštitna mera.** A grep of čl. 273–276a for `zaštitn` returns zero. Per standing project rule, say only *"these articles prescribe none"* — never *"a trading ban is unavailable"* (ZoP čl. 55 st. 2 remains unresolved, SW11-SW15 §5 Q-10).
- **No small-employer exemption.** The only headcount thresholds in ZoR concern višak zaposlenih (čl. 153: 20/100/300). The duty binds from the first employee.
- **The ZoR consolidation is current.** Amendment chain verified against the official PIS registry entry: 24/2005, 61/2005, 54/2009, 32/2013, 75/2014, 13/2017 - odluka US, 113/2017, 95/2018 - autentično tumačenje, 109/2025 - dr. zakon. In the consolidated body only čl. 37, 103, 147, 179 and 204 carry amendment footnote markers — **čl. 55 and čl. 276 carry none.** No Zakon o izmenama i dopunama ZoR exists for 2024–2026. The announced new Zakon o radu was still in preparation as at 01.08.2026.

---

## 2. Does a prescribed obrazac exist?

**Answer, unambiguously: NO for the record SW-14 builds. The module designs its own layout. But the answer flips for the ZEOR wage record it feeds, and that boundary is load-bearing.**

### 2.1 The ZoR čl. 55 st. 6 daily overtime record — no obrazac, no prescribed content, no prescribed medium

> **ZoR čl. 55 st. 6** — "Poslodavac je dužan da vodi dnevnu evidenciju o prekovremenom radu zaposlenih."
> Source: https://www.paragraf.rs/propisi/zakon_o_radu.html · origin: Zakon o izmenama i dopunama ZoR, "Sl. glasnik RS" 113/2017, čl. 2 — https://www.paragraf.rs/izmene_i_dopune/171217-zakon-o-izmenama-i-dopunama-zakona-o-radu.html

That nine-word sentence is the entire duty. **It names zero fields, prescribes no obrazac, no medium, no retention period, and contains no delegating clause.** Four independent lines of proof:

1. **No delegation in the operative text.** čl. 55 st. 6 contains no *"propisuje ministar"*. **Method rule, learned the hard way: do NOT use ZoR čl. 277 as a delegation index.** It is a 2005 transitional savings clause under *"XXIII PRELAZNE I ZAVRŠNE ODREDBE"* — *"Do donošenja podzakonskih akata iz čl. 46. stav 2, 96. stav 5, 103. stav 6, 204. stav 6, 217. stav 2. i 266. stav 2. ovog zakona, ostaju na snazi:"* followed by six pravilnici from 1997–2002. It is disproven as an index by counterexample: **ZoR čl. 121 st. 8** (*"Sadržaj obračuna iz st. 1. i 2. ovog člana propisuje ministar."*) and **čl. 140** (*"Sadržaj obrasca iz stava 1. ovog člana … propisuje ministar."*) are delegations absent from its list. Test for delegation by reading the operative article's own text.
2. **The contrast pattern.** Where Serbia wants a form it says so and prints it. *Pravilnik o načinu vođenja i rokovima čuvanja evidencija u oblasti bezbednosti i zdravlja na radu* ("Sl. glasnik RS" 5/2025, 38/2025, 118/2025 i 57/2026) čl. 2 st. 4: *"Poslodavac je dužan da evidencije vodi na propisanim obrascima (obrasci 1-11), koji su odštampani uz ovaj pravilnik i čine njegov sastavni deo."* — and čl. 2 st. 3 additionally demands a *kvalifikovani elektronski potpis* when they are kept electronically. **čl. 55 st. 6 has no counterpart, and the BZR e-signature requirement must not be imported.** Similarly ZoR čl. 121 st. 8 *did* delegate → Pravilnik o sadržaju obračuna zarade, odnosno naknade zarade ("Sl. glasnik RS" 90/2014 i 44/2018 - dr. zakon) — and even that prescribes **content only**, ending at čl. 5 with no obrazac attached.
3. **The Ministry's own published list** of podzakonska akta for the Sektor za rad i zapošljavanje contains no such pravilnik.
4. **Electronic-only is expressly lawful.** ZEOR čl. 45 st. 1 lists *"sredstva za automatsku obradu podataka"* on equal footing with kartoteke, knjige and obrasci. No wet signature, no bound book, no notarisation.

**"Pravilnik o sadržini evidencije o radu, Sl. glasnik RS 120/2014" does not exist in Serbian law.** An exact-phrase search returns exactly one source on the whole web — the blog `radnapravda.rs` — which also asserts a fabricated two-year retention "under čl. 122". Paragraf's register lists no in-force act published in Sl. glasnik RS 120/2014 in this field. **Most likely origin (inference, not established): a digit corruption of "Sl. glasnik RS" 90/2014**, the real 2014 Serbian labour bylaw in this area. Foreign attribution was tested and ruled out for Croatia (NN 32/15 → NN 73/17 → NN 55/2024), FBiH (Sl. novine FBiH 92/16) and North Macedonia (Sl. vesnik 78/2015) — the forensic question stays UNRESOLVED and is immaterial. **Treat any future search snippet naming "120/2014" or "two years" as contaminated by that single blog.**

### 2.2 The ZEOR records — NOT free-form. Do not carry the answer across.

> **ZEOR čl. 44 st. 1 i st. 2** — "Evidencije u oblasti rada utvrđene ovim zakonom vode se po propisanim jedinstvenim metodološkim principima. Unošenje podataka u evidencije vrši se prema propisanom jedinstvenom kodeksu šifara."
> **ZEOR čl. 50 st. 1 tač. 1)** — "ako ne vodi **ili ako ne vodi po propisanim jedinstvenim metodološkim principima** evidencije iz člana 2. tač. 1), 4), 6) … i tačke 8) ovog zakona (član 3. stav 1. i član 44. stav 1);"
> **ZEOR čl. 45 st. 3** — "Prijave i izveštaji sa podacima iz evidencije u oblasti rada, utvrđeni ovim zakonom, dostavljaju se na propisanim obrascima."
> Source: https://www.paragraf.rs/propisi/zakon_o_evidencijama_u_oblasti_rada.html · full text also at https://ius.bg.ac.rs/wp-content/uploads/2020/11/ZAKON-o-evidencijama-u-oblasti-rada.doc

Two live bylaws execute those two delegations:

> **Odluka o jedinstvenim metodološkim principima za vođenje evidencija u oblasti rada i obrascima prijava i izveštaja** ("Sl. list SRJ", br. 40/97 i 25/2000, "Sl. list SCG", br. 1/2003 - Ustavna povelja, "Sl. glasnik RS", br. 15/2010 - dr. pravilnik i 54/2010 - dr. uredba), **tač. 2** — "Evidencije u oblasti rada vode se unošenjem podataka u obrasce propisane ovom odlukom."
> **tač. 10–11** (surviving list) — "1) E-1 …; 2) E-2 …; **tač. 3) i 4) (Prestale da važe)**; 5) E-4 …; 6) E-4/1 …; 7) E-5 …; 8) E-6 …; 9) E-6/1 …; 10) E-7 …" · "Obrasci E-1 do E-7 iz tačke 10. odštampani su uz ovu odluku i čine njen sastavni deo."
> **tač. 12** — "Evidencija o korisnicima prava invalidskog osiguranja vodi se **na obrascu MIN** koji je propisan Odlukom o obrascima prijava podataka za matičnu evidenciju…"
> **Napomena (second partial cessation)** — "Tač. 10. i 11. … u delu koji se odnosi na zapošljavanje, **osim obrasca E-3 … i obrasca E-3/1 …** prestaju da važe **27. marta 2010. godine**, danom stupanja na snagu Pravilnika o bližoj sadržini podataka i načinu vođenja evidencija u oblasti zapošljavanja ("Sl. glasnik RS", br. 15/2010)."
> Source: https://www.paragraf.rs/propisi/odluka-metodoloskim-principima-vodjenje-evidencija.html
>
> **Odluka o Jedinstvenom kodeksu šifara za unošenje i šifriranje podataka u evidencijama u oblasti rada** ("Sl. glasnik RS", br. 56/2018, 101/2020 i 74/2021), adopted *"a u vezi sa članom 44. stav 2. Zakona o evidencijama u oblasti rada"*, applied from 01.01.2019. Coded obeležja: **zanimanje; nivo i vrsta kvalifikacija; država i teritorija; opština; naseljeno mesto.** Ministry portal: https://kodekssifara.minrzs.gov.rs/

**Consequence.** ZEOR čl. 45 st. 1 makes electronic media lawful; it does **not** make the schema free. The wage record's identity fields — which čl. 24 imports from čl. 5 st. 1 tač. 7)–13): delatnost poslodavca, zanimanje, vrsta i stepen stručne spreme — are precisely the obeležja the 2018/2021 Kodeks codes. **Store zanimanje and nivo/vrsta kvalifikacija as CODE + label, never free text.** And note tač. 12: the *evidencija o korisnicima prava invalidskog osiguranja* is an **employer-kept ZEOR record on a prescribed obrazac** — so a blanket "ZEOR prescribes no forms" is false.

### 2.3 Resolution of §6 item 11 — explicit

| Half of the open item | Resolution |
|---|---|
| "Whether any bylaw prescribes a mandatory FORM for the daily work-time/overtime record" | **CLOSED. NO.** ZoR čl. 55 st. 6 delegates nothing; no ministerial act exists; ZEOR čl. 45 st. 1 permits automatic data processing. **Do not build a statutory-form renderer for SW-14, and do not carry over the KEP (SW-9c) print-fidelity constraints — that module reproduces a real 5-column obrazac, this one does not.** |
| "'Pravilnik 120/2014' appears to be a mis-citation — found on one blog only, likely foreign" | **CONFIRMED. It does not exist.** Strike it from `COMPETITIVE-GAP-ANALYSIS.md:85` and from §6 item 11. The "likely foreign" hypothesis is **not** confirmed; a digit corruption of 90/2014 is likelier. Record the negative so it is not re-litigated. |
| Mitigation text "free-form electronic record sufficient" | **Keep, but scope it to the ZoR record only.** Add: *"the ZEOR evidencija o zaradama is subject to propisani jedinstveni metodološki principi and the jedinstveni kodeks šifara — its schema is not free."* |
| **New item spawned** | Whether a surviving obrazac covers the *evidencija o zaposlenim licima* or the *evidencija o zaradama* specifically. The negative rests solely on tač. 10's surviving list not naming one; **Odluka tač. 3–9 was never read in full by any pass**, and tač. 12 proves the Odluka prescribes forms outside tač. 10. → §6 **W-2**. |

**Product consequence of §2, in one line:** VantumPOS may design its own table, columns, ordering and print layout for the overtime register — but **must never label any output a "propisani obrazac", and must never claim to produce a "propisana evidencija o zaradama".**

---

## 3. Verified rules, per topic

### W1 — The daily overtime record: duty, scope, penalty

**Encodable rule.** Every poslodavac must keep a **per-employee, per-calendar-day** record of employees' **prekovremeni rad**. "Dnevna" fixes the record's **granularity**, not a filing or printing cadence — nothing is submitted to any authority. The record must exist and be producible to the inspektor rada on demand. The duty is conditional in practice: it bites where prekovremeni rad within the meaning of čl. 53 actually occurs — an employer whose staff never exceed puno radno vreme has an **empty register, not a breach** — but the register must exist the moment overtime is ordered.

**Scope trap.** čl. 55 st. 6 is **overtime-only**. It creates no general attendance, clock-in or ordinary-hours record. **It also does not discharge the employer's working-time record duties** — those live in ZEOR čl. 23–24 (W2) and ZoR čl. 122.

> **ZoR čl. 55 st. 6** — "Poslodavac je dužan da vodi dnevnu evidenciju o prekovremenom radu zaposlenih."
> **ZoR čl. 276 st. 1, uvodna rečenica** — "Novčanom kaznom od 150.000 do 300.000 dinara kazniće se za prekršaj poslodavac sa svojstvom pravnog lica, **a preduzetnik sa kaznom od 50.000 do 150.000 dinara**:"
> **ZoR čl. 276 st. 1 tač. 1a)** — "1a) ako ne vodi dnevnu evidenciju o prekovremenom radu zaposlenih u skladu sa odredbama ovog zakona (član 55. stav 6);"
> **ZoR čl. 276 st. 2** — "Novčanom kaznom od 10.000 do 20.000 dinara kazniće se za prekršaj iz stava 1. ovog člana odgovorno lice u pravnom licu, odnosno zastupnik pravnog lica."
> **ZoR čl. 274 st. 1 tač. 3 + st. 2** — "3) ako zaposlenom odredi prekovremeni rad suprotno odredbama ovog zakona (član 53); […] Novčanom kaznom od 200.000 do 400.000 dinara za prekršaj iz stava 1. ovog člana kazniće se preduzetnik."
> Source: https://www.paragraf.rs/propisi/zakon_o_radu.html

**Binds whom.** Every *poslodavac* — ZoR čl. 2 st. 1 defines it as any *"domaće ili strano pravno, odnosno fizičko lice"*. čl. 276 st. 1 names the preduzetnik as a distinct fine addressee; čl. 192 tač. 2 and čl. 270 (*"poslodavac, odnosno direktor ili preduzetnik"*) confirm. **No headcount, turnover or micro-employer carve-out anywhere.** Scope of persons: *zaposleni* under a ugovor o radu. Persons engaged outside a radni odnos (ZoR čl. 197–202) are outside the record — but misclassifying a shop assistant to dodge it is itself čl. 273 st. 1 tač. 1 (preduzetnik 300.000–500.000).

**Penalties — ZoR. All prekršaji; ZoR contains no privredni prestup.**

| Article | Predicate (working-time relevant) | Pravno lice | **Preduzetnik** | Odgovorno lice u pravnom licu / zastupnik |
|---|---|---|---|---|
| **čl. 276 st. 1** (both tiers in the uvodna rečenica) / st. 2 | **t. 1a)** čl. 55 st. 6 overtime register · t. 2) čl. 64–67 rest · t. 3) čl. 77 paid leave · t. 4) čl. 122 monthly wage record | 150.000–300.000 | **50.000–150.000** | 10.000–20.000 (st. 2) |
| **čl. 274** st. 1 / st. 2 / st. 3 | t. 3) čl. 53 overtime caps · t. 4) čl. 57, 60 preraspodela · t. 5) čl. 62 night · t. 6) čl. 63 shifts · t. 7) čl. 84, 87, 88 minors | 600.000–1.500.000 | **200.000–400.000** | 30.000–150.000 |
| **čl. 275** st. 1 / st. 2 / st. 3 | t. 3) čl. 68–75 annual leave incl. the rešenje | 400.000–1.000.000 | **100.000–300.000** | 20.000–40.000 |
| **čl. 273** st. 1 / st. 2 / st. 3 | t. 1) engaging work outside a radni odnos contrary to the law | 800.000–2.000.000 | **300.000–500.000** | 50.000–150.000 |

**Can a preduzetnik be the subject? YES, of all four, as prekršaji, personally.** The čl. 276 st. 2 odgovorno-lice tier reaches only an odgovorno lice or zastupnik **of a pravno lice** — **a preduzetnik faces one fine, not a company fine plus a personal one.** Suppress that line entirely for preduzetnik tenants.

**Zaštitna mera:** none prescribed anywhere in čl. 273–276a. State only that; do not assert immunity from closure (ZoP čl. 55 st. 2, unresolved).

**Limitation.** ZoP čl. 84 st. 1 — one year relative; st. 7 — two years absolute. Labour is not in the čl. 84 st. 5 extended list.

**Drafting asymmetry, to be recorded in the compliance register once:** čl. 273, 274 and 275 each put the preduzetnik tier in a **separate stav 2**; **čl. 276 embeds it in the uvodna rečenica of stav 1**. Anyone scanning for a "stav 2 = preduzetnik" pattern in čl. 276 will misread it and print 150.000–300.000. That is exactly what happened before the 31.07.2026 correction, and čl. 276 is also the hook for the čl. 122 monthly wage-record duty — so the same slip will recur unless the asymmetry is documented.

---

### W2 — ZEOR: still in force, and what survives

**Encodable rule.** A preduzetnik with employees is bound by ZEOR as a *"fizičko lice koje ima zaposlene"* and must keep, **trajno**, the **evidencija o zaposlenim licima** (čl. 5–7) and the **evidencija o zaradama zaposlenih lica** (čl. 23–25). The wage record is where the **statutory hour classification** lives. ZEOR requires **classified period totals**, never timestamps.

> **ZEOR čl. 3 st. 1** — "Evidencije iz člana 2. tač. 1), 4), 6) - o zaposlenim licima koja poslodavci upućuju na privremeni rad u svoje poslovne jedinice u inostranstvu, 7) - o zaposlenim strancima u Saveznoj Republici Jugoslaviji i tačke 8) ovog zakona, vode preduzeća i druga pravna lica, državni organi i organizacije, organi jedinica lokalne samouprave i **fizička lica koja imaju zaposlene** (u daljem tekstu: poslodavci), ako drugim saveznim zakonom nije drukčije određeno."
> **ZEOR čl. 23** — "U evidenciju o zaradama zaposlenih lica unose se podaci o radnom vremenu, bruto zaradi, neto zaradi, neto naknadi zarade, bruto naknadi zarade iz sredstava poslodavca, dodacima, naknadama i drugim primanjima, kao i bruto zaradama ostvarenim na teret drugih poslodavaca."
> **ZEOR čl. 24** (opening) — "Evidencija o zaradama zaposlenih lica sadrži podatke iz **člana 5. stav 1. tač. 1) i 2) i tač. 7) do 13)** ovog zakona. Pored tih podataka, evidencija sadrži i: **1) podatke o radnom vremenu i njegovom korišćenju:**"
> **ZEOR čl. 25 st. 1 i st. 3** — "Evidencija o zaradama zaposlenih lica za pojedino zaposleno lice počinje da se vodi danom početka rada, a prestaje danom prestanka radnog odnosa. […] **Podaci iz evidencije o zaradama zaposlenih lica čuvaju se trajno.**"
> **ZEOR čl. 7 st. 2** — "Podaci iz evidencije o zaposlenim licima čuvaju se trajno."
> **ZEOR čl. 46 st. 1** — "Za tačnost podataka u evidenciji odgovoran je subjekt koji vodi evidenciju."
> **ZEOR čl. 50 st. 1 tač. 3)** — "ako **ne čuva trajno** podatke iz evidencije o zaposlenim licima (član 7. stav 2), evidencije o zaradama zaposlenih (član 25. stav 3), … ;"
> **ZZOSN čl. 110** (the partial repeal) — "Danom stupanja na snagu ovog zakona prestaje da važi: … odredbe **čl. 8-22. i čl. 26-40.** Zakona o evidencijama u oblasti rada …"
> Sources: https://www.paragraf.rs/propisi/zakon_o_evidencijama_u_oblasti_rada.html · https://www.paragraf.rs/propisi/zakon_o_zaposljavanju_i_osiguranju_za_slucaj_nezaposlenosti.html · fine provenance: Zakon o izmenama zakona kojima su određene novčane kazne za privredne prestupe i prekršaje, "Sl. glasnik RS" 101/2005, čl. 81 — http://demo.paragraf.rs/demo/combined/Old/t/t2005_11/t11_0189.htm

**Penalties — ZEOR. All prekršaji (čl. 50 opens "kazniće se za prekršaj"), so ZPP čl. 6 st. 1 does not bite; the tier split does.**

| Subject | Range | Article | Status |
|---|---|---|---|
| Preduzeće ili drugo **pravno lice** | 500.000–1.000.000 | čl. 50 st. 1 | **Verbatim, settled.** This is the figure register row 15 wrongly attributed to the pilot |
| **Odgovorno lice** (u državnom organu/organizaciji, organu JLS, preduzeću ili drugom pravnom licu) | 30.000–50.000 | čl. 50 st. 2 | Verbatim. **Structurally inapplicable to a preduzetnik** |
| **"Fizičko lice koje ima zaposlene"** | 300.000–500.000 | **čl. 51** | **Text verbatim and settled. Attribution to a preduzetnik and validity of the amount: UNRESOLVED — render no number.** See below and §6 W-1 |
| Fizičko lice — **the EMPLOYEE**, for failing to report a change of own data within 8 days (čl. 6) | do 30.000 | čl. 52 | Verbatim. Binds the employee, **not** the shop. Do not model as employer risk |

**Why the čl. 51 number is quarantined.** ZEOR is a 1996 SRJ act that never uses the word *preduzetnik*. Its amounts come from the 101/2005 revalorisation (verbatim: *"U članu 51. reči: 'od 250 do 2.500 novih dinara' zamenjuju se rečima: 'od 300.000 do 500.000 dinara'."*). But **Zakon o prekršajima čl. 39 st. 1** caps what may be prescribed: *"1) od 5.000 do 150.000 dinara za fizičko lice ili odgovorno lice; 2) od 50.000 do 2.000.000 dinara za pravno lice; **3) od 10.000 do 500.000 dinara za preduzetnika.**"* — so 300.000–500.000 sits **3.3× above** the ceiling for the class ZEOR actually names, and fits **only** the preduzetnik band. That asymmetry is the strongest textual support for the preduzetnik reading **and** simultaneously the reason the figure cannot be quoted with confidence: ZoP's transitional command (*"Propisi o prekršajima koji nisu u skladu sa ovim zakonom uskladiće se u roku od jedne godine…"*) was never complied with by ZEOR, which was last touched in 2009. The consolidated text marks čl. 50, 51 and 52 with an asterisk whose footnote is not rendered on the public page. **`[LEGAL-INFERRED]`, not `[LEGAL]` — the exposure is real, the number is not shippable.**

**Two omissions the register must absorb.** (1) **ZEOR čl. 25 st. 2** — *"Izveštaji sa podacima iz evidencije o zaradama zaposlenih lica dostavljaju se organizaciji za penzijsko i invalidsko osiguranje."* — is verbatim alive and penalised at čl. 50 st. 1 tač. 7). Employer M-4/M-UN submission was reportedly abolished from 2019 in favour of CROSO/PPP-PD, **but no pass read the abolishing ZPIO article verbatim.** Do not issue a "do not build" directive against a live, penalised duty on secondary evidence — mark it **out of VantumPOS scope (F-12)** and flag it, §6 W-5. (2) **ZEOR čl. 49 st. 2** confidentiality and čl. 46 st. 1 accuracy responsibility both attach to whoever keeps the record.

---

### W3 — Medium, access and the "produce on demand" constraint

**Encodable rule.** There is **no place-of-keeping rule for evidencije**. ZoR čl. 35 st. 1 imposes physical presence only for the *ugovor o radu*, and has no analogue for records. What the law requires is **uvid on the inspector's demand, in whatever form the subject holds them.**

> **ZoR čl. 268b** — "Poslodavac, odgovorno lice kod poslodavca i zaposleni dužni su da inspektoru omoguće vršenje nadzora, uvid u dokumentaciju i nesmetan rad i da mu obezbede podatke potrebne za vršenje inspekcijskog nadzora, u skladu sa zakonom."
> **Zakon o inspekcijskom nadzoru čl. 20 st. 7** — "…obezbedi uvid u poslovne knjige, opšte i pojedinačne akte, evidencije, izveštaje, ugovore, privatne isprave i drugu dokumentaciju nadziranog subjekta od značaja za inspekcijski nadzor, **a u obliku u kojem ih poseduje i čuva**;"
> **ZIN čl. 21 tač. 4)** — "…naloži da mu se u određenom roku stave na uvid poslovne knjige, opšti i pojedinačni akti, evidencije, ugovori i druga dokumentacija … **a u obliku u kojem ih nadzirani subjekat poseduje i čuva**;"
> **ZEOR čl. 45 st. 1** — "Evidencije u oblasti rada utvrđene ovim zakonom vode se unošenjem podataka u kartoteke, knjige, obrasce, **sredstva za automatsku obradu podataka** i druga sredstva za vođenje evidencija."
> Sources: https://www.paragraf.rs/propisi/zakon_o_radu.html · https://www.paragraf.rs/propisi/zakon_o_inspekcijskom_nadzoru.html · https://www.paragraf.rs/propisi/zakon_o_evidencijama_u_oblasti_rada.html

**Consequence for the build.** An electronic record is compliant **as held**. The real compliance risk is not layout — it is **availability**: cloud-only with no offline fallback fails the "produce on demand" test at a counter with no network. Ship an offline-capable on-device view **plus** an immediate print/PDF export.

**Inspection practice** was not settled from any primary text and is labelled **practice, not authority**: the official Inspektorat za rad *Kontrolna lista — inspekcijski nadzor u oblasti radnih odnosa* (minrzs.gov.rs, adopted 2019) asks *"Da li je prekovremeni rad zaposlenih organizovan u skladu sa zakonom?"* and *"Da li se kod poslodavca vodi evidencija zarade i naknade zarade?"* and **references no obrazac, no layout and no medium anywhere** — consistent with a form-free duty, but a 2019 administrative instrument, not a source of law.

---

### W4 — Fields, caps, timing, retention, signature

#### 4a. The field list — one row per (employee, calendar date)

**Store HOURS as the primitive, not timestamps.** The statute is hours-based; a timestamp model invents obligations the law does not impose. The fifteen buckets below are **the statutory enumeration**, from ZEOR čl. 24 tač. 1 a)–ž), read letter-by-letter and independently confirmed:

| Bucket | ZEOR čl. 24 tač. 1 | Class |
|---|---|---|
| `moguci_casovi` (puno / kraće od punog radnog vremena) | a) | `[LEGAL]` |
| `ukupno_ostvareni_casovi` | b) | `[LEGAL]` |
| `efektivno_izvrseni_casovi` | b), 1st indent | `[LEGAL]` |
| `casovi_cekanja_i_zastoja_i_prekida` | b), 2nd indent | `[LEGAL]` |
| `casovi_obustave_rada_zbog_strajka` | b), 3rd indent | `[LEGAL]` |
| `ukupno_neizvrseni_casovi` | v) | `[LEGAL]` |
| `casovi_godisnjeg_odmora` | g) | `[LEGAL]` |
| `casovi_odmora_za_dane_drzavnih_praznika` | g) | `[LEGAL]` |
| `casovi_odsustva_uz_naknadu_zarade` | g) | `[LEGAL]` |
| `casovi_strucnog_osposobljavanja_i_usavrsavanja` | g) | `[LEGAL]` |
| **`casovi_privremene_sprecenosti__poslodavac`** | **g)** — naknada iz sredstava poslodavca (ZoR čl. 115, first 30 days) | `[LEGAL]` |
| `casovi_naknade_na_teret_drugih_poslodavaca` | d) | `[LEGAL]` |
| **`casovi_privremene_sprecenosti__rfzo`** | **đ)** — iz sredstava organizacija za zdravstveno osiguranje | `[LEGAL]` |
| `casovi_porodiljskog_i_skracenog_rv_roditelja` | đ) | `[LEGAL]` |
| `casovi_neplacenog_odsustva` | e) | `[LEGAL]` |
| **`casovi_prekovremenog_rada`** | **ž)** — "časovi rada dužeg od punog radnog vremena" | `[LEGAL]` — also the ZoR čl. 55 st. 6 column |

> **ZEOR čl. 24 tač. 1 b)** — "b) ukupno ostvareni časovi sa punim radnim vremenom i radnim vremenom kraćim od punog radnog vremena (puno i skraćeno radno vreme), od toga: - efektivno izvršeni časovi; - časovi čekanja na posao i časovi zastoja i prekida u radu; - časovi obustave rada zbog štrajka;"
> **ZEOR čl. 24 tač. 1 g)** — "g) ukupno neizvršeni časovi za koje se prima naknada zarade: - časovi godišnjeg odmora; - časovi odmora za dane državnih praznika; - časovi odsustva sa rada uz naknadu zarade; […] - časovi privremene nesposobnosti ili sprečenosti za rad;"
> **ZEOR čl. 24 tač. 1 e) i ž)** — "e) neizvršeni časovi za koje se ne prima naknada zarade; ž) časovi rada dužeg od punog radnog vremena;"

**Critical correction to the sick-leave design.** The phrase *"časovi privremene nesposobnosti ili sprečenosti za rad"* appears **twice** in čl. 24 tač. 1 — under **g)** (employer-funded) and under **đ)** (RFZO-funded). **The poslodavac/RFZO split is statutory, not optional.** An earlier design note that said "do NOT record whether the bolovanje is na teret poslodavca vs. RFZO" is wrong and must not be encoded. What stays prohibited is diagnosis, doznaka, ICD codes, free text and attachments (§5).

**Identity fields, by cross-reference** — čl. 24 → čl. 5 st. 1 tač. 1), 2) i 7) do 13): prezime i ime · matični broj (JMBG) · naziv i adresa poslodavca · delatnost poslodavca · zanimanje · vrsta i stepen stručne spreme · osposobljenost za obavljanje određenih poslova · naziv radnog mesta · **ugovoreno radno vreme u časovima (nedeljno)**. `[LEGAL]` — but zanimanje and nivo/vrsta kvalifikacija must carry the **Kodeks šifara code**, per §2.2.

**Explicitly NOT statutory fields — label them in code and UI as `izračunato radi provere usklađenosti`, never as `zakonom propisano polje`:** `nocni_casovi` (needed for the čl. 62 st. 2 thresholds and the čl. 108 st. 1 tač. 2 uplift) · `casovi_rada_na_praznik` (čl. 108 st. 1 tač. 1) · clock-in/clock-out times · break start/end. **No Serbian provision requires a start/end timestamp.** This distinction is the whole point of W4 — do not let a UI copywriter flatten it.

#### 4b. The overtime caps the record must make checkable — exact numbers

| Rule | Threshold | Article |
|---|---|---|
| Prekovremeni rad per week | **≤ 8 h** | čl. 53 st. 2 |
| Total per day incl. overtime | **≤ 12 h** | čl. 53 st. 3 |
| **Annual overtime ceiling** | **NONE EXISTS** — grep-verified negative. **Do not encode one** | — |
| No overtime at all on skraćeno radno vreme jobs | ban | čl. 53 st. 4 + čl. 52 |
| Under the čl. 56 st. 3 monthly-average scheme | **≤ 12 h/day AND ≤ 48 h/week** incl. overtime | čl. 56 st. 4 |
| Preraspodela — 6-month average | ≤ contracted working time, **within the calendar year** | čl. 57 st. 2 |
| Preraspodela — 9-month variant | **only "kolektivnim ugovorom"** — a preduzetnik on a pravilnik o radu is capped at 6 months | čl. 57 st. 3 |
| Preraspodela — absolute weekly ceiling | **≤ 60 h** | čl. 57 st. 5 |
| Preraspodela is NOT overtime | hard branch | **čl. 58** |
| Daily rest | ≥ 12 h; **≥ 11 h in preraspodela** | čl. 66 st. 1 / st. 2 |
| Weekly rest | ≥ 24 h + the daily rest; **≥ 24 h in shifts/preraspodela** | čl. 67 st. 1 / st. 4 |
| In-shift break | ≥ 30 min if ≥ 6 h · ≥ 15 min if > 4 h and < 6 h · ≥ 45 min if > 10 h; **counts as working time** | čl. 64 st. 1–3, st. 5 |
| Night window | **22:00–06:00** | čl. 62 st. 1 |
| Under 18 | overtime and preraspodela **banned**; ≤ 35 h/week and ≤ 8 h/day; night work banned (narrow exceptions) | čl. 88 st. 1; čl. 87; čl. 88 st. 2 |
| Parent of a child **up to 3** | overtime or night work only with **written consent** | čl. 91 st. 1 |
| **Samohrani roditelj — child up to SEVEN**, or a child who is a **težak invalid** | overtime or night work only with **written consent** | **čl. 91 st. 2** |
| Pregnant / nursing | no overtime or night work **if harmful, na osnovu nalaza nadležnog zdravstvenog organa** | čl. 90 |

> **ZoR čl. 53 st. 2 i st. 3** — "Prekovremeni rad ne može da traje duže od osam časova nedeljno. Zaposleni ne može da radi duže od 12 časova dnevno uključujući i prekovremeni rad."
> **ZoR čl. 58** — "Preraspodela radnog vremena ne smatra se prekovremenim radom."
> **ZoR čl. 57 st. 5** — "U slučaju preraspodele radnog vremena, radno vreme ne može da traje duže od 60 časova nedeljno."
> **ZoR čl. 64 st. 5** — "Vreme odmora iz st. 1-3. ovog člana uračunava se u radno vreme."
> **ZoR čl. 88 st. 1** — "Zabranjen je prekovremeni rad i preraspodela radnog vremena zaposlenog koji je mlađi od 18 godina života."
> **ZoR čl. 91** — "Jedan od roditelja sa detetom do tri godine života može da radi prekovremeno, odnosno noću, samo uz svoju pisanu saglasnost. / **Samohrani roditelj koji ima dete do sedam godina života** ili dete koje je težak invalid može da radi prekovremeno, odnosno noću, samo uz svoju pisanu saglasnost."

**Three operative conditions that must NOT be dropped** — each turns a conditional duty into an unconditional one if elided:

> **ZoR čl. 62 st. 2** (full) — "Zaposlenom koji radi noću najmanje tri časa svakog radnog dana ili trećinu punog radnog vremena u toku jedne radne nedelje poslodavac je dužan da obezbedi obavljanje poslova u toku dana **ako bi, po mišljenju nadležnog zdravstvenog organa, takav rad doveo do pogoršanja njegovog zdravstvenog stanja**."
> **ZoR čl. 90 st. 1** (full) — "Zaposlena za vreme trudnoće i zaposlena koja doji dete ne može da radi prekovremeno i noću, ako bi takav rad bio štetan za njeno zdravlje i zdravlje deteta, **na osnovu nalaza nadležnog zdravstvenog organa**."
> **ZoR čl. 57 st. 4** (full) — "**Zaposlenom koji se saglasio** da u preraspodeli radnog vremena radi u proseku duže od vremena utvrđenog u st. 2. i 3. ovog člana časovi rada duži od prosečnog radnog vremena obračunavaju se i isplaćuju kao prekovremeni rad."

So: the 3 h/⅓ night threshold is an **advisory flag**, never an automatic reassignment obligation. čl. 90 is a **stored flag set from a medical finding**, not an unconditional block. čl. 57 st. 4 conversion applies **only** to a consenting employee.

**Adjacent, also verified:** čl. 62 st. 3 requires the employer to seek the union's opinion before introducing night work (no carve-out on the face of the text where no union exists — §6 W-7); čl. 63 st. 3 caps continuous night work at one working week, st. 4 longer only with written consent; čl. 60 bars preraspodela on skraćeno radno vreme jobs; čl. 61 forces a choice on mid-period termination (convert to working time with delayed deregistration, or pay as overtime); čl. 56 st. 1 requires **5 days' advance notice** of the raspored (min 48 h under unforeseen circumstances, st. 2).

#### 4c. Timing — PARTIALLY UNRESOLVED

The only textual anchor is the word **"dnevnu"** in čl. 55 st. 6. It fixes per-day **granularity**. **No provision in ZoR, ZEOR or the Odluka states a deadline by which the entry must be made.** Serbia has **no analogue of the Croatian "by the 7th of the following month" rule — do not import it.** What *is* fixed: the wage record is monthly (ZoR čl. 122 st. 1) and the per-employee record runs *"danom početka rada"* to *"danom prestanka radnog odnosa"* (ZEOR čl. 25 st. 1, čl. 7 st. 1). Whether a month reconstructed at month-end from register data satisfies "dnevnu" is inspection practice, not text → §6 **W-3**. **Design as if daily entry is required.**

#### 4d. Retention — three clocks, and the purge must fail safe toward KEEPING

**ZoR sets no retention period for anything.** Grep-verified: zero hits for `čuvaju se | čuva se | trajno | arhiv` over the whole consolidated text. čl. 122 contains no retention rule (it has exactly three stavovi). **The circulating "2 years under čl. 122" is fabricated; "6 years" is Croatian law.**

| Record class | Period | Basis | Note |
|---|---|---|---|
| **Evidencija o zaposlenim licima** | **trajno** | ZEOR čl. 7 st. 2 | Under-retention is itself the offence (čl. 50 st. 1 tač. 3) |
| **Evidencija o zaradama** — incl. the čl. 24 tač. 1 hour classification and the overtime hours (ž) | **trajno** | ZEOR čl. 25 st. 3 | Same |
| Isplatne liste / analitičke evidencije zarada | **trajno** | ZoRač čl. 28 st. 6 | Accountant's side (F-12) |
| **Standalone ZoR čl. 55 st. 6 overtime register**, considered apart from the wage record it feeds | **no statutory period.** Defensive floor = **max(ZP čl. 84: 1 y relative / 2 y absolute; ZoR čl. 196: 3 y for money claims) → 3 years** | ZP čl. 84 st. 1, st. 7; ZoR čl. 196 | `[LEGAL-INFERRED]`. **Do not assert a statutory period for the log itself** |
| Raw punch events at minute granularity; device/terminal id, IP, workstation fingerprint; edit trails beyond defence needs; draft rosters; remote-support session copies | **bounded and purged** once the period is closed and the classification derived | ZZPL čl. 5 st. 1 tač. 3 i 5 | Keeping these forever is itself the breach |
| POS shift-open/shift-close records | **UNRESOLVED — do not auto-purge** | — | Simultaneously cash-control artefact and time evidence. Carried from §6 item 7 / SW11-SW15 Q-15 → §6 **W-8** |

> **ZP čl. 84 st. 1 i st. 7** — "Prekršajni postupak ne može se pokrenuti niti voditi ako protekne jedna godina od dana kada je prekršaj učinjen. […] Pokretanje i vođenje prekršajnog postupka zastareva u svakom slučaju kad protekne dva puta onoliko vremena koliko se po zakonu traži za zastarelost."
> **ZoR čl. 196** — "Sva novčana potraživanja iz radnog odnosa zastarevaju u roku od tri godine od dana nastanka obaveze."
> **ZZPL čl. 5 st. 1 tač. 5** — "se čuvati u obliku koji omogućava identifikaciju lica samo u roku koji je neophodan za ostvarivanje svrhe obrade ('ograničenje čuvanja');"

**The direction of the trade-off, stated once so the purge design cannot get it backwards.** For this shop, **under-retention outranks over-retention.** The ZEOR offence (čl. 50 st. 1 tač. 3, *"ako ne čuva trajno"*) sits at a preduzetnik exposure that is at minimum comparable to — and on the čl. 51 reading, larger than — the ZZPL over-retention exposure of 20.000–500.000 (čl. 95 st. 1 t. 1 → st. 4). **The trajno classes must be structurally unreachable by every purge, reset, restore and backup-prune path.**

#### 4e. Signature / acknowledgement — NO

**No provision requires the employee to sign or acknowledge the working-time record.** The only signature duty in this area is on the **monthly wage record**, and it is the **employer's**:

> **ZoR čl. 122 st. 1 i st. 3** — "Poslodavac je dužan da vodi mesečnu evidenciju o zaradi i naknadi zarade. […] **Evidenciju potpisuje lice ovlašćeno za zastupanje ili drugo lice koje ono ovlasti.**"
> **ZoR čl. 121 st. 6** — "Obračun zarade i naknade zarade koje je dužan da isplati poslodavac u skladu sa zakonom predstavlja **izvršnu ispravu**."
> **ZoR čl. 83 st. 1** — "Zaposleni ima pravo uvida u dokumente koji sadrže lične podatke koji se čuvaju kod poslodavca i pravo da zahteva brisanje podataka koji nisu od neposrednog značaja za poslove koje obavlja, kao i ispravljanje netačnih podataka."

VantumPOS does not produce the čl. 122 record (F-12), so **no signature affordance belongs in SW-14.** If an attestation feature is built anyway, label it *"interna potvrda zaposlenog"* and mark it optional. What the employee **does** have is ZoR čl. 83 st. 1 + ZZPL čl. 26 — discharge both with a read-only per-employee "moji sati" view.

---

### W5 — ZZPL: lawful basis, sick leave as health data, the retention split

**State of law.** ZZPL is still ("Sl. glasnik RS", br. 87/2018), **unamended** at 01.08.2026 — verified on the gazette reproduction (header: *"Osnovni tekst na snazi od 21/11/2018, u primeni od 22/08/2019"*, no amendment lines) and on a second consolidated source. A Ministarstvo pravde working group was constituted 30.12.2024 (Rešenje 119-01-100/2024-0) for *"izmene i dopune"*; the Poverenik has since been reported as saying a **new** law will be enacted instead. **Nothing adopted either way — build against 87/2018.**

**Lawful basis — hard-coded, never asked.**

> **ZZPL čl. 12 st. 1 tač. 3** — "obrada je neophodna u cilju poštovanja pravnih obaveza rukovaoca;"
> **ZZPL čl. 12 st. 1 tač. 2** — "obrada je neophodna za izvršenje ugovora zaključenog sa licem na koje se podaci odnose …"
> **ZZPL čl. 15 st. 4** — "Prilikom ocenjivanja da li je pristanak za obradu podataka o ličnosti slobodno dat, posebno se mora voditi računa o tome da li se izvršenje ugovora, uključujući i pružanje usluga, uslovljava davanjem pristanka koji nije neophodan za njegovo izvršenje."

**tač. 3 is primary** (ZoR čl. 55 st. 6 and ZEOR čl. 23–24 compel the record); **tač. 2 carries the residue** — computing the hours the wage is paid for, rostering. **Never consent:** in an employment relationship it is not *slobodno dat*, and it is incoherent to ask permission for a record the employer must keep regardless — a withdrawal under čl. 15 st. 3 would leave the employer either in breach of čl. 55 st. 6 or processing with no basis. Consent also imports the čl. 15 st. 1 demonstrability duty, breach of which is čl. 95 st. 1 tač. 5 — an exposure created **only** by choosing the wrong basis. **Never legitimni interes** (tač. 6): it carries a čl. 37 prigovor right the employer cannot honour on a statutory record.

**Sick leave IS health data. This is the sharpest point and the answer is unambiguous.**

> **ZZPL čl. 17 st. 1** — "Zabranjena je obrada kojom se otkriva rasno ili etničko poreklo, političko mišljenje, versko ili filozofsko uverenje ili članstvo u sindikatu, kao i obrada genetskih podataka, **biometrijskih podataka u cilju jedinstvene identifikacije lica, podataka o zdravstvenom stanju** …"
> **ZZPL čl. 17 st. 2 tač. 2** — "obrada je neophodna u cilju izvršenja obaveza ili primene zakonom propisanih ovlašćenja rukovaoca ili lica na koje se podaci odnose **u oblasti rada, socijalnog osiguranja i socijalne zaštite, ako je takva obrada propisana zakonom**"

A field whose value is *bolovanje* / *privremena sprečenost za rad* reveals that an identified employee was medically unfit on identified dates. **No diagnosis is needed for it to be health data — the fact of medical incapacity IS a podatak o zdravstvenom stanju.** The applicable exception is **čl. 17 st. 2 tač. 2**, and the prescribing law is **ZEOR čl. 24 tač. 1 g) i đ)**. Because the exception is drawn to the *obligation*, the lawful scope is **exactly the hours count and the two statutory payer categories** — not diagnosis, not the doznaka, not ICD codes, not free text. Do **not** use čl. 17 st. 2 tač. 1 (izričit pristanak): same imbalance problem, and tač. 1 is expressly disapplied where the law provides the processing is not consent-based.

**Three concrete consequences, and the first one bites hardest:**

**(a) The small-controller exemption collapses — on TWO independent limbs.**
> **ZZPL čl. 47 st. 9** — "Odredbe st. 1. i 4. ovog člana **ne primenjuju se** na privredne subjekte i organizacije u kojima je zaposleno manje od 250 lica, **osim ako**: … **2) obrada nije povremena; 3) obrada obuhvata posebne vrste podataka o ličnosti iz člana 17. stav 1.**"

A three-employee boutique hits both: daily working-time processing is not *povremena*, and the bolovanje category is a čl. 17 st. 1 class. `docs/compliance/evidencija-obrade-cl47.md` is therefore **legally mandatory, not hygiene** — and it currently invokes only limb 2. Penalty for not keeping it: preduzetnik **fixed 50.000** (čl. 95 st. 6), not the 100.000 the file prints.

**(b) The mandatory DPIA does NOT trigger.** čl. 54 st. 4 tač. 2 requires processing *"u velikom obimu"*; a boutique is not large scale. A general čl. 54 st. 1 risk assessment stays prudent, not compelled on this ground.

**(c) But the *prior-opinion* gate is a live, unresolved risk.**
> **ZZPL čl. 55 st. 10** — "Poverenik može da sastavi i javno objavi na svojoj internet prezentaciji listu vrsta radnji obrade **u vezi sa kojima se mora tražiti njegovo mišljenje**."

This is an **express standalone power**, independent of čl. 54 st. 5. So if a feature falls inside the Poverenik's *Odluka o listi vrsta radnji obrade* ("Sl. glasnik RS" 45/2019, reportedly amended 112/2020), the opinion duty attaches **on listing**, and the čl. 55 st. 4–5 clock (60 days, extensible by 45) is real — a 3.5-month gate a boutique will never clear. Its **tačka 7** is reported (two independent secondary reproductions, consistent wording) to cover employee data processed *"upotrebom aplikacija ili sistema za praćenje njihovog rada, kretanja, komunikacije i sl."* **No pass read the primary gazette text** — poverenik.rs migrated from `/sr-yu/` to `/sr/` and the article pages are gone since the site rebuild. → §6 **W-4**. **The gate is triggered by FEATURES, not by the base record:** a plain PIN/card punch kept to satisfy čl. 55 st. 6 reads as an *evidencija*; geolocation, idle detection, screenshots, keystroke counts or productivity scoring read as *praćenje*. **Design to stay on the evidencija side of that line** (§5).

**The čl. 23 notice must change, on four points, and one is a hard timing rule.**
- **čl. 23 st. 1 tač. 3** — add a working-time purpose row and, **separately**, a posebne-vrste row citing čl. 17 st. 2 tač. 2. The current two-row table says nothing about special categories.
- **čl. 23 st. 1 tač. 5** — name the recipients if hour classifications or absence hours reach the knjigovođa, RFZO or PIO. The template names only *"nadležni državni organi"* and Actaer.
- **čl. 23 st. 2 tač. 1** — split the retention statement into the trajno classes and the purged raw data (§4d). As written, the undifferentiated *"trajno"* in §5 of the template will be read to cover punch events, making the notice itself evidence of a čl. 5 st. 1 tač. 5 breach.
- **čl. 23 st. 3 — deliver BEFORE switching the module on.**
> **ZZPL čl. 23 st. 3** — "Ako rukovalac namerava da dalje obrađuje podatke o ličnosti u drugu svrhu koja je različita od one za koju su podaci prikupljeni, rukovalac je dužan da **pre započinjanja dalje obrade** … pruži informacije o toj drugoj svrsi"

The shop already collects shift open/close for **cash control**. Repurposing it as a **labour-law time record** is a new purpose. The template currently frames delivery as onboarding-only (*"Uručuje se zaposlenom pri zasnivanju radnog odnosa"*) — that is insufficient for existing employees.

*(Minor: čl. 23 st. 1 tač. 4 requires disclosure of the **existence** of a legitimate interest, not the balancing test. Immaterial here, since legitimni interes is not used.)*

**Penalties — ZZPL. All prekršaji; express preduzetnik tiers exist, so ZPP čl. 6 st. 1 does not arise.**

| Breach | Tačka | Pravno lice | **Preduzetnik** | Fizičko / odgovorno lice |
|---|---|---|---|---|
| Over-retention / minimisation breach (čl. 5 st. 1) | st. 1 **t. 1** | 50.000–2.000.000 (st. 1) | **20.000–500.000 (st. 4)** | 5.000–150.000 (st. 5) |
| Special categories contrary to čl. 17–18 (incl. biometric clock-in) | st. 1 **t. 6** | 50.000–2.000.000 | **20.000–500.000** | 5.000–150.000 |
| Failing the čl. 23 st. 1–3 notice | st. 1 **t. 8** | 50.000–2.000.000 | **20.000–500.000** | 5.000–150.000 |
| No privacy-by-design (čl. 42) | st. 1 **t. 20** | 50.000–2.000.000 | **20.000–500.000** | 5.000–150.000 |
| Processor appointment contrary to čl. 45 | st. 1 **t. 22** | 50.000–2.000.000 | **20.000–500.000** | 5.000–150.000 |
| No DPIA where čl. 54 requires it | st. 1 **t. 26** | 50.000–2.000.000 | **20.000–500.000** | 5.000–150.000 |
| No čl. 55 prior opinion | st. 1 **t. 27** | 50.000–2.000.000 | **20.000–500.000** | 5.000–150.000 |
| **No čl. 47 evidencija o radnjama obrade** | st. 2 **t. 5** | fixed 100.000 (st. 2) | **fixed 50.000 (st. 6)** | fixed 20.000 (st. 7) |

> **ZZPL čl. 95 st. 4** — "Za prekršaj iz stava 1. ovog člana kazniće se preduzetnik novčanom kaznom od 20.000 do 500.000 dinara."
> **ZZPL čl. 95 st. 6** — "Za prekršaj iz stava 2. ovog člana kazniće se preduzetnik novčanom kaznom u iznosu od 50.000 dinara."
> **ZZPL čl. 95 st. 1 tač. 8** — "licu na koje se podaci odnose ne pruži informacije iz člana 23. st. 1. do 3. i člana 24. st. 1. do 4. ovog zakona;"
> Sources: https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html · gazette text: https://mduls.gov.rs/wp-content/uploads/Zakon-o-za%C5%A1titi-podataka-o-li%C4%8Dnosti.pdf

**Note on čl. 95 st. 3 and st. 5:** neither reaches an ordinary cashier at a preduzetnik shop — st. 3 is confined to čl. 57 st. 7 (DPO) and čl. 76 (Poverenik staff), and st. 5 addresses a natural person acting as controller/processor or an *odgovorno lice u pravnom licu*. **Access control must be enforced by the software and the employment contract, not by deterrence.**

**Vendor exposure.** Because this feature adds čl. 17 health data, `docs/compliance/ugovor-o-obradi-nacrt.md` must be checked to confirm its čl. 45 st. 3 *"vrsta podataka o ličnosti"* clause **names posebne vrste podataka**, and that the čl. 45 st. 4 tač. 7 delete-or-return duty covers support artefacts. čl. 95 st. 1 opens *"rukovalac, odnosno obrađivač"* — Actaer is exposed in its own right at 50.000–2.000.000 as a pravno lice, and čl. 95 st. 1 tač. 23 separately punishes processing *"bez naloga ili suprotno nalogu rukovaoca (član 46)"*.

---

## 4. Encodable software requirements

### SW-14(a) — The record itself

1. **`[LEGAL]` → §3 W1, W4a.** One row per **(employee, calendar date)**. Not per register, not per shift. A POS register shift is not an employee's working time (čl. 64 st. 5 breaks count; two employees can share a till). Join register shifts as **corroborating evidence only**.
2. **`[LEGAL]` → §3 W4a.** Store **hours** as the primitive. Model the **fifteen ZEOR čl. 24 tač. 1 buckets** exactly as enumerated in §3 W4a, including the **statutory poslodavac / RFZO split** of sick-leave hours (g) vs đ)).
3. **`[LEGAL]` → §3 W4a.** Absence category is a **closed enum mirroring ZEOR čl. 24 tač. 1**. Enforce with a DB `CHECK` constraint or a Rust enum, not UI validation alone. **Zero free-text on an absence row.** A free-text column that exists will eventually be filled with a diagnosis.
4. **`[ACCURACY]` → §3 W4a.** Tag `nocni_casovi`, `casovi_rada_na_praznik`, clock times and break times in code and UI as **`izračunato radi provere usklađenosti`**, never as `zakonom propisano polje`. No Serbian provision requires a timestamp.
5. **`[LEGAL]` → §2.2.** Store `zanimanje` and `nivo i vrsta kvalifikacija` as **CODE + label** from the *Jedinstveni kodeks šifara* ("Sl. glasnik RS" 56/2018, 101/2020, 74/2021), never free text — ZEOR čl. 44 st. 2, penalised at čl. 50 st. 1 tač. 1.
6. **`[LEGAL]` → §3 W2, W4e.** Append-only with a superseding-row correction log carrying who / when / why; render the original struck through. **Never back-date** — "dnevnu" in čl. 55 st. 6 is what makes a reconstructed month look like a fabrication. ZEOR čl. 46 st. 1 puts accuracy responsibility on the record-keeper.

### SW-14(b) — Compliance checks over the record

7. **`[LEGAL]` → §3 W4b.** Validate at data entry: overtime **≤ 8 h** in any calendar week (čl. 53 st. 2) and total **≤ 12 h** in any day incl. overtime (čl. 53 st. 3). **This is the bigger fine** — breaching the caps is čl. 274 st. 1 tač. 3, preduzetnik **200.000–400.000** vs 50.000–150.000 for the missing register.
8. **`[LEGAL]` → §3 W4b. Add NO annual overtime counter.** No annual ceiling exists in ZoR. A yearly warning or block would be an invented rule.
9. **`[LEGAL]` → §3 W4b.** **Preraspodela is a hard branch, not a display toggle.** Under čl. 58 preraspodela hours are **not** overtime, so a naive `hours > 8 ⇒ prekovremeni` rule is legally wrong and would inflate the penalised čl. 55 st. 6 register. Model čl. 56 st. 4 (12 h/48 h monthly averaging) and čl. 57 (6-month average, 60 h/week) as **separate rule sets**, not variants of one cap.
10. **`[LEGAL]` → §3 W4b.** Gate the **9-month** reference period behind an explicit `kolektivni_ugovor_postoji` flag — čl. 57 st. 3 offers it only *kolektivnim ugovorom*; a preduzetnik on a pravilnik o radu is capped at 6 months within the calendar year. On mid-period termination, **surface** the čl. 61 choice; do not decide it.
11. **`[LEGAL]` → §3 W4b.** čl. 57 st. 4 overtime conversion applies **only** to an employee *"koji se saglasio"* — store the consent as a boolean + date.
12. **`[LEGAL]` → §3 W4b.** Age and status guards from an employee master record: under 18 → **reject** any overtime or preraspodela entry outright (čl. 88 st. 1), cap 35 h/week and 8 h/day (čl. 87), block night hours except the čl. 88 st. 2 exceptions. Parent of a child **up to 3** (čl. 91 st. 1) or **samohrani roditelj of a child up to SEVEN or a težak invalid** (čl. 91 st. 2) → require a stored written-consent flag before overtime or night hours can be saved. **The threshold is seven, not fourteen.**
13. **`[LEGAL]` → §3 W4b.** čl. 90 (pregnant/nursing) is a **stored flag set from a nalaz nadležnog zdravstvenog organa**, not an unconditional block. Store the boolean + date, **never the underlying medical evidence**.
14. **`[LEGAL-INFERRED]` → §3 W4b.** The čl. 62 st. 2 night threshold (≥ 3 h/day or ⅓ of the week) is an **advisory flag only** — the reassignment duty fires only on a zdravstveni organ's opinion. Do not render it as an automatic obligation.
15. **`[PRUDENTIAL]` → §3 W4b.** Rest-period checks (čl. 64, 66, 67) as warnings, with the preraspodela variants (11 h daily / 24 h weekly) selected by the same branch as requirement 9.
16. **`[ACCURACY]` → §3 W4b.** Holiday calendar: encode Zakon o državnim i drugim praznicima čl. 1/1a/2, and čl. 3a (state holiday on a Sunday → the next working day is non-working). **Exclusion set for the čl. 108 st. 1 tač. 1 (110 %) flag, which applies only to a *"praznik koji je neradni dan"*:** **Dan pobede 9 May** (čl. 3, *"koji se praznuje radno"*) **plus the čl. 5 days — Sveti Sava 27.01, Dan sećanja na žrtve holokausta 22.04, Vidovdan 28.06, and 21.10** — all observed as **working** days. Do not hard-code a "Julian calendar" rule for Vaskrs; the statute says only *"Vaskršnji praznici počev od Velikog petka zaključno sa drugim danom Vaskrsa"*, with čl. 4 tač. 2 giving other Christians their Uskrs *"prema njihovom kalendaru"*. Expose čl. 4's other-faith entitlements as configurable paid-absence days.

### SW-14(c) — Timing, retention, export

17. **`[PRUDENTIAL]`, designed as if `[LEGAL]` → §3 W4c.** Contemporaneous daily write; mark late or corrected entries rather than back-dating. **Do not build a "you must close today's overtime log" blocker** — no deadline is prescribed. The requirement is that the record **exist and be queryable**.
18. **`[LEGAL]` → §3 W4d.** **Two retention classes in the schema, and the purge job must know both.** Class A = the derived, period-closed hour classification per employee per month → retention `trajno`, structurally unreachable by every purge, reset, restore and backup-prune path. Class B = raw punch events, device/IP/geolocation metadata, draft rosters, support-session logs → bounded, purged once the period is closed and Class A is derived and archived. **Reuse the single shared retention table mandated by SW11-SW15 §3 requirement 42.**
19. **`[LEGAL]` → §3 W4d.** **Period-close is a real state transition, not a report:** derive and freeze Class A → mark Class B purge-eligible → irreversible without an audited unlock. Without it there is no defensible moment at which raw punches stop being necessary, and čl. 5 st. 1 tač. 5 has no anchor.
20. **`[LEGAL]` → §3 W4d.** Retention for the **standalone** overtime log where it is not folded into a wage record: floor **3 years** (`max(ZP čl. 84 absolute 2 y, ZoR čl. 196 3 y)`), upward-only. **Never surface a "2 years" or "6 years" figure in the UI.**
21. **`[LEGAL]` → §3 W3.** Export/print, **not an obrazac**: a plain per-employee monthly sheet plus CSV/XLSX for the knjigovođa, columns mapped 1:1 onto ZEOR čl. 24 tač. 1. **Must render offline, from the till, with no cloud** — ZIN čl. 20 st. 7 / čl. 21 tač. 4 require production *"u obliku u kojem ih nadzirani subjekat poseduje i čuva"*, and availability is the real constraint.
22. **`[ACCURACY]` → §2.** Never label any output a *"propisani obrazac"*, and never call the export a *"propisana evidencija o zaradama"*. Available copy: *"Evidencija prekovremenog rada — ZoR čl. 55 st. 6. Zakon ne propisuje obrazac."*
23. **`[LEGAL]` → §3 W4e, W5.** **No employee signature.** If an attestation is built, label it *"interna potvrda zaposlenog"*, optional. Ship a read-only per-employee **"moji sati"** view — it discharges ZoR čl. 83 st. 1 and ZZPL čl. 26 at once and is far cheaper than answering such a request by hand.

### SW-14(d) — Privacy surface

24. **`[LEGAL]` → §3 W5.** Basis is a **constant, never asked**: čl. 12 st. 1 tač. 3 for the statutory record classes, tač. 2 for contract-performance computation, **čl. 17 st. 2 tač. 2** for the absence-hour category. Surface these strings in the app's privacy screen **and** in the čl. 47 record generator so they cannot drift. **Build no consent UI anywhere in the employee surface.**
25. **`[LEGAL]` → §3 W5.** The absence **reason** is a special-category column with its own access gate (čl. 50, čl. 42). Owner/payroll role only. Every other role sees `odsutan` plus an hour total. Never on a counter-facing screen, a roster print, or a generic employee CSV export.
    - **Carve-out — the data subject's own row.** The gate governs **disclosure to anyone other than the data subject**. It does not run against the employee themselves: ZZPL čl. 26 is the right to be told what is recorded about oneself, and withholding the category on a surface that exists to discharge that right would defeat it. A surface may therefore render the category to a non-payroll role **only** when it resolves the employee from the server-side session and returns own rows only — no employee parameter, no picker, nothing an admin gate would protect. The sanctioned instance is `crate::commands::worktime::my_hours` (`require_session`, own rows only) rendered by `src/app/worktime/MyHoursPanel.tsx`. This is why the counter-facing-screen sentence above does not bar „Moji sati“ from the shop-counter PC: the viewer is the subject, and no colleague's category is reachable from it. Nowhere else is the gate widened — `WorkTimeModule`'s `canSeeAbsenceReason` stays role-driven, and any new surface taking a `userId` stays behind the gate.
    - **No employee-side export.** „Moji sati“ deliberately ships **no** export control. `worktime_export_csv` is `require_admin`-gated backend-side, so an employee-side button could only produce a refusal; the ZZPL čl. 26 duty to supply a copy falls on the **controller**, who produces it from the admin grid. Read the missing control as a decision, not as unshipped work.
26. **`[LEGAL]` → §3 W5.** **Regenerate and re-deliver the čl. 23 notice before the module goes live**, per čl. 23 st. 3. Gate: the working-time module refuses to activate for an employee until an acknowledgement dated on/after the feature's introduction is recorded. Add the posebne-vrste row, the named recipients, and the two-class retention statement.
27. **`[LEGAL]` → §3 W5.** Ship the **čl. 47 evidencija o radnjama obrade as a generated artefact**, driven off the app's configured purposes, recipients, retention-policy rows and čl. 50 measures — this feature is what destroys the <250 exemption on **both** čl. 47 st. 9 limbs. Wire it to `docs/compliance/evidencija-obrade-cl47.md`, and fix that file's three defects (§1 rows 16–17).
28. **`[LEGAL]` → §3 W5.** Remote support must **not** see the absence reason by default: mask the column and payroll screens unless the shop explicitly unmasks for that session, and **log the unmask** (feeds SW-10).
29. **`[ACCURACY]` → §3 W1, W2.** All penalty copy resolves from `pravra_forma` in `legal.rs` (SW-15). Preduzetnik → *"prekršaj, novčana kazna od 50.000 do 150.000 RSD (ZoR čl. 276 st. 1 u vezi sa tač. 1a)"*, **and suppress the čl. 276 st. 2 odgovorno-lice line entirely**. Pravno lice → 150.000–300.000 (čl. 276 st. 1) + odgovorno lice 10.000–20.000 (st. 2). **Never render any ZEOR figure** (§5 item 16). Add a CI guard mirroring SW-15's: the literal string `300.000` must not be reachable as the preduzetnik's čl. 276 exposure.
30. **`[ACCURACY]` → §3 W1.** Where the app names an inspector or a sanction: **inspektor rada** (ZoR čl. 268a tač. 1, čl. 268b, čl. 269 st. 3, čl. 270). Say *"ovi članovi ne propisuju zaštitnu meru"* — **never** *"zabrana obavljanja delatnosti nije moguća"* (ZoP čl. 55 st. 2, unresolved).
31. **`[LEGAL]` → §3 W1.** **No headcount gate and no plan-tier gate.** There is no small-employer exemption; the register must be available to a 1–3-employee boutique. Do not put it behind an "enterprise HR" upsell.

---

## 5. What must NOT be built

1. **F-12 — VantumPOS stays OUT of payroll. Standing rule, and the line runs *inside* ZEOR čl. 24.** **Tačka 1 (hours) is VantumPOS's side.** **Tačka 2 (bruto/neto zarada, porezi i doprinosi, dodaci, naknade, otpremnina, regres, jubilarne nagrade, lična primanja iz dobiti) and tačka 3 (beneficirani staž, stopa uvećanja) are the accountant's side** and must not be modelled, stored, imported or displayed. Crossing that line inherits three duties VantumPOS must not take on: the **trajno** retention of a wage record (ZEOR čl. 25 st. 3), the **ZoR čl. 122 st. 3 signature** by the authorised representative, and the **ZEOR čl. 25 st. 2 reporting duty to the PIO fund** (čl. 50 st. 1 tač. 7).
2. **Never produce an obračun zarade.** ZoR čl. 121 st. 6 makes it an **izvršna isprava** — a directly enforceable instrument. A POS app that emits one has created an enforceable debt document from unaudited data.
3. **Never compute the ZoR čl. 108 uplifts** (110 % praznik / 26 % noć / 26 % prekovremeni / 0,4 % minuli rad). Emit hour counts; let payroll apply percentages.
4. **No free-text absence note, no diagnosis field, no ICD code, no doznaka number, no file attachment.** This is the single worst thing that could be added: it converts a bounded, čl. 17 st. 2 tač. 2-authorised health data point into unbounded health data with no statutory cover, on a shop-counter PC the vendor can reach over AnyDesk.
5. **No biometric clock-in. Ever.** ZZPL čl. 17 st. 1 prohibits *"biometrijskih podataka u cilju jedinstvene identifikacije lica"*, no čl. 17 st. 2 exception rescues it for a boutique, and the Poverenik's published position is that biometrics purely for working-time control is disproportionate. If a customer asks, refuse in writing. **PIN or card.**
6. **Do not cross into a "sistem za praćenje rada":** no geolocation on punch, no idle/active detection, no screenshots, no keystroke or click counts, no per-employee productivity scoring, no cashier league tables, no automated lateness alerts. Any one plausibly triggers the Poverenik's Odluka tačka 7 → mandatory DPIA **plus** a prior opinion with a **60 + 45-day** clock (ZZPL čl. 55 st. 4–5, empowered by st. 10). Record this as an explicit product constraint so a roadmap item does not silently cross it.
7. **No promet attribution to a named cashier inside this feature.** Keep attribution at smena level with no employee join in reporting, or declare it as its **own purpose** in the čl. 23 notice and the čl. 47 record. Never let it drive an automated consequence (čl. 38).
8. **No consent UI for employee data**, and no consent audit table. See §3 W5.
9. **No e-Bolovanje integration.** The preduzetnik's registration deadline is **1 January 2027** (Zakon 109/2025 čl. 13) and the flow is HR/accounting, not POS. Model sick leave as **two hour buckets and nothing else**.
10. **No permanent retention of raw punch events, device/IP metadata, geolocation or draft rosters.** ZEOR's *trajno* attaches to the **derived, period-closed classification**, not the event stream. Keeping minute-level punches plus a bolovanje flag forever is a čl. 5 st. 1 tač. 3 and tač. 5 breach — preduzetnik 20.000–500.000 (čl. 95 st. 1 t. 1 → st. 4).
11. **No auto-purge of POS shift-open/close records on a ZZPL clock.** They are simultaneously cash-control artefacts and time evidence, and no statute classifies them. Deleting an inspector's evidence to satisfy čl. 5 is the worst possible trade (§6 W-8).
12. **No annual overtime counter** and **no `hours > 8 ⇒ prekovremeni` rule during preraspodela** — both are invented rules that would corrupt the penalised čl. 55 st. 6 register.
13. **No "Pravilnik 120/2014"** anywhere in code, docs, UI copy, marketing or compliance metadata. It does not exist.
14. **Never claim a "propisani obrazac"** for the overtime register, and **never claim to produce a "propisana evidencija o zaradama"** — the latter's methodology, entry rules and codes are prescribed and the surviving-obrazac question is open (§6 W-2).
15. **Never say "ZoR-compliant time tracking" while logging only overtime.** ZoR čl. 55 st. 6 does not discharge the ZEOR čl. 23–24 duty or the ZoR čl. 122 duty.
16. **Never put a ZEOR fine figure in front of a customer.** čl. 50 st. 1's 500.000–1.000.000 is the pravno-lice tier and does not reach the pilot; čl. 51's 300.000–500.000 names *"fizičko lice koje ima zaposlene"* and exceeds the ZoP čl. 39 st. 1 tač. 1 ceiling for that class. Say only that a separate statute imposes a broader ordinary-hours dataset and permanent retention, and that the figure is unsettled (§6 W-1).
17. **Never surface "2 years" or "6 years" retention.** The first is a Serbian blog error; the second is Croatian law.
18. **Never threaten zabrana obavljanja delatnosti for a missing overtime register.** ZoR prescribes no zaštitna mera in čl. 273–276a. Per standing rule, say only that these articles prescribe none.
19. **No headcount gate, no "enterprise HR" upsell** on the working-time register.
20. **Method rule: never treat ZoR čl. 277 as a delegation index.** It is a 2005 transitional savings clause, disproven by čl. 121 st. 8 and čl. 140. Test for delegation by reading the operative article's own text.
21. **Regional portability:** if VantumPOS is ever sold into FBiH, Croatia or North Macedonia, this module is **not portable as-is** — those jurisdictions do prescribe record content and forms (FBiH Pravilnik, Sl. novine FBiH 92/16; HR Pravilnik, NN 55/2024; MK Pravilnik, Sl. vesnik 78/2015, with mandatory *electronic* records above 25 employees). Design a pluggable jurisdiction profile rather than assuming the Serbian free-form rule.

---

## 6. Residual open questions

| # | Question to put, and to whom | Why unresolved | Risk of proceeding |
|---|---|---|---|
| **W-1** | **To a lawyer, against the Službeni glasnik text:** *"Da li se član 51. Zakona o evidencijama u oblasti rada ('fizičko lice koje ima zaposlene', 300.000–500.000) primenjuje na preduzetnika, i da li taj iznos opstaje naspram člana 39. stav 1. tačka 1) Zakona o prekršajima (fizičko lice: 5.000–150.000)?"* | ZEOR never uses the word *preduzetnik*; 300.000–500.000 sits above the ZoP fizičko-lice ceiling and inside the preduzetnik band, so only the preduzetnik reading makes the amount valid — but that is construction. Provenance is resolved (101/2005 čl. 81); ZEOR was never harmonised with ZoP 65/2013 despite its transitional command. The consolidated text asterisks čl. 50–52 with no rendered footnote | **HIGH.** It is the largest number in this area and register row 15 currently prints an even bigger wrong one. **Ship no number until answered**; correct row 15 to the pravno-lice/preduzetnik split with the preduzetnik figure marked unsettled |
| **W-2** | **To a labour lawyer:** *"Da li za evidenciju o zaposlenim licima i evidenciju o zaradama postoji propisani obrazac koji je i dalje na snazi — po tač. 3–9 Odluke o jedinstvenim metodološkim principima (Sl. list SRJ 40/97), i kakav je status obrasca MIN i obrazaca E-3/E-3/1 posle CROSO reforme?"* | The negative rests solely on tač. 10's surviving list not naming one. **Odluka tač. 3–9 was never read in full by any pass**, and tač. 12 proves the Odluka prescribes forms outside tač. 10 (obrazac MIN — an employer-kept record). E-3 and E-3/1 expressly survived the 27.03.2010 partial cessation | **HIGH.** Decides whether SW-14's export may be called a statutory record at all, and whether a layout constraint exists downstream of the overtime table |
| **W-3** | **To Ministarstvo za rad (mišljenje) or the Inspektorat za rad:** *"Da li se zahtev iz člana 55. stav 6. Zakona o radu ('dnevna evidencija') ispunjava evidencijom koja se rekonstruiše na kraju meseca iz podataka o smenama, ili se traži unos istog dana?"* | *"Dnevnu"* fixes granularity; **no provision anywhere states a deadline for the entry.** No ministry mišljenje or inspectorate guidance found in primary sources. Serbia has no analogue of the Croatian 7-day rule | **MEDIUM.** Design as if daily entry is required; the cost of the safe assumption is near zero |
| **W-4** | **To the Poverenik (issues opinions on request), with the gazette text in hand:** *"Da li evidencija radnog vremena zasnovana isključivo na PIN/kartica prijavi predstavlja 'sistem za praćenje rada' iz tačke 7. Odluke o listi vrsta radnji obrade, i da li obaveza traženja mišljenja nastaje samim uvrštenjem na listu (čl. 55 st. 10 ZZPL)?"* | **No pass read the primary text of the Odluka ("Sl. glasnik RS" 45/2019, reportedly 112/2020).** poverenik.rs migrated `/sr-yu/` → `/sr/` and the article pages are gone since the site rebuild; tačka 7's wording rests on two consistent secondary reproductions, and the 112/2020 amendment's effect on it is unverified. ZZPL čl. 55 st. 10 *does* give a standalone listing power, so the gate is real if the feature is caught | **HIGH.** The difference between shipping and a **60 + 45-day** approval gate a boutique will never clear. Mitigated to near zero by staying strictly on the evidencija side (§5 item 6) |
| **W-5** | **To the shop's knjigovođa + a lawyer:** *"Da li je obaveza dostavljanja izveštaja iz evidencije o zaradama organizaciji za PIO (ZEOR čl. 25 st. 2, kažnjivo po čl. 50 st. 1 tač. 7) i dalje živa posle ukidanja obrasca M-4?"* | The abolition (M-4/M-UN from 2019, PIO now sourcing from CROSO/PPP-PD) rests on secondary reporting only — **no pass read the ZPIO amending article verbatim.** The ZEOR text is verbatim alive and penalised | **LOW for software** (out of scope under F-12), **medium for the shop.** Do not issue a "do not build" directive against a live, penalised duty on secondary evidence |
| **W-6** | **To a labour lawyer:** *"Da li se podaci o časovima iz člana 24. tač. 1) ZEOR moraju voditi po danu, ili je dovoljna agregacija po obračunskom periodu?"* | ZEOR prescribes no periodicity for the hour fields; ZoR čl. 122 st. 1 makes the wage record monthly; only čl. 55 st. 6 says *"dnevnu"*, and only about overtime. A literal reading permits monthly aggregation of everything except overtime | **LOW.** Per-day for all buckets is strictly safer and costs nothing — **but the doc must not claim the law compels it for non-overtime categories** |
| **W-7** | **To a labour lawyer:** *"Kako se primenjuje obaveza iz člana 62. stav 3. Zakona o radu (zatražiti mišljenje sindikata pre uvođenja noćnog rada) kod poslodavca kod koga sindikat ne postoji?"* | The text has no carve-out. Presumably moot where no sindikat exists, but no primary text or authentic interpretation says so | **LOW.** Relevant only if the boutique introduces night shifts; normal boutique hours do not reach 22:00 |
| **W-8** | **To the shop's accountant + a lawyer:** retention classification of **POS shift / cash-movement records** — statutory labour record (trajno)? pomoćna knjiga (5 y)? purely operational (ZZPL purge)? | Statutes do not classify POS-generated records. Carried unresolved from `SERBIAN-LAW-COMPLIANCE.md` §6 item 7 and SW11-SW15 §5 Q-15 | **MEDIUM, and now more urgent** — this feature gives the purge job a reason to touch those rows. Exclude them from automatic deletion until classified |
| **W-9** | **To a labour lawyer:** *"Da li ZoR čl. 103 (rok od tri dana za dostavljanje potvrde) i dalje obavezuje zaposlene kod preduzetnika koji još nije registrovan na 'e-Bolovanje – Poslodavac' u periodu 01.01.2026 – 01.01.2027?"* | Zakon 109/2025 čl. 15 applies the law from 01.01.2026 generally (čl. 4 st. 1 tač. 2)–3) from 01.04.2026); čl. 13 gives a preduzetnik until 01.01.2027; čl. 14 disapplies ZoR čl. 103 *"danom početka primene"*. Not resolvable on the face of the text. **The pilot is in that window right now** | **Nil for VantumPOS** if the app stays out of sick-leave documents — which is the recommendation. Shop-side question only |
| **W-10** | **To the pilot shop's accountant / local Novi Pazar practice:** does an inspektor rada accept a purely electronic register produced on screen, or expect a printed extract? | The legal answer is clear (ZoR silent on medium; ZEOR čl. 45 st. 1 permits automatic processing; ZIN requires production *"u obliku u kojem ih … poseduje i čuva"*). **Inspection practice is not settled in any text read** | **LOW-MEDIUM.** Mitigated by requirement 21 (offline print/PDF on demand) |
| **W-11** | **Forensic only:** where did *"Pravilnik 120/2014"* come from? | Ruled out for Croatia (NN 32/15 → 73/17 → 55/2024), FBiH (92/16) and North Macedonia (78/2015). "Sl. glasnik RS" 120/2014 could not be enumerated — Paragraf's gazette archive starts at 2015, propisi.com 404s, pravno-informacioni-sistem.rs is a JS SPA. Digit corruption of **90/2014** is the likeliest explanation | **Nil.** Does not affect the operative answer. Record the negative so it is not re-litigated |
| **W-12** | **Monitoring, not a question:** the announced **new Zakon o radu** (ministry statements late July 2026, EU-harmonisation driven) | Not adopted as at 01.08.2026; no draft located in primary form. čl. 55 st. 6 and čl. 276 tač. 1a untouched since 113/2017 and carry no amendment footnote | **Document rot.** Re-check before any release hard-coding čl. 55 st. 6 or the čl. 276 tiers |
| **W-13** | **Design question to settle before SW-14 is built:** can one data model satisfy **ZoR čl. 55 st. 6** (daily, overtime), **ZoR čl. 122** (monthly wage record, employer-signed) and **ZEOR čl. 23–24** (classified period totals, prescribed coding) together? | Likely yes — the natural pipeline is *daily punch + overtime log → monthly ZoR čl. 122 evidencija → ZEOR čl. 24 aggregation → permanent store* — but the čl. 122 leg belongs to the accountant under F-12, so the boundary must be drawn explicitly | **MEDIUM.** Getting it wrong either duplicates the accountant's record or leaves a gap the shop believes is covered |
| **W-14** | **Fact question to the shop, not to a lawyer:** does the shop have a **kolektivni ugovor** or only a **pravilnik o radu**? | Gates the čl. 57 st. 3 nine-month preraspodela (available *only* kolektivnim ugovorom) and offers a second route under ZZPL čl. 17 st. 2 tač. 2. Never established | **MEDIUM if unasked.** A preraspodela feature built without the flag would offer a boutique a reference period it cannot lawfully use |
| **W-15** | **To the lawyer reviewing F-1:** does `docs/compliance/ugovor-o-obradi-nacrt.md` name **posebne vrste podataka** in its ZZPL čl. 45 st. 3 *"vrsta podataka o ličnosti"* clause, and does the čl. 45 st. 4 tač. 7 delete-or-return duty cover support artefacts? | Not checked in this pass. This feature is what first puts čl. 17 health data on the machine Actaer reaches over AnyDesk — which also sharpens `SERBIAN-LAW-COMPLIANCE.md` §6 item 5 (AnyDesk = prenos u drugu državu?) | **MEDIUM.** čl. 95 st. 1 tač. 22 reaches the shop; tač. 23 and st. 1's *"rukovalac, odnosno obrađivač"* reach Actaer at 50.000–2.000.000 |

---

**Source-hygiene note for the next verification pass.** ZoR, ZEOR, ZIN, ZZPL, ZoP, the Odluka o jedinstvenim metodološkim principima, the Pravilnik o sadržaju obračuna zarade and the BZR pravilnik were read from consolidated commercial texts (paragraf.rs, cross-checked on profisistem.com, ius.bg.ac.rs, cekos.rs, propisi.net). **Two provisions were additionally verified against the enacting instrument itself** — ZoR čl. 55 st. 6 and the čl. 276 tiers against Zakon o izmenama i dopunama ZoR, "Sl. glasnik RS" 113/2017 čl. 2 i čl. 7; ZEOR's fine amounts against Zakon o izmenama zakona kojima su određene novčane kazne…, "Sl. glasnik RS" 101/2005 čl. 81. ZZPL was read twice, including a local extraction of the gazette reproduction on mduls.gov.rs. **Three primary texts remain unread by anyone and are the substance of W-1, W-2 and W-4:** the Službeni glasnik footnotes behind the asterisks on ZEOR čl. 50–52; Odluka (Sl. list SRJ 40/97) tač. 3–9; and the Poverenik's *Odluka o listi vrsta radnji obrade* ("Sl. glasnik RS" 45/2019, 112/2020). `pravno-informacioni-sistem.rs` renders via JavaScript and returns page shells; `poverenik.rs` now serves `/sr/` not `/sr-yu/` and its article pages are gone since the site rebuild — **a 404 there is a stale path, not evidence of unavailability.** Do one manual gazette pass over the ZEOR čl. 51 tier before any figure derived from it is rendered to a shop owner.