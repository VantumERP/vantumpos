# Campaign Engine (SW-6b) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the campaign/sniženje engine: the closed 4-type campaign entity whose activation drives till prices through the offered-price log, the frozen-at-activation prethodna cena anchor (ZoT čl. 37 st. 5), all hard validations H1–H14, warnings W1–W6, attestations, the perishability flag, and the rasprodaja no-restocking block.

**Architecture:** Migration v10 adds `campaigns` + `campaign_items` + `products.perishable*` and rebuilds `price_history`'s `source` CHECK (v6 precedent) to admit `campaign_start`/`campaign_step`/`campaign_end`. A new domain module `src-tauri/src/campaigns.rs` owns validation, anchor snapshotting, and lifecycle; `commands/campaigns.rs` is a thin admin-gated command layer. Activation/step/end write `products.sale_price_minor` and `price_history` in one transaction — the register already sells at `sale_price_minor`, so till, log, and campaign cannot diverge.

**Tech Stack:** Rust + rusqlite/SQLite, `time` (RFC3339 + date math), Tauri v2 commands; React + TypeScript + shadcn/ui + Vitest/RTL.

## Global Constraints

- **Legal authority is `docs/ZOT-36-37-VERIFIED-RULES.md`**; design is `docs/superpowers/specs/2026-07-17-campaign-engine-design.md`. If code and those documents disagree, STOP and report — never improvise law.
- **DO NOT boot, launch, or run the application** (no `tauri dev`, dev/preview server, or built binary). Verify only via `cargo test` / `cargo build` / `bun run test` / `bun run build` / clippy / fmt. Standing user instruction.
- **Timestamps are RFC3339 only.** Never `datetime('now')` near `price_history` (space sorts before `T` and corrupts window queries). All date math in Rust via `time`; SQL compares RFC3339 strings lexicographically only. Year filters use string ranges (`>= '2026-01-01' AND < '2027-01-01'`), never `strftime`.
- **`price_history` stays append-only** (INSERT via `record_offered_price_change` only). The v10 CHECK rebuild copies rows verbatim **including `id`** — a schema migration, not a data mutation.
- **The anchor freezes at activation:** no code path may change `campaign_items.prethodna_cena_minor` on a campaign whose `activated_at` is set. Draft edits may re-snapshot (a draft is unannounced).
- Money in integer minor units (para): 12.900,00 RSD = `1290000`. Never floats.
- Serbian Latin copy with correct diacritics (šđčćž) — exact strings given below; copy character-for-character.
- Acting user from the session (`require_admin(...)?.id`), never from a client payload.
- Migrations append-only: v10 next; bump count assertion 9 → 10; extend `CORE_TABLES` / `EXPLICIT_INDEXES`.
- No output may affirm legality („this promotion is legal") — warnings only inform; čl. 38 st. 4 can bite even when st. 3 arithmetic is right.
- Every task ends green on: `bun run test`, `bun run build`, `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`.
- Commit trailer on every commit:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  ```

**Spec fix carried by this plan:** the spec's §1 SQL omits `headline_percent` although W3 references it. The schema below includes `headline_percent INTEGER CHECK (headline_percent IS NULL OR (headline_percent BETWEEN 1 AND 99))` on `campaigns`. Deliberate correction, not drift.

---

### Task 1: Migration v10 — campaigns, campaign_items, perishable, price_history CHECK rebuild

**Files:**
- Modify: `src-tauri/src/db/migrations.rs` (append v10 after v9)
- Modify: `src-tauri/src/db/mod.rs` (`CORE_TABLES`, `EXPLICIT_INDEXES`, count assertion at ~line 612)
- Test: `src-tauri/src/db/mod.rs` (`mod tests`)

**Interfaces:**
- Consumes: nothing.
- Produces: tables `campaigns`, `campaign_items`; columns `products.perishable`, `products.perishable_justification`; widened `price_history.source` CHECK admitting `'campaign_start','campaign_step','campaign_end'`. Consumed by every later task.

- [ ] **Step 1: Write the failing tests**

Add to `src-tauri/src/db/mod.rs` `mod tests`:

```rust
    #[test]
    fn migration_v10_creates_campaign_tables_and_perishable_columns() {
        with_test_database("migration_v10_campaigns", |db| {
            let connection = db.open().expect("database should open");

            for table in ["campaigns", "campaign_items"] {
                assert!(
                    schema_object_exists(&connection, "table", table),
                    "expected table {table}"
                );
            }
            for column in ["perishable", "perishable_justification"] {
                let exists: i64 = connection
                    .query_row(
                        "SELECT COUNT(*) FROM pragma_table_info('products') WHERE name = ?1",
                        params![column],
                        |row| row.get(0),
                    )
                    .expect("column metadata should query");
                assert_eq!(exists, 1, "expected products.{column}");
            }

            let campaigns_schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='campaigns'",
                    [],
                    |row| row.get(0),
                )
                .expect("campaigns schema should load");
            for token in [
                "rasprodaja",
                "sezonsko_snizenje",
                "akcijska_prodaja",
                "promotivna_prodaja",
                "headline_percent",
            ] {
                assert!(campaigns_schema.contains(token), "campaigns schema missing {token}");
            }
        });
    }

    #[test]
    fn migration_v10_widens_price_history_sources_and_preserves_rows() {
        with_test_database("migration_v10_price_history_sources", |db| {
            let connection = db.open().expect("database should open");

            // The widened CHECK admits campaign sources.
            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='price_history'",
                    [],
                    |row| row.get(0),
                )
                .expect("price_history schema should load");
            for token in ["campaign_start", "campaign_step", "campaign_end"] {
                assert!(schema.contains(token), "price_history CHECK missing {token}");
            }
            assert!(
                schema_object_exists(&connection, "index", "idx_price_history_product"),
                "rebuild must recreate idx_price_history_product"
            );

            // Rows written pre-rebuild shape survive with identity intact: insert
            // via the legacy sources and via the new ones — both must work.
            connection
                .execute(
                    "INSERT INTO products (id, name, sku, sale_price_minor, purchase_price_minor,
                                           tax_rate_id, minimum_stock_milli, created_at, updated_at)
                     SELECT 901, 'P', 'SKU-V10', 1000, 0, id, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'
                     FROM tax_rates LIMIT 1",
                    [],
                )
                .ok(); // tax rate may not exist on a bare DB; fall back below
            let product_seeded: i64 = connection
                .query_row("SELECT COUNT(*) FROM products WHERE id = 901", [], |row| row.get(0))
                .expect("count");
            if product_seeded == 0 {
                connection
                    .execute_batch(
                        "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                         VALUES (900, 'T', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
                         INSERT INTO products (id, name, sku, sale_price_minor, purchase_price_minor,
                                               tax_rate_id, minimum_stock_milli, created_at, updated_at)
                         VALUES (901, 'P', 'SKU-V10', 1000, 0, 900, 0, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');",
                    )
                    .expect("seed product");
            }
            for source in ["update", "campaign_start", "campaign_step", "campaign_end"] {
                connection
                    .execute(
                        "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                         VALUES (901, '2026-07-01T00:00:00Z', 1000, ?1, '2026-07-01T00:00:00Z')",
                        params![source],
                    )
                    .unwrap_or_else(|error| panic!("source {source} should insert: {error}"));
            }
        });
    }
```

Update the migration-count test: `assert_eq!(migration_count, 9);` → `assert_eq!(migration_count, 10);`.

- [ ] **Step 2: Run to verify failure**

Run: `cargo test --manifest-path src-tauri/Cargo.toml migration_v10 -- --test-threads=1`
Expected: FAIL — tables missing.

- [ ] **Step 3: Append migration v10**

In `src-tauri/src/db/migrations.rs`, after the v9 entry:

```rust
    Migration {
        version: 10,
        name: "campaigns_and_price_history_campaign_sources",
        sql: r#"
CREATE TABLE campaigns (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    campaign_type TEXT NOT NULL CHECK (campaign_type IN ('rasprodaja', 'sezonsko_snizenje', 'akcijska_prodaja', 'promotivna_prodaja')),
    status TEXT NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'active', 'ended', 'cancelled')),
    starts_on TEXT NOT NULL,
    ends_on TEXT,
    display_mode TEXT NOT NULL DEFAULT 'two_prices' CHECK (display_mode IN ('two_prices', 'percentage')),
    headline_percent INTEGER CHECK (headline_percent IS NULL OR (headline_percent BETWEEN 1 AND 99)),
    rasprodaja_ground TEXT CHECK (rasprodaja_ground IN ('prestanak_poslovanja', 'prestanak_u_objektu', 'prestanak_prodaje_robe')),
    special_conditions TEXT,
    reduced_utility_reason TEXT,
    marketing_label TEXT,
    season_attested INTEGER NOT NULL DEFAULT 0 CHECK (season_attested IN (0, 1)),
    separation_attested INTEGER NOT NULL DEFAULT 0 CHECK (separation_attested IN (0, 1)),
    activated_at TEXT,
    ended_at TEXT,
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_campaigns_type_start ON campaigns(campaign_type, starts_on);

CREATE TABLE campaign_items (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    campaign_id INTEGER NOT NULL REFERENCES campaigns(id) ON DELETE CASCADE,
    product_id INTEGER NOT NULL REFERENCES products(id),
    campaign_price_minor INTEGER NOT NULL CHECK (campaign_price_minor >= 0),
    prethodna_cena_minor INTEGER CHECK (prethodna_cena_minor IS NULL OR prethodna_cena_minor >= 0),
    anchor_status TEXT NOT NULL CHECK (anchor_status IN ('computed', 'manual', 'none')),
    anchor_window_days INTEGER,
    anchor_truncated INTEGER NOT NULL DEFAULT 0 CHECK (anchor_truncated IN (0, 1)),
    anchor_reason TEXT,
    anchor_justification TEXT,
    future_regular_price_minor INTEGER CHECK (future_regular_price_minor IS NULL OR future_regular_price_minor >= 0),
    pre_campaign_price_minor INTEGER,
    UNIQUE (campaign_id, product_id)
);
CREATE INDEX idx_campaign_items_campaign ON campaign_items(campaign_id);
CREATE INDEX idx_campaign_items_product ON campaign_items(product_id);

