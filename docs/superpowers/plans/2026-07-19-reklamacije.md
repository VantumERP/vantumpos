# Reklamacije Register + Deadline Engine (SW-7) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A consumer-complaint (reklamacija) register whose regime-versioned deadline engine computes the legally-correct answer/resolution dates for both the current (88/2021) and post-cutover (35/2026) regimes, with intake, printable potvrda, and lifecycle tracking.

**Architecture:** Migration v11 adds `reklamacije` (parent, inline PII + the 11 evidencija fields) and `reklamacija_events` (dated lifecycle events). A pure `reklamacije.rs::compute_deadlines` derives all deadlines from the stored events, branched by the persisted `regime`. Thin admin-gated commands in `commands/reklamacije.rs`; potvrda/notice HTML in `reklamacije_docs.rs` (SW-6c renderer shape) opened via the SW-8 print primitive. Frontend: a `ReklamacijeService` + an admin-only module.

**Tech Stack:** Rust + rusqlite + `time` (RFC3339 date math); Tauri v2 commands; React + TypeScript + shadcn/ui + Vitest/RTL.

## Global Constraints

- **Legal authority is `docs/ZZP-REKLAMACIJE-VERIFIED-RULES.md`**; design is `docs/superpowers/specs/2026-07-19-reklamacije-design.md`. If code and those documents disagree, STOP and report — never improvise law. This is a DEADLINE ENGINE shown to shops; a wrong date is the false-assurance harm.
- **DO NOT boot, launch, or run the application** (no `tauri dev`, dev/preview server, or built binary). Verify only via `cargo test` / `cargo build` / `bun run test` / `bun run build` / clippy / fmt. Standing user instruction.
- **Deadlines are DERIVED, never stored.** No column holds `answer_due`/`resolution_due`; `compute_deadlines` computes them from `reklamacije` + `reklamacija_events` on every read.
- **Regime is persisted at creation and never recomputed:** `regime = filed_at < CUTOVER_DATE ? 'old' : 'new'`. `CUTOVER_DATE` is a single named constant, default `2026-08-01`, with a counsel-flag comment (the 1-vs-2-Aug question is unresolved).
- **Calendar days; no weekend/holiday rollover that extends a date.** All date math in Rust via `time`. `today` is passed into pure functions (never `datetime('now')` inside them).
- **Both regimes pause the resolution clock between `consumer_received_answer` and `consumer_responded`.** The difference is on `consumer_responded`: OLD restarts to a fresh full span from that date; NEW resumes (base + elapsed-suspension). The 8-day answer clock never pauses. Silence past 3 days → impasse (`nije_saglasan`), both regimes.
- **New-regime `log_answer` is gated on the 3-part express warning; old-regime is NOT.**
- Serbian Latin copy with correct diacritics (šđčćž) — exact strings from the memo/plan, character-for-character.
- Commands `require_admin` (PII); acting id from the session; each event carries `user_id`.
- Migrations append-only: v11 next; bump the count assertion 10 → 11; extend `CORE_TABLES` / `EXPLICIT_INDEXES`.
- No document resembles a fiscal receipt (no QR/PIB/brojač) — SW-1 rule.
- Every task ends green on: `bun run test`, `bun run build`, `cargo test --manifest-path src-tauri/Cargo.toml -- --test-threads=1`, `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets --all-features --locked -- -D warnings`, `cargo fmt --manifest-path src-tauri/Cargo.toml --check`, `git diff --check`.
- Commit trailer on every commit:
  ```
  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  ```

---

### Task 1: Migration v11 — `reklamacije` + `reklamacija_events`

**Files:**
- Modify: `src-tauri/src/db/migrations.rs` (append v11 after v10)
- Modify: `src-tauri/src/db/mod.rs` (`CORE_TABLES`, `EXPLICIT_INDEXES`, count assertion ~line 728)
- Test: `src-tauri/src/db/mod.rs` (`mod tests`)

**Interfaces:**
- Produces: tables `reklamacije`, `reklamacija_events` (columns per the spec §1). Consumed by every later task.

- [ ] **Step 1: Write the failing test**

