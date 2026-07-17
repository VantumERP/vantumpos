# Campaign Evidence & Labels (SW-6c) — Design

**Date:** 2026-07-17
**Status:** Approved direction (HTML-files-only; in-app print view deferred to SW-8-proper)
**Legal authority:** `docs/ZOT-36-37-VERIFIED-RULES.md` — §4.4 (what evidence the trader must produce), §4.6 (tamper-resistance + print duties), §5.1 (Output). This spec encodes that memo; where they could disagree, the memo wins.
**Builds on:** SW-6a (price_history, `compute_prethodna_cena`), SW-6b (campaigns/campaign_items with frozen anchors, lifecycle).

## Scope

The evidentiary/output layer for campaigns, as **self-contained `.html` files** written to the existing `exports/` directory:
1. **Inspector evidence export** — the walk-away čl. 48 artifact proving each campaign's prethodna cena from the underlying price rows.
2. **Shelf-label sheet** — printable labels branching on type + display mode (čl. 37 st. 2/11, čl. 36 st. 3).
3. **Correction report** — on-screen + exportable: which active-campaign articles need which label content, flagging anchors that need human attention.

**Deferred to SW-8-proper:** the in-app print view (`window.print()` + `@media print` CSS) and any physical-printer path — their rendering fidelity cannot be verified without booting the app, so they are out of this cycle. The HTML files are printable today via any browser's Ctrl+P; that is the user's manual path until SW-8.

**No new legal research:** the export format is not statutorily prescribed (memo §5.1: the offered-price log is "free-form; its only legal job is discharging the evidentiary burden"), and the user's standing decision (from 6a) is to design against the statute and skip the inspectorate inquiry. Memo §6.9 (a future control list could define exactly what inspectors demand) remains an open flag; because the underlying *data* is complete regardless of presentation, a later format change is low-cost rework.

## Locked decisions

| Decision | Choice |
|---|---|
| Medium | **Self-contained HTML files** to `exports/`, returning the existing `ExportedFile` shape. No new dependency; native šđčćž. |
| In-app print view | **Deferred to SW-8-proper** (unverifiable without a boot). |
| Correction report framing | **Honest:** lists every active-campaign article with its required label content and flags manual/truncated/incomputable anchors — never claims to know which physical tags are currently wrong. |
| Privacy | The evidence file exposes **only** the campaign's own articles, their anchors, and their supporting price rows. No employee data, no unrelated sales, no other campaigns. |

## 1. Domain module — `src-tauri/src/campaign_evidence.rs`

Pure assembly + rendering; no `State`, no auth (the command layer gates). Two responsibilities kept separate so each is independently testable.

### 1.1 Assembly

```rust
pub struct EvidenceItem {
    pub product_id: i64,
    pub product_name: String,
    pub sku: String,
    pub campaign_price_minor: i64,
    pub anchor_status: String,               // "computed" | "manual" | "none"
    pub prethodna_cena_minor: Option<i64>,
    pub anchor_window_days: Option<i64>,
    pub anchor_truncated: bool,
    pub anchor_reason: Option<String>,
    pub anchor_justification: Option<String>,
    pub future_regular_price_minor: Option<i64>,
    pub window_from: Option<String>,         // recomputed in Rust: starts_on − window_days
    pub window_to: Option<String>,           // = starts_on
    pub supporting_rows: Vec<SupportingRow>, // the offered-price rows that justify the MIN
}

pub struct SupportingRow { pub effective_from: String, pub price_minor: i64 }

pub struct CampaignEvidence {
    pub campaign_id: i64,
    pub campaign_type: String,
    pub status: String,
    pub starts_on: String,
    pub ends_on: Option<String>,
    pub display_mode: String,
    pub headline_percent: Option<i64>,
    pub rasprodaja_ground: Option<String>,
    pub special_conditions: Option<String>,
    pub reduced_utility_reason: Option<String>,
    pub marketing_label: Option<String>,
    pub season_attested: bool,
    pub separation_attested: bool,
    pub activated_at: Option<String>,
    pub ended_at: Option<String>,
    pub items: Vec<EvidenceItem>,
}

pub fn assemble_evidence(conn: &Connection, campaign_id: i64) -> Result<CampaignEvidence, AppError>
```

