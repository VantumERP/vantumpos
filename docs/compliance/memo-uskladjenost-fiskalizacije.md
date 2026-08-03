> NACRT — pre upotrebe obavezna advokatska revizija. Stanje prava: 16.07.2026.

# Interni memo: nefiskalna namena i anti-evazioni dizajn VantumPOS-a (SW-4)

**Svrha:** stalni pisani odgovor (rebuttal) na svaku moguću kvalifikaciju da VantumPOS „služi za izbegavanje evidentiranja prometa, odnosno izbegavanje plaćanja poreza“ u smislu [ZPPPA čl. 175b](https://www.paragraf.rs/propisi/zakon_o_poreskom_postupku_i_poreskoj_administraciji.html) (na snazi od 20.12.2022; zaprećena kazna 1–5 godina zatvora + zabrana poziva 1–5 godina + oduzimanje). Napomena: [ZPPPA čl. 175a](https://www.paragraf.rs/propisi/zakon_o_poreskom_postupku_i_poreskoj_administraciji.html) — konstitutivni element je **funkcija izbegavanja**, a ne (ne)registracija; „drži“ kriminalizuje i posedovanje kod prodavnice, pa čist dizajn štiti obe strane.

## 1. Nefiskalna namena

VantumPOS **nije elektronski fiskalni uređaj niti ESIR i ne izdaje fiskalne račune**; promet na malo evidentira se preko odobrenog ESIR-a prodavnice. VantumPOS ne štampa ništa nalik fiskalnom računu (bez QR koda, bez naslova „FISKALNI RAČUN“, bez PIB/PFR/brojač bloka), ne komunicira sa PFR-om i ne poseduje bezbednosni element. Njegova kategorija — interna evidencija prometa/zaliha — za trgovca je zakonom **propisana** ([ZoT čl. 29–30](https://www.paragraf.rs/propisi/zakon_o_trgovini.html); [PEP čl. 10](https://www.paragraf.rs/propisi/pravilnik_o_evidenciji_prometa.html)), bez forme i bez odobrenja.

## 2. Append-only (samo-dodavanje) dizajn glavne knjige

Finalizovan promet se **ne može izmeniti na mestu niti obrisati bez traga**. Ispravke se izvode isključivo kroz nove, dodatne zapise u knjizi — model kontra-dokumenata.

## 3. Model kontra-dokumenata za storno i povraćaj

- **Storno (void)** ne briše i ne menja originalni novčani red — dodaje se kompenzujući zapis; originalni red ostaje u knjizi.
- **Povraćaj (return)** je takođe samo-dodavanje — originalna prodaja ostaje netaknuta, povraćaj je poseban zapis.

## 4. Reset: evidentiran, ograničen na administratora, „backup-first“

Reset (brisanje pre puštanja u produkciju) je alat pre-produkcije: (a) **zahteva prethodnu svežu rezervnu kopiju**, (b) prikazuje upozorenje o retencionom horizontu (10 godina; [ZoRač čl. 28](https://www.paragraf.rs/propisi/zakon_o_racunovodstvu.html); [ZPDV čl. 47](https://www.paragraf.rs/propisi/zakon_o_porezu_na_dodatu_vrednost.html) + [ZPPPA čl. 114ž](https://www.paragraf.rs/propisi/zakon_o_poreskom_postupku_i_poreskoj_administraciji.html)) i obavezi arhivske saglasnosti za d.o.o. prodavnice, i (c) upisuje **trajni tombstone/log zapis** o tome ko/kada/šta je obrisano. Reset je dostupan samo administratoru.

## 5. Invarijante potvrđene testovima

Anti-evazioni dizajn je pokriven izvršnim testovima (SW-4). Reference po imenu:

- **`void_is_append_only_and_preserves_original_monetary_row`** — storno je samo-dodavanje i čuva originalni novčani red (`src-tauri/src/commands/receipts.rs`).
- **`return_is_append_only_and_preserves_original`** — povraćaj je samo-dodavanje i čuva original (`src-tauri/src/commands/receipts.rs`).
- **`reset_requires_backup_and_tombstone_together`** — reset zahteva rezervnu kopiju i tombstone zajedno (`src-tauri/src/commands/backup.rs`).

## 6. Dizajn-pravilo za rotaciju/uklanjanje rezervnih kopija

Kada se ikada bude gradila rotacija/uklanjanje (pruning) rezervnih kopija, **rezervne kopije zatvorenih godina isključuju se iz uklanjanja** — retencioni horizont od 10 godina (§4) mora preživeti svaku rotaciju.

---

**Zaključak:** VantumPOS nema funkciju izbegavanja evidentiranja prometa; naprotiv, njegova knjiga je samo-dodavajuća i otporna na neevidentirano brisanje, a reset je ograničen, evidentiran i uslovljen prethodnom rezervnom kopijom. Ovaj memo, uz invarijantne testove iz §5, čini stalni odgovor na svaku kvalifikaciju po ZPPPA čl. 175b i štiti i Actaer i prodavnicu-držaoca.