ALTER TABLE products ADD COLUMN perishable INTEGER NOT NULL DEFAULT 0 CHECK (perishable IN (0, 1));
ALTER TABLE products ADD COLUMN perishable_justification TEXT;

CREATE TABLE price_history_next (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    product_id INTEGER NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    effective_from TEXT NOT NULL,
    price_minor INTEGER CHECK (price_minor IS NULL OR price_minor >= 0),
    source TEXT NOT NULL CHECK (source IN ('create', 'update', 'import', 'deactivate', 'reactivate', 'seed', 'campaign_start', 'campaign_step', 'campaign_end')),
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);

INSERT INTO price_history_next (id, product_id, effective_from, price_minor, source, user_id, created_at)
SELECT id, product_id, effective_from, price_minor, source, user_id, created_at FROM price_history;

DROP TABLE price_history;
ALTER TABLE price_history_next RENAME TO price_history;
CREATE INDEX idx_price_history_product ON price_history(product_id, effective_from);
"#,
    },
```

- [ ] **Step 4: Register schema objects**

In `src-tauri/src/db/mod.rs`: add `"campaigns",` and `"campaign_items",` to `CORE_TABLES`; add `"idx_campaigns_type_start",`, `"idx_campaign_items_campaign",`, `"idx_campaign_items_product",` to `EXPLICIT_INDEXES`.

- [ ] **Step 5: Run DB tests, then full gates**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --lib db:: -- --test-threads=1` → PASS.
Then the full gate set → PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/db/migrations.rs src-tauri/src/db/mod.rs
git commit -m "feat(campaigns): migration v10 — campaigns, campaign_items, perishable, campaign price sources (SW-6b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: `campaigns.rs` — types + pure validation rules (H1/H2/H4/H5/H6/H10/H14)

**Files:**
- Create: `src-tauri/src/campaigns.rs`
- Modify: `src-tauri/src/lib.rs` (add `mod campaigns;` after `mod app_error;`, keeping alphabetical order)
- Test: `src-tauri/src/campaigns.rs` (`mod tests`)

**Interfaces:**
- Consumes: `AppError`; `time` crate.
- Produces (relied on by Tasks 3–9 verbatim):

```rust
pub const TYPE_RASPRODAJA: &str = "rasprodaja";
pub const TYPE_SEZONSKO: &str = "sezonsko_snizenje";
pub const TYPE_AKCIJSKA: &str = "akcijska_prodaja";
pub const TYPE_PROMOTIVNA: &str = "promotivna_prodaja";

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignItemInput {
    pub product_id: i64,
    pub campaign_price_minor: i64,
    #[serde(default)] pub manual_prethodna_minor: Option<i64>,
    #[serde(default)] pub anchor_justification: Option<String>,
    #[serde(default)] pub future_regular_price_minor: Option<i64>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignInput {
    pub campaign_type: String,
    pub starts_on: String,              // RFC3339
    #[serde(default)] pub ends_on: Option<String>,
    pub display_mode: String,           // "two_prices" | "percentage"
    #[serde(default)] pub headline_percent: Option<i64>,
    #[serde(default)] pub rasprodaja_ground: Option<String>,
    #[serde(default)] pub special_conditions: Option<String>,
    #[serde(default)] pub reduced_utility_reason: Option<String>,
    #[serde(default)] pub marketing_label: Option<String>,
    #[serde(default)] pub season_attested: bool,
    #[serde(default)] pub separation_attested: bool,
    pub items: Vec<CampaignItemInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Violation {
    pub code: &'static str,
    pub message: String,
    pub product_id: Option<i64>,
}

pub fn validate_shape(input: &CampaignInput) -> Result<Vec<Violation>, AppError>
pub fn declared_duration_days(starts_on: &str, ends_on: &str) -> Result<i64, AppError>  // inclusive: 05.07→02.09 = 60
```

`validate_shape` covers the DB-independent rules. Violation codes are the H-numbers as lowercase strings: `"h1"`, `"h2"`, …

Exact Serbian messages (copy character-for-character):

| code | message |
|---|---|
| h1 | `Vrsta kampanje nije ispravna.` |
| h2 | `Sezonsko sniženje mora početi u periodu 25.12–10.01. ili 01.07–15.07. (čl. 37 st. 8).` |
| h4 | `Sezonsko sniženje može trajati najviše 60 dana (čl. 37 st. 9 u vezi st. 8).` |
| h5 | `Akcijska prodaja može trajati najviše 31 dan (čl. 37 st. 10).` |
| h6 | `Isticanje samo procenta je dozvoljeno isključivo za akcijsku prodaju sa rokom važenja do 3 dana (čl. 37 st. 11).` |
| h6b | `Za isticanje samo procenta unesite jasno određenje procenta sniženja (čl. 37 st. 11).` |
| h7b | `Za promotivnu prodaju unesite redovnu cenu koja će važiti nakon isteka (čl. 36 st. 9).` |
| h7d | `Promotivna prodaja može trajati najviše 60 dana (čl. 36 st. 9).` |
| h10 | `Za rasprodaju izaberite jedan od tri zakonska osnova (čl. 37 st. 6).` |
| h12a | `Potvrdite da je sezona protekla (čl. 37 st. 8).` |
| h12b | `Potvrdite da je roba na rasprodaji fizički izdvojena (čl. 37 st. 7).` |
| h14a | `Datum početka nije ispravan.` |
| h14b | `Datum isteka je obavezan (osim za rasprodaju — „dok traju zalihe") (čl. 36 st. 2 t. 3).` |
| h14c | `Datum isteka mora biti posle datuma početka.` |
| h14d | `Kampanja mora imati bar jedan artikal.` |
| h14e | `Način isticanja nije ispravan.` |

Rules `validate_shape` enforces:
- h1: `campaign_type` ∈ the four constants.
- h14a: `starts_on` parses as RFC3339; h14b: `ends_on` required unless rasprodaja; h14c: when present, `ends_on` ≥ `starts_on`; h14d: `items` non-empty; h14e: `display_mode` ∈ {`two_prices`,`percentage`}.
- h2 (sezonsko only): `starts_on`'s `(month, day)` ∈ [25.12–31.12] ∪ [01.01–10.01] ∪ [01.07–15.07] — the winter window straddles the year boundary, so compare month/day, never a single date range.
- h4 (sezonsko): `declared_duration_days ≤ 60`; h5 (akcijska): `≤ 31`; h7d (promotivna): `≤ 60`.
- h6: `display_mode == "percentage"` requires `campaign_type == akcijska` AND `declared_duration_days ≤ 3`.
- h6b: `display_mode == "percentage"` requires `headline_percent` present. Added after review: čl. 37 st. 11's „već" is adversative — the memo (§2.6) reads it as a SUBSTITUTION („A ≤3-day akcija showing neither is unlawful"), so h6 alone would pass a campaign that displays nothing. Independent of h6: an akcija of a lawful duration still owes the number.
- h10: rasprodaja requires `rasprodaja_ground` ∈ the three; non-rasprodaja must NOT set one.
- h12a: sezonsko requires `season_attested`; h12b: rasprodaja requires `separation_attested`.

