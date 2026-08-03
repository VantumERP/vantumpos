# VantumPOS: Pilot-Readiness Verdict

## 1. The blunt verdict

**No. Not today, and not with a two-week polish pass.**

Three independent reasons, any one of which is disqualifying:

1. **It is illegal as a till.** In Serbia the cash register is not a product category — it is a legal object. `Zakon o fiskalizaciji` čl. 2(2): the electronic fiscal device is an *approved* ESIR + an *approved* PFR + a security element. VantumPOS is neither approved nor connected to anything approved (`sales.fiscal_status` is hard-coded to the literal `'not_fiscalized'` at `sales.rs:217`, `receipts.rs:359`, `receipts.rs:517`; there is no HTTP client in the backend capable of talking to a PFR — the only outbound call is Open Food Facts). A pilot shop that rings sales here commits the čl. 15 offence: 50k–500k RSD for a preduzetnik **plus a 15/90/365-day business ban**. And čl. 18 fines **you, the vendor**, 300,000 RSD (pravno lice) / 150,000 (preduzetnik) for *supplying* a non-approved element. Handing this to a shop as a till is legal exposure for you, not just for them.

2. **It silently corrupts money the first time anyone does a return.** `return_items` (`receipts.rs:438-603`) writes **zero** `sale_payments` rows and flips the *entire original sale* to `status='refunded'` (`receipts.rs:591-597`). `shifts.rs:248` only sums cash from `status='completed'` sales. So a 10.000 RSD cash sale with a 2.000 RSD return produces a **phantom +8.000 RSD surplus** at shift close and a **−12.000 RSD** daily turnover for a day that took +8.000. Void has the identical double-subtraction bug. This is worse than a missing feature: it is a shipped feature that destroys the two numbers the owner buys a POS for. In a boutique, returns and size exchanges fire in **week one**.

3. **It does not work in Serbian.** `catalog.rs:593` lowercases the search term with Rust's Unicode `to_lowercase()`, then compares it against SQLite's **ASCII-only** `LOWER(p.name)` (`catalog.rs:633`). `lower('KOŠULJA')` = `'koŠulja'`. An imported ALL-CAPS supplier price list is **unfindable**. Anything starting with Š/Č/Ć/Ž/Đ (Šorc, Čarape, Džemper) is unfindable when typed lowercase. And the register's *only* path to the cart is that search box, which then blindly takes `result.items[0]` (`RegisterScreen.tsx:161`) — so an ambiguous query rings up the **wrong product at the wrong price** with no disambiguation. On top of that, a fresh DB seeds **no VAT rate** while `products.tax_rate_id` is `NOT NULL` — so out of the box you cannot create a single product, and every CSV import row fails.

Fixing #3 is days. Fixing #2 is a fortnight. #1 is a strategic decision you have not yet made, and it is the whole ballgame — see §5.

The good news, stated honestly: **most of the plumbing is right.** The counter-document model for voids/returns, `original_sale_id`/`original_sale_item_id` linkage, partial-return quantity enforcement, inventory restoration, session-derived auth (`shifts.rs:191`, and `auth.rs:156` even carries the correct comment about never trusting client-supplied roles), the multi-row `sale_payments` table, a real CSV importer with delimiter sniffing, argon2 hashing, a real backup snapshot via rusqlite's online backup API. This is a ~6–9 week problem, not a rewrite.

---

## 2. Gap tiers

Effort scale: **S** ≤ 2 days · **M** ~1–2 weeks · **L** 3–6 weeks · **XL** months.

---

### T0 — LEGAL / CANNOT SHIP

Six items. Two are strategic; four are trivial and just have to be done before anyone touches the app.

#### T0-A: legal exposure

| # | Gap | Why it matters here | Effort | Depends on |
|---|---|---|---|---|
| **0.1** | **No fiscalization (ESIR + L-PFR + bezbednosni element)** | Every retail sale in Serbia must pass an approved EFU. Clothing/textile retail is **not** on the exemption list (`Uredba` 32/2021). A physical shop needs ≥1 **L-PFR** per poslovno mesto (čl. 6(4)) — V-PFR alone doesn't cover it. Also missing: the fiscal document matrix (Promet/Kopija/Predračun/Obuka/Avans × Prodaja/Refundacija), tax rates taken *from the PFR* not a local table, buyer-ID šifarnik on cash refunds, 30-day electronic journal, 5-day offline sync clock. Today, a pilot shop must **double-ring every sale** — the single most reliable way to get a POS thrown out in week one. | XL (own ESIR) / M (handoff) / S (companion posture) | Everything fiscal: printing, 7 tenders, buyer ID, refund-references-PFR, SEF corporate-card, avans |
| **0.2** | **Vendor-side approval + "shadow till" risk** | You cannot *deliver* an EFU element before PURS approves it. And a non-fiscal sales-recording system running beside the legal till is precisely the fact pattern an inspector reads as evidence of unrecorded turnover. Until 0.1 is resolved, the app must be **shaped so it cannot look like a shadow till** — no customer-facing slip. | S (posture) / XL (approval) | 0.1 |
| **0.3** | **No DPA / no privacy notice / no terms** | You will be a **obrađivač** (processor) of the shop's personal data — employee records today, and JMBG/lična karta the moment buyer-ID fiscal fields land. ZZPL requires a **written ugovor o obradi** with each shop. Remote AnyDesk support cements processor status. You legally cannot start a paid pilot without one. Fines 50k–2M RSD. There is no DPA, no EULA, no SLA, no privacy notice anywhere in this project. | S (lawyer + template) | — |

#### T0-B: the app does not work on day one