```rust
    #[test]
    fn migration_v11_creates_reklamacije_tables() {
        with_test_database("migration_v11_reklamacije", |db| {
            let connection = db.open().expect("database should open");
            for table in ["reklamacije", "reklamacija_events"] {
                assert!(schema_object_exists(&connection, "table", table), "expected {table}");
            }
            let schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='reklamacije'",
                    [],
                    |row| row.get(0),
                )
                .expect("schema");
            for token in ["register_number", "regime", "opsta", "tehnicka", "namestaj", "podnosilac_ime_prezime"] {
                assert!(schema.contains(token), "reklamacije schema missing {token}");
            }
            let events_schema: String = connection
                .query_row(
                    "SELECT sql FROM sqlite_master WHERE type='table' AND name='reklamacija_events'",
                    [],
                    |row| row.get(0),
                )
                .expect("events schema");
            for token in ["answer_given", "consumer_received_answer", "consumer_responded", "extension_granted", "resolved"] {
                assert!(events_schema.contains(token), "events CHECK missing {token}");
            }
        });
    }
```

Update the count test: `assert_eq!(migration_count, 10);` → `assert_eq!(migration_count, 11);`.

- [ ] **Step 2: Run to verify failure** → `cargo test --manifest-path src-tauri/Cargo.toml migration_v11 -- --test-threads=1` → FAIL.

- [ ] **Step 3: Append migration v11** — in `src-tauri/src/db/migrations.rs`, after the v10 entry:

```rust
    Migration {
        version: 11,
        name: "reklamacije_register",
        sql: r#"
CREATE TABLE reklamacije (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    register_number INTEGER NOT NULL UNIQUE,
    regime TEXT NOT NULL CHECK (regime IN ('old', 'new')),
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'answered', 'awaiting_consumer', 'impasse', 'resolved')),
    filed_at TEXT NOT NULL,
    podnosilac_ime_prezime TEXT NOT NULL,
    kontakt TEXT,
    podaci_o_robi TEXT NOT NULL,
    opis_nesaobraznosti TEXT NOT NULL,
    zahtev TEXT NOT NULL,
    roba_kind TEXT NOT NULL DEFAULT 'opsta' CHECK (roba_kind IN ('opsta', 'tehnicka', 'namestaj')),
    datum_izdavanja_potvrde TEXT NOT NULL,
    created_by INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE INDEX idx_reklamacije_status ON reklamacije(status, filed_at);

CREATE TABLE reklamacija_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    reklamacija_id INTEGER NOT NULL REFERENCES reklamacije(id) ON DELETE CASCADE,
    event_type TEXT NOT NULL CHECK (event_type IN
        ('answer_given', 'consumer_received_answer', 'consumer_responded', 'extension_granted', 'resolved')),
    event_date TEXT NOT NULL,
    detail_json TEXT,
    consumer_consent INTEGER NOT NULL DEFAULT 0 CHECK (consumer_consent IN (0, 1)),
    user_id INTEGER REFERENCES users(id),
    created_at TEXT NOT NULL
);
CREATE INDEX idx_reklamacija_events_parent ON reklamacija_events(reklamacija_id, event_date);
"#,
    },
```

- [ ] **Step 4: Register** — add `"reklamacije",` and `"reklamacija_events",` to `CORE_TABLES`; add `"idx_reklamacije_status",` and `"idx_reklamacija_events_parent",` to `EXPLICIT_INDEXES`.

- [ ] **Step 5: DB tests + full gates** → PASS. **Step 6: Commit**

```bash
git add src-tauri/src/db/migrations.rs src-tauri/src/db/mod.rs
git commit -m "feat(reklamacije): migration v11 — register + lifecycle events (SW-7)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 2: Deadline engine — `compute_deadlines` (THE load-bearing task)

**Files:**
- Create: `src-tauri/src/reklamacije.rs`
- Modify: `src-tauri/src/lib.rs` (`mod reklamacije;`, alphabetical — after `mod price_history;`)
- Test: `src-tauri/src/reklamacije.rs` (`mod tests`)

**Interfaces:**
- Consumes: `AppError`; `time`.
- Produces (used verbatim by Tasks 3–6):

```rust
pub const CUTOVER_DATE: &str = "2026-08-01"; // ZZP 35/2026 application. 1-vs-2-Aug UNRESOLVED; earlier = safe. Counsel-flag.
pub const REGIME_OLD: &str = "old";
pub const REGIME_NEW: &str = "new";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadlineEvent { pub event_type: String, pub event_date: String }

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlineState {
    pub answer_due: String,
    pub resolution_due: Option<String>,       // None while paused / at impasse / resolved
    pub clock: String,                        // "running" | "paused" | "impasse" | "resolved"
    pub consumer_window_due: Option<String>,
    pub answer_overdue: bool,
    pub resolution_overdue: bool,
    pub one_extension_used: bool,
}

