# Campaign Evidence & Labels (SW-6c) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce, per campaign, self-contained HTML files — a walk-away inspector evidence document proving each prethodna cena from the actual offered-price rows, a shelf-label sheet branching on type/display-mode, and an on-screen-plus-exportable correction report.

**Architecture:** A new pure module `src-tauri/src/campaign_evidence.rs` assembles a campaign's evidentiary record (read-only over 6a/6b data) and renders it to self-contained HTML strings. Four admin-gated commands in `commands/campaigns.rs` write those strings to the existing `exports/` dir (reusing `reports::ExportedFile`) or return structured data. The frontend adds export buttons and a correction panel — no `window.print()`, no print CSS (deferred to SW-8-proper).

**Tech Stack:** Rust + rusqlite (window functions), `time` (RFC3339 date math), Tauri v2 commands; React + TypeScript + shadcn/ui + Vitest/RTL.

## Global Constraints

- **Legal authority is `docs/ZOT-36-37-VERIFIED-RULES.md`**; design is `docs/superpowers/specs/2026-07-17-campaign-evidence-design.md`. If code and those documents disagree, STOP and report — never improvise law.
- **DO NOT boot, launch, or run the application** (no `tauri dev`, dev/preview server, or built binary). Verify only via `cargo test` / `cargo build` / `bun run test` / `bun run build` / clippy / fmt. Standing user instruction.
- **Read-only over campaign/price data.** This cycle changes no computation, no anchor, no lifecycle. It only reads `campaigns`, `campaign_items`, `products`, `price_history`.
- **Privacy:** the evidence file must expose only the campaign's own articles, their anchors, and their supporting price rows. It must never touch `users`, `sales`, `sale_items`, `shifts`, or other campaigns. A test pins that an unrelated product name and an employee username are absent from the output.
- **No fiscal-receipt resemblance:** no QR, no PIB, no PFR/brojač block in any document (SW-1 design rule).
- **Citation follows the window:** a 30-day window cites „čl. 37 st. 3"; a shorter window cites „st. 4" (the fix already applied in 6a's catalog advisory). Never hardcode one stav.
- **Dynamic values are HTML-escaped** (product names may contain `<`, `&`, `"`). Timestamps are RFC3339; date math in Rust via `time`.
- Money in integer minor units (para): 11.900,00 RSD = `1190000`. Never floats. Percentages via integer floor division (understates the discount — the legally safe direction).
- Serbian Latin copy with correct diacritics (šđčćž) — exact strings given below; copy character-for-character.
- Commands `require_admin`; export dir resolved as `reports_export_csv` does.
- Every task ends green on: `bun run test`, `bun run build`, `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`.
- Commit trailer on every commit:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  ```

---

### Task 1: `campaign_evidence.rs` — evidence assembly

**Files:**
- Create: `src-tauri/src/campaign_evidence.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod campaign_evidence;` after `mod campaigns;`, alphabetical)
- Test: `src-tauri/src/campaign_evidence.rs` (`mod tests`)

**Interfaces:**
- Consumes: the v10 schema (6b); `price_history` (6a); `AppError`; `time`.
- Produces (used by Tasks 2–5 verbatim):

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupportingRow { pub effective_from: String, pub price_minor: i64 }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceItem {
    pub product_id: i64,
    pub product_name: String,
    pub sku: String,
    pub campaign_price_minor: i64,
    pub anchor_status: String,
    pub prethodna_cena_minor: Option<i64>,
    pub anchor_window_days: Option<i64>,
    pub anchor_truncated: bool,
    pub anchor_reason: Option<String>,
    pub anchor_justification: Option<String>,
    pub future_regular_price_minor: Option<i64>,
    pub window_from: Option<String>,
    pub window_to: Option<String>,
    pub supporting_rows: Vec<SupportingRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
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

pub fn assemble_evidence(conn: &rusqlite::Connection, campaign_id: i64) -> Result<CampaignEvidence, AppError>
```

Assembly rules:
- Load the campaign row (all columns above); missing id → `AppError::not_found("Kampanja nije pronađena.")`.
- Load `campaign_items` joined to `products` (name, sku), ordered by `campaign_items.id`.
- Per item, by `anchor_status`:
  - `"computed"`: `window_to = starts_on`; `window_from` = `starts_on − anchor_window_days` calendar days (parse RFC3339 with `time`, subtract `Duration::days`, reformat). `supporting_rows` = offered rows overlapping the window (query below).
  - `"manual"`: `window_from/to = None`, `supporting_rows = vec![]`; keep `prethodna_cena_minor`, `anchor_justification`, `anchor_reason`.
  - `"none"`: `prethodna_cena_minor = None`, no window, no rows; keep `future_regular_price_minor`.

Supporting-rows query (the same overlap predicate `compute_prethodna_cena` uses in `price_history.rs`, minus the MIN):

```rust
let mut stmt = conn.prepare(
    "WITH timeline AS (
        SELECT price_minor,
               effective_from AS valid_from,
               LEAD(effective_from) OVER (
                   PARTITION BY product_id ORDER BY effective_from, id
               ) AS valid_to
        FROM price_history
        WHERE product_id = ?1
     )
     SELECT valid_from, price_minor
     FROM timeline
     WHERE price_minor IS NOT NULL
       AND valid_from < ?3
       AND (valid_to IS NULL OR valid_to > ?2)
     ORDER BY valid_from",
)?;
```
(`?1` product_id, `?2` window_from, `?3` window_to.)

- [ ] **Step 1: Write the failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{test_database_path, Db};
    use rusqlite::{params, Connection};

    fn with_db(test_name: &str, test: impl FnOnce(&Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("db init");
            let conn = db.open().expect("open");
            conn.execute_batch(
                "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                 VALUES (1, 'PDV 20', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');",
            )
            .expect("seed tax");
            test(&conn);
        }
        std::fs::remove_file(&path).expect("cleanup");
    }

    fn seed_product(conn: &Connection, id: i64, name: &str, price: i64, created_at: &str) {
        conn.execute(
            "INSERT INTO products (id, name, sku, sale_price_minor, purchase_price_minor,
                                   tax_rate_id, minimum_stock_milli, active, created_at, updated_at)
             VALUES (?1, ?2, ?2, ?3, 0, 1, 0, 1, ?4, ?4)",
            params![id, name, price, created_at],
        )
        .expect("seed product");
    }

    // Worked example (b): jacket established in assortment -> st. 3, 30-day window,
    // anchor 11.900 taken from offered rows inside [2026-06-05, 2026-07-05).
    #[test]
    fn assembles_computed_anchor_with_supporting_rows_in_window() {
        with_db("evidence_computed", |conn| {
            seed_product(conn, 1, "Jakna", 1290000, "2026-05-15T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at) VALUES
                 (1, '2026-05-15T00:00:00Z', 1290000, 'create', '2026-05-15T00:00:00Z'),
                 (1, '2026-06-21T00:00:00Z', 1190000, 'update', '2026-06-21T00:00:00Z'),
                 (1, '2026-07-01T00:00:00Z', 1290000, 'update', '2026-07-01T00:00:00Z');
                 INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode,
                                        season_attested, activated_at, created_at, updated_at)
                 VALUES (1, 'sezonsko_snizenje', 'active', '2026-07-05T00:00:00Z', '2026-09-02T00:00:00Z',
                         'two_prices', 1, '2026-07-05T00:00:00Z', '2026-07-04T00:00:00Z', '2026-07-05T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor,
                                             prethodna_cena_minor, anchor_status, anchor_window_days)
                 VALUES (1, 1, 990000, 1190000, 'computed', 30);",
            )
            .expect("seed campaign");

            let evidence = assemble_evidence(conn, 1).expect("assemble");
            assert_eq!(evidence.campaign_type, "sezonsko_snizenje");
            let item = &evidence.items[0];
            assert_eq!(item.prethodna_cena_minor, Some(1190000));
            assert_eq!(item.anchor_window_days, Some(30));
            assert_eq!(item.window_from.as_deref(), Some("2026-06-05T00:00:00Z"));
            assert_eq!(item.window_to.as_deref(), Some("2026-07-05T00:00:00Z"));
            // The 11.900 (from 21.06) and 12.900 (from 01.07 and the pre-window
            // 15.05 interval that overlaps) are in; nothing outside the window leaks.
            let prices: Vec<i64> = item.supporting_rows.iter().map(|row| row.price_minor).collect();
            assert!(prices.contains(&1190000), "the MIN's source row must be present: {prices:?}");
            assert!(item.supporting_rows.iter().all(|row| row.effective_from.as_str() < "2026-07-05T00:00:00Z"));
        });
    }

    #[test]
    fn manual_anchor_carries_justification_and_no_rows() {
        with_db("evidence_manual", |conn| {
            seed_product(conn, 1, "Jogurt", 50000, "2026-07-01T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode, created_at, updated_at)
                 VALUES (1, 'akcijska_prodaja', 'draft', '2026-07-10T00:00:00Z', '2026-07-20T00:00:00Z', 'two_prices', '2026-07-09T00:00:00Z', '2026-07-09T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor,
                                             prethodna_cena_minor, anchor_status, anchor_reason, anchor_justification)
                 VALUES (1, 1, 45000, 48000, 'manual', 'perishable', 'Cena sa police');",
            )
            .expect("seed");
            let evidence = assemble_evidence(conn, 1).expect("assemble");
            let item = &evidence.items[0];
            assert_eq!(item.anchor_status, "manual");
            assert_eq!(item.prethodna_cena_minor, Some(48000));
            assert_eq!(item.anchor_justification.as_deref(), Some("Cena sa police"));
            assert!(item.window_from.is_none());
            assert!(item.supporting_rows.is_empty());
        });
    }

    // Worked example (c1): promotivna -> no anchor, future price present.
    #[test]
    fn promotivna_has_no_anchor_and_carries_future_price() {
        with_db("evidence_promotivna", |conn| {
            seed_product(conn, 2, "Patike", 890000, "2026-07-01T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode, created_at, updated_at)
                 VALUES (1, 'promotivna_prodaja', 'draft', '2026-07-01T00:00:00Z', '2026-08-29T00:00:00Z', 'two_prices', '2026-06-30T00:00:00Z', '2026-06-30T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor,
                                             anchor_status, future_regular_price_minor)
                 VALUES (1, 2, 890000, 'none', 1090000);",
            )
            .expect("seed");
            let evidence = assemble_evidence(conn, 1).expect("assemble");
            let item = &evidence.items[0];
            assert_eq!(item.anchor_status, "none");
            assert!(item.prethodna_cena_minor.is_none());
            assert_eq!(item.future_regular_price_minor, Some(1090000));
            assert!(item.supporting_rows.is_empty());
        });
    }

    #[test]
    fn missing_campaign_is_not_found() {
        with_db("evidence_missing", |conn| {
            let error = assemble_evidence(conn, 999).expect_err("missing");
            assert_eq!(error.code(), "not_found");
        });
    }
}
```

- [ ] **Step 2: Run to verify failure** → `cargo test --manifest-path src-tauri/Cargo.toml campaign_evidence -- --test-threads=1` → FAIL.
- [ ] **Step 3: Implement** `assemble_evidence` + structs; declare `mod campaign_evidence;` in lib.rs. Window math uses `time` exactly as `price_history.rs`'s `parse_rfc3339` + `Duration::days`.
- [ ] **Step 4: Run tests** → PASS. **Step 5: Full gates** → PASS.
- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/campaign_evidence.rs src-tauri/src/lib.rs
git commit -m "feat(campaigns): evidence assembly — anchors + supporting price rows (SW-6c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: Evidence HTML renderer

**Files:**
- Modify: `src-tauri/src/campaign_evidence.rs`
- Test: `src-tauri/src/campaign_evidence.rs` (`mod tests`)

**Interfaces:**
- Consumes: `CampaignEvidence` (Task 1).
- Produces: `pub fn render_evidence_html(evidence: &CampaignEvidence) -> String`; private `fn escape_html(&str) -> String`; private `fn format_rsd_minor(i64) -> String` (e.g. `1190000 → "11.900,00"`); private `fn stav_for_window(window_days: Option<i64>) -> &'static str` (`Some(30) → "3"`, other `Some(_) → "4"`, `None → "3"`).

