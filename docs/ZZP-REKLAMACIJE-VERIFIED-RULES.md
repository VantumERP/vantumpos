# ZZP Reklamacije — Verified Rule Set for SW-7

Scope: encodable deadline-engine + register rules for the Novi Pazar pilot (a **preduzetnik** selling goods to consumers — squarely in scope; `trgovac` is defined to include preduzetnik, ZZP 88/2021 & 35/2026 čl. 5 tač. 2). Every load-bearing rule below is pinned to primary text at stav level and was adversarially verified against the official gazette PDFs / adopted texts. Today = 2026-07-19, so the **operative regime right now is 88/2021 čl. 55**.

---

## 1. Which law governs, and the cutover

**The report's core identity claim was CORRECT; the context's suspicion was wrong.** The new Zakon o zaštiti potrošača **IS** published in **"Službeni glasnik RS", br. 35/2026** (gazette dated **23.04.2026**). That single gazette issue carries *both* the new ZZP *and* the Zakon o izmenama i dopunama Zakona o trgovini — which is what caused the confusion. Reklamacije in the new law = **čl. 63** (correct). Masthead verbatim: `ZAKON O ZAŠTITI POTROŠAČA ("Sl. glasnik RS", br. 35/2026)` — https://www.paragraf.rs/propisi/zakon-o-zastiti-potrosaca-2026.html ; gazette ToC (both laws in issue 35/2026): https://www.paragraf.rs/glasila/rs/sluzbeni-glasnik-republike-srbije-35-2026.html ; adopted text: https://www.parlament.gov.rs/upload/archive/files/lat/pdf/zakoni/14_saziv/1317-26-lat..pdf

**Entry into force (stupanje na snagu): 1 May 2026 — solid.** Čl. 220 verbatim: *"Ovaj zakon stupa na snagu osmog dana od dana objavljivanja u 'Službenom glasniku Republike Srbije', a primenjuje se po isteku tri meseca od dana stupanja na snagu ovog zakona, osim člana 4. stav 1. i člana 6, koji počinju da se primenjuju stupanjem na snagu ovog zakona."* Published 23.04.2026 → 8th day = 01.05.2026.

**Application date (početak primene): 1 or 2 August 2026 — GENUINELY UNRESOLVED. The report's "2 Aug 2026" is not proven; "1 Aug" is equally sourced.** The law states only a formula ("po isteku tri meseca od dana stupanja na snagu"), no calendar date. The split:
- **1 Aug 2026** — IPC (professional source) computes it explicitly "not August 2": https://www.ipc.rs/vest/novi-zakon-o-zastiti-potrosaca-odredbe-koje-se-primenjuju-od-1-maja-2026-godine_v2452
- **2 Aug 2026** — calibration against the identically-worded 88/2021 precedent: 88/2021 published 11.09.2021, in force 19.09.2021, application documented verbatim as *"u primeni od 20. decembra 2021"* (= 3-month anniversary 19.12 **+1 day**) — https://www.paragraf.rs/dnevne-vesti/071221/071221-vest3.html . Same method on the new law: 01.05.2026 +3mo = 01.08.2026 → application 02.08.2026.

**Repeal:** čl. 219 — *"Danom početka primene ovog zakona prestaje da važi Zakon o zaštiti potrošača ('Službeni glasnik RS', broj 88/21)."* Old law dies **on the application date**, not on entry into force.

### Regime-selection rule the software uses
Key the regime off the **complaint's FILING date** (`datum prijema reklamacije` / potvrda-o-prijemu date) — **NOT** the purchase/delivery date, **NOT** the resolution date. Basis: čl. 215 — *"Postupci koji nisu okončani do dana početka primene ovog zakona, okončaće se po odredbama propisa po kojima su započeti."* A complaint filed before the cutover keeps the OLD clock for its entire life even if resolved in September; one filed on/after the cutover uses the NEW clock. **Persist the chosen regime on the record at creation** so a later config change can't retroactively flip an in-flight complaint.

- **`filed_at < CUTOVER_DATE`** → OLD regime (88/2021 čl. 55, restart-from-zero).
- **`filed_at >= CUTOVER_DATE`** → NEW regime (35/2026 čl. 63, suspend/resume + express warning).