| # | Gap | Evidence | Effort |
|---|---|---|---|
| **0.4** | **Broken out of the box: no seeded VAT rate.** `products.tax_rate_id` is `NOT NULL REFERENCES tax_rates(id)` (`migrations.rs:73`), and the only production `INSERT INTO tax_rates` is the admin CRUD command. Until the owner finds Podešavanja → PDV unaided, **no product can be created** and every import row dies with "PDV stopa nije pronađena". The bug is invisible in dev because `mock-adapter.ts` pre-seeds PDV 20/10. **Do not blind-seed 20/10** — a large slice of the target market is below the 8.000.000 RSD PDV threshold and would silently issue wrong receipts. Seed behind one question: *"Da li ste u sistemu PDV-a?"* | `migrations.rs:73`, `importer.rs:575`, `CatalogModule.tsx:1867` | **S** |
| **0.5** | **Serbian text is unsearchable + wrong item rings up.** ASCII `LOWER()` vs Unicode `to_lowercase()`; no diacritic folding (cashier on a US layout types `sorc`, finds nothing); BINARY collation so Š/Č/Ž sort after Z; and `items[0]` blindly added with no disambiguation. There is **no local exact-barcode resolver anywhere** — `catalog_lookup_product_by_barcode` is an Open Food Facts HTTP call (`catalog.rs:502-536`), useless for jeans. | `catalog.rs:593/631-642`, `RegisterScreen.tsx:147-170` | **S** |
| **0.6** | **Out-of-stock sale hard-refused with no override.** `validate_stock` (`sales.rs:474-500`) rejects the whole sale unless the product carries `allow_negative_stock` (default 0, buried in the Katalog edit form). The customer is holding the garment; the till says no; the only escape is to abandon the sale, leave the counter, edit the product, come back. On day one book stock is wrong for a large fraction of SKUs, so this fires **constantly**. Staff response: ring it as a different article, or take cash and write it in a notebook. Either destroys the pilot's data. Needs a shop-level setting + a per-sale override. | `sales.rs:474-500`, `CatalogModule.tsx:1375` | **S** |
| **0.7** | **Will it even launch?** `tauri.conf.json` has **no `bundle.windows` block**, so Tauri's default `downloadBootstrapper` applies: the installer tries to *download* the WebView2 runtime. A Novi Pazar counter PC frequently has no internet, and on older Windows it fails outright on TLS 1.2. Window is untouched scaffold 800×600, `productName` is literally `"vantumpos"`, identifier `com.adnan.vantumpos`. Fix: `embedBootstrapper` (+1.8 MB) or `offlineInstaller`, minWidth, real product name, single-instance guard. | `src-tauri/tauri.conf.json` | **S** |

**Everything in T0-B is under two weeks total.** There is no excuse to have a pilot conversation before it is done.

---

### T1 — PILOT BLOCKER (works, but they will suffer daily or fall back to paper)

