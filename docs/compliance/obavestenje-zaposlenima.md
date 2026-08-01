> NACRT — pre upotrebe obavezna advokatska revizija. Stanje prava: 01.08.2026.
>
> **Ponovno uručenje (ZZPL čl. 23 st. 3).** Uvođenjem modula „Radno vreme“ (evidencija radnog vremena,
> SW-14) proširene su svrhe obrade i vrste podataka — dodata je obrada **posebne vrste podataka**
> (kategorija odsustva) i uveden je razdvojen režim čuvanja. Ovo obaveštenje se zbog toga **iznova uručuje svakom
> zaposlenom pre nego što modul počne da se koristi**, a datum uručenja se evidentira.

# Obaveštenje o obradi podataka o ličnosti zaposlenih

**Pravni osnov:** [ZZPL čl. 23](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html) (obaveštavanje lica na koje se podaci odnose prilikom prikupljanja). Uručuje se zaposlenom pri zasnivanju radnog odnosa (onboarding). Propust: prekršaj — novčana kazna **50.000–2.000.000 RSD za pravno lice** ([čl. 95 st. 1 tač. 8](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html)), odnosno **20.000–500.000 RSD za preduzetnika** ([čl. 95 st. 4](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html)). Iznos zavisi od pravne forme poslodavca.

---

## 1. Identitet i kontakt rukovaoca

Rukovalac podataka je **prodavnica** — `[naziv / poslovno ime, sedište, PIB/MB]`.
Kontakt za zaštitu podataka: `[ime i prezime kontakt-osobe, e-pošta, telefon]`.

> Napomena (interno): koristi se naziv **„kontakt za zaštitu podataka“**, a **ne** „lice za zaštitu podataka (DPO)“ — dobrovoljno imenovanje DPO stvara obaveze objave i prijave (v. `evidencija-obrade-cl47.md`).

## 2. Svrhe obrade i pravni osnov

Vaši podaci obrađuju se radi:

| Svrha | Pravni osnov (ZZPL čl. 12 st. 1) |
|---|---|
| Zasnivanje i izvršenje ugovora o radu, obračun rada i pripisivost prometa/smena | **tač. 2** — izvršenje ugovora / preduzimanje radnji pre zaključenja ugovora |
| Ispunjenje zakonskih obaveza poslodavca (radno, računovodstveno, poresko zakonodavstvo) | **tač. 3** — poštovanje pravnih obaveza rukovaoca |
| Vođenje evidencije radnog vremena: dnevna evidencija prekovremenog rada (ZoR čl. 55 st. 6), časovi po vrstama iz ZEOR čl. 24 tač. 1, kao i provera zakonskih ograničenja iz ZoR čl. 53 i zaštita zaposlenih iz ZoR čl. 87–91 | **tač. 3** — poštovanje pravnih obaveza rukovaoca |

### 2.1 Posebne vrste podataka o ličnosti