**SAFE default for CUTOVER_DATE = `2026-08-01`** (the *earlier* candidate), configurable, flagged for counsel. Rationale (the false-assurance test): the NEW suspend clock is **never more generous** than the OLD restart clock, and NEW adds duties (express warning, no-fee). Applying NEW too early only ever gives the shop a *tighter* deadline / *extra* compliance — safe. Applying OLD too late would wrongly grant an **unearned full restart** → the shop thinks it has a fresh 15/30 days → blows the real suspend deadline → consumer harmed + shop fined. So at the ambiguous 1–2 Aug cusp, resolve toward NEW. Do **not** silently hard-code 2 Aug.

---

## 2. The deadline engine — exact, encodable

### 2.1 Answer deadline (identical in both regimes)
`answer_due = receipt_date + 8 days`. Verbatim, **88/2021 čl. 55 st. 9** / **35/2026 čl. 63 st. 9** (same wording, `prodavac`→`trgovac`): *"...bez odlaganja, a najkasnije u roku od osam dana od dana prijema reklamacije, pisanim ili elektronskim putem odgovori potrošaču..."* Anchor = **prijem** (receipt). Days = **calendar** (text says "dana", no "radnih dana"). This is a **hard, non-extendable, non-suspendable** deadline — the st. 10 suspend/restart mechanics operate on the *resolution* clock only, and only kick in after the answer is already given. Required answer content (all four; a reply missing the concrete-proposal element is legally defective): accept/reject decision; reasons if rejected; statement on the consumer's requested remedy; concrete proposal of *how and by when* if accepted.

### 2.2 Resolution deadline (identical anchor & lengths in both regimes)
`resolution_due = submission_date + (15 or 30) days`. Verbatim (both laws, čl. 55 st. 9 / čl. 63 st. 9): *"Rok za rešavanje reklamacije ne može da bude duži od 15 dana, odnosno 30 dana za tehničku robu i nameštaj, od dana podnošenja reklamacije."* Anchor = **podnošenje** (submission). Note the **two anchors differ in the text** (answer from *prijem*, resolution from *podnošenje*) — in practice the same timestamp at filing; store one `filed_at` and document it, but keep the fields distinct.

**Who gets 30 days:** only **`tehnička roba`** and **`nameštaj`**.
- `tehnička roba` is **legally defined**, 88/2021 čl. 5 t. 41 (verbatim): *"...сложена ствар, односно уређај индустријске производње трајније употребе (апарати за домаћинство, компјутери, телефони, моторна возила и сл.) за чији је рад неопходна електрична енергија, друго средство напајања (нпр. батерија или акумулатор) или мотор на унутрашње сагоревање"* — i.e. power-driven industrial devices. Encode against this definition.
- `nameštaj` (furniture) is **NOT defined anywhere in either law**. Do **not** hard-code a narrow list. Surface a user-selectable "nameštaj" flag with a tooltip that classification is the shop's legal call.

### 2.3 Extension — one only, with consent (identical in both regimes)
88/2021 čl. 55 st. 11 / 35/2026 čl. 63 st. 11 (verbatim, `prodavac`→`trgovac`): *"Ukoliko trgovac iz objektivnih razloga nije u mogućnosti da udovolji zahtevu potrošača u propisanom roku, dužan je da o produžavanju roka za rešavanje reklamacije obavesti potrošača i navede rok u kome će je rešiti, kao i da dobije njegovu saglasnost, što je u obavezi da evidentira u evidenciji primljenih reklamacija. Produžavanje roka za rešavanje reklamacija moguće je samo jednom."* → Allow **at most ONE** extension. Valid only if all three captured: (1) stated new deadline, (2) recorded consumer consent, (3) register entry. On extension, `resolution_due` becomes the **consumer-consented new date** (not an automatic +N days). Block a second extension.

### 2.4 Clock mechanics — the one place the regimes DIFFER
Both regimes: after the trader answers (st. 9), the consumer must respond **within 3 days** of receiving the answer; **silence = deemed NOT to agree** (`smatraće se da nije saglasan`). The trader may perform its proposed resolution only with the consumer's prior consent. **The 3-day window and silence=disagreement already exist in the CURRENT 88/2021 law — they are NOT new.** What changed is the interruption model and the warning duty.