Per item, by `anchor_status`:
- **computed** → `window_to = starts_on`; `window_from = starts_on − anchor_window_days` (calendar, in Rust). `supporting_rows` = the `price_history` rows for that product whose validity interval (same `LEAD` timeline as `compute_prethodna_cena`) overlaps `[window_from, window_to)` **and** carry a non-NULL price — the exact offered prices from which the MIN was taken. This is the isprava trail: an inspector sees the number *and* its justification.
- **manual** → no window, no supporting rows; carry `prethodna_cena_minor` + `anchor_justification` + `anchor_reason` (perishable / incomputable). The evidence states the value was merchant-entered and why the computation method did not apply.
- **none** (promotivna) → no prethodna cena; carry `future_regular_price_minor`. Evidence states it is a first-introduction promotivna prodaja (čl. 36 st. 9), which carries no prethodna cena by law.

**Privacy scoping is structural:** the query joins only `campaigns`, `campaign_items`, `products`, and `price_history` filtered to this campaign's product ids. No `users`, `sales`, `sale_items`, `shifts`, or other campaigns are touched. A test asserts an unrelated product and an employee username never appear in the rendered file.

### 1.2 Rendering (each returns a self-contained HTML `String`)

```rust
pub fn render_evidence_html(evidence: &CampaignEvidence) -> String
pub fn render_labels_html(evidence: &CampaignEvidence) -> String
pub fn render_correction_html(report: &CorrectionReport) -> String
```

All three: `<!doctype html>` + inline `<style>` (print-friendly `@page`/`@media print`, but no interactivity), UTF-8, HTML-escaped dynamic values (a small private `escape_html` — product names may contain `<`, `&`, `"`). **None carries a QR, PIB, or PFR/brojač block** (SW-1 design rule); a footer marks each as an internal evidence/label document, not a fiscal receipt.