pub fn regime_for(filed_at: &str) -> Result<&'static str, AppError>;   // date compare vs CUTOVER_DATE
pub fn compute_deadlines(regime: &str, filed_at: &str, roba_kind: &str, events: &[DeadlineEvent], today: &str) -> Result<DeadlineState, AppError>;
```

Algorithm (memo §2, verbatim math):
- `span = if roba_kind ∈ {"tehnicka","namestaj"} {30} else {15}`. `answer_due = filed_at + 8d`. `base = filed_at + span`.
- Find the earliest `consumer_received_answer` (`received`), `consumer_responded` (`responded`), any `answer_given`, `resolved`, and the latest `extension_granted` (`ext` — its `event_date` is the consented new deadline).
- `one_extension_used = ext.is_some()`.
- **If `resolved` present:** `clock="resolved"`, `resolution_due=None`, both overdue `false`, `consumer_window_due=None`. Done (answer_overdue also false).
- Else compute the regime clock's `clock_due: Option<String>`:
  - `consumer_window_due = received.map(|r| r.date + 3d)`.
  - If `received` present and `responded` absent:
    - if `today > received.date + 3d` → `clock="impasse"`, `clock_due=None`.
    - else → `clock="paused"`, `clock_due=None`.
  - Else if `received` present and `responded` present:
    - `clock="running"`;
    - OLD → `clock_due = Some(responded.date + span)` (fresh restart).
    - NEW → `suspension_days = date(responded.date) − date(received.date)`; `clock_due = Some(base + suspension_days)`.
  - Else (`received` absent) → `clock="running"`, `clock_due = Some(base)`.
- **Extension tiebreak (safe = tighter):** `resolution_due = match (clock_due, ext) { (Some(c), Some(e)) => Some(min_date(c, e.date)), (Some(c), None) => Some(c), (None, _) => None }`.
- `answer_overdue = answer_given absent && today > answer_due`.
- `resolution_overdue = resolution_due.is_some() && today > resolution_due.unwrap()`.

Provide private helpers `add_days(rfc3339, i64) -> String`, `days_between(a, b) -> i64` (whole calendar days, on `.date()`), `min_date(a, b) -> String`, `date_gt(a, b) -> bool` (compare `.date()`), all via `time` with a `parse_rfc3339` mirroring `price_history.rs`.

- [ ] **Step 1: Write the failing tests — the memo's worked examples verbatim**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn ev(t: &str, d: &str) -> DeadlineEvent { DeadlineEvent { event_type: t.into(), event_date: d.into() } }

    #[test]
    fn regime_is_selected_at_the_cutover_boundary() {
        assert_eq!(regime_for("2026-07-31T00:00:00Z").unwrap(), REGIME_OLD);
        assert_eq!(regime_for("2026-08-01T00:00:00Z").unwrap(), REGIME_NEW);
    }

    // Memo §2.5 OLD/general/restart.
    #[test]
    fn worked_example_old_general_restart() {
        let events = [
            ev("answer_given", "2026-06-05T00:00:00Z"),
            ev("consumer_received_answer", "2026-06-06T00:00:00Z"),
            ev("consumer_responded", "2026-06-08T00:00:00Z"),
        ];
        let s = compute_deadlines(REGIME_OLD, "2026-06-01T00:00:00Z", "opsta", &events, "2026-06-10T00:00:00Z").unwrap();
        assert_eq!(&s.answer_due[..10], "2026-06-09");
        assert_eq!(s.resolution_due.as_deref().map(|d| &d[..10]), Some("2026-06-23"), "restart = responded + 15");
        assert_eq!(s.clock, "running");
    }

    // Memo §2.5 NEW/tehnička/suspend.
    #[test]
    fn worked_example_new_tehnicka_suspend() {
        let events = [
            ev("answer_given", "2026-08-20T00:00:00Z"),
            ev("consumer_received_answer", "2026-08-21T00:00:00Z"),
            ev("consumer_responded", "2026-08-25T00:00:00Z"),
        ];
        let s = compute_deadlines(REGIME_NEW, "2026-08-15T00:00:00Z", "tehnicka", &events, "2026-08-30T00:00:00Z").unwrap();
        assert_eq!(&s.answer_due[..10], "2026-08-23");
        assert_eq!(s.resolution_due.as_deref().map(|d| &d[..10]), Some("2026-09-18"), "base 09-14 + 4-day suspension");
    }

    #[test]
    fn clock_pauses_between_received_and_response() {
        let events = [ev("answer_given", "2026-08-20T00:00:00Z"), ev("consumer_received_answer", "2026-08-21T00:00:00Z")];
        let s = compute_deadlines(REGIME_NEW, "2026-08-15T00:00:00Z", "opsta", &events, "2026-08-22T00:00:00Z").unwrap();
        assert_eq!(s.clock, "paused");
        assert!(s.resolution_due.is_none());
        assert_eq!(s.consumer_window_due.as_deref().map(|d| &d[..10]), Some("2026-08-24"));
    }

    #[test]
    fn silence_past_three_days_is_impasse() {
        let events = [ev("answer_given", "2026-08-20T00:00:00Z"), ev("consumer_received_answer", "2026-08-21T00:00:00Z")];
        let s = compute_deadlines(REGIME_NEW, "2026-08-15T00:00:00Z", "opsta", &events, "2026-08-26T00:00:00Z").unwrap();
        assert_eq!(s.clock, "impasse");
        assert!(s.resolution_due.is_none());
    }

    #[test]
    fn answer_clock_never_suspends_and_flags_overdue() {
        // No answer given; today is past filed+8.
        let s = compute_deadlines(REGIME_NEW, "2026-08-15T00:00:00Z", "opsta", &[], "2026-08-24T00:00:00Z").unwrap();
        assert!(s.answer_overdue);
        // Answered → not overdue.
        let s2 = compute_deadlines(REGIME_NEW, "2026-08-15T00:00:00Z", "opsta", &[ev("answer_given", "2026-08-22T00:00:00Z")], "2026-08-24T00:00:00Z").unwrap();
        assert!(!s2.answer_overdue);
    }

    #[test]
    fn extension_takes_the_tighter_date() {
        // Running with base 09-14; an extension to 09-10 (earlier) wins (safe = tighter).
        let events = [ev("extension_granted", "2026-09-10T00:00:00Z")];
        let s = compute_deadlines(REGIME_NEW, "2026-08-15T00:00:00Z", "tehnicka", &events, "2026-08-16T00:00:00Z").unwrap();
        assert_eq!(s.resolution_due.as_deref().map(|d| &d[..10]), Some("2026-09-10"));
        assert!(s.one_extension_used);
    }

    #[test]
    fn resolved_clears_the_clock() {
        let s = compute_deadlines(REGIME_OLD, "2026-06-01T00:00:00Z", "opsta", &[ev("resolved", "2026-06-12T00:00:00Z")], "2026-07-01T00:00:00Z").unwrap();
        assert_eq!(s.clock, "resolved");
        assert!(!s.resolution_overdue && !s.answer_overdue);
    }
}
```