**OLD (88/2021 čl. 55 st. 10) — RESTART FROM ZERO, no warning duty.** Verbatim: *"Rok za rešavanje reklamacije **prekida se** kada potrošač primi odgovor prodavca iz stava 9. ovog člana i **počinje da teče iznova** kada prodavac primi izjašnjenje potrošača. Potrošač je dužan da se izjasni ... najkasnije u roku od tri dana od dana prijema odgovora prodavca. Ukoliko se potrošač u propisanom roku ne izjasni, smatraće se da nije saglasan..."* → elapsed days **discarded**; a **fresh full 15/30-day period** starts when the trader receives the consumer's response. No express-warning obligation.

**NEW (35/2026 čl. 63 st. 10) — SUSPEND/RESUME + mandatory express warning.** Verbatim: *"Rok za rešavanje reklamacije **zastaje** kada potrošač primi odgovor trgovca iz stava 9. ovog člana i **nastavlja da teče** kada trgovac primi izjašnjenje potrošača. Potrošač je dužan da se izjasni ... najkasnije u roku od tri dana ... **Trgovac je dužan da u odgovoru na reklamaciju izričito obavesti potrošača o obavezi izjašnjenja, posledicama propuštanja tog roka i o zastoju rokova.** Ukoliko se potrošač u propisanom roku ne izjasni, smatraće se da nije saglasan..."* → elapsed days **preserved**; clock pauses on "consumer received answer" and resumes on "trader received consumer response". `effective_resolution_due = original_resolution_due + (consumer_reply_received_at − answer_received_at)`.

These are **two separate code paths keyed on the persisted regime.** Never run restart for a new-regime complaint (over-counts) or suspend for an old-regime one (under-counts).

### 2.5 Worked examples

**OLD regime, general goods (15 days), restart:**
| Event | Date |
|---|---|
| Complaint received = submitted (`filed_at`) | Mon **2026-06-01** |
| Answer due (`+8`) | **2026-06-09** |
| Provisional resolution due (`+15`) | **2026-06-16** |
| Trader answers (proposes repair, needs consent) | 2026-06-05 |
| Consumer *receives* answer → clock **interrupts** | 2026-06-06 |
| 3-day consumer window ends | 2026-06-09 |
| **Path A:** trader *receives* consumer response 2026-06-08 → clock **restarts from zero**, fresh 15 days | **new resolution due = 2026-06-23** |
| **Path B:** consumer silent past 2026-06-09 → status `nije saglasan`; no restart event; complaint at impasse (flag, do not silently extend) | — |

**NEW regime, tehnička roba (30 days), suspend:**
| Event | Date |
|---|---|
| Complaint received = submitted (`filed_at`) | Sat **2026-08-15** |
| Answer due (`+8`) | **2026-08-23** |
| Provisional resolution due (`+30`) | **2026-09-14** |
| Trader answers (proposes replacement; **includes 3-element express warning**) | 2026-08-20 |
| Consumer *receives* answer → clock **suspends** (6 days elapsed, 24 remain) | 2026-08-21 |
| 3-day consumer window ends | 2026-08-24 |
| **Path A:** trader *receives* consumer response 2026-08-25 → clock **resumes**, +4 days suspension | **new resolution due = 2026-09-18** (`= 09-14 + (08-25 − 08-21)`) |
| **Path B:** consumer silent past 2026-08-24 → status `nije saglasan`; no resume event; impasse (flag) | — |

### 2.6 Ambiguities → SAFE defaults (safe = the reading that gives the shop LESS time; over-promising a deadline is the harm)

