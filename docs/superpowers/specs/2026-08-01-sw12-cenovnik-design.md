# SW-12 — Mašinski čitljiv cenovnik — Design

**Date:** 2026-08-01 · **Status:** proposed · **Verified law:** [REMAINING-SW-VERIFIED-RULES.md](../../REMAINING-SW-VERIFIED-RULES.md) §2b, §3 V2, §4 reqs. 10–18

---

## 0. The one decision this design does not make

Requirement 15 says to **ship a hosted cenovnik endpoint** — a stable public URL per prodajni objekat — because a trader who wants to comply cannot without one.

**That is a founder decision, not an engineering one.** VantumPOS is local-first; its only network touch today is the optional Open Food Facts lookup. A hosted endpoint means Actaer runs a public web service, with the hosting cost, the uptime expectation, the TLS certificate, and — because the file carries the shop's prices — a new processing relationship to paper.

So this design builds **everything that is local**, and models publication as a **pluggable target** with an honest "not configured" state:

| Layer | This cycle | Decision needed |
|---|---|---|
| Generate the file (correct columns, correct format) | ✅ build | — |
| Immutable dated archive of published snapshots | ✅ build | — |
| Republish on every price write | ✅ build | — |
| Till-side price-integrity guard | ✅ build | — |
| Write to a local folder / manual upload | ✅ build | — |
| **Push to a hosted URL** | interface only, no implementation | **founder: does Actaer host this?** |

A shop that already has a website can satisfy čl. 6 today with the generated file and its own hosting. A shop that does not is in the **unresolved** case (§2b) and must not be told it is in breach.

---

## 1. What the file must contain (req. 10)

**Not only `prodajna cena`.** Čl. 6 st. 2's second sentence pulls st. 1 into the published file, so it must carry **`jedinična cena` and `jedinica mere`** as well. Audit this against the current catalog schema first — a product priced per piece still needs a unit price expressed per the statutory measure.

Per prodajni objekat, one file. Columns (req. 16 — de facto data.gov.rs shape, since **no format is legally mandated** and the čl. 6 st. 7 bylaw does not exist):

`sifra ; barkod ; naziv ; jedinica_mere ; prodajna_cena ; jedinicna_cena ; jedinica_za_jedinicnu_cenu ; datum_azuriranja`

UTF-8 **with BOM**, `;` separator, `DD-MM-YYYY` dates, 2-decimal prices with `.`, 13-digit barcode **as text** (so a spreadsheet cannot eat the leading zero). **The column mapping lives in configuration**, and a schema swap is a routine release, not a migration.

---

## 2. Republish on write, never on a timer (req. 11)

Publication is driven off the same price table the till reads, **on write**. A price change republishes.

A nightly batch is **a defect against čl. 6 st. 3**, not a simplification — st. 3 requires the published file to match current prices *"u realnom vremenu"*. The existing `price_history` table (migration v9) already records every offered-price change with its `effective_from`, so the trigger point exists; SW-12 hangs off it rather than inventing a second source of truth.

---

## 3. The published-price archive (req. 14)

Čl. 6 st. 5 requires the trader to **enable comparison of *prethodno objavljene cene* against *cene objavljene u realnom vremenu***. Prior published snapshots must therefore remain retrievable — not overwritten.

**Immutable dated snapshots plus a current pointer.** Retention floor tied to the čl. 213 two-year limitation, via the shared `retention_policies` table.

**This is uncosted in the original SW-12 estimate.** The roadmap called SW-12 an "S" because it was read as a CSV dump; the archive alone makes it an M.

---

## 4. Till-side price-integrity guard (req. 12)

Refuse-or-loudly-warn when an operator rings an item **above** the last-published price for that outlet, and log the divergence. Čl. 6 st. 4 binds a trader *who publishes* to adhere to the published prices, so this is the cheapest way to keep the shop out of čl. 207/206 territory.

Below-published is fine — a discount is not a breach. **Above-published is the only direction that matters**, which also makes the guard cheap: one comparison against the current snapshot, no history walk.

Warn, do not hard-block: the register must be able to record what actually happened, and a hard block at the till over a stale snapshot would be worse than the exposure it prevents.

---

## 5. Fetchability rules (req. 13)

Čl. 6 st. 5 is a duty to **enable**. Whatever target is eventually configured, the generated artefact and any publishing adapter must not defeat it:

- no login, no session, no CAPTCHA
- no JS-rendering requirement — a plain file at a plain URL
- no `robots.txt` disallow, no bot-fight rule, no aggressive rate limit
- stable URL per prodajni objekat
- correct `Content-Type`

Putting the cenovnik behind bot protection would **manufacture a breach for our own customer**. This is written into the adapter contract as a documented constraint, and the local-folder adapter satisfies it trivially.

---

## 6. Copy rules (req. 18, §2b)

- Preduzetnik exposure is **fixed 100.000 (čl. 210 st. 3)**, halved to 50.000 on payment within 8 days of a prekršajni nalog. **The literal `200.000` must not be reachable as a preduzetnik's čl. 6 exposure** — a CI guard mirroring SW-15's, in `legal.rs`.
- **Do not claim the customer has been exposed since May.** Čl. 210 was not in the čl. 220 carve-out; only čl. 4 st. 1 and čl. 6 apply from 1 May 2026.
- **Describe the capability, never assert the customer is currently in breach** (§2b). The no-website case is genuinely unresolved and extending a prekršaj to an unwritten duty to create a website runs into lex certa (ZoP čl. 3).
- Document in the dossier that **čl. 6 st. 6's prescribed standard does not yet exist**, and set a monitoring trigger on Sl. glasnik for *cenovnik* / *zaštita potrošača* through 01.05.2027 (req. 17). Label the data.gov.rs shape **practice, not law**.

---

## 7. What must NOT be built

1. No nightly/batch republish.
2. No login, CAPTCHA, JS requirement or bot-blocking on the published artefact.
3. No claim that the shop is in breach today, in product copy, onboarding or marketing.
4. No `200.000` reachable as a preduzetnik figure.
5. No hard block at the till on a price divergence.
6. No hosted endpoint implementation until the founder decides — interface only.
7. No format validator against a standard that does not exist.

---

## 8. Testing

**Rust:** the file carries `jedinicna_cena` and `jedinica_mere` for every row; a price write republishes and stamps a new snapshot; the previous snapshot remains retrievable; the till guard fires above the published price and stays silent below it; the divergence is logged; the CSV is UTF-8 with BOM, `;`-separated, with the barcode quoted as text; retention resolves through the shared table; `legal.rs` renders the preduzetnik tier and `200.000` is unreachable under that profile.

**Frontend:** the settings panel states plainly that no publishing target is configured and what that means; the copy never asserts current breach; the archive is browsable by date.

---

## 9. Open items

| # | Item | Handling |
|---|---|---|
| §2b | Does a website-less trader have to create a site? | **UNRESOLVED.** Build the capability; never assert breach. Founder question for counsel. |
| req. 15 | Does Actaer host the endpoint? | **Founder decision.** Interface shipped, implementation deferred. |
| req. 17 | The čl. 6 st. 7 bylaw | Does not exist. Monitor Sl. glasnik through 01.05.2027. |