| # | Gap | Why it matters here | Effort | Depends |
|---|---|---|---|---|
| **1.1** | **Returns/voids move no money; partial return nukes the whole receipt.** As above. Also structurally blocked: `sale_payments.amount_minor CHECK (>= 0)` (`migrations.rs:133`) — a refund row **cannot be written** without a migration. And `ensure_voidable` requires `status=='completed'`, so a receipt with any partial return **can never be voided, ever**. | Size exchanges are the #1 counter event in a boutique. Serbian law gives no change-of-mind refund right in-store, so the shop's remedy is exchange — which today is return (no money) + new sale (demands full tender). The drawer and the Z-report drift on every single one. | **M** | schema migration |
| **1.2** | **Shift attribution ignores the logged-in user.** `load_open_shift` (`sales.rs:451-471`) selects `WHERE status='open' ORDER BY opened_at DESC LIMIT 1` — **no user predicate** — and `complete_sale_transaction` stamps `shift.cashier_id` onto the sale, never reading `state.session_user_id`. The one-open-shift index is *per user*, so two staff can hold open shifts concurrently. Plus: no admin force-close (`close_shift_for_user` requires `shift.user_id == acting user`), so the owner **cannot close the shift of a cashier who went home** — and that stale shift keeps swallowing everyone else's sales. No idle lock, no fast user switch. | This is how an honest employee gets accused of theft. Owner + one cashier, one forgotten close = corrupted attribution from that moment on. | **M** | — |
| **1.3** | **No cash in/out during a shift.** `expected_cash = opening + cash_sales`, full stop (`shifts.rs:154`). The owner takes 50.000 to the bank at noon; the bread delivery is paid 800 from the till. Nowhere to record either. Combined with 1.1, the reconciliation number is **structurally guaranteed to be wrong** — and a wrong control is worse than no control, because the owner learns to ignore it. | Serbian cash cycle pushes money out of the till weekly by law (pazar to the current account) and routinely to suppliers. | **S** | — |
| **1.4** | **Authorization: the gate is on what the owner *looks at*, not what a thief *does*.** `require_admin` exists and works (`auth.rs:156-176`) and guards users/settings/reports/backup. It appears **zero times** in `catalog.rs`, `inventory.rs`, `sales.rs`, `receipts.rs`, `imports.rs`. A cashier can rewrite any price, write off stock, void any receipt from any day, and bulk-import a new price list. Worse: the acting user on a void/return/stock adjustment comes from the **client IPC payload** (`receipts.rs:117-137`, `inventory.rs:64` — and inventory's is `Option<i64>`, never validated at all). The correct pattern already exists in-repo and is simply not called. | Complete void-fraud loop: ring it, void it, pocket the cash, write off the stock. Also: no exception report exists, so the data that *is* recorded is never surfaced. | **M** | — |
| **1.5** | **No product variants (veličina × boja).** `products` is flat. One style in 6 sizes × 3 washes = 18 hand-created SKUs, each with its own name/SKU/barcode, no grouping, no matrix receiving, no matrix stock view, no matrix popis, no style rollup. `inventory_balances.product_id` is a PK, so stock is structurally one row per flat product. The importer can't even carry a size column. | **This is the product for a Novi Pazar boutique, not a feature.** UniSoft's "program za butik" leads on *vođenje zaliha po veličinama i bojama*; Logosoft SmartPOS was built with clothing retailers. A 300-style boutique becomes ~5.000 near-identical product names — which then collide with the hard **500-row product list cap** (`catalog.rs:14`) and with the broken search (0.5). *If your pilot shops are not apparel, this drops to T3.* | **L** | 0.5 (search), scale cap |
| **1.6** | **No printing of any kind.** Zero hits for `window.print`, `@media print`, jsPDF, ESC/POS, ZPL/TSPL, or any printing crate. The post-sale modal is a preview with no print action — which the **approved design spec explicitly required** (design doc :134, :322); only ESC/POS *driver commands* were out of scope. There is also no reprint, no PDF, no export of a receipt-shaped document. | Even in companion posture you need output: **shelf tags / price labels / barcode stickers** for locally-made goods that arrive with no EAN (Zakon o trgovini čl. 34 st. 8 mandates a machine-readable mark, sanction on the **seller**: ~100k RSD pravno lice / ~40k preduzetnik), and popisne liste. Note the drawer kick comes free once ESC/POS exists (`ESC p m t1 t2` over the printer's RJ11). | **M** | 1.5 (a per-model tag with no size/colour is half-useless) |
| **1.7** | **The backup switch is a lie.** `automaticBackupEnabled` is persisted and rendered — and **nothing reads it**. No scheduler, no timer, no thread, no exit hook. Failed backups are never written to `backup_jobs` (every insert hard-codes `'completed'`), so the failure UI is unreachable dead code. Default destination is `<app_data_dir>/backups`, **the same disk beside the live DB**. No rotation, no `PRAGMA integrity_check`, no file picker (hand-typed AppData path), restore doesn't reload the app, and **there is no happy-path restore test**. | Sales are recoverable from PURS. **Catalog, prices, barcodes, stock balances and movement history are not** — losing them is days-to-weeks of re-keying plus a full physical count. And you told the owner in the UI they were protected. | **S–M** | — |
| **1.8** | **You cannot support the pilot.** No CI, no logging framework at all (zero `tracing`/`log`/`tauri-plugin-log`; the entire codebase has **one** `console.warn`), no panic hook, no version displayed anywhere (`health.rs` returns `app_version`; `BackendStatus.tsx` throws it away). "It froze at 6pm during a sale" → nothing to read, and you can't even establish which build they're on. Note `docs/PROGRESS.md:104` claims "green CI" — there is no CI. Local logging was a **spec'd MVP requirement** (design doc :94, :376) that was skipped. | The entire purpose of a pilot is to surface bugs. | **S** | — |
| **1.9** | **Importer decodes UTF-8 only.** `ImportWizard.tsx:205` does `await file.text()`. Excel on a Serbian Windows exports **Windows-1250**. `Blob.text()` decodes with error mode "replace", so "Košulja" becomes "Ko\uFFFDulja" — **lossy, not mojibake**: the original byte is destroyed and the data cannot be repaired from the DB, only by re-importing the source. Credit where due: the importer already does semicolon-first delimiter detection, BOM stripping, header auto-mapping, dry-run with per-row errors, and an optional `initial_stock` column. It breaks on exactly the files Serbian shops actually possess. | Install-day is make-or-break: the market convention is *"give us your Excel with opening stock and we load it at installation."* | **S** | — |
| **1.10** | **Till ergonomics: the discount you need daily doesn't exist.** `DiscountRequest::Percent` exists in Rust (`sales.rs:33-34`) **and in the TS contract** (`types.ts:257`) — and `moneyDiscountFromInput` unconditionally returns `{type:'amount'}` (`RegisterScreen.tsx:623-627`). Told *"daj 20%"*, the cashier does arithmetic by hand. Cash field doesn't prefill to the total. No open-price / "razno" line. Preview isn't debounced so totals flicker to 0 while editing a quantity. | ~10 lines of UI for the percent discount; one line for the tender prefill. Highest value-per-hour on the list. | **S** | — |
| **1.11** | **Demo/training data is permanently mixed into real turnover.** No reset-to-clean, no "begin real trading" boundary, no training mode, no receipt-counter reset. The vendor-installs-and-trains convention *guarantees* the DB is dirty on go-live morning — and the only escape is a restore-from-backup nobody took (see 1.7). Every report and every accountant export thereafter includes the practice sales, forever. Related: **you cannot demo the product** — there's no seeded Serbian boutique catalog, so a demo means live-configuring a VAT rate in front of the customer. | **S** | 0.4 |
| **1.12** | **The UI is not actually written in Serbian.** Across all 88 files in `src/`, **not one contains š/đ/č/ć/ž**. "Podesavanja", "Kolicina", "Zavrsi prodaju". To a shop owner this reads as software written by someone who doesn't speak the language — visible in the first ten seconds, before any feature is discussed. Also: locale is applied by hand and inconsistently (`sr-Latn-RS` in one file, `sr-RS` in another, system locale in `chart.tsx`). | **S** | — |

---

### T2 — EARLY PAIN (bites within weeks)

| Gap | Why here | Effort |
|---|---|---|
| **KEP knjiga** (Pravilnik o evidenciji prometa 99/2015, 44/2018 — **not** abolished by e-fiscalization; still enforced by tržišna inspekcija, and survived the 35/2026 trade-law amendment). Zero hits repo-wide. Kept per prodajni objekat, entries by the next day, electronic allowed but printable on demand. Fine for a preduzetnik is ~**40.000 RSD**, not the 2M often quoted. | **This is what the knjigovođa asks for in week two, and the knjigovođa can veto you.** Not buildable in isolation: KEP zaduženje is at *maloprodajna cena sa PDV* derived from a kalkulacija — and `inventory_movements` has **no value/cost/price column at all** (`migrations.rs:87-97`). | **M**, blocked on kalkulacija |
| **Suppliers / goods receipt / kalkulacija.** No suppliers table, no purchase document. Receiving a 40-line delivery = 40 one-product dialogs — and the dialog **doesn't even send the purchase price** (`inventory.rs:65` accepts `purchase_price_minor`; `InventoryScreen.tsx:483-489` builds the request without it — dead code path). | The legal document that lets goods go on sale. Also the only source of COGS. And for Novi Pazar the invoice arrives **in EUR/USD/TRY from Turkey with carina + špedicija + prevoz** — so the kalkulacija must be **FX- and landed-cost-aware from the start** or it's a rewrite. | **L** |
| **Price history / nivelacija / prethodna cena.** `catalog.rs:367-419` overwrites `sale_price_minor` with a bare UPDATE; `importer.rs:732` and `inventory.rs:422` *also* silently overwrite prices/costs. No history table. | **Zakon o trgovini as amended by Sl. glasnik RS 35/2026, in force 1 May 2026, čl. 37**: when discounting you must display the **lowest price at which you offered the goods in the previous 30 days**, and prove it to inspectors. Sezonsko sniženje may only *start* 25.12–10.01 or **1–15 July** — i.e. your pilot shops are inside the window right now. Season-end markdown is the core commercial act of a boutique. **The loss is irreversible and accrues daily** — price history cannot be backfilled once never written. Also: "bulk" in CatalogModule is bulk *creation*, so a 400-SKU markdown is 400 destructive edits. | **M** |
| **Popis / inventura.** Only signed-delta corrections, one product at a time — count 17 against a book 20 and type "−3" by hand. No session, no frozen book quantity, no variance report, no manjak/višak. | Annual, as at 31.12 (Zakon o računovodstvu + Pravilnik 89/2020). **Hard deadline, not week-one pain.** But the cheap half is week-one: let `inventory_correct` accept an **absolute counted quantity** and derive the delta. Note Pravilnik čl. 8 **forbids** giving book quantities to the popis commission before actual quantities are recorded — so the count sheet must be blank-quantity. | S (absolute entry) / **L** (full session) |
| **Accountant exports are numerically wrong.** `reports.rs:752-755` writes `row.total_minor.to_string()` — a 120,00 RSD day exports as `12000`; quantities as milli-units (one pair of jeans = `1000`); **no UTF-8 BOM** so šđčćž mojibake in Excel; and `csv_line` joins with a **comma** (`reports.rs:893`) while Serbian Windows Excel expects `;`, so even ASCII collapses into one column. 4 of 7 exports have no button. Files land in a hidden folder announced only in a toast. No export contains a PDV column or receipt-level rows. | A shipped feature that emits misleading files is worse than an absent one — the owner hits it the first time they click Izvezi. Hours of work. | **S** |
| **Payment tenders.** `CHECK (payment_method IN ('cash','card'))` (`migrations.rs:133`) — law enumerates **seven** (Pravilnik 31/2021 čl. 6, reverted by 57/2022; the reduced 4-type mode survives only for on-site food service and bakeries, **not retail**). Not a one-line swap: `PaymentAllocation` is a fixed 2-arm struct, `validate_payments` has two hardcoded accumulators, `ReportsScreen.tsx:872` renders any non-cash tender as "Kartica". | **IPS QR is not theoretical**: >10M payments worth >53bn RSD in Q1 2026, and every acquiring bank must offer it to merchants. Recording an IPS payment as "cash" is a false entry in a mandatory fiscal field — and the card total won't reconcile against the bank's terminal settlement. Generating the NBS IPS QR is **free and works offline** (NBS publishes the payload spec + generator); only the "paid" confirmation needs the bank, so the honest v1 is: render QR → merchant sees the push in their bank app → clicks Confirm. | **M** |
| **Audit log + price permission.** No audit table, no logging framework, and — the sharp end — `catalog.rs` has **no `require_admin`**, so any cashier can silently rewrite any price and nothing records it. Voids, returns and stock write-offs *are* attributed (counter-documents + `inventory_movements.user_id`); catalog/settings/user/import/login events are not. | Fix order: (1) `require_admin` on price mutations — this is what competitors sell as *prava pristupa*; (2) an append-only `price_history` row, which also feeds nivelacija/KEP. A full who-did-what audit log is bloat for a two-person shop. | **S–M** |
| **Stock valuation / COGS.** No inventory value report at all. `estimated_margin_minor` (`reports.rs:539-546`) joins **live** `products.purchase_price_minor` — a single mutable scalar — so raising a purchase price today retroactively rewrites last quarter's margin. And it subtracts a **VAT-exclusive** cost ("Nabavna cena bez PDV") from **VAT-inclusive** revenue: a 1200 RSD item at 20% costing 600 shows margin 600 instead of 400 — **50% overstated on a screen that already ships**. | Cheap slice: snapshot `unit_cost_minor` on `sale_items`, and net VAT out using the already-stored `si.tax_minor`. True FIFO/as-of-date valuation needs kalkulacija. | S (fix) / **L** (real) |
| **Scale caps.** Product list hard-capped at 500 with `total = items.len()` — a 600-SKU shop is shown "500" and never told. Receipt search capped at 100, same fake total. Reports are **sync Tauri commands** (main thread) with non-sargable `substr(created_at,1,10)` predicates and correlated per-day subqueries. Measured: 0.56s at 11k sales, 3.7–7.6s at 60k. | The 500 cap is the sharp edge and it **collides directly with variants** — the moment 1.5 ships, every boutique exceeds it. Report latency is a year-two problem. | **S** (paging) |
| **Reklamacije register** (ZZP 88/2021 čl. 55): complainant's name, date, goods, fault, decision, complaint number; kept 2 years; **paper book explicitly permitted**. | Legal, but paper-solvable for 300 RSD, and no Serbian small-shop competitor ships it. Do **not** model it as an extension of returns — a reklamacija is an asynchronous multi-day case with 8/15/30-day clocks that often ends in repair/replacement/rejection and produces no sale document. Needs no customers table (free-text `complainant_name`). | **S** |
| **Evidencija prekovremenog rada.** Mandatory (**Zakon o radu čl. 55 st. 6 — daily record of OVERTIME only; Serbia imposes no general daily working-time record**). **No obrazac is prescribed** — "Pravilnik 120/2014" does not exist and was struck on 01.08.2026. Retention floor **3 years**, not 2 (the "2 years" figure was a Serbian blog error; "6 years" is Croatian law). Inspekcija rada fines **50.000–150.000 for a preduzetnik** (čl. 276 st. 1 u vezi sa tač. 1a) — but the **larger** exposure is breaching the čl. 53 overtime caps: **200.000–400.000** (čl. 274 st. 1 tač. 3). A register shift is **not** an employee's working time (two people can share a till; čl. 64 breaks), so the stored shift data is **corroborating evidence, not the record**. See [SW14-VERIFIED-RULES.md](SW14-VERIFIED-RULES.md). | Still a strong legal win, but **not** a thin report over existing data — it needs a per-(employee, date) record with the ZEOR čl. 24 hour buckets and the čl. 53 cap checks. | **M** |
| **Distance selling.** A large and growing share of Novi Pazar boutique revenue is Instagram/Viber DM orders shipped **pouzećem**. There the 14-day unconditional right of withdrawal **does** apply (unlike in-store), with a prescribed obrazac za odustanak and a mandatory refund within 14 days. No order concept, no reservation, no address, no COD reconciliation (courier remits days later, net of fee, in one lump). | The one place the app would confidently say "no return right" is the one place the law grants an unconditional one. | **L** |
| **Multi-register forward-compat.** DB path hard-wired (`state.rs:53-60`), no `terminal_id` on sales, single global receipt counter, no WAL, no single-instance guard, DEFERRED transactions. | Real multi-till is correctly out of scope. But **add `terminal_id` to `sales` and make the receipt series per-terminal now** — retrofitting a terminal dimension after live receipt data exists is a painful migration. WAL + single-instance guard is ~30 minutes. | **S** |
| **Installer / signing / updater.** No CI, no `.yml` anywhere, no signing, no updater plugin. | Signing does **not** fix SmartScreen anymore (Microsoft removed EV's instant-reputation bypass; OV/EV both still warn until hundreds of clean installs — which 2 shops will never reach). And a USB/FAT32 copy strips Mark-of-the-Web, so SmartScreen never fires on an on-site install. **Defer signing.** Patch delivery for 2 shops = AnyDesk file transfer. | **S** (defer most) |

---

### T3 — LATER

SEF e-faktura for corporate cards (real since tax periods after 31.03.2026, but **subsumed by fiscalization** — SEF v3.12+ generates the e-invoice *from the fiscal receipt* on the state portal, so no POS-side SEF API is required; just carry `buyer_id_type`/`buyer_id_value`) · exchange-as-a-primitive and store credit (Serbian law prescribes **two documents** — Refundacija then an independent Prodaja — which the existing primitives already produce) · customers / veresija (paper notebook keeps working; and note `sales.rs:529` *actively rejects* `tendered < total`, so an on-account sale is structurally impossible, not merely untracked) · merchandising reports (sell-through, ageing, dead stock, season tags) — genuinely the *analytical* differentiator, but blocked on variants · card-terminal ECR integration (proprietary per acquirer, contract required; small shops key the amount by hand — instead make the shift Z-report reconcile POS card total vs terminal batch) · production (radni nalog, normativ/BOM) and komision — real for half this market, and nobody serves it · veleprodaja documents (otpremnica, rabat, medjuskladisnica) · e-Otpremnica (B2B mandatory **1.10.2027** — design goods-receipt so it can ingest one) · AML cash cap (≥ EUR 10.000 aggregated over 12 months — matters only for the wholesale slice) · phone companion (scan-to-count for popis, "imamo li ovo u 32?" on the floor) · blind close, drawer kick · arhivska knjiga (30 April filing) · VAT-by-rate reporting (**POPDV was abolished from Jan 2026** — verify before building anything) · licensing/activation/entitlement · Cyrillic UI.

---

## 3. The strategic fork on fiscalization

Three options. This decision determines everything else in the roadmap.

### A. Certify VantumPOS as its own ESIR
Register as a *dobavljač* on the PURS sandbox portal → dev certificates → submit the technical-review application with the full ESIR self-assessment questionnaire, Serbian-language product/user/install docs, and sample receipts of every supported type with a scannable QR → technical review → administrative review → *rešenje* with an IB number, published in the Registar odobrenih elemenata. PURS has **15 days** to issue the rešenje — **but that clock only starts after the technical review passes, and there is no statutory deadline for the technical review itself.** Do not plan on 15 days end to end.

Mandatory feature set to pass (all marked *Obavezno*): printing (§10 P10 — the ESIR must issue *and print*), GTIN + scan-to-select, all 7 payment types + the reduced-mode switch, the OSNOVNI receipt matrix (Promet/Kopija/Predračun/Obuka × Prodaja/Refundacija — 8 kinds), refund referencing the original PFR number + buyer identification + a hand-signed Kopija-Refundacije, tax rates *only* from the PFR, a searchable 30-day electronic journal, item-list import/export, half-up rounding matching the L-PFR to 4 decimals.

The killer: **any change that alters functionality or the appearance of the receipt requires a new approval cycle**, and PURS spot-checks that the deployed build matches the approved model. That freezes your release cadence forever.

**Verdict: XL, months, and it constrains the product permanently. Not for the pilot.**

### B. Integrate an existing certified ESIR / L-PFR (handoff)
Two shapes, both already sold in Serbia:
- **File-drop**: Softek "ESIR Link" — *"Povežite Vaše postojeće POS/ERP rešenje sa sistemom eFiskalizacije bez potrebe za sertifikaciju vašeg softvera"*. You write a fixed-format `.txt` into an IN folder; the approved ESIR fiscalizes and prints; `.ok`/`.err` come back in OUT. https://www.softek.rs/e-fiskalizacija/esir-link-za-programere/
- **REST/JSON**: Teron (https://api.teron.rs/), and EPOS/Europos/Calculus/Octopos all publish REST interfaces for handing a sale to an approved element. Software L-PFRs (MyLPFR, L-PFR+, Master LPFR) are localhost HTTP/JSON services at ~5–10 EUR/month.

VantumPOS stays uncertified; the fiscal receipt, its PFR receipt number, and the PFR-supplied tax rates flow *back into* `sales`/`sale_items`. What is then non-negotiable: no VantumPOS-printed slip is ever handed to a customer in place of a fiscal receipt; refunds carry the original PFR reference; rates come from the PFR, not `tax_rates`.

**⚠️ THIS IS A VENDOR MARKETING CLAIM. Nobody in this research could find a PURS ruling confirming that a non-certified front-end feeding an approved ESIR is compliant.** Verify it before you bet the architecture. One phone call to `budiefiskalizovan@purs.gov.rs` and one to Softek/Teron.

**Verdict: M (weeks), IF it holds. This is the fast path to being a legal till.**

### C. Companion posture — don't be the till at all
The shop already has, and legally must have, a certified ESIR. **Sell VantumPOS as the thing the ESIR vendors are genuinely bad at**: the size×colour matrix, matrix receiving, matrix popis, labels and declarations for barcode-less locally-made goods, kalkulacija/nivelacija/KEP, stock ageing and markdown planning, production. No PURS approval, no čl. 18 vendor fine exposure, no asking the shop to abandon software it already paid for, and no double-ringing (because you never ring).

The cost: your two headline modules (`register-sales`, `receipts-returns`) become internal/back-office, not customer-facing. And you must ensure the app **cannot be mistaken for a shadow till** — no customer slip, no receipt-shaped print.

**Verdict: S (posture) — and it is the only version of this product sellable in Serbia in 2026 without months of certification work.**

---

### 🎯 RECOMMENDATION: **C now, B next, A never (or only if the product succeeds).**

**Ship the pilot in companion posture.** The shop keeps its ESIR. You take the back office. This is achievable in 6–9 weeks and it is *exactly* the founder's stated goal — "cover most of the use cases so pilot users don't suffer."

**In parallel, verify option B in week 0** (two phone calls, zero code). If the handoff is legitimate, build the adapter as sprint 4 and you become a legal till *while keeping the boutique wedge*. That is the winning combination: legal receipts via someone else's approved element + a boutique workflow nobody else has.

**Do not start option A.** You would spend months rebuilding the till layer to a fixed spec, in a market with 900+ approved elements, where the Tax Administration gives away a free basic ESIR, where the price ceiling is ~€12–13/month **including a human who answers the phone**, and where the release-approval cycle would then throttle you forever. You cannot win on "we have a till."

---

## 4. Pilot-readiness plan

### Sprint 0 — Week 0: decisions and phone calls (zero code)
1. **Verify the ESIR-handoff legality** (PURS + Softek + Teron). Highest-leverage hour you will spend all quarter.
2. **Serbian legal entity?** Every supplier in the PURS register carries a Serbian PIB/matični broj. Confirm whether a foreign entity can register at all — this gates option B *and* A. It also gates issuing a faktura (which for a preduzetnik buyer now goes over SEF), which gates getting paid.
3. **DPA + terms + privacy notice.** A lawyer, a day, a template. You cannot lawfully start a paid pilot without a written *ugovor o obradi*.
4. **Interview the two pilot shops.** Apparel or not? (decides whether variants are T1 or T3). Do they take *kapara* on made-to-order? Do they do *komision*? Do they import from Turkey? Do they sell via Instagram/pouzeće? Do they manufacture? **Who is their knjigovođa, and what does that person want?** — the accountant is the de-facto decision maker and can veto you.

### Sprint 1 — Week 1: "the app works at all" (all of T0-B + the free wins)
Seed VAT behind a *"Da li ste u sistemu PDV-a?"* question · Serbian search (SQLite `NOCASE`/ICU or a normalized `search_name` column with folded diacritics; route scanner-shaped input to a **real local exact-barcode resolver**; show a disambiguation picker instead of `items[0]`) · out-of-stock override (shop-level setting + per-sale bypass) · `embedBootstrapper` + window sizing + real product name + single-instance + WAL · `tauri-plugin-log` to appLogDir + `panic::set_hook` + version in an About dialog · importer cp1250 fallback (`encoding_rs`) + encoding selector on the preview step · percent discount + cash-field prefill · diacritics throughout the UI + one locale helper · go-live reset command.

**Everything above is S. One week, and the app stops embarrassing you.**

### Sprint 2 — Weeks 2–3: "the numbers are right"
Migrate `sale_payments.amount_minor` to allow signed/negative · write a payment row on every return/void in the original tender · introduce `partially_refunded` (better: **stop deriving money from `sales.status` entirely** and sum payments) · fix the void/return double-subtraction in reports · allow void after a partial return · derive the acting user from the session (`require_session()`), drop `user_id` from `VoidReceiptRequest`/`ReturnItemsRequest`/`InventoryAdjustmentRequest` · `require_admin` on price mutations, imports, write-offs, cross-shift voids · admin force-close of a stale shift · `cash_movements` table (in/out, reason) folded into `expected_cash` · one owner-facing exception report (voids, returns, write-offs, price overrides — by user, by day) · absolute-quantity entry on `inventory_correct` · exports: divide by 100/1000, Serbian decimal comma, U+FEFF BOM, `;` delimiter, wire the 4 orphaned buttons, reveal the folder (`tauri-plugin-opener` is already a dependency and permitted — just never called).

**Do not ship the pilot without this sprint. A return button that silently corrupts the drawer is worse than no return button — if you must ship early, *disable it*.**

### Sprint 3 — Weeks 3–6: "it's a boutique system" (skip if the pilot isn't apparel)
Style → size×colour variants (attribute sets, style-level shared data, variant-level barcode + stock — Lightspeed's model is the reference) · matrix receiving grid · matrix stock view · matrix-assisted popis · variant-aware importer · product-list paging + real `COUNT(*)` (kills the 500 cap) · **`price_history` table from day one** — this data is unrecoverable once not written.

### Sprint 4 — Weeks 6–9: legal + accountant
**If B verified:** fiscal handoff adapter → widen the payment CHECK to all 7 tenders, generalize `PaymentAllocation` (keep cash as the only change-absorbing tender), add nullable `buyer_id_type`/`buyer_id_value` (PURS šifarnik: 10 PIB, 11 JMBG, 20 lična karta, 50 corporate card), carry the PFR receipt number back onto `sales`, refunds reference it, tax rates read from the PFR. Printing arrives with it (and the drawer kick comes free).
**Either way:** label/declaration printing · kalkulacija (FX-aware) → nivelacija → KEP · backup that actually schedules, off-volume, with one tested restore · IPS QR generation + manual confirm.

### Deliberately deferred, with the honest workaround
| Deferred | Workaround during pilot | Clock |
|---|---|---|
| Fiscal receipts | **The shop's existing certified ESIR.** It already has one — legally must. | none, if you don't print a slip |
| Reklamacije register | Bound paper book. Explicitly legal (ZZP čl. 55: *"u obliku ukoričene knjige ILI u elektronskom obliku"*). ~300 RSD. | none |
| KEP / kalkulacija | The knjigovođa keeps doing it. | **Week 2** — this is when they ask, and they can kill the pilot |
| Full popis session | Paper/Excel count sheets to the accountant. Absolute-quantity correction lands in sprint 2. | **31.12** — hard |
| Store credit / exchange primitive | Return + new sale — which is the **two-document form Serbian fiscal law actually prescribes**. | none |
| Veresija | Paper notebook, unchanged. | none |
| Auto-update / code signing | AnyDesk file transfer + Next-Next-Finish. On-site USB install strips Mark-of-the-Web, so SmartScreen never fires. | ~20 shops |
| Multi-register | One till. But **add `terminal_id` + per-terminal receipt series now.** | before shop #2 |
| Returns | **NOT deferrable.** Fix it or disable the button. | — |

---

## 5. Competitor matrix

| | **VantumPOS (today)** | **UniSoft POS** | **Softkom Sors MP** | **BizniSoft** | **SKY POS** | **PURS free ESIR** | **Loyverse** |
|---|---|---|---|---|---|---|---|
| Certified ESIR (PURS register) | ❌ | ✅ | ✅ | ✅ | ✅ | ✅ | ❌ (not Serbian-legal) |
| L-PFR bundled / paired | ❌ | ✅ | ✅ | ✅ | ✅ (software LPFR) | ✅ (free V-PFR) | ❌ |
| Prints a fiscal receipt | ❌ **none at all** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ (non-fiscal) |
| 7 legal payment tenders | ❌ cash+card only | ✅ (+ ček) | ✅ | ✅ | ✅ | ✅ | ❌ |
| **Size × colour variants** | ❌ | ✅ *headline* | partial | partial | partial | ❌ | ✅ |
| **Matrix receiving / matrix popis** | ❌ | partial | partial | partial | ❌ | ❌ | ❌ |
| Works fully offline | ✅ **(genuine)** | ✅ *marketed* | ✅ | ✅ | ✅ | ⚠️ | ✅ |
| KEP auto-generated | ❌ | ✅ *"100% automatska"* | ✅ | ✅ | ✅ | ❌ | ❌ |
| Kalkulacija / nivelacija | ❌ | ✅ (+ *masovne nivelacije*) | ✅ | ✅ | ✅ | ❌ | ❌ |
| **FX / import (Turkey) landed cost** | ❌ | ❓ | ❓ | ✅ (ERP) | ❌ | ❌ | ❌ |
| Label / deklaracija printing | ❌ | ✅ | ✅ | ✅ | ⚠️ | ❌ | ✅ (labels) |
| Popis with variance | ❌ (delta only) | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| Excel import w/ opening stock | ⚠️ (breaks on cp1250) | ✅ | ✅ | ✅ | ✅ | ❌ | ✅ |
| Accountant export | ⚠️ (numerically wrong) | ✅ XML | ✅ | ✅ | ✅ | ❌ (SUF portal) | ✅ |
| Returns move money correctly | ❌ **corrupts data** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Production / komision** | ❌ | ❌ | ❌ | ✅ (ERP) | ❌ | ❌ | ❌ |
| Sell-through / ageing / markdown | ❌ | ❌ | ❌ | ⚠️ | ❌ | ❌ | ⚠️ |
| Install + training + phone support | ❌ **none** | ✅ free demo + training | ✅ **incl. 50 calls/yr** | ✅ unlimited | ✅ | ❌ *"bez podrške"* | ❌ |
| **Price** | — | on request | **€13/mo (€156/yr)** | **€390 once**, 3 PCs, support incl. | 3.490–6.990 RSD/mo | **FREE** | **FREE** |

### Where VantumPOS could actually win

**Not on being a till.** ~900+ approved elements in the register, a €12–13/month ceiling that *includes a human*, and the tax authority gives one away.

Three real wedges, in order:

1. **The boutique matrix.** Style × size × colour, with matrix *receiving* (one screen, rows=colour, cols=size — not 18 dialogs), matrix *stock*, matrix *popis*, and style-level sell-through. UniSoft mentions size/colour in a trailing promo paragraph; nobody at this price point has actually built the workflow. The one competitor that does it properly (LS Central) is enterprise-priced. **This is the product.**

2. **Labels + declarations for barcode-less local goods.** Novi Pazar shops sell garments made down the street with no EAN — and Zakon o trgovini čl. 34 st. 8 puts the machine-readable-mark obligation **on the seller**. Internal-code generation + a Zebra/TSC label with barcode, price and (where they *are* the manufacturer) fibre composition. Generic ESIR vendors treat this as an afterthought. ⚠️ Do **not** print declarations for third-party goods — that assumes manufacturer liability (EU 1007/2011 Art. 15).

3. **Import kalkulacija with FX + landed cost, and light production.** Half this market is manufacturer + wholesaler + retailer at once, buying fabric from Turkey in EUR/TRY and selling jeans they cut themselves. Every generic ESIR assumes a domestic reseller. The ERPs that handle it (Pantheon, Calculus, Wings) are too heavy and too expensive for a two-person boutique. Even a minimal *radni nalog* + *normativ* would be unmatched at this price.

Plus two structural advantages you already have and should stop hiding: **genuine offline** (the law positively rewards it — the L-PFR is *supposed* to queue offline for 5 days), and **no per-till subscription** (the loudest grievance in the market is that e-fiscalization took monthly cost from ~400 to ~1.600–2.000 RSD; BizniSoft already exploits this with €390-once + support included). Counter to the obvious objection — "if you disappear, what happens to my three years of sales?" — with a documented open SQLite schema and an export-everything guarantee. You have neither today.

---

## 6. What the research could **not** establish — go verify these yourself

Ranked by how much damage a wrong assumption does.

1. 🔴 **Is the ESIR-handoff / "ESIR Link" posture actually accepted by PURS?** Vendor marketing claim only. No PURS ruling found either way. **This single fact determines your entire fiscalization strategy.** Ask `budiefiskalizovan@purs.gov.rs` and Softek/Teron directly.
2. 🔴 **Can a non-Serbian legal entity register as a *dobavljač*?** Every supplier in the register has a Serbian PIB/matični broj. Nothing explicitly bars a foreign entity, but nothing permits one either. Gates certification *and* invoicing *and* getting paid.
3. 🔴 **Real cost and elapsed timeline for ESIR approval.** No prescribed state fee found; the 15-day clock starts only after the technical review passes, and the technical review has **no statutory deadline**. Nobody could tell us how long it really takes.
4. 🟠 **Is the "shadow till" inspection risk real?** Whether a tržišna/poreska inspector treats a parallel non-fiscal sales system beside the legal till as evidence of unrecorded turnover. Verify with a Serbian tax lawyer — the sanction is a **business ban**, not a fine.
5. 🟠 **Tehničko uputstvo version and specific citations.** v1.17 (Feb 2025) appears current, but several load-bearing questionnaire citations (§10 P7 all-payment-methods, §10 P16 30-day journal, §12 P1/P6 rates-from-PFR) could not be independently line-verified; some sources still reference v1.16, and §11 P6 is *"uvoza **ILI** izvoza"* (import **OR** export — a disjunction, not both). Pull the current PDF from purs.gov.rs before scoping certification.
6. 🟠 **POPDV appears to have been abolished** from tax period Jan-2026, replaced by a Tax-Authority-generated *preliminarna poreska prijava* (postponed to Jan-2027). If true, do not build a POPDV feed. Confirm with the pilot's knjigovođa before writing a line of VAT-report code.
7. 🟡 **Ask the two pilot shops, do not assume:** Do they take *kapara/avans*? (The PURS free ESIR — targeted at boutiques — does **not** support avans, which is strong evidence it isn't a baseline need.) Do they do *komision*? Do they import from Turkey? Do they sell via Instagram with *pouzeće*? Do they manufacture? Do they run *veresija*? Each of these swings a whole workstream.
8. 🟡 **Enforcement reality of Zakon o trgovini čl. 34 st. 8** (machine-readable mark) against small boutiques. The law is unambiguous; enforcement anecdotes are not. Determines whether label printing is a legal must or a merchandising nice-to-have.
9. 🟡 **Paušal ceiling** — one 2026 source suggested a rise from 6M to 8M RSD; could not confirm. Irrelevant anyway for retail, which is *excluded* from paušal entirely (ZPDG čl. 40, kiosks/mobile objects and own-production sellers excepted) — so **every real pilot shop keeps books and has an accountant.** Plan for that, not for a paušalac.
10. 🟡 **Customer-facing display**: could not confirm whether the pre-2022 mandate survived e-fiscalization. Circumstantial evidence says no. Don't tell a shop it can skip one until you've checked.

---

**Bottom line for the founder:** you are ~1 week from an app that doesn't embarrass you, ~3 weeks from an app whose numbers are trustworthy, and ~6–9 weeks from a boutique system a Novi Pazar shop would genuinely want — **provided you stop trying to be the till.** Make two phone calls this week (PURS on the handoff, a lawyer on the DPA), then build in the order above.