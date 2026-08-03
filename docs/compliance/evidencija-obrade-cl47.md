> NACRT — pre upotrebe obavezna advokatska revizija. Stanje prava: 16.07.2026.

# Evidencija o radnjama obrade (ZZPL čl. 47)

**Pravni osnov:** [ZZPL čl. 47](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html). Evidencija se vodi trajno i stavlja na uvid Povereniku na zahtev. Izuzeće za male subjekte (< 250 zaposlenih) **ne važi**, i to po **oba** osnova iz st. 9 koja se ovde stiču: (1) obrada „nije povremena“ (st. 9 tač. 2) — dnevna obrada na POS-u nije povremena; (2) obrada obuhvata **posebne vrste podataka o ličnosti** iz [čl. 17 st. 1](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html) (st. 9 tač. 3) — časovi odsustva zbog privremene sprečenosti za rad otkrivaju podatak o zdravstvenom stanju, i bez dijagnoze. Propust: prekršaj — za preduzetnika **fiksna kazna 50.000 RSD** ([čl. 95 st. 6](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html), za prekršaj iz čl. 95 st. 2 tač. 5); za pravno lice kaznu propisuje čl. 95 st. 2, pa iznos zavisi od pravne forme rukovaoca.

> **Kontakt za zaštitu podataka — NE imenovati DPO (dobrovoljno imenovanje stvara obaveze objave i prijave).**

---

## A. Evidencija RUKOVAOCA (prodavnica) — 7 stavki (ZZPL čl. 47 st. 1)

1. **Ime i kontakt podaci rukovaoca** (i zajedničkog rukovaoca, predstavnika i kontakt-osobe za zaštitu podataka, ako postoje): `[naziv prodavnice, sedište, PIB/MB; kontakt-osoba za zaštitu podataka]`.
2. **Svrha obrade:** vođenje radnog odnosa i obračun rada; ispunjenje zakonskih obaveza (radno, računovodstveno, poresko zakonodavstvo); pripisivost prometa i smena zaposlenima.
3. **Vrsta lica i vrsta podataka:** vrsta lica — **zaposleni**; vrsta podataka — identitet (ime, korisničko ime), metapodaci o pristupu (heš PIN-a, evidencija prijava), pripisivost smena/prodaja, časovi rada i odsustva po kategorijama; **posebne vrste podataka o ličnosti** ([čl. 17 st. 1](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html)) — časovi privremene sprečenosti za rad, sa zakonskom podelom na teret poslodavca i na teret RFZO-a; **bez dijagnoze, doznake i slobodnog teksta**.
4. **Vrsta primalaca** kojima su podaci otkriveni ili će biti otkriveni: nadležni državni organi po zakonu; **Actaer** kao obrađivač tokom tehničke podrške.
5. **Prenos u drugu državu ili međunarodnu organizaciju:** `[ne / uskladiti sa AnyDesk odgovorom — §6.5]`.
6. **Rok čuvanja** za pojedine vrste podataka: prema zakonskim rokovima (evidencije u oblasti rada i obračun zarada — **trajno**; ostalo u propisanim rokovima).
7. **Opšti opis mera zaštite** iz [čl. 50](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html): kontrola pristupa i uloge, heširanje lozinki/PIN-ova (argon2), WAL i automatske rezervne kopije, **kriptozaštita rezervnih kopija** (SW-2) — **dostupna, a na snazi tek kada vlasnik postavi lozinku; do tada rezervne kopije nisu šifrovane** (ispravljeno 02.08.2026: ranije je stajala bez ograde, iako generator registra i red 8 registra usklađenosti oboje navode uslovno), evidencija sesija podrške (SW-10).

## B. Evidencija OBRAĐIVAČA (Actaer) — 4 stavke (ZZPL čl. 47 st. 4)

1. **Ime i kontakt podaci obrađivača** i svakog rukovaoca za koga obrađuje, predstavnika i kontakt-osobe za zaštitu podataka: Actaer (`com.actaer.vantumpos`), `[sedište, MB]`; rukovaoci — pilot-prodavnice `[spisak]`.
2. **Vrste obrade** koje se vrše u ime svakog rukovaoca: uvid i upotreba podataka o ličnosti tokom **daljinske tehničke podrške i održavanja** VantumPOS-a, po dokumentovanom nalogu rukovaoca.
3. **Prenos u drugu državu ili međunarodnu organizaciju:** `[ne / uskladiti sa AnyDesk odgovorom — §6.5]`.
4. **Opšti opis mera zaštite** iz [čl. 50](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html): pristup samo po nalogu rukovaoca, obaveza poverljivosti angažovanih lica, evidencija sesija podrške, uslovi za angažovanje podobrađivača.

---

**Napomena o DPO:** nijedna strana **nije** dužna da imenuje lice za zaštitu podataka (DPO) po [čl. 56 st. 2](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html). Ne imenovati DPO dobrovoljno — dobrovoljno imenovanje aktivira obaveze objavljivanja kontakta i prijave Povereniku (propust: za preduzetnika **fiksna kazna 50.000 RSD** — čl. 95 st. 6, za prekršaj iz čl. 95 st. 2 tač. 6). Koristi se termin „kontakt za zaštitu podataka“.