- [ ] **Step 2: Run to verify failure** → FAIL. **Step 3: Implement** `compute_deadlines` + helpers + `mod reklamacije;`. **Step 4: Run** `cargo test --manifest-path src-tauri/Cargo.toml reklamacije:: -- --test-threads=1` → PASS (all 8; if a worked example fails, the date math is wrong — fix it, not the test). **Step 5: Full gates** → PASS. **Step 6: Commit**

```bash
git add src-tauri/src/reklamacije.rs src-tauri/src/lib.rs
git commit -m "feat(reklamacije): regime-versioned deadline engine (SW-7)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 3: Intake + register number + views (`create`/`get`/`list`)

**Files:**
- Modify: `src-tauri/src/reklamacije.rs`
- Test: `src-tauri/src/reklamacije.rs` (`mod tests`)

**Interfaces:**
- Consumes: Task 2; the v11 schema.
- Produces:
  ```rust
  pub struct ReklamacijaInput { pub podnosilac_ime_prezime, pub kontakt: Option<String>, pub podaci_o_robi, pub opis_nesaobraznosti, pub zahtev: String, pub roba_kind: String, pub filed_at: String }  // serde camelCase Deserialize
  pub struct EventView { pub event_type, pub event_date: String, pub detail_json: Option<String>, pub consumer_consent: bool }  // Serialize
  pub struct ReklamacijaView { /* all reklamacije columns + regime + events: Vec<EventView> + deadlines: DeadlineState */ }  // Serialize
  pub struct ReklamacijaSummary { pub id, pub register_number: i64, pub regime, pub status, pub podnosilac_ime_prezime, pub filed_at: String, pub answer_due: String, pub resolution_due: Option<String>, pub answer_overdue, pub resolution_overdue: bool, pub purge_eligible: bool }
  pub fn create_reklamacija(conn: &mut Connection, input: &ReklamacijaInput, acting_user_id: i64, now: &str) -> Result<ReklamacijaView, AppError>;
  pub fn get_reklamacija(conn: &Connection, id: i64, today: &str) -> Result<ReklamacijaView, AppError>;
  pub fn list_reklamacije(conn: &Connection, today: &str) -> Result<Vec<ReklamacijaSummary>, AppError>;
  ```

Semantics: `create` — validate non-empty required fields + `roba_kind` ∈ the three + parseable `filed_at`; in ONE transaction compute `regime = regime_for(filed_at)`, allocate `register_number = SELECT COALESCE(MAX(register_number),0)+1`, insert with `datum_izdavanja_potvrde = now`, `status='open'`. `get`/`list` load events and call `compute_deadlines` (status in the summary/view is the stored column, but the view also carries the derived `DeadlineState`). Order `list` by `filed_at DESC, id DESC`. **Retention (spec §6):** both `ReklamacijaView` and `ReklamacijaSummary` carry `purge_eligible: bool` = `add_days(filed_at, 730) ≤ today` (2-year floor; it is an eligibility flag only — nothing auto-deletes).

- [ ] **Step 1: Write failing tests** — `create` persists all fields, assigns `register_number=1` then `2` on a second intake, sets regime from filing date (a `2026-06-01` filing → `old`, `2026-09-01` → `new`); `get` returns the `DeadlineState`; validation rejects empty `podnosilac_ime_prezime`. Use a migrated test DB (mirror the `with_campaign_db_mut` fixture).
- [ ] **Step 2: FAIL → Step 3: implement → Step 4: PASS → Step 5: gates → Step 6: commit:**

```bash
git add src-tauri/src/reklamacije.rs
git commit -m "feat(reklamacije): intake with sequential register number + views (SW-7)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 4: Lifecycle — answer (warning gate) / consumer events / extension / resolve