`declared_duration_days` = `(end_date - start_date).whole_days() + 1` on the calendar dates (times ignored).

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/campaigns.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn base_input(campaign_type: &str) -> CampaignInput {
        CampaignInput {
            campaign_type: campaign_type.to_string(),
            starts_on: "2026-07-05T00:00:00Z".to_string(),
            ends_on: Some("2026-07-20T00:00:00Z".to_string()),
            display_mode: "two_prices".to_string(),
            headline_percent: None,
            rasprodaja_ground: None,
            special_conditions: None,
            reduced_utility_reason: None,
            marketing_label: None,
            season_attested: true,
            separation_attested: false,
            items: vec![CampaignItemInput {
                product_id: 1,
                campaign_price_minor: 9900,
                manual_prethodna_minor: None,
                anchor_justification: None,
                future_regular_price_minor: None,
            }],
        }
    }

    fn codes(violations: &[Violation]) -> Vec<&'static str> {
        violations.iter().map(|violation| violation.code).collect()
    }

    #[test]
    fn duration_is_inclusive_of_both_endpoints() {
        // Memo worked example (b): 05.07 -> 02.09 is exactly the 60-day cap.
        assert_eq!(
            declared_duration_days("2026-07-05T00:00:00Z", "2026-09-02T00:00:00Z").expect("duration"),
            60
        );
        assert_eq!(
            declared_duration_days("2026-07-05T00:00:00Z", "2026-07-05T00:00:00Z").expect("duration"),
            1
        );
    }

    #[test]
    fn sezonsko_inside_july_window_passes_h2() {
        let input = base_input(TYPE_SEZONSKO);
        let violations = validate_shape(&input).expect("validate");
        assert!(!codes(&violations).contains(&"h2"), "05.07 is inside 01–15.07: {violations:?}");
    }

    #[test]
    fn sezonsko_outside_windows_fails_h2() {
        let mut input = base_input(TYPE_SEZONSKO);
        input.starts_on = "2026-03-01T00:00:00Z".to_string();
        input.ends_on = Some("2026-03-20T00:00:00Z".to_string());
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h2"));
    }

    #[test]
    fn winter_window_straddles_the_year_boundary() {
        let mut december = base_input(TYPE_SEZONSKO);
        december.starts_on = "2026-12-28T00:00:00Z".to_string();
        december.ends_on = Some("2027-01-30T00:00:00Z".to_string());
        assert!(!codes(&validate_shape(&december).expect("validate")).contains(&"h2"));

        let mut january = base_input(TYPE_SEZONSKO);
        january.starts_on = "2026-01-08T00:00:00Z".to_string();
        january.ends_on = Some("2026-02-10T00:00:00Z".to_string());
        assert!(!codes(&validate_shape(&january).expect("validate")).contains(&"h2"));

        let mut late_january = base_input(TYPE_SEZONSKO);
        late_january.starts_on = "2026-01-11T00:00:00Z".to_string();
        late_january.ends_on = Some("2026-02-10T00:00:00Z".to_string());
        assert!(codes(&validate_shape(&late_january).expect("validate")).contains(&"h2"));
    }

    #[test]
    fn sezonsko_over_60_days_fails_h4() {
        let mut input = base_input(TYPE_SEZONSKO);
        input.ends_on = Some("2026-09-03T00:00:00Z".to_string()); // 61 days
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h4"));
    }

    #[test]
    fn akcijska_over_31_days_fails_h5() {
        let mut input = base_input(TYPE_AKCIJSKA);
        input.starts_on = "2026-05-01T00:00:00Z".to_string();
        input.ends_on = Some("2026-06-01T00:00:00Z".to_string()); // 32 days
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h5"));
    }

    #[test]
    fn percentage_display_needs_akcijska_of_three_days_or_less() {
        let mut four_day = base_input(TYPE_AKCIJSKA);
        four_day.starts_on = "2026-05-01T00:00:00Z".to_string();
        four_day.ends_on = Some("2026-05-04T00:00:00Z".to_string()); // 4 days
        four_day.display_mode = "percentage".to_string();
        assert!(codes(&validate_shape(&four_day).expect("validate")).contains(&"h6"));

        let mut three_day = base_input(TYPE_AKCIJSKA);
        three_day.starts_on = "2026-05-01T00:00:00Z".to_string();
        three_day.ends_on = Some("2026-05-03T00:00:00Z".to_string()); // 3 days
        three_day.display_mode = "percentage".to_string();
        assert!(!codes(&validate_shape(&three_day).expect("validate")).contains(&"h6"));

        let mut sezonsko_pct = base_input(TYPE_SEZONSKO);
        sezonsko_pct.display_mode = "percentage".to_string();
        assert!(codes(&validate_shape(&sezonsko_pct).expect("validate")).contains(&"h6"));
    }

    #[test]
    fn promotivna_needs_end_date_within_60_days() {
        let mut input = base_input(TYPE_PROMOTIVNA);
        input.season_attested = false;
        input.ends_on = Some("2026-09-03T00:00:00Z".to_string()); // 61 days
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h7d"));
    }

    #[test]
    fn rasprodaja_requires_a_statutory_ground_and_allows_open_end() {
        let mut input = base_input(TYPE_RASPRODAJA);
        input.season_attested = false;
        input.separation_attested = true;
        input.ends_on = None; // "dok traju zalihe"
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h10"));

        input.rasprodaja_ground = Some("prestanak_prodaje_robe".to_string());
        let violations = validate_shape(&input).expect("validate");
        assert!(!codes(&violations).contains(&"h10"));
        assert!(!codes(&violations).contains(&"h14b"), "rasprodaja may omit ends_on");
    }

    #[test]
    fn non_rasprodaja_missing_end_date_fails_h14b() {
        let mut input = base_input(TYPE_AKCIJSKA);
        input.ends_on = None;
        assert!(codes(&validate_shape(&input).expect("validate")).contains(&"h14b"));
    }

    #[test]
    fn attestations_gate_sezonsko_and_rasprodaja() {
        let mut sezonsko = base_input(TYPE_SEZONSKO);
        sezonsko.season_attested = false;
        assert!(codes(&validate_shape(&sezonsko).expect("validate")).contains(&"h12a"));

        let mut rasprodaja = base_input(TYPE_RASPRODAJA);
        rasprodaja.rasprodaja_ground = Some("prestanak_poslovanja".to_string());
        rasprodaja.ends_on = None;
        rasprodaja.separation_attested = false;
        assert!(codes(&validate_shape(&rasprodaja).expect("validate")).contains(&"h12b"));
    }

    #[test]
    fn unknown_type_and_empty_items_fail() {
        let mut input = base_input("outlet");
        input.items.clear();
        let violations = validate_shape(&input).expect("validate");
        assert!(codes(&violations).contains(&"h1"));
        assert!(codes(&violations).contains(&"h14d"));
    }
}
```

- [ ] **Step 2: Run to verify failure** — `cargo test --manifest-path src-tauri/Cargo.toml campaigns:: -- --test-threads=1` → FAIL (module/functions undefined).

- [ ] **Step 3: Implement**

Write the module above the tests. Doc comment mirrors `price_history.rs`'s (legal authority + design pointers). Implementation notes:
- Parse RFC3339 with the same `parse_rfc3339` shape as `price_history.rs` (private copy here; two small private helpers beat a premature shared module).
- `declared_duration_days`: parse both, take `.date()`, `(end - start).whole_days() + 1`; error `h14c`-style validation if negative (return violation, not `Err`, from `validate_shape`; the function itself returns `AppError` only for unparseable input).
- h2 month/day check: `let (m, d) = (date.month() as u8, date.day());` then `matches!((m, d), (12, 25..=31) | (1, 1..=10) | (7, 1..=15))`.
- Collect ALL violations (no early return) so the wizard shows everything at once.
- Add `#![cfg_attr(not(test), expect(dead_code, reason = "wired up by the campaigns command layer in a later task"))]` at module top — Task 8 removes it (same pattern Task 2/Task 5 of SW-6a used; the remover must delete it or clippy fails on the unfulfilled expectation).
- Declare `mod campaigns;` in `src-tauri/src/lib.rs`.

- [ ] **Step 4: Run tests** → PASS. **Step 5: Full gates** → PASS.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/campaigns.rs src-tauri/src/lib.rs
git commit -m "feat(campaigns): campaign types + pure validation rules H1-H14 (SW-6b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: DB validations + anchor snapshot (H3/H7/H8/H9/H13)

**Files:**
- Modify: `src-tauri/src/campaigns.rs`
- Test: `src-tauri/src/campaigns.rs` (`mod tests`)

**Interfaces:**
- Consumes: Task 2 types; `price_history::{compute_prethodna_cena, PrethodnaCenaResult, IncomputableReason}`; the v10 schema.
- Produces:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemAnchor {
    pub product_id: i64,
    pub anchor_status: &'static str,           // "computed" | "manual" | "none"
    pub prethodna_cena_minor: Option<i64>,
    pub anchor_window_days: Option<i64>,
    pub anchor_truncated: bool,
    pub anchor_reason: Option<String>,
    pub anchor_justification: Option<String>,
}

pub fn snapshot_anchors(conn: &Connection, input: &CampaignInput) -> Result<(Vec<ItemAnchor>, Vec<Violation>), AppError>
pub fn validate_against_db(conn: &Connection, input: &CampaignInput, anchors: &[ItemAnchor], exclude_campaign_id: Option<i64>) -> Result<Vec<Violation>, AppError>
pub fn validate_campaign(conn: &Connection, input: &CampaignInput, exclude_campaign_id: Option<i64>) -> Result<(Vec<ItemAnchor>, Vec<Violation>), AppError>  // shape + snapshot + db, all violations merged
```

Messages:

| code | message |
|---|---|
| h3 | `Sezonsko sniženje je dozvoljeno najviše dva puta godišnje — u {godina}. su već održana dva (čl. 37 st. 8; broji se po datumu početka u kalendarskoj godini).` |
| h7a | `Promotivna prodaja je samo za robu koja se prvi put uvodi u ponudu — artikal je već bio u ponudi (čl. 36 st. 9).` |
| h7c | `Artikal za promotivnu prodaju ne sme biti aktivan pre početka — ponudu započinje aktivacija kampanje.` |
| h9 | `Snižena cena mora biti niža od prethodne cene (čl. 37 st. 6/8/10).` |
| h13a | `Za ovaj artikal prethodna cena mora biti uneta ručno uz obrazloženje.` |
| h13b | `Artikal nije u ponudi — sniženje je moguće samo za aktivne artikle.` |
| h14f | `Artikal nije pronađen.` |

Semantics:
- **snapshot_anchors** — per item, by type: promotivna → `anchor_status="none"`, all None. Sniženje types: load `products.perishable`; if perishable → require `manual_prethodna_minor` + non-empty `anchor_justification` (else violation h13a with `product_id`), `anchor_status="manual"`, `anchor_reason=Some("perishable")`. Else call `compute_prethodna_cena(conn, product_id, &input.starts_on)`: `Computed(v)` → status `computed`, price/window/truncated from `v`; `Incomputable(reason)` → require manual + justification (h13a), `anchor_reason` = `"too_new_in_assortment"`/`"not_offered_in_window"`/`"no_history"`. Missing product → h14f.
- **validate_against_db**:
  - h3 (sezonsko): count activated campaigns in the calendar year of `starts_on`: `SELECT COUNT(*) FROM campaigns WHERE campaign_type='sezonsko_snizenje' AND activated_at IS NOT NULL AND starts_on >= ?1 AND starts_on < ?2 AND id != COALESCE(?3, -1)` with `?1 = "{year}-01-01"`, `?2 = "{year+1}-01-01"` (lexicographic on RFC3339; year computed in Rust). Violation when count ≥ 2.
  - h7a (promotivna, per item): violation if `EXISTS (SELECT 1 FROM price_history WHERE product_id=? AND price_minor IS NOT NULL AND effective_from < ?starts_on)`.
  - h7c (promotivna, per item): violation if `products.active = 1`.
  - h7b (promotivna, per item): `future_regular_price_minor` required (> 0).
  - h13b (sniženje types, per item): violation if `products.active = 0`.
  - h9 (sniženje types, per item): with the item's anchor (computed or manual): violation unless `campaign_price_minor < prethodna_cena_minor`. Skipped when the anchor is itself missing (h13a already fired).
- **validate_campaign** merges: `validate_shape` + `snapshot_anchors` violations + `validate_against_db`.

- [ ] **Step 1: Write the failing tests**

The tests need a real migrated DB. Add a fixture to `mod tests`:

```rust
    use crate::db::{test_database_path, Db};
    use rusqlite::{params, Connection};

    fn with_campaign_db(test_name: &str, test: impl FnOnce(&Connection)) {
        let path = test_database_path(test_name);
        {
            let db = Db::new(&path).expect("db init");
            let connection = db.open().expect("open");
            connection
                .execute_batch(
                    "INSERT INTO tax_rates (id, name, rate_basis_points, created_at, updated_at)
                     VALUES (1, 'PDV 20', 2000, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');",
                )
                .expect("seed tax rate");
            test(&connection);
        }
        std::fs::remove_file(&path).expect("cleanup");
    }

    fn seed_product(conn: &Connection, id: i64, price: i64, active: bool, created_at: &str) {
        conn.execute(
            "INSERT INTO products (id, name, sku, sale_price_minor, purchase_price_minor,
                                   tax_rate_id, minimum_stock_milli, active, created_at, updated_at)
             VALUES (?1, ?2, ?2, ?3, 0, 1, 0, ?4, ?5, ?5)",
            params![id, format!("P{id}"), price, if active { 1 } else { 0 }, created_at],
        )
        .expect("seed product");
        if active {
            conn.execute(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (?1, ?2, ?3, 'create', ?2)",
                params![id, created_at, price],
            )
            .expect("seed history");
        }
    }
