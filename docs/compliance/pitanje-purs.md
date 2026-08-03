> NACRT — pre upotrebe obavezna advokatska revizija. Stanje prava: 16.07.2026.

# Pisano pitanje Poreskoj upravi — status softvera koji predaje podatke odobrenom ESIR-u

## 1. Kontekst (ko pita i o čemu)

Privredno društvo **Actaer** (razvojni proizvod: **VantumPOS**, aplikacioni identifikator `com.actaer.vantumpos`) razvija i isporučuje **nefiskalni back-office / prateći softver** za trgovinu na malo. VantumPOS vodi internu evidenciju prometa i zaliha, upravljanje smenama i pravima korisnika, i internu kalkulaciju — dakle softver čije je vođenje za trgovca ionako zakonom propisano (v. [ZoT čl. 29–30](https://www.paragraf.rs/propisi/zakon_o_trgovini.html); KEP se može voditi elektronski, [PEP čl. 10](https://www.paragraf.rs/propisi/pravilnik_o_evidenciji_prometa.html)).

**VantumPOS ne izdaje fiskalne račune, nije elektronski fiskalni uređaj niti ESIR, ne komunicira sa procesorom fiskalnih računa (PFR) i ne poseduje bezbednosni element.** U pilot-postavci promet na malo evidentira se preko odobrenog ESIR-a same prodavnice („ESIR prvo, VantumPOS drugo“).

Actaer razmatra buduću tehničku integraciju u kojoj bi VantumPOS predao podatke o pojedinačnom prometu odobrenom ESIR-u prodavnice, koji bi potom izdao fiskalni račun u momentu prometa. Pre bilo kakvog uvođenja takve integracije tražimo pisano tumačenje Poreske uprave.

## 2. Pravni osnov pitanja

- **Definicija ESIR-a je kumulativna** — element „čija je upotreba odobrena od strane Poreske uprave, u koji obveznik fiskalizacije unosi podatke o prometu i iz kojeg se izdaje fiskalni račun“ ([ZF čl. 2 st. 1 tač. 5](https://www.paragraf.rs/propisi/zakon-o-fiskalizaciji-republike-srbije.html)): unos podataka **i** izdavanje računa.
- **Zakon reguliše obveznika, dobavljača EFU i odobrene elemente** (ESIR + PFR) — [ZF čl. 6](https://www.paragraf.rs/propisi/zakon-o-fiskalizaciji-republike-srbije.html) — a ne definiše softver koji podatke priprema uzvodno od ESIR-a i ne izdaje račune.
- **Tehničko uputstvo predviđa spolja hranjene ESIR-e** — fajl-ulaz transakcije je deklarativna opciona funkcija ESIR-a: „P17: ESIR podržava unos podataka o transakciji preko fajla… — Opciono“ ([TU 7.4.2 tač. 10](https://purs.gov.rs/upload/media/2025/2/4/414144/Tehnickouputstvo-ESIRiliL-PFR.pdf)).

## 3. Pitanje (verbatim, za dostavu PURS-u)

> „Da li softver koji nije odobreni element EFU sme da prenosi podatke o pojedinačnom prometu odobrenom ESIR-u putem ulaznog kanala dokumentovanog u odobrenju tog ESIR-a (fajl-ulaz po TU 7.4.2 tač. 10 P17 ili API), pri čemu ESIR izdaje fiskalni račun u momentu prometa — i da li takav softver sam po sebi predstavlja ESIR koji zahteva odobrenje po čl. 2 st. 1 tač. 5 Zakona?“

## 4. Kanal za podnošenje

- E-pošta: **budiefiskalizovan@gov.rs**
- Alternativno: pisano obraćanje Poreskoj upravi (redovan pisani zahtev za mišljenje).

## 5. Napomena o hitnosti (interno)

Odgovor **nije** potreban za sadašnji pilot sa dvostrukim kucanjem (svaka prodaja se posebno evidentira na odobrenom ESIR-u prodavnice). Odgovor **jeste** potreban pre bilo kakve integracije ili skaliranja preko pilota — v. §6.1 i F-5 u `docs/SERBIAN-LAW-COMPLIANCE.md`.