**Files:**
- Modify: `src-tauri/src/reklamacije.rs`
- Test: `src-tauri/src/reklamacije.rs` (`mod tests`)

**Interfaces:**
- Consumes: Tasks 2–3.
- Produces:
  ```rust
  pub struct AnswerInput { pub answer_text: String, pub warning_duty: Option<String>, pub warning_consequences: Option<String>, pub warning_zastoj: Option<String>, pub event_date: String }
  pub fn log_answer(conn: &mut Connection, id: i64, input: &AnswerInput, acting: i64, now: &str) -> Result<ReklamacijaView, AppError>;
  pub fn log_consumer_received_answer(conn: &mut Connection, id: i64, event_date: &str, acting: i64, now: &str) -> Result<ReklamacijaView, AppError>;
  pub fn log_consumer_response(conn: &mut Connection, id: i64, event_date: &str, acting: i64, now: &str) -> Result<ReklamacijaView, AppError>;
  pub fn grant_extension(conn: &mut Connection, id: i64, new_deadline: &str, consumer_consent: bool, reason: &str, acting: i64, now: &str) -> Result<ReklamacijaView, AppError>;
  pub fn resolve_reklamacija(conn: &mut Connection, id: i64, nacin: &str, event_date: &str, acting: i64, now: &str) -> Result<ReklamacijaView, AppError>;
  ```

