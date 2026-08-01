> NACRT — pre upotrebe obavezna advokatska revizija. Stanje prava: 16.07.2026.

# Ugovor o obradi podataka o ličnosti (nacrt)

**Pravni osnov:** [ZZPL čl. 45](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html) (Zakon o zaštiti podataka o ličnosti, Sl. glasnik RS 87/2018). Preporučeni polazni obrazac: Standardne ugovorne klauzule Poverenika (Odluka, Sl. glasnik RS 5/2020, doneta po [ZZPL čl. 45 st. 11](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html)).

**Ugovorne strane:**
- **Rukovalac:** prodavnica (trgovac) — `[naziv, sedište, PIB/MB, zakonski zastupnik]`.
- **Obrađivač:** Actaer (`com.actaer.vantumpos`) — `[pun naziv, sedište, MB]`.

**Svrha ugovora:** Actaer, prilikom pružanja daljinske tehničke podrške i održavanja VantumPOS-a, može da izvrši uvid u podatke o ličnosti koje rukovalac obrađuje (obrada uključuje i uvid i upotrebu — [ZZPL čl. 4 st. 1 tač. 3](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html)), pa se ovim ugovorom uređuje obrada u ime rukovaoca.

---

## Član 1 — Predmet, trajanje, priroda i svrha obrade (ZZPL čl. 45 st. 3)

1. **Predmet obrade:** pružanje daljinske tehničke podrške i održavanja informacionog sistema VantumPOS.
2. **Trajanje obrade:** za vreme trajanja ugovora o podršci/održavanju, odnosno do prestanka ugovora; posle prestanka postupa se po članu 9 ovog ugovora.
3. **Priroda i svrha obrade:** „daljinska tehnička podrška i održavanje VantumPOS-a“ — pristup se vrši isključivo radi otklanjanja smetnji, dijagnostike i konfiguracije, po dokumentovanom nalogu rukovaoca.
4. **Vrsta podataka o ličnosti:** identitet zaposlenih (ime i korisničko ime), metapodaci o pristupnim podacima (npr. heš PIN-a, evidencija prijava — ne i sam PIN u čitljivom obliku) i pripisivost smena/prodaja pojedinom zaposlenom.
5. **Vrsta lica** (zakonski termin je „vrsta lica“, ne „kategorije lica“): **zaposleni prodavnice**.
6. **Prava i obaveze rukovaoca:** rukovalac utvrđuje svrhu i način obrade, daje dokumentovane naloge i odgovoran je za zakonitost osnova obrade.

## Član 2 — Obaveze obrađivača (ZZPL čl. 45 st. 4 tač. 1–8)

Obrađivač se obavezuje da:

1. **(tač. 1)** obrađuje podatke isključivo na osnovu **dokumentovanih pisanih naloga** rukovaoca, uključujući i u pogledu svakog prenosa podataka u drugu državu ili međunarodnu organizaciju, osim ako je na to obavezan zakonom (u kom slučaju o tome obaveštava rukovaoca pre obrade, ako zakon to ne zabranjuje);
2. **(tač. 2)** obezbedi da su lica ovlašćena za obradu obavezala se na **čuvanje poverljivosti** ili da su pod odgovarajućom zakonskom obavezom čuvanja tajnosti;
3. **(tač. 3)** preduzme sve mere zaštite iz **[ZZPL čl. 50](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html)** (tehničke, organizacione i kadrovske mere; kriptozaštita; pristup samo po nalogu);
4. **(tač. 4)** poštuje **uslove za angažovanje drugog obrađivača** iz stava 2 i 7 ovog člana zakona (v. Aneks 1);
5. **(tač. 5)** pomaže rukovaocu, odgovarajućim tehničkim i organizacionim merama, u ispunjavanju obaveza prema zahtevima lica na koje se podaci odnose (ostvarivanje prava iz Glave III ZZPL);
6. **(tač. 6)** pomaže rukovaocu u obezbeđivanju primene obaveza iz **čl. 50 i čl. 52–55** (bezbednost, obaveštavanje o povredi, procena uticaja, prethodno mišljenje);
7. **(tač. 7)** po prestanku pružanja usluga, prema izboru rukovaoca, **izbriše ili vrati** sve podatke i uništi postojeće kopije, osim ako je čuvanje propisano zakonom;
8. **(tač. 8)** učini dostupnim rukovaocu **sve informacije** neophodne za dokazivanje ispunjenosti obaveza i **omogući i doprinese sprovođenju kontrole (audita)**, uključujući inspekcijski nadzor koji sprovodi rukovalac ili lice ovlašćeno od strane rukovaoca.