```

Tests (all in `mod tests`):

```rust
    #[test]
    fn snapshot_computes_anchor_for_established_item() {
        with_campaign_db("anchor_computed", |conn| {
            seed_product(conn, 1, 1290000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 990000;
            let (anchors, violations) = snapshot_anchors(conn, &input).expect("snapshot");
            assert!(violations.is_empty(), "{violations:?}");
            assert_eq!(anchors[0].anchor_status, "computed");
            assert_eq!(anchors[0].prethodna_cena_minor, Some(1290000));
        });
    }

    #[test]
    fn perishable_item_requires_manual_anchor_with_justification() {
        with_campaign_db("anchor_perishable", |conn| {
            seed_product(conn, 1, 50000, true, "2026-01-01T00:00:00Z");
            conn.execute("UPDATE products SET perishable = 1 WHERE id = 1", [])
                .expect("mark perishable");

            let input = base_input(TYPE_AKCIJSKA);
            let (_, violations) = snapshot_anchors(conn, &input).expect("snapshot");
            assert!(codes(&violations).contains(&"h13a"));

            let mut with_manual = base_input(TYPE_AKCIJSKA);
            with_manual.items[0].manual_prethodna_minor = Some(48000);
            with_manual.items[0].anchor_justification = Some("Cena sa police, rok trajanja 3 dana".to_string());
            let (anchors, violations) = snapshot_anchors(conn, &with_manual).expect("snapshot");
            assert!(violations.is_empty(), "{violations:?}");
            assert_eq!(anchors[0].anchor_status, "manual");
            assert_eq!(anchors[0].prethodna_cena_minor, Some(48000));
            assert_eq!(anchors[0].anchor_reason.as_deref(), Some("perishable"));
        });
    }

    #[test]
    fn below_anchor_rule_rejects_equal_or_higher_price() {
        with_campaign_db("h9_below_anchor", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 1000000; // equal — not below
            let (anchors, _) = snapshot_anchors(conn, &input).expect("snapshot");
            let violations = validate_against_db(conn, &input, &anchors, None).expect("validate");
            assert!(codes(&violations).contains(&"h9"));
        });
    }

    #[test]
    fn seasonal_quota_counts_only_activated_campaigns_in_the_start_year() {
        with_campaign_db("h3_quota", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            // Two ACTIVATED seasonal campaigns in 2026 + one draft + one cancelled.
            conn.execute_batch(
                "INSERT INTO campaigns (campaign_type, status, starts_on, ends_on, season_attested, activated_at, created_at, updated_at)
                 VALUES
                 ('sezonsko_snizenje','ended','2026-01-05T00:00:00Z','2026-02-20T00:00:00Z',1,'2026-01-05T08:00:00Z','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z'),
                 ('sezonsko_snizenje','active','2026-07-01T00:00:00Z','2026-08-25T00:00:00Z',1,'2026-07-01T08:00:00Z','2026-06-20T00:00:00Z','2026-06-20T00:00:00Z'),
                 ('sezonsko_snizenje','draft','2026-07-02T00:00:00Z','2026-08-01T00:00:00Z',1,NULL,'2026-06-20T00:00:00Z','2026-06-20T00:00:00Z'),
                 ('sezonsko_snizenje','cancelled','2026-07-03T00:00:00Z','2026-08-01T00:00:00Z',1,NULL,'2026-06-20T00:00:00Z','2026-06-20T00:00:00Z');",
            )
            .expect("seed campaigns");

            let mut input = base_input(TYPE_SEZONSKO);
            input.items[0].campaign_price_minor = 900000;
            let (anchors, _) = snapshot_anchors(conn, &input).expect("snapshot");
            let violations = validate_against_db(conn, &input, &anchors, None).expect("validate");
            assert!(codes(&violations).contains(&"h3"), "third activated-in-2026 must be blocked");

            // A December start in the same year still counts against 2026;
            // a 2027 January start does not.
            let mut next_year = base_input(TYPE_SEZONSKO);
            next_year.starts_on = "2027-01-05T00:00:00Z".to_string();
            next_year.ends_on = Some("2027-02-10T00:00:00Z".to_string());
            next_year.items[0].campaign_price_minor = 900000;
            let (anchors, _) = snapshot_anchors(conn, &next_year).expect("snapshot");
            let violations = validate_against_db(conn, &next_year, &anchors, None).expect("validate");
            assert!(!codes(&violations).contains(&"h3"));
        });
    }

    #[test]
    fn promotivna_rejects_previously_offered_or_active_items() {
        with_campaign_db("h7_promotivna", |conn| {
            seed_product(conn, 1, 800000, true, "2026-01-01T00:00:00Z");  // offered: has history
            seed_product(conn, 2, 800000, false, "2026-07-01T00:00:00Z"); // never offered, inactive

            let mut input = base_input(TYPE_PROMOTIVNA);
            input.season_attested = false;
            input.items = vec![
                CampaignItemInput { product_id: 1, campaign_price_minor: 700000, manual_prethodna_minor: None, anchor_justification: None, future_regular_price_minor: Some(900000) },
                CampaignItemInput { product_id: 2, campaign_price_minor: 700000, manual_prethodna_minor: None, anchor_justification: None, future_regular_price_minor: Some(900000) },
            ];
            let (anchors, _) = snapshot_anchors(conn, &input).expect("snapshot");
            assert!(anchors.iter().all(|anchor| anchor.anchor_status == "none"));
            let violations = validate_against_db(conn, &input, &anchors, None).expect("validate");
            let product_one: Vec<_> = violations.iter().filter(|violation| violation.product_id == Some(1)).collect();
            assert!(product_one.iter().any(|violation| violation.code == "h7a"));
            assert!(product_one.iter().any(|violation| violation.code == "h7c"));
            assert!(!violations.iter().any(|violation| violation.product_id == Some(2)),
                "never-offered inactive item is exactly what promotivna is for: {violations:?}");
        });
    }

    #[test]
    fn snizenje_on_inactive_item_fails_h13b() {
        with_campaign_db("h13b_inactive", |conn| {
            seed_product(conn, 1, 1000000, false, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 900000;
            let (anchors, _) = snapshot_anchors(conn, &input).expect("snapshot");
            let violations = validate_against_db(conn, &input, &anchors, None).expect("validate");
            assert!(codes(&violations).contains(&"h13b"));
        });
    }
```

- [ ] **Step 2: Run to verify failure** → FAIL (functions undefined).
- [ ] **Step 3: Implement** per the semantics above. Year strings built in Rust: parse `starts_on`, `let year = date.year();`, format `"{year}-01-01"` / `"{}-01-01", year + 1`.
- [ ] **Step 4: Run tests** → PASS. **Step 5: Full gates** → PASS.
- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/campaigns.rs
git commit -m "feat(campaigns): DB validations + anchor snapshot H3/H7/H8/H9/H13 (SW-6b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Persistence — create / get / list / update / cancel (draft lifecycle)

**Files:**
- Modify: `src-tauri/src/campaigns.rs`
- Test: `src-tauri/src/campaigns.rs` (`mod tests`)

**Interfaces:**
- Consumes: Tasks 2–3.
- Produces:

```rust
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignItemView {
    pub product_id: i64,
    pub product_name: String,
    pub sku: String,
    pub campaign_price_minor: i64,
    pub prethodna_cena_minor: Option<i64>,
    pub anchor_status: String,
    pub anchor_window_days: Option<i64>,
    pub anchor_truncated: bool,
    pub anchor_reason: Option<String>,
    pub anchor_justification: Option<String>,
    pub future_regular_price_minor: Option<i64>,
    pub pre_campaign_price_minor: Option<i64>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignView {
    pub id: i64,
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
    pub overdue: bool,
    pub items: Vec<CampaignItemView>,
    pub warnings: Vec<Violation>,          // empty until Task 6 fills it
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CampaignSummary {
    pub id: i64,
    pub campaign_type: String,
    pub status: String,
    pub starts_on: String,
    pub ends_on: Option<String>,
    pub marketing_label: Option<String>,
    pub item_count: i64,
    pub overdue: bool,
}

pub fn create_campaign(conn: &mut Connection, input: &CampaignInput, acting_user_id: i64, now: &str) -> Result<CampaignView, AppError>
pub fn update_campaign(conn: &mut Connection, id: i64, input: &CampaignInput, now: &str) -> Result<CampaignView, AppError>   // draft only; deletes+reinserts items (re-snapshot)
pub fn cancel_campaign(conn: &mut Connection, id: i64, now: &str) -> Result<CampaignView, AppError>                          // draft only
pub fn get_campaign(conn: &Connection, id: i64, now: &str) -> Result<CampaignView, AppError>
pub fn list_campaigns(conn: &Connection, now: &str) -> Result<Vec<CampaignSummary>, AppError>
```

Semantics: `create` runs `validate_campaign`; ANY hard violation → `AppError::validation` with message `Kampanja nije ispravna.` and `details: {"violations": [...]}` (serialize the violations into the details JSON so the wizard can render them). On pass, insert campaign + items with the snapshot in one transaction. `update` requires `status='draft'` (`AppError::business("invalid_state", "Samo nacrt kampanje može da se menja.")` otherwise), re-validates with `exclude_campaign_id = Some(id)`, replaces items wholesale (draft items are not evidence; `price_history` is). `cancel` requires draft. `overdue` = `status == "active" && ends_on.is_some() && now >= ends_on + 1 day`. `ends_on` is the declared `datum isteka` and is **inclusive** — čl. 36 st. 2 t. 3 makes it the last day of the `period važenja`, and `declared_duration_days` counts both endpoints — so a campaign is NOT overdue during the whole of its declared end day; it becomes overdue at the first instant of the following day. Do the `+ 1 day` in Rust via the `time` crate and compare the RFC3339 strings (never `ends_on < now`, which fires a day early and would make the UI order a revert while the promotion is still lawfully running). `list` orders by `starts_on DESC, id DESC`.

- [ ] **Step 1: Write the failing tests**

```rust
    #[test]
    fn create_rejects_hard_violations_with_details() {
        with_campaign_db_mut("create_rejects_mut", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 1000000; // h9
            let error = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z")
                .expect_err("h9 must block create");
            assert_eq!(error.code(), "validation_error");
        });
    }

    #[test]
    fn create_then_get_round_trips_with_frozen_snapshot() {
        with_campaign_db_mut("create_get_round_trip", |conn| {
            seed_product(conn, 1, 1290000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 990000;
            let created = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z").expect("create");
            assert_eq!(created.status, "draft");
            assert_eq!(created.items[0].prethodna_cena_minor, Some(1290000));

            let fetched = get_campaign(conn, created.id, "2026-07-01T00:00:00Z").expect("get");
            assert_eq!(fetched.items.len(), 1);
            assert_eq!(fetched.items[0].anchor_status, "computed");
            assert!(!fetched.overdue);
        });
    }

    #[test]
    fn update_is_draft_only_and_resnapshots() {
        with_campaign_db_mut("update_draft_only", |conn| {
            seed_product(conn, 1, 1290000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 990000;
            let created = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z").expect("create");

            // Cheaper price appears before the draft is edited: re-snapshot must see it.
            conn.execute(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at)
                 VALUES (1, '2026-06-20T00:00:00Z', 1190000, 'update', '2026-06-20T00:00:00Z')",
                [],
            )
            .expect("history row");
            let updated = update_campaign(conn, created.id, &input, "2026-07-02T00:00:00Z").expect("update");
            assert_eq!(updated.items[0].prethodna_cena_minor, Some(1190000), "draft edit re-snapshots");

            conn.execute("UPDATE campaigns SET status = 'active', activated_at = '2026-07-05T00:00:00Z' WHERE id = ?1", params![created.id])
                .expect("force active");
            let error = update_campaign(conn, created.id, &input, "2026-07-06T00:00:00Z")
                .expect_err("active campaign must not be editable");
            assert_eq!(error.code(), "invalid_state");
        });
    }

    #[test]
    fn cancel_is_draft_only_and_overdue_flags_past_end() {
        with_campaign_db_mut("cancel_overdue", |conn| {
            seed_product(conn, 1, 1290000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 990000;
            let created = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z").expect("create");

            conn.execute("UPDATE campaigns SET status='active', activated_at='2026-07-05T00:00:00Z' WHERE id=?1", params![created.id]).expect("force");
            let error = cancel_campaign(conn, created.id, "2026-07-06T00:00:00Z").expect_err("active cannot cancel");
            assert_eq!(error.code(), "invalid_state");

            let view = get_campaign(conn, created.id, "2026-08-01T00:00:00Z").expect("get");
            assert!(view.overdue, "past declared end while active");
            let list = list_campaigns(conn, "2026-08-01T00:00:00Z").expect("list");
            assert!(list[0].overdue);
        });
    }
```

Add the `with_campaign_db_mut` fixture (same as `with_campaign_db` but hands out `&mut Connection`).

- [ ] **Step 2: Run to verify failure** → FAIL.
- [ ] **Step 3: Implement.** Views join `products` for `product_name`/`sku`. `AppError::validation` details: `serde_json::json!({ "violations": violations })`.
- [ ] **Step 4: Tests** → PASS. **Step 5: Full gates** → PASS.
- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/campaigns.rs
git commit -m "feat(campaigns): draft persistence — create/get/list/update/cancel (SW-6b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Lifecycle — activate / adjust_item_price / end (atomic write-through)

**Files:**
- Modify: `src-tauri/src/campaigns.rs`
- Test: `src-tauri/src/campaigns.rs` (`mod tests`)

**Interfaces:**
- Consumes: Tasks 2–4; `price_history::{load_offering_state, record_offered_price_change, OfferingState}`.
- Produces:

```rust
pub struct EndOverride { pub product_id: i64, pub return_price_minor: i64 }   // serde camelCase Deserialize
pub fn activate_campaign(conn: &mut Connection, id: i64, acting_user_id: i64, now: &str) -> Result<CampaignView, AppError>
pub fn adjust_item_price(conn: &mut Connection, campaign_id: i64, product_id: i64, new_price_minor: i64, acting_user_id: i64, now: &str) -> Result<CampaignView, AppError>
pub fn end_campaign(conn: &mut Connection, id: i64, overrides: &[EndOverride], acting_user_id: i64, now: &str) -> Result<CampaignView, AppError>
```

Semantics (each in ONE transaction):
- **activate**: status must be `draft`. Rebuild a `CampaignInput` from the stored rows (items with their stored manual anchors/justifications) and re-run `validate_campaign(conn, &input, Some(id))` — the world may have changed since the draft. On pass, per item: `before = load_offering_state`, store `pre_campaign_price_minor = before.price_minor`, `UPDATE products SET sale_price_minor = campaign_price, active = 1, updated_at = now` (promotivna items go active here; sniženje items already are), `record_offered_price_change(tx, product_id, before, OfferingState { active: true, price_minor: campaign_price }, "campaign_start", Some(acting), now)`. Then `UPDATE campaigns SET status='active', activated_at=now, updated_at=now`. **The stored anchors are NOT recomputed** — the re-validation validates; the snapshot stands (it was re-snapshotted on every draft edit; activation freezes it).
- **adjust_item_price** (markdown step): campaign `active`; item exists. For sniženje types: `new_price < prethodna_cena_minor` (h9 message) — the stored anchor, never recomputed. Write-through: `before = load_offering_state`, `UPDATE products.sale_price_minor`, `record(..., "campaign_step", ...)`, `UPDATE campaign_items.campaign_price_minor = new_price`.
- **end**: campaign `active`. Per item, return price = override if given, else `future_regular_price_minor` for promotivna (its absence is impossible — H7b validated), else `pre_campaign_price_minor`. Write-through with `"campaign_end"`. `UPDATE campaigns SET status='ended', ended_at=now, updated_at=now`.

- [ ] **Step 1: Write the failing tests — memo worked examples (b) and (c1) verbatim**

```rust
    // Memo worked example (b) — ZOT-36-37-VERIFIED-RULES.md §2.7.
    // JAKNA-Z-L applied/offered: 12.900 (15.05–20.06), 11.900 (21.06–30.06),
    // 12.900 (01.07→). Sezonsko starts 05.07.2026: anchor = 11.900, FROZEN
    // across markdown steps 9.900 → 8.900 → 7.500. Both named wrong
    // implementations must be impossible: 12.900 ("price at first reduction")
    // and lazy recompute (which would yield 9.900 at step 2).
    #[test]
    fn worked_example_b_progressive_anchor_snapshots_and_freezes_at_11900() {
        with_campaign_db_mut("worked_example_b", |conn| {
            seed_product(conn, 1, 1290000, true, "2026-05-15T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at) VALUES
                 (1, '2026-06-21T00:00:00Z', 1190000, 'update', '2026-06-21T00:00:00Z'),
                 (1, '2026-07-01T00:00:00Z', 1290000, 'update', '2026-07-01T00:00:00Z');",
            )
            .expect("timeline");

            let mut input = base_input(TYPE_SEZONSKO);
            input.starts_on = "2026-07-05T00:00:00Z".to_string();
            input.ends_on = Some("2026-09-02T00:00:00Z".to_string()); // exactly 60 days
            input.items[0].campaign_price_minor = 990000; // step 1: 9.900

            let created = create_campaign(conn, &input, 1, "2026-07-04T00:00:00Z").expect("create");
            assert_eq!(created.items[0].prethodna_cena_minor, Some(1190000), "anchor is the MIN, not the first-reduction price");

            let activated = activate_campaign(conn, created.id, 1, "2026-07-05T00:00:00Z").expect("activate");
            assert_eq!(activated.status, "active");

            // Till price now 9.900 with campaign_start provenance.
            let (till, source): (i64, String) = conn
                .query_row(
                    "SELECT p.sale_price_minor, ph.source FROM products p
                     JOIN price_history ph ON ph.product_id = p.id
                     WHERE p.id = 1 ORDER BY ph.id DESC LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("till state");
            assert_eq!((till, source.as_str()), (990000, "campaign_start"));

            // Steps 2 and 3: the anchor NEVER moves.
            let after_step2 = adjust_item_price(conn, created.id, 1, 890000, 1, "2026-07-20T00:00:00Z").expect("step 2");
            assert_eq!(after_step2.items[0].prethodna_cena_minor, Some(1190000), "frozen — lazy recompute would say 990000");
            let after_step3 = adjust_item_price(conn, created.id, 1, 750000, 1, "2026-08-05T00:00:00Z").expect("step 3");
            assert_eq!(after_step3.items[0].prethodna_cena_minor, Some(1190000));

            // A step at-or-above the anchor is rejected.
            let error = adjust_item_price(conn, created.id, 1, 1190000, 1, "2026-08-06T00:00:00Z")
                .expect_err("must stay below the anchor");
            assert_eq!(error.code(), "validation_error");

            // End: default return = pre-campaign price (12.900).
            let ended = end_campaign(conn, created.id, &[], 1, "2026-09-02T00:00:00Z").expect("end");
            assert_eq!(ended.status, "ended");
            let (till, source): (i64, String) = conn
                .query_row(
                    "SELECT p.sale_price_minor, ph.source FROM products p
                     JOIN price_history ph ON ph.product_id = p.id
                     WHERE p.id = 1 ORDER BY ph.id DESC LIMIT 1",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("till state");
            assert_eq!((till, source.as_str()), (1290000, "campaign_end"));
        });
    }

    // Memo worked example (c1): genuine first introduction -> promotivna.
    // No anchor, ≤60 days, declared future regular price, first offering IS the
    // activation, expiry transitions to the declared regular price.
    #[test]
    fn worked_example_c1_promotivna_first_offer_and_transition() {
        with_campaign_db_mut("worked_example_c1", |conn| {
            seed_product(conn, 2, 890000, false, "2026-07-01T00:00:00Z"); // inactive, never offered

            let mut input = base_input(TYPE_PROMOTIVNA);
            input.season_attested = false;
            input.starts_on = "2026-07-01T00:00:00Z".to_string();
            input.ends_on = Some("2026-08-29T00:00:00Z".to_string()); // 60 days
            input.items = vec![CampaignItemInput {
                product_id: 2,
                campaign_price_minor: 890000,
                manual_prethodna_minor: None,
                anchor_justification: None,
                future_regular_price_minor: Some(1090000),
            }];

            let created = create_campaign(conn, &input, 1, "2026-06-30T00:00:00Z").expect("create");
            assert_eq!(created.items[0].anchor_status, "none");
            assert_eq!(created.items[0].prethodna_cena_minor, None);

            activate_campaign(conn, created.id, 1, "2026-07-01T00:00:00Z").expect("activate");
            // First offering: product is now active and its FIRST history row is campaign_start.
            let (active, first_source): (i64, String) = conn
                .query_row(
                    "SELECT p.active, (SELECT source FROM price_history WHERE product_id = 2 ORDER BY id LIMIT 1)
                     FROM products p WHERE p.id = 2",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .expect("state");
            assert_eq!((active, first_source.as_str()), (1, "campaign_start"));

            end_campaign(conn, created.id, &[], 1, "2026-08-29T00:00:00Z").expect("end");
            let till: i64 = conn
                .query_row("SELECT sale_price_minor FROM products WHERE id = 2", [], |row| row.get(0))
                .expect("price");
            assert_eq!(till, 1090000, "expiry transitions to the declared regular price");
        });
    }

    #[test]
    fn activation_re_validates_the_changed_world() {
        with_campaign_db_mut("activate_revalidates", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_SEZONSKO);
            input.items[0].campaign_price_minor = 900000;
            let created = create_campaign(conn, &input, 1, "2026-06-01T00:00:00Z").expect("create");

            // Two other seasonal campaigns get ACTIVATED after this draft was written.
            conn.execute_batch(
                "INSERT INTO campaigns (campaign_type, status, starts_on, ends_on, season_attested, activated_at, created_at, updated_at)
                 VALUES
                 ('sezonsko_snizenje','active','2026-07-01T00:00:00Z','2026-08-01T00:00:00Z',1,'2026-07-01T00:00:00Z','2026-06-20T00:00:00Z','2026-06-20T00:00:00Z'),
                 ('sezonsko_snizenje','ended','2026-01-02T00:00:00Z','2026-02-01T00:00:00Z',1,'2026-01-02T00:00:00Z','2026-01-01T00:00:00Z','2026-01-01T00:00:00Z');",
            )
            .expect("seed rivals");

            let error = activate_campaign(conn, created.id, 1, "2026-07-05T00:00:00Z")
                .expect_err("quota is now exhausted — activation must re-validate");
            assert_eq!(error.code(), "validation_error");
        });
    }

    #[test]
    fn activation_is_atomic_across_items() {
        with_campaign_db_mut("activate_atomic", |conn| {
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            seed_product(conn, 2, 800000, true, "2026-01-01T00:00:00Z");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items = vec![
                CampaignItemInput { product_id: 1, campaign_price_minor: 900000, manual_prethodna_minor: None, anchor_justification: None, future_regular_price_minor: None },
                CampaignItemInput { product_id: 2, campaign_price_minor: 700000, manual_prethodna_minor: None, anchor_justification: None, future_regular_price_minor: None },
            ];
            let created = create_campaign(conn, &input, 1, "2026-07-01T00:00:00Z").expect("create");
            // Sabotage item 2 so activation's write-through fails mid-way.
            conn.execute("DELETE FROM products WHERE id = 2", []).expect("sabotage");

            let result = activate_campaign(conn, created.id, 1, "2026-07-05T00:00:00Z");
            assert!(result.is_err(), "missing product must fail activation");

            // NOTHING moved: product 1 keeps its price and no campaign_start rows exist.
            let price: i64 = conn
                .query_row("SELECT sale_price_minor FROM products WHERE id = 1", [], |row| row.get(0))
                .expect("price");
            assert_eq!(price, 1000000);
            let starts: i64 = conn
                .query_row("SELECT COUNT(*) FROM price_history WHERE source = 'campaign_start'", [], |row| row.get(0))
                .expect("count");
            assert_eq!(starts, 0, "activation must be all-or-nothing");
        });
    }
```

- [ ] **Step 2: Run to verify failure** → FAIL.
- [ ] **Step 3: Implement** per the semantics. The anchor-immutability invariant lives here: `adjust_item_price` and `end_campaign` never touch `prethodna_cena_minor`; grep-proof by construction (no UPDATE lists that column outside draft `update_campaign`'s delete+reinsert).
- [ ] **Step 4: Tests** → PASS. **Step 5: Full gates** → PASS.
- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/campaigns.rs
git commit -m "feat(campaigns): lifecycle activate/step/end with atomic price write-through (SW-6b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: Warnings W1–W6

**Files:**
- Modify: `src-tauri/src/campaigns.rs` (add `compute_warnings`; wire into `get_campaign` + a public `validation_report` used by the dry-run command)
- Test: `src-tauri/src/campaigns.rs` (`mod tests`)

**Interfaces:**
- Consumes: Tasks 2–5; `sale_items`/`sales` tables.
- Produces: `pub fn compute_warnings(conn: &Connection, input: &CampaignInput, anchors: &[ItemAnchor], exclude_campaign_id: Option<i64>, now: &str) -> Result<Vec<Violation>, AppError>` and `pub struct ValidationReport { pub hard: Vec<Violation>, pub warnings: Vec<Violation>, pub anchors: Vec<ItemAnchor> }` + `pub fn validation_report(conn: &Connection, input: &CampaignInput, exclude_campaign_id: Option<i64>, now: &str) -> Result<ValidationReport, AppError>`. `CampaignView.warnings` now populated on `get_campaign`.

Codes and exact messages:

| code | message |
|---|---|
| w1 | `Prethodna cena je važila kraće od 3 dana u referentnom periodu — rizik „zanemarljivo kratkog perioda" (čl. 38 st. 4).` |
| w2 | `Artikal je bio u drugoj kampanji koja je počela u poslednjih 30 dana — ponovljene akcije obaraju prethodnu cenu.` |
| w3 | `Istaknuti procenat važi za manje od petine aktivnog asortimana (čl. 38 st. 2).` |
| w4 | `Evidencija cena ne pokriva ceo referentni period — proverite podatke.` |
| w5 | `Na kasi je zabeležena cena niža od izračunate prethodne cene — strože tumačenje „primenjivao" može zahtevati nižu prethodnu cenu.` |
| w6 | `Kampanja je prošla deklarisani datum isteka — završite je i vratite cene.` |

Semantics (per item where applicable, `product_id` set on the violation):
- **w1**: for computed anchors — fetch the product's price intervals overlapping the anchor window (same LEAD timeline as `compute_prethodna_cena`, executed here with rows fetched into Rust), sum the days where `price_minor == anchor` clipped to the window; warn if total < 3 days.
- **w2**: `EXISTS` another campaign (`status != 'cancelled'`, `id != exclude`) containing the product whose `starts_on` ∈ `[input.starts_on − 30d, input.starts_on + 30d]` (bounds computed in Rust, lexicographic compare).
- **w3**: only when `headline_percent` is set — numerator = items whose `(anchor − campaign_price) * 100 / anchor ≥ headline_percent` (integer math); denominator = `SELECT COUNT(*) FROM products WHERE active = 1`; warn when `numerator * 5 < denominator` (strictly less than one fifth). Campaign-level violation (`product_id: None`).
- **w4**: any anchor with `anchor_truncated` (per item).
- **w5**: `SELECT MIN((si.unit_price_minor * si.quantity_milli - si.discount_minor * 1000) / si.quantity_milli) FROM sale_items si JOIN sales s ON s.id = si.sale_id WHERE si.product_id = ?1 AND s.document_type = 'sale' AND s.created_at >= ?window_from AND s.created_at < ?window_to` — warn if below the computed anchor. (Line-level only; the message's „strože tumačenje" phrasing carries the limitation.)
- **w6**: computed on `get_campaign`/`list` from `overdue` (already built in Task 4) — emit the w6 violation into `warnings` when overdue.

- [ ] **Step 1: Write the failing tests** — one triggering + one non-triggering per warning. Representative code (write all six pairs following these exact shapes):

```rust
    #[test]
    fn w1_flags_anchor_in_force_under_three_days() {
        with_campaign_db_mut("w1_token_period", |conn| {
            // 1.000 for months, then 800 for exactly 2 days inside the window, back to 1.000.
            seed_product(conn, 1, 1000000, true, "2026-01-01T00:00:00Z");
            conn.execute_batch(
                "INSERT INTO price_history (product_id, effective_from, price_minor, source, created_at) VALUES
                 (1, '2026-06-20T00:00:00Z', 800000, 'update', '2026-06-20T00:00:00Z'),
                 (1, '2026-06-22T00:00:00Z', 1000000, 'update', '2026-06-22T00:00:00Z');",
            )
            .expect("timeline");
            let mut input = base_input(TYPE_AKCIJSKA);
            input.items[0].campaign_price_minor = 700000;
            let report = validation_report(conn, &input, None, "2026-07-01T00:00:00Z").expect("report");
            assert!(report.hard.is_empty(), "{:?}", report.hard);
            // Anchor is 800000 (the MIN) but held only 2 days -> w1.
            assert!(report.warnings.iter().any(|warning| warning.code == "w1"));
        });
    }

    #[test]
    fn w2_flags_repeat_campaign_within_30_days() { /* seed another non-cancelled campaign with the
        same product starting 10 days earlier; assert w2 present; then move it 40 days back and
        assert w2 absent */ }

    #[test]
    fn w3_flags_headline_percent_under_one_fifth_of_assortment() { /* 6 active products, campaign
        with 1 item at 50% discount, headline_percent = 50 -> 1*5 < 6 -> w3; with 2 of 6 items
        (2*5 >= 6... use 10 products / 1 item vs 2 items) assert presence/absence */ }

    #[test]
    fn w4_flags_truncated_anchor() { /* seed history starting mid-window (later than window_from);
        assert w4 present with product_id */ }

    #[test]
    fn w5_flags_till_price_below_anchor() { /* seed a shift+sale+sale_item (copy the INSERT shapes
        from backup.rs seed_trading_data) with unit_price 700000 inside the window while the
        offered anchor is 1000000; assert w5 present; without the cheap sale absent */ }

    #[test]
    fn w6_appears_on_overdue_campaign_view() { /* create+activate a campaign whose ends_on is past
        `now`; get_campaign(now) -> warnings contain w6 */ }
```

- [ ] **Step 2: Run to verify failure** → FAIL. **Step 3: Implement.** **Step 4: Tests** → PASS. **Step 5: Full gates** → PASS.
- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/campaigns.rs
git commit -m "feat(campaigns): advisory warnings W1-W6 (SW-6b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: Rasprodaja receive-block (H11)

**Files:**
- Modify: `src-tauri/src/commands/inventory.rs` (`apply_inventory_adjustment`, ~line 414)
- Test: `src-tauri/src/commands/inventory.rs` (`mod tests`)

**Interfaces:**
- Consumes: `campaigns`/`campaign_items` tables (Task 1).
- Produces: `inventory_receive` rejects in-rasprodaja SKUs. Nothing else consumed downstream.

Semantics: inside `apply_inventory_adjustment`, only for `InventoryMovementType::Receive`, after `validate_adjustment_request` and before any write:

```rust
    if matches!(movement_type, InventoryMovementType::Receive) {
        let in_active_rasprodaja: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM campaign_items ci
                JOIN campaigns c ON c.id = ci.campaign_id
                WHERE ci.product_id = ?1
                  AND c.campaign_type = 'rasprodaja'
                  AND c.status = 'active'
             )",
            params![request.product_id],
            |row| row.get(0),
        )?;
        if in_active_rasprodaja {
            return Err(AppError::business(
                "rasprodaja_receive_blocked",
                "Artikal je predmet aktivne rasprodaje — prijem nove količine nije dozvoljen do kraja rasprodaje (čl. 37 st. 7).",
            ));
        }
    }
```

(Query runs on `&Connection` before the transaction opens — read-only, no race concern beyond what SQLite serializes anyway. Corrections and write-offs stay allowed: the statute bans *adding new quantities*, not counting or damage.)

- [ ] **Step 1: Write the failing test** — in `inventory.rs` `mod tests`, reuse the module's existing fixture for a product + session, insert an active rasprodaja campaign + item row directly via SQL, call the receive path, assert `error.code() == "rasprodaja_receive_blocked"`; then `UPDATE campaigns SET status='ended'` and assert the same receive succeeds. Also assert a `correction` movement on the blocked SKU still succeeds.
- [ ] **Step 2: Run to verify failure** → FAIL (receive succeeds today).
- [ ] **Step 3: Implement** the block above. **Step 4: Tests** → PASS. **Step 5: Full gates** → PASS.
- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/inventory.rs
git commit -m "feat(campaigns): block receiving stock of in-rasprodaja articles (SW-6b H11)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 8: Command layer + catalog perishable backend

**Files:**
- Create: `src-tauri/src/commands/campaigns.rs`
- Modify: `src-tauri/src/commands/mod.rs` (`pub mod campaigns;`), `src-tauri/src/lib.rs` (register 9 commands), `src-tauri/src/campaigns.rs` (remove the `cfg_attr(expect(dead_code))` scaffold)
- Modify: `src-tauri/src/commands/catalog.rs` (`SaveProductRequest`, `NormalizedProductRequest`, `ProductSummary`, INSERT/UPDATE/SELECT columns)
- Test: `src-tauri/src/commands/campaigns.rs`, `src-tauri/src/commands/catalog.rs`

**Interfaces:**
- Consumes: the whole domain module.
- Produces — commands (all `require_admin`, acting id from the session): `campaigns_list`, `campaigns_get(id)`, `campaigns_validate(input) -> ValidationReport`, `campaigns_create(input)`, `campaigns_update(id, input)`, `campaigns_activate(id)`, `campaigns_adjust_item_price(campaign_id, product_id, new_price_minor)`, `campaigns_end(id, overrides: Vec<EndOverride>)`, `campaigns_cancel(id)`. Catalog: `SaveProductRequest.perishable: bool` (serde default false) + `perishable_justification: Option<String>`; `ProductSummary` gains both.

Command wrapper shape (repeat for all nine; `now` from `crate::clock::utc_now()`):

```rust
#[tauri::command]
pub fn campaigns_create(
    state: State<'_, AppState>,
    input: CampaignInput,
) -> Result<CampaignView, CommandError> {
    let acting = super::auth::require_admin(state.inner())?;
    let mut connection = state.db().open().map_err(CommandError::from)?;
    let now = crate::clock::utc_now()?;
    crate::campaigns::create_campaign(&mut connection, &input, acting.id, &now).map_err(Into::into)
}
```

Catalog: perishable=true requires a non-empty justification (validation message: `Za lako kvarljivu robu unesite obrazloženje.`, field `perishableJustification`); a perishable flag change is NOT a price event (no history row).

- [ ] **Step 1: Write failing tests** — campaigns commands: an admin-gate rejection test per the established pattern (`sign_in_cashier` → `forbidden`) for `campaigns_create` and `campaigns_activate` (module fixture mirroring `backup.rs`'s `with_state`/`sign_in_*`); a happy-path `campaigns_validate` dry-run returning a report without persisting (`COUNT(campaigns) == 0` after). Catalog: round-trip test that `create_product` persists `perishable` + justification and `ProductSummary` returns them; rejection test for perishable-without-justification.
- [ ] **Step 2: Run to verify failure** → FAIL. **Step 3: Implement** (including lib.rs registration after `commands::catalog::catalog_prethodna_cena,` and removal of the dead_code scaffold). **Step 4: Tests** → PASS. **Step 5: Full gates** → PASS.
- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/commands/campaigns.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs src-tauri/src/campaigns.rs src-tauri/src/commands/catalog.rs
git commit -m "feat(campaigns): command layer + catalog perishable field (SW-6b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 9: Frontend service contract

**Files:**
- Modify: `src/services/types.ts`, `src/services/ports.ts`, `src/services/local-adapter.ts`, `src/services/mock-adapter.ts`
- Test: `src/services/local-adapter.test.ts` (invoke-name mapping, following the file's existing pattern)

**Interfaces:**
- Consumes: Task 8 command names + serde camelCase DTOs.
- Produces (TS, consumed by Tasks 10–12):

```ts
export type CampaignType = "rasprodaja" | "sezonsko_snizenje" | "akcijska_prodaja" | "promotivna_prodaja";
export type CampaignStatus = "draft" | "active" | "ended" | "cancelled";
export type CampaignDisplayMode = "two_prices" | "percentage";
export type RasprodajaGround = "prestanak_poslovanja" | "prestanak_u_objektu" | "prestanak_prodaje_robe";

export interface CampaignItemInput {
  productId: number;
  campaignPriceMinor: number;
  manualPrethodnaMinor?: number | null;
  anchorJustification?: string | null;
  futureRegularPriceMinor?: number | null;
}
export interface CampaignInput {
  campaignType: CampaignType;
  startsOn: string;
  endsOn?: string | null;
  displayMode: CampaignDisplayMode;
  headlinePercent?: number | null;
  rasprodajaGround?: RasprodajaGround | null;
  specialConditions?: string | null;
  reducedUtilityReason?: string | null;
  marketingLabel?: string | null;
  seasonAttested: boolean;
  separationAttested: boolean;
  items: CampaignItemInput[];
}
export interface CampaignViolation { code: string; message: string; productId: number | null; }
export interface CampaignItemAnchor {
  productId: number; anchorStatus: "computed" | "manual" | "none";
  prethodnaCenaMinor: number | null; anchorWindowDays: number | null;
  anchorTruncated: boolean; anchorReason: string | null; anchorJustification: string | null;
}
export interface CampaignValidationReport { hard: CampaignViolation[]; warnings: CampaignViolation[]; anchors: CampaignItemAnchor[]; }
export interface CampaignItemView { /* mirror Rust CampaignItemView, camelCase */ }
export interface CampaignView { /* mirror Rust CampaignView */ }
export interface CampaignSummary { /* mirror Rust CampaignSummary */ }
export interface EndCampaignOverride { productId: number; returnPriceMinor: number; }

export interface CampaignsService {
  listCampaigns(): Promise<CampaignSummary[]>;
  getCampaign(id: number): Promise<CampaignView>;
  validateCampaign(input: CampaignInput): Promise<CampaignValidationReport>;
  createCampaign(input: CampaignInput): Promise<CampaignView>;
  updateCampaign(id: number, input: CampaignInput): Promise<CampaignView>;
  activateCampaign(id: number): Promise<CampaignView>;
  adjustItemPrice(campaignId: number, productId: number, newPriceMinor: number): Promise<CampaignView>;
  endCampaign(id: number, overrides: EndCampaignOverride[]): Promise<CampaignView>;
  cancelCampaign(id: number): Promise<CampaignView>;
}
```

`PosServices` gains `campaigns: CampaignsService`. Local adapter maps 1:1 to the nine command names (`invoke("campaigns_create", { input })` etc.). Mock adapter: an in-memory array with the same lifecycle semantics at mock fidelity (create assigns id, validate returns empty report, activate/end flip status) — enough for UI tests. `SaveProductRequest`/`ProductSummary` in types.ts gain `perishable: boolean` + `perishableJustification?: string | null` and every mock product literal gains `perishable: false, perishableJustification: null`.

- [ ] **Step 1: Failing test** (local-adapter mapping per existing pattern) → **Step 2: FAIL** → **Step 3: implement** → **Step 4: PASS** → **Step 5: full gates** → **Step 6: Commit**

```bash
git add src/services
git commit -m "feat(campaigns): frontend service contract (SW-6b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 10: CampaignsModule — list + detail + lifecycle actions

**Files:**
- Create: `src/app/campaigns/CampaignsModule.tsx`
- Create: `src/app/campaigns/CampaignsModule.test.tsx`
- Modify: `src/app/navigation.ts` (add nav item), `src/app/AppShell.tsx` (render branch)

**Interfaces:**
- Consumes: `CampaignsService` (Task 9).
- Produces: `<CampaignsModule services={services} />`; nav id `"campaigns"`. The wizard arrives in Task 11 — this task renders a disabled „Nova kampanja" button placeholder wired up in Task 11.

Content requirements:
- `navigation.ts`: insert after the `receipts` item:
  ```ts
  {
    id: "campaigns",
    label: "Kampanje",
    icon: TagIcon,
    adminOnly: true,
  },
  ```
  (import `TagIcon` from `lucide-react`). `AppShell.tsx`: `if (activeId === "campaigns") { return <CampaignsModule services={services} />; }` following the existing branches.
- List: table of `CampaignSummary` — type label (Serbian: `Rasprodaja` / `Sezonsko sniženje` / `Akcijska prodaja` / `Promotivna prodaja`), status badge (`Nacrt` / `Aktivna` / `Završena` / `Otkazana`), dates (`ends_on == null` renders the literal `dok traju zalihe`), item count, marketing label, and a destructive-styled `Isteklo — vratite cene` badge when `overdue`.
- Detail (Sheet or inline panel, following ReceiptsScreen's pattern): all campaign fields incl. attestations and justifications; items table with campaign price, prethodna cena, anchor status (`Izračunata` / `Ručno uneta` / `—`), window days, truncation marker; the `warnings` list rendered as amber advisory rows (never as blockers, never any „u redu"/legal-OK affirmation).
- Lifecycle buttons by status: draft → `Aktiviraj` + `Otkaži`; active → `Nova cena…` (per-item inline input calling `adjustItemPrice`), `Završi kampanju…` (dialog with a per-item return-price table prefilled from `preCampaignPriceMinor` / `futureRegularPriceMinor`, editable, calling `endCampaign` with overrides). Errors from commands surface via the module's error pattern (reuse `errorMessage` helper as in AppShell).
- Tests (RTL, mock services): list renders statuses and the overdue badge; detail shows anchors and warnings; activate calls the service; end dialog sends overrides; cancel only visible on drafts.

- [ ] Steps: failing tests → verify FAIL → implement → PASS → full gates → commit:

```bash
git add src/app/campaigns src/app/navigation.ts src/app/AppShell.tsx
git commit -m "feat(campaigns): campaigns module — list, detail, lifecycle actions (SW-6b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 11: Campaign wizard (create/edit draft)

**Files:**
- Create: `src/app/campaigns/CampaignWizard.tsx`
- Modify: `src/app/campaigns/CampaignsModule.tsx` (wire `Nova kampanja` + draft `Izmeni`)
- Test: `src/app/campaigns/CampaignWizard.test.tsx`

**Interfaces:**
- Consumes: `CampaignsService.validateCampaign/createCampaign/updateCampaign`; `CatalogService.searchProducts` (existing) for item picking.
- Produces: `<CampaignWizard services={services} campaign={CampaignView | null} onClose={() => void} onSaved={(view) => void} />`.

Structure (single dialog, three sections — not a multi-route wizard):
1. **Type + core**: type select with plain-language legal notes under each option (sezonsko: `Najviše dva puta godišnje, počinje 25.12–10.01. ili 01–15.07, do 60 dana. Brojimo po datumu početka u kalendarskoj godini.`; akcijska: `Do 31 dan. Do 3 dana može samo procenat umesto dve cene.`; promotivna: `Samo roba koja se prvi put uvodi u ponudu; do 60 dana; unosi se buduća redovna cena.`; rasprodaja: `Samo uz zakonski osnov; roba se fizički izdvaja; bez prijema novih količina.`), dates (end optional only for rasprodaja, placeholder `dok traju zalihe`), display mode (percentage option disabled unless akcijska ≤3d declared), headline percent, marketing label, special conditions, reduced-utility reason, ground select (rasprodaja), attestation checkboxes (sezonsko/rasprodaja).
2. **Items**: product search + add; per-row campaign price input (RSD, `parseRsdInput`); an anchor preview column driven by the last `validateCampaign` response (`anchors` + per-item violations): computed → `Prethodna: {formatRsd} (prozor {n} d.)` with truncation marker; manual-required → inline manual price + justification inputs; promotivna → future-regular-price input instead.
3. **Validation summary**: on every meaningful change (debounced), call `validateCampaign`; render `hard` as red blocking rows (Save disabled while any exist) and `warnings` as amber advisory rows with the sentence `Upozorenja ne blokiraju čuvanje.`; Save calls create/update and hands the view back.

Tests: hard violations disable Save and render messages; warnings do not disable Save; percentage option disabled for a 4-day akcijska; manual-anchor inputs appear when the report demands them; promotivna shows the future-price input; save calls `createCampaign` with a correctly-shaped `CampaignInput`.

- [ ] Steps: failing tests → FAIL → implement → PASS → full gates → commit:

```bash
git add src/app/campaigns
git commit -m "feat(campaigns): campaign wizard with live validation (SW-6b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 12: Catalog perishable UI

**Files:**
- Modify: `src/app/catalog/CatalogModule.tsx` (product form)
- Test: `src/app/catalog/CatalogModule.test.tsx`

**Interfaces:**
- Consumes: `SaveProductRequest.perishable`/`perishableJustification` (Task 9).
- Produces: form controls; nothing downstream.

Add to the product form (near `allow_negative_stock`-style toggles): a checkbox `Lako kvarljiva roba / kratak rok trajanja` and, when checked, a required text input `Obrazloženje` with helper copy `Za kvarljivu robu prethodna cena se unosi ručno pri sniženju (čl. 37 st. 3).` Both round-trip through save. Test: toggling shows the justification field; submitting includes both in the request; unchecked sends `perishable: false`.

- [ ] Steps: failing test → FAIL → implement → PASS → full gates → commit:

```bash
git add src/app/catalog
git commit -m "feat(campaigns): catalog perishable flag UI (SW-6b)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Self-Review Notes

- **Spec coverage:** §1 schema (+ the headline_percent fix) → T1; §2 domain/lifecycle → T2–T5; §3 H1–H14 → T2 (pure), T3 (DB), T5 (activation re-validation, step h9), T7 (H11), T8 (perishable backend feeding H13); §4 W1–W6 → T6; §5 commands → T8; §6 UI → T10–T12; §7 tests → embedded per task, worked examples (b)/(c1) verbatim in T5.
- **Type consistency:** `CampaignInput`/`CampaignItemInput`/`Violation`/`ItemAnchor`/`CampaignView`/`CampaignSummary`/`EndOverride` defined once (T2–T5) and mirrored camelCase in T9; command names in T8 = invoke names in T9.
- **H12 placement:** attestations validated in `validate_shape` (T2) — they gate activation because activation re-runs the full validation (T5); the wizard exposes them (T11).
- **Sequencing:** T1 → T2 → T3 → T4 → T5 → T6 → T7 (only needs T1) → T8 → T9 → T10 → T11 → T12. T7 could run any time after T1; kept in slot to keep the chain linear for the sequential executor.
- **H6 "extension" note:** the spec's concern about extending a percentage-labelled akcijska past 3 days is satisfied *structurally* — `update_campaign` is draft-only, so no path extends an ACTIVE campaign's `ends_on` at all. A reviewer should not hunt for an extension handler; overdue-but-active is W6's territory. If an extend-active operation is ever added, H6 re-validation must come with it.
- **No placeholders:** W2/W3/W4/W5/W6 test bodies in T6 are specified as exact scenarios with seeds and assertions to write; all mandated copy strings are verbatim in tables.