Semantics (each appends a `reklamacija_events` row + updates `reklamacije.status`/`updated_at`, in one transaction; load the record for its `regime`):
- `log_answer` — **NEW regime:** reject with `AppError::validation("Za novu reklamaciju odgovor mora sadržati izričito obaveštenje potrošaču o obavezi izjašnjenja, posledicama i zastoju rokova (čl. 63 st. 10).", {field:"warning"})` unless all three warning fields are present and non-empty. **OLD regime:** the warning fields are optional. Store `answer_text` + warnings in `detail_json`; status → `answered`.
- `log_consumer_received_answer` — requires a prior `answer_given`; status → `awaiting_consumer`.
- `log_consumer_response` — requires a prior `consumer_received_answer`; status back to `answered`/running.
- `grant_extension` — reject if an `extension_granted` already exists (`AppError::business("invalid_state","Produženje roka je moguće samo jednom (čl. 55/63 st. 11).")`); reject unless `consumer_consent` (`AppError::validation("Produženje roka zahteva saglasnost potrošača.", {field:"consumerConsent"})`); store `new_deadline` as the event's `event_date`, `consumer_consent=1`, reason in `detail_json`.
- `resolve_reklamacija` — appends `resolved`, status → `resolved`, `nacin` in `detail_json`.

- [ ] **Step 1: Write failing tests** — new-regime `log_answer` without warnings → `validation_error`; with all three → succeeds. Old-regime `log_answer` without warnings → succeeds. Second `grant_extension` → `invalid_state`. `grant_extension` without consent → `validation_error`. A full old-regime cycle (answer→received→responded) yields the memo's 06-23 via `get`. Resolve sets status `resolved`.
- [ ] **Step 2: FAIL → Step 3: implement → Step 4: PASS → Step 5: gates → Step 6: commit:**

```bash
git add src-tauri/src/reklamacije.rs
git commit -m "feat(reklamacije): lifecycle events with new-regime warning gate + one-extension rule (SW-7)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 5: Documents — potvrda + prodajno-mesto notice

**Files:**
- Create: `src-tauri/src/reklamacije_docs.rs`
- Modify: `src-tauri/src/lib.rs` (`mod reklamacije_docs;`)
- Test: `src-tauri/src/reklamacije_docs.rs` (`mod tests`)

**Interfaces:**
- Consumes: `ReklamacijaView` (Task 3).
- Produces: `pub fn render_potvrda_html(view: &ReklamacijaView) -> String`; `pub fn render_notice_html() -> String`. Reuse the SW-6c renderer idioms (`<!doctype html>`, inline `<style>`, HTML-escape all dynamic values via a local `escape_html`, no `<script>`, no QR/PIB/brojač, non-fiscal footer).

Content:
- **potvrda o prijemu** (memo §3): header „Potvrda o prijemu reklamacije", the **register number** prominently, `datum prijema`, filer name, goods, complaint description, requested remedy, and the line „Ova potvrda nije fiskalni dokument." No due-date.
- **notice** (memo §4(b)): „Obaveštenje o načinu i mestu prijema reklamacija" — the statutory display notice for the prodajno mesto; static Serbian text describing how/where complaints are received.

- [ ] **Step 1: Failing tests** — `render_potvrda_html` contains the register number and filer name (escaped), starts `<!doctype html>`, no `<script>`, carries the non-fiscal line; `render_notice_html` contains „Obaveštenje o načinu i mestu prijema reklamacija". Privacy: it contains only this complaint's data.
- [ ] **Step 2: FAIL → Step 3: implement → Step 4: PASS → Step 5: gates → Step 6: commit:**

```bash
git add src-tauri/src/reklamacije_docs.rs src-tauri/src/lib.rs
git commit -m "feat(reklamacije): potvrda o prijemu + prodajno-mesto notice HTML (SW-7)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 6: Command layer + registration