U evidenciji radnog vremena obrađuje se i **kategorija odsustva** — npr. časovi privremene sprečenosti za
rad na teret sredstava poslodavca, odnosno na teret sredstava RFZO, ili časovi porodiljskog odsustva i
skraćenog radnog vremena roditelja. U delu u kojem otkriva podatke o zdravstvenom stanju, to je
**posebna vrsta podataka o ličnosti** u smislu [ZZPL čl. 17](https://www.paragraf.rs/propisi/zakon_o_zastiti_podataka_o_licnosti.html).

| Podatak | Osnov za obradu posebne vrste |
|---|---|
| Kategorija odsustva i broj časova po kategoriji | **ZZPL čl. 17 st. 2 tač. 2** — obrada je neophodna radi izvršenja obaveza i primene zakonom propisanih ovlašćenja rukovaoca i lica na koje se podaci odnose **u oblasti rada, socijalnog osiguranja i socijalne zaštite** |

**Šta se NE obrađuje:** ne evidentiraju se dijagnoza, šifra bolesti, broj doznake, nalaz, niti bilo kakav
slobodan tekst ili prilog uz odsustvo. Kategorija odsustva je **zatvorena lista vrednosti** i uz nju se
upisuje **isključivo broj časova**. Podatak o trudnoći ili dojenju vodi se samo kao oznaka „da/ne“ sa
datumom (ZoR čl. 90), bez medicinske dokumentacije.

**Ko sme da vidi kategoriju odsustva:** samo vlasnik, odnosno lice zaduženo za obračun zarada. Ostale
uloge u aplikaciji vide isključivo oznaku „odsutan“ i zbir časova. **Vi sami uvek vidite svoje podatke** —
u aplikaciji, kroz pregled „Moji sati“ (ZoR čl. 83 st. 1, ZZPL čl. 26).

Obrada se **ne zasniva na pristanku (saglasnosti)** — pristanak se u radnom odnosu tumači restriktivno i ne koristi se kao osnov za obradu podataka zaposlenih (v. [ZZPL čl. 12 st. 1 tač. 2–3](https://mduls.gov.rs/wp-content/uploads/Zakon-o-za%C5%A1titi-podataka-o-li%C4%8Dnosti.pdf)). Ni evidencija radnog vremena ne traži i ne prikuplja Vaš pristanak. Kada zakon traži pisanu saglasnost zaposlenog za prekovremeni rad (ZoR čl. 91) ili za preraspodelu radnog vremena (ZoR čl. 57 st. 4), aplikacija **beleži samo da takva saglasnost postoji i od kog datuma** — sama saglasnost se daje i čuva van aplikacije.

## 3. Primaoci podataka

Podaci se ne dostavljaju nikome osim sledećim primaocima:

| Primalac | Šta prima | Osnov |
|---|---|---|
| **Inspektorat za rad — inspektor rada** | Evidenciju radnog vremena i evidencije u oblasti rada, na zahtev, u postupku inspekcijskog nadzora | ZoR čl. 268a tač. 1, čl. 269 st. 3, čl. 270; [ZIN čl. 20 st. 7 i čl. 21 tač. 4](https://www.paragraf.rs/propisi/zakon-o-inspekcijskom-nadzoru.html) (dokumentacija se daje na uvid u obliku u kojem se poseduje i čuva) |
| **Poreska uprava i drugi nadležni državni organi** | Podatke iz poslovnih i računovodstvenih evidencija, kada je to propisano zakonom | ZPPPA; ZoRač |
| **Knjigovođa, odnosno računovodstvena agencija** koju je poslodavac angažovao — `[naziv, sedište, PIB/MB]` | **Samo zbirne časove po vrstama** iz evidencije radnog vremena, radi obračuna zarade | ZZPL čl. 12 st. 1 tač. 3, kao obrađivač po ugovoru |
| **Actaer (`com.actaer.vantumpos`)** — dobavljač softvera VantumPOS | Pristup podacima **tokom sesija tehničke podrške i održavanja**, isključivo po nalogu poslodavca | Obrađivač, po zaključenom ugovoru o obradi (`ugovor-o-obradi-nacrt.md`), ZZPL čl. 45–46 |

Obračun zarade, poreza i doprinosa **ne vrši se u aplikaciji VantumPOS** — aplikacija knjigovođi
prosleđuje samo broj časova po vrstama (ZEOR čl. 24 tač. 1), bez iznosa.

## 4. Prenos u druge države

Bez prenosa u druge države ili međunarodne organizacije.
`[NAPOMENA: uskladiti sa odgovorom na pitanje da li AnyDesk sesija predstavlja prenos u drugu državu — v. §6.5 u docs/SERBIAN-LAW-COMPLIANCE.md]`

## 5. Rok čuvanja (kriterijumi)

Podaci se čuvaju za vreme trajanja radnog odnosa i nakon njega u rokovima propisanim zakonom. Po isteku
roka podaci se brišu ili anonimizuju, osim onih čije je čuvanje zakonom propisano.

**Podaci o radnom vremenu ne čuvaju se u jednom roku — razdvojeni su po vrsti zapisa.** Zaključena
klasifikacija čuva se trajno, a sirovi i radni podaci imaju ograničen rok:

| Šta | Koliko | Zašto |
|---|---|---|
| **Izvedena mesečna klasifikacija časova** — zaključeni mesečni zbir časova po vrstama iz ZEOR čl. 24 tač. 1, po zaposlenom | **Trajno** | ZEOR čl. 7 st. 2 i čl. 25 st. 3. Ovaj skup podataka aplikacija tehnički izuzima iz brisanja i iz resetovanja podataka. **Vraćanje iz rezervne kopije vraća celu bazu na stanje iz te kopije, pa i ovu evidenciju** — na tom putu zaštita je rezervna kopija zatečenog stanja koju aplikacija obavezno napravi pre vraćanja, uz zapis o tome koliko je zapisa vraćanje pomerilo. **Automatsko čišćenje starih rezervnih kopija ne postoji** — stare kopije ostaju dok ih neko ručno ne obriše |
| **Samostalna evidencija prekovremenog rada** (ZoR čl. 55 st. 6), kada nije deo obračuna zarade | **Najmanje tri godine** od dana na koji se odnosi, uz mogućnost produženja | Zakon ne propisuje rok; primenjuje se odbrambeni minimum izveden iz rokova zastarelosti (ZoR čl. 196; Zakon o prekršajima čl. 84). Rok se pomera **samo unapred**, nikada unazad |
| **Radne verzije unosa i pomoćni podaci o vremenu** — nezaključeni unosi i podaci o vremenu koji služe samo proveri usklađenosti | **Brišu se pošto je mesec zaključen** i klasifikacija izvedena | ZZPL čl. 5 st. 1 tač. 5 — podaci se ne čuvaju duže nego što je neophodno za svrhu |

Ispravka unosa **ne briše** prethodni zapis: raniji zapis ostaje sačuvan i vidljiv kao ispravljen, sa
podatkom ko je, kada i zbog čega izvršio ispravku (ZEOR čl. 46 st. 1). Zbog toga se pravo na brisanje iz
odeljka 6 ovog obaveštenja na ove zapise primenjuje u granicama zakonskih obaveza čuvanja.

## 6. Vaša prava

Kao lice na koje se podaci odnose, imate pravo na:

- **pristup** podacima (čl. 26),
- **ispravku i dopunu** (čl. 29),
- **brisanje** („pravo na zaborav“, čl. 30) — u granicama zakonskih obaveza čuvanja,
- **ograničenje obrade** (čl. 31),
- **prigovor** na obradu (čl. 37),
- **prenosivost podataka** (čl. 36),
- **pritužbu Povereniku** za informacije od javnog značaja i zaštitu podataka o ličnosti ([www.poverenik.rs](https://www.poverenik.rs)).

## 7. Obaveznost davanja podataka i posledice

Davanje podataka je **obavezno** — neophodno je za zasnivanje i izvršenje radnog odnosa i za ispunjenje zakonskih obaveza poslodavca. Ukoliko podatke ne date, poslodavac **ne može da zasnuje niti izvršava radni odnos**.

## 8. Automatizovano odlučivanje

Ne vrši se **automatizovano donošenje odluka**, uključujući profilisanje, koje proizvodi pravne posledice po vas ili na vas značajno utiče.

---

Datum uručenja `[________]` · Potpis zaposlenog `[___________]` · Za rukovaoca `[___________]`.