1. **Day counting.** Text says "dana" (calendar). Serbian procedural convention (ZUP čl. 91) counts from the day *after* the anchor event and rolls a period ending on a non-working day to the next working day — **but it is UNCONFIRMED that ZUP governs these consumer-law deadlines.** *Safe default:* count **calendar days**, and **never apply a weekend/holiday rollover that extends the deadline** (rollover gives the shop more time). Treat the computed date as a hard ceiling and fire the shop's reminder **≥1 day early**. Do not encode any rollover until ZUP applicability is confirmed.
2. **Does the 3-day window suspend the answer clock or the resolution clock?** The text is explicit — *"Rok za **rešavanje** reklamacije zastaje/prekida se"* — it is the **resolution** clock. The 8-day answer clock is never suspended. *Safe default:* keep the answer deadline fixed; only the resolution clock pauses/restarts.
3. **When does the pause/restart actually trigger?** On "consumer received the answer" and "trader received the consumer's response" — events the shop must *prove*. *Safe default:* do **not** grant the pause/restart until a solid, dated event is recorded; absent it, keep the resolution clock running (tightest due date). Recompute when the event is logged.
4. **`nameštaj` classification.** Undefined. *Safe default:* a good is **15-day unless the shop affirmatively flags it** as tehnička roba or nameštaj. Over-flagging (calling a plain good 30-day) grants unearned time → harm; under-flagging is safe.

---

## 3. The register — schema-driving fields

**One schema serves both regimes** — the evidencija content, retention, potvrda, and PII rules are word-for-word identical (only `prodavac`→`trgovac`). Branch the *deadline engine* by regime, not the register.

**Enumerated fields — 88/2021 čl. 55 st. 8 / 35/2026 čl. 63 st. 8** (verbatim): *"Evidencija o primljenim reklamacijama vodi se u obliku ukoričene knjige ili u elektronskom obliku i **sadrži naročito** ime i prezime podnosioca i datum prijema reklamacije, podatke o robi, kratkom opisu nesaobraznosti i zahtevu iz reklamacije, datumu izdavanja potvrde o prijemu reklamacije, odluci o odgovoru potrošaču, datumu dostavljanja te odluke, ugovorenom primerenom roku za rešavanje na koji se saglasio potrošač, načinu i datumu rešavanja reklamacije, kao i informacije o produžavanju roka za rešavanje reklamacije."*

Mandatory columns (`naročito` = non-exhaustive **minimum floor** — you may add more, e.g. register number, regime tag; you may **never omit** any):