**Files:**
- Create: `src-tauri/src/commands/reklamacije.rs`
- Modify: `src-tauri/src/commands/mod.rs` (`pub mod reklamacije;`), `src-tauri/src/lib.rs` (register commands)
- Test: `src-tauri/src/commands/reklamacije.rs` (`mod tests`)

**Interfaces:**
- Consumes: the domain module + `reklamacije_docs` + `reports::ExportedFile` + the `exports/` write helper pattern.
- Produces (all `require_admin`, acting id from session, `now` from `crate::clock::utc_now()`, `today` = `now`):
  - `reklamacije_list`, `reklamacija_get(id)`, `reklamacija_create(input)`, `reklamacija_log_answer(id, input)`, `reklamacija_consumer_received(id, event_date)`, `reklamacija_consumer_responded(id, event_date)`, `reklamacija_grant_extension(id, new_deadline, consumer_consent, reason, event_date)`, `reklamacija_resolve(id, nacin, event_date)`, `reklamacija_export_potvrda(id) -> ExportedFile`, `reklamacija_export_notice() -> ExportedFile`.

Export commands write the rendered HTML to `exports/` (mirror `campaigns_export_evidence`: `potvrda-reklamacija-{register_number}.html`, `obavestenje-reklamacije.html`, `mime_type "text/html"`). Register all ten in `lib.rs`.

- [ ] **Step 1: Failing tests** — a cashier gets `forbidden` from `reklamacija_create` and `reklamacija_list`; an admin happy path creates a complaint and `reklamacija_export_potvrda` writes a file at the returned path whose bytes contain the register number. (Mirror `commands/campaigns.rs` test fixtures.)
- [ ] **Step 2: FAIL → Step 3: implement + register → Step 4: PASS → Step 5: gates → Step 6: commit:**

```bash
git add src-tauri/src/commands/reklamacije.rs src-tauri/src/commands/mod.rs src-tauri/src/lib.rs
git commit -m "feat(reklamacije): admin-gated command layer + exports (SW-7)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 7: Frontend service contract

**Files:**
- Modify: `src/services/types.ts`, `src/services/ports.ts`, `src/services/local-adapter.ts`, `src/services/mock-adapter.ts`
- Test: `src/services/local-adapter.test.ts`

**Interfaces:**
- Consumes: Task 6 command names + serde camelCase DTOs.
- Produces (TS mirrors of the Rust DTOs — `DeadlineState`, `ReklamacijaView`, `ReklamacijaSummary`, `ReklamacijaInput`, `AnswerInput`) and:

```ts
export interface ReklamacijeService {
  list(): Promise<ReklamacijaSummary[]>;
  get(id: number): Promise<ReklamacijaView>;
  create(input: ReklamacijaInput): Promise<ReklamacijaView>;
  logAnswer(id: number, input: AnswerInput): Promise<ReklamacijaView>;
  consumerReceived(id: number, eventDate: string): Promise<ReklamacijaView>;
  consumerResponded(id: number, eventDate: string): Promise<ReklamacijaView>;
  grantExtension(id: number, newDeadline: string, consumerConsent: boolean, reason: string, eventDate: string): Promise<ReklamacijaView>;
  resolve(id: number, nacin: string, eventDate: string): Promise<ReklamacijaView>;
  exportPotvrda(id: number): Promise<ExportedFile>;
  exportNotice(): Promise<ExportedFile>;
}
```

`PosServices` gains `reklamacije: ReklamacijeService`. Local adapter maps 1:1 to the command names (camelCase args). Mock adapter: an in-memory stub with the same lifecycle at mock fidelity (create assigns id + register number; deadlines a plausible `DeadlineState`). Every `PosServices` construction gains `reklamacije`.

- [ ] Steps: failing adapter-mapping test → FAIL → implement → PASS → full gates → commit:

```bash
git add src/services
git commit -m "feat(reklamacije): frontend service contract (SW-7)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 8: Reklamacije module — list + intake