Requirements:
- `<!doctype html>`, `<meta charset="utf-8">`, inline `<style>` (simple print-friendly layout; `@page { margin: 1cm }`). No `<script>`. No QR/PIB/brojač.
- Header: campaign type in Serbian (`Rasprodaja`/`Sezonsko sniženje`/`Akcijska prodaja`/`Promotivna prodaja`), the line `Prethodna cena utvrđena prema čl. 37 Zakona o trgovini.`, dates (`ends_on` NULL → `dok traju zalihe`), marketing label + ground + attestations when present.
- Per item: name/sku, snižena cena, and by status:
  - computed → `Prethodna cena: {fmt} (čl. 37 st. {stav}, period {window_from:date}–{window_to:date})` + a `<table>` of supporting rows (date, price); if `anchor_truncated`, the line `Napomena: evidencija ne pokriva ceo referentni period.`
  - manual → `Prethodna cena (ručni unos): {fmt} — {justification}`
  - none → `Promotivna prodaja — roba se prvi put uvodi u ponudu; nema prethodne cene (čl. 36 st. 9). Redovna cena po isteku: {future}.`
- Footer: `Interni dokaz o formiranju cene. Nije fiskalni dokument.`

- [ ] **Step 1: Write the failing tests** (reuse Task 1's fixtures/helpers):

```rust
    #[test]
    fn evidence_html_shows_prices_stav_and_supporting_rows() {
        with_db("render_evidence_computed", |conn| {
            // ...same seed as assembles_computed_anchor_with_supporting_rows_in_window...
            let evidence = assemble_evidence(conn, 1).expect("assemble");
            let html = render_evidence_html(&evidence);
            assert!(html.starts_with("<!doctype html>"));
            assert!(html.contains("Sezonsko sniženje"));
            assert!(html.contains("čl. 37 st. 3"), "30-day window cites st. 3");
            assert!(html.contains("11.900,00")); // prethodna
            assert!(html.contains("9.900,00"));   // snižena
            assert!(!html.contains("<script"));
            assert!(!html.to_lowercase().contains("fiskalni račun") || html.contains("Nije fiskalni dokument"));
        });
    }

    #[test]
    fn evidence_html_cites_st4_for_a_short_window() {
        with_db("render_evidence_st4", |conn| {
            seed_product(conn, 1, "Majica", 249000, "2026-06-25T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at) VALUES
                 (1, '2026-06-25T00:00:00Z', 249000, 'create', '2026-06-25T00:00:00Z');
                 INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode, created_at, updated_at)
                 VALUES (1, 'akcijska_prodaja', 'active', '2026-07-17T00:00:00Z', '2026-07-20T00:00:00Z', 'two_prices', '2026-07-16T00:00:00Z', '2026-07-16T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor, prethodna_cena_minor, anchor_status, anchor_window_days)
                 VALUES (1, 1, 200000, 229000, 'computed', 22);",
            ).expect("seed");
            let html = render_evidence_html(&assemble_evidence(conn, 1).expect("assemble"));
            assert!(html.contains("čl. 37 st. 4"), "22-day window must cite st. 4, not st. 3");
        });
    }

    // Privacy (čl. 48 walk-away copy): the file must carry ONLY this campaign's
    // own data — never an unrelated product, never an employee username.
    #[test]
    fn evidence_html_excludes_unrelated_and_personal_data() {
        with_db("render_evidence_privacy", |conn| {
            seed_product(conn, 1, "Jakna", 1290000, "2026-05-15T00:00:00Z");
            // An unrelated product and an employee that must NOT leak.
            seed_product(conn, 2, "TAJNI-ARTIKAL", 500000, "2026-01-01T00:00:00Z");
            conn.execute(
                "INSERT INTO users (username, display_name, role, created_at, updated_at)
                 VALUES ('tajni_kasir', 'Tajni Kasir', 'cashier', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
                [],
            )
            .expect("seed user");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-05-15T00:00:00Z', 1290000, 'create', '2026-05-15T00:00:00Z');
                 INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode, created_at, updated_at)
                 VALUES (1, 'akcijska_prodaja', 'active', '2026-07-17T00:00:00Z', '2026-07-20T00:00:00Z', 'two_prices', '2026-07-16T00:00:00Z', '2026-07-16T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor, prethodna_cena_minor, anchor_status, anchor_window_days)
                 VALUES (1, 1, 990000, 1290000, 'computed', 30);",
            )
            .expect("seed");
            let html = render_evidence_html(&assemble_evidence(conn, 1).expect("assemble"));
            assert!(html.contains("Jakna"), "the campaign's own article must appear");
            assert!(!html.contains("TAJNI-ARTIKAL"), "an unrelated product must not leak");
            assert!(!html.contains("tajni_kasir"), "an employee username must not leak");
        });
    }

    #[test]
    fn evidence_html_escapes_dynamic_values() {
        with_db("render_evidence_escape", |conn| {
            seed_product(conn, 1, "A & <b>", 100000, "2026-01-01T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-01-01T00:00:00Z', 100000, 'create', '2026-01-01T00:00:00Z');
                 INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode, created_at, updated_at)
                 VALUES (1, 'akcijska_prodaja', 'active', '2026-07-17T00:00:00Z', '2026-07-20T00:00:00Z', 'two_prices', '2026-07-16T00:00:00Z', '2026-07-16T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor, prethodna_cena_minor, anchor_status, anchor_window_days)
                 VALUES (1, 1, 80000, 100000, 'computed', 30);",
            ).expect("seed");
            let html = render_evidence_html(&assemble_evidence(conn, 1).expect("assemble"));
            assert!(html.contains("A &amp; &lt;b&gt;"));
            assert!(!html.contains("A & <b>"));
        });
    }
```

- [ ] **Step 2: Run to verify failure** → FAIL. **Step 3: Implement.** **Step 4: Tests** → PASS. **Step 5: Full gates** → PASS.
- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/campaign_evidence.rs
git commit -m "feat(campaigns): evidence HTML renderer with supporting-row trail (SW-6c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Shelf-label HTML renderer

**Files:**
- Modify: `src-tauri/src/campaign_evidence.rs`
- Test: `src-tauri/src/campaign_evidence.rs` (`mod tests`)

**Interfaces:**
- Consumes: `CampaignEvidence` (Task 1); the `escape_html`/`format_rsd_minor` helpers (Task 2).
- Produces: `pub fn render_labels_html(evidence: &CampaignEvidence) -> String`; private `fn discount_percent(anchor: i64, price: i64) -> i64` = `(anchor - price) * 100 / anchor` (integer floor; guard `anchor > 0`, else 0).

Label-card content branches on the campaign:
- **percentage** display (`display_mode == "percentage"`, only reachable for a declared ≤3d akcijska per 6b's H6): show only `-{discount_percent}%` and the article name — **no two prices** (čl. 37 st. 11's substitution). Uses the frozen anchor for the %.
- else **two prices**: `Prethodna cena {anchor, struck-through}` + `Nova cena {campaign_price}` (čl. 37 st. 2).
- rasprodaja: the card carries `dok traju zalihe` where a two_prices card would show the end date; if `reduced_utility_reason` present, print it (čl. 36 st. 3).
- promotivna items (`anchor_status == "none"`): `Uvodna cena {campaign_price}` + `Redovna cena po isteku {future_regular}` — no prethodna cena.
- Grid layout via inline CSS (`display:grid; grid-template-columns:repeat(2,1fr)`); no QR/PIB.

- [ ] **Step 1: Write the failing tests:**

```rust
    #[test]
    fn labels_default_to_two_prices() {
        with_db("labels_two_prices", |conn| {
            // ...seed a two_prices akcijska with computed anchor 100000, price 80000...
            let html = render_labels_html(&assemble_evidence(conn, 1).expect("assemble"));
            assert!(html.contains("Prethodna cena"));
            assert!(html.contains("1.000,00") && html.contains("800,00"));
        });
    }

    #[test]
    fn labels_percentage_only_for_short_akcija_show_no_two_prices() {
        with_db("labels_percentage", |conn| {
            seed_product(conn, 1, "Sok", 10000, "2026-01-01T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-01-01T00:00:00Z', 10000, 'create', '2026-01-01T00:00:00Z');
                 INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode, created_at, updated_at)
                 VALUES (1, 'akcijska_prodaja', 'active', '2026-07-17T00:00:00Z', '2026-07-19T00:00:00Z', 'percentage', '2026-07-16T00:00:00Z', '2026-07-16T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor, prethodna_cena_minor, anchor_status, anchor_window_days)
                 VALUES (1, 1, 8000, 10000, 'computed', 30);",
            ).expect("seed");
            let html = render_labels_html(&assemble_evidence(conn, 1).expect("assemble"));
            assert!(html.contains("-20%"));
            assert!(!html.contains("Prethodna cena"), "percentage label must NOT show two prices");
        });
    }

    #[test]
    fn rasprodaja_labels_say_dok_traju_zaliha_and_show_reason() {
        with_db("labels_rasprodaja", |conn| {
            seed_product(conn, 1, "Sto", 500000, "2026-01-01T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-01-01T00:00:00Z', 500000, 'create', '2026-01-01T00:00:00Z');
                 INSERT INTO campaigns (id, campaign_type, status, starts_on, ends_on, display_mode, rasprodaja_ground, reduced_utility_reason, separation_attested, activated_at, created_at, updated_at)
                 VALUES (1, 'rasprodaja', 'active', '2026-07-01T00:00:00Z', NULL, 'two_prices', 'prestanak_prodaje_robe', 'Oštećena ambalaža', 1, '2026-07-01T00:00:00Z', '2026-06-30T00:00:00Z', '2026-07-01T00:00:00Z');
                 INSERT INTO campaign_items (campaign_id, product_id, campaign_price_minor, prethodna_cena_minor, anchor_status, anchor_window_days)
                 VALUES (1, 1, 400000, 500000, 'computed', 30);",
            ).expect("seed");
            let html = render_labels_html(&assemble_evidence(conn, 1).expect("assemble"));
            assert!(html.contains("dok traju zalihe"));
            assert!(html.contains("Oštećena ambalaža"));
        });
    }

    #[test]
    fn promotivna_labels_show_intro_and_future_price() {
        with_db("labels_promotivna", |conn| {
            // ...seed the (c1) promotivna: intro 890000, future 1090000, anchor_status 'none'...
            let html = render_labels_html(&assemble_evidence(conn, 1).expect("assemble"));
            assert!(html.contains("Uvodna cena") && html.contains("8.900,00"));
            assert!(html.contains("10.900,00"));
            assert!(!html.contains("Prethodna cena"));
        });
    }
```

- [ ] **Step 2: Run to verify failure** → FAIL. **Step 3: Implement.** **Step 4: Tests** → PASS. **Step 5: Full gates** → PASS.
- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/campaign_evidence.rs
git commit -m "feat(campaigns): shelf-label HTML renderer branching on type/display-mode (SW-6c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Correction report — assembly + renderer

**Files:**
- Modify: `src-tauri/src/campaign_evidence.rs`
- Test: `src-tauri/src/campaign_evidence.rs` (`mod tests`)

**Interfaces:**
- Consumes: v10 schema.
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionRow {
    pub campaign_id: i64,
    pub product_id: i64,
    pub product_name: String,
    pub sku: String,
    pub campaign_type: String,
    pub display_mode: String,
    pub campaign_price_minor: i64,
    pub prethodna_cena_minor: Option<i64>,
    pub needs_attention: bool,
    pub attention_reason: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CorrectionReport { pub rows: Vec<CorrectionRow> }

pub fn assemble_correction_report(conn: &rusqlite::Connection) -> Result<CorrectionReport, AppError>
pub fn render_correction_html(report: &CorrectionReport) -> String
```

Semantics: rows for items of campaigns with `status = 'active'` only, joined to `products`, ordered by `campaign_id, campaign_items.id`. `needs_attention = anchor_status == 'manual' OR anchor_truncated == 1 OR (campaign_type is a sniženje type AND anchor_status == 'none')`. `attention_reason`: `Some("Ručni unos")` for manual, `Some("Nepotpuna evidencija")` for truncated, else `None`. The HTML is a `<table>` (article, sku, type, snižena, prethodna, `Napomena`), same escaping/no-fiscal rules, header `Etikete koje treba proveriti`.

- [ ] **Step 1: Write the failing tests** — one active campaign with a clean computed item (attention false) + a manual item (attention true); a draft campaign whose items must be **absent**; assert `assemble_correction_report` row set and that `render_correction_html` contains the manual item's `Ručni unos`.
- [ ] **Step 2: Run to verify failure** → FAIL. **Step 3: Implement.** **Step 4: Tests** → PASS. **Step 5: Full gates** → PASS.
- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/campaign_evidence.rs
git commit -m "feat(campaigns): correction report — active-campaign label data + attention flags (SW-6c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Commands + registration

**Files:**
- Modify: `src-tauri/src/commands/campaigns.rs`, `src-tauri/src/lib.rs`
- Test: `src-tauri/src/commands/campaigns.rs` (`mod tests` if present, else add one mirroring `backup.rs`'s `with_state`/`sign_in_*`)

**Interfaces:**
- Consumes: `campaign_evidence::*`; `reports::ExportedFile`.
- Produces (all `require_admin`):
  - `campaigns_export_evidence(campaign_id: i64) -> ExportedFile`
  - `campaigns_export_labels(campaign_id: i64) -> ExportedFile`
  - `campaigns_correction_report() -> CorrectionReport`
  - `campaigns_export_correction_report() -> ExportedFile`

Shared helper in `commands/campaigns.rs`:

```rust
fn write_export(state: &AppState, file_name: &str, html: &str, row_count: usize) -> Result<crate::commands::reports::ExportedFile, AppError> {
    let export_dir = state.db().path().parent().map_or_else(
        || std::path::Path::new(".").join("exports"),
        |parent| parent.join("exports"),
    );
    std::fs::create_dir_all(&export_dir)?;
    let path = export_dir.join(file_name);
    std::fs::write(&path, html)?;
    Ok(crate::commands::reports::ExportedFile {
        file_name: file_name.to_string(),
        path: path.display().to_string(),
        mime_type: "text/html",
        row_count,
    })
}
```

Command bodies open the connection, assemble, render, write. File names: `format!("dokaz-cene-kampanja-{campaign_id}.html")`, `format!("etikete-kampanja-{campaign_id}.html")`, `"ispravke-etiketa.html"`. Row counts: evidence/labels → `evidence.items.len()`; correction → `report.rows.len()`. Register all four in `lib.rs` after the existing `commands::campaigns::*` entries.

- [ ] **Step 1: Write the failing tests** — a cashier gets `forbidden` from `campaigns_export_evidence` and `campaigns_correction_report`; an admin happy path writes a file whose bytes contain the campaign type marker and whose returned `path` exists on disk. (Seed a campaign via SQL in the test DB; use the module's session fixture.)
- [ ] **Step 2: Run to verify failure** → FAIL. **Step 3: Implement + register.** **Step 4: Tests** → PASS. **Step 5: Full gates** → PASS.
- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/campaigns.rs src-tauri/src/lib.rs
git commit -m "feat(campaigns): evidence/label/correction export commands (SW-6c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: Frontend — service contract + export UI + correction panel

**Files:**
- Modify: `src/services/types.ts`, `src/services/ports.ts`, `src/services/local-adapter.ts`, `src/services/mock-adapter.ts`
- Modify: `src/app/campaigns/CampaignsModule.tsx` (detail export buttons + correction panel)
- Test: `src/services/local-adapter.test.ts`; `src/app/campaigns/CampaignsModule.test.tsx`

**Interfaces:**
- Consumes: the four commands (Task 5); `ExportedFile` (already in types as the reports export shape — reuse it).
- Produces (TS):

```ts
export interface CorrectionRow {
  campaignId: number; productId: number; productName: string; sku: string;
  campaignType: CampaignType; displayMode: CampaignDisplayMode;
  campaignPriceMinor: number; prethodnaCenaMinor: number | null;
  needsAttention: boolean; attentionReason: string | null;
}
export interface CorrectionReport { rows: CorrectionRow[]; }
```

`CampaignsService` gains: `exportEvidence(campaignId): Promise<ExportedFile>`, `exportLabels(campaignId): Promise<ExportedFile>`, `correctionReport(): Promise<CorrectionReport>`, `exportCorrectionReport(): Promise<ExportedFile>`. Local adapter maps 1:1 (`invoke("campaigns_export_evidence", { campaignId })` etc.). Mock adapter returns a stub `ExportedFile`/`CorrectionReport`.

UI:
- On the campaign **detail** (the `selected` view), add an „Dokazi i etikete" group with buttons „Izvezi dokaz o ceni" and „Izvezi etikete" calling `exportEvidence(selected.id)` / `exportLabels(selected.id)`; on success show the saved path via the existing toast pattern (`toast.success("Izvezeno", { description: exported.path })`), on error the error toast. (`toast` from `sonner`, as `ReportsScreen` uses.)
- Add an „Ispravke etiketa" section (button/panel) that calls `correctionReport()` and renders a table (article, type, snižena, prethodna, `Napomena` = attention reason or „—"), plus an „Izvezi" button calling `exportCorrectionReport()`.

- [ ] **Step 1: Failing tests** — local-adapter invoke-name mapping for the four; RTL: clicking „Izvezi dokaz o ceni" calls `exportEvidence` with the campaign id and toasts the path; the correction panel renders a row with its attention note. **Step 2: FAIL → Step 3: implement → Step 4: PASS → Step 5: full gates → Step 6: commit:**

```bash
git add src/services src/app/campaigns
git commit -m "feat(campaigns): evidence/label/correction export UI (SW-6c)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Self-Review Notes

- **Spec coverage:** §1.1 assembly → T1; §1.2 evidence render → T2, labels render → T3, correction render → T4; §1.3 correction assembly → T4; §2 commands → T5; §3 frontend → T6; §4 tests embedded per task (worked examples (b)/(c1) in T1, privacy + escaping in T2, label branching in T3).
- **Privacy test placement:** the "no unrelated product / no employee username" assertion belongs in T2 (the rendered evidence HTML is where a leak would surface). Add to T2: seed an extra product + a user, assert neither appears in `render_evidence_html` output for a single-item campaign.
- **Type consistency:** `SupportingRow`/`EvidenceItem`/`CampaignEvidence`/`CorrectionRow`/`CorrectionReport` defined once (T1/T4), mirrored camelCase in T6; four command names in T5 = invoke names in T6. `ExportedFile` reused from `reports`, not redefined.
- **Read-only:** no task writes to `campaigns`, `campaign_items`, `products`, or `price_history` — assembly is pure SELECT; commands only write files under `exports/`.
- **No boot:** every deliverable is a file write or a pure function; nothing needs the app running. Print fidelity is explicitly out of scope.
