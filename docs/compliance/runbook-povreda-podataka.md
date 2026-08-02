> NACRT — pre upotrebe obavezna advokatska revizija. Stanje prava: 16.07.2026.

# Runbook: povreda podataka o ličnosti (obe strane)

**Pravni osnov:** [ZZPL čl. 52–53](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html); obrazac obaveštenja Povereniku [Pravilnik 40/2019](https://www.paragraf.rs/propisi/pravilnik-o-obrascu-obavestavanja-poverenika-za-informacije-od-javnog-znacaja-o-povredi.html).

Povreda = povreda bezbednosti koja dovodi do slučajnog ili nezakonitog uništenja, gubitka, izmene, neovlašćenog otkrivanja ili pristupa podacima o ličnosti.

## Lanac obaveštavanja (rokovi)

1. **Obrađivač (Actaer) → rukovalac (prodavnica): ≤ 24 časa.**
   Čim utvrdi povredu, Actaer bez nepotrebnog odlaganja, a najkasnije u **24 časa** (ugovorni rok, `ugovor-o-obradi-nacrt.md` čl. 4; zakonski osnov [čl. 52 st. 3](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html)), obaveštava rukovaoca sa sadržinom iz čl. 52 st. 4 (priroda povrede; vrsta i približan broj lica/podataka; kontakt tačka; moguće posledice; preduzete/predložene mere).

2. **Rukovalac (prodavnica) → Poverenik: ≤ 72 časa.**
   Rukovalac obaveštava Poverenika bez nepotrebnog odlaganja, a najkasnije u **72 časa** od saznanja ([čl. 52 st. 1](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html) — **ispravljeno 02.08.2026: ranije je stajalo „čl. 53 st. 1“, a čl. 53 uređuje obaveštavanje lica na koje se podaci odnose, ne Poverenika**), na **obrascu iz Pravilnika 40/2019**, skeniran primerak na **povredapodataka@poverenik.rs**. Ako se obaveštava posle 72 časa, navode se razlozi kašnjenja.

3. **Rukovalac → lica na koje se podaci odnose: samo kod visokog rizika.**
   Neposredno obaveštavanje zaposlenih (jasnim jezikom) samo ako povreda **verovatno predstavlja visok rizik** za prava i slobode lica ([čl. 53](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html)).

## Interna dokumentacija (uvek)

Rukovalac **dokumentuje svaku povredu** — činjenice, posledice i preduzete mere — nezavisno od toga da li se obaveštava Poverenik; dokumentacija omogućava Povereniku proveru postupanja ([čl. 52 st. 6–7](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html)). (Ekran za evidenciju incidenata: **Privatnost → Evidencija povreda**, isporučen 02.08.2026 (SW-17). Obrazac iz Pravilnika 40/2019 se generiše iz zapisa; podnošenje je i dalje ručno — API za podnošenje ne postoji.)

## Izuzetak: šifrovane rezervne kopije (safe-harbor)

Neposredno obaveštavanje lica **nije obavezno** ako je rukovalac primenio odgovarajuće mere zaštite — posebno mere zbog kojih su podaci **nerazumljivi** neovlašćenim licima (npr. **kriptozaštita**) — [čl. 53 st. 3 tač. 1](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html). **Ovaj izuzetak se primenjuje na izgubljene/kompromitovane rezervne kopije tek pošto je uključeno šifrovanje rezervnih kopija (SW-2).**

---

**Kontakti (popuniti):** kontakt-osoba rukovaoca `[__________]`; kontakt Actaer podrške `[__________]`; Poverenik `povredapodataka@poverenik.rs`.