- **Evidence** — campaign header (type in Serbian, dates, ground, attestations, marketing label), then a per-article section: snižena cena, prethodna cena with the **stav that matches the window** („čl. 37 st. 3" for a 30-day window, „st. 4" for a shorter one — the 6a citation fix), window dates, and a table of the supporting offered-price rows. Manual/none items render their respective statements. Header line: „Prethodna cena utvrđena prema čl. 37 Zakona o trgovini."
- **Labels** — a grid of per-article label cards branching on the campaign:
  - default → **two prices**: prethodna (struck-through) + snižena (čl. 37 st. 2);
  - akcijska with `display_mode == "percentage"` (declared ≤3d) → **percentage only**, computed from the frozen anchor `round((anchor − price) / anchor * 100)` (čl. 37 st. 11's mandatory substitution);
  - rasprodaja → the literal `dok traju zalihe` in place of an end date (čl. 36 st. 2 t. 3);
  - reduced-utility reason present → printed on the label (čl. 36 st. 3);
  - promotivna → intro price + the „nova cena po isteku" (future regular), no prethodna cena.
- **Correction** — a table of the active-campaign articles with their required label content and an „Napomena" column carrying the attention flag.

### 1.3 Correction report

```rust
pub struct CorrectionRow {
    pub campaign_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub sku: String,
    pub campaign_type: String,
    pub display_mode: String,
    pub campaign_price_minor: i64,
    pub prethodna_cena_minor: Option<i64>,
    pub needs_attention: bool,               // manual | truncated | incomputable(none-when-required)
    pub attention_reason: Option<String>,
}
pub struct CorrectionReport { pub rows: Vec<CorrectionRow> }
pub fn assemble_correction_report(conn: &Connection) -> Result<CorrectionReport, AppError>
```

Covers all **active** campaigns' items. `needs_attention` = the anchor is `manual`, or `anchor_truncated`, or an incomputable case that forced manual entry — i.e. the label content a human should double-check. It never asserts a physical tag is wrong (we cannot observe the shelf); it lists what each label must say and which ones warrant a second look.

## 2. Commands — `commands/campaigns.rs` (extend), all `require_admin`

Reuse `reports::ExportedFile`. Export dir resolved as `reports_export_csv` does (`state.db().path().parent() … .join("exports")`). File names: `dokaz-cene-kampanja-{id}.html`, `etikete-kampanja-{id}.html`, `ispravke-etiketa.html`. `mime_type: "text/html"`.

- `campaigns_export_evidence(campaign_id: i64) -> ExportedFile`
- `campaigns_export_labels(campaign_id: i64) -> ExportedFile`
- `campaigns_correction_report() -> CorrectionReport` (on-screen data)
- `campaigns_export_correction_report() -> ExportedFile`

Register all four in `lib.rs`.

## 3. Frontend

`CampaignsService` gains the four methods (camelCase DTOs, invoke 1:1). On the **campaign detail** view (SW-6b): an „Dokazi i etikete" group with „Izvezi dokaz o ceni" and „Izvezi etikete" buttons, each showing the saved path on success (mirror `ReportsScreen`'s CSV pattern — the returned `path` shown as confirmation). A new **„Ispravke etiketa"** panel on the Campaigns screen renders `campaigns_correction_report()` as a table (article, type, required prices, attention flag) with an „Izvezi" button. No print view, no `window.print()`.

## 4. Testing

- **Assembly:** worked-example (b) campaign (jacket well-established in assortment → st. 3, 30-day window) → assert the evidence item carries `prethodna_cena_minor == 1190000` and `window_days == 30`, and that the supporting rows include the 11.900 and 12.900 offered prices that fall inside `[2026-06-05, 2026-07-05)` and **exclude** any row outside it. Promotivna (c1) → `anchor_status == "none"`, `future_regular_price_minor` present, no supporting rows.
- **Evidence HTML:** contains the campaign type, both prices, the correct stav (st. 3 for 30-day, st. 4 for a 22-day case), the supporting-row table; contains no `<script`, no QR/PIB; **privacy** — seed an unrelated product + an employee, assert neither the product name nor the username appears in the output.
- **Labels HTML:** two-price by default; percentage-only for a declared ≤3d akcija (assert the computed % and the *absence* of the two prices); `dok traju zalihe` for rasprodaja; reduced-utility reason printed when set; promotivna shows the future price and no prethodna cena.
- **Correction report:** active campaigns only (a draft/ended campaign's items absent); `needs_attention` true for a manual/truncated anchor, false for a clean computed one.
- **Commands:** admin-gate rejection (`forbidden` for a cashier); a happy path asserts the file exists at the returned path and its bytes contain the campaign marker.
- **Frontend:** buttons call the service and surface the path; the correction table renders rows and the attention flag; RTL only (no print assertions).
- **HTML-escaping:** a product named `A & <b>` round-trips escaped in every renderer.

## Non-goals
- In-app print view, `window.print()`, `@media print` fidelity, physical-printer integration (→ SW-8-proper).
- PDF generation.
- KEP / popis / year-end-close documents (separate P1 items; SW-8 is their shared prerequisite too).
- Any change to campaign computation or the frozen anchor (read-only over 6b's data).
- Multi-store scoping.

## Acceptance criteria
- Each campaign yields a self-contained evidence `.html` proving its prethodna cena from the actual offered-price rows, citing the stav that matches the window, exposing no unrelated or personal data.
- The label sheet renders the legally-correct content per type/display-mode (two prices / percentage / „dok traju zalihe" / reduced-utility reason / promotivna future price).
- The correction report lists active-campaign label content and flags anchors needing attention, without claiming to know the physical shelf state.
- No document resembles a fiscal receipt.
- All gates green: `bun run test`, `bun run build`, `cargo test -- --test-threads=1`, `cargo clippy --all-targets --all-features --locked -- -D warnings`, `cargo fmt --check`, `git diff --check`.