| # | Field | Type |
|---|---|---|
| 1 | `podnosilac_ime_prezime` | text |
| 2 | `datum_prijema` | date |
| 3 | `podaci_o_robi` | text/ref |
| 4 | `opis_nesaobraznosti` | text |
| 5 | `zahtev` (consumer's request) | text |
| 6 | `datum_izdavanja_potvrde` | date |
| 7 | `odluka_odgovor` (decision in reply) | text |
| 8 | `datum_dostavljanja_odluke` | date |
| 9 | `ugovoreni_rok_resavanja` + `consumer_consent` flag/timestamp | date + bool |
| 10 | `nacin_resavanja` + `datum_resavanja` | text + date |
| 11 | `produzenje_roka_info` (new deadline + consent + objective reason) | structured |

Model fields 9 and 11 as **structured, auditable events**, not free text — the deadline engine depends on them.

**No prescribed obrazac (verified negative).** The law fixes only the medium ("ukoričene knjige ili u elektronskom obliku" — electronic is expressly lawful) and the minimum content. Čl. 63 delegates **no** form-making power; čl. 216 gives a 1-year window for bylaws under *other* delegations, none for reklamacija. Publicly sold "Obrazac br. 3", "Pravilnik o rešavanju reklamacija", stationery "knjiga evidencije reklamacija" are **private templates, not state forms.** VantumPOS is free to design its own layout.

**Potvrda o prijemu** — 88/2021 čl. 55 st. 7 / 35/2026 čl. 63 st. 7 (verbatim): *"Trgovac je dužan da potrošaču **bez odlaganja** izda pisanu potvrdu ili elektronskim putem potvrdi prijem reklamacije, odnosno saopšti **broj pod kojim je zavedena** njegova reklamacija u evidenciji primljenih reklamacija."* → On intake, **synchronously** allocate a sequential register number (atomic at insert) and render a printable/electronic potvrda carrying it. **`bez odlaganja` is NOT a timer — do NOT compute a due-date for the potvrda.** Ties into the SW-8 print stack.

**Retention** — čl. 55 st. 6 / čl. 63 st. 6 (verbatim): *"...da je čuva **najmanje dve godine od dana podnošenja reklamacija potrošača.**"* Trigger = **submission (`filed_at`)**, not resolution, not year-end. `najmanje` = **floor, not cap** → do not auto-purge before `filed_at + 2y`; **do not surface a "must delete by" date** (indefinite retention is compliant). Implement as a purge-*eligibility* flag.

**PII / ZZPL** — second sentence of čl. 55 st. 6 / čl. 63 st. 6 (present in **both** laws, not a new-law addition): *"Prilikom obrade podataka o ličnosti potrošača, trgovac postupa u skladu sa propisima kojima se uređuje zaštita podataka o ličnosti."* The register is PII (name + goods + purchase history). Lawful basis under ZZPL (Sl. glasnik RS 87/2018) čl. 12 st. 1 tač. 3 = **legal obligation** (not consent — so the consumer cannot revoke to force deletion before the ZZP floor). Apply storage-limitation (ZZPL čl. 5 st. 1 tač. 5) after the 2-year floor unless an open dispute/accounting hold applies. Role-gate and access-log the register. ZZPL text: https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html

---

## 4. Mandatory texts + notices

**(a) Express 3-part warning — NEW regime only (35/2026 čl. 63 st. 10).** The law prescribes **content, not exact wording.** The trader's answer MUST `izričito obavesti potrošača` of all three: (1) **obaveza izjašnjenja** — the consumer's duty to respond; (2) **posledice propuštanja tog roka** — that missing the 3-day deadline = deemed disagreement; (3) **zastoj rokova** — that the resolution deadline is suspended. **Gate answer submission on all three template fields being present** for new-regime complaints. A compliant Serbian template (recommend legal review before go-live):

> *"Dužni ste da se na ovaj odgovor izjasnite najkasnije u roku od 3 (tri) dana od dana njegovog prijema. Ako se u tom roku ne izjasnite, smatraće se da niste saglasni sa našim predlogom. Rok za rešavanje reklamacije zastaje danom Vašeg prijema ovog odgovora i nastavlja da teče danom kada primimo Vaše izjašnjenje."*

Do **NOT** require this warning for OLD-regime complaints — 88/2021 čl. 55 st. 10 has no such duty; failing an old-regime answer for lacking it would be wrong.

**(b) Display obligation at the prodajno mesto.** 88/2021 čl. 55 st. 4: *"Prodavac je dužan da na prodajnom mestu vidno istakne obaveštenje o načinu i mestu prijema reklamacija, kao i da obezbedi prisustvo lica ovlašćenog za prijem reklamacija u toku radnog vremena."* NEW 35/2026 čl. 63 st. 4 **extends this to the website in distance selling**: *"Trgovac je dužan da na prodajnom mestu **i internet stranici (u slučaju daljinske trgovine)** vidno istakne obaveštenje..."* → VantumPOS can generate a printable "Obaveštenje o načinu i mestu prijema reklamacija" (SW-8 print stack).

**(c) No-fee for utvrđivanje nesaobraznosti — NEW regime only (35/2026 čl. 63 st. 3).** Verbatim: *"Trgovac je dužan da primi izjavljenu reklamaciju. **Zabranjeno je da trgovac naplaćuje utvrđivanje nesaobraznosti.**"* The old 88/2021 čl. 55 st. 3 has **only** the first sentence — **no fee ban.** → Block/warn on any inspection charge for new-regime complaints; do **not** present this as binding for pre-cutover complaints. (Separately, in both regimes the *remedy* — repair/replacement — is free: 88/2021 čl. 51 st. 1 / 35/2026 čl. 56 st. 1; that is a different concept.)

**(d) Other supporting duties (88/2021 čl. 55 st. 12–14, mirrored in čl. 63):** on rejection, inform consumer of vansudsko dispute resolution + competent bodies (st. 12); inability to return packaging **cannot** gate resolution (st. 13) — never require packaging; an orally-lodged complaint resolved on the spot per the consumer's request skips the potvrda/answer duties (st. 14) — provide that path.

---

## 5. Penalties + presumption

**Reklamacija breaches sit in the LOWER, fixed-amount tier — NOT the 300.000–2.000.000 din range (čl. 187 old / čl. 209 new). No zaštitna mera (business ban / published judgment) attaches to reklamacija breaches** (those attach only to čl. 187 tač. 2–6, 60 / čl. 209 tač. 1, 40).

| | OLD — 88/2021 čl. 188 (breach of čl. 55 st. 3,4,6,7,8,9,10,11,12) | NEW — 35/2026 čl. 210 st. 1 tač. 24 (breach of čl. 63 st. 3,4,6,7,8,9,10,11,12) |
|---|---|---|
| Pravno lice | **50.000 din** (st. 1) | **200.000 din** (st. 1) |
| Odgovorno lice | **8.000 din** (st. 2) | **50.000 din** (st. 2) |
| **Preduzetnik (the pilot)** | **30.000 din** (st. 3) | **100.000 din** (st. 3) |

Amounts rose ~4× at the cutover → **penalty figures must be regime-versioned.** Old adopted text: https://www.parlament.gov.rs/upload/archive/files/lat/pdf/zakoni/2021/1290-21-lat.pdf ; new: https://www.parlament.gov.rs/upload/archive/files/lat/pdf/zakoni/14_saziv/1317-26-lat..pdf . (Trap: gazette 35/2026 also carries the Zakon o trgovini amendment with its own separate 100.000 din pravno-lice fine — do not confuse it with the ZZP's 200.000 din.)

**Nesaobraznost presumption — an advisory FLAG, not a hard deadline.** Overall trader liability = **2 years** in both regimes.
- **OLD — 88/2021 čl. 52 st. 2:** presumption window **6 months from prelazak rizika** (passing of risk); burden on **prodavac**. Verbatim: *"Ako nesaobraznost nastane u roku od šest meseci od dana prelaska rizika na potrošača, pretpostavlja se da je nesaobraznost postojala u trenutku prelaska rizika... Teret dokazivanja da nije postojala nesaobraznost snosi prodavac."*
- **NEW — 35/2026 čl. 59 st. 3** (CORRECTION: the report's "čl. 56" is wrong — čl. 56 is the *remedies* article): presumption window **1 year from isporuka** (delivery); burden on **trgovac**. Verbatim: *"Ako nesaobraznost nastane u roku od godinu dana od dana isporuke robe potrošaču, pretpostavlja se da je nesaobraznost postojala u trenutku isporuke... Teret dokazivanja da nije postojala nesaobraznost snosi trgovac."*

Window and anchor both changed (6mo/prelazak rizika → 12mo/isporuka). Make window length + anchor regime-versioned; drive a warning ("teret dokazivanja na trgovcu") only — the register records the complaint regardless.

---

## 6. What VantumPOS must build (and must NOT)

**Deadline engine — build:**
- Two regime code paths, selected by persisted `regime` set at creation from `filed_at` vs configurable `CUTOVER_DATE` (default `2026-08-01`).
- `answer_due = receipt + 8` calendar days (hard, non-suspendable, both regimes).
- `resolution_due = submission + (15 | 30)` calendar days; 30 only when shop flags `tehnička roba` (per čl. 5 t. 41) or `nameštaj`.
- OLD path: on logged "trader received consumer response", **restart** resolution clock to a fresh 15/30 from that date. NEW path: **suspend** on logged "consumer received answer", **resume** on logged "trader received response", preserving elapsed days.
- Separate 3-day consumer-response timer from "consumer received answer"; on expiry with no response, auto-set status `nije saglasan` and flag impasse (do not auto-extend).
- At most one extension; require new date + consumer consent + register entry; block a second.
- Reminders fire ≥1 day early; deadlines are hard ceilings.

**Register — build:** all 11 `naročito` fields as first-class columns + register number + regime tag; synchronous register-number allocation + printable potvrda at intake; retention purge-*eligibility* at `filed_at + 2y` (floor, no purge-by date); role-gated, access-logged PII; free-form layout (no obrazac to match).

**Notices — build:** new-regime answer template gated on the 3-element express warning; printable prodajno-mesto notice; block/warn on utvrđivanje-nesaobraznosti fee for new-regime complaints; ADR-notice on rejection; oral-on-the-spot resolution path; never gate resolution on packaging return.

**Must NOT encode:**
- ❌ A single hard-coded cutover date without a review flag; do **not** silently assume 2 Aug 2026.
- ❌ Suspend/resume for old-regime complaints, or restart for new-regime complaints.
- ❌ "No fee for utvrđivanje nesaobraznosti" for OLD-regime complaints (new-only, čl. 63 st. 3).
- ❌ A due-date/timer for the potvrda ("bez odlaganja" is not a countdown).
- ❌ A "must delete by" retention date (2 years is a floor, not a cap).
- ❌ A weekend/holiday rollover that *extends* a deadline (unconfirmed ZUP applicability — safe = no extension).
- ❌ A narrow hard-coded `nameštaj` list (undefined term — user flag only); default goods to 15-day.
- ❌ The 300k–2M penalty tier for reklamacija breaches (wrong tier).
- ❌ Pinning the new presumption to čl. 56 (it is čl. 59 st. 3).
- ❌ The express-3-part-warning requirement on old-regime answers.
- ❌ Any invented day-count or restart/suspend rule not pinned above.

---

## 7. Still unverified

1. **Exact first day of application — 1 vs 2 Aug 2026.** *Tried:* čl. 220 primary text (formula only, no date); IPC computes 1 Aug (explicitly "not 2"); 88/2021 precedent calibration (application 20.12.2021 = anniversary+1) computes 2 Aug. Unresolved. *Ask lawyer/Ministarstvo trgovine:* "Koji je tačan kalendarski dan početka primene ZZP 35/2026 — 1. ili 2. avgust 2026. godine?" *Risk:* mis-selects regime for complaints filed at the cusp. *Mitigation in place:* conservative boundary 2026-08-01 (never grants more time) + configurable.
2. **Day-counting rule.** *Tried:* čl. 55/63 say bare "dana", no qualifier; which procedural law governs (ZUP vs ZOO vs plain calendar), whether the anchor day is counted, and holiday rollover are not stated. *Ask:* "Da li se rokovi od 8/15/30 dana iz čl. 55/63 računaju kao kalendarski dani od dana koji sledi za prijemom/podnošenjem, i da li se rok koji ističe u neradni dan pomera na prvi naredni radni dan?" *Risk:* off-by-one or an over-generous rollover. *Mitigation:* strict calendar, no rollover, reminders 1 day early.
3. **`nameštaj` definition for the 30-day track.** *Tried:* no definition in either law (word appears once); no bylaw located. *Ask:* "Da li je 'nameštaj' za rok od 30 dana definisan podzakonskim aktom ili je stvar redovnog značenja / procene trgovca?" *Risk:* mis-bucketing 15 vs 30 days. *Mitigation:* default 15-day; shop must affirmatively flag 30-day.
4. **Regime-selection trigger confirmation.** *Tried:* čl. 215 keys off unfinished "postupci"; inference is the reklamacija procedure "starts" at filing. *Ask:* "Da li se za izbor režima (stari/novi) merodavan datum izjavljivanja reklamacije, a ne datum kupovine/isporuke?" *Risk:* wrong regime if the procedure is deemed to start earlier/later. *Mitigation:* key off `filed_at`, persist regime at creation.
5. **Services vs goods.** *Tried:* čl. 55/63 & 52/59 are framed around `roba` (goods) + digital content; pure services follow a partly separate saobraznost pathway. *Ask:* "Da li reklamacije na čiste usluge ulaze u istu evidenciju i isti rok-engine?" *Risk:* wrong deadlines for service complaints. *Mitigation:* register records all; gate service-specific deadline logic until confirmed.
6. **Official-gazette byte cross-check.** *Tried:* verbatim text read from paragraf.rs, the official Ministry PDF (88/2021), the parlament.gov.rs adopted text (35/2026), and the pravno-informacioni-sistem.rs ELI for čl. 63 st. 11; the ELI HTML pages are JS-rendered and mostly non-extractable. *Ask/do:* final belt-and-suspenders diff of čl. 63 (35/2026) and čl. 55/52/188 (88/2021) against the printed Sl. glasnik RS PDFs before shipping. *Risk:* a transcription error in a mirror. *Mitigation:* two independent primary sources agree on every load-bearing stav above.
7. **Accounting-retention overlap.** *Tried:* not researched. If a reklamacija triggers a refund/replacement affecting fiscal isprave, Zakon o računovodstvu retention periods may apply to the *underlying transaction docs* (not the register itself). *Ask:* accountant/counsel, only if reklamacija records are linked to fiscal receipts. *Risk:* purging linked fiscal docs too early — low, since the ZZP 2-year floor is a floor and indefinite retention is compliant.