**Files:**
- Create: `src/app/reklamacije/ReklamacijeModule.tsx`, `src/app/reklamacije/ReklamacijeModule.test.tsx`
- Modify: `src/app/navigation.ts` (admin-only „Reklamacije" nav item, `MessageSquareWarningIcon`), `src/app/AppShell.tsx` (render branch)

**Interfaces:**
- Consumes: `ReklamacijeService`.
- Produces: `<ReklamacijeModule services={services} />`; nav id `"reklamacije"`.

Content: nav item after `receipts`, `adminOnly: true`. List table: register number, filer, status (Serbian labels: `Otvorena`/`Odgovoreno`/`Čeka potrošača`/`Zastoj`/`Rešena`), answer-due + resolution-due with **overdue** emphasis (red when `answerOverdue`/`resolutionOverdue`). An intake form (the evidencija fields + `roba_kind` select with a tooltip that „nameštaj"/„tehnička roba" classification is the shop's legal call, per the memo; filing date defaulting today) calling `create`. Tests (RTL, mock services): list renders overdue emphasis; intake calls `create` with the shaped input.

- [ ] Steps: failing tests → FAIL → implement → PASS → full gates → commit:

```bash
git add src/app/reklamacije src/app/navigation.ts src/app/AppShell.tsx
git commit -m "feat(reklamacije): register module — list + intake (SW-7)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

### Task 9: Detail / timeline + lifecycle actions

**Files:**
- Modify: `src/app/reklamacije/ReklamacijeModule.tsx`
- Test: `src/app/reklamacije/ReklamacijeModule.test.tsx`

**Interfaces:**
- Consumes: `ReklamacijeService` full surface + `PrintService.openForPrint` (SW-8).
- Produces: nothing downstream.

Content: a detail/timeline panel showing the record, the computed `DeadlineState` (answer-due, resolution-due or „pauzirano"/„zastoj" when null, clock status), the event history, and lifecycle actions: „Unesi odgovor" (a form; for **new-regime** complaints the three express-warning fields are shown **pre-filled** with the memo §4(a) template and required; for old-regime they are absent), „Potrošač primio odgovor", „Potrošač se izjasnio", „Produži rok" (date + consent checkbox + reason; disabled once `oneExtensionUsed`), „Reši reklamaciju" (nacin). Plus „Štampaj potvrdu" → `runPrint(() => exportPotvrda(id))` and „Štampaj obaveštenje" → `runPrint(() => exportNotice())`, reusing the SW-8 export-then-open helper. Regime shown as read-only context. Tests: the warning fields appear + are required for a new-regime complaint and absent for old-regime; grant-extension disabled after one; „Štampaj potvrdu" exports then opens.

- [ ] Steps: failing tests → FAIL → implement → PASS → full gates → commit:

```bash
git add src/app/reklamacije
git commit -m "feat(reklamacije): detail timeline + lifecycle actions + print (SW-7)

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>"
```

---

## Self-Review Notes

- **Spec coverage:** §1 schema → T1; §2 engine → T2; §3 intake/potvrda → T3 (+ T5 render, T6 export); §4 lifecycle → T4; §5 texts → T4 (warning gate) + T5 (notice); §6 retention/PII → the `require_admin` gating in T6 (retention eligibility is a derived `filed_at+2y ≤ today` boolean — add it to `ReklamacijaView` in T3 as `purge_eligible` and assert it in a T3 test); §7 penalties → advisory, shown in T9 as context (regime-versioned figures 30k/100k — non-blocking); §8 frontend → T7–T9; §9 testing → worked examples in T2, lifecycle in T4.
- **Retention gap fix:** add `purge_eligible: bool` to `ReklamacijaView`/`ReklamacijaSummary` in Task 3 (computed `filed_at + 2y ≤ today`) with a test that it flips — the spec §6 requires it and no other task carried it.
- **Type consistency:** `DeadlineState`/`DeadlineEvent`/`ReklamacijaInput`/`AnswerInput`/`ReklamacijaView`/`ReklamacijaSummary` defined once (T2–T4), mirrored camelCase in T7; ten command names in T6 = invoke names in T7.
- **Regime never recomputed:** persisted in T3's `create`; `get`/`list` read the stored `regime` and only compute deadlines — no task recomputes regime.
- **No boot:** the deadline engine and renderers are pure; commands write files; UI is RTL-tested. Nothing needs the app running.