## Član 3 — Upozorenje o nezakonitom nalogu (ZZPL čl. 45 st. 5)

Obrađivač je dužan da **bez odlaganja obavesti rukovaoca** ako smatra da je neki nalog rukovaoca u suprotnosti sa ZZPL ili drugim propisom o zaštiti podataka o ličnosti.

## Član 4 — Povreda podataka o ličnosti (ZZPL čl. 52)

1. Obrađivač je dužan da, čim utvrdi povredu podataka o ličnosti, o tome obavesti rukovaoca **bez nepotrebnog odlaganja, a najkasnije u roku od 24 časa** ([ZZPL čl. 52 st. 3](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html)).
2. Obaveštenje sadrži sadržinu iz **čl. 52 st. 4** (opis prirode povrede i vrste/približnog broja lica i podataka; kontakt tačku; opis mogućih posledica; opis preduzetih ili predloženih mera).
3. Detaljan tok postupanja uređen je runbook-om `docs/compliance/runbook-povreda-podataka.md`.

## Član 5 — Kontrola i evidentiranje pristupa (audit)

1. Obrađivač vodi evidenciju o sesijama daljinske podrške (početak/kraj sesije i operater) i o pristupu podacima o ličnosti, čime dokazuje pristup **isključivo po nalogu** rukovaoca ([ZZPL čl. 50 st. 5](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html)).
2. Ova klauzula se oslanja na budući modul evidencije (audit log) u VantumPOS-u (SW-10); do njegovog uvođenja obrađivač vodi ručnu evidenciju sesija.

## Član 6 — Pretvaranje obrađivača u rukovaoca (ZZPL čl. 45 st. 8)

Ako obrađivač obrađuje podatke za sopstvene svrhe, odnosno određuje svrhu i način obrade suprotno nalozima rukovaoca, **smatra se rukovaocem** u pogledu te obrade i snosi odgovarajuću odgovornost.

---

## Aneks 1 — Angažovanje drugog (pod)obrađivača (ZZPL čl. 45 st. 2 i st. 7)

1. **Ovlašćenje (st. 2):** obrađivač ne sme da angažuje drugog obrađivača bez prethodnog **opšteg ili posebnog pisanog ovlašćenja** rukovaoca. Kod opšteg ovlašćenja, obrađivač obaveštava rukovaoca o svakoj nameravanoj promeni podobrađivača i omogućava rukovaocu da uloži **prigovor**.
2. **Prenos obaveza / puna odgovornost (st. 7):** obrađivač na svakog podobrađivača ugovorom prenosi **iste obaveze zaštite** koje ima prema ovom ugovoru; ako podobrađivač ne ispuni svoje obaveze zaštite, **obrađivač ostaje u punoj odgovornosti** prema rukovaocu za ispunjenje obaveza tog podobrađivača.

> **[ZA PRAVNIKA: AnyDesk = prenos u drugu državu / pod-obrađivač? — potvrditi]**
> Sesije daljinske podrške preko AnyDesk-a mogu potencijalno predstavljati (a) prenos podataka u drugu državu ([ZZPL čl. 63](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html) i dalje) i/ili (b) angažovanje AnyDesk GmbH kao podobrađivača. Potrebno je pravno mišljenje pre potpisivanja (v. §6.5 u `docs/SERBIAN-LAW-COMPLIANCE.md`). Klauzula o prenosu i obaveštenje zaposlenima (`obavestenje-zaposlenima.md`) usklađuju se sa odgovorom.

---

**Potpisi:** za rukovaoca `[___________]` · za obrađivača `[___________]` · datum `[________]`.